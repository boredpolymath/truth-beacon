import { INITIAL_BENCHMARKS, INITIAL_INCIDENTS, INITIAL_AUDIT_LOGS } from './mock_data.js';
import { invokeCommand } from './ipc.js';

class StateStore {
  constructor() {
    this.benchmarks = [...INITIAL_BENCHMARKS];
    this.incidents = [...INITIAL_INCIDENTS];
    this.auditLogs = [...INITIAL_AUDIT_LOGS];
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

  setTab(tab) {
    this.activeTab = tab;
    this.notify();
  }

  resolveIncident(id, action, reason = "") {
    const incIndex = this.incidents.findIndex(i => i.id === id);
    if (incIndex === -1) return;

    const incident = this.incidents[incIndex];
    let newStatus = 'dismissed';
    let auditAction = 'dismiss';

    if (action === 'ban') {
      newStatus = 'banned';
      auditAction = 'ban_and_purge';
    } else if (action === 'exclude') {
      newStatus = 'excluded';
      auditAction = 'exclude_user';
    } else if (action === 'whitelist') {
      newStatus = 'whitelisted';
      auditAction = 'whitelist_alternate';
    }

    incident.status = newStatus;
    incident.resolved_at = Date.now();
    incident.resolution_notes = reason || `Actioned by operator: ${action}`;

    // Record audit entry
    this.auditLogs.unshift({
      id: `aud_${Date.now()}`,
      timestamp: Date.now(),
      action: auditAction,
      guild_id: this.selectedGuild.id,
      operator_id: "LocalOperator",
      target_user_id: incident.discrepancy.suspect_user_id,
      incident_id: id,
      reason: incident.resolution_notes,
      metadata: { action, suspect_username: incident.discrepancy.suspect_username }
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
    this.benchmarks.push(benchmark);
    this.auditLogs.unshift({
      id: `aud_${Date.now()}`,
      timestamp: Date.now(),
      action: 'create_benchmark',
      guild_id: this.selectedGuild.id,
      operator_id: "LocalOperator",
      target_user_id: benchmark.user_id,
      incident_id: null,
      reason: `Added benchmark for ${benchmark.canonical_username}`,
      metadata: { role: benchmark.community_role }
    });
    this.notify();
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
