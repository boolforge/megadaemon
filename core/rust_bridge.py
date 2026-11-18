#!/usr/bin/env python3
"""
MegaDaemon Rust Bridge
Zero-latency interface to the native ARM64 megadaemon-core engine.
Provides transparent acceleration with automatic Python fallback.
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
    def run_entropy(file_path, block_size=1024):
        if not RustEngineBridge.is_available():
            from core.entropy import EntropyScanner
            return EntropyScanner.scan_file(file_path, block_size=block_size)
        try:
            cmd = [RUST_BIN, "entropy", file_path, str(block_size)]
            res = subprocess.run(cmd, capture_output=True, text=True, check=True)
            out = json.loads(res.stdout)
            out["engine"] = "rust-arm64-v6.0"
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
            out["engine"] = "rust-arm64-v6.0"
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
            out["engine"] = "rust-arm64-v6.0"
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
                out["engine"] = "rust-arm64-v6.0"
                return out
            finally:
                if os.path.exists(p_a): os.unlink(p_a)
                if os.path.exists(p_b): os.unlink(p_b)
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
