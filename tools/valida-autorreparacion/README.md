# Arnés AISLADO de `autorreparacion` (validar la `lib` sin compilarla entera)

## POR QUÉ EXISTE

En este repo **no se puede compilar el crate real en local**: son 2 CPU y
3 GB **sin swap** ⇒ el OOM-killer se come el `cargo check` (y también el
`cargo build`).  Por eso los cambios en `rust-core/src/` se validan en un
crate **aislado** que incluye los **MISMOS ficheros fuente** con `#[path]` y
**stubs** para los módulos que el daemon sí tiene.

Este arnés cubre `autorreparacion.rs` (el motor de autorreparación) y
`reparar_entorno.rs` (el reparador de base).

## CÓMO SE EJECUTA

```sh
cd tools/valida-autorreparacion
GOCACHE=/tmp/gocache-local TMPDIR=/tmp cargo test --offline --lib
```

⇒ **37 tests en ~2 s** (con el crate real serían ~15-20 min de CI).

El crate lleva `[workspace]` vacío ⇒ **no** se cuelga del workspace del repo
(es un crate independiente, como `tools/word-fusion-lint`).

## LA LECCIÓN (2026-11-28): UN STUB MENTIROSO ESCONDE EL BUG

El CI salió rojo con este fallo:

```
autorreparacion::tests::un_entorno_sano_no_tiene_sintomas
  panicked at src/autorreparacion.rs:2698:
  "un entorno sano no tiene sintomas: ["canal_no_publicado"]"
```

La causa: `diagnosticar()` leía el canal con `channel::read_channel_json`, que
va al **sistema de ficheros REAL**, mientras que el resto del diagnóstico
(la telemetría) lee por la abstracción `sis` (el `Fake` de los tests tiene sus
ficheros **en memoria**) ⇒ el `channel.json` del entorno de mentira no se veía
⇒ el síntoma salía **siempre** presente.

**El bug estaba escondido porque el stub de este arnés devolvía SIEMPRE un
canal publicado** (`read_channel_json` devolvía un JSON de mentira).  Con el
stub **fiel** (leer del disco, como el crate real) el fallo aparece exacto
como en el CI:

| código | resultado en el arnés |
|---|---|
| `97fdc01` (el del gallito 6) | 2 tests **fallan**, mismo mensaje y mismas líneas (2698, 2883) |
| con el arreglo | **37 passed; 0 failed** |

⇒ **REGLA: un stub debe fallar como el código real.  Si el stub siempre
responde "bien", el test no prueba nada.**

## REGLAS PARA QUIEN TOQUE ESTE ARNÉS

1. **El stub tiene que ser fiel** al crate real: si el real lee el disco, el
   stub lee el disco (no devuelve valores inventados).
2. Si añades un `crate::` nuevo en el módulo, añádelo también al stub.
3. `tools/check-arnes.py` vigila `rust-core/tests/` (NO `tools/`): este
   arnés puede usar `#[path]` porque **es un crate aparte**, no un target de
   test de la librería (dentro de un target de test, `crate::` es la raíz del
   arnés y el módulo copiado no compila ⇒ por eso el gate).
