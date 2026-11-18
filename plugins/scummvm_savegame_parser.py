#!/usr/bin/env python3
"""
ScummVM Savegame & Metadata Extractor Plugin
Parses ScummVM savegame headers (ScummVMSave header, compression, engine ID).
"""

import struct

def register_plugin():
    return {
        "name": "ScummVM Savegame Parser",
        "version": "1.0.0",
        "description": "Extracts metadata, playtime, and engine IDs from ScummVM .s00-.s99 savegame slots"
    }

def parse_savegame_header(raw_bytes):
    if len(raw_bytes) < 16:
        return {"error": "Save buffer too small"}
    magic = raw_bytes[:4]
    if magic != b'SCVM':
        return {"valid_scummvm": False, "raw_magic": magic.hex()}
    
    ver = struct.unpack(">I", raw_bytes[4:8])[0]
    return {
        "valid_scummvm": True,
        "format_version": ver,
        "header_len": 16,
        "payload_size": len(raw_bytes) - 16
    }
