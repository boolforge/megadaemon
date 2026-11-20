# Agente externo de Reverse Android → instancia MAESTRA de MegaDaemon (Samsung)

> **Documento verificado contra la instancia en vivo el 2026-11-19** (26 tools,
> daemon v13.3.0 desplegado). Sustituye a `docs/CONNECT_EXTERNAL_AGENT.md`, que
> prometía tools que el binario desplegado **no expone** (41 vs 26) y usaba
> nombres de variable de entorno equivocados.
>
> **Este documento NO contiene secretos.** Las claves viven fuera del repo, en
> `/workspace/md-secrets/instance.env` (0600, dir 0700).

---

## 0. RESUMEN: qué tienes que darle APARTE de la API key

La API key sola **no basta**. Sin lo que sigue, el agente externo podrá leer el
catálogo de tools pero **no podrá escribir ni una sola línea** en la instancia
(le caerán `401`).

| # | Dato | Por qué hace falta | Dónde está |
|---|---|---|---|
| 1 | **API key** (`X-API-Key`) | Control de acceso remoto. Ya la tienes. | `MEGADAEMON_API_KEY` en `md-secrets/instance.env` |
| 2 | **Token de escritura** (`X-Seere-Auth-Token`) | **Obligatorio**: sin él, `fs_write` y `POST /api/chat_upload` devuelven 401. Sólo se necesita en POST/PUT. | `MEGADAEMON_COCKPIT_TOKEN` en el mismo fichero |
| 3 | **URL del MCP** | `POST <base>/mcp`, JSON-RPC 2.0. **No** `/sse`. | `MEGADAEMON_MCP` (túnel Cloudflare) |
| 4 | **Rol** (`worker`, cap. `android-re`) | La maestra (Samsung) decide; el externo ejecuta y publica. | Se lo dices tú en el prompt |
| 5 | **Identificador de agente** | Nombra las rutas y firma el manifiesto, para que los volcados de distintos agentes no choquen. | Tú eliges: p. ej. `josefrancisco-minimax` |
| 6 | **Convención de escritura + manifiesto** | Sin esto el agente no sabe dónde dejar los datos ni cómo se los inventaría la maestra. | §4 de este documento (incluido en el prompt) |
| 7 | **Permiso de lectura del repo** (opcional) | `https://github.com/boolforge/megadaemon` — sólo lectura, para leer `docs/MASTER_PLAN.md` y `docs/COORDINATION.md`. **Nunca** push. | GitHub, rama `main` |
| 8 | **Límite de recursos** | El móvil va JUSTO de RAM (10,2 GB de 10,5 GB). Un bucle agresivo lo tumba. | Incluido en el prompt |
| 9 | **Cliente MCP** (`tools/mdmcp.py`) | Listo para usar: `tools` / `call` / `raw`. Ya sabe las cabeceras. | `tools/mdmcp.py` del repo |

### Lo que **NO** debes darle (y por qué)

| No darle | Por qué |
|---|---|
| Clave privada RSA del host ADB | Vive **sólo en el móvil** (`/data/local/tmp/megadaemon/adb_host_key.pem`, 0600). Es lo que autentica `adb connect` contra el `adbd`. Que salga = cualquiera con esa clave entra como root en tu móvil. |
| Token de GitHub | Sólo el principal hace push a `main`. |
| La API key escrita en un fichero del repo | El `.gitignore` ya filtra `*.key`/`*.env`, pero un `git add -f` los cuela. |
| Acceso a `root_exec` sin supervisión | `root_exec` ejecuta como root real. Úsalo con criterio, no como una API de conveniencia. |

---

## 1. EL PROMPT (copiar y pegar tal cual)

