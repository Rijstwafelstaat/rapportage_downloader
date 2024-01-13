use std::{
    fmt::Display, io, num::ParseIntError, path::PathBuf, str::FromStr as _, string::FromUtf8Error,
    time::Duration,
};

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
use scraper::error::SelectorErrorKind;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt as _,
    join,
    sync::mpsc,
};

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
    Utf8(#[from] FromUtf8Error),
    Selector(#[from] SelectorErrorKind<'static>),
    UrlParse(#[from] url::ParseError),
    ParseInt(#[from] ParseIntError),
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
    let (file_name, aansluitingen) = Report::Aansluitinglijst
        .download_latest_version(cookie_store)
        .await?;
    let mut file = File::create(&file_name).await?;
    file.write_all(&aansluitingen).await?;
    file.flush().await?;
    let mut workbook: Xlsx<_> = calamine::open_workbook(file_name)?;
    let range = workbook.worksheet_range("Lijst_Export")?;
    let mut rows = range.rows().take(11);
    let index = rows
        .next()
        .ok_or(MainError::ValueMissing("Empty worksheet"))?
        .iter()
        .enumerate()
        .filter(|(_, value)| {
            value
                .get_string()
                .is_some_and(|value| value == "EAN code" || value == "Beschikbare meetdata")
        })
        .map(|pair| pair.0)
        .collect::<Vec<_>>();
    Ok(rows
        .filter_map(|row| row.get(index[0]).map(|value| Ean::from(value.to_string())))
        .collect())
}

async fn load_ids<I: IntoIterator<Item = Ean> + std::marker::Send>(
    eans: I,
    id_tx: mpsc::Sender<(Ean, Id)>,
    cookie_store: CookieStore,
) {
    let mut sleep_time = Duration::from_micros(1);
    for ean in eans {
        loop {
            sleep_time *= 2;
            eprintln!("{} seconds to load ids", sleep_time.as_secs_f64());
            // Retrieve the id of the meter

            let Ok(meter_id) = Id::from_ean(&cookie_store, &ean).await else {
                continue;
            };

            let Ok(received_ean) = Ean::from_id(&cookie_store, meter_id).await else {
                eprintln!("Failed to check ean and date range");
                tokio::time::sleep(sleep_time).await;
                continue;
            };

            if received_ean != ean {
                eprintln!(
                    "Received ean is not the same as requested ean!\nExpected: {ean}\nReceived: {received_ean}\nId: {meter_id}\n"
                );
                tokio::time::sleep(sleep_time).await;
                continue;
            }

            println!("{ean}: {meter_id}");
            while id_tx.send((ean.clone(), meter_id)).await.is_err() {
                tokio::time::sleep(sleep_time).await;
            }
            sleep_time = (sleep_time / 4).max(Duration::from_micros(1));
            break;
        }
    }
}

fn receive_id<T>(rx: &mut mpsc::Receiver<T>, sender_closed: &mut bool) -> Option<T> {
    if *sender_closed {
        return None;
    }
    match rx.try_recv() {
        Ok(value) => Some(value),
        Err(e) => match e {
            mpsc::error::TryRecvError::Empty => None,
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
    let mut i = 0;
    let mut sender_closed = false;
    let mut sleep_time = Duration::from_micros(1);
    loop {
        tokio::time::sleep(sleep_time).await;
        while let Some(id) = receive_id(&mut rx, &mut sender_closed) {
            ids.push(id);
        }
        sleep_time *= 2;
        if i >= ids.len() && !sender_closed {
            continue;
        }
        i %= ids.len();

        let today = chrono::Local::now().date_naive();
        // Download the latest report
        let Ok((file_name, report)) = Report::EnergieVerbruikPerUur(
            ids[i].1,
            today.with_year(today.year() - 1).unwrap(),
            today,
        )
        .download_latest_version(cookie_store)
        .await
        else {
            eprintln!("Failed to download report");
            cookie_store.redo_login().await.ok();
            continue;
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

        eprintln!("Saved the report");
        i += 1;
        sleep_time = (sleep_time / 4).min(Duration::from_micros(1));
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

    let (tx, rx) = mpsc::channel(10);

    eprintln!("Reading eans");
    let eans = read_eans(&cookie_store)
        .await
        .expect("Failed to read ean codes");

    let ids = Vec::with_capacity(eans.len());

    eprintln!("Loading ids and reports");
    let id_loader = tokio::spawn(load_ids(eans, tx, cookie_store.clone()));
    let report_loader = download_reports(ids, &cookie_store, &args.output, rx);
    let (join_result, ()) = join!(id_loader, report_loader);
    join_result.unwrap();
}
