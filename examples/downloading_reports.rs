use std::path::PathBuf;

use clap::Parser;
use rapportage_downloader::{login::CookieStore, report::Report};
use tokio::{fs::File, io::AsyncWriteExt};

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    mail: String,
    #[arg(short, long)]
    password: String,
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    // Parse arguments
    let args = Args::parse();

    // Login to DB Energie
    let cookie_store = CookieStore::login(args.mail, args.password)
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

        // Download the report
        let (file_name, data) = report
            .download_latest_version(&cookie_store)
            .await
            .unwrap_or_else(|error| panic!("Failed to request {:?}\n{error:?}", report));

        // Make sure it isn't empty
        assert!(!data.is_empty(), "{:?} report is empty!", report);

        // Save the file if requested
        if let Some(output) = &args.output {
            File::create(output.clone().join(file_name))
                .await
                .unwrap()
                .write_all(&data)
                .await
                .unwrap();
        }
    }
}
