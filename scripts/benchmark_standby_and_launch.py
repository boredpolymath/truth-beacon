#!/usr/bin/env python3
"""
TruthBeacon: Standby Memory & Cold Launch Performance Verification (Phase 20.2)
Verifies:
1. Physical RAM consumption strictly under 30 MB during idle monitoring.
2. Cold launch startup to listening state completes in < 2.5 seconds (2500 ms).
"""

import subprocess
import sys
import re

def run_benchmarks():
    print("=" * 70)
    print("TruthBeacon: Standby Memory & Cold Launch Benchmark (Phase 20.2)")
    print("=" * 70)

    cmd = [
        "cargo", "test", "--lib", "gateway::daemon::tests", "--", "--nocapture"
    ]
    try:
        proc = subprocess.run(
            cmd,
            cwd="src-tauri",
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            check=True
        )
    except subprocess.CalledProcessError as e:
        print(f"❌ Benchmark execution failed with exit code {e.returncode}:")
        print(e.output)
        sys.exit(1)

    output = proc.stdout

    # Parse Standby Memory
    # Pattern: [Standby Memory Test] Resident Memory: 13.55 MB (14172160 bytes) | Budget: 30.00 MB
    mem_match = re.search(r"Resident Memory:\s+([0-9.]+)\s+MB.*?Budget:\s+([0-9.]+)\s+MB", output)
    launch_match = re.search(r"Startup to listening state completed in\s+([0-9.]+)ms\s+\(([0-9.]+)s\)", output)

    if not mem_match or not launch_match:
        print("❌ Could not parse benchmark metrics from cargo test output.")
        print("Raw output:\n", output)
        sys.exit(1)

    resident_mb = float(mem_match.group(1))
    budget_mb = float(mem_match.group(2))
    launch_ms = float(launch_match.group(1))
    launch_s = float(launch_match.group(2))

    print(f"\n1. Standby Memory Consumption:")
    print(f"   • Measured Resident RSS: {resident_mb:.2f} MB")
    print(f"   • Budget Limit:          {budget_mb:.2f} MB")
    if resident_mb < budget_mb:
        print(f"   ✅ PASS: Physical RAM consumption ({resident_mb:.2f} MB) is strictly under {budget_mb:.2f} MB.")
    else:
        print(f"   ❌ FAIL: Physical RAM consumption ({resident_mb:.2f} MB) exceeded {budget_mb:.2f} MB budget!")
        sys.exit(1)

    print(f"\n2. Cold Launch Benchmark:")
    print(f"   • Measured Cold Launch:  {launch_ms:.2f} ms ({launch_s:.4f} s)")
    print(f"   • Budget Limit:          2500.00 ms (2.5000 s)")
    if launch_ms < 2500.0:
        print(f"   ✅ PASS: Cold launch to listening state ({launch_ms:.2f} ms) completed in < 2.5 seconds.")
    else:
        print(f"   ❌ FAIL: Cold launch took {launch_ms:.2f} ms, exceeding 2.5s budget!")
        sys.exit(1)

    print("\n" + "=" * 70)
    print("ALL PERFORMANCE & RESOURCE STRESS BENCHMARKS PASSED (100% COMPLIANT)")
    print("=" * 70)

if __name__ == "__main__":
    run_benchmarks()
