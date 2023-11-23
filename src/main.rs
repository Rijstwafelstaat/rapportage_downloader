use std::str::FromStr;

use clap::Parser;
use report::Report;
use reqwest::{header::HeaderValue, Body, Client, Method, Request};
use url::Url;

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

async fn get_verification_token() -> String {
    let response = reqwest::get("https://www.dbenergie.nl/Authorization/Login/Default")
        .await
        .expect("Failed to request login page");
    let login_page = response
        .bytes()
        .await
        .expect("Failed to read login request body")
        .into_iter()
        .collect::<Vec<u8>>();
    let login_page = core::str::from_utf8(&login_page).expect("Login page isn't valid utf-8");
    let login_page = scraper::Html::parse_document(login_page);
    let token_selector = scraper::Selector::parse("[name=\"__RequestVerificationToken\"]")
        .expect("Invalid selector");
    login_page
        .select(&token_selector)
        .next()
        .expect("Verification token not found")
        .attr("value")
        .expect("Verification token doesn't have a value")
        .to_owned()
}

async fn login(client: &Client, args: &Args) -> String {
    let mut request = Request::new(
        Method::POST,
        Url::from_str("https://www.dbenergie.nl/Home/Login").expect("Invalid password url"),
    );
    let login_data = [
        ("user[emailAddress]", &args.mail),
        ("user[passWord]", &args.password),
        (
            "__RequestVerificationToken",
            &get_verification_token().await,
        ),
    ];
    request
        .url_mut()
        .set_username(&args.mail)
        .expect("Failed to set mail as username");
    request
        .url_mut()
        .set_password(Some(&args.password))
        .expect("Failed to set password");
    let login_data = serde_urlencoded::to_string(login_data)
        .expect("Failed to convert login data to url encoded string");
    request.headers_mut().insert(
        reqwest::header::CONTENT_LENGTH,
        HeaderValue::from_str(&login_data.len().to_string())
            .expect("Failed to create content-length header"),
    );
    *request.body_mut() = Some(Body::from(login_data));
    let response = client
        .execute(request)
        .await
        .expect("Failed to send login request");
    let cookies = response
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .map(|cookie| cookie.to_str().expect("A received cookie isn't a string"))
        .expect("Login response didn't contain a cookie")
        .to_owned();

    cookies
}

#[tokio::main]
async fn main() {
    // Parse the arguments
    let args = Args::parse();

    // Create a client
    let client = Client::new();

    // Read the cookie
    let cookie = login(&client, &args).await;
    panic!("{cookie}");

    // Download the latest version of the requested report
    args.report
        .download_latest_version(&client, &cookie)
        .await
        .expect("Failed to download latest report version");
}
