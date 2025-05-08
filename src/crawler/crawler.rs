use crate::{
    database::{
        crawl::{Crawl, CrawlDraft, CrawlPromise},
        database::Database,
        page::{Page, PageDraft, PagePromise},
    }, net::req_sender::{scrape, scrape_resp, RequestSender}, process_manager::manager::ProcessManager, utils::compress::Compress, Error, Result
};
use chrono::Local;
use dashmap::DashMap;
use futures::future::join_all;
use reqwest::{Client, Response};
use std::collections::HashMap;
use std::{ops::Deref, result::Result as StdResult, sync::Arc};
use tokio::sync::RwLock;
use trust_dns_resolver::{
    TokioAsyncResolver,
    config::{ResolverConfig, ResolverOpts},
};
use url::Url;

/// more optimized for DNS indexable stuff
pub struct Crawler {
    database: Arc<Database>,
    process_manager: ProcessManager<Arc<Url>, (Result<Response>, Arc<Url>)>,
}

impl Crawler {
    pub async fn new(database: Arc<Database>) -> Self {
        let dns_resolver =
            TokioAsyncResolver::tokio(ResolverConfig::google(), ResolverOpts::default());
        let client = Client::new();
        let reqsender = RequestSender::new(Arc::new(RwLock::new(client)), Arc::new(dns_resolver));

        // here :3
        let process_manager = ProcessManager::<Arc<Url>, (Result<Response>, Arc<Url>)>::new_with_preporcessed_with_wrapper(
            reqsender,
            Arc::new,
            |x, y| async move { (x.send(y.as_ref()).await, y) },
        )
        .await;

        Self {
            process_manager,
            database,
        }
    }

    pub async fn crawl(
        &mut self,
        name: String,
        starting_url: Url,
        final_depth: usize,
    ) -> Result<(Crawl, Vec<Page>)> {
        let starting_url = Arc::new(starting_url);
        let database = Arc::clone(&self.database);

        let started_at = Local::now();

        let crawl_draft = CrawlDraft::new(
            Some(name),
            final_depth,
            starting_url.clone().as_ref().to_owned(),
            started_at,
        );

        let crawl = Arc::new(database.insert_crawl(crawl_draft).await?);

        self.process_manager.push(Arc::clone(&starting_url)).await;
        self.process_manager.process().await;

        let first = self
            .process_manager
            .get_batch()
            .await
            .into_iter()
            .next()
            .unwrap();

        let (response, url) = (first.0?, first.1);

        let text = Arc::new(response.text().await?);

        let links_start =
            crate::net::req_sender::scrape(Arc::clone(&text), Arc::clone(&url)).await?;
        let first_page_draft = PageDraft::new(
            Arc::clone(&starting_url),
            text.compress()?,
            None,
            0,
            Arc::clone(&crawl),
        );

        let first_page_promise = Arc::new(database.insert_page(first_page_draft).await?);

        let links_map = links_start
            .1
            .clone() // ? inevitable, unfortunately, but super cheap because it just clones all the Arc`s
            .into_iter()
            .map(|y| (y, Arc::clone(&first_page_promise)));

        let mut links_store = vec![links_start];

        // *                           child, father
        let mut father_map: HashMap<Arc<Url>, Arc<PagePromise<CrawlPromise>>> = HashMap::new();
        father_map.extend(links_map);

        // let pages_promises = vec![]; 

        for depth in 1..final_depth {
            let next_links = links_store.into_iter().flat_map(|(_, v)| v);

            self.process_manager.push_many(next_links).await;
            self.process_manager.process().await;

            let batch = self
                .process_manager
                .get_batch()
                .await
                .into_iter()
                .filter_map(|(result, url)| result.ok().map(|resp| (resp, url)));

            let scrape_futures = batch
                .map(|(resp, url)| {
                    let crawl_clone = Arc::clone(&crawl);
                    let father_map_clone = father_map.clone();

                    async move {
                        let text = Arc::new(resp.text().await?);
                        let page_draft = PageDraft::new(
                            Arc::clone(&url),
                            text.compress()?,
                            // ! FRAGILEEEE
                            father_map_clone
                                .get(&Arc::clone(&url))
                                .and_then(|a| Some(Arc::clone(&a))), // ! <<<< ! fragile 
                            // ! FRAGILEEEE
                            depth,
                            crawl_clone,
                        );

                        let scrape = scrape(text, Arc::clone(&url))
                            .await
                            .map_err(|e| Error::NetError(e));

                        match scrape {
                            Ok((_, scraped)) => Ok((Arc::new(page_draft), scraped)),
                            Err(e) => Err(e),
                        }
                    }
                })
                .collect::<Vec<_>>();

            let joined = join_all(scrape_futures)
                .await
                .into_iter()
                .filter_map(StdResult::ok)
                .collect::<Vec<_>>();

            father_map.clear();

                for (father, children) in &joined {
                    let (temp_store_children, temp_store_parents): (Vec<_>, Vec<_>) = children
                        .iter()
                        .map(|child| (Arc::clone(&child), Arc::clone(&father)))
                        .unzip();

                    let temp_store_parents = database
                        .insert_many_pages(temp_store_parents.iter().map(Arc::as_ref).collect())
                        .await?
                        .into_iter()
                        .filter_map(Result::ok)
                        .map(Arc::new);

                    let store = temp_store_children.into_iter().zip(temp_store_parents);

                    father_map.extend(store);
                }

                links_store = joined
                    .into_iter()
                    .map(|(x, y)| (Arc::clone(&x.url), y))
                    .collect();
        }

        // last iteration

        let next_links = links_store.into_iter().flat_map(|(_, v)| v);

        self.process_manager.push_many(next_links).await;
        self.process_manager.process().await;

        let batch = self
            .process_manager
            .get_batch()
            .await
            .into_iter()
            .filter_map(|(result, url)| result.ok().map(|resp| (resp, url))); 

        let drafts_future = batch.map(|(response, url)| {
            let father_map_clone = father_map.clone();
            let crawl_clone = Arc::clone(&crawl);
            async move {
                let draft = PageDraft::new(Arc::clone(&url), response.text().await?.compress()?, father_map_clone.get(&url).and_then(|x| Some(Arc::clone(&x))), final_depth, crawl_clone);
                Result::<_>::Ok(draft)
            }
        });

        let page_drafts = join_all(drafts_future).await.into_iter().filter_map(Result::ok).collect::<Vec<_>>();
        

        let promises = database.insert_many_pages(page_drafts).await;


        // ----------------------------

        let finished_at = Local::now();

        let finished_crawl = (*crawl.clone())
            .clone()
            .resolve(finished_at, database)
            .await;

        todo!()

        // Ok(finished_crawl)
    }
}
