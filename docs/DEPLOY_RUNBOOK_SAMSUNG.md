# Runbook de despliegue en el Samsung (instancia maestra)

> Procedimiento verificado el 2026-11-19 durante el despliegue del binario
> ARM64 `4b27c1a3…` (v13.4.0). Escribido porque el despliegue **dejó la web
> caída** y el post-mortem estaba sólo en el chat, que se pierde.
>
> Regla de oro: **el móvil no puede quedarse sin supervisor.** Si el supervisor
> muere, nada relanza el daemon y la web queda muerta (ERR_CONNECTION_REFUSED)
> hasta que alguien mire el teléfono.

---

## 1. Identidad de la instancia

| Dato | Valor |
|---|---|
| Dispositivo | Samsung SM-G998B (S21 Ultra, Exynos 2100), Android 14 |
| Rol | `primary` (maestra) — decide, reparte y sincroniza |
| state_dir | `/data/local/tmp/megadaemon/state` (real: leído de `/proc/<pid>/environ`, `MEGADAEMON_STATE_DIR`) |
| Binario | `/data/local/tmp/megadaemon/megadaemon-core` (ARM64, 6.409.448 B) |
| Supervisor | `/data/adb/service.d/99-megadaemon.sh` (chmod 755) |
| Puente | `/data/local/tmp/megadaemon/web_bridge.py` (python3 de Termux) |
| Tablero | `/data/local/tmp/megadaemon/state/board.sqlite` (0600) |
| UI | `http://localhost:3000` (móvil) · `http://192.168.1.124:3000` (LAN) |
| Túnel | `https://webpage-ultram-please-communicate.trycloudflare.com` |

---

## 2. Orden de despliegue (NO improvisar)

```
0) ANUNCIAR por `communicate` antes de tocar el móvil. Nadie reinicia
   supervisor/daemon/puente sin avisar.

1) PARAR:  detener el daemon, el supervisor viejo y el puente.
   Comprobar: `pidof megadaemon-core` vacío y ningún `web_bridge.py` vivo.

2) INSTALAR el binario nuevo (con .bak): verificar SHA-256 en destino ANTES
   de tocar el daemon. Si no cuadra, restaurar el .bak y parar.

3) INSTALAR el supervisor canónico desde el repo:
   deploy/99-megadaemon.sh → /data/adb/service.d/99-megadaemon.sh (755).

4) ARRANCAR EL SUPERVISOR CON `am broadcast` A TERMUX  ←  CLAVE
   El supervisor debe sobrevivir al cierre del root_exec que lo lanzó.
   Un root_exec suelto muere con la shell y deja el móvil SIN nada.
   Los dos cloudflared siguen vivos y el túnel da 502 si nadie escucha.

5) VERIFICAR (ver §4). Si algo falla, NO seguir: restaurar el .bak.

6) AVISAR por `communicate` de que la ventana está cerrada, con las medidas.
```

---

## 3. Las tres trampas que costaron el despliegue

### 3.1 El banner MIENTE sobre el bind

El banner imprime SIEMPRE el literal `http://127.0.0.1:<p>`, aunque el bind
real sea `0.0.0.0` + `[::1]`. Además saca **una línea por listener** (IPv4 y
IPv6 del mismo puerto), así que "9090" aparece repetido y parece el único
puerto.

⇒ **No diagnostiques con el banner.** La verdad está en:
```sh
(ss -ltn || netstat -ltn) | grep -E ':(9090|3000|3030|8090|8080|80) '
curl -s http://127.0.0.1:9090/health        # trae "bind" y "ports_open"
```
Ya arreglado (commit `f089bd9`): el banner dice el bind real y un puerto por
línea, deduplicado.

### 3.2 El daemon y el puente se pelean por los puertos

El daemon nuevo sirve 3000, 3030, 8090 y 9090. El puente histórico servía
también 3000, 3030, 8090, 8080 y 80. Con ambos vivos: `EADDRINUSE`, y el
puente **reintentaba cada 5 s** → 660 líneas de "reintento" en `bridge.log`.
Eso es un bucle de polling en un móvil que va JUSTO de RAM (10,2 GB de 10,5 GB
usados): prohibido.

⇒ Arreglado (commit `a2b263f`): el puente **cede** el puerto que ya sirve otro
proceso y no reintenta. Reparto actual y correcto:

| Puerto | Quién lo sirve |
|---|---|
| 3000, 3030, 8090, 9090 | el daemon (IPv4 + IPv6) |
| 80, 8080 | el puente (IPv4 + IPv6) |