```
Tienes acceso a una instancia de MegaDaemon que YA está funcionando y que actúa
como harness de ingeniería inversa para Android. Tu trabajo es USARLA, no
reimplementar nada.

═══════════════════════════════════════════════════════════════════
1. QUÉ ES MEGADAEMON (30 segundos de contexto)
═══════════════════════════════════════════════════════════════════
MegaDaemon es un bench de ingeniería inversa escrito en Rust PURO (sin Python en
los hot paths, sin JVM, sin binarios externos en runtime). Corre sobre Android
ARM64 en un Samsung Galaxy S21 Ultra (Exynos 2100, root + Magisk) y también en
PCs Linux. Es un único binario: `megadaemon-core`.

- Instancia MAESTRA (`primary`) = el móvil Samsung. Decide, reparte y sincroniza.
- Tu rol = `worker` con capacidad `android-re`. Ejecutas y publicas; no coordinas.
- La maestra expone 26 tools vía MCP (JSON-RPC 2.0). Tú no las reimplementas.

═══════════════════════════════════════════════════════════════════
2. CONEXIÓN
═══════════════════════════════════════════════════════════════════
Endpoint MCP (JSON-RPC 2.0 sobre HTTP POST):

    POST https://TUNEL.trycloudflare.com/mcp

Cabeceras obligatorias:
    X-API-Key: <META_LA_API_KEY_AQUI>          # SIEMPRE
    X-Seere-Auth-Token: <META_EL_TOKEN_AQUI>   # en toda escritura (POST/PUT)

Body JSON-RPC 2.0 puro:
    {"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
    {"jsonrpc":"2.0","id":2,"method":"tools/call",
     "params":{"name":"system_overview","arguments":{}}}

VERIFICACIÓN (hazlo ANTES de nada; si esto falla, para y repórtalo):
    curl -s -X POST <MCP> \
      -H "X-API-Key: <KEY>" -H "X-Seere-Auth-Token: <TOKEN>" \
      -H "Content-Type: application/json" \
      -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'

Debe devolver HTTP 200 con la lista de tools. Si devuelve 401 → falta la API
key o el token. Si devuelve 429 → estás disparando demasiado (ver §4).

CLIENTE LISTO (recomendado): `tools/mdmcp.py` del repo
`https://github.com/boolforge/megadaemon`. Cópialo y úsalo así:

    export MEGADAEMON_MCP="https://TUNEL.trycloudflare.com/mcp"
    export MEGADAEMON_API_KEY="<KEY>"
    export MEGADAEMON_TOKEN="<TOKEN>"      # OJO: el nombre es MEGADAEMON_TOKEN

    python3 mdmcp.py tools
    python3 mdmcp.py call system_overview '{}'

(Opcionalmente: si el cliente no está disponible, `curl` + JSON a mano.)

RUTAS ALTERNATIVAS si el túnel no te responde:
  * Desde la red local del usuario: http://<IP_DEL_MOVIL>:3000 (cockpit) y el
    daemon real en 127.0.0.1:9090 (exige API key).
  * `adb connect <IP_DEL_MOVIL>:5555` (el `adbd` del Samsung ya acepta la clave
    de host registrada). NO te han dado la clave privada ADB y no la necesitas:
    el cliente ADB en Rust puro ya está en el repo (`rust-core/src/adb_client.rs`).
  * Prioriza SIEMPRE el túnel: es inmune a bloqueos de IP (por eso lo usa la
    flota).

═══════════════════════════════════════════════════════════════════
3. CATÁLOGO REAL DE TOOLS (27, verificado) — empieza por tools/list
═══════════════════════════════════════════════════════════════════
Sistema y dispositivo:
  system_overview, props_and_settings, root_exec, fs_read, fs_write,
  find_duplicates, kernel_and_blocks, screen_and_ui, selinux_control,
  magisk_module, app_control, adb

Apps / binarios (motor RE):
  megadaemon_die_binwalk   (DiE + Binwalk + Scalpel + Bulk Extractor, nativo)
  megadaemon_rizin         (Rizin 0.9.1 multi-OS: funciones, imports, strings)
  megadaemon_cirx_analyze  (LTO-Decomp CIRX: Micro-RTL/SSA, Risk Score)
  megadaemon_mba_simplify  (desofuscación MBA)
  megadaemon_npu_cv        (NPU Exynos 2100 / Mali-G78 / CV nativo)

Conversaciones / datos:
  sync_conversations, sync_web_conversations, harvest_sessions, ingest_export,
  list_conversations, get_conversation, search_conversations,
  conversation_stats

LLM:
  minimax_chat, swarm_chat

