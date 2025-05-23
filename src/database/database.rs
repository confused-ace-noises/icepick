use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::process_manager::instructions::execute_single_instr::{ExecuteSingleInstruction, HijackOnceInstruction};
use crate::process_manager::instructions::stream_instr::StreamInstruction;
use crate::process_manager::proc_manager_handle::{ProcManagerHandle, TaskPromise};
// use crate::database::page;
use crate::process_manager::proc_manager_new::ProcManager;
use crate::utils::errors;
// use crate::process_manager::manager::ProcessManager;
// use crate::process_manager::tracker_new::ProcessTracker;
use crate::utils::wrappers::ToStorable;
use crate::{Error, Result};
use chrono::{Local, NaiveDateTime, TimeZone};
use futures::stream;
use futures::{StreamExt, TryStreamExt};
use tokio_rusqlite::{Connection, ToSql};
use url::Url;

use super::crawl::{CrawlDraft, CrawlPromise, ToCrawlId};
use super::page::{PageDraft, PagePromise};

// use super::crawl::{CrawlDraft, CrawlPromise, ToCrawlId};
// use super::page::{Page, PageDraft, PagePromise};

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
    // pub const STANDARD_DB_NAME: &str = "icepick_crawl_data.db";
    pub const STANDARD_DB_NAME: &str = "test.db";


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
            );

        Ok(Self { proc_manager })
    }

    pub async fn get_handle(&self) -> DatabaseHandle {
        DatabaseHandle { proc_manager_handle: self.proc_manager.get_handle() }
    }
}

pub type DbProcessManagerHandle = ProcManagerHandle<(String, SqlArgs), Result<i64>, Arc<Connection>>;


pub struct DatabaseHandle {
    proc_manager_handle: DbProcessManagerHandle
}

impl DatabaseHandle {
    pub async fn execute(&self, sql: String, args: SqlArgs) -> TaskPromise<std::result::Result<i64, errors::Error>> {
        let instruction = ExecuteSingleInstruction::new((sql, args));

        let task = self.proc_manager_handle.get_task_handle(instruction);
        task.send().await.expect("DatabseHandle, execute(): the receiver closed unexpectedly.")
    }

    const INSERT_CRAWL_SQL: &str = "INSERT INTO crawls (name, root_url, depth, started_at, finished_at) VALUES (?1, ?2, ?3, ?4, ?5);";
    pub async fn insert_crawl(&self, crawl_draft: CrawlDraft) -> Result<CrawlPromise> {
        self.execute(
            Self::INSERT_CRAWL_SQL.to_string(), 
            sendable_params![
                crawl_draft.name.clone(), 
                crawl_draft.root_url.as_str().to_string(), 
                crawl_draft.depth, 
                crawl_draft.started_at.to_storable(), 
                None::<String>
            ]
            ).await
            .resolve()
            .await
            .expect("DatabseHandle, insert_crawl(): the sender closed unexpectedly.")
            .map(|id| CrawlPromise { id, inner: crawl_draft } )    
    }

    const INSERT_PAGE_SQL: &str = "INSERT INTO pages (url, html, father_id, depth, crawl_id) VALUES (?1, ?2, ?3, ?4, ?5);";
    pub async fn insert_page<C: ToCrawlId>(&self, page_draft: PageDraft<C>) -> PagePromise<C> {
        let id = self.execute(Self::INSERT_PAGE_SQL.to_string(), sendable_params![page_draft.url.as_str().to_string(), page_draft.html, page_draft.father.clone().and_then(|x| Some(x.id)), page_draft.depth, page_draft.crawl.crawl_id()]).await.resolve().await.expect("DatabseHandle, insert_pages(): receiver dropped unexpectedly").unwrap();

        PagePromise {
            id,
            url: page_draft.url,
            crawl: page_draft.crawl,
            depth: page_draft.depth,
            father: page_draft.father,
            page_kind: page_draft.page_kind
        }
    }

    pub async fn insert_many_pages<C: ToCrawlId>(&self, page_drafts: Vec<PageDraft<C>>) -> Vec<Result<PagePromise<C>>> {
        let mut instruction = StreamInstruction::new();

        // Collect insertion parameters and future result metadata
        let inserts = page_drafts.into_iter().map(|draft| {
            
            let crawl = Arc::clone(&draft.crawl);
            let url = Arc::clone(&draft.url);
            let html = Arc::clone(&draft.html);
            let father = draft.father.as_ref().map(Arc::clone);
            let page_kind = draft.page_kind.clone(); // TODO: look into if this cost is acceptable
            let depth = draft.depth;

            let sql_params = sendable_params![
                url.as_str().to_string(),
                html,
                father.as_ref().map(|f| f.id),
                depth,
                crawl.crawl_id()
            ];

            instruction.push((Self::INSERT_PAGE_SQL.to_string(), sql_params));

            (crawl, depth, father, url, page_kind)
        }).collect::<Vec<_>>();

        let task_handle = self.proc_manager_handle
            .get_task_handle(instruction)
            .send()
            .await
            .expect("insert_many_pages(): sender closed unexpectedly.")
            .resolve()
            .await
            .expect("insert_many_pages(): receiver dropped unexpectedly");

        task_handle
            .into_stream()
            .zip(stream::iter(inserts))
            .map(|(id_res, (crawl, depth, father, url, page_kind))| {
                id_res.map(|id| PagePromise {
                    id,
                    url,
                    crawl,
                    father,
                    depth,
                    page_kind,
                })
            })
            .collect::<Vec<_>>()
            .await
    }

