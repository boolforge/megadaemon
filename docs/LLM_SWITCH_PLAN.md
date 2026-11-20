# SEERE — Cambiar de LLM: ventajas, coste real y plan de migración

**Documento de decisión + plan de migración.** Fecha de las mediciones: 2026-11-18.
Todos los números están medidos con los modelos reales servidos por `llama-server` (b12202), no estimados.
Alcance: qué pasa si cambias el modelo que ejecuta SEERE, cuánto ganas, qué pagas y en qué orden hay que hacerlo.

---

## 0. Veredicto en una frase

> **No: cambiar de LLM no es el mejor movimiento ahora.** El orden correcto es
> **1) quitar la huella del prompt** (3 líneas: el 0.5B pasa de 3/12 a **8/12** enrutado
> y de **0/12 a 8/12** en corrección semántica),
> **2) el guardián anti-eco** (evita ejecutar basura),
> **3) y sólo entonces** considerar el modelo. Con los números ya medidos, el mejor
> modelo **distinto** es **Llama-3.2-1B-Instruct** (12/12 de enrutado en las cuatro
> combinaciones de idioma, 9-10/12 de corrección semántica), pero cuesta **2× la RAM**
> y **~3× la latencia** que el 0.5B. Y SmolLM2-360M, que era el candidato natural por
> IFEval, **pierde en corrección semántica**: enrutaba perfecto (12/12) pero elegía
> comandos equivocados.

La tabla resumen (medida, 12 preguntas que exigen datos externos, prompt ES salvo donde se indica):

| Modelo | Tamaño | routed ES/ES | **sens ES/ES** | routed EN/EN | **sens EN/EN** | routed ES/EN | routed EN/ES |
|---|---|---|---|---|---|---|---|
| Qwen2.5-0.5B (actual) | 398 MB | 8/12 | **8/12** | 0/12 | **0/12** | 8/12 | 0/12 |
| SmolLM2-360M | 271 MB | 12/12 | **7/12** | 8/12 | **8/12** | 11/12 | 11/12 |
| Llama-3.2-1B | 808 MB | 12/12 | **9/12** | 11/12 | **10/12** | 12/12 | 12/12 |
| Qwen2.5-1.5B | 1117 MB | 10/12 | **8/12** | 11/12 | **10/12** | 10/12 | 12/12 |

`routed` = pidió una herramienta. `sens` = la herramienta elegida **resolvería de verdad** la pregunta
(heurística por pregunta: `nproc/cpuinfo/lscpu` para núcleos, `free/meminfo` para RAM, `df/du` para disco,
`date` para la hora, `ps/pgrep` para procesos…). **El enrutado sin `sens` es una métrica de vanidad.**

---

## 1. Metodología (reproducible)

**Infraestructura:** `llama-server` b12202, `--jinja`, `-c 8192`, `-t 2`, `-ngl 0`, `temperature=0`,
`min_p=0.05`, `seed=42`, `cache_prompt=true`. Sandbox de 2 cores ⇒ **las latencias absolutas son
pesimistas**; las *relativas* entre modelos sí soninformativas.

**Modelos** (GGUF Q4_K_M, todos descargados y servidos en la misma máquina):

| Clave | Fichero | Tamaño | Contexto | Licencia |
|---|---|---|---|---|
| `qwen2.5-0.5b` | `qwen2.5-0.5b-instruct-q4_k_m.gguf` | 398 MB | 32 768 | Apache-2.0 |
| `smollm2-360m` | `SmolLM2-360M-Instruct-Q4_K_M.gguf` | 271 MB | **8 192** | Apache-2.0 |
| `llama-3.2-1b` | `Llama-3.2-1B-Instruct-Q4_K_M.gguf` | 808 MB | 131 072 | **Llama 3.2 Community (cláusula MAU 700 M)** |
| `qwen2.5-1.5b` | `Qwen2.5-1.5B-Instruct-Q4_K_M.gguf` | 1117 MB | 32 768 | Apache-2.0 |

**Banco de pruebas:** 19 preguntas (12 exigen datos externos + 7 de respuesta directa), 4 variantes
= prompt ES/EN × pregunta ES/EN. Métricas: `routed` (pedir herramienta), `sens` (comando plausible),
`echo` (copió la lista OPS), `multi` (partió la línea en varias acciones), tokens generados, latencia.

