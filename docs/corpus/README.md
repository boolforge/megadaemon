# Corpus de conversaciones — todos los agentes

Todo lo dicho en las sesiones de todos los agentes, volcado a notas legibles,
decodibles e indexables. Este directorio es el entregable; se regenera con
`python3 tools/corpus_build.py` + `python3 tools/corpus_facts.py`.

## Estado medido

| magnitud | valor |
|---|---|
| sesiones registradas en el backend | **97** |
| sesiones con al menos una página | **96** (5 sin ningún mensaje) |
| páginas únicas recuperadas | **96** |
| mensajes volcados | **7 358** |
| sesiones completas (sin truncado) | **31** |
| sesiones truncadas por el límite del backend | **65** |
| índice de términos | 4 000 términos en `KEYWORDS.tsv` |

## Ficheros

| fichero | contenido |
|---|---|
| `sessions/*.md` | transcripción por sesión: cabecera YAML + mensajes con razonamiento y llamadas a herramienta |
| `MESSAGES.jsonl` | un mensaje por línea, aplanado y con `session_id` + índice (17 MB, grepeable) |
| `KEYWORDS.tsv` | índice invertido término → nº documentos / nº sesiones |
| `SESSIONS_INDEX.tsv` | metadatos de las 97 sesiones (agente, título, fechas) |
| `COVERAGE.tsv` | **cobertura real por sesión**: páginas, mensajes, primer/último timestamp, si está completa y el cursor |
| `PAGINATE_PLAN.tsv` | qué falta en cada sesión y con qué cursor pedirla |
| `CURSORS.tsv` | cursor de paginación por sesión |
| `FACTS.md` | **índice temático**: 9 temas con los mensajes de mayor señal, referenciados (sesión · nº de mensaje) |
| `INDEX.md` | tabla de sesiones con enlace al fichero correspondiente |
| `inline/*.json` | respuestas que el harness dejó en línea (no persistidas) y sehado a mano |

## LIMITACIÓN MEDIDA (importante)

`session_messages` **no pagina**. Medido en esta sesión:

1. **El parámetro `before` es inoperante.** La misma llamada con `before`
   distinto devuelve la ventana *idéntica* (mismo hash de contenido):
   `before=1795237399195` y `sin before` → ambas devuelven
   `[1795233656315, 1795274996040]`, 100 mensajes, mismo sha256 de la lista.
2. **Tope duro de 100 mensajes.** `limit=500` devuelve 100. `limit=5` devuelve
   los 5 **últimos**.
3. **Siempre los últimos.** Sin `before`, la respuesta es la ventana final de
   la sesión (los 100 últimos mensajes); si la sesión tiene ≤100 mensajes,
   sale completa.

Consecuencia: **para las 65 sesiones con `has_more=true` faltan los mensajes
del principio** y no hay forma de obtenerlos con la herramienta expuesta.
`COVERAGE.tsv` marca esas sesiones con `nota=tiene_mas` y su cursor.

**NO VERIFICADO**: si el backend tiene un cursor de paginación real (por
`msg_id` y no por `timestamp`) que la herramienta no expone. La API pública
`session_messages(session_id, limit, before)` no lo ofrece.

## Cómo se resolvió el reparto página → sesión

Como `before` no pagina, **cada sesión aporta como mucho una página** ⇒ el
reparto es una biyección. El asignador (`tools/corpus_build.py`,
`assign_pages`) empareja por coste mínimo:

```
coste(página, sesión) = |inicio_página − created_at| + |fin_página − updated_at|
```

Una página completa arranca exactamente en `created_at` y termina en el
último mensaje, luego el coste ~0 identifica la sesión sin ambigüedad. Las
páginas se deduplican por hash de contenido antes de asignar (45 de 153
artefactos iniciales eran duplicados: cada llamada MCP genera un artefacto
nuevo aunque la página sea la misma).

### Qué artefactos se leen

| fuente | qué aporta |
|---|---|
| `/workspace/.mavis/tool-results/*/*.readable.v1.jsonl` | respuestas externalizadas por el harness (las grandes) |
| `/workspace/.mavis/tool-results/*/call_*/chunks/*.txt` | artefactos crudos de una sesión anterior (25 `session_messages`, hasta 5,4 MB, truncados con estrategia `global_head_tail`) |
| `docs/corpus/inline/*.json` | respuestas pequeñas que el harness dejó en línea y no persistió, recopiladas a mano |

## Reproducir

```bash
cd /workspace/antigravity
python3 tools/corpus_build.py     # reconstruye sesiones/, MESSAGES.jsonl, índices
python3 tools/corpus_facts.py     # regenera FACTS.md
```
