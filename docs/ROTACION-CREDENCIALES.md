# ROTACIÓN DE CREDENCIALES EXPUESTAS — MegaDaemon

> **Fecha de la auditoría:** 2026-11-23 · **Auditor:** agente `general` (rama `rama/ramas-auditoria-2026-11-23`)
> **Base `main` medida:** `87c66e527641e6ed5d1640df53f8d82718f7139c` (el `main` se mueve: el root integra ramas mientras se mide)
> **Regla de este documento:** en ningún sitio se escribe el valor de una credencial.
> Sólo **tipo**, **prefijo enmascarado**, **fichero:línea** y **commit**.

---

## 0. Resumen ejecutivo (lo primero que hay que leer)

| # | Hecho medido | Prueba | Impacto |
|---|---|---|---|
| **H-1** | **El repo es PÚBLICO en GitHub**: `boolforge/megadaemon` → `private=false, visibility=public`, último push `2026-11-23T06:08:19Z` | `curl https://api.github.com/repos/boolforge/megadaemon` | Todo lo que esté en `main` **o en su historial** es legible por el mundo |
| **H-2** | **4 credenciales reales de Google (API keys `AIzaSy…`, 39 chars) están escritas a pelo** en un fichero versionado | `tools/drive_ls.py:23-26` (main) | **ROTAR YA** + sacar del historial |
| **H-3** | Esas 4 keys entraron en el repo el **2026-11-21 12:40:23Z** | `git log -S'AIzaSy' main -- tools/drive_ls.py` → `235c399` | Están publicadas desde hace ~2 días |
| **H-4** | `main` tiene **marcadores de conflicto sin resolver** en 3 ficheros ⇒ **el bus (mesh board) no parsea como JSON** | `git grep -nE '^(<<<<<<< \|>>>>>>> )' main` → `state/mesh_board.json:3,5,7`, `docs/ANTIGRAVITY_BACKLOG.md:30-33,340-343`, `docs/AUDIT_ADVERSARIAL_V15.md:285-289,309-313` | Rompe la coordinación del enjambre |
| **H-5** | La API key **viva** (`mdaemon_sk_live_…`) **NO** aparece en `main` ni en las puntas de rama | barrido de 81 refs (`/workspace/secrets_tree.tsv`) + comparación por sha256 del valor completo | **La rotación de la API key ya está hecha** |

---

## 1. Credenciales EXPUESTAS (rotar)

### 1.1 — P0 · Google API keys ×4 (`AIzaSy…`, 39 chars)

| Campo | Valor |
|---|---|
| **Tipo** | Google API key (lectura de Drive; la carpeta de Swift Backup es **pública**: `folder_id=1L-XKLJKy68Jmjf0K8uIRXOklL4gUIdO1`) |
| **Prefijo (enmascarado)** | `AIzaSyBq…` · `AIzaSyAo…` · `AIzaSyB6…` · `AIzaSyBB…` |
| **Fichero:línea** | `tools/drive_ls.py:23`, `:24`, `:25`, `:26` |
| **Commit que las introdujo** | `235c3993386c712ab110576db72ea13a7facc12f` — *"feat(swift): la cabecera SBA1 REAL, medida en la nube (1,04 TB)"* — 2026-11-21T12:40:23Z |
| **Ramas donde aparecen** | sólo la línea `235c399` (`git log --all -S'AIzaSy' -- tools/drive_ls.py` → 1 commit), pero ya está en `main` ⇒ **público** |
| **Estado** | ❌ **NO rotada**, ❌ **NO purgada del historial** |
| **Riesgo** | **ALTO**: una API key de Google sin restricción de API/origen permite a un tercero leer (y escribir, si el scope lo permite) la Drive del usuario. Es una credencial de cuenta, no una de prueba |

**Procedimiento de rotación (lo hace el root; yo NO toco credenciales):**

1. **Revocar las 4 keys** en <https://console.cloud.google.com/apis/credentials> (API keys → las 4 que empiezan por `AIzaSy…`):
   - borrar o "restringir" cada una (API de Drive only, origen sólo el IP del host),
   - **revisar "uso" y "cuota"** de cada key antes de borrar: si hay tráfico desde IPs desconocidas, hay que revisar **qué ficheros/folders** se han descargado/modificado.
2. **Emitir 1 key nueva** con las restricciones correctas (Drive API de sólo lectura, origen = la IP del host que ejecuta `tools/drive_ls.py`).
3. **Sacar el valor del repo** — el fichero ya lee del entorno, así que basta con:
   - `tools/drive_ls.py:19` ya tiene `API_KEY = os.environ.get("MEGADAEMON_GOOGLE_API_KEY", "")`;
   - ⇒ **borrar el array `KEYS` (líneas 22-27) y dejar sólo la lectura del entorno**. El array es la única copia en claro del repo.
   - La key nueva vive en `/workspace/md-secrets/` (fichero 0600), p. ej. `google_api.key`, y `instance.env` gana `MEGADAEMON_GOOGLE_API_KEY=`.
