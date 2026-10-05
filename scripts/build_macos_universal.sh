#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# TruthBeacon macOS Universal 2 & DMG Packager
# Orange Heart Industries - Release Staging Pipeline
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
SRC_TAURI_DIR="${ROOT_DIR}/src-tauri"

echo "======================================================================"
echo " TruthBeacon: macOS Universal 2 Build & DMG Packaging"
echo "======================================================================"

cd "${ROOT_DIR}"

# 1. Verify Rust targets
echo "==> [1/5] Checking Rust targets..."
rustup target add aarch64-apple-darwin 2>/dev/null || true
rustup target add x86_64-apple-darwin 2>/dev/null || true

# 2. Generate installer assets (DMG background, icon layout)
echo "==> [2/5] Generating branded installer assets..."
python3 "${SCRIPT_DIR}/generate_installer_assets.py"

# 3. Compile target architectures
echo "==> [3/5] Compiling release binaries for Apple Silicon (arm64) and Intel (x86_64)..."
cargo build --manifest-path "${SRC_TAURI_DIR}/Cargo.toml" --release --target aarch64-apple-darwin
cargo build --manifest-path "${SRC_TAURI_DIR}/Cargo.toml" --release --target x86_64-apple-darwin

# 4. Assemble Universal 2 binary with lipo
echo "==> [4/5] Assembling Universal 2 Fat Binary via lipo..."
UNIVERSAL_DIR="${SRC_TAURI_DIR}/target/universal-apple-darwin/release"
mkdir -p "${UNIVERSAL_DIR}"

lipo -create \
  "${SRC_TAURI_DIR}/target/aarch64-apple-darwin/release/truth-beacon" \
  "${SRC_TAURI_DIR}/target/x86_64-apple-darwin/release/truth-beacon" \
  -output "${UNIVERSAL_DIR}/truth-beacon"

echo "Universal binary verified:"
lipo -info "${UNIVERSAL_DIR}/truth-beacon"
file "${UNIVERSAL_DIR}/truth-beacon"

# 5. Build Tauri App bundle and package DMG via dmgbuild
echo "==> [5/5] Packaging TruthBeacon Drag-and-Drop .dmg with Orange Heart layout..."
cd "${SRC_TAURI_DIR}"
cargo tauri build --target universal-apple-darwin --no-sign -b app
python3 "${SCRIPT_DIR}/package_macos_dmg.py"

# Optional: Execute Code Signing & Notarization if configured
if [[ "${SIGN_RELEASE:-false}" == "true" || -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
    echo "==> Code signing and notarization configured; executing sign_and_notarize_macos.sh..."
    "${SCRIPT_DIR}/sign_and_notarize_macos.sh"
fi

echo "======================================================================"
echo " macOS Universal 2 DMG packaging completed successfully!"
echo " Output artifacts:"
find "${SRC_TAURI_DIR}/target/universal-apple-darwin/release/bundle" -name "*.dmg" -o -name "*.app" || true
echo "======================================================================"

