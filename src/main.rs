use std::{
    io::{self, Write as _},
    path::PathBuf,
    str::FromStr as _,
    time::Duration,
};

use calamine::{Reader, Xlsx};
use clap::Parser;
use rapportage_downloader::login::CookieStore;
use rapportage_downloader::report::Report;
use reqwest::Client;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt as _,
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

/// Saves the data to a server or file
async fn save(client: &Client, output: &str, data: Vec<u8>, filename: String) {
    if let Ok(url) = url::Url::from_str(output) {
        // Store the report in a form
        let form = reqwest::multipart::Form::new().part(
            "file",
            reqwest::multipart::Part::bytes(data).file_name(filename),
        );

        // Send the file to the requested url
        client
            .post(url)
            .multipart(form)
            .send()
            .await
            .expect("Failed to send file");
    } else if let Ok(directory) = PathBuf::from_str(output) {
        // Create the requested directory
        fs::create_dir_all(&directory)
            .await
            .expect("Failed to create the requested directory");

        // Create the file
        let mut file = File::create(directory.join(filename))
            .await
            .expect("Failed to create file");

        // Write the report to the file
        file.write_all(&data)
            .await
            .expect("Failed to write report to file");
    } else {
        // The output should be a url or filepath, so panic if it is neither
        panic!("Passed output isn't a valid url or filepath");
    }
}

async fn read_eans(cookie_store: &CookieStore) -> Vec<String> {
    let (file_name, aansluitingen) = Report::Aansluitinglijst
        .download_latest_version(cookie_store)
        .await
        .unwrap();
    let mut file = File::create(&file_name).await.unwrap();
    file.write_all(&aansluitingen).await.unwrap();
    file.flush().await.unwrap();
    let mut workbook: Xlsx<_> = calamine::open_workbook(file_name).unwrap();
    let range = workbook.worksheet_range("Lijst_Export").unwrap();
    let mut rows = range.rows();
    let index = rows
        .next()
        .unwrap()
        .iter()
        .enumerate()
        .find(|(_, value)| value.get_string().is_some_and(|value| value == "EAN code"))
        .unwrap()
        .0;
    rows.filter_map(|row| row.get(index).and_then(|value| value.as_string()))
        .collect()
}

async fn ean_to_id(cookie_store: &CookieStore, ean: &str) -> String {
    let content = String::from_utf8(
        cookie_store
            .client()
            .get("https://www.dbenergie.nl/Connections/List/Index")
            .header(reqwest::header::COOKIE, format!("PersonalFilter=%7B%22mainPortalId%22%3A1%2C%22portalId%22%3A6%2C%22productId%22%3A%5B1%5D%2C%22statusId%22%3A%5B%5D%2C%22providerId%22%3A0%2C%22gridId%22%3A0%2C%22meterreadingcompanyId%22%3A0%2C%22customerId%22%3A%5B50%5D%2C%22departmentId%22%3A%5B%5D%2C%22gvkvId%22%3A0%2C%22monitoringTypesId%22%3A0%2C%22characteristicId%22%3A0%2C%22consumptionCategoryId%22%3A0%2C%22consumptionTypeId%22%3A%5B%5D%2C%22costplaceId%22%3A0%2C%22energytaxationclusterId%22%3A0%2C%22classificationId%22%3A0%2C%22labelId%22%3A0%2C%22ConnectionTypeId%22%3A0%2C%22meterNumber%22%3A%22%22%2C%22eanSearch%22%3A%22%22%2C%22meterDeleted%22%3Afalse%2C%22ListMap%22%3Afalse%2C%22pageSize%22%3A50%2C%22pageNumber%22%3A{}%2C%22orderBy%22%3A%22%22%2C%22orderDirection%22%3A%22asc%22%7D", 11))
            .header("request", base64::encode("false"))
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap()
            .into_iter()
            .collect::<Vec<u8>>(),
    )
    .unwrap();
    let page = scraper::Html::parse_document(&content);
    let selector = scraper::Selector::parse("a.list-row-visible").unwrap();
    page.select(&selector)
        .next()
        .unwrap_or_else(|| panic!("Failed to find connection in: {content}"))
        .attr("href")
        .unwrap()
        .to_owned()
}

#[tokio::main]
async fn main() {
    let single_reports = [
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
    ];
    // Parse the arguments
    let args = Args::parse();

    // Log in to receive a cookie
    let cookie_store = CookieStore::login(&args.mail, &args.password)
        .await
        .expect("Login failed");

    /*for report in single_reports.iter() {
        // Download the latest version of the requested report
        let (filename, response) = report
            .download_latest_version(&cookie_store)
            .await
            .expect("Failed to download latest report version");

        // Save the response to a server or file
        save(cookie_store.client(), &args.output, response, filename).await;
    }*/
    /*let id = 84104;
    let (file_name, content) = Report::EnergieVerbruikPerUur(id, "2023-12-01".to_owned())
        .download_latest_version(&cookie_store)
        .await
        .expect("Failed to request energie verbruik per uur");
    if content.is_empty() {
        println!("No content");
    } else {
        println!("Found file");
        save(
            cookie_store.client(),
            &args.output,
            content,
            format!("{id}_{file_name}"),
        )
        .await;
    }*/
    let ean = "871694840032658904";
    println!("{ean}: {}", ean_to_id(&cookie_store, ean).await);
    /*for ean in read_eans(&cookie_store).await {
        println!("{ean}: {}", ean_to_id(&cookie_store, &ean).await);
        tokio::time::sleep(Duration::from_secs(1)).await;
    }*/
}
