use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;

use reqwest::header;
use reqwest::Client as ReqwestClient;
use reqwest::ClientBuilder as ReqwestClientBuilder;
use reqwest::Response;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::trace;
use tracing::warn;
use url::Url;
use xxhash_rust::xxh3::xxh3_64;

use crate::error::Error;
use crate::rate_limiter::RateLimiter;

pub struct Client {
    client: Arc<ReqwestClient>,
    rate_limiting_duration: Duration,
    rate_lims: Arc<DashMap<u64, Arc<RateLimiter>>>,
}

impl Client {
    fn reqwest_client() -> ReqwestClient {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static(
                "Mozilla/5.0 (compatible; IcepickCrawler/1.0)",
            ),
        );

        ReqwestClient::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(5))
            .pool_max_idle_per_host(20)
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .expect("Failed to build reqwest client")
    }    

    pub async fn new(duration: Duration) -> Self {
        let client = Arc::new(Self::reqwest_client());
        let rate_lims = Arc::new(DashMap::new());

        Self {
            client,
            rate_lims,
            rate_limiting_duration: duration
        }
    }

    async fn _get(&self, target: &Url, rate_lim: &RateLimiter, name: &str) -> Result<Response, Error> {
        let target = target.as_str();
        rate_lim.wait(target).await;

        for retry_number in 0_u8..=2 {

            match self.client.get(target).send().await {
                
                Ok(resp) if resp.status().is_success() => {
                    info!(url=target, status=?resp.status(), "successful fetch");
                    return Ok(resp);
                },

                Ok(resp) => {
                    warn!(url=target, status=?resp.status(), "non-success response");
                    // aka, just give up and return the not successful response
                    if retry_number >= 2 {
                        return Ok(resp);
                    }
                },

                Err(e) => {
                    error!(url=target, error=%e, attempt=retry_number, "request failed");
                    if retry_number >= 2 {
                        return Err(e.into())
                    }
                }
            }

            rate_lim.wait(name).await
        }

        unimplemented!()
    }

    pub async fn make_request(&self, target: &Url) -> Result<Response, Error> {
        let as_str_url = target.as_str();
        trace!(url = as_str_url, "starting request making");
        let domain = target.domain();
        
        match domain {
            Some(domain) => {
                trace!(url = as_str_url, domain, "Continuing: found domain in url");
                let hash = xxh3_64(domain.as_bytes());

                let rate_lim= {
                    if !self.check_rate_limiter_hash(hash) {
                        self.add_rate_limiter_hash(hash);
                    }

                    self.get_rate_limiter_hash(hash).unwrap() //? can't fail
                };

                debug!(url = as_str_url, "sending request to: {}", as_str_url);
                let resp = self._get(target, &rate_lim, domain).await?;
                Ok(resp)
            },

            None => {
                trace!(soft_error=true, url = as_str_url, "no domain found in url");
                Err(Error::NoDomain)
            },
        } 
    }
    
    fn check_rate_limiter_hash(&self, hash_to_check: u64) -> bool {
        trace!(hash=hash_to_check, "checking if domain table contains hash");
        self.rate_lims.contains_key(&hash_to_check)
    }

    #[allow(dead_code)]
    fn check_rate_limiter_domain(&self, domain_to_check: impl AsRef<str>) -> bool {
        self.check_rate_limiter_hash(xxh3_64(domain_to_check.as_ref().as_bytes()))
    }

    fn get_rate_limiter_hash(&self, hash: u64) -> Option<Arc<RateLimiter>> {
        trace!(hash, "getting rate limiter instance from hash");
        self.rate_lims
            .get(&hash)
            .map(|inner| Arc::clone(&inner))
    }

    #[allow(dead_code)]
    fn get_rate_limiter_domain(&self, domain: impl AsRef<str>) -> Option<Arc<RateLimiter>> {
        self.get_rate_limiter_hash(xxh3_64(domain.as_ref().as_bytes()))
    }

    fn add_rate_limiter_hash(&self, hash: u64) {
        trace!(hash, duration=self.rate_limiting_duration.as_millis(), "adding new rate limiter with hash");
        
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

#[cfg(test)]
mod tests {
    use std::{str::FromStr, time::Duration};

    use tokio::join;
    use url::Url;

    use super::Client;

    #[tokio::test(flavor = "multi_thread", worker_threads = 6)]
    pub async fn test() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("trace")
            .with_test_writer()
            .try_init()
            .unwrap();

        let client = Client::new(Duration::from_secs(1)).await;

        let binding = Url::from_str("https://askiiart.net/").unwrap();
        let resp1 = client.make_request(&binding);
        
        let binding = Url::from_str("https://www.fanton.com/").unwrap();
        let resp2 = client.make_request(&binding);

        let (resp1, resp2) = join!(resp1, resp2);

        resp1.unwrap();
        resp2.unwrap();
    }
}