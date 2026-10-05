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
const toastContainer = document.getElementById('desktop-toast-container');
const btnTestToast = document.getElementById('btn-test-toast');
const statusGateway = document.getElementById('status-gateway');

// Discord Setup Elements
const discordTokenInput = document.getElementById('discord-token-input');
const discordGuildIdInput = document.getElementById('discord-guild-id-input');
const btnToggleTokenVis = document.getElementById('btn-toggle-token-vis');
const btnTestHandshake = document.getElementById('btn-test-handshake');
const btnTestHandshakeText = document.getElementById('btn-test-handshake-text');
const btnSaveDiscord = document.getElementById('btn-save-discord');
const btnDisconnectDiscord = document.getElementById('btn-disconnect-discord');
const btnSaveThresholds = document.getElementById('btn-save-thresholds');

// Dynamic Bot Invite & Server Discovery Elements
const dynamicBotInviteCard = document.getElementById('dynamic-bot-invite-card');
const botClientIdLabel = document.getElementById('bot-client-id-label');
const btnQuickInviteBot = document.getElementById('btn-quick-invite-bot');
const btnCopyInviteLink = document.getElementById('btn-copy-invite-link');
const btnCopyInviteText = document.getElementById('btn-copy-invite-text');
const discoveredGuildsWrap = document.getElementById('discovered-guilds-wrap');
const discordGuildSelect = document.getElementById('discord-guild-select');
const btnRefreshGuilds = document.getElementById('btn-refresh-guilds');
const btnRefreshGuildsText = document.getElementById('btn-refresh-guilds-text');
const guildDiscoveryStatus = document.getElementById('guild-discovery-status');

// Discord Hero Status Elements
const discordHeroStatusDot = document.getElementById('discord-hero-status-dot');
const discordHeroBadge = document.getElementById('discord-hero-badge');
const discordHeroBotName = document.getElementById('discord-hero-bot-name');
const discordHeroServerName = document.getElementById('discord-hero-server-name');
const discordHeroGuildId = document.getElementById('discord-hero-guild-id');
const discordBotAvatarImg = document.getElementById('discord-bot-avatar-img');
const discordBotAvatarFallback = document.getElementById('discord-bot-avatar-fallback');

// Diagnostics Elements
const diagHeaderDot = document.getElementById('diag-header-dot');
const diagHeaderTitle = document.getElementById('diag-header-title');
const diagTimestamp = document.getElementById('diag-timestamp');
const diagCheckFormat = document.getElementById('diag-check-format');
const diagCheckAuth = document.getElementById('diag-check-auth');
const diagCheckIntents = document.getElementById('diag-check-intents');
const diagCheckMembership = document.getElementById('diag-check-membership');
const diagCheckPermissions = document.getElementById('diag-check-permissions');