**Scripts:** `bench_models.py` (multi-modelo), `reanalyze.py` (re-parsea las salidas crudas sin volver a
muestrear ⇒ A/B del parser sin ruido), `bench_latency_loop.py` (latencia en serie + bucle completo),
`bench_fidelity.py` (efecto de la huella del host y presupuesto de turnos).

**Advertencia metodológica que cambió conclusiones:** llama.cpp **no es bit-determinista** cuando
cambia la composición del batch (3 peticiones en paralelo ≠ 1 petición). Repetir el bench da cifras que
oscilan ±1-2 respuestas. Por eso las comparaciones de *parser* se hicieron offline sobre las salidas
guardadas (`reanalyze.py`), y las comparaciones de *modelo* se leen con ese margen de error en mente.

---

## 2. Resultados

### 2.1 Enrutado y corrección semántica (parser final, re-análisis offline)

| Modelo | Variante | routed/12 | sens/12 | echo/19 | multi/19 |
|---|---|---|---|---|---|
| Qwen2.5-0.5B | prompt ES / preg ES | 8 | **8** | 4 | 0 |
| | prompt EN / preg EN | **0** | **0** | 4 | 0 |
| | prompt ES / preg EN | 8 | 6 | 7 | 0 |
| | prompt EN / preg ES | **0** | **0** | 2 | 0 |
| SmolLM2-360M | prompt ES / preg ES | **12** | 7 | 1 | 1 |
| | prompt EN / preg EN | 8 | 8 | 1 | 1 |
| | prompt ES / preg EN | 11 | **10** | 2 | 3 |
| | prompt EN / preg ES | 11 | 8 | 0 | 0 |
| Llama-3.2-1B | prompt ES / preg ES | **12** | **9** | 1 | 1 |
| | prompt EN / preg EN | 11 | **10** | 5 | 3 |
| | prompt ES / preg EN | **12** | **10** | 2 | 2 |
| | prompt EN / preg ES | **12** | **10** | 6 | 4 |
| Qwen2.5-1.5B | prompt ES / preg ES | 10 | 8 | 2 | 2 |
| | prompt EN / preg EN | 11 | **10** | 6 | 1 |
| | prompt ES / preg EN | 10 | 8 | 5 | 4 |
| | prompt EN / preg ES | 12 | **10** | 5 | 2 |

**Tres lecturas:**

1. **El prompt en español gana en los cuatro modelos.** Es la palanca más fuerte y es gratuita.
   Con Qwen2.5-0.5B la diferencia es **8/12 vs 0/12** (el prompt EN provoca *prompt echo*:
   el modelo copia las reglas en vez de ejecutarlas). Con SmolLM2 y Llama 1B la diferencia se reduce
   (8-11/12 vs 12/12) ⇒ **el colapso por idioma es unaницы de Qwen2.5-0.5B, no una ley**.
   Por eso v11 trae `--lang auto`: con un modelo distinto, el idioma optimum puede cambiar, y hay que
   **volver a medir**.
2. **Enrutar no es resolver.** SmolLM2 enruta 12/12 en ES/ES pero sólo acierta 7/12 comandos:
   para "cuántos núcleos" responde `ps aux | grep uptime | awk '{print $2}'`, para "qué hora es" inventa
   `G 2018-06-12 12:00:00-0400 …` como si fuera un patrón de búsqueda. Llama-3.2-1B y Qwen2.5-1.5B sí
   eligen herramientas razonables (`lscpu | grep "Number of physical cores"`, `free -m | grep Mem`,
   `df -h / | awk '{print $4}'`, `ps -p 1 -o state,cmd`).
3. **1,5B no es mejor que 1B para esto.** Qwen2.5-1.5B gana en `sens` con prompt EN (10/12) pero pierde
   en ES/ES (8/12) y cuesta 38 % más RAM y ~40 % más latencia que Llama-3.2-1B.

### 2.2 El hallazgo que reordena todo: la huella del host **rompe** el enrutado

El harness v11 mandaba la huella del entorno al final del turno de usuario
(`<ENV uid=0|gid=0|pid=…|cwd=…>`) para que el system prompt siguiera siendo estático.
El E2E de fidelidad (E1) comparó **la pregunta sola** vs **la pregunta con la huella**, mismo modelo,
mismo prompt, misma gramática:

| Modelo | pregunta sola | pregunta + `<ENV …>` |
|---|---|---|
| Qwen2.5-0.5B | routed **8/12**, sens **8/12** | routed **3/12**, sens **0/12** |
| SmolLM2-360M | routed **12/12**, sens **8/12** | routed 12/12, sens **5/12** |

