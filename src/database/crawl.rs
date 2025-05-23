// CREATE TABLE crawls (id INTEGER PRIMARY KEY, name TEXT, depth INTEGER NOT NULL, root_url TEXT NOT NULL, started_at DATETIME NOT NULL, finished_at DATETIME

use std::borrow::Borrow;

use chrono::{DateTime, Local};
use tokio_rusqlite::{types::ToSqlOutput, Error, ToSql};
use url::Url;

use crate::{sendable_params, utils::wrappers::ToStorable};

use super::database::{Database, DatabaseHandle};

#[derive(Debug, Clone)]
pub struct CrawlDraft {
    pub(super) name: Option<String>,
    pub(super) depth: usize,
    pub(super) root_url: Url,
    pub(super) started_at: DateTime<Local>
}
impl CrawlDraft {
    pub fn new(name: Option<String>, depth: usize, root_url: Url, started_at: DateTime<Local>) -> Self {
        Self { name, depth, root_url, started_at }
    }
}

#[derive(Debug, Clone)]
pub struct CrawlPromise {
    pub(super) id: i64,
    pub(super) inner: CrawlDraft
} 
impl CrawlPromise {
    pub async fn resolve_db(self, finished_at: DateTime<Local>, database: impl Borrow<Database>) -> Crawl {
        let database: &Database = database.borrow();
        
        database.get_handle().await.execute("UPDATE crawls SET finished_at = ?1 WHERE id = ?2;".to_string(), sendable_params!(finished_at.to_storable(), self.id)).await;
        Crawl { id: self.id, name: self.inner.name, depth: self.inner.depth, root_url: self.inner.root_url, started_at: self.inner.started_at, finished_at }
    }

    pub async fn resolve_db_handle(self, finished_at: DateTime<Local>, db_handle: impl Borrow<DatabaseHandle>) -> Crawl {
        let db_handle: &DatabaseHandle = db_handle.borrow();

        // TODO: FIXME: find the right way to deal with this chain of results and stuff
        let _ = db_handle.execute("UPDATE crawls SET finished_at = ?1 WHERE id = ?2;".to_string(), sendable_params!(finished_at.to_storable(), self.id)).await.resolve().await.unwrap().unwrap();

        Crawl { id: self.id, name: self.inner.name, depth: self.inner.depth, root_url: self.inner.root_url, started_at: self.inner.started_at, finished_at }
    }
}

#[derive(Debug, Clone)]
pub struct Crawl {
    id: i64,
    name: Option<String>,
    depth: usize,
    root_url: Url,
    started_at: DateTime<Local>,
    finished_at: DateTime<Local>,
}

pub trait ToCrawlId {
    fn crawl_id(&self) -> i64;
}

impl ToCrawlId for CrawlPromise {
    fn crawl_id(&self) -> i64 {
        self.id
    }
}

impl ToCrawlId for Crawl {
    fn crawl_id(&self) -> i64 {
        self.id
    }
}