// Threshold Controls & Badges
const sliderSimilarity = document.getElementById('slider-similarity');
const badgeSimilarity = document.getElementById('badge-similarity');
const selectAccountAge = document.getElementById('select-account-age');
const badgeAccountAge = document.getElementById('badge-account-age');
const sliderAvatarHamming = document.getElementById('slider-avatar-hamming');
const badgeAvatarHamming = document.getElementById('badge-avatar-hamming');
const inputCircuitLimit = document.getElementById('input-circuit-limit');
const badgeCircuitLimit = document.getElementById('badge-circuit-limit');

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

  if (appState.benchmarks.length === 0) {
    vaultContainer.innerHTML = `
      <div style="grid-column: 1 / -1; text-align: center; padding: 56px 20px; background: var(--bg-card); border-radius: var(--radius-md); border: 1px solid var(--border-subtle); box-shadow: var(--shadow-card);">
        <div style="width: 48px; height: 48px; margin: 0 auto 14px; border-radius: 50%; background: rgba(88, 101, 242, 0.12); border: 1px solid rgba(88, 101, 242, 0.25); display: flex; align-items: center; justify-content: center; color: var(--brand-blurple);">
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
          </svg>
        </div>
        <h3 style="color: var(--text-header); font-weight: 700; font-size: 1.15rem; margin-bottom: 6px;">No Protected Benchmarks Yet</h3>
        <p style="color: var(--text-secondary); font-size: 0.85rem; max-width: 440px; margin: 0 auto 18px;">
          Add pastors, elders, or staff to create official ground-truth benchmarks that safeguard against imposter accounts.
        </p>
        <button class="btn btn-primary" id="btn-empty-add-bm" style="display: inline-flex; align-items: center; gap: 8px; margin: 0 auto;">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
          <span>Enroll First Leader</span>
        </button>
      </div>
    `;
    document.getElementById('btn-empty-add-bm')?.addEventListener('click', () => {
      openModal(modalCreateBm);
    });
    return;
  }

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
  if (!appState.auditLogs || appState.auditLogs.length === 0) {
    auditTableBody.innerHTML = `
      <tr>
        <td colspan="5" style="text-align: center; padding: 48px 20px; color: var(--text-muted); font-size: 0.9rem;">
          No activity recorded in local audit ledger yet.
        </td>
      </tr>
    `;
    return;
  }

  auditTableBody.innerHTML = appState.auditLogs.map(aud => {
    let actionBadgeColor = 'var(--text-secondary)';
    let actionBadgeBg = 'var(--bg-secondary)';
    let actionBadgeText = aud.action;

    const actionLower = (aud.action || '').toLowerCase();

    if (actionLower.includes('adjudicate') || actionLower.includes('ban')) {
      actionBadgeColor = '#ffa1a4';
      actionBadgeBg = 'var(--discord-red-subtle)';
      actionBadgeText = 'Banned Imposter';
    } else if (actionLower.includes('authorize') || actionLower.includes('whitelist')) {
      actionBadgeColor = '#c7d2fe';
      actionBadgeBg = 'var(--brand-blurple-subtle)';
      actionBadgeText = 'Allowed Known Alt';
    } else if (actionLower.includes('quarantine') || actionLower.includes('exclude')) {
      actionBadgeColor = '#e9d5ff';
      actionBadgeBg = 'var(--discord-purple-subtle)';
      actionBadgeText = 'Restricted Account';
    } else if (actionLower.includes('dismiss')) {
      actionBadgeColor = 'var(--text-secondary)';
      actionBadgeBg = 'rgba(255, 255, 255, 0.05)';
      actionBadgeText = 'Ignored Alert (Safe)';
    } else if (actionLower.includes('benchmark')) {
      actionBadgeColor = 'var(--oh-orange-400)';
      actionBadgeBg = 'var(--oh-orange-subtle)';
      actionBadgeText = 'Protected Leader Added';
    } else if (actionLower.includes('circuit_breaker_tripped')) {
      actionBadgeColor = '#fbbf24';
      actionBadgeBg = 'rgba(251, 191, 36, 0.12)';
      actionBadgeText = 'Safety Pause Tripped';
    } else if (actionLower.includes('circuit_breaker_reset')) {
      actionBadgeColor = '#34d399';
      actionBadgeBg = 'var(--discord-green-subtle)';
      actionBadgeText = 'Safety Pause Reset';
    }

    const suspectName = aud.metadata?.suspect_username;
    const targetDisplay = suspectName 
      ? `<span style="font-weight: 600; color: var(--text-primary);">@${suspectName}</span> <span style="font-family: var(--font-mono); font-size: 0.72rem; color: var(--text-muted);">(${aud.target_user_id || ''})</span>`
      : (aud.target_user_id || 'System');

    return `
      <tr style="border-bottom: 1px solid var(--border-subtle);">
        <td style="padding: 14px 20px; color: var(--text-muted); font-size: 0.82rem;" class="tabular-nums">${formatTime(aud.timestamp)}</td>
        <td style="padding: 14px 20px;">
          <span style="font-weight: 700; font-size: 0.78rem; color: ${actionBadgeColor}; background: ${actionBadgeBg}; padding: 4px 10px; border-radius: var(--radius-xs); border: 1px solid rgba(255, 255, 255, 0.08); display: inline-block;">
            ${actionBadgeText}
          </span>
        </td>
        <td style="padding: 14px 20px; font-size: 0.84rem; color: var(--text-secondary);">${targetDisplay}</td>
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

// Tab Switching Helper
export function switchTab(targetTab) {
  navTabs.forEach(t => t.classList.remove('active'));
  tabPanes.forEach(p => p.classList.remove('active'));

  const tabBtn = document.querySelector(`[data-tab="${targetTab}"]`);
  if (tabBtn) tabBtn.classList.add('active');
  const pane = document.getElementById(`pane-${targetTab}`);
  if (pane) pane.classList.add('active');
  appState.setTab(targetTab);

  if (targetTab === 'discord') {
    loadDiscordConfig();
  }
}

// Tab Switching Handler
navTabs.forEach(tab => {
  tab.addEventListener('click', () => {
    const targetTab = tab.getAttribute('data-tab');
    switchTab(targetTab);
  });
});

// Direct link from header status pill to discord configuration
statusGateway?.addEventListener('click', () => {
  switchTab('discord');
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

  // Number keys 1-5 switch tabs
  if (['1', '2', '3', '4', '5'].includes(e.key)) {
    const tabs = ['triage', 'vault', 'audit', 'discord', 'help'];
    const idx = parseInt(e.key, 10) - 1;
    if (tabs[idx]) {
      switchTab(tabs[idx]);
    }
  }
});

// ==========================================================================
// Phase 17.2: Native OS Desktop Notifications & Interactive Action Toasts
// ==========================================================================
const notifiedIncidentIds = new Set();

export function showDesktopNotificationToast(payload, forceShow = false) {
  if (!toastContainer) return;

  const incidentId = payload.incident_id || payload.id;
  if (!forceShow && notifiedIncidentIds.has(incidentId)) {
    return;
  }
  notifiedIncidentIds.add(incidentId);

  const rawTier = (payload.risk_tier || payload.discrepancy?.risk_tier || '').toLowerCase();
  const isCritical = rawTier === 'critical';
  const isElevated = rawTier === 'elevated';

  // Only dispatch for Elevated or Critical discrepancies per Phase 17.2 specification
  if (!isCritical && !isElevated && !forceShow) {
    return;
  }

  const tierClass = isCritical ? 'risk-critical' : 'risk-elevated';
  const tierBadge = isCritical ? 'Critical' : 'Elevated';

  const suspectName = payload.suspect_username || payload.discrepancy?.suspect_username || 'Unknown_Suspect';
  const suspectId = payload.suspect_user_id || payload.discrepancy?.suspect_user_id || '998877665544332211';
  const targetName = payload.matched_benchmark_name || payload.discrepancy?.matched_benchmark_name || 'Protected Leader';

  let similarityPct = 95;
  if (payload.similarity_score !== undefined) {
    similarityPct = Math.round(payload.similarity_score * 100);
  } else if (payload.discrepancy?.string_similarity_score !== undefined) {
    similarityPct = Math.round(payload.discrepancy.string_similarity_score * 100);
  }

  const reason = payload.reason || payload.discrepancy?.normalized_diff || 'Lookalike impersonation candidate detected';

  // Remove existing toast for same incident if already present
  const existingToast = document.getElementById(`toast-${incidentId}`);
  if (existingToast) existingToast.remove();

  const toastEl = document.createElement('div');
  toastEl.className = `desktop-toast ${tierClass}`;
  toastEl.id = `toast-${incidentId}`;
  toastEl.setAttribute('role', 'alert');
  toastEl.innerHTML = `
    <div class="toast-header">
      <div class="toast-brand-row">
        <img src="assets/truthbeacon_emblem.png" class="toast-brand-icon" alt="TruthBeacon">
        <span class="toast-brand-title">TruthBeacon Alert</span>
      </div>
      <div class="toast-header-right">
        <span class="toast-risk-badge ${isCritical ? 'critical' : 'elevated'}">${tierBadge}</span>
        <button class="toast-close-btn" data-action="close" title="Dismiss notification" aria-label="Close notification">&times;</button>
      </div>
    </div>
    <div class="toast-body">
      <div class="toast-comparison-strip">
        <span class="toast-suspect-name" title="Suspect Imposter: @${suspectName}">@${suspectName}</span>
        <span class="toast-vs-tag">VS</span>
        <span class="toast-target-name" title="Official Leader: @${targetName}">@${targetName}</span>
      </div>
      <div class="toast-meta-line">
        <span class="toast-meta-pill">${similarityPct}% Match</span>
        ${reason}
      </div>
    </div>
    <div class="toast-actions-row">
      <button class="btn-toast btn-toast-inspect" data-action="inspect" data-incident-id="${incidentId}" title="Inspect in alerts queue">
        Inspect
      </button>
      <button class="btn-toast btn-toast-dismiss" data-action="dismiss" data-incident-id="${incidentId}" title="Dismiss as benign coincidence [D]">
        <kbd>D</kbd> Dismiss
      </button>
      <button class="btn-toast btn-toast-ban" data-action="ban" data-incident-id="${incidentId}" title="Ban imposter from server [B]">
        <kbd>B</kbd> Ban & Purge
      </button>
    </div>
    <div class="toast-progress-track">
      <div class="toast-progress-bar"></div>
    </div>
  `;

  toastContainer.appendChild(toastEl);

  // Dispatch Native OS Notification Toast
  invokeCommand('dispatch_desktop_notification', {
    payload: {
      incident_id: incidentId,
      risk_tier: isCritical ? 'Critical' : 'Elevated',
      suspect_username: suspectName,
      suspect_user_id: suspectId,
      matched_benchmark_name: targetName,
      similarity_score: similarityPct / 100,
      reason,
      actions: ['Inspect', 'Dismiss', 'Ban & Purge']
    }
  }).catch(err => {
    console.debug('[TruthBeacon Notification] OS notification note:', err);
  });

  // Auto-dismiss countdown timer (12s)
  const autoDismissTimer = setTimeout(() => {
    closeToastWithAnimation(toastEl);
  }, 12000);

  toastEl._dismissTimer = autoDismissTimer;
}

function closeToastWithAnimation(toastEl) {
  if (!toastEl) return;
  if (toastEl._dismissTimer) clearTimeout(toastEl._dismissTimer);
  toastEl.classList.add('hiding');
  setTimeout(() => {
    if (toastEl.parentNode) toastEl.remove();
  }, 220);
}

export function inspectIncidentCard(incidentId) {
  // 1. Switch to triage alerts tab
  navTabs.forEach(t => t.classList.remove('active'));
  tabPanes.forEach(p => p.classList.remove('active'));
  document.querySelector('[data-tab="triage"]')?.classList.add('active');
  document.getElementById('pane-triage')?.classList.add('active');
  appState.setTab('triage');

  // 2. Locate inspection card, scroll into center, and pulse highlight
  requestAnimationFrame(() => {
    const card = document.getElementById(`card-${incidentId}`);
    if (card) {
      card.scrollIntoView({ behavior: 'smooth', block: 'center' });
      card.classList.remove('card-inspected-highlight');
      void card.offsetWidth; // trigger reflow
      card.classList.add('card-inspected-highlight');
      setTimeout(() => card.classList.remove('card-inspected-highlight'), 3000);
    }
  });
}

// Event delegation for notification toast action buttons: Inspect, Dismiss, Ban & Purge
toastContainer?.addEventListener('click', async e => {
  const btn = e.target.closest('button[data-action]');
  if (!btn) return;

  const action = btn.getAttribute('data-action');
  const incidentId = btn.getAttribute('data-incident-id');
  const toastEl = btn.closest('.desktop-toast');

  if (action === 'close') {
    closeToastWithAnimation(toastEl);
  } else if (action === 'inspect') {
    closeToastWithAnimation(toastEl);
    inspectIncidentCard(incidentId);
    invokeCommand('execute_notification_action', {
      incident_id: incidentId,
      action: 'Inspect'
    }).catch(() => {});
  } else if (action === 'dismiss') {
    closeToastWithAnimation(toastEl);
    appState.resolveIncident(incidentId, 'dismiss');
    invokeCommand('execute_notification_action', {
      incident_id: incidentId,
      action: 'Dismiss'
    }).catch(() => {});
  } else if (action === 'ban') {
    closeToastWithAnimation(toastEl);
    appState.resolveIncident(incidentId, 'ban');
    invokeCommand('execute_notification_action', {
      incident_id: incidentId,
      action: 'Ban & Purge'
    }).catch(() => {});
  }
});

// Manual Test Toast Button
btnTestToast?.addEventListener('click', () => {
  const pendingIncidents = appState.incidents.filter(i => i.status === 'pending');
  const targetIncident = pendingIncidents.find(i => i.discrepancy.risk_tier === 'critical')
    || pendingIncidents[0]
    || {
      id: `test_crit_${Date.now()}`,
      discrepancy: {
        suspect_username: "Pastor_Dan",
        suspect_user_id: "987654321012345678",
        matched_benchmark_name: "PastorDan",
        string_similarity_score: 0.98,
        normalized_diff: "Lookalike homoglyph substitution detected",
        risk_tier: "critical"
      }
    };

  showDesktopNotificationToast(targetIncident, true);
});

// Listen for Tauri backend events
if (typeof window !== 'undefined' && window.__TAURI__?.event?.listen) {
  window.__TAURI__.event.listen('truthbeacon://inspect-incident', event => {
    if (event.payload?.incident_id) {
      inspectIncidentCard(event.payload.incident_id);
    }
  });

  window.__TAURI__.event.listen('truthbeacon://native-notification', event => {
    if (event.payload) {
      showDesktopNotificationToast(event.payload);
    }
  });
}

// ==========================================================================
// Discord Setup, Pre-flight Diagnostics & Parameters Controller
// ==========================================================================

function updateSimilarityBadge(val) {
  if (!badgeSimilarity) return;
  const num = parseInt(val, 10);
  const label = num >= 92 ? 'Near Exact' : num >= 82 ? 'Standard' : 'Broad';
  badgeSimilarity.textContent = `${num}% (${label})`;
}

function updateAccountAgeBadge(val) {
  if (!badgeAccountAge) return;
  const num = parseInt(val, 10);
  if (num >= 720) {
    badgeAccountAge.textContent = '30 Days';
  } else if (num >= 168) {
    badgeAccountAge.textContent = '7 Days';
  } else {
    badgeAccountAge.textContent = `${num} Hours`;
  }
}

function updateAvatarHammingBadge(val) {
  if (!badgeAvatarHamming) return;
  const num = parseInt(val, 10);
  const label = num <= 4 ? 'Pixel Clones Only' : num <= 10 ? 'Notable' : 'Loose';
  badgeAvatarHamming.innerHTML = `&le; ${num} Bits (${label})`;
}

function updateCircuitLimitBadge(val) {
  if (!badgeCircuitLimit) return;
  badgeCircuitLimit.textContent = `${val} Actions / Min`;
}

// Live parameter slider badges
sliderSimilarity?.addEventListener('input', e => {
  updateSimilarityBadge(e.target.value);
});

selectAccountAge?.addEventListener('change', e => {
  updateAccountAgeBadge(e.target.value);
});

sliderAvatarHamming?.addEventListener('input', e => {
  updateAvatarHammingBadge(e.target.value);
});

inputCircuitLimit?.addEventListener('input', e => {
  updateCircuitLimitBadge(e.target.value);
});

// Toggle password mask visibility
btnToggleTokenVis?.addEventListener('click', () => {
  if (!discordTokenInput) return;
  const isPass = discordTokenInput.type === 'password';
  discordTokenInput.type = isPass ? 'text' : 'password';
  btnToggleTokenVis.innerHTML = isPass
    ? `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"/><line x1="1" y1="1" x2="23" y2="23"/></svg>`
    : `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>`;
});

