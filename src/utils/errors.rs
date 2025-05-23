use std::process::exit;
use thiserror::{self, Error};
use tokio::{sync::watch::error::SendError, task::JoinError};
use trust_dns_resolver::error::ResolveError;

use crate::net::net_error::NetError;

#[derive(Debug, Error)]
pub enum Error {
    #[error("A value wasn't wasn't of the expected kind: {0}")]
    XNotTypeY(String),

    #[error("{0}")]
    GenericError(String),

    // #[error("Internet error: {0}")]
    // ReqwestError(#[from] reqwest::Error),

    #[error("Net errror: {0}")]
    NetError(#[from] NetError),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("DB error: {0}")]
    DBError(#[from] tokio_rusqlite::Error),

    #[error("Async context error: {0}")]
    JoinError(#[from] JoinError),

    #[error("URL parse error: {0}")]
    UrlParseError(#[from] url::ParseError),
}

pub type Result<T> = core::result::Result<T, Error>;

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Error::NetError(NetError::ReqwestError(value))
    }
}

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
            Ok(ok) => ok,
            Err(error) => {
                eprintln!("An error occurred.\n{}", error);
                exit(1);
            },
        }
    }
} 