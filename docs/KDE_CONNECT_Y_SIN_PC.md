# KDE Connect y el problema de "no hay PC"

> Estado: **diseño + implementación parcial** en este commit.
> Lo que va marcado como **MEDIDO** está comprobado en este repo o en el
> dispositivo; lo marcado como **ESTIMACIÓN** es una cota razonable **sin
> medir** (y por tanto, no es una promesa). Lo que no se ha podido
> comprobar se dice explícitamente (`NO VERIFICADO`).

---

## 0. El problema, en una frase

Hoy el **canónico** (`https://mdtmfnc.duckdns.org`) lo publica el **PC de
casa** (`deploy/relay-pc.sh`), así que **si el PC está apagado o dormido,
el enjambre se queda ciego aunque el móvil esté encendido y con internet**
(medido el 2026-11-21, ver `docs/RESILIENCE.md` §0 y §5.1: el A record
apuntaba a la IP pública del sandbox y el relay de casa no estaba).

Este documento:

1. define la **escalera de canales** (qué se usa, en qué orden, y qué
   requisito falla en cada escalón),
2. explica cómo encaja **KDE Connect** como protocolo de *descubrimiento +
   identidad + emparejado* **sin duplicar la API** que ya existe,
3. mide (o estima) **latencia y riesgo** por canal,
4. decide qué **extensiones** de KDE Connect merece la pena,
5. fija la **frontera de seguridad** (el PIN),
6. dice qué haría falta para hablar con la **app oficial** (y por qué este
   módulo no lo hace).

Módulos de este commit:

| Fichero | Qué es |
|---|---|
| `rust-core/src/kdeconnect.rs` | Núcleo del protocolo: paquete, framing, identidad, emparejado con PIN, TLS, canal de control (`ping`/`echo`/`md.request`) |
| `rust-core/src/selfsigned_cert.rs` | Certificado X.509 autofirmado (Ed25519, DER escrito a mano, sin crates nuevos) |
| `rust-core/src/bin/kde-discover.rs` | CLI: escucha el descubrimiento, lista vecinos, hace `ping` por el canal cifrado |
| `tools/kde_discover.py` | El mismo lector en Python, sin dependencias, para el PC |

---

## 1. La escalera de canales

La regla de diseño (ya la tenía `docs/RESILIENCE.md` §1) es que **la
pérdida de un canal no ciegue al enjambre**. Cada canal tiene un requisito
distinto, y por eso **no se solapan**: se prueban en orden y el primero que
cumple su requisito gana.

| # | Canal | Ruta | Internet | Tercero | Requisito que debe cumplirse | Si falla, siguiente |
|---|---|---|---|---|---|---|
| **1** | **KDE Connect emparejado (LAN)** | TCP 1716-1764 + TLS, directo IP | No | No | Móvil y par en la **misma subred** + **emparejado con PIN** | 2 |
| **2** | **Beacon LAN + HTTP directo** | `http://<IP-móvil>:9090` | No | No | Móvil y par en la **misma subred** (la difusión UDP no cruza routers) | 3 |
| **3** | **Relay en un nodo siempre activo (VPS)** | `https://<dominio>/…` → túnel inverso → móvil | Sí | DuckDNS/Caddy (en el VPS) | El VPS tiene **IP pública fija** y **el móvil mantiene una conexión saliente viva** | 4 |
| **4** | **Quick tunnel efímero (Cloudflare)** | `https://<algo>.trycloudflare.com` | Sí | Cloudflare | El móvil tiene internet y `cloudflared` vivo (el hostname **cambia en cada reinicio**) | 5 |
| **5** | **Paste + repo** | `raw.githubusercontent.com` / paste | Sí | GitHub | Que **alguien** pegue el estado a mano | — (no hay siguiente) |

### 1.1 Canal 1 — KDE Connect emparejado (nuevo en este commit)

**Qué es.** Un protocolo de descubrimiento + identidad + emparejado, en LAN,
que además transporta un canal de control cifrado.

**Por qué va el primero.** Es el único canal que no depende de internet, de
terceros **ni de que el PC esté encendido**, y el único que aporta
**identidad verificada** (certificado propio + *pinning*) en lugar de
"confiar en que la IP es la del móvil". Un canal HTTP directo sobre LAN
(sólo el beacon) no dice nada de *quién* es el otro extremo: cualquiera que
haga `curl` al puerto 9090 del móvil ve la API.

**Qué da, en concreto:**