// Footer button to open Discord setup from bottom of help screen
document.getElementById('btn-help-go-discord')?.addEventListener('click', () => {
  switchTab('discord');
});

// Extract numeric Bot Client ID from segment 1 of Discord bot token
export function extractBotClientId(token) {
  if (!token || typeof token !== 'string') return null;
  const clean = token.trim().replace(/^Bot\s+/i, '');
  const parts = clean.split('.');
  if (parts.length < 2) return null;
  try {
    let b64 = parts[0].replace(/-/g, '+').replace(/_/g, '/');
    while (b64.length % 4 !== 0) {
      b64 += '=';
    }
    const decoded = atob(b64);
    if (/^\d{17,20}$/.test(decoded)) {
      return decoded;
    }
  } catch (_) {}
  return null;
}

// Render dynamic 1-click bot invite assistant
function renderBotInviteCard(clientId) {
  if (!dynamicBotInviteCard) return;
  if (!clientId) {
    dynamicBotInviteCard.style.display = 'none';
    return;
  }
  // Permission bitfield: 1099511628806
  // VIEW_CHANNEL (1024) | KICK_MEMBERS (2) | BAN_MEMBERS (4) | MODERATE_MEMBERS (1099511627776)
  const inviteUrl = `https://discord.com/oauth2/authorize?client_id=${clientId}&scope=bot%20applications.commands&permissions=1099511628806`;
  dynamicBotInviteCard.style.display = 'block';
  if (botClientIdLabel) {
    botClientIdLabel.textContent = `App ID: ${clientId}`;
  }
  if (btnQuickInviteBot) {
    btnQuickInviteBot.href = inviteUrl;
  }
  if (btnCopyInviteLink) {
    btnCopyInviteLink.onclick = async () => {
      try {
        await navigator.clipboard.writeText(inviteUrl);
        if (btnCopyInviteText) btnCopyInviteText.textContent = 'Copied!';
        setTimeout(() => {
          if (btnCopyInviteText) btnCopyInviteText.textContent = 'Copy Link';
        }, 2000);
      } catch (_) {
        prompt('Copy Discord Bot Invite Link:', inviteUrl);
      }
    };
  }
}