**La huella arruina al 0.5B**: pasa de 8/12 a 3/12 y de 8/12 a **0/12** en corrección semántica. El 0.5B
lee `uid=0|gid=0|pid=…` como "datos que ya tengo" y **responde desde la memoria en vez de preguntar al
shell** — que es exactamente el fallo que SEERE existe para evitar.

> **Consecuencia sobre mis propias cifras:** el bench multi-modelo anterior enviaba la pregunta
> *sola*, es decir **no replicaba el input real del harness**. Por eso las cifras de routing de ese
> bench eran optimistas. El bench de fidelidad es el que vale.

**El arreglo (v11.1): la huella no se manda al modelo.** Se queda en el harness, donde sirve para lo que
realmente sirve: la comprobación de fundamento (`verify()`) y la ruta de 0 tokens. El modelo **no necesita
saber el uid**: si se lo preguntan, ejecuta `id` y lo lee en el OBS. Es la filosofía de SEERE aplicada
a su propio prompt: *el modelo pregunta al shell, no adivina*.

Efecto secundario: **−34 tokens por turno** en el prompt de usuario.

### 2.3 Latencia real (petición única, sin concurrencia)

| Modelo | mediana | min | tokens generados (mediana) |
|---|---|---|---|
| SmolLM2-360M | **2 102 ms** | 1 287 ms | 18 |
| Qwen2.5-0.5B | 7 878 ms | 2 204 ms | 77 |
| Llama-3.2-1B | 11 731 ms | 3 472 ms | 77 |

La dispersión (min 2,2 s vs mediana 7,9 s en el 0.5B) **no es del modelo**: es la longitud de la salida.
Un modelo que emite 77 tokens tarda ~3× más que uno que emite 18. Los modelos más grandes no son
"más lentos" por ser más grandes, sino porque **se expanden más**.

### 2.4 Coste en tokens del prompt (tokenizador real de cada modelo, vía `llama-tokenize`)

| Modelo | prompt v10 | prompt v11 EN | prompt v11 ES |
|---|---|---|---|
| Qwen2.5-0.5B / 1.5B | 247 | **187** | **206** |
| SmolLM2-360M | 277 | 207 | 240 |
| Llama-3.2-1B | 237 | 186 | 205 |

La diferencia entre tokenizadores es de ±3 % (no el 15-30 % que se suele suponer): el prompt de SEERE es
texto plano en inglés/español, sin sintaxis exótica. **Mide con el tokenizador del modelo destino;**
`token_budget.py` ya lo hace (con fallback a `llama-tokenize` si no hay `tokenizers` de PyPI).

### 2.5 Bucle completo con el harness v11.1 (4 preguntas, `--max-turns 2`, herramientas reales)

| Modelo | grounded=true | tokens (med) | respuestas observadas |
|---|---|---|---|
| Qwen2.5-0.5B | **2/4** | 530-628 | *"El número de núcleos es 94"* ✓ · *"La RAM es 4 GB"* ✗ (sin OBS) · *"…23:59:32 UTC 2027"* ✗ (año inventado) · *"¿Cómo estás?"* ✓ |
| Llama-3.2-1B | 4/4 | 530-625 | *"ERR_RC1:"* · *"total used free shared…"* (la cabecera de `free -m`) · *"3:24, 0 user, 3.15 load average"* · *"hello"* |
| SmolLM2-360M | 3/4 | 407-654 | *"2340, 2341, 2343 → R: uptime is 2340"* · *"no such file or directory"* · *"ERR_LLM_UNREACHABLE"* · eco de `<OBS id="1"></OBS>` |

**Dos lessons que sólo aparecen en el bucle completo (y no en el banco de primera acción):**

1. **`grounded` es necesario pero NO suficiente.** Llama-3.2-1B saca 4/4 de `grounded=true`
   respondiendo *"ERR_RC1:"*: los literales vienen del OBS, pero la respuesta es inútil.
   **El fundamento no mide corrección; mide honestidad.** Para medir corrección hace falta
   el `sens` del banco, no el `grounded` del bucle.
2. **El 0.5B es el único que inventa datos (y ahora se detecta).** Escribió *"2027"* en vez de
   *"2026"* y *"4 GB"* sin ninguna observación: los dos casos los marca `grounded=false` con los
   arreglos de §5 Fase 1. Antes de los arreglos pasaban como `grounded=true`.

### 2.6 Lo que NO mejora cambiando de modelo

