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

# Package and sign macOS updater bundle (.app.tar.gz)
VERSION=$(grep '^version =' "${SRC_TAURI_DIR}/Cargo.toml" | head -n 1 | cut -d '"' -f 2)
APP_DIR="${SRC_TAURI_DIR}/target/universal-apple-darwin/release/bundle/macos"
if [ -d "${APP_DIR}/TruthBeacon.app" ]; then
    echo "==> Generating macOS auto-updater archive (.app.tar.gz)..."
    UPDATER_TAR="${APP_DIR}/TruthBeacon_${VERSION}_universal.app.tar.gz"
    tar -czf "${UPDATER_TAR}" -C "${APP_DIR}" TruthBeacon.app
    if [ -f "${ROOT_DIR}/scripts/.keys/truthbeacon_updater.key" ]; then
        echo "==> Signing updater archive with minisign release key file..."
        cargo tauri signer sign -f "${ROOT_DIR}/scripts/.keys/truthbeacon_updater.key" -p "" --app-version "${VERSION}" "${UPDATER_TAR}"
    elif [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
        echo "==> Signing updater archive with TAURI_SIGNING_PRIVATE_KEY environment variable..."
        cargo tauri signer sign -p "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" --app-version "${VERSION}" "${UPDATER_TAR}"
    fi
    cp -f "${UPDATER_TAR}" "${ROOT_DIR}/release/" 2>/dev/null || true
    if [ -f "${UPDATER_TAR}.sig" ]; then
        cp -f "${UPDATER_TAR}.sig" "${ROOT_DIR}/release/" 2>/dev/null || true
    fi
    echo "==> Creating portable macOS .zip archive..."
    (cd "${APP_DIR}" && zip -rq "${ROOT_DIR}/release/TruthBeacon_${VERSION}_macos_universal.zip" TruthBeacon.app)
fi

# Optional: Execute Code Signing & Notarization if configured
if [[ "${SIGN_RELEASE:-false}" == "true" || -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
    echo "==> Code signing and notarization configured; executing sign_and_notarize_macos.sh..."
    "${SCRIPT_DIR}/sign_and_notarize_macos.sh"
fi

echo "======================================================================"
echo " macOS Universal 2 packaging & updater artifacts completed successfully!"
echo " Output artifacts:"
find "${SRC_TAURI_DIR}/target/universal-apple-darwin/release/bundle" -name "*.dmg" -o -name "*.app" -o -name "*.tar.gz*" || true
echo "======================================================================"

