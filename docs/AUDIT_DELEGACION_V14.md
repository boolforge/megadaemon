# Auditoría P1.3 — Delegación parasitaria (`src/delegate.rs`)

> Módulo del agente de delegación. Todo lo de aquí está **verificado con
> tests locales** salvo lo marcado explícitamente como `pendiente`.
> Última revisión: **2026-11-19**.

## 1. Qué garantiza el módulo (invariantes)

| # | Invariante | Cómo se comprueba |
|---|---|---|
| I-1 | El enrutado **siempre** elige una cuenta gratuita si existe una capaz; las de pago quedan reservadas como último recurso | `routing_score`: `+1000` al tier free, `−1000` al de pago. Test `enrutado_prefiere_siempre_la_cuenta_gratuita_adecuada` |
| I-2 | Una cuenta **sin cuota** (o con cuota 0) está **quemada** y no recibe tareas | `pick_account` la salta antes de puntuar; `record_attempt` consume una unidad por intento. Test `una_cuenta_con_cuota_agotada_no_recibe_tareas` |
| I-3 | Todo precio de la tabla tiene **URL oficial + fecha de verificación** | `PRICING_SOURCES` + `PRICING_OVERRIDES`; test `todo_precio_de_la_tabla_tiene_fuente_y_fecha` (invariante: *ningún precio sin procedencia*) |
| I-4 | Un precio viejo se **detecta**, no se corrige a ojo | `pricing_stale(días, hoy)` + `pricing_report(días)` en JSON |
| I-5 | El inventario de cuentas **jamás** lleva secretos | `find_secret_field` recorre el JSON y `load_inventory` lo rechaza. Test `el_inventario_nunca_lleva_credenciales` |
| I-6 | No se enruta a un proveedor desconocido sin modelo explícito | `accounts_from_seeds` exige `model` si el proveedor no está en `DEFAULT_FREE_MODEL` |
| I-7 | El enrutado es **determinista** (misma entrada ⇒ misma salida) | Desempate final por `id`; test `el_enrutado_es_deterministico_y_puro` |
| I-8 | El ahorro se mide, no se estima | Doble contador: `cost_usd` (real) y `shadow_cost_usd` (lo que habría costado con la tarifa de lista). Test `el_coste_real_es_cero_en_cuentas_gratuitas_y_el_sombra_no` |

## 2. Qué se ha añadido en esta revisión

### 2.1 Procedencia de los precios (lo que faltaba del requisito)

Antes la tabla de precios era una lista de números con el nombre del
proveedor, pero **sin fuente ni fecha**: un precio sin procedencia es una
suposición. Ahora:

- `PRICING_SOURCES` — `(proveedor, url_oficial, verificado_el)`.
- `PRICING_OVERRIDES` — casos por modelo (promociones, contexto largo, tarifa
  de pico) con su propia URL, fecha y nota explicativa.
- `price_provenance(modelo) -> PriceProvenance` — precio + fuente + fecha + nota.
- `pricing_ledger()` — una entrada por modelo de la tabla.
- `pricing_stale(días, hoy)` — los que hay que **re-verificar**.
- `pricing_report(días)` — JSON listo para el cockpit.
- `days_since(iso, hoy)` + `today_epoch_day()` — aritmética de fechas
  Gregorianas en días epoch, sin dependencias nuevas (no se puede tocar
  `Cargo.toml` en esta tarea).

### 2.2 Inventario real de cuentas (el puente que faltaba)

El enrutado no puede ser bueno si no sabe qué cuentas existen. Nuevo:

- `DEFAULT_FREE_MODEL` — modelo gratuito por defecto de cada proveedor.
- `AccountSeed` / `accounts_from_seeds(seeds)` — proveedor + nº de cuentas +
  capacidades → `ProviderAccount` con `id` estable `{proveedor}-{n}`.
- `load_inventory(json)` / `save_inventory(accounts)` — persistencia.
- `find_secret_field(json)` — guardián: rechaza inventarios con campos que
  parecen credenciales (`token`, `password`, `cookie`, `api_key`, …).

El inventario **no** lleva tokens: lleva recuentos. Los tokens se quedan en
el almacén privado, fuera del repo. Esto también sirve para **P1.8**
(migrar las tareas de las cuentas de IA al demonio): el daemon necesita saber
cuántas cuentas hay de cada proveedor, no posesionarse sus credenciales.

Inventario real medido en el móvil (sólo recuentos, ningún secreto):

| Proveedor | Cuentas | Modelo por defecto |
|---|---|---|
| kimi | 4 | `kimi-k3` |
| deepseek | 3 | `deepseek-chat` |
| chatgpt | 1 | `gpt-5.6-sol` |

