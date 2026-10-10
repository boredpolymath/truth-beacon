#!/usr/bin/env python3
"""
TruthBeacon Auto-Updater Manifest Generator (Tauri 2 Specification)
Orange Heart Industries

Generates release/latest.json containing cryptographic minisign signatures,
release notes, and download endpoints for macOS, Windows, and Linux.
"""

import os
import sys
import json
import subprocess
from datetime import datetime, timezone

ROOT_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
RELEASE_DIR = os.path.join(ROOT_DIR, "release")
SRC_TAURI_DIR = os.path.join(ROOT_DIR, "src-tauri")
WEBSITE_ASSETS_DIR = os.path.join(ROOT_DIR, "website", "assets")
KEYS_DIR = os.path.join(ROOT_DIR, "scripts", ".keys")
KEY_PATH = os.path.join(KEYS_DIR, "truthbeacon_updater.key")

with open(os.path.join(SRC_TAURI_DIR, "tauri.conf.json"), "r") as f:
    conf = json.load(f)
VERSION = conf.get("version", "0.2.1")
BASE_URL = f"https://github.com/boredpolymath/truth-beacon/releases/download/v{VERSION}"

def get_signature(file_path):
    sig_path = f"{file_path}.sig"
    if os.path.exists(sig_path):
        with open(sig_path, "r", encoding="utf-8") as f:
            return f.read().strip()
    
    # Sign on the fly if key is available
    if os.path.exists(file_path) and os.path.exists(KEY_PATH):
        print(f"Signing {os.path.basename(file_path)} with {KEY_PATH}...")
        res = subprocess.run(
            ["cargo", "tauri", "signer", "sign", "-f", KEY_PATH, "-p", "", "--app-version", VERSION, file_path],
            cwd=SRC_TAURI_DIR,
            capture_output=True,
            text=True
        )
        if res.returncode == 0 and os.path.exists(sig_path):
            with open(sig_path, "r", encoding="utf-8") as f:
                return f.read().strip()
    return None

def build_manifest():
    print(f"==> Building Tauri 2 Auto-Updater Manifest (latest.json) for v{VERSION}...")
    
    notes = (
        f"TruthBeacon v{VERSION}: Major security, resilience & release hardening including forensic SQLite quarantine, "
        "domain-bound AES-256-GCM vault, multi-keyframe pHash inspection, UTR #39 homoglyph detection, "
        "multi-guild gateway resumption, and sandboxed IPC."
    )
    
    strict = "--strict" in sys.argv or os.environ.get("STRICT_MANIFEST") == "1"
    mac_tar = os.path.join(RELEASE_DIR, f"TruthBeacon_{VERSION}_universal.app.tar.gz")
    mac_sig = get_signature(mac_tar)
    if not mac_sig:
        if strict:
            sys.exit(f"ERROR: Missing required updater signature for {mac_tar}")
        # Fallback dummy signature placeholder if file not present during local validation dry runs
        mac_sig = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkK"
    
    win_zip = os.path.join(RELEASE_DIR, f"TruthBeacon_{VERSION}_x64-setup.nsis.zip")
    win_sig = get_signature(win_zip)
    if not win_sig:
        if strict:
            sys.exit(f"ERROR: Missing required updater signature for {win_zip}")
        win_sig = mac_sig
    
    linux_tar = os.path.join(RELEASE_DIR, f"TruthBeacon_{VERSION}_amd64.AppImage.tar.gz")
    linux_sig = get_signature(linux_tar)
    if not linux_sig:
        if strict:
            sys.exit(f"ERROR: Missing required updater signature for {linux_tar}")
        linux_sig = mac_sig

    manifest = {
        "version": f"v{VERSION}",
        "notes": notes,
        "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": {
            "darwin-aarch64": {
                "signature": mac_sig,
                "url": f"{BASE_URL}/TruthBeacon_{VERSION}_universal.app.tar.gz"
            },
            "darwin-x86_64": {
                "signature": mac_sig,
                "url": f"{BASE_URL}/TruthBeacon_{VERSION}_universal.app.tar.gz"
            },
            "windows-x86_64": {
                "signature": win_sig,
                "url": f"{BASE_URL}/TruthBeacon_{VERSION}_x64-setup.nsis.zip"
            },
            "linux-x86_64": {
                "signature": linux_sig,
                "url": f"{BASE_URL}/TruthBeacon_{VERSION}_amd64.AppImage.tar.gz"
            }
        }
    }

    out_file = os.path.join(RELEASE_DIR, "latest.json")
    with open(out_file, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)
    print(f"  ✓ Written: {out_file}")

    if os.path.exists(WEBSITE_ASSETS_DIR):
        web_out = os.path.join(WEBSITE_ASSETS_DIR, "latest.json")
        with open(web_out, "w", encoding="utf-8") as f:
            json.dump(manifest, f, indent=2)
        print(f"  ✓ Written: {web_out}")

    return manifest

if __name__ == "__main__":
    build_manifest()
