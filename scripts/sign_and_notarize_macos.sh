#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# TruthBeacon macOS Code Signing, Notarization & Stapling Pipeline
# Orange Heart Industries - Phase 24 Release Staging
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
SRC_TAURI_DIR="${ROOT_DIR}/src-tauri"
RELEASE_DIR="${ROOT_DIR}/release"
ENTITLEMENTS_FILE="${SRC_TAURI_DIR}/Entitlements.plist"

APP_PATH="${SRC_TAURI_DIR}/target/universal-apple-darwin/release/bundle/macos/TruthBeacon.app"
DMG_PATH="${SRC_TAURI_DIR}/target/universal-apple-darwin/release/bundle/dmg/TruthBeacon_0.1.0_universal.dmg"
RELEASE_DMG="${RELEASE_DIR}/TruthBeacon_0.1.0_universal.dmg"

# Configuration via environment or parameters
SIGN_IDENTITY="${APPLE_SIGNING_IDENTITY:-}"
NOTARY_KEYCHAIN_PROFILE="${NOTARY_KEYCHAIN_PROFILE:-}"
APPLE_ID="${APPLE_ID:-}"
APPLE_APP_SPECIFIC_PASSWORD="${APPLE_APP_SPECIFIC_PASSWORD:-${APPLE_PASSWORD:-}}"
APPLE_TEAM_ID="${APPLE_TEAM_ID:-}"

