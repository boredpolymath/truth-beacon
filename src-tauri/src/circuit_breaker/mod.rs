//! Operational Circuit Breaker & Safety Brake (Phase 15.1)
//!
//! Provides a thread-safe in-memory state machine protecting Discord moderation actions
//! from runaway loops or automated cascades:
//! - States: `Closed` (Arm Ready), `Open` (Tripped), `HalfOpen` (Cooling Probe).
//! - Rolling sliding-window timestamp deque (default: 60-second window).
//! - Configurable threshold: Default maximum 5 automated/accelerated mitigation actions per 60s.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

static AUDIT_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Logs a circuit breaker event (`circuit_breaker_tripped` or `circuit_breaker_reset`) to SQLite `audit_logs` table (Phase 15.3).
pub fn log_circuit_breaker_audit(
    conn: &Connection,
    action: crate::models::audit::ActionType,
    operator_id: &str,
    reason: &str,
    metadata: Option<serde_json::Value>,
) -> Result<String, rusqlite::Error> {
    let now = chrono::Utc::now().timestamp();
    let counter = AUDIT_COUNTER.fetch_add(1, Ordering::SeqCst);
    let audit_id = format!("audit_cb_{}_{}", now, counter);
    let action_str = match action {
        crate::models::audit::ActionType::CircuitBreakerTripped => "circuit_breaker_tripped",
        crate::models::audit::ActionType::CircuitBreakerReset => "circuit_breaker_reset",
        _ => "circuit_breaker_event",
    };
    let meta_json = metadata.map(|m| m.to_string());

    conn.execute(
        "INSERT INTO audit_logs (
            id, timestamp, action, guild_id, operator_id, target_user_id,
            incident_id, reason, metadata_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);",
        params![
            audit_id,
            now,
            action_str,
            "global",
            operator_id,
            None::<String>,
            None::<String>,
            reason,
            meta_json,
        ],
    )?;

    log::info!(
        "Recorded circuit breaker audit log (id: {}, action: '{}', operator: '{}', reason: '{}')",
        audit_id,
        action_str,
        operator_id,
        reason
    );
    Ok(audit_id)
}

/// Default configuration constants for Discord moderation safety brake
pub const DEFAULT_MAX_ACTIONS_PER_WINDOW: usize = 5;
pub const DEFAULT_WINDOW_DURATION_SECS: u64 = 60;
pub const DEFAULT_COOLDOWN_DURATION_SECS: u64 = 300; // 5 minutes

/// Thread-safe in-memory Circuit Breaker State Machine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CircuitState {
    /// Normal operation: Automated and accelerated mitigations allowed (Arm Ready).
    Closed,
    /// Tripped: Safety brake active. Automated mitigations halted, operator confirmation required.
    Open,
    /// Cooling Probe: Cooldown period has elapsed. A single trial action is evaluated before closing.
    HalfOpen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MitigationOrigin {
    /// Automated detection loop triggering immediate action
    Automated,
    /// Bulk or multi-target action across a member list
    Batch,
    /// Manual operator click from triage console
    Manual { operator_confirmed: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum CircuitBreakerError {
    #[error(
        "Circuit breaker is tripped (state: Open). Cooling down for {remaining_cooldown_secs}s"
    )]
    Tripped { remaining_cooldown_secs: u64 },

    #[error("Action rejected during Half-Open evaluation probe")]
    ProbeRejected,

    #[error("Automated mitigation calls locked by circuit breaker (state: Open)")]
    AutomatedActionsLocked,

    #[error("Batch moderation calls locked by circuit breaker (state: Open)")]
    BatchActionsLocked,

    #[error("Circuit breaker is tripped to Open: explicit operator manual confirmation required")]
    ConfirmationRequired,
}

/// Configuration settings for CircuitBreaker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub max_actions: usize,
    pub window_duration: Duration,
    pub cooldown_duration: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            max_actions: DEFAULT_MAX_ACTIONS_PER_WINDOW,
            window_duration: Duration::from_secs(DEFAULT_WINDOW_DURATION_SECS),
            cooldown_duration: Duration::from_secs(DEFAULT_COOLDOWN_DURATION_SECS),
        }
    }
}

