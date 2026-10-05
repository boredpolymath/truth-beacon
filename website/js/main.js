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

// Release Asset Configuration (v0.1.1)
const RELEASE_BASE_URL = 'https://github.com/orangeheart-industries/truth-beacon/releases/download/v0.1.1/';
const ASSETS = {
  mac_dmg: {
    filename: 'TruthBeacon_0.1.1_universal.dmg',
    size: '27 MB',
    arch: 'Universal 2 (Apple Silicon & Intel)',
    label: 'Download for macOS',
    sub: 'Universal DMG (macOS 10.15+)',
    hash: '52131ee3ebb13fc8a07ba3d10eb456cc095735dcad8bbb01b7b62f165536a8cb'
  },
  mac_zip: {
    filename: 'TruthBeacon_0.1.1_macos_universal.zip',
    size: '26 MB',
    arch: 'Universal 2 (.app Portable)',
    hash: '537e9c9209807901570789d723d8d0b50f21a5652ba29b34d6c23878378833f5'
  },
  mac_bin: {
    filename: 'truth-beacon-universal',
    size: '53 MB',
    arch: 'Mach-O Universal Fat Binary',
    hash: '1e78b07176204f93ded2d42eb7a7f96bebf66d208fb40d3e647d57d132b93417'
  },
  win_exe: {
    filename: 'TruthBeacon_0.1.1_x64-setup.exe',
    size: '9.5 MB',
    arch: 'Windows 64-bit (NSIS Installer)',
    label: 'Download for Windows',
    sub: 'Windows 10 / 11 (64-bit Installer)',
    hash: '00c3ed6f9788e5b90a96c3e5e2c78359bdf8fb3688b602b4ee862506a822fee9'
  },
  win_msi: {
    filename: 'TruthBeacon_0.1.1_x64_en-US.msi',
    size: '12 MB',
    arch: 'Windows 64-bit (Enterprise WiX MSI)',
    hash: '1dc49e95c08abc40f74ee80c2a00bd15eb9f328188ea7c13913208c92d5f6228'
  },
  win_portable: {
    filename: 'TruthBeacon-Portable.exe',
    size: '24 MB',
    arch: 'Windows 64-bit (Zero-install Portable)',
    hash: 'ab2d95433a5e348ebfe4f84a318f2eeaa9138a3a25f825adb5eec76aee779543'
  },
  linux_appimage: {
    filename: 'TruthBeacon_0.1.1_amd64.AppImage',
    size: '86 MB',
    arch: 'Linux x86_64 (Universal AppImage)',
    label: 'Download for Linux',
    sub: 'x86_64 AppImage (Any Distro)',
    hash: '146696070566d33444bdb51a4f22bdc7a1610a3e8e3cfa1632f0eee4d2262e52'
  },
  linux_deb: {
    filename: 'TruthBeacon_0.1.1_amd64.deb',
    size: '14 MB',
    arch: 'Debian / Ubuntu / Pop!_OS',
    hash: 'fde939e596f7389175f00b782e202c08483ff174e332df7705a8a8646eeb510e'
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
    code: `# Verify SHA-256 Checksum on macOS\nshasum -a 256 TruthBeacon_0.1.1_universal.dmg\n\n# Expected Output:\n# 52131ee3ebb13fc8a07ba3d10eb456cc095735dcad8bbb01b7b62f165536a8cb  TruthBeacon_0.1.1_universal.dmg`
  },
  linux: {
    title: 'bash — terminal (Linux)',
    code: `# Verify SHA-256 Checksum on Linux\nsha256sum TruthBeacon_0.1.1_amd64.AppImage\n\n# Expected Output:\n# 146696070566d33444bdb51a4f22bdc7a1610a3e8e3cfa1632f0eee4d2262e52  TruthBeacon_0.1.1_amd64.AppImage`
  },
  windows: {
    title: 'PowerShell — Windows 10 / 11',
    code: `# Verify SHA-256 Checksum in Windows PowerShell\nGet-FileHash TruthBeacon_0.1.1_x64-setup.exe -Algorithm SHA256\n\n# Expected Output:\n# 00C3ED6F9788E5B90A96C3E5E2C78359BDF8FB3688B602B4EE862506A822FEE9`
  },
  gpg: {
    title: 'terminal — GnuPG Detached Signature',
    code: `# Download SHA256SUMS and signature:\ncurl -LO https://github.com/orangeheart-industries/truth-beacon/releases/download/v0.1.1/SHA256SUMS.txt\ncurl -LO https://github.com/orangeheart-industries/truth-beacon/releases/download/v0.1.1/SHA256SUMS.txt.asc\n\n# Verify GPG Detached Signature:\ngpg --verify SHA256SUMS.txt.asc SHA256SUMS.txt\n\n# Batch check all files:\nshasum -a 256 -c SHA256SUMS.txt`
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
