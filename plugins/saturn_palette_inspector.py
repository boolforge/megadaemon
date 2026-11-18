#!/usr/bin/env python3
"""
=============================================================================
MEGADAEMON PLUGIN: SEGA SATURN CRAM PALETTE INSPECTOR
=============================================================================
Decodes Sega Saturn VDP2 Color RAM (CRAM) 15-bit RGB entries:
Format: 0b0BBBBBGGGGGRRRRR (16-bit word, Big-Endian or Little-Endian)
Transforms 5-bit color components to 8-bit truecolor (#RRGGBB).
=============================================================================
"""

import struct
from typing import Dict, Any, List

def register_plugin():
    return {
        "name": "saturn_palette_inspector",
        "version": "1.0.0",
        "description": "Decodes Sega Saturn 15-bit VDP2 CRAM color palettes into 24-bit RGB values",
        "author": "MegaDaemon Team",
        "hooks": ["inspect_palette"]
    }

def inspect_palette(data_bytes: bytes, endian: str = "be") -> Dict[str, Any]:
    """
    Decodes raw bytes containing 16-bit Saturn color words.
    """
    fmt = ">H" if endian == "be" else "<H"
    colors: List[Dict[str, Any]] = []

    words_count = len(data_bytes) // 2
    for i in range(words_count):
        word = struct.unpack_from(fmt, data_bytes, i * 2)[0]
        # Saturn 15-bit RGB:
        # bit 15: MSB (color calc / transparent)
        # bits 14-10: Blue (5 bits)
        # bits 9-5: Green (5 bits)
        # bits 4-0: Red (5 bits)
        r5 = word & 0x1F
        g5 = (word >> 5) & 0x1F
        b5 = (word >> 10) & 0x1F
        msb = (word >> 15) & 0x01

        # Scale 0-31 to 0-255
        r8 = (r5 * 255) // 31
        g8 = (g5 * 255) // 31
        b8 = (b5 * 255) // 31

        hex_color = f"#{r8:02X}{g8:02X}{b8:02X}"
        colors.append({
            "index": i,
            "raw_word": f"0x{word:04X}",
            "r5": r5,
            "g5": g5,
            "b5": b5,
            "rgb": [r8, g8, b8],
            "hex": hex_color,
            "msb_flag": msb
        })

    return {
        "status": "ok",
        "endian": endian,
        "palette_entries": len(colors),
        "colors": colors
    }

if __name__ == "__main__":
    # Test with standard Saturn colors: Red, Green, Blue, White
    # Red: 0x001F, Green: 0x03E0, Blue: 0x7C00, White: 0x7FFF
    test_raw = struct.pack(">4H", 0x001F, 0x03E0, 0x7C00, 0x7FFF)
    res = inspect_palette(test_raw, "be")
    print(res)