/// Serializable telemetry and status report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerStatus {
    pub state: CircuitState,
    pub is_tripped: bool,
    pub recent_action_count: usize,
    pub max_actions: usize,
    pub window_secs: u64,
    pub cooldown_secs: u64,
    pub remaining_cooldown_secs: u64,
}

/// Thread-safe Operational Circuit Breaker
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitBreakerInner>>,
}

static GLOBAL_CIRCUIT_BREAKER: std::sync::OnceLock<CircuitBreaker> = std::sync::OnceLock::new();

/// Returns the global shared instance of the Operational Circuit Breaker
pub fn get_global_circuit_breaker() -> &'static CircuitBreaker {
    GLOBAL_CIRCUIT_BREAKER.get_or_init(CircuitBreaker::default)
}

#[derive(Debug)]
struct CircuitBreakerInner {
    state: CircuitState,
    history: VecDeque<Instant>,
    config: CircuitBreakerConfig,
    tripped_at: Option<Instant>,
    half_open_probe_in_flight: bool,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(
            DEFAULT_MAX_ACTIONS_PER_WINDOW,
            DEFAULT_WINDOW_DURATION_SECS,
            DEFAULT_COOLDOWN_DURATION_SECS,
        )
    }
}

impl CircuitBreaker {
    /// Creates a new circuit breaker with specified limits
    pub fn new(max_actions: usize, window_secs: u64, cooldown_secs: u64) -> Self {
        Self::with_config(CircuitBreakerConfig {
            max_actions,
            window_duration: Duration::from_secs(window_secs),
            cooldown_duration: Duration::from_secs(cooldown_secs),
        })
    }

