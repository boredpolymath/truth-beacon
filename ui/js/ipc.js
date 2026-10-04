import { INITIAL_BENCHMARKS, INITIAL_INCIDENTS, INITIAL_AUDIT_LOGS } from './mock_data.js';

let mockBenchmarks = [...INITIAL_BENCHMARKS];
let mockIncidents = [...INITIAL_INCIDENTS];
let mockAuditLogs = [...INITIAL_AUDIT_LOGS];
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
        gateway_connected: false,
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
      mockAuditLogs.unshift({
        id: `aud_${now}`,
        timestamp: now,
        action: "create_benchmark",
        guild_id: newBm.guild_id,
        operator_id: "LocalSteward",
        target_user_id: newBm.user_id,
        incident_id: null,
        reason: `Created benchmark for @${newBm.canonical_username}`,
        metadata: { role: newBm.community_role }
      });
      return newBm;
    }

    case 'list_incidents':
      return [...mockIncidents];

    case 'resolve_incident': {
      const { incident_id, status, resolution_notes, operator_id, target_benchmark_id } = args;
      const inc = mockIncidents.find(i => i.id === incident_id);
      const now = Date.now();
      if (inc) {
        inc.status = status;
        inc.resolution_notes = resolution_notes;
        inc.resolved_at = Math.floor(now / 1000);
      }

      if (status === 'whitelisted') {
        const targetId = target_benchmark_id || inc?.discrepancy?.matched_benchmark_id;
        const bm = mockBenchmarks.find(b => b.id === targetId);
        if (bm && inc?.discrepancy?.suspect_user_id) {
          const altTag = `Whitelisted Alt: ${inc.discrepancy.suspect_user_id}`;
          if (!bm.tags.includes(altTag)) bm.tags.push(altTag);
          if (!bm.tags.includes("Authorized Alt")) bm.tags.push("Authorized Alt");
        }
      }

      const auditAction = status === 'banned' ? 'ban_and_purge'
        : (status === 'whitelisted' ? 'whitelist_alternate'
        : (status === 'excluded' ? 'exclude_user' : 'dismiss'));

      mockAuditLogs.unshift({
        id: `aud_${now}`,
        timestamp: now,
        action: auditAction,
        guild_id: inc?.guild_id || "guild_crossroads_9921",
        operator_id: operator_id || "LocalSteward",
        target_user_id: inc?.discrepancy?.suspect_user_id || "user_unknown",
        incident_id,
        reason: resolution_notes || `Adjudicated with status: ${status}`,
        metadata: {
          suspect_username: inc?.discrepancy?.suspect_username,
          status,
          target_benchmark_id
        }
      });

      return true;
    }

    case 'list_audit_logs':
      return [...mockAuditLogs];

    case 'reset_circuit_breaker':
      mockCircuitBreakerTripped = false;
      mockAuditLogs.unshift({
        id: `aud_${Date.now()}`,
        timestamp: Date.now(),
        action: "circuit_breaker_reset",
        guild_id: "global",
        operator_id: "LocalSteward",
        target_user_id: null,
        incident_id: null,
        reason: "Manual operator reset via console or system tray",
        metadata: null
      });
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
      mockAuditLogs = [];
      return true;

    case 'dispatch_desktop_notification':
      console.info("[TruthBeacon IPC] Dispatched OS notification:", args.payload);
      return true;

    case 'execute_notification_action': {
      const { incident_id, action } = args;
      console.info(`[TruthBeacon IPC] Executing notification action '${action}' for ${incident_id}`);
      if (action === 'Inspect') {
        return true;
      } else if (action === 'Dismiss') {
        return await invokeCommand('resolve_incident', {
          incident_id,
          status: 'dismissed',
          resolution_notes: 'Dismissed via desktop notification toast action button',
          operator_id: 'NotificationToast'
        });
      } else if (action === 'Ban & Purge') {
        return await invokeCommand('resolve_incident', {
          incident_id,
          status: 'banned',
          resolution_notes: 'Banned and purged via desktop notification toast action button',
          operator_id: 'NotificationToast'
        });
      }
      return true;
    }

    case 'get_discord_config':
      return {
        has_token: false,
        guild_id: "",
        registered_guilds: [],
        string_similarity_threshold: 0.85,
        new_account_age_hours_threshold: 72,
        avatar_hamming_threshold: 10,
        privileged_intent_declared: true
      };

    case 'verify_bot_handshake': {
      const { token, guild_id } = args;
      if (!token || token.length < 15) {
        throw { code: "VALIDATION_FAILED", message: "Invalid bot token format: Discord Bot tokens must contain 3 segments." };
      }
      return {
        bot_id: "109827364512938475",
        bot_username: "TruthBeacon Guard",
        bot_discriminator: "0",
        bot_avatar: null,
        is_official_bot: true,
        target_guild_id: guild_id || "",
        target_guild_name: guild_id ? `Server (${guild_id})` : "Verified Community Sanctuary",
        permissions: {
          is_administrator: false,
          has_kick_members: true,
          has_ban_members: true,
          has_moderate_members: true,
          has_view_channel: true,
          is_fully_authorized: true,
          missing_permissions: []
        },
        verified_at: Math.floor(Date.now() / 1000)
      };
    }

    case 'save_discord_config': {
      const { guild_id, token } = args;
      if (!guild_id?.trim()) {
        throw { code: "VALIDATION_FAILED", message: "Server Guild ID cannot be empty" };
      }
      if (!token?.trim()) {
        throw { code: "VALIDATION_FAILED", message: "Bot token cannot be empty" };
      }
      return {
        bot_id: "109827364512938475",
        bot_username: "TruthBeacon Guard",
        bot_discriminator: "0",
        bot_avatar: null,
        is_official_bot: true,
        target_guild_id: guild_id,
        target_guild_name: "Crossroads Community Sanctuary",
        permissions: {
          is_administrator: false,
          has_kick_members: true,
          has_ban_members: true,
          has_moderate_members: true,
          has_view_channel: true,
          is_fully_authorized: true,
          missing_permissions: []
        },
        verified_at: Math.floor(Date.now() / 1000)
      };
    }

    case 'disconnect_discord':
      return true;

    default:
      console.warn(`[TruthBeacon Mock IPC] Unhandled command: ${cmd}`);
      return null;
  }
}
