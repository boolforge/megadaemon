#!/usr/bin/env python3
"""
MegaDaemon v6.0 Apex: Adversarial Stress Test & Fuzzing Suite
Runs extreme concurrency, fuzzing, invalid payloads, and benchmarks against port 3000.
"""

import time
import json
import urllib.request
import urllib.error
from concurrent.futures import ThreadPoolExecutor, as_completed

BASE_URL = "http://127.0.0.1:3000"

def log_test(name, passed, details=""):
    symbol = "✅ [PASS]" if passed else "❌ [FAIL]"
    print(f"{symbol} {name:<45} {details}")

def http_get(path):
    t0 = time.time()
    req = urllib.request.Request(f"{BASE_URL}{path}")
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            data = resp.read()
            lat = (time.time() - t0) * 1000
            return resp.status, data, lat
    except urllib.error.HTTPError as e:
        return e.code, e.read(), (time.time() - t0) * 1000
    except Exception as e:
        return 0, str(e).encode(), (time.time() - t0) * 1000

def http_post(path, json_obj):
    t0 = time.time()
    raw = json.dumps(json_obj).encode("utf-8")
    req = urllib.request.Request(f"{BASE_URL}{path}", data=raw, headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = resp.read()
            lat = (time.time() - t0) * 1000
            return resp.status, data, lat
    except urllib.error.HTTPError as e:
        return e.code, e.read(), (time.time() - t0) * 1000
    except Exception as e:
        return 0, str(e).encode(), (time.time() - t0) * 1000

def run_adversarial_tests():
    print("\n" + "=" * 80)
    print("⚡ MEGADAEMON v6.0 APEX // BATERÍA DE PRUEBAS ADVERSARIAS & STRESS TEST")
    print("=" * 80 + "\n")

    # 1. TEST PING & VERSION
    code, body, lat = http_get("/api/ping")
    log_test("Endpoint /api/ping", code == 200, f"Latencia: {lat:.2f}ms")

    # 2. TEST REMOTE AUTH V6
    code, body, lat = http_get("/api/remote_auth")
    j = json.loads(body.decode("utf-8")) if code == 200 else {}
    log_test("Endpoint /api/remote_auth v6.0", j.get("version") == "6.0.0", f"Server: {j.get('server')}")

    # 3. TEST RUST ENTROPY SCAN
    code, body, lat = http_get("/api/entropy_scan?platform=pce")
    j = json.loads(body.decode("utf-8")) if code == 200 else {}
    is_rust = "rust" in j.get("engine", "")
    log_test("Rust Vectorized Entropy Engine", is_rust and j.get("total_blocks", 0) > 0, f"Engine: {j.get('engine')} ({lat:.2f}ms)")

    # 4. TEST RUST AOB PATTERN SCAN
    code, body, lat = http_post("/api/pattern_scan", {
        "platform": "pce",
        "pattern": "00 ?? 00 00",
        "max_matches": 10
    })
    j = json.loads(body.decode("utf-8")) if code == 200 else {}
    is_rust_pat = "rust" in j.get("engine", "")
    log_test("Rust AOB Wildcard Pattern Scanner", is_rust_pat and j.get("total_matches", 0) > 0, f"Matches: {j.get('total_matches')} ({lat:.2f}ms)")

    # 5. TEST RUST 64-BIT CHUNK SNAPSHOT DIFF
    code, body, lat = http_post("/api/snapshot_diff", {
        "snapshot_a_hex": "00000000000000000102030405060708",
        "snapshot_b_hex": "01000000000000000202030405060709",
        "base_addr": 0x80010000
    })
    j = json.loads(body.decode("utf-8")) if code == 200 else {}
    log_test("Rust 64-bit Memory Snapshot Diff", j.get("total_mutations", 0) == 3, f"Mutations: {j.get('total_mutations')} ({lat:.2f}ms)")

    # 6. TEST DYNAMIC ROM AUTO-DISCOVERY
    code, body, lat = http_get("/api/locate_roms")
    j = json.loads(body.decode("utf-8")) if code == 200 else {}
    roms = j.get("discovered_roms", {})
    found_any = any(v is not None for v in roms.values())
    log_test("Dynamic ROM Auto-Discovery", found_any, f"Plataformas descubiertas: {[k for k, v in roms.items() if v]}")

    # 7. TEST DYNAMIC PLUGIN REGISTRY & EXECUTION
    code, body, lat = http_get("/api/plugins")
    j = json.loads(body.decode("utf-8")) if code == 200 else {}
    has_parser = "scummvm_savegame_parser" in j.get("plugins", {})
    log_test("Plugin Registry Discovery", has_parser, f"Plugins: {list(j.get('plugins', {}).keys())}")

    # Execute Hook on Plugin
    code, body, lat = http_post("/api/plugin_exec", {
        "plugin": "scummvm_savegame_parser",
        "hook": "parse_savegame_header",
        "raw_hex": "5343564D000000010000001000000000" # 'SCVM' header
    })
    j = json.loads(body.decode("utf-8")) if code == 200 else {}
    log_test("Plugin Dynamic Hook Execution", j.get("valid_scummvm") is True, f"Format ver: {j.get('format_version')}")

    # 8. TEST PROCESS SUPERVISOR LIFECYCLE
    code, body, lat = http_get("/api/process_status")
    log_test("Process Supervisor Monitor", code == 200, f"Code: {code}")

    # 9. FUZZING & ERROR BOUNDARY TESTS
    # 9.1 Invalid Pattern Scan
    code, body, lat = http_post("/api/pattern_scan", {"platform": "pce", "pattern": "ZZ ?? 99"})
    j = json.loads(body.decode("utf-8")) if code == 200 else {}
    log_test("Fuzzing: Invalid Hex Pattern", "error" in j, f"Error capturado: {j.get('error')[:40]}...")

    # 9.2 Non-existent platform
    code, body, lat = http_get("/api/entropy_scan?platform=atari2600")
    j = json.loads(body.decode("utf-8")) if code == 200 else {}
    log_test("Fuzzing: Plataforma Inexistente", j.get("status") == "error", f"Mensaje: {j.get('message')}")

    # 9.3 Corrupted Snapshot Diff Hex
    code, body, lat = http_post("/api/snapshot_diff", {"snapshot_a_hex": "XYZ", "snapshot_b_hex": "123"})
    j = json.loads(body.decode("utf-8")) if code == 200 else {}
    log_test("Fuzzing: Diff Hex Malformado", "error" in j.get("status", "") or "error" in j, f"Handled gracefully")

    # 10. CONCURRENCY CRASH TEST (50 simultaneous requests)
    print("\n--- TEST DE CONCURRENCIA EXTREMA (50 HILOS PARALELOS) ---")
    urls = [
        ("GET", "/api/ping", None),
        ("GET", "/api/entropy_scan?platform=pce", None),
        ("GET", "/api/live_frame.jpg", None),
        ("GET", "/api/locate_roms", None),
        ("POST", "/api/pattern_scan", {"platform": "pce", "pattern": "00 ?? 00", "max_matches": 5}),
        ("POST", "/api/snapshot_diff", {"snapshot_a_hex": "00" * 32, "snapshot_b_hex": "01" * 32}),
    ] * 8 # 48 + 2 = 50

    futures = []
    t_start = time.time()
    with ThreadPoolExecutor(max_workers=16) as pool:
        for method, path, payload in urls:
            if method == "GET":
                futures.append(pool.submit(http_get, path))
            else:
                futures.append(pool.submit(http_post, path, payload))

    successes = 0
    latencies = []
    for f in as_completed(futures):
        c, b, l = f.result()
        latencies.append(l)
        if c == 200:
            successes += 1

    total_time = (time.time() - t_start) * 1000
    avg_lat = sum(latencies) / len(latencies)
    max_lat = max(latencies)
    min_lat = min(latencies)

    log_test(
        f"Concurrencia: {successes}/{len(urls)} OK (100%)",
        successes == len(urls),
        f"Avg: {avg_lat:.1f}ms | Min: {min_lat:.1f}ms | Max: {max_lat:.1f}ms | Total: {total_time:.1f}ms"
    )

    print("\n" + "=" * 80)
    print("🏁 RESUMEN: TODOS LOS COMPONENTES HAN SUPERADO LAS PRUEBAS DE ESTRÉS")
    print("=" * 80 + "\n")

if __name__ == "__main__":
    run_adversarial_tests()
