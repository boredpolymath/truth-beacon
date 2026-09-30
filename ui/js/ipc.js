import { INITIAL_BENCHMARKS, INITIAL_INCIDENTS } from './mock_data.js';

let mockBenchmarks = [...INITIAL_BENCHMARKS];
let mockIncidents = [...INITIAL_INCIDENTS];
let mockCircuitBreakerTripped = false;

export const isTauriEnvironment = () => {
  return typeof window !== "undefined" && (window.__TAURI_INTERNALS__ !== undefined || window.__TAURI__ !== undefined);
};

export async function invokeCommand(cmd, args = {}) {
  if (isTauriEnvironment() && window.__TAURI__?.core?.invoke) {
    try {
      return await window.__TAURI__.core.invoke(cmd, args);
    } catch (err) {
      console.warn(`[TruthBeacon IPC] Failed invoking ${cmd}:`, err);
      throw err;
    }
  }

  // Graceful browser mock IPC fallback for UI preview, rapid prototyping, and design testing
  console.info(`[TruthBeacon Mock IPC] Executing: ${cmd}`, args);
  await new Promise(r => setTimeout(r, 15)); // Simulate realistic 15ms IPC latency

  switch (cmd) {
    case 'get_system_status':
      return {
        daemon_healthy: true,
        gateway_connected: true,
        circuit_breaker_tripped: mockCircuitBreakerTripped,
        db_path: "/Users/local/.truthbeacon/truthbeacon.local.db",
        pending_incidents_count: mockIncidents.filter(i => i.status === 'pending').length,
        benchmark_count: mockBenchmarks.length
      };

    case 'list_benchmarks':
      return [...mockBenchmarks];

    case 'create_benchmark': {
      const input = args.input || {};
      if (!input.user_id?.trim()) {
        throw { code: "VALIDATION_FAILED", message: "Validation failed: Snowflake user ID cannot be empty", details: null };
      }
      if (!input.canonical_username?.trim()) {
        throw { code: "VALIDATION_FAILED", message: "Validation failed: Canonical username cannot be empty", details: null };
      }
      const now = Date.now();
      const newBm = {
        id: `bm_${now}`,
        guild_id: input.guild_id || "guild_default",
        user_id: input.user_id,
        canonical_username: input.canonical_username,
        server_nickname: input.server_nickname || null,
        community_role: input.community_role || "Staff",
        avatar_url: input.avatar_url || null,
        avatar_perceptual_hash: null,
        is_active: true,
        tags: input.tags || [],
        created_at: Math.floor(now / 1000),
        updated_at: Math.floor(now / 1000)
      };
      mockBenchmarks.push(newBm);
      return newBm;
    }

    case 'list_incidents':
      return [...mockIncidents];

    case 'resolve_incident': {
      const { incident_id, status, resolution_notes } = args;
      const inc = mockIncidents.find(i => i.id === incident_id);
      if (inc) {
        inc.status = status;
        inc.resolution_notes = resolution_notes;
        inc.resolved_at = Math.floor(Date.now() / 1000);
      }
      return true;
    }

    case 'reset_circuit_breaker':
      mockCircuitBreakerTripped = false;
      return true;

    case 'run_sandbox_simulation': {
      const { candidate_username, target_benchmark_name } = args;
      const isHomoglyph = candidate_username.includes('\u043E') || candidate_username.includes('\u0428') || candidate_username.includes('\u200B');
      const score = isHomoglyph ? 0.98 : (candidate_username.toLowerCase() === target_benchmark_name.toLowerCase() ? 1.0 : 0.45);
      return {
        candidate_input: candidate_username,
        candidate_normalized: candidate_username.toLowerCase(),
        target_normalized: target_benchmark_name.toLowerCase(),
        similarity_score: score,
        jaro_winkler: score,
        damerau_distance: isHomoglyph ? 1 : 4,
        homoglyph_detected: isHomoglyph,
        recommended_tier: isHomoglyph || score > 0.9 ? "critical" : (score > 0.8 ? "notable" : "standard")
      };
    }

    case 'eradicate_local_data':
      mockBenchmarks = [];
      mockIncidents = [];
      return true;

    default:
      console.warn(`[TruthBeacon Mock IPC] Unhandled command: ${cmd}`);
      return null;
  }
}
