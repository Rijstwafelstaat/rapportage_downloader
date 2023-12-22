#![warn(clippy::pedantic, clippy::nursery)]
use std::time::Duration;
use std::{fmt::Display, io, path::PathBuf, str::FromStr as _, string::FromUtf8Error};

use base64::Engine;
use calamine::{Reader, Xlsx};
use chrono::Datelike;
use clap::Parser;
use rapportage_downloader::report::Report;
use rapportage_downloader::{login::CookieStore, report};
use reqwest::{cookie, Client};
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

async fn ean_to_id(
    cookie_store: &CookieStore,
    cookies: &cookie::Jar,
    ean: &str,
) -> Result<String, MainError> {
    cookies.add_cookie_str(&format!("PersonalFilter=%7B%22mainPortalId%22%3A1%2C%22portalId%22%3A6%2C%22productId%22%3A%5B1%5D%2C%22statusId%22%3A%5B%5D%2C%22providerId%22%3A0%2C%22gridId%22%3A0%2C%22meterreadingcompanyId%22%3A0%2C%22customerId%22%3A%5B50%5D%2C%22departmentId%22%3A%5B%5D%2C%22gvkvId%22%3A0%2C%22monitoringTypesId%22%3A0%2C%22characteristicId%22%3A0%2C%22consumptionCategoryId%22%3A0%2C%22consumptionTypeId%22%3A%5B%5D%2C%22costplaceId%22%3A0%2C%22energytaxationclusterId%22%3A0%2C%22classificationId%22%3A0%2C%22labelId%22%3A0%2C%22ConnectionTypeId%22%3A0%2C%22meterNumber%22%3A%22%22%2C%22eanSearch%22%3A%22{ean}%22%2C%22meterDeleted%22%3Afalse%2C%22ListMap%22%3Afalse%2C%22pageSize%22%3A15%2C%22pageNumber%22%3A1%2C%22orderBy%22%3A%22%22%2C%22orderDirection%22%3A%22asc%22%7D"), &Url::from_str("https://www.dbenergie.nl/Connections/List/Index")?);
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

async fn id_to_ean(cookie_store: &CookieStore, id: u32) -> Result<String, MainError> {
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

async fn id_to_ean_date_range(
    cookie_store: &CookieStore,
    id: u32,
) -> Result<(String, chrono::NaiveDate, chrono::NaiveDate), MainError> {
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

    let selector = scraper::Selector::parse("#statusDataOdaRequest")?;
    let dates = page
        .select(&selector)
        .next()
        .ok_or(MainError::ValueMissing("No dates section found"))?
        .attr("value")
        .ok_or(MainError::ValueMissing(
            "Dates section doesn't have a value",
        ))?;
    let dates = dates
        .split_whitespace()
        .filter_map(|date| chrono::NaiveDate::parse_from_str(date, "%d-%m-%Y").ok())
        .collect::<Vec<_>>();
    if dates.len() < 2 {
        return Err(MainError::ValueMissing("Not enough dates found"));
    }
    Ok((ean, dates[0], dates[1]))
}

#[tokio::main]
async fn main() {
    // Parse the arguments
    let args = Args::parse();

    // Log in to receive a cookie
    let (cookie_store, cookies) = CookieStore::login(&args.mail, &args.password)
        .await
        .expect("Login failed");

    let eans = read_eans(&cookie_store)
        .await
        .expect("Failed to read ean codes");

    let mut ids = Vec::with_capacity(eans.len());

    for ean in &eans {
        loop {
            // Retrieve the id of the meter
            let Ok(meter_id) = ean_to_id(&cookie_store, &cookies, ean).await else {
                continue;
            };
            let Some(meter_id) = meter_id.split('/').last() else {
                println!("No meter_id found");
                continue;
            };

            let Ok(meter_id) = meter_id.parse::<u32>() else {
                println!("meter_id isn't an integer");
                continue;
            };

            let Ok(received_ean) = id_to_ean(&cookie_store, meter_id).await else {
                println!("Failed to check ean and date range");
                continue;
            };

            if &received_ean != ean {
                println!(
                "Received ean is not the same as requested ean: {ean} {received_ean} {meter_id}"
            );
                continue;
            }

            println!("{ean}: {meter_id}");
            ids.push(meter_id);
            break;
        }
    }

    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let today = chrono::Local::now().date_naive();
        // Download the latest report
        let Ok((file_name, report)) = Report::EnergieVerbruikPerUur(
            ids.clone(),
            today.with_year(today.year() - 1).unwrap(),
            chrono::Local::now().date_naive(),
        )
        .download_latest_version(&cookie_store)
        .await
        else {
            println!("Failed to download report");
            continue;
        };

        // Save the file
        save(cookie_store.client(), &args.output, report, file_name)
            .await
            .ok();
    }
}
