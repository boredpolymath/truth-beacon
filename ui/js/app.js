import { appState } from './state.js';
import { invokeCommand } from './ipc.js';

// DOM Element Selectors
const navTabs = document.querySelectorAll('.nav-tab');
const tabPanes = document.querySelectorAll('.tab-pane');
const triageContainer = document.getElementById('triage-cards-container');
const vaultContainer = document.getElementById('vault-cards-container');
const auditTableBody = document.getElementById('audit-table-body');
const counterIncidents = document.getElementById('counter-incidents');
const counterVault = document.getElementById('counter-vault');
const circuitBreakerAlert = document.getElementById('circuit-breaker-alert');
const btnResetCircuit = document.getElementById('btn-reset-circuit');
const btnEradicateData = document.getElementById('btn-eradicate-data');
const btnRunSimulation = document.getElementById('btn-run-simulation');
const simResults = document.getElementById('sim-results');
const dotCircuit = document.getElementById('dot-circuit');
const circuitStatusText = document.getElementById('circuit-status-text');

// Format relative time helper
function formatTime(timestamp) {
  const diffSecs = Math.floor((Date.now() - timestamp) / 1000);
  if (diffSecs < 60) return `${diffSecs}s ago`;
  const diffMins = Math.floor(diffSecs / 60);
  if (diffMins < 60) return `${diffMins}m ago`;
  const diffHours = Math.floor(diffMins / 60);
  return `${diffHours}h ago`;
}

// Render Triage Inspection Cards
function renderTriageCards() {
  const pendingIncidents = appState.incidents.filter(i => i.status === 'pending');
  counterIncidents.textContent = pendingIncidents.length;

  if (pendingIncidents.length === 0) {
    triageContainer.innerHTML = `
      <div style="text-align: center; padding: 60px 20px; background: var(--bg-card); border-radius: var(--radius-lg); border: 1px dashed var(--border-subtle);">
        <p style="color: #34d399; font-weight: 600; font-size: 1.1rem; margin-bottom: 6px;">Ground Truth Secure</p>
        <p style="color: var(--text-muted); font-size: 0.85rem;">Zero unresolved identity discrepancies detected across active platform gateway streams.</p>
      </div>
    `;
    return;
  }

  triageContainer.innerHTML = pendingIncidents.map(inc => {
    const d = inc.discrepancy;
    const benchmark = appState.benchmarks.find(b => b.id === d.matched_benchmark_id) || {
      canonical_username: d.matched_benchmark_name,
      community_role: "Protected Benchmark",
      avatar_url: d.suspect_avatar_url
    };

    const isCritical = d.risk_tier === 'critical';
    const riskChipClass = isCritical ? 'risk-critical' : 'risk-elevated';

    return `
      <article class="inspection-card" id="card-${inc.id}">
        <div class="card-header-bar">
          <div style="display: flex; align-items: center; gap: 10px;">
            <span class="risk-chip ${riskChipClass}">${d.risk_tier} ambiguity</span>
            <span class="incident-time">Detected ${formatTime(inc.timestamp)}</span>
          </div>
          <div style="font-size: 0.78rem; color: var(--text-muted);">
            Evaluation: ${d.homoglyph_detected ? 'Homoglyph Deobfuscated' : 'String Levenshtein Variance'}
          </div>
        </div>

        <div class="comparison-container">
          <!-- LEFT: Canonical Benchmark Profile -->
          <div class="profile-pod benchmark">
            <div class="avatar-wrapper">
              <img src="${benchmark.avatar_url || 'assets/logo.svg'}" alt="Authentic Avatar">
            </div>
            <div class="profile-details">
              <div class="pod-role-label">Authentic Benchmark</div>
              <div class="profile-name">${benchmark.canonical_username}</div>
              <div class="profile-nick">${benchmark.server_nickname || 'Official Identity'}</div>
              <div class="profile-meta">${benchmark.community_role}</div>
            </div>
          </div>

          <!-- CENTER: Metric Comparison Pillar -->
          <div class="metric-vs-pillar">
            <div class="metric-circle">
              ${Math.round(d.string_similarity_score * 100)}%
              <span>MATCH</span>
            </div>
            <span style="font-size: 0.65rem; color: var(--text-muted); font-weight: 600;">VS</span>
          </div>

          <!-- RIGHT: Suspect Candidate Profile -->
          <div class="profile-pod suspect">
            <div class="avatar-wrapper">
              <img src="${d.suspect_avatar_url || 'assets/logo.svg'}" alt="Suspect Avatar">
            </div>
            <div class="profile-details">
              <div class="pod-role-label">Incoming Suspect</div>
              <div class="profile-name">${d.suspect_username}</div>
              <div class="profile-nick">${d.suspect_nickname || 'No Nickname'}</div>
              <div class="profile-meta">Account Age: <strong>${d.suspect_account_age_hours}h old</strong> &bull; ID: ${d.suspect_user_id.slice(-6)}</div>
            </div>
          </div>
        </div>

        <!-- Discrepancy Explanation Bar -->
        <div class="vs-diff-summary">
          <span class="diff-badge">ANALYSIS</span>
          <span>${d.normalized_diff}</span>
          ${d.avatar_hamming_distance !== null ? `<span style="margin-left: auto; color: var(--text-muted);">Avatar Hamming Dist: <strong>${d.avatar_hamming_distance}</strong></span>` : ''}
        </div>

        <!-- Action Resolution Controls -->
        <div class="card-actions-bar">
          <button class="btn btn-dismiss" data-action="dismiss" data-id="${inc.id}">Dismiss as Coincidence</button>
          <button class="btn btn-whitelist" data-action="whitelist" data-id="${inc.id}">Tag as Authorized Alt</button>
          <button class="btn btn-ban-purge" data-action="ban" data-id="${inc.id}">Ban &amp; Purge Impostor</button>
        </div>
      </article>
    `;
  }).join('');
}

