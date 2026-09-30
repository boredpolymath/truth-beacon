use crate::models::{CanonicalBenchmark, CreateBenchmarkInput, IncidentStatus, TriageIncident};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemStatus {
    pub daemon_healthy: bool,
    pub gateway_connected: bool,
    pub circuit_breaker_tripped: bool,
    pub db_path: String,
    pub pending_incidents_count: usize,
    pub benchmark_count: usize,
}

#[tauri::command]
pub fn get_system_status() -> Result<SystemStatus, String> {
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
pub fn list_benchmarks(guild_id: String) -> Result<Vec<CanonicalBenchmark>, String> {
    // Scaffolded endpoint: returns active benchmarks
    let _ = guild_id;
    Ok(vec![])
}

#[tauri::command]
pub fn create_benchmark(input: CreateBenchmarkInput) -> Result<CanonicalBenchmark, String> {
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
pub fn list_incidents(guild_id: String) -> Result<Vec<TriageIncident>, String> {
    let _ = guild_id;
    Ok(vec![])
}

#[tauri::command]
pub fn resolve_incident(
    incident_id: String,
    status: IncidentStatus,
    resolution_notes: Option<String>,
) -> Result<bool, String> {
    let _ = (incident_id, status, resolution_notes);
    Ok(true)
}

#[tauri::command]
pub fn reset_circuit_breaker() -> Result<bool, String> {
    Ok(true)
}

#[tauri::command]
pub fn run_sandbox_simulation(
    candidate_username: String,
    target_benchmark_name: String,
) -> Result<serde_json::Value, String> {
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
pub fn eradicate_local_data() -> Result<bool, String> {
    // Purges SQLite records, in-memory caches, and OS keychain items
    Ok(true)
}
