# TruthBeacon v0.1.0 — Official Release Staging Manifest
**Orange Heart Industries — Release Engineering Pipeline**

## Overview
- **Application Name**: TruthBeacon
- **Version**: 0.1.0
- **Identifier**: `com.orangeheartindustries.truthbeacon`
- **Frontend Engine**: Tauri 2 (Webkit / Cocoa WebView, zero npm blobs)
- **Backend Architecture**: Rust 1.98.1 (Universal 2: `x86_64` + `aarch64`)
- **Status**: Production Release Build Staged

---

## Packaged Release Artifacts

| Artifact File | Architecture | Size | SHA-256 Checksum | Description |
| :--- | :--- | :--- | :--- | :--- |
| `TruthBeacon_0.1.0_universal.dmg` | Universal 2 (Intel + Apple Silicon) | ~28 MB | `06f2ea99cdabc5a4d58a15b68c544e6aebf261769ba3b46c06e0c406aafb0e4d` | Drag-and-drop installer with branded Discord charcoal canvas |
| `TruthBeacon_0.1.0_macos_universal.zip` | Universal 2 (Intel + Apple Silicon) | ~25 MB | `624611f1305d10e4d624be9b6544ec8dbaf5bf4038541f0f50a0c73e5d276839` | Standalone portable `.app` bundle archive |
| `truth-beacon-universal` | Universal 2 (Intel + Apple Silicon) | ~53 MB | `5616b4eb2b4c89828d8585abd41abcba65d6a61da6c05889377a510b8a9d43d4` | Standalone Mach-O fat binary |

---

## Verification & Pipeline Audits

1. **Unit & Integration Test Suite**:
   - `cargo test`: **149 passed, 0 failed, 0 ignored** (100% pass rate).
2. **Packaging Pipeline Verification**:
   - `scripts/verify_packaging_pipeline.py`: **5/5 checks passed** (DMG layout, NSIS/WiX assets, Freedesktop compliance, multi-res icons, automation scripts).
3. **Software Bill of Materials (SBOM) & License Audit**:
   - `scripts/audit_licenses.py`: **100% compliant** (0 copyleft / GPL/AGPL transitive crates; zero unvetted npm blobs).
4. **Adversarial Red Team Simulation**:
   - `scripts/red_team_adversarial_simulation.py`: **100% mitigation rate** across script-mixed homoglyphs, zero-width spaces, and DCT perceptual avatar clones.
5. **Universal Binary Verification**:
   - `lipo -info truth-beacon-universal`: `[x86_64: Mach-O 64-bit executable x86_64]` + `[arm64: Mach-O 64-bit executable arm64]`.

---

## Cross-Platform Distribution Matrix

- **macOS**: Built and packaged locally into Universal 2 `.dmg` and `.app`.
- **Windows**: Ready for build via `scripts/build_windows_bundle.ps1` (WiX MSI, NSIS EXE, and portable `.exe`). Configured in `.github/workflows/release.yml`.
- **Linux**: Ready for build via `scripts/build_linux_bundle.sh` (Debian `.deb` and `.AppImage`). Configured in `.github/workflows/release.yml`.