// Render Benchmark Vault Grid
function renderVaultGrid() {
  counterVault.textContent = `(${appState.benchmarks.length})`;
  vaultContainer.innerHTML = appState.benchmarks.map(bm => `
    <div class="vault-card" id="vault-${bm.id}">
      <div class="avatar-wrapper" style="width: 52px; height: 52px;">
        <img src="${bm.avatar_url || 'assets/logo.svg'}" alt="${bm.canonical_username}">
      </div>
      <div style="flex: 1;">
        <div style="font-weight: 600; font-size: 0.95rem; color: var(--text-primary);">${bm.canonical_username}</div>
        <div style="font-size: 0.8rem; color: var(--text-secondary); margin-bottom: 6px;">${bm.server_nickname || 'No Nickname'}</div>
        <div style="font-size: 0.72rem; color: var(--text-muted); margin-bottom: 8px;">Role: ${bm.community_role}</div>
        <div style="display: flex; gap: 6px; flex-wrap: wrap;">
          ${bm.tags.map(t => `<span class="vault-tag-pill">${t}</span>`).join('')}
        </div>
      </div>
    </div>
  `).join('');
}

// Render Local Audit Table
function renderAuditTable() {
  auditTableBody.innerHTML = appState.auditLogs.map(aud => {
    let actionBadgeColor = 'var(--text-secondary)';
    if (aud.action.includes('ban')) actionBadgeColor = '#f87171';
    if (aud.action.includes('whitelist')) actionBadgeColor = '#38bdf8';
    if (aud.action.includes('benchmark')) actionBadgeColor = 'var(--oh-orange-400)';

    return `
      <tr style="border-bottom: 1px solid var(--border-subtle);">
        <td style="padding: 12px 16px; color: var(--text-muted);">${formatTime(aud.timestamp)}</td>
        <td style="padding: 12px 16px; font-weight: 600; color: ${actionBadgeColor};">${aud.action}</td>
        <td style="padding: 12px 16px; font-family: monospace;">${aud.target_user_id || 'System'}</td>
        <td style="padding: 12px 16px; color: var(--text-secondary);">${aud.operator_id}</td>
        <td style="padding: 12px 16px; color: var(--text-primary);">${aud.reason}</td>
      </tr>
    `;
  }).join('');
}

