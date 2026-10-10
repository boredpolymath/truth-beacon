# TruthBeacon v0.3.0 — Universal Community Moderation Architecture, Manifest Cryptographic Sync & Production Release
**Orange Heart Industries — Identity Ground-Truth & Community Stewardship**

---

### Overview
TruthBeacon v0.3.0 delivers a fully generalized community moderation architecture, eradicating legacy placeholder defaults in favor of standard universal Discord administration roles (`Server Owner / Administrator`, `Lead Moderator / Staff`, `Community Moderator`, `Verified VIP / Core Member`, `Official Bot / System Service`, `Media / Presentation Device`). Additionally, this release synchronizes public distribution manifests, resolves website checksum verification mismatches, establishes continuous cryptographic asset alignment across GitHub Pages and release artifacts, and verifies 100% test compliance across all 208 test suites.

---

### What's New & Hardened in v0.3.0

#### 1. Universal Community Moderation Architecture
- **Sanitized Administration Roles**: Fully neutralized role selectors and guidance terminology across the Protect Member modal (`Server Owner / Administrator`, `Lead Moderator / Staff`, `Community Moderator`, `Verified VIP / Core Member`, `Official Bot / System Service`).
- **Clean Alt Device Options**: Replaced device examples with generalized operational options (`Media / Presentation Device`, `Staff / Volunteer Rotation Account`, `Secondary Mobile Phone Account`).
- **Zero Placeholder Default Leakage**: Removed hardcoded pre-filled tag values (`value="Core Staff, Verified VIP"`) in favor of clean form placeholders, preventing unvetted data from being saved inadvertently.
- **Normalized Mock & Offline Fallbacks**: Aligned local offline IPC fallbacks and demo fixtures to professional standard identities (`Alex Rivera`, `CommunityHelpDesk`, `Primary Community Server`).

#### 2. Cryptographic Manifest & Distribution Asset Synchronization
- **Website Checksum Alignment**: Fixed checksum desynchronizations on the public website for macOS Universal DMG and macOS Portable ZIP archives.
- **Dual-License URL Remediation**: Repointed documentation and website links to `LICENSE-MIT` to prevent 404 navigation errors.
- **Automated Two-Way Manifest Sync**: Enhanced `scripts/sign_release_artifacts.py` to automatically mirror `SHA256SUMS.txt`, `RELEASE_MANIFEST.md`, and OpenPGP detached signatures (`.asc`) directly into `website/assets/`.

#### 3. Verification & Compliance
- **100% Pass Rate**: Maintained 208/208 tests passing in `cargo test --all-targets`.
- **Zero Warnings**: Verified with `cargo clippy -- -D warnings`.
- **5/5 Packaging Stages**: Verified drag-and-drop DMG layouts, icon packs, and FreeDesktop desktop entries.
- **Red Team Mitigation**: 100% mitigation rate across script-mixed homoglyphs and DCT perceptual clone evasion attempts.

---

# TruthBeacon v0.2.2 — Hermetic Test Storage Isolation, Professional UI Guidance & Pristine Clean-Slate Architecture
**Orange Heart Industries — Identity Ground-Truth & Community Stewardship**

---

### Overview
TruthBeacon v0.2.2 introduces hermetic database storage isolation for development and testing environments, sanitizes all default guidance placeholders with neutral, professional identity examples, and ensures an immaculate clean-slate deployment experience. Automated test suites (`cargo test`) now run in completely quarantined sandbox directories (`temp_dir()`), preventing mock records from accumulating in operator workstations or leaking into production UI installations.

---

### What's New & Hardened in v0.2.2

#### 1. Hermetic Storage & Test Sandbox Isolation
- **Quarantined Database Resolution**: In test mode (`#[cfg(test)]`), the SQLite engine (`StorageManager::default_db_path()`) automatically routes storage to a temporary directory (`truthbeacon_test_env`), ensuring automated test suites never write mock benchmarks or incidents into the operator's local `~/.truthbeacon/` database.
- **Quarantined Vault Directory Resolution**: Credential vault storage paths (`vault_dir()`) similarly default to isolated test directories when running under test harnesses.
- **Self-Cleaning Test Fixtures**: Added explicit post-test teardown and deletion routines across `commands/mod.rs` test cases, guaranteeing that repeated test executions leave zero residual state.
- **Zero-Pollution Verified**: Verified via file modification timestamps that full automated test executions produce exactly zero writes to user machine databases.

