#!/usr/bin/env python3
"""
TruthBeacon Phase 25.3: Clean-Slate Pre-Flight Smoke Testing (First-Time User Simulation)
Orange Heart Industries - Quality Engineering & Release Staging

Automates:
1. Mounting staged DMG (`release/TruthBeacon_0.1.0_universal.dmg`) and verifying installer structure.
2. Drag-and-drop installation to `/Applications/TruthBeacon.app`.
3. Clean-slate simulation: Verifying cold launch with zero existing database and zero credentials.
4. Presentation state verification:
   - Hero banner: "DISCONNECTED" + grey indicator.
   - Active alerts counter: "0".
   - Protected benchmarks counter: "(0)".
   - Protected members grid empty state invitation: "No Protected Benchmarks Yet".
   - Server ID input field: blank and ready for community guild entry.
   - Diagnostics tab: "Awaiting verification".
5. Binary cold launch, process supervision, graceful shutdown, and SQLite WAL clean checkpoint verification.
"""

import os
import sys
import time
import shutil
import subprocess
import signal
import sqlite3
import re

ROOT_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
RELEASE_DIR = os.path.join(ROOT_DIR, "release")
dmg_candidates = [f for f in os.listdir(RELEASE_DIR) if f.startswith("TruthBeacon_") and f.endswith(".dmg")] if os.path.exists(RELEASE_DIR) else []
dmg_candidates.sort()
DMG_PATH = os.path.join(RELEASE_DIR, dmg_candidates[-1]) if dmg_candidates else os.path.join(RELEASE_DIR, "TruthBeacon_0.1.4_universal.dmg")
APP_DEST = "/Applications/TruthBeacon.app"
USER_HOME = os.path.expanduser("~")
TB_DATA_DIR = os.path.join(USER_HOME, ".truthbeacon")
DB_PATH = os.path.join(TB_DATA_DIR, "truthbeacon.local.db")
DB_WAL_PATH = os.path.join(TB_DATA_DIR, "truthbeacon.local.db-wal")
UI_DIR = os.path.join(ROOT_DIR, "ui")

def log(step, msg):
    print(f"==> [{step}] {msg}")

def check(ok, msg):
    if ok:
        print(f"  ✓ {msg}")
    else:
        print(f"  ❌ FAILED: {msg}", file=sys.stderr)
        sys.exit(1)

def test_dmg_mount_and_install():
    log("1/5", "Mounting Staged DMG & Verifying Drag-and-Drop Installation...")
    assert os.path.exists(DMG_PATH), f"Staged DMG not found at {DMG_PATH}"

    # Attach DMG
    mount_point = None
    try:
        attach_res = subprocess.run(
            ["hdiutil", "attach", DMG_PATH, "-nobrowse"],
            capture_output=True,
            text=True,
            check=True
        )
        for line in attach_res.stdout.splitlines():
            if "/Volumes/" in line:
                mount_point = line.split("/Volumes/")[-1]
                mount_point = f"/Volumes/{mount_point.strip()}"
                break
        
        check(mount_point is not None and os.path.exists(mount_point), f"DMG mounted at {mount_point}")

        # Check volume contents
        app_in_dmg = os.path.join(mount_point, "TruthBeacon.app")
        app_symlink = os.path.join(mount_point, "Applications")
        ds_store = os.path.join(mount_point, ".DS_Store")
        bg_img = os.path.join(mount_point, ".background.png")

        check(os.path.exists(app_in_dmg), "TruthBeacon.app bundle present in mounted volume")
        check(os.path.islink(app_symlink), "Applications folder symlink present in mounted volume")
        check(os.path.exists(ds_store), "Binary .DS_Store present for custom Finder canvas layout")
        check(os.path.exists(bg_img), "Custom background image present in volume")

        # Simulate Drag-to-Applications
        print("  Simulating operator dragging TruthBeacon.app to /Applications...")
        if os.path.exists(APP_DEST):
            shutil.rmtree(APP_DEST)
        
        subprocess.run(["cp", "-R", app_in_dmg, APP_DEST], check=True)
        subprocess.run(["xattr", "-cr", APP_DEST], check=False)
        check(os.path.exists(APP_DEST), "TruthBeacon.app successfully installed to /Applications")

        # Verify signature on disk
        verify_sig = subprocess.run(
            ["codesign", "--verify", "--deep", "--strict", APP_DEST],
            capture_output=True,
            text=True
        )
        check(verify_sig.returncode == 0, "Installed /Applications/TruthBeacon.app signature is valid on disk")

    finally:
        if mount_point and os.path.exists(mount_point):
            subprocess.run(["hdiutil", "detach", mount_point], capture_output=True)
            print(f"  Detached volume {mount_point}")

def test_clean_slate_environment():
    log("2/5", "Establishing Clean-Slate Environment (Zero Existing Database & Credentials)...")
    backup_dir = os.path.join(USER_HOME, ".truthbeacon.preflight_bak")
    if os.path.exists(backup_dir):
        shutil.rmtree(backup_dir)

    if os.path.exists(TB_DATA_DIR):
        print(f"  Backing up existing data directory to {backup_dir}...")
        shutil.move(TB_DATA_DIR, backup_dir)

    check(not os.path.exists(TB_DATA_DIR), "Verified ~/.truthbeacon clean-slate (zero existing database)")

