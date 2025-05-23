use super::rate_limiter::RateLimiter;
use crate::net::error::NetError;
use crate::{Error, Result};
use std::borrow::Borrow;
use std::result::Result as StdResult;
use std::str::FromStr;
use dashmap::DashMap;
use reqwest::{Client, Response};
use scraper::{Html, Selector};
use std::rc::Rc;
use std::{net::IpAddr, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use trust_dns_resolver::TokioAsyncResolver;
use url::Url;

/// trivially clonable
#[derive(Debug)]
pub struct RequestSender {
    reqwest_client: Arc<RwLock<Client>>,
    rate_limiter_map: Arc<DashMap<IpAddr, RateLimiter>>,
    domain_to_ip_map: Arc<DashMap<String, IpAddr>>,
    dns_resolver: Arc<TokioAsyncResolver>,
    duration_new_ratelimiters: Duration,
}

impl Clone for RequestSender {
    fn clone(&self) -> Self {
        Self {
            reqwest_client: Arc::clone(&self.reqwest_client),
            rate_limiter_map: Arc::clone(&self.rate_limiter_map),
            domain_to_ip_map: Arc::clone(&self.domain_to_ip_map),
            dns_resolver: Arc::clone(&self.dns_resolver),
            duration_new_ratelimiters: self.duration_new_ratelimiters,
        }
    }
}

impl RequestSender {
    pub const STANDARD_NEW_RATELIMITERS_DURATION_MS: u64 = 200;

    pub fn new(reqwest_client: Arc<RwLock<Client>>, dns_resolver: Arc<TokioAsyncResolver>) -> Self {
        Self::new_with_ratelimiter_duration(
            reqwest_client,
            dns_resolver,
            Duration::from_millis(Self::STANDARD_NEW_RATELIMITERS_DURATION_MS),
        )
    }

    pub fn new_with_ratelimiter_duration(
        reqwest_client: Arc<RwLock<Client>>,
        dns_resolver: Arc<TokioAsyncResolver>,
        duration_new_ratelimiters: Duration,
    ) -> Self {
        Self {
            reqwest_client,
            rate_limiter_map: Arc::new(DashMap::new()),
            domain_to_ip_map: Arc::new(DashMap::new()),
            dns_resolver,
            duration_new_ratelimiters: duration_new_ratelimiters,
        }
    }

    pub async fn resolve_dns(domain: &str, resolver: &TokioAsyncResolver) -> Result<IpAddr> {
        // let resolver = TokioAsyncResolver::tokio_from_system_conf()?;
        let response = resolver
            .lookup_ip(domain)
            .await
            .map_err(|e| crate::Error::NetError(super::error::NetError::DNSResolverError(e)));

        // Return the first IP address found (IPv4 or IPv6)
        let x = response
            .into_iter()
            .next()
            .ok_or(NetError::IPFromDnsResoveNotFound(domain.to_string()))?;

        Ok(x.into_iter()
            .next()
            .ok_or(NetError::IPFromDnsResoveNotFound(domain.to_string()))?)
    }

    pub async fn resolve_ip<'a>(&'a self, url: &'a Url) -> Result<IpAddr> {
        let domain_to_ip_map = Arc::clone(&self.domain_to_ip_map);

        let domain = url.domain().and_then(|s| Some(String::from(s)));
        match domain {
            Some(domain) => {
                if let Some(ip) = domain_to_ip_map.get(&domain) {
                    Ok(*ip)
                } else {
                    let resolved =
                        Self::resolve_dns(&domain, Arc::clone(&self.dns_resolver).as_ref()).await;

                    match resolved {
                        Ok(ip) => {
                            self.domain_to_ip_map.insert(domain, ip);
                            self.rate_limiter_map.insert(
                                ip,
                                RateLimiter::new(
                                    self.duration_new_ratelimiters,
                                ),
                            );
                            Ok(ip)
                        }

                        Err(
                            e @ Error::NetError(
                                NetError::DNSResolverError(_)
                                | NetError::IPFromDnsResoveNotFound(_),
                            ),
                        ) => {
                            return Err(e);
                        }

                        _ => unimplemented!(), // isn't ever thrown
                    }
                }
            }

            None => {
                return Err(Error::NetError(NetError::NoDomain(url.clone())));
            }
        }
    }

    pub async fn resolve_ip_owned(&self, url: Url) -> Result<IpAddr> {
        let domain_to_ip_map = Arc::clone(&self.domain_to_ip_map);

        match url.domain().and_then(|s| Some(String::from(s))) {
            Some(domain) => {
                if let Some(ip) = domain_to_ip_map.get::<str>(domain.as_ref()) {
                    Ok(*ip)
                } else {
                    let resolved =
                        Self::resolve_dns(domain.as_ref(), Arc::clone(&self.dns_resolver).as_ref()).await;

                    match resolved {
                        Ok(ip) => {
                            self.domain_to_ip_map.insert(domain, ip);
                            self.rate_limiter_map.insert(
                                ip,
                                RateLimiter::new(
                                    self.duration_new_ratelimiters,
                                ),
                            );
                            Ok(ip)
                        }

                        Err(
                            e @ Error::NetError(
                                NetError::DNSResolverError(_)
                                | NetError::IPFromDnsResoveNotFound(_),
                            ),
                        ) => {
                            return Err(e);
                        }

                        _ => unimplemented!(), // isn't ever thrown
                    }
                }
            }

            None => {
                return Err(Error::NetError(NetError::NoDomain(url.clone())));
            }
        }
    }

    pub async fn send_owned(&self, url: Url) -> Result<Response> {
        let url = url;
        let ip = self.resolve_ip_owned(url.clone()).await;
    
        match ip {
            Ok(ip) => {
                let rate_limiter = self.rate_limiter_map
                    .get(&ip)
                    .expect("Missing IP in rate limiter map");
                rate_limiter.value().wait().await;
                let resp = self
                    .reqwest_client
                    .read()
                    .await
                    .get(url)
                    .send()
                    .await
                    .map_err(NetError::ReqwestError)?;
                Ok(resp)
            }
            Err(Error::NetError(NetError::IPFromDnsResoveNotFound(_)
                | NetError::NoDomain(_))) => {
                // Fallback without rate limiter
                tokio::time::sleep(Duration::from_millis(
                    Self::STANDARD_NEW_RATELIMITERS_DURATION_MS,
                )).await;
                let resp = self
                    .reqwest_client
                    .read()
                    .await
                    .get(url.as_str())
                    .send()
                    .await
                    .map_err(NetError::ReqwestError)?;
                Ok(resp)
            }
            Err(e @ Error::NetError(NetError::DNSResolverError(_))) => Err(e),
            _ => unimplemented!(),
        }
    }

    pub async fn send<'a>(&'a self, url: &'a Url) -> Result<Response> {
        let rate_limiter_map = Arc::clone(&self.rate_limiter_map);

        let ip = self.resolve_ip(&url).await;

        match ip {
            Ok(ip) => {
                let rate_limiter = rate_limiter_map.get(&ip).expect("Fatal: 'ip' of type 'IpAddress' isn't present in 'self.rate_limiter_map' when it should.");
                rate_limiter.value().wait().await;
                let resp = self
                    .reqwest_client
                    .read()
                    .await
                    .get(url.as_str())
                    .send()
                    .await
                    .map_err(|reqwest_err| NetError::ReqwestError(reqwest_err))?;
                Ok(resp)
            }
            Err(Error::NetError(NetError::IPFromDnsResoveNotFound(_) | NetError::NoDomain(_))) => {
                //? try to do it without a rate limiter
                tokio::time::sleep(Duration::from_millis(
                    Self::STANDARD_NEW_RATELIMITERS_DURATION_MS,
                ))
                .await;
                let resp = self
                    .reqwest_client
                    .read()
                    .await
                    .get(url.as_str())
                    .send()
                    .await
                    .map_err(|reqwest_err| NetError::ReqwestError(reqwest_err))?;
                Ok(resp)
            }
            Err(e @ Error::NetError(NetError::DNSResolverError(_))) => return Err(e),

            _ => unimplemented!(),
        }
    }
}

pub async fn scrape(text: impl Borrow<String>, url_of_resp: Arc<Url>) -> StdResult<(Arc<Url>, Vec<Arc<Url>>), NetError> {
    let url = url_of_resp;
    // let text = resp.text().await?;
    let document = Html::parse_document(&text.borrow());
    let selector = Selector::parse("a").unwrap();
    Ok((url, 
        document.select(&selector)
            .filter_map(|element| element.value().attr("href").map(|href| href.to_string()))
            .filter_map(|url| Url::from_str(&url).ok())
            .map(Arc::new)
            .collect())
    )
}

pub async fn scrape_resp(resp: Response, url_of_resp: Arc<Url>) -> StdResult<(Arc<Url>, Vec<Arc<Url>>), NetError> {
    scrape(&resp.text().await?, url_of_resp).await
}