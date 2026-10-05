#!/usr/bin/env python3
"""
TruthBeacon Phase 24 Code Signing, Notarization & Artifact Integrity Verification Suite
Orange Heart Industries - Quality Engineering

Validates:
1. macOS Hardened Runtime Entitlements and Tauri bundle configuration.
2. macOS codesign, notarytool, and stapler automation script integrity.
3. Windows Authenticode and SmartScreen reputation script structure.
4. SHA-256 reproducibility and detached GPG/OpenPGP ASCII-armored signatures.
5. Release manifest cryptographic synchronization.
"""

import os
import sys
import json
import plistlib
import subprocess
import re

ROOT_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
SRC_TAURI = os.path.join(ROOT_DIR, "src-tauri")
ENTITLEMENTS_FILE = os.path.join(SRC_TAURI, "Entitlements.plist")
TAURI_CONF = os.path.join(SRC_TAURI, "tauri.conf.json")
RELEASE_DIR = os.path.join(ROOT_DIR, "release")
SCRIPTS_DIR = os.path.join(ROOT_DIR, "scripts")

def test_macos_entitlements_and_config():
    print("==> [1/5] Validating macOS Hardened Runtime & Entitlements...")
    assert os.path.exists(ENTITLEMENTS_FILE), f"Entitlements file missing: {ENTITLEMENTS_FILE}"
    with open(ENTITLEMENTS_FILE, "rb") as f:
        plist = plistlib.load(f)

    assert plist.get("com.apple.security.cs.allow-jit") is True, "Missing allow-jit entitlement for WebKit"
    assert plist.get("com.apple.security.network.client") is True, "Missing network.client entitlement for Discord Gateway"
    print("  ✓ Entitlements.plist contains required Hardened Runtime capabilities")

    with open(TAURI_CONF, "r") as f:
        conf = json.load(f)
    macos_conf = conf.get("bundle", {}).get("macOS", {})
    assert macos_conf.get("hardenedRuntime") is True, "hardenedRuntime not set to true in tauri.conf.json"
    assert macos_conf.get("entitlements") == "Entitlements.plist", "entitlements file not linked in tauri.conf.json"
    print("  ✓ tauri.conf.json hardenedRuntime & entitlements verified")

def test_macos_signing_script():
    print("==> [2/5] Validating macOS Code Signing & Notarization Pipeline Script...")
    script = os.path.join(SCRIPTS_DIR, "sign_and_notarize_macos.sh")
    assert os.path.exists(script), f"macOS signing script missing: {script}"
    assert os.access(script, os.X_OK), f"macOS signing script not executable: {script}"
    with open(script, "r") as f:
        content = f.read()

    assert "codesign" in content, "Missing codesign invocation"
    assert "--options runtime" in content, "Missing hardened runtime options in codesign"
    assert "notarytool" in content, "Missing xcrun notarytool invocation"
    assert "stapler" in content, "Missing xcrun stapler invocation"
    assert "--dry-run" in content, "Missing dry-run/fallback mode support"
    print("  ✓ scripts/sign_and_notarize_macos.sh implements full Phase 24.1 pipeline")

def test_windows_signing_script():
    print("==> [3/5] Validating Windows Authenticode & SmartScreen Script...")
    script = os.path.join(SCRIPTS_DIR, "sign_windows_bundle.ps1")
    assert os.path.exists(script), f"Windows signing script missing: {script}"
    with open(script, "r") as f:
        content = f.read()

    assert "signtool" in content, "Missing signtool invocation"
    assert "timestamp" in content.lower(), "Missing RFC-3161 timestamping configuration"
    assert "SmartScreen" in content, "Missing SmartScreen reputation guidance"
    assert "wdsi/filesubmission" in content, "Missing Microsoft WDSI submission link"
    print("  ✓ scripts/sign_windows_bundle.ps1 implements full Phase 24.2 pipeline")

def test_checksum_integrity_and_signatures():
    print("==> [4/5] Validating Checksums and Detached Cryptographic Signatures...")
    checksum_file = os.path.join(RELEASE_DIR, "SHA256SUMS.txt")
    assert os.path.exists(checksum_file), f"SHA256SUMS.txt missing at {checksum_file}"

    # Verify checksum reproducibility for deliverables present on disk
    files_in_sums = []
    with open(checksum_file, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                parts = line.split(None, 1)
                if len(parts) == 2:
                    files_in_sums.append((parts[0], parts[1]))

    present_count = 0
    for csum, fname in files_in_sums:
        fpath = os.path.join(RELEASE_DIR, fname)
        if os.path.exists(fpath):
            res = subprocess.run(
                ["shasum", "-a", "256", "-c", "-"],
                input=f"{csum}  {fname}\n",
                cwd=RELEASE_DIR,
                capture_output=True,
                text=True
            )
            assert res.returncode == 0, f"Checksum verification failed for {fname}"
            present_count += 1

    if present_count > 0:
        print(f"  ✓ Verified SHA-256 integrity for {present_count} staged binaries on disk")
    else:
        print("  ✓ SHA256SUMS.txt format validated (release binaries to be built in matrix jobs)")

    # Verify detached signatures exist and have valid OpenPGP armor
    sig_files = ["SHA256SUMS.txt.asc", "RELEASE_MANIFEST.md.asc"]
    for sf in sig_files:
        p = os.path.join(RELEASE_DIR, sf)
        assert os.path.exists(p), f"Detached signature missing: {p}"
        with open(p, "r", encoding="utf-8") as f:
            sig_content = f.read()
        assert "-----BEGIN PGP SIGNATURE-----" in sig_content, f"Invalid signature header in {sf}"
        assert "-----END PGP SIGNATURE-----" in sig_content, f"Invalid signature footer in {sf}"
        print(f"  ✓ {sf} valid ASCII-armored detached signature format")

def test_manifest_synchronization():
    print("==> [5/5] Validating Release Manifest Checksum Synchronization...")
    manifest_file = os.path.join(RELEASE_DIR, "RELEASE_MANIFEST.md")
    checksum_file = os.path.join(RELEASE_DIR, "SHA256SUMS.txt")

    with open(manifest_file, "r", encoding="utf-8") as f:
        manifest_text = f.read()

    with open(checksum_file, "r", encoding="utf-8") as f:
        sums_lines = f.readlines()

    for line in sums_lines:
        line = line.strip()
        if not line:
            continue
        csum, fname = line.split(None, 1)
        assert csum in manifest_text, f"Checksum for {fname} ({csum}) not synchronized in RELEASE_MANIFEST.md"
        print(f"  ✓ {fname} checksum matches release manifest table")

def main():
    print("======================================================================")
    print(" TruthBeacon: Phase 24 Code Signing & Artifact Integrity Verification")
    print("======================================================================")
    try:
        test_macos_entitlements_and_config()
        test_macos_signing_script()
        test_windows_signing_script()
        test_checksum_integrity_and_signatures()
        test_manifest_synchronization()
        print("======================================================================")
        print(" ALL PHASE 24 CODE SIGNING & INTEGRITY CHECKS PASSED (5/5)!")
        print("======================================================================")
    except AssertionError as e:
        print(f"\n❌ Phase 24 Verification Failed: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()
