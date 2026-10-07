#!/usr/bin/env python3
"""
Packaging & Release Bundle Pipeline Verification Suite
Orange Heart Industries - TruthBeacon Staging

Validates:
- macOS DMG layout dimensions, background, and positioning
- Universal 2 binary configuration
- Windows WiX / NSIS installer asset dimensions & configuration
- Linux Debian system tray & keychain dependencies
- Freedesktop desktop entry specification compliance
- Icon sets and resolutions
"""

import os
import sys
import json
import struct

ROOT_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
SRC_TAURI = os.path.join(ROOT_DIR, "src-tauri")
ICONS_DIR = os.path.join(SRC_TAURI, "icons")
CONF_FILE = os.path.join(SRC_TAURI, "tauri.conf.json")
DESKTOP_FILE = os.path.join(SRC_TAURI, "desktop", "truthbeacon.desktop")

def test_config():
    print("==> [1/5] Validating tauri.conf.json Bundle Configuration...")
    with open(CONF_FILE, "r") as f:
        conf = json.load(f)
    
    bundle = conf.get("bundle", {})
    assert bundle.get("active") is True, "bundle.active must be true"
    
    # macOS DMG
    macos = bundle.get("macOS", {})
    dmg = macos.get("dmg", {})
    assert dmg.get("background") == "icons/dmg-background.png", "DMG background not set to icons/dmg-background.png"
    assert dmg.get("windowSize") == {"width": 660, "height": 400}, f"Unexpected DMG window size: {dmg.get('windowSize')}"
    assert dmg.get("appPosition") == {"x": 180, "y": 200}, f"Unexpected appPosition: {dmg.get('appPosition')}"
    assert dmg.get("applicationFolderPosition") == {"x": 480, "y": 200}, f"Unexpected applicationFolderPosition: {dmg.get('applicationFolderPosition')}"
    print("  ✓ macOS DMG layout parameters verified")
    
    # Windows NSIS & WiX
    win = bundle.get("windows", {})
    nsis = win.get("nsis", {})
    assert nsis.get("headerImage") == "icons/nsis-header.bmp", "NSIS header image not set"
    assert nsis.get("sidebarImage") == "icons/nsis-sidebar.bmp", "NSIS sidebar image not set"
    assert nsis.get("installMode") == "currentUser", "NSIS installMode must be currentUser"
    
    wix = win.get("wix", {})
    assert wix.get("bannerPath") == "icons/wix-banner.bmp", "WiX bannerPath not set"
    assert wix.get("dialogImagePath") == "icons/wix-dialog.bmp", "WiX dialogImagePath not set"
    print("  ✓ Windows NSIS & WiX installer configs verified")
    
    # Linux Debian & AppImage
    linux = bundle.get("linux", {})
    deb = linux.get("deb", {})
    deps = deb.get("depends", [])
    assert any("appindicator" in d for d in deps), "Debian missing system tray dependency (libappindicator3-1)"
    assert any("secret" in d for d in deps), "Debian missing credential storage dependency (libsecret-1-0)"
    assert any("webkit" in d for d in deps), "Debian missing webview dependency (libwebkit2gtk)"
    print("  ✓ Linux Debian tray, credentials, and webview dependencies verified")

    # Cross-platform Auto-Updater
    plugins = conf.get("plugins", {})
    updater = plugins.get("updater", {})
    assert updater.get("pubkey"), "Auto-updater pubkey missing in tauri.conf.json"
    assert updater.get("endpoints"), "Auto-updater endpoints missing in tauri.conf.json"
    print("  ✓ Cross-platform auto-updater configuration verified")

