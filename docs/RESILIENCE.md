# Resiliencia de MegaDaemon: cómo se vuelve a encontrar el móvil

> Estado: **medido** en el dispositivo real (Galaxy S21 Ultra, Exynos 2100,
> Android 14, Magisk Kitsune) y en el sandbox. Todo lo que aquí va lleva la
> fecha o el SHA con el que se comprobó.

## 0. El problema, medido

El 2026-11-21 el enjambre llevaba horas sin poder hablar con el móvil. La
cadena de causas, todas verificadas:

| # | Síntoma | Causa real (medida) |
|---|---|---|
| 1 | `/channel.json` servía `"quick": ""` | `98-cloudflared.sh` escribía la URL en `LOG_DIR` y el daemon la leía en otro `state_dir` ⇒ nunca se fusionaba |
| 2 | El estado estaba disperso (key, `tunnel.url`, `peers.json`, `board.sqlite` en sitios distintos) | El supervisor hacía `source` del env **tarde** y nunca exportaba `MEGADAEMON_STATE_DIR` ⇒ los hijos caían al default del crate (`/data/user/0/com.agy/...`), que **no existe** en el móvil |
| 3 | El canónico `mdtmfnc.duckdns.org` daba `000` | El A record apuntaba a `47.253.152.120` = **IP pública del sandbox** (NAT del datacenter), no a la del PC de casa. `duckdns.org/update` pone el registro a la IP de **quien llama** |
| 4 | El shell root persistente (F3) no arrancaba | `ensure_shell_service` usaba `$P`, variable **inexistente** en el supervisor (es `$PREFIX`) ⇒ el test `[ -x "$P/md-shell-serve" ]` era siempre falso |
| 5 | Los arreglos de la UI no llegaban al móvil | `resolve_template()` buscaba en `/data/user/0/com.agy/.../web/` ⇒ no lo encontraba ⇒ caía al `include_str!` embebido ⇒ hacía falta **recompilar** el binario |

Cada uno de estos cinco está corregido y verificado en `main` (ver §4).
El punto que queda abierto **no es de código**: es que el A record del
canónico lo tiene que publicar el PC de casa.

## 1. Los cuatro canales, y cuál manda cuando

El objetivo de diseño es que **la pérdida de un canal no ciegue al
enjambre**. Cada canal tiene un requisito distinto, y no se solapan:

| # | Canal | Ruta | Requiere internet | Requiere tercero | Para qué |
|---|---|---|---|---|---|
| 1 | **LAN directo** | `http://<IP-del-móvil>:9090` | **No** | No | Canal de verdad: el móvil y el PC están en la misma red. El daemon ya escucha en `0.0.0.0` |
| 2 | **Quick tunnel** | `https://<algo>.trycloudflare.com` | Sí | Cloudflare | Cuando se sale de casa (móvil con datos) |
| 3 | **Canónico DuckDNS** | `https://mdtmfnc.duckdns.org/channel.json` | Sí | DuckDNS | URL estable, la que se publica y la que remembers |
| 4 | **Respaldo: paste + repo** | `raw.githubusercontent.com` / paste | Sí | GitHub | Último recurso: el lector oficial cae aquí |

**Regla de degradación**: si el canónico responde `200` pero el lector
oficial cae al respaldo, es un **fallo**, no un aviso ⇒ `tools/verify_canal.sh`
lo marca como `FAIL`. "CANAL OK" significa de verdad que el enjambre
encuentra al móvil.

### 1.1 El canal LAN (nuevo, y el que de verdad resuelve el problema)

Hasta ahora, la única forma de alcanzar el móvil era por internet y por
terceros. Con internet caído, o con DuckDNS mal configurado, el móvil
**desaparecía** aunque estuviera a dos metros del PC.

Ahora el daemon emite un **beacon UDP en difusión** (`rust-core/src/lan.rs`):

- Puerto `9099` (UDP), cada 15 s.
- Contenido: `{"kind":"megadaemon-beacon/v1","id":...,"ips":[...],"ip":...,"port":9090,"url":"http://192.168.1.124:9090","ts":...}`.
- **Sin secretos**: ni la API key, ni el token de sesión (test
  `el_beacon_no_lleva_ningun_secreto`).
- Se escribe además en `state/lan.json`.

Y el lado del PC/sandbox:

```sh
python3 tools/lan_discover.py --probe     # descubre y comprueba /health
megadaemon-core-bin md-lan-listen --probe  # equivalente en Rust
```

Salida real:

```
móvil encontrado por LAN: s21-ulta
  IPs: ['192.168.1.124']
  URL: http://192.168.1.124:9090
  /health -> HTTP 401 (vivo)
```

**Límite honesto**: la difusión UDP **no cruza routers**. Sirve en la misma
subred (casa). Para salir a casa hace falta el canal 2/3.

## 2. Los tres que se auto-reparan solos

| Componente | Qué hace | Quién lo relanza |
|---|---|---|
| `99-megadaemon.sh` (Magisk `service.d`) | Supervisa el daemon: si `/health` falla dos veces, lo reinicia; si el binario nuevo no arranca, **auto-rollback** al `.prev` | Él mismo (bucle) y Magisk al arrancar el móvil |
| `98-cloudflared.sh` (Magisk `service.d`) | Levanta **un** `cloudflared` con `setsid nohup` (independiente del daemon: reiniciar el daemon **no** tumba el túnel) | Magisk al arrancar; el watchdog si muere |
| `tunnel-watchdog.sh` | Sondea el túnel. Semántica HTTP: `530/000/502/503/504` = muerto ⇒ relanza; `401/403` = **vivo** (el daemon exige credencial) ⇒ no toca nada. No relanza con `load1 ≥ 8` (el móvil está justo de RAM) | Se relanza a sí mismo |
| `channel-publish` | Publica `state/channel.json` con el `quick` vivo | `98-cloudflared.sh` al subir el túnel |