function updateBotInviteCard(token) {
  const clientId = extractBotClientId(token);
  if (clientId) {
    renderBotInviteCard(clientId);
  } else if (!token || !token.startsWith('••••')) {
    renderBotInviteCard(null);
  }
}

// Automated server discovery
let isDiscoveringGuilds = false;
async function discoverServers(tokenOverride) {
  if (isDiscoveringGuilds) return;
  const rawToken = tokenOverride !== undefined ? tokenOverride : (discordTokenInput ? discordTokenInput.value.trim() : '');
  const token = rawToken.startsWith('••••') ? '' : rawToken;

  isDiscoveringGuilds = true;
  if (btnRefreshGuilds) btnRefreshGuilds.disabled = true;
  if (btnRefreshGuildsText) btnRefreshGuildsText.textContent = 'Discovering...';
  if (guildDiscoveryStatus) {
    guildDiscoveryStatus.innerHTML = '<span class="status-scanning">Scanning Discord Gateway for your bot\'s servers...</span>';
  }

  try {
    const guilds = await invokeCommand('fetch_bot_guilds', { token: token || null });
    if (Array.isArray(guilds) && guilds.length > 0) {
      if (discoveredGuildsWrap) discoveredGuildsWrap.style.display = 'block';
      if (discordGuildSelect) {
        discordGuildSelect.innerHTML = `<option value="">-- Discovered Servers (${guilds.length}) --</option>` +
          guilds.map(g => `<option value="${g.id}">${g.name} (${g.id})</option>`).join('');

        const currentGuildId = discordGuildIdInput ? discordGuildIdInput.value.trim() : '';
        const match = guilds.find(g => g.id === currentGuildId);
        if (match) {
          discordGuildSelect.value = match.id;
        } else if (guilds.length === 1 && !currentGuildId) {
          discordGuildSelect.value = guilds[0].id;
          if (discordGuildIdInput) {
            discordGuildIdInput.value = guilds[0].id;
            sessionStorage.setItem('truthbeacon_setup_guild_id', guilds[0].id);
          }
        }
      }
      if (guildDiscoveryStatus) {
        guildDiscoveryStatus.innerHTML = `<span style="color: var(--color-success); font-weight: 500;">✓ Discovered ${guilds.length} server${guilds.length > 1 ? 's' : ''}. Select your server above.</span>`;
      }
    } else {
      if (guildDiscoveryStatus) {
        guildDiscoveryStatus.innerHTML = `<span style="color: var(--color-warning);">Bot is active, but not in any servers yet. Click <strong>"1-Click Authorize & Invite"</strong> above!</span>`;
      }
    }
  } catch (err) {
    console.warn('Guild discovery note:', err);
    if (guildDiscoveryStatus) {
      guildDiscoveryStatus.innerHTML = `<span style="color: var(--color-text-muted); font-size: 0.8125rem;">${err?.message || 'Paste bot token or invite bot to server to discover.'}</span>`;
    }
  } finally {
    isDiscoveringGuilds = false;
    if (btnRefreshGuilds) btnRefreshGuilds.disabled = false;
    if (btnRefreshGuildsText) btnRefreshGuildsText.textContent = 'Discover Servers';
  }
}

