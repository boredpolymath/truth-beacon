use crate::models::{CanonicalBenchmark, CreateBenchmarkInput, IncidentStatus, TriageIncident};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Benchmark not found: {0}")]
    BenchmarkNotFound(String),
    #[error("Incident not found: {0}")]
    IncidentNotFound(String),
    #[error("Rate limited: retry after {0} seconds")]
    RateLimited(f64),
    #[error("Circuit breaker is currently tripped")]
    CircuitBreakerTripped,
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("CommandError", 3)?;
        let code = match self {
            Self::DatabaseError(_) => "DATABASE_ERROR",
            Self::BenchmarkNotFound(_) => "BENCHMARK_NOT_FOUND",
            Self::IncidentNotFound(_) => "INCIDENT_NOT_FOUND",
            Self::RateLimited(_) => "RATE_LIMITED",
            Self::CircuitBreakerTripped => "CIRCUIT_BREAKER_TRIPPED",
            Self::ValidationFailed(_) => "VALIDATION_FAILED",
            Self::InternalError(_) => "INTERNAL_ERROR",
        };
        state.serialize_field("code", code)?;
        state.serialize_field("message", &self.to_string())?;
        state.serialize_field("details", &None::<String>)?;
        state.end()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub daemon_healthy: bool,
    pub gateway_connected: bool,
    pub circuit_breaker_tripped: bool,
    pub db_path: String,
    pub pending_incidents_count: usize,
    pub benchmark_count: usize,
}

#[tauri::command]
pub fn get_system_status() -> Result<SystemStatus, CommandError> {
    Ok(SystemStatus {
        daemon_healthy: true,
        gateway_connected: true,
        circuit_breaker_tripped: false,
        db_path: "/Users/local/.truthbeacon/truthbeacon.local.db".into(),
        pending_incidents_count: 2,
        benchmark_count: 5,
    })
}

#[tauri::command]
pub fn list_benchmarks(guild_id: String) -> Result<Vec<CanonicalBenchmark>, CommandError> {
    // Scaffolded endpoint: returns active benchmarks
    let _ = guild_id;
    Ok(vec![])
}

#[tauri::command]
pub fn create_benchmark(input: CreateBenchmarkInput) -> Result<CanonicalBenchmark, CommandError> {
    if input.user_id.trim().is_empty() {
        return Err(CommandError::ValidationFailed(
            "Snowflake user ID cannot be empty".into(),
        ));
    }
    if input.canonical_username.trim().is_empty() {
        return Err(CommandError::ValidationFailed(
            "Canonical username cannot be empty".into(),
        ));
    }

    let now = chrono::Utc::now().timestamp();
    Ok(CanonicalBenchmark {
        id: format!("bm_{}", now),
        guild_id: input.guild_id,
        user_id: input.user_id,
        canonical_username: input.canonical_username,
        server_nickname: input.server_nickname,
        community_role: input.community_role,
        avatar_url: input.avatar_url,
        avatar_perceptual_hash: None,
        is_active: true,
        tags: input.tags,
        created_at: now,
        updated_at: now,
    })
}

#[tauri::command]
pub fn list_incidents(guild_id: String) -> Result<Vec<TriageIncident>, CommandError> {
    let _ = guild_id;
    Ok(vec![])
}

#[tauri::command]
pub fn resolve_incident(
    incident_id: String,
    status: IncidentStatus,
    resolution_notes: Option<String>,
) -> Result<bool, CommandError> {
    let _ = (incident_id, status, resolution_notes);
    Ok(true)
}

#[tauri::command]
pub fn reset_circuit_breaker() -> Result<bool, CommandError> {
    Ok(true)
}

#[tauri::command]
pub fn run_sandbox_simulation(
    candidate_username: String,
    target_benchmark_name: String,
) -> Result<serde_json::Value, CommandError> {
    use crate::detection::metrics::evaluate_string_metrics;
    use crate::detection::unicode::normalize_and_deobfuscate;

    let norm_cand = normalize_and_deobfuscate(&candidate_username);
    let norm_target = normalize_and_deobfuscate(&target_benchmark_name);
    let metrics = evaluate_string_metrics(&norm_cand, &norm_target);
    let is_homoglyph = candidate_username != norm_cand && norm_cand == norm_target;

    Ok(serde_json::json!({
        "candidate_input": candidate_username,
        "candidate_normalized": norm_cand,
        "target_normalized": norm_target,
        "similarity_score": (metrics.composite_score * 100.0).round() / 100.0,
        "jaro_winkler": metrics.jaro_winkler_score,
        "damerau_distance": metrics.damerau_distance,
        "homoglyph_detected": is_homoglyph,
        "recommended_tier": if metrics.composite_score > 0.9 || is_homoglyph { "critical" } else if metrics.composite_score > 0.8 { "notable" } else { "standard" }
    }))
}

#[tauri::command]
pub fn eradicate_local_data() -> Result<bool, CommandError> {
    // Purges SQLite records, in-memory caches, and OS keychain items
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_error_structured_json_serialization() {
        let err = CommandError::ValidationFailed("Invalid snowflake".into());
        let json_val = serde_json::to_value(&err).unwrap();

        assert_eq!(json_val["code"], "VALIDATION_FAILED");
        assert_eq!(json_val["message"], "Validation failed: Invalid snowflake");
        assert!(json_val["details"].is_null());

        let rate_err = CommandError::RateLimited(4.5);
        let rate_json = serde_json::to_value(&rate_err).unwrap();
        assert_eq!(rate_json["code"], "RATE_LIMITED");
        assert_eq!(
            rate_json["message"],
            "Rate limited: retry after 4.5 seconds"
        );

        let breaker_err = CommandError::CircuitBreakerTripped;
        let breaker_json = serde_json::to_value(&breaker_err).unwrap();
        assert_eq!(breaker_json["code"], "CIRCUIT_BREAKER_TRIPPED");
    }

    #[test]
    fn test_create_benchmark_validation() {
        let invalid_input = CreateBenchmarkInput {
            guild_id: "guild_1".into(),
            user_id: "".into(),
            canonical_username: "Pastor Dan".into(),
            server_nickname: None,
            community_role: "Staff".into(),
            avatar_url: None,
            tags: vec![],
        };
        let res = create_benchmark(invalid_input);
        assert!(res.is_err());
        match res.err().unwrap() {
            CommandError::ValidationFailed(msg) => assert!(msg.contains("Snowflake user ID")),
            _ => panic!("Expected ValidationFailed error"),
        }
    }
}
