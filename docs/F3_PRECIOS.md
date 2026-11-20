# F3 — Tabla de precios de la flota (con fuente y fecha)

> Referencia: [`MASTER_PLAN_V16.md`](MASTER_PLAN_V16.md) (fase **F3**).
> No es un plan: es el **enumerado de modelos con su `source`** que el
> `PriceBook` consume (`rust-core/src/price_table.rs`).

**Regla**: no se inventa una cifra. Cada precio viene de la página oficial
del proveedor, con la URL y la **fecha de captura** (`2026-11-20`). Lo que no
se puede citar **no entra** ⇒ el `PriceBook` lo marca *desconocido* y el
router lo penaliza (`NO VERIFICADO`), nunca `0`.

**Unidades**: los proveedores publican **USD por 1M de tokens**; el `PriceBook`
trabaja **por 1K** ⇒ conversión ÷1000 (`price_table.rs` lo hace y hay un test
que lo fija: confundir 1M con 1K = 1000× de más en el coste).

## Con precio citable

| Proveedor | Modelo | USD/1M in | USD/1M out | Fuente | Matiz |
|---|---|---|---|---|---|
| OpenAI | `gpt-6.2-astra` | 8.00 | 40.00 | [platform.openai.com/docs/pricing](https://platform.openai.com/docs/pricing) | |
| OpenAI | `gpt-6.2-sol` | 2.00 | 10.00 | idem | |
| OpenAI | `gpt-6-astra` | 10.00 | 50.00 | idem | |
| OpenAI | `gpt-6.1-sol` | 2.00 | 10.00 | idem | |
| OpenAI | `gpt-5.6-sol` | 4.00 | 20.00 | idem | precio estándar |
| OpenAI | `gpt-5.6-terra` | 2.00 | 12.00 | idem | |
| OpenAI | `gpt-5.6-luna` | 0.20 | 1.20 | idem | |
| OpenAI | `gpt-5.5` | 12.50 | 75.00 | idem | prompt < 272k |
| OpenAI | `gpt-5.4` | 5.00 | 30.00 | idem | prompt < 272k |
| OpenAI | `gpt-5.4-mini` | 0.75 | 4.50 | idem | |
| OpenAI | `gpt-5.4-nano` | 0.20 | 1.25 | idem | |
| OpenAI | `gpt-5.3` / `gpt-5.2` | 1.75 | 14.00 | idem | |
| OpenAI | `gpt-5.1` / `gpt-5` | 1.25 | 10.00 | idem | |
| OpenAI | `gpt-5-mini` | 0.25 | 2.00 | idem | |
| OpenAI | `gpt-5-nano` | 0.05 | 0.40 | idem | |
| OpenAI | `gpt-4.1` | 2.00 | 8.00 | idem | |
| OpenAI | `gpt-4.1-mini` | 0.40 | 1.60 | idem | |
| OpenAI | `gpt-4.1-nano` | 0.05 | 0.20 | idem | |
| OpenAI | `gpt-4o` | 2.50 | 10.00 | idem | |
| OpenAI | `gpt-4o-mini` | 0.15 | 0.60 | idem | |
| OpenAI | `o3` | 2.00 | 8.00 | idem | |
| OpenAI | `o3-mini` / `o4-mini` | 1.10 | 4.40 | idem | |
| Anthropic | `claude-sonnet-5-5` | 2.00 | 10.00 | [docs.anthropic.com/…/pricing](https://docs.anthropic.com/en/docs/about-claude/pricing) | |
| Anthropic | `claude-opus-5-5` | 4.00 | 20.00 | idem | |
| Anthropic | `claude-haiku-5-5` | 0.10 | 0.50 | idem | prompt ≤ 100k (≥100k: 0.50/2.50) |
| Anthropic | `claude-sonnet-5` | 2.00 | 10.00 | idem | |
| Anthropic | `claude-opus-5` / `claude-opus-4-8` | 5.00 | 25.00 | idem | |
| Anthropic | `claude-haiku-4-5` | 1.00 | 5.00 | idem | |
| Google | `gemini-3.8-flash` | 1.50 | 7.50 | [aistudio.google.com/docs/pricing](https://aistudio.google.com/docs/pricing) | precio estándar; **introductorio 0.75/3.75 hasta 2026-12-31** |
| MiniMax | `minimax-m3.1-flash` | 0.15 | 0.60 | [platform.minimax.io/docs/pricing/overview](https://platform.minimax.io/docs/pricing/overview) | tarifa estándar |
| MiniMax | `minimax-m3` | 0.30 | 1.20 | idem | prompt ≤ 512k; descuento permanente 50% (>512k: 0.60/2.40) |
| DeepSeek | `deepseek-flash` | 0.15 | 0.60 | [api-docs.deepseek.com/…/pricing](https://api-docs.deepseek.com/quick_start/pricing) | cache miss, **franja valle** (pico ×2) |
| DeepSeek | `deepseek-v4-pro` | 0.66 | 1.98 | idem | cache miss, franja valle (pico ×2) |

**Fecha de captura: 2026-11-20.** Los precios cambian: hay que re-capturar y
re-commitear la tabla (con la nueva fecha) o el coste medido se queda viejo.

## Sin precio citable (⇒ `NO VERIFICADO`, el router los penaliza)

| Modelo | Motivo |
|---|---|
| `kimi-k3`, `kimi-k3.1-pro` | el proveedor publica en **CNY** y no hay fuente de tipo de cambio citable |
| `mistral-small-latest`, `mistral-free` | sin página de precios oficial consultada en esta captura |
| `grok` | sin página de precios oficial consultada en esta captura |
| `gpt-oss-120b` | pesos abiertos: se sirve en local, no hay precio por API |
| `claude-code` | alias de cliente, no es un modelo facturable |

## Cómo se usa

```rust
use megadaemon_core::price_table::fleet_price_book;
let precios = fleet_price_book();      // ya con source + fecha en cada entrada
let coste = precios.cost_of("gpt-6.2-sol", tokens_in, tokens_out);
// coste.usd      -> Some(..) si hay fuente
// coste.source   -> "https://... (capturado 2026-11-20)"
// coste.note     -> "NO VERIFICADO: ..." si no la hay
```

Para añadir un precio: una fila en `FLEET_PRICES` (`price_table.rs`) **con su
URL y su fecha** ⇒ y el test `el_libro_cubre_los_modelos_de_la_flota_con_precio`
avisa si un modelo de la flota se queda sin cotizar.
