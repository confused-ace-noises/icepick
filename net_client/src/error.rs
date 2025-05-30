use thiserror::Error;


#[derive(Debug, Error)]
pub enum Error {
    #[error("Url provided doesn't have a domain")]
    NoDomain,

    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("The site returned a non-success state.")]
    SiteReturnedNotSuccess
}