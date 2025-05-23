use std::sync::Arc;

use mime::Mime;
use tokio_rusqlite::ToSql;
use url::Url;

use crate::Result;
use super::{crawl::{Crawl, CrawlPromise, ToCrawlId}, database::Database};

#[derive(Clone, Debug)]
pub enum PageKind {
    Html,
    PlainText,
    NonSearchable,
    Unknown
}


// id INTEGER PRIMARY KEY, url TEXT NOT NULL, html BLOB, father_id INTEGER, depth INTEGER NOT NULL, crawl_id INTEGER NOT NULL, FOREIGN KEY (crawl_id) REFERENCES crawls(id), FOREIGN KEY (father_id) REFERENCES pages(id))
pub struct PageDraft<T: ToCrawlId> {
    pub url: Arc<Url>,
    pub(super) html: Arc<Option<Vec<u8>>>,
    pub(super) father: Option<Arc<PagePromise<T>>>,
    pub(super) depth: usize,
    pub(super) crawl: Arc<T>,
    pub(super) page_kind: PageKind,
}

impl<T: ToCrawlId> PageDraft<T> {
    pub fn new(url: Arc<Url>, html: Option<Vec<u8>>, father: Option<Arc<PagePromise<T>>>, depth: usize, crawl: Arc<T>, page_kind: PageKind) -> Self {
        Self { url, html: Arc::new(html), father, depth, crawl, page_kind }
    }

    // pub async fn confim(self, db: impl AsRef<Database>) -> Result<PagePromise<T>> {
    //     let db = db.as_ref();

    //     db.insert_page(self).await
    // }
}

#[derive(Debug)]
pub struct PagePromise<T: ToCrawlId> {
    pub(super) id: i64,
    pub url: Arc<Url>,
    pub(super) father: Option<Arc<PagePromise<T>>>,
    pub(super) depth: usize,
    pub(super) crawl: Arc<T>,
    pub(super) page_kind: PageKind,
} 
impl<T: ToCrawlId> PagePromise<T> {
    pub fn get_html() -> Vec<u8> {
        todo!()
    }

    // pub fn resolve(self, crawl: Arc<Crawl>, father: ) -> Page {
    //     Page { id: self.id, url: self.url, father: self.father, depth: self.depth, crawl: crawl }
    // }
}

pub struct Page {
    id: i64,
    url: Arc<Url>,
    father: Option<Arc<Page>>,
    depth: usize,
    crawl: Arc<Crawl>,
    page_kind: PageKind,
}

