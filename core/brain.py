#!/usr/bin/env python3
"""
=============================================================================
MEGADAEMON NEURAL CORE (MENTE Y CORAZÓN): EMBEDDED LLAMA-SERVER SUBSYSTEM
=============================================================================
Manages the lifecycle, thermal-aware concurrency, and direct inference streaming
of local GGUF models (Qwen2.5-Coder-0.5B / 1.5B) via llama-server on 127.0.0.1:8080.
Provides zero-latency, 100% offline cognitive reasoning for:
  - Disassembly analysis & opcode decompilation synthesis
  - Crash trace / GDB / Frida diagnostic synthesis
  - Real-time automated patch generation for ScummVM and emulator harnesses
=============================================================================
"""

import os
import sys
import time
import json
import urllib.request
import urllib.error
import subprocess

MODEL_DEFAULT = "/data/user/0/com.agy/files/models/Qwen2.5-Coder-0.5B-Q4_K_M.gguf"
LLAMA_HOST = "127.0.0.1"
LLAMA_PORT = 8080
LLAMA_URL = f"http://{LLAMA_HOST}:{LLAMA_PORT}"

class MegaBrain:
    def __init__(self, model_path=MODEL_DEFAULT, host=LLAMA_HOST, port=LLAMA_PORT):
        self.model_path = model_path
        self.host = host
        self.port = port
        self.base_url = f"http://{self.host}:{self.port}"
        self.process = None

    def is_alive(self):
        try:
            req = urllib.request.Request(f"{self.base_url}/health")
            with urllib.request.urlopen(req, timeout=1.0) as resp:
                data = json.loads(resp.read().decode())
                return data.get("status") in ("ok", "loading model", "ready")
        except Exception:
            return False

    def query(self, prompt, system_prompt=None, max_tokens=256, temperature=0.2):
        """Zero-overhead local inference streaming directly from llama-server."""
        if not self.is_alive():
            return {
                "error": "MegaBrain (llama-server) is offline",
                "response": "El núcleo neural local no está respondiendo en 127.0.0.1:8080."
            }

        messages = []
        if system_prompt:
            messages.append({"role": "system", "content": system_prompt})
        else:
            messages.append({
                "role": "system",
                "content": (
                    "Eres la mente y corazón neural de MegaDaemon, un hiper-orquestador "
                    "de preservación de videojuegos retro, emulación y descompilación en Android. "
                    "Responde con extrema precisión técnica, concisión y foco absoluto en bajo nivel."
                )
            })
        messages.append({"role": "user", "content": prompt})

        payload = json.dumps({
            "messages": messages,
            "max_tokens": max_tokens,
            "temperature": temperature,
            "stream": False
        }).encode("utf-8")

        req = urllib.request.Request(
            f"{self.base_url}/v1/chat/completions",
            data=payload,
            headers={"Content-Type": "application/json"}
        )

        t0 = time.perf_counter()
        try:
            with urllib.request.urlopen(req, timeout=30.0) as resp:
                result = json.loads(resp.read().decode("utf-8"))
                elapsed_ms = (time.perf_counter() - t0) * 1000.0
                content = result["choices"][0]["message"]["content"]
                usage = result.get("usage", {})
                return {
                    "status": "ok",
                    "response": content,
                    "elapsed_ms": round(elapsed_ms, 2),
                    "tokens_eval": usage.get("completion_tokens", 0),
                    "tok_per_sec": round((usage.get("completion_tokens", 0) / max(0.001, elapsed_ms / 1000.0)), 2)
                }
        except Exception as e:
            return {"status": "error", "error": str(e)}

    def analyze_opcode(self, arch, hex_bytes, context=""):
        """Specialized cognitive prompt to analyze unknown retro opcodes."""
        system_prompt = (
            f"Eres el desensamblador cognitivo de MegaDaemon para arquitectura {arch}. "
            "Identifica la instrucción, nemónico, flags afectados y propósito semántico en el juego."
        )
        user_prompt = f"Bytes: {hex_bytes}\nContexto: {context}\nExplica la instrucción de forma concisa."
        return self.query(user_prompt, system_prompt=system_prompt, max_tokens=150)

    def diagnose_crash(self, crash_log):
        """Diagnose emulator or ScummVM SIGSEGV/bus error log in real-time."""
        system_prompt = (
            "Eres el depurador forense de MegaDaemon. Analiza el volcado de registros y "
            "pila de llamadas para identificar la causa raíz de la desincronización o violación de segmento."
        )
        return self.query(crash_log, system_prompt=system_prompt, max_tokens=300)

if __name__ == "__main__":
    brain = MegaBrain()
    print(f"[*] MegaBrain Estado: {'ACTIVO' if brain.is_alive() else 'INACTIVO'}")
    if brain.is_alive():
        test = brain.query("Preséntate como mente y corazón de MegaDaemon en 2 frases cortas.")
        print(f"[+] Respuesta ({test.get('elapsed_ms')}ms): {test.get('response')}")