DRY_RUN=false
for arg in "$@"; do
    case $arg in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --identity=*)
            SIGN_IDENTITY="${arg#*=}"
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [--dry-run] [--identity=\"Developer ID Application: ...\"]"
            echo "Environment variables:"
            echo "  APPLE_SIGNING_IDENTITY       Apple Developer ID certificate name or SHA-1 hash"
            echo "  NOTARY_KEYCHAIN_PROFILE      notarytool stored credentials profile"
            echo "  APPLE_ID                     Apple account email (if not using keychain profile)"
            echo "  APPLE_APP_SPECIFIC_PASSWORD  Apple app-specific password (if not using keychain profile)"
            echo "  APPLE_TEAM_ID                Apple Developer Team ID (10-character alphanumeric)"
            exit 0
            ;;
    esac
done

echo "======================================================================"
echo " TruthBeacon: macOS Code Signing & Notarization Pipeline (Phase 24.1)"
echo "======================================================================"

# Ensure targets exist
if [[ ! -d "${APP_PATH}" ]]; then
    echo "❌ Error: App bundle not found at ${APP_PATH}"
    echo "Please build the application first (e.g. via scripts/build_macos_universal.sh)."
    exit 1
fi

if [[ ! -f "${ENTITLEMENTS_FILE}" ]]; then
    echo "❌ Error: Entitlements file not found at ${ENTITLEMENTS_FILE}"
    exit 1
fi

# Detect signing identity if not explicitly specified
if [[ -z "${SIGN_IDENTITY}" && "${DRY_RUN}" = false ]]; then
    echo "==> Searching for Developer ID Application identities in keychain..."
    DETECTED_IDENTITY=$(security find-identity -v -p codesigning 2>/dev/null | grep "Developer ID Application:" | head -n 1 | awk -F '"' '{print $2}' || true)
    if [[ -n "${DETECTED_IDENTITY}" ]]; then
        SIGN_IDENTITY="${DETECTED_IDENTITY}"
        echo "  Found active identity: ${SIGN_IDENTITY}"
    else
        echo "  ⚠️ No active Developer ID Application identity found on this machine."
        echo "  Switching to test/dry-run mode using ad-hoc signing (-s -)."
        DRY_RUN=true
    fi
fi

# 1. Sign .app bundle
echo "==> [1/4] Signing macOS Application Bundle with Hardened Runtime..."
xattr -cr "${APP_PATH}" 2>/dev/null || true
if [[ "${DRY_RUN}" = true ]]; then
    echo "  [DRY-RUN / TEST] Performing ad-hoc hardened signing (-s -)..."
    codesign --deep --force --options runtime --entitlements "${ENTITLEMENTS_FILE}" --sign - "${APP_PATH}"
else
    echo "  Signing with: ${SIGN_IDENTITY}"
    codesign --deep --force --options runtime --entitlements "${ENTITLEMENTS_FILE}" --timestamp --sign "${SIGN_IDENTITY}" "${APP_PATH}"
fi

echo "  Verifying .app bundle signature..."
codesign --verify --deep --strict --verbose=2 "${APP_PATH}"
codesign -dv --verbose=4 "${APP_PATH}" 2>&1 | grep -E "(Identifier|Format|CodeDirectory|Authority|TeamIdentifier)" || true
echo "  ✓ Application bundle signature verified successfully."

# 2. Package / Sign DMG
echo "==> [2/4] Packaging & Signing macOS Drag-and-Drop .dmg Installer..."
python3 "${SCRIPT_DIR}/package_macos_dmg.py"

if [[ "${DRY_RUN}" = true ]]; then
    echo "  [DRY-RUN / TEST] Performing ad-hoc DMG signing..."
    codesign --force --sign - "${DMG_PATH}"
else
    echo "  Signing DMG with: ${SIGN_IDENTITY}"
    codesign --force --timestamp --sign "${SIGN_IDENTITY}" "${DMG_PATH}"
fi

echo "  Verifying DMG signature..."
codesign --verify --verbose=2 "${DMG_PATH}"
echo "  ✓ DMG signature verified successfully."

# 3. Notarize via xcrun notarytool
echo "==> [3/4] Apple Notarization Submission..."
CAN_NOTARIZE=false

if [[ -n "${NOTARY_KEYCHAIN_PROFILE}" ]]; then
    CAN_NOTARIZE=true
    NOTARY_CMD=(xcrun notarytool submit "${DMG_PATH}" --keychain-profile "${NOTARY_KEYCHAIN_PROFILE}" --wait)
elif [[ -n "${APPLE_ID}" && -n "${APPLE_APP_SPECIFIC_PASSWORD}" && -n "${APPLE_TEAM_ID}" ]]; then
    CAN_NOTARIZE=true
    NOTARY_CMD=(xcrun notarytool submit "${DMG_PATH}" --apple-id "${APPLE_ID}" --password "${APPLE_APP_SPECIFIC_PASSWORD}" --team-id "${APPLE_TEAM_ID}" --wait)
fi

if [[ "${CAN_NOTARIZE}" = true && "${DRY_RUN}" = false ]]; then
    echo "  Submitting ${DMG_PATH} to Apple Notary Service..."
    "${NOTARY_CMD[@]}"
    echo "  ✓ Notarization submission validated and ticket accepted!"

    # 4. Staple ticket to DMG
    echo "==> [4/4] Stapling Notarization Ticket to DMG..."
    xcrun stapler staple "${DMG_PATH}"
    xcrun stapler validate "${DMG_PATH}"
    spctl --assess --type open --context context:primary-signature --verbose "${DMG_PATH}"
    echo "  ✓ Notarization ticket successfully stapled and validated by Gatekeeper."
else
    echo "  [NOTICE] Skipping online Apple Notarization submission:"
    if [[ "${DRY_RUN}" = true ]]; then
        echo "    Reason: Operating in dry-run/local test mode."
    else
        echo "    Reason: Neither NOTARY_KEYCHAIN_PROFILE nor (APPLE_ID, APPLE_APP_SPECIFIC_PASSWORD, APPLE_TEAM_ID) provided."
    fi
    echo "    To submit for notarization, configure credentials in environment or CI secrets."
    echo "==> [4/4] Stapling step skipped (notarization not submitted)."
fi

# Stage final signed artifact
echo "==> Staging signed release artifact to release/..."
mkdir -p "${RELEASE_DIR}"
cp -f "${DMG_PATH}" "${RELEASE_DMG}"
echo "  Updated: ${RELEASE_DMG} ($(wc -c < "${RELEASE_DMG}" | tr -d ' ') bytes)"

echo "======================================================================"
echo " macOS Code Signing & Integrity Verification Complete!"
echo "======================================================================"
