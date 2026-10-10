// TruthBeacon - GitHub Pages Interactive Logic
// OS Detection, Asset Routing, Dynamic Filtering, and Clipboard Utilities

document.addEventListener('DOMContentLoaded', () => {
  initOSDetection();
  initPlatformTabs();
  initVerifyTabs();
  initShowcaseTabs();
  initCopyButtons();
  initLightbox();
});

// Release Asset Configuration (v0.2.2)
const RELEASE_BASE_URL = 'https://github.com/boredpolymath/truth-beacon/releases/download/v0.2.2/';
const ASSETS = {
  mac_dmg: {
    filename: 'TruthBeacon_0.2.2_universal.dmg',
    size: '29.5 MB',
    arch: 'Universal 2 (Apple Silicon & Intel)',
    label: 'Download for macOS',
    sub: 'Universal DMG (macOS 10.15+)',
    hash: 'c450fa110a54f2d64eab14763b8d9a7505f7bb0100c95417eca0d185c713a629'
  },
  mac_zip: {
    filename: 'TruthBeacon_0.2.2_macos_universal.zip',
    size: '27.9 MB',
    arch: 'Universal 2 (.app Portable)',
    hash: '58101f915f97e4950a8d03ff85a32d21fb6e49d9dc7703ca0bb71e3c06375730'
  },
  mac_bin: {
    filename: 'truth-beacon-universal',
    size: '59.3 MB',
    arch: 'Mach-O Universal Fat Binary',
    hash: '056044419ee0d312329a0f58365c25293db8f92f1bca51d1849b6ab444363ce5'
  },
  win_exe: {
    filename: 'TruthBeacon_0.2.2_x64-setup.exe',
    size: '9.5 MB',
    arch: 'Windows 64-bit (NSIS Installer)',
    label: 'Download for Windows',
    sub: 'Windows 10 / 11 (64-bit Installer)',
    hash: '6f36a65df228544edaa36111ea7ae5eac11baa754fae81adc62956953267586e'
  },
  win_msi: {
    filename: 'TruthBeacon_0.2.2_x64_en-US.msi',
    size: '12 MB',
    arch: 'Windows 64-bit (Enterprise WiX MSI)',
    hash: '436d9f7be2f5776455e9fe06b111c079c703f184ce56c9bc5ee570ea3e64aaad'
  },
  win_portable: {
    filename: 'TruthBeacon-Portable.exe',
    size: '24 MB',
    arch: 'Windows 64-bit (Zero-install Portable)',
    hash: '03858df78e645d9c3d38ea1ac0c146d86e8aa843f22fcba8678f59d39985a098'
  },
  linux_appimage: {
    filename: 'TruthBeacon_0.2.2_amd64.AppImage',
    size: '86 MB',
    arch: 'Linux x86_64 (Universal AppImage)',
    label: 'Download for Linux',
    sub: 'x86_64 AppImage (Any Distro)',
    hash: 'ec3203b01db76b58b2c55311ea5fdbfeca4c97111cc8207073038c382ca9d3c3'
  },
  linux_deb: {
    filename: 'TruthBeacon_0.2.2_amd64.deb',
    size: '14 MB',
    arch: 'Debian / Ubuntu / Pop!_OS',
    hash: '3173c92fa36016171fa7d1efabeb2b1c67b82e7ad2e490f6426d7bbebbb94136'
  }
};

// 1. Detect User OS and Update Hero Primary Button
function initOSDetection() {
  const ua = navigator.userAgent || '';
  const platform = navigator.platform || '';
  let os = 'mac'; // default
  let detectedName = 'macOS';

  if (/Win/i.test(platform) || /Windows/i.test(ua)) {
    os = 'win';
    detectedName = 'Windows (64-bit)';
  } else if (/Linux/i.test(platform) || /Linux/i.test(ua)) {
    os = 'linux';
    detectedName = 'Linux (x86_64)';
  } else if (/Mac/i.test(platform) || /Macintosh/i.test(ua)) {
    os = 'mac';
    detectedName = 'macOS (Universal 2)';
  }

  const primaryBtn = document.getElementById('hero-primary-download-btn');
  const osLabel = document.getElementById('hero-detected-os');
  const metaSize = document.getElementById('hero-meta-size');
  const metaArch = document.getElementById('hero-meta-arch');
  const metaFormat = document.getElementById('hero-meta-format');

  let targetAsset;
  if (os === 'win') {
    targetAsset = ASSETS.win_exe;
  } else if (os === 'linux') {
    targetAsset = ASSETS.linux_appimage;
  } else {
    targetAsset = ASSETS.mac_dmg;
  }

  if (primaryBtn) {
    primaryBtn.href = RELEASE_BASE_URL + targetAsset.filename;
    primaryBtn.setAttribute('data-filename', targetAsset.filename);
    const btnLabelEl = primaryBtn.querySelector('.btn-label-text');
    if (btnLabelEl) btnLabelEl.textContent = targetAsset.label;
  }

  if (osLabel) osLabel.textContent = detectedName;
  if (metaSize) metaSize.textContent = targetAsset.size;
  if (metaArch) metaArch.textContent = targetAsset.arch;
  if (metaFormat) metaFormat.textContent = targetAsset.filename.split('.').pop().toUpperCase();
}

