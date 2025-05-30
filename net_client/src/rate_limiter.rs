use tracing;

#[cfg(target_has_atomic = "64")]
pub use rate_limiter_atomic::RateLimiter;

#[cfg(not(target_has_atomic = "64"))]
pub use no_atomic;

#[cfg(target_has_atomic = "64")]
mod rate_limiter_atomic {
    use std::{
        sync::{atomic::{AtomicU64, Ordering}, Arc},
        time::{Duration, Instant},
    };
    use tokio::time::{Instant as TokioInstant, sleep_until};
    use tracing::{debug, instrument, trace};

    #[derive(Debug)]
    pub struct RateLimiter {
        next_allowed_ms: AtomicU64, // val
        interval: Duration,
        start: TokioInstant,
    }

    impl RateLimiter {
        #[must_use]
        pub fn new(interval: Duration) -> Self {
            let now = Instant::now();
            Self {
                next_allowed_ms: AtomicU64::new(now.elapsed().as_millis() as u64),
                interval,
                // name,
                start: TokioInstant::now(),
            }
        }

        pub async fn wait(&self, name: &str) {
            loop {
                let now = TokioInstant::now().duration_since(self.start).as_millis() as u64;
                let next = self.next_allowed_ms.load(Ordering::Acquire);

                if now >= next {
                    let new_next = now + self.interval.as_millis() as u64;

                    if self
                        .next_allowed_ms
                        .compare_exchange(next, new_next, Ordering::AcqRel, Ordering::Acquire) // TODO: understand CAS
                        .is_ok()
                    {
                        debug!(rate_limiter_name = name, "going without waiting, already clear");
                        return;
                    } else {
                        trace!(soft_error=true, rate_limiter_name = name, "CAS failed, retrying");
                    }
                } else {
                    let wait_duration = Duration::from_millis(next - now);

                    debug!(
                        rate_limiter_name = name,
                        wait_ms = wait_duration.as_millis(),
                        "waiting for: {} millisecs", wait_duration.as_millis()
                    );

                    sleep_until(TokioInstant::now() + wait_duration).await;
                    debug!(rate_limiter_name = name, "done waiting");
                }
            }
        }
    }
}

#[cfg(not(target_has_atomic = "64"))]
mod no_atomic {
    compile_error!("targets without 64 bit-wide Atomics support aren't supported yet.");
}



#[cfg(test)]
mod tests {
    use std::{sync::Arc, thread::sleep, time::Duration};
    use tokio::time;

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 6)]
    pub async fn test() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("trace")
            .with_test_writer()
            .with_file(true)
            .try_init().unwrap();

        let ratelimiter1 = Arc::new(RateLimiter::new(Duration::from_secs(3)));
        let ratelimiterclone = Arc::clone(&ratelimiter1);
        
        let y = tokio::spawn(async move {
            let ratelimiter1 = ratelimiterclone.as_ref();
            for _ in 0..10 {
                ratelimiter1.wait("test1.com").await;
            }
        });
    
        let x = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let ratelimiter2 = ratelimiter1.as_ref();
            for _ in 0..10 {
                ratelimiter2.wait("test2.com").await;
            }
        });
    
        let _ = tokio::join!(x, y);
    }
}