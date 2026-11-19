# Utilidades de operación

| Script | Para qué |
|---|---|
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
