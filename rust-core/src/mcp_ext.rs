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
use crate::csc_verify;
use crate::npu_vs4l;

/// Definiciones de las herramientas nuevas.
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "drive_backup",
            "description": "Google Drive desde el propio MegaDaemon: sube un fichero o un directorio ENTERO (se comprime en ZIP) con subida resumible en trozos de 256 KiB, SHA-256 verificado en local y credenciales OAuth leidas del fichero 0600. Sin curl, sin dd, sin scripts: todo en Rust. Acciones: upload | download | status. Subir publica el contenido en un remoto: exige confirm=true.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["upload", "download", "status"], "default": "upload" },
                    "path": { "type": "string", "description": "Ruta local a subir (fichero o directorio)" },
                    "name": { "type": "string", "description": "Nombre en Drive (opcional; por defecto, el del fichero)" },
                    "folder_id": { "type": "string", "description": "Carpeta destino en Drive (vacio = Mi Drive)" },
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
            "name": "task_queue",
            "description": "Planificador multitarea: lista la cola, encola trabajo o ejecuta el lote pendiente respetando prioridad, dependencias y la política de energía del dispositivo.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "submit", "run", "stats", "cancel", "retry", "export", "journal"] },
                    "kind": { "type": "string", "description": "tipo de tarea (sync.local, apk.inventory, alerts.check, ...)" },
                    "payload": { "type": "object" },
                    "priority": { "type": "integer", "default": 5 },
                    "task_id": { "type": "string" },
                    "max": { "type": "integer", "default": 5 },
                    "node": { "type": "string", "description": "nodo destino para repartir trabajo (distribuido)" }
                }
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
])
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
