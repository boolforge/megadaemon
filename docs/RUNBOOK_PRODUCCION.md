# RUNBOOK DE PRODUCCIÓN — la instancia local del usuario (Samsung S21 Ultra = MAESTRA)

> **Estado de este documento**: 2026-11-21, contra `main` = `eec5ecb`.
> Todo lo que sigue está **medido** (log del CI, `DoH`, `curl` real, código
> leído en `origin/main`) o marcado explícitamente como **[NO VERIFICADO]**.
> **Ninguna credencial aparece aquí**: sólo rutas y `sha16`.

---

## 0. TL;DR — qué está roto y qué hay que hacer (en este orden)

| # | Qué | Dónde | Por qué (medido) |
|---|---|---|---|
| 1 | **El móvil no tiene canal público** | móvil | El `quick tunnel` de `instance.env` (`berlin-too-advice-point.trycloudflare.com`) da **NXDOMAIN** en DoH ⇒ el `cloudflared` del móvil está muerto |
| 2 | **El daemon no encuentra su propia key** | móvil | El cockpit busca la key en `/data/user/0/com.agy/files/megadaemon/state` (`cockpit_server.rs:27`) y el instalador la deja en `/data/local/tmp/megadaemon/state` ⇒ **403 para toda la flota**, con la key correcta en la mano |
| 3 | **El binario del móvil es viejo** | móvil | `phone_finalize.sh` tenía el release **fijado** a un binario ya retirado ⇒ instalaba un daemon que no era el del repo |
| 4 | **No hay URL estable para los agentes** | PC | El `quick tunnel` **regenera el hostname en cada reinicio** ⇒ la URL de `instance.env` caduca sola |
| 5 | **`main` no compilaba** (ya arreglado) | repo | 6 errores de compilación tras los merges de flota ⇒ **los 4 jobs del CI en rojo y ningún ARM64 publicado** desde las 14:51 |

Los pasos **1–3** se resuelven con **un pegado en el móvil** (paso A).
El paso **4** se resuelve en el **PC del usuario** (paso B).

---

## 1. La cadena (y quién es quién)

```
  ┌───────────────────────── MÓVIL (S21 Ultra, MAESTRA) ─────────────────────────┐
  │  megadaemon-core (cockpit :9090)  ──►  cloudflared (quick tunnel)            │
  │        ▲ state_dir = $MEGADAEMON_STATE_DIR                                 │
  │        │  (por defecto /data/user/0/com.agy/files/megadaemon/state)          │
  │  megadaemon_api.key  (600, el daemon DEBE encontrarla)                      │
  └───────────────┬──────────────────────────────────────────────────────────────┘
                  │ HTTPS (túnel rápido: URL que rota)  ──┐
                  │ HTTPS (relay del PC: URL estable)  ───┤
                                                      ┌────▼─────────────────────┐
                                                      │  PC del usuario (caddy)  │
                                                      │  mdtmfnc.duckdns.org:443 │
                                                      │  cert DNS-01 + relay al  │
                                                      │  móvil por beacon LAN    │
                                                      └────┬─────────────────────┘
                                                           │
                                                  ┌────────▼─────────┐
                                                  │  agentes (aquí)  │
                                                  │  /workspace/md-* │
                                                  └──────────────────┘
```

**El sandbox (donde corre este agente) es sólo EGRESS**: medido el 2026-11-21,
un `fetch` externo contra un listener en el puerto 80 de esta máquina **no
llega** (`web_fetch` falla y el `curl` local sale con 000) ⇒ **el relay no puede
vivir aquí** ⇒ vive en el PC del usuario (paso B).

---

## 2. PASO A — el móvil, en un pegado (Termux, con root)

El repo ya trae el script que hace todo lo del móvil:
`deploy/phone_finalize.sh` (credencial → binario → CSC legal → reinicio →
`/health` → túnel → mediciones).

```bash
# 1) bajar el script (en Termux, con root)
curl -sL https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/phone_finalize.sh \
     -o /data/local/tmp/megadaemon/phone_finalize.sh
su -mm -c 'sh /data/local/tmp/megadaemon/phone_finalize.sh'
```

Qué hace y **qué Accepted gate** mirar en su log
(`/data/local/tmp/megadaemon/phone_finalize.log`):

| Paso del script | Qué resuelve | Gate (texto en el log) |
|---|---|---|
| **0) credencial** | copia `megadaemon_api.key` a **los dos** state_dirs (`/data/local/tmp/megadaemon/state` **y** `/data/user/0/com.agy/files/megadaemon/state`), permisos 600 | `api/status local con la key: 200 (desbloqueado)` |
| **1) binario oficial** | descarga el **último release** + verifica su `.sha256` + `mv` atómico + `chmod 755` **después** del `mv` + reinicia **sólo** el daemon | `instalado (sha256 verificado)` |
| **2) CSC legal** | repone el valor con aviso (`RecordingAllowedByMenu,disclaimer_latin`) — **sin** el aviso la grabación nativa automática y silenciosa es delito en España (art. 197 CP) | `cscfeature` con aviso |
| **3) daemon + túnel** | reinicia el daemon, mira `/health`, y **relanza `cloudflared` si el túnel está muerto (530/000)** | `health local: 200` |
| **4) mediciones** | publica CSC, sha256 del binario, disco y RAM | `binario sha256: …` |

