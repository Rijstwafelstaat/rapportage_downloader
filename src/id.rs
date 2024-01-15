use std::{fmt::Display, num::ParseIntError, str::FromStr, string::FromUtf8Error};

use base64::Engine;
use scraper::error::SelectorErrorKind;
use url::Url;

use crate::{ean::Ean, login::CookieStore};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Id(u32);

impl From<u32> for Id {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<Id> for u32 {
    fn from(value: Id) -> Self {
        value.0
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Id {
    pub async fn from_ean(cookie_store: &CookieStore, ean: &Ean) -> Result<Id, Error> {
        // Set the cookie for the ean
        cookie_store.add_cookie_str(&format!("PersonalFilter=%7B%22mainPortalId%22%3A1%2C%22portalId%22%3A6%2C%22productId%22%3A%5B1%5D%2C%22statusId%22%3A%5B%5D%2C%22providerId%22%3A0%2C%22gridId%22%3A0%2C%22meterreadingcompanyId%22%3A0%2C%22customerId%22%3A%5B50%5D%2C%22departmentId%22%3A%5B%5D%2C%22gvkvId%22%3A0%2C%22monitoringTypesId%22%3A0%2C%22characteristicId%22%3A0%2C%22consumptionCategoryId%22%3A0%2C%22consumptionTypeId%22%3A%5B%5D%2C%22costplaceId%22%3A0%2C%22energytaxationclusterId%22%3A0%2C%22classificationId%22%3A0%2C%22labelId%22%3A0%2C%22ConnectionTypeId%22%3A0%2C%22meterNumber%22%3A%22%22%2C%22eanSearch%22%3A%22{ean}%22%2C%22meterDeleted%22%3Afalse%2C%22ListMap%22%3Afalse%2C%22pageSize%22%3A15%2C%22pageNumber%22%3A1%2C%22orderBy%22%3A%22%22%2C%22orderDirection%22%3A%22asc%22%7D"), &Url::from_str("https://www.dbenergie.nl/Connections/List/Index")?);

        // Download the page for the ean
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

        // Parse the page
        let page = scraper::Html::parse_document(&content);

        // Create a selector for the id
        let selector = scraper::Selector::parse("a.list-row-visible")?;

        let ean_selector = scraper::Selector::parse(".row-cell.width-140")?;

        // Select, parse, and return the ID
        Ok(page
            .select(&selector)
            .find(|element| {
                let Some(ean_element) = element.select(&ean_selector).next() else {
                    return false;
                };
                let Some(found_ean) = ean_element.text().next() else {
                    return false;
                };
                ean.value() == found_ean.trim()
            })
            .ok_or(Error::ValueMissing(
                "Failed to find connection with expected ean",
            ))?
            .attr("href")
            .ok_or(Error::ValueMissing("Connection doesn't contain a link"))?
            .split('/')
            .last()
            .ok_or(Error::ValueMissing("Connection url doesn't contain an id"))?
            .parse::<u32>()?
            .into())
    }
}