| Cosa | Detalle |
|---|---|
| Descubrimiento | UDP en difusión a **1716** (oficial de KDE Connect) y **1776** (enjambre). Datagram JSON plano, cuerpo con `deviceId`, `deviceName`, `deviceType`, `protocolVersion`, `incomingCapabilities`, `outgoingCapabilities`, `tcpPort`, `appName` |
| Identidad | `deviceId` persistente (UUID v4 sin guiones, 32 hex, patrón del oficial `^[a-zA-Z0-9_-]{32,38}$`) + certificado autofirmado propio (CN = `deviceId`, Ed25519) |
| Empareado | `kdeconnect.pair` con **PIN de 8 dígitos** + **confirmación humana obligatoria** + comparación en **tiempo constante**; el PIN **no** se guarda en claro (sólo su digest con sal, y sólo mientras la petición está pendiente) |
| Cifrado | Todo el canal TCP va cifrado con **TLS** (`rustls` + `ring`). El handshake empieza en claro (identidad, como en el oficial) y luego **ambos extremos suben a TLS**: el que **inició** el TCP es **servidor TLS**, el que lo **aceptó** es **cliente TLS** |
| Anti-replay / anti-sustitución | Tras el pareado, el certificado del par queda **pinneado** (SHA-256 del DER en `state/kde/peers.json`). Si el par presenta otro certificado, el TLS **falla**: no hay CA, el pin *es* la confianza |
| Canal de control | `kdeconnect.ping`/`pong`, `kdeconnect.mock.echo`, y `md.request`/`md.response` (§2.2) |

**Coste y límites (honestos).**

* La difusión UDP **no cruza routers**: el canal 1 sólo sirve en la misma
  subred (igual que el canal 2, y está medido en `RESILIENCE.md` §1.1).
* La app oficial de KDE Connect **no** habla con este módulo tal cual (§6).
* El módulo **no** está arrancado por el daemon todavía: existe como
  biblioteca + CLI. El enganche (§7.1) es de una línea en `cockpit_server.rs`
  y **no** se ha hecho aquí para no tocar el daemon que hoy es el origen del
  túnel (ver §7.2).

### 1.2 Canal 2 — Beacon LAN + HTTP directo (ya implementado)

`rust-core/src/lan.rs` emite un beacon UDP en difusión (puerto 9099) con
`id`, `ips` y `port`; el PC lo lee con `tools/lan_discover.py --probe` y
llama a `http://<IP>:9090`. Sin secretos en el beacon (test
`el_beacon_no_lleva_ningun_secreto`).

**Qué le falta al canal 2 frente al 1:** identidad (cualquiera en la LAN
puede suplantar la IP), cifrado (HTTP en claro) y control de quién puede
hablar (la API key protege las rutas, pero el beacon es abierto).

### 1.3 Canal 3 — Relay en un nodo siempre activo (lo que quita la dependencia del PC)

**Este es el canal que resuelve el problema de verdad.** El resto de la
escalera degrada *el acceso*; este elimina *la dependencia*.

**El diseño, en una frase:** el móvil mantiene una **conexión TCP
saliente persistente** contra un **nodo siempre activo con IP pública
fija** (un VPS), y ese nodo **sirve el canónico** y hace de proxy inverso
hacia el móvil a través de ese túnel. Es el quick tunnel de Cloudflare
**invertido y con vida útil de días en vez de horas**.

```
                    ┌───────────────────────────────┐
   navegador ──────►│  VPS (siempre activo, IP fija)│
   (sandbox/PC)     │  Caddy: TLS + canónico        │
                    │  relay: proxy inverso ────────┼──► túnel TCP
                    └───────────────────────────────┘      (saliente,
                                   ▲                       persistente)
                                   │ conexión saliente
                                   │ (la abre el móvil)
                    ┌──────────────┴────────────────┐
                    │  móvil (behind NAT)            │
                    │  daemon: 0.0.0.0:3000/3030/8090/9090
                    └────────────────────────────────┘
```

**Por qué el móvil tiene que poder hablar con un relay remoto aunque el de
casa esté apagado:**

1. **El que tiene que estar siempre vivo es el móvil**, no el PC. El
   móvil es el recurso escaso: es lo que tiene los datos, la pantalla, el
   root. El PC es un portátil que se duerme, se va de casa y se apaga.
2. **Hoy el canónico lo publica el PC de casa** (`deploy/relay-pc.sh`
   escribe el A record de DuckDNS con el token). Con el PC apagado: el
   dominio resuelve a la IP de casa pero **nadie escucha el 443** ⇒
   `000`/`502` para todo el enjambre. Con un VPS, el canónico lo publica
   el VPS, que sí está siempre encendido.
3. **El relay no es un proxy de la API**: es un **tubería de bytes** (§2.3).
   El daemon del móvil sigue siendo **la única implementación** de la API y
   del MCP; el relay sólo mueve bytes. Eso significa que **cambiar de relay
   no cambia la API** y que el canal 3 no añade una superficie de
   mantenimiento nueva.
4. **El relay puede ser tonto y barato**: un proceso, un `Caddyfile`, un
   `channel.json`. No sabe nada del protocolo MegaDaemon.

**Por qué la conexión tiene que ser SALIENTE (del móvil al relay):**

* **El móvil está detrás de NAT**, y a menudo detrás de CGNAT en la red
  móvil: no existe ninguna ruta entrante, ni con port-forward en casa (el
  router de casa no es el router de la SIM). Una conexión **saliente** al
  VPS funciona desde cualquier red, atraviesa cualquier NAT/firewall, sin tocar
  la configuración del router.
