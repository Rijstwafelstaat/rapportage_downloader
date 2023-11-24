use std::{path::PathBuf, str::FromStr};

use clap::Parser;
use report::Report;
use reqwest::Client;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};

mod login;
mod report;

#[derive(Parser)]
pub struct Args {
    /// The email to use to login at DB Energie
    #[arg(short, long)]
    mail: String,

    /// The password to use to login at DB Energie
    #[arg(short, long)]
    password: String,

    /// The rapport to download
    #[arg(short, long)]
    report: Report,

    /// The directory path or url to write the received message to
    #[arg(short, long)]
    output: String,
}

/// Saves the data to a server or file
async fn save(client: &Client, args: &Args, data: Vec<u8>, filename: String) {
    if let Ok(url) = url::Url::from_str(&args.output) {
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
    } else if let Ok(directory) = PathBuf::from_str(&args.output) {
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

#[tokio::main]
async fn main() {
    // Parse the arguments
    let args = Args::parse();

    // Create a client
    let client = Client::builder()
        .cookie_store(true)
        .build()
        .expect("Failed to create client");

    // Log in to receive a cookie
    login::login(&client, &args).await.expect("Login failed");

    // Download the latest version of the requested report
    let (filename, response) = args
        .report
        .download_latest_version(&client)
        .await
        .expect("Failed to download latest report version");

    // Save the response to a server or file
    save(&client, &args, response, filename).await;
}
