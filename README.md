# TruthBeacon
### Desktop Identity Ground-Truth & Impersonation Prevention Console
**Crafted with care by Orange Heart Industries**

---

## 1. Overview & Core Philosophy

**TruthBeacon** is an authentic identity verification and impersonation mitigation desktop application designed for community leaders, digital creators, pastors, and server administrators.

Rather than adopting aggressive or punitive cybersecurity tropes (firewalls, countermeasures, war rooms), TruthBeacon approaches online safety through the lens of **community stewardship and kinship preservation**. It acts as a watchful, quiet friend standing by the door:
* **Remembers Your Real Team:** Tracks canonical accounts, server nicknames, role hierarchies, and perceptual avatar hashes.
* **Evaluates Lookalikes in Milliseconds:** Normalizes Unicode homoglyphs, strips invisible spacing, and scores Damerau-Levenshtein / Jaro-Winkler string distances.
* **Protects Local Sovereignty:** Operates **100% locally**. Zero telemetry, zero analytics tracking, and zero member chat data leaves your host machine.
* **Prevents Runaway Moderation:** Features a built-in sliding-window **circuit breaker** to halt automated mass actions before mistakes happen.

---

## 2. Architecture & Technology Stack

TruthBeacon leverages a hybrid architecture combining a memory-safe, low-overhead native Rust backend with a modern, responsive web presentation layer via **Tauri v2**:

* **Desktop Framework:** [Tauri v2](https://v2.tauri.app/)
* **Backend Core:** Rust (Edition 2021)
  * Credential Store: Native OS Keychain (`keyring`)
  * Local Database: SQLite with Write-Ahead Logging (`rusqlite`)
  * Visual Perceptual Hashing: Discrete Cosine Transform 64-bit (`img_hash`, `image`)
  * String Metric Analysis: Jaro-Winkler & Damerau-Levenshtein (`strsim`)
  * Unicode Deobfuscation: NFKD decomposition & transliteration (`unicode-normalization`, `deunicode`)
  * Concurrency & Gateway: Asynchronous Tokio runtime (`tokio`, `tokio-tungstenite`)
* **Presentation Layer:**
  * Semantic HTML5 & Modern Vanilla CSS (Deep slate surfaces, Orange Heart warm accents, glassmorphic inspection cards)
  * Native IPC message bindings (`window.__TAURI__`) with fallback preview simulation

---

## 3. Directory Layout & Scaffolding

```
truth-beacon/
├── .gitignore                      # Git ignore rules for Cargo, SQLite, and OS artifacts
├── README.md                       # System architecture and documentation
├── truthbeacon.config.json         # Default engine and threshold configuration
├── src-tauri/                      # Native Tauri v2 Rust backend
│   ├── Cargo.toml                  # Rust dependencies & metadata
│   ├── build.rs                    # Tauri build orchestrator
│   ├── tauri.conf.json             # Tauri window, CSP, tray, and capability config
│   ├── capabilities/
│   │   └── default.json            # Tauri v2 permission capabilities
│   └── src/
│       ├── main.rs                 # Native binary entry point
│       ├── lib.rs                  # Library crate & Tauri command registrar
│       ├── models/                 # Shared data models
│       │   ├── mod.rs
│       │   ├── benchmark.rs        # Canonical benchmark definitions & tags
│       │   ├── incident.rs         # Triage incidents & risk tiers
│       │   └── audit.rs            # Immutable local audit logs
│       ├── credentials/            # OS Keychain integration (macOS/Win/Linux)
│       │   └── mod.rs
│       ├── vault/                  # Benchmark identity registry manager
│       │   └── mod.rs
│       ├── gateway/                # Discord Gateway v10 WebSocket listener
│       │   └── mod.rs
│       ├── detection/              # Multi-vector heuristic detection engine
│       │   ├── mod.rs              # Compound threat adjudicator
│       │   ├── unicode.rs          # Homoglyph transliteration & NFKD cleaning
│       │   ├── metrics.rs          # String similarity metrics (Jaro/Levenshtein)
│       │   └── perceptual_hash.rs  # DCT avatar visual hashing & Hamming distance
│       ├── circuit_breaker/        # Sliding-window moderation safety brake
│       │   └── mod.rs
│       ├── storage/                # SQLite connection manager & WAL pragmas
│       │   ├── mod.rs
│       │   └── migrations.rs       # Embedded database schema migrations
│       └── commands/               # Tauri IPC command handlers
│           └── mod.rs
└── ui/                             # Responsive web presentation layer
    ├── index.html                  # Accessible layout & tab navigation
    ├── css/
    │   └── styles.css              # Orange Heart design system & dark aesthetics
    ├── js/
    │   ├── app.js                  # UI controllers & event delegation
    │   ├── state.js                # Central reactive state store
    │   ├── ipc.js                  # Tauri IPC bridge & preview fallback
    │   └── mock_data.js            # Initial ground-truth benchmarks & sample incidents
    └── assets/
        └── logo.svg                # Orange Heart Beacon vector mark
```

---

## 4. Key Subsystems

### 4.1 Canonical Benchmark Vault
Maintains protected ground-truth profiles designated by server administrators. Each benchmark entry tracks canonical usernames, server nicknames, assigned roles, tags (e.g. `Core Staff`, `Authorized Alt`), and 64-bit DCT perceptual avatar hashes.

### 4.2 Multi-Vector Detection Pipeline
1. **Unicode Deobfuscation:** Strips zero-width characters, bidirectional overrides, and control codes. Transliterates Cyrillic, Greek, and mathematical lookalike homoglyphs.
2. **String Distance Scoring:** Calculates composite Jaro-Winkler and normalized Damerau-Levenshtein distance against protected benchmarks.
3. **Visual Perceptual Hashing:** Ingests incoming avatar buffers and calculates Hamming distance against cached official hashes.
4. **Snowflake Account Age Correlation:** Differentiates freshly minted scam accounts from mature community members.
5. **Compound Risk Adjudication:** Flags discrepancies into `Standard`, `Notable`, `Elevated`, or `Critical` risk tiers.

### 4.3 Operational Circuit Breaker
Enforces an in-memory sliding window (e.g., maximum 5 actions per 60 seconds). If outbound mitigation traffic crosses the limit, the safety brake trips, requiring manual operator confirmation and displaying a cooldown countdown.

### 4.4 Data Sovereignty & Eradication
All operational logs and benchmark records reside in a local SQLite database (`truthbeacon.local.db`) with Write-Ahead Logging. A permanent, one-click local data eradication function safely vacuums and deletes all local data and keychain secrets on demand.

---

## 5. Development & Verification

### Prerequisites
* Rust toolchain (1.75+ or newer recommended, `cargo`, `rustc`)
* Operating system build essentials for Tauri

### Rust Backend Check
```bash
cd src-tauri
cargo check
```

### Static UI Preview
The presentation layer in `ui/` can be previewed directly in any web browser or via a static file server:
```bash
python3 -m http.server 8000 --directory ui
```
Open `http://localhost:8000` to inspect the side-by-side triage console, simulator sandbox, benchmark vault, and data sovereignty settings.

---

## 6. License & Stewardship

TruthBeacon is developed by **Orange Heart Industries**.
* **Community Edition:** Free for volunteer-run clubs, open-source communities, and indie servers.
* **Charity & Faith Stewardship Grant:** 100% free access to all advanced features for registered non-profits, charities, shelters, and faith communities.
