//! Request rate limit: a token bucket shared by all authenticated clients. There is one API
//! token, so "per client" would be the same thing; the limit only stops a runaway client loop
//! from eating the server. Requests without a valid token never reach it, so strangers cannot
//! use up the budget of the real user.

use std::sync::Mutex;
use std::time::Instant;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::auth::PUBLIC_PATH;
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug)]
pub struct RateLimiter {
    /// `None`: no limit.
    bucket: Option<Mutex<Bucket>>,
}

#[derive(Debug)]
struct Bucket {
    capacity: f64,
    refill_per_sec: f64,
    tokens: f64,
    updated: Instant,
}

impl RateLimiter {
    /// `per_minute == 0` turns the limit off. The burst is ten seconds' worth of requests
    /// (at least 5), so a screen that fires a few requests at once is never refused.
    pub fn new(per_minute: u32) -> RateLimiter {
        RateLimiter::new_at(per_minute, Instant::now())
    }

    fn new_at(per_minute: u32, now: Instant) -> RateLimiter {
        if per_minute == 0 {
            return RateLimiter { bucket: None };
        }
        let capacity = f64::from((per_minute / 6).max(5));
        RateLimiter {
            bucket: Some(Mutex::new(Bucket {
                capacity,
                refill_per_sec: f64::from(per_minute) / 60.0,
                tokens: capacity,
                updated: now,
            })),
        }
    }

    /// `Ok` if the request may go on, otherwise how many seconds to wait (at least 1).
    pub fn check(&self) -> Result<(), u64> {
        self.check_at(Instant::now())
    }

    fn check_at(&self, now: Instant) -> Result<(), u64> {
        let Some(bucket) = &self.bucket else {
            return Ok(());
        };
        let mut b = bucket.lock().unwrap();
        let elapsed = now.saturating_duration_since(b.updated).as_secs_f64();
        b.tokens = (b.tokens + elapsed * b.refill_per_sec).min(b.capacity);
        b.updated = now;
        if b.tokens >= 1.0 {
            b.tokens -= 1.0;
            Ok(())
        } else {
            let wait = (1.0 - b.tokens) / b.refill_per_sec;
            Err(wait.ceil().max(1.0) as u64)
        }
    }
}

/// Middleware: refuses with `429` when the bucket is empty. Must sit behind the token check.
pub async fn rate_limit(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if request.uri().path() != PUBLIC_PATH {
        if let Err(retry_after) = state.limiter.check() {
            return ApiError::TooManyRequests {
                message: "too many requests, slow down".into(),
                retry_after,
            }
            .into_response();
        }
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn zero_means_unlimited() {
        let limiter = RateLimiter::new(0);
        assert!((0..10_000).all(|_| limiter.check().is_ok()));
    }

    #[test]
    fn allows_a_burst_then_refuses_then_refills() {
        let start = Instant::now();
        let limiter = RateLimiter::new_at(60, start); // 1 per second, burst 10
        for i in 0..10 {
            assert!(limiter.check_at(start).is_ok(), "request {i}");
        }
        assert_eq!(limiter.check_at(start), Err(1));

        // one second later there is exactly one more request
        let later = start + Duration::from_secs(1);
        assert!(limiter.check_at(later).is_ok());
        assert!(limiter.check_at(later).is_err());
    }

    #[test]
    fn the_wait_matches_the_rate() {
        let start = Instant::now();
        let limiter = RateLimiter::new_at(6, start); // one per 10 s, burst 5
        for _ in 0..5 {
            limiter.check_at(start).unwrap();
        }
        assert_eq!(limiter.check_at(start), Err(10));
        assert_eq!(limiter.check_at(start + Duration::from_secs(4)), Err(6));
    }

    #[test]
    fn the_bucket_never_holds_more_than_the_burst() {
        let start = Instant::now();
        let limiter = RateLimiter::new_at(60, start);
        let much_later = start + Duration::from_secs(3600);
        for _ in 0..10 {
            assert!(limiter.check_at(much_later).is_ok());
        }
        assert!(limiter.check_at(much_later).is_err());
    }
}
