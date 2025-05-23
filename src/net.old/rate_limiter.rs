use std::{sync::Arc, time::Duration};

// pub struct RateTracker (DashMap<IpAddr, Arc<RwLock<RateLimiter>>>);
// type Uhh = RateLimiter<NotKeyed, InMemoryState, QuantaClock, NoOpMiddleware<QuantaInstant>>;

// pub async fn test() {
//     let rate_limits: Arc<DashMap<String, Arc<RwLock<_>>>> = Arc::new(DashMap::new());

//     rate_limits.insert(String::from("x"), Arc::new(RwLock::new(RateLimiter::direct(Quota::with_period(Duration::from_millis(50)).unwrap()))));
//     // Simulate incoming IPs and requests (for demonstration)
//     let ip_addresses = vec![
//         "192.168.1.1".to_string(),
//         "192.168.1.2".to_string(),
//         "192.168.1.1".to_string(),
//     ];

//     // Spawn cleanup task (to remove expired IPs, if needed)
//     let cleanup_task = tokio::spawn({
//         let rate_limits = rate_limits.clone();
//         async move {
//             loop {
//                 sleep(Duration::from_secs(60)).await;
//                 println!("Cleanup task: pruning expired IPs...");
//                 // Here, you would add custom pruning logic, like expiration time or manual removal
//             }
//         }
//     });

//     // Process the IPs (simulate requests from these IPs)
//     for ip in ip_addresses {
//         let rate_limits = rate_limits.clone();
//         tokio::spawn(async move {
//             handle_request(ip, rate_limits).await;
//         });
//     }

//     // Wait for cleanup task to run (this will run indefinitely)
//     cleanup_task.await.unwrap();
// }

// async fn handle_request(ip: String, rate_limits: Arc<DashMap<String, Arc<RwLock<RateLimiter<NotKeyed, InMemoryState, QuantaClock, NoOpMiddleware<QuantaInstant>>>>>>) {
//     // Lookup or create a new RateLimiter for this IP
//     let limiter = rate_limits.entry(ip.clone()).or_insert_with(|| {
//         let quota = Quota::per_second(NonZeroU32::new(5).unwrap()); // 5 requests per second
//         Arc::new(RwLock::new(RateLimiter::direct(quota)))
//     });

//     // Lock the RateLimiter for this IP
//     let limiter = limiter.clone();
//     let mut limiter = limiter.write().await;

//     // Try to rate-limit the request
//     if limiter.check_key(&()).is_ok() {
//         println!("Request from {} allowed.", ip);
//         // Simulate request handling (e.g., web scraping task)
//         tokio::time::sleep(Duration::from_millis(100)).await;
//     } else {
//         println!("Request from {} denied. Rate limit exceeded.", ip);
//     }
// }

// * ----------

// pub struct RateLimiter {
//     duration: Duration,
//     sleep: Option<Pin<Box<Sleep>>>,
// }

// impl RateLimiter {
//     pub fn new(duration: Duration) -> Self {
//         Self {
//             duration,
//             sleep: None,
//         }
//     }

//     pub fn wait(mut self) -> RateLimiterFuture {
//         let sleep = Box::pin(time::sleep(self.duration));
//         self.sleep = Some(sleep);
//         RateLimiterFuture { limiter: self }
//     }
// }

// pub struct RateLimiterFuture {
//     limiter: RateLimiter,
// }

// impl Future for RateLimiterFuture {
//     type Output = RateLimiter;

//     fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//         let this = &mut *self;

//         if let Some(ref mut sleep) = this.limiter.sleep {
//             match sleep.as_mut().poll(cx) {
//                 Poll::Ready(_) => {
//                     this.limiter.sleep = None; // reset timer
//                     let duration = this.limiter.duration;
//                     Poll::Ready(std::mem::replace(&mut this.limiter, RateLimiter::new(duration)))
//                 }
//                 Poll::Pending => Poll::Pending,
//             }
//         } else {
//             Poll::Pending
//         }
//     }
// }

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
        // name: String,
        start: TokioInstant,
    }

    impl RateLimiter {
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
                    } else {
                        continue;
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