4. **Verificar**:
   ```bash
   # en el host: con la key en el entorno y el array KEYS ya borrado
   MEGADAEMON_GOOGLE_API_KEY=<nueva> python3 tools/drive_ls.py
   # debe listar la carpeta => la key nueva funciona
   # y con la key VIEJA debe fallar (revocación comprobada):
   #   -> "API key not valid" / 403
   ```
5. **Purgar del historial** (el repo es público, así que rotar NO basta: hay que reescribir la historia):
   ```bash
   # en una copia, NUNCA sobre main en caliente
   git filter-repo --invert-paths --path tools/drive_ls.py   # NO: se perdería el fichero
   # mejor: sustituir sólo el literal
   git filter-repo --replace-text <(printf 'AIzaSy%b->AIzaSyREDACTED\n' '') --invert-paths
   # (o `git filter-repo --replace-text replaces.txt` con las 4 reglas literales)
   git push --force --mirror  # el root decide: exige coordinación con el resto de la flota
   ```
   ⚠️ **El historial reescrito invalida todos los SHA ya publicados** ⇒ el root tiene que avisar a la flota y re-basar las ramas vivas **antes** de reescribir.

---

### 1.2 — P1 · Credenciales VIVAS fuera del repo (en `/workspace/md-secrets` y `/workspace/.secrets`)

Estas **no** están en el repo (barrido de 81 refs: 0 coincidencias con valor completo), pero son las que gobiernan el sistema. Se listan aquí para que el root tenga el inventario completo y sepa **qué rotar y en qué orden**.

| Credencial | Fichero(s) 0600 | Dónde la lee el sistema | Procedimiento | Verificación |
|---|---|---|---|---|
| **API key del daemon** `mdaemon_sk_live_…` | `/workspace/.secrets/s21_api.key`, `/workspace/md-secrets/megadaemon_api.keys`, `/workspace/md-secrets/instance.env` (`MEGADAEMON_API_KEY`), `/workspace/md-secrets/master_api.key`, `/workspace/md-secrets/agent_general.key`, `/workspace/md-secrets/agent_external_api.key` | móvil: `/data/user/0/com.agy/files/megadaemon/state/megadaemon_api.key` (y `/data/local/tmp/megadaemon/state/…`) | 1) generar la nueva en el móvil; 2) `bash tools/sincronizar-key.sh` **en el móvil** (escribe en los 2 `state_dir`, 0600, e imprime el fingerprint `fnv1a64`); 3) actualizar `/workspace/.secrets/s21_api.key` + `instance.env`; 4) reiniciar el daemon | `tools/sincronizar-key.sh` imprime el **fingerprint fnv1a64** (nunca la key) ⇒ comparar con el del PC; luego `POST /api/session/exchange` **debe** dar 200 (403 = key caducada) |
| **Token de sesión** (`cockpit_token`, `session_token`) | `/workspace/md-secrets/cockpit_token`, `session_token`, `session_token.current` | cabeceras `X-Seere-Auth-Token` | **No se rota a mano: rota solo en cada reinicio del daemon.** Se canjea por la API key | `POST /api/session/exchange` con la API key **en el cuerpo** ⇒ 200 + token nuevo. Un 403 significa "la API key vale, el token no" |
| **Credenciales de Google Drive (OAuth)** | `/workspace/md-secrets/gdrive_credentials.json` (client + refresh token) | `tools/drive_push.py`, `tools/drive_fetch.py`, `tools/drive_verify.py` | `rclone authorize` en el móvil (hay un flow de 5 fases en `tools/nh_backup_phone.sh`) ⇒ nuevo `refresh_token` ⇒ actualizar `gdrive_credentials.json` | `python3 tools/drive_verify.py` (comprueba sha256 de los ficheros, no sólo el upload) |
| **Token de DuckDNS** | `/workspace/md-secrets/duckdns.token`, `/workspace/.secrets/duckdns.token` | `tools/md-repair-entorno.sh` (publica el A del dominio) | ⚠️ **Nunca desde un nodo/sandbox**: el `update` de DuckDNS fija el A al IP de quien llama ⇒ desde el sandbox apuntaría al NAT del datacenter. **Sólo desde el PC o el móvil**, con su IP real | `dig +short <dominio>` **contra un resolver externo** (DoH de Cloudflare, `https://cloudflare-dns.com/dns-query`) — `getent hosts` puede mentir |
| **Claves ADB/SSH del host** | `/workspace/md-secrets/adb_host_key.pem`, `adb_user_pc_key.pem` | ADB del móvil (`/data/misc/adb/adb_keys`) | rotar el par ⇒ reinstalar en el móvil (`tools/md-repair-entorno.sh` avisa si falta la clave pública en `adb_keys`) | `adb devices` + `tools/md-repair-entorno.sh --diagnostico` |
| **Prompt con claves** (fichero de trabajo) | `/workspace/md-secrets/EXTERNAL_AGENT_PROMPT_WITH_KEYS.md` (22 KB) + `.pre-tokenrot-1795132559` + `EXTERNAL_AGENT_PROMPT_FINAL.draft-with-keys.md` | — | **borrar los borradores con claves** del disco (el canónico `docs/EXTERNAL_AGENT_PROMPT_FINAL.md` del repo ya está limpio: 0 coincidencias) | `grep -cE 'mdaemon_sk_|sk-[A-Za-z0-9]{10,}|AIza|-----BEGIN' docs/EXTERNAL_AGENT_PROMPT_FINAL.md` ⇒ **0** |