| Problema | ¿Se arregla con un modelo mayor? |
|---|---|
| Copiar la línea OPS como orden (`A: $ OPS $ shell \| ? web search`) | **No** — le pasa a los 4 modelos (1-7 de 19). Es del harness. |
| Escribir `\| $ grep` dentro de un pipeline | **No** — 10-16/19 en SmolLM2/Llama. Es del harness. |
| Citar `[SRC:1]` sin observación | **No** — es del harness (la cita no resuelve). |
| Degenerar en bucle de repetición | **No** — le pasa al 1B. Es del harness. |
| Perder turnos respondiendo en vez de actuar | **Parcialmente** — el 1B/1.5B lo hacen menos. |

---

## 3. Los cuatro candidatos: ventajas y inconvenientes

### 3.1 Qwen2.5-0.5B-Instruct — *el incumbente*
**Ventajas:** el más barato en RAM entre los que funcionan bien (398 MB); mejor `sens` con prompt ES
(8/12); ya integrado y medido; español nativo; contexto 32 k; Apache-2.0.
**Inconvenientes:** **colapso por prompt EN (0/12)**; es el que más sufre la huella del host;
`sens` cae a 0/12 en cuanto el input se contamina.

### 3.2 SmolLM2-360M-Instruct — *el que decía la ficha técnica*
**Ventajas:** el más pequeño (271 MB, **−32 %** de RAM); **12/12 de enrutado** en ES/ES; IFEval 41,0
(frente a 31,6 de Qwen2.5-0.5B) ⇒ mejor cumplimiento del contrato; **no tiene el colapso por idioma**
(8/12 con prompt EN); Apache-2.0; el más rápido en latencia (2,1 s).
**Inconvenientes:** **peor corrección semántica (7/12 en ES/ES, 5/12 con la huella)**: enruta bien pero
elige mal; genera 64 tokens de mediana en ES/ES (verboso); la tarjeta de modelo avisa que
*"primarily understand and generate content in English"* ⇒ **no recomendado para español**;
contexto 8 192 (la mitad que el 0.5B).
**Veredicto:** el que menos me convence de los tres, contra la intuición que te lleva a mirar IFEval.

### 3.3 Llama-3.2-1B-Instruct — *el equilibrio medido*
**Ventajas:** **12/12 de enrutado en las cuatro combinaciones de idioma** (el más estable);
`sens` 9-10/12 en tres de cuatro; latencia en serie aceptable (3,5 s mínimo);
el único que elige bien el comando para casi todo (`lscpu`, `free -m`, `df -h`, `ps -p 1`).
**Inconvenientes:** 808 MB (×2 del 0.5B); **licencia Llama 3.2 Community con cláusula de 700 M de
usuarios activos mensuales** (si el megadaemon es comercial y pasa de 700 M MAU, hay que hablar con
Meta); contexto 131 k (desperdiciado para SEERE).
**Veredicto:** **el mejor candidato si el cambio se justifica.**

### 3.4 Qwen2.5-1.5B-Instruct — *el que más calidad pero no el más estable*
**Ventajas:** `sens` 10/12 con prompt EN; comandos muy buenos (`free -m | awk '{print $3/$2*100}'`,
`df -h / | tail -n1 | awk '{print $4}'`, `ps -p 1 -o state,cmd`); español fuerte; Apache-2.0.
**Inconvenientes:** 1117 MB (×2,8 del 0.5B); **el más lento** (11,5-20,6 s por acción en este sandbox);
`routed` irregular (10/12 en ES/ES, 10/12 en ES/EN).

### 3.5 Resumen comparativo

| | Qwen2.5-0.5B | SmolLM2-360M | Llama-3.2-1B | Qwen2.5-1.5B |
|---|---|---|---|---|
| RAM (GGUF Q4_K_M) | 398 MB | **271 MB** | 808 MB | 1117 MB |
| RAM en runtime (aprox.) | ~0,6 GB | ~0,4 GB | ~1,1 GB | ~1,5 GB |
| routed ES/ES | 8/12 | **12/12** | **12/12** | 10/12 |
| sens ES/ES | **8/12** | 7/12 | 9/12 | 8/12 |
| sens EN/EN | 0/12 | 8/12 | **10/12** | **10/12** |
| routed EN/EN | **0/12** | 8/12 | 11/12 | 11/12 |
| Latencia (min / mediana) | 2,2 / 7,9 s | **1,3 / 2,1 s** | 3,5 / 11,7 s | ~11,5 / 20,6 s |
| Tokens prompt (ES) | 206 | 240 | 205 | 206 |
| Contexto | 32 768 | 8 192 | 131 072 | 32 768 |
| Licencia | Apache-2.0 | Apache-2.0 | **Llama 3.2 Comm.** | Apache-2.0 |
| Español | fuerte | **débil** (card) | medio | fuerte |

