#![warn(clippy::unwrap_used, clippy::expect_used)]

use std::str::Utf8Error;

use reqwest::Client;
use scraper::error::SelectorErrorKind;
use thiserror::Error;

use crate::Args;

/// Errors that can happen during log in
#[derive(Debug, Error)]
pub enum LoginError {
    FailedRequest(#[from] reqwest::Error),
    MissingVerificationToken,
    TokenHasNoValue,
    Utf8Error(#[from] Utf8Error),
    InvalidTokenSelector(#[from] SelectorErrorKind<'static>),
}

impl std::fmt::Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Retrieve the verification token from the website
async fn get_verification_token(client: &Client) -> Result<String, LoginError> {
    // Get the login page
    let response = client
        .get("https://www.dbenergie.nl/Authorization/Login/Default")
        .send()
        .await?;

    // Read the body
    let login_page = response.bytes().await?.into_iter().collect::<Vec<u8>>();

    // Convert it to &str
    let login_page = core::str::from_utf8(&login_page)?;

    // Parse the document as html
    let login_page = scraper::Html::parse_document(login_page);

    // Create a selector for the request verification token
    let token_selector = scraper::Selector::parse("[name=\"__RequestVerificationToken\"]")?;

    // Retrieve and return the value of the request verification token
    Ok(login_page
        .select(&token_selector)
        .next()
        .ok_or(LoginError::MissingVerificationToken)?
        .attr("value")
        .ok_or(LoginError::TokenHasNoValue)?
        .to_owned())
}

/// Logs in to the DB Energie website.
/// The client should save the cookies automatically.
pub async fn login(client: &Client, args: &Args) -> Result<(), LoginError> {
    // Create the login data
    let login_data = [
        ("user[emailAddress]", &args.mail),
        ("user[passWord]", &args.password),
        (
            "__RequestVerificationToken",
            &get_verification_token(client).await?,
        ),
    ];

    // Send it to the server to retrieve the cookies
    client
        .post("https://www.dbenergie.nl/Home/Login")
        .form(&login_data)
        .send()
        .await?;
    Ok(())
}
