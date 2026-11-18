#!/usr/bin/env python3
"""
=============================================================================
SEERE PHOENIX // AUTONOMOUS AGENTIC LLM ROUTER (v6.5 APEX)
=============================================================================
Autonomous, zero-cost cognitive intelligence layer for real-time systems
reverse engineering, memory inspection, and root-level orchestration.

Core Architecture:
  1. Tier 0 [LOCAL PRIMARY]: Qwen 2.5 Coder 0.5B on 127.0.0.1:8080 (100% Offline).
  2. Native Root Tool Execution: Native capability to invoke `rish -c` with uid=0.
  3. Zero-Copy SHM Reader: Direct memory introspection of snatcher_vla_shm.bin.
  4. Multi-Turn ReAct Loop: Thought -> Tool Action -> Observation -> Final Analysis.
=============================================================================
"""

import os
import sys
import time
import json
import urllib.request
import urllib.parse
import subprocess

LOCAL_LLM_URL = "http://127.0.0.1:8080/v1/chat/completions"
SHM_PATH = "/data/local/tmp/snatcher_vla_shm.bin"

AGENT_SYSTEM_PROMPT = """Eres SEERE PHOENIX, un agente avanzado de ingeniería inversa y análisis de sistemas ejecutándose directamente en Android 14 ARM64 (Exynos 2100).
Tienes acceso a herramientas nativas con permisos elevados de root (uid=0) a través de Shizuku.
Tus respuestas deben ser técnicas, precisas, analíticas, concisas y en español.

HERRAMIENTAS DISPONIBLES:
1. Para ejecutar comandos del sistema o diagnóstico como root:
   ROOT_EXEC: <comando bash>
   Ejemplo: ROOT_EXEC: id

2. Para inspeccionar bytes de memoria compartida SHM (1024 KB):
   READ_SHM: <offset_hex> <bytes_dec>
   Ejemplo: READ_SHM: 0x0000 64

Si necesitas información real del sistema, EMITE EXCLUSIVAMENTE LA LÍNEA DE LA HERRAMIENTA. Si no la requieres, responde directamente con tu análisis técnico conciso."""