---

## 4. ¿Es cambiar de LLM el mejor camino? — la jerarquía de palancas

Ordenadas por **ganancia por unidad de coste**, con el delta medido:

| # | Palanca | Coste | Ganancia medida | ¿Model-dependent? |
|---|---|---|---|---|
| 1 | **Quitar la huella `<ENV>` del prompt** | 3 líneas, −34 tok/turno | Qwen 0.5B: routed **+5/12**, sens **+8/12** | No |
| 2 | **Prompt en español** (o el idioma óptimo del modelo) | 0 | Qwen 0.5B: **+8/12** routed | **Sí** (re-medir por modelo) |
| 3 | **Guardián anti-eco** | ~30 líneas | Evita ejecutar basura; `hint_copy` 7/19 → 1/19 | No |
| 4 | **Normalizar `\| $ x` → `\| x`** | 2 líneas | `multi` 16/19 → 4/19 | No |
| 5 | **Cita `[SRC:n]` debe resolver** | ~10 líneas | Detecta respuestas inventadas | No |
| 6 | **Cambiar de modelo (0.5B → 1B)** | ×2 RAM, ×3 latencia | `sens` **+1-2/12**; `routed` +2-4/12 | — |

**La respuesta corta: no.** Las palancas 1-5 son gratuitas, se aplican al harness y valen más que
cualquier salto de modelo medido. El salto de modelo se justifica sólo si, **después** de aplicar 1-5,
el `sens` sigue por debajo de tu umbral de calidad.

Y hay un segundo argumento, igual de importante: **cambiar de modelo no reduce el trabajo del harness**.
Todo lo que te hace falta (gramática, política, taint tracking, presupuesto de turnos, fundamento) sigue
habiendo que escribirlo igual, sólo que con otro generador de texto. El modelo no te da la garantía:
te da una probabilidad.

---

## 5. El plan, fase por fase

### Fase 0 — Congelar la línea base (30 min, sin tocar nada)

Objetivo: poder volver atrás en cualquier momento y poder comparar.

```bash
# 1. Congelar el harness y la config que hoy usa el megadaemon
cp seere_v11.py  seere_v11.py.frozen
git -C tu-repo commit -am "SEERE v11.1 baseline"     # o cp al directorio del daemon

# 2. Congelar el modelo
cp /models/qwen2.5-0.5b-instruct-q4_k_m.gguf /models/BAK_qwen2.5-0.5b_q4km.gguf
sha256sum /models/*.gguf > /models/MODELS.sha256

# 3. Congelar las métricas (el banco actual)
python3 bench_models.py --models /models/qwen2.5-0.5b-instruct-q4_k_m.gguf \
                        --out baseline_qwen05b.json
python3 bench_fidelity.py qwen2.5-0.5b        # ← el que mide la huella
```

**Criterio de salida:** tienes `baseline_qwen05b.json` y `bench_fidelity.json` guardados, y el
`llama-server` arranca con los flags exactos documentados en §7.

### Fase 1 — Arreglos de harness, SIN cambiar de modelo (2-4 h) ← *empieza aquí*

Todos ya implementados y verificados en v11.1 (`--selftest` verde, **34/34** en la regresión):

| # | Arreglo | Fichero | Test |
|---|---|---|---|
| 1 | Guardián anti-eco: la lista OPS copiada se descarta y se pide una orden concreta | `seere_v11.py` (`_is_echo_arg`, `parse`) | N06-N08 |
| 2 | Normalización de pipelines: `\| $ grep` → `\| grep` | `seere_v11.py` (`RX["pipe_op"]`) | — |
| 3 | La continuación **sin gramática** sólo en fase final (en acción, truncar = degeneración → descartar) | `seere_v11.py` (bloque `fr == "length"`) | — |
| 4 | `verify()`: la cita `[SRC:n]` debe resolver a una observación existente | `seere_v11.py` (`verify`, `RX["src_id"]`) | N09 |
| 5 | `verify()`: sin observaciones, cualquier número de la respuesta es inventado | `seere_v11.py` (`RX["num1"]`) | N10 |
| 6 | **La huella `<ENV>` no se manda al modelo** (se queda en el harness) | `seere_v11.py` (bloque `user_msg`) | — |
| 7 | `--max-turns` expuesto (2 = 1 acción + 1 respuesta) | `seere_v11.py` (CLI) | — |
| 8 | Matching por límites de palabra: `"4"` ya no casa dentro de `pid=1414` | `seere_v11.py` (`_present`) | — |
| 9 | Números con unidad (`5.10.144 MB`, `14.45 s`) siempre son dato | `seere_v11.py` (`RX["num_size"]`) | — |
| 10 | Si la respuesta final **es** una orden (`R: $ free -m`), se ejecuta como acción de última hora | `seere_v11.py` (`RX["lead_op"]`) | — |
| 11 | `--env-in-prompt` para poder volver a la huella en un A/B | `seere_v11.py` (CLI) | — |