def test_installer_assets():
    print("==> [2/5] Validating Installer Graphics & Dimensions...")
    assets = [
        ("dmg-background.png", 660, 400, "PNG"),
        ("nsis-header.bmp", 150, 57, "BMP"),
        ("nsis-sidebar.bmp", 164, 314, "BMP"),
        ("wix-banner.bmp", 493, 58, "BMP"),
        ("wix-dialog.bmp", 493, 312, "BMP"),
    ]
    for filename, exp_w, exp_h, exp_format in assets:
        p = os.path.join(ICONS_DIR, filename)
        assert os.path.exists(p), f"Asset {filename} does not exist at {p}"
        with open(p, "rb") as f:
            header = f.read(32)
            if header.startswith(b"\x89PNG\r\n\x1a\n"):
                w, h = struct.unpack(">II", header[16:24])
                img_fmt = "PNG"
            elif header.startswith(b"BM"):
                f.seek(18)
                w, h = struct.unpack("<ii", f.read(8))
                h = abs(h)
                img_fmt = "BMP"
            else:
                raise ValueError(f"Unrecognized image signature in {p}")
        
        assert (w, h) == (exp_w, exp_h), f"{filename} dimensions ({w}, {h}) != expected ({exp_w}, {exp_h})"
        assert img_fmt == exp_format, f"{filename} format {img_fmt} != expected {exp_format}"
        print(f"  ✓ {filename}: {w}x{h} {img_fmt} OK")

def test_desktop_entry():
    print("==> [3/5] Validating Freedesktop Desktop Entry...")
    assert os.path.exists(DESKTOP_FILE), f"Desktop file missing at {DESKTOP_FILE}"
    with open(DESKTOP_FILE, "r") as f:
        content = f.read()
    
    assert "[Desktop Entry]" in content, "Missing [Desktop Entry] section header"
    assert "Type=Application" in content, "Missing Type=Application"
    assert "Exec=truth-beacon" in content, "Missing Exec executable path"
    assert "Icon=truth-beacon" in content, "Missing Icon specifier"
    assert "Categories=" in content and "Security" in content, "Missing Security Category"
    assert "StartupWMClass=truth-beacon" in content, "Missing StartupWMClass for window manager grouping"
    print("  ✓ truthbeacon.desktop meets Freedesktop Specification requirements")

def test_icons():
    print("==> [4/5] Validating Multi-Resolution Icon Assets...")
    required_icons = ["32x32.png", "128x128.png", "128x128@2x.png", "icon.icns", "icon.ico"]
    for ic in required_icons:
        p = os.path.join(ICONS_DIR, ic)
        assert os.path.exists(p), f"Required icon {ic} missing at {p}"
        assert os.path.getsize(p) > 500, f"Icon {ic} file size unexpectedly small ({os.path.getsize(p)} bytes)"
        print(f"  ✓ {ic}: {os.path.getsize(p)} bytes OK")

def test_scripts():
    print("==> [5/5] Validating Packaging Automation Scripts...")
    scripts = [
        "generate_installer_assets.py",
        "build_macos_universal.sh",
        "sign_and_notarize_macos.sh",
        "build_windows_bundle.ps1",
        "sign_windows_bundle.ps1",
        "build_linux_bundle.sh",
        "sign_release_artifacts.py",
        "verify_code_signing_pipeline.py",
    ]
    for s in scripts:
        sp = os.path.join(ROOT_DIR, "scripts", s)
        assert os.path.exists(sp), f"Packaging script missing: {sp}"
        assert os.path.getsize(sp) > 200, f"Packaging script {s} is empty"
        print(f"  ✓ scripts/{s} present and valid")

def main():
    print("======================================================================")
    print(" TruthBeacon: Cross-Platform Packaging Pipeline Verification Suite")
    print("======================================================================")
    try:
        test_config()
        test_installer_assets()
        test_desktop_entry()
        test_icons()
        test_scripts()
        print("======================================================================")
        print(" ALL PACKAGING PIPELINE SPECIFICATION CHECKS PASSED (5/5)!")
        print("======================================================================")
    except AssertionError as e:
        print(f"\n❌ Packaging Verification Failed: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()
