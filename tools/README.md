# Utilidades de operación

| Script | Para qué |
|---|---|
| `md-recordings.sh` | Índice unificado de grabaciones + backup de seguridad con MANIFEST (usa el binario Rust `md-recordings`). `--self-test` hace una prueba determinista local. |
| `megadaemon_local_doctor.sh` | Diagnóstico de sólo lectura del MegaDaemon **local**: DNS, relay 80/443, canónico, móvil en la LAN y la key del móvil (sólo sha16). Dice el REMEDIO exacto de cada fallo. |
| `mdmcp.py` | Cliente MCP de **megadaemon** (JSON-RPC 2.0 sobre HTTP). Lista herramientas, invoca herramientas y hace búsquedas de conversaciones. |
| `upload_bin.py` | Sube un binario al dispositivo por trozos vía MCP y **verifica el SHA-256** en destino. |

## Uso

```bash
export MEGADAEMON_API_KEY="$(cat ~/.md_key)"
export MEGADAEMON_TOKEN="$(cat ~/.md_token)"

python3 tools/mdmcp.py tools
python3 tools/mdmcp.py call system_overview '{}'
python3 tools/mdmcp.py call sync_conversations '{"include_cookies": true}'
python3 tools/mdmcp.py call search_conversations '{"query": "carteles anuncios", "limit": 5}'
python3 tools/mdmcp.py call conversation_stats '{}'

python3 tools/upload_bin.py rust-core/target/aarch64-linux-android/release/megadaemon-core \
    /data/local/tmp/megadaemon/megadaemon-core.new
```

Las claves se leen del propio dispositivo:
`/data/local/tmp/megadaemon/state/megadaemon_api.key` y `.../cockpit_token`.

## Grabaciones (índice unificado y backup)

```bash
tools/md-recordings.sh --help
tools/md-recordings.sh scan   --root /sdcard/MegaDaemon/recordings --app /sdcard/WhatsApp/Media
tools/md-recordings.sh backup --root /sdcard/MegaDaemon/recordings --dest ~/backups/recordings
tools/md-recordings.sh verify --dest ~/backups/recordings
tools/md-recordings.sh --self-test
```

No graba nada: sólo indexa, unifica y respalda lo que ya existe. El MANIFEST
(NDJSON + `manifest.json`) queda en el destino y el backup es incremental e
idempotente. Detalle: `docs/GRABACIONES_INDICE_Y_BACKUP.md`.

## "No me funciona" (el MegaDaemon local)

```bash
tools/megadaemon_local_doctor.sh
tools/megadaemon_local_doctor.sh --json
```

Sólo lee: comprueba DNS, el relay en 80/443, el canónico `/channel.json`, el
móvil en la LAN y si el móvil encuentra su **propia** key (un 403 no es key
caducada: es que el daemon no encuentra el fichero). Imprime el comando
exacto que arregla cada fallo. No imprime secretos (de la key, sólo el sha16).
