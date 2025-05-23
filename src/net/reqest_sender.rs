#![warn(unused_allocation)]

use std::{sync::Arc, time::Duration};

use dashmap::DashMap;
use reqwest::{Client, Response};
use url::Url;
use xxhash_rust::xxh3::xxh3_64;

use super::{net_error::NetError, rate_limiter::RateLimiter};

pub struct ReqSender {
    client: Arc<Client>,
    rate_limiting_duration: Duration,
    rate_lims: Arc<DashMap<u64, Arc<RateLimiter>>>,
}

impl ReqSender {
    pub async fn make_request(&self, target: &Url) -> Result<Response, NetError> {
        let domain = target.domain();
        let as_str_url = target.as_str();
        match domain {
            Some(domain) => {
                let hash = xxh3_64(domain.as_bytes());

                let rate_lim= {
                    if !self.check_rate_limiter_hash(hash) {
                        self.add_rate_limiter_hash(hash);
                    }

                    self.get_rate_limiter_hash(hash).unwrap() //? can't fail
                };

                rate_lim.wait().await;
                let resp = self.client.get(as_str_url).send().await?;
                Ok(resp)
            },

            None => Err(NetError::NoDomain),
        } 
    }

    fn check_rate_limiter_hash(&self, hash_to_check: u64) -> bool {
        self.rate_lims.contains_key(&hash_to_check)
    }

    #[allow(dead_code)]
    fn check_rate_limiter_domain(&self, domain_to_check: impl AsRef<str>) -> bool {
        self.check_rate_limiter_hash(xxh3_64(domain_to_check.as_ref().as_bytes()))
    }

    fn get_rate_limiter_hash(&self, hash: u64) -> Option<Arc<RateLimiter>> {
        self.rate_lims
            .get(&hash)
            .map(|inner| Arc::clone(&inner))
    }

    #[allow(dead_code)]
    fn get_rate_limiter_domain(&self, domain: impl AsRef<str>) -> Option<Arc<RateLimiter>> {
        self.get_rate_limiter_hash(xxh3_64(domain.as_ref().as_bytes()))
    }

    fn add_rate_limiter_hash(&self, hash: u64) {
        let rate_lim = RateLimiter::new(self.rate_limiting_duration);

        self.rate_lims.insert(hash, Arc::new(rate_lim));
    }

    #[allow(dead_code)]
    fn add_rate_limiter_domain(&self, domain: impl AsRef<str>) -> u64 {
        let hash = xxh3_64(domain.as_ref().as_bytes());
        self.add_rate_limiter_hash(hash);
        hash
    }
}

impl Clone for ReqSender {
    fn clone(&self) -> Self {
        Self { client: Arc::clone(&self.client), rate_limiting_duration: self.rate_limiting_duration, rate_lims: Arc::clone(&self.rate_lims) }
    }
}