use std::{
    io::{self, Write as _},
    path::PathBuf,
    str::FromStr as _,
};

use clap::{Parser, ValueEnum as _};
use login::CookieStore;
use report::Report;
use reqwest::Client;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt as _,
};

mod login;
mod report;

#[derive(Parser)]
pub struct Args {
    /// The email to use to login at DB Energie
    #[arg(short, long)]
    mail: Option<String>,

    /// The password to use to login at DB Energie
    #[arg(short, long)]
    password: Option<String>,

    /// The rapport to download
    #[arg(short, long)]
    report: Option<Report>,

    /// The directory path or url to write the received message to
    #[arg(short, long)]
    output: Option<String>,
}

impl Args {
    pub fn all(&self) -> bool {
        !(self.mail.is_none()
            || self.password.is_none()
            || self.report.is_none()
            || self.output.is_none())
    }

    pub fn any(&self) -> bool {
        self.mail.is_some()
            || self.password.is_some()
            || self.report.is_some()
            || self.output.is_some()
    }
}

/// Saves the data to a server or file
async fn save(client: &Client, output: &str, data: Vec<u8>, filename: String) {
    if let Ok(url) = url::Url::from_str(output) {
        // Store the report in a form
        let form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(data).file_name(filename),
        );

        // Send the file to the requested url
        client
            .post(url)
            .multipart(form)
            .send()
            .await
            .expect("Failed to send file");
    } else if let Ok(directory) = PathBuf::from_str(output) {
        // Create the requested directory
        fs::create_dir_all(&directory)
            .await
            .expect("Failed to create the requested directory");

        // Create the file
        let mut file = File::create(directory.join(filename))
            .await
            .expect("Failed to create file");

        // Write the report to the file
        file.write_all(&data)
            .await
            .expect("Failed to write report to file");
    } else {
        // The output should be a url or filepath, so panic if it is neither
        panic!("Passed output isn't a valid url or filepath");
    }
}

fn read_line(request: &str) -> String {
    // Write the request to the screen
    io::stdout()
        .write_all(request.as_bytes())
        .expect("Failed to write output");
    io::stdout().flush().expect("Failed to flush output");

    // Read the data from the screen
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .expect("Failed to read mail");

    // Trim and return the data
    line.trim().to_owned()
}

fn unpack_arguments(args: Args) -> (String, String, Report, String) {
    if args.any() && !args.all() {
        // Check all missing arguments
        let mut output = "Missing arguments:".to_owned();
        if args.mail.is_none() {
            output.push_str(" -m [mail]");
        }
        if args.password.is_none() {
            output.push_str(" -p [password]");
        }
        if args.report.is_none() {
            output.push_str(" -r [report]");
        }

        // Also add the output criteria when the output is missing
        if args.output.is_none() {
            output
                .push_str(" -o [output]\n\nOutput path must be either an url or a directory path.");
        }

        // Add the possible report types, if the report type was missing
        if args.report.is_none() {
            output.push_str("\nThe report must be either aansluitinglijst, belastingcluster, co2, datakwaliteit, gebouwen, meet-en-infra, metadata, meterstanden, mj, tussenmeter, verbruik.");
        }

        // Make sure the user knows about the help option
        output.push_str("\nFor more information, try '--help'\n");

        // Print output
        io::stdout()
            .write_all(output.as_bytes())
            .expect("Failed to write output");

        // Exit the process with an error code
        std::process::exit(-1);
    }

    // Unpack the arguments
    let mail = args.mail.unwrap_or_else(|| read_line("Enter mail: "));
    let password = args
        .password
        .unwrap_or_else(|| read_line("Enter password: "));
    let report = args.report.unwrap_or_else(|| {
        Report::from_str(&read_line("Report types: aansluitinglijst, belastingcluster, co2, datakwaliteit, gebouwen, meet-en-infra, metadata, meterstanden, mj, tussenmeter, verbruik\nEnter report type: "), true).expect("Invalid report type")
    });
    let output = args
        .output
        .unwrap_or_else(|| read_line("Enter the output path: "));
    (mail, password, report, output)
}

#[tokio::main]
async fn main() {
    // Parse the arguments
    let args = Args::parse();

    // Unpack the arguments, read them from the terminal if None
    let (mail, password, report, output) = unpack_arguments(args);

    // Create a client
    let client = Client::builder()
        .cookie_store(true)
        .build()
        .expect("Failed to create client");

    // Log in to receive a cookie
    let cookie_store = CookieStore::login(&mail, &password)
        .await
        .expect("Login failed");

    // Download the latest version of the requested report
    let (filename, response) = report
        .download_latest_version(&cookie_store)
        .await
        .expect("Failed to download latest report version");

    if output.is_empty() {
        println!(
            "{}",
            core::str::from_utf8(&response).expect("Response wasn't valid utf-8")
        );
    } else {
        // Save the response to a server or file
        save(&client, &output, response, filename).await;
    }
}
