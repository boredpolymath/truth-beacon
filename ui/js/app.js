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
const dotCircuit = document.getElementById('dot-circuit');
const circuitStatusText = document.getElementById('circuit-status-text');
const gatewayLatency = document.getElementById('gateway-latency');

// Modals & Controls
const modalCreateBm = document.getElementById('modal-create-benchmark');
const btnAddBenchmark = document.getElementById('btn-add-benchmark');
const btnCloseBmModal = document.getElementById('btn-close-bm-modal');
const btnCancelBm = document.getElementById('btn-cancel-benchmark');
const btnSaveBm = document.getElementById('btn-save-benchmark');

const modalTagAlt = document.getElementById('modal-tag-alt');
const btnCloseAltModal = document.getElementById('btn-close-alt-modal');
const btnCancelAlt = document.getElementById('btn-cancel-alt');
const btnConfirmAlt = document.getElementById('btn-confirm-alt');
const altCandidateSummary = document.getElementById('alt-candidate-summary');
const altSelectBenchmark = document.getElementById('alt-select-benchmark');
const altInputPurpose = document.getElementById('alt-input-purpose');
const altInputJustification = document.getElementById('alt-input-justification');

const modalEradicateData = document.getElementById('modal-eradicate-data');
const btnEradicateData = document.getElementById('btn-eradicate-data');
const btnCloseEradicateModal = document.getElementById('btn-close-eradicate-modal');
const btnCancelEradicate = document.getElementById('btn-cancel-eradicate');
const btnConfirmEradicate = document.getElementById('btn-confirm-eradicate');
const eradicateConfirmInput = document.getElementById('eradicate-confirm-input');

const btnRunSimulation = document.getElementById('btn-run-simulation');
const simResults = document.getElementById('sim-results');

let activeAltIncidentId = null;

// Format relative time helper
function formatTime(timestamp) {
  const diffSecs = Math.floor((Date.now() - timestamp) / 1000);
  if (diffSecs < 60) return `${diffSecs}s ago`;
  const diffMins = Math.floor(diffSecs / 60);
  if (diffMins < 60) return `${diffMins}m ago`;
  const diffHours = Math.floor(diffMins / 60);
  return `${diffHours}h ago`;
}

// Modal open/close helpers
function openModal(modal) {
  modal.classList.add('open');
  document.body.style.overflow = 'hidden';
}

function closeModal(modal) {
  modal.classList.remove('open');
  document.body.style.overflow = '';
}

function closeAllModals() {
  document.querySelectorAll('.modal-overlay.open').forEach(m => closeModal(m));
}

