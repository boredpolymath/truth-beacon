#!/usr/bin/env python3
"""
TruthBeacon macOS DMG Packager
Uses dmgbuild to inject binary .DS_Store with modern macOS 'pBBk' bookmark aliases,
guaranteeing Finder displays the custom Orange Heart Discord-charcoal canvas and icon positions.
"""

import os
import sys
import shutil

try:
    import dmgbuild
except ImportError:
    print("dmgbuild not found. Installing dmgbuild, ds-store, and mac-alias...")
    import subprocess
    subprocess.check_call([sys.executable, "-m", "pip", "install", "--user", "dmgbuild", "ds-store", "mac-alias"])
    import dmgbuild

import json

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
ROOT_DIR = os.path.dirname(SCRIPT_DIR)
SRC_TAURI_DIR = os.path.join(ROOT_DIR, "src-tauri")

with open(os.path.join(SRC_TAURI_DIR, "tauri.conf.json"), "r") as f:
    tauri_conf = json.load(f)
VERSION = tauri_conf.get("version", "0.1.1")

APP_BUNDLE = os.path.join(SRC_TAURI_DIR, "target/universal-apple-darwin/release/bundle/macos/TruthBeacon.app")
BG_IMAGE = os.path.join(SRC_TAURI_DIR, "icons/dmg-background.png")
ICON_ICNS = os.path.join(SRC_TAURI_DIR, "icons/icon.icns")

DMG_TARGET_DIR = os.path.join(SRC_TAURI_DIR, "target/universal-apple-darwin/release/bundle/dmg")
DMG_TARGET_FILE = os.path.join(DMG_TARGET_DIR, f"TruthBeacon_{VERSION}_universal.dmg")
RELEASE_DIR = os.path.join(ROOT_DIR, "release")
RELEASE_DMG_FILE = os.path.join(RELEASE_DIR, f"TruthBeacon_{VERSION}_universal.dmg")

def build_dmg():
    if not os.path.exists(APP_BUNDLE):
        raise FileNotFoundError(f"Application bundle not found at {APP_BUNDLE}. Run cargo tauri build -b app first.")
    
    if not os.path.exists(BG_IMAGE):
        raise FileNotFoundError(f"Background canvas image not found at {BG_IMAGE}")

    os.makedirs(DMG_TARGET_DIR, exist_ok=True)
    os.makedirs(RELEASE_DIR, exist_ok=True)

    if os.path.exists(DMG_TARGET_FILE):
        os.remove(DMG_TARGET_FILE)

    settings = {
        'format': 'UDZO',
        'filesystem': 'HFS+',
        'badge_icon': ICON_ICNS if os.path.exists(ICON_ICNS) else None,
        'background': BG_IMAGE,
        'window_rect': ((200, 150), (660, 400)),
        'default_view': 'icon-view',
        'show_status_bar': False,
        'show_tab_view': False,
        'show_toolbar': False,
        'show_pathbar': False,
        'show_sidebar': False,
        'icon_size': 128,
        'text_size': 14,
        'files': [APP_BUNDLE],
        'symlinks': {'Applications': '/Applications'},
        'hide_extensions': ['TruthBeacon.app'],
        'icon_locations': {
            'TruthBeacon.app': (180, 200),
            'Applications': (480, 200)
        }
    }

    print(f"Building macOS DMG with binary .DS_Store and 'pBBk' bookmark alias...")
    print(f"  Canvas: {BG_IMAGE} (660x400)")
    print(f"  App Icon Position: (180, 200)")
    print(f"  Applications Drop Position: (480, 200)")

    dmgbuild.build_dmg(DMG_TARGET_FILE, "TruthBeacon", settings=settings)

    print(f"Copying DMG to release staging directory: {RELEASE_DMG_FILE}")
    shutil.copyfile(DMG_TARGET_FILE, RELEASE_DMG_FILE)
    print(f"Successfully packaged DMG ({os.path.getsize(RELEASE_DMG_FILE):,} bytes)")

if __name__ == "__main__":
    build_dmg()