IMPORTANTE: este listado es el del binario DESPLEGADO (v13.4.0). El código del
repo tiene módulos más nuevos (tablón, bus de malla, vault de firma) que AÚN
NO están expuestos como tool MCP. **No los busques en tools/list**: si no
están, no los uses. Lo que no exista, lo haces con `root_exec` + tu propia
herramienta local, y reportas el hueco.

═══════════════════════════════════════════════════════════════════
3b. SEGUNDO CANAL: EL CATÁLOGO EXTENDIDO POR HTTP (¡y funciona!)
═══════════════════════════════════════════════════════════════════
Hay más motores que NO salen en `tools/list` porque son extendidos. Se llaman
por HTTP POST, con las mismas dos cabeceras:

    POST <BASE>/api/tool/<nombre>
    X-API-Key: <KEY>
    X-Seere-Auth-Token: <TOKEN>
    Content-Type: application/json
    {"action":"...","path":"...","limit":50}

Verificados EN VIVO contra la instancia:
  · apk_inventory   {"action":"list"}  → inventario real de las apps instaladas:
      package, apk_path, dex_count, fingerprint, native_libs, permisos,
      activities, receivers, services, providers. (action "list" = el
      default; también "analyze" {path}, "dump" {dest_dir}, "drift".)
  · dex_decompile   {"action":"decompile"|"strings"|"obfuscation", "path":<ABS>}
      "path" = ruta ABSOLUTA de un .dex suelto o de un APK (extrae los
      classes*.dex). "strings" = las cadenas del pool; "obfuscation" = el
      mapa de desofuscación; "decompile" = clases, métodos, strings, pool.
      ► ESTA ES LA HERRAMIENTA QUE HACE EL TRABAJO PESADO POR TI: apúntale
        cualquier APK de `incoming/<TU_ID>/` y te extrae el pool de
        constantes. No implementes tú el detector de strings.
  · capabilities    {"action":"verify"|"manifest"|"summary"} → qué funciona de verdad en el móvil
  · task_queue      {"action":"submit"|"run"|"stats"|"cancel"|"retry"|"export"|"journal"}
  · alerts          {"action":"check"|"status"|"ack"|"prune"}
  · agent           {"action":"plan"|"run"}
  (No existen: `mesh` ni `signing_vault` en este binario.)

⇒ USA LOS DOS CANALES: MCP para lo tipado del dispositivo y el catálogo
  extendido por HTTP para el motor de APKs/DEX.

═══════════════════════════════════════════════════════════════════
4. REGLAS DE OPERACIÓN (obligatorias)
═══════════════════════════════════════════════════════════════════
- RATE LIMIT: la instancia limita a 60 peticiones/minuto por IP de origen, y el
  túnel comparte cubo con el propio daemon. Batching y ESPERA entre ráfagas:
  máximo ~40 llamadas/minuto, idealmente 1 llamada/segundo. Si ves
  `429 rate_limited`, PARA 60 s y sigue. No es caída, es límite.
- RECURSOS: el móvil va JUSTO de RAM (10,2 GB usados de 10,5 GB, 3,3 GB de
  swap). Nada de bucles de polling, nada de reintentos agresivos, nada de
  `dump_partition` salvo que se te pida. Prefiere 1 llamada larga a 50 cortas.
- TIMEOUTS: 30 s por defecto, 300 s máximo. Topes de salida: 64 KB / 512 KB.
  Si te truncan la salida, pagina tú.
- ESCRITURAS: `root_exec` pide `confirm: true` para acciones destructivas
  (instalar, matar, escribir fuera de tu sandbox, commit/push). Úsalo sólo con
  criterio.
- AUDITORÍA: cada llamada queda registrada en `audit.ndjson` de la instancia.
  Sé limpio: no lances comandos de comprobación redundantes.
- ROL: eres `worker`. La maestra (Samsung) es `primary` y decide. Publica tus
  hallazgos; no intentes ser el coordinador ni tocar el repo.
- NO INVENTES: si una tool devuelve `encrypted_messages_unreadable`, un error o
  un estado que no entiendes, repórtalo EXACTO. Nada de "creo que el paquete
  es X".
- SECRETOS: los tokens, cookies y claves que encuentres van al almacén de la
  instancia (ruta §5), NUNCA al chat. Al chat sólo el inventario y las rutas.