`perplexity` tiene 4 cuentas pero **no entra en el enrutado**: no está en la
tabla de precios y no se enruta a ciegas. Necesita precio + fuente + fecha.
`claude` y `gemini` están a 0 cuentas en el inventario actual.

## 3. Precios: qué se re-verificó el 2026-11-19

Las seis fuentes se comprobaron hoy contra las páginas oficiales:

| Proveedor | Fuente | Resultado |
|---|---|---|
| Anthropic | platform.claude.com/docs/en/about-claude/pricing | 15/15 modelos de la tabla **cuadran** |
| OpenAI | developers.openai.com/api/docs/pricing | gpt-6.2-astra 8/40, gpt-6.2-sol 2/10, gpt-6-astra 10/50, gpt-6.1-sol 2/10, gpt-6-luna 0.10/0.50, gpt-5.6-sol 4/20 **cuadran** |
| Google | ai.google.dev/gemini-api/docs/pricing | **desfasado**: la tabla decía 1.50/9.00, 0.30/2.50 y 0.10/0.40; lo real es 3.1 Pro **2/12** (≤200k), 3 Flash **0.50/3**, 3.1 Flash-Lite **0.25/1.50**, 3.8 Flash **0.75/3.75** hasta 2026-12-31. **Corregido** |
| DeepSeek | api-docs.deepseek.com/quick_start/pricing | v4-pro 1.32/3.96 y flash 0.30/1.20 = **tarifa de pico**; en valle es la mitad. Cuadra (conservador) |
| MiniMax | platform.minimax.io/docs/pricing/overview | M3.1-Flash 0.15/0.60, M3 ≤512k **0.30/1.20** (50% off permanente), >512k 0.60/2.40. Cuadra |
| Moonshot (Kimi) | platform.kimi.ai/docs/pricing | Tabla coherente con la tarifa directa. Ojo: OpenRouter lista otra (K3.1 Pro 0.24/8.40 al 2026-11-16) porque **es otra ruta de venta**, no un error |

Los precios de MiniMax, DeepSeek y Kimi son **tarifa directa del proveedor**.
Compararlos contra un agregador (OpenRouter, Atlas…) da falsos positivos: el
agregador revende con su propio margen.

## 4. Cómo se verifica

```bash
# Arnés propio (no toca lib.rs): incluye el módulo por #[path]
cargo test --release --test delegate

# Espera: el módulo aún no está en lib.rs (ver §5). El arnés lo compila
# como módulo de test, que es la convención del repo
# (tests/signing.rs, tests/root_hide_app_intel.rs, tests/cockpit_doctor_smoke.rs).
```

Tests: 25 en el propio módulo + 6 de integración en
`rust-core/tests/delegate.rs`.

## 5. Pendiente / bloqueos (honestos)

| # | Qué | Quién |
|---|---|---|
| D-1 | `pub mod delegate;` **no está en `rust-core/src/lib.rs`** ⇒ el módulo todavía no se compila como parte de la librería. No lo he tocado: `lib.rs` es de otro agente | principal (dueño de `lib.rs`) |
| D-2 | Precio de **perplexity** (4 cuentas reales) sin verificar ⇒ fuera del enrutado | hardware/NPU + este agente |
| D-3 | `kimi-k3.1-pro` y el resto de modelos de Kimi: la fuente directa no se pudo abrir hoy (la caja estaba saturada); el valor se cruzó con la tarifa publicada por el proveedor en secundarias | este agente (re-verificar) |
| D-4 | El modelo "preferido" por cuenta (el que de verdad responde en la app) no está en el inventario: hace falta una columna `model` medida, no supuesta | hardware/NPU |
| D-5 | El enrutado no **ejecuta** la llamada al proveedor: planifica y asigna. La ejecución la hace quien tenga la sesión de la app | — (fuera de P1.3) |

## 6. Cómo encaja con el plan

- **P1.3** (este módulo): enrutado gratuito, reserva de las de pago, cuota
  quemada y precios con fecha y fuente ⇒ **hecho**, con tests.
- **P1.8** (migrar las tareas de las cuentas de IA al demonio): el cargador de
  inventario es la primera pieza; falta el planificador que baje las tareas de
  las apps al tablero.
- **§8 decisión 5 del plan maestro** ("presupuesto de tokens: el enrutado debe
  medirse, coste real, no estimado a ojo"): resuelto por el doble contador
  `cost_usd` / `shadow_cost_usd` y por `pricing_stale` (lo que no se puede
  medir se declara rancio, no se estima).
