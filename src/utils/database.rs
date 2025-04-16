use std::{borrow::Borrow, fs::{File, OpenOptions}, io::ErrorKind, path::Path};
use r2d2::{Pool, PooledConnection};
use rusqlite::{params, Connection};
use crate::{store::{crawl::Crawl, page::Page}, Result};
use super::{compress::Compress, wrappers::DateTime};
use r2d2_sqlite::SqliteConnectionManager;

pub struct Database {
    pool: Pool<SqliteConnectionManager>
}

impl Database {
    pub const DATABASE_NAME: &str = "icepick_crawl_data.db";
    pub const MAX_SIZE_POOL: u32 = 15;

    pub fn new(path_name: impl AsRef<Path>) -> Result<Self> {
        let mut make_db = false;

        // Check if the database exists
        if let Err(e) = File::open(&path_name) {
            match e.kind() {
                ErrorKind::NotFound => {
                    OpenOptions::new().write(true).read(true).create_new(true).open(&path_name)?;
                    make_db = true;
                },
                _ => return Err(e.into()),
            }
        }

        if make_db {
            Self::make_db(&path_name)?;
        }

        // Set up connection pooling
        let pool = Self::open_connection(path_name)?;

        Ok(Self { pool })
    }

    fn make_db(path_name: impl AsRef<Path>) -> Result<()> {
        let connection = Connection::open(path_name)?;
        
        // ? new table "crawls", (id, name(nullable), depth, root_url, started_at, finished_at)
        connection.execute("CREATE TABLE crawls (id INTEGER PRIMARY KEY, name TEXT, depth INTEGER NOT NULL, root_url TEXT NOT NULL, started_at DATETIME NOT NULL, finished_at DATETIME);", ())?;
        
        // ? new table "pages", (id, url, html(nullable), father_id(nullable) -> pages(id), depth, response_code, time_ms, crawl_id -> crawls(id))
        connection.execute("CREATE TABLE pages (id INTEGER PRIMARY KEY, url TEXT NOT NULL, html BLOB, father_id INTEGER, depth INTEGER NOT NULL, response_code INTEGER NOT NULL, time_ms INTEGER, crawl_id INTEGER NOT NULL, FOREIGN KEY (crawl_id) REFERENCES crawls(id), FOREIGN KEY (father_id) REFERENCES pages(id));", ())?;
        
        Ok(())
    }

    pub fn get_conn(&self) -> Result<PooledConnection<SqliteConnectionManager>> {
        let connection = self.pool.get()?;
        Ok(connection)
    }

    fn open_connection(path_database: impl AsRef<Path>) -> Result<Pool<SqliteConnectionManager>> {        
        let manager = SqliteConnectionManager::file(path_database);
        let pool = Pool::builder()
            .max_size(Self::MAX_SIZE_POOL) // Adjust the max size as needed
            .build(manager)?;
        
        return Ok(pool);
    }

    /// ## Usage
    /// inserts a new record in `crawls`.
    /// returns the assigned id of the crawl.
    pub async fn insert_crawl(
        &self,
        name: Option<impl AsRef<str>>,
        root_url: impl AsRef<str>,
        depth: usize,
        started_at: impl Borrow<DateTime>,
        finished_at: impl Borrow<DateTime>,
    ) -> Result<i64> {
        let name = name.and_then(|x| Some(x.as_ref().to_string()));
        let root_url = root_url.as_ref().to_string();
        let started_at = started_at.borrow().to_string();
        let finished_at = finished_at.borrow().to_string();

        let pool = self.pool.clone(); // or whatever you need to pass in

        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?; // replace with actual logic
            let trans = conn.transaction()?;

            let parameters = params![name, root_url, depth, started_at, finished_at];

            trans.execute(
                "INSERT INTO crawls (name, root_url, depth, started_at, finished_at) VALUES (?1, ?2, ?3, ?4, ?5);",
                parameters,
            )?;

            let id = trans.last_insert_rowid();
            trans.commit()?;

            Ok(id)
        })
        .await?
    }

    pub async fn insert_page(
        &self, 
        crawl: impl Borrow<Crawl>, 
        url: impl AsRef<str>,
        html: &Option<impl AsRef<str>>, 
        father: &Option<impl Borrow<Page>>, 
        depth: usize, 
        response_code: u16,
        time_ms: u32
    ) -> Result<i64> {
        let url = url.as_ref().to_string();
        let father_id = father.as_ref().and_then(|inner| Some(Borrow::<Page>::borrow(inner).id));
        let crawl_id = {let crawl_borrow: &Crawl = crawl.borrow(); crawl_borrow.id};
        // compressed
        let html = html.as_ref().and_then(|inner| Some(inner.as_ref().to_string().compress().ok())).flatten();

        let pool = self.pool.clone(); 
    
        tokio::task::spawn_blocking(move || {
            let mut conn = pool.get()?;
            let trans = conn.transaction()?;
    
            let parameters = params![url, html, father_id, depth, response_code, time_ms, crawl_id];
            // ? new table "pages", (id, url, html(nullable), father_id(nullable) -> pages(id), depth, response_code, time_ms, crawl_id -> crawls(id))

            trans.execute(
                "INSERT INTO pages (url, father_id, html, depth, response_code, time_ms, crawl_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
                parameters,
            )?;
    
            let id = trans.last_insert_rowid();
            trans.commit()?;
    
            Ok(id)
        })
        .await?
    }
}