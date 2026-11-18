#!/usr/bin/env python3
"""
=============================================================================
MEGADAEMON SHANNON ENTROPY & STRUCTURE CLASSIFIER (v5.0)
=============================================================================
High-performance Shannon entropy scanner and binary classifier.
Identifies code, data tables, uncompressed graphics, and compressed/encrypted
blobs across retro platform ROMs, CD sectors, and executable slices.
=============================================================================
"""

import os
import math
from typing import Dict, List, Any

def calculate_shannon_entropy(data: bytes) -> float:
    """Calculates Shannon entropy in bits per byte (range: 0.0 to 8.0)."""
    if not data:
        return 0.0
    freq = [0] * 256
    for b in data:
        freq[b] += 1
    length = len(data)
    entropy = 0.0
    for count in freq:
        if count > 0:
            p = count / length
            entropy -= p * math.log2(p)
    return round(entropy, 4)

def classify_entropy(entropy: float) -> str:
    """Classifies binary content based on entropy value."""
    if entropy < 1.0:
        return "PADDING_NULL"
    elif entropy < 3.5:
        return "STRUCT_POINTER_TABLE"
    elif entropy <= 5.2:
        return "CODE_OR_TEXT"
    elif entropy <= 7.0:
        return "PLANAR_GFX_OR_PCM"
    else:
        return "COMPRESSED_OR_ENCRYPTED"

class EntropyScanner:
    """Scans binary files or buffers and produces structured entropy maps."""

    @staticmethod
    def scan_buffer(data: bytes, block_size: int = 1024, max_blocks: int = 128) -> Dict[str, Any]:
        total_len = len(data)
        if total_len == 0:
            return {"error": "Empty buffer", "blocks": []}

        actual_block_size = max(64, block_size)
        step = max(actual_block_size, total_len // max_blocks)
        
        blocks = []
        overall_entropy = calculate_shannon_entropy(data[:min(total_len, 65536)])
        
        offset = 0
        while offset < total_len and len(blocks) < max_blocks:
            chunk = data[offset : min(offset + actual_block_size, total_len)]
            ent = calculate_shannon_entropy(chunk)
            classification = classify_entropy(ent)
            blocks.append({
                "offset": f"0x{offset:06X}",
                "size": len(chunk),
                "entropy": ent,
                "category": classification
            })
            offset += step

        # Summary statistics
        categories = {}
        for b in blocks:
            cat = b["category"]
            categories[cat] = categories.get(cat, 0) + 1

        return {
            "status": "ok",
            "total_bytes": total_len,
            "overall_entropy": overall_entropy,
            "overall_category": classify_entropy(overall_entropy),
            "blocks_scanned": len(blocks),
            "categories_summary": categories,
            "blocks": blocks
        }

    @classmethod
    def scan_file(cls, file_path: str, block_size: int = 1024, max_blocks: int = 64) -> Dict[str, Any]:
        if not os.path.exists(file_path):
            return {"status": "error", "message": f"File not found: {file_path}"}
        try:
            with open(file_path, "rb") as f:
                data = f.read(512 * 1024) # Read up to 512KB for instant scan
            res = cls.scan_buffer(data, block_size, max_blocks)
            res["file_path"] = file_path
            return res
        except Exception as e:
            return {"status": "error", "message": str(e)}

if __name__ == "__main__":
    test_data = b"\x00" * 500 + b"Hello World This is ASCII Text" * 20 + bytes(range(256)) * 4
    print(EntropyScanner.scan_buffer(test_data, block_size=256))
