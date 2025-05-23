use thiserror::Error;
use trust_dns_resolver::error::ResolveError;

#[derive(Debug, Error)]
pub enum NetError {
    #[error("Dns resolver error: {0}")]
    DNSResolverError(#[from] ResolveError),

    #[error("Couldn't find any IPs that resolve from \'{0}\'")]
    IPFromDnsResoveNotFound(String),

    #[error("Url provided doesn't have a domain")]
    NoDomain,

    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("The site returned a non-success state.")]
    SiteReturnedNotSuccess
}