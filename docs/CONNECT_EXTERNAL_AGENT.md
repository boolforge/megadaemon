# Prompt de conexión para un agente externo a tu instancia de MegaDaemon

> Para pegar en el agente que corre en la cuenta
> `josefranciscocarrascobenitez@gmail.com` (sesión de MiniMax con login de Google).
> Objectivo: que ese agente se **conecte a tu instancia de MegaDaemon**, use el MCP
> nativo en vez de improvisar, y devuelva los resultados de su reversing de Android
> al almacén común, para que la instancia maestra (el Samsung) los vea.

---

## 1. Lo que tienes que darle (completa esta tabla antes de pegarle el prompt)

| Dato | Dónde está | Valor para tu caso |
|---|---|---|
| **URL base del cockpit** | Fichero `megadaemon.env` de la instancia | `https://webpage-ultram-please-communicate.trycloudflare.com` |
| **Ruta del MCP** | Mismo host | `POST /mcp` (JSON-RPC 2.0) — **usa esta, no `/sse`** |
| **API key remota** | `state/megadaemon_api.key` (0600) | `mdaemon_sk_live_…` (la que ya tienes) |
| **Token de escritura** | `state/cockpit_token` (0600) | `X-Seere-Auth-Token` — **sólo** para POST/PUT |
| **Instancia** | — | el **PC Linux es `worker`**, el **Samsung es `primary`/maestro** |
| **Rol del agente externo** | — | `worker` con capacidad `android-re` |

Cabeceras obligatorias en cada llamada:

```
X-API-Key: <mdaemon_sk_live_...>          # siempre
X-Seere-Auth-Token: <cockpit_token>        # sólo si modifica estado
```

---

## 2. El prompt para pegar (copia desde aquí)

```
Tienes acceso a una instancia de MegaDaemon (Rust puro) que ya está funcionando y
que actúa como harness de ingeniería inversa para Android. Tu trabajo es USARLA, no
reimplementar nada.

═══════════════════════════════════════════════════════════════════
1. CONEXIÓN
═══════════════════════════════════════════════════════════════════
Endpoint MCP (JSON-RPC 2.0 sobre HTTP POST):

    POST https://webpage-ultram-please-communicate.trycloudflare.com/mcp
    X-API-Key: <META_LA_API_KEY_AQUI>
    X-Seere-Auth-Token: <META_EL_TOKEN_AQUI>

El body es JSON-RPC 2.0 puro:

    {"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
    {"jsonrpc":"2.0","id":2,"method":"tools/call",
     "params":{"name":"system_overview","arguments":{}}}

Empieza SIEMPRE por `tools/list` y lee el catálogo completo antes de actuar.
La lista de herramientas incluye (entre otras):
  - android: system_overview, root_exec, fs_read, fs_write, find_duplicates,
             props_and_settings, selinux_control, magisk_module,
             kernel_and_blocks, app_control, screen_and_ui
  - sync: extracción y búsqueda de conversaciones (Claude/DeepSeek/Kimi/ChatGPT)
  - RE: inventario de APKs, decompilación DEX, ocúltación root, firma, backed
  - orquestación: capabilities, task_queue, alerts, agent, mesh (coordinación)

═══════════════════════════════════════════════════════════════════
2. REGLAS DE OPERACIÓN (obligatorias)
═══════════════════════════════════════════════════════════════════
- Eres un WORKER. La instancia maestra es el Samsung (S21 Ultra, root, Magisk).
  Publica tus hallazgos en el tablón; NO intentes ser el coordinador.
- Nada de shell libre: usa las tools tipadas. `root_exec` exige
  `confirm: true` para acciones destructivas (instalar, matar, escribir fuera
  de tu sandbox, commit/push).
- Respeta los timeouts del daemon (30 s por defecto, 300 s máximo) y los topes
  de salida (64 KB / 512 KB). Si un comando devuelve mucho, pagínalo tú.
- Cada llamada queda auditada en `audit.ndjson` en la instancia. Sé limpio.
- Si una tool devuelve `encrypted_messages_unreadable`, `reason` o estado de
  sesión caducado: NO inventes el contenido. Reporta el estado exacto.
- Antes de una acción irreversible, comprueba el resultado en el móvil:
  `system_overview` + `app_control`.

═══════════════════════════════════════════════════════════════════
3. TU ENCARGO: REVERSE DE ANDROID
═══════════════════════════════════════════════════════════════════
a) Descubre qué APKs hay instaladas y su estado:
     app_control  → listar paquetes
     apk_inventory → analyse / dump de una APK concreta
b) Para cada APK que te interese:
     - decompila el DEX a pseudo-Java y produce el mapa de desofuscación
       (tool de decompilación DEX de MegaDaemon)
     - busca señales de anti-root / anti-debug / anti-tamper: cadenas como
       "su", "/sbin/.magisk", "isRooted", "SafetyNet", "PlayIntegrity",
       "Xposed", "Frida", "TracerPid", "/proc/self/maps"
     - detecta empaquetado (bangcle/ijiami/360/apkprotect), ofuscación de
       strings (XOR, Base64, arrays de bytes) y flujos de control(climbers)
c) App de escritorio o APK con información de sesión local:
     - extrae rutas de datos, cookies, tokens y endpoints
     - NO copies credenciales a un chat: guárdalas en el almacén de la
       instancia y devuelve SÓLO el inventario y las rutas
d) Entrega: un informe por app con
     {paquete, versión, hashes de firma, dex count, libs nativas,
      señales anti-root encontradas (con clase/método), mapa de desofuscación,
      plan de parche}
   y guárdalo en la instancia con fs_write para que el Samsung lo vea.

═══════════════════════════════════════════════════════════════════
4. COORDINACIÓN (esto es lo que hace que NO nos pisemos)
═══════════════════════════════════════════════════════════════════
- Antes de empezar, lee el tablón compartido del repo:
    docs/COORDINATION.md   (estado de cada agente y qué ficheros están tocados)
    docs/MASTER_PLAN.md    (plan maestro por fases)
- Publica tu trabajo con la tool de mesh/task_queue: registra la tarea, la
  reclamas (claim), y libérala con resultado al terminar. Nunca reclamues una
  tarea que ya tenga dueño.
- Firma/firma compartida: todo material criptográfico (claves, certificados,
  firmas) va al almacén cifrado de la instancia y se replica a TODAS las
  instancias por el canal de sincronización. No lo pegues en el chat ni lo
  dejes en un repo.
- Los ficheros de código los tocas sólo en TU módulo (src/<nombre>.rs) y
  commiteas con rutas exactas: NUNCA `git add -A`, NUNCA `--force`.
- Si encuentras un bug en MegaDaemon: no lo parchees a mano. Reporta
  (app+tool+pasos) y deja la corrección propuesta; el ciclo lo aplica.

═══════════════════════════════════════════════════════════════════
5. ENTREGABLES Y CRITERIO DE CALIDAD
═══════════════════════════════════════════════════════════════════
- Nada de "fantasmadas": cada afirmación va con el fichero/línea o la
  medición que la respalda. Lo que no se pueda verificar, márcalo
  explícitamente como pendiente/bloqueado y di POR QUÉ.
- Cada fallo que encuentres debe producir una mejora en MegaDaemon
  (regla nueva, tool nueva, test) — no sólo un informe.
- Al terminar: mensaje de estado en el tablón (una línea), resumen de 10
  líneas en español, y el informe completo guardado en la instancia.
```

