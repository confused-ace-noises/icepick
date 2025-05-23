use std::{str::FromStr, sync::Arc};

use dashmap::DashMap;
use mime::Mime;
use reqwest::{
    Client,
    header::{CONTENT_TYPE, HeaderName, HeaderValue},
};
use url::Url;

use crate::{
    database::{
        crawl::CrawlPromise,
        database::Database,
        page::{PageDraft, PageKind, PagePromise},
    },
    net::{net_error::NetError, reqest_sender::ReqSender},
    process_manager::proc_manager_new::ProcManager,
    utils::compress::Compress,
};

pub struct Crawler {
    database: Arc<Database>,
    proc_manager: ProcManager<
        (Url, usize, Arc<CrawlPromise>, Option<Arc<PagePromise<CrawlPromise>>>),
        Result<PageDraft<CrawlPromise>, NetError>,
        Arc<ReqSender>,
    >,
}
impl Crawler {
    pub async fn new(database: Arc<Database>, reqsender: ReqSender) -> Self {
        let proc_manager: ProcManager<(Url, usize, Arc<CrawlPromise>, Option<Arc<crate::database::page::PagePromise<CrawlPromise>>>), Result<PageDraft<CrawlPromise>, NetError>, Arc<ReqSender>> = ProcManager::new_with_buffer_with_preprocessed_with_wrapper(
            100,
            reqsender,
            Arc::new,
            |req_sender, (url, depth, crawl_promise, father)| {
                Box::pin(async move {
                    let response = req_sender.make_request(&url).await?;

                    if !response.status().is_success() {
                        return Err(NetError::SiteReturnedNotSuccess);
                    }

                    let content_type = response
                        .headers()
                        .get(CONTENT_TYPE)
                        .map(HeaderValue::to_str)
                        .map(Result::ok)
                        .flatten();

                    let page_kind: PageKind;

                    match content_type {
                        Some("text/html" | "text/xhtml+xml") => page_kind = PageKind::Html,

                        Some("text/plain" | "text/markdown") => page_kind = PageKind::PlainText,

                        Some(_) => page_kind = PageKind::NonSearchable,

                        _ => page_kind = PageKind::Unknown,
                    }

                    match page_kind {
                        PageKind::Html | PageKind::PlainText => {
                            match response.text().await {
                                Ok(text) => {
                                    let compressed = text.compress().unwrap(); // You might want to handle compress errors
                                    Ok(PageDraft::<CrawlPromise>::new(
                                        url.into(),
                                        Some(compressed),
                                        father,
                                        depth,
                                        crawl_promise,
                                        page_kind,
                                    ))
                                }
                                Err(_) => Ok(PageDraft::new(
                                    Arc::new(url),
                                    None,
                                    father,
                                    depth,
                                    crawl_promise,
                                    PageKind::Unknown,
                                )),
                            }
                        }
                        PageKind::NonSearchable | PageKind::Unknown => Ok(PageDraft::new(
                            Arc::new(url),
                            None,
                            father,
                            depth,
                            crawl_promise,
                            page_kind,
                        )),
                    }
                })
            },
        );

        Self {
            database,
            proc_manager
        }
    }
}