// Server selector change handler
discordGuildSelect?.addEventListener('change', async e => {
  const val = e.target.value;
  if (val && discordGuildIdInput) {
    discordGuildIdInput.value = val;
    sessionStorage.setItem('truthbeacon_setup_guild_id', val);
    const selectedOption = e.target.options[e.target.selectedIndex];
    if (guildDiscoveryStatus) {
      guildDiscoveryStatus.innerHTML = `<span style="color: var(--color-success); font-weight: 500;">Target server set to: <strong>${selectedOption ? selectedOption.text : val}</strong></span>`;
    }
    // Automatically trigger handshake check for zero-friction verification
    const token = discordTokenInput ? discordTokenInput.value.trim() : '';
    if (token) {
      try {
        const result = await invokeCommand('verify_bot_handshake', {
          token: token.startsWith('••••') ? '' : token,
          guild_id: val
        });
        renderHandshakeResult(result);
      } catch (_) {}
    }
  }
});

// Refresh discovered servers button
btnRefreshGuilds?.addEventListener('click', () => {
  discoverServers();
});

// Persist draft setup inputs during tab navigation
discordGuildIdInput?.addEventListener('input', e => {
  try {
    sessionStorage.setItem('truthbeacon_setup_guild_id', e.target.value.trim());
  } catch (_) {}
});

