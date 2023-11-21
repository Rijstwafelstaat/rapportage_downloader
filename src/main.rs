use std::path::Path;

use clap::Parser;
use report::Report;
use reqwest::Client;
use tokio::fs;

mod report;

#[derive(Parser)]
struct Args {
    /// The path to the file containing the cookie
    #[arg(short, long)]
    cookie: Box<Path>,

    /// The rapport to download
    #[arg(short, long)]
    report: Report,
}

#[tokio::main]
async fn main() {
    // Parse the arguments
    let args = Args::parse();

    // Create a client
    let client = Client::new();

    // Read the cookie
    let cookie = fs::read_to_string(args.cookie)
        .await
        .expect("Failed to read cookie file")
        .trim()
        .to_owned();

    // Download the latest version of the requested report
    args.report
        .download_latest_version(&client, &cookie)
        .await
        .expect("Failed to download latest report version");
}
