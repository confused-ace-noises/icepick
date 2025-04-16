use std::{str::FromStr, sync::Arc};

use reqwest::Client;
use scraper::{Html, Selector};
use url::Url;

use crate::{Result, utils::errors::Error};

#[derive(Debug, Clone)]
pub struct Scraper {
    client: Arc<Client>, // cloned, not owned
    url: Url,
    html: Option<String>,
}

impl Scraper {
    pub fn new(client: Arc<Client>, url_to_scrape: Url) -> Self {
        Self {
            client: Arc::clone(&client),
            url: url_to_scrape,
            html: None,
        }
    }

    pub async fn get(&mut self) -> Result<()> {
        let response = self.client.get(self.url.as_str()).send().await?;

        if response.status().is_success() {
            let text = response.text().await?;

            self.html = Some(text);
            return Ok(());
        } else {
            return Err(Error::GenericError(
                "Internet error: request unsuccessful".to_string(),
            ));
        }
    }

    /// ## Usage
    /// gets all links out of a previously gotten html string.
    /// if self.html is none, it'll return None itself.
    pub async fn scrape(self) -> Option<ScrapeOut> {
        match self.html {
            None => return None,
            Some(html) => {
                let document = Html::parse_document(&html);
                let selector = Selector::parse("a").unwrap();
                let links: Vec<Url> = document
                    .select(&selector)
                    .filter_map(|element| element.value().attr("href").map(|href| href.to_string()))
                    .filter_map(|url| Url::from_str(&url).ok())
                    .collect();

                let scrape_out = ScrapeOut { html, links };
                Some(scrape_out)
            }
        }
    }

    pub async fn get_and_scrape(mut self) -> Result<ScrapeOut> {
        self.get().await?;
        let scrape_out = self
            .scrape() // Always Ok
            .await
            .unwrap();

        Ok(scrape_out)
    }
}

#[derive(Debug)]
pub struct ScrapeOut {
    pub(super) html: String,
    pub(super) links: Vec<Url>
}