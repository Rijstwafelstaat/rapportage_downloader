use std::{path::PathBuf, str::FromStr};

use clap::Parser;
use report::Report;
use reqwest::Client;
use tokio::{fs::File, io::AsyncWriteExt};

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

    /// The path or url to write the received message to
    #[arg(short, long)]
    output: String,
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
    let (path, response) = args
        .report
        .download_latest_version(&client)
        .await
        .expect("Failed to download latest report version");

    if let Ok(url) = url::Url::from_str(&args.output) {
        let form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(response).file_name(path),
        );
        client
            .post(url)
            .multipart(form)
            .send()
            .await
            .expect("Failed to send file");
    } else if let Ok(filepath) = PathBuf::from_str(&args.output) {
        let mut file = File::create(filepath).await.expect("Failed to create file");
        file.write_all(&response)
            .await
            .expect("Failed to write report to file");
    } else {
        panic!("Passed output isn't a valid url or filepath");
    }
}