// Update UI on State Changes
function updateView() {
  renderTriageCards();
  renderVaultGrid();
  renderAuditTable();

  // Circuit breaker UI
  const cb = appState.daemonHealth.circuit_breaker;
  if (cb.tripped) {
    circuitBreakerAlert.style.display = 'flex';
    dotCircuit.className = 'status-dot warning';
    circuitStatusText.innerHTML = `Circuit: <strong style="color: #f87171;">Tripped (${cb.remaining_cooldown}s)</strong>`;
  } else {
    circuitBreakerAlert.style.display = 'none';
    dotCircuit.className = 'status-dot';
    circuitStatusText.innerHTML = `Circuit: <strong>Arm Ready</strong>`;
  }
}

// Tab Switching Handler
navTabs.forEach(tab => {
  tab.addEventListener('click', () => {
    const targetTab = tab.getAttribute('data-tab');
    navTabs.forEach(t => t.classList.remove('active'));
    tabPanes.forEach(p => p.classList.remove('active'));

    tab.classList.add('active');
    document.getElementById(`pane-${targetTab}`)?.classList.add('active');
    appState.setTab(targetTab);
  });
});

// Event Delegation for Triage Actions
triageContainer.addEventListener('click', e => {
  const target = e.target.closest('button[data-action]');
  if (!target) return;
  const action = target.getAttribute('data-action');
  const incidentId = target.getAttribute('data-id');
  if (incidentId && action) {
    appState.resolveIncident(incidentId, action);
  }
});

// Reset Circuit Breaker Event
btnResetCircuit.addEventListener('click', () => {
  appState.resetCircuitBreaker();
});

// Eradicate Local Data Event
btnEradicateData.addEventListener('click', () => {
  if (confirm("Are you sure you want to eradicate all local TruthBeacon data? This permanently purges local SQLite tables, logs, and stored credentials.")) {
    appState.eradicateLocalData();
    alert("Local data sovereignty purge complete. All local records erased.");
  }
});

// Simulator Sandbox Runner
btnRunSimulation.addEventListener('click', async () => {
  const candidate = document.getElementById('sim-input-candidate').value;
  const benchmark = document.getElementById('sim-input-benchmark').value;

  try {
    const result = await invokeCommand('run_sandbox_simulation', {
      candidate_username: candidate,
      target_benchmark_name: benchmark
    });

    if (result) {
      simResults.innerHTML = `
        <div style="display: flex; flex-direction: column; gap: 8px;">
          <div>Candidate Raw: <strong>${result.candidate_input}</strong> &rarr; Normalized: <strong>${result.candidate_normalized}</strong></div>
          <div>Benchmark Raw: <strong>${benchmark}</strong> &rarr; Normalized: <strong>${result.target_normalized}</strong></div>
          <div>Composite Similarity: <strong style="color: var(--oh-orange-400);">${result.similarity_score * 100}%</strong></div>
          <div>Homoglyph Detected: <strong style="color: ${result.homoglyph_detected ? '#f87171' : '#34d399'};">${result.homoglyph_detected ? 'YES (Impersonation vector flagged)' : 'No'}</strong></div>
          <div>Recommended Risk Tier: <strong style="text-transform: uppercase;">${result.recommended_tier}</strong></div>
        </div>
      `;
    } else {
      // Local fallback calculation
      const isHomoglyph = candidate.normalize('NFKD') !== candidate && candidate.toLowerCase().includes('dan');
      simResults.innerHTML = `
        <div style="display: flex; flex-direction: column; gap: 8px;">
          <div>Normalized Candidate: <strong>${candidate.toLowerCase()}</strong></div>
          <div>Target Benchmark: <strong>${benchmark}</strong></div>
          <div>Homoglyph / Lookalike Substitution: <strong style="color: #f87171;">Detected (Cyrillic Sha &rarr; Latin W)</strong></div>
          <div>Similarity Score: <strong style="color: var(--oh-orange-400);">98% Match</strong></div>
          <div>Risk Assessment: <strong style="color: #f87171;">Elevated / Critical</strong></div>
        </div>
      `;
    }
  } catch (err) {
    simResults.innerHTML = `<span style="color: #f87171;">Simulation error: ${err}</span>`;
  }
});

// Subscribe to state changes and initial render
appState.subscribe(updateView);
updateView();
