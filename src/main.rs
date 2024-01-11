#![warn(clippy::pedantic, clippy::nursery)]
use std::{
    fmt::Display, io, path::PathBuf, str::FromStr as _, string::FromUtf8Error, time::Duration,
};

use base64::Engine;
use calamine::{Reader, Xlsx};
use chrono::Datelike;
use clap::Parser;
use rapportage_downloader::{
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
use url::Url;

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

async fn read_eans(cookie_store: &CookieStore) -> Result<Vec<String>, MainError> {
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
        .filter_map(|row| row.get(index[0]).map(calamine::DataType::to_string))
        .collect())
}

async fn ean_to_id(cookie_store: &CookieStore, ean: &str) -> Result<String, MainError> {
    cookie_store.add_cookie_str(&format!("PersonalFilter=%7B%22mainPortalId%22%3A1%2C%22portalId%22%3A6%2C%22productId%22%3A%5B1%5D%2C%22statusId%22%3A%5B%5D%2C%22providerId%22%3A0%2C%22gridId%22%3A0%2C%22meterreadingcompanyId%22%3A0%2C%22customerId%22%3A%5B50%5D%2C%22departmentId%22%3A%5B%5D%2C%22gvkvId%22%3A0%2C%22monitoringTypesId%22%3A0%2C%22characteristicId%22%3A0%2C%22consumptionCategoryId%22%3A0%2C%22consumptionTypeId%22%3A%5B%5D%2C%22costplaceId%22%3A0%2C%22energytaxationclusterId%22%3A0%2C%22classificationId%22%3A0%2C%22labelId%22%3A0%2C%22ConnectionTypeId%22%3A0%2C%22meterNumber%22%3A%22%22%2C%22eanSearch%22%3A%22{ean}%22%2C%22meterDeleted%22%3Afalse%2C%22ListMap%22%3Afalse%2C%22pageSize%22%3A15%2C%22pageNumber%22%3A1%2C%22orderBy%22%3A%22%22%2C%22orderDirection%22%3A%22asc%22%7D"), &Url::from_str("https://www.dbenergie.nl/Connections/List/Index")?);
    let content = String::from_utf8(
        cookie_store
            .client()
            .get("https://www.dbenergie.nl/Connections/List/Index")
            .header(
                "request",
                base64::engine::general_purpose::STANDARD.encode("false"),
            )
            .send()
            .await?
            .bytes()
            .await?
            .into_iter()
            .collect::<Vec<u8>>(),
    )?;
    let page = scraper::Html::parse_document(&content);
    let selector = scraper::Selector::parse("a.list-row-visible")?;
    Ok(page
        .select(&selector)
        .next()
        .ok_or(MainError::ValueMissing("Failed to find connection"))?
        .attr("href")
        .ok_or(MainError::ValueMissing("Connection doesn't contain a link"))?
        .to_owned())
}

async fn id_to_ean(cookie_store: &CookieStore, id: &str) -> Result<String, MainError> {
    let page = cookie_store
        .client()
        .get(format!(
            "https://www.dbenergie.nl/Connections/Edit/Index/{id}"
        ))
        .send()
        .await?
        .bytes()
        .await?
        .to_vec();
    let page = scraper::Html::parse_document(&String::from_utf8(page)?);

    let selector = scraper::Selector::parse("#Mod_ean")?;
    let ean = page
        .select(&selector)
        .next()
        .ok_or(MainError::ValueMissing("No ean code found"))?
        .attr("value")
        .ok_or(MainError::ValueMissing("Ean code doesn't have a value"))?
        .trim()
        .to_owned();
    Ok(ean)
}

