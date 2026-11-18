#!/usr/bin/env python3
"""
=============================================================================
MEGADAEMON CROSS-ARCHITECTURE POINTER & SCRIPT MINER (v5.0)
=============================================================================
Automated discovery of pointer tables and embedded dialogue text across
MIPS (PS1), SH-2 (Saturn), 68000 (Sega CD), Z80 (MSX2 / PC-88), and HuC6280 (PCE).
=============================================================================
"""

import os
import struct
from typing import Dict, List, Any, Optional

class PointerTableMiner:
    """Mines pointer tables and extracts referenced text strings."""

    @staticmethod
    def extract_ascii_strings(data: bytes, min_len: int = 4, max_strings: int = 64) -> List[Dict[str, Any]]:
        strings = []
        cur = []
        start_off = 0
        for i, b in enumerate(data):
            if 0x20 <= b <= 0x7E:
                if not cur:
                    start_off = i
                cur.append(chr(b))
            else:
                if len(cur) >= min_len:
                    s = "".join(cur)
                    strings.append({
                        "offset": f"0x{start_off:06X}",
                        "raw_offset": start_off,
                        "length": len(s),
                        "text": s[:60] + ("..." if len(s) > 60 else "")
                    })
                    if len(strings) >= max_strings:
                        break
                cur = []
        return strings

    @staticmethod
    def mine_pointers_16le(data: bytes, base_offset: int = 0, min_entries: int = 4) -> List[Dict[str, Any]]:
        """Mines 16-bit Little-Endian pointer tables (common in Z80, 6502)."""
        tables = []
        length = len(data)
        if length < 8:
            return tables

        i = 0
        while i < length - (min_entries * 2):
            consecutive = []
            cur = i
            while cur + 2 <= length:
                ptr = struct.unpack("<H", data[cur:cur+2])[0]
                # A valid pointer points within reasonable binary range
                if 0x100 <= ptr < length or (base_offset > 0 and base_offset <= ptr < base_offset + length):
                    consecutive.append(ptr)
                    cur += 2
                else:
                    break

            if len(consecutive) >= min_entries:
                tables.append({
                    "table_offset": f"0x{i:06X}",
                    "width": "16-bit LE",
                    "count": len(consecutive),
                    "first_pointers": [f"0x{p:04X}" for p in consecutive[:8]]
                })
                i = cur
            else:
                i += 2
        return tables[:10]

    @staticmethod
    def mine_pointers_32le(data: bytes, min_entries: int = 4) -> List[Dict[str, Any]]:
        """Mines 32-bit Little-Endian pointer tables (common in MIPS PS1)."""
        tables = []
        length = len(data)
        if length < 16:
            return tables

        i = 0
        while i < length - (min_entries * 4):
            consecutive = []
            cur = i
            while cur + 4 <= length:
                ptr = struct.unpack("<I", data[cur:cur+4])[0]
                # PS1 RAM addresses typically start with 0x80xxxxxx
                if (0x80000000 <= ptr <= 0x80200000) or (0x00000100 <= ptr < length):
                    consecutive.append(ptr)
                    cur += 4
                else:
                    break

            if len(consecutive) >= min_entries:
                tables.append({
                    "table_offset": f"0x{i:06X}",
                    "width": "32-bit LE (MIPS/PS1)",
                    "count": len(consecutive),
                    "first_pointers": [f"0x{p:08X}" for p in consecutive[:8]]
                })
                i = cur
            else:
                i += 4
        return tables[:10]

    @classmethod
    def scan_binary(cls, file_path: str, max_bytes: int = 65536) -> Dict[str, Any]:
        if not os.path.exists(file_path):
            return {"status": "error", "message": f"File not found: {file_path}"}
        try:
            with open(file_path, "rb") as f:
                data = f.read(max_bytes)

            strings = cls.extract_ascii_strings(data, min_len=4, max_strings=32)
            ptrs_16 = cls.mine_pointers_16le(data, min_entries=4)
            ptrs_32 = cls.mine_pointers_32le(data, min_entries=4)

            return {
                "status": "ok",
                "file_path": file_path,
                "bytes_inspected": len(data),
                "strings_found": len(strings),
                "pointer_tables_16le": ptrs_16,
                "pointer_tables_32le": ptrs_32,
                "strings_sample": strings[:16]
            }
        except Exception as e:
            return {"status": "error", "message": str(e)}

if __name__ == "__main__":
    sample = struct.pack("<HHHH", 0x200, 0x220, 0x240, 0x280) + b"TEST" * 50
    print(PointerTableMiner.mine_pointers_16le(sample))