* **Es lo mismo que ya funciona**: el quick tunnel de Cloudflare
  (`deploy/98-cloudflared.sh`) funciona **exactamente** porque el móvil
  **marca** hacia el edge de Cloudflare. La variante del canal 3 es el
  mismo patrón con un destino fijo (el VPS) en vez de un destino efímero.
* **El móvil cambia de red** (WiFi de casa ⇄ datos móviles). Una conexión
  saliente se rehace en segundos y el relay sólo ve "un cliente nuevo con
  otra IP". Al revés (que el VPS llame al móvil) exigiría IP fija + NAT
  estático + que el router esté encendido: justo lo que se quiere quitar.
* **El que manda es el móvil**: si el móvil está apagado, el túnel se cae y
  el relay responde 503; cuando vuelve, se rehace solo. No hace falta que
  "nadie" tenga que ir a reiniciar el PC.

**Requisitos del canal 3 (y qué falla si no se cumplen):**

| Requisito | Si falla |
|---|---|
| Un nodo siempre activo con **IP pública fija** (VPS) | No hay canal 3 ⇒ se cae al 4 |
| El móvil puede **salir** a internet (puerto TCP saliente permitido) | No hay canal 3 (pero el 4 tampoco serviría: también necesita salida) |
| El móvil **mantiene viva** la conexión (watchdog en el móvil) | El relay se queda sin destino ⇒ 503 ⇒ se cae al 4 |
| TLS válido en el canónico (Caddy con DNS-01, como ya hace `relay-pc.sh`) | Se puede usar HTTP, pero el navegador/MCP avisará |

**Estado: NO IMPLEMENTADO en este commit.** El módulo `kdeconnect.rs` es la
pieza de *descubrimiento/identidad/emparejado* del canal 1; el túnel
inverso del canal 3 es un componente **distinto** (un `relay` en el VPS + un
cliente saliente en el móvil). La §7.3 dice exactamente qué habría que
escribir y por qué no cabe en este commit.

### 1.4 Canal 4 — Quick tunnel efímero (Cloudflare)

`deploy/98-cloudflared.sh` levanta **un** `cloudflared` con `setsid nohup`
(independiente del daemon: reiniciar el daemon **no** tumba el túnel) y
`deploy/tunnel-watchdog.sh` lo relanza si muere (`530/000/502/503/504` =
muerto; `401/403` = vivo, porque el daemon exige credencial).

**Límite conocido y medido**: el **hostname cambia en cada reinicio** del
túnel ⇒ el canónico sirve la URL nueva y el enjambre tiene que volver a
leerla. Por eso el canal 4 es "de emergencia", no "el canal".

### 1.5 Canal 5 — Paste + repo

El último recurso: alguien pega el estado (`channel.json`, una URL, un
hash) en un paste o lo sube al repo y el lector oficial lo toma. Requiere
intervención humana ⇒ es el final de la escalera, no una degradación
automática.

### 1.6 La escalera en 5 líneas

1. **KDE Connect emparejado en LAN** (0 latencia extra, TLS, identidad, PIN) — si el par no está en la misma subred o no está emparejado, se cae al 2.
2. **Beacon LAN + HTTP directo** (ya implementado) — si no hay subred común, se cae al 3.
3. **Relay en nodo siempre activo** (VPS + conexión saliente del móvil) — si el VPS no está o el túnel no vive, se cae al 4.
4. **Quick tunnel de Cloudflare** (hostname efímero) — si `cloudflared` no vive, se cae al 5.
5. **Paste + repo** — último recurso, requiere a un humano.

---

## 2. Cómo encaja KDE Connect **sin duplicar la API**

### 2.1 El reparto de papeles

| Capa | Quién la pone | Qué es |
|---|---|---|
| **API / MCP** | El daemon (cockpit) | `POST /mcp` + `/api/*` en 3000/3030/8090/9090. **Una sola implementación** |
| **Transporte** | El canal (KDE Connect LAN, relay del VPS, quick tunnel, LAN directo) | Mueve bytes: TLS, framing, descubrimiento |

KDE Connect **no** reimplementa ni duplica ninguna ruta de la API. Aporta
tres cosas que el HTTP plano no tiene:

1. **Descubrimiento** — quién está, sin configurar IP a mano (la IP por
   DHCP cambia).
2. **Identidad** — un `deviceId` estable + un certificado propio, para que
   "el nodo de la esquina" no sea sólo una IP.
3. **Emparejado** — una frontera de confianza explícita (PIN + confirmación
   humana), que es lo que permite que el canal sea **cifrado y abierto a
   la vez**.

### 2.2 El canal de control: `md.request` / `md.response`

Ésta es la pieza que da **acceso remoto bidireccional con latencia mínima**:

```
Nodo A ──(KDE Connect, TLS)──► Nodo B
       "reenvía GET /api/status a TU daemon local"
                                     │
                                     ▼
                              daemon local de B
                                     │
       ◄── "200, { ...cuerpo... }" ─┘
```

* El que pregunta **no habla con la API del otro**: habla con **su propio
  daemon local**, que a su vez pide al daemon del par. La API sigue
  teniendo **un solo dueño** (cada nodo sirve la suya) y **una sola
  implementación**.