Cuando el binario definitivo sirva también 80 y 8080, **el puente puede
borrarse del todo** (y con él el keeper y el python extra). Hasta entonces, no.

### 3.3 El "todo murió a la vez" NO fue OOM

El puente murió con **SIGTERM** ("Terminated"); el OOM es SIGKILL. Un OOM no
dice "Terminated". Fue un **kill de grupo de procesos** (el supervisor viejo
reiniciando, o el shell de Termux al caer). El daemon pesa 8,9 MB RSS / 4
hilos: no es el culpable. Para confirmarlo en el futuro:
`dmesg | tail` y `logcat -b crash` (buscar "lowmemorykiller" / "oom").

---

## 4. Verificación (las 9 rutas + el túnel)

```sh
# en el móvil (python3 de Termux; NO hay curl ni toybox wget)
for u in http://127.0.0.1:3000/ http://[::1]:3000/ http://192.168.1.124:3000/ \
         http://127.0.0.1:3030/ http://127.0.0.1:8090/ http://127.0.0.1:8080/ \
         http://127.0.0.1:80/ http://localhost/ http://127.0.0.1:9090/ ; do
  python3 -c "import urllib.request,sys; r=urllib.request.urlopen(sys.argv[1],timeout=8); print(sys.argv[1],r.status,len(r.read()))" "$u"
done
```
Todas deben dar **200** con la UI real (~89.680 B). Y:
```sh
grep -c rate_limited /sdcard/megadaemon.log      # 0 nuevos
grep -c "puerto alias" /sdcard/megadaemon.log     # 0 ⇒ ningún EADDRINUSE
grep -c "reintento"  /data/local/tmp/megadaemon/bridge.log  # congelado
ps -A -o pid,etime,args | grep -E "megadaemon-core|web_bridge|99-megadaemon"
```

Desde fuera (túnel):
```sh
curl -s -o /dev/null -w "%{http_code}\n" https://<tunel>/            # 200 (UI, exento del gate)
curl -s -o /dev/null -w "%{http_code}\n" https://<tunel>/health      # 200 (sonda)
curl -s -o /dev/null -w "%{http_code}\n" https://<tunel>/api/status  # 401 sin API key
```

**El HTML público NO debe llevar credenciales incrustadas** (si las lleva, el
túnel es un RCE root). Comprobación:
```sh
curl -s https://<tunel>/ | grep -c "$MEGADAEMON_COCKPIT_TOKEN"   # debe ser 0
```

---

## 5. Receta de compilación ARM64 (medida)

```sh
export NDK=/workspace/ndk/android-ndk-r26c/toolchains/llvm/prebuilt/linux-x86_64
export CARGO_HOME=/run/agent-runtime/md/cargo2           # registro LOCAL (fuera del NFS)
export CARGO_TARGET_DIR=/run/agent-runtime/md/target-arm # artefactos LOCALES (ext4)
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=$NDK/bin/aarch64-linux-android26-clang
export CC_aarch64_linux_android=$NDK/bin/aarch64-linux-android26-clang
export AR_aarch64_linux_android=$NDK/bin/llvm-ar        # el NDK r26c NO trae aarch64-linux-android-ar
export CARGO_PROFILE_RELEASE_DEBUG=0
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
cargo build --release --target aarch64-linux-android -j 1
```

Por qué: `/workspace` está en **NFS v3 (nolock, hard)** → compilar ahí tarda
8+ min y falla con "database is locked" / "failed to open object file". Con
target y registro en disco local, el mismo build baja a minutos. Y la caja tiene
**3 GB de RAM**: release+LTO+codegen-units=1 muere por SIGKILL ⇒ `-j 1` +
`debug=0` + `codegen-units=16`.

Ver el binario antes de subirlo:
```sh
strings megadaemon-core | grep -E "puerto alias|loopback IPv6"   # si aparece, trae alias y dual-stack
sha256sum megadaemon-core
```

---

## 6. `/sdcard/megadaemon.url`

Lo escribe el `write_url` del supervisor con los puertos que funcionan:
`80/3000/3030/8090/8080` + HEALTH. Mientras el daemon NO sirva 80 y 8080, esa
lista es la correcta (el puente los cubre). Si el próximo binario los sirve
también, hay que ajustar la lista a los alias reales (3000/3030/8090) y borrar
el puente.

---

*Runbook del agente de coordinación y dispositivo. Medido, no supuesto: cada
punto tiene su comando de comprobación.*