---

## 3. Cómo lo consume el agente (copiar/pegar)

Hay dos formas, y la segunda es la buena:

1. **Rápida**: el agente pega el bloque de arriba y llama a `/mcp` con `curl` o
   con un cliente MCP. Funciona, pero cada tool la escribe a mano.

2. **La buena — cliente ya hecho**: el repo trae `rust-core/tools/mdmcp.py`,
   un cliente MCP que ya habla con el daemon:

```bash
# listar herramientas
python3 mdmcp.py tools

# llamar una tool
python3 mdmcp.py call system_overview '{}'
python3 mdmcp.py call app_control '{"action":"list"}'

# decompilar una APK del dispositivo
python3 mdmcp.py call apk_inventory '{"action":"analyze","path":"/data/app/.../base.apk"}'
```

Copia `rust-core/tools/mdmcp.py` a la máquina del agente y dale las dos
credenciales por variable de entorno (**nunca** en el prompt ni en el repo):

```bash
export MEGADAEMON_URL="https://webpage-ultram-please-communicate.trycloudflare.com"
export MEGADAEMON_API_KEY="mdaemon_sk_live_…"
export MEGADAEMON_AUTH_TOKEN="…"     # sólo si va a escribir
```

---

## 4. Lo que el agente externo NO puede hacer (límites reales)

| No puede | Por qué | Alternativa |
|---|---|---|
| Escribir en el repo directamente | Sólo el agente principal hace push a `main` | Publica en el tablón; el principal commitea |
| Falsificar Play Integrity | Atestation de hardware firmada por el fabricante | Herramientas públicas de la comunidad (PIF/TrickyStore), sin claims falsas |
| Leer cookies de Chrome | Cifradas por AndroidKeyStore, ni root extrae | Hook en proceso (Frida) |
| Leer contenido de WhatsApp | Cifrado con la clave del keystore | Hook en proceso o export oficial |
| Ser el coordinador | La maestra es el Samsung | `task_queue` + `mesh` |

---

## 5. Orden de conexión recomendado

```
1. tools/list                       → confirmar el catálogo
2. system_overview                  → ver el móvil y su estado
3. app_control(list)                → qué APKs hay
4. apk_inventory(analyze)           → 2-3 APKs concretas
5. decompilación DEX + mapa de desofuscación
6. fs_write del informe a la instancia  → el Samsung lo ve
7. task_queue: registrar y cerrar la tarea en el tablón
```

---

*Documento generado por el agente principal de MegaDaemon (sesión 464802478674020).
Repositorio: https://github.com/boolforge/megadaemon*
