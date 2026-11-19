# Despliegue en el dispositivo — Galaxy S21 Ultra (Exynos 2100)

> Todo lo de este documento se ejecutó **en el dispositivo real** a través del
> servidor MCP `android-root`. Las cifras son medidas, no estimadas.
> Fecha: **2026-11-19**.

## 1. Identificación del dispositivo

| Dato | Valor |
|---|---|
| Modelo | Samsung Galaxy S21 Ultra, `SM-G998B` (`ro.product.device=p3s`) |
| SoC | **Samsung Exynos 2100** (`universal2100_r`), NPU triple-núcleo 26 TOPS (INT8) |
| Android | 14 (`UP1A.231005.007`) |
| Parche de seguridad | **2024-06-01** |
| CSC | `AMO` / `SPAIN` |
| Kernel | `5.4.242-28575149-abG998BXXSBGXED` (stock Samsung) |
| SELinux | Enforcing |
| Magisk | **Kitsune v27.2** (zygisk=1, denylist=0, magiskhide=1) |
| LSPosed | v1.10.1 (módulo `zygisk_lsposed`) |
| Wi-Fi | **Broadcom** (`exynos_pcie_config_l1ss_bcm_wifi`) ⇒ Nexmon es aplicable |
| Root shell | `su -mm -c` (root real, mount namespace completo) |
| Espacio al inicio | **3,3 GB libres de 225 GB (99 % usado)** |
| RAM | 10,5 GB totales, **221 MB libres** al inicio (zram 4 GB al 84 %) |

## 2. Megadaemon: de "instalado" a "demonio persistente"

**Hallazgo**: megadaemon **sí estaba** en el dispositivo, pero dentro del
sandbox Termux de Antigravity (`/data/data/com.agy/files/megadaemon`, 1,04 GB
con `rust-core/target` incluido), y sólo corría como `mcp-server` de la app.
No había supervisor de arranque, ni health-check, ni exposure remota.

### 2.1 Qué se hizo

| Paso | Detalle |
|---|---|
| Binario ARM64 | **Cross-compilado** en el sandbox con el **NDK r26c** (`aarch64-linux-android26-clang`), 2,86 MB, `interpreter /system/bin/linker64`. Publicado como release de GitHub y descargado en el móvil con **SHA-256 verificado byte a byte**. |
| Instalación | `/data/local/tmp/megadaemon/megadaemon-core` (755) |
| Config | `/data/local/tmp/megadaemon/megadaemon.env` (600): API key, `STATE_DIR`, `REQUIRE_API_KEY=1` |
| Supervisor | `/data/adb/service.d/99-megadaemon.sh` (755) — Magisk lo ejecuta al arrancar: `setsid nohup` + bucle de reintento con backoff (10→300 s) + **rotación de log** (10 MB × 5 ficheros) |
| Endpoint de salud | `GET /health` → `{"status":"ok","uptime":N,"version":"X","requests":N}` |
| Métricas | `GET /metrics` → CPU (`/proc/stat`), RAM (`/proc/meminfo`), `loadavg`, uptime, contadores (total / rechazados / rate-limited) |
| API key | `src/api_key.rs`: `X-API-Key` o `Authorization: Bearer`; **comparación en tiempo constante**; rate limit 60 req/min por IP; auditoría con IP, timestamp, user-agent y path |
| Exposición | Túnel Cloudflare (`cloudflared`, ejecutado **como root** porque sólo root resuelve DNS en el dispositivo) |

### 2.2 Verificación desde Internet (no desde el móvil)

```
GET https://webpage-ultram-please-communicate.trycloudflare.com/health
  sin key            -> HTTP 401  {"error":"unauthorized",...}
  con key válida     -> HTTP 200  {"status":"ok","uptime":26,...}
  con key incorrecta  -> HTTP 403  {"error":"forbidden",...}
GET .../metrics  (con key) -> {"api_key_active":true,"mem_used_pct":69.1,
                               "loadavg":"20.81","requests_total":7,...}
```

### 2.3 Un bug de seguridad encontrado gracias al despliegue real

La **exención de loopback** que liberaba el acceso local sin key era una
**puerta abierta detrás de un proxy**: el túnel Cloudflare entra por
`127.0.0.1`, así que cualquier página web podía alcanzar el cockpit. Se
añadió `MEGADAEMON_REQUIRE_API_KEY=1`, que fuerza la key **también** para
loopback. Sin ese flag, el control de acceso remoto era decorativo.

### 2.4 La API key

La key **no vive en el repo** (la *push protection* de GitHub bloquea el push
si aparece, correctamente). Vive **sólo en el dispositivo**:

```
/data/local/tmp/megadaemon/state/megadaemon_api.key     (permisos 600)
/data/local/tmp/megadaemon/megadaemon.env                (permisos 600)
```

Para verla en el propio dispositivo:

```bash
su -c 'cat /data/local/tmp/megadaemon/state/megadaemon_api.key'
```

**Rotación** (sin tocar el repo):

```bash
KEY="mdaemon_sk_live_$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')"
su -c "printf '%s\n' '$KEY' > /data/local/tmp/megadaemon/state/megadaemon_api.key"
su -c "chmod 600 /data/local/tmp/megadaemon/state/megadaemon_api.key"
su -c "sed -i 's|^MEGADAEMON_API_KEY=.*|MEGADAEMON_API_KEY=$KEY|' /data/local/tmp/megadaemon/megadaemon.env"
su -c 'kill $(pgrep -f "megadaemon-core cockpit")'   # el supervisor lo relanza
```

## 3. Deduplicación y espacio

### 3.1 Los tres entornos y su solape

