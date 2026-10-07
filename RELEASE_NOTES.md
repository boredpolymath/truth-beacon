# TruthBeacon v0.1.4 — Official Release Notes
**Orange Heart Industries — Identity Ground-Truth & Community Stewardship**

---

### Overview
TruthBeacon v0.1.4 is a major reliability, usability, and release infrastructure update for our standalone desktop security console. This release delivers:

1. **Zero-Crash Startup Resilience**: Resolves a critical Tokio runtime panic (`EXC_CRASH (SIGABRT)`) on macOS during boot when auto-connecting with saved bot credentials.
2. **Zero-Prompt Authenticated AES-256-GCM Machine Vault**: Replaces disruptive native OS Keychain modal dialogs with a seamless, machine-bound authenticated AES-256-GCM credential vault with strict POSIX permissions.
3. **Cross-Platform Cryptographic Auto-Updater Engine**: Integrates in-app update checks, release notification cards, streamed progress downloads, and 1-click seamless restarts across macOS, Windows, and Linux powered by `tauri-plugin-updater` and Minisign Ed25519 signatures.
4. **Hardened CI/CD & Multi-Platform Release Staging**: Implements automated Minisign signing in CI workflows, fallback signing for local developer packaging, automated updater catalog generation (`latest.json`), and detached GPG signatures.

---

### What's New & Fixed in v0.1.4

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
