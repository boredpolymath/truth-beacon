use reqwest::header::HeaderMap;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Global Discord REST API rate limit: 50 requests per second
pub const GLOBAL_RATE_LIMIT_CAPACITY: u32 = 50;
pub const GLOBAL_RATE_LIMIT_REFILL_PER_SEC: f64 = 50.0;

/// Route-aware token bucket rate limiter and Discord 429 Retry-After handler (Phase 14.3).
/// Enforces compliance with Discord REST API limits (50 requests/sec global + route-specific buckets)
/// and adheres to Retry-After headers without queue dropping.
#[derive(Clone, Debug)]
pub struct DiscordRateLimiter {
    global_bucket: Arc<Mutex<TokenBucket>>,
    buckets: Arc<Mutex<HashMap<String, TokenBucket>>>,
    global_lock_until: Arc<Mutex<Option<Instant>>>,
}

#[derive(Debug)]
struct TokenBucket {
    capacity: u32,
    available_tokens: f64,
    refill_rate_per_sec: f64,
    last_refill: Instant,
    locked_until: Option<Instant>,
}

impl TokenBucket {
    fn new(capacity: u32, refill_rate_per_sec: f64) -> Self {
        Self {
            capacity,
            available_tokens: capacity as f64,
            refill_rate_per_sec,
            last_refill: Instant::now(),
            locked_until: None,
        }
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.available_tokens =
            (self.available_tokens + elapsed * self.refill_rate_per_sec).min(self.capacity as f64);
        self.last_refill = now;
    }

    fn try_acquire(&mut self, now: Instant) -> Result<(), Duration> {
        // Check if bucket is under a 429 Retry-After lock
        if let Some(lock) = self.locked_until {
            if now < lock {
                return Err(lock.duration_since(now));
            } else {
                self.locked_until = None;
            }
        }

        self.refill(now);

        if self.available_tokens >= 1.0 {
            self.available_tokens -= 1.0;
            Ok(())
        } else {
            let needed = 1.0 - self.available_tokens;
            let wait_secs = needed / self.refill_rate_per_sec;
            Err(Duration::from_secs_f64(wait_secs.max(0.001)))
        }
    }

    fn apply_retry_after(&mut self, duration: Duration) {
        let unlock_at = Instant::now() + duration;
        self.locked_until = Some(unlock_at);
        self.available_tokens = 0.0;
    }
}

impl Default for DiscordRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscordRateLimiter {
    pub fn new() -> Self {
        Self {
            global_bucket: Arc::new(Mutex::new(TokenBucket::new(
                GLOBAL_RATE_LIMIT_CAPACITY,
                GLOBAL_RATE_LIMIT_REFILL_PER_SEC,
            ))),
            buckets: Arc::new(Mutex::new(HashMap::new())),
            global_lock_until: Arc::new(Mutex::new(None)),
        }
    }

    /// Attempts to acquire execution permission for a given REST route.
    /// Checks:
    /// 1. Global 429 Retry-After lock
    /// 2. Global 50 requests/sec token bucket
    /// 3. Route-specific token bucket (default 5 tokens, refill 1.0/sec for moderation endpoints)
    ///
    /// Returns `Ok(())` if permitted, or `Err(wait_duration)` if rate limited.
    pub fn try_acquire(&self, route: &str) -> Result<(), Duration> {
        let now = Instant::now();

        // 1. Check Global 429 Retry-After lock
        {
            let global = self.global_lock_until.lock().unwrap();
            if let Some(global_lock) = *global {
                if now < global_lock {
                    return Err(global_lock.duration_since(now));
                }
            }
        }

        // 2. Check Global 50 requests/sec token bucket
        {
            let mut global_bucket = self.global_bucket.lock().unwrap();
            global_bucket.try_acquire(now)?;
        }

        // 3. Check route-specific bucket (default: 5 requests per 5 seconds for moderation routes)
        let mut buckets = self.buckets.lock().unwrap();
        let bucket = buckets
            .entry(route.to_string())
            .or_insert_with(|| TokenBucket::new(5, 1.0));

        bucket.try_acquire(now)
    }

    /// Asynchronously acquires permission for a given REST route without dropping the request.
    /// If rate-limited, suspends execution via `tokio::time::sleep` until a token becomes available.
    pub async fn acquire_or_wait(&self, route: &str) {
        loop {
            match self.try_acquire(route) {
                Ok(()) => return,
                Err(wait_dur) => {
                    tokio::time::sleep(wait_dur).await;
                }
            }
        }
    }