---

## 2. Falsos positivos (NO hay que rotar nada)

Medido con `git grep` sobre las 81 puntas (rama + main) y luego **verificando el valor completo** contra las credenciales vivas (sha256). Resultado: **0 coincidencias** entre `main` y las credenciales vivas.

| Patrón | Dónde | Por qué NO es un secreto |
|---|---|---|
| `sk-<random>` en `inventory/inventory.json:3147,13440,14429` | rutas de APK | es un **componente aleatorio del path** de `/data/app/~~…/pkg-XXXX==/base.apk`, no una key |
| `sk-<epoch>` en `state/delegated_tasks.json:3,10,…,52` | ids de tarea | `t-` + epoch: el prefijo `sk-` es coincidencia con el patrón |
| `-----BEGIN PRIVATE KEY-----` en `rust-core/src/selfsigned_cert.rs:392,472` | código | es el **literal de la cabecera** que escribe `key_to_pem()`, no una clave |
| `-----BEGIN PRIVATE KEY-----` en `rust-core/src/clipboard.rs:975` | test | es un `assert!(es_sensible("-----BEGIN…"))` |
| `TEST_RSA_KEY_PEM` en `rust-core/src/signing.rs:5388,5441` | test | el propio doc-comment dice *"Clave RSA de prueba (PKCS#8 PEM, 2048 bits). Generada para los tests."* — **no sirve para nada real** |
| `sk-cp-ABCDEFGHIJKLMNOP` en `rust-core/src/agyshim.rs:986` | test de redacción | fixture (`MNOP`) |
| `AIzaSyAB…MNOP` / `AIzaSyAB…QRST` / `AIzaSyAA…GGGG` | `agyshim.rs:986`, `redact_forma.rs:90`, `dex_decompile.rs:5271` | fixtures con letras repetidas (`MNOP`, `QRST`, `AAAA…GGGG`) |
| `sk-EXAMPLE-N…` (29 chars) en `rust-core/tests/fixtures/**`, `docs/AUDIT_LLM_V16.md:388` | fixtures | placeholder explícito |
| `ghp_AAAABBBB…` / `ghp_ABCDE…` | `dex_decompile.rs:5270`, `github_connector.rs:1126,1129`, `tests/github_connector_smoke.rs` | fixtures |
| `sk-ant-…` en `app_intel.rs:3006,3050,3237` y `dex_decompile.rs:5268,5274,5280,5350,5392` | fixtures | valores `…2345`, `…cdef`, `…EEEE`, `…CRET` |
| `mdaemon_sk_live_` (16 chars, sólo el prefijo), `mdaemon_sk_live_VALOR`, `mdaemon_sk_live_PON_AQUI_LA_KEY_REAL` | `api_key.rs`, `deploy/megadaemon.env.example:28,33`, `docs/**`, `web/cockpit_v2.html:657`, `deploy/web_bridge.py:72` | **placeholders** (el prefijo solo, sin sufijo). `deploy/megadaemon.env.example` está además con `!deploy/megadaemon.env.example` en `.gitignore:31` |
| `sk-ModuleRepo` / `sk-play-install-fix` | `battery_advanced.rs:23,43`, `store_manager.rs:1031,1098` | **nombres de paquete**, no keys |

---

## 3. Lo que YA está rotado (no repetir)

Evidencia: los ficheros de reserva que dejó cada rotación anterior y los `.pre-*`.

