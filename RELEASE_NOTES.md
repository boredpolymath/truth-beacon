# TruthBeacon v0.1.2 — Official Release Notes
**Orange Heart Industries — Identity Ground-Truth & Community Stewardship**

---

### Overview
TruthBeacon v0.1.2 is a major usability and friction-removal release of our standalone desktop security console for Discord community leadership, pastoral teams, and server moderators. 

This release completely eliminates manual setup friction, snowflake ID hunting, and complex bot permission calculations—empowering community leaders to pair TruthBeacon with their Discord servers in under 60 seconds with zero technical complexity.

---

### What's New in v0.1.2

#### 1. Automated 1-Click Bot Authorization & Pre-Calculated Permissions
- **Instant Client ID Extraction**: TruthBeacon automatically extracts and base64-decodes your bot's application snowflake ID directly from segment 1 of your Bot Token upon pasting. No need to navigate to the Discord Developer Portal's "General Information" tab just to hunt down an Application ID.
- **Pre-Calculated Permissions Bitfield**: Eliminates the manual Discord OAuth2 URL Generator. TruthBeacon pre-calculates the exact bitwise integer required for anti-impersonation defense:
  $$\text{VIEW\_CHANNEL (1024)} \mid \text{KICK\_MEMBERS (2)} \mid \text{BAN\_MEMBERS (4)} \mid \text{MODERATE\_MEMBERS (1099511627776)} = \mathbf{1099511628806}$$
- **1-Click "Authorize & Invite Bot" Button**: A prominent action card provides direct 1-click addition of the bot to your Discord server, plus a 1-click "Copy Link" utility for server owners.

#### 2. Automated Server Discovery (Zero Snowflake Hunting)
- **Automatic Discord Gateway Server Discovery**: Added `fetch_bot_guilds` Tauri IPC command querying Discord v10 REST API (`GET /users/@me/guilds`).
- **Elimination of Developer Mode**: Community leaders are no longer forced to enable Discord Developer Mode, right-click their server icon, and copy 19-digit numeric snowflake Guild IDs.
- **Dynamic Discovered Servers Dropdown**: Discovered servers appear in an interactive dropdown with server names and IDs. If the bot is present in a single server, TruthBeacon auto-selects it instantly.

#### 3. Frictionless Automated Handshake Auto-Verification
- **Instant Pre-Flight Testing**: When a server is selected from the discovered server list, TruthBeacon automatically triggers the 5-point diagnostic pre-flight verification in the background.
- **Instant Visual Confirmation**: Token Format, Gateway Handshake, Privileged `GUILD_MEMBERS` Intent, Server Membership, and Moderation Capabilities turn green automatically without requiring operators to hunt down a separate test button.

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
