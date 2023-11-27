use clap::Parser;
use rapportage_downloader::{login::login, report::Report};
use reqwest::Client;

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    mail: String,
    #[arg(short, long)]
    password: String,
}

#[tokio::main]
async fn main() {
    // Parse arguments
    let args = Args::parse();

    // Create a cookie storing client
    let client = Client::builder()
        .cookie_store(true)
        .build()
        .expect("Failed to create client");

    // Login to DB Energie
    login(&client, &args.mail, &args.password)
        .await
        .expect("Login failed");

    // Test for all reports whether they can be downloaded
    for report in [
        Report::Aansluitinglijst,
        Report::Belastingcluster,
        Report::Co2,
        Report::Datakwaliteit,
        Report::Gebouwen,
        Report::MeetEnInfra,
        Report::Metadata,
        Report::Meterstanden,
        Report::Mj,
        Report::Tussenmeter,
        Report::Verbruik,
    ] {
        // Print the current report
        println!("{report:?}");

        // Download it and make sure it isn't empty
        assert!(
            !report
                .download_latest_version(&client)
                .await
                .unwrap_or_else(|error| panic!("Failed to request {:?}\n{error:?}", report))
                .1
                .is_empty(),
            "{:?} report is empty!",
            report
        );
    }
}