* Por el cable viaja **una** petición y **una** respuesta (el cuerpo de la
  respuesta, recortado a 256 KiB), no "toda la API" ni el tráfico de
  streaming.
* El par **no emparejado no puede pedir nada**: `handle_control()` descarta
  el paquete antes de tocar el daemon (test
  `un_md_request_de_un_peer_no_emparejado_se_descarta`).
* El path **tiene que ser relativo** (`/api/status`), el host es siempre el
  del daemon local ⇒ **no hay SSRF** (test
  `el_md_request_rechaza_rutas_absolutas`), el método está en lista blanca
  y el cuerpo tiene tope de tamaño.
* La **API key local no sale del nodo**: se lee de
  `<state_dir>/megadaemon_api.key` y se usa sólo para llamar al daemon
  propio (además, el daemon exime al loopback).

### 2.3 El otro modo posible (NO implementado): túnel de bytes

La alternativa natural es que el canal KDE Connect transporte **bytes en
crudo** hacia el daemon local del par (como hace el relay del VPS). Entonces
`curl http://<par>:9090/mcp` funcionaría tal cual, sin tocar nada.

Se deja **fuera** a propósito:

* el modo `md.request` ya cubre el caso real (peticiones sporadicas del
  enjambre al móvil), con menos superficie y con el trazo de auditoría
  dentro del protocolo;
* un túnel de bytes por el canal KDE Connect multiplicaría los riesgos
  (SSRF, túneles hacia puertos locales arbitrarios) sin ganar nada, porque
  el daemon ya tiene su propio control de acceso.

### 2.4 Por qué el MCP no se duplica

El MCP se expone en `POST /mcp` del daemon. Con el canal 1, un agente que
no esté en la LAN puede pedirle al par que llame a **su** `/mcp` y le
devuelva la respuesta — el mismo endpoint, el mismo servidor, las mismas
herramientas. **No hay una segunda lista de herramientas MCP**: hay un
segundo camino de transporte hacia el mismo servidor.

---

## 3. Latencia esperada y riesgo por canal

### 3.1 Latencia (ida y vuelta del KC de la API, no del RTT puro)

| # | Canal | Latencia esperada | Tipo | Nota |
|---|---|---|---|---|
| 1 | KDE Connect LAN (mismo switch) | **1–5 ms** | ESTIMACIÓN | RTT de LAN; el coste real es el *handshake* TLS la primera vez (1 RTT extra) |
| 1b | KDE Connect LAN (WiFi ↔ Ethernet) | **5–30 ms** | ESTIMACIÓN | WiFi introduce jitter de 1–10 ms en cada sentido |
| 2 | Beacon LAN + HTTP directo | **1–30 ms** (igual que el 1) | ESTIMACIÓN | Sin handshake extra: HTTP en claro |
| 3 | Relay en VPS (VPS en la misma región que el móvil) | **40–120 ms** | ESTIMACIÓN | Suma el RTT móvil↔VPS + el RTT VPS↔cliente, más el TLS del canónico |
| 3b | Relay en VPS (VPS en otro continente) | **150–400 ms** | ESTIMACIÓN | Idem + RTT intercontinental |
| 4 | Quick tunnel de Cloudflare | **60–200 ms** | ESTIMACIÓN | El edge de Cloudflare añade un salto TLS |
| 5 | Paste + repo | **minutos** (horas si hay que esperar a un humano) | MEDIDO por diseño | Depende de una persona |

> **NO VERIFICADO**: ninguna de estas cifras está medida en este repo con
> el móvil real. Lo que **sí** está medido en este repo es el *coste de
>romper*: el A record del canónico apuntaba a la IP del sandbox
> (`RESILIENCE.md` §0, caso 3) y el `quick tunnel` cambia de hostname en
> cada reinicio (`deploy/98-cloudflared.sh`, cabecera). Medir la latencia
> real de cada escalón exige el móvil en la mano: `kde-discover --ping`
> da el RTT del canal 1 y `time curl` el de los demás.

### 3.2 Riesgo por canal

| # | Canal | Expuesto a internet | Credenciales que viajan | Qué rompe | Riesgo principal |
|---|---|---|---|---|---|
| 1 | KDE Connect LAN | No | Ninguna (TLS + pin) | Se cae el emparejado / se cambia el cert | **Bajo**: cifrado + pinning + emparejado. Riesgo: un MITM *durante* el pareado (§5.5) |
| 2 | Beacon + HTTP directo | No (LAN) | API key en la cabecera, en claro por el cable | Nada (es best-effort) | **Medio**: la API key viaja en claro en la LAN y cualquiera en la subred ve las IPs |
| 3 | Relay en VPS | Sí (el VPS es público) | API key (al daemon del móvil, a través del túnel) | El VPS se cae / se paga / lo secuestra alguien | **Medio**: superficie pública (el VPS) + el canónico queda apuntando a un tercero |
| 4 | Quick tunnel | Sí (edge de Cloudflare) | API key | El hostname cambia; Cloudflare puede tumbarlo | **Medio-alto**: el estado de conmutación es externo y efímero |
| 5 | Paste + repo | Sí (GitHub) | Lo que se pegue | Se pierde el paste | **Alto en control**: depende de una persona y puede filtrar secretos si se pega de más |

