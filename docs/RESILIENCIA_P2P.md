# RESILIENCIA P2P — diseño del subsistema de identidad, bus y reconexión

> **Estado: DISEÑO. Nada de esto está implementado.**
> Fecha: 2026-11-26 · Rama: `integracion/todas` · Base: `b8de8e9`.
>
> Este documento **no** implementa nada: fija la arquitectura, los formatos de
> mensaje, los números y el plan por fases. Los ficheros Rust citados son los
> que **se tocarían**, no los que se han tocado.
>
> Regla del repo respetada (`docs/COORDINATION.md` §2): *"Un agente = un módulo
> Rust. Nadie toca `lib.rs`, `main.rs`, `Cargo.toml` ni módulos de otro agente
> sin acuerdo explícito en este tablón."* Por eso §6 marca qué módulos **ajenos**
> hay que tocar y por qué requieren acuerdo.

---

## 0. Resumen ejecutivo

El problema **no** es "el túnel se cae". Es:

> **El emisor (root, sandbox, otro agente) necesita conocer la URL *actual* del
> móvil para hablar con él, y esa URL cambia cada vez que `cloudflared` se
> relanza.** Todo lo demás (watchdog, paste, `channel.json` en el repo) son
> parches sobre el mismo problema.

| Nivel | Nombre | Qué rompe | Estado en el repo |
|---|---|---|---|
| **1** | Identidad + endpoint reubicable | "no sé dónde estás" | **Parcial**: hay identidad Ed25519 (`resonance.rs`) y beacon LAN (`lan.rs`); falta el registro de endpoints con prioridad |
| **2** | Bus de control **PULL** | "no me puedes empujar órdenes" | **Parcial**: hay `task_sync.rs` (cola por Drive) y `resonance.rs` (cartas firmadas); falta lease + idempotencia + anti-rebote |
| **3** | Reconexión extremo a extremo | "la sesión se rompe al cambiar de IP/red" | **Inexistente** |

**Tesis central**: *la continuidad no la da el transporte, la da el bus.* El
Nivel 3 conserva el *contexto* de una sesión; el Nivel 2 conserva el *trabajo*
(qué se pidió, qué se hizo, qué falta). Si el Nivel 3 falla, el trabajo no se
pierde: el bus lo reconcilia. **Por eso el Nivel 2 es obligatorio y el Nivel 3
es una optimización.**

Lo que **no** se va a hacer, y por qué:

- **No** QUIC/HTTP3 como transporte. QUIC resuelve la migración de IP *de una
  conexión ya establecida*; aquí el problema es *volver a descubrir una URL*.
  QUIC no ayuda si no hay con quién hablar. Se documenta (§2.3) porque su idea
  (Connection ID) es exactamente la del Nivel 1.
- **No** WireGuard como camino **por defecto**; queda como modo opcional (§5)
  que **no puede bloquear** el modo por defecto.
- **No** crates nuevos: `ring 0.17` ya da Ed25519 y (a verificar) X25519 +
  HKDF + ChaCha20-Poly1305.

---

## 1. El problema, medido

| # | Síntoma | Causa | Dónde |
|---|---|---|---|
| 1 | La flota queda ciega al relanzarse `cloudflared` | El *quick tunnel* **regenera el hostname** en cada arranque | `deploy/98-cloudflared.sh:76-78` |
| 2 | El `channel.json` del repo sirve una URL vieja | El `raw` de GitHub **cachea en CDN** | `rust-core/src/channel.rs:59-61` (lo dice el propio código) |
| 3 | El `paste` no sirve para descubrir nada | Cada publicación crea un paste **nuevo**; el siguiente es incognoscible | `rust-core/src/channel.rs:55-58` |
| 4 | El canónico `mdtmfnc.duckdns.org` da HTTP 000 | El A record lo publica quien llama (`duckdns.org/update` pone la IP de quien pregunta) | `docs/RESILIENCE.md` §0, punto 3 |
| 5 | No se sabe si el túnel está muerto o es la credencial | El 403 y el 530 se tomaban por el mismo caso | **Arreglado**: `channel.rs:309-361` (`probe_url_detalle`, `es_canal_muerto`) |

Los puntos 1-4 ya tienen parches (`docs/RESILIENCE.md` §2). Funcionan **hasta que
el canal entero cae**, y entonces el emisor se queda sin ningún camino de
vuelta. Eso es lo que este documento arregla.

**La asimetría a romper**:

```
EMISOR (root/sandbox)                      MÓVIL (S22)
  "¿dónde estás?"                            está en 192.168.1.124:9090
  "haz esto"                                 está en https://xyz.trycloudflare.com
        │                                    (cambia cada vez)
        ▼                                    está en https://mdtmfnc.duckdns.org
  ┌───────────────┐                          (a veces HTTP 000)
  │ necesita la   │◄────── ✗ ───────────────┘
  │ URL ACTUAL     │
  └───────────────┘
```

El emisor no puede tener la URL actual porque no puede adivinar el hostname
rotatorio, porque el `duckdns` canónico depende de un PC de casa que puede
estar apagado, y porque el `raw` de GitHub cachea y da la URL de *ayer*.
⇒ **El emisor debe preguntar "¿dónde estás?" sin conocer la respuesta de
antemano** (Nivel 1). Y, como la ruta puede estar caída en ese instante,
**el móvil debe tirar (pull) de las órdenes** (Nivel 2).

### 1.1 Lo que ya existe y hay que reutilizar

| Pieza | Fichero | Líneas | Qué aporta |
|---|---|---|---|
| Identidad Ed25519 por nodo | `rust-core/src/resonance.rs` | 483 | `clave-<nodo>.ed25519` (0600), firma de cartas, CRDT LWW `epoca`+`seq` |
| Bus por Drive (cartas firmadas) | `rust-core/src/resonance.rs` | (el mismo) | Journal NDJSON + snapshot `foto.json` + merge idempotente por `id` |
| Sincronización de cola por Drive | `rust-core/src/task_sync.rs` | 183 | El patrón exacto *listar → descargar → fusionar por id* |
| Tablón con lease + reloj monótono | `rust-core/src/board.rs` | 1960 | SQLite+WAL, `owner`, `lease_expires_at`, `attempts`, `LocalQueue` degradado |
| Subida/bajada nativa a Drive (sin curl) | `rust-core/src/drive_native.rs` | 870 | `ensure_folder`, `upload_file`, `download_file`, SHA-256 |
| OAuth de Drive | `rust-core/src/google_auth.rs` | 483 | `auth_header(state_dir)`, refresco automático |
| Autenticación del daemon | `rust-core/src/api_key.rs` | — | `X-API-Key`/`Bearer`, `constant_time_eq`, `fingerprint()` (sha16), 403 con `sent_key_fp` |
| Publicación del canal | `rust-core/src/channel.rs` | 613 | `channel.json`, paste, Contents API con `sha` previo |
| Beacon LAN (descubrimiento local) | `rust-core/src/lan.rs` | — | UDP difusión `:9099` cada 15 s, `state/lan.json`, **sin secretos** |
| Diagnóstico publicado a Drive | `rust-core/src/diagnostico.rs` | — | `MegaDaemon/diagnostico/<nodo>/`, `publicar_periodico(minutos)` |
| Energía del dispositivo | `rust-core/src/battery_advanced.rs` | — | `read_battery()`, `DeviceState`, `classify()`, `Thresholds` |
| Cuota/pausa por proveedor | `rust-core/src/quota_guard.rs` | — | Niveles, ventanas, `en_pausa()` |

**Conclusión de la auditoría: el Nivel 2 no es un bus nuevo, es `resonance.rs`
con leases e idempotencia añadidos.** Diseñar un bus paralelo sería el peor
resultado posible (dos formatos, dos carpetas en Drive, dos "outward-facing").
→ §3.2.7.

---

## 2. Estado del arte

La pregunta que ordena esta sección: **¿qué problema resuelve cada mecanismo?**
Hay que distinguir tres familias:

- **(A) Protocolo de transporte con reconexión**: el enlace se cae, se levanta
  y **la sesión sobrevive** porque el estado vive en los extremos.
- **(B) VPN con identidad estable**: la identidad del par no depende de su IP,
  así que puede moverse de red sin que nadie lo "busque".
- **(C) CDN / relé que reencamina**: un tercero **reenvía** el tráfico ciego.

### 2.1 mosh — (A)

