# TruthBeacon: Desktop Identity Ground-Truth & Impersonation Prevention Console

<p align="center">
  <img src="ui/assets/app_preview.png" alt="TruthBeacon Desktop Application Preview" width="100%" style="border-radius: 8px; box-shadow: 0 12px 32px rgba(0,0,0,0.35);">
</p>

---

## 1. Overview & Core Philosophy

**TruthBeacon** is a privacy-first identity verification and impersonation mitigation desktop console built for community leaders, digital creators, pastors, and server moderators.

Rather than adopting punitive or aggressive cybersecurity tropes, TruthBeacon approaches safety through **community stewardship and kinship preservation**. It acts as a watchful, quiet guardian standing by the door:

* **Remembers Your Real Team:** Tracks canonical accounts, server nicknames, role hierarchies, and perceptual avatar hashes.
* **Evaluates Lookalikes in Milliseconds:** Normalizes Unicode homoglyphs, strips zero-width spaces, and scores string distances with sub-50ms heuristic budgets.
* **Protects Local Sovereignty:** Operates **100% locally**. Zero cloud tracking, zero telemetry, and zero chat data ever leaves your host machine.
* **Prevents Runaway Moderation:** Features a built-in sliding-window **circuit breaker** safety brake to halt automated mass actions before mistakes happen.

> [!NOTE]
> **Zero Telemetry Guarantee:** All ground-truth profiles, audit logs, and perceptual image hashes remain encrypted in your local SQLite database and OS Keychain.

---

## 2. Key Features

| Capability | Description |
| :--- | :--- |
| **2x2 Triage Grid** | Clean, Discord-familiar side-by-side inspection cards comparing official protected members against flagged lookalike accounts. |
| **Instant Action Shortcuts** | One-click keyboard-driven resolution: `[B]` Ban Imposter, `[W]` Allow Known Alt, `[E]` Restrict Account, `[D]` Ignore Safe. |
| **Multi-Vector Detection** | Unicode NFKD decomposition, Cyrillic/Greek homoglyph detection, Jaro-Winkler scoring, and DCT visual avatar hashing. |
| **Discord Gateway v10 Daemon** | Real-time WebSocket connection to Discord Gateway handling `GUILD_MEMBERS` events with exponential backoff resilience. |
| **Operational Safety Brake** | Sliding-window circuit breaker (max 5 actions / 60s) to eliminate moderation loops or false-positive cascades. |
| **Multi-Resolution System Tray** | Native macOS menu bar and Windows system tray presence with instant status indicators. |

---

## 3. Technology Stack

* **Desktop Application Layer:** [Tauri v2](https://v2.tauri.app/) (macOS, Windows, Linux)
* **Backend Core (Rust 2021):**
  * **Credential Store:** Native OS Keychain (`keyring`)
  * **Local Database:** SQLite with Write-Ahead Logging (`rusqlite`)
  * **Perceptual Hashing:** Discrete Cosine Transform 64-bit visual hashing (`img_hash`, `image`)
  * **String Metrics:** Jaro-Winkler & Damerau-Levenshtein distance algorithms (`strsim`)
  * **Unicode Deobfuscation:** NFKD decomposition & transliteration (`unicode-normalization`, `deunicode`)
  * **Concurrency & Gateway:** Asynchronous Tokio runtime with WebSockets (`tokio`, `tokio-tungstenite`)
* **Presentation Layer:**
  * Semantic HTML5 & Modern Vanilla CSS (Discord-dark theme, curated HSL color tokens, responsive cards)
  * Native IPC bindings (`window.__TAURI__`) with automatic simulation fallback

---

## 4. Directory Layout

```
truth-beacon/
├── README.md                       # System architecture, preview, and guide
├── truthbeacon.config.json         # Engine configuration & detection thresholds
├── docs/                           # Architecture specifications & system envelopes
│   ├── assets/
│   │   └── app_preview.png         # Desktop application preview screenshot
│   └── 01-scope-boundaries-...     # Detailed specifications & STRIDE analysis
├── src-tauri/                      # Native Tauri v2 Rust backend
│   ├── Cargo.toml                  # Dependencies & crate configuration
│   ├── build.rs                    # Tauri build orchestrator
│   ├── tauri.conf.json             # Window settings, CSP policies, and tray icon spec
│   ├── icons/                      # Multi-resolution application & tray icons
│   │   ├── tray_icon.png           # 64x64 Retina system tray icon
│   │   ├── icon.icns               # macOS multi-size bundle (16x16 to 1024x1024)
│   │   └── icon.ico                # Windows multi-size tray icon
│   └── src/
│       ├── main.rs                 # Native binary entry point
│       ├── lib.rs                  # Library crate & Tauri command registrar
│       ├── models/                 # Benchmarks, incidents, and audit logs
│       ├── credentials/            # OS Keychain integration & token validator
│       ├── vault/                  # Ground-truth benchmark management & avatar sync
│       ├── gateway/                # Discord Gateway v10 client & background daemon
│       ├── detection/              # Multi-vector heuristic scoring engine
│       ├── circuit_breaker/        # Closed/Open/HalfOpen safety brake state machine
│       ├── storage/                # SQLite connection manager & migrations
│       └── commands/               # Tauri IPC command dispatchers
└── ui/                             # Responsive web presentation layer
    ├── index.html                  # Accessible layout & centered tab navigation
    ├── css/
    │   └── styles.css              # Discord-familiar styling & 2x2 grid system
    ├── js/
    │   ├── app.js                  # UI controllers, keyboard shortcuts & renderers
    │   ├── state.js                # Central reactive state store
    │   ├── ipc.js                  # Tauri IPC bridge & preview simulation
    │   └── mock_data.js            # Initial benchmarks & sample incident scenarios
    └── assets/
        ├── truthbeacon_wordmark.png # Transparent top-left brand wordmark
        ├── truthbeacon_emblem.png  # High-resolution colored medallion badge
        ├── tray_icon.png           # System tray icon asset
        └── avatars/                # Local benchmark & suspect test avatars
```

---

## 5. Quick Start & Development

### Prerequisites
* **Rust Toolchain:** 1.75+ (`cargo`, `rustc`)
* **OS Build Essentials:** Xcode Command Line Tools (macOS) or Visual Studio C++ Build Tools (Windows)

### Running the Desktop Application
```bash
# Navigate to the native backend
cd src-tauri

# Run the desktop app in development mode
cargo run
```

### Building the Production Release
```bash
cd src-tauri
cargo build --release
```

### Running Backend Unit & Integration Tests
```bash
cd src-tauri
# Executes all 133 automated tests across detection, gateway, and vault
cargo test
```

### Static UI Browser Preview
The presentation layer in `ui/` can also be previewed directly via any local static web server:
```bash
python3 -m http.server 8080 --directory ui
```
Open [http://localhost:8080](http://localhost:8080) to test the UI in simulation mode.

---

## 6. License & Stewardship

TruthBeacon is engineered by **Orange Heart Industries**.

* **Community Edition:** Free for volunteer-run clubs, open-source communities, and indie servers.
* **Charity & Faith Stewardship Grant:** 100% free access to all advanced features for registered non-profits, charities, shelters, and faith communities.
