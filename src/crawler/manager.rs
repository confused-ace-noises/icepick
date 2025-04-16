use std::{
    collections::VecDeque, ops::{Deref, DerefMut}, sync::Arc, thread::JoinHandle, vec
};

use super::scraper::{ScrapeOut, Scraper};
use chrono::Local;
use reqwest::Client;
use tokio::sync::{
    Mutex,
    mpsc::{self, Receiver, Sender},
};

static mut ID: usize = 0;

#[derive(Debug)]
pub struct ScrapeManager {
    reqwest_client: Arc<Client>, // owned
    vec_scrapers: Arc<Mutex<VecDeque<(usize, Scraper)>>>,
    out_tx: Sender<ScrapeOut>,
    out_rx: Receiver<ScrapeOut>,
    thread_handle: JoinHandle<()>,
}
impl ScrapeManager {
    pub fn new(client: Client) -> Self {
        // let (out_tx, out_rx) = mpsc::channel(100);
        
        // let vec_scrapers: Arc<Mutex<VecDeque<Scraper>>> = Arc::new(Mutex::new(VecDeque::new()));

        // let client = Arc::new(client);

        // let x = tokio::spawn(async move {
        //     loop {
        //         let url_opt = {
        //             let mut q = queue.lock().await;
        //             q.pop_front()
        //         };

        //         if let Some(url) = url_opt {
        //             // download HTML
        //             let html = match reqwest::get(&url).await {
        //                 Ok(resp) => resp.text().await.unwrap_or_default(),
        //                 Err(_) => continue,
        //             };

        //             for scraper in &scrapers {
        //                 let html_clone = html.clone();
        //                 let tx_clone = tx.clone();
        //                 let scraper = scraper.clone();
        //                 tokio::spawn(async move {
        //                     let out = scraper.scrape(&html_clone).await;
        //                     let _ = tx_clone.send(out).await;
        //                 });
        //             }
        //         } else {
        //             tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        //         }
        //     }
        // });

        // Self {
        //     reqwest_client: client,
        //     vec_scrapers,
        //     out_rx,
        //     out_tx,
        // };

        // let thing = async move {
        //         loop {
        //             let url_opt = {
        //                 let mut q = queue.lock().await;
        //                 q.pop_front()
        //             };
    
        //             if let Some(url) = url_opt {
        //                 // download HTML
        //                 let html = match reqwest::get(&url).await {
        //                     Ok(resp) => resp.text().await.unwrap_or_default(),
        //                     Err(_) => continue,
        //                 };
    
        //                 for scraper in &scrapers {
        //                     let html_clone = html.clone();
        //                     let tx_clone = tx.clone();
        //                     let scraper = scraper.clone();
        //                     tokio::spawn(async move {
        //                         let out = scraper.scrape(&html_clone).await;
        //                         let _ = tx_clone.send(out).await;
        //                     });
        //                 }
        //             } else {
        //                 tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        //             }
        //         }
        //     };
        // tokio::spawn(future)
        todo!()
    }

    /// returns the id of whatever was passed in
    pub async fn push(&mut self, scraper: Scraper) -> usize {
        let vec_scrapers = self.vec_scrapers.clone();
        let mut lock = vec_scrapers.lock().await;
        
        unsafe { 
            lock.push_back((ID, scraper)) 
        };
        
        drop(lock);
        
        unsafe {
            ID += 1;
            return ID - 1;
        };
    }
}
