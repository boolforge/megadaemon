#!/usr/bin/env python3
"""
MegaDaemon Dynamic ROM & Binary Auto-Discovery with High-Speed Thread-Safe Cache
Finds historical game binaries across standard Android storage locations.
"""

import os
import time
import threading

SEARCH_ROOTS = [
    "/storage/emulated/0/Download/Snatcher-Collection",
    "/storage/emulated/0/Download",
    "/storage/emulated/0/Documents",
    "/data/user/0/com.agy/files/megadaemon/extracted",
    "/data/user/0/com.agy/files/scummvm"
]

PLATFORM_PATTERNS = {
    "ps1": ["SLPS_001.54", "SLPS", ".chd", "snatcher.bin"],
    "saturn": ["MAIN_L.BIN", "0.BIN"],
    "pce": ["scene_06_sec0158_bank16.bin", "Track 02", "pce_mode1_clean.iso"],
    "msx2": ["SNATCHER.COM", "SNATCHER.FNT", "Disk 1 of 3.dsk"],
    "pc88": ["pc88_linear.img", "Disk 1 of 5", "Disk1-Program_linear.img"],
    "segacd": ["SUBCODE.BIN", "segacd"]
}

_cached_roms = None
_cached_time = 0
_cache_lock = threading.Lock()

class RomLocator:
    @staticmethod
    def locate_all(force_refresh=False):
        global _cached_roms, _cached_time
        now = time.time()
        with _cache_lock:
            if not force_refresh and _cached_roms is not None and (now - _cached_time) < 60.0:
                return _cached_roms

            discovered = {}
            for plat, needles in PLATFORM_PATTERNS.items():
                discovered[plat] = None
                for root in SEARCH_ROOTS:
                    if not os.path.exists(root):
                        continue
                    for dirpath, _, filenames in os.walk(root):
                        for fname in filenames:
                            for needle in needles:
                                if needle.lower() in fname.lower():
                                    full = os.path.join(dirpath, fname)
                                    try:
                                        sz = os.path.getsize(full)
                                        discovered[plat] = {
                                            "path": full,
                                            "filename": fname,
                                            "size_bytes": sz,
                                            "readable": os.access(full, os.R_OK)
                                        }
                                        break
                                    except Exception:
                                        pass
                            if discovered[plat]:
                                break
                        if discovered[plat]:
                            break

            _cached_roms = discovered
            _cached_time = now
            return _cached_roms

    @staticmethod
    def resolve_binary(plat_key):
        all_roms = RomLocator.locate_all()
        return all_roms.get(plat_key.lower())
