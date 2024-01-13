use std::{fmt::Display, num::ParseIntError, string::FromUtf8Error};

use scraper::error::SelectorErrorKind;

use crate::{id::Id, login::CookieStore};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    Request(#[from] reqwest::Error),
    #[allow(dead_code)]
    ValueMissing(&'static str),
    Utf8(#[from] FromUtf8Error),
    Selector(#[from] SelectorErrorKind<'static>),
    UrlParse(#[from] url::ParseError),
    ParseInt(#[from] ParseIntError),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ean(String);

impl From<String> for Ean {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<Ean> for String {
    fn from(value: Ean) -> Self {
        value.0
    }
}

impl Display for Ean {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Ean {
    pub async fn from_id(cookie_store: &CookieStore, id: Id) -> Result<Ean, Error> {
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
            .ok_or(Error::ValueMissing("No ean code found"))?
            .attr("value")
            .ok_or(Error::ValueMissing("Ean code doesn't have a value"))?
            .trim()
            .to_owned();
        Ok(ean.into())
    }
}