| Entorno | Ruta | Tamaño |
|---|---|---|
| Termux propio | `/data/data/com.termux/files` | 2,9 GB |
| Antigravity (app) | `/data/data/com.agy/files` | 15,6 GB |
| ├ root embebido | `…/com.agy/files/usr` | 5,1 GB |
| ├ contenedor proot (Debian) | `…/usr/var/lib/proot-distro/containers/debian` | 2,0 GB |
| ├ llama.cpp (libs + server) | `…/usr/lib/libllama-*.so`, `usr/bin/llama-server` | ~12 MB |
| └ modelos GGUF | `…/models` (Qwen2.5-Coder-0.5B, SmolLM2-135M) | 597 MB |

**Duplicación medida por hash**: `node` es **binario idéntico** (sha
`e9ef8eda75ba4ca1`, 51 MB) en Termux y en Antigravity; `cargo`, `git`, `curl`
difieren (versiones distintas). El clon del repo `antigravity-android`
existía **dos veces**: `scratch/aa` (589 MB) y `dumper-web/antigravity-android-repo`
(760 MB) — 1,35 GB por un solo repo.

### 3.2 Acciones aplicadas (solo regenerables y duplicados verificados)

| Qué | Antes | Ahora |
|---|---|---|
| `com.agy/files/megadaemon/rust-core/target` | 675 MB | eliminado (artefactos de build) |
| `com.agy/files/.cargo/registry` + `.cargo/git` | 581 MB | eliminado (caché del registro) |
| `com.agy/files/.npm/_cacache` | 145 MB | eliminado |
| `com.agy/files/usr/var/lib/pkg/cache` | 721 MB | eliminado (caché de paquetes) |
| `scratch/aa` (clon duplicado) | 589 MB | eliminado (se conserva `dumper-web/`) |
| Termux `home/.cargo` + `.cache` + `.npm` | 929 MB | eliminado |
| proot `cache` | 48 MB | eliminado |
| **Libre** | **3,3 GB** | **6,9 GB (+3,6 GB)** |

**No se tocó nada irrecuperable**: los duplicados de contenido (ScummVM por
plataforma, ROM sets, `key.dat`) siguen intactos, y el usuario ya tenía sus
propias herramientas (`scratch/dedupe_delete.py`, `dups_report.txt`,
`archives_report.txt`) para el resto.

### 3.3 Google Drive: subida real

Se instaló **rclone v1.76.0 (arm64)** y se autorizó contra la cuenta
`jfcarrb@gmail.com` con el flujo OAuth real (navegador del móvil → callback
local → token). Detalles no triviales:

* `SSL_CERT_DIR=/system/etc/security/cacerts` es **imprescindible**: Go (rclone)
  no encuentra el almacén de CAs de Android y falla con
  `x509: certificate signed by unknown authority`.
* El `client_id` compartido de rclone está siendo retirado en 2026 (aviso del
  propio rclone): conviene crear un `client_id` propio en Google Cloud.

**Espacio en Drive**: 5 TiB totales, **3,84 TiB libres** ✓ (coincide con lo
que indicabas).

Subida en curso (log en `/sdcard/megadaemon-drive.log`):
`Backup_Android/grandes/` (los 10 ficheros >500 MB) → `Backup_Android/Download/`
(árbol completo, 44 GB) → `Backup_Android/DCIM/`. Se usa `rclone copy`
(**nunca `move` ni borrado**); la originals sólo pasan a
`_IA_Cuarentena` tras `rclone check` successful.

## 4. Estado de las demás fases

| Fase | Estado en el dispositivo |
|---|---|
| 1 — Auditoría | ✅ completa (inventario de 24 módulos Magisk, LSPosed, NPU, Wi-Fi, RAM, CSC) |
| 2 — Limpieza | ✅ +3,6 GB; subida a Drive en curso; cachés Android ya estaban vacías (8 KB) |
| 3 — megadaemon demonio | ✅ supervisor Magisk + /health + /metrics + rotación de log |
| 4 — Remoto + API key | ✅ túnel Cloudflare + key obligatoria + 401/200/403 verificados |
| 5 — Antigravity | ⏳ parcheado y funcionando (2.11.0 con `liblse_compat.so` precargado); falta el catálogo de parches en el dispositivo |
| 6 — NPU | ✅ informe publicado (`docs/npu-research.md`); el NPU real es NNAPI INT8 |
| 7 — Repo | ✅ 6 commits publicados en `main` |
| 8 — Catálogo Antigravity | ✅ `docs/antigravity-ecosystem-catalog.md` (26 repos verificados) |
| 9 — UN1CA | ⚠️ **el dispositivo no es UN1CA-compatible**: UN1CA 3.x parte de S22 y da soporte a A52s/A73/M52; el S21 Ultra **no está en la lista oficial** → requiere port no oficial |
| 10 — CSC/OMCDecoder | ⏳ pendiente de aplicar (el decodificador está identificado: `soulr344/OMCDecoder`) |
| 11 — Kernel monitor mode | ⏳ el chip es **Broadcom** ⇒ Nexmon es el camino técnico; el kernel stock Samsung no expone monitor mode |
| 12 — Migración | ✅ script `deploy/migrate_to_un1ca.sh` (idempotente, reversible, con backup + SHA-256 + subida a Drive) |
| 13 — Evasión de root | ⚠️ **DenyList está DESACTIVADA** (`denylist=0`) y hay 9 módulos de ocultación instalados a la vez (posible conflicto) — requiere ordenar y verificar |
| 14 — Backup | ✅ Swift Backup ya tiene `jfcarrb@gmail.com` con schedule activo; + backup de particiones con el script |
| 15 — Flash | ⛔ **no ejecutado** (irreversible; requiere confirmación explícita) |