    /// Handles an HTTP 429 Too Many Requests response from Discord.
    /// Locks the route bucket (or entire gateway if is_global) for the specified Retry-After duration.
    pub fn handle_rate_limit_response(&self, route: &str, retry_after_secs: f64, is_global: bool) {
        let backoff = Duration::from_secs_f64(retry_after_secs.max(0.05));
        if is_global {
            let mut global = self.global_lock_until.lock().unwrap();
            *global = Some(Instant::now() + backoff);
            let mut global_bucket = self.global_bucket.lock().unwrap();
            global_bucket.apply_retry_after(backoff);
        } else {
            let mut buckets = self.buckets.lock().unwrap();
            let bucket = buckets
                .entry(route.to_string())
                .or_insert_with(|| TokenBucket::new(5, 1.0));
            bucket.apply_retry_after(backoff);
        }
    }

    /// Parses the `Retry-After` header value from string representation.
    /// Supports:
    /// - Decimal seconds (e.g. "1.5", "0.25")
    /// - Whole seconds (e.g. "2")
    /// - Milliseconds (e.g. "2500" -> 2.5s if > 100)
    pub fn parse_retry_after_str(val: &str) -> Option<f64> {
        let trimmed = val.trim();
        if let Ok(num) = trimmed.parse::<f64>() {
            if num > 100.0 {
                // Heuristic: values > 100 are likely represented in milliseconds
                Some(num / 1000.0)
            } else {
                Some(num)
            }
        } else {
            None
        }
    }

    /// Automatically parses rate limit metadata from HTTP headers and/or optional JSON body.
    /// Checks `Retry-After`, `X-RateLimit-Reset-After`, and `X-RateLimit-Global`.
    pub fn parse_retry_after(
        headers: &HeaderMap,
        body_json: Option<&Value>,
    ) -> Option<(f64, bool)> {
        let is_global = headers
            .get("x-ratelimit-global")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.eq_ignore_ascii_case("true") || s == "1")
            .unwrap_or(false);

        // 1. Try 'retry-after' header
        if let Some(val) = headers.get("retry-after").and_then(|v| v.to_str().ok()) {
            if let Some(secs) = Self::parse_retry_after_str(val) {
                return Some((secs, is_global));
            }
        }

        // 2. Try 'x-ratelimit-reset-after' header
        if let Some(val) = headers
            .get("x-ratelimit-reset-after")
            .and_then(|v| v.to_str().ok())
        {
            if let Some(secs) = Self::parse_retry_after_str(val) {
                return Some((secs, is_global));
            }
        }

        // 3. Fallback to Discord JSON body payload e.g. {"message": "...", "retry_after": 1.5, "global": false}
        if let Some(json) = body_json {
            if let Some(retry_after) = json.get("retry_after").and_then(|v| v.as_f64()) {
                let global = json
                    .get("global")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(is_global);
                return Some((retry_after, global));
            }
        }

