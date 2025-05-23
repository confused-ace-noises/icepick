use std::{sync::Arc, time::Duration};

#[cfg(target_has_atomic = "64")]
pub use rate_limiter_atomic::RateLimiter;

#[cfg(not(target_has_atomic = "64"))]
pub use no_atomic;

#[cfg(target_has_atomic = "64")]
mod rate_limiter_atomic {
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{Duration, Instant},
    };
    use tokio::time::{Instant as TokioInstant, sleep_until};

    #[derive(Debug)]
    pub struct RateLimiter {
        next_allowed_ms: AtomicU64, // val
        interval: Duration,
        start: TokioInstant,
    }

    impl RateLimiter {
        #[must_use]
        pub fn new(interval: Duration /* name: String */) -> Self {
            let now = Instant::now();
            Self {
                next_allowed_ms: AtomicU64::new(now.elapsed().as_millis() as u64),
                interval,
                // name,
                start: TokioInstant::now(),
            }
        }

        pub async fn wait(&self) {
            loop {
                let now = TokioInstant::now().duration_since(self.start).as_millis() as u64;
                let next = self.next_allowed_ms.load(Ordering::Acquire);

                if now >= next {
                    // println!("going without waiting: {}", self.name);
                    let new_next = now + self.interval.as_millis() as u64;

                    if self
                        .next_allowed_ms
                        .compare_exchange(next, new_next, Ordering::AcqRel, Ordering::Acquire) // TODO: understand CAS
                        .is_ok()
                    {
                        return;
                    }
                } else {
                    // println!("waiting...: {}", self.name);
                    let wait_duration = Duration::from_millis(next - now);
                    sleep_until(TokioInstant::now() + wait_duration).await;
                    // println!("waited!: {}", self.name);
                }
            }
        }
    }
}

#[cfg(not(target_has_atomic = "64"))]
mod no_atomic {
    compile_error!("targets without 64 bit-wide Atomics support aren't supported yet.");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
pub async fn test() {
    let ratelimiter1 = Arc::new(RateLimiter::new(Duration::from_nanos(3)));
    let ratelimiterclone = Arc::clone(&ratelimiter1);
    let ratelimiter2 = RateLimiter::new(Duration::from_secs(7));

    println!("test");

    let y = tokio::spawn(async move {
        let ratelimiter1 = ratelimiterclone.as_ref();
        for _ in 0..10 {
            ratelimiter1.wait().await;
            println!("loop 1!");
        }
    });

    let x = tokio::spawn(async move {
        let ratelimiter2 = ratelimiter1.as_ref();
        for _ in 0..10 {
            ratelimiter2.wait().await;
            println!("loop 2!");
        }
    });

    let _ = tokio::join!(x, y);
}