═══════════════════════════════════════════════════════════════════
5. PROTOCOLO DE VOLCADO (tu encargo principal)
═══════════════════════════════════════════════════════════════════
Escribe TODO lo tuyo bajo un único directorio con TU identificador:

    /data/local/tmp/megadaemon/incoming/<TU_ID>/

Tu identificador es: <TU_ID>

Escribe ahí los artefactos y, además, un fichero de manifiesto:

    /data/local/tmp/megadaemon/incoming/<TU_ID>/MANIFEST.ndjson

Formato: una línea JSON por artefacto (NDJSON, sin>array):
    {"id":"<uuid o hash>","name":"<nombre legible>","path":"<ruta en el móvil>",
     "origin":"<de dónde lo sacaste>","type":"apk|dex|so|json|log|txt|binary",
     "size_bytes":12345,"sha256":"<hex>",
     "summary":"<1 frase>","ts":"<ISO-8601>"}

Y el informe de reverse, en Markdown:

    /data/local/tmp/megadaemon/incoming/<TU_ID>/REPORT.md

Y las SEÑALES en un fichero aparte, **JSON Lines (NDJSON) — una línea JSON por
hallazgo, sin array**:

    /data/local/tmp/megadaemon/incoming/<TU_ID>/findings.ndjson

CONTRATO ÚNICO DE CADA LÍNEA (campos exactos):
    {"ts":1753000000,
     "package":"com.bbva.bbvacontigo","version":"5.4.1",
     "detector":"<enum cerrado, abajo>",
     "class":"<enum cerrado, abajo>",
     "method":"<cómo lo obtuviste, enum abajo>",
     "signal":"<valor crudo observado>",
     "java_class":"<clase Java donde está, o vacío>",
     "java_method":"<método Java donde está, o vacío>",
     "original":"<texto literal tal cual aparece>",
     "decoded":"<candidata descifrada, o vacío>",
     "kind":"<subtipo libre>",
     "severity":"info|low|medium|high",
     "confidence":0-100,
     "bypassable":true|false,
     "offset":-1,
     "evidence_file":"/data/local/tmp/megadaemon/incoming/<TU_ID>/...apk",
     "evidence_offset":1234,
     "notes":"<opcional, texto corto>"}

ENUMS CERRADOS (no inventes valores: si no encaja, `unknown`):
  · detector = magisk | su | frida | xposed | safetynet | playintegrity |
              tamper | emulator | root_behavior | obf_string | crypto_api |
              net_endpoint
  · class    = fs_path | property | process | maps | ptrace | keystore |
              signature | network | behavior | pool_string
  · method   = stat | readlink | grep_props | proc_read | package_manager |
              signature_check | runtime_probe | dex_pool_scan | manifest_parse |
              static_scan

REGLAS DE LOS CAMPOS:
  · `detector` = la CLASE de detección. `signal` = el valor CRUDO ("/sbin/.magisk",
    "ro.debuggable=1", "TracerPid=1234", "api.anthropic.com", "sk-…").
  · `class` = TIPO de evidencia (fs_path/property/process/…). `method` = CÓMO lo
    obtuviste (stat/readlink/grep_props/…). NO son la clase Java.
  · `java_class` / `java_method` = la clase y el método JAVA donde está el
    hallazgo ("" si no aplica). Éstos alimentan el mapa de desofuscación.
  · `original` = texto literal (para strings ofuscadas: la ofuscada). `decoded` =
    tu candidata. `kind` = subtipo libre ("xor", "base64", "array_bytes").
  · `offset` = índice en el pool de constantes del DEX (-1 si no aplica).
    `evidence_offset` = byte offset DENTRO del artefacto; si el hallazgo viene
    del propio dispositivo y no de un artefacto → `evidence_file` = ruta
    absoluta en el móvil y `evidence_offset` = -1.
  · `bypassable` = si la señal de root se puede ocultar (Magisk/su/Xposed/Frida/
    TracerPid/props de depuración). NO es "si es fácil": es "si lo cubrimos".
  · `severity` = info | low | medium | high. `confidence` = 0-100.
  · `version` = la del paquete (sin ella no se correlaciona con un APK concreto).