async fn load_ids(
    eans: Vec<String>,
    id_tx: mpsc::Sender<(String, String)>,
    cookie_store: CookieStore,
) {
    let mut sleep_time = Duration::from_micros(1);
    for ean in eans {
        loop {
            // Double the sleep time
            sleep_time *= 2;
            eprintln!("{} seconds to load ids", sleep_time.as_secs_f64());

            // Retrieve the id of the meter
            let Ok(meter_id) = ean_to_id(&cookie_store, &ean).await else {
                eprintln!("Failed to checked id for ean {ean}.");
                tokio::time::sleep(sleep_time).await;
                continue;
            };

            // Take the id
            let Some(meter_id) = meter_id.split('/').last() else {
                eprintln!("No meter_id found for ean {ean}");
                tokio::time::sleep(sleep_time).await;
                continue;
            };

            // Check the ean corresponding to the id
            let Ok(received_ean) = id_to_ean(&cookie_store, meter_id).await else {
                eprintln!(
                    "Failed to check ean and date range for id {meter_id} received for ean {ean}"
                );
                tokio::time::sleep(sleep_time).await;
                continue;
            };

            // Make sure the received ean is equal to the expected ean
            if received_ean != ean {
                eprintln!("Received ean {received_ean} is not the same as requested ean {ean}!");
                tokio::time::sleep(sleep_time).await;
                continue;
            }

            // Display the ean and its id
            println!("{ean}: {meter_id}");

            // Send the ean and id, this will only fail if the report downloader stopped
            if id_tx
                .send((ean.clone(), meter_id.to_owned()))
                .await
                .is_err()
            {
                break;
            }

            // Decrease the sleep time
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
    mut ids: Vec<(String, String)>,
    cookie_store: &CookieStore,
    output: &str,
    mut rx: mpsc::Receiver<(String, String)>,
) {
    let mut i = 0;
    let mut sender_closed = false;
    let mut sleep_time = Duration::from_millis(1);
    loop {
        tokio::time::sleep(sleep_time).await;

        // Add all known id - ean matches to the list of received eans
        while let Some(id) = receive_id(&mut rx, &mut sender_closed) {
            ids.push(id);
        }

        // Double the sleep time for the next iteration
        sleep_time *= 2;
        eprintln!("{} seconds to download reports", sleep_time.as_secs_f64());

        // Wait if i reaches the number of ids and not all ids have been received yet
        if i >= ids.len() && !sender_closed {
            continue;
        }

        // Reset i to 0, if it reaches the current number of ids
        i %= ids.len();

        // Load the current date
        let today = chrono::Local::now().date_naive();

        // Download the latest report
        let (file_name, report) = match Report::EnergieVerbruikPerUur(
            &ids[i].1,
            today.with_year(today.year() - 1).unwrap(),
            chrono::Local::now().date_naive(),
        )
        .download_latest_version(cookie_store)
        .await
        {
            Ok((file_name, report)) => (file_name, report),
            Err(e) => {
                eprintln!("Failed to download report for ean {}\n{e:?}\n", ids[i].0);
                cookie_store.redo_login().await.ok();
                continue;
            }
        };

        // Save the file
        if let Err(e) = save(
            cookie_store.client(),
            output,
            report,
            format!("{}_{file_name}", ids[i].0),
        )
        .await
        {
            eprintln!("Failed to save report for ean {}\n{e:?}\n", ids[i].0);
            continue;
        };

        // Tell the user, the report has been saved
        eprintln!("Saved the report for ean {}", ids[i].0);

        // Continue to the next report and decrease the sleep time
        i += 1;
        sleep_time = (sleep_time / 4).max(Duration::from_millis(1));
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

    // Create a channel
    let (tx, rx) = mpsc::channel(10);

    // Download the eans
    eprintln!("Reading eans");
    let eans = read_eans(&cookie_store)
        .await
        .expect("Failed to read ean codes");

    // Create a vector for the ids
    let ids = Vec::with_capacity(eans.len());

    // Load the ids and download and store the reports
    eprintln!("Loading ids and reports");
    let id_loader = tokio::spawn(load_ids(eans, tx, cookie_store.clone()));
    let report_loader = download_reports(ids, &cookie_store, &args.output, rx);

    // Wait for the loading of ids and downloading of reports is done.
    let (join_result, ()) = join!(id_loader, report_loader);
    join_result.unwrap();
}