---

## 4. Extensiones: qué merece la pena y qué no

**Criterio (se aplica a las tres preguntas):** una capacidad entra sólo si
(1) **no amplía la superficie de ataque** más allá de lo que ya implica el
emparejado, (2) el **usuario puede desactivarla** y entender qué hace, y
(3) es **reversible** (se puede revocar el empareado y recuperar el control).

### 4.1 SÍ

| Capacidad | Tipo | Por qué | Riesgo residual | Notas |
|---|---|---|---|---|
| `kdeconnect.battery` | Lectura | El arnés necesita saber si el móvil está cargando o al 3 % antes de lanzar un job de 40 min | Nulo | Sólo lectura, 1 paquete |
| `kdeconnect.notification` | Escritura (móvil→par) | "El agente ha terminado" / "la build ha fallado" sin tener que mirar la pantalla | Bajo: el par puede inyectar notificaciones falsas | Con límite de ritmo y sin datos sensibles |
| `kdeconnect.ping` | Lectura | Latencia y presencia. **Ya implementado** | Nulo | |
| `kdeconnect.mock.echo` | Lectura | Prueba de que el canal está vivo de extremo a extremo. **Ya implementado** | Nulo | |
| `md.request` / `md.response` | Proxy de API | Acceso remoto bidireccional sin duplicar la API. **Ya implementado** | §2.2 | Es la extensión propia, no del oficial |
| `kdeconnect.share.request` (ficheros) | Escritura | Pasar un log o un artefacto al par | Medio: escritura en disco | Preferible usar el `uploads/` que ya existe, con su manifiesto |

### 4.2 NO (y por qué)

| Capacidad | Por qué NO |
|---|---|
| `kdeconnect.mousepad.*` / `kdeconnect.digitizer.*` (**control remoto de input**) | Da control de la pantalla y del teclado del móvil a otro equipo: es, funcionalmente, un **shell remoto**. El harness **ya tiene** shell root (SSH nativo, `sshd.rs`), con su propia auditoría y su propio policy. Duplicarlo por un canal nuevo multiplica el peor caso |
| `kdeconnect.runcommand.*` | Ejecución de órdenes remota. Mismo argumento: ya existe el shell root; además el oficial **no pide PIN** para runcommand (el plugin se activa al emparejar) ⇒ un par comprometido ejecuta órdenes |
| `kdeconnect.sms.*`, `kdeconnect.telephony.*` | Datos personales y coste. El móvil ya tiene root y canales auditados para eso |
| `kdeconnect.clipboard` | El portapapeles contiene contraseñas, códigos 2FA y claves. El oficial lo sincroniza **solo si la opción está activa**, pero el plugin se instala al emparejar ⇒ hay que desactivarlo a mano. Con un pin de 8 dígitos, "lo activé sin querer" es un incidente |
| `kdeconnect.sftp.*` | Duplica el `uploads/` del daemon y el servidor SSH. Más superficie, más un servicio que mantener |
| `kdeconnect.mpris.*`, `kdeconnect.systemvolume.*` | No aporta nada al arnés |
| `kdeconnect.findmyphone.request` | Acciona el dispositivo (suena/vibra). Utilidad baja para un arnés |

**Resumen del criterio:** en este enjambre el canal KDE Connect es
**descubrimiento + identidad + emparejado + un canal de control medido**. No
es un "mando a distancia completo" del móvil, porque ese mando a distancia
ya existe (root) y por otra vía.

---

## 5. Seguridad

### 5.1 Qué puede hacer un nodo **no emparejado**

**Nada.** Y esto está implementado, no prometido:

| Capa | Comportamiento | Dónde |
|---|---|---|
| TLS | Antes de emparejar, el handshake acepta cualquier certificado (TOFU): hace falta poder **emparejarse**. **Después** de emparejar, el certificado tiene que ser **exactamente** el pinneado | `TlsMaterial::client_config()` (verificador `PinnedVerifier`) |
| Sesión | De un par no emparejado **sólo** se acepta `kdeconnect.pair`; todo lo demás se **descarta** | `handle_control()` + la capa de sesión |
| `md.request` | Descartado antes de tocar el daemon local | test `un_md_request_de_un_peer_no_emparejado_se_descarta` |
| Certificado distinto | El TLS falla: "el certificado del peer no es el que se emparejó" | `PinnedVerifier` |
| Peer revocado | Se borra del almacén ⇒ el siguiente handshake falla el pin ⇒ el par queda fuera | `PeerStore::revoke()` |

### 5.2 El emparejado: PIN de 8 dígitos + confirmación humana

`decide_pairing()` aplica, **en este orden**:

1. `pair == false` ⇒ `Deny` (es un "desemparejar" o un rechazo).
2. Petición con más de 30 min ⇒ `Expired` (ventana igual que el oficial).
3. Si la política exige PIN: sin PIN ⇒ `MissingPin`; PIN distinto ⇒
   `WrongPin` (comparación en **tiempo constante**).
