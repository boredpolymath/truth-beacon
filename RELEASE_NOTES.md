# TruthBeacon v0.1.0 — Official Release Notes
**Orange Heart Industries — Identity Ground-Truth & Community Stewardship**

---

### Overview
TruthBeacon v0.1.0 is the inaugural production release of our standalone desktop security console designed specifically for Discord community leadership, pastoral teams, and server moderators. 

TruthBeacon continuously inspects live Discord member events in real-time, detecting and mitigating identity impersonation attacks before bad actors can exploit pastoral or administrative trust to defraud community members.

---

### Key Capabilities

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
   - No cloud snooping, no external logging, and no analytics beacons.
   - All benchmarks, audit logs, and incident records are retained exclusively on the operator's machine in encrypted SQLite databases.
   - Bot tokens and credentials securely held in the host operating system's native keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service).

5. **Discord-Familiar Operator Interface**:
   - Styled after Discord’s high-contrast dark theme (`#111214`, `#1e1f22`, `#2b2d31`, `#313338`) accented with Orange Heart warmth (`#f97316`).
   - Clean, non-technical triage card layout with comparative side-by-side identity adjudication.

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

### Cryptographic Checksums (SHA-256)

```text
06f2ea99cdabc5a4d58a15b68c544e6aebf261769ba3b46c06e0c406aafb0e4d  TruthBeacon_0.1.0_universal.dmg
624611f1305d10e4d624be9b6544ec8dbaf5bf4038541f0f50a0c73e5d276839  TruthBeacon_0.1.0_macos_universal.zip
5616b4eb2b4c89828d8585abd41abcba65d6a61da6c05889377a510b8a9d43d4  truth-beacon-universal
```

---

### Installation & Quickstart

#### macOS:
1. Double-click `TruthBeacon_0.1.0_universal.dmg`.
2. Drag `TruthBeacon` into `Applications`.
3. Open TruthBeacon from `/Applications` or Spotlight.
4. On initial launch, navigate to **Settings** and supply your Discord Bot Token and Guild ID.
5. In **Benchmark Vault**, import verified server leaders to establish ground-truth protection.
<<<<<<< HEAD
=======

#### Windows:
1. Run `TruthBeacon_0.1.0_x64-setup.exe` (or deploy `TruthBeacon_0.1.0_x64.msi` for enterprise management).
2. For zero-install portable usage, launch `TruthBeacon-Portable.exe` directly from any folder or USB drive.
3. Open TruthBeacon from the Start Menu or desktop shortcut.
4. Navigate to **Settings** to securely save your Discord Bot Token and target Guild ID.

#### Linux:
1. **AppImage:** Make executable (`chmod +x TruthBeacon-0.1.0.AppImage`) and run `./TruthBeacon-0.1.0.AppImage`.
2. **Debian / Ubuntu:** Install package via `sudo dpkg -i truth-beacon_0.1.0_amd64.deb` (resolving dependencies with `sudo apt-get install -f`).
3. Ensure system dependencies are met (`libappindicator3-1`, `libsecret-1-0`, and `libwebkit2gtk-4.1-0`).
4. Launch TruthBeacon from your desktop environment menu or terminal (`truth-beacon`).
>>>>>>> 9e22ef1 (docs: add Windows and Linux installation instructions to release notes and documentation)