#### 2. Sanitized & Professional Guidance Placeholders
- **Neutral Community Member Guidance**: Replaced developer-specific placeholders in modal forms with neutral, professional examples:
  - Username: `placeholder="e.g. AlexMorgan"`
  - Display Name: `placeholder="e.g. Alex (Community Lead)"`
  - Secondary Account Justification: `placeholder="e.g. Confirmed with team member in person; approved mobile device account"`
- **Neutralized Alert Notification Fallbacks**: Sanitized desktop notification test toast fallback attributes to neutral leadership terminology.

#### 3. Production Clean-Slate Assurance
- **Cold Launch Verification**: Verified cold application launch against an empty database correctly renders zero alerts, zero benchmarks, and zero audit logs with high-clarity empty-state prompts.
- **All 208 Tests Passing**: Maintained 100% pass rate across the full test suite with clean Clippy linting and formatting compliance.

---

# TruthBeacon v0.2.1 — Multi-Guild Benchmark Persistence, Native IPC Hardening, Dynamic System Tray & Live Diagnostics Polling
**Orange Heart Industries — Identity Ground-Truth & Community Stewardship**

---

### Overview
TruthBeacon v0.2.1 delivers critical multi-guild community administration, native IPC defense-in-depth, proactive system supervision, and accessibility hardening. This release integrates SQLite-backed benchmark persistence with guild isolation, mitigates Unicode homoglyphs and Zalgo-combining marks via Unicode Technical Report #39 canonical skeletons, implements live system tray status reflection, sanitizes sensitive bot credentials from logs, introduces real-time health diagnostics polling, and establishes multi-threaded credential vault test isolation, expanding automated regression test coverage to **208 passing tests** (100% pass rate).

---

### What's New & Hardened in v0.2.1

#### 1. Multi-Guild Benchmark Persistence & Isolation (Phase 4)
- **Authoritative SQLite Benchmarks**: Community steward benchmark records are now durably persisted into SQLite with automatic cache hydration upon startup.
- **Strict Guild-Level Access Separation**: Prevents cross-guild benchmark leakage; mutations and lookups enforce explicit guild ID boundaries.
- **Duplicate & Validation Guardrails**: Rejects duplicate benchmark additions, validates inputs, and exposes live backend status reporting (`system_status`).

#### 2. Native Tauri IPC Hardening & Unicode TR #39 Defenses (Phase 5)
- **Parameter Validation & Bounds**: IPC command boundaries strictly enforce limits on guild IDs, benchmark IDs, and image payloads (8 MB max, 4096px bounds).
- **UTR #39 Skeletons & Homoglyph Unmasking**: Uses canonical confusable mapping tables to detect cross-script spoofing attacks (e.g. Cyrillic/Greek lookalikes).
- **Zalgo & Invisible Character Stripping**: Filters zero-width spaces, bidirectional override formatting characters, and excessive combining marks.
- **Incident Anti-Replay & Consequential Action Guards**: Destructive moderation commands enforce operator attribution and reject repeated executions against already-mitigated incidents.

#### 3. Dynamic System Tray, Log Sanitization & Clean Shutdown (Phase 6)
- **Dynamic Tray Status Summary**: Multi-resolution system tray icon and menu reflect real-time backend protection state (Online, Degrading, Cooldown, Circuit Breaker).
- **Zero-Leak Log Sanitization**: Regex-driven log filters automatically mask Discord bot tokens (`Bot [REDACTED]`) and webhook URLs in all application traces and diagnostic exports.
- **Idempotent Graceful Shutdown**: Process termination coordinates clean SQLite WAL checkpoints and zero-fill memory wiping on shutdown.
- **Health Diagnostics Reporting**: Comprehensive system health reports verify storage integrity, gateway connectivity, and resource consumption.

#### 4. UI Focus Management, Hydration & Diagnostics Polling (Phase 7)
- **WCAG 2.1 AA Accessibility**: Full keyboard focus traps, accessible modal interactions, and screen-reader announcements across triage cards and modal dialogues.
- **Reactive Diagnostics Polling**: System health metrics continuously poll the native backend, displaying real-time memory usage, database status, and uptime.
- **Direct Vault State Hydration**: UI instantly synchronizes with the native credential vault and benchmark database upon launch.