### 2.1 Las tres comprobaciones que YO haría después (sin el pegado)

```bash
# (a) ¿el daemon tiene la key donde la mira?
su -mm -c 'ls -l /data/user/0/com.agy/files/megadaemon/state/megadaemon_api.key \
                      /data/local/tmp/megadaemon/state/megadaemon_api.key'

# (b) ¿con qué state_dir está corriendo de verdad?  (el ENV manda sobre el fichero)
su -mm -c 'for p in $(pidof megadaemon-core); do tr "\0" "\n" < /proc/$p/environ | grep -E "^MEGADAEMON_(STATE_DIR|API_KEY)=" | sed "s/^/  /"; done'
#     si no sale MEGADAEMON_STATE_DIR ⇒ el daemon usa /data/user/0/com.agy/.../state
#     si sale MEGADAEMON_API_KEY=... ⇒ el ENV manda (v16): el fichero se sincroniza,
#        pero si el ENV lleva una key VIEJA, el 403 vuelve (es el P0 de disponibilidad de v16)

# (c) ¿responde?
su -mm -c 'curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:9090/health'      # 200
su -mm -c 'curl -s -o /dev/null -w "%{http_code}\n" -H "X-API-Key: <KEY>" http://127.0.0.1:9090/api/status'   # 200
```

### 2.2 Por qué el 403 **NO** es "key caducada"

Medido: `api_key.rs` (v16) resuelve la credencial así —
`env` **si tiene ≥16 caracteres** ⇒ manda el env; si no, el fichero
`<state_dir>/megadaemon_api.key`; y si el env y el fichero **discrepan**, el
env gana y **sincroniza** el fichero ⇒ una sola fuente de verdad.
El 403 que ve el enjambre (`cockpit_server.rs:1031` → `api_key::forbidden_body`)
incluye **la huella de la key presentada (`sent_key_fp`) y la de la vigente**
(`key_rotated_at`) ⇒ **dice en un vistazo si la credencial es la misma**.

⇒ **Interpretación del 403** (copiar/pegar el JSON del 403):

| Lo que dice el 403 | Significado real | Acción |
|---|---|---|
| `"reason": "el daemon no tiene ninguna key configurada"` + `key_path` | el daemon **no tiene** key (su `state_dir` está vacío o el env no la trae) | paso A.0 |
| `"sent_key_fp": "…" == "key_rotated_at"` coherente con la key que mandas | la key es la vigente pero **no llega** al daemon (otra instancia, otro móvil, otro `state_dir`) | mira la `key_path` del JSON |
| `sent_key_fp` distinto del que esperas | estás con una key **caducada** (rotada) | copia la vigente (sólo `sha16` en el tablón) |
| `403` **sin** JSON del daemon | **no es el daemon**: es un proxy/relay que corta | ver §5 |

---

## 3. PASO B — el PC del usuario: la URL estable (para los agentes)

El `quick tunnel` **rota el hostname en cada reinicio** ⇒ los agentes se
quedan ciegos. La URL estable es el **relay del PC**:

```bash
# en el PC del usuario (Linux/macOS con sudo)
git clone https://github.com/boolforge/megadaemon.git && cd megadaemon
sudo ./deploy/relay-pc.sh --diagnostico   # ¿ve el beacon LAN del móvil? ¿hay caddy?
sudo ./deploy/relay-pc.sh --instalar      # instala caddy + cert DNS-01 + reverse-proxy
```

* **Necesita**: el token de DuckDNS en `~/.mavis/duckdns.token` (0600) — *no*
  se escribe en el repo ni en el chat.
* **Descubre el móvil** por el **beacon LAN** (UDP 9099, `megadaemon-beacon/v1`):
  si no lo ve, se puede fijar `MEGADAEMON_PHONE_IP=<ip-del-móvil>`.
* **Resultado**: `https://mdtmfnc.duckdns.org` ⇒ URL estable, con TLS válido.

Medido el 2026-11-21: `mdtmfnc.duckdns.org` **resuelve** (A → `47.253.152.120`,
DoH 1.1.1.1, Status 0) pero **no responde nada en 80/443** ⇒ el relay **no está
levantado** ⇒ ése es el lado PC que falta.

### 3.1 Verificación del relay (desde cualquier sitio)

```bash
curl -sS -o /dev/null -w "health: %{http_code}\n" https://mdtmfnc.duckdns.org/health
curl -sS -o /dev/null -w "status: %{http_code}\n" -H "X-API-Key: <KEY>" https://mdtmfnc.duckdns.org/api/status
```