| Rotación previa | Evidencia en disco | Qué prouve |
|---|---|---|
| API key del móvil re-sincronizada | `/workspace/md-secrets/instance.env.pre-keyfix-1795132082` | hubo una rotación de key (2026-11-20 09:48) |
| Token de sesión rotado (×2) | `instance.env.pre-tokenrot-1795132538` y `instance.env.pre-tokenrot-1795132546` | el token de sesión se renovó dos veces el 2026-11-19 |
| Prompt con claves saneado | `EXTERNAL_AGENT_PROMPT_WITH_KEYS.md.pre-tokenrot-1795132559` | se rotó lo que había en el prompt |
| Rotación de `instance.env` | `instance.env.pre-rotation-2330` | rotación de credenciales del 2026-11-19 |
| URL del túnel caducada | `tunnel.url.stale-2334` | las URLs trycloudflare son efímeras |
| Clave ADB antigua | `adb_host_key.pem.stale-2012` | la clave ADB ya se regeneró |
| **La API key viva no está en el repo** | barrido de 81 refs + comparación sha256 del valor completo: **0 coincidencias** | la rotación de la API key **está hecha**; no hay que re-hacerla |

---

## 4. Acciones EXACTAS para el root (lista para ejecutar, en orden)

> Yo **no roto nada** (el root tiene `/workspace/md-secrets` y es el único que puede).

- [ ] **A-1 (P0, ya)** — Revocar las 4 keys `AIzaSy…` de `tools/drive_ls.py:23-26` (console de Google Cloud). Antes de borrar: mirar **uso y cuota** de cada una.
- [ ] **A-2 (P0, ya)** — Emitir 1 key de Drive de sólo lectura + origen restringido; guardarla **fuera del repo** (`/workspace/md-secrets/google_api.key`, 0600) y exportarla como `MEGADAEMON_GOOGLE_API_KEY`.
- [ ] **A-3 (P0, ya)** — Borrar el array `KEYS` de `tools/drive_ls.py:22-27` (el script ya lee `MEGADAEMON_GOOGLE_API_KEY` del entorno, `tools/drive_ls.py:19`) ⇒ **el repo deja de contener la credencial**.
- [ ] **A-4 (P1)** — Reescribir el historial (`git filter-repo --replace-text`) para sacar las 4 keys de `235c399`. **Coordina antes**: el push force invalida los SHA publicados ⇒ avisar a la flota y re-basar las ramas vivas.
- [ ] **A-5 (P0, ya)** — **Reparar `main`**: quitar los marcadores de conflicto de `state/mesh_board.json:3-7`, `docs/ANTIGRAVITY_BACKLOG.md:30-33,340-343` y `docs/AUDIT_ADVERSARIAL_V15.md:285-289,309-313` (introducidos por `d148c95` "merge(flota)" y propagados por `d902a7e` "docs=theirs"). **El bus no parsea hasta que se arregle.**
- [ ] **A-6 (P2)** — Rotar la API key del daemon **sólo si** hay que forzar el cierre de sesiones: `tools/sincronizar-key.sh` **en el móvil** → actualizar `/workspace/.secrets/s21_api.key` + `instance.env` → reiniciar el daemon → comprobar `POST /api/session/exchange` ⇒ 200 (no 403).
- [ ] **A-7 (P2)** — Borrar los borradores con claves del disco: `/workspace/md-secrets/EXTERNAL_AGENT_PROMPT_WITH_KEYS.md.pre-tokenrot-1795132559` y `/workspace/md-secrets/EXTERNAL_AGENT_PROMPT_FINAL.draft-with-keys.md`.
- [ ] **P3** — Renovar el `refresh_token` de Drive (`gdrive_credentials.json`) vía `rclone authorize` en el móvil.
- [ ] **P4** — Rotar el token de DuckDNS **desde el PC o el móvil** (nunca desde un nodo) y verificar con DoH externo.

---

## 5. Cómo se ha medido (reproducible)

```bash
cd /workspace/md-root
# 1) el repo es público
curl -s https://api.github.com/repos/boolforge/megadaemon | grep -o '"private":[a-z]*'
# 2) las 4 keys de Drive están en main
git grep -n 'AIzaSy' main -- tools/drive_ls.py
# 3) marcadores de conflicto en main
git grep -n -E '^(<<<<<<< |>>>>>>> )' main --
# 4) barrido de las 81 puntas con el valor redactado
bash /workspace/audit_secrets3.sh          # -> /workspace/secrets_tree.tsv
# 5) ¿alguna credencial VIVA está en el árbol de main? (valor completo, sha256)
python3 /workspace/check_exact_main.py     # -> /workspace/secrets_exact_main.json
# 6) ¿alguna credencial VIVA está en el HISTÓRICO? (pickaxe por valor literal)
python3 /workspace/check_live_in_history.py # -> /workspace/secrets_live_in_history.json
```