    /// Creates a circuit breaker with a custom configuration struct
    pub fn with_config(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitBreakerInner {
                state: CircuitState::Closed,
                history: VecDeque::with_capacity(config.max_actions + 2),
                config,
                tripped_at: None,
                half_open_probe_in_flight: false,
            })),
        }
    }

    /// Legacy / default check forwarding to automated mitigation origin
    pub fn check_and_record(&self) -> Result<(), CircuitBreakerError> {
        self.try_acquire_mitigation(MitigationOrigin::Automated)
    }

    /// Deterministic test helper for check_and_record
    #[cfg(test)]
    pub(crate) fn check_and_record_at(&self, now: Instant) -> Result<(), CircuitBreakerError> {
        self.try_acquire_mitigation_at(MitigationOrigin::Automated, now)
    }

    /// Evaluates whether a mitigation action can proceed according to origin and circuit state (Phase 15.2):
    /// - `Closed`: checks sliding window. If threshold exceeded, trips to `Open` and rejects.
    /// - `Open`: locks all automated and batch moderation calls. Allows manual actions only with explicit operator confirmation.
    /// - `HalfOpen`: evaluates a single trial probe to gauge stability before returning to Closed.
    pub fn try_acquire_mitigation(
        &self,
        origin: MitigationOrigin,
    ) -> Result<(), CircuitBreakerError> {
        self.try_acquire_mitigation_at(origin, Instant::now())
    }

    /// Internal deterministic helper for testing mitigation acquisition with explicit timestamp
    pub(crate) fn try_acquire_mitigation_at(
        &self,
        origin: MitigationOrigin,
        now: Instant,
    ) -> Result<(), CircuitBreakerError> {
        let mut inner = self.state.lock().unwrap();

        // 1. Evaluate Open state
        if inner.state == CircuitState::Open {
            if let Some(tripped_time) = inner.tripped_at {
                if now.duration_since(tripped_time) >= inner.config.cooldown_duration {
                    log::info!("Circuit breaker cooldown elapsed: transitioning Open -> HalfOpen");
                    inner.state = CircuitState::HalfOpen;
                    inner.tripped_at = None;
                    inner.history.clear();
                    inner.half_open_probe_in_flight = false;
                } else {
                    // Under active Open cooldown: enforce mitigation locks
                    match origin {
                        MitigationOrigin::Automated => {
                            log::warn!(
                                "Automated mitigation call blocked: circuit breaker is Open"
                            );
                            return Err(CircuitBreakerError::AutomatedActionsLocked);
                        }
                        MitigationOrigin::Batch => {
                            log::warn!("Batch moderation call blocked: circuit breaker is Open");
                            return Err(CircuitBreakerError::BatchActionsLocked);
                        }
                        MitigationOrigin::Manual {
                            operator_confirmed: false,
                        } => {
                            log::warn!(
                                "Manual action blocked: requires explicit operator confirmation"
                            );
                            return Err(CircuitBreakerError::ConfirmationRequired);
                        }
                        MitigationOrigin::Manual {
                            operator_confirmed: true,
                        } => {
                            log::info!("Manual mitigation permitted with explicit operator confirmation while Open");
                            return Ok(());
                        }
                    }
                }
            } else {
                inner.state = CircuitState::Closed;
            }
        }

        // 2. Evaluate Half-Open state (cooling probe)
        if inner.state == CircuitState::HalfOpen {
            if inner.half_open_probe_in_flight {
                return Err(CircuitBreakerError::ProbeRejected);
            }
            if matches!(origin, MitigationOrigin::Batch) {
                // Batch calls are never permitted as a trial probe
                return Err(CircuitBreakerError::BatchActionsLocked);
            }
            inner.half_open_probe_in_flight = true;
            inner.history.push_back(now);
            return Ok(());
        }

        // 3. Evaluate Closed state (Arm Ready)
        let window_duration = inner.config.window_duration;
        let cutoff = now.checked_sub(window_duration).unwrap_or(now);

        // Evict expired entries
        while let Some(&front) = inner.history.front() {
            if front < cutoff {
                inner.history.pop_front();
            } else {
                break;
            }
        }

        // Check if threshold exceeded
        if inner.history.len() >= inner.config.max_actions {
            log::warn!(
                "Circuit breaker tripped to Open! Exceeded {} mitigation actions in {:?}",
                inner.config.max_actions,
                inner.config.window_duration
            );
            inner.state = CircuitState::Open;
            inner.tripped_at = Some(now);
            return Err(CircuitBreakerError::Tripped {
                remaining_cooldown_secs: inner.config.cooldown_duration.as_secs(),
            });
        }

        inner.history.push_back(now);
        Ok(())
    }

    /// Records the successful completion of a probe action in Half-Open state,
    /// recovering the circuit back to Closed.
    pub fn record_probe_success(&self) {
        let mut inner = self.state.lock().unwrap();
        if inner.state == CircuitState::HalfOpen {
            log::info!(
                "Half-Open probe succeeded: returning Circuit Breaker to Closed (Arm Ready)"
            );
            inner.state = CircuitState::Closed;
            inner.half_open_probe_in_flight = false;
            inner.history.clear();
            inner.tripped_at = None;
        }
    }

    /// Records probe failure in Half-Open state, immediately tripping back to Open.
    pub fn record_probe_failure(&self) {
        let mut inner = self.state.lock().unwrap();
        if inner.state == CircuitState::HalfOpen {
            log::warn!("Half-Open probe failed: tripping Circuit Breaker back to Open");
            inner.state = CircuitState::Open;
            inner.tripped_at = Some(Instant::now());
            inner.half_open_probe_in_flight = false;
        }
    }

    /// Manually reset the circuit breaker back to normal Closed state (Arm Ready)
    pub fn reset(&self) {
        let mut inner = self.state.lock().unwrap();
        log::info!("Operator manually reset Circuit Breaker -> Closed (Arm Ready)");
        inner.state = CircuitState::Closed;
        inner.tripped_at = None;
        inner.half_open_probe_in_flight = false;
        inner.history.clear();
    }

    /// Manually reset the circuit breaker back to normal Closed state (Arm Ready),
    /// recording an immutable audit entry into SQLite `audit_logs` if connection is provided (Phase 15.3).
    pub fn reset_with_audit(
        &self,
        conn: Option<&Connection>,
        operator_id: &str,
        reason: &str,
    ) -> Result<Option<String>, rusqlite::Error> {
        self.reset();
        if let Some(c) = conn {
            let audit_id = log_circuit_breaker_audit(
                c,
                crate::models::audit::ActionType::CircuitBreakerReset,
                operator_id,
                reason,
                None,
            )?;
            Ok(Some(audit_id))
        } else {
            Ok(None)
        }
    }

    /// Manually trip the circuit breaker into Open state
    pub fn trip(&self) {
        let mut inner = self.state.lock().unwrap();
        log::warn!("Operator manually triggered Circuit Breaker trip -> Open");
        inner.state = CircuitState::Open;
        inner.tripped_at = Some(Instant::now());
        inner.half_open_probe_in_flight = false;
    }

    /// Manually trip the circuit breaker into Open state,
    /// recording an immutable audit entry into SQLite `audit_logs` if connection is provided (Phase 15.3).
    pub fn trip_with_audit(
        &self,
        conn: Option<&Connection>,
        operator_id: &str,
        reason: &str,
    ) -> Result<Option<String>, rusqlite::Error> {
        self.trip();
        if let Some(c) = conn {
            let audit_id = log_circuit_breaker_audit(
                c,
                crate::models::audit::ActionType::CircuitBreakerTripped,
                operator_id,
                reason,
                None,
            )?;
            Ok(Some(audit_id))
        } else {
            Ok(None)
        }
    }

    /// Returns cooldown progress `(remaining_cooldown_secs, total_cooldown_secs)`
    /// providing the live second-by-second countdown for UI and tray displays (Phase 15.3).
    pub fn cooldown_progress(&self) -> (u64, u64) {
        let inner = self.state.lock().unwrap();
        let total = inner.config.cooldown_duration.as_secs();
        if inner.state != CircuitState::Open {
            return (0, total);
        }
        if let Some(tripped_at) = inner.tripped_at {
            let elapsed = Instant::now().duration_since(tripped_at);
            let remaining = inner
                .config
                .cooldown_duration
                .saturating_sub(elapsed)
                .as_secs();
            (remaining, total)
        } else {
            (0, total)
        }
    }

    /// Returns the current state of the circuit breaker
    pub fn state(&self) -> CircuitState {
        let inner = self.state.lock().unwrap();
        inner.state
    }

    /// Query whether the circuit is currently tripped
    pub fn is_tripped(&self) -> bool {
        self.state() == CircuitState::Open
    }

    /// Query whether the circuit is armed and ready
    pub fn is_arm_ready(&self) -> bool {
        self.state() == CircuitState::Closed
    }

    /// Query whether the circuit is currently in half-open cooling probe
    pub fn is_half_open(&self) -> bool {
        self.state() == CircuitState::HalfOpen
    }

    /// Number of actions registered within the active sliding window
    pub fn recent_action_count(&self) -> usize {
        let mut inner = self.state.lock().unwrap();
        let now = Instant::now();
        let cutoff = now.checked_sub(inner.config.window_duration).unwrap_or(now);
        while let Some(&front) = inner.history.front() {
            if front < cutoff {
                inner.history.pop_front();
            } else {
                break;
            }
        }
        inner.history.len()
    }

    /// Returns the remaining cooldown duration, if tripped
    pub fn remaining_cooldown_secs(&self) -> u64 {
        let inner = self.state.lock().unwrap();
        if inner.state != CircuitState::Open {
            return 0;
        }
        if let Some(tripped_at) = inner.tripped_at {
            let elapsed = Instant::now().duration_since(tripped_at);
            inner
                .config
                .cooldown_duration
                .saturating_sub(elapsed)
                .as_secs()
        } else {
            0
        }
    }

    /// Produces a comprehensive status report
    pub fn status(&self) -> CircuitBreakerStatus {
        let inner = self.state.lock().unwrap();
        let now = Instant::now();
        let remaining_cooldown_secs = if inner.state == CircuitState::Open {
            inner
                .tripped_at
                .map(|t| {
                    let elapsed = now.duration_since(t);
                    inner
                        .config
                        .cooldown_duration
                        .saturating_sub(elapsed)
                        .as_secs()
                })
                .unwrap_or(0)
        } else {
            0
        };

        CircuitBreakerStatus {
            state: inner.state,
            is_tripped: inner.state == CircuitState::Open,
            recent_action_count: inner.history.len(),
            max_actions: inner.config.max_actions,
            window_secs: inner.config.window_duration.as_secs(),
            cooldown_secs: inner.config.cooldown_duration.as_secs(),
            remaining_cooldown_secs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_configuration_and_arm_ready_state() {
        let breaker = CircuitBreaker::default();

        assert_eq!(breaker.state(), CircuitState::Closed);
        assert!(breaker.is_arm_ready());
        assert!(!breaker.is_tripped());
        assert!(!breaker.is_half_open());
        assert_eq!(breaker.recent_action_count(), 0);
        assert_eq!(breaker.remaining_cooldown_secs(), 0);
    }

    #[test]
    fn test_rolling_sliding_window_eviction() {
        let breaker = CircuitBreaker::new(5, 60, 300);
        let start = Instant::now();

        // Record 3 actions at start
        assert!(breaker.check_and_record_at(start).is_ok());
        assert!(breaker
            .check_and_record_at(start + Duration::from_secs(10))
            .is_ok());
        assert!(breaker
            .check_and_record_at(start + Duration::from_secs(20))
            .is_ok());

        // 90 seconds later: all 3 previous actions (at +0s, +10s, +20s) have exited 60s sliding window
        let later = start + Duration::from_secs(90);
        assert!(breaker.check_and_record_at(later).is_ok());

        let inner = breaker.state.lock().unwrap();
        assert_eq!(inner.history.len(), 1);
        assert_eq!(inner.state, CircuitState::Closed);
    }

    #[test]
    fn test_trip_to_open_upon_exceeding_threshold() {
        let breaker = CircuitBreaker::new(5, 60, 300);
        let now = Instant::now();

        // 5 actions succeed
        for i in 0..5 {
            let res = breaker.check_and_record_at(now + Duration::from_secs(i));
            assert!(res.is_ok(), "Action #{} must succeed", i + 1);
        }

        assert_eq!(breaker.state(), CircuitState::Closed);

        // 6th action in same 60s window exceeds threshold -> trips to Open
        let trip_res = breaker.check_and_record_at(now + Duration::from_secs(5));
        assert!(trip_res.is_err());
        assert_eq!(breaker.state(), CircuitState::Open);
        assert!(breaker.is_tripped());

        match trip_res.err().unwrap() {
            CircuitBreakerError::Tripped {
                remaining_cooldown_secs,
            } => {
                assert_eq!(remaining_cooldown_secs, 300);
            }
            other => panic!("Expected Tripped error, got {:?}", other),
        }
    }

    #[test]
    fn test_transition_from_open_to_half_open_and_recovery() {
        let breaker = CircuitBreaker::new(2, 60, 10);
        let t0 = Instant::now();

        // Trip the breaker (2 actions + 3rd trips)
        assert!(breaker.check_and_record_at(t0).is_ok());
        assert!(breaker.check_and_record_at(t0).is_ok());
        assert!(breaker.check_and_record_at(t0).is_err());
        assert_eq!(breaker.state(), CircuitState::Open);

        // Check before cooldown has elapsed (5s < 10s cooldown)
        let t1 = t0 + Duration::from_secs(5);
        assert!(breaker.check_and_record_at(t1).is_err());
        assert_eq!(breaker.state(), CircuitState::Open);

        // Check after cooldown has elapsed (11s >= 10s cooldown) -> transitions to HalfOpen
        let t2 = t0 + Duration::from_secs(11);
        let probe_res = breaker.check_and_record_at(t2);
        assert!(probe_res.is_ok(), "Trial probe in HalfOpen must be granted");
        assert_eq!(breaker.state(), CircuitState::HalfOpen);
        assert!(breaker.is_half_open());

        // A second concurrent probe while first is in-flight is rejected
        let second_probe = breaker.check_and_record_at(t2);
        assert_eq!(second_probe, Err(CircuitBreakerError::ProbeRejected));

        // Successful completion of probe recovers circuit back to Closed
        breaker.record_probe_success();
        assert_eq!(breaker.state(), CircuitState::Closed);
        assert!(breaker.is_arm_ready());
    }

    #[test]
    fn test_half_open_probe_failure_trips_back_to_open() {
        let breaker = CircuitBreaker::new(2, 60, 10);
        let t0 = Instant::now();

        // Trip the breaker
        assert!(breaker.check_and_record_at(t0).is_ok());
        assert!(breaker.check_and_record_at(t0).is_ok());
        assert!(breaker.check_and_record_at(t0).is_err());

        // Advance past cooldown into HalfOpen probe
        let t1 = t0 + Duration::from_secs(11);
        assert!(breaker.check_and_record_at(t1).is_ok());
        assert_eq!(breaker.state(), CircuitState::HalfOpen);

        // Probe fails -> trips back to Open
        breaker.record_probe_failure();
        assert_eq!(breaker.state(), CircuitState::Open);
        assert!(breaker.is_tripped());
    }

    #[test]
    fn test_manual_reset_and_trip() {
        let breaker = CircuitBreaker::default();

        breaker.trip();
        assert_eq!(breaker.state(), CircuitState::Open);
        assert!(breaker.is_tripped());

        breaker.reset();
        assert_eq!(breaker.state(), CircuitState::Closed);
        assert!(breaker.is_arm_ready());
    }

    #[test]
    fn test_thread_safe_concurrent_access() {
        let breaker = CircuitBreaker::new(100, 60, 300);
        let mut handles = Vec::new();

        for _ in 0..10 {
            let b = breaker.clone();
            handles.push(std::thread::spawn(move || {
                for _ in 0..5 {
                    let _ = b.check_and_record();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(breaker.recent_action_count(), 50);
        assert_eq!(breaker.state(), CircuitState::Closed);
    }

    #[test]
    fn test_phase_15_2_mitigation_locks_and_operator_confirmation() {
        let breaker = CircuitBreaker::new(3, 60, 300);
        let now = Instant::now();

        // 1. Fill sliding window up to threshold
        assert!(breaker
            .try_acquire_mitigation_at(MitigationOrigin::Automated, now)
            .is_ok());
        assert!(breaker
            .try_acquire_mitigation_at(MitigationOrigin::Batch, now)
            .is_ok());
        assert!(breaker
            .try_acquire_mitigation_at(
                MitigationOrigin::Manual {
                    operator_confirmed: false
                },
                now
            )
            .is_ok());

        assert_eq!(breaker.state(), CircuitState::Closed);

        // 2. Next automated action exceeds limit (3 actions in 60s) -> trips to Open
        let trip_err = breaker
            .try_acquire_mitigation_at(MitigationOrigin::Automated, now)
            .unwrap_err();
        assert!(matches!(trip_err, CircuitBreakerError::Tripped { .. }));
        assert_eq!(breaker.state(), CircuitState::Open);

        // 3. Subsequent automated call must be locked with AutomatedActionsLocked
        let auto_err = breaker
            .try_acquire_mitigation_at(MitigationOrigin::Automated, now + Duration::from_secs(1))
            .unwrap_err();
        assert_eq!(auto_err, CircuitBreakerError::AutomatedActionsLocked);

        // 4. Subsequent batch moderation call must be locked with BatchActionsLocked
        let batch_err = breaker
            .try_acquire_mitigation_at(MitigationOrigin::Batch, now + Duration::from_secs(2))
            .unwrap_err();
        assert_eq!(batch_err, CircuitBreakerError::BatchActionsLocked);

        // 5. Subsequent manual action without explicit confirmation must be rejected
        let manual_unconfirmed = breaker
            .try_acquire_mitigation_at(
                MitigationOrigin::Manual {
                    operator_confirmed: false,
                },
                now + Duration::from_secs(3),
            )
            .unwrap_err();
        assert_eq!(
            manual_unconfirmed,
            CircuitBreakerError::ConfirmationRequired
        );

        // 6. Explicit operator manual confirmation bypasses the lock for emergency manual action
        let manual_confirmed = breaker.try_acquire_mitigation_at(
            MitigationOrigin::Manual {
                operator_confirmed: true,
            },
            now + Duration::from_secs(4),
        );
        assert!(
            manual_confirmed.is_ok(),
            "Manual mitigation with explicit operator confirmation must proceed"
        );
    }

    #[test]
    fn test_cooling_timer_second_by_second_countdown() {
        let breaker = CircuitBreaker::new(1, 60, 300);
        let t0 = Instant::now();

        // 1. Initial Closed state has 0 remaining cooldown
        assert_eq!(breaker.remaining_cooldown_secs(), 0);

        // 2. Action 1 succeeds, Action 2 trips breaker at t0
        assert!(breaker.check_and_record_at(t0).is_ok());
        assert!(breaker.check_and_record_at(t0).is_err());
        assert_eq!(breaker.state(), CircuitState::Open);

        // 3. Check countdown at various elapsed intervals
        let check_countdown = |elapsed_secs: u64| {
            let now = t0 + Duration::from_secs(elapsed_secs);
            let inner = breaker.state.lock().unwrap();
            let tripped_at = inner.tripped_at.unwrap();
            let elapsed = now.duration_since(tripped_at);
            inner
                .config
                .cooldown_duration
                .saturating_sub(elapsed)
                .as_secs()
        };

        assert_eq!(check_countdown(0), 300, "Initial countdown is 300s");
        assert_eq!(check_countdown(1), 299, "Countdown after 1s is 299s");
        assert_eq!(check_countdown(60), 240, "Countdown after 1 min is 240s");
        assert_eq!(check_countdown(150), 150, "Countdown halfway is 150s");
        assert_eq!(check_countdown(299), 1, "Countdown with 1s left is 1s");
        assert_eq!(check_countdown(300), 0, "Countdown at 300s reaches 0");
        assert_eq!(check_countdown(305), 0, "Countdown past 300s stays at 0");

        // 4. Verify status struct accurately reports countdown
        let status = breaker.status();
        assert!(status.is_tripped);
        assert_eq!(status.cooldown_secs, 300);
    }

    #[test]
    fn test_log_circuit_breaker_tripped_and_reset_audit_trail() {
        let storage = crate::storage::StorageManager::in_memory().expect("In-memory storage");
        let conn_guard = storage.get_connection();
        let conn = conn_guard.lock().unwrap();

        let breaker = CircuitBreaker::default();

        // 1. Log trip event
        let trip_id = breaker
            .trip_with_audit(
                Some(&conn),
                "system",
                "Automated burst of 6 actions in 60s exceeded threshold",
            )
            .expect("Trip audit write must succeed")
            .expect("Audit ID must be returned");
        assert!(trip_id.starts_with("audit_cb_"));
        assert!(breaker.is_tripped());

        // 2. Verify trip record in SQLite audit_logs table
        let (action, reason): (String, String) = conn
            .query_row(
                "SELECT action, reason FROM audit_logs WHERE id = ?1",
                params![trip_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("Must find tripped audit record");
        assert_eq!(action, "circuit_breaker_tripped");
        assert!(reason.contains("exceeded threshold"));

        // 3. Log reset event
        let reset_id = breaker
            .reset_with_audit(
                Some(&conn),
                "sec_operator_42",
                "Manual operator reset via system tray control",
            )
            .expect("Reset audit write must succeed")
            .expect("Audit ID must be returned");
        assert!(reset_id.starts_with("audit_cb_"));
        assert!(!breaker.is_tripped());
        assert!(breaker.is_arm_ready());

        // 4. Verify reset record in SQLite audit_logs table
        let (action_reset, operator, reason_reset): (String, String, String) = conn
            .query_row(
                "SELECT action, operator_id, reason FROM audit_logs WHERE id = ?1",
                params![reset_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("Must find reset audit record");
        assert_eq!(action_reset, "circuit_breaker_reset");
        assert_eq!(operator, "sec_operator_42");
        assert!(reason_reset.contains("system tray control"));
    }
}