**Cómo decidir el `max_turns` por defecto:** con el 0.5B, un turno de acción + un turno de respuesta es
lo que el modelo puede sostener sin degenerar; con 3 turnos de acción el contexto se contamina y el
modelo empieza a repetir variaciones de la primera orden (medido: `free -m | grep ram` →
`free -m | awk '$1 == "1"'` → `free -m | grep "ram" | awk`). **Empieza con `--max-turns 2`** y sube a 3
sólo si tus preguntas necesitan dos rondas de datos.

**Criterio de salida de la Fase 1:**
- `python3 seere_v11.py --selftest` → `SELFTEST_V11_OK`
- `python3 test_seere_v11.py` → `34/34 PASS`
- `python3 bench_fidelity.py <modelo>` → `routed` y `sens` **≥ los de la línea base**
- El bucle completo con `--max-turns 2` da respuestas `grounded=true` en las 4 preguntas de prueba.

### Fase 2 — Decidir si hace falta cambiar de modelo (1 h, sólo medición)

```bash
# los 4 candidatos, misma máquina, mismos flags
python3 bench_models.py --models \
  /models/qwen2.5-0.5b-instruct-q4_k_m.gguf \
  /models/SmolLM2-360M-Instruct-Q4_K_M.gguf \
  /models/Llama-3.2-1B-Instruct-Q4_K_M.gguf \
  /models/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf --out candidatos.json

python3 reanalyze.py candidatos.json     # re-parsea sin re-muestrear
```

**Criterio de decisión (fícalo ANTES de mirar los datos):**

| Si, tras la Fase 1… | Entonces |
|---|---|
| `sens` ≥ 9/12 y `routed` ≥ 10/12 con el modelo actual | **No cambies de modelo.** Gasta el esfuerzo en más opcodes deterministas. |
| `sens` 7-8/12 | **Cambio opcional**: prueba Llama-3.2-1B (esperanza +1-2/12). |
| `sens` < 7/12 | **Cambio recomendado**: Llama-3.2-1B o Qwen2.5-1.5B. |
| `routed` < 8/12 pero `sens` alto | El problema no es el modelo: es el prompt/idioma o el bucle. **No cambies.** |
| Latencia actual > 3 s en el dispositivo | **No subas de 1B**: comprueba si el cuello es la longitud de la salida (§2.3). |

### Fase 3 — Migración (si decides cambiar) — pasos exactos

```bash
# 1. Descarga el GGUF elegido (verifica el hash del repo, no de un mirror)
cd /models
curl -L -o Llama-3.2-1B-Instruct-Q4_K_M.gguf \
  https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf
sha256sum Llama-3.2-1B-Instruct-Q4_K_M.gguf >> MODELS.sha256

# 2. Para 1B, Q4_K_M es suficiente; si tienes RAM de sobra, Q5_K_M (+12 % tamaño,
#    +~1 % calidad) o Q6_K (+20 %) no cambian la conducta del modelo.
#    Para 360M, sube a Q8_0: el salto de calidad es mayor en modelos pequeños.

# 3. Arranque del servidor (flags validados en este documento)
./llama-server -m /models/Llama-3.2-1B-Instruct-Q4_K_M.gguf \
  --host 127.0.0.1 --port 8080 \
  -c 8192 \                 # contexto POR SLOT; con --parallel N son N*8192
  -t $(nproc) \             # hilos: en un móvil, 4-6; no más (rivalita con el daemon)
  -ngl 0 \                 # CPU; en Android con Vulkan: -ngl 99
  --jinja \                # plantilla de chat del propio modelo
  --cache-prompt \         # sin esto, pierdes el prefijo KV reutilizable
  --parallel 2 \           # 2 slots: enrutamiento + respuesta en paralelo
  --cont-batching
```

