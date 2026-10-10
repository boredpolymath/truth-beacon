# TruthBeacon v0.3.1 — Official Release Staging Manifest
**Bored Polymath Studios — Release Engineering Pipeline**

## Overview
- **Application Name**: TruthBeacon
- **Version**: 0.3.1
- **Identifier**: `com.boredpolymath.truthbeacon`
- **Frontend Engine**: Tauri 2 (Webkit / Cocoa WebView, zero npm blobs)
- **Backend Architecture**: Rust 2021 (Universal 2: `x86_64` + `aarch64`)
- **Status**: Production Release Build Staged & Signed

---

## Packaged Release Artifacts

| Artifact File | Architecture | Size | SHA-256 Checksum | Description |
| :--- | :--- | :--- | :--- | :--- |
| `TruthBeacon_0.3.1_universal.dmg` | Universal 2 (Intel + Apple Silicon) | ~29 MB | `5249bc8cf775c99878f43340b7fb73633270d0c1ed98ed827ec17e9aeb8c5d20` | Release deliverable |
| `TruthBeacon_0.3.1_universal.app.tar.gz` | Universal 2 (Intel + Apple Silicon) | ~27 MB | `22fcb3375a27df0f717fa6cc741b4c9e48a1633cde55bd1ef25900fae6130bbb` | Release deliverable |
| `TruthBeacon_0.3.1_macos_universal.zip` | Universal 2 (Intel + Apple Silicon) | ~27 MB | `67d4713af9e7c2b22023e44715d42ac7a7b8dedfdda59af3ddbe32f307fd8499` | Release deliverable |
| `TruthBeacon_0.3.0_universal.dmg` | Universal 2 (Intel + Apple Silicon) | ~29 MB | `c41fa2f23b5edf61c0b18cb81e5df0d665b89a741d6ddb484bffe9482b684204` | Release deliverable |
| `TruthBeacon_0.3.0_universal.app.tar.gz` | Universal 2 (Intel + Apple Silicon) | ~27 MB | `d4f10404c4bd979186dafe2e31967142a327ef621fdc8041ecdfe45ce572804a` | Release deliverable |
| `TruthBeacon_0.3.0_macos_universal.zip` | Universal 2 (Intel + Apple Silicon) | ~27 MB | `a209d63d56d88beb2ec94b5644f1947022f6d08e0f3844aa2858d091a2e88d33` | Release deliverable |
| `TruthBeacon_0.2.2_universal.dmg` | Universal 2 (Intel + Apple Silicon) | ~29.5 MB | `c450fa110a54f2d64eab14763b8d9a7505f7bb0100c95417eca0d185c713a629` | Apple Developer ID signed drag-and-drop macOS installer |
| `TruthBeacon_0.2.2_macos_universal.zip` | Universal 2 (Intel + Apple Silicon) | ~27.9 MB | `58101f915f97e4950a8d03ff85a32d21fb6e49d9dc7703ca0bb71e3c06375730` | Standalone portable macOS `.app` bundle archive |
| `TruthBeacon_0.2.2_universal.app.tar.gz` | Universal 2 (Intel + Apple Silicon) | ~27.9 MB | `7bd8294fa257176482da5af1651681ab3c4fe348d996bf99832e0abbc0215ceb` | Standalone macOS auto-updater tarball archive |
| `truth-beacon-universal` | Universal 2 (Intel + Apple Silicon) | ~59.3 MB | `c9dda263d5d58adb46fe73025947e127a5fe200b7677bcc2ecf2e5145209890e` | Standalone Mach-O fat binary |
| `TruthBeacon_0.2.2_x64-setup.exe` | Windows x64 (MSVC) | ~9.5 MB | `6f36a65df228544edaa36111ea7ae5eac11baa754fae81adc62956953267586e` | Windows NSIS single-executable installer |
| `TruthBeacon_0.2.2_x64_en-US.msi` | Windows x64 (MSVC) | ~12 MB | `436d9f7be2f5776455e9fe06b111c079c703f184ce56c9bc5ee570ea3e64aaad` | Windows WiX MSI enterprise installer |
| `TruthBeacon-Portable.exe` | Windows x64 (MSVC) | ~24 MB | `03858df78e645d9c3d38ea1ac0c146d86e8aa843f22fcba8678f59d39985a098` | Standalone portable Windows executable |
| `TruthBeacon_0.2.2_amd64.deb` | Linux x86_64 | ~14 MB | `3173c92fa36016171fa7d1efabeb2b1c67b82e7ad2e490f6426d7bbebbb94136` | Debian / Ubuntu `.deb` package |
| `TruthBeacon_0.2.2_amd64.AppImage` | Linux x86_64 | ~86 MB | `ec3203b01db76b58b2c55311ea5fdbfeca4c97111cc8207073038c382ca9d3c3` | Standalone cross-distro AppImage executable |

---

## Verification & Pipeline Audits

1. **Unit & Integration Test Suite**:
   - `cargo test --all-targets`: **208 passed, 0 failed, 0 ignored** (100% pass rate).
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
