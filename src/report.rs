use std::{fmt::Display, io as std_io, str::FromStr, string::FromUtf8Error};

use reqwest::{
    header::{HeaderValue, InvalidHeaderValue},
    Method, Request, Url,
};

use crate::login::CookieStore;

/// Errors that can occur while downloading a report
#[derive(Debug, thiserror::Error)]
pub enum Error {
    InvalidUrl(#[from] url::ParseError),
    InvalidHeaderValue(#[from] InvalidHeaderValue),
    Request(#[from] reqwest::Error),
    Utf8(#[from] FromUtf8Error),
    Json(#[from] serde_json::Error),
    NotAnObject,
    KeyNotFound(&'static str),
    ValueNotAString,
    Io(#[from] std_io::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// The available report types
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Report {
    /// Energie aansluitingenlijst
    Aansluitinglijst,

    /// Energie belastingcluster per meter
    Belastingcluster,

    /// CO2 verbruiks rapportage
    Co2,

    /// Datakwaliteits rapportage
    Datakwaliteit,

    /// Gebouwen
    Gebouwen,

    /// Meet- en infradiensten
    MeetEnInfra,

    /// Aansluiting metadata
    Metadata,

    /// Meterstanden
    Meterstanden,

    /// MJ verbruiks rapportage
    Mj,

    /// Tussenmeters
    Tussenmeter,

    /// Verbruiksrapportage per product
    Verbruik,
}

impl Report {
    /// Returns the corresponding url for a report
    #[must_use]
    pub const fn url(&self) -> &str {
        match self {
            Self::Aansluitinglijst => "https://www.dbenergie.nl/Connections/List/ExportList",
            Self::Belastingcluster => {
                "https://www.dbenergie.nl/Connections/List/ExportTaxationCluster"
            }
            Self::Co2 | Self::Mj => "https://www.dbenergie.nl/Report/Co2/GetDownload",
            Self::Datakwaliteit => {
                "https://www.dbenergie.nl/Report/DataEntiretyCheck/GetDataToDownload"
            }
            Self::Gebouwen => "https://www.dbenergie.nl/Buildings/List/ExportList",
            Self::MeetEnInfra => "https://www.dbenergie.nl/Report/MeteringServices/ExportList",
            Self::Metadata => "https://www.dbenergie.nl/Connections/List/ExportMetaData",
            Self::Meterstanden => "https://www.dbenergie.nl/Connections/List/ExportMeterReading",
            Self::Tussenmeter => {
                "https://www.dbenergie.nl/Connections/IntermediateMeter/ExportList"
            }
            Self::Verbruik => "https://www.dbenergie.nl/Report/Consumption/GetDownload",
        }
    }

    /// Checks for the latest version of the report and returns it's filename.
    /// The client should contain the required cookies.
    ///
    /// # Errors
    /// - If the url corresponding to the Report is invalid.
    /// - If the cookie isn't a valid header value.
    /// - If the request failed
    /// - If the response body couldn't be read
    /// - If the response body isn't valid json
    /// - If the response body wasn't a json object
    /// - If the response body didn't contain a fileName
    /// - If the fileName isn't a string
    pub async fn latest_version(&self, cookie_store: &CookieStore) -> Result<String, Error> {
        let client = cookie_store.client();

        // Create a get request for the report
        let mut request = Request::new(Method::GET, Url::from_str(self.url())?);

        match self {
            Self::Aansluitinglijst
            | Self::Belastingcluster
            | Self::Gebouwen
            | Self::MeetEnInfra
            | Self::Metadata
            | Self::Meterstanden
            | Self::Tussenmeter => request
                .headers_mut()
                .insert("request", HeaderValue::from_str("MjAyMw==")?),
            Self::Co2 => request.headers_mut().insert("request", HeaderValue::from_str("eyJwb3J0YWxJZCI6IjYiLCJ1bml0SWQiOjEsImN1c3RvbWVySWRzIjoiNTAiLCJ5ZWFyRnJvbSI6MjAyMywieWVhclRpbGwiOjIwMjMsInJlcG9ydFR5cGUiOiJ0b3RhbCJ9")?),
            Self::Datakwaliteit => request.headers_mut().insert("request", HeaderValue::from_str("eyJwb3J0YWxJZCI6MCwicHJvZHVjdElkIjoxLCJjdXN0b21lcklkIjoiNTAiLCJkZXBhcnRtZW50SWRzIjoiIiwiY29zdHNwbGFjZUlkIjoiMCIsImNvbnN1bXB0aW9uQ2F0ZWdvcnlJZHMiOiIiLCJjb25zdW1wdGlvblR5cGVJZHMiOiIiLCJ0YXhhdGlvbkNsdXN0ZXJJZCI6IjAiLCJlYW5Db2RlIjoiIiwieWVhciI6MjAyMywibW9udGgiOjExfQ==")?),
            Self::Mj => request.headers_mut().insert("request", HeaderValue::from_str("eyJwb3J0YWxJZCI6IjYiLCJ1bml0SWQiOjIsImN1c3RvbWVySWRzIjoiNTAiLCJ5ZWFyRnJvbSI6MjAyMywieWVhclRpbGwiOjIwMjMsInJlcG9ydFR5cGUiOiJ0b3RhbCJ9")?),
            Self::Verbruik => request.headers_mut().insert("request", HeaderValue::from_str("eyJjbGFzc2lmaWNhdGlvbklkIjowLCJjb25zdW1wdGlvbmNhdGVnb3J5SWRzIjoiIiwiY29uc3VtcHRpb250eXBlSWRzIjoiIiwiY29zdHNwbGFjZUlkcyI6IiIsImN1c3RvbWVySWRzIjoiNTAiLCJwb3J0YWxDb2xsZWN0aXZlSWRzIjoiIiwiZGF0YWNoZWNrcmVwb3J0IjpmYWxzZSwiZGVwYXJ0bWVudElkcyI6IiIsImVhbmNvZGUiOiIiLCJlbmVyZ3l0YXhJZHMiOiIiLCJnZXRPREEiOnRydWUsIm1vbnRoRnJvbSI6MSwibW9udGhUaWxsIjoxMiwibW9udGhzIjpmYWxzZSwicG9ydGFsSWQiOiIwIiwicHJvZHVjdElkIjoxLCJyZXBvcnRUeXBlIjoidG90YWwiLCJ5ZWFyRnJvbSI6MjAyMywieWVhclRpbGwiOjIwMjMsImlzQ29sbGVjdGl2ZSI6ZmFsc2V9")?),
        };

        // Send the request
        let response = client.execute(request).await?;

        // Turn the response body (payload) into a string
        let body = String::from_utf8(response.bytes().await?.into_iter().collect::<Vec<u8>>())?;

        // Convert the response body into json
        let body: serde_json::Value = serde_json::from_str(&body)?;

        // Turn json into an object
        // Take the fileName property
        // Turn the value into a string and make it an owned string
        Ok(body
            .as_object()
            .ok_or(Error::NotAnObject)?
            .get("fileName")
            .ok_or(Error::KeyNotFound("fileName"))?
            .as_str()
            .ok_or(Error::ValueNotAString)?
            .to_owned())
    }

    /// Downloads the requested version.
    /// The client should contain the required cookies.
    /// The filename is the version of the report that should be downloaded.
    ///
    /// # Errors
    /// - If the Url couldn't be created with the requested version
    /// - If the cookie isn't a valid header value
    /// - If the request failed
    /// - If the file couldn't be created
    /// - If the response body couldn't be read
    /// - If the response body couldn't be written to the file
    pub async fn download_version(
        &self,
        cookie_store: &CookieStore,
        filename: &str,
    ) -> Result<Vec<u8>, Error> {
        // Create a request for the file
        let response = cookie_store
            .client()
            .get(format!(
                "https://www.dbenergie.nl/Global/Download?fileName={filename}"
            ))
            .send()
            .await?;

        Ok(response.bytes().await?.into_iter().collect::<Vec<u8>>())
    }

    /// Downloads the latest version of the report.
    /// The client should contain the required cookies.
    ///
    /// # Errors
    /// - If requesting the latest version returns an error
    /// - If downloading the version returns an error
    pub async fn download_latest_version(
        &self,
        cookie_store: &CookieStore,
    ) -> Result<(String, Vec<u8>), Error> {
        // Request the latest version
        let latest_version = self.latest_version(cookie_store).await?;

        // Download the version
        let response = self.download_version(cookie_store, &latest_version).await?;
        Ok((latest_version, response))
    }
}
