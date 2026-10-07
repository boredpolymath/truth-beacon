# TruthBeacon v0.1.4 — Official Release Notes
**Orange Heart Industries — Identity Ground-Truth & Community Stewardship**

---

### Overview
TruthBeacon v0.1.4 is a critical stability patch resolving an immediate startup panic (`EXC_CRASH (SIGABRT)`) on macOS when launching with bot credentials saved in the native OS Keychain.

---

### What's Fixed in v0.1.4

#### 1. Resolved Startup Tokio Reactor Panic on Credential Auto-Connect
- **Issue**: When bot credentials had been registered and stored in the native OS Keychain (`truth_beacon_secure_vault`), TruthBeacon automatically initiates background connection to the Discord Gateway daemon during startup. In v0.1.3, this async task was spawned using `tokio::spawn` inside Tauri's synchronous `.setup(|app| ...)` hook on the main thread. Because Tauri executes `setup` on the main thread outside of an active Tokio runtime context, `tokio::spawn` panicked immediately with:
  ```text
  there is no reactor running, must be called from the context of a Tokio 1.x runtime
  ```
  Because the panic occurred inside macOS's native `applicationDidFinishLaunching:` delegate, Rust's panic unwinding crossed an FFI C/Objective-C runtime boundary, triggering `panic_cannot_unwind` and causing `abort()` (`SIGABRT`).
- **Fix**: Replaced direct `tokio::spawn` calls in `src/lib.rs` with `tauri::async_runtime::spawn`. Tauri manages its own background Tokio runtime handle, allowing asynchronous tasks to be safely scheduled from synchronous main-thread setup routines.
- **Defense-in-Depth**: Also hardened `spawn_background_avatar_fetch` and `start_periodic_avatar_refresh` in `src/vault/avatar_sync.rs` to use `tauri::async_runtime::spawn`, guaranteeing that background avatar hashing and periodic sync routines can never panic if invoked from synchronous threads.

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

#### 4. Native OS Keychain & Zeroized In-Memory Cache Resilience
- **Persistent Credential Lifecycle**: Dual-tier credential architecture combining native OS Secure Enclaves (`keyring-rs` targeting Apple Keychain Services, Windows Credential Manager DPAPI, and Linux Secret Service) with zeroized in-memory fallback caches.
- **Zero-Plaintext Security Policy**: Bot tokens remain strictly zeroized and never touch SQLite databases, flat files, or unredacted serialization logs.

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
  - Packaged as a drag-and-drop `.dmg` installer with custom Orange Heart backdrop and a standalone portable `.app` bundle.
- **Windows (x64)**:
  - Supports Windows 10 and Windows 11 (64-bit).
  - Packaged via WiX (`.msi`) for enterprise deployment, NSIS (`.exe`) for standard setup, and a portable executable.
- **Linux (x86_64)**:
  - Supports Ubuntu 20.04+, Debian 11+, Fedora, and Arch.
  - Packaged as a native Debian `.deb` package and portable standalone `.AppImage`.

---

### Verification & Checksums

All binaries are verified through GitHub Actions CI and signed with SHA-256 cryptographic checksums. Check `SHA256SUMS.txt` and `RELEASE_MANIFEST.md` for verification details.
