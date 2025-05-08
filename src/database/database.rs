use std::arch::x86_64;
use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::ops::{Deref, DerefMut};
use std::{clone, sync::Arc};

use crate::database::page;
use crate::process_manager::proc_manager_new::ProcManager;
// use crate::process_manager::manager::ProcessManager;
// use crate::process_manager::tracker_new::ProcessTracker;
use crate::utils::wrappers::ToStorable;
use crate::{Error, Result};
use scraper::html;
use tokio_rusqlite::{params, Connection, DropBehavior, ToSql, TransactionBehavior};

use super::crawl::{CrawlDraft, CrawlPromise, ToCrawlId};
use super::page::{Page, PageDraft, PagePromise};

pub type SqlArgs = Vec<Box<dyn ToSql + Send>>;
pub type DbProcessManager = ProcManager<(String, SqlArgs), Result<i64>, Arc<Connection>>;

#[macro_export]
macro_rules! sendable_params {
    ($($param:expr),* $(,)?) => {
        {
            use tokio_rusqlite::ToSql;
            let mut tmp = Vec::new();

            $(
                tmp.push(Box::new($param) as Box<(dyn ToSql + Send)>);
            )*

            tmp
        }
    };
}

pub struct Database {
    proc_manager: DbProcessManager,
}

impl Database {
    pub const STANDARD_DB_NAME: &str = "icepick_crawl_data.db";

    async fn init() -> Result<Connection> {
        let path_name = Self::STANDARD_DB_NAME;

        let mut make_db = false;

        // Check if the database exists
        if let Err(e) = File::open(&path_name) {
            match e.kind() {
                ErrorKind::NotFound => {
                    OpenOptions::new()
                        .write(true)
                        .read(true)
                        .create_new(true)
                        .open(&path_name)?;
                    make_db = true;
                }
                _ => return Err(e.into()),
            }
        }

        if make_db {
            Self::make_db(&path_name).await
        } else {
            Ok(Connection::open(&path_name).await?)
        }


    }

    async fn make_db(path_name: &str) -> Result<Connection> {
        let connection = Connection::open(&path_name).await?;

        connection.call(|conn| {
            
            conn.execute("CREATE TABLE crawls (id INTEGER PRIMARY KEY, name TEXT, depth INTEGER NOT NULL, root_url TEXT NOT NULL, started_at DATETIME NOT NULL, finished_at DATETIME);", ())?;
            conn.execute("CREATE TABLE pages (id INTEGER PRIMARY KEY, url TEXT NOT NULL, html BLOB, father_id INTEGER, depth INTEGER NOT NULL, crawl_id INTEGER NOT NULL, FOREIGN KEY (crawl_id) REFERENCES crawls(id), FOREIGN KEY (father_id) REFERENCES pages(id));", ())?;
            Ok(())
        }).await?;

        Ok(connection)
    }

    pub async fn new() -> Result<Self> {
        let connection = Self::init().await?;

        let proc_manager: ProcManager<(String, Vec<Box<dyn ToSql + Send + 'static>>), std::result::Result<i64, Error>, Arc<Connection>> =
            ProcManager::new_with_buffer_with_preprocessed_with_wrapper(
                100,
                connection,
                Arc::new,
                |x, y: (String, Vec<Box<dyn ToSql + Send + 'static>>)| Box::pin(async move {
                    let z = x.call(move |a| {
                        a.execute(
                            y.0.as_str(),
                            &y.1.iter()
                                .map(|b| b.as_ref() as &dyn ToSql)
                                .collect::<Vec<&dyn ToSql>>()[..],
                        )?;

                        let x = a.last_insert_rowid();

                        Ok(x)
                    }).await;

                    z.map_err(|e| Error::DBError(e))
                }),
            )
            .await;

        Ok(Self { proc_manager })
    }
    
    const INSERT_CRAWL_SQL: &str = "INSERT INTO crawls (name, root_url, depth, started_at, finished_at) VALUES (?1, ?2, ?3, ?4, ?5);";
    // pub async fn insert_crawl(&self, crawl_draft: CrawlDraft) -> Result<CrawlPromise> {
    //     let id = self.((Self::INSERT_CRAWL_SQL.to_string(), sendable_params![crawl_draft.name.clone(), crawl_draft.root_url.as_str().to_string(), crawl_draft.depth, crawl_draft.started_at.to_storable(), None::<String>])).await.expect("wtf why did the sender drop early");
        
    //     Ok(CrawlPromise {
    //         id,
    //         inner: crawl_draft
    //     })
    // }
    
    // const INSERT_PAGE_SQL: &str = "INSERT INTO crawls (url, html, father_id, depth, crawl_id) VALUES (?1, ?2, ?3, ?4, ?5);";
    // pub async fn insert_page<T: ToCrawlId>(&self, page_draft: PageDraft<T>) -> Result<PagePromise<T>> {
    //     let id = self.exec((Self::INSERT_PAGE_SQL.to_string(), sendable_params![page_draft.url.as_str().to_string(), page_draft.html, page_draft.father.clone().and_then(|x| Some(x.id)), page_draft.depth, page_draft.crawl.crawl_id()])).await.expect("wtf why did the sender drop early");

    //     Ok(PagePromise {
    //         id,
    //         url: page_draft.url,
    //         crawl: page_draft.crawl,
    //         depth: page_draft.depth,
    //         father: page_draft.father,
    //     })
    // }

    // pub async fn insert_many_pages<T: ToCrawlId>(&self, page_drafts: Vec<&PageDraft<T>>) -> Result<Vec<Result<PagePromise<T>>>> {

    //     let non_html_data = page_drafts.iter().map(|x| (Arc::clone(&x.crawl), x.depth, x.father.as_ref().and_then(|y| Some(Arc::clone(&y))), Arc::clone(&x.url))).collect::<Vec<_>>();
    //     let htmls = page_drafts.into_iter().map(|x| Arc::clone(&x.html));
        
    //     self.push_many(non_html_data.iter().zip(htmls).map(|(single_draft_data, html_data)| (Self::INSERT_PAGE_SQL.to_string(), sendable_params![single_draft_data.3.as_str().to_string(), html_data, single_draft_data.2.as_ref().and_then(|x| Some(x.id)), single_draft_data.1, single_draft_data.0.crawl_id()])));

    //     let batch = self.process().await;

    //     let ids = self.get(batch)
    //         .into_iter()
    //         .zip(non_html_data)
    //         .map(|(id, draft)| {
    //             id.map(|num_id| PagePromise {
    //                 id: num_id,
    //                 url: draft.3,
    //                 crawl: draft.0,
    //                 father: draft.2,
    //                 depth: draft.1
    //             })
    //         }).collect::<Vec<_>>();

    //     Ok(ids)
    // }
}


impl Deref for Database {
    type Target = DbProcessManager;

    fn deref(&self) -> &Self::Target {
        &self.proc_manager
    }
}

impl DerefMut for Database {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.proc_manager
    }
}

#[cfg(test)]
mod tests {}