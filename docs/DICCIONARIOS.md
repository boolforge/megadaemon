# Diccionarios del usuario y corrección de erratas

> Módulo: `rust-core/src/dict_sync.rs` · CLI: `rust-core/src/bin/md-dict.rs`
> Estado: **20/20 tests del módulo verdes**, suite completa **719 passed / 0 failed**.

## 1. Qué hace

Dos cosas que el usuario pidió y que hasta ahora nadie tenía:

1. **Sincronizar el diccionario de teclado entre nodos.** Cada dispositivo
   (móvil, tablet, PC) tiene el **diccionario personal** de AOSP, y son
   distintos: lo que aprendes en el móvil no aparece en el otro lado.
2. **Corregir erratas** de forma automática o casi automática, usando ese
   mismo diccionario: si escribes "qeu" y tu diccionario tiene "que", se
   corrige; si escribes "habia" y tienes "había", se propone.

## 2. Medición en el móvil (comandos exactos)

```sh
# Volcado del diccionario personal (root):
su -mm -c 'content query --uri content://user_dictionary/words --projection word:shortcut:timestamp'

# Volcado en JSON (para las herramientas que lo exportan así):
su -mm -c 'content query --uri content://user_dictionary/words --projection word:shortcut:timestamp' \
  | sed 's/^Row: [0-9]* //'
```

El parser acepta **las dos formas**:

* `content query` → filas `Row: 0 word=habla, shortcut=habia, timestamp=1712345678`
* JSON → `[{"word":"habla","shortcut":"habia","timestamp":1712345678}]`

Y es **tolerante**: una fila que no se entiende se salta, no rompe el resto.

> **NO VERIFICADO**: el esquema exacto que devuelve este Samsung S21 Ultra
> con Android 14 (la app de teclado de Samsung puede tener su propio
> diccionario, no el de AOSP). El parser está escrito para tolerar
> columnas de más o de menos; falta la medición real, que se hace con el
> comando de arriba.

## 3. El corrector: cómo decide

La confianza **no es una caja negra**: cada sugerencia lleva su `motivo`.

| Motivo | Ejemplo | Confianza | ¿Se autocorrige? |
|---|---|---|---|
| `transposicion` | `qeu` → `que` | 0,92 | sí |
| `caracteres_confusables` | `rn` → `m`, `0` → `o` | 0,88 | sí |
| `un_caracter` (≥6 letras) | `administracion` → `administracio` | 0,95 | sí |
| `un_caracter` (5 letras) | `habia` → `habla` | 0,75 | **no** (sólo se propone) |
| `un_caracter` (4 letras) | `casa` → `caso` | 0,55 | **no** |
| `un_caracter` (3 letras) | `abc` → `abd` | 0,40 | no |
| `caracteres_repetidos` | `aaa` → `a` | 0,75 | no |
| `dos_caracteres` (≥6 letras) | — | 0,78 | no |

**Por qué la longitud importa**: "casa" y "caso" son dos palabras reales y
autocorregir una por otra sería un error. "administración" no tiene
competidor. Por eso una distancia de 1 vale 0,95 con 6+ letras y 0,55 con
4.

El umbral por defecto es **0,9**: sólo se aplica lo que es prácticamente
seguro; el resto se **sugiere** para que el usuario lo acepte.

La sustitución respeta **límites de palabra** (sin regex): "qeu" NO se
toca dentro de "alqueusar".

## 4. Sincronización

Formato: **NDJSON append-only** (una entrada por línea) + un **manifiesto**
con el SHA-256 y el número de entradas.

```json
{"palabra":"habla","atajos":["habia"],"ts":1712345678,"origen":"teclado"}
```

```json
{"entradas": 2, "sha256": "…64 hex…", "bytes": 118}
```

* **Merge idempotente**: la clave es la palabra **normalizada** (minúsculas,
  sin acentos). Gana la entrada con el `ts` mayor; si empatan, la entrante.
  Los atajos se unen.
* **Verificación**: `verificar(ndjson, manifiesto)` falla si el SHA-256 o el
  número de entradas no cuadran. Detecta manipulación.
* Los ficheros van **0600** en el `state_dir`.

## 5. CLI

```sh
# ver el diccionario volcado
md-dict scan --file volcado.txt

# corregir texto (sin aplicar)
md-dict correct --dict volcado.txt --text "qeu habia megadaemonn"

# corregir y aplicar por encima del umbral
md-dict correct --dict volcado.txt --text "qeu habia" --apply --umbral 0.9

# fusionar dos diccionarios (idempotente)
md-dict merge --a movil.ndjson --b pc.ndjson
```

Salida real medida:

```
  APLICADA  megadaemonn -> megadaemon  (0.95, un_caracter)
  sugerida  habia -> habla  (0.75, un_caracter)
```

## 6. Lo que falta (y es lo siguiente)

* **Aplicar las correcciones al vuelo**: hoy es una API y un CLI. Para
  corregir mientras se teclea hace falta un IME propio o usar la
  accesibilidad de Android; el corrector ya está preparado para eso
  (`sugerir` + `aplicar` son funciones puras).
* **Los diccionarios del teclado de Samsung** (no el de AOSP): hay que
  medir en el móvil dónde guarda Samsung sus palabras aprendidas.
* **La GUI**: cuando el módulo esté cableado al MCP, el natural para la GUI es
  `GET /api/dict` (entradas + manifiesto) y `POST /api/dict/correct` (texto
  → sugerencias). Como el resto, **primero la funcionalidad, luego la GUI**.
