#![warn(clippy::unwrap_used, clippy::expect_used)]
use std::{fmt::Display, io as std_io, str::FromStr, string::FromUtf8Error};

use reqwest::{
    header::{HeaderValue, InvalidHeaderValue},
    Client, Method, Request, Url,
};
use thiserror::Error;
use tokio::{
    fs,
    io::{self as rust_io, BufWriter},
};

#[derive(Debug, Error)]
pub enum ReportError {
    InvalidUrl(#[from] url::ParseError),
    InvalidHeaderValue(#[from] InvalidHeaderValue),
    RequestError(#[from] reqwest::Error),
    Utf8Error(#[from] FromUtf8Error),
    JsonError(#[from] serde_json::Error),
    NotAnObject,
    KeyNotFound(&'static str),
    ValueNotAString,
    IoError(#[from] std_io::Error),
}

impl Display for ReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Report {
    /// Energie aansluitingenlijst
    Aansluitinglijst,

    /// Energie belastingcluster per meter
    Belastingcluster,

    /// Tussenmeters
    Tussenmeter,

    /// Gebouwen
    Gebouwen,

    /// Meet- en infradiensten
    MeetEnInfra,
}

impl Report {
    /// Returns the corresponding url for a report
    pub const fn url(&self) -> &str {
        match self {
            Self::Aansluitinglijst => "https://www.dbenergie.nl/Connections/List/ExportList",
            Self::Belastingcluster => {
                "https://www.dbenergie.nl/Connections/List/ExportTaxationCluster"
            }
            Self::Tussenmeter => {
                "https://www.dbenergie.nl/Connections/IntermediateMeter/ExportList"
            }
            Self::Gebouwen => "https://www.dbenergie.nl/Buildings/List/ExportList",
            Self::MeetEnInfra => "https://www.dbenergie.nl/Report/MeteringServices/ExportList",
        }
    }

    /// Checks for the latest version of the report and returns it's filename.
    ///
    /// # Panics
    /// - If the url corresponding to the Report is invalid.
    /// - If the cookie isn't a valid header value.
    /// - If the request failed
    /// - If the response body couldn't be read
    /// - If the response body isn't valid json
    /// - If the response body wasn't a json object
    /// - If the response body didn't contain a fileName
    /// - If the fileName isn't a string
    pub async fn latest_version(
        &self,
        client: &Client,
        cookies: &str,
    ) -> Result<String, ReportError> {
        // Create a get request for the report
        let mut request = Request::new(Method::GET, Url::from_str(self.url())?);

        // Add the cookie
        request
            .headers_mut()
            .insert("Cookie", HeaderValue::from_str(cookies)?);

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
            .ok_or(ReportError::NotAnObject)?
            .get("fileName")
            .ok_or(ReportError::KeyNotFound("fileName"))?
            .as_str()
            .ok_or(ReportError::ValueNotAString)?
            .to_owned())
    }

    /// Downloads the requested version.
    ///
    /// # Panics
    /// - If the Url couldn't be created with the requested version
    /// - If the cookie isn't a valid header value
    /// - If the request failed
    /// - If the file couldn't be created
    /// - If the response body couldn't be read
    /// - If the response body couldn't be written to the file
    pub async fn download_version(
        &self,
        client: &Client,
        cookies: &str,
        filename: &str,
    ) -> Result<(), ReportError> {
        // Create a request for the file
        let mut request = Request::new(
            Method::GET,
            Url::from_str(&format!(
                "https://www.dbenergie.nl/Global/Download?fileName={filename}"
            ))?,
        );

        // Add the cookie
        request
            .headers_mut()
            .insert("Cookie", HeaderValue::from_str(cookies)?);

        // Send the request
        let response = client.execute(request).await?;

        // Create a file to write the content to
        let mut file = BufWriter::new(fs::File::create(filename).await?);

        // Copy the payload of the response to the file
        rust_io::copy(
            &mut response
                .bytes()
                .await?
                .into_iter()
                .collect::<Vec<u8>>()
                .as_slice(),
            &mut file,
        )
        .await?;
        Ok(())
    }

    /// Downloads the latest version of the report
    ///
    /// # Panics
    /// - If requesting the latest version panics
    /// - If downloading the version panics
    pub async fn download_latest_version(
        &self,
        client: &Client,
        cookies: &str,
    ) -> Result<(), ReportError> {
        // Request the latest version
        let latest_version = self.latest_version(client, cookies).await?;

        // Download the version
        self.download_version(client, cookies, &latest_version)
            .await?;
        Ok(())
    }
}