POLÍTICA (no negociable): aquí se documentan y se ocultan **señales de root**
(Magisk, su, Xposed, Frida, TracerPid, props de depuración). **NO** se falsea
attestation de Play Integrity ni identidad de app: eso no lo hace nadie aquí.
Si te piden algo de ese tipo, se rechaza y se reporta.

CÓMO ESCRIBIR LOS HALLAZGOS (para que se puedan procesar solos):
  Opción A (preferida): escribe tú mismo `findings.ndjson` en el directorio,
    una línea JSON por hallazgo, con los 19 campos EXACTOS en el orden de este
    documento.
  Opción B: pon los hallazgos en `REPORT.md` en una de estas TRES formas, que
    un conversor de MegaDaemon sabe leer:
      (B1) bloque ```json con un objeto por hallazgo
      (B2) tabla markdown (una fila por hallazgo, con los nombres de campo
           como cabeceras)
      (B3) bloques "clave: valor" (un bloque por hallazgo)
  ► LA PROSA SUELTA NO SE CONVIERTE. Si escribes "encontré una comprobación de
    Magisk en la clase X", eso no lo lee nadie: pon los campos.
  · `evidence_file`: puedes poner la RUTA ABSOLUTA del artefacto en el móvil,
    o el `id`, el `name` o el `sha256` con el que lo registraste en
    `MANIFEST.ndjson` — el conversor lo resuelve a la ruta absoluta. Si el
    hallazgo viene del propio dispositivo (no de un artefacto), pon la ruta
    absoluta del fichero en el móvil y `evidence_offset` = -1.
  · Si declaras un campo que no sepas: déjalo con su valor neutro y anótalo en
    `notes`. No lo rellenes con suposiciones.

Rutas: **absolutas y sin renombrar**. El módulo de decompilación DEX de
MegaDaemon lee cualquier ruta (`std::fs::read`): no necesita que el fichero esté
en `/data/app`, ni copia, ni renombrado. Escribe el APK/DEX tal cual en
`incoming/<TU_ID>/` y pon su ruta absoluta en `MANIFEST.ndjson`.

Para binarios (>1 MB) usa POST /api/chat_upload (base64 en JSON) o `fs_write`
con `base64: true` en trozos; el endpoint de subida además TE ANALIZA el
fichero y te devuelve formato, packer y entropía de salida:

    POST <BASE>/api/chat_upload
    X-API-Key / X-Seere-Auth-Token
    {"filename":"app.apk","data_b64":"<BASE64>"}

═══════════════════════════════════════════════════════════════════
6. QUÉ HACER, EN ORDEN
═══════════════════════════════════════════════════════════════════
FASE 1 — INVENTARIO Y VOLCADO (prioritario, no lo saltes)
  1) `tools/list` → confirma el catálogo real (27).
  2) `system_overview` → estado del móvil (RAM, apps, Android, SELinux).
  3) `POST /api/tool/apk_inventory` con {"action":"list"} → INVENTARIO REAL de
     las apps instaladas: package, apk_path, dex_count, native_libs, permisos,
     activities, receivers, providers. Es mejor que `app_control {action:list}`
     + `pm path`: te da la ruta de cada APK ya resuelta.
  4) Para las apps con IA (Claude, ChatGPT, Gemini, DeepSeek, Kimi, MiniMax)
     y las que te interesen: vuelca el APK CRUDO a tu directorio de entrada
     (`fs_write` en base64 por trozos o `POST /api/chat_upload`) y pon su
     ruta absoluta + sha256 en el MANIFEST.
  5) Metadata: permisos, activities, receivers, providers, libs nativas.
  6) Lo que tu reversing ya tenga y NO esté en el móvil: súbelo igual
     (es tu aporte diferencial). Índice todo en el MANIFEST.

  ► LO MÁS VALIOSO QUE PUEDES APORTAR: EL APK/DEX CRUDO, no sólo el digest.
    El motor de decompilación DEX de MegaDaemon (`POST /api/tool/dex_decompile`
    con {"action":"strings"|"obfuscation","path":<ruta absoluta>}) extrae del
    APK lo que a ti te costaría — clases, métodos, pool de constantes y el
    mapa de desofuscación. Si le mandas el APK entero, tú no tienes que
    implementar ni el detector de strings ni el de ofuscación: él lo hace.
    Prioridad de apps: las de IA del usuario — Claude, ChatGPT, Gemini,
    DeepSeek, Kimi, MiniMax — y cualquier otra con integraciones de IA.

FASE 2 — REVERSE DE ANDROID sobre lo volcado
  Tu contribución de más valor, por orden:
   a) APKs/DEX CRUDOS subidos a `incoming/<TU_ID>/` (ruta absoluta en el
      manifiesto). Esto es lo que alimenta el motor de MegaDaemon.
   b) SEÑALES con su CLASE y MÉTODO, en `findings.ndjson` o en el informe en una
      de las TRES formas legibles (contrato exacto en §5):
      `detector` ∈ magisk | su | frida | xposed | safetynet | playintegrity |
      tamper | emulator | root_behavior. Pista de búsqueda: "/sbin/.magisk",
      "su", "isRooted", "SafetyNet", "PlayIntegrity", "Xposed", "Frida",
      "TracerPid", "/proc/self/maps", "ptrace", "ro.debuggable".
   c) CADENAS EMBEBIDAS DE APIs Y CREDENCIALES (tipo net_endpoint /
      crypto_api). Apunta a las integraciones de IA y a las formas de token:
        · endpoints: api.anthropic.com, api.openai.com,
          generativelanguage.googleapis.com, api.deepseek.com,
          api.moonshot.cn/kimi, api.minimax.chat
        · credenciales con forma: sk-…, ghp_…, "Bearer ", x-api-key
      ⇒ el texto crudo va en `original` y, si deduces la candidata, en
        `decoded`. NO pegues tokens reales en el chat: van en el artefacto.
   d) EMPAQUETADO (packer): bangcle / ijiami / 360 / apkprotect / jiagu.
      Esto sí es un hueco de MegaDaemon: tu pre-digest sirve de CRUZADO.
   e) Ofuscación de strings: XOR, Base64, arrays de bytes; flujo de control.
   f) Clasificación por paquete + efecto real de la detección.
  Puedes apoyarte en `megadaemon_die_binwalk` (formato, packer, entropía,
  firmas embebidas) y `megadaemon_rizin` (funciones, imports, strings) sobre
  los ficheros que tú subas.

  ► LO QUE NO TIENES QUE HACER (ya está cubierto aquí, no lo inviertas):
    no hagas Ghidra de DEX, ni fidelidad de bytecode, ni descompilador de
    smali completo. MegaDaemon tiene su propio descompilador DEX.
    Si algo de eso te sale, mándalo como artefacto si sale, pero no lo
    conviertas en tu entregable principal.

FASE 3 — APORTE AL ENJAMBRE (si te queda margen)
  - Revisa `docs/MASTER_PLAN.md` y `docs/COORDINATION.md` (repo, sólo lectura)
    para ver dónde encaja tu trabajo y qué módulos están en marcha.
  - Reporta capability gaps: qué tool/endpoints faltarían, con su contrato.
  - Nunca sobrescribas un artefacto ya volcado por otro agente sin registrar
    la versión (sha256) en tu manifiesto.

═══════════════════════════════════════════════════════════════════
7. ENTREGABLES Y CRITERIO DE CALIDAD
═══════════════════════════════════════════════════════════════════
- Evidencia o nada: cada afirmación va con su fichero/línea o su medición.
  Lo que no se pueda verificar, márcalo `pendiente`/`bloqueado` y di POR QUÉ.
- Al terminar devuelve en el chat:
    1) el ÍNDICE de lo volcado (cuántos artefactos, de qué tipos, qué queda
       cubierto y qué no),
    2) un resumen de 10 líneas en español con los hallazgos de reverse,
    3) la lista de capability gaps que has detectado.
- Y deja en la instancia: los artefactos + MANIFEST.ndjson + REPORT.md +
  findings.ndjson (o los hallazgos en el informe, en JSON / tabla markdown /
  bloques "clave: valor" — ver §5).
- La instancia tiene un conversor (`tools/findings_from_manifest.py`, corre en el
  PC, sin binario nuevo) que normaliza tu salida al esquema cerrado. Funciona
  mejor si declaras los 19 campos y la evidencia por id/nombre/sha256 o ruta
  absoluta; si no, rellena lo que falta con valores neutros y lo anota. En
  ningún caso inventa: lo que no declares, queda vacío/unknown y anotado.
- Cada hallazgo tuyo que sirva al proyecto debe acabar como mejora
  verificable en MegaDaemon (tool nueva, test, regla), no sólo en un informe.
```