4. PIN correcto **pero sin confirmación humana** ⇒ `NeedsUserConfirmation`.
   **Esto es la frontera**: un PIN correcto **no** empareja solo.
5. Si no ⇒ `Accept`.

Y el PIN **no se guarda en claro**: se guarda su digest con sal
(`pin_digest()`), y sólo mientras la petición está pendiente.

### 5.3 Por qué el PIN se verifica en el móvil y no en el PC

1. **El móvil es el ancla de confianza**: es el dispositivo que el usuario
   tiene **en la mano**. El PC puede estar apagado, dormido o
   desatendido; el móvil no.
2. **El que decide es el que está presente**: si el PC fuese el que
   verifica, bastaría con que alguien publicara un `kdeconnect.pair`
   correcto desde la LAN para que el PC quedara emparejado con un
   atacante. Con el móvil verificando, hace falta **alguien mirando la
   pantalla del móvil**.
3. **El PC es la parte que puede estar ausente**: toda la escalera de
   canales está diseñada para que el PC no esté. Un control de seguridad
   que dependa del PC es un control que no existe cuando el PC no está.
4. **El flujo de dos sentidos**: si el PC inicia el pareado, **también** es
   el móvil el que exige que su usuario confirme el PIN (el "aceptar" del
   PC no basta). El PIN se muestra en una pantalla y se teclea en la otra:
   es una comprobación *fuera de banda* humana, no un automatismo.

### 5.4 Qué es 0600 y dónde se aplica

`0600` = el fichero se lee y se escribe **sólo por su dueño** (ni grupo ni
otros). Es el modo que usan ya `megadaemon_api.key` y la clave de host ADB
(`adb_client.rs`).

En este módulo:

