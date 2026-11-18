#!/usr/bin/env python3
"""
=============================================================================
MEGADAEMON PLUGIN: RETRO 1-BIT TILE & FONT EXTRACTOR (MSX2 / PC-88 / GB)
=============================================================================
Decodes 1-bit per pixel (1bpp) 8x8 character font tiles from raw ROMs or VRAM.
Each character occupies 8 bytes (1 byte per 8-pixel horizontal scanline).
=============================================================================
"""

from typing import Dict, Any, List

def register_plugin():
    return {
        "name": "msx2_font_extractor",
        "version": "1.0.0",
        "description": "Extracts 8x8 1bpp monochrome font glyphs from MSX2, PC-8801, and Game Boy ROMs/VRAM",
        "author": "MegaDaemon Team",
        "hooks": ["extract_glyphs"]
    }

def extract_glyphs(data_bytes: bytes, max_chars: int = 32) -> Dict[str, Any]:
    """
    Decodes 8x8 glyphs where each glyph is 8 consecutive bytes.
    """
    total_chars = min(len(data_bytes) // 8, max_chars)
    glyphs: List[Dict[str, Any]] = []

    for char_idx in range(total_chars):
        offset = char_idx * 8
        tile_bytes = data_bytes[offset:offset+8]
        lines_ascii = []
        matrix = []

        for row_byte in tile_bytes:
            row_bits = []
            row_str = ""
            for bit_pos in range(7, -1, -1):
                is_set = (row_byte >> bit_pos) & 1
                row_bits.append(is_set)
                row_str += "█" if is_set else " "
            matrix.append(row_bits)
            lines_ascii.append(row_str)

        glyphs.append({
            "char_index": char_idx,
            "offset": f"0x{offset:06X}",
            "raw_hex": tile_bytes.hex(),
            "ascii_render": lines_ascii,
            "matrix": matrix
        })

    return {
        "status": "ok",
        "glyphs_extracted": len(glyphs),
        "glyphs": glyphs
    }

if __name__ == "__main__":
    # Letter 'A' 8x8 bitmap:
    # 0x18 (..XX....), 0x24 (.X..X...), 0x42 (X....X..), 0x7E (.XXXXXX.), 0x42, 0x42, 0x42, 0x00
    letter_a = bytes([0x18, 0x24, 0x42, 0x7E, 0x42, 0x42, 0x42, 0x00])
    res = extract_glyphs(letter_a)
    print(res["glyphs"][0]["ascii_render"])
