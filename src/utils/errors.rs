use std::process::exit;

use thiserror::{self, Error};
use tokio::task::JoinError;

#[derive(Debug, Error)]
pub enum Error {
    #[error("A value wasn't wasn't of the expected kind: {0}")]
    XNotTypeY(String),

    #[error("{0}")]
    GenericError(String),

    #[error("Internet error: {0}")]
    ReqwestError(#[from] reqwest::Error),


    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("SQL error: {0}")]
    RusqliteError(#[from] rusqlite::Error),

    #[error("SQL error: {0}")]
    R2d2Error(#[from] r2d2::Error),

    #[error("Async context error: {0}")]
    JoinError(#[from] JoinError),

    #[error("URL parse error: {0}")]
    UrlParseError(#[from] url::ParseError)
}



pub type Result<T> = core::result::Result<T, Error>;

/// ## Usage
/// provides [`Explain::explain`].
pub trait Explain<T> {
    /// ## Info
    /// [`Result::unwrap()`], but explains the problem based on [`Error`] on exit.
    fn explain(self) -> T;
}

impl<T> Explain<T> for Result<T> {
    fn explain(self) -> T {
        match self {
            Ok(ok) => return ok,
            Err(error) => {
                eprintln!("An error occurred.\n{}", error.to_string());
                exit(1);
            },
        }
    }
} 