Fuentes: <https://mosh.org/> · paper
<https://dspace.mit.edu/bitstream/handle/1721.1/71700/mosh-paper-1.pdf?sequence=1>
(USENIX ATC '12, *"Mosh: An Interactive Remote Shell for Mobile Clients"*) ·
ensayo USENIX `;login:` ago 2012
<https://www.usenix.org/system/files/login/issues/august2012.pdf>.

> ⚠️ **Corrección de título**: el encargo citaba *"The Design and Implementation
> of Mosh"*. El título real es **"Mosh: An Interactive Remote Shell for Mobile
> Clients"** (USENIX ATC '12, Winstein & Balakrishnan) y el ensayo
> complementario **"Mosh: A State-of-the-Art Good Old-Fashioned Mobile Shell"**
> (`;login:` vol. 37 nº 4, agosto 2012). No existe un paper con el título del
> encargo.

Los cuatro mecanismos que importan:

1. **State Synchronization Protocol (SSP)**: se sincroniza un *objeto de estado*,
   no un flujo de bytes. Cliente y servidor mantienen una instantánea y la
   sincronizan por parches; el servidor manda "el estado más reciente", no
   "todos los bytes". Cifrado y autenticación **por datagrama**: AES-128 en
   OCB3 (no TLS de flujo).
2. **Predicción de la IP del cliente (roaming)** — textual de mosh.org: *"Every
   time the server receives an authentic packet from the client with a sequence
   number higher than any it has previously received, the IP source address of
   that packet becomes the server's new target for its outgoing packets"* …
   *"Roaming works even when the client is not aware that its Internet-visible
   IP address has changed."* ⇒ **El servidor deduce la dirección del cliente
   del propio paquete autenticado**: sin señalización, sin re-negociación, sin
   timeout.
3. **Heartbeat cada ≤3 s**: el servidor aprende el destino rápido y el cliente
   avisa de que lleva tiempo sin oír al servidor (SSH no avisa).
4. **Prophylactic retransmission** (ensayo `;login;`): como el estado es
   idempotente, se reenvía lo que el cliente *podría* necesitar en vez de
   "lo que falta".

**Por qué sobrevive a cambios de IP/red**: la dirección **no está en el estado
de la sesión**; sólo en el transporte, y el transporte se re-deriva del último
paquete autenticado.

**Lo que mosh NO resuelve**: necesita un `mosh-server` al otro extremo y un
`ssh` de bootstrap ⇒ sigue necesitando **alcanzar** ese servidor. Para
MegaDaemon el servidor es el que rota ⇒ mosh tal cual no sirve. **Lo que se
toma es el patrón datagrama+seq+roaming+heartbeat, y se aplica al revés**: el
emisor es quien rota, así que el **receptor** deduce la dirección del emisor.

### 2.2 WireGuard — (B)

Fuentes: <https://www.wireguard.com/protocol/> ·
<https://www.wireguard.com/papers/wireguard.pdf> (§2.1 "Endpoints & Roaming") ·
<https://www.wireguard.com/known-limitations/>.

> ⚠️ `web_fetch` directo sobre `wireguard.com/protocol/` falló (la página no se
> extrajo); el texto citado procede del índice de búsqueda, que reproduce el
> texto de la página, y del PDF del paper, que sí se leyó. **El texto de
> `wireguard.com/protocol/` no se abrió con el lector de páginas.**

- **Identidad = clave pública Curve25519 de 32 bytes** (paper §2.1): *"Since a
  public key uniquely identifies a peer, the outer external source IP of an
  encrypted WireGuard packet is used to identify the remote endpoint of a peer,
  allowing peers to roam freely between different external IPs, between mobile
  networks for example, similar to what is allowed by Mosh."*
- **El endpoint es opcional y se re-ubica solo**: *"if not specified […],
  WireGuard receives a correctly authenticated packet from a peer, it will use
  the outer external source IP address for determining the endpoint."*
- **Handshake `Noise_IKpsk2_25519_ChaChaPoly_BLAKE2s`**: 2 mensajes, 1 RTT.
  `I` = la estática del iniciador se envía **cifrada** en el mensaje 1; `K` = el
  iniciador **ya conoce** la estática del respondedor. Primitivas: Curve25519
  (ECDH), HKDF (expansión), ChaCha20-Poly1305 (AEAD), BLAKE2s (hash).
- **Reconexión/rekey**: `KEEPALIVE` (paquete vacío) cada `KEEPALIVE` ms; sin
  respuesta en `KEEPALIVE + REKEY_TIMEOUT` se inicia un handshake nuevo;
  `REJECT_AFTER_TIME` descarta las claves. Es un protocolo *sin estado de
  conexión* en el sentido de TCP: el estado es el par de claves.
- **Riesgo documentado por el propio proyecto** (known-limitations, "Roaming
  Mischief"): *"WireGuard's roaming happens without an additional round trip or
  other authentication, which means an active man in the middle can replace
  source IP addresses."* ⇒ el roaming es **gratis pero no autenticado**: el
  atacante redirige el endpoint, aunque no descifra nada.

| Pieza | ¿Reutilizable? | Cómo |
|---|---|---|
| "Identidad estable + endpoint reubicable" | **Sí → es el Nivel 1** | Ya hay Ed25519 (`ring::signature`); falta el "endpoint opcional que se actualiza" |
| Noise IK / HKDF / ChaCha20-Poly1305 | **Sí, con `ring`** | `ring` 0.17 trae `agreement::X25519`, `hkdf`, `aead::CHACHA20_POLY1305`. **NO VERIFICADO** que la feature `alloc` esté activa y la API sea la esperada (el repo hoy sólo usa `ring::signature` y `ring::rand`) |
| Criptokey routing (`AllowedIPs` = rutas + ACL) | **No como descubrimiento** | `AllowedIPs` es ACL local; no resuelve "quién es este nodo" |
| El data plane | **Opcional (§5)** | Requiere `wireguard-tools` en el móvil (**NO VERIFICADO**) |

### 2.3 QUIC / HTTP3 y la migración de conexión — (A) por otra vía

Fuentes: <https://www.rfc-editor.org/rfc/rfc9000.html> (§5.1, §8.2, §9, §9.1-9.3,
§9.5) · <https://www.rfc-editor.org/rfc/rfc9001.html> (§4.5, §5.8, §9.1) ·
<https://www.rfc-editor.org/rfc/rfc9308.html> (§9).

- **Connection ID**: *"The primary function of a connection ID is to ensure that
  changes in addressing at lower protocol layers (UDP, IP) do not cause packets
  for a QUIC connection to be delivered to the wrong endpoint."* El CID **no**
  se deriva de la IP ⇒ **mismo principio que WireGuard**, con un identificador
  opaco en lugar de una clave.
- **Path validation (§8.2)**: `PATH_CHALLENGE`/`PATH_RESPONSE`. La RFC es
  explícita: *"Path validation is not designed as a NAT traversal mechanism."*
- **Migración (§9)**: *"The use of a connection ID allows connections to
  survive changes to endpoint addresses (IP address and port), such as those
  caused by an endpoint migrating to a new network."* Al detectar cambio de
  dirección del par: *"An endpoint MUST perform path validation (Section 8.2)
  if it detects any change to a peer's address, unless it has previously
  validated that address."* Y al migrar: *"An endpoint MUST use a new
  connection ID if it initiates connection migration"* (reducir vinculabilidad).
- **NAT rebinding**: caso particular de cambio de dirección; §9.3 lo distingue
  del rebinding y exige validar la ruta anterior con `PATH_CHALLENGE` para
  detectar *address spoofing*.
- **Sesión TLS (§9001)**: `NewSessionTicket` permite reanudar con 0-RTT, pero
  §9.1 avisa: *"Session resumption allows servers to link activity on the
  original connection with the resumed connection, which might be a privacy
  issue for clients."*
- **Límite honesto (§9308 §9)**: *"Some QUIC connections might not be robust to
  NAT rebinding because the routing infrastructure (in particular, load
  balancers) uses the address/port 4-tuple to direct traffic."*

**Por qué NO se usa QUIC**: el problema no es "una conexión que se mueve", es
"no hay con quién hablar". QUIC requiere que **los dos extremos ya se hayan
encontrado** (handshake); MegaDaemon necesita **encontrarse**. QUIC resuelve
lo segundo sólo si se resuelve lo primero. ⇒ Pero **la idea se copia**: el
Nivel 1 usa un **identificador de sesión estable e independiente de la
dirección** (§3.1) = el CID de QUIC convertido en "identidad de nodo + endpoint
reubicable".

### 2.4 MPTCP (RFC 8684) y su uso real en redes móviles

Fuente: <https://www.rfc-editor.org/rfc/rfc8684.html>.

- `MP_CAPABLE` sólo en el **primer** subflow; el intercambio de claves da
  material para autenticar los subflows futuros.
- `MP_JOIN` en cada subflow nuevo, con un **token de 32 bits** que es un hash
  de la clave del receptor negociada en el `MP_CAPABLE`. Ese token **identifica
  la conexión** a la que se une el subflow ⇒ otro "identificador de sesión
  estable" (como el CID de QUIC).
- `ADD_ADDR`: el emisor **anuncia direcciones alternativas** (con HMAC);
  `REMOVE_ADDR` las retira. `MP_PRIO` cambia la prioridad de un subflow y
  permite *make-before-break* y el modo "backup". `DSS` (Data Sequence Signal)
  **mapea los bytes de datos a nivel de conexión** sobre subflows
  independientes ⇒ la conexión sobrevive a que un subflow muera.

Uso real (de la literatura, no medido por nosotros):

- **iOS**: MPTCP soportado en iPhones/iPads **desde septiembre de 2013**
  (<https://datatracker.ietf.org/doc/draft-ietf-mptcp-experience/05/>), con
  cientos de millones de dispositivos, pero *"only used to support a single
  application"* (Safari).
- **Android**: varios fabricantes lo soportan, pero *"they are currently
  limited to specific countries"*
  (<https://inl.info.ucl.ac.be/system/files/multipathtester.pdf>). El kernel
  AOSP no lo trae de serie ⇒ en el S22 **NO VERIFICADO**.
- **3GPP**: *"3GPP has selected MPTCP and the 0-RTT Convert Protocol to support
  Access Traffic Steering, Switching & Splitting for TCP-based applications"*
  (<https://www.irtf.org/anrw/2021/anrw2021-final38.pdf>). Es decir: **una red
  móvil puede ofrecer agregación WiFi/LTE al usuario, pero eso es del
  operador, no de la app.**

⇒ **Por qué no sirve aquí**: MPTCP migra **dentro** de una conexión TCP ya
establecida entre extremos conocidos. El daemon usa HTTP sobre TLS en un
túnel HTTP de Cloudflare: **el intermediario es HTTP/1.1 sobre TCP, no TCP
directo** ⇒ no se puede negociar MPTCP a través de él. Y el problema (la URL
rota) está **por encima** de la conexión.

### 2.5 Overlays: Tailscale, DERP, Headscale, Netmaker, ZeroTier — (B) + (C)

Fuentes: <https://tailscale.com/docs/reference/derp-servers> ·
<https://tailscale.com/docs/reference/connection-types> ·
<https://tailscale.com/docs/reference/device-connectivity> ·
<https://tailscale.com/docs/features/peer-relay> ·
<https://tailscale.com/blog/peer-relays-international-networks> ·
<https://modem7.github.io/cheat-sheets/networking/mesh-vpn-comparison/> ·
<https://www.ssdnodes.com/learn/tailscale-alternatives>.

- **Tailscale = WireGuard + plano de control**. El plano de control reparte
  claves públicas y **endpoint info**; el plano de datos va WireGuard.
- **DERP** (*Designated Encrypted Relay for Packets*): es (C). *"A DERP server
  blindly forwards already-encrypted traffic from one device to another"*.
  Sirve para (a) negociar conexiones y (b) **relay de último recurso**. *"All
  connections start as relayed through a DERP server, and Tailscale then tries
  to upgrade them to a direct connection."* ⇒ el **relay es el piso, no el
  techo**.
- **DISCO**: protocolo ligero de Tailscale sobre DERP para descubrirse antes de
  intentar el directo. **No es ICE**.
- **NAT types**: *hard NAT* (el mapeo de puertos varía según el destino) ⇒
  directo imposible, sólo relay. `MappingVariesByDestIP` es el indicador de
  `netcheck`.
- **Headscale**: servidor de control autosuficiente (BSD-3) que usa los
  clientes oficiales de Tailscale, con **DERP embebido** opcional; por defecto
  la flota pública de DERP. **Netmaker**: automatiza configuración WireGuard;
  `netclient` configura la interfaz del kernel; gateways para nodos que no
  pueden correr el agente. **ZeroTier**: protocolo propio (ZTNVP), emula
  **Ethernet** (L2, broadcast); relay como fallback.

⇒ **Lección**: el plano de control y el plano de datos son **cosas separadas**,
y el relay es un **piso de degradación**, no la arquitectura. El Nivel 1 es el
*plano de control* de MegaDaemon; el Nivel 3 es el *plano de datos*.

### 2.6 Relés y descubrimiento: Syncthing, TURN/ICE, BitTorrent DHT — (C)

- **Syncthing Relay v1** (<https://docs.syncthing.net/specs/relay-v1.html>):
  dos modos — *protocol mode* (TLS, con `Device ID` de 32 bytes; el relé
  **verifica la identidad** del dispositivo) y *session mode* (a pelo, reenvía
  bytes), con una **clave de sesión única** en el `SessionInvitation`. Y:
  *"the connection between two devices is still end to end encrypted, the relay
  only retransmits the encrypted data much like a router"*.
  (<https://docs.syncthing.net/users/relaying.html>: Syncthing reintenta el
  directo periódicamente y abandona el relay si lo consigue.)
  ⇒ **El relé de Syncthing es el modelo exacto del relé `duckdns` de
  MegaDaemon**, con una diferencia: Syncthing **verifica la identidad** en el
  handshake; el relé de MegaDaemon no.
- **STUN / ICE / TURN** (<https://www.rfc-editor.org/rfc/rfc5389.html> ·
  <https://www.rfc-editor.org/rfc/rfc8656.html> ·
  <https://datatracker.ietf.org/doc/draft-ietf-ice-rfc5245bis/09/>): STUN es
  **una herramienta**, no una solución — *"STUN is not a NAT traversal solution
  by itself. Rather, it is a tool to be used in the context of a NAT traversal
  solution"* — y devuelve la **dirección reflexiva** (`XOR-MAPPED-ADDRESS`).
  ICE combina candidatas *host* / *peer-reflexive* / *server-reflexive* /
  *relay* y las prueba por pares con `Binding` requests. TURN es el relé: *"A
  client using TURN must have some way to communicate the relayed transport
  address to its peers"* ⇒ **el relé no descubre peers por sí mismo**: hace
  falta señalización aparte.
  ⇒ **Lección**: descubrimiento y relé son **problemas separados**. MegaDaemon
  ya tiene el equivalente del canal de señalización (el bus).
- **BitTorrent DHT (BEP 5)** (<https://www.bittorrent.org/beps/bep_0005.html>):
  Kademlia sobre UDP; *"each peer becomes a tracker"*; operaciones `get_peers`
  y `announce_peer`; el `announce_peer` **lleva la IP:puerto** del que se
  anuncia ⇒ **el DHT es un "registro de endpoints reubicables" descentralizado**.
  Es el ancestors conceptual más cercano al Nivel 1 + descubrimiento por
  hashing. **Pero con 3-5 nodos no hace falta un DHT**: un tablón central
  (Drive) es más simple y más fiable.

### 2.7 SSH mux / autossh / wstunnel / túneles inversos — (A)

Fuentes: <https://en.wikibooks.org/wiki/OpenSSH/Cookbook/Multiplexing> ·
<https://manpages.ubuntu.com/manpages/bionic/man3/Config::Model::models::Ssh.3pm.html> ·
<https://man.archlinux.org/man/autossh.1> ·
<https://github.com/erebe/wstunnel/blob/main/README.md>.

- **ControlMaster / ControlPath / ControlPersist**: la primera conexión abre un
  socket UNIX y las siguientes **multiplexan canales por dentro** (*"These
  sessions will try to reuse the master instance's network connection rather
  than initiating new ones"*). `ControlPersist` deja el master en segundo plano
  `N` minutos tras la última sesión; `MaxSessions` (10) limita los canales.
  ⇒ **Limitación dura**: un master SSH **muere** al cambiar la IP (el 4-tuple
  TCP es la identidad) ⇒ *"There is no configuration that survives an IP
  change; that is TCP, not SSH."* El multiplexing **ahorra handshakes**, **no**
  salva la conexión.
- **autossh**: *"a program to start a copy of ssh and monitor it, restarting it
  as necessary should it die or stop passing traffic"*. Detecta el túnel
  *atascado* (abierto pero sin tráfico) con un puerto de monitor y un eco cada
  10 min; backoff gradual hasta el intervalo máximo. Si el `ssh` sale
  "normalmente", `autossh` **no** reinicia.
- **wstunnel**: túnel por **WebSocket** (o HTTP/2, o WebTransport) sobre TLS,
  con **pool de conexiones**, **reconexión con backoff exponencial** (1 s por
  defecto) y modo *reverse tunnel* donde *"the client will try to always keep
  a connection to the server to await for new tunnels"*.
  ⇒ **Ésta es la pieza que falta**: el **túnel inverso persistente** (el móvil
  mantiene el socket abierto; el emisor entra por el lado que puede).

### 2.8 Xray / V2Ray / sing-box — (A)+(C), con otro objetivo

Fuentes: <https://github.com/TrayBer/Mochi/blob/lx/docs-lx/xray-protocols-explained.md>
(documentación de un *fork* de sing-box que describe el comportamiento de Xray) ·
<https://www.usenix.org/system/files/sec24summer-prepub-465-xue.pdf> (Sec'24) ·
<https://www.usenix.org/system/files/usenixsecurity23-wu-mingshi.pdf> (Sec'23).

- **Tunneling vs. mimicry**: *tunneling* = el tráfico viaja **dentro** de una
  sesión TLS/HTTP real; *mimicry* = imitar el aspecto de un protocolo
  permitido. La literatura es tajante: Houmansadr et al. concluyen que
  *"obfuscation by imitation is fundamentally flawed"* y que **tunelizar es más
  resistente**.
- **Fragmentación del ClientHello**: Xray (`settings.fragment`: qué paquetes,
  longitud, retardo) y sing-box (`common/tlsfragment`: corta el SNI **dentro de
  cada etiqueta**, en un punto aleatorio; `tls.fragment` espera el ACK entre
  segmentos con Nagle desactivado; `tls.record_fragment` mete cada trozo en
  **su propio registro TLS**). Objetivo: que un clasificador no vea el SNI
  entero.
- **REALITY**: el TLS se hace **contra el sitio de cobertura**; el cliente
  manda la clave del servidor real en la extensión. **XHTTP**: el proxy viaja
  **dentro de peticiones HTTP** (GET de descarga + POST de subida), con `xmux`
  para reutilizar conexiones.
- **El punto clave**: *"the connection rides on ordinary TLS, HTTP or
  WebSocket and passes through a CDN"*. **Un CDN no sabe a quién pertenece la
  conexión**, pero **sí puede enrutar por *hostname*** ⇒ por eso el problema
  del hostname rotatorio de Cloudflare **no** se arregla con tunneling: hace
  falta **cambiar el nombre**, no el transporte.
- **"Angelic"** (mencionado en el encargo): **NO VERIFICADO**, no se ha
  encontrado ningún proyecto relevante con ese nombre en el contexto de
  roaming/túneles. Probablemente designa la familia Xray/V2Ray/sing-box (y
  posiblemente **AnyTLS**, que aparece en los surveys como protocolo
  desplegado). Si designa otra cosa, este documento no la cubre.

⇒ **Conclusión**: el "disfraz" **no aplica** al problema de MegaDaemon, que no
es de censura sino de **descubrimiento**. Lo que sí se toma de esta familia es
la idea de **tunelizar dentro de un protocolo que los proxies ya dejan pasar**
(§3.3.6).

### 2.9 Tabla resumen: las tres familias

| | **(A) Transporte con reconexión** | **(B) VPN con identidad estable** | **(C) Relé que reencamina** |
|---|---|---|---|
| Ejemplos | mosh/SSP, SSH mux+autossh, wstunnel, MPTCP, QUIC | WireGuard, Tailscale, Headscale, Netmaker, ZeroTier | DERP, TURN, Syncthing relay, Cloudflare Tunnel, relé `duckdns` |
| Identidad | suele depender de la conexión (4-tuple) | **clave pública** (independiente de la IP) | ninguna (ve sólo bytes) |
| ¿Sobrevive a cambio de IP? | **Sí** si el protocolo lo soporta (mosh/QUIC/MPTCP); **no** en SSH TCP | **Sí**, gratis (roaming) | no aplica (no hay sesión) |
| ¿Sobrevive a que el *servidor* cambie de dirección? | mosh: **no** (el servidor es fijo por diseño) | WireGuard: **no** sin plano de control que reubique al par | **Sí**: el relé tiene nombre fijo |
| Descubre peers | no | **sí** (plano de control) | no (hace falta señalización aparte) |
| Cifra de extremo a extremo | sí | sí (el relé no descifra) | sí (si el relé sólo reenvía) |
| **Aplícale a MegaDaemon** | **Nivel 3** (mosh-like) | **Nivel 1** (identidad) + §5 (modo WG) | **Ya lo hay**: `duckdns`, Cloudflare |

⇒ **MegaDaemon necesita las tres, pero en orden inverso al habitual**: primero
el (C) que ya existe y ya rota, después el (B) para saber a quién se le habla,
después el (A) para no perder el contexto, y el bus por debajo de todo.

---

## 3. Diseño propuesto

### 3.0 Vista general

```
                 ┌──────────────────────────────────────────┐
                 │  EMISOR  (root / sandbox / otro agente)  │
   "¿dónde estás?" ─────────►  ┌────────────────────────┐  │
   "haz esto"      ◄─────────  │  Nivel 1: identidad +   │  │
                              │  endpoints reubicables  │  │
                              └───────────┬────────────┘  │
                                          │ carta firmada Ed25519
                                          ▼
   ┌───────────────────────────────────────────────────────────────┐
   │  BUS (Nivel 2) — el móvil TIRA de él, nunca le empujan      │
   │  ● Drive:  MegaDaemon/bus/<nodo>/           (primaria)        │
   │  ● Repo:   state/bus/<nodo>.inbox.jsonl     (respaldo)        │
   │  ● Relé:   https://mdtmfnc.duckdns.org/bus/ (si está vivo)    │
   │  ● Paste:  cadena prev_paste                (último recurso)  │
   │  Cada mensaje: firma Ed25519 + epoch/seq + idem_key + lease │
   └───────────────┬───────────────────────────────┬───────────────┘
                   │ pull (cada 15-30 min)         │ pull
                   ▼                               ▼
        ┌─────────────────────┐         ┌─────────────────────┐
        │  MÓVIL (S22)        │         │  PC / otros nodos  │
        │  claim + lease      │         │  claim + lease     │
        │  ejecuta            │         │  ejecuta           │
        └──────────┬──────────┘         └─────────────────────┘
                   │  Nivel 3 (opcional, mosh-like):
                   │  UDP + seq + roaming + heartbeat
                   ▼
        ┌─────────────────────┐
        │  EMISOR (directo)    │  ← con Nivel 3 vivo, el push es instantáneo
        └─────────────────────┘
```

**Regla de oro**: *ningún camino de entrega es obligatorio*. Si el Nivel 3 no
funciona, el bus entrega en el siguiente pull. Si Drive no funciona, el repo
entrega. Si el repo cachea, el relé entrega. **El trabajo nunca depende de que
un canal esté vivo.**

### 3.1 Nivel 1 — Identidad y endpoint reubicable

**Identidad.** **Ya existe** (`resonance.rs:515-549`): clave Ed25519 por nodo
en `<state_dir>/tablón/clave-<nodo>.ed25519` (0600), generada con
`ring::signature::Ed25519KeyPair::generate_pkcs8`; `Tablon.pub_hex` es la
pública en hex. Lo que **falta** es un **identificador de nodo estable e
independiente del nombre de host**:

```
node_id = "md-" + <nombre_normalizado> + "-" + sha256(pubkey_ed25519)[0..8]
```

8 bytes de hash ⇒ 2⁶⁴ de colisión (de sobra para una flota de 10 nodos) y
legible al depurar. El **nombre se puede cambiar** (`MEGADAEMON_NODE` en
`resonance.rs:452` es el host); el `node_id` **no**, porque depende de la
clave. Consecuencia: **rotar la Ed25519 cambia el `node_id`** ⇒ hace falta una
rotación con periodo de gracia (**NO VERIFICADO** que el repo tenga ya
procedimiento de rotación de claves de nodo).

**Registro de endpoints reubicables.** Un **endpoint** no es una URL: es una
**dirección que puede cambiar**, con la información para *descartarla* cuando
envejezca.

```json
{
  "kind": "https",
  "url": "https://graphical-subscriber-sys-oil.trycloudflare.com",
  "transport": "quick-tunnel",
  "priority": 40,
  "epoch": 1793000000,
  "expires": 1793003600,
  "probe": "POST /mcp",
  "note": "rota en cada relanzamiento de cloudflared"
}
```

| `transport` | Ejemplo | Prio | Quién lo publica | Vida |
|---|---|---|---|---|
| `lan` | `http://192.168.1.124:9090` | **100** | beacon `lan.rs` (UDP :9099, 15 s) | efímera, sólo subred |
| `relay-duckdns` | `https://mdtmfnc.duckdns.org` | **70** | `channel.rs` / `duckdns-updater` | estable, depende del PC de casa |
| `wg` | `wg://<pubkey>@<host>:<port>` | **65** | Nivel 1 (§5) | estable si el peer tiene NAT estable |
| `quick-tunnel` | `https://<rand>.trycloudflare.com` | **40** | `channel.rs` + watchdog | **rota**, ~1 h de vida útil |
| `repo-contents` | `https://raw.githubusercontent.com/.../state/channel.json` | **20** | Contents API | sólo metadatos, **cachea en CDN** |

**Algoritmo del emisor** (`resolver_endpoint(nodo)`):

```
1. Leer el bus (Nivel 2) → lista de endpoints del nodo, firmados.
2. Descartar los expirados (epoch + expires < ahora).
3. Ordenar por (prioridad desc, expires desc).
4. Sondear con la SEMÁNTICA YA EXISTENTE:
      530/000/502/503/504  ⇒ muerto, siguiente
      401/403              ⇒ VIVO (es la credencial, no la red) → ÉSTE
      200                  ⇒ VIVO → ÉSTE
5. Si ninguno responde ⇒ "nodo inalcanzable" + el último error.
```

El paso 4 **no es nuevo**: es `channel::probe_url_detalle()` +
`channel::es_canal_muerto()` (`channel.rs:327-361`), que ya distingue "túnel
muerto" de "credencial mala". **Reutilización directa, sin cambiar la
semántica.**

**El mensaje "¿dónde estás?".** **Ya existe casi entero**: `resonance.rs`
define el tipo de carta `"latido"` (`TIPOS[0]`) con cuerpo libre. El Nivel 1
**extiende** ese cuerpo:

```json
{
  "kind": "md-presencia/v1",
  "node_id": "md-s21-3f9a2c11",
  "nodo": "s21",
  "endpoints": [ { ... }, { ... } ],
  "lan": { "ip": "192.168.1.124", "port": 9090, "beacon_ts": 1793000000 },
  "build": "13.4.0+20261126",
  "carga": { "load1": 3, "ram_free_kb": 812345 },
  "energia": { "pct": 72, "charging": false, "policy": "normal" },
  "canales": { "quick": "...", "duckdns": "...", "canonical": "..." }
}
```

- Va **firmado** (`resonance.rs:105-122`: `Carta::canónico()` +
  `Carta::verificar(pub_hex)`) ⇒ nadie suplanta al nodo ni publica endpoints
  falsos. `id = sha256(canonico)` ⇒ merge idempotente. `epoch`/`seq` ⇒ el más
  nuevo gana (LWW por época).
- El `channel.json` que ya se publica **gana los campos de endpoint** (`quick`,
  `duckdns`) ⇒ el lector legacy del repo también los ve: se añade `node_id` y
  `endpoints[]` **sin quitar nada** ⇒ compatible.

**Cómo se resuelve el caso "el hostname rotó".**

1. El watchdog detecta el túnel muerto (530/000), relanza `cloudflared`
   (`tunnel-watchdog.sh:208-226`) y extrae la URL nueva del log
   (`98-cloudflared.sh:76-78`).
2. Publica: `channel-publish --repo-publish` (repo) y el daemon publica el
   `channel.json` local (`cockpit_server.rs:3321`).
3. **Nuevo**: el daemon publica una carta de presencia en el bus (Nivel 2) con
   el endpoint nuevo, firmada con la Ed25519 del nodo.
4. El emisor, que estaba sondeando `alpha`, hace su siguiente pull (o su
   siguiente probe, si está en Nivel 3) y recibe `beta`.
5. **Latencia de redescubrimiento**: con Nivel 3, **≤3 s** (heartbeat de
   mosh); sólo con Nivel 2, **≤15-30 min** (intervalo de pull) o **≤5 min** si
   además se mira `state/channel.json` del repo (cuyo push es inmediato).
   **Éste es el número que hay que bajar**, y es el motivo del Nivel 3.

### 3.2 Nivel 2 — Bus de control PULL

**Por qué PULL y no PUSH.** Un bus PUSH exige que el emisor **encuentre** al
móvil **y** que el móvil esté en línea en el instante de la orden. Con un
hostname rotatorio, las dos cosas fallan a la vez. Un bus **PULL** invierte la
asimetría: el emisor **deja** la orden en un sitio accesible (Drive, el repo) y
el móvil, **cuando puede**, la recoge. Esto es exactamente lo que permite
*"controlar el móvil y sus apps (incluso en primer plano) sin métodos directos
de comunicación"*: **no hace falta que el móvil sea localizable en el instante
de la orden**, sólo que vuelva a pasar por el bus.

⇒ **El PUSH queda como optimización, nunca como garantía.** El emisor *intenta*
el push (si conoce un endpoint vivo) para bajar la latencia a segundos; si
falla, la orden **se queda en el bus** y se entrega en el siguiente pull.

**Los cuatro transportes.** Todos implementan la misma interfaz
(`bus_transport.rs`):

```rust
trait Transport {
    fn listar(&self, nodo: &str) -> Result<Vec<Envelope>, String>;
    fn publicar(&self, nodo: &str, env: &Envelope) -> Result<(), String>;
}
```

| # | Transporte | Ruta | Latencia | Cachea | Prioridad |
|---|---|---|---|---|---|
| 1 | **Drive** | `MegaDaemon/bus/<nodo>/msg-<id>.json` | 1-3 s | no | **primaria** |
| 2 | **Repo** | `state/bus/<nodo>.inbox.jsonl` (append) | 5-60 s | **sí** (CDN) | respaldo |
| 3 | **Relé** | `GET https://mdtmfnc.duckdns.org/bus/<nodo>` | 1 s | no | si está vivo |
| 4 | **Paste** | `prev_paste` del `channel.json` | 10-60 s | no | último recurso |

**Sobre el repo y el CDN**: el propio `channel.rs:59-61` ya avisa de que el
`raw` *"cachea en CDN ⇒ puede servir la versión vieja"*. Para el bus: **no** se
usa el `raw` como fuente de *órdenes urgentes*; se rompe la cache con un
**query param de bust** (`.../main/state/bus/<nodo>.inbox.jsonl?t=<epoch>`; un
parámetro de query **no** lo cachea el CDN); y **además** el mensaje lleva
`epoch`/`seq` ⇒ si llega viejo, el **anti-rebote del receptor lo rechaza**
(§3.2.4). La caché degrada la latencia, **nunca la corrección**.

**Reuso**: el patrón *listar → descargar → fusionar por id* de
`task_sync.rs:61-172` se copia **casi literal** (con los dos bugs ya
arreglados allí: comprobar el status HTTP antes de parsear, y no tragarse el
error de lectura). `drive_native::ensure_folder` + `upload_file` +
`download_file` hacen el 90 % del trabajo.

**Formato del mensaje.** Una línea JSON por mensaje (NDJSON), con el estilo de
`resonance.rs`:

```json
{
  "v": 1,
  "bus": "md-bus/v1",
  "id": "sha256-hex-del-canonico",
  "epoch": 1793000000,
  "seq": 42,
  "from": "md-pc-7a1b9c02",
  "to": "md-s21-3f9a2c11",
  "kind": "task",
  "issued_at": 1793000000,
  "ttl": 900,
  "lease_secs": 300,
  "idem_key": "backup-completo-2026-11-26",
  "payload": { "cmd": "backup_apps", "args": { "dest": "drive" } },
  "firma": "hex-ed25519-sobre-el-canonico"
}
```

| Campo | Regla |
|---|---|
| `id` | `sha256(canonico)` ⇒ dedup byte a byte |
| `epoch` | Época del **emisor** |
| `seq` | Secuencia del emisor, monotónica dentro de la época |
| `kind` | `task` \| `cmd` \| `ping` \| `ack` \| `state` |
| `ttl` | Tras `issued_at + ttl` el mensaje **caduca** (no se ejecuta) |
| `lease_secs` | Duración del reclamo (por defecto 900 = `board::DEFAULT_LEASE_SECS`) |
| `idem_key` | Clave **lógica** de idempotencia (no el hash del contenido) |
| `firma` | Ed25519 sobre el canónico (mismo patrón que `Carta::canónico()`) |

El **canónico** que se firma excluye `firma` e `id` (como en
`resonance.rs:85-98`).

**Idempotencia: dos capas para dos problemas distintos.**

- **Capa A — deduplicación de transporte** (`id`): "¿he visto *exactamente*
  este mensaje?" ⇒ `HashSet` de ids vistos (o tabla SQLite con `PRIMARY
  KEY`). Resuelve: el mismo fichero descargado dos veces, el mensaje reenviado
  por dos transportes, el pull repetido.
- **Capa B — idempotencia lógica** (`idem_key`): "¿he *ejecutado* ya esta
  orden?" ⇒ tabla `bus_dedupe(idem_key PRIMARY KEY, id, ts, resultado)`.
  Resuelve: el emisor reintenta porque no vio el ACK, dos transportes
  entregan el mismo mensaje con ids **distintos** (re-firmado o re-serializado),
  o el móvil se reinicia a mitad y vuelve a ejecutar.

⇒ **Regla dura**: un mensaje con `idem_key` ya presente en `bus_dedupe` se
**ACK-ea y se descarta**, sin ejecutar. Eso es lo que permite al emisor
**reenviar sin miedo**.

**Anti-rebote (epoch/seq) — por emisor, no por reloj.** Se guarda
`(epoch_max, seq_max)` por `from`:

```
msg.epoch  >  epoch_max                       → aceptar; epoch_max = msg.epoch; seq_max = msg.seq
msg.epoch  == epoch_max && msg.seq > seq_max  → aceptar; seq_max = msg.seq
msg.epoch  == epoch_max && msg.seq <= seq_max → RECHAZAR (rebote)
msg.epoch  <  epoch_max                       → RECHAZAR (obsoleto)
```

**Por qué epoch/seq y no timestamps**: el reloj del móvil puede ir atrás (el
repo ya lo contempla: `board.rs` usa un "reloj monótono del tablón" para que
*"un nodo con el reloj atrás no pueda acortar un lease ni robar una tarea"*). Un
mensaje con `ts` nuevo pero `epoch` vieja **debe** rechazarse; uno con `ts`
viejo pero `epoch` nueva **debe** aceptarse. **Sólo epoch/seq lo consigue.**

**Quién incrementa la época**: el emisor. Y hay un comando explícito para
**invalidar** una época cuando se sospecha que un emisor está zombi o que se ha
roto su clave: `md-bus bump-epoch --from <node_id>`. El emisor, al ver una
`epoch` mayor para sí mismo, adopta esa época y **descarta** lo pendiente de
la época anterior.

**Leases.** El bus **transporta órdenes**; el **estado de verdad es
`board.sqlite`**. Ésta es la separación que evita tener dos motores de estado.

- Al recibir `kind: "task"`, el móvil hace `board.claim(id, owner=node_id,
  lease_secs)` (`board.rs:797-860`). Si el board dice que **otro** nodo la
  tiene con lease vigente ⇒ **se ignora el mensaje del bus**.
- Mientras ejecuta, el móvil renueva el lease en cada pull (mensaje `ack` con
  `lease`).
- Si el móvil desaparece, el lease caduca (900 s por defecto) y
  `reclaim_expired` (`board.rs:451`, `md-board reclaim`) la devuelve a `queued`.
- `attempts` y `last_error` (`board.rs:288-290`) dan trazabilidad y backoff.

⇒ **El lease del bus y el del board son el mismo mecanismo**: el bus no
implementa leases, **los pide** al board. Esto evita el error clásico de dos
relojes de lease desincronizados.

**El ciclo de pull del móvil.**

```
cada T (= 15..30 min, ajustado por la batería; §4):
  1. estado local: ¿queda trabajo de un pull anterior? → terminarlo
  2. pull: para cada transporte (drive → repo → relé → paste), listar y
     descargar los mensajes dirigidos a <node_id>
  3. merge: dedup por `id`; anti-rebote por (epoch, seq); TTL
  4. ejecutar: los `kind: task` que me tocan → board.claim → ejecutar →
     board.report(done|failed)
  5. push: subir resultados, ACK de los procesados, y una carta de presencia
     con los endpoints actuales (§3.1)
```

**Ciclo del emisor**: idéntico, pero además intenta el **push best-effort** al
endpoint vivo que resolvió el Nivel 1 para entregar `kind: cmd` en segundos en
vez de minutos.

**Por qué el Nivel 2 NO es un bus nuevo.** **Éste es el riesgo nº1 del diseño**
(§7, R-9). El repo **ya tiene** un bus por Drive con cartas firmadas:
`resonance.rs`.

| Capacidad | ¿Ya existe? | Dónde | Qué falta |
|---|---|---|---|
| Transporte por Drive | ✅ | `resonance.rs:345-443` (`sincronizar()`) | — |
| Firma Ed25519 del emisor | ✅ | `resonance.rs:105-122` | — |
| Merge idempotente | ✅ | `resonance.rs:272-288` (`fusionar`) | — |
| Anti-rebote epoch/seq | ✅ | `resonance.rs:26-36` (LWW por época) | adaptarlo a "por destinatario" |
| Tipos de carta | ✅ | `resonance.rs:54-61` (`latido`, `peticion`, `aviso`, `cambio`, `cierre`) | faltan `ack`, `state` |
| **Lease** | ❌ | — | delegar en `board.rs` |
| **Idempotencia lógica** (`idem_key`) | ❌ | — | tabla `bus_dedupe` |
| **Ciclo de pull automático** | ❌ | — | `bus_worker.rs` |
| **Payload de órdenes** | ❌ (cuerpos libres) | — | `kind: task` con `payload.cmd` |

⇒ **Decisión**: el Nivel 2 **extiende** `resonance.rs`:
`resonance.rs` = el **bus** (transporte + firma + merge); `bus.rs` (nuevo) = la
**semántica de órdenes** (formato, `idem_key`, lease, anti-rebote por
destinatario, ciclo de pull); `bus_store.rs` (nuevo) = `bus_dedupe` y
`bus_epoch`. **No** se crea carpeta nueva en Drive: se reutiliza
`15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0/tablon-resiliencia` (la de `resonance.rs`)
con un sub-nivel `bus/<node_id>/`. **No** se crea un formato nuevo de carta:
se añaden tipos a `TIPOS`.

**"Controlar el móvil y sus apps, incluso en primer plano".** El root **deja**
un `kind: "cmd"` en el bus con `payload: {"cmd": "abrir_app", "args": {"pkg":
"com.foo"}}`. El móvil lo recoge en el **siguiente pull** (≤15-30 min) y lo
ejecuta **con la app en primer plano**, porque el daemon ya tiene root y las
herramientas de automatización (`automation.rs`, `automation_runtime.rs`,
`adb_client.rs`). **No hace falta ningún método directo de comunicación**: ni
ADB, ni SSH, ni túnel, ni push. El bus basta. Si el root quiere **inmediatez**,
el push best-effort lo baja a segundos **si** hay un endpoint vivo; si no,
espera al pull.

⇒ **Éste es el punto donde el diseño cumple el requisito**: "incluso en primer
plano" no significa "en tiempo real", significa "sin canal de control
dedicado". El bus es ese canal.

### 3.3 Nivel 3 — Reconexión extremo a extremo

**Modelo de sesión.** Inspirado en SSP de mosh, pero **invertido** (el emisor
es el que rota).

| Elemento | Valor | Origen |
|---|---|---|
| `session_id` | `sha256(pubkey_emisor ‖ pubkey_receptor ‖ nonce)` (32 B) | análogo al **Connection ID** de QUIC (§2.3) |
| `nonce` | 16 B de `ring::rand::SystemRandom` | aleatoriedad criptográfica |
| Clave de sesión | `HKDF-SHA256(salt=session_id, ikm=clave_compartida, info="md-session/v1")` → 32 B | `ring::hkdf` (**API exacta NO VERIFICADA**) |
| Rotación | cada `REKEY_AFTER = 300 s` o cada 512 mensajes: `HKDF(prk=clave_i, info="md-rekey/v1"‖contador)` | análogo al rekey de WireGuard |
| Canal | **UDP** | como mosh; alternativa WS/HTTP2 (§3.3.6) |

**El handshake criptográfico**: aquí no se necesita Noise IK completo, porque
el repo ya tiene un canal autenticado de sobra — la **identidad del nodo** es
la clave Ed25519 (`resonance.rs`) y el **bus** autentica los mensajes por
firma. El Nivel 3 sólo necesita **autenticar el transporte** y **derivar la
clave de sesión**.

- **Opción A (recomendada, más simple)**: **derivar la clave de sesión de la
  firma Ed25519**. El emisor firma un `transcript` (session_id + nonce +
  timestamp + pubkey efímera) y el receptor verifica la firma con la pubkey
  Ed25519 **ya conocida por el bus** ⇒ la firma **es** el canal de
  autenticación, y la clave de sesión se deriva por HKDF de (firma + secreto
  compartido). No da secreto hacia adelante, pero reutiliza el trust store de
  `resonance.rs` tal cual.
- **Opción B (si hace falta secreto hacia adelante)**: **Noise IK con X25519**,
  igual que WireGuard, con `ring::agreement` + `ring::hkdf` +
  `ring::aead::CHACHA20_POLY1305`.

⇒ **Decisión**: empezar por la **Opción A**; evaluar la B en la fase 3B.

**El handshake "reconocerse" (tolerante a pérdida).**

```
EMISOR                                          RECEPTOR
  |                                                 |
  |-- HELLO(session_id, nonce, pubkey_ef, seq=1) -->|   (firmado)
  |                                                 |  ¿session_id conocido?
  |                                                 |  sí ⇒ re-ancla (§3.3.4)
  |                                                 |  no ⇒ sesión nueva
  |<-- HELLO_ACK(session_id, challenge, seq=1) -----|
  |                                                 |  verifica ⇒ estado activo
  |-- PROOF = HMAC(clave_sesion, challenge‖nonce) -->|
  |<=========== DATAGRAMAS (seq, ack_window) =====>|
```

- **`seq` monotónico**: el receptor ve `seq` mayor ⇒ actualiza su destino.
- **`ack_window`**: cada datagrama lleva *"el `seq` del último mensaje que he
  recibido"* ⇒ el emisor sabe exactamente qué se perdió y **retransmite sólo
  esa ventana** (N=8). No reenvía el estado entero.
- **Heartbeat cada 3 s**: mantiene viva la ruta y las tablas NAT, actualiza el
  destino aunque no haya datos, y detecta la muerte del enlace en ≤3 s.
- **Reintentos**: el emisor reenvía `HELLO` cada 1 s hasta 10 intentos, con
  backoff; el receptor responde **igual** aunque no reconozca la sesión (para
  no filtrar información).
- **Tolerancia a pérdida**: el handshake **no** exige ACK del ACK. Si se pierde
  el `HELLO_ACK`, el emisor reenvía el `HELLO` con el **mismo** `session_id` y
  `nonce` ⇒ el receptor, al ver un `HELLO` repetido, **no crea una sesión
  nueva**: responde con el mismo `challenge`.

**El roaming (predicción de la dirección del emisor).** **El corazón del Nivel
3**, y es mosh invertido. Del `mosh.org`: *"Every time the server receives an
authentic packet from the client with a sequence number higher than any before,
it sets the packet's source IP address and UDP port number as its new
'target'."* En MegaDaemon el que rota es el **emisor**. Por tanto:

> *Cada vez que el receptor recibe un datagrama **autenticado** con un `seq`
> mayor que cualquiera anterior, la **IP:puerto origen** de ese datagrama pasa
> a ser el nuevo destino de los datagramas de salida.*

Consecuencias: el receptor **nunca necesita preguntar "¿dónde estás?"** en
cuanto ha recibido un solo paquete; el cambio de IP del emisor (WiFi → datos,
o nuevo quick tunnel) **se corrige solo** en ≤1 RTT. ⚠️ **La misma debilidad
que WireGuard documenta** ("Roaming Mischief"): un atacante activo puede
**suplantar la dirección** origen y desviar el tráfico, aunque **no puede
descifrarlo** (cada datagrama va autenticado con AEAD). Se acepta el mismo
riesgo que en WireGuard y en mosh, y se documenta.

**La re-emigración de sesión.** Cuando el **transporte subyacente** cambia
(quick tunnel → relé, WiFi → datos):

- La sesión **no depende del transporte**: `session_id` y `clave_sesion` viven
  en disco (`<state_dir>/sesiones/<session_id>.json`, `0600`).
- El nuevo transporte hace un `HELLO` con el **mismo `session_id`** y `nonce`.
- El receptor: (1) verifica la firma con la pubkey del emisor (ya conocida por
  el bus); (2) reconoce el `session_id` ⇒ **re-ancla**: actualiza su destino al
  nuevo origen y responde `HELLO_ACK` con `seq` = último recibido; (3) el
  emisor sabe por dónde continuar ⇒ **no se pierde el contexto**.
- Si la sesión **no** se puede re-anclar (clave rotada, sesión expirada), el
  emisor abre una sesión nueva y **reconcilia por el bus** (§3.2) el estado que
  falte.

⇒ **Distinción que hay que dejar clara en el código y en la doc**:

| Tipo de continuidad | Mecanismo | ¿Lo garantiza? |
|---|---|---|
| **Contexto de sesión** (qué se estaba haciendo, buffers parciales) | Nivel 3 (mosh-like) | mejor, no garantizado |
| **Continuidad del trabajo** (qué se pidió, qué se hizo, qué falta) | Nivel 2 (bus) | **sí**, es transaccional |

Ésta es la razón por la que el Nivel 2 **no es opcional**.

**Reuso del sistema de tokens.** El repo ya tiene credenciales maduras en
`api_key.rs`: `X-API-Key` o `Authorization: Bearer` (`:11`, `extract_key` en
`:394`), `constant_time_eq` (`:416`), `fingerprint()` → sha16 (`:534`,
exponible sin exponer el secreto; el 403 ya la incluye, `forbidden_body` en
`:557`) y rate limiting por IP (`:430-516`).

- El plano de datos (Nivel 3) usa la **clave de sesión derivada**, **no** la
  API key: la API key es de largo plazo, se puede rotar (`api_key.rs:199`) y
  caduca ⇒ **no puede ser una clave de sesión**.
- El plano de control (Nivel 2) autentica **por firma Ed25519 del nodo** ⇒
  **rotar la API key no invalida el bus**, y viceversa. Es una mejora real
  sobre el estado actual, donde todo depende de la API key.
- `channel.rs` sigue siendo la fuente de la URL canónica y del `paste` ⇒ el
  Nivel 3 **no lo toca**: sólo lo consume.

**Por qué UDP y qué pasa si el sandbox no puede.**

- **UDP** es lo natural (mosh) porque el problema es "la dirección cambió", no
  "la latencia".
- ⚠️ **Riesgo medido del entorno**: el egress del sandbox está interceptado
  ("las conexiones salientes a puertos no-HTTPS mueren con EOF"). **NO
  VERIFICADO** si un datagrama UDP saliente sale. Si no sale ⇒ el Nivel 3
  **sólo** funciona entre el móvil y un nodo con UDP limpio (el PC de casa), y
  el sandbox sigue dependiendo del bus. **Esto NO bloquea el diseño**: el
  Nivel 2 es el que garantiza el trabajo.
- **Alternativa portable**: tunelizar el Nivel 3 dentro de **WebSocket o
  HTTP/2** (como `wstunnel`, §2.7) ⇒ atraviesa proxies HTTP y funciona en
  cualquier red que deje pasar HTTPS. Trade-off: WebSocket añade cabecera y
  estado ⇒ la "predicción de dirección" se degrada a nivel de **conexión WS**,
  no de paquete.
- **Fase 0 (medición obligatoria)**:
  `python3 -c "import socket;s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM);s.sendto(b'x',('1.1.1.1',9999))"`
  — si `sendto` no lanza `EPERM`/`ENETUNREACH`, el sandbox emite UDP.

---

## 4. Batería

**El requisito y el parámetro X.** El modo "silencioso" (sólo *pull* del bus
cada 15-30 min con red) **no debe gastar más de X % diario de batería**.

```
MEGADAEMON_BUS_BATTERY_BUDGET_PCT   (1..10, por defecto 2)
MEGADAEMON_BUS_PULL_MINUTES         (por defecto 15)
MEGADAEMON_BUS_PULL_MAX_MINUTES     (por defecto 30)
```

**X = 2 % por defecto**. **Por qué 2 %**: con una celda de 5000 mAh, 2 % =
**100 mAh/día**. Con 96 pulls/día (cada 15 min) son **≈1,04 mAh por pull** ⇒
un presupuesto holgado para 3 peticiones HTTP cortas. Si la medición real lo
deja más caro, se sube X; si más barato, se baja. **X es un parámetro, no una
constante.** ⚠️ **NO VERIFICADO**: el consumo real de un pull contra la API de
Drive desde el S22 no está medido. **La Fase 0 (§6) lo mide antes de escribir
código.**

**Presupuesto por nivel de batería y estado de carga.** Se reutiliza
`battery_advanced::classify()` (`battery_advanced.rs:1895`), que ya devuelve
`Normal | Conserve | Thermal | Seal` a partir de `DeviceState` (batería,
cargando, temperatura, RAM libre, disco libre) y de `Thresholds`
(`seal_below_pct = 20`, `thermal_warn_c = 45`, `ram_floor_kb = 512 MiB`).

| `PolicyClass` | Batería / carga | Pull | Beacon LAN | Watchdog | Publicación del canal | Sesión Nivel 3 |
|---|---|---|---|---|---|---|
| **Normal** | > 35 % | **cada 15 min** | 15 s | 60 s | como hoy (si cambia el `quick` o caduca el paste, 5 h) | activa si hay push pendiente |
| **Conserve** | ≤ 35 % **sin cargador** | **cada 30 min** | 60 s | **300 s** | sólo si cambia el `quick` | **sólo si hay `cmd` urgente** |
| **Thermal** | ≥ 45 °C | cada 30 min | apagado | 300 s | **desactivada** | apagada |
| **Seal** | ≤ 20 % | **cada 60 min**, sólo con WiFi | apagado | 600 s | desactivada | apagada |

Reglas adicionales: **Cargando** (`charging == true`) ⇒ se permite el nivel
inmediatamente superior (en `Conserve` se puede subir a 15 min) ⇒ el móvil
aprovecha el cargador. **Sólo con WiFi** en `Seal`: los datos móviles cuestan
batería y dinero. El `PolicyClass` se **persiste** en
`<state_dir>/bus/presupuesto.json` para que un reinicio no suba el nivel.

**Mecanismos Android: el problema de fondo.** ⚠️ **Dato clave**: el daemon **no
es una app**. Es un binario nativo bajo Magisk (`/data/adb/service.d/
99-megadaemon.sh`, `deploy/98-cloudflared.sh`, `deploy/tunnel-watchdog.sh`).

| Mecanismo | ¿Disponible para el daemon? | Nota |
|---|---|---|
| `JobScheduler` (ventana *inexacta*, batching de Doze) | **No directamente** | requiere una app registrada |
| `WorkManager` (`setRequiresCharging`, `setPeriodic`) | **No directamente** | igual |
| `AlarmManager.setExactAndAllowWhileIdle` | **No directamente** | hay que *programar* la alarma desde una app; `cmd alarm` sólo lista |
| Doze / App Standby | **Sí, pero congelando** | el proceso se congela ⇒ un hilo con `sleep()` puede no despertar |
| `cmd deviceidle whitelist +<pkg>` | **Sí (root)** | lo deja fuera de Doze ⇒ **gasta más**: vía de emergencia |
| Hilo propio con `sleep()` (lo de `cockpit_server.rs` hoy) | **Sí** | simple; no garantizado en Doze |
| Magisk `service.d` | **Sí** | arranque + supervisor; ya en uso |

⇒ **Vía recomendada**: una **app-shim Android mínima** con un `JobService`:

```kotlin
// JobInfo: ventana INEXACTA; el sistema agrupa las ejecuciones
val j = JobInfo.Builder(JOB_ID, ComponentName(ctx, BusShim::class.java))
    .setRequiredNetworkType(JobInfo.NETWORK_TYPE_ANY)
    .setPeriodic(15 * 60 * 1000)   // ventana inexacta, >=15 min (minimo del SO)
    .setPersisted(true)            // requiere RECEIVE_BOOT_COMPLETED
    .setRequiresCharging(false)    // false en Normal; true en Sellado
    .build()
```

- `setRequiresCharging(true)` ⇒ el pull **espera** a que el móvil esté cargando
  ⇒ **reparto del coste entre periodos de carga**. Es el equivalente de
  `WorkManager.setRequiresCharging`.
- `setPersisted(true)` ⇒ sobrevive a reinicios (hoy el `service.d` de Magisk
  levanta el daemon, pero **no** hay planificación de tareas).
- La app hace `su -mm -c '<binario> bus pull --once'` y termina.
- **Límite del SO**: el periodo mínimo de `JobScheduler` es **15 minutos** ⇒
  "cada 15 min" es el mínimo por esa vía; para intervalos menores, el hilo
  propio del daemon.

**Wakelocks**: el daemon **no debe tomar wakelocks**. La forma correcta en
Android es devolver la radio a Doze entre ventanas; tomar un wakelock desde un
binario nativo exigiría JNI (el crate no lo tiene) o `/sys/power/wake_lock`
(API obsoleta, **NO VERIFICADO** que funcione en Android 14 con Magisk
Kitsune). ⇒ **Decisión: cero wakelocks**; el trabajo se agrupa en una ventana
corta y se devuelve la radio.

**Coste estimado por evento.** ⚠️ **Todas las cifras de esta tabla son
ESTIMACIONES NO VERIFICADAS.** No he medido el consumo en el S22.

| Evento | Frecuencia actual | Radio ON | Bytes | Coste estimado | Cómo medir |
|---|---|---|---|---|---|
| **Pull completo del bus** (list + 0..3 GET + 1 POST) | 0 (no existe) | 3-6 s | 40-120 kB | **0,05-0,30 mAh** | Fase 0: 50 pulls + `current_now`×`voltage_now` |
| **Sondeo HTTP a un endpoint** (`probe_url_detalle`) | **1440/día** (watchdog, 60 s) | ~1 s | < 5 kB | **0,01-0,05 mAh** ⇒ **0,3-0,7 mAh/día** | `/proc/<pid>/io` + `current_now` |
| **Publicación del canal** (paste + repo) | ~6/día (si rota el `quick`) | 2-4 s | ~60 kB | **0,05-0,20 mAh** | igual |
| **Beacon LAN** (UDP difusión, 15 s) | **240/día** | n/a (link-local) | < 1 kB | red ≈0; **CPU ≈0,01-0,05 mAh** | `dumpsys batterystats` filtrando el UID |
| **Diagnóstico a Drive** (`publicar_periodico(30)`) | 48/día | 3-10 s | 100-500 kB | **0,2-1,0 mAh** | Fase 0 |
| **Sesión Nivel 3 activa** (heartbeat 3 s) | 28.800/día | permanente | ~2 kB | **3-10 mAh** | idem |
| **`state/channel.json` al repo** | ≤6/día | 2 s | 1 kB | <0,02 mAh | idem |

**Lectura honesta de la tabla**: el **watchdog a 60 s (1440 sondas/día) es
probablemente el mayor coste del sistema actual**, por encima del pull del bus.
Por eso la tabla de presupuesto **baja el watchdog a 300 s en `Conserve`** y a
600 s en `Seal`: **optimizar el watchdog libera más batería que optimizar el
bus**.

⇒ **Presupuesto del modo silencioso** (objetivo ≤ 100 mAh/día ≈ 2 %):

```
96 pulls/día      x 0,15 mAh  = 14,4 mAh
1440 sondas/día   x 0,02 mAh  = 28,8 mAh   (si se baja a 300 s: 4,8 mAh)
240 beacons/día   x 0,03 mAh  =  7,2 mAh
6 publicaciones   x 0,10 mAh  =  0,6 mAh
-------------------------------------------------------
TOTAL modo silencioso optimizado:  ~27 mAh/dia ~ 0,54 %   OK
TOTAL sin optimizar el watchdog:  ~51 mAh/dia ~ 1,02 %
```

⇒ Margen amplio: **X=2 % deja 2× de holgura** para el caso de que el pull real
sea más caro de lo estimado y para los imprevistos (el daemon completo hace
muchas más cosas que el bus).

**Cómo medirlo de verdad.** **Instrumentación (mínima, reutilizable):**
(1) añadir `current_now` y `voltage_now` a `BatterySnapshot`
(`battery_advanced.rs:154`), que hoy sólo lee `capacity` — con esos dos campos
`P_mW = V_mV x I_mA / 1000`; (2) un hilo de muestreo (o el propio
`bus_budget`) que integra la potencia cada 30 s y guarda el acumulado en
`<state_dir>/bus/energia-<fecha>.json`; (3) `tools/medir-bus.sh` desde el root,
que compara el acumulado con `dumpsys batterystats --charged` del paquete del
daemon.

**Aislamiento del experimento (el punto clave)**: **A-B** con el mismo móvil y
red, 24 h con el bus **desactivado** vs. 24 h con el bus en modo silencioso; la
diferencia es el coste del bus. El resto (watchdog, beacon, diagnóstico) se
**mantiene igual** en ambos brazos; para aislar el watchdog, un tercer brazo
con el watchdog a 300 s.

**Aceptación**: el brazo con bus en modo silencioso no debe consumir **más de
X %** (2 % por defecto) en 24 h con el móvil **en reposo y la pantalla apagada**.

---

## 5. Integración opcional con WireGuard

**Qué es**: un **cuarto transporte opcional**. El daemon sabe generar su par de
claves WireGuard, publicarlo en el bus (Nivel 1) y consumir configs de peers.

1. Genera/guarda su keypair Curve25519 en `<state_dir>/wg/<nodo>.key` (`0600`).
2. Publica la **pubkey** en su carta de presencia (§3.1) ⇒ los demás nodos
   pueden configurarlo como peer sin pegarse claves en el chat.
3. Acepta una config WG y la escribe en `<state_dir>/wg/<nodo>.conf` (`0600`):
   ```ini
   [Interface]
   PrivateKey = <...>          # NUNCA sale del movil
   Address = 10.77.0.2/32
   DNS = 1.1.1.1
   [Peer]
   PublicKey = <pubkey del peer>
   AllowedIPs = 10.77.0.1/32
   Endpoint = <host>:<port>
   PersistentKeepalive = 25
   ```
4. **No** levanta WireGuard por sí mismo: escribe la config y, si encuentra
   `wg`/`wg-quick`, la aplica. **NO VERIFICADO** que haya `wireguard-tools` en
   el S22 (Termux puede instalarlas, pero el `service.d` de Magisk no hereda el
   PATH de Termux ⇒ habría que usar la ruta absoluta o copiar el binario al
   prefijo del daemon, como ya se hace con `cloudflared` en
   `98-cloudflared.sh:22-38`).

**Qué se reutiliza**: `rustls`/`ring` (handshake propio del Nivel 3; WireGuard
no necesita TLS) · `ureq` (descargar config o módulo del kernel) ·
`drive_native` + `google_auth` (transportar el config de forma autenticada y
sin `curl`, patrón de `task_sync.rs`) · `api_key` (autorizar **quién** puede
cambiar la config de WG: sólo el root con la API key vigente) · `resonance`
(publicar la pubkey del nodo en el bus).

**Qué es opcional y qué NO debe bloquear el modo por defecto.** **Regla dura**:
*ningún camino del Nivel 1, 2 o 3 puede depender de que WireGuard esté
instalado, configurado o funcionando.* Si el modo WG falla, el bus sigue
funcionando por Drive/repo/relé.

| | Por defecto | Opcional |
|---|---|---|
| Descubrimiento (Nivel 1) | ✅ por bus (HTTP) | ⬜ por WG (`AllowedIPs` como tabla de rutas) |
| Órdenes (Nivel 2) | ✅ por Drive/repo | ⬜ por WG (mismo bus, otro transporte) |
| Datos (Nivel 3) | ✅ por HTTP/UDP propio | ⬜ por WG |
| Latencia | segundos-minutos | milisegundos (a cambio de batería y complejidad) |

**El modo WG es, para MegaDaemon, una optimización de latencia**, no un
requisito de conectividad. La conectividad la da el bus.

**Aviso de seguridad.** `AllowedIPs` es **a la vez tabla de rutas y ACL** (paper
de WireGuard, §"cryptokey routing"). Un peer con `AllowedIPs = 0.0.0.0/0` puede
**capturar todo el tráfico** del móvil; en un dispositivo con root, eso es un
vector de escalada de privilegios. ⇒ **Requisitos del modo WG**: (1)
**`confirm=true` obligatorio** (política de `docs/API_V2.md` §5.15, ya aplicada
en `channel.rs`); (2) `AllowedIPs` **restrictivo** por defecto
(`10.77.0.0/24`), nunca `0.0.0.0/0` sin `confirm` explícito; (3) **auditoría**:
cada cambio deja un evento en `board.task_events` (`board.rs:649-658`) con el
hash del config, **nunca** las claves privadas; (4) **el `PrivateKey` no sale
nunca** del móvil: ni al bus, ni a Drive, ni al repo — con un test, como el que
ya existe para el beacon (`lan.rs:266-275`, `el_beacon_no_lleva_ningun_secreto`).

---

## 6. Plan de implementación por fases

**Regla de convivencia** (`docs/COORDINATION.md` §2): *"Un agente = un módulo
Rust. Nadie toca `lib.rs`, `main.rs`, `Cargo.toml` ni módulos de otro agente sin
acuerdo explícito en este tablón."* ⇒ Los ficheros marcados **"acuerdo"**
requieren tablón antes de tocarse; los marcados **"nuevo"** son módulos nuevos
⇒ **sólo `lib.rs` necesita acuerdo** (por los `pub mod`).

| Fase | Objetivo | Ficheros nuevos | Ficheros a tocar (acuerdo) | Líneas (est.) |
|---|---|---|---|---|
| **0** | **Medir** el coste de un pull y la viability de UDP | `tools/medir-bus.sh` (80) | — | 80 shell |
| **1** | **Nivel 1**: identidad + endpoints | `rust-core/src/node_identity.rs` (260)<br>`rust-core/src/endpoint_registry.rs` (320)<br>`rust-core/src/bin/md-nodes.rs` (140) | `rust-core/src/lib.rs` (**+2 `pub mod`**)<br>`rust-core/src/channel.rs` (+60)<br>`rust-core/src/cockpit_server.rs` (+30) | ~800 Rust |
| **2** | **Nivel 2**: bus PULL | `rust-core/src/bus.rs` (700)<br>`rust-core/src/bus_store.rs` (380)<br>`rust-core/src/bus_transport.rs` (420)<br>`rust-core/src/bus_worker.rs` (300)<br>`rust-core/src/bin/md-bus.rs` (180) | `rust-core/src/lib.rs` (**+4 `pub mod`**)<br>`rust-core/src/resonance.rs` (+~80: nuevos tipos de carta) | ~1.980 Rust |
| **3** | **Nivel 3**: sesión mosh-like | `rust-core/src/session.rs` (520)<br>`rust-core/src/session_udp.rs` (480)<br>`rust-core/src/session_store.rs` (240)<br>`rust-core/src/bin/md-session.rs` (160) | `rust-core/src/lib.rs` (**+3 `pub mod`**)<br>`rust-core/Cargo.toml` (sólo si `ring` necesita features) | ~1.400 Rust |
| **4** | **Batería** | `rust-core/src/bus_budget.rs` (280)<br>`rust-core/src/bin/md-bus-budget.rs` (120)<br>`app-shim/` (Kotlin, 180) | `rust-core/src/battery_advanced.rs` (+25: `current_now`/`voltage_now`)<br>`rust-core/src/lib.rs` (**+1 `pub mod`**) | ~400 Rust + 180 Kotlin |
| **5** | **Modo WG (opcional)** | `rust-core/src/wg_config.rs` (300) | `rust-core/src/lib.rs` (**+1 `pub mod`**) | ~300 Rust |
| **6** | **Verificación en el móvil** | `tools/medir-resiliencia.sh` (200) | — | 200 shell |

**Totales**: ~5.100 líneas Rust nuevas, ~180 Kotlin, ~480 shell. **Impacto en el
binario**: **NO VERIFICADO**; estimación ~150-250 KB sobre los ~33,8 MB
actuales (`docs/AUTORREPARACION.md`, "Cifras medidas") ⇒ **< 1 %**.

**Qué NO se toca nunca**: `rust-core/Cargo.toml` (no se añaden crates; `ring`
0.17, `rusqlite`, `serde`, `ureq`, `rustls` ya están — se toca **sólo** si la
fase 3 necesita activar una feature de `ring`) · `rust-core/src/main.rs` (los
nuevos binarios son `rust-core/src/bin/md-*.rs`, patrón existente: `md-tasks`,
`md-board`, `md-lan-listen`) · `rust-core/src/board.rs` (el bus **usa** el
board) · `deploy/99-megadaemon.sh` y `deploy/98-cloudflared.sh` (el bus es un
proceso más, no un cambio del supervisor).

**Criterios de aceptación por fase.**

| Fase | Criterio |
|---|---|
| 0 | Número medido de mAh por pull. Se sabe si el sandbox emite UDP. |
| 1 | `md-nodes where <nodo>` devuelve el endpoint **vivo** tras un relanzamiento de `cloudflared`, en < 60 s, **sin** que nadie pegue la URL. |
| 2 | Un `cmd` depositado en el bus llega al móvil y se ejecuta, y el ACK vuelve al emisor. Repetir el depósito **no** re-ejecuta (idem_key). |
| 3 | Con la red del móvil cambiando (WiFi → datos), la sesión sobrevive y el emisor sigue viendo el mismo `session_id`. |
| 4 | 24 h de modo silencioso ⇒ **≤ X %** de batería (X=2 por defecto), medido A-B. |
| 5 | (Opcional) Un peer WG alcanza al móvil y el bus **sigue funcionando** con el WG apagado. |
| 6 | El escenario completo (matar `cloudflared` + cambiar de red) se recupera sin intervención manual. |

---

## 7. Riesgos

| # | Riesgo | Impacto | Mitigación |
|---|---|---|---|
| **R-1** | **El sandbox no puede emitir UDP saliente** (egress interceptado). | El Nivel 3 no se puede validar desde el sandbox | **NO VERIFICADO** (Fase 0). El Nivel 2 no depende de UDP ⇒ el diseño no se bloquea |
| **R-2** | **El `raw` de GitHub cachea en CDN** ⇒ el bus por repo entrega mensajes viejos | Latencia y, si no se rompe la cache, *corrección* | Bust de cache con query param `?t=<epoch>` + **anti-rebote por epoch/seq en el receptor** ⇒ la caché degrada la latencia, nunca la corrección |
| **R-3** | **Drive no da "leer y marcar" atómico** ⇒ dos nodos leen el mismo mensaje | Doble ejecución | `idem_key` + `board.claim()` (un solo winner) + ACK |
| **R-4** | **El reloj del móvil va atrás** | Un mensaje "nuevo" parece viejo | epoch/seq por emisor, **nunca** timestamps para el anti-rebote (mismo criterio que el "reloj monótono" de `board.rs`) |
| **R-5** | **Dos hostnames canónicos distintos**: `channel.rs:72` usa `mdtmfnc.duckdns.org` pero `state/channel.json` dice `mdtmfnc-s21.duckdns.org` | La flota no sabe a cuál ir | **MEDIDO en el repo.** Decidir el canónico único antes de la Fase 1 y dejar **un** hostname en `channel.rs`, `state/channel.json` y `deploy/relay-pc.sh` |
| **R-6** | **La API key es de largo plazo y rotables** | Si se usara como clave de sesión, rotarla tumba las sesiones | El Nivel 3 usa clave de sesión derivada; el bus usa **firmas Ed25519** ⇒ la rotación de la API key no invalida nada |
| **R-7** | **`ring` no expone hoy X25519/HKDF/AEAD en el código del repo** (sólo `ring::signature` y `ring::rand`) | La fase 3 se bloquea en la compilación | **NO VERIFICADO**: confirmar la feature `alloc` de `ring` 0.17 y la API exacta. **Plan B**: el handshake por firma Ed25519 (§3.3) no necesita nada nuevo |
| **R-8** | **Doze congela el proceso del daemon** ⇒ el pull del hilo propio no despierta | El móvil "desaparece" otra vez | App-shim con `JobScheduler` (§4) + `cmd deviceidle whitelist` como red de seguridad |
| **R-9** | **Crear un bus paralelo a `resonance.rs`** | Dos formatos, dos carpetas, dos fuentes de verdad ⇒ el sistema más confuso es el que se ha creado | **El Nivel 2 extiende `resonance.rs`** (§3.2). Si alguien abre una carpeta nueva en Drive sin acuerdo en el tablón ⇒ **rechazado** |
| **R-10** | **El watchdog a 60 s (1440 sondas/día) domina el presupuesto de batería** | El modo silencioso no cumple X | Bajar el watchdog a 300/600 s en `Conserve`/`Seal` (§4) |
| **R-11** | **`AllowedIPs = 0.0.0.0/0` en el modo WG** | Escalada de privilegios en un móvil con root | `confirm=true` obligatorio, `AllowedIPs` restrictivo por defecto, auditoría en `task_events`, test de "no secretos" |
| **R-12** | **El emisor no distingue "el móvil está caído" de "la credencial caducó"** | Reintentos infinitos | **Ya resuelto**: `probe_url_detalle` distingue 530/000/502/503/504 (muerto) de 401/403 (vivo) (`channel.rs:309-361`). Reutilizar **sin cambiar la semántica** |

**Lo que queda explícitamente `NO VERIFICADO`**: (1) **consumo real en mAh** de
pull, sondeo, beacon, publicación, diagnóstico y sesión UDP — todas las cifras
de la tabla de coste son estimaciones; (2) **coste real del modo silencioso**
(el A-B de 24 h); (3) **si el sandbox puede emitir UDP saliente** (R-1);
(4) **API exacta de `ring` 0.17** para `agreement::X25519`, `hkdf` y
`aead::CHACHA20_POLY1305` y si la feature `alloc` está activa; (5) **si el S22
tiene `wireguard-tools`**; (6) **si `JobScheduler`/`WorkManager` son
accesibles** desde el shim en Android 14 con Magisk Kitsune; (7)
**comportamiento de `cmd deviceidle whitelist`** con un binario root fuera de
un APK; (8) **si el `raw` de GitHub respeta el bust por query param** en
`state/bus/...`; (9) **latencia real** de Drive desde el móvil (debería ser
~200-800 ms vía `googleapis.com`, no por el CDN); (10) **"Angelic"**: no se ha
identificado ningún proyecto relevante con ese nombre — si designa algo
concreto, este documento no lo cubre; (11) **el título del paper de mosh** del
encargo no existe con ese nombre (§2.1); (12) **coste en binario** del Nivel 3
(~150-250 KB estimados, no medidos).

---

## 8. Referencias

**Primarias (leídas):**

- mosh.org — <https://mosh.org/>
- Winstein & Balakrishnan, *"Mosh: An Interactive Remote Shell for Mobile Clients"*, USENIX ATC '12 — <https://dspace.mit.edu/bitstream/handle/1721.1/71700/mosh-paper-1.pdf?sequence=1>
- Winstein & Balakrishnan, *"Mosh: A State-of-the-Art Good Old-Fashioned Mobile Shell"*, USENIX `;login:` vol. 37 nº 4, ago 2012 — <https://www.usenix.org/system/files/login/issues/august2012.pdf>
- RFC 9000 (QUIC Transport) — <https://www.rfc-editor.org/rfc/rfc9000.html>
- RFC 9001 (Using TLS to Secure QUIC) — <https://www.rfc-editor.org/rfc/rfc9001.html>
- RFC 9308 (QUIC Applicability) — <https://www.rfc-editor.org/rfc/rfc9308.html>
- RFC 8684 (Multipath TCP) — <https://www.rfc-editor.org/rfc/rfc8684.html>
- RFC 5389 (STUN) — <https://www.rfc-editor.org/rfc/rfc5389.html>
- RFC 8656 (TURN) — <https://www.rfc-editor.org/rfc/rfc8656.html>
- WireGuard, *Protocol & Cryptography* — <https://www.wireguard.com/protocol/> ⚠️ *(el lector de páginas no lo extrajo; el texto citado procede del índice de búsqueda y del paper)*
- WireGuard whitepaper (§2.1 "Endpoints & Roaming") — <https://www.wireguard.com/papers/wireguard.pdf>
- WireGuard, *Known Limitations* ("Roaming Mischief") — <https://www.wireguard.com/known-limitations/>
- Tailscale, *DERP servers* — <https://tailscale.com/docs/reference/derp-servers>
- Tailscale, *Connection types* — <https://tailscale.com/docs/reference/connection-types>
- Tailscale, *Device connectivity* — <https://tailscale.com/docs/reference/device-connectivity>
- Tailscale, *Peer Relays* — <https://tailscale.com/docs/features/peer-relay>
- Tailscale, blog *Peer Relays in international networks* — <https://tailscale.com/blog/peer-relays-international-networks>
- Syncthing, *Relay Protocol v1* — <https://docs.syncthing.net/specs/relay-v1.html>
- Syncthing, *Relaying* — <https://docs.syncthing.net/users/relaying.html>
- BitTorrent, *BEP 5: DHT Protocol* — <https://www.bittorrent.org/beps/bep_0005.html>
- OpenSSH Cookbook, *Observing Multiplexing* — <https://en.wikibooks.org/wiki/OpenSSH/Cookbook/Multiplexing>
- `ssh_config(5)` (ControlMaster / ControlPath / ControlPersist) — <https://manpages.ubuntu.com/manpages/bionic/man3/Config::Model::models::Ssh.3pm.html>
- `autossh(1)` — <https://man.archlinux.org/man/autossh.1>
- `wstunnel` (erebe) — <https://github.com/erebe/wstunnel/blob/main/README.md>
- `wstunnel` (sammck-go) — <https://github.com/sammck-go/wstunnel>
- ICE (borrador) — <https://datatracker.ietf.org/doc/draft-ietf-ice-rfc5245bis/09/>
- MPTCP operational experience — <https://datatracker.ietf.org/doc/draft-ietf-mptcp-experience/05/>
- MultipathTester (MPTCP vs MPQUIC) — <https://inl.info.ucl.ac.be/system/files/multipathtester.pdf>
- IRTF ANRW 2021, *"Leveraging the 0-RTT Convert Protocol to improve Wi-Fi/Cellular convergence"* — <https://www.irtf.org/anrw/2021/anrw2021-final38.pdf>
- Xue et al., Sec'24, *"Fingerprinting Obfuscated Proxy Traffic with Encapsulated TLS Handshakes"* — <https://www.usenix.org/system/files/sec24summer-prepub-465-xue.pdf>
- Wu & Wang, Sec'23, *"How the Great Firewall of China Detects and Blocks Fully Encrypted Traffic"* — <https://www.usenix.org/system/files/usenixsecurity23-wu-mingshi.pdf>
- Fork de sing-box documentando los protocolos de Xray (fragmentación, XHTTP, REALITY) — <https://github.com/TrayBer/Mochi/blob/lx/docs-lx/xray-protocols-explained.md>
- Comparativas de mesh VPN (Headscale / Netmaker / ZeroTier / NetBird) — <https://modem7.github.io/cheat-sheets/networking/mesh-vpn-comparison/> y <https://www.ssdnodes.com/learn/tailscale-alternatives>

**Internas (leídas):**

`rust-core/src/channel.rs` (613) · `rust-core/src/resonance.rs` (483) ·
`rust-core/src/task_sync.rs` (183) · `rust-core/src/board.rs` (1960) ·
`rust-core/src/lan.rs` · `rust-core/src/api_key.rs` ·
`rust-core/src/battery_advanced.rs` · `rust-core/src/alerts.rs` ·
`rust-core/src/drive_native.rs` (870) · `rust-core/src/google_auth.rs` (483) ·
`rust-core/src/diagnostico.rs` · `deploy/tunnel-watchdog.sh` (233) ·
`deploy/98-cloudflared.sh` (158) · `docs/RESILIENCE.md` ·
`docs/AUTORREPARACION.md` · `docs/COORDINATION.md` · `state/channel.json`