| Resultado | Significado |
|---|---|
| `health: 200` | el relay + el móvil están vivos |
| `000` | **no resuelve o no hay ruta**: no es el móvil (comprobar DNS primero) |
| `530` | el relay está vivo pero **el origen no está conectado** (el móvil caído o sin túnel) |
| `401/403` **con JSON** | el móvil responde: es la credencial (§2.2) |
| `401/403` **sin JSON** | el corte es del relay, no del daemon |

---

## 4. PASO C — el binario: puerta de aceptación antes de tocar el móvil

El CI publica el ARM64 con su `.sha256` (asset `megadaemon-core-android-arm64`).
**Antes** de reiniciar el móvil, se pasa la puerta de aceptación
(`tools/acceptance_release.sh`, nueva en este commit):

```bash
curl -sLO https://github.com/boolforge/megadaemon/releases/download/<TAG>/megadaemon-core-android-arm64
curl -sLO https://github.com/boolforge/megadaemon/releases/download/<TAG>/megadaemon-core-android-arm64.sha256
chmod 755 megadaemon-core-android-arm64
tools/acceptance_release.sh ./megadaemon-core-android-arm64 arm64
```

Qué comprueba (todo mecánico): sha256 contra el `.sha256` publicado, cabecera
ELF `aarch64`, que no es un stub (>200 KB), y **permiso de ejecución**.
Sale `PUEDE desplegarse` ⇒ se puede reiniciar el móvil.

Medido (2026-11-21, release `vd148c95`): el binario **Linux x86_64** exige
**glibc ≥ 2.39** (`weak version GLIBC_2.39 not found`) ⇒ en un Linux de
escritorio más antiguo **no arranca**: usar el ARM64 en el móvil o un binario
estático.

---

## 5. Tabla de diagnóstico (síntoma → causa → acción)

| Síntoma | Causa probable | Dónde se mira | Acción |
|---|---|---|---|
| `403` + JSON `forbidden` | credencial (env vs fichero, o fichero en otro `state_dir`) | `api_key.rs` (`load_with_env`) | §2.1 (a) y (b) |
| `403` sin JSON | no es el daemon (proxy/relay) | respuesta cruda | §3.1 |
| `000` / `curl: (6)` | no resuelve o no hay ruta | `getent hosts <dominio>` + DoH | si NXDOMAIN ⇒ falta el relay (paso B) |
| `530` | relay vivo, origen caído | `relay-pc.sh --diagnostico` | reiniciar el móvil |
| `401` | la ruta pide token de sesión, no API key | la página incrusta el token | abrir la UI en el móvil, no desde el PC |
| `/health` = 200 pero `/api/status` = 403 | el daemon vive y la credencial no | `sent_key_fp` del 403 | §2.2 |
| binario instalado pero el daemon no arranca | **sin permiso de ejecución** (`chmod 755` DESPUÉS del `mv`) | `ls -l` | re-hacer el `mv` + `chmod 755` + reiniciar |
| el móvil va JUSTO de RAM (10,2/10,5 GB) | polling agresivo o procesos extra | `dumpsys meminfo` | nada de bucles de polling: todo lo pesado va a `tools/` del PC |

---

## 6. Rollback

| Qué | Cómo |
|---|---|
| binario | el script deja `$PREFIX/megadaemon-core.prev` ⇒ `mv` de vuelta + `chmod 755` + reiniciar |
| CSC legal | `touch /data/adb/modules/megadaemon_csc_recording/disable` + reiniciar |
| credencial | volver a copiar la key a los dos state_dirs (`phone_finalize.sh` paso 0) |
| relay (PC) | `sudo systemctl stop caddy` (el dominio queda NXDOMAIN ⇒ los agentes ven 000, que es honesto) |

---

## 7. Qué **NO** se ha podido verificar desde aquí (honestidad)

* **[NO VERIFICADO]** que el móvil tenga ahora el binario nuevo, la key copiada
  o el túnel vivo: **no hay LAN ni ADB desde el sandbox** (el proxy transparente
  hace que `connect()` a IPs privadas devuelva "OK" sin que haya nada ⇒ falsos
  positivos). Hace falta un nodo con LAN (el PC del usuario o el nodo externo).
* **[NO VERIFICADO]** el estado del CI tras `eec5ecb` (fix del binario) en el
  momento de escribir esto: se estaba ejecutando.
* **[MEDIDO]** que el sandbox es sólo egress ⇒ el relay **no** puede vivir aquí.
* **[MEDIDO]** que el `quick tunnel` de `instance.env` está en NXDOMAIN ⇒ a la
  espera del paso A.3.

---

## 8. Comandos de una línea para el tablón (3 líneas de estado)

```bash
# estado del móvil (desde el PC con adb)
adb shell 'su -mm -c "curl -s -o /dev/null -w \"%{http_code}\" http://127.0.0.1:9090/health"'
# huella de la key (NUNCA la key): sha16
printf '%s' "$KEY" | sha256sum | cut -c1-16
# estado del repo
git -C /ruta/megadaemon log --oneline -1 && git -C /ruta/megadaemon status --short
```
