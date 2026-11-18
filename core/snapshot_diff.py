#!/usr/bin/env python3
"""
=============================================================================
MEGADAEMON DIFFERENTIAL MEMORY TAINT & SNAPSHOT TRACKER (v5.0)
=============================================================================
Differential RAM snapshot comparator. Pinpoints exact memory offsets modified
between emulator frames or game scenes (flags, counters, inventory, scene IDs).
=============================================================================
"""

import os
from typing import Dict, List, Any

class MemorySnapshotDiff:
    """Calculates diffs and classifies memory mutations between two states."""

    @staticmethod
    def diff_buffers(snap_a: bytes, snap_b: bytes, base_addr: int = 0, max_diffs: int = 128) -> Dict[str, Any]:
        min_len = min(len(snap_a), len(snap_b))
        if min_len == 0:
            return {"status": "error", "message": "One or both snapshots are empty"}

        diffs = []
        changed_bytes_count = 0

        # Scan for changed bytes
        for i in range(min_len):
            val_a = snap_a[i]
            val_b = snap_b[i]
            if val_a != val_b:
                changed_bytes_count += 1
                if len(diffs) < max_diffs:
                    delta = val_b - val_a
                    classification = "UNKNOWN"
                    if val_a == 0 and val_b == 1:
                        classification = "FLAG_SET"
                    elif val_a == 1 and val_b == 0:
                        classification = "FLAG_CLEARED"
                    elif delta == 1:
                        classification = "COUNTER_INCREMENT"
                    elif delta == -1:
                        classification = "COUNTER_DECREMENT"
                    elif 0x20 <= val_b <= 0x7E:
                        classification = f"TEXT_CHAR_'{chr(val_b)}'"

                    diffs.append({
                        "offset": f"0x{i:06X}",
                        "addr": f"0x{(base_addr + i):08X}",
                        "old_val": f"0x{val_a:02X}",
                        "new_val": f"0x{val_b:02X}",
                        "delta": delta,
                        "type": classification
                    })

        return {
            "status": "ok",
            "bytes_compared": min_len,
            "changed_bytes": changed_bytes_count,
            "mutation_rate_pct": round((changed_bytes_count / min_len) * 100, 3),
            "diffs_sample": diffs
        }

    @classmethod
    def diff_files(cls, file_a: str, file_b: str, base_addr: int = 0) -> Dict[str, Any]:
        if not os.path.exists(file_a) or not os.path.exists(file_b):
            return {"status": "error", "message": "One or both snapshot files missing"}
        try:
            with open(file_a, "rb") as fa, open(file_b, "rb") as fb:
                return cls.diff_buffers(fa.read(), fb.read(), base_addr=base_addr)
        except Exception as e:
            return {"status": "error", "message": str(e)}

if __name__ == "__main__":
    a = b"\x00\x00\x05\x10\x00" * 20
    b = b"\x01\x00\x06\x10\x00" * 20
    print(MemorySnapshotDiff.diff_buffers(a, b, base_addr=0x80000000))