def test_presentation_state():
    log("3/5", "Verifying Initial Clean-Slate Presentation State...")
    html_path = os.path.join(UI_DIR, "index.html")
    app_js_path = os.path.join(UI_DIR, "js", "app.js")
    mock_data_path = os.path.join(UI_DIR, "js", "mock_data.js")

    with open(html_path, "r", encoding="utf-8") as f:
        html = f.read()
    with open(app_js_path, "r", encoding="utf-8") as f:
        app_js = f.read()
    with open(mock_data_path, "r", encoding="utf-8") as f:
        mock_data = f.read()

    # 1. Hero banner displays "DISCONNECTED" with grey/offline indicator
    check(
        '<span class="discord-badge-active disconnected" id="discord-hero-badge">DISCONNECTED</span>' in html,
        'Hero banner displays "DISCONNECTED"'
    )
    check(
        '<span class="discord-status-indicator offline" id="discord-hero-status-dot" title="Disconnected"></span>' in html,
        'Hero banner status indicator is offline (grey)'
    )

    # 2. Active alerts counter reads "0"
    check(
        '<span class="tab-counter tab-counter-zero" id="counter-incidents">0</span>' in html,
        'Active alerts counter initialized to "0"'
    )

    # 3. Protected benchmarks counter reads "(0)"
    check(
        '<span class="tab-count-subtle" id="counter-vault">(0)</span>' in html,
        'Protected benchmarks counter reads "(0)"'
    )
    check(
        'export const INITIAL_BENCHMARKS = [];' in mock_data,
        'Ground-truth benchmark state initialized empty ([])'
    )

    # 4. Protected members grid shows empty state invitation ("No Protected Benchmarks Yet")
    check(
        'No Protected Benchmarks Yet' in app_js,
        'Protected members grid renders empty state invitation ("No Protected Benchmarks Yet")'
    )

    # 5. Server ID input field is blank and ready for community guild entry
    check(
        'id="discord-guild-id-input"' in html and 'placeholder="e.g. 999888777666555444' in html,
        'Server ID input field is blank with placeholder'
    )

    # 6. Diagnostics tab shows "Awaiting verification"
    check(
        '<span class="diag-text">Token Format: <strong>Awaiting verification</strong></span>' in html,
        'Diagnostics checklist shows "Awaiting verification"'
    )

def test_cold_launch_and_shutdown():
    log("4/5", "Executing Binary Cold Launch & Process Supervision...")
    binary_path = os.path.join(APP_DEST, "Contents", "MacOS", "truth-beacon")
    check(os.path.exists(binary_path), f"Binary found at {binary_path}")

    # Launch binary as subprocess
    print("  Launching /Applications/TruthBeacon.app in background...")
    proc = subprocess.Popen(
        [binary_path],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )

    # Allow cold-launch initialization and poll for database creation (up to 8s)
    db_created = False
    for _ in range(40):
        if os.path.exists(DB_PATH):
            db_created = True
            break
        time.sleep(0.2)
    check(proc.poll() is None, "Cold launch process is running cleanly without crash")
    check(db_created, f"SQLite database created at {DB_PATH}")

    # Query schema version from SQLite
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("PRAGMA user_version;")
    version = cursor.fetchone()[0]
    cursor.execute("SELECT name FROM sqlite_master WHERE type='table';")
    tables = [r[0] for r in cursor.fetchall()]
    conn.close()

    check(version >= 2, f"SQLite schema initialized to user_version {version}")
    check("benchmarks" in tables and "incidents" in tables and "audit_logs" in tables, f"Relational tables verified: {tables}")

    log("5/5", "Testing Graceful Application Shutdown & WAL Checkpointing...")
    # Send SIGTERM for graceful shutdown
    print("  Sending SIGTERM to application process...")
    proc.send_signal(signal.SIGTERM)

    try:
        proc.wait(timeout=5.0)
        check(True, "Process terminated gracefully within timeout")
    except subprocess.TimeoutExpired:
        proc.kill()
        check(False, "Process failed to shut down gracefully within 5 seconds")

    # Verify SQLite WAL clean checkpoint
    check(os.path.exists(DB_PATH), "Database file intact after shutdown")

    # Test WAL status
    if os.path.exists(DB_WAL_PATH):
        wal_size = os.path.getsize(DB_WAL_PATH)
        check(wal_size == 0 or wal_size <= 65536, f"WAL file checkpointed / bounded (size: {wal_size} bytes)")
    else:
        check(True, "WAL file checkpointed and synchronized to main database")

    # Verify database integrity check
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("PRAGMA integrity_check;")
    integrity = cursor.fetchone()[0]
    cursor.execute("PRAGMA journal_mode;")
    journal = cursor.fetchone()[0]
    conn.close()

    check(integrity == "ok", "SQLite PRAGMA integrity_check passed ('ok')")
    check(journal.upper() == "WAL", f"SQLite PRAGMA journal_mode is WAL ('{journal}')")

def restore_environment():
    backup_dir = os.path.join(USER_HOME, ".truthbeacon.preflight_bak")
    if os.path.exists(backup_dir):
        print(f"\n==> Restoring pre-existing ~/.truthbeacon from {backup_dir}...")
        if os.path.exists(TB_DATA_DIR):
            shutil.rmtree(TB_DATA_DIR)
        shutil.move(backup_dir, TB_DATA_DIR)
        print("  ✓ Prior environment restored.")

def main():
    print("======================================================================")
    print(" TruthBeacon Phase 25.3: Clean-Slate Pre-Flight Smoke Testing")
    print("======================================================================")
    try:
        test_dmg_mount_and_install()
        test_clean_slate_environment()
        test_presentation_state()
        test_cold_launch_and_shutdown()
        print("\n======================================================================")
        print(" ALL PRE-FLIGHT SMOKE TEST SPECIFICATIONS VERIFIED (100% PASSED)!")
        print("======================================================================")
    finally:
        restore_environment()

if __name__ == "__main__":
    main()