#### 5. Multi-Threaded Test Isolation & Test Suite Expansion
- **Vault Cache Concurrency Isolation**: In-memory credential registry and token caches are completely flushed and thread-isolated under `#[cfg(test)]`, preventing cross-thread test contamination.
- **208 Passing Automated Tests**: Full suite expanded across credential vaults, SQLite engines, gateway resilience, perceptual hashing, IPC security, and lifecycle management with zero warnings under `-D warnings`.

---

# TruthBeacon v0.2.0 — Major Security, Resilience & Release Hardening
**Orange Heart Industries — Identity Ground-Truth & Community Stewardship**

---

### Overview
TruthBeacon v0.2.0 is a major security, resilience, testability, and release-hardening release. It addresses comprehensive application-security audit findings, eliminates trust assumptions between webview and native core, enforces domain-bound authenticated encryption on credentials, preserves database integrity under corruption, hardens multi-guild gateway connections, and expands automated regression coverage to 177 passing tests.

---

### What's New & Hardened in v0.2.0

#### 1. Database Integrity & Non-Destructive Forensic Quarantine
- **Pre-Flight Integrity Gate**: Startup now runs `PRAGMA integrity_check` before applying schema migrations, preventing migration runs on corrupted databases.
- **Safe Forensic Quarantine**: Corrupted databases and active WAL/SHM frames are automatically copied to a timestamped file (`<path>.corrupt.<timestamp>.db`) rather than being silently deleted or overwritten, preserving evidence for recovery.
- **Native Online Backup & Pruning**: Implemented `backup_to(&self, dest)` using SQLite's online backup API to take consistent snapshots without blocking concurrent readers, and added configurable data retention pruning.

#### 2. Authenticated Credential Vault with AAD Domain Binding
- **AES-256-GCM Domain Separation**: Vault payloads are authenticated using domain-bound Associated Data (`TruthBeacon:EncryptedVault:v1`), preventing cross-context ciphertext replay.
- **Tamper & Corruption Detection**: The vault loader strictly validates authentication tags and AAD context, returning explicit `VaultCorruptedOrTampered` errors on tampered bytes without overwriting corrupted payloads.
- **Argon2id Key Isolation**: Vault keys are derived via Argon2id with salt-scoped caching, preventing memory-key collisions in multi-threaded environments. Sensitive tokens are strictly redacted (`[REDACTED]`) in all logs and displays.

#### 3. Backend Moderation Authority & Incident Anti-Replay
- **Authoritative Database Validation**: The `resolve_incident` IPC command now resolves targets, guild associations, and incident statuses directly against SQLite rather than trusting webview parameters.
- **Anti-Replay Safeguards**: Moderation commands reject already-resolved incidents to eliminate command replay risks.
- **Operator Attribution Requirement**: Consequential actions (`Ban`, `Exclude`) strictly require a non-empty, identified operator.
- **Persistent Circuit Breaker**: Trip state and cooldowns are reconstructed from persisted audit logs on startup, preventing bypasses by restarting the application.

#### 4. Multi-Guild Gateway Lifecycle & Event Deduplication
- **Gateway Connection Registry**: Replaced single-guild startup logic with `GatewayConnectionRegistry`, automatically restoring daemons for all registered guilds while deduplicating shared bot tokens to a single resilient connection.
- **Event Deduplication**: Gateway Opcode 0 event dispatch deduplicates incoming events using an LRU session ring buffer, preventing double-processing on network reconnects.

#### 5. Resource Bounds & IPC Sandboxing
- **Avatar Memory Limits**: Remote avatar downloads and processing are constrained to 8 MB maximum buffer size, 4096 px maximum dimension, 16M maximum pixels, and a 10-second download timeout.
- **Tauri Capability Sandboxing**: Constrained `shell:allow-open` to trusted Discord and TruthBeacon GitHub URLs, and removed unnecessary origins from the webview CSP.

#### 6. Dependency Supply Chain Hardening
- **Documented Audit Policy**: Transitive dependency advisory `RUSTSEC-2023-0080` (in `transpose` via `img_hash` 8x8 DCT) is formally documented with bounds proofs in `.cargo/audit.toml`, enabling clean zero-flag `cargo audit` execution.

