//! MegaDaemon v13.4 — Catálogo MCP extendido.
//!
//! Todas las capacidades nuevas (framework, APK, repos, perfil, mensajería,
//! marcadores, batería, git, identidad, SSH) se publican como herramientas
//! MCP con el mismo contrato que el resto: JSON Schema tipado, errores
//! estructurados y `structuredContent` para clientes que lo saben usar.
//!
//! Regla de seguridad aplicada en todo el fichero: **ninguna acción que
//! modifique el dispositivo, publique en un remoto o mate procesos se ejecuta
//! sin `confirm: true`**.

use serde_json::{json, Value};

/// Definiciones de las herramientas nuevas.
pub fn tool_definitions() -> Value {
    // Las herramientas del bloque "backups Titanium / catalogo / plan de
    // migracion" y del "contenedor Swift Backup" viven en su propio modulo
    // (otro dueho de fichero): aqui solo se **extiende** el array (un hunk)
    // para que la CLI, el MCP y el agente hablen del mismo codigo.
    let base = json!([
        {
            "name": "drive_backup",
            "description": "Google Drive desde el propio MegaDaemon: sube un fichero o un directorio ENTERO (se comprime en ZIP) con subida resumible en trozos de 256 KiB, SHA-256 verificado en local y credenciales OAuth leidas del fichero 0600. Sin curl, sin dd, sin scripts: todo en Rust. Acciones: upload | download | status. Subir publica el contenido en un remoto: exige confirm=true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["upload", "sync", "download", "status"], "default": "upload" },
                    "path": { "type": "string", "description": "Ruta local a subir (fichero o directorio)" },
                    "name": { "type": "string", "description": "Nombre en Drive (opcional; por defecto, el del fichero)" },
                    "folder_id": { "type": "string", "description": "Carpeta destino en Drive (vacio = Mi Drive)" },
                    "workers": { "type": "integer", "default": 8, "description": "Hilos paralelos para sync (por defecto 8)" },
                    "verify_delete": { "type": "boolean", "default": false, "description": "Borrar local tras verificar tamano remoto (sync)" },
                    "file_id": { "type": "string", "description": "Id de Drive a descargar (action=download)" },
                    "dest": { "type": "string", "description": "Ruta destino de la descarga (action=download)" },
                    "confirm": { "type": "boolean", "default": false }
                },
                "required": ["confirm"]
            }
        },
        {
            "name": "channel_publish",
            "description": "Publica el estado del canal del movil (URL del quick tunnel y la del canal estable DuckDNS) FUERA del movil: escribe state/channel.json y lo publica en un paste publico, para que el sandbox redescubra la URL aunque el tunel este caido. Publicar expone una URL del dispositivo en un sitio publico: por eso exige confirm=true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "quick": { "type": "string", "description": "URL del quick tunnel vigente (opcional)" },
                    "duckdns": { "type": "string", "description": "URL estable de DuckDNS (opcional)" },
                    "ip_publica": { "type": "string", "description": "IP publica que ve el host (opcional)" },
                    "cloudflared_pids": { "type": "string", "description": "pids de cloudflared (opcional)" },
                    "confirm": { "type": "boolean", "default": false }
                },
                "required": ["confirm"]
            }
        },
        {
            "name": "daad_harvester",
            "description": "Evidence-led DAAD database recognizer, full DDB IR extractor, interpreter fingerprinter, load-model verifier (10 retro targets), recursive media unpacker (ZIP, GZ, +3DOS, P00, TAP, TZX, D64, CPC DSK, ADF, DMS, FAT12), CondAct disassembler, and SQLite preservation catalog interface.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Ruta al fichero, imagen de disco, o base de datos state.db" },
                    "action": { "type": "string", "enum": ["scan", "embedded", "media", "disasm", "catalog", "ir", "fingerprint", "load_model", "unpack"], "default": "scan" },
                    "platform": { "type": "string", "description": "Filtro de plataforma para catalog, dialecto (v1/v2) para disasm, o target para load_model (amiga_hunk, atari_st_prg, c64_prg, c64_entry_state, cpc_amsdos, dos_mz, plus4_prg, zx_plus3dos, msx_z80, pcw_z80)" },
                    "query": { "type": "string", "description": "Filtro de busqueda para catalog (titulo o id)" }
                }
            }
        },
        {
            "name": "coding_agent",
            "description": "Autonomous multi-turn ReAct software engineering agent in pure Rust. Reads, edits, searches, and runs commands inside a sandboxed workspace using MegaDaemon's live provider vault with automatic SEERE fallback and JSONL session persistence.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "goal": { "type": "string", "description": "Objetivo de ingenieria de software a resolver en la sesion" },
                    "workspace_root": { "type": "string", "description": "Directorio raiz del workspace (opcional; por defecto '.')" },
                    "model": { "type": "string", "description": "Modelo LLM preferido (opcional)" },
                    "session_id": { "type": "string", "description": "ID de sesion JSONL para reanudar o etiquetar (opcional)" },
                    "max_turns": { "type": "integer", "default": 15, "description": "Numero maximo de turnos ReAct (por defecto 15)" },
                    "allow_outside_workspace": { "type": "boolean", "default": false, "description": "Permitir acceso fuera de workspace_root (por defecto false)" }
                },
                "required": ["goal"]
            }
        },
        {
            "name": "go_retool",
            "description": "Pure-Rust Go binary reverse engineering toolset (ELF/PE/Mach-O container scan, Go 1.18+ pclntab function & package recovery, Ghidra JSON export, and stackmap inspection).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Ruta al binario Go (ELF, PE o Mach-O)" },
                    "action": { "type": "string", "enum": ["analyze", "container", "functions", "ghidra_json"], "default": "analyze" },
                    "query": { "type": "string", "description": "Filtro opcional de nombre de funcion (accion functions)" },
                    "limit": { "type": "integer", "default": 200, "description": "Maximo de funciones a devolver" }
                },
                "required": ["path"]
            }
        },
        {
            "name": "es_eid",
            "description": "Spanish eID engine (FNMT Certificado Electrónico .p12/.pfx via p12-keystore + x509-cert + rustls mTLS, and DNI Electrónico 3.0/4.0 via rdnie ATR/APDU).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["inspect_p12", "dnie_atr", "dnie_apdu"], "default": "inspect_p12" },
                    "path": { "type": "string", "description": "Path to .p12/.pfx file (inspect_p12)" },
                    "password": { "type": "string", "description": "PKCS#12 password (inspect_p12)" },
                    "atr_hex": { "type": "string", "description": "Hex ATR bytes (dnie_atr)" },
                    "pin": { "type": "string", "description": "DNIe PIN (dnie_apdu)" },
                    "digest_hex": { "type": "string", "description": "32-byte SHA-256 hex digest (dnie_apdu)" },
                    "key_ref": { "type": "integer", "default": 1 }
                },
                "required": ["action"]
            }
        },
        {
            "name": "ast_analyze",
            "description": "Tree-sitter Rust AST symbol extraction and syntax-error verification.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to .rs file" }
                },
                "required": ["path"]
            }
        },
        {
            "name": "re_disasm",
            "description": "Multi-ISA disassembler & provenance inspector (iced-x86, Capstone ARM64/ARM/x64, goblin + gimli DWARF).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["disasm", "provenance"], "default": "disasm" },
                    "arch": { "type": "string", "enum": ["arm64", "arm", "x86_64", "x86_32"], "default": "arm64" },
                    "hex": { "type": "string", "description": "Hex machine code bytes (disasm)" },
                    "base_addr": { "type": "string", "description": "Hex base address e.g. 0x400000" },
                    "max_insns": { "type": "integer", "default": 64 },
                    "path": { "type": "string", "description": "Binary path (provenance)" }
                }
            }
        },
        {
            "name": "job_queue",
            "description": "Durable SQLite job queue with leases, idempotency keys, retries, and crash recovery.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["enqueue", "claim", "complete", "fail", "cancel", "list"], "default": "list" },
                    "kind": { "type": "string" },
                    "payload": { "type": "object" },
                    "idempotency_key": { "type": "string" },
                    "id": { "type": "integer" },
                    "error": { "type": "string" },
                    "limit": { "type": "integer", "default": 50 }
                }
            }
        },
        {
            "name": "rhai_eval",
            "description": "Resource-bounded sandboxed Rhai scripting engine.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "script": { "type": "string" },
                    "max_ops": { "type": "integer", "default": 50000 }
                },
                "required": ["script"]
            }
        },
        {
            "name": "term_snapshot",
            "description": "Parse raw ANSI/VT terminal output into a rendered vt100 screen snapshot.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "raw": { "type": "string" },
                    "rows": { "type": "integer", "default": 24 },
                    "cols": { "type": "integer", "default": 80 }
                },
                "required": ["raw"]
            }
        },
        {
            "name": "tool_audit",
            "description": "Query recent tool invocations and policy decisions from the SQLite audit journal.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "default": 30 }
                }
            }
        },
        {
            "name": "drive_vfs",
            "description": "Google Drive Virtual Filesystem (ls, stat, read, find, mkdir, write) with SQLite metadata cache for browsing MakeMKV-RE-v6-backup, playground-sdk-research/output, etc.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["ls", "stat", "read", "find", "mkdir", "write"], "default": "ls" },
                    "path": { "type": "string", "description": "VFS path e.g. drive:/MakeMKV-RE-v6-backup" },
                    "query": { "type": "string", "description": "Search substring (find)" },
                    "local_path": { "type": "string", "description": "Local source path (write)" },
                    "remote_name": { "type": "string", "description": "Target filename (write)" },
                    "max_bytes": { "type": "integer", "default": 65536 },
                    "limit": { "type": "integer", "default": 100 }
                }
            }
        },
        {
            "name": "retro_apk_tools",
            "description": "Pure-Rust Retro & Android binary tools (AXML AndroidManifest decoder, 1bpp 8x8 font extractor, Saturn VDP2 CRAM palette inspector, ScummVM savegame parser).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["axml", "font_1bpp", "saturn_palette", "scummvm_save"], "default": "axml" },
                    "path": { "type": "string", "description": "Path to binary file" },
                    "max_chars": { "type": "integer", "default": 64 },
                    "endian": { "type": "string", "enum": ["be", "le"], "default": "be" }
                },
                "required": ["path"]
            }
        },

        {
            "name": "clipboard",
            "description": "Portapapeles distribuido: historial con deduplicacion y caducidad, busqueda, clasificacion (url/telefono/codigo) y sincronizacion entre nodos. Lo que parece un secreto (clave, token, OTP) se marca como SENSIBLE y NO se persiste en el historial ni se propaga por la malla. Acciones: list, put, get, copy, clear, search, status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "put", "get", "copy", "clear", "search", "status"], "default": "list" },
                    "text": { "type": "string", "description": "texto a guardar (accion put)" },
                    "query": { "type": "string", "description": "texto a buscar (accion search)" },
                    "id": { "type": "string", "description": "id de un item concreto (acciones get/copy)" },
                    "origin": { "type": "string", "description": "origen del item (paquete de la app)" },
                    "limit": { "type": "number", "description": "maximo de resultados" }
                }
            }
        },

        {
            "name": "capabilities",
            "description": "Catálogo de capacidades de MegaDaemon con verificación REAL de dependencias en el dispositivo (binarios, rutas, módulos). Devuelve también la huella del catálogo para detectar deriva entre versiones.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["verify", "manifest", "summary"], "default": "verify" }
                }
            }
        },
        {
            "name": "env_introspect",
            "description": "Pregunta al ENTORNO REAL del dispositivo (particiones, /dev, binarios, apps, RAM, rutas, props, procesos) y responde con la verdad: source + ts + device. Si algo no existe devuelve available:false + reason. Nunca inventa.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "question": { "type": "string", "description": "pregunta en lenguaje natural (\"¿existe NPU.bin?\")" },
                    "action": { "type": "string", "enum": ["partitions", "dev", "binaries", "app", "ram", "path", "prop", "processes", "help"] },
                    "filter": { "type": "string", "description": "filtro para action=binaries (nombre del binario)" },
                    "package": { "type": "string", "description": "paquete para action=app" }
                },
                "required": ["question"]
            }
        },
        {
            "name": "alerts",
            "description": "Sistema de alertas: evalúa las reglas reales (batería, thermal, disco, cola, citas) o lista/acuza las alertas emitidas.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["check", "list", "ack", "status", "prune"] },
                    "id": { "type": "string" },
                    "severity": { "type": "string", "enum": ["info", "warn", "crit"] },
                    "limit": { "type": "integer", "default": 50 }
                }
            }
        },
        {
            "name": "agent",
            "description": "Núcleo agéntico: convierte un objetivo en un plan de tareas y lo ejecuta. Usa el LLM local si está disponible (0 tokens) y un planificador heurístico determinista si no.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["plan", "run", "status"] },
                    "goal": { "type": "string" },
                    "prefer_llm": { "type": "boolean", "default": true },
                    "max_tasks": { "type": "integer", "default": 5 }
                },
                "required": ["goal"]
            }
        },
        {
            "name": "apk_inventory",
            "description": "Inventario y análisis de APKs instaladas: manifiesto binario (AXML) con permisos y componentes, DEX, librerías nativas, SHA-256, huella de manifiesto y detección de cambios entre versiones.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["inventory", "analyze", "dump", "drift"], "default": "inventory" },
                    "path": { "type": "string" },
                    "limit": { "type": "integer", "default": 50 },
                    "dest_dir": { "type": "string" },
                    "known": { "type": "object", "description": "manifiesto previo para comparar" }
                }
            }
        },
        {
            "name": "dex_decompile",
            "description": "Decompilador DEX en Rust puro (sin Java, dex2jar ni Ghidra): pseudo-Java por clase, desensamblado Dalvik, mapa de desofuscación (nombres ofuscados, strings cifrados, flujo de control) y strings del DEX. Acepta un .dex suelto o un APK (extrae classes*.dex).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["decompile", "strings", "obfuscation"], "default": "decompile" },
                    "path": { "type": "string", "description": "ruta ABSOLUTA de un .dex o de un APK" }
                },
                "required": ["path"]
            }
        },
        {
            "name": "app_intel",
            "description": "Inteligencia de apps de IA (Claude, ChatGPT/Codex, Gemini, DeepSeek, Kimi, MiniMax, Grok, Manus, Cursor, Windsurf, Antigravity): rutas de datos locales, formatos de sesión, endpoints, plan de exportación/importación idempotente y notas de interoperabilidad sin API key. Los secretos se redactan siempre.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["profiles", "profile", "scan", "exchange", "notes"], "default": "profiles" },
                    "id": { "type": "string" },
                    "root": { "type": "string", "description": "directorio raíz de datos de la app" }
                }
            }
        },
        {
            "name": "root_hide",
            "description": "Auditoría de ocultación de root: matriz de señales de fuga por app (su, Magisk, Zygisk, Xposed, props, Play Integrity), plan de remediación con acciones reversibles y layout del módulo Magisk propio.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["audit", "plan", "magisk_layout"], "default": "audit" },
                    "packages": { "type": "array", "items": { "type": "string" } }
                }
            }
        },
        {
            "name": "store_manager",
            "description": "Gestor de tiendas/repos universal: registra tiendas (repo, modulo Magisk, APK, payload, herramienta), descarga y cachea su indice, lo verifica (sha256 por entrada y firma RSA opcional del indice), busca entradas y las instala con verificacion de hash. Sin SDK de terceros ni binarios externos. Acciones: list, add, remove, refresh, search, plan, install, stats.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "add", "remove", "refresh", "search", "plan", "install", "stats"], "default": "list" },
                    "id": { "type": "string", "description": "Identificador de la tienda" },
                    "name": { "type": "string" },
                    "kind": { "type": "string", "enum": ["repo", "magisk", "apk", "payload", "tool"], "default": "repo" },
                    "base_url": { "type": "string" },
                    "index_path": { "type": "string" },
                    "public_key": { "type": "string", "description": "Clave publica RSA en base64 con el formato n:e" },
                    "trust": { "type": "string", "enum": ["sha256", "signed"], "default": "sha256" },
                    "query": { "type": "string" },
                    "entry": { "type": "string", "description": "id de la entrada a instalar" },
                    "confirm": { "type": "boolean", "default": false }
                }
            }
        },
        {
            "name": "root_detectors",
            "description": "Apps que DETECTAN root, medido en el dispositivo: extrae la tabla de cadenas real del DEX de cada APK, busca firmas de deteccion (rutas de su, Magisk, props, Xposed/Frida, Play Integrity, emulador, Knox) y anade la evidencia dinamica vista en logcat. Cada hallazgo lleva la cadena exacta y el fichero de la que salio.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["scan", "summary"], "default": "scan" },
                    "limit": { "type": "integer", "default": 40 },
                    "scope": { "type": "string", "enum": ["third_party", "all"], "default": "third_party" }
                }
            }
        },
        {
            "name": "root_consumers",
            "description": "Apps que USAN root, medido en el dispositivo: politicas reales de Magisk (deny list y allow list leidas de la base de datos de la app Magisk o de `magisk --sqlite`), gestores de root instalados y eventos de `su` observados en logcat con la app que los pidio.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["scan", "summary"], "default": "scan" }
                }
            }
        },
        {
            "name": "repo_updates",
            "description": "Actualización automática de apps desde repositorios (F-Droid, Obtainium). Calcula el plan de actualización comparando versiones; descargar e instalar exige confirmación explícita.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["repos", "status", "plan", "watch", "download", "install"] },
                    "repo_id": { "type": "string" },
                    "package": { "type": "string" },
                    "dest_dir": { "type": "string" },
                    "apk_path": { "type": "string" },
                    "confirm": { "type": "boolean", "default": false }
                }
            }
        },
        {
            "name": "profile",
            "description": "Perfil del usuario a partir de TODO el historial sincronizado: intereses, fortalezas, debilidades, pendientes, temas en evolución, digest diario y un informe para profesionales.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["analyze", "briefing", "drift", "digest"] },
                    "markdown": { "type": "boolean", "default": true },
                    "window_days": { "type": "integer", "default": 30 },
                    "days": { "type": "integer", "default": 14 }
                }
            }
        },
        {
            "name": "messaging",
            "description": "Mensajería y citas: Telegram (base SQLite del propio dispositivo), SMS/MMS (content provider) y metadatos de WhatsApp. Los mensajes de WhatsApp están cifrados por el keystore y se reportan como no legibles, nunca inventados.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["inventory", "telegram", "sms", "whatsapp", "medical", "ingest", "candidates"] },
                    "limit": { "type": "integer", "default": 500 }
                }
            }
        },
        {
            "name": "bookmarks",
            "description": "Marcadores, sesiones guardadas y passkeys de todos los navegadores del dispositivo, con exportación a HTML de Netscape (importable por cualquier navegador o sistema).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["inventory", "export", "passkeys", "manager_export"] },
                    "dest_path": { "type": "string" },
                    "package": { "type": "string" }
                }
            }
        },
        {
            "name": "battery_saver",
            "description": "Ahorro de energía avanzado: integra la configuración real de Xtreme Battery Saver si está instalada y mata sólo procesos en segundo plano cacheables, respetando primer plano, notificaciones y la política de energía.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["status", "apply", "protect", "xbs"] },
                    "aggressive": { "type": "boolean", "default": false },
                    "confirm": { "type": "boolean", "default": false },
                    "protected": { "type": "array", "items": { "type": "string" } }
                }
            }
        },
        {
            "name": "git_ops",
            "description": "Operaciones git autónomas de megadaemon: estado del repo del dispositivo, diff, commit y push con la identidad del demonio. Push exige confirmación explícita y nunca usa --force.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["status", "identity", "diff", "commit_push", "log"] },
                    "message": { "type": "string" },
                    "paths": { "type": "array", "items": { "type": "string" } },
                    "push": { "type": "boolean", "default": false },
                    "confirm": { "type": "boolean", "default": false }
                }
            }
        },
        {
            "name": "identity_and_web",
            "description": "Perfiles de identidad por servicio (UA y cabeceras de la app real) y cliente HTTP discreto con scraping avanzado, extracción de enlaces, texto plano y respeto de robots.txt.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "scrape"] },
                    "url": { "type": "string" },
                    "identity": { "type": "string", "default": "browser.mobile" },
                    "respect_robots": { "type": "boolean", "default": true },
                    "max_bytes": { "type": "integer", "default": 524288 }
                }
            }
        },
        {
            "name": "root_stealth",
            "description": "Perfil de ocultación de root para que las apps que rechazan entornos rooteados (banca, AEAT) vuelvan a funcionar. No falsifica attestation de hardware; todo es reversible y auditable.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["status", "preview", "apply"] },
                    "scope": { "type": "string", "enum": ["all", "targeted"], "default": "all" },
                    "confirm": { "type": "boolean", "default": false }
                }
            }
        },
        {
            "name": "ssh_remote",
            "description": "Servidor SSH nativo: estado, preparación de host key y ejecución de la gramática de comandos de megadaemon por SSH.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["status", "install_host_key", "authorized_keys", "dispatch"] },
                    "command": { "type": "string" }
                }
            }
        },
        {
            "name": "meta_route",
            "description": "Meta-agente: convierte una petición en lenguaje natural en un plan ejecutable (tipo de tarea, servicio del bus con su salud REAL, tool MCP a la que llamar y tarea del tablón). No ejecuta salvo que se pida execute=true. Sin secretos.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "request": { "type": "string", "description": "la petición del usuario en texto libre" },
                    "execute": { "type": "boolean", "default": false, "description": "si es true, ejecuta el primer paso del plan y devuelve la respuesta real" }
                },
                "required": ["request"]
            }
        },
        {
            "name": "service_bus",
            "description": "Bus de servicios: catálogo unificado de MODELOS (los proveedores de providers.rs) y AGENTES/SERVICIOS (otros nodos MegaDaemon, clientes MCP como Antigravity, servidores MCP de terceros, herramientas locales). Selección automática por tipo de tarea (inference/build/analysis/writing) con fallback real y presupuesto por tick/día. La salud NUNCA se inventa: si no se puede comprobar, available=false con el motivo. Sin secretos: sólo auth_kind y la ruta del fichero de credenciales.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["catalog", "select", "probe", "bridge"], "default": "catalog" },
                    "task": { "type": "string", "enum": ["inference", "build", "analysis", "writing"] },
                    "id": { "type": "string", "description": "servicio concreto (para probe)" },
                    "path": { "type": "string", "description": "fichero de config MCP de Antigravity (para bridge)" },
                    "enabled_tools": { "type": "array", "items": { "type": "string" }, "description": "subconjunto de tools a exponer (bridge); vacío = todas" },
                    "confirm": { "type": "boolean", "default": false }
                }
            }
        },
        {
            "name": "github",
            "description": "Conector de GitHub: lista los repos del usuario, lee issues y PRs, y publica el estado del enjambre como issue o PR. Publicar exige confirm=true. El token se lee de un fichero local en el momento de enviar: nunca viaja en las peticiones, en el catálogo ni en el estado compartido.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["repos", "issues", "prs", "publish", "bridge"] },
                    "repo": { "type": "string", "description": "owner/name" },
                    "state": { "type": "string", "default": "open", "enum": ["open", "closed", "all"] },
                    "title": { "type": "string" },
                    "head": { "type": "string", "description": "rama con el commit (para publicar como PR)" },
                    "base": { "type": "string", "default": "main" },
                    "confirm": { "type": "boolean", "default": false }
                }
            }
        },
        {
            "name": "delegate",
            "description": "Delegación parasitaria a cuentas gratuitas cloud: breaker de 3 estados POR CUENTA, presupuesto por hora (global y por modelo), coste MEDIDO (tokens y latencia reales de providers::execute_chat) con la fuente del precio (NO VERIFICADO si no hay fuente) y anuncio al bus SIN credenciales (sólo modo de auth y ruta del fichero). Con el circuito abierto, la voz local (SEERE) sigue disponible: el enjambre nunca se queda mudo.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["accounts", "status", "announce", "execute"], "default": "status" },
                    "prompt": { "type": "string", "description": "encargo a delegar (para execute)" },
                    "account": { "type": "string", "description": "cuenta concreta (para execute); vacío = la que elija el enrutado" },
                    "kind": { "type": "string", "default": "research", "description": "tipo de tarea (plantilla de prompt)" }
                }
            }
        },
        {
            "name": "api_keys",
            "description": "Gestiona el registro de API keys por agente: acuña una clave nueva y revocable sin tocar la clave principal, revoca una existente o lista el registro. La revocacion es destructiva y exige confirm=true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["mint", "revoke", "list"] },
                    "label": { "type": "string", "description": "etiqueta del agente (cloud-sandbox, agente-externo, ...)" },
                    "target": { "type": "string", "description": "clave completa, prefijo de 8+ caracteres o etiqueta a revocar" },
                    "reveal": { "type": "boolean", "default": false, "description": "listar las claves en claro (queda auditado)" },
                    "confirm": { "type": "boolean", "default": false }
                }
            }
        },
        {
            "name": "npu_vs4l_probe",
            "description": "Sondea el driver VS4L/NPU del Exynos 2100 (/dev/vertex10) con el metodo correcto: ioctl SIN argumento => el driver devuelve el TAMANO de datos que espera. Distingue EINVAL (reconocido, argumento invalido) de ENOTTY (no existe). No escribe nada y no necesita root (nodo 0644).",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "task_queue",
            "description": "Cola de tareas COMPARTIDA de MegaDaemon (movil + PC + agentes). Acciones: list (ver cola), submit (encolar), claim (TOMAR tareas con lease para este nodo), lease (renovar), migrate (pasar el trabajo de un nodo a otro cuando uno falla o se queda sin cuota), pause (pausar por cuota/red), merge (fusionar con lo que hay en Drive). IMPORTANTE: si la cuota esta en 'pausa', claim NO devuelve tareas: termina tu paso actual, guarda tu checkpoint y espera al reset.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "accion": {
                        "type": "string",
                        "description": "list | submit | claim | lease | migrate | pause | merge | sync"
                    },
                    "nodo": { "type": "string", "description": "Nombre de este nodo (pc, movil, agente-...)." },
                    "kind": { "type": "string", "description": "Tipo de tarea (para submit)." },
                    "payload": { "type": "string", "description": "JSON del payload (para submit)." },
                    "prioridad": { "type": "integer", "description": "Prioridad (0-9, 9 = maxima)." },
                    "max": { "type": "integer", "description": "Máximo de tareas a tomar (claim)." },
                    "lease_s": { "type": "integer", "description": "Duración del lease en segundos (defecto 900)." },
                    "desde": { "type": "string", "description": "Nodo origen (migrate)." },
                    "hacia": { "type": "string", "description": "Nodo destino (migrate)." },
                    "until": { "type": "integer", "description": "Epoch hasta el que se pausa (pause)." },
                    "motivo": { "type": "string", "description": "Motivo de la pausa (cuota, red...)." }
                }
            }
        },
        {
            "name": "http_fetch",
            "description": "Transferencias HTTP nativas (el 'curl' de MegaDaemon, en Rust puro: el movil NO tiene curl/wget). Descarga una URL a fichero (con resume por Range, reintentos y verificacion SHA-256), consulta cabeceras (HEAD), hace GET de texto o POST de un fichero. Tambien acepta una lista de URLs para descargarlas EN PARALELO. Devuelve bytes, sha256, segundos y MB/s reales.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL a descargar/consultar (para paralelo, la lista va en 'urls')." },
                    "urls": { "type": "array", "items": { "type": "string" }, "description": "Varias URLs para descargar en paralelo." },
                    "destino": { "type": "string", "description": "Ruta del fichero de destino (si se omite, se devuelve el cuerpo en memoria)." },
                    "method": { "type": "string", "description": "GET (defecto), HEAD o POST/PUT." },
                    "sha256": { "type": "string", "description": "SHA-256 esperado: si no coincide, NO se da por buena la descarga." },
                    "workers": { "type": "integer", "description": "Hilos en paralelo (defecto 4)." },
                    "timeout_s": { "type": "integer", "description": "Timeout global en segundos (defecto 120)." },
                    "retries": { "type": "integer", "description": "Reintentos (defecto 3)." }
                }
            }
        },
        {
            "name": "antigravity_sessions",
            "description": "Instrumentacion completa (solo lectura) de las sesiones de Antigravity: estado agregado, lista, sub-agentes muertos/colgados, arbol padre->hijos, detalle con transcript y tasks/log del daemon.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "accion": { "type": "string", "description": "estado (defecto) | list | dead | stalled | tree | detail | log | tasks" },
                    "conversation_id": { "type": "string", "description": "ID de conversacion para detail o raiz opcional para tree." },
                    "tail": { "type": "integer", "description": "Numero de lineas/pasos finales para detail o log (defecto 20)." },
                    "app_dir": { "type": "string", "description": "Directorio alternativo de datos de Antigravity (opcional)." }
                }
            }
        },
{
            "name": "re_analyze",
            "description": "MOTOR DE RE/DEOFUSCACION de MegaDaemon (punto de entrada UNICO): identifica el binario (ELF/PE/MachO/DEX/APK/Go/UPX) CON evidencia, extrae cadenas interesting, y deofusca de verdad: MBA con PRUEBA de equivalencia (BDD), control-flow flattening con prueba diferencial, strings cifrados, constantes ofuscadas y predicados opacos (accion 'motor'); la accion 'mba' simplifica una expresion y devuelve su demostracion. Accion catalogo lista TODAS las capacidades con el modulo que las implementa. NUNCA ejecuta el binario analizado: es analisis estatico.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "accion": { "type": "string", "description": "catalogo | identify | analyze | strings | deobfuscate | motor | mba" },
                    "ruta": { "type": "string", "description": "Ruta del binario (obligatoria salvo en catalogo y mba)." },
                    "max_cadenas": { "type": "integer", "description": "Maximo de cadenas interesting (defecto 60)." },
                    "plat_key": { "type": "string", "description": "Clave de plataforma para descifrar strings." },
                    "expr": { "type": "string", "description": "Expresion MBA a simplificar (accion mba), p.ej. '(x ^ y) + 2*(x & y)'." },
                    "ventana_disasm": { "type": "integer", "description": "Bytes de codigo a desensamblar para el motor (defecto 262144)." }
                }
            }
},
{
            "name": "re_fusion",
            "description": "RECONSTRUCCION MULTI-MOTOR: ejecuta los motores que ya tiene MegaDaemon (Ghidra headless para el pseudocodigo, Rizin para funciones/imports/strings, DiE+Binwalk para el formato, el motor propio para MBA/CFF/crypto y el pclntab para los simbolos Go con fichero:linea), COMPARA lo que da cada uno y FUSIONA lo mejor: pseudocodigo + nombre Go original + fichero:linea + strings. Acciones: compare (rapido, sin Ghidra) | fusion (todo) | informe (solo la tabla comparativa). El parametro binario es obligatorio; simbolos/anclas son los TSV; ghidra_dir y java_home se autodetectan (Ghidra 11.2+ exige JDK 21).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "accion": { "type": "string", "description": "compare | fusion | informe" },
                    "binario": { "type": "string", "description": "Ruta del binario a reconstruir (obligatoria)." },
                    "simbolos": { "type": "string", "description": "TSV de simbolos (0xADDR <TAB> nombre <TAB> comentario)." },
                    "anclas": { "type": "string", "description": "TSV de anclas a decompilar (0xADDR <TAB> nombre <TAB> simbolo <TAB> loc <TAB> tag)." },
                    "out": { "type": "string", "description": "Directorio de salida (por defecto /workspace/agy-re/tools-out/fusion)." },
                    "project_dir": { "type": "string", "description": "Proyecto Ghidra." },
                    "project": { "type": "string", "description": "Nombre del proyecto Ghidra (Go)." },
                    "ghidra_dir": { "type": "string", "description": "Instalacion de Ghidra (autodetectada si se omite)." },
                    "java_home": { "type": "string", "description": "JAVA_HOME (autodetectado si se omite)." },
                    "max_memoria": { "type": "string", "description": "Heap de la JVM (defecto 1400M; con menos de 3 GB de RAM usar 1200M)." },
                    "timeout_s": { "type": "integer", "description": "Timeout por fase de Ghidra en segundos (defecto 1800)." },
                    "sin_ghidra": { "type": "string", "description": "Poner cualquier valor para NO usar Ghidra (evita la JVM)." },
                    "sin_rizin": { "type": "string", "description": "Poner cualquier valor para NO usar Rizin." },
                    "sin_die": { "type": "string", "description": "Poner cualquier valor para NO usar DiE/Binwalk." }
                }
            }
        },
{
            "name": "supervisor",
            "description": "WATCHDOG de Android: instala (o consulta) el supervisor en /data/adb/service.d/99-megadaemon.sh, que Magisk ejecuta en cada boot. Garantiza que el daemon siga vivo, que la auto-actualizacion entre en servicio (el daemon sale con codigo 10 y el watchdog lo relanza) y que el parche del endpoint y los montajes se mantengan. Acciones: estado | instalar | desinstalar | watchdog (el texto del script).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "accion": { "type": "string", "description": "estado | instalar | desinstalar | watchdog" }
                }
            }
        },
{
            "name": "frida_hooks",
            "description": "Hooks de Frida para OBSERVAR Antigravity sin tocar sus datos: sqlite (que tablas/consultas toca la app, con credenciales redactadas), red (a que hosts conecta: sirve para verificar el parche del endpoint) y ficheros (que ficheros abre o borra). Acciones: listar | obtener.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "accion": { "type": "string", "description": "listar | obtener" },
                    "nombre": { "type": "string", "description": "sqlite | red | ficheros" }
                }
            }
        },
{
            "name": "phone_backup",
            "description": "Motor de backup nativo de apps (Rust puro, sin curl ni shell): inventario de apps del movil, contenedor cifrado AEGIS-256 por segmentos con manifiesto sha256 por fichero, verificacion del AEAD, restauracion (simulacro por defecto, exige confirm=true) y SUBIDA PIPELINING a Drive (sube cada segmento ya cerrado mientras se recoge el siguiente; mide MiB/s y ETA). Anti-congelacion: nice/ionice de la clase mas baja, sleep entre lotes y descanso si la carga del sistema se dispara. Acciones: inventory | create | list | verify | restore | upload. create/restore/upload escriben o publican en Drive: exigen confirm=true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "description": "inventory | create | list | verify | restore | upload" },
                    "out": { "type": "string", "description": "Directorio/ruta del contenedor." },
                    "folder_id": { "type": "string", "description": "ID de carpeta en Drive (para upload)." },
                    "packages": { "type": "array", "items": { "type": "string" } },
                    "confirm": { "type": "boolean" }
                }
            }
        },
        {
            "name": "quota_status",
            "description": "Estado de la CUOTA de los proveedores/modelos y presupuesto diario USD: cuanto queda (%), cuando resetea y en que modo esta (normal/aviso/ahorro/pausa). Respuesta MUY compacta (~70 tokens) para no gastar. En 'pausa' significa que la cuota se agota: termina el paso actual, guarda tu checkpoint y espera al reset; no sigas abriendo tareas nuevas.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "provider": {
                        "type": "string",
                        "description": "Id del proveedor (opcional; sin el, el peor de todos)."
                    },
                    "full": {
                        "type": "boolean",
                        "description": "true = detalle completo + daily_budget (gasta mas tokens); por defecto, snapshot compacto."
                    },
                    "set_daily_budget_usd": {
                        "type": ["number", "null"],
                        "description": "Opcional: fija el limite diario de gasto en USD (null = sin limite)."
                    }
                }
            }
        },
        {
            "name": "csc_verify",
            "description": "Verificador post-parche del CSC: lee el CSC activo (mps_code.dat), descifra el cscfeature.xml OMC del movil, comprueba el valor de CscFeature_VoiceCall_ConfigRecording, lo contrasta con el fileSize de contents.db, busca copias en claro del framework y devuelve el mapa vivo de los CSC con la feature. Con expect_value exige un valor EXACTO (p.ej. 'RecordingAllowed' o 'RecordingAllowedByMenu,disclaimer_latin'): si el valor presente difiere devuelve verdict MISMATCH, y siempre indica la variante (sin-disclaimer / con-disclaimer / apagado).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "expect_value": {
                        "type": "string",
                        "description": "Valor exacto que se espera para CscFeature_VoiceCall_ConfigRecording (opcional)."
                    }
                }
            }
        },
    ]);
    let mut out = base.as_array().cloned().unwrap_or_default();
    if let Some(extra) = crate::titanium_backup::tool_definitions_mcp().as_array() {
        out.extend(extra.clone());
    }
    if let Some(extra) = crate::swift_container::tool_definitions_mcp().as_array() {
        out.extend(extra.clone());
    }
    Value::Array(out)
}