discordTokenInput?.addEventListener('input', e => {
  try {
    const val = e.target.value.trim();
    if (!val.startsWith('••••')) {
      sessionStorage.setItem('truthbeacon_setup_token_draft', val);
      updateBotInviteCard(val);
      if (val.split('.').length >= 3) {
        discoverServers(val);
      }
    }
  } catch (_) {}
});

discordTokenInput?.addEventListener('paste', () => {
  setTimeout(() => {
    const val = discordTokenInput.value.trim();
    updateBotInviteCard(val);
    if (val.split('.').length >= 3) {
      discoverServers(val);
    }
  }, 50);
});

// Pre-flight handshake diagnostic check
btnTestHandshake?.addEventListener('click', async () => {
  const token = discordTokenInput ? discordTokenInput.value.trim() : '';
  const guildId = discordGuildIdInput ? discordGuildIdInput.value.trim() : '';

  if (!guildId) {
    alert('Please enter a Target Discord Server (Guild) ID to run the diagnostic check.');
    discordGuildIdInput?.focus();
    return;
  }

  btnTestHandshake.disabled = true;
  if (btnTestHandshakeText) btnTestHandshakeText.textContent = 'Validating Gateway...';
  if (diagHeaderDot) diagHeaderDot.className = 'diagnostic-status-indicator warning';
  if (diagHeaderTitle) diagHeaderTitle.textContent = 'Pre-flight Verification Running...';

  try {
    const result = await invokeCommand('verify_bot_handshake', {
      token: token.startsWith('••••') ? '' : token,
      guild_id: guildId
    });

    renderHandshakeResult(result);
  } catch (err) {
    console.error('Handshake verification error:', err);
    if (diagHeaderDot) diagHeaderDot.className = 'diagnostic-status-indicator danger';
    if (diagHeaderTitle) diagHeaderTitle.textContent = 'Handshake Failed';
    alert(err?.message || 'Handshake check failed. Verify token and bot permissions.');
  } finally {
    btnTestHandshake.disabled = false;
    if (btnTestHandshakeText) btnTestHandshakeText.textContent = 'Test Handshake & Permissions';
  }
});

function setDiagItem(el, ok, okText, failText) {
  if (!el) return;
  const icon = el.querySelector('.diag-icon');
  const text = el.querySelector('.diag-text');
  if (icon) {
    icon.className = `diag-icon ${ok ? 'success' : 'fail'}`;
    icon.innerHTML = ok ? '&#10003;' : '&#10007;';
  }
  if (text) {
    text.innerHTML = ok ? okText : `<strong style="color: var(--color-danger);">${failText}</strong>`;
  }
}

function renderHandshakeResult(res) {
  if (!res) return;
  const formatValid = res.format_valid !== undefined ? !!res.format_valid : !!(res.is_official_bot || res.bot_id);
  const authValid = res.gateway_authenticated !== undefined ? !!res.gateway_authenticated : !!(res.bot_id || res.bot_username);
  const intentsValid = res.privileged_intents_active !== undefined ? !!res.privileged_intents_active : true;
  const guildFound = res.guild_found !== undefined ? !!res.guild_found : !!(res.target_guild_id || res.target_guild_name);
  const permsOk = res.moderation_permissions_ok !== undefined ? !!res.moderation_permissions_ok : (res.permissions ? !!res.permissions.is_fully_authorized : true);

  const passed = formatValid && authValid && intentsValid && guildFound && permsOk;

  if (diagHeaderDot) diagHeaderDot.className = `diagnostic-status-indicator ${passed ? 'online' : 'danger'}`;
  if (diagHeaderTitle) {
    diagHeaderTitle.textContent = passed ? 'All 5 Pre-flight Checks Passed' : 'Verification Issue Detected';
  }
  if (diagTimestamp) {
    diagTimestamp.textContent = `Tested ${new Date().toLocaleTimeString()}`;
  }

  const serverName = res.guild_name || res.target_guild_name || 'Target Server';
  const botName = res.bot_name || res.bot_username || 'TruthBeacon Guard';

  setDiagItem(diagCheckFormat, formatValid, 'Token Format: <strong>Official 3-Part Bot Token</strong>', 'Token Format: Invalid or Malformed Token');
  setDiagItem(diagCheckAuth, authValid, 'Gateway Handshake: <strong>Authenticated with Discord v10</strong>', 'Gateway Handshake: Authentication Failed (401 Unauthorized)');
  setDiagItem(diagCheckIntents, intentsValid, 'Privileged Intent: <strong>Server Members Intent (GUILD_MEMBERS) Active</strong>', 'Privileged Intent: Missing GUILD_MEMBERS Intent (Enable in Dev Portal!)');
  setDiagItem(diagCheckMembership, guildFound, `Server Membership: <strong>Bot Present in "${serverName}"</strong>`, 'Server Membership: Bot Not Found in Target Server (Invite Bot First)');
  setDiagItem(diagCheckPermissions, permsOk, 'Moderation Permissions: <strong>Kick, Ban, Moderate & View Channels Granted</strong>', 'Moderation Permissions: Missing Required Moderation Grants');

  if (discordHeroBotName) discordHeroBotName.textContent = botName;
  if (discordHeroServerName && serverName) discordHeroServerName.textContent = serverName;
}