// Render Side-by-Side Comparative Inspection Cards (7.2 Specification)
function renderTriageCards() {
  const pendingIncidents = appState.incidents.filter(i => i.status === 'pending');
  counterIncidents.textContent = pendingIncidents.length;
  if (pendingIncidents.length === 0) {
    counterIncidents.classList.add('tab-counter-zero');
  } else {
    counterIncidents.classList.remove('tab-counter-zero');
  }

  if (pendingIncidents.length === 0) {
    triageContainer.innerHTML = `
      <div style="text-align: center; padding: 64px 24px; background: var(--bg-card-glass); border-radius: var(--radius-lg); border: 1px dashed var(--border-subtle); box-shadow: var(--shadow-card);">
        <div style="width: 52px; height: 52px; margin: 0 auto 16px; border-radius: 50%; background: var(--status-healthy-bg); border: 1px solid var(--status-healthy-border); display: flex; align-items: center; justify-content: center; color: var(--status-healthy);">
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="20 6 9 17 4 12"/>
          </svg>
        </div>
        <h3 style="color: #34d399; font-weight: 700; font-size: 1.2rem; margin-bottom: 6px;">Ground Truth Secure</h3>
        <p style="color: var(--text-secondary); font-size: 0.88rem; max-width: 480px; margin: 0 auto;">
          Zero unresolved identity discrepancies detected across active platform gateway streams. All community leadership baselines remain authentic.
        </p>
      </div>
    `;
    return;
  }

  triageContainer.innerHTML = pendingIncidents.map((inc, index) => {
    const d = inc.discrepancy;
    const benchmark = appState.benchmarks.find(b => b.id === d.matched_benchmark_id) || {
      canonical_username: d.matched_benchmark_name,
      server_nickname: "Official Verified Identity",
      community_role: "Protected Benchmark",
      avatar_url: d.suspect_avatar_url,
      avatar_perceptual_hash: "a8f3b4c9e1d2f071",
      user_id: "291039401928374829"
    };

    const isCritical = d.risk_tier === 'critical';
    const riskChipClass = isCritical ? 'risk-critical' : 'risk-elevated';
    const podSuspectClass = isCritical ? 'suspect' : 'suspect elevated';

    // Highlight homoglyph character in suspect username if detected
    let renderedSuspectName = d.suspect_username;
    if (d.homoglyph_detected && d.homoglyph_char) {
      renderedSuspectName = d.suspect_username.replace(
        d.homoglyph_char,
        `<mark class="homoglyph-mark" title="Deobfuscated Cyrillic/Greek homoglyph">${d.homoglyph_char}</mark>`
      );
    } else if (d.suspect_username.endsWith('_')) {
      renderedSuspectName = `${d.suspect_username.slice(0, -1)}<mark class="homoglyph-mark" title="Trailing underscore typosquat">_</mark>`;
    }

    const similarityPct = Math.round(d.string_similarity_score * 100);
    const pHashDist = d.avatar_hamming_distance !== null ? d.avatar_hamming_distance : 0;
    const isRapidJoin = d.suspect_account_age_hours < 24;

    return `
      <article class="inspection-card" id="card-${inc.id}" data-incident-id="${inc.id}">
        <!-- Card Header Bar -->
        <div class="card-header-bar">
          <div style="display: flex; align-items: center; gap: 12px;">
            <span class="risk-chip ${riskChipClass}">
              <span class="risk-icon-dot"></span>
              ${d.risk_tier} Ambiguity
            </span>
            <span class="incident-time">Detected ${formatTime(inc.timestamp)}</span>
            ${index === 0 ? `<span style="font-size: 0.7rem; background: rgba(249, 115, 22, 0.15); color: var(--oh-orange-400); padding: 2px 7px; border-radius: var(--radius-sm); border: 1px solid rgba(249, 115, 22, 0.3); font-weight: 600;">Active Hotkey Target [#1]</span>` : ''}
          </div>
          <div class="pipeline-eval-tag">
            ${d.homoglyph_detected ? 'Unicode Compatibility NFKD Deobfuscated' : 'String Metric / Levenshtein Variance'}
          </div>
        </div>

        <!-- 3-Column Comparative Inspection Container (7.2 Specification) -->
        <div class="comparison-container">
          <!-- LEFT: Canonical Benchmark Pod -->
          <div class="profile-pod benchmark">
            <div class="avatar-wrapper">
              <img src="${benchmark.avatar_url || 'assets/logo.svg'}" alt="Authentic Benchmark Avatar">
            </div>
            <div class="profile-details">
              <div class="pod-role-label">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>
                Authentic Benchmark
              </div>
              <div class="profile-name">${benchmark.canonical_username}</div>
              <div class="profile-nick">${benchmark.server_nickname || 'Official Identity'}</div>
              <div class="profile-meta">
                <span class="meta-chip">${benchmark.community_role}</span>
                <span class="meta-chip" title="Snowflake User ID">ID: ${benchmark.user_id.slice(-6)}</span>
                <span class="meta-chip" title="64-bit DCT Perceptual Hash">pHash: ${benchmark.avatar_perceptual_hash ? benchmark.avatar_perceptual_hash.slice(0, 8) : 'N/A'}</span>
              </div>
            </div>
          </div>

          <!-- CENTER: Metric Comparison Pillar -->
          <div class="metric-vs-pillar">
            <div class="metric-circle">
              ${similarityPct}%
              <span>MATCH</span>
            </div>
            <span class="pillar-vs-badge">VS</span>
            <div class="pillar-breakdown">
              <div>Sim: <strong>${similarityPct}%</strong></div>
              <div>pHash: <strong>&Delta;${pHashDist}</strong></div>
            </div>
          </div>

          <!-- RIGHT: Suspect Candidate Pod -->
          <div class="profile-pod ${podSuspectClass}">
            <div class="avatar-wrapper">
              <img src="${d.suspect_avatar_url || 'assets/logo.svg'}" alt="Suspect Candidate Avatar">
            </div>
            <div class="profile-details">
              <div class="pod-role-label">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
                Incoming Suspect Candidate
              </div>
              <div class="profile-name">${renderedSuspectName}</div>
              <div class="profile-nick">${d.suspect_nickname || 'No Server Nickname'}</div>
              <div class="profile-meta">
                <span class="meta-chip ${isRapidJoin ? 'meta-chip-rapid' : ''}">
                  Account Age: <strong>${d.suspect_account_age_hours}h old</strong> ${isRapidJoin ? '(Rapid Join)' : ''}
                </span>
                <span class="meta-chip" title="Snowflake User ID">ID: ${d.suspect_user_id.slice(-6)}</span>
              </div>
            </div>
          </div>
        </div>

        <!-- Discrepancy Breakdown Row -->
        <div class="vs-diff-summary">
          <span class="diff-badge">EVALUATION ANALYSIS</span>
          <span class="diff-content">${d.normalized_diff}</span>
          ${d.avatar_hamming_distance !== null ? `
            <span class="diff-phash-stat">
              Avatar Hamming Dist: <strong>${d.avatar_hamming_distance}</strong> (${d.avatar_hamming_distance <= 5 ? 'Visual Clone Flagged' : 'Distinct Avatar'})
            </span>
          ` : ''}
        </div>

        <!-- Action Resolution Bar (Kinship Stewardship & Ergonomic Hotkeys) -->
        <div class="card-actions-bar">
          <div style="font-size: 0.74rem; color: var(--text-muted);">
            Operator Adjudication Required &bull; Zero Cloud Egress
          </div>
          <div class="action-buttons-group">
            <button class="btn btn-dismiss" data-action="dismiss" data-id="${inc.id}" title="Dismiss as harmless coincidental naming overlap">
              <kbd>D</kbd> Dismiss Coincidence
            </button>
            <button class="btn btn-whitelist" data-action="open-alt-modal" data-id="${inc.id}" title="Authorize this account as an official secondary staff account or device">
              <kbd>W</kbd> Authorize Alt Identity...
            </button>
            <button class="btn btn-exclude" data-action="exclude" data-id="${inc.id}" title="Apply temporary quarantine role pending identity verification">
              <kbd>E</kbd> Quarantine Member
            </button>
            <button class="btn btn-adjudicate" data-action="ban" data-id="${inc.id}" title="Adjudicate as malicious impostor and purge fraudulent messages">
              <kbd>B</kbd> Adjudicate Impersonation
            </button>
          </div>
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
      <div class="avatar-wrapper" style="width: 54px; height: 54px; border-color: rgba(249, 115, 22, 0.4);">
        <img src="${bm.avatar_url || 'assets/logo.svg'}" alt="${bm.canonical_username}">
      </div>
      <div style="flex: 1; min-width: 0;">
        <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 2px;">
          <div style="font-weight: 700; font-size: 1rem; color: var(--text-primary);">${bm.canonical_username}</div>
          <span style="font-size: 0.65rem; color: var(--status-healthy); background: var(--status-healthy-bg); padding: 1px 6px; border-radius: var(--radius-xs); border: 1px solid var(--status-healthy-border); font-weight: 600;">ACTIVE</span>
        </div>
        <div style="font-size: 0.82rem; color: var(--text-secondary); margin-bottom: 4px;">${bm.server_nickname || 'Official Identity'}</div>
        <div style="font-size: 0.74rem; color: var(--text-muted); margin-bottom: 8px;">Role: <strong style="color: var(--text-primary);">${bm.community_role}</strong></div>

        <div style="display: flex; gap: 6px; flex-wrap: wrap; margin-bottom: 8px;">
          ${(bm.tags || []).map(t => `<span class="vault-tag-pill">${t}</span>`).join('')}
        </div>

        ${bm.authorized_alts && bm.authorized_alts.length > 0 ? `
          <div style="border-top: 1px solid var(--border-subtle); padding-top: 6px; margin-top: 6px;">
            <div style="font-size: 0.68rem; color: var(--text-muted); text-transform: uppercase; margin-bottom: 4px; font-weight: 700;">Authorized Alternates:</div>
            <div style="display: flex; gap: 6px; flex-wrap: wrap;">
              ${bm.authorized_alts.map(alt => `<span class="vault-alt-tag" title="${alt.note}">Alt: ${alt.label} (${alt.user_id.slice(-4)})</span>`).join('')}
            </div>
          </div>
        ` : ''}

        <div style="font-family: var(--font-mono); font-size: 0.68rem; color: var(--text-muted); margin-top: 8px;">
          Snowflake: ${bm.user_id} &bull; pHash: ${bm.avatar_perceptual_hash ? bm.avatar_perceptual_hash.slice(0, 12) + '...' : 'none'}
        </div>
      </div>
    </div>
  `).join('');
}

// Render Local Audit Table
function renderAuditTable() {
  auditTableBody.innerHTML = appState.auditLogs.map(aud => {
    let actionBadgeColor = 'var(--text-secondary)';
    let actionBadgeText = aud.action;

    if (aud.action.includes('adjudicate') || aud.action.includes('ban')) {
      actionBadgeColor = '#fda4af';
      actionBadgeText = 'Adjudicate Impersonation';
    } else if (aud.action.includes('authorize') || aud.action.includes('whitelist')) {
      actionBadgeColor = '#7dd3fc';
      actionBadgeText = 'Authorize Alternate';
    } else if (aud.action.includes('quarantine') || aud.action.includes('exclude')) {
      actionBadgeColor = '#d8b4fe';
      actionBadgeText = 'Quarantine User';
    } else if (aud.action.includes('dismiss')) {
      actionBadgeColor = 'var(--text-secondary)';
      actionBadgeText = 'Dismiss Coincidence';
    } else if (aud.action.includes('benchmark')) {
      actionBadgeColor = 'var(--oh-orange-400)';
      actionBadgeText = 'Canonical Benchmark Enrolled';
    }

    return `
      <tr style="border-bottom: 1px solid var(--border-subtle);">
        <td style="padding: 12px 18px; color: var(--text-muted);" class="tabular-nums">${formatTime(aud.timestamp)}</td>
        <td style="padding: 12px 18px; font-weight: 600; color: ${actionBadgeColor};">${actionBadgeText}</td>
        <td style="padding: 12px 18px; font-family: var(--font-mono); color: var(--text-secondary);">${aud.target_user_id || 'System'}</td>
        <td style="padding: 12px 18px; color: var(--text-primary); font-weight: 500;">${aud.operator_id}</td>
        <td style="padding: 12px 18px; color: var(--text-secondary);">${aud.reason}</td>
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
    circuitStatusText.innerHTML = `Circuit: <strong style="color: #fda4af;">Escrow Active (${cb.remaining_cooldown}s)</strong>`;
  } else {
    circuitBreakerAlert.style.display = 'none';
    dotCircuit.className = 'status-dot';
    circuitStatusText.innerHTML = `Circuit: <strong>Armed (Nominal)</strong>`;
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

  if (action === 'open-alt-modal') {
    openAuthorizedAltModal(incidentId);
  } else if (incidentId && action) {
    appState.resolveIncident(incidentId, action);
  }
});

// Reset Circuit Breaker Event
btnResetCircuit.addEventListener('click', () => {
  appState.resetCircuitBreaker();
});

// MODAL WORKFLOW 1: Benchmark Creation (7.2 Specification)
btnAddBenchmark.addEventListener('click', () => {
  document.getElementById('bm-input-snowflake').value = '';
  document.getElementById('bm-input-username').value = '';
  document.getElementById('bm-input-nickname').value = '';
  document.getElementById('bm-input-avatar').value = '';
  openModal(modalCreateBm);
});

btnCloseBmModal.addEventListener('click', () => closeModal(modalCreateBm));
btnCancelBm.addEventListener('click', () => closeModal(modalCreateBm));

btnSaveBm.addEventListener('click', () => {
  const snowflake = document.getElementById('bm-input-snowflake').value.trim();
  const username = document.getElementById('bm-input-username').value.trim();
  const nickname = document.getElementById('bm-input-nickname').value.trim();
  const role = document.getElementById('bm-input-role').value;
  const avatarUrl = document.getElementById('bm-input-avatar').value.trim();
  const tagsStr = document.getElementById('bm-input-tags').value.trim();
  const tags = tagsStr ? tagsStr.split(',').map(s => s.trim()).filter(Boolean) : ['Verified VIP'];

  if (!snowflake || !/^\d{17,20}$/.test(snowflake)) {
    alert("Please provide a valid 17-20 digit Discord Snowflake User ID.");
    return;
  }
  if (!username) {
    alert("Please provide a canonical username.");
    return;
  }

  appState.addBenchmark({
    user_id: snowflake,
    canonical_username: username,
    server_nickname: nickname || null,
    community_role: role,
    avatar_url: avatarUrl || 'assets/logo.svg',
    tags: tags
  });

  closeModal(modalCreateBm);
});

// MODAL WORKFLOW 2: Authorized Alt Tagging (7.2 Specification)
function openAuthorizedAltModal(incidentId) {
  const inc = appState.incidents.find(i => i.id === incidentId);
  if (!inc) return;

  activeAltIncidentId = incidentId;
  const d = inc.discrepancy;

  altCandidateSummary.innerHTML = `
    <div style="font-weight: 700; color: var(--text-primary); margin-bottom: 2px;">Candidate: ${d.suspect_username}</div>
    <div style="font-size: 0.75rem; color: var(--text-muted); font-family: var(--font-mono);">Snowflake ID: ${d.suspect_user_id} &bull; Nickname: ${d.suspect_nickname || 'None'}</div>
  `;

  altSelectBenchmark.innerHTML = appState.benchmarks.map(b => `
    <option value="${b.id}" ${b.id === d.matched_benchmark_id ? 'selected' : ''}>
      ${b.canonical_username} (${b.community_role})
    </option>
  `).join('');

  altInputJustification.value = `Operator verified secondary account for ${d.matched_benchmark_name}`;
  openModal(modalTagAlt);
}

btnCloseAltModal.addEventListener('click', () => closeModal(modalTagAlt));
btnCancelAlt.addEventListener('click', () => closeModal(modalTagAlt));

btnConfirmAlt.addEventListener('click', () => {
  if (!activeAltIncidentId) return;
  const benchmarkId = altSelectBenchmark.value;
  const purpose = altInputPurpose.value;
  const justification = altInputJustification.value.trim() || "Operator verified alternate identity";

  appState.resolveIncident(activeAltIncidentId, 'whitelist', justification, {
    targetBenchmarkId: benchmarkId,
    label: purpose,
    note: justification
  });

  closeModal(modalTagAlt);
  activeAltIncidentId = null;
});

// MODAL WORKFLOW 3: Sovereign Local Data Eradication (7.2 Specification)
btnEradicateData.addEventListener('click', () => {
  eradicateConfirmInput.value = '';
  btnConfirmEradicate.disabled = true;
  openModal(modalEradicateData);
});

btnCloseEradicateModal.addEventListener('click', () => closeModal(modalEradicateData));
btnCancelEradicate.addEventListener('click', () => closeModal(modalEradicateData));

eradicateConfirmInput.addEventListener('input', () => {
  const matches = eradicateConfirmInput.value.trim() === 'ERADICATE';
  btnConfirmEradicate.disabled = !matches;
});

btnConfirmEradicate.addEventListener('click', () => {
  if (eradicateConfirmInput.value.trim() !== 'ERADICATE') return;
  appState.eradicateLocalData();
  closeModal(modalEradicateData);
});

// Close modals when clicking backdrop
document.querySelectorAll('.modal-overlay').forEach(overlay => {
  overlay.addEventListener('click', e => {
    if (e.target === overlay) closeModal(overlay);
  });
});

// Keyboard Accessibility & Hotkey Adjudication Loop (AC-4.3 & DoD Gate 2)
window.addEventListener('keydown', e => {
  // Always allow Escape to close open modals
  if (e.key === 'Escape') {
    closeAllModals();
    return;
  }

  // Do not trigger single-key hotkeys when typing in form controls
  const activeTagName = document.activeElement ? document.activeElement.tagName.toLowerCase() : '';
  if (activeTagName === 'input' || activeTagName === 'textarea' || activeTagName === 'select') {
    return;
  }

  // If any modal is open, prevent hotkeys from executing background triage
  if (document.querySelector('.modal-overlay.open')) {
    return;
  }

  // Triage single-key hotkeys targeting the first pending incident
  const pendingIncidents = appState.incidents.filter(i => i.status === 'pending');
  const firstIncident = pendingIncidents[0];

  if (firstIncident) {
    if (e.key === 'b' || e.key === 'B') {
      e.preventDefault();
      appState.resolveIncident(firstIncident.id, 'ban');
    } else if (e.key === 'd' || e.key === 'D') {
      e.preventDefault();
      appState.resolveIncident(firstIncident.id, 'dismiss');
    } else if (e.key === 'w' || e.key === 'W') {
      e.preventDefault();
      openAuthorizedAltModal(firstIncident.id);
    } else if (e.key === 'e' || e.key === 'E') {
      e.preventDefault();
      appState.resolveIncident(firstIncident.id, 'exclude');
    }
  }

  // Number keys 1-5 switch tabs
  if (['1', '2', '3', '4', '5'].includes(e.key)) {
    const tabs = ['triage', 'vault', 'sandbox', 'audit', 'sovereignty'];
    const idx = parseInt(e.key, 10) - 1;
    if (tabs[idx]) {
      const targetTab = tabs[idx];
      navTabs.forEach(t => t.classList.remove('active'));
      tabPanes.forEach(p => p.classList.remove('active'));
      document.querySelector(`[data-tab="${targetTab}"]`)?.classList.add('active');
      document.getElementById(`pane-${targetTab}`)?.classList.add('active');
      appState.setTab(targetTab);
    }
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
          <div>Candidate Raw: <strong>${result.candidate_input}</strong> &rarr; Normalized: <code style="font-family: var(--font-mono); color: var(--oh-orange-300);">${result.candidate_normalized}</code></div>
          <div>Benchmark Raw: <strong>${benchmark}</strong> &rarr; Normalized: <code style="font-family: var(--font-mono); color: var(--oh-orange-300);">${result.target_normalized}</code></div>
          <div>Composite Similarity Score: <strong style="color: var(--oh-orange-400);">${Math.round(result.similarity_score * 100)}%</strong> (Jaro-Winkler 70% + Damerau 30%)</div>
          <div>Homoglyph Substitution Flagged: <strong style="color: ${result.homoglyph_detected ? '#fda4af' : '#34d399'};">${result.homoglyph_detected ? 'YES (Cyrillic Sha &rarr; Latin W substitution)' : 'No Homoglyphs Detected'}</strong></div>
          <div>Recommended Ground-Truth Risk Tier: <strong style="text-transform: uppercase; color: ${result.recommended_tier === 'critical' ? '#fda4af' : 'var(--oh-orange-400)'};">${result.recommended_tier}</strong></div>
        </div>
      `;
    }
  } catch (err) {
    simResults.innerHTML = `<span style="color: #fda4af;">Simulation error: ${err}</span>`;
  }
});

// Subscribe to state changes and initial render
appState.subscribe(updateView);
updateView();