/// Despacha las herramientas nuevas. `None` = no es de este grupo.
pub fn dispatch(name: &str, args: &Value) -> Option<Value> {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let n = |k: &str| args.get(k).and_then(|v| v.as_u64()).unwrap_or(0);
    let i = |k: &str| args.get(k).and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let b = |k: &str| args.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
    let sa = |k: &str| {
        args.get(k)
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect::<Vec<String>>())
            .unwrap_or_default()
    };

    // Bloque "backups Titanium / catalogo / plan de migracion" y bloque
    // "contenedor Swift Backup": se resuelven antes del `match` porque sus
    // nombres no chocan con los de aqui.
    if let Some(r) = crate::titanium_backup::dispatch_mcp(name, args) {
        return Some(r);
    }
    if let Some(r) = crate::swift_container::dispatch_mcp(name, args) {
        return Some(r);
    }

    let r = match name {
        "drive_backup" => {
            // Toda la ruta Drive del daemon (token OAuth + subida resumible)
            // vive en Rust: aqui no se shella a curl ni se invoca Python.
            let action = {
                let a = s("action");
                if a.trim().is_empty() { "upload".to_string() } else { a }
            };
            let st = crate::cockpit_server::state_dir();
            match action.as_str() {
                "status" => {
                    let v = crate::drive_native::status(Some(&st));
                    json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }] })
                }
                "download" => {
                    let file_id = s("file_id");
                    if file_id.is_empty() {
                        json!({ "content": [{ "type": "text", "text": "drive_backup download: falta file_id" }], "isError": true })
                    } else {
                        let dest = {
                            let d = s("dest");
                            if d.trim().is_empty() {
                                crate::drive_native::default_download_path(&file_id).display().to_string()
                            } else { d }
                        };
                        match crate::drive_native::download_file(&file_id, std::path::Path::new(&dest), Some(&st)) {
                            Ok(n) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&json!({ "ok": true, "bytes": n, "dest": dest })).unwrap_or_default() }] }),
                            Err(e) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&json!({ "ok": false, "error": e })).unwrap_or_default() }], "isError": true }),
                        }
                    }
                }
                "sync" => {
                    if !b("confirm") {
                        json!({ "content": [{ "type": "text", "text": "drive_backup sync exige confirm=true" }] })
                    } else {
                        let path = s("path");
                        let name = {
                            let nm = s("name");
                            if nm.trim().is_empty() {
                                std::path::Path::new(&path)
                                    .file_name()
                                    .map(|x| x.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "MegaDaemon_Sync".to_string())
                            } else { nm }
                        };
                        let folder = s("folder_id");
                        let raw_w = n("workers") as usize;
                        let workers = if raw_w == 0 { 8 } else { raw_w.clamp(1, 16) };
                        match crate::drive_native::upload_dir_parallel(
                            std::path::Path::new(&path),
                            &name,
                            &folder,
                            workers,
                            b("verify_delete"),
                            Some(&st),
                        ) {
                            Ok(v) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }] }),
                            Err(e) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&json!({ "ok": false, "error": e })).unwrap_or_default() }], "isError": true }),
                        }
                    }
                }
                _ => {
                    if !b("confirm") {
                        json!({ "content": [{ "type": "text", "text": "drive_backup exige confirm=true (sube el contenido a Google Drive)" }] })
                    } else {
                        let path = s("path");
                        if path.trim().is_empty() {
                            json!({ "content": [{ "type": "text", "text": "drive_backup upload: falta path" }], "isError": true })
                        } else {
                            let name = {
                                let n = s("name");
                                if n.trim().is_empty() {
                                    std::path::Path::new(&path)
                                        .file_name()
                                        .map(|x| x.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "backup.bin".to_string())
                                } else { n }
                            };
                            let folder = s("folder_id");
                            match crate::drive_native::upload_path(std::path::Path::new(&path), &name, &folder, Some(&st)) {
                                Ok(u) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&json!({
                                    "ok": true, "file_id": u.file_id, "name": u.name,
                                    "bytes": u.bytes, "sha256": u.sha256,
                                    "transport": "nativo (ureq, Rust puro)"
                                })).unwrap_or_default() }] }),
                                Err(e) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&json!({ "ok": false, "error": e })).unwrap_or_default() }], "isError": true }),
                            }
                        }
                    }
                }
            }
        }
        "channel_publish" => {
            // Publicar la URL del dispositivo fuera del movil. NO es
            // destructivo, pero SI expone una URL publica: por eso exige
            // `confirm: true` (misma politica que el resto de tools).
            if !b("confirm") {
                json!({ "content": [{ "type": "text", "text": "channel_publish exige confirm=true (publica la URL del dispositivo en un paste publico)" }] })
            } else {
                let out = crate::channel::publish(
                    None,
                    Some(s("quick")).filter(|x| !x.is_empty()).as_deref(),
                    Some(s("duckdns")).filter(|x| !x.is_empty()).as_deref(),
                    Some(s("ip_publica")).filter(|x| !x.is_empty()).as_deref(),
                    Some(s("cloudflared_pids")).filter(|x| !x.is_empty()).as_deref(),
                );
                let txt = serde_json::to_string_pretty(&out).unwrap_or_default();
                json!({ "content": [{ "type": "text", "text": txt }] })
            }
        }
        "npu_vs4l_probe" => {
            let rep = crate::npu_vs4l::probe();
            let txt = serde_json::to_string_pretty(&rep).unwrap_or_default();
            json!({ "content": [{ "type": "text", "text": txt }] })
        }
        "task_queue" => {
            let accion = args.get("accion").and_then(|v| v.as_str()).unwrap_or("list");
            let nodo = args.get("nodo").and_then(|v| v.as_str()).unwrap_or("pc");
            let v = match accion {
                "list" => {
                    let t = crate::task_scheduler::list();
                    json!({ "ok": true, "count": t.len(), "tareas": t.iter().map(|x| json!({
                        "id": x.id, "kind": x.kind, "state": x.state.as_str(),
                        "priority": x.priority, "node": x.node,
                        "resume_after": x.resume_after, "motivo_pausa": x.motivo_pausa,
                    })).collect::<Vec<_>>() })
                }
                "submit" => {
                    let kind = args.get("kind").and_then(|v| v.as_str()).unwrap_or("generica");
                    let payload = args.get("payload")
                        .and_then(|v| v.as_str())
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or(Value::Null);
                    let prio = args.get("prioridad").and_then(|v| v.as_u64()).unwrap_or(5) as u8;
                    let id = crate::task_scheduler::submit(&kind, payload, prio, vec![], Default::default());
                    json!({ "ok": true, "id": id })
                }
                "claim" => {
                    if crate::quota_guard::nivel_global() == crate::quota_guard::Nivel::Pausa {
                        json!({
                            "ok": false,
                            "motivo": "cuota en pausa: termina tu paso actual, guarda el checkpoint y espera al reset",
                            "aviso": crate::quota_guard::aviso_para_agente(),
                        })
                    } else {
                        let max = args.get("max").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
                        let lease = args.get("lease_s").and_then(|v| v.as_u64()).unwrap_or(900);
                        let ids = crate::task_scheduler::repartir_por_nodo(nodo, max, lease);
                        json!({ "ok": true, "nodo": nodo, "tomadas": ids })
                    }
                }
                "lease" => {
                    let s = args.get("lease_s").and_then(|v| v.as_u64()).unwrap_or(900);
                    json!({ "ok": true, "renovadas": crate::task_scheduler::renovar_lease(nodo, s) })
                }
                "migrate" => {
                    let desde = args.get("desde").and_then(|v| v.as_str()).unwrap_or("");
                    let hacia = args.get("hacia").and_then(|v| v.as_str()).unwrap_or("");
                    json!({ "ok": true, "migradas": crate::task_scheduler::migrar_tareas(desde, hacia) })
                }
                "pause" => {
                    let until = args.get("until").and_then(|v| v.as_u64());
                    let motivo = args.get("motivo").and_then(|v| v.as_str()).unwrap_or("cuota");
                    json!({ "ok": true, "pausadas": crate::task_scheduler::pausar_por_cuota(nodo, until, motivo) })
                }
                "sync" => match crate::task_sync::sincronizar_drive() {
                    Ok(r) => r,
                    Err(e) => json!({ "ok": false, "error": e }),
                },
                "merge" => {
                    let export = crate::task_scheduler::export_all_tasks(nodo);
                    json!({ "ok": true, "export": export })
                }
                _ => json!({ "ok": false, "error": format!("accion desconocida: {accion}") }),
            };
            let txt = serde_json::to_string(&v).unwrap_or_default();
            json!({ "content": [{ "type": "text", "text": txt }] })
        }
        "http_fetch" => {
            let mut o = crate::http_transfer::Opciones::default();
            if let Some(t) = args.get("timeout_s").and_then(|v| v.as_u64()) {
                o.timeout_s = t;
            }
            if let Some(r) = args.get("retries").and_then(|v| v.as_u64()) {
                o.reintentos = r as u32;
            }
            if let Some(s) = args.get("sha256").and_then(|v| v.as_str()) {
                o.sha256_esperado = Some(s.to_string());
            }
            if let Some(d) = args.get("destino").and_then(|v| v.as_str()) {
                if !d.is_empty() {
                    o.destino = Some(std::path::PathBuf::from(d));
                }
            }
            let method = args.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
            let urls: Vec<String> = args
                .get("urls")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let v = if !urls.is_empty() {
                // Paralelo.
                let workers = args.get("workers").and_then(|x| x.as_u64()).unwrap_or(4) as usize;
                let dir = o
                    .destino
                    .clone()
                    .unwrap_or_else(|| std::path::PathBuf::from("/data/local/tmp/megadaemon/downloads"));
                let _ = std::fs::create_dir_all(&dir);
                crate::http_transfer::descargar_paralelo(&urls, &dir, workers, &o)
            } else {
                let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
                if url.is_empty() {
                    json!({ "content": [{ "type": "text", "text": "http_fetch necesita 'url' o 'urls'" }] })
                } else if method.eq_ignore_ascii_case("HEAD") {
                    match crate::http_transfer::head(url, &o) {
                        Ok(h) => serde_json::to_value(h).unwrap_or(Value::Null),
                        Err(e) => json!({ "error": e }),
                    }
                } else if method.eq_ignore_ascii_case("GET") && o.destino.is_none() {
                    match crate::http_transfer::get_texto(url, &o) {
                        Ok((st, t)) => json!({ "status": st, "body": t.chars().take(4000).collect::<String>() }),
                        Err(e) => json!({ "error": e }),
                    }
                } else if method.eq_ignore_ascii_case("POST") || method.eq_ignore_ascii_case("PUT") {
                    let f = o.destino.clone().unwrap_or_default();
                    match crate::http_transfer::subir(url, &f, &o, method) {
                        Ok(r) => serde_json::to_value(r).unwrap_or(Value::Null),
                        Err(e) => json!({ "error": e }),
                    }
                } else {
                    match crate::http_transfer::descargar_con_reintentos(url, &o) {
                        Ok(r) => serde_json::to_value(r).unwrap_or(Value::Null),
                        Err(e) => json!({ "error": e }),
                    }
                }
            };
            let txt = serde_json::to_string(&v).unwrap_or_default();
            json!({ "content": [{ "type": "text", "text": txt }] })
        }
                "phone_backup" => {
                    // Motor de backup nativo de apps: la MISMA libreria que el
                    // binario `md-backup` (paridad CLI <-> MCP <-> agent_coding).
                    let action = {
                        let a = s("action");
                        if a.trim().is_empty() { "inventory".to_string() } else { a }
                    };
                    let st = crate::cockpit_server::state_dir();
                    let root = std::path::PathBuf::from({
                        let r = s("root");
                        if r.is_empty() { "/".to_string() } else { r }
                    });
                    let packages = sa("packages");
                    // `create`, `restore` y `upload` escriben o publican: exigen
                    // `confirm: true` (misma politica que el resto de tools).
                    let needs_confirm = matches!(action.as_str(), "create" | "restore" | "upload");
                    if needs_confirm && !b("confirm") {
                        json!({ "content": [{ "type": "text", "text": format!("phone_backup {action} exige confirm=true") }], "isError": true })
                    } else {
                        let res: Result<Value, String> = match action.as_str() {
                            "inventory" => {
                                let opts = crate::backup_apps::InventoryOptions {
                                    root: root.clone(),
                                    packages: packages.clone(),
                                    measure: b("measure"),
                                    ..Default::default()
                                };
                                crate::backup_apps::inventory(&opts).map(|apps| {
                                    json!({
                                        "ok": true,
                                        "packages": apps.len(),
                                        "apps": apps,
                                    })
                                })
                            }
                            "list" => {
                                let out = s("out");
                                if out.is_empty() {
                                    Err("phone_backup list necesita 'out'".to_string())
                                } else {
                                    crate::backup_apps::list(std::path::Path::new(&out)).map(|v| {
                                        json!({ "ok": true, "containers": v })
                                    })
                                }
                            }
                            "verify" => {
                                let out = s("out");
                                if out.is_empty() {
                                    Err("phone_backup verify necesita 'out'".to_string())
                                } else {
                                    let key = match s("key") {
                                        p if p.trim().is_empty() => Ok(None),
                                        p => crate::backup_apps::read_key_file(std::path::Path::new(&p)).map(Some),
                                    };
                                    match key {
                                        Err(e) => Err(e),
                                        Ok(key) => crate::backup_apps::verify(std::path::Path::new(&out), key.as_deref()).map(|r| {
                                            json!({ "ok": r.ok, "report": r })
                                        }),
                                    }
                                }
                            }
                            "create" => {
                                let out = s("out");
                                if out.is_empty() {
                                    Err("phone_backup create necesita 'out'".to_string())
                                } else {
                                    let opts = crate::backup_apps::CreateOptions {
                                        root: root.clone(),
                                        out: std::path::PathBuf::from(&out),
                                        packages: packages.clone(),
                                        segment_bytes: if n("segment_bytes") > 0 { n("segment_bytes") } else { crate::backup_apps::DEFAULT_SEGMENT_BYTES },
                                        passphrase: s("password").into(),
                                        key_out: {
                                            let k = s("key_out");
                                            if k.is_empty() { None } else { Some(std::path::PathBuf::from(k)) }
                                        },
                                        ..Default::default()
                                    };
                                    let mut sink = crate::backup_apps::NullSink;
                                    crate::backup_apps::create(&opts, &mut sink).map(|r| json!({ "ok": true, "report": r }))
                                }
                            }
                            "restore" => {
                                let out = s("out");
                                if out.is_empty() {
                                    Err("phone_backup restore necesita 'out'".to_string())
                                } else {
                                    let key = match s("key") {
                                        p if p.trim().is_empty() => Ok(None),
                                        p => crate::backup_apps::read_key_file(std::path::Path::new(&p)).map(Some),
                                    };
                                    let key = match key {
                                        Err(e) => return Some(json!({ "content": [{ "type": "text", "text": json!({"ok": false, "error": e}).to_string() }], "isError": true })),
                                        Ok(k) => k,
                                    };
                                    let dest = {
                                        let d = s("dest");
                                        if d.is_empty() { "/".to_string() } else { d }
                                    };
                                    let opts = crate::backup_apps::RestoreOptions {
                                        dir: std::path::PathBuf::from(&out),
                                        dest: std::path::PathBuf::from(&dest),
                                        packages: packages.clone(),
                                        confirm: b("confirm"),
                                        overwrite: b("overwrite"),
                                        key,
                                    };
                                    crate::backup_apps::restore(&opts).map(|r| json!({ "ok": r.mismatches.is_empty(), "report": r }))
                                }
                            }
                            "upload" => {
                                let out = s("out");
                                let folder = s("folder_id");
                                if out.is_empty() {
                                    Err("phone_backup upload necesita 'out'".to_string())
                                } else if folder.is_empty() {
                                    Err("phone_backup upload necesita 'folder_id'".to_string())
                                } else {
                                    let workers = if n("workers") == 0 { 3usize } else { (n("workers") as usize).clamp(1, 16) };
                                    let state_dir = {
                                        let sd = s("state_dir");
                                        if sd.is_empty() { st.clone() } else { sd }
                                    };
                                    // El `?` no vale aquí (esta función devuelve
                                    // `Option`): la subida va en una clausura que
                                    // sí devuelve `Result`.
                                    (|| -> Result<Value, String> {
                                    // Subcarpeta (p.ej. `P0-apps`): se busca o se crea
                                    // con el VFS nativo.
                                    let folder = match s("subfolder") {
                                        sub if sub.trim().is_empty() => folder.clone(),
                                        sub => crate::drive_native::ensure_folder(&sub, &folder, Some(&state_dir))?,
                                    };
                                    let name = {
                                        let nm = s("name");
                                        if nm.is_empty() {
                                            std::path::Path::new(&out)
                                                .file_name()
                                                .map(|x| x.to_string_lossy().to_string())
                                                .unwrap_or_else(|| "backup".to_string())
                                        } else { nm }
                                    };
                                    let mut up = crate::backup_apps::DriveUploader::new(&folder, &name, workers);
                                    up.chunk_bytes = if n("chunk_bytes") > 0 { n("chunk_bytes") } else { crate::backup_apps::DRIVE_CHUNK_MIN };
                                    up.state_dir = Some(state_dir.clone());
                                    up.delete_after_upload = b("delete_after_upload");
                                    up.container_dir = Some(std::path::PathBuf::from(&out));
                                    up.start()?;
                                    let header = crate::backup_apps::read_header(std::path::Path::new(&out))?;
                                    for sg in &header.segments {
                                        let p = std::path::Path::new(&out).join("segments").join(&sg.name);
                                        if !p.exists() { continue; }
                                        // `submit` es un método del trait `SegmentSink`:
                                        // se llama con la ruta completa para no
                                        // depender del `use`.
                                        crate::backup_apps::SegmentSink::submit(
                                            &mut up,
                                            &crate::backup_apps::SealedSegment {
                                                index: sg.index,
                                                path: p,
                                                info: sg.clone(),
                                            },
                                        )?;
                                    }
                                    let manifest = crate::backup_apps::read_manifest(std::path::Path::new(&out))?;
                                    crate::backup_apps::SegmentSink::finish(&mut up, &header, &manifest)?;
                                    let stats = up.stats();
                                    let up_ms = stats.ended_ms.saturating_sub(stats.started_ms).max(1);
                                    Ok(json!({
                                        "ok": up.errors().is_empty(),
                                        "segments": stats.segments,
                                        "bytes": stats.bytes,
                                        "mib_per_s": (crate::backup_apps::mib_per_s(stats.bytes, up_ms) * 1000.0).round() / 1000.0,
                                        "eta_s": crate::backup_apps::eta_s(stats.bytes, stats.total_bytes.max(stats.bytes), up_ms).round() as u64,
                                        "errors": up.errors(),
                                    }))
                                    })()
                                }
                            }
                            other => Err(format!("unknown phone_backup action '{}'", other)),
                        };
                        match res {
                            Ok(v) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }] }),
                            Err(e) => json!({ "content": [{ "type": "text", "text": json!({"ok": false, "error": e}).to_string() }], "isError": true }),
                        }
                    }
                }
        "antigravity_sessions" => {
            let accion = args.get("accion").and_then(|v| v.as_str()).unwrap_or("estado");
            let conv_id = args.get("conversation_id").and_then(|v| v.as_str()).unwrap_or("");
            let tail = args.get("tail").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
            let dir_buf = match args.get("app_dir").and_then(|v| v.as_str()) {
                Some(d) if !d.trim().is_empty() => std::path::PathBuf::from(d.trim()),
                _ => crate::antigravity_sessions::default_app_dir(),
            };
            let dir = dir_buf.as_path();
            let v = match accion {
                "list" => crate::antigravity_sessions::sesiones_en(dir)
                    .map(|x| serde_json::to_value(x).unwrap_or(Value::Null)),
                "dead" => crate::antigravity_sessions::muertas_en(dir)
                    .map(|x| serde_json::to_value(x).unwrap_or(Value::Null)),
                "stalled" => crate::antigravity_sessions::colgadas_en(dir)
                    .map(|x| serde_json::to_value(x).unwrap_or(Value::Null)),
                "tree" => crate::antigravity_sessions::arbol_en(
                    dir,
                    if conv_id.is_empty() { None } else { Some(conv_id) },
                ),
                "detail" => crate::antigravity_sessions::detalle_sesion_en(dir, conv_id, tail),
                "log" => crate::antigravity_sessions::daemon_log_en(dir, tail)
                    .map(|lines| json!({ "lines": lines })),
                "tasks" => crate::antigravity_sessions::tareas_daemon_en(dir),
                _ => crate::antigravity_sessions::estado_en(dir)
                    .map(|x| serde_json::to_value(x).unwrap_or(Value::Null)),
            };
            let txt = match v {
                Ok(x) => serde_json::to_string_pretty(&x).unwrap_or_default(),
                Err(e) => format!("ERROR: {e}"),
            };
            json!({ "content": [{ "type": "text", "text": txt }] })
        }
        "sqlite_mitm" => {
            let accion = args.get("accion").and_then(|v| v.as_str()).unwrap_or("estado");
            let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
            match accion {
                "estado" => {
                    let a = crate::sqlite_mitm::ruta_db_antigravity();
                    let p = crate::sqlite_mitm::ruta_db_propia();
                    let sz = |x: &std::path::Path| std::fs::metadata(x).map(|m| m.len()).unwrap_or(0);
                    json!({
                        "ok": true,
                        "antigravity": {"ruta": a.display().to_string(), "bytes": sz(&a),
                                        "modo": "SOLO LECTURA (busy_timeout 2s): Antigravity puede escribir"},
                        "megadaemon": {"ruta": p.display().to_string(), "bytes": sz(&p),
                                       "modo": "escritura propia (WAL, indexada)"},
                    })
                }
                "replicar" => {
                    let limite = args.get("limite").and_then(|v| v.as_u64()).unwrap_or(5000) as usize;
                    match crate::sqlite_mitm::replicar(limite) {
                        Ok(v) => v,
                        Err(e) => json!({"ok": false, "error": e}),
                    }
                }
                "servida" => {
                    let ocultar: Vec<String> = args
                        .get("ocultar")
                        .and_then(|v| v.as_array())
                        .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                        .unwrap_or_default();
                    let reescribir: Vec<(String, String)> = args
                        .get("reescribir")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_str())
                                .filter_map(|t| t.split_once('=').map(|(a, b)| (a.to_string(), b.to_string())))
                                .collect()
                        })
                        .unwrap_or_default();
                    let dest = std::path::PathBuf::from(if s("destino").is_empty() {
                        format!("{}/antigravity_servida.db", crate::cockpit_server::state_dir())
                    } else {
                        s("destino")
                    });
                    match crate::sqlite_mitm::generar_db_servida(&ocultar, &reescribir, &dest) {
                        Ok(v) => json!({"ok": true, "resultado": v,
                                       "montaje": crate::sqlite_mitm::plan_montaje(&dest)}),
                        Err(e) => json!({"ok": false, "error": e}),
                    }
                }
                "montaje" => {
                    let dest = std::path::PathBuf::from(if s("destino").is_empty() {
                        format!("{}/antigravity_servida.db", crate::cockpit_server::state_dir())
                    } else {
                        s("destino")
                    });
                    crate::sqlite_mitm::plan_montaje(&dest)
                }
                _ => json!({"ok": false, "error": "accion desconocida: usa estado|replicar|servida|montaje"}),
            }
        }
        "re_analyze" => {
            let accion = args.get("accion").and_then(|v| v.as_str()).unwrap_or("catalogo");
            let ruta = args.get("ruta").and_then(|v| v.as_str()).unwrap_or("");
            let v = match accion {
                "catalogo" => crate::re_engine::catalogo(),
                "identify" => match crate::re_engine::identificar(std::path::Path::new(ruta)) {
                    Ok(id) => serde_json::to_value(id).unwrap_or(Value::Null),
                    Err(e) => json!({ "ok": false, "error": e }),
                },
                "strings" => match std::fs::read(ruta) {
                    Ok(d) => json!({
                        "ok": true,
                        "cadenas": crate::re_engine::cadenas_interesantes(
                            &d,
                            args.get("max_cadenas").and_then(|v| v.as_u64()).unwrap_or(60) as usize,
                        ),
                    }),
                    Err(e) => json!({ "ok": false, "error": format!("leer: {e}") }),
                },
                "deobfuscate" => {
                    let key = args.get("plat_key").and_then(|v| v.as_str()).unwrap_or("");
                    let rep = crate::deobfuscate::deobfuscate_binary(key, ruta, 500);
                    serde_json::to_value(rep).unwrap_or(Value::Null)
                }
                "motor" => {
                    // MOTOR DE DEOFUSCACION (unico punto de entrada): MBA con
                    // prueba BDD, control-flow flattening con prueba diferencial,
                    // strings cifrados, constantes ofuscadas y predicados opacos.
                    let mut o = crate::re_engine::OpcionesDeofuscacion::default();
                    if let Some(n) = args.get("max_cadenas").and_then(|v| v.as_u64()) {
                        o.max_cadenas = n as usize;
                    }
                    if let Some(n) = args.get("ventana_disasm").and_then(|v| v.as_u64()) {
                        o.ventana_disasm = n as usize;
                    }
                    match crate::re_engine::deobfuscar(std::path::Path::new(ruta), &o) {
                        Ok(v) => v,
                        Err(e) => json!({ "ok": false, "error": e }),
                    }
                }
                "mba" => {
                    // Simplificacion MBA con PRUEBA de equivalencia (BDD).
                    let expr = args.get("expr").and_then(|v| v.as_str()).unwrap_or("");
                    crate::re_engine::mba(expr)
                }
                _ => {
                    let mut o = crate::re_engine::OpcionesAnalisis::default();
                    o.max_cadenas = args.get("max_cadenas").and_then(|v| v.as_u64()).unwrap_or(60) as usize;
                    match crate::re_engine::analizar(std::path::Path::new(ruta), &o) {
                        Ok(v) => v,
                        Err(e) => json!({ "ok": false, "error": e }),
                    }
                }
            };
            let txt = serde_json::to_string(&v).unwrap_or_default();
            json!({ "content": [{ "type": "text", "text": txt }] })
        }
        "quota_status" => {
            if let Some(lim) = args.get("set_daily_budget_usd") {
                if lim.is_null() {
                    crate::providers::set_daily_budget_usd(None);
                } else if let Some(v) = lim.as_f64() {
                    crate::providers::set_daily_budget_usd(Some(v));
                }
            }
            let full = args.get("full").and_then(|v| v.as_bool()).unwrap_or(false);
            let provider = args.get("provider").and_then(|v| v.as_str()).unwrap_or("");
            let v = if full {
                json!({
                    "estado": crate::quota_guard::estado(if provider.is_empty() { None } else { Some(provider) }),
                    "aviso": crate::quota_guard::aviso_para_agente(),
                    "daily_budget": crate::providers::daily_budget_snapshot(),
                })
            } else {
                crate::quota_guard::snapshot_compacto_de(if provider.is_empty() { None } else { Some(provider) })
            };
            let txt = serde_json::to_string(&v).unwrap_or_default();
            json!({ "content": [{ "type": "text", "text": txt }] })
        }
        "csc_verify" => {
            // `expect_value` exige un valor EXACTO de la feature: el verificador
            // NO presupone cual de las dos variantes del CSC es la legal.
            let exp = s("expect_value");
            let rep = crate::csc_verify::verify_expecting(if exp.is_empty() {
                None
            } else {
                Some(exp.as_str())
            });
            let txt = serde_json::to_string_pretty(&rep).unwrap_or_default();
            json!({ "content": [{ "type": "text", "text": txt }] })
        }
        "daad_harvester" => {
            let path = s("path");
            let action = {
                let a = s("action");
                if a.is_empty() { "scan".to_string() } else { a }
            };
            if action == "catalog" {
                let cat = if path.is_empty() {
                    crate::daad::DAADCatalog::open_default()
                } else {
                    crate::daad::DAADCatalog::open(&path)
                };
                match cat {
                    Ok(catalog) => {
                        let platform = if s("platform").is_empty() { None } else { Some(s("platform")) };
                        let query = if s("query").is_empty() { None } else { Some(s("query")) };
                        let stats = catalog.get_stats().ok();
                        let games = catalog.query_games(platform.as_deref(), query.as_deref()).unwrap_or_default();
                        let res = json!({
                            "ok": true,
                            "stats": stats,
                            "count": games.len(),
                            "games": games,
                        });
                        let txt = serde_json::to_string_pretty(&res).unwrap_or_default();
                        json!({ "content": [{ "type": "text", "text": txt }] })
                    }
                    Err(e) => {
                        let txt = serde_json::to_string_pretty(&json!({ "ok": false, "error": e })).unwrap_or_default();
                        json!({ "content": [{ "type": "text", "text": txt }], "isError": true })
                    }
                }
            } else if path.is_empty() {
                json!({ "content": [{ "type": "text", "text": "{\"ok\":false,\"error\":\"missing_path\"}" }], "isError": true })
            } else {
                match std::fs::read(&path) {
                    Ok(data) => {
                        let res = match action.as_str() {
                            "embedded" => {
                                if let Some((offset, size, analysis)) = crate::daad::DAADBytecodeParser::find_embedded_ddb(&data) {
                                    json!({ "ok": true, "embedded": true, "offset": offset, "size": size, "analysis": analysis })
                                } else {
                                    json!({ "ok": true, "embedded": false, "message": "no embedded DDB found" })
                                }
                            }
                            "media" => {
                                let media = crate::daad::MediaInspector::inspect_media(&data, &path);
                                json!({ "ok": true, "media": media })
                            }
                            "disasm" => {
                                let dialect = if s("platform") == "v1" { "v1" } else { "v2" };
                                let (condacts, next_offset) = crate::daad::CondActDisassembler::disassemble_stream(&data, 0, dialect);
                                json!({ "ok": true, "count": condacts.len(), "next_offset": next_offset, "condacts": condacts })
                            }
                            "ir" => {
                                match crate::daad_ir::parse_ddb_ir_auto(&data, &path) {
                                    Ok(doc) => json!({ "ok": true, "ir": doc }),
                                    Err(e) => json!({ "ok": false, "error": e }),
                                }
                            }
                            "fingerprint" => {
                                let fp = crate::daad_ir::fingerprint_binary(&data, &path);
                                json!({ "ok": true, "fingerprint": fp })
                            }
                            "load_model" => {
                                let platform = s("platform");
                                let admission = crate::daad_load_models::verify_load_model(&platform, &data);
                                json!({ "ok": true, "admission": admission })
                            }
                            "unpack" => {
                                let members = crate::daad_unpack::unpack_recursive(&data, &path, 3);
                                json!({ "ok": true, "count": members.len(), "members": members })
                            }
                            _ => {
                                let analysis = crate::daad::DAADBytecodeParser::parse_ddb(&data, &path);
                                json!({ "ok": true, "analysis": analysis })
                            }
                        };
                        let txt = serde_json::to_string_pretty(&res).unwrap_or_default();
                        json!({ "content": [{ "type": "text", "text": txt }] })
                    }
                    Err(e) => {
                        let txt = serde_json::to_string_pretty(&json!({ "ok": false, "error": e.to_string() })).unwrap_or_default();
                        json!({ "content": [{ "type": "text", "text": txt }], "isError": true })
                    }
                }
            }
        }
        "coding_agent" => {
            let goal = s("goal");
            if goal.trim().is_empty() {
                json!({ "content": [{ "type": "text", "text": "{\"ok\":false,\"error\":\"missing_goal\"}" }], "isError": true })
            } else {
                let ws_raw = s("workspace_root");
                let workspace_root = if ws_raw.trim().is_empty() {
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
                } else {
                    std::path::PathBuf::from(ws_raw)
                };
                let model = {
                    let m = s("model");
                    if m.trim().is_empty() { None } else { Some(m) }
                };
                let session_id = {
                    let sid = s("session_id");
                    if sid.trim().is_empty() { None } else { Some(sid) }
                };
                let max_turns = args
                    .get("max_turns")
                    .and_then(|v| v.as_u64())
                    .map(|n| (n as usize).clamp(1, 50))
                    .unwrap_or(15);
                let cfg = crate::agent_coding::CodingAgentConfig {
                    workspace_root,
                    max_turns,
                    max_context_chars: 120_000,
                    model,
                    session_id,
                    allow_outside_workspace: b("allow_outside_workspace"),
                };
                let report = crate::agent_coding::run_coding_session_live(&goal, &cfg);
                let txt = serde_json::to_string_pretty(&report).unwrap_or_default();
                json!({ "content": [{ "type": "text", "text": txt }] })
            }
        }
        "go_retool" => {
            let path = s("path");
            if path.trim().is_empty() {
                json!({ "content": [{ "type": "text", "text": "{\"ok\":false,\"error\":\"missing_path\"}" }], "isError": true })
            } else {
                let action = {
                    let a = s("action");
                    if a.is_empty() { "analyze".to_string() } else { a }
                };
                let query = {
                    let q = s("query");
                    if q.trim().is_empty() { None } else { Some(q) }
                };
                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize)
                    .unwrap_or(200);
                let res = crate::go_retool::handle_action(
                    &action,
                    std::path::Path::new(&path),
                    query.as_deref(),
                    limit,
                );
                let txt = serde_json::to_string_pretty(&res).unwrap_or_default();
                json!({ "content": [{ "type": "text", "text": txt }] })
            }
        }
        "es_eid" => {
            let action = {
                let a = s("action");
                if a.is_empty() { "inspect_p12".to_string() } else { a }
            };
            let decode_hex = |h: &str| -> Vec<u8> {
                (0..h.len())
                    .step_by(2)
                    .filter_map(|i| h.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok()))
                    .collect()
            };
            match action.as_str() {
                "inspect_p12" => {
                    let path = s("path");
                    let pass = s("password");
                    match crate::es_eid::load_pkcs12(std::path::Path::new(&path), &pass) {
                        Ok(id) => {
                            let txt = serde_json::to_string_pretty(&id.inspection).unwrap_or_default();
                            json!({ "content": [{ "type": "text", "text": txt }] })
                        }
                        Err(e) => json!({ "content": [{ "type": "text", "text": json!({"ok": false, "error": e}).to_string() }], "isError": true }),
                    }
                }
                "dnie_atr" => {
                    let bytes = decode_hex(&s("atr_hex"));
                    let res = crate::es_eid::inspect_dnie_atr(&bytes);
                    let txt = serde_json::to_string_pretty(&res).unwrap_or_default();
                    json!({ "content": [{ "type": "text", "text": txt }] })
                }
                "dnie_apdu" => {
                    let pin = s("pin");
                    let digest_bytes = decode_hex(&s("digest_hex"));
                    if digest_bytes.len() != 32 {
                        json!({ "content": [{ "type": "text", "text": "{\"ok\":false,\"error\":\"digest_hex must be 32 bytes (64 hex chars)\"}" }], "isError": true })
                    } else {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&digest_bytes);
                        let key_ref = args.get("key_ref").and_then(|v| v.as_u64()).unwrap_or(1) as u8;
                        let frames = crate::es_eid::dnie_sign_apdu_sequence(&pin, key_ref, &arr);
                        let txt = serde_json::to_string_pretty(&frames).unwrap_or_default();
                        json!({ "content": [{ "type": "text", "text": txt }] })
                    }
                }
                other => json!({ "content": [{ "type": "text", "text": format!("{{\"ok\":false,\"error\":\"unknown action {}\"}}", other) }], "isError": true }),
            }
        }
        "ast_analyze" => {
            let path = s("path");
            match crate::code_ast::analyze_file(std::path::Path::new(&path)) {
                Ok(rep) => {
                    let txt = serde_json::to_string_pretty(&rep).unwrap_or_default();
                    json!({ "content": [{ "type": "text", "text": txt }] })
                }
                Err(e) => json!({ "content": [{ "type": "text", "text": json!({"ok": false, "error": e}).to_string() }], "isError": true }),
            }
        }
        "re_disasm" => {
            let action = {
                let a = s("action");
                if a.is_empty() { "disasm".to_string() } else { a }
            };
            if action == "provenance" {
                let path = s("path");
                match crate::re_disasm::inspect_binary_provenance(std::path::Path::new(&path)) {
                    Ok(v) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }] }),
                    Err(e) => json!({ "content": [{ "type": "text", "text": json!({"ok": false, "error": e}).to_string() }], "isError": true }),
                }
            } else {
                let arch = {
                    let a = s("arch");
                    if a.is_empty() { "arm64".to_string() } else { a }
                };
                let hex_str = s("hex").replace(' ', "");
                let bytes: Vec<u8> = (0..hex_str.len())
                    .step_by(2)
                    .filter_map(|i| hex_str.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok()))
                    .collect();
                let base_str = s("base_addr");
                let base = u64::from_str_radix(base_str.trim_start_matches("0x"), 16).unwrap_or(0x400000);
                let max_insns = args.get("max_insns").and_then(|v| v.as_u64()).unwrap_or(64) as usize;
                let res = if arch == "x86_32" || arch == "x86_64" {
                    let bits = if arch == "x86_32" { 32 } else { 64 };
                    Ok(crate::re_disasm::disasm_x86_iced(&bytes, bits, base, max_insns))
                } else {
                    crate::re_disasm::disasm_capstone(&bytes, &arch, base, max_insns)
                };
                match res {
                    Ok(rep) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&rep).unwrap_or_default() }] }),
                    Err(e) => json!({ "content": [{ "type": "text", "text": json!({"ok": false, "error": e}).to_string() }], "isError": true }),
                }
            }
        }
        "job_queue" => {
            let db = std::path::Path::new(&crate::cockpit_server::state_dir()).join("megadaemon_jobs.db");
            match crate::job_queue::JobQueue::open(&db) {
                Ok(q) => {
                    let action = {
                        let a = s("action");
                        if a.is_empty() { "list".to_string() } else { a }
                    };
                    let out: Result<Value, String> = match action.as_str() {
                        "enqueue" => {
                            let kind = s("kind");
                            let payload = args.get("payload").cloned().unwrap_or_else(|| json!({}));
                            let key = {
                                let k = s("idempotency_key");
                                if k.is_empty() { None } else { Some(k) }
                            };
                            q.enqueue(&kind, &payload, key.as_deref(), 3)
                                .map(|r| serde_json::to_value(r).unwrap_or(Value::Null))
                        }
                        "claim" => q.claim_next(60).map(|r| serde_json::to_value(r).unwrap_or(Value::Null)),
                        "complete" => {
                            let id = args.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                            let res_v = args.get("payload").cloned().unwrap_or_else(|| json!({}));
                            q.complete(id, &res_v).map(|_| json!({"ok": true, "id": id}))
                        }
                        "fail" => {
                            let id = args.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                            let err = s("error");
                            q.fail_or_retry(id, &err).map(|st| json!({"ok": true, "id": id, "status": st}))
                        }
                        "cancel" => {
                            let id = args.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                            q.cancel(id).map(|c| json!({"ok": true, "cancelled": c}))
                        }
                        _ => {
                            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
                            q.list(limit).map(|v| serde_json::to_value(v).unwrap_or(Value::Null))
                        }
                    };
                    match out {
                        Ok(v) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }] }),
                        Err(e) => json!({ "content": [{ "type": "text", "text": json!({"ok": false, "error": e}).to_string() }], "isError": true }),
                    }
                }
                Err(e) => json!({ "content": [{ "type": "text", "text": json!({"ok": false, "error": e}).to_string() }], "isError": true }),
            }
        }
        "rhai_eval" => {
            let script = s("script");
            let max_ops = args.get("max_ops").and_then(|v| v.as_u64()).unwrap_or(50_000);
            let rep = crate::script_sandbox::eval_sandboxed(&script, max_ops);
            json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&rep).unwrap_or_default() }] })
        }
        "term_snapshot" => {
            let raw = s("raw");
            let rows = args.get("rows").and_then(|v| v.as_u64()).unwrap_or(24) as u16;
            let cols = args.get("cols").and_then(|v| v.as_u64()).unwrap_or(80) as u16;
            let snap = crate::term_state::render_vt100_snapshot(raw.as_bytes(), rows, cols);
            json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&snap).unwrap_or_default() }] })
        }
        "tool_audit" => {
            let db = std::path::Path::new(&crate::cockpit_server::state_dir()).join("megadaemon_audit.db");
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(30) as usize;
            match crate::tool_policy::AuditJournal::open(&db).and_then(|j| j.recent(limit)) {
                Ok(rows) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&rows).unwrap_or_default() }] }),
                Err(e) => json!({ "content": [{ "type": "text", "text": json!({"ok": false, "error": e}).to_string() }], "isError": true }),
            }
        }
        "drive_vfs" => {
            let st = crate::cockpit_server::state_dir();
            let action = {
                let a = s("action");
                if a.is_empty() { "ls".to_string() } else { a }
            };
            let vpath = {
                let p = s("path");
                if p.is_empty() { "drive:/".to_string() } else { p }
            };
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
            let res: Result<Value, String> = match action.as_str() {
                "ls" => crate::drive_vfs::ls(&vpath, Some(&st), limit)
                    .map(|v| serde_json::to_value(v).unwrap_or(Value::Null)),
                "stat" => crate::drive_vfs::stat_path(&vpath, Some(&st))
                    .map(|v| serde_json::to_value(v).unwrap_or(Value::Null)),
                "find" => crate::drive_vfs::find_by_name(&s("query"), Some(&st), limit)
                    .map(|v| serde_json::to_value(v).unwrap_or(Value::Null)),
                "mkdir" => crate::drive_vfs::mkdir_p(&vpath, Some(&st))
                    .map(|id| json!({"ok": true, "folder_id": id})),
                "read" => {
                    let max_b = args.get("max_bytes").and_then(|v| v.as_u64()).unwrap_or(65536) as usize;
                    crate::drive_vfs::read_file_bytes(&vpath, max_b, Some(&st))
                        .map(|b| json!({"ok": true, "bytes": b.len(), "text": String::from_utf8_lossy(&b)}))
                }
                "write" => {
                    let lp = s("local_path");
                    let rn = s("remote_name");
                    crate::drive_vfs::write_local_file(std::path::Path::new(&lp), &vpath, &rn, Some(&st))
                }
                other => Err(format!("unknown drive_vfs action '{}'", other)),
            };
            match res {
                Ok(v) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }] }),
                Err(e) => json!({ "content": [{ "type": "text", "text": json!({"ok": false, "error": e}).to_string() }], "isError": true }),
            }
        }
        "retro_apk_tools" => {
            let action = {
                let a = s("action");
                if a.is_empty() { "axml".to_string() } else { a }
            };
            let path = s("path");
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let out: Result<Value, String> = match action.as_str() {
                        "axml" => crate::axml_parser::parse_axml(&bytes)
                            .map(|r| serde_json::to_value(r).unwrap_or(Value::Null)),
                        "font_1bpp" => {
                            let max_c = args.get("max_chars").and_then(|v| v.as_u64()).unwrap_or(64) as usize;
                            Ok(crate::retro_gfx::extract_1bpp_glyphs(&bytes, max_c))
                        }
                        "saturn_palette" => {
                            let be = s("endian") != "le";
                            Ok(crate::retro_gfx::inspect_saturn_palette(&bytes, be))
                        }
                        "scummvm_save" => Ok(crate::scummvm_save::parse_scummvm_savegame(&bytes)),
                        "dex_origins" => Ok(crate::dex_origins::scan_dex_origins(&bytes)),
                        other => Err(format!("unknown retro_apk_tools action '{}'", other)),
                    };
                    match out {
                        Ok(v) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }] }),
                        Err(e) => json!({ "content": [{ "type": "text", "text": json!({"ok": false, "error": e}).to_string() }], "isError": true }),
                    }
                }
                Err(e) => json!({ "content": [{ "type": "text", "text": json!({"ok": false, "error": e.to_string()}).to_string() }], "isError": true }),
            }
        }
        "clipboard" => {
            let dir = crate::cockpit_server::state_dir();
            let accion = {
                let a = s("action");
                if a.is_empty() { "list".to_string() } else { a }
            };
            let cuerpo = json!({
                "action": accion,
                "text": s("text"),
                "query": s("query"),
                "id": s("id"),
                "origin": s("origin"),
                "limit": n("limit"),
            });
            let r = crate::clipboard::handle_action(&accion, &cuerpo, &dir);
            let txt = serde_json::to_string_pretty(&r).unwrap_or_default();
            json!({ "content": [{ "type": "text", "text": txt }] })
        }
        "capabilities" => match s("action").as_str() {
            "manifest" => crate::capability::manifest(),
            "summary" => crate::capability::summary_by_kind(),
            _ => crate::capability::verify(),
        },
        "task_queue" => match s("action").as_str() {
            "submit" => {
                let kind = s("kind");
                if kind.is_empty() {
                    json!({ "ok": false, "error": "missing_kind" })
                } else if !crate::agent_core::is_known_kind(&kind) {
                    json!({ "ok": false, "error": "unknown_kind", "hint": kind })
                } else {
                    let payload = args.get("payload").cloned().unwrap_or(json!({}));
                    let id = crate::task_scheduler::submit(
                        &kind,
                        payload,
                        n("priority") as u8,
                        vec![],
                        crate::task_scheduler::Resources::default(),
                    );
                    json!({ "ok": true, "id": id, "kind": kind })
                }
            }
            "run" => {
                let max = if n("max") == 0 { 5 } else { n("max") as usize };
                json!({ "ok": true, "executed": crate::task_scheduler::run_due(max) })
            }
            "stats" => crate::task_scheduler::stats(),
            "cancel" => json!({ "ok": crate::task_scheduler::cancel(&s("task_id")) }),
            "retry" => json!({ "ok": crate::task_scheduler::retry(&s("task_id")) }),
            "export" => crate::task_scheduler::export_tasks(&s("node")),
            "journal" => crate::task_scheduler::journal_json(if n("limit") == 0 { 50 } else { n("limit") as usize }),
            _ => crate::task_scheduler::list_json(),
        },
        "alerts" => match s("action").as_str() {
            "check" => crate::alerts::check_rules(),
            "ack" => json!({ "ok": crate::alerts::acknowledge(&s("id")) }),
            "status" => crate::alerts::status(),
            "prune" => json!({ "ok": true, "removed": crate::alerts::prune() }),
            _ => {
                let sev = s("severity");
                let min = if sev.is_empty() { None } else { Some(crate::alerts::Severity::parse(&sev)) };
                crate::alerts::list(min, b("include_ack"), if n("limit") == 0 { 50 } else { n("limit") as usize })
            }
        },
        "agent" => match s("action").as_str() {
            "plan" => {
                let p = crate::agent_core::plan(&s("goal"), b("prefer_llm"));
                json!({
                    "ok": true, "source": p.source, "notes": p.notes,
                    "tasks": p.tasks.iter().map(|t| json!({
                        "kind": t.kind, "payload": t.payload, "priority": t.priority
                    })).collect::<Vec<Value>>()
                })
            }
            "run" => crate::agent_core::run(
                &s("goal"),
                b("prefer_llm"),
                if n("max_tasks") == 0 { 5 } else { n("max_tasks") as usize },
            ),
            _ => crate::agent_core::status(),
        },
        "apk_inventory" => match s("action").as_str() {
            "analyze" => crate::apk_inventory::analyze_apk(&s("path")),
            "dump" => crate::apk_inventory::dump_all(&s("dest_dir")),
            "drift" => crate::apk_inventory::drift(args.get("known").unwrap_or(&Value::Null)),
            _ => crate::apk_inventory::inventory(if n("limit") == 0 { 50 } else { n("limit") as usize }),
        },
        // v14: decompilador DEX.  El agente externo de Android lo usa para
        // analizar APKs crudas sin invertir en Ghidra-DEX ni en smali.
        "dex_decompile" => match s("action").as_str() {
            "strings" => {
                let dex = match crate::dex_decompile::load_dex(&s("path")) {
                    Ok(d) => d,
                    Err(e) => return Some(json!({ "ok": false, "error": e })),
                };
                let v = crate::dex_decompile::dex_strings(&dex);
                json!({
                    "ok": true,
                    "count": v.len(),
                    "strings": v,
                })
            }
            "obfuscation" => {
                let dex = match crate::dex_decompile::load_dex(&s("path")) {
                    Ok(d) => d,
                    Err(e) => return Some(json!({ "ok": false, "error": e })),
                };
                serde_json::to_value(crate::dex_decompile::analyze_obfuscation(&dex))
                    .unwrap_or_else(|e| json!({ "ok": false, "error": e.to_string() }))
            }
            "decompile" => {
                let dex = match crate::dex_decompile::load_dex(&s("path")) {
                    Ok(d) => d,
                    Err(e) => return Some(json!({ "ok": false, "error": e })),
                };
                let java = crate::dex_decompile::decompile_all(&dex);
                json!({
                    "ok": true,
                    "chars": java.len(),
                    "java": java,
                })
            }
            _ => json!({
                "ok": false,
                "error": "unknown_action",
                "hint": "acciones: decompile | strings | obfuscation",
                "path": "ruta ABSOLUTA de un .dex o de un APK (se extrae classes*.dex)",
            }),
        },
        // v14: intel de apps (rutas de datos, endpoints, plan de export/import)
        // y auditoría de ocultación de root.
        "app_intel" => match s("action").as_str() {
            "profiles" => json!({
                "ok": true,
                "profiles": crate::app_intel::known_profiles(),
            }),
            "profile" => match crate::app_intel::profile_by_id(&s("id")) {
                Some(p) => json!({ "ok": true, "profile": p }),
                None => json!({ "ok": false, "error": "unknown_profile", "id": s("id") }),
            },
            "scan" => {
                let scan = crate::app_intel::scan_local_storage(&s("root"));
                json!({
                    "ok": true,
                    "files": scan.files,
                    "summary": scan.summary,
                })
            }
            "exchange" => {
                let Some(p) = crate::app_intel::profile_by_id(&s("id")) else {
                    return Some(json!({ "ok": false, "error": "unknown_profile", "id": s("id") }));
                };
                let plan = crate::app_intel::data_exchange_plan(&p, &s("root"));
                json!({
                    "ok": true,
                    "items": plan.items,
                    "total_bytes": plan.total_bytes,
                    "warnings": plan.warnings,
                })
            }
            "notes" => json!({
                "ok": true,
                "notes": crate::app_intel::interop_notes(&s("id")),
            }),
            _ => json!({
                "ok": false,
                "error": "unknown_action",
                "hint": "acciones: profiles | profile | scan | exchange | notes",
            }),
        },
        "root_detectors" => {
            let limit = n("limit").max(1) as usize;
            let scope = s("scope");
            let third = scope != "all";
            crate::root_intel::detectors(limit, third)
        }
        "root_consumers" => crate::root_intel::consumers(),
        "root_hide" => match s("action").as_str() {
            "audit" => {
                let pkgs = sa("packages");
                // MEDIDO: el entorno se sondea en el dispositivo real, no se
                // simula. Antes usaba DeviceEnv::clean() y devolvia
                // "clean (simulado)" con un 100 de score falso.
                let env = crate::root_intel::probe_env(&pkgs);
                let audits = crate::root_hide::audit_all(&pkgs, &env);
                json!({
                    "ok": true,
                    "audits": audits,
                    "env": "medido en el dispositivo",
                    "ficheros_presentes": env.files_present,
                    "props": env.props,
                    "magisk_version": env.magisk_version,
                    "zygisk": env.zygisk,
                    "denylist_apps": env.denylist_apps,
                    "shamiko": env.shamiko,
                    "trickystore": env.trickystore,
                    "play_integrity_fix": env.play_integrity_fix,
                    "hide_my_applist": env.hide_my_applist,
                    "zygisk_next": env.zygisk_next,
                    "procesos_detectados": env.processes.len(),
                })
            }
            "plan" => {
                let pkgs = sa("packages");
                let env = crate::root_intel::probe_env(&pkgs);
                let audits = crate::root_hide::audit_all(&pkgs, &env);
                let remeds: Vec<crate::root_hide::Remediation> = audits
                    .iter()
                    .flat_map(|a| crate::root_hide::plan_remediation(a))
                    .collect();
                json!({ "ok": true, "remediations": remeds })
            }
            "magisk_layout" => json!({
                "ok": true,
                "files": crate::root_hide::magisk_module_layout(),
            }),
            _ => json!({
                "ok": false,
                "error": "unknown_action",
                "hint": "acciones: audit | plan | magisk_layout",
            }),
        },
        "repo_updates" => match s("action").as_str() {
            "repos" => crate::repo_updates::repos(),
            "status" => crate::repo_updates::status(),
            "plan" => crate::repo_updates::plan(None),
            "watch" => crate::repo_updates::watch_once(),
            "download" => crate::repo_updates::download(&s("package"), &s("repo_id"), &s("dest_dir")),
            "install" => {
                if !b("confirm") {
                    json!({ "ok": false, "error": "needs_confirm", "hint": "instalar una app exige confirm=true" })
                } else {
                    crate::repo_updates::apply(&s("package"), &s("apk_path"), true)
                }
            }
            _ => crate::repo_updates::status(),
        },
        "profile" => match s("action").as_str() {
            "briefing" => crate::profile_analysis::briefing(b("markdown")),
            "drift" => crate::profile_analysis::topic_drift(if n("window_days") == 0 { 30 } else { n("window_days") as u32 }),
            "digest" => crate::profile_analysis::daily_digest(if n("days") == 0 { 14 } else { n("days") as u32 }),
            _ => crate::profile_analysis::analyze(),
        },
        "messaging" => match s("action").as_str() {
            "telegram" => crate::messaging::telegram(if n("limit") == 0 { 500 } else { n("limit") as usize }),
            "sms" => crate::messaging::sms(if n("limit") == 0 { 500 } else { n("limit") as usize }),
            "whatsapp" => crate::messaging::whatsapp(if n("limit") == 0 { 500 } else { n("limit") as usize }),
            "medical" => crate::messaging::medical(if n("limit") == 0 { 200 } else { n("limit") as usize }),
            "ingest" => crate::messaging::ingest_all(),
            "candidates" => crate::messaging::alert_candidates(),
            _ => crate::messaging::inventory(),
        },
        "bookmarks" => match s("action").as_str() {
            "export" => crate::bookmarks::export_all(&s("dest_path")),
            "passkeys" => crate::bookmarks::passkey_inventory(),
            "manager_export" => crate::bookmarks::password_manager_export(&s("package"), &s("dest_path")),
            _ => crate::bookmarks::inventory(),
        },
        "battery_saver" => match s("action").as_str() {
            "apply" => crate::battery_saver::apply(b("confirm"), b("aggressive"), Some(sa("protected"))),
            "protect" => crate::battery_saver::set_protected(sa("protected")),
            "xbs" => crate::battery_saver::xbs_config(),
            _ => crate::battery_saver::status(),
        },
        "git_ops" => match s("action").as_str() {
            "identity" => crate::git_ops::identity(),
            "diff" => crate::git_ops::diff(if n("limit") == 0 { 65536 } else { n("limit") as usize }),
            "commit_push" => {
                crate::git_ops::commit_push(&s("message"), Some(sa("paths")), b("confirm"), b("push"))
            }
            "log" => crate::git_ops::log(if n("limit") == 0 { 10 } else { n("limit") as usize }),
            _ => crate::git_ops::status(),
        },
        "identity_and_web" => match s("action").as_str() {
            "scrape" => {
                let ident = s("identity");
                let ident = if ident.is_empty() { "browser.mobile".to_string() } else { ident };
                crate::service_identity::scrape(
                    &s("url"),
                    &ident,
                    b("respect_robots"),
                    if n("max_bytes") == 0 { 512 * 1024 } else { n("max_bytes") as usize },
                )
            }
            _ => crate::service_identity::list(),
        },
        "root_stealth" => match s("action").as_str() {
            "preview" => crate::root_stealth::preview(&s("scope")),
            "apply" => crate::root_stealth::apply(&s("scope"), b("confirm")),
            _ => crate::root_stealth::status(),
        },
        "ssh_remote" => match s("action").as_str() {
            "install_host_key" => crate::sshd::install_host_key(),
            "authorized_keys" => crate::sshd::authorized_keys_status(),
            "dispatch" => json!({
                "ok": true,
                "output": crate::sshd::dispatch(&s("command")),
            }),
            _ => crate::sshd::status(),
        },
        "env_introspect" => crate::env_introspect::ask(&s("question"), args),
        "api_keys" => {
            let dir = crate::cockpit_server::state_dir();
            match s("action").as_str() {
                "mint" => {
                    let label = s("label");
                    match crate::api_key::generate_extra_key(&dir, &label) {
                        Ok(key) => json!({
                            "ok": true,
                            "action": "mint",
                            "label": label,
                            "api_key": key,
                            "note": "esta clave es unica para el agente y se revoca sin tocar la principal; no la persistas en ningun estado compartido",
                        }),
                        Err(e) => json!({ "ok": false, "error": e }),
                    }
                }
                "revoke" => {
                    let target = s("target");
                    if target.is_empty() {
                        json!({ "ok": false, "error": "missing_target" })
                    } else if !b("confirm") {
                        json!({
                            "ok": false,
                            "error": "confirmation_required",
                            "hint": "revocar deja al agente sin acceso: repite con confirm=true",
                        })
                    } else {
                        match crate::api_key::revoke_extra_key(&dir, &target) {
                            Ok(true) => json!({ "ok": true, "action": "revoke", "revoked": true }),
                            Ok(false) => json!({
                                "ok": false,
                                "error": "not_found",
                                "hint": "ninguna clave del registro coincide con ese target",
                            }),
                            Err(e) => json!({ "ok": false, "error": e }),
                        }
                    }
                }
                _ => {
                    let reveal = b("reveal");
                    let rows = crate::api_key::list_extra_keys(&dir, reveal);
                    json!({
                        "ok": true,
                        "action": "list",
                        "revealed": reveal,
                        "count": rows.len(),
                        "keys": rows
                            .into_iter()
                            .map(|(label, key)| json!({ "label": label, "key": key }))
                            .collect::<Vec<_>>(),
                    })
                }
            }
        }
        // v15 (W6): bus de servicios — catálogo unificado + selección.
        "meta_route" => {
            // `s()` ya devuelve &str (no Option) -> no hay que desempaquetar.
            let request = s("request").to_string();
            crate::meta_agent::handle(&request, b("execute"))
        }
        "service_bus" => {
            let dir = crate::mesh::default_state_dir();
            let snapshot = match crate::mesh::Mesh::open(&dir) {
                Ok(m) => m.snapshot(),
                Err(e) => json!({ "peers": [], "error": e.to_string() }),
            };
            let mut bus = crate::service_bus::bus_from_runtime(
                &crate::providers::list_providers_redacted(),
                crate::providers::PROVIDERS_PATH,
                &snapshot,
                300,
                "http://127.0.0.1:9090/mcp",
            );
            match s("action").as_str() {
                "probe" => {
                    let prober = crate::service_bus::TcpProber::default();
                    let id = s("id");
                    if id.is_empty() {
                        let res = bus.probe_all(&prober);
                        json!({ "ok": true, "probed": res.len(), "health": res.into_iter().map(|(id, h)| json!({"id": id, "available": h.available, "reason": h.reason})).collect::<Vec<_>>() })
                    } else {
                        match bus.probe_one(&id, &prober) {
                            Ok(h) => json!({ "ok": true, "id": id, "available": h.available, "reason": h.reason, "detail": h.detail, "checked_at": h.checked_at }),
                            Err(e) => json!({ "ok": false, "error": e }),
                        }
                    }
                }
                "select" => {
                    let task = crate::service_bus::TaskType::parse(&s("task"))
                        .unwrap_or(crate::service_bus::TaskType::Inference);
                    let sel = bus.select(task);
                    json!({
                        "ok": true,
                        "task": task.as_str(),
                        "chosen": sel.chosen,
                        "chain": sel.chain,
                        "reason": sel.reason,
                        "estimated_cost_usd": sel.estimated_cost_usd,
                        "rejected": sel.rejected.into_iter().map(|(id, why)| json!({"id": id, "why": why})).collect::<Vec<_>>(),
                    })
                }
                // Puente MCP de Antigravity: genera la config y, con
                // confirm=true, la **fusiona** en el fichero real (merge +
                // backup + rename atómico, idempotente).  Conserva las claves
                // raíz que ya había (`providers`) y las cabeceras reales.
                "bridge" => {
                    let path = if s("path").is_empty() {
                        "/data/user/0/com.agy/files/.gemini/config/mcp_config.json".to_string()
                    } else {
                        s("path")
                    };
                    let et_owned = sa("enabled_tools");
                    let enabled_tools: Vec<&str> =
                        et_owned.iter().map(|s| s.as_str()).collect();
                    let bridge = crate::service_bus::antigravity_bridge_config(
                        "http://127.0.0.1:9090/mcp",
                        "md-secrets/instance.env",
                        &enabled_tools,
                    );
                    if !b("confirm") {
                        return Some(json!({
                            "ok": true,
                            "dry_run": true,
                            "path": path,
                            "server": crate::service_bus::ANTIGRAVITY_SERVER_NAME,
                            "config": bridge,
                            "hint": "repite con confirm=true para fusionarlo en el fichero (merge + backup + rename)",
                        }));
                    }
                    match crate::service_bus::apply_bridge_file(&path, &bridge) {
                        Ok(v) => v,
                        Err(e) => json!({ "ok": false, "error": e }),
                    }
                }
                _ => bus.catalog_json(),
            }
        }
        // v15 (W6): conector de GitHub (repos, issues/PRs, publicar estado).
        // v15 (W6): delegación parasitaria (breaker por cuenta, presupuesto
        // por hora y por modelo, coste medido, anuncio al bus sin credenciales).
        "delegate" => {
            let redacted = crate::providers::list_providers_redacted();
            let accounts = crate::delegate::accounts_from_redacted(
                &redacted,
                crate::providers::PROVIDERS_PATH,
            );
            let mut engine = crate::delegate::DelegateEngine::new(accounts);
            match s("action").as_str() {
                "accounts" => json!({ "ok": true, "accounts": engine.account_statuses() }),
                "status" => {
                    let now = crate::service_bus::now_epoch();
                    json!({
                        "ok": true,
                        "budget": engine.budget_status(now),
                        "breakers": engine.breaker_states().into_iter().map(|(id, state, fails)| json!({"id": id, "state": state, "failures": fails})).collect::<Vec<_>>(),
                        "measured_calls": engine.measured_calls().len(),
                    })
                }
                "announce" => {
                    let dir = crate::mesh::default_state_dir();
                    let snapshot = match crate::mesh::Mesh::open(&dir) {
                        Ok(m) => m.snapshot(),
                        Err(e) => json!({ "peers": [], "error": e.to_string() }),
                    };
                    let mut bus = crate::service_bus::bus_from_runtime(
                        &redacted,
                        crate::providers::PROVIDERS_PATH,
                        &snapshot,
                        300,
                        "http://127.0.0.1:9090/mcp",
                    );
                    let n = bus.announce_delegate(&engine);
                    json!({ "ok": true, "announced": n, "catalog": bus.catalog_json() })
                }
                "execute" => {
                    let prompt = s("prompt");
                    if prompt.trim().is_empty() {
                        json!({ "ok": false, "error": "falta el prompt" })
                    } else {
                        let mut runner = |model: &str, p: &str, max: u32| {
                            let r = crate::providers::execute_chat(p, Some(model), None, None, max);
                            crate::delegate::ProviderReply {
                                reply: r.reply,
                                model_id: r.model_id,
                                tokens_used: r.tokens_used,
                                latency_ms: r.latency_ms,
                                fallback_used: r.fallback_used,
                            }
                        };
                        let mut task = crate::delegate::DelegationTask {
                            id: String::new(),
                            prompt: prompt.clone(),
                            kind: s("kind"),
                            required_capabilities: Vec::new(),
                            max_tokens: 1024,
                            deadline: None,
                            status: "queued".to_string(),
                            attempts: Vec::new(),
                            assigned_account: None,
                        };
                        if task.kind.trim().is_empty() {
                            task.kind = "research".to_string();
                        }
                        let task_id = engine.submit(task);
                        let cuenta = if !s("account").is_empty() {
                            s("account")
                        } else {
                            engine
                                .task(&task_id)
                                .and_then(|t| t.assigned_account.clone())
                                .unwrap_or_default()
                        };
                        if cuenta.is_empty() {
                            json!({ "ok": false, "error": "no hay cuenta que pueda atender la tarea" })
                        } else {
                            match engine.execute_measured_with(&task_id, &cuenta, &prompt, &mut runner) {
                                Ok(m) => json!({
                                    "ok": true,
                                    "account": m.account_id,
                                    "model": m.model,
                                    "served_by": m.served_by,
                                    "llamada_ok": m.ok,
                                    "tokens": m.tokens_total,
                                    "tokens_in": m.tokens_in,
                                    "tokens_out": m.tokens_out,
                                    "latency_ms": m.latency_ms,
                                    "coste_usd": m.cost.usd,
                                    "coste_fuente": m.cost.source,
                                    "coste_nota": m.cost.note,
                                    "error": m.error,
                                }),
                                Err(e) => json!({ "ok": false, "error": e }),
                            }
                        }
                    }
                }
                _ => json!({ "ok": false, "error": "acción desconocida" }),
            }
        }
        "github" => {
            let cfg = crate::github_connector::GitHubConfig::with_token_ref(
                crate::github_connector::DEFAULT_TOKEN_PATH,
            );
            let client = crate::github_connector::UreqClient::new(cfg.token_ref.as_str());
            let repo = if s("repo").is_empty() {
                cfg.default_repo.clone()
            } else {
                s("repo")
            };
            match s("action").as_str() {
                "repos" => match crate::github_connector::list_repos(&cfg, 1, 50, &client) {
                    Ok(repos) => json!({ "ok": true, "count": repos.len(), "repos": repos }),
                    Err(e) => json!({ "ok": false, "error": e }),
                },
                "issues" | "prs" => {
                    let only_prs = s("action") == "prs";
                    match crate::github_connector::list_issues(&cfg, &repo, &s("state"), only_prs, &client) {
                        Ok(items) => json!({ "ok": true, "repo": repo, "count": items.len(), "items": items }),
                        Err(e) => json!({ "ok": false, "error": e }),
                    }
                }
                "publish" => {
                    let dir = crate::mesh::default_state_dir();
                    let snapshot = match crate::mesh::Mesh::open(&dir) {
                        Ok(m) => m.snapshot(),
                        Err(e) => json!({ "peers": [], "error": e.to_string() }),
                    };
                    let title = if s("title").is_empty() {
                        format!("Estado del enjambre · {}", crate::mesh::now_iso())
                    } else {
                        s("title")
                    };
                    let kind = if s("head").is_empty() {
                        crate::github_connector::PublishKind::Issue
                    } else {
                        crate::github_connector::PublishKind::PullRequest {
                            head: s("head"),
                            base: if s("base").is_empty() { "main".into() } else { s("base") },
                        }
                    };
                    match crate::github_connector::publish_swarm_state(
                        &cfg,
                        &repo,
                        &snapshot,
                        &kind,
                        &title,
                        b("confirm"),
                        &client,
                    ) {
                        Ok(v) => v,
                        Err(e) => json!({ "ok": false, "error": e }),
                    }
                }
                "bridge" => json!({
                    "ok": true,
                    "config": crate::service_bus::antigravity_bridge_config(
                        "http://127.0.0.1:9090/mcp",
                        "md-secrets/instance.env",
                        &["minimax_chat", "swarm_chat"],
                    ),
                    "note": "la clave se inyecta al escribir la config; no se publica en el estado compartido",
                }),
                _ => json!({ "ok": false, "error": "unknown_action", "hint": "repos|issues|prs|publish|bridge" }),
            }
        }
        // v15: gestor de tiendas/repos universal ("mega store manager").
        // Sin SDK de terceros ni binarios externos: todo en Rust, con
        // sha256 por entrada y firma RSA opcional del índice.
        "store_manager" => {
            let action = s("action");
            let dir = std::path::PathBuf::from(crate::mesh::default_state_dir());
            crate::store_manager::handle_action(&action, args, &dir)
        }
        // v16 (asimilación de Antigravity): las tools de los módulos de
        // asimilación se enrutan AQUÍ también, para que `agent_coding`
        // (que usa `megadaemon_tool` -> `mcp_ext::dispatch`) llegue a ellas
        // con la misma paridad que el MCP y la CLI.
        "antigravity_model" => {
            return crate::antigravity_model::dispatch(name, args);
        }
        "antigravity_env" => {
            return crate::antigravity_env::dispatch(name, args);
        }
        "antigravity_endpoint" => {
            return crate::antigravity_endpoint::dispatch(name, args);
        }
        // Reconstruccion multi-motor: Ghidra + Rizin + DiE + motor propio
        // + pclntab.  Compara los metodos y fusiona lo mejor de cada uno.
        // Watchdog (supervisor) de Android: el daemon se instala a si mismo
        // en /data/adb/service.d para Magisk, de modo que la
        // auto-actualizacion (el daemon sale con codigo 10) entra en servicio.
        "supervisor" => {
            let accion = args.get("accion").and_then(|v| v.as_str()).unwrap_or("estado");
            match accion {
                "instalar" => crate::supervisor::instalar(),
                "desinstalar" => crate::supervisor::desinstalar(),
                "estado" => crate::supervisor::estado(),
                "watchdog" => json!({
                    "ok": true,
                    "script": crate::supervisor::WATCHDOG_SH,
                    "ruta_destino": crate::supervisor::RUTA_SERVICE_D,
                }),
                _ => json!({"ok": false, "error": "accion desconocida: usa estado|instalar|desinstalar|watchdog"}),
            }
        }
        // Hooks de Frida para observar Antigravity (SQLite, red, ficheros).
        "frida_hooks" => {
            let accion = args.get("accion").and_then(|v| v.as_str()).unwrap_or("listar");
            match accion {
                "listar" => {
                    let hooks: Vec<Value> = crate::frida_hooks::hooks_disponibles()
                        .into_iter()
                        .map(|(n, s)| json!({"nombre": n, "bytes": s.len()}))
                        .collect();
                    json!({"ok": true, "hooks": hooks})
                }
                "obtener" => {
                    let n = args.get("nombre").and_then(|v| v.as_str()).unwrap_or("");
                    match crate::frida_hooks::hook_por_nombre(n) {
                        Some(s) => json!({"ok": true, "nombre": n, "script": s}),
                        None => json!({"ok": false, "error": format!("hook desconocido: {n}")}),
                    }
                }
                _ => json!({"ok": false, "error": "accion desconocida: usa listar|obtener"}),
            }
        }
        "re_fusion" => {
            let accion = args.get("accion").and_then(|v| v.as_str()).unwrap_or("compare");
            let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let ruta = s("binario");
            if ruta.is_empty() {
                return Some(json!({"ok": false, "error": "falta 'binario'"}));
            }
            let opt = |k: &str| -> Option<std::path::PathBuf> {
                let v = s(k);
                if v.trim().is_empty() {
                    None
                } else {
                    Some(std::path::PathBuf::from(v))
                }
            };
            let o = crate::re_fusion::OpcionesFusion {
                binario: std::path::PathBuf::from(&ruta),
                simbolos_tsv: opt("simbolos"),
                anchors_tsv: opt("anclas"),
                ghidra_dir: opt("ghidra_dir"),
                java_home: opt("java_home"),
                project_dir: opt("project_dir")
                    .unwrap_or_else(|| std::path::PathBuf::from("/workspace/agy-re/tools-out/proj")),
                project_name: if s("project").is_empty() { "Go".to_string() } else { s("project") },
                out_dir: opt("out")
                    .unwrap_or_else(|| std::path::PathBuf::from("/workspace/agy-re/tools-out/fusion")),
                max_memory: if s("max_memoria").is_empty() { "1400M".to_string() } else { s("max_memoria") },
                timeout: std::time::Duration::from_secs(s("timeout_s").parse().unwrap_or(1800)),
                hacer_motor: true,
                hacer_die: s("sin_die").is_empty(),
                hacer_rizin: s("sin_rizin").is_empty(),
                // Ghidra es el lento (JVM): solo si lo piden explicitamente.
                hacer_ghidra: !s("sin_ghidra").is_empty(),
            };
            let r = crate::re_fusion::ejecutar(&o);
            match accion {
                "informe" => json!({
                    "ok": true,
                    "comparativa": r.get("comparativa").cloned().unwrap_or_default(),
                }),
                _ => r,
            }
        }
        _ => return None,
    };
    let _ = i("unused");
    Some(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todas_las_herramientas_nuevas_tienen_esquema() {
        for d in tool_definitions().as_array().unwrap() {
            assert!(d["name"].as_str().is_some());
            assert!(d["description"].as_str().is_some());
            assert_eq!(d["inputSchema"]["type"], json!("object"));
        }
    }

    #[test]
    fn los_parametros_de_accion_son_enums() {
        for d in tool_definitions().as_array().unwrap() {
            if let Some(props) = d["inputSchema"]["properties"].as_object() {
                if let Some(a) = props.get("action") {
                    assert!(
                        a.get("enum").is_some(),
                        "{}: action debe ser enum (texto libre = inyección de comandos)",
                        d["name"]
                    );
                }
            }
        }
    }

    #[test]
    fn un_nombre_desconocido_no_lo_gestiona_este_modulo() {
        assert!(dispatch("no_existe", &json!({})).is_none());
    }

    #[test]
    fn las_acciones_destructivas_exigen_confirmacion() {
        // root_stealth.apply sin confirm debe fallar.
        let r = dispatch("root_stealth", &json!({ "action": "apply", "scope": "all" })).unwrap();
        assert_eq!(r["ok"], json!(false));
        // battery_saver.apply igual.
        let r2 = dispatch("battery_saver", &json!({ "action": "apply" })).unwrap();
        assert_eq!(r2["ok"], json!(false));
        // git commit_push igual.
        let r3 = dispatch("git_ops", &json!({ "action": "commit_push", "message": "x" })).unwrap();
        assert_eq!(r3["ok"], json!(false));
    }
}