---

## 2. Lo que MEGADAEMON es (para explicárselo a quien pregunte)

| Capa | Qué es |
|---|---|
| Núcleo | Un binario Rust (`megadaemon-core`, 26 módulos) que corre en el Samsung y en PCs Linux. Sin Python en los hot paths, sin JVM. |
| Instancia maestra | El propio Samsung: `primary`. Decide, reparte trabajo y sincroniza. Es el que guarda las claves. |
| Cockpit | Servidor HTTP en Rust puro con la UI (mobile-first). En el móvil: `http://localhost:3000`. Desde la LAN: `http://<IP_DEL_MOVIL>:3000`. |
| MCP | JSON-RPC 2.0 sobre HTTP (`POST /mcp`). Es la vía por la que los agentes externos (y tú) hablan con la instancia sin tocar su código. |
| ADB | Cliente ADB escrito en Rust puro (`rust-core/src/adb_client.rs`), porque el móvil no tiene binario `adb`. `adbd` escucha en TCP 5555 en la LAN. |
| RE | DiE + Binwalk + Scalpel + Bulk Extractor, Rizin 0.9.1, Kaitai/ImHex, Tile Molester/YY-CHR, desofuscador MBA, LTO-Decomp CIRX (Micro-RTL/SSA), Exynos 2100 NPU + CV. |
| Swarm | Enrutador LLM multi-modelo con failover: MiniMax, GitHub Models, Pollinations, OpenRouter, Groq, Cerebras, Google AI Studio, Mistral, y SEERE local. |
| Coordinación | Tablero de tareas SQLite con *claims* y *leases*, bus de malla peer-a-peer, y roles `primary`/`worker`/`observer`. |

