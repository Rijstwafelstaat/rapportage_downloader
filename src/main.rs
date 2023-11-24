use clap::Parser;
use report::Report;
use reqwest::Client;

mod login;
mod report;

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    mail: String,

    #[arg(short, long)]
    password: String,

    /// The rapport to download
    #[arg(short, long)]
    report: Report,
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
    args.report
        .download_latest_version(&client)
        .await
        .expect("Failed to download latest report version");
}