Los cuatro están en `/data/adb/service.d/` ⇒ **sobreviven a un reinicio del
móvil** sin intervención manual.

## 3. Recuperación en un comando

```sh
su -mm -c 'sh /data/local/tmp/megadaemon/phone-recover.sh'
```

`deploy/phone-recover.sh` es **idempotente** y **conservador**:

- **NO** reinicia el móvil.
- **NO** reinicia el daemon si está vivo y sano (sólo lo relanza si no
  responde).
- **NO** toca la API key, el token ni ningún secreto: sólo crea lo que falta.
- **NO** toca el binario salvo que se pase `--install <ruta>`.
- **NO** mata el daemon nunca al relanzar el túnel (sólo mata `cloudflared`).

Qué garantiza al terminar:

1. `state/` existe y es el `state_dir` real (declarado en `megadaemon.env`).
2. `megadaemon.env` declara `MEGADAEMON_STATE_DIR` y `MEGADAEMON_LISTEN`.
3. Supervisor + cloudflared + watchdog están en `service.d`.
4. El daemon está vivo y `/health` responde.
5. El túnel está vivo y su URL está en `state/tunnel.url`.
6. `state/channel.json` tiene el `quick` correcto.
7. `state/lan.json` está escrito (beacon LAN).

Opciones: `--diagnostico` (sólo informe, no cambia nada), `--install <ruta>`,
`--no-tunnel` (recuperación sin internet).

## 4. Qué se corrigió (con SHA)

| Corrección | Commit | Qué arregla |
|---|---|---|
| Raíz común del `state_dir` | `f99819e` | El env se lee en la cabecera del supervisor; los scripts de Magisk lo leen por su cuenta; `--state-dir` explícito al publicar |
| Shell root persistente restaurado | `a53fa3f` | `ensure_shell_service` volvía a existir, y con `$PREFIX` (con `$P` **nunca** arrancaba) |
| Export del `state_dir` incondicional | `a53fa3f` | Sin fichero de env, el daemon caía a un directorio inexistente |
| `quick` al `/channel.json` | `f99819e` + tests en `2e40190` | La URL viva llega al canal aunque nadie publique |
| Verificador de una pasada | `2e40190` | `tools/verify_canal.sh`: DNS (con DoH), canónico, payload, túnel, lector. `FAIL` si el canónico responde 200 y el lector cae al respaldo |
| Rutas derivadas del `state_dir` | `b9dbb95` | `uploads`, `extracted`, `web`, `re_pipeline` ⇒ la plantilla desplegada **sí** se usa (antes hacía falta recompilar) |
| `providers.json` en el `state_dir` | `115dd29` | Las API keys BYOK estaban en el dir privado de la app, **fuera** de la zona de backup |
| Tablón por candidatas | `38d232a` | `board.sqlite` se buscaba un nivel más abajo de donde estaba |
| Un solo escritor del DNS | `c7c0e2c` | `duckdns-updater` ya no toca el canónico (salvo `--allow-canonical`) |
| Canal LAN (beacon) | este bloque | Descubrimiento del móvil **sin internet** |

## 5. Lo que queda en manos del usuario (y por qué no es código)

Tres cosas no las puede hacer ningún agente, porque dependen del entorno
físico del usuario:

1. **El A record del canónico.** Lo publica `deploy/relay-pc.sh` desde el
   PC de casa, con la IP pública de casa. Si lo publica el sandbox, el
   dominio apunta al datacenter y el relay es inalcanzable. Por eso
   `duckdns-updater` (en el móvil o en el sandbox) ya no toca el canónico.
2. **El port-forward 443** del router al PC (Caddy usa DNS-01 ⇒ no hace
   falta abrir el 80).
3. **El token OAuth de Drive** (0600) para cerrar F1 (>1 GB subido y bajado
   con MANIFEST verificado).

Y una cuarta, opcional pero la más útil para desbloquear el AEAD:

4. **Un export real de Swift Backup** (el `.dat`/SBA1 que genera la app; en
   el móvil hay backups MTP por tipo, que no son el formato). Sin un
   fichero real, el round-trip del AEAD por trozos no se puede cerrar
   — aunque el layout, el KDF, el key-check y el HMAC del payload ya están
   medidos y verdes.

## 6. Comprobación de estado (una sola pregunta)

```sh
bash tools/verify_canal.sh
```

| Salida | Significado |
|---|---|
| `{"ok":true,"fallos":0}` | El enjambre encuentra al móvil por la vía principal |
| `{"ok":false,...,"canonico_http":0}` | El relay del PC no está levantado (o el A record no apunta a casa) |
| `{"ok":false,...,"lector":"ko"}` con `canonico_http:200` | El canónico responde pero el lector no lo usa ⇒ hay que mirar el `canonical` que publica el móvil |
| `{"ok":false,...,"quick":"ko"}` | El móvil no ha publicado el túnel ⇒ `98-cloudflared.sh` no arrancó: mira `state/cloudflared.log` |

Y el atajo, sin internet:

```sh
python3 tools/lan_discover.py --probe
```