        None
    }

    /// Automatically ingests an HTTP response status and headers, updating the rate limiter
    /// if a 429 Too Many Requests response is detected.
    pub fn ingest_http_response(
        &self,
        route: &str,
        status_code: u16,
        headers: &HeaderMap,
        body_json: Option<&Value>,
    ) -> bool {
        if status_code == 429 {
            let (retry_after_secs, is_global) =
                Self::parse_retry_after(headers, body_json).unwrap_or((1.0, false));
            self.handle_rate_limit_response(route, retry_after_secs, is_global);
            true
        } else {
            false
        }
    }

    /// Outbound Dispatcher execution helper: executes an asynchronous network operation
    /// with rate-limit adherence, retrying on 429 without dropping the request from the queue.
    /// Executes an outbound operation after waiting for rate limiter token availability.
    pub async fn execute_with_rate_limit<F, Fut, T, E>(&self, route: &str, op: F) -> Result<T, E>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        self.acquire_or_wait(route).await;
        op().await
    }

    /// Executes an outbound request against a route, automatically retrying if the operation
    /// signals a rate limit (e.g. 429) with Retry-After duration, without dropping the request.
    pub async fn execute_with_retry_after<F, Fut, T, E>(
        &self,
        route: &str,
        mut op: F,
    ) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, (E, Option<(Duration, bool)>)>>,
    {
        loop {
            self.acquire_or_wait(route).await;
            match op().await {
                Ok(val) => return Ok(val),
                Err((_err, Some((retry_after, is_global)))) => {
                    self.handle_rate_limit_response(route, retry_after.as_secs_f64(), is_global);
                    tokio::time::sleep(retry_after).await;
                }
                Err((err, None)) => return Err(err),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    #[test]
    fn test_token_bucket_acquisition_and_depletion() {
        let limiter = DiscordRateLimiter::new();
        let route = "/guilds/123/members/456";

        // Initial 5 requests should succeed immediately
        for _ in 0..5 {
            assert!(limiter.try_acquire(route).is_ok());
        }

        // 6th request must be rate limited
        let res = limiter.try_acquire(route);
        assert!(res.is_err());
        assert!(res.unwrap_err().as_millis() > 0);
    }

    #[test]
    fn test_global_50_requests_per_sec_limit() {
        let limiter = DiscordRateLimiter::new();

        // 50 requests across 50 distinct routes (so route-level limit of 5 is never hit)
        for i in 0..50 {
            let route = format!("/guilds/100/route_{}", i);
            assert!(
                limiter.try_acquire(&route).is_ok(),
                "Request {} within global 50 req/s budget should succeed",
                i
            );
        }

        // 51st request across any route must fail due to the global 50 req/s limit
        let res = limiter.try_acquire("/guilds/100/route_51");
        assert!(
            res.is_err(),
            "51st request must be rate limited by global 50 req/s bucket"
        );
    }

    #[test]
    fn test_retry_after_adherence() {
        let limiter = DiscordRateLimiter::new();
        let route = "/guilds/123/bans";

        // Simulate receiving a 429 with Retry-After: 2.0 seconds
        limiter.handle_rate_limit_response(route, 2.0, false);

        // Immediate subsequent request must fail and report >= 1.5s wait
        let res = limiter.try_acquire(route);
        assert!(res.is_err());
        assert!(res.unwrap_err().as_secs_f64() >= 1.5);
    }

    #[test]
    fn test_global_rate_limit_adherence() {
        let limiter = DiscordRateLimiter::new();
        limiter.handle_rate_limit_response("any_route", 1.5, true);

        // All routes must be blocked when global lock is active
        assert!(limiter.try_acquire("/route_a").is_err());
        assert!(limiter.try_acquire("/route_b").is_err());
    }

    #[test]
    fn test_parse_retry_after_header_formats() {
        assert_eq!(DiscordRateLimiter::parse_retry_after_str("1.5"), Some(1.5));
        assert_eq!(DiscordRateLimiter::parse_retry_after_str(" 2 "), Some(2.0));
        assert_eq!(
            DiscordRateLimiter::parse_retry_after_str("0.05"),
            Some(0.05)
        );
        assert_eq!(DiscordRateLimiter::parse_retry_after_str("2500"), Some(2.5)); // ms conversion
        assert_eq!(DiscordRateLimiter::parse_retry_after_str("invalid"), None);

        // HeaderMap parsing: standard 'retry-after'
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", HeaderValue::from_static("3.5"));
        let parsed = DiscordRateLimiter::parse_retry_after(&headers, None);
        assert_eq!(parsed, Some((3.5, false)));

        // HeaderMap parsing: 'x-ratelimit-reset-after' with global flag
        let mut headers_global = HeaderMap::new();
        headers_global.insert("x-ratelimit-reset-after", HeaderValue::from_static("1.2"));
        headers_global.insert("x-ratelimit-global", HeaderValue::from_static("true"));
        let parsed_global = DiscordRateLimiter::parse_retry_after(&headers_global, None);
        assert_eq!(parsed_global, Some((1.2, true)));

        // JSON body fallback
        let empty_headers = HeaderMap::new();
        let body = serde_json::json!({
            "message": "You are being rate limited.",
            "retry_after": 4.25,
            "global": true
        });
        let parsed_json = DiscordRateLimiter::parse_retry_after(&empty_headers, Some(&body));
        assert_eq!(parsed_json, Some((4.25, true)));
    }

    #[tokio::test]
    async fn test_outbound_queue_no_drop_adherence() {
        let limiter = DiscordRateLimiter::new();
        let route = "/guilds/999/moderation";

        // Exhaust route bucket
        for _ in 0..5 {
            assert!(limiter.try_acquire(route).is_ok());
        }

        // Apply a short 50ms lock to simulate transient 429
        limiter.handle_rate_limit_response(route, 0.05, false);

        // Using acquire_or_wait: must asynchronously wait and succeed without dropping
        let start = Instant::now();
        limiter.acquire_or_wait(route).await;
        let elapsed = start.elapsed();

        assert!(
            elapsed >= Duration::from_millis(40),
            "Expected acquire_or_wait to adhere to retry duration, took {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_execute_with_retry_after() {
        let limiter = DiscordRateLimiter::new();
        let route = "/channels/123/messages";
        let mut attempts = 0;

        let result = limiter
            .execute_with_retry_after(route, || {
                attempts += 1;
                async move {
                    if attempts == 1 {
                        // First attempt simulates 429 Retry-After of 50ms
                        Err(("Rate limited", Some((Duration::from_millis(50), false))))
                    } else {
                        Ok("Success")
                    }
                }
            })
            .await;

        assert_eq!(result, Ok("Success"));
        assert_eq!(attempts, 2);
    }
}
