use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed, // Normal operation: requests allowed
    Open,   // Tripped: automated requests halted, requires manual reset or cooldown
}

#[derive(Clone)]
pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitBreakerInner>>,
}

struct CircuitBreakerInner {
    state: CircuitState,
    history: Vec<Instant>,
    window_duration: Duration,
    max_requests: usize,
    cooldown_duration: Duration,
    tripped_at: Option<Instant>,
}

impl CircuitBreaker {
    pub fn new(max_requests: usize, window_secs: u64, cooldown_secs: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitBreakerInner {
                state: CircuitState::Closed,
                history: Vec::new(),
                window_duration: Duration::from_secs(window_secs),
                max_requests,
                cooldown_duration: Duration::from_secs(cooldown_secs),
                tripped_at: None,
            })),
        }
    }

    /// Checks if an outbound mitigation request is permissible.
    /// Returns Ok(()) if allowed, or Err(remaining_cooldown_secs) if tripped.
    pub fn check_and_record(&self) -> Result<(), u64> {
        let mut inner = self.state.lock().unwrap();
        let now = Instant::now();

        // Check if currently open
        if inner.state == CircuitState::Open {
            if let Some(tripped_time) = inner.tripped_at {
                if now.duration_since(tripped_time) >= inner.cooldown_duration {
                    // Cooldown has elapsed; reset to closed
                    inner.state = CircuitState::Closed;
                    inner.tripped_at = None;
                    inner.history.clear();
                } else {
                    let elapsed = now.duration_since(tripped_time).as_secs();
                    let total_cooldown = inner.cooldown_duration.as_secs();
                    let remaining = total_cooldown.saturating_sub(elapsed);
                    return Err(remaining);
                }
            }
        }

        // Clean up old events outside sliding window
        let cutoff = now.checked_sub(inner.window_duration).unwrap_or(now);
        inner.history.retain(|&t| t >= cutoff);

        if inner.history.len() >= inner.max_requests {
            inner.state = CircuitState::Open;
            inner.tripped_at = Some(now);
            return Err(inner.cooldown_duration.as_secs());
        }

        inner.history.push(now);
        Ok(())
    }

    /// Manually reset the circuit breaker back to normal Closed state
    pub fn reset(&self) {
        let mut inner = self.state.lock().unwrap();
        inner.state = CircuitState::Closed;
        inner.tripped_at = None;
        inner.history.clear();
    }

    /// Query current status
    pub fn is_tripped(&self) -> bool {
        let inner = self.state.lock().unwrap();
        inner.state == CircuitState::Open
    }
}
