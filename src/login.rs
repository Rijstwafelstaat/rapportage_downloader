#![warn(clippy::unwrap_used, clippy::expect_used)]

use std::str::Utf8Error;

use reqwest::Client;
use scraper::error::SelectorErrorKind;
use thiserror::Error;

use crate::Args;

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

async fn get_verification_token(client: &Client) -> Result<String, LoginError> {
    let response = client
        .get("https://www.dbenergie.nl/Authorization/Login/Default")
        .send()
        .await?;
    let login_page = response.bytes().await?.into_iter().collect::<Vec<u8>>();
    let login_page = core::str::from_utf8(&login_page)?;
    let login_page = scraper::Html::parse_document(login_page);
    let token_selector = scraper::Selector::parse("[name=\"__RequestVerificationToken\"]")?;
    Ok(login_page
        .select(&token_selector)
        .next()
        .ok_or(LoginError::MissingVerificationToken)?
        .attr("value")
        .ok_or(LoginError::TokenHasNoValue)?
        .to_owned())
}

pub async fn login(client: &Client, args: &Args) -> Result<(), LoginError> {
    let login_data = [
        ("user[emailAddress]", &args.mail),
        ("user[passWord]", &args.password),
        (
            "__RequestVerificationToken",
            &get_verification_token(client).await?,
        ),
    ];
    client
        .post("https://www.dbenergie.nl/Home/Login")
        .form(&login_data)
        .send()
        .await?;
    Ok(())
}
