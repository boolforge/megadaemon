# RECUPERAR EL MÓVIL — 2026-11-23

> El S21 Ultra del usuario está **CAÍDO**.  Este documento es el plan de
> recuperación, con **un comando** por vía.  Todo lo que hay aquí está
> **medido** (2026-11-23, 07:45) o marcado `NO VERIFICADO`.

---

## 1. Diagnóstico (medido)

| # | Hecho | Cómo se ha medido |
|---|---|---|
| 1 | `mdtmfnc.duckdns.org` resuelve a **47.253.152.120** | DoH de Cloudflare (`cloudflare-dns.com/dns-query`) |
| 2 | Mi IP de salida del sandbox es **47.253.4.207** | `api.ipify.org` |
| 3 | ⇒ el A record apunta al **NAT del datacenter**, **no** a casa | 1 vs 2: mismo bloque 47.253.x.x, y la IP pública de casa no puede ser la del datacenter |
| 4 | ElDominio estable (dominio → router → PC → móvil) está **roto** | consecuencia de 3 |
| 5 | Desde el sandbox el móvil es **inalcanzable** | `curl` al túnel ⇒ `http=000`; `connect()` a IPs privadas "acepta y resetea" ⇒ falso positivo del proxy de salida |
| 6 | El **A record lo tiene que publicar el MÓVIL** (su IP pública SÍ es la de casa) | `docs/RESCATE_20261122.md` §2 punto 7 + el propio instalador (`instalador-megadaemon.sh`, bloque "5b) DuckDNS: A record desde el movil") |
| 7 | La release ARM64 **existe** y es descargable | `HTTP 200` en `releases/latest/download/megadaemon-core-android-arm64` + `.sha256` oficial |
| 8 | El token de DuckDNS **existe** en el sandbox | `/workspace/md-secrets/duckdns.token` (36 B, 0600) — **no se ha usado**: desde aquí publicaría la IP del datacenter |

> ⚠ **NADIE debe llamar a `duckdns.org/update` desde un nodo/sandbox**: eso
> es exactamente lo que secuestró el dominio (documentado en
> `docs/RESCATE_20261122.md` §1.5).  El `OK` de la API **no prueba nada**:
> hay que **verificar por DoH** después.

---

## 2. Las tres vías (una sola basta)

### VÍA A — Desde el PC (con el móvil por USB o ADB TCP en la LAN)  ⭐ recomendada

Un comando.  Sube el instalador y el token al móvil, lo ejecuta **como
root** y **verifica** desde el PC (que tiene la red de casa de verdad):

```bash
sh deploy/phone-recover-from-pc.sh
```

Variantes:

```bash
sh deploy/phone-recover-from-pc.sh --diagnostico            # sólo informa
sh deploy/phone-recover-from-pc.sh --serie=192.168.1.124:5555
sh deploy/phone-recover-from-pc.sh --token-ruta ~/.mavis/duckdns.token
```

Qué hace, en orden:

1. Detecta el móvil (`adb devices` → USB; si no, `adb connect 192.168.1.124:5555`).
2. Diagnostica: ¿root? ¿daemon vivo? ¿`channel.json` publicado?
3. Sube `deploy/instalador-megadaemon.sh` a `/data/local/tmp/megadaemon/`.
4. Sube el **token de DuckDNS** (0600) para que el móvil pueda publicar
   el A record **con su IP real**.
5. Ejecuta en el móvil, como root:
   `su -mm -c 'sh /data/local/tmp/megadaemon/instalador-megadaemon.sh --instalar --movil'`
   ⇒ baja el ARM64 (sha256 verificado), unifica la key, instala el trío
   `service.d` (supervisor + cloudflared + watchdog), escribe la URL del
   túnel en los 3 sitios, publica el canal y **actualiza DuckDNS desde el
   móvil + lo verifica por DoH**.
6. **Verifica**: A record por DoH, `/health` por el dominio y por el
   túnel rápido, y el estado del daemon.

**No reinicia el móvil y no borra nada.**

### VÍA B — Desde el móvil (Termux), sin PC

```sh
pkg install curl -y >/dev/null 2>&1
mkdir -p /data/local/tmp/megadaemon
curl -fsSL https://raw.githubusercontent.com/boolforge/megadaemon/main/deploy/instalador-megadaemon.sh \
  -o /data/local/tmp/megadaemon/instalador-megadaemon.sh
su -mm -c 'sh /data/local/tmp/megadaemon/instalador-megadaemon.sh --instalar --movil'
```

* El token de DuckDNS tiene que estar en el móvil, en
  `~/.mavis/duckdns.token` **o** en `/data/local/tmp/megadaemon/duckdns.token`
  (0600).  Sin él, el móvil publica **sólo** el túnel rápido (que cambia
  de hostname en cada reinicio ⇒ la flota hay que buscarlo).
* Si el móvil no tiene internet (o no hay `curl`), el camino corto es:
  `su -mm -c 'sh /data/local/tmp/megadaemon/phone-recover.sh'` (ya está en
  el móvil; es idempotente y **no reinicia**).

### VÍA C — El relay del PC (si el móvil está vivo pero su daemon, no)

```bash
sudo ./deploy/relay-pc.sh --instalar      # Caddy en el PC: dominio → PC → móvil:3000
sudo ./deploy/relay-pc.sh --diagnostico
```

El PC publica el A record (también está en casa ⇒ su IP pública es la
correcta) y hace de relay al puerto 3000 del móvil en la LAN.
**No arregla un daemon muerto**: sólo el camino.

### Si el móvil está APAGADO

Ninguna vía remota lo enciende.  Encenderlo (y conectarlo al cargador)
basta: con el `service.d` instalado, el daemon, el supervisor y el
túnel **levantan solos** en el arranque.  Después, Vía A o B.

---

## 3. Qué hago yo en cuanto vuelva

1. **Verificar desde fuera** (el sandbox no llega al móvil: se verifica con
   un fetch externo y con el A record por DoH).
2. **Ejecutar los 9 puntos NO VERIFICADO** de `docs/MITM-SQLITE-Y-PROXY.md`
   §4 (V1–V9), cada uno con su comando exacto: el bind-mount de la DB, la
   captura, el re-merge, el proxy `/v1internal:generateContent`, la traza
   del modelo, la fusión con la GUI y el **fichero falso** del `.so`.
3. **Soltar el aviso de "NO VERIFICADO"** de la entrega del MITM.

---

## 4. Coordinación (hecho)

* **Jefe (root)**: informado del diagnóstico (dominio secuestrado, móvil
  inalcanzable, el A record sólo desde el móvil/PC) y de que **nadie
  publique DuckDNS desde un nodo**.  Preguntado por el dueño del build
  ARM64 y del push, para no pisarle.
* **Otro agente (instrumentación de Antigravity)**: avisado de lo mismo y
  preguntado por un canal vivo hacia el móvil (ADB USB/TCP, KDE Connect) y
  por un ARM64 recién compilado.  Acordado: **nadie hace push al móvil
  sin avisar** (no quiero dos pushes al mismo dispositivo).
