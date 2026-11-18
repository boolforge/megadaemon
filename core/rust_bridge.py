#!/usr/bin/env python3
"""
MegaDaemon Rust Bridge v11.0 Apex
Zero-latency interface to the native ARM64 megadaemon-core engine:
- SEERE v11 GBNF KV-Pinned Harness
- Exynos 2100 NPU / Samsung EDEN / NNAPI / KleidiAI Hardware Accelerator Bridge
- Native Multi-Arch Disassembler (MIPS R3000A, M68K, SH-2, Z80, HuC6280)
- Native Multi-Platform ROM & Binary Auto-Discovery
- Native Konami XOR Script Deobfuscator & Asset Dumper
- Shannon Entropy, Wildcard AOB Pattern, 32-Bit Pointer Miner, Snapshot Diff, Zero-Copy SHM
"""

import os
import json
import subprocess
import tempfile

RUST_BIN = "/data/user/0/com.agy/files/megadaemon/rust-core/target/release/megadaemon-core"


class RustEngineBridge:
    @staticmethod
    def is_available():
        return os.path.exists(RUST_BIN) and os.access(RUST_BIN, os.X_OK)

    @staticmethod
    def run_npu_probe():
        if not RustEngineBridge.is_available():
            return {"status": "error", "message": "Native Rust binary not compiled"}
        try:
            res = subprocess.run([RUST_BIN, "npu-probe"], capture_output=True, text=True, check=True, timeout=5)
            return json.loads(res.stdout)
        except Exception as e:
            return {"status": "error", "message": f"NPU probe failed: {e}"}

    @staticmethod
    def run_seere(query):
        if not RustEngineBridge.is_available():
            return {"status": "error", "message": "Native Rust SEERE binary unavailable"}
        try:
            res = subprocess.run([RUST_BIN, "seere", query], capture_output=True, text=True, check=True, timeout=30)
            return json.loads(res.stdout)
        except Exception as e:
            return {"status": "error", "message": f"Rust SEERE failed: {e}"}

    @staticmethod
    def run_seere_selftest():
        if not RustEngineBridge.is_available():
            return "ERR_RUST_BIN_MISSING"
        try:
            res = subprocess.run([RUST_BIN, "seere-selftest"], capture_output=True, text=True, check=True, timeout=5)
            return res.stdout.strip()
        except Exception as e:
            return f"ERR_SELFTEST:{e}"

    @staticmethod
    def run_locate_roms(force_refresh=False):
        if not RustEngineBridge.is_available():
            from core.rom_locator import RomLocator
            return RomLocator.locate_all(force_refresh=force_refresh)
        try:
            cmd = [RUST_BIN, "locate-roms", "1" if force_refresh else "0"]
            res = subprocess.run(cmd, capture_output=True, text=True, check=True, timeout=8)
            return json.loads(res.stdout)
        except Exception:
            from core.rom_locator import RomLocator
            return RomLocator.locate_all(force_refresh=force_refresh)

    @staticmethod
    def run_disasm(plat, file_path, offset=0, count=16):
        if not RustEngineBridge.is_available():
            return {"error": "Native Rust disassembler unavailable", "instructions": []}
        try:
            cmd = [RUST_BIN, "disasm", plat, file_path, str(offset), str(count)]
            res = subprocess.run(cmd, capture_output=True, text=True, check=True, timeout=5)
            return json.loads(res.stdout)
        except Exception as e:
            return {"error": f"Rust disasm failed: {e}", "instructions": []}

    @staticmethod
    def run_hex(file_path, offset=0, length=256):
        if not RustEngineBridge.is_available():
            return {"error": "Native Rust hex inspector unavailable", "lines": []}
        try:
            cmd = [RUST_BIN, "hex", file_path, str(offset), str(length)]
            res = subprocess.run(cmd, capture_output=True, text=True, check=True, timeout=5)
            return json.loads(res.stdout)
        except Exception as e:
            return {"error": f"Rust hex inspect failed: {e}", "lines": []}

    @staticmethod
    def run_deobfuscate(plat, file_path):
        if not RustEngineBridge.is_available():
            return {"error": "Native Rust deobfuscator unavailable"}
        try:
            cmd = [RUST_BIN, "deobfuscate", plat, file_path]
            res = subprocess.run(cmd, capture_output=True, text=True, check=True, timeout=5)
            return json.loads(res.stdout)
        except Exception as e:
            return {"error": f"Rust deobfuscate failed: {e}"}

    @staticmethod
    def run_dumper(plat, file_path):
        if not RustEngineBridge.is_available():
            return {"error": "Native Rust dumper unavailable"}
        try:
            cmd = [RUST_BIN, "dumper", plat, file_path]
            res = subprocess.run(cmd, capture_output=True, text=True, check=True, timeout=5)
            return json.loads(res.stdout)
        except Exception as e:
            return {"error": f"Rust dumper failed: {e}"}

    @staticmethod
    def run_entropy(file_path, block_size=1024):
        if not RustEngineBridge.is_available():
            from core.entropy import EntropyScanner
            return EntropyScanner.scan_file(file_path, block_size=block_size)
        try:
            cmd = [RUST_BIN, "entropy", file_path, str(block_size)]
            res = subprocess.run(cmd, capture_output=True, text=True, check=True)
            out = json.loads(res.stdout)
            out["engine"] = "rust-arm64-v11.0"
            return out
        except Exception as e:
            from core.entropy import EntropyScanner
            fallback = EntropyScanner.scan_file(file_path, block_size=block_size)
            fallback["engine"] = f"python-fallback ({e})"
            return fallback

    @staticmethod
    def run_pattern(file_path, pattern_str, max_matches=128):
        if not RustEngineBridge.is_available():
            return {"error": "Native pattern engine not available and no Python equivalent"}
        try:
            cmd = [RUST_BIN, "pattern", file_path, pattern_str, str(max_matches)]
            res = subprocess.run(cmd, capture_output=True, text=True, check=True)
            out = json.loads(res.stdout)
            out["engine"] = "rust-arm64-v11.0"
            return out
        except Exception as e:
            return {"error": f"Pattern scan failed: {e}"}

    @staticmethod
    def run_pointers(file_path, base_vaddr=0x80010000, is_be=False, max_matches=128):
        if not RustEngineBridge.is_available():
            from core.pointers import PointerTableMiner
            return PointerTableMiner.scan_binary(file_path)
        try:
            cmd = [RUST_BIN, "pointers", file_path, hex(base_vaddr), "1" if is_be else "0", str(max_matches)]
            res = subprocess.run(cmd, capture_output=True, text=True, check=True)
            out = json.loads(res.stdout)
            out["engine"] = "rust-arm64-v11.0"
            return out
        except Exception as e:
            from core.pointers import PointerTableMiner
            fallback = PointerTableMiner.scan_binary(file_path)
            fallback["engine"] = f"python-fallback ({e})"
            return fallback

    @staticmethod
    def run_diff(raw_a, raw_b, base_addr=0, max_deltas=256):
        if not RustEngineBridge.is_available():
            from core.snapshot_diff import MemorySnapshotDiff
            return MemorySnapshotDiff.diff_buffers(raw_a, raw_b, base_addr=base_addr)
        try:
            with tempfile.NamedTemporaryFile("wb", delete=False) as f_a, \
                 tempfile.NamedTemporaryFile("wb", delete=False) as f_b:
                f_a.write(raw_a)
                f_b.write(raw_b)
                p_a, p_b = f_a.name, f_b.name

            try:
                cmd = [RUST_BIN, "diff", p_a, p_b, hex(base_addr), str(max_deltas)]
                res = subprocess.run(cmd, capture_output=True, text=True, check=True)
                out = json.loads(res.stdout)
                out["engine"] = "rust-arm64-v11.0"
                return out
            finally:
                if os.path.exists(p_a):
                    os.unlink(p_a)
                if os.path.exists(p_b):
                    os.unlink(p_b)
        except Exception as e:
            from core.snapshot_diff import MemorySnapshotDiff
            fallback = MemorySnapshotDiff.diff_buffers(raw_a, raw_b, base_addr=base_addr)
            fallback["engine"] = f"python-fallback ({e})"
            return fallback

    @staticmethod
    def run_shm_status():
        if not RustEngineBridge.is_available():
            return None
        try:
            cmd = [RUST_BIN, "shm-status"]
            res = subprocess.run(cmd, capture_output=True, text=True, check=True)
            return json.loads(res.stdout)
        except Exception:
            return None

    @staticmethod
    def get_shm_frame_bmp():
        if not RustEngineBridge.is_available():
            return None
        try:
            cmd = [RUST_BIN, "shm-frame-bmp"]
            res = subprocess.run(cmd, capture_output=True, check=True)
            return res.stdout
        except Exception:
            return None