// 2. Platform Switcher Tabs (macOS, Windows, Linux, All)
function initPlatformTabs() {
  const tabs = document.querySelectorAll('.platform-tabs .tab-btn');
  const cards = document.querySelectorAll('.downloads-grid .download-card');

  tabs.forEach(tab => {
    tab.addEventListener('click', () => {
      tabs.forEach(t => t.classList.remove('active'));
      tab.classList.add('active');

      const filter = tab.getAttribute('data-platform');
      cards.forEach(card => {
        const cardPlatform = card.getAttribute('data-platform');
        if (filter === 'all' || cardPlatform === filter) {
          card.style.display = 'flex';
        } else {
          card.style.display = 'none';
        }
      });
    });
  });
}

// 3. Verification Terminal Snippets Tab Switcher
const VERIFY_SNIPPETS = {
  macos: {
    title: 'terminal — zsh (macOS)',
    code: `# Verify SHA-256 Checksum on macOS\nshasum -a 256 TruthBeacon_0.2.2_universal.dmg\n\n# Expected Output:\n# c450fa110a54f2d64eab14763b8d9a7505f7bb0100c95417eca0d185c713a629  TruthBeacon_0.2.2_universal.dmg`
  },
  linux: {
    title: 'bash — terminal (Linux)',
    code: `# Verify SHA-256 Checksum on Linux\nsha256sum TruthBeacon_0.2.2_amd64.AppImage\n\n# Expected Output:\n# ec3203b01db76b58b2c55311ea5fdbfeca4c97111cc8207073038c382ca9d3c3  TruthBeacon_0.2.2_amd64.AppImage`
  },
  windows: {
    title: 'PowerShell — Windows 10 / 11',
    code: `# Verify SHA-256 Checksum in Windows PowerShell\nGet-FileHash TruthBeacon_0.2.2_x64-setup.exe -Algorithm SHA256\n\n# Expected Output:\n# 6F36A65DF228544EDAA36111EA7AE5EAC11BAA754FAE81ADC62956953267586E`
  },
  gpg: {
    title: 'terminal — GnuPG Detached Signature',
    code: `# Download SHA256SUMS and signature:\ncurl -LO https://github.com/boredpolymath/truth-beacon/releases/download/v0.2.2/SHA256SUMS.txt\ncurl -LO https://github.com/boredpolymath/truth-beacon/releases/download/v0.2.2/SHA256SUMS.txt.asc\n\n# Verify GPG Detached Signature:\ngpg --verify SHA256SUMS.txt.asc SHA256SUMS.txt\n\n# Batch check all files:\nshasum -a 256 -c SHA256SUMS.txt`
  }
};

function initVerifyTabs() {
  const tabs = document.querySelectorAll('.verify-nav .verify-tab-btn');
  const codeEl = document.getElementById('verify-code-content');
  const titleEl = document.getElementById('terminal-title-text');

  tabs.forEach(tab => {
    tab.addEventListener('click', () => {
      tabs.forEach(t => t.classList.remove('active'));
      tab.classList.add('active');

      const key = tab.getAttribute('data-verify-tab');
      if (VERIFY_SNIPPETS[key] && codeEl) {
        codeEl.textContent = VERIFY_SNIPPETS[key].code;
        if (titleEl) titleEl.textContent = VERIFY_SNIPPETS[key].title;
      }
    });
  });

  const copyCodeBtn = document.getElementById('btn-copy-terminal-code');
  if (copyCodeBtn && codeEl) {
    copyCodeBtn.addEventListener('click', () => {
      navigator.clipboard.writeText(codeEl.textContent).then(() => {
        showToast('Command copied to clipboard!');
      });
    });
  }
}