| Ruta | Permisos | Contenido |
|---|---|---|
| `<state_dir>/kde/` | `0700` | Directorio del canal |
| `<state_dir>/kde/device.json` | `0600` | `device_id` + nombre del nodo |
| `<state_dir>/kde/cert.pem` | `0600` | Certificado autofirmado (público, pero no se regala) |
| `<state_dir>/kde/key.pem` | `0600` | **Clave privada** del nodo (PKCS#8) |
| `<state_dir>/kde/peers.json` | `0600` | Peers emparejados + sus pins |

La clave privada **nunca** se imprime: el `Debug` de `TlsMaterial` sólo
muestra la huella.

### 5.5 Qué se audita (y qué no)

**Se audita hoy:**

* El **almacén de peers** es el registro de auditoría: quién está
  emparejado, con qué huella de certificado, desde cuándo y cuándo se vio
  por última vez.
* La **decisión de empareado** queda reflected en el almacén: si un peer
  no está en `peers.json`, no está emparejado (no hay un "sí" implícito).
* La **revocación** se guarda al instante (`revoke()` escribe el fichero).

**NO se audita todavía (TODO, y hay que decirlo):**

* No hay un **log NDJSON de eventos** (empareado, rechazo, `md.request`,
  revocación) con marca de tiempo. Hoy la huella del almacén es el único
  rastro, y no distingue "se emparejó a las 10:00" de "se emparejó ayer".
* No se registra **qué rutas** se han pedido por `md.request` (sólo que
  fueron peticiones legítimas de un par emparejado).

**Limitación honesta (MITM durante el pareado).** El PIN viaja por el
canal cifrado; si un atacante **termina** el TLS durante el handshake de
pareado (MITM activo), puede **leer el PIN** e impersonar al par en el
momento del empareado. Lo que **sí** cierra esa ventana:

* el **pinning** posterior: en cuanto el empareado está hecho, el
  certificado del par queda fijado y un MITM posterior es rechazado por el
  TLS;
* el requisito de **confirmación humana** (un MITM no puede teclear el PIN
  en el móvil).

Para cerrar del todo la ventana del MITM hace falta lo que hace KDE
Connect oficial: un **código de verificación** derivado de las dos claves
públicas y del timestamp, que se muestra en **las dos pantallas** y que el
usuario compara. Está en la lista de §7.1 (`pairing verification key`); hoy
**no** está implementado (`NO VERIFICADO`).

### 5.6 Qué pasa si se pierde el empareado

| Situación | Qué hacer | Efecto |
|---|---|---|
| Se sospecha que un par está comprometido | `PeerStore::revoke(device_id)` (o borrar su entrada de `peers.json`) | Al instante: el siguiente handshake falla el pin ⇒ el par no puede ni hablar |
| Se pierde el móvil | Se borra `<state_dir>/kde/` del PC y el del móvil | Empareado perdido de ambos lados; hay que volver a emparejar (con PIN nuevo) |
| Se regenera el certificado del móvil (`rm cert.pem key.pem`) | Los pins que **guardan los pares** dejan de coincidir ⇒ los pares tienen que re-emparejar | Es lo correcto: el certificado es la identidad |
| Se cambia el `deviceId` | Todos los empareados quedan huérfanos | Igual que el caso anterior |
| El PIN se olvida | Se genera uno nuevo y se vuelve a emparejar | El PIN no se guarda en claro ⇒ no se puede recuperar, y no hace falta |

---

## 6. Interoperar con la app oficial de KDE Connect

**Lo que este módulo NO hace y por qué.** El protocolo oficial (v8) tiene
un handshake más picky que el que aquí se implementa, y esta implementación
se ha hecho **compatible en el formato**, no **completa en el protocolo**:

| Pieza del oficial | Estado aquí | Qué faltaría |
|---|---|---|
| Paquete `{"id","type","body"}` | **Hecho** (id = UUID en vez de timestamp: el `id` oficial es un `QVariant`) | — |
| Descubrimiento UDP en 1716, datagram JSON plano, tipo `kdeconnect.identity` | **Hecho** (además se escucha/anuncia el 1776 del enjambre) | — |
| `deviceId` con el patrón `^[a-zA-Z0-9_-]{32,38}$` | **Hecho** | — |
| `deviceName` saneado (`DeviceInfo::filterName`) | **Hecho** | — |
| Paquete de conexión en claro con `targetDeviceId`/`targetProtocolVersion` | **Hecho** | — |
| Subida a **TLS** con los roles invertidos (el que inicia el TCP es servidor TLS) | **Hecho** | — |
| Framing: JSON + `\n` | **Hecho** (y además el de 2 bytes BE, negociado) | — |
| Empareado `kdeconnect.pair` con `pair`/`timestamp` | **Hecho** (+ `pin` propio, campo extra que el oficial ignora) | — |
| **Código de verificación** (8 chars de SHA-256 de las dos claves públicas + timestamp, mostrado en ambas pantallas) | **NO** | Calcularlo y mostrarlo en las dos UIs (§5.5) |
| **TLS mutuo** (el oficial manda el certificado del cliente y lo valida) | **NO**: aquí el servidor TLS **no** pide certificado de cliente | `with_client_auth_cert` + verificación del cert del par en el rol de servidor TLS |
| **Certificado propio con el algoritmo del oficial** (el oficial usa RSA-2048 o EC y un CN con el deviceId) | **Parcial**: aquí es Ed25519 con CN = deviceId | Generar también (o sólo) RSA-2048 si el par oficial lo exige |
| **Plugins** (notificación, portapapeles, SMS, share, SFTP, mousepad…) | **NO** | Implementar plugin por plugin (§4: aquí no interesa) |
| **Bluetooth / mDNS** | **NO** | mDNS (`_kdeconnect._udp`) es bastante más código; Bluetooth, otro mundo |
| **Descarga de payloads** (`payloadSize`/`payloadTransferInfo`, transfer job) | **NO** | Está pensado para ficheros grandes; aquí el canal es de control |

**Conclusión honesta:** este módulo es **un sibling del protocolo**, no una
implementación de la app oficial. Se puede ver en la LAN y se puede
**hablar** con la app oficial en el discovery (mismo datagram, mismo tipo de
paquete, mismo `deviceId`), pero **no** se puede emparejar con ella ni
intercambiar mensajes, porque el oficial exige el TLS mutuo y el código de
verificación, y porque sus plugins no están implementados.

**Qué haría falta (en orden de dificultad):**

1. **Certificado RSA-2048** además del Ed25519 (o sólo RSA), con el CN del
   `deviceId` y el SubjectPublicKeyInfo idéntico al que espera rustls.
   Coste: el generador de DER ya está escrito (`selfsigned_cert.rs`), sólo
   cambia la firma (PKCS#1 v1.5 sobre SHA-256) y el SPKI.
2. **TLS mutuo**: el rol de servidor TLS tiene que pedir el certificado
   del cliente y validarlo contra el pin. Con `rustls`:
   `with_client_auth_cert` + un verificador de cliente propio.
3. **Código de verificación**: `SHA-256(pubkeyA_ordenada + pubkeyB_ordenada +
   timestamp)` → 8 hex. Se muestra en las dos pantallas y el usuario
   compara (o lo teclea).
4. **El `id` numérico**: el oficial manda `id` = timestamp en ms. Nuestro
   UUID es válido (`QVariant`), pero algunos clientes antiguos asumen que el `id` es un número.
   Easy fix: usar el timestamp y un uuid aparte como campo `md.uuid`.
5. **Los plugins que interesen** (notificación, batería) con sus nombres
   exactos de tipo.

---

## 7. Estado, qué falta y qué se ha tocado

### 7.1 Lo que hay en este commit

* `rust-core/src/kdeconnect.rs` — protocolo completo (paquete, framing con
  autodetección y negociación, identidad persistente, emparejado con PIN,
  almacén de peers, TLS con pinning, canal de control, descubrimiento UDP,
  `md.request`).
* `rust-core/src/selfsigned_cert.rs` — certificado autofirmado (DER escrito
  a mano, Ed25519 con `ring`, sin crates nuevos).
* `rust-core/src/bin/kde-discover.rs` — CLI: escucha, lista, y hace `ping`
  (y `md.request`) por el canal cifrado.
* `tools/kde_discover.py` — el lector en Python, sin dependencias.
* Tests: 31 en el módulo, incluidos el round-trip del framing **troceado en
  bytes arbitrarios** (1, 3, 7 y 1000 bytes), el PIN (correcto/incorrecto/
  ausente/vencido/sin confirmar) y una **sesión completa TCP+TLS entre dos
  nodos en loopback** (identidad → TLS → pin/pong → `md.request`).

### 7.2 Lo que NO se ha tocado (a propósito)

* **`cockpit_server.rs` no se ha modificado.** El módulo se puede arrancar
  desde el daemon con una línea (un `spawn` como el de `lan::spawn_beacon`),
  pero el daemon es hoy el origen del túnel de Cloudflare y su proceso se
  ha llevado dos OOM kills (`RESILIENCE.md` §2). Añadir un hilo más con
  sockets y TLS sin watchdog propio es exactamente el tipo de cambio que
  ya ha dado problemas ⇒ **queda pendiente y dicho aquí**, no escondido.
* **`main.rs` no se ha modificado** (mismo motivo).
* **El `Cargo.toml` sí** ha cambiado: `rustls` pasa a dependencia directa
  (ya estaba en el árbol por `ureq`, con el mismo proveedor `ring`) y `ring`
  pasa a dependencia directa. **No se añade ningún crate nuevo** al
  `Cargo.lock` (sólo dos aristas más en el paquete del crate).

### 7.3 Lo que falta para el canal 3 (relay en nodo siempre activo)

Dos piezas pequeñas y **separadas** de este commit:

1. **En el VPS**: un relay que (a) publica el canónico
   (`channel.json` con la URL estable) y (b) hace de proxy inverso: acepta
   conexiones TLS en 443 y las pasa, por un multiplexer propio, a la
   conexión saliente del móvil. Puede ser un `Caddyfile` + 200 líneas de
   relay, o un `ssh -R` si el VPS acepta `sshd` (más simple: `ssh -R` ya
   hace el trabajo de tubería de bytes, con el bonus de la identidad por
   `known_hosts`).
2. **En el móvil**: un cliente que **mantiene viva** la conexión saliente
   (reconexión con backoff, "hello" con el `device_id` al reconectar) y un
   watchdog al lado del supervisor (`99-megadaemon.sh`), con la misma
   semántica HTTP que `tunnel-watchdog.sh` (`530/000/502/503/504` = muerto).

Nada de esto necesita tocar la API: el relay mueve bytes.

### 7.4 Cómo probarlo hoy

```sh
# 1) Ver quién hay cerca (sólo descubrimiento, sin tocar nada)
python3 tools/kde_discover.py --timeout 15 --json
./rust-core/target/debug/kde-discover --timeout 15 --json

# 2) Anunciarse y ver al otro
./rust-core/target/debug/kde-discover --broadcast --name "PC-casa" --json

# 3) Ping por el canal cifrado (requiere estar emparejados)
./rust-core/target/debug/kde-discover --ping 192.168.1.124 --json

# 4) Pedir una ruta del daemon del par (acceso remoto bidireccional)
./rust-core/target/debug/kde-discover --ping 192.168.1.124 --request /api/status --json
```

Para que el paso 3 funcione, los dos nodos tienen que estar emparejados
(§5.2); el emparejado en sí (generar el PIN, confirmarlo y escribir el pin
en `peers.json`) **todavía no tiene interfaz de usuario**: falta el
"emparejar" en la UI del cockpit (TODO, §7.5).

### 7.5 TODO (ordenado por valor)

1. **Emparejar desde la UI/CLI**: generar PIN, mostrarlo, confirmar,
   escribir el pin del par. Es lo que bloquea el uso real del canal 1.
2. **Código de verificación** (§5.5) para cerrar el MITM del pareado.
3. **Log NDJSON de auditoría** del canal (empareados, revocaciones,
   `md.request`).
4. **Arrancar el módulo en el daemon** (`cockpit_server.rs`), con watchdog.
5. **Relay del canal 3** en el VPS + cliente saliente en el móvil (§7.3).
6. **Plugins** que interesen: `kdeconnect.battery` y `kdeconnect.notification`
   (§4.1).
7. **RSA-2048** en el certificado, si se quiere interoperar con la app
   oficial (§6).

---

## 8. Resumen para quien decide

* **El problema** ("no hay PC") **no se arregla con más túneles**: se
  arregla con un canal que no dependa del PC. El **canal 3** (relay en un
  nodo siempre activo + conexión **saliente** del móvil) es el que lo
  quita; el **canal 1** (KDE Connect) es el que da identidad y emparejado
  para que el LAN deje de ser una trusted network abierta.
* **La latencia** la manda el canal 1 (1–30 ms) y el riesgo lo baja el
  emparejado con PIN (§5).
* **No se duplica la API**: KDE Connect es descubrimiento + identidad +
  emparejado + un canal de control; el daemon sigue siendo la API.
* **La app oficial no es el objetivo**: este módulo es un sibling
  compatible en formato, no una implementación del protocolo oficial (§6).
