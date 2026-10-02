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

let activeAltIncidentId = null;

// Format relative time helper in simple plain language
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
  if (!modal) return;
  modal.classList.add('open');
  document.body.style.overflow = 'hidden';
}

function closeModal(modal) {
  if (!modal) return;
  modal.classList.remove('open');
  document.body.style.overflow = '';
}

function closeAllModals() {
  document.querySelectorAll('.modal-overlay.open').forEach(m => closeModal(m));
}

// Render Alerts Queue (Simplified, non-technical, high-clarity)
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
      <div style="text-align: center; padding: 56px 20px; background: var(--bg-card); border-radius: var(--radius-md); border: 1px solid var(--border-subtle); box-shadow: var(--shadow-card);">
        <div style="width: 48px; height: 48px; margin: 0 auto 14px; border-radius: 50%; background: var(--discord-green-subtle); border: 1px solid var(--discord-green-border); display: flex; align-items: center; justify-content: center; color: var(--discord-green);">
          <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="20 6 9 17 4 12"/>
          </svg>
        </div>
        <h3 style="color: var(--text-header); font-weight: 700; font-size: 1.15rem; margin-bottom: 6px;">All Clear! No Imposter Alerts</h3>
        <p style="color: var(--text-secondary); font-size: 0.85rem; max-width: 440px; margin: 0 auto;">
          Zero unresolved imposter alerts detected. All protected community leaders and staff are safe.
        </p>
      </div>
    `;
    return;
  }

  triageContainer.innerHTML = pendingIncidents.map((inc, index) => {
    const d = inc.discrepancy;
    const benchmark = appState.benchmarks.find(b => b.id === d.matched_benchmark_id) || {
      canonical_username: d.matched_benchmark_name,
      server_nickname: "Official Member",
      community_role: "Protected Leader",
      avatar_url: d.suspect_avatar_url,
      user_id: "291039401928374829"
    };

    const isCritical = d.risk_tier === 'critical';
    const riskChipClass = isCritical ? 'risk-critical' : 'risk-elevated';
    const riskText = isCritical ? 'High Risk Imposter' : 'Possible Lookalike';
    const podSuspectClass = isCritical ? 'suspect' : 'suspect elevated';

    // Highlight lookalike character in suspect username if detected
    let renderedSuspectName = d.suspect_username;
    if (d.homoglyph_detected && d.homoglyph_char) {
      renderedSuspectName = d.suspect_username.replace(
        d.homoglyph_char,
        `<mark class="homoglyph-mark" title="Lookalike letter substitution">${d.homoglyph_char}</mark>`
      );
    } else if (d.suspect_username.endsWith('_')) {
      renderedSuspectName = `${d.suspect_username.slice(0, -1)}<mark class="homoglyph-mark" title="Extra underscore added">_</mark>`;
    }

    const similarityPct = Math.round(d.string_similarity_score * 100);
    const isMatchingPhoto = d.avatar_hamming_distance !== null && d.avatar_hamming_distance <= 5;
    const isMatchingName = Boolean(d.homoglyph_detected || (d.string_similarity_score !== undefined && d.string_similarity_score >= 0.75) || !isMatchingPhoto);

    return `
      <article class="inspection-card ${isCritical ? '' : 'risk-elevated-card'}" id="card-${inc.id}" data-incident-id="${inc.id}">
        <!-- Card Header Bar with Risk Badge and Reason Pills -->
        <div class="card-header-bar">
          <div style="display: flex; align-items: center; gap: 12px; flex-wrap: wrap;">
            <span class="risk-chip ${riskChipClass}">
              <span class="risk-icon-dot"></span>
              ${riskText}
            </span>
            <span class="incident-time">Flagged ${formatTime(inc.timestamp)}</span>
          </div>
          <div class="flagged-reasons-group">
            ${isMatchingPhoto ? `<span class="reason-pill reason-photo" title="Matching Profile Photo (Hamming Distance: ${d.avatar_hamming_distance})">Photo</span>` : ''}
            ${isMatchingName ? `<span class="reason-pill reason-name" title="Matching Name (${similarityPct}% similarity${d.homoglyph_detected ? ' with lookalikes' : ''})">Name</span>` : ''}
          </div>
        </div>

        <!-- Clean, High-Readability Side-by-Side Comparison -->
        <div class="comparison-container">
          <!-- LEFT: Official Protected Member -->
          <div class="profile-side official-side">
            <div class="side-header">
              <span class="side-indicator-dot official"></span>
              <span>Official Protected Member</span>
            </div>
            <div class="profile-main-box">
              <div class="avatar-container">
                <img src="${benchmark.avatar_url || 'assets/logo.svg'}" alt="${benchmark.canonical_username}" onerror="this.onerror=null; this.src='assets/logo.svg'">
                <span class="status-verified-check" title="Verified Server Leader">✓</span>
              </div>
              <div class="profile-identity">
                <div class="profile-display-name" title="${benchmark.server_nickname || benchmark.canonical_username}">${benchmark.server_nickname || benchmark.canonical_username}</div>
                <div class="profile-handle-sub" title="Canonical Username: @${benchmark.canonical_username}">
                  <span class="handle-prefix">User:</span>
                  <span class="full-username-val">@${benchmark.canonical_username}</span>
                </div>
                <div class="profile-role-tag">
                  <span class="role-dot"></span>
                  ${benchmark.community_role}
                </div>
              </div>
            </div>
          </div>

          <!-- CENTER: Comparison Connector -->
          <div class="comparison-connector">
            <div class="match-score-badge">${similarityPct}% Match</div>
            <div class="match-vs-pill">VS</div>
          </div>

          <!-- RIGHT: Flagged New Account -->
          <div class="profile-side suspect-side ${isCritical ? '' : 'elevated'}">
            <div class="side-header">
              <span class="side-indicator-dot ${isCritical ? 'suspect' : 'suspect-elevated'}"></span>
              <span>Flagged New Account</span>
            </div>
            <div class="profile-main-box">
              <div class="avatar-container">
                <img src="${d.suspect_avatar_url || 'assets/logo.svg'}" alt="${d.suspect_username}" onerror="this.onerror=null; this.src='assets/logo.svg'">
                <span class="status-alert-mark ${isCritical ? '' : 'elevated'}" title="Flagged Imposter Account">!</span>
              </div>
              <div class="profile-identity">
                <div class="profile-display-name" title="${d.suspect_nickname || d.suspect_username}">${d.suspect_nickname || renderedSuspectName}</div>
                <div class="profile-handle-sub" title="Imposter Username: @${d.suspect_username}">
                  <span class="handle-prefix">User:</span>
                  <span class="full-username-val suspect">@${renderedSuspectName}</span>
                </div>
                <div class="profile-age-tag ${isCritical ? '' : 'elevated'}">
                  Joined ${d.suspect_account_age_hours}h ago
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Dedicated Full Username Threat Assessment Strip -->
        <div class="username-threat-bar" aria-label="Exact Username Threat Comparison">
          <div class="threat-user-box real-box" title="Full Real Canonical Username: @${benchmark.canonical_username}">
            <div class="threat-user-label">
              <span class="threat-label-dot real"></span>
              <span>Real Username</span>
            </div>
            <div class="threat-user-value">@${benchmark.canonical_username}</div>
          </div>

          <div class="threat-vs-divider">
            <span class="threat-vs-text">VS</span>
            <span class="threat-vs-metric">${similarityPct}% match</span>
          </div>

          <div class="threat-user-box imposter-box" title="Full Imposter Username: @${d.suspect_username}">
            <div class="threat-user-label">
              <span class="threat-label-dot imposter"></span>
              <span>Imposter Username</span>
            </div>
            <div class="threat-user-value imposter">@${renderedSuspectName}</div>
          </div>
        </div>



        <!-- Full-Spanning Action Resolution Bar -->
        <div class="card-actions-bar">
          <div class="action-buttons-group">
            <button class="btn btn-dismiss" data-action="dismiss" data-id="${inc.id}" title="Ignore this alert as harmless">
              <kbd>D</kbd> Ignore Alert (Safe)
            </button>
            <button class="btn btn-whitelist" data-action="open-alt-modal" data-id="${inc.id}" title="Allow this account as an approved alternate or secondary device">
              <kbd>W</kbd> Allow Known Alt...
            </button>
            <button class="btn btn-exclude" data-action="exclude" data-id="${inc.id}" title="Mute or restrict this account temporarily">
              <kbd>E</kbd> Restrict Account
            </button>
            <button class="btn btn-adjudicate" data-action="ban" data-id="${inc.id}" title="Ban this imposter from the server">
              <kbd>B</kbd> Ban Imposter
            </button>
          </div>
        </div>
      </article>
    `;
  }).join('');
}

// Render Protected Members Grid
function renderVaultGrid() {
  counterVault.textContent = `(${appState.benchmarks.length})`;
  vaultContainer.innerHTML = appState.benchmarks.map(bm => {
    const displayName = bm.server_nickname || bm.canonical_username;
    const hasDistinctHandle = bm.server_nickname && bm.server_nickname !== bm.canonical_username;
    
    return `
    <div class="vault-card" id="vault-${bm.id}">
      <div class="vault-card-banner"></div>
      
      <div class="vault-avatar-wrapper">
        <img src="${bm.avatar_url || 'assets/logo.svg'}" alt="${bm.canonical_username}" onerror="this.onerror=null; this.src='assets/logo.svg'">
        <span class="vault-status-dot online" title="Status: Online & Protected"></span>
      </div>

      <div class="vault-card-body">
        <div class="vault-header-row">
          <div class="vault-user-info">
            <div class="vault-display-name" title="${displayName}">${displayName}</div>
            <div class="vault-user-meta">
              <span class="vault-handle">@${bm.canonical_username.toLowerCase()}</span>
              ${hasDistinctHandle ? `<span class="vault-meta-divider">•</span><span class="vault-account-name">${bm.canonical_username}</span>` : ''}
            </div>
          </div>
          <span class="vault-protected-badge" title="Active Protected Identity">
            <svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor"><path d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4zm-2 16l-4-4 1.41-1.41L10 14.17l6.59-6.59L18 9l-8 8z"/></svg>
            PROTECTED
          </span>
        </div>

        <div class="vault-role-line">
          <span>Role:</span>
          <span class="vault-role-badge">
            <span class="role-circle" style="background-color: var(--brand-blurple);"></span>
            ${bm.community_role}
          </span>
        </div>

        <div class="vault-tags-row">
          ${(bm.tags || []).map(t => `<span class="vault-tag-pill">${t}</span>`).join('')}
        </div>

        ${bm.authorized_alts && bm.authorized_alts.length > 0 ? `
          <div class="vault-alts-section">
            <div class="vault-alts-label">Approved Secondary Accounts:</div>
            <div class="vault-tags-row" style="margin-bottom: 0;">
              ${bm.authorized_alts.map(alt => `<span class="vault-alt-tag" title="${alt.note}">Alt: ${alt.label} (${alt.user_id.slice(-4)})</span>`).join('')}
            </div>
          </div>
        ` : ''}

        <div class="vault-shield-status">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
          <span>Active Impersonation Shield</span>
        </div>
      </div>
    </div>
    `;
  }).join('');
}

// Render Activity Log Table
function renderAuditTable() {
  auditTableBody.innerHTML = appState.auditLogs.map(aud => {
    let actionBadgeColor = 'var(--text-secondary)';
    let actionBadgeBg = 'var(--bg-secondary)';
    let actionBadgeText = aud.action;

    if (aud.action.includes('adjudicate') || aud.action.includes('ban')) {
      actionBadgeColor = '#ffa1a4';
      actionBadgeBg = 'var(--discord-red-subtle)';
      actionBadgeText = 'Banned Imposter';
    } else if (aud.action.includes('authorize') || aud.action.includes('whitelist')) {
      actionBadgeColor = '#c7d2fe';
      actionBadgeBg = 'var(--brand-blurple-subtle)';
      actionBadgeText = 'Allowed Known Alt';
    } else if (aud.action.includes('quarantine') || aud.action.includes('exclude')) {
      actionBadgeColor = '#e9d5ff';
      actionBadgeBg = 'var(--discord-purple-subtle)';
      actionBadgeText = 'Restricted Account';
    } else if (aud.action.includes('dismiss')) {
      actionBadgeColor = 'var(--text-secondary)';
      actionBadgeBg = 'rgba(255, 255, 255, 0.05)';
      actionBadgeText = 'Ignored Alert';
    } else if (aud.action.includes('benchmark')) {
      actionBadgeColor = 'var(--oh-orange-400)';
      actionBadgeBg = 'var(--oh-orange-subtle)';
      actionBadgeText = 'Protected Leader Added';
    }

    return `
      <tr style="border-bottom: 1px solid var(--border-subtle);">
        <td style="padding: 14px 20px; color: var(--text-muted); font-size: 0.82rem;" class="tabular-nums">${formatTime(aud.timestamp)}</td>
        <td style="padding: 14px 20px;">
          <span style="font-weight: 700; font-size: 0.78rem; color: ${actionBadgeColor}; background: ${actionBadgeBg}; padding: 4px 10px; border-radius: var(--radius-xs); border: 1px solid rgba(255, 255, 255, 0.08); display: inline-block;">
            ${actionBadgeText}
          </span>
        </td>
        <td style="padding: 14px 20px; font-size: 0.84rem; color: var(--text-secondary);">${aud.target_user_id || 'System'}</td>
        <td style="padding: 14px 20px; color: var(--text-header); font-weight: 600; font-size: 0.86rem;">${aud.operator_id}</td>
        <td style="padding: 14px 20px; color: var(--text-primary); font-size: 0.84rem; line-height: 1.4;">${aud.reason}</td>
      </tr>
    `;
  }).join('');
}

// Update UI on State Changes
function updateView() {
  renderTriageCards();
  renderVaultGrid();
  renderAuditTable();

  // Safety circuit breaker UI
  const cb = appState.daemonHealth.circuit_breaker;
  if (cb.tripped) {
    circuitBreakerAlert.style.display = 'flex';
    dotCircuit.className = 'status-dot warning';
    circuitStatusText.innerHTML = `Protection: <strong style="color: #ffa1a4;">Safety Pause (${cb.remaining_cooldown}s)</strong>`;
  } else {
    circuitBreakerAlert.style.display = 'none';
    dotCircuit.className = 'status-dot online';
    circuitStatusText.innerHTML = `Protection: <strong>Active</strong>`;
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
  const btn = e.target.closest('button[data-action]');
  if (!btn) return;

  const action = btn.getAttribute('data-action');
  const incidentId = btn.getAttribute('data-id');

  if (action === 'open-alt-modal') {
    openAuthorizedAltModal(incidentId);
  } else if (action === 'dismiss') {
    appState.resolveIncident(incidentId, 'dismiss');
  } else if (action === 'ban') {
    appState.resolveIncident(incidentId, 'ban');
  } else if (action === 'exclude') {
    appState.resolveIncident(incidentId, 'exclude');
  }
});

// Circuit Breaker Reset Button
btnResetCircuit.addEventListener('click', () => {
  appState.resetCircuitBreaker();
});

// Add Protected Member Modal Workflows
btnAddBenchmark.addEventListener('click', () => {
  document.getElementById('bm-input-snowflake').value = '';
  document.getElementById('bm-input-username').value = '';
  document.getElementById('bm-input-nickname').value = '';
  document.getElementById('bm-input-avatar').value = '';
  openModal(modalCreateBm);
});

btnCloseBmModal.addEventListener('click', () => closeModal(modalCreateBm));
btnCancelBm.addEventListener('click', () => closeModal(modalCreateBm));

btnSaveBm.addEventListener('click', async () => {
  const snowflake = document.getElementById('bm-input-snowflake').value.trim();
  const username = document.getElementById('bm-input-username').value.trim();
  const nickname = document.getElementById('bm-input-nickname').value.trim();
  const role = document.getElementById('bm-input-role').value;
  const avatarUrl = document.getElementById('bm-input-avatar').value.trim();
  const tagsStr = document.getElementById('bm-input-tags').value.trim();

  if (!snowflake) {
    alert("Please enter a Discord User ID.");
    return;
  }
  if (!username) {
    alert("Please enter a username.");
    return;
  }

  const tags = tagsStr ? tagsStr.split(',').map(t => t.trim()).filter(Boolean) : [];

  const newBm = {
    guild_id: appState.selectedGuild.id,
    user_id: snowflake,
    canonical_username: username,
    server_nickname: nickname || null,
    community_role: role,
    avatar_url: avatarUrl || null,
    tags
  };

  try {
    await appState.addBenchmark(newBm);
    closeModal(modalCreateBm);
  } catch (err) {
    alert(`Could not save protected member: ${err.message || err}`);
  }
});

// Authorize Alternate Account Modal
function openAuthorizedAltModal(incidentId) {
  activeAltIncidentId = incidentId;
  const incident = appState.incidents.find(i => i.id === incidentId);
  if (!incident) return;

  const d = incident.discrepancy;

  altCandidateSummary.innerHTML = `
    <div>Reviewing candidate: <strong>${d.suspect_username}</strong> (${d.suspect_nickname || 'No Display Name'})</div>
    <div style="font-family: var(--font-mono); font-size: 0.72rem; color: var(--text-muted); margin-top: 3px;">
      Discord ID: ${d.suspect_user_id}
    </div>
  `;

  altSelectBenchmark.innerHTML = appState.benchmarks.map(bm => `
    <option value="${bm.id}" ${bm.id === d.matched_benchmark_id ? 'selected' : ''}>
      ${bm.canonical_username} (${bm.community_role})
    </option>
  `).join('');

  altInputJustification.value = `Approved secondary account for ${d.suspect_nickname || d.suspect_username}`;
  openModal(modalTagAlt);
}

btnCloseAltModal.addEventListener('click', () => closeModal(modalTagAlt));
btnCancelAlt.addEventListener('click', () => closeModal(modalTagAlt));

btnConfirmAlt.addEventListener('click', () => {
  if (!activeAltIncidentId) return;

  const targetBenchmarkId = altSelectBenchmark.value;
  const purpose = altInputPurpose.value;
  const justification = altInputJustification.value.trim();

  if (!justification) {
    alert("Please enter a short reason for approving this account.");
    return;
  }

  appState.authorizeAlternate(activeAltIncidentId, targetBenchmarkId, purpose, justification);
  closeModal(modalTagAlt);
  activeAltIncidentId = null;
});

// Close modals when clicking backdrop
document.querySelectorAll('.modal-overlay').forEach(overlay => {
  overlay.addEventListener('click', e => {
    if (e.target === overlay) closeModal(overlay);
  });
});

// Keyboard Accessibility & Hotkeys
window.addEventListener('keydown', e => {
  if (e.key === 'Escape') {
    closeAllModals();
    return;
  }

  const activeTagName = document.activeElement ? document.activeElement.tagName.toLowerCase() : '';
  if (activeTagName === 'input' || activeTagName === 'textarea' || activeTagName === 'select') {
    return;
  }

  if (document.querySelector('.modal-overlay.open')) {
    return;
  }

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

  // Number keys 1-3 switch tabs
  if (['1', '2', '3'].includes(e.key)) {
    const tabs = ['triage', 'vault', 'audit'];
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

// Subscribe to state changes and initial render
appState.subscribe(updateView);
updateView();
