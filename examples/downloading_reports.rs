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
    let args = Args::parse();
    let client = Client::builder()
        .cookie_store(true)
        .build()
        .expect("Failed to create client");
    login(&client, &args.mail, &args.password)
        .await
        .expect("Login failed");
    for report in [
        Report::Aansluitinglijst,
        Report::Belastingcluster,
        Report::Gebouwen,
        Report::MeetEnInfra,
        Report::Metadata,
        Report::Meterstanden,
        Report::Tussenmeter,
    ] {
        println!("{report:?}");
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
