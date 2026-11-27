# Autorreparación y actualización total del móvil

*Medido y escrito el 2026-11-25, después de que el móvil quedara en un bucle
(arriba, moría, el supervisor lo reiniciaba) y de que la Web UI perdiera la
vista de Antigravity.*

## El problema real

El binario se actualizaba, pero **el resto del stack no**. El supervisor
instalado en el móvil era una versión vieja cuyo prefijo era la ruta
*volátil*:

```text
[2026-11-25 12:00:57] ERROR: no existe /data/local/tmp/megadaemon/megadaemon-core
                         (y no hay despliegue en curso)
```

El binario canónico vive en `/data/adb/megadaemon` ⇒ el supervisor no lo
encontraba ⇒ **no podía relanzar el daemon** ⇒ el móvil se quedaba sin
daemon (UI y gateway caídos) y entraba en bucle de reinicios. Además:

- la Web UI servida era la **V1** (7 vistas, sin Antigravity) porque el HTML
  en disco era viejo;
- el NDJSON del shim no tenía **ni una línea nueva desde el 2026-11-23** ⇒ el
  language server no estaba hablando con el gateway;
- el daemon real sirve en el **9090** (`PORT=9090` en el supervisor), no en el
  3000 ⇒ un health check en 3000 da un **falso negativo**;
- el proceso supervisor en ejecución era el viejo, aunque su fichero ya
  fuera el nuevo ⇒ había que **matarlo** para que cargara la versión buena.

## Las tres piezas

### 1. Stack autocontenido (`rust-core/src/stack.rs`)

El binario lleva **dentro** el stack: supervisor, keeper, cloudflared,
ADB-TCP, los scripts de Antigravity y la Web UI (v2 + css + v1).

- `reconciliar()` compara el SHA-256 de lo que hay en disco con lo embebido y
  reescribe **sólo lo que difiere** (idempotente, escritura atómica
  `.tmp` + `rename`).
- `reiniciar_supervisor()` mata el supervisor viejo cuando su fichero cambia
  (lo revive el keeper, que lo comprueba cada 60 s).
- Al arrancar, el daemon reconcilia y **vuelve a hacerlo cada 10 min**.

> **Regla de oro:** el stack que se reconcilia es siempre el del **binario en
> ejecución** ⇒ nunca hay mezcla de versiones.

Integrado en: `GET/POST /api/v2/stack`, tool MCP `stack`, panel en la Web UI
y CLI `megadaemon-core stack [--reconciliar] [--json]`.

### 2. Build meta (`rust-core/src/bundle.rs`)

Un artefacto único con **todo** dentro: el binario, el stack y un
`manifest.json` con el SHA-256 de cada fichero.

- El daemon lo descarga, **verifica los 10 hashes antes de tocar nada** y lo
  instala. Si un hash no coincide, no instala nada (el móvil no se queda a
  medias).
- Destinos: `prefijo/…` → `<prefijo>/…`; `servicio/…` →
  `<padre del prefijo>/service.d/…`.
- `tools/construir-bundle.sh` lo empaqueta de forma reproducible; el CI lo
  publica como asset `megadaemon-movil.tar.gz` (medido: 12.569.641 B).

CLI: `megadaemon-core bundle [--instalar] [--json]`.

### 3. Diagnóstico automático (`rust-core/src/diagnostico.rs`)

El nodo recoge su estado completo (procesos con RSS, puertos en LISTEN,
`/health`, stack, binario y su SHA-256, energía, red, log del supervisor y
NDJSON del shim) y **lo publica en Drive** en `MegaDaemon/diagnostico/<nodo>/`.

- El root lo lee desde el sandbox con `tools/leer-diagnostico.py`: **sin ADB,
  sin túnel y sin que nadie pegue texto en el chat**.
- El daemon publica cada `diagnostico.publicar_min` (30 por defecto).
- **Sin secretos**: no se cita ni la API key, ni el token de sesión, ni las
  credenciales de Drive.

## Cómo se usa (desde el móvil, con root)

```sh
cd ~/storage/downloads/MiniMax
curl -fsSL -o r.sh https://raw.githubusercontent.com/boolforge/megadaemon/8dfea14/tools/recuperar-en-movil.sh
curl -fsSL -o d.sh https://raw.githubusercontent.com/boolforge/megadaemon/8dfea14/tools/diagnostico-movil.sh
sudo sh r.sh     # instala binario + supervisor + keeper y reinicia el stack
sudo sh d.sh     # publica el diagnóstico en Drive
```

Y desde el root:

```sh
tools/leer-diagnostico.py          # el informe del móvil, sin ADB
tools/leer-diagnostico.py --listar # qué hay publicado
```

## Cifras medidas

| | valor |
|---|---|
| binario ARM64 antes del strip | 42.988.440 B (41,00 MiB) |
| binario ARM64 con strip + payload comprimido | 34.587.920 B (32,99 MiB) |
| binario ARM64 **con el stack embebido** | **33.792.160 B (32,23 MiB)** |
| bundle meta (`megadaemon-movil.tar.gz`) | 12.569.641 B (11,99 MiB) |
| ficheros del bundle verificados byte a byte | 10/10 |
| tests del CI | 1481 pasan, 4 fallos (arreglados) |

El binario **bajó** 777 KiB al meter el stack dentro: `.text` bajó 842 KiB
(optimización del build) y `.rodata` subió 102 KiB (el stack embebido).