class ParasiteLLMRouter:
    def __init__(self):
        self.privacy_mode = True  # Default strictly local offline
        self.total_queries = 0
        self.tier_stats = {
            "tier0_local": 0,
            "tier1_agent_root": 0,
            "tier2_error": 0
        }

    def execute_root_tool(self, cmd):
        """Execute a bash command via Shizuku rish with root privileges."""
        try:
            res = subprocess.run(
                ["rish", "-c", cmd],
                capture_output=True,
                text=True,
                timeout=12
            )
            out = res.stdout.strip()
            err = res.stderr.strip()
            # Clean up Shizuku welcome message
            if "Entering shell..." in err:
                err = err.replace("Entering shell...", "").strip()
            result = out if out else err
            return result if result else "(comando ejecutado sin salida)"
        except subprocess.TimeoutExpired:
            return "ERROR: Tiempo de espera agotado (12s) en comando root."
        except Exception as e:
            return f"ERROR ejecutando comando root: {e}"

    def read_shm_memory(self, offset_str, size_str):
        """Read arbitrary bytes from the live VLA SHM buffer."""
        try:
            offset = int(offset_str, 16) if offset_str.startswith("0x") else int(offset_str)
            size = min(int(size_str), 1024)
            if not os.path.exists(SHM_PATH):
                return f"ERROR: Búfer SHM no encontrado en {SHM_PATH}"
            with open(SHM_PATH, "rb") as f:
                f.seek(offset)
                data = f.read(size)
            hex_dump = " ".join(f"{b:02X}" for b in data)
            ascii_dump = "".join(chr(b) if 32 <= b <= 126 else "." for b in data)
            return f"SHM[0x{offset:06X}..+{size}B]:\nHEX:   {hex_dump}\nASCII: {ascii_dump}"
        except Exception as e:
            return f"ERROR leyendo SHM: {e}"

    def _call_llama(self, messages, max_tokens=96, temperature=0.2):
        """Query llama-server endpoint on port 8080."""
        payload = {
            "model": "qwen2.5-coder",
            "messages": messages,
            "max_tokens": max_tokens,
            "temperature": temperature
        }
        req = urllib.request.Request(
            LOCAL_LLM_URL,
            data=json.dumps(payload).encode("utf-8"),
            headers={"Content-Type": "application/json"}
        )
        with urllib.request.urlopen(req, timeout=30) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            return data["choices"][0]["message"]["content"].strip()

    def query_agent(self, prompt, allow_tools=True, max_turns=3):
        """
        Autonomous ReAct Agent Loop:
        Executes real tools on demand and feeds observations back to the LLM.
        """
        self.total_queries += 1
        start_time = time.time()
        tool_audit = []

        messages = [
            {"role": "system", "content": AGENT_SYSTEM_PROMPT},
            {"role": "user", "content": "Verifica si eres root con id"},
            {"role": "assistant", "content": "ROOT_EXEC: id"},
            {"role": "user", "content": "OBSERVACIÓN DEL SISTEMA (root):\nuid=0(root) gid=0(root) groups=0(root)\n\nCon esta información, continúa el análisis o concluye."},
            {"role": "assistant", "content": "Acceso root confirmado en el entorno Android. UID y GID establecidos en 0 (root)."},
            {"role": "user", "content": prompt}
        ]

        final_response = ""
        try:
            for turn in range(max_turns):
                model_reply = self._call_llama(messages, max_tokens=96)

                # Check for ROOT_EXEC tool trigger
                if allow_tools and "ROOT_EXEC:" in model_reply:
                    lines = model_reply.split("\n")
                    exec_cmd = None
                    for line in lines:
                        if "ROOT_EXEC:" in line:
                            exec_cmd = line.split("ROOT_EXEC:")[1].strip()
                            break
                    if exec_cmd:
                        tool_out = self.execute_root_tool(exec_cmd)
                        tool_audit.append({"tool": "ROOT_EXEC", "cmd": exec_cmd, "output": tool_out[:300]})
                        messages.append({"role": "assistant", "content": f"ROOT_EXEC: {exec_cmd}"})
                        messages.append({"role": "user", "content": f"OBSERVACIÓN DEL SISTEMA (root):\n{tool_out}\n\nCon esta información, continúa el análisis o concluye."})
                        self.tier_stats["tier1_agent_root"] += 1
                        continue

                # Check for READ_SHM tool trigger
                if allow_tools and "READ_SHM:" in model_reply:
                    parts = model_reply.split("READ_SHM:")[1].strip().split()
                    if len(parts) >= 2:
                        tool_out = self.read_shm_memory(parts[0], parts[1])
                        tool_audit.append({"tool": "READ_SHM", "offset": parts[0], "size": parts[1], "output": tool_out[:300]})
                        messages.append({"role": "assistant", "content": f"READ_SHM: {parts[0]} {parts[1]}"})
                        messages.append({"role": "user", "content": f"OBSERVACIÓN DE MEMORIA (SHM):\n{tool_out}\n\nCon esta información, continúa el análisis o concluye."})
                        self.tier_stats["tier1_agent_root"] += 1
                        continue

                # Normal text completion
                final_response = model_reply
                break

            if not final_response:
                final_response = model_reply

            latency = int((time.time() - start_time) * 1000)
            self.tier_stats["tier0_local"] += 1

            return {
                "status": "OK",
                "provider": "Qwen 2.5 Coder 0.5B (Local ReAct Agent)",
                "response": final_response,
                "tool_executions": tool_audit,
                "latency_ms": latency,
                "privacy": "100% OFFLINE (Root Shizuku Active)"
            }

        except urllib.error.URLError as e:
            self.tier_stats["tier2_error"] += 1
            return {
                "status": "ERROR",
                "provider": "Local Host 127.0.0.1:8080",
                "response": f"Fallo de conexión con llama-server (127.0.0.1:8080): {e}. Asegúrese de que el daemon esté activo.",
                "tool_executions": [],
                "latency_ms": int((time.time() - start_time) * 1000),
                "privacy": "OFFLINE"
            }
        except Exception as e:
            self.tier_stats["tier2_error"] += 1
            return {
                "status": "ERROR",
                "provider": "Local Agent Engine",
                "response": f"Error interno en ejecución del agente: {e}",
                "tool_executions": [],
                "latency_ms": int((time.time() - start_time) * 1000),
                "privacy": "OFFLINE"
            }

    def query(self, prompt, system_prompt=None, max_tokens=128, temperature=0.3, force_provider=None):
        """Standard query interface for backwards compatibility with MegaDaemon."""
        return self.query_agent(prompt, allow_tools=True)


_GLOBAL_ROUTER = None

def get_router():
    global _GLOBAL_ROUTER
    if _GLOBAL_ROUTER is None:
        _GLOBAL_ROUTER = ParasiteLLMRouter()
    return _GLOBAL_ROUTER


def query_parasite_router(prompt, preferred_tier=0, privacy_mode=True):
    """Bridge for megadaemon_cockpit and legacy endpoints."""
    router = get_router()
    res = router.query_agent(prompt)
    if res.get("status") == "OK":
        audit = ""
        if res.get("tool_executions"):
            tools_str = ", ".join(f"{t['tool']} ({t.get('cmd', t.get('offset', ''))})" for t in res["tool_executions"])
            audit = f"[Root Tools: {tools_str}]\n"
        return f"{audit}{res['response']}"
    return res.get("response", "Error procesando consulta con el agente local")


if __name__ == "__main__":
    router = get_router()
    test_query = sys.argv[1] if len(sys.argv) > 1 else "Comprueba si tienes permisos root con el comando id y muestra el paquete de Seere."
    print(f"=== TESTING SEERE PHOENIX AGENT: '{test_query}' ===")
    res = router.query_agent(test_query)
    print(json.dumps(res, indent=2, ensure_ascii=False))