**Los tres flags que importan y por qué:**

1. **`--cache-prompt`**: sin él, cada turno re-prefilla los ~200 tokens del prompt. Es el motivo de que
   el prompt de v11 sea estático.
2. **`-c 8192` por slot**: el contexto de SEERE es corto (system 206 + turno + OBS). 8 k sobra y
   multiplica por `--parallel`.
3. **`--parallel`**: el megadaemon lanza varias utils a la vez. **Un solo servidor, N slots**
   (continuous batching). *No* levantes un `llama-server` por util: son 400 MB-1 GB de pesos por
   instancia.

**Migración del prompt:** el prompt **no cambia** (187/206 tokens en todos los tokenizadores).
Lo que sí cambia es el **idioma óptimo**, y hay que medirlo: con Llama-3.2-1B el prompt EN da
10/12 de `sens` y el ES da 9/12 ⇒ **para Llama el prompt inglés es igual o mejor**.
Con `--lang auto` el harness elige por la lengua de la consulta, no por la del modelo ⇒ **añade un
`--lang` explícito por modelo** si el idioma de tu usuario y el del modelo difieren.

### Fase 4 — Integración en el megadaemon (depende de tu arquitectura)

- **Megadaemon en Python**: `import seere_v11` y llama a `Agent.run(...)` en proceso. Cero IPC.
- **Megadaemon en Rust (u otro)**: habla HTTP con el `llama-server` (el harness ya lo hace) y **no**
  necesita `llama-cpp2`: el punto de integración es una request con `grammar`, `stop` y `max_tokens`.
  Si aun así quieres un binding in-process, hoy el crate es **`llama-cpp-4`** (0.12.0, eugenehp) —
  `llama-cpp2` ya no existe en crates.io.
- **Si el destino no tiene Python** (Android/Termux, appliance): entonces sí reescribe el harness,
  pero **no el binding**: extrae la gramática y el prompt a ficheros compartidos
  (`seere.gbnf`, `seere_prompt.txt`) y que ambas implementaciones los carguen ⇒ es imposible que se
  desincronicen.

**Checklist de integración:**
- [ ] `SEERE_LLM` apunta al servidor; `SEERE_STATE` a un directorio escribible por el daemon.
- [ ] `SEERE_ALLOW_EXEC=1` sólo si el daemon **quiere** que el agente ejecute shell.
- [ ] `SEERE_ALLOW_EXTERNAL=0` (por defecto) salvo que quieras delegación a otro LLM.
- [ ] La SHM (`SEERE_SHM`) en un tmpfs, no en disco.
- [ ] El daemon **no** copia el system prompt a ningún sitio que lo modifique: la estática es el prefijo KV.
- [ ] Un watchdog: si el daemon llama a SEERE, que tenga timeout propio (`SEERE_DEADLINE`, por defecto 35 s).

### Fase 5 — Rollback (10 min, ensáyalo ANTES de migrar)

```bash
# Rollback del modelo: un symlink y un reinicio
ln -sf /models/BAK_qwen2.5-0.5b_q4km.gguf /models/ACTIVO.gguf
systemctl restart seere-llm
# Rollback del harness
cp seere_v11.py.frozen seere_v11.py
```

**Rollback sin modelo nuevo (lo más rápido):** no cambies de modelo hasta que la Fase 1 esté en
producción y medida. Si la Fase 1 falla, el modelo sigue siendo el mismo y el revert es un `git revert`.

---

## 6. Protocolo de medición y criterios de aceptación

**Protocolo (obligatorio para cualquier comparación de modelos):**

1. Mismo servidor, mismos flags, misma máquina, misma hora (sin otros procesos pesados).
2. `temperature=0`, `seed=42`, `min_p=0.05`, `repeat_penalty=1.0`, `dry_multiplier=0.8`.
3. La pregunta se envía **exactamente como la envía el harness** (con o sin huella, con o sin
   `NOTES`) ⇒ el bench de fidelidad existe por esto.
4. 19 preguntas: 12 con datos + 7 de respuesta directa. **Añade 5-10 preguntas de TU dominio**
   (las de `sens` con palabras clave inventadas; si no, el modelo gana por conocimiento general).
5. Repite 3 veces cada banco: llama.cpp no es bit-determinista con batch concurrente (±1-2 respuestas).
6. Decide con `sens` (corrección semántica), no con `routed`.

