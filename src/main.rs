use std::{fmt::Display, io, path::PathBuf, str::FromStr as _, time::Duration};

use calamine::{Reader, Xlsx};
use chrono::Datelike;
use clap::Parser;
use rapportage_downloader::{
    ean::Ean,
    id::Id,
    login::CookieStore,
    report::{self, Report},
};
use reqwest::Client;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt as _,
    join,
    sync::mpsc,
};

const MINIMUM_DURATION: Duration = Duration::from_millis(1);

#[derive(Parser)]
pub struct Args {
    /// The email to use to login at DB Energie
    #[arg(short, long)]
    mail: String,

    /// The password to use to login at DB Energie
    #[arg(short, long)]
    password: String,

    /// The directory path or url to write the received message to
    #[arg(short, long)]
    output: String,
}

#[derive(Debug, thiserror::Error)]
enum MainError {
    Request(#[from] reqwest::Error),
    Io(#[from] io::Error),
    Report(#[from] report::Error),
    Xlsx(#[from] calamine::XlsxError),
    #[allow(dead_code)]
    ValueMissing(&'static str),
}

impl Display for MainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Saves the data to a server or file
async fn save(
    client: &Client,
    output: &str,
    data: Vec<u8>,
    filename: String,
) -> Result<(), MainError> {
    if let Ok(url) = url::Url::from_str(output) {
        // Store the report in a form
        let form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(data).file_name(filename),
        );

        // Send the file to the requested url
        client.post(url).multipart(form).send().await?;
    } else if let Ok(directory) = PathBuf::from_str(output) {
        // Create the requested directory
        fs::create_dir_all(&directory).await?;

        // Create the file
        let mut file = File::create(directory.join(filename)).await?;

        // Write the report to the file
        file.write_all(&data).await?;
    } else {
        // The output should be a url or filepath, so panic if it is neither
        panic!("Passed output isn't a valid url or filepath");
    }
    Ok(())
}

async fn read_eans(cookie_store: &CookieStore) -> Result<Vec<Ean>, MainError> {
    // Download the latest aansluitingen report
    let (file_name, aansluitingen) = Report::Aansluitinglijst
        .download_latest_version(cookie_store)
        .await?;

    // Save the report
    let mut file = File::create(&file_name).await?;
    file.write_all(&aansluitingen).await?;
    file.flush().await?;

    // Open the list
    let mut workbook: Xlsx<_> = calamine::open_workbook(file_name)?;
    let range = workbook.worksheet_range("Lijst_Export")?;
    let mut rows = range.rows();

    // Take the first row, only store the ean code and status
    let columns = rows
        .next()
        .ok_or(MainError::ValueMissing("Empty worksheet"))?
        .iter()
        .enumerate()
        .filter_map(|(index, column)| {
            let column = column.to_string();
            if ["EAN code", "Status"].contains(&column.trim()) {
                Some((index, column))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    // Take the ean code of every active row
    Ok(rows
        .filter_map(|row| {
            let mut ean = None;
            for (index, column) in &columns {
                let value = &row[*index];
                match column.trim() {
                    "EAN code" => ean = Some(Ean::from(value.to_string())),
                    "Status" if value.to_string().trim() != "Actief" => return None,
                    _ => {}
                }
            }
            ean.filter(|ean| !ean.value().trim().is_empty())
        })
        .collect())
}

async fn load_ids<I: IntoIterator<Item = Ean> + std::marker::Send>(
    eans: I,
    id_tx: mpsc::Sender<(Ean, Id)>,
    cookie_store: CookieStore,
) {
    // Set an initial delay
    let mut sleep_time = MINIMUM_DURATION;
    for ean in eans {
        loop {
            // Wait before downloading
            tokio::time::sleep(sleep_time).await;
            eprintln!("Id download delay: {}", sleep_time.as_secs_f64());
            sleep_time *= 2;

            // Retrieve the id of the meter
            let meter_id = match Id::from_ean(&cookie_store, &ean).await {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("Failed to receive id.\n{e}\n");
                    continue;
                }
            };

            // Retrieve the ean corresponding to received ID
            let received_ean = match Ean::from_id(&cookie_store, meter_id).await {
                Ok(ean) => ean,
                Err(e) => {
                    eprintln!("Failed to check ean and date range\n{e}\n");
                    continue;
                }
            };

            // If the received ean is not the same as the current ean, print an error and try again
            if received_ean != ean {
                eprintln!(
                    "Received ean is not the same as requested ean!\nExpected: {ean}\nReceived: {received_ean}\nId: {meter_id}\n"
                );
                continue;
            }

            // Print the ean and id match
            println!("{ean}: {meter_id}");

            // Send the id for it to be downloaded
            while id_tx.send((ean.clone(), meter_id)).await.is_err() {
                tokio::time::sleep(sleep_time).await;
            }

            // Decrease delay
            sleep_time = (sleep_time / 4).max(MINIMUM_DURATION);
            break;
        }
    }
}

fn receive_id<T>(rx: &mut mpsc::Receiver<T>, sender_closed: &mut bool) -> Option<T> {
    // We're done receiving ids, if the sender is closed
    if *sender_closed {
        return None;
    }

    // Try to receive the next ID
    match rx.try_recv() {
        Ok(value) => Some(value),
        Err(e) => match e {
            mpsc::error::TryRecvError::Empty => None,

            // Store that the sender has disconnected
            mpsc::error::TryRecvError::Disconnected => {
                *sender_closed = true;
                None
            }
        },
    }
}

async fn download_reports(
    mut ids: Vec<(Ean, Id)>,
    cookie_store: &CookieStore,
    output: &str,
    mut rx: mpsc::Receiver<(Ean, Id)>,
) {
    // Initialize an index
    let mut i = 0;

    // a boolean for checking whether the sender has disconnected
    let mut sender_closed = false;

    // And an initial delay
    let mut sleep_time = MINIMUM_DURATION;
    loop {
        // Wait before sending a request
        tokio::time::sleep(sleep_time).await;

        // Load available ids
        while let Some(id) = receive_id(&mut rx, &mut sender_closed) {
            ids.push(id);
        }

        // Increase delay
        eprintln!("Report download delay: {}", sleep_time.as_secs_f64());
        sleep_time *= 2;

        // Make sure there is an id available for download.
        // Reset the index to 0, if the last ID has been reached and the sender has disconnected
        if i >= ids.len() && !sender_closed {
            continue;
        } else if i >= ids.len() {
            i = 0;
        }

        // Download the latest energy usage report for the last year
        let today = chrono::Local::now().date_naive();
        let (file_name, report) = match Report::EnergieVerbruikPerUur(
            ids[i].1,
            today.with_year(today.year() - 1).unwrap(),
            today,
        )
        .download_latest_version(cookie_store)
        .await
        {
            Ok((file_name, report)) => (file_name, report),
            Err(e) => {
                eprintln!("Failed to download report for ean {}\n{e}\n", ids[i].0);
                cookie_store.redo_login().await.ok();
                continue;
            }
        };

        // Save the file
        save(
            cookie_store.client(),
            output,
            report,
            format!("{}_{file_name}", ids[i].0),
        )
        .await
        .ok();

        eprintln!("Saved the report for ean {}", ids[i].0);
        i += 1;
        sleep_time = (sleep_time / 4).max(MINIMUM_DURATION);
    }
}

#[tokio::main]
async fn main() {
    // Parse the arguments
    let args = Args::parse();

    // Log in to receive a cookie
    eprintln!("Logging in");
    let cookie_store = CookieStore::login(args.mail, args.password)
        .await
        .expect("Login failed");

    // Read the eans
    eprintln!("Reading eans");
    let eans = read_eans(&cookie_store)
        .await
        .expect("Failed to read ean codes");

    // Create a channel and vector for eans and ids
    let (tx, rx) = mpsc::channel(10);
    let ids = Vec::with_capacity(eans.len());

    // Download the ids amd reports
    eprintln!("Loading ids and reports");
    let id_loader = tokio::spawn(load_ids(eans, tx, cookie_store.clone()));
    let report_loader = download_reports(ids, &cookie_store, &args.output, rx);
    let (join_result, ()) = join!(id_loader, report_loader);
    join_result.unwrap();
}
