use std::collections::HashMap;
use std::sync::Arc;

use futures::future::join_all;
use futures::join;
use reqwest::Client;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use url::Url;

use crate::crawler::scraper::ScrapeOut;
use crate::process_manager::tracker::Tracker;
use crate::store::{crawl::Crawl, page::Page};   
use crate::utils::database::Database;
use crate::Result;
use crate::utils::wrappers::DateTime;

use super::scraper::Scraper;

pub struct Crawler {
    reqwest_client: Arc<Client>,
    scrapers: Vec<Scraper>,
    starting_url: Url,
    final_depth: usize,
}
impl Crawler {
    pub fn new(client: Arc<Client>, starting_url: Url, final_depth: usize) -> Self {
        let reqwest_client = client;
        let scraper_vec: Vec<Scraper> = vec![];
        
        Self { reqwest_client, scrapers: scraper_vec, starting_url, final_depth }
    }

pub async fn crawl(self, database: Database, name: String) -> JoinHandle<Result<(Crawl, Vec<Page>)>> {
    todo!();
    tokio::spawn(async move {
        let crawl = Crawl::new(&database, Some(name), &self.starting_url, self.final_depth, DateTime::now(), DateTime::now()); // TODO_FIXME: remove the dates 

        let mut pages = Vec::new();

        let mut process_manager = Tracker::new(async move |scraper_info: (Arc<Client>, Url)| -> Result<ScrapeOut> { 
            let scraper = Scraper::new(scraper_info.0, scraper_info.1);
            scraper.get_and_scrape().await 
        });
        
        let scrapeout = process_manager.submit((self.reqwest_client.clone(), self.starting_url.clone()));

        let (crawl, first_scrape) = join!(crawl, scrapeout);

        let crawl = crawl?;
        let first_scrape = first_scrape?;

        let first_page = Page::new(&database, &crawl, None::<Page>, &self.starting_url, Some(first_scrape.html.clone()), 0, first_scrape.response_code, 100).await?; // TODO: REMOVE time_ms
        pages.push(first_page.clone());
        
        let mut n_urls: usize;
        let mut current_to_crawl: Vec<Url> = first_scrape.links;
        let mut current_father = Arc::new(first_page);

        let mut hashmap_tracker: HashMap<&Url, &Url> = HashMap::new(); // find a lib w/ the faster hashmap 4 this
        let mut pages_sql_calls = Vec::new();

        for depth in 1..=self.final_depth {
            
            // let ids = process_manager.push_many(current_to_crawl.into_iter().map(|link| links.into_iter().map(|url| (Arc::clone(&self.reqwest_client), url)).collect::<Vec<_>>()).flatten()).await;
            let ids = process_manager.push_many(current_to_crawl.into_iter().map(|link| (Arc::clone(&self.reqwest_client), link))).await;
            process_manager.poll(ids.len()).await;
            let scrapeouts = process_manager.take_many(ids).await;
            
            let new_scrapes = scrapeouts.into_iter().filter_map(|x| x).filter_map(|x| x.ok()).collect::<Vec<_>>();
            
            hashmap_tracker = new_scrapes.iter().map(|x| x.links.iter().map(|y| (y, &x.url))).flatten().collect::<HashMap<_, _>>(); 
            current_to_crawl = new_scrapes.iter().map(|x| x.links.clone()).flatten().collect(); // TODO: try to find a way not to clone the links here?
            
            let pages = new_scrapes.into_iter().map(|scrape| Page::new(&database, &crawl, Some(hashmap_tracker.get(&scrape.url).and_then(|x| x)), scrape.url, Some(scrape.html), depth, scrape.response_code, 100));
            pages_sql_calls.extend(pages); // TODO_FIXME: maybe don't do all calls together?
        }

        join_all(pages_sql_calls).await;

        Result::Ok((crawl, pages))
    })
}
}