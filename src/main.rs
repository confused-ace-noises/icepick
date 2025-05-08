// use icepick::{database::{crawl::Crawl, page::Page}, utils::{database::Database, wrappers::{DateTime, ToUrlStr}}};
use tokio;

#[tokio::main]
async fn main() {
    // let database = Database::new(Database::DATABASE_NAME).unwrap();

    // let current_crawl = Crawl::new(&database, Some(String::from("test crawl")), "https://google.com".to_url().unwrap(), 2, DateTime::now(), DateTime::now()).await.unwrap();
    // let page1 = Page::new(&database, &current_crawl, None::<Page>, "https://askiiart.net".to_url().unwrap(), None::<String>, 2, 404, 100).await.unwrap();
    // let page2 = Page::new(&database, &current_crawl, None::<Page>, "https://askiiart.com".to_url().unwrap(), None::<String>, 1, 401, 102).await.unwrap();

}