    pub async fn get_page_from_id(&self, id: usize) {
        Some(x)
    }

    pub async fn get_crawl_from_id(&self, id: usize) -> Result<Option<CrawlPromise>> {
        let instruction: HijackOnceInstruction<(String, Vec<Box<dyn ToSql + Send + 'static>>), Result<Option<CrawlPromise>>, Arc<Connection>> = HijackOnceInstruction::new(
            ("SELECT * FROM crawls WHERE id=?1".to_string(), sendable_params!(id)), 
            Arc::new(Box::new(
                |preprocessed, (sql_query, params)| Box::pin(async move {
                    let res = preprocessed.call(move |connection| {
                        let mut statement = connection.prepare(&sql_query).map_err(|err| tokio_rusqlite::Error::Rusqlite(err))?;
                        let params = &params.iter().map(|x| x.as_ref() as &dyn ToSql).collect::<Vec<_>>()[..];
                        let iter = statement.query_map(params, |row| {
                            Ok(
                                CrawlPromise {
                                    id: row.get::<_, i64>(0).expect("FATAL: THE DATABASE FAILED CATASTROPHICALLY"),
                                    inner: CrawlDraft { 
                                        name: row.get::<_, Option<String>>(1).expect("FATAL: THE DATABASE FAILED CATASTROPHICALLY"), 
                                        depth: row.get::<_, usize>(2).expect("FATAL: THE DATABASE FAILED CATASTROPHICALLY"), 
                                        root_url: Url::parse(row.get::<_, String>(3).expect("FATAL: THE DATABASE FAILED CATASTROPHICALLY").as_str()).expect("FATAL: THE DATABASE FAILED CATASTROPHICALLY"), 
                                        started_at: {
                                            let string_rep = row.get::<_, String>(4).expect("FATAL: THE DATABASE FAILED CATASTROPHICALLY");
                                            let naive_date_time = NaiveDateTime::parse_from_str(&string_rep, "%Y-%m-%d %H:%M:%S").unwrap();
                                            let local_date_time = Local.from_local_datetime(&naive_date_time).single().unwrap();

                                            local_date_time
                                        } 
                                    }
                                }
                            )
                        }).unwrap();

                        Ok(iter.map(|x| x.unwrap()).collect::<Vec<CrawlPromise>>())
                    }).await;

                    Ok(
                        res.map(|mut inner| {
                            if inner.len() == 0 {
                                None
                            } else {
                                Some(inner.remove(0))
                            }
                        })?
                    )
                })
            ))
        );

        let res = self.proc_manager_handle.get_task_handle(instruction)
            .send()
            .await
            .expect("fatal: receiver dropped early.")
            .resolve()
            .await
            .expect("fatal: sender dropped early.");

        res
    }
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

// pub struct DbInstruction;

// impl<Input, Output> InstructionExecutor<Input, Output, Output> for DbInstruction {
//     fn new() -> Self {
//         DbInstruction
//     }

//     fn execute<Preprocessed>(
//         self,
//         preprocessed: Preprocessed,
//         process: UserProcess<Input, Output, Preprocessed>,
//     ) -> impl Future<Output = Output> + Send
//     where
//         Preprocessed: Clone + Send + 'static 
//     {
        
//     }
// }

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::{Arc, Weak}};

    use chrono::Local;
    use url::Url;

    use crate::{database::{crawl::CrawlDraft, database::Database, page::{PageDraft, PageKind}}, utils::{compress::Compress, wrappers::ToStorable}};

    #[tokio::test(flavor = "current_thread")]
    pub async fn test_db() {
        let db = Database::new().await.unwrap();

        let handle = db.get_handle().await;

        let crawl_draft = CrawlDraft::new(Some(String::from("test")), 4, Url::from_str("https://askiiart.net").unwrap(), Local::now());

        let id_crawl = handle.insert_crawl(crawl_draft).await.unwrap();

        let arc_crawl = Arc::new(id_crawl);

        let page_draft_1 = PageDraft::new(Arc::new(Url::from_str("https://google.com").unwrap()), Some("hai".to_string().compress().unwrap()), None, 0, arc_crawl.clone(), PageKind::Html);
        
        let page_promise = handle.insert_page(page_draft_1).await;

        let arcd_page_promise = Arc::new(page_promise);
        
        let page_draft_2 = PageDraft::new(Arc::new(Url::from_str("https://hiii.com").unwrap()), Some("haiii2".to_string().compress().unwrap()), Some(arcd_page_promise.clone()), 1, arc_crawl.clone(), PageKind::Html);
        let page_draft_3 = PageDraft::new(Arc::new(Url::from_str("https://nya.net").unwrap()), Some("mrow".to_string().compress().unwrap()), Some(arcd_page_promise.clone()), 1, arc_crawl.clone(), PageKind::Html);
        let page_draft_4 = PageDraft::new(Arc::new(Url::from_str("https://fops.net").unwrap()), Some("*screams*".to_string().compress().unwrap()), Some(arcd_page_promise.clone()), 1, arc_crawl.clone(), PageKind::Html);

        let x = handle.insert_many_pages(vec![page_draft_2, page_draft_3, page_draft_4]).await.into_iter().map(Result::unwrap).collect::<Vec<_>>();

        let crawl = arc_crawl.clone().as_ref().clone().resolve_db_handle(Local::now(), handle).await;

        println!("crawl: {:?},\n page_promise1: {:?},\n page_promise2-4: {:?}", crawl, arcd_page_promise, x);
    }
}