---

## 3. Canales verificados (probados contra la instancia en vivo)

| Canal | Uso | Verificado |
|---|---|---|
| `POST /mcp` + `tools/list` | catálogo (26 tools) | ✅ HTTP 200 |
| `POST /mcp` + `tools/call fs_write` | escribir cualquier fichero (texto o base64) | ✅ `ok:true` |
| `POST /mcp` + `tools/call fs_read` | leer (tope 8 MB) | ✅ |
| `POST /api/chat_upload` | subir binario (base64) → lo analiza con DiE | ✅ devuelve formato/packer/entropía |
| `POST /mcp` + `tools/call root_exec` | shell como root (con `confirm`) | ✅ |
| `GET /health` (con `X-API-Key`) | estado del daemon | ✅ `{"status":"ok","version":"13.3.0"}` |
| `POST /api/session/exchange` | canjea la API key por un token de sesión (navegador) | ✅ (usado por la UI) |

---

## 4. Lo que el agente externo NO puede hacer (límites reales)

| No puede | Por qué | Alternativa |
|---|---|---|
| Escribir en el repo o hacer push | Sólo el principal commitea a `main` | Publica el manifiesto; el principal lo ingiere |
| Leer cookies de Chrome | Cifradas por el AndroidKeyStore (TEE); ni root las saca | Hook en proceso (Frida) o exportación oficial |
| Leer el contenido cifrado de WhatsApp | Clave en el keystore del proceso | Hook en proceso o export oficial |
| Falsificar Play Integrity | Atestación firmada por el fabricante | No se hace (claim falso) |
| Ser el coordinador | La maestra es el Samsung | Publica en el tablón y espera |
| Traer la clave ADB privada | Es del host y vive en el móvil | Usa `adb connect` con la clave ya registrada |

---

## 5. Nota de mantenimiento

Este documento debe revisarse en cuanto el binario **v13.4+** esté desplegado
en el móvil: entonces el catálogo de tools crecerá (inventario de APKs,
decompilador DEX, tablero de coordinación, vault de firma) y el puente web
dejará de ser necesario. Hasta entonces, el catálogo de 26 tools de §3 del
prompt es el bueno.

*Verificado por el agente de coordinación y dispositivo contra la instancia en
vivo el 2026-11-19.*
