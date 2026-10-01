use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Route-aware token bucket rate limiter and Discord 429 Retry-After handler.
/// Enforces compliance with Discord API rate limits and prevents request flooding.
#[derive(Clone, Debug)]
pub struct DiscordRateLimiter {
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
            Err(Duration::from_secs_f64(wait_secs))
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
            buckets: Arc::new(Mutex::new(HashMap::new())),
            global_lock_until: Arc::new(Mutex::new(None)),
        }
    }

    /// Attempts to acquire execution permission for a given REST route.
    /// Returns Ok(()) if permitted, or Err(wait_duration) if rate limited.
    pub fn try_acquire(&self, route: &str) -> Result<(), Duration> {
        let now = Instant::now();

        // 1. Check Global Rate Limit lock
        {
            let global = self.global_lock_until.lock().unwrap();
            if let Some(global_lock) = *global {
                if now < global_lock {
                    return Err(global_lock.duration_since(now));
                }
            }
        }

        // 2. Check route-specific bucket (default: 5 requests per 5 seconds for moderation routes)
        let mut buckets = self.buckets.lock().unwrap();
        let bucket = buckets
            .entry(route.to_string())
            .or_insert_with(|| TokenBucket::new(5, 1.0));

        bucket.try_acquire(now)
    }

    /// Handles an HTTP 429 Too Many Requests response from Discord.
    /// Locks the route bucket (or entire gateway if is_global) for the specified Retry-After duration.
    pub fn handle_rate_limit_response(&self, route: &str, retry_after_secs: f64, is_global: bool) {
        let backoff = Duration::from_secs_f64(retry_after_secs.max(0.1));
        if is_global {
            let mut global = self.global_lock_until.lock().unwrap();
            *global = Some(Instant::now() + backoff);
        } else {
            let mut buckets = self.buckets.lock().unwrap();
            let bucket = buckets
                .entry(route.to_string())
                .or_insert_with(|| TokenBucket::new(5, 1.0));
            bucket.apply_retry_after(backoff);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