// 4. Feature Showcase Tab Switcher
const SHOWCASE_ITEMS = {
  protection: {
    title: 'Active Protection Dashboard',
    desc: 'Real-time telemetry and continuous threat detection. Visual shield status indicates real-time memory-safe daemon integrity.',
    points: [
      'Visual green shield heartbeat verifying zero-privilege daemon status',
      'Real-time count of total scans, quarantined payloads, and threats blocked',
      'Instant toggle for live Discord client memory & asset hooks'
    ],
    imgSrc: 'assets/screenshot_protected.png',
    imgAlt: 'TruthBeacon Active Protection Dashboard'
  },
  alerts: {
    title: 'Instant Threat Isolation & Alerts',
    desc: 'Automated defense against token grabbers, malicious Discord Nitro scams, homoglyph impersonations, and avatar cloning.',
    points: [
      'Comprehensive threat severity classification (Critical, Warning, Info)',
      'Deterministic isolation of malicious webhooks and hijacked process sockets',
      'Direct one-click threat review with payload signatures and stack traces'
    ],
    imgSrc: 'assets/screenshot_alerts.png',
    imgAlt: 'TruthBeacon Threat Alerts View'
  },
  activity: {
    title: 'Audit-Grade Activity Journal',
    desc: 'Full immutable local logging of all inspected events, hashes, and daemon diagnostics without any external telemetry.',
    points: [
      'Sub-millisecond latency timestamps with category tags',
      'Filterable by severity level, threat category, and timestamp',
      'Exportable audit trail in JSON & CSV for security team investigations'
    ],
    imgSrc: 'assets/screenshot_activity_log.png',
    imgAlt: 'TruthBeacon Activity Log View'
  },
  discord: {
    title: 'Frictionless Discord Integration',
    desc: 'Effortless setup guide providing immediate out-of-the-box protection for Discord Stable, PTB, Canary, and Discord Development builds.',
    points: [
      'Automatic detection of installed Discord client release channels',
      'Guided configuration with step-by-step security hardening instructions',
      'Zero modification of official Discord binaries or API credentials'
    ],
    imgSrc: 'assets/screenshot_discord_setup.png',
    imgAlt: 'TruthBeacon Discord Setup Guide'
  }
};

function initShowcaseTabs() {
  const tabs = document.querySelectorAll('.showcase-tabs .showcase-tab');
  const titleEl = document.getElementById('showcase-item-title');
  const descEl = document.getElementById('showcase-item-desc');
  const pointsEl = document.getElementById('showcase-item-points');
  const imgEl = document.getElementById('showcase-item-img');

  tabs.forEach(tab => {
    tab.addEventListener('click', () => {
      tabs.forEach(t => t.classList.remove('active'));
      tab.classList.add('active');

      const key = tab.getAttribute('data-showcase');
      const item = SHOWCASE_ITEMS[key];
      if (!item) return;

      if (titleEl) titleEl.textContent = item.title;
      if (descEl) descEl.textContent = item.desc;
      if (imgEl) {
        imgEl.src = item.imgSrc;
        imgEl.alt = item.imgAlt;
      }
      if (pointsEl) {
        pointsEl.innerHTML = item.points
          .map(pt => `<li class="showcase-point"><span class="showcase-point-icon">✓</span><span>${pt}</span></li>`)
          .join('');
      }
    });
  });
}

// 5. Copy Buttons & Toast Notifications
function initCopyButtons() {
  document.querySelectorAll('.btn-copy-checksum').forEach(btn => {
    btn.addEventListener('click', () => {
      const hash = btn.getAttribute('data-hash');
      if (!hash) return;

      navigator.clipboard.writeText(hash).then(() => {
        showToast('SHA-256 Checksum copied!');
        const origSvg = btn.innerHTML;
        btn.innerHTML = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#23a55a" stroke-width="2.5"><polyline points="20 6 9 17 4 12"></polyline></svg>`;
        setTimeout(() => {
          btn.innerHTML = origSvg;
        }, 1800);
      }).catch(err => {
        console.error('Clipboard copy failed:', err);
      });
    });
  });
}

function showToast(message) {
  let toast = document.getElementById('site-toast');
  if (!toast) {
    toast = document.createElement('div');
    toast.id = 'site-toast';
    toast.className = 'toast';
    document.body.appendChild(toast);
  }

  toast.innerHTML = `<span class="toast-icon">✓</span> <span>${message}</span>`;
  toast.classList.add('show');

  setTimeout(() => {
    toast.classList.remove('show');
  }, 2400);
}

// 6. Screenshot Lightbox
function initLightbox() {
  const modal = document.getElementById('lightbox-modal');
  const modalImg = document.getElementById('lightbox-img');
  const closeBtn = document.getElementById('lightbox-close');
  const previewFrame = document.querySelector('.showcase-preview-frame');

  if (!modal || !modalImg) return;

  if (previewFrame) {
    previewFrame.addEventListener('click', () => {
      const currentImg = document.getElementById('showcase-item-img');
      if (currentImg) {
        modalImg.src = currentImg.src;
        modal.classList.add('open');
      }
    });
  }

  const closeModal = () => modal.classList.remove('open');

  if (closeBtn) closeBtn.addEventListener('click', closeModal);
  modal.addEventListener('click', (e) => {
    if (e.target === modal) closeModal();
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && modal.classList.contains('open')) closeModal();
  });
}
