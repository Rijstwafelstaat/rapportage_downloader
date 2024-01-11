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
    #[allow(dead_code)]
    IncorrectIdOrEan {
        id: String,
        ean: String,
    },
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
    // Download the latest version of the aansluitingen report
    let (file_name, aansluitingen) = Report::Aansluitinglijst
        .download_latest_version(cookie_store)
        .await?;

    // Save the aansluitingen
    let mut file = File::create(&file_name).await?;
    file.write_all(&aansluitingen).await?;
    file.flush().await?;

    // Load the lijst export worksheet
    let mut workbook: Xlsx<_> = calamine::open_workbook(file_name)?;
    let range = workbook.worksheet_range("Lijst_Export")?;
    let mut rows = range.rows();

    // Take the ean code and beschikbare meetdata columns
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

    // Take the ean code and beschikbare meetdata values from every row and return them in a vector
    Ok(rows
        .filter_map(|row| row.get(index[0]).map(calamine::DataType::to_string))
        .collect())
}

async fn ean_to_id(cookie_store: &CookieStore, ean: &str) -> Result<String, MainError> {
    // Add the id of the cookies
    cookie_store.add_cookie_str(&format!("PersonalFilter=%7B%22mainPortalId%22%3A1%2C%22portalId%22%3A6%2C%22productId%22%3A%5B1%5D%2C%22statusId%22%3A%5B%5D%2C%22providerId%22%3A0%2C%22gridId%22%3A0%2C%22meterreadingcompanyId%22%3A0%2C%22customerId%22%3A%5B50%5D%2C%22departmentId%22%3A%5B%5D%2C%22gvkvId%22%3A0%2C%22monitoringTypesId%22%3A0%2C%22characteristicId%22%3A0%2C%22consumptionCategoryId%22%3A0%2C%22consumptionTypeId%22%3A%5B%5D%2C%22costplaceId%22%3A0%2C%22energytaxationclusterId%22%3A0%2C%22classificationId%22%3A0%2C%22labelId%22%3A0%2C%22ConnectionTypeId%22%3A0%2C%22meterNumber%22%3A%22%22%2C%22eanSearch%22%3A%22{ean}%22%2C%22meterDeleted%22%3Afalse%2C%22ListMap%22%3Afalse%2C%22pageSize%22%3A15%2C%22pageNumber%22%3A1%2C%22orderBy%22%3A%22%22%2C%22orderDirection%22%3A%22asc%22%7D"), &Url::from_str("https://www.dbenergie.nl/Connections/List/Index")?);

    // Read the search page for the ean
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

    // Parse the document
    let page = scraper::Html::parse_document(&content);

    // Search for and return the connection id
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

async fn load_id(ean: &str, cookie_store: &CookieStore) -> Result<String, MainError> {
    let meter_id = ean_to_id(cookie_store, ean)
        .await?
        .split('/')
        .last()
        .ok_or(MainError::ValueMissing("No meter id for ean"))?
        .to_owned();
    let received_ean = id_to_ean(cookie_store, &meter_id).await?;
    if received_ean != meter_id {
        return Err(MainError::IncorrectIdOrEan {
            id: meter_id,
            ean: received_ean,
        });
    }
    Ok(meter_id)
}

async fn load_ids(
    eans: &[String],
    output: &str,
    cookie_store: CookieStore,
) -> Vec<(String, String)> {
    let mut sleep_time = Duration::from_micros(1);
    let mut ids = Vec::with_capacity(eans.len());
    for ean in eans {
        loop {
            tokio::time::sleep(sleep_time).await;
            // Double the sleep time
            sleep_time *= 2;
            eprintln!("{} seconds to load ids", sleep_time.as_secs_f64());

            // Load the id
            let meter_id = match load_id(ean, &cookie_store).await {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("Failed to load id for {ean}: {e}");
                    continue;
                }
            };

            // Display the ean and its id
            println!("{ean}: {meter_id}");

            let (file_name, report) = match download_report(&meter_id, &cookie_store).await {
                Ok((file_name, report)) => (file_name, report),
                Err(e) => {
                    eprintln!("Failed to download report for {ean}: {e}");
                    continue;
                }
            };

            if let Err(e) = save(
                cookie_store.client(),
                output,
                report,
                format!("{ean}_{file_name}"),
            )
            .await
            {
                eprintln!("Failed to save file for {ean}: {e}");
            }

            ids.push((ean.clone(), meter_id));

            // Decrease the sleep time
            sleep_time = (sleep_time / 4).max(Duration::from_micros(1));
            break;
        }
    }
    ids
}

async fn download_report(
    id: &str,
    cookie_store: &CookieStore,
) -> Result<(String, Vec<u8>), report::Error> {
    // Load the current date
    let today = chrono::Local::now().date_naive();

    // Download the latest report
    Report::EnergieVerbruikPerUur(
        id,
        today.with_year(today.year() - 1).unwrap(),
        chrono::Local::now().date_naive(),
    )
    .download_latest_version(cookie_store)
    .await
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

    // Download the eans
    eprintln!("Reading eans");
    let eans = read_eans(&cookie_store)
        .await
        .expect("Failed to read ean codes");

    // Load the ids and download and store the reports
    eprintln!("Loading ids and reports");
    let ids = load_ids(&eans, &args.output, cookie_store.clone()).await;
    loop {
        while cookie_store.redo_login().await.is_err() {}
        for (ean, id) in &ids {
            let (file_name, report) = match download_report(id, &cookie_store).await {
                Ok((file_name, report)) => (file_name, report),
                Err(e) => {
                    eprintln!("Failed to download report: {e}");
                    continue;
                }
            };

            if let Err(e) = save(
                cookie_store.client(),
                &args.output,
                report,
                format!("{ean}_{file_name}"),
            )
            .await
            {
                eprintln!("Failed to save file: {e}");
            }
        }
    }
}
