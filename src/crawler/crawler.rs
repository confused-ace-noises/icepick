use std::sync::Arc;

use reqwest::Client;
use url::Url;

use super::scraper::Scraper;

pub struct Crawler {
    reqwest_client: Arc<Client>,
    scrapers: Vec<Scraper>,
    starting_url: Url,
    final_depth: usize,
}
impl Crawler {
    pub fn new(client: Client, starting_url: Url, final_depth: usize) -> Self {
        let reqwest_client = Arc::new(client);
        let scraper_vec: Vec<Scraper> = vec![];
        
        Self { reqwest_client, scrapers: scraper_vec, starting_url, final_depth }
    }
}