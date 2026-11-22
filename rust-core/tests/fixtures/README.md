# Banco de respuestas CAPTURADAS de proveedores (v16)

Cuerpos de respuesta **congelados** de proveedores de LLM para poder
ejercitar el parseo, la cascada y el fallback **sin red** y **sin secretos**.

## Procedencia: TODOS son `SINTETICO`

En esta sesión **no se capturó ninguna traza HTTP cruda** de un proveedor
real (el daemon no guarda el cuerpo crudo de las respuestas).  Por tanto
todos los ficheros están marcados `SINTETICO`: la **forma** es la que
documenta cada proveedor, pero el cuerpo se compuso a mano.

La única medición real del 2026-11-22 (móvil del usuario) fue la **salida
del daemon**, no el cuerpo HTTP del proveedor:

```
megadaemon-core chat minimax-m3.1-flash "Responde exactamente: MINIMAX_OK"
=> {"reply":"MINIMAX_OK","vendor":"MiniMax","latency_ms":1232.0,"fallback_used":false}
```

Eso es lo que dice la nota del fixture `minimax-m3.1-flash__ok.json`.

## Credenciales

**Ninguna.**  Las claves que aparecen en los cuerpos de error son
FICTICIAS (`sk-EXAMPLE-NO-ES-UNA-KEY-0000`) y están ahí para comprobar que
la redacción (`providers::redact_secret`) las tapa antes de que el error
llegue al cliente.

## Índice

| fichero | status | caso raro que cubre |
|---|---|---|
| `minimax-m3.1-flash__ok.json` | 200 | respuesta normal (`content` string + `usage`) |
| `minimax-m3.1-flash__reasoning_field.json` | 200 | razonamiento en `reasoning_content` |
| `minimax-m3.1-flash__reasoning_think.json` | 200 | razonamiento `<think>…</think>` dentro del texto |
| `minimax-m3.1-flash__http_401.json` | 401 | el cuerpo de error **ECOA la api key** |
| `minimax-m3.1-flash__http_402.json` | 402 | sin saldo (cuota) |
| `minimax-m3.1-flash__http_404.json` | 404 | modelo inexistente (no reintentar) |
| `groq-free__http_429.json` | 429 | `resets at` + `retry after` (pausa de cuota) |
| `google-ai-studio__http_401.json` | 401 | envoltorio de error de Google |
| `anthropic__http_500.json` | 500 | reintentable (backoff) |
| `openrouter-free__tool_calls.json` | 200 | `tool_calls` nativas (`arguments` como texto) |
| `openrouter-free__tool_calls_bad_arguments.json` | 200 | `arguments` no JSON (sin panic) |
| `openrouter-free__content_array.json` | 200 | `message.content` como array |
| `ovhcloud-free__empty_choices.json` | 200 | `choices` vacío |
| `pollinations-free__html_error.json` | 502 | cuerpo HTML (no JSON) |
| `pollinations-free__text_plain.json` | 200 | texto plano |
| `pollinations-free__text_plain_utf8.json` | 200 | UTF-8 multibyte (recorte sin panic) |
| `zai-glm-free__truncated.json` | 200 | respuesta truncada (JSON inválido) |
| `openai-compatible__legacy_text.json` | 200 | forma legacy `choices[0].text` |
| `openai-compatible__error_in_200.json` | 200 | 2xx con error dentro |
| `local-seere__offline.json` | 200 | forma propia del SEERE local (no OpenAI) |

## Formato

```json
{ "_meta": { "id", "provider", "status", "origen", "nota" }, "body": { ... } }
```

o, si el cuerpo del proveedor **no** es JSON:

```json
{ "_meta": { ... }, "raw": "<html>…</html>" }
```

El índice que usan los tests está en `fixtures/mod.rs` (`FIXTURES`).