// Save Discord Pairing & Credentials
btnSaveDiscord?.addEventListener('click', async () => {
  const token = discordTokenInput ? discordTokenInput.value.trim() : '';
  const guildId = discordGuildIdInput ? discordGuildIdInput.value.trim() : '';
  const similarity = sliderSimilarity ? parseInt(sliderSimilarity.value, 10) : 85;
  const accountAge = selectAccountAge ? parseInt(selectAccountAge.value, 10) : 72;
  const avatarHamming = sliderAvatarHamming ? parseInt(sliderAvatarHamming.value, 10) : 10;
  const circuitLimit = inputCircuitLimit ? parseInt(inputCircuitLimit.value, 10) : 5;

  if (!guildId) {
    alert('Please enter a Target Discord Server (Guild) ID.');
    discordGuildIdInput?.focus();
    return;
  }

  btnSaveDiscord.disabled = true;
  const origText = btnSaveDiscord.textContent;
  btnSaveDiscord.textContent = 'Saving to Keychain...';

  try {
    const payload = {
      token: token.startsWith('••••') ? '' : token,
      guild_id: guildId,
      thresholds: {
        similarity: isNaN(similarity) ? 85 : similarity,
        account_age_hours: isNaN(accountAge) ? 72 : accountAge,
        avatar_hamming_distance: isNaN(avatarHamming) ? 10 : avatarHamming,
        circuit_limit_per_minute: isNaN(circuitLimit) ? 5 : circuitLimit
      }
    };
    const result = await invokeCommand('save_discord_config', payload);
    btnSaveDiscord.textContent = 'Connected & Saved!';

    sessionStorage.removeItem('truthbeacon_setup_token_draft');
    sessionStorage.setItem('truthbeacon_setup_guild_id', guildId);

    if (result) {
      renderHandshakeResult(result);
    }

    setTimeout(() => {
      btnSaveDiscord.textContent = origText;
      btnSaveDiscord.disabled = false;
    }, 1500);

    await loadDiscordConfig();
  } catch (err) {
    console.error('Save discord error:', err);
    btnSaveDiscord.textContent = 'Failed to Save';
    alert(err?.message || 'Failed to save Discord configuration. Please check your credentials.');
    setTimeout(() => {
      btnSaveDiscord.textContent = origText;
      btnSaveDiscord.disabled = false;
    }, 2000);
  }
});

// Disconnect Discord
btnDisconnectDiscord?.addEventListener('click', async () => {
  if (!confirm('Are you sure you want to disconnect Discord and erase the bot token from your OS Keychain?')) {
    return;
  }
  try {
    const guildId = discordGuildIdInput ? discordGuildIdInput.value.trim() : (appState.selectedGuild.id || '');
    await invokeCommand('disconnect_discord', { guild_id: guildId });
    if (discordTokenInput) discordTokenInput.value = '';
    if (discordGuildIdInput) discordGuildIdInput.value = '';
    if (discoveredGuildsWrap) discoveredGuildsWrap.style.display = 'none';
    if (dynamicBotInviteCard) dynamicBotInviteCard.style.display = 'none';
    if (discordGuildSelect) discordGuildSelect.innerHTML = '<option value="">-- Select Discovered Server --</option>';
    if (guildDiscoveryStatus) guildDiscoveryStatus.textContent = 'Paste your Bot Token above and TruthBeacon will automatically discover your servers.';
    sessionStorage.removeItem('truthbeacon_setup_guild_id');
    sessionStorage.removeItem('truthbeacon_setup_token_draft');
    await loadDiscordConfig();
  } catch (err) {
    console.error('Disconnect discord error:', err);
  }
});

// Save Parameters only
btnSaveThresholds?.addEventListener('click', async () => {
  const guildId = discordGuildIdInput ? discordGuildIdInput.value.trim() : (appState.selectedGuild.id || '');
  const token = discordTokenInput ? discordTokenInput.value.trim() : '';
  const similarity = sliderSimilarity ? parseInt(sliderSimilarity.value, 10) : 85;
  const accountAge = selectAccountAge ? parseInt(selectAccountAge.value, 10) : 72;
  const avatarHamming = sliderAvatarHamming ? parseInt(sliderAvatarHamming.value, 10) : 10;
  const circuitLimit = inputCircuitLimit ? parseInt(inputCircuitLimit.value, 10) : 5;

  btnSaveThresholds.disabled = true;
  btnSaveThresholds.textContent = 'Saving...';
  try {
    await invokeCommand('save_discord_config', {
      token: token.startsWith('••••') ? '' : token,
      guild_id: guildId,
      thresholds: {
        similarity: isNaN(similarity) ? 85 : similarity,
        account_age_hours: isNaN(accountAge) ? 72 : accountAge,
        avatar_hamming_distance: isNaN(avatarHamming) ? 10 : avatarHamming,
        circuit_limit_per_minute: isNaN(circuitLimit) ? 5 : circuitLimit
      }
    });
    btnSaveThresholds.textContent = 'Parameters Saved!';
    setTimeout(() => {
      btnSaveThresholds.textContent = 'Save Parameters';
      btnSaveThresholds.disabled = false;
    }, 1200);
  } catch (err) {
    console.error('Save thresholds error:', err);
    btnSaveThresholds.textContent = 'Error';
    btnSaveThresholds.disabled = false;
  }
});