---

# TruthBeacon v0.1.5 — Official Release Notes
**Orange Heart Industries — Identity Ground-Truth & Community Stewardship**

---

### Overview
TruthBeacon v0.1.5 is a user-experience and interface alignment release that synchronizes both the standalone desktop security console and the official web distribution center with all recent architectural advancements:

1. **Complete UI Alignment with Machine-Bound AES-256-GCM Vault**: Modernized all desktop console badges, security enclave pills, input help notes, and interactive FAQ guidance to accurately describe the zero-prompt machine-bound AES-256-GCM credential vault (`0600` POSIX) replacing legacy OS keychain references.
2. **Auto-Updater Interface & Version Synchronization**: Elevated the desktop header version pill, update polling fallback routines, and website manifests to v0.1.5 with full Minisign Ed25519 cryptographic auto-updater catalog (`latest.json`) support.
3. **Distribution Portal & Architecture Showcase Refresh**: Updated the public website with dedicated highlights and architecture pillars for the zero-prompt vault and auto-updater engine, along with updated download matrices and verification snippets.
4. **Universal 2 Packaging & Cryptographic Integrity Verification**: Rebuilt and staged production Universal 2 macOS artifacts (`.dmg`, `.app.tar.gz`), updated detached signatures, and verified clean-slate launch with 150 passing unit and integration tests.

---

### What's New & Fixed in v0.1.5

#### 1. Desktop Console Interface Modernization
- **Credential Enclave Status Pill**: Replaced the legacy "OS Secure Enclave / Apple Keychain" pill in Discord Settings with the high-visibility **"Zero-Prompt AES-256-GCM Vault Active"** badge, reflecting local authenticated encryption with hardware entropy binding and strict owner-only (`0600`) POSIX permissions.
- **Form Field Security Guidance**: Clarified token storage security notes to reassure operators that bot credentials are encrypted locally with machine-bound AES-256-GCM and never sent to remote cloud infrastructure.
- **Credential Purge Tooltip**: Updated the disconnect button tooltip to explicitly reflect cryptographic zeroization and eradication from the local AES-256-GCM vault.
- **In-App FAQ Guidance**: Revised the token storage security FAQ entry to provide precise technical disclosure of the Ring AEAD AES-256-GCM scheme, SHA-256 HKDF key derivation, and zero-telemetry policy.

#### 2. Auto-Updater Engine & Version Bumping
- **Desktop Version Badge**: Synchronized the interactive update button in the desktop header to default to `v0.1.5`.
- **IPC Update Dispatcher**: Bumped IPC mock and fallback version values to `0.1.5`.
- **Catalog Synchronization**: Automated deployment of Tauri 2 `latest.json` updater catalogs across release directories and website assets.

#### 3. Web Distribution Portal Alignment
- **Feature Highlights & Architecture Pillars**: Added dedicated showcase cards for the **AES-256-GCM Machine Vault** and **Cross-Platform Auto-Updater Engine** on the official website.
- **Download Catalogs & Verification Snippets**: Updated all direct download links, terminal SHA-256 verification instructions, and GPG signature endpoints to point to v0.1.5 deliverables.

---

### Previous Highlights from v0.1.4

#### 1. Resolved Startup Tokio Reactor Panic on Credential Auto-Connect
- **Issue**: When bot credentials had been registered and stored in the secure vault, TruthBeacon automatically initiates background connection to the Discord Gateway daemon during startup. In v0.1.3, this async task was spawned using `tokio::spawn` inside Tauri's synchronous `.setup(|app| ...)` hook on the main thread. Because Tauri executes `setup` on the main thread outside of an active Tokio runtime context, `tokio::spawn` panicked immediately with:
  ```text
  there is no reactor running, must be called from the context of a Tokio 1.x runtime
  ```
  Because the panic occurred inside macOS's native `applicationDidFinishLaunching:` Cocoa delegate, Rust's panic unwinding crossed an FFI C/Objective-C runtime boundary, triggering `panic_cannot_unwind` and causing `abort()` (`SIGABRT`).