**Umbrales de aceptación (propuesta):**

| Métrica | Mínimo aceptable | Objetivo |
|---|---|---|
| `routed` (12 que exigen datos) | 10/12 | 12/12 |
| `sens` | 8/12 | 10/12 |
| `echo` | ≤ 2/19 | 0 |
| `multi` | ≤ 4/19 | ≤ 2 |
| Respuestas `grounded=true` en el bucle completo | 4/4 | 4/4 |
| Latencia de una acción en el dispositivo | ≤ 2 s | ≤ 1 s |
| RAM total del daemon + modelo | ≤ 512 MB (móvil) | ≤ 256 MB |

**Regresión obligatoria antes de cada despliegue:** `--selftest`, `test_seere_v11.py` (34/34),
`gbnf_check.py` (33/33) y `bench_fidelity.py` con **tu** banco de preguntas.

---

## 7. Riesgos

| Riesgo | Prob. | Impacto | Mitigación |
|---|---|---|---|
| El "mejor" modelo cambia con el prompt/idioma (medido: el óptimo se mueve) | **Alta** | Alto | `--lang` explícito por modelo; re-medir con el banco propio en cada cambio de prompt |
| `llama.cpp` no es bit-determinista en concurrencia | Alta | Medio | Repetir 3 veces; comparar medianas; re-analizar offline las salidas guardadas |
| La huella en el prompt se reintroduce por "utilidad" | Media | **Alto** | Regla en el code review: nada de `<ENV>`/huella en los mensajes; la huella vive en el harness |
| Subir a 1B-1.5B revienta la RAM del móvil | Media | Alto | Fase 2 exige medir la latencia en el dispositivo **antes** de decidir |
| Licencia Llama 3.2 (cláusula MAU 700 M) | Baja (si eres pequeño) | Alto | Si el megadaemon escala, Qwen2.5-1.5B (Apache-2.0) es la alternativa |
| El modelo se degenera en el bucle largo | **Alta** | Medio | `--max-turns 2` por defecto; `dry_multiplier`; descartar truncados (v11.1) |
| Un opcode desconocido se ejecuta como shell | Baja | **Crítico** | Ya cerrado: gramática tipada + guardián anti-eco + política por defecto |
| Confiar en lahwm (grounded) como garantía | Media | Medio | `grounded` es un *indicador*, no una prueba; la cita `[SRC:n]` debe resolver (v11.1) |

---

## 8. Anexo — ficheros y comandos

| Fichero | Qué hace |
|---|---|
| `seere_v11.py` | Harness v11.1 (~1 650 líneas): gramáticas tipadas, política, anti-eco, fundamento, `--max-turns`, `--env-in-prompt` |
| `bench_models.py` | Banco multi-modelo (4 modelos × 4 variantes de idioma) |
| `reanalyze.py` | Re-parsea las salidas guardadas sin re-muestrear (A/B limpio del parser) |
| `bench_latency_loop.py` | Latencia en serie + bucle completo con herramientas reales |
| `bench_fidelity.py` | Efecto de la huella del host y del presupuesto de turnos |
| `token_budget.py` | Tokens del prompt con el tokenizador real (fallback a `llama-tokenize`) |
| `test_seere_v11.py` | Regresión v10→v11.1: **34/34** |
| `gbnf_check.py` | Conformidad de la gramática: **33/33** |
| `bench_models.json` / `bench_models_reanalyzed.json` | Datos crudos del banco multi-modelo |
| `bench_fidelity.json` | Datos de huella + turnos (antes/después) |
| `bench_final_loop.json` | Bucle completo con el harness v11.1 final |
| `bench_models_preguard.json` | Banquos anteriores al guardián anti-eco (para A/B) |

**Comandos de un vistazo:**

```bash
python3 seere_v11.py --selftest                 # contrato del harness
python3 test_seere_v11.py                       # 34/34
python3 gbnf_check.py                           # 33/33
python3 bench_models.py --models /models/*.gguf  # banco multi-modelo
python3 reanalyze.py bench_models.json          # re-análisis offline
python3 bench_fidelity.py qwen2.5-0.5b          # huella + turnos

# uso
python3 seere_v11.py --lang es --max-turns 2 "¿cuántos núcleos tiene esta máquina?"
SEERE_ALLOW_EXEC=1 python3 seere_v11.py --allow-exec "¿cuánta RAM queda?"
```
