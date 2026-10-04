#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# TruthBeacon Linux Bundle Packager (AppImage & Debian .deb)
# Orange Heart Industries - Release Staging Pipeline
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
SRC_TAURI_DIR="${ROOT_DIR}/src-tauri"

echo "======================================================================"
echo " TruthBeacon: Linux AppImage & Debian (.deb) Packaging"
echo "======================================================================"

cd "${ROOT_DIR}"

# 1. Verify Rust Target
echo "==> [1/5] Checking Rust targets..."
rustup target add x86_64-unknown-linux-gnu 2>/dev/null || true

# 2. Validate Desktop Entry Compliance (Freedesktop Standard)
echo "==> [2/5] Validating desktop entry compliance (truthbeacon.desktop)..."
DESKTOP_FILE="${SRC_TAURI_DIR}/desktop/truthbeacon.desktop"
if [ ! -f "${DESKTOP_FILE}" ]; then
  echo "Error: ${DESKTOP_FILE} not found!"
  exit 1
fi

if command -v desktop-file-validate >/dev/null 2>&1; then
  desktop-file-validate "${DESKTOP_FILE}"
  echo "Desktop file passed desktop-file-validate checks."
else
  echo "desktop-file-validate not available on host; verifying key required fields manually..."
  grep -q "^Type=Application" "${DESKTOP_FILE}"
  grep -q "^Exec=truth-beacon" "${DESKTOP_FILE}"
  grep -q "^Icon=truth-beacon" "${DESKTOP_FILE}"
  grep -q "^Categories=" "${DESKTOP_FILE}"
  grep -q "^StartupWMClass=truth-beacon" "${DESKTOP_FILE}"
  echo "Required Freedesktop keys present in desktop entry."
fi

# 3. Check / Document Linux System Dependencies for System Tray & Keychain
echo "==> [3/5] Verifying declared Debian package dependencies..."
python3 -c "
import json
with open('${SRC_TAURI_DIR}/tauri.conf.json') as f:
    conf = json.load(f)
deps = conf.get('bundle', {}).get('linux', {}).get('deb', {}).get('depends', [])
print('Configured Debian dependencies:')
for d in deps:
    print(f'  - {d}')
assert any('appindicator' in d for d in deps), 'Missing system tray dependency (libappindicator3)!'
assert any('secret' in d for d in deps), 'Missing credentials keychain dependency (libsecret)!'
assert any('webkit' in d for d in deps), 'Missing webview dependency (libwebkit2gtk)!'
print('All core Linux runtime dependencies verified in tauri.conf.json.')
"

# 4. Build Bundles via Tauri CLI
echo "==> [4/5] Building AppImage and Debian (.deb) packages..."
cd "${SRC_TAURI_DIR}"

# In Linux build environments (e.g. CI or native host)
if [[ "$(uname -s)" == "Linux" ]]; then
  cargo tauri build --bundles deb,appimage
  echo "======================================================================"
  echo " Linux AppImage & Debian packaging completed successfully!"
  echo " Generated Artifacts:"
  find "${SRC_TAURI_DIR}/target/release/bundle" -name "*.deb" -o -name "*.AppImage" || true
  echo "======================================================================"
else
  echo "Note: Host OS is not Linux ($(uname -s)). Linux binaries/AppImages are packaged on Ubuntu runner in CI."
  echo "To build on Linux runner: cargo tauri build --bundles deb,appimage"
fi
