use std::sync::Arc;

use tokio_rusqlite::ToSql;
use url::Url;

use crate::Result;
use super::{crawl::{Crawl, CrawlPromise, ToCrawlId}, database::Database};



// id INTEGER PRIMARY KEY, url TEXT NOT NULL, html BLOB, father_id INTEGER, depth INTEGER NOT NULL, crawl_id INTEGER NOT NULL, FOREIGN KEY (crawl_id) REFERENCES crawls(id), FOREIGN KEY (father_id) REFERENCES pages(id))
pub struct PageDraft<T: ToCrawlId> {
    pub url: Arc<Url>,
    pub(super) html: Arc<Vec<u8>>,
    pub(super) father: Option<Arc<PagePromise<T>>>,
    pub(super) depth: usize,
    pub(super) crawl: Arc<T>,
}

impl<T: ToCrawlId> PageDraft<T> {
    pub fn new(url: Arc<Url>, html: Vec<u8>, father: Option<Arc<PagePromise<T>>>, depth: usize, crawl: Arc<T>) -> Self {
        Self { url, html: Arc::new(html), father, depth, crawl }
    }

    pub async fn confim(self, db: impl AsRef<Database>) -> Result<PagePromise<T>> {
        let db = db.as_ref();

        db.insert_page(self).await
    }
}

pub struct PagePromise<T: ToCrawlId> {
    pub(super) id: i64,
    pub url: Arc<Url>,
    pub(super) father: Option<Arc<PagePromise<T>>>,
    pub(super) depth: usize,
    pub(super) crawl: Arc<T>,
} 
impl<T: ToCrawlId> PagePromise<T> {
    pub fn get_html() -> Vec<u8> {
        todo!()
    }
}

pub struct Page {
    id: i64,
    url: Arc<Url>,
    father: Option<Arc<Page>>,
    depth: usize,
    crawl: Arc<Crawl>,
}

