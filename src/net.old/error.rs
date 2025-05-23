use thiserror::Error;
use trust_dns_resolver::error::ResolveError;
use url::Url;
use reqwest::Error as ReqwestError;

#[derive(Debug, Error)]
pub enum NetError {
    #[error("Dns resolver error: {0}")]
    DNSResolverError(#[from] ResolveError),

    #[error("Couldn't find any IPs that resolve from \'{0}\'")]
    IPFromDnsResoveNotFound(String),

    #[error("Url \'{0}\' doesn't have a domain")]
    NoDomain(Url),

    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
}