- **Fix**: Replaced direct `tokio::spawn` calls in `src/lib.rs` with `tauri::async_runtime::spawn`. Tauri manages its own background Tokio runtime handle, allowing asynchronous tasks to be safely scheduled from synchronous main-thread setup routines.
- **Defense-in-Depth**: Hardened `spawn_background_avatar_fetch` and `start_periodic_avatar_refresh` in `src/vault/avatar_sync.rs` to use `tauri::async_runtime::spawn`, guaranteeing that background avatar hashing and periodic sync routines can never panic if invoked from synchronous threads.

#### 2. Zero-Prompt Authenticated AES-256-GCM Machine Credential Vault
- **Frictionless Operator Experience**: Replaced native OS Keychain integrations (`keyring-rs`) with an authenticated local vault. Native OS keychains frequently prompt users with modal security dialogues (such as macOS *"TruthBeacon wants to access key truth_beacon_secure_vault in your keychain"* or system lock prompts), interrupting background sync, headless test suites, and desktop startup.
- **Cryptographic Authenticated Encryption (`AES-256-GCM`)**: The vault stores bot credentials using `ring::aead::AES_256_GCM` with 12-byte cryptographically secure random nonces and 128-bit authentication tags, preventing ciphertext tampering or truncation.
- **Machine-Unique Key Derivation via SHA-256 HKDF**: Encryption keys are deterministically derived using SHA-256 HKDF bound to host machine entropy, the current user environment, and application domain salts. Stolen vault files cannot be decrypted on other machines or user accounts.
- **Strict OS Filesystem Security**: On Unix and macOS platforms, the vault storage file (`~/.truthbeacon/vault.enc`) is created with strict `0600` permissions (read/write by owner only), blocking multi-user host inspection.
- **Dual-Tier Zeroized In-Memory Cache**: Credential lookups prioritize a thread-safe in-memory cache (`RwLock<HashMap<String, Zeroizing<String>>>`) wrapped in `zeroize::Zeroizing`. Lookups achieve sub-millisecond retrieval without disk access, decrypting the on-disk vault only on initial cold-start cache misses.
- **Secure Cryptographic Erasure & System Eradication**: Credential deletion and system eradication (`purge_all_credentials`) overwrite the vault file with zeros before removing it from disk, ensuring no data residue remains.
- **Zero-Plaintext Policy Maintained**: Bot tokens remain strictly zeroized in memory, are redacted in all log output (`[REDACTED]`), and are never serialized to JSON, SQLite, or plain text.

#### 3. Cross-Platform Cryptographic Auto-Updater Engine (macOS, Windows, Linux)
- **Tauri 2 Plugin Updater Integration**: Integrated `tauri-plugin-updater` with cryptographically enforced Minisign Ed25519 signature verification to protect operators against MITM tampering.
- **Passive Background & 1-Click Manual Updates**: The desktop client executes a silent background update check 5 seconds after launch and provides an interactive version badge button in the main header.
- **Interactive Update Notification**: Operators receive an in-app notification card with release highlights and a 1-click "Download & Install" workflow to seamlessly relaunch the updated application.
- **Full Platform Parity**: Staged and verified across macOS (Universal 2 `.app.tar.gz`), Windows (NSIS `.zip`), and Linux (AppImage `.tar.gz`) alongside public `latest.json` release manifests.

#### 4. Automated Release Pipeline, CI Signing & Artifact Packaging Hardening
- **CI/CD Minisign Key Integration**: Configured `TAURI_SIGNING_PRIVATE_KEY` and automated key decoding in `.github/workflows/ci.yml` and `.github/workflows/release.yml`, ensuring production builds generate authentic cryptographically signed manifests.
- **Safe Fallback Signing across Packaging Scripts**: Hardened `scripts/build_macos_universal.sh`, `scripts/build_linux_bundle.sh`, and `scripts/build_windows_bundle.ps1` with fallback signature generation and validation checks, enabling graceful local development builds and offline packaging.
- **Automated Manifest Generator**: Added `scripts/generate_updater_manifest.py` to deterministically assemble and format `latest.json` catalogs from release builds and signatures.
- **Detached GPG Signatures & Release Manifests**: Staged updater tarballs and bundles in `RELEASE_MANIFEST.md` and `SHA256SUMS.txt`, verified with detached ASCII-armored GPG signatures (`.asc`).
- **Clean-Slate Smoke Testing**: Updated `scripts/smoke_test_clean_slate.py` and `scripts/verify_packaging_pipeline.py` to validate clean-slate environment setup, zero-prompt vault operations, and release bundle integrity.

---

### Previous Highlights from v0.1.3 & v0.1.2

#### 1. Tauri 2 IPC Parameter Normalization
- **Resolved Argument Serialization**: Tauri 2 commands expect camelCase argument names by default (`guildId`). Added automatic bidirectional casing normalization in `ui/js/ipc.js` to ensure seamless deserialization across all Tauri commands.
- **Resilient Daemon Lifecycle**: Hardened background Discord Gateway connection lifecycle and periodic heartbeat transmission.

#### 2. Automated 1-Click Bot Authorization & Pre-Calculated Permissions
- **Instant Client ID Extraction**: TruthBeacon automatically extracts and base64-decodes your bot's application snowflake ID directly from segment 1 of your Bot Token upon pasting.
- **Pre-Calculated Permissions Bitfield**: Eliminates manual Discord OAuth2 URL Generator calculations:
  $$\text{VIEW\_CHANNEL (1024)} \mid \text{KICK\_MEMBERS (2)} \mid \text{BAN\_MEMBERS (4)} \mid \text{MODERATE\_MEMBERS (1099511627776)} = \mathbf{1099511628806}$$
- **1-Click "Authorize & Invite Bot" Button**: Direct 1-click addition of the bot to your Discord server, plus a 1-click "Copy Link" utility.

#### 3. Automated Server Discovery (Zero Snowflake Hunting)
- **Automatic Discord Gateway Server Discovery**: Integrated `fetch_bot_guilds` Tauri IPC command querying Discord v10 REST API (`GET /users/@me/guilds`).
- **Dynamic Discovered Servers Dropdown**: Discovered servers appear in an interactive dropdown with server names and IDs.

#### 4. Credential Architecture Evolution
- Evolved from system-prompted OS Keychains (`keyring-rs`) to TruthBeacon's zero-prompt Authenticated AES-256-GCM Vault architecture, preserving strict zero-plaintext security without OS modal interruptions.

---

### Core Security Engine

1. **Deterministic Multi-Script Homoglyph Detection**:
   - Sub-50ms deterministic normalization and skeleton decomposition (NFKD).
   - Defeats cross-script lookalike substitutions across Cyrillic, Greek, Mathematical, and Latin lookalikes.
   - De-obfuscates invisible unicode anomalies, zero-width spaces, and bidirectional text overrides.

2. **Perceptual Avatar Clone Defense**:
   - Discrete Cosine Transform (DCT) perceptual image hashing (`img_hash`).
   - Automatically flags unauthorized copies and subtle visual perturbations of moderator and VIP avatars.

3. **Snowflake Cryptographic Account Age Analysis**:
   - Parses Discord 64-bit integer IDs directly to extract real account creation epochs.
   - Escalates risk tier when brand-new accounts (< 72 hours old) exhibit visual or naming similarity to benchmark staff.

4. **Zero-Telemetry Local Data Sovereignty**:
   - 100% offline-first architecture; all benchmark vaults, audit logs, and incident records are retained exclusively on the operator's machine.

---

### Platform Availability & Supported Operating Systems

- **macOS (Universal 2)**:
  - Supports Apple Silicon (M1/M2/M3/M4) and Intel Macs (`macOS 10.15 Catalina` or higher).
  - Packaged as a drag-and-drop `.dmg` installer with custom Orange Heart backdrop, a standalone portable `.app` bundle, and an updater archive (`.app.tar.gz`).
- **Windows (x64)**:
  - Supports Windows 10 and Windows 11 (64-bit).
  - Packaged via WiX (`.msi`) for enterprise deployment, NSIS (`.exe`) for standard setup, a portable executable, and an NSIS updater package (`.nsis.zip`).
- **Linux (x86_64)**:
  - Supports Ubuntu 20.04+, Debian 11+, Fedora, and Arch.
  - Packaged as a native Debian `.deb` package, portable standalone `.AppImage`, and an AppImage updater archive (`.AppImage.tar.gz`).

---

### Verification & Checksums

All binaries are verified through GitHub Actions CI and signed with SHA-256 cryptographic checksums. Check `SHA256SUMS.txt` and `RELEASE_MANIFEST.md` for verification details.
