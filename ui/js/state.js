import { INITIAL_BENCHMARKS, INITIAL_INCIDENTS, INITIAL_AUDIT_LOGS } from './mock_data.js';
import { invokeCommand } from './ipc.js';

class StateStore {
  constructor() {
    this.benchmarks = JSON.parse(JSON.stringify(INITIAL_BENCHMARKS));
    this.incidents = JSON.parse(JSON.stringify(INITIAL_INCIDENTS));
    this.auditLogs = JSON.parse(JSON.stringify(INITIAL_AUDIT_LOGS));
    this.activeTab = 'triage';
    this.selectedGuild = {
      id: 'guild_crossroads_9921',
      name: 'Crossroads Community Sanctuary',
      member_count: 1420
    };
    this.daemonHealth = {
      online: true,
      gateway_connected: true,
      latency_ms: 24,
      memory_mb: 26.4,
      circuit_breaker: {
        tripped: false,
        rolling_requests: 1,
        max_requests: 5,
        remaining_cooldown: 0
      }
    };
    this.listeners = [];

    // Subtle gentle latency jitter (21 - 28ms) to reflect live WebSocket connection
    setInterval(() => {
      this.daemonHealth.latency_ms = 20 + Math.floor(Math.random() * 8);
      this.notifyHeaderOnly();
    }, 4000);
  }

  subscribe(listener) {
    this.listeners.push(listener);
    return () => {
      this.listeners = this.listeners.filter(l => l !== listener);
    };
  }

  notify() {
    for (const listener of this.listeners) {
      listener(this);
    }
  }

  notifyHeaderOnly() {
    const latencyEl = document.getElementById('gateway-latency');
    if (latencyEl) {
      latencyEl.textContent = `${this.daemonHealth.latency_ms}ms`;
    }
  }

  setTab(tab) {
    this.activeTab = tab;
    this.notify();
  }

  resolveIncident(id, action, reason = "", altMetadata = null) {
    const incIndex = this.incidents.findIndex(i => i.id === id);
    if (incIndex === -1) return;

    const incident = this.incidents[incIndex];
    let newStatus = 'dismissed';
    let auditAction = 'dismiss_coincidence';

    if (action === 'ban') {
      newStatus = 'adjudicated';
      auditAction = 'adjudicate_impersonation';
    } else if (action === 'exclude') {
      newStatus = 'quarantined';
      auditAction = 'quarantine_user';
    } else if (action === 'whitelist') {
      newStatus = 'authorized_alt';
      auditAction = 'authorize_alternate';

      // Link alternate identity to target benchmark if provided
      if (altMetadata && altMetadata.targetBenchmarkId) {
        const bm = this.benchmarks.find(b => b.id === altMetadata.targetBenchmarkId);
        if (bm) {
          if (!bm.authorized_alts) bm.authorized_alts = [];
          bm.authorized_alts.push({
            user_id: incident.discrepancy.suspect_user_id,
            label: altMetadata.label || "Authorized Secondary Account",
            note: reason || altMetadata.note || "Operator verified alternate identity"
          });
        }
      }
    }

    incident.status = newStatus;
    incident.resolved_at = Date.now();
    incident.resolution_notes = reason || `Adjudicated by operator: ${auditAction}`;

    // Record audit entry in local ledger
    this.auditLogs.unshift({
      id: `aud_${Date.now()}`,
      timestamp: Date.now(),
      action: auditAction,
      guild_id: this.selectedGuild.id,
      operator_id: "LocalSteward",
      target_user_id: incident.discrepancy.suspect_user_id,
      incident_id: id,
      reason: incident.resolution_notes,
      metadata: { action, suspect_username: incident.discrepancy.suspect_username, altMetadata }
    });

    // Check circuit breaker trigger simulation
    this.daemonHealth.circuit_breaker.rolling_requests += 1;
    if (this.daemonHealth.circuit_breaker.rolling_requests >= this.daemonHealth.circuit_breaker.max_requests) {
      this.daemonHealth.circuit_breaker.tripped = true;
      this.daemonHealth.circuit_breaker.remaining_cooldown = 60;
    }

    invokeCommand('resolve_incident', { incident_id: id, status: newStatus, resolution_notes: incident.resolution_notes });
    this.notify();
  }

  resetCircuitBreaker() {
    this.daemonHealth.circuit_breaker.tripped = false;
    this.daemonHealth.circuit_breaker.rolling_requests = 0;
    this.daemonHealth.circuit_breaker.remaining_cooldown = 0;
    invokeCommand('reset_circuit_breaker');
    this.notify();
  }

  addBenchmark(benchmark) {
    const newBm = {
      id: benchmark.id || `bm_${Date.now()}`,
      guild_id: benchmark.guild_id || this.selectedGuild.id,
      user_id: benchmark.user_id,
      canonical_username: benchmark.canonical_username,
      server_nickname: benchmark.server_nickname || null,
      community_role: benchmark.community_role || "Community Steward",
      avatar_url: benchmark.avatar_url || "assets/logo.svg",
      avatar_perceptual_hash: benchmark.avatar_perceptual_hash || "e2b4f9c18d0739aa",
      is_active: true,
      tags: benchmark.tags || ["Verified Baseline"],
      authorized_alts: [],
      created_at: Math.floor(Date.now() / 1000),
      updated_at: Math.floor(Date.now() / 1000)
    };

    this.benchmarks.unshift(newBm);
    this.auditLogs.unshift({
      id: `aud_${Date.now()}`,
      timestamp: Date.now(),
      action: 'create_benchmark',
      guild_id: this.selectedGuild.id,
      operator_id: "LocalSteward",
      target_user_id: newBm.user_id,
      incident_id: null,
      reason: `Enrolled canonical ground-truth benchmark for ${newBm.canonical_username} (${newBm.community_role})`,
      metadata: { role: newBm.community_role, tags: newBm.tags }
    });

    invokeCommand('create_benchmark', { input: newBm });
    this.notify();
    return newBm;
  }

  removeBenchmark(id) {
    this.benchmarks = this.benchmarks.filter(b => b.id !== id);
    this.notify();
  }

  eradicateLocalData() {
    this.benchmarks = [];
    this.incidents = [];
    this.auditLogs = [];
    invokeCommand('eradicate_local_data');
    this.notify();
  }
}

export const appState = new StateStore();