// Load Discord Configuration from Backend
async function loadDiscordConfig() {
  try {
    const config = await invokeCommand('get_discord_config');
    renderDiscordConfig(config);
  } catch (err) {
    console.error('Failed to load discord config:', err);
  }
}

function renderDiscordConfig(config) {
  if (!config) return;

  if (discordHeroBotName) discordHeroBotName.textContent = config.bot_name || 'TruthBeacon Bot';
  if (discordHeroServerName) discordHeroServerName.textContent = config.guild_name || 'Not Configured';
  if (discordHeroGuildId) discordHeroGuildId.textContent = config.guild_id ? `ID: ${config.guild_id}` : 'No Server Set';

  const currentGuildName = document.getElementById('current-guild-name');
  const currentGuildIcon = document.getElementById('current-guild-icon');
  if (currentGuildName) {
    currentGuildName.textContent = config.guild_name || (config.guild_id ? `Server (${config.guild_id})` : 'No Server Connected');
  }
  if (currentGuildIcon) {
    currentGuildIcon.textContent = config.guild_name ? config.guild_name.charAt(0).toUpperCase() : '—';
  }

  if (config.connected !== undefined) {
    appState.daemonHealth.gateway_connected = !!config.connected;
  }
  if (config.guild_id) {
    appState.selectedGuild.id = config.guild_id;
  }
  if (config.guild_name) {
    appState.selectedGuild.name = config.guild_name;
  }

  if (discordGuildIdInput && !discordGuildIdInput.matches(':focus')) {
    if (config.guild_id) {
      discordGuildIdInput.value = config.guild_id;
    } else {
      const draft = sessionStorage.getItem('truthbeacon_setup_guild_id');
      if (draft && !discordGuildIdInput.value) {
        discordGuildIdInput.value = draft;
      }
    }
  }

  if (discordTokenInput && !discordTokenInput.matches(':focus')) {
    if (config.has_token) {
      discordTokenInput.value = config.token_masked || '••••••••••••••••••••••••••••••••';
    } else {
      const draft = sessionStorage.getItem('truthbeacon_setup_token_draft');
      if (draft && !discordTokenInput.value) {
        discordTokenInput.value = draft;
      } else if (!draft && !discordTokenInput.value) {
        discordTokenInput.value = '';
      }
    }
  }

  if (config.connected) {
    if (discordHeroStatusDot) discordHeroStatusDot.className = 'discord-status-indicator online';
    if (discordHeroBadge) {
      discordHeroBadge.className = 'discord-badge-active';
      discordHeroBadge.textContent = 'CONNECTED';
    }
    if (statusGateway) {
      statusGateway.innerHTML = `<span class="status-dot online"></span><span>Discord: <strong>Connected</strong></span>`;
    }
  } else {
    if (discordHeroStatusDot) discordHeroStatusDot.className = 'discord-status-indicator offline';
    if (discordHeroBadge) {
      discordHeroBadge.className = 'discord-badge-active disconnected';
      discordHeroBadge.textContent = 'DISCONNECTED';
    }
    if (statusGateway) {
      statusGateway.innerHTML = `<span class="status-dot offline"></span><span>Discord: <strong>Disconnected</strong></span>`;
    }
  }

  if (config.bot_avatar_url && discordBotAvatarImg && discordBotAvatarFallback) {
    discordBotAvatarImg.src = config.bot_avatar_url;
    discordBotAvatarImg.style.display = 'block';
    discordBotAvatarFallback.style.display = 'none';
  } else if (discordBotAvatarImg && discordBotAvatarFallback) {
    discordBotAvatarImg.style.display = 'none';
    discordBotAvatarFallback.style.display = 'flex';
  }

  if (config.thresholds) {
    const t = config.thresholds;
    if (sliderSimilarity && t.similarity !== undefined) {
      sliderSimilarity.value = t.similarity;
      updateSimilarityBadge(t.similarity);
    }
    if (selectAccountAge && t.account_age_hours !== undefined) {
      selectAccountAge.value = t.account_age_hours;
      updateAccountAgeBadge(t.account_age_hours);
    }
    if (sliderAvatarHamming && t.avatar_hamming_distance !== undefined) {
      sliderAvatarHamming.value = t.avatar_hamming_distance;
      updateAvatarHammingBadge(t.avatar_hamming_distance);
    }
    if (inputCircuitLimit && t.circuit_limit_per_minute !== undefined) {
      inputCircuitLimit.value = t.circuit_limit_per_minute;
      updateCircuitLimitBadge(t.circuit_limit_per_minute);
    }
  }

  if (config.bot_id) {
    renderBotInviteCard(config.bot_id);
  } else if (!config.has_token) {
    renderBotInviteCard(null);
  }

  if (config.has_token && !isDiscoveringGuilds && (!discordGuildSelect || discordGuildSelect.options.length <= 1)) {
    discoverServers();
  }
}

// Subscribe to state changes and initial render
appState.subscribe(updateView);
updateView();
loadDiscordConfig();

