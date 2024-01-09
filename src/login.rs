use std::{str::Utf8Error, sync::Arc};

use reqwest::Client;
use scraper::error::SelectorErrorKind;

/// Errors that can happen during log in
#[derive(Debug, thiserror::Error)]
pub enum Error {
    FailedRequest(#[from] reqwest::Error),
    MissingVerificationToken,
    TokenHasNoValue,
    Utf8(#[from] Utf8Error),
    InvalidTokenSelector(#[from] SelectorErrorKind<'static>),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Clone)]
pub struct CookieStore {
    client: Client,
    jar: Arc<reqwest::cookie::Jar>,
    mail: String,
    password: String,
}

impl CookieStore {
    /// Retrieve the verification token from the website
    async fn get_verification_token(client: &Client) -> Result<String, Error> {
        // Get the login page
        let response = client
            .get("https://www.dbenergie.nl/Authorization/Login/Default")
            .send()
            .await?;

        // Read the body
        let login_page = response.bytes().await?.to_vec();

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
            .ok_or(Error::MissingVerificationToken)?
            .attr("value")
            .ok_or(Error::TokenHasNoValue)?
            .to_owned())
    }

    /// Logs in to the DB Energie website.
    /// The client should save the cookies automatically.
    ///
    /// # Errors
    /// Returns an error if the verification token couldn't be retrieved or the login form couldn't be send.
    pub async fn login(mail: String, password: String) -> Result<Self, Error> {
        let jar = Arc::new(reqwest::cookie::Jar::default());
        let client = Client::builder().cookie_provider(jar.clone()).build()?;

        let client = Self {
            client,
            jar,
            mail,
            password,
        };
        client.inner_login().await?;
        Ok(client)
    }

    async fn inner_login(&self) -> Result<(), Error> {
        // Create the login data
        let login_data = [
            ("user[emailAddress]", &self.mail),
            ("user[passWord]", &self.password),
            (
                "__RequestVerificationToken",
                &Self::get_verification_token(&self.client).await?,
            ),
        ];

        // Send it to the server to retrieve the cookies
        self.client
            .post("https://www.dbenergie.nl/Home/Login")
            .form(&login_data)
            .send()
            .await?;
        Ok(())
    }

    pub async fn redo_login(&self) -> Result<(), Error> {
        self.inner_login().await
    }

    #[allow(clippy::must_use_candidate)]
    pub const fn client(&self) -> &Client {
        &self.client
    }

    pub fn add_cookie_str(&self, cookie: &str, url: &url::Url) {
        self.jar.add_cookie_str(cookie, url);
    }
}
