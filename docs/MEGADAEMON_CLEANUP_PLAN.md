# Plan de higiene del repo MegaDaemon: ramas, duplicación y "mega store manager"

> Pedido del usuario (2026-11-20): *"arreglar el desastre de branches y cosas raras en megadaemon y
> código duplicado y mil chapizas y desorden etc"* + *"ampliar megadaemon para que haga más cosas
> incluyendo que sea un mega repo/store manager universal super ligero y ya se buscará un gui etc"*.
> Todos los números de este documento están **medidos sobre el repo**, no estimados.

## 1. Estado real de las ramas (medido)

```
rama                                  ahead  behind  estado
fix/megadaemon-stability-20261119      10     253   OBSOLETA (sus 10 commits ya están en main)
fix/robustness-20261120                  3      83   VIVA  (PR #7: root_intel + test + docs)
publish/drive-handoff-20261119           0     159   FUSIONADA (0 commits únicos) -> borrar
publish/state-sync                       1       0   FUSIONADA (el commit ya está en la rama viva) -> borrar
```

Remotas: sólo `origin/main` (+ `origin/HEAD`). No hay ramas remotas huérfanas ⇒ el "desastre" es
**local**: 4 ramas donde sólo hace falta 1.

**Plan de saneado (sin perder trabajo)**:
1. `git branch -d publish/drive-handoff-20261119` (0 commits únicos ⇒ borrado seguro).
2. `git branch -d publish/state-sync` (1 commit ya presente en `fix/robustness` como `9358f4e`).
3. `fix/megadaemon-stability-20261119`: sus 10 commits **ya están en `main`** (está 253 commits
   detrás, luego main los absorbió) ⇒ se puede borrar tras verificar con
   `git cherry main fix/megadaemon-stability-20261119` (debe salir vacío).
4. Dejar **una sola rama de trabajo** (`fix/robustness-20261120`) y `main`.
5. Regla nueva: **una rama por trabajo, nombre `fix/<tema>-<fecha>`, y se borra al integrar**.
   Las ramas `publish/*` se usaron para "publicar estado" yYa no hacen falta: el estado va al
   **tablón** (`board.sqlite`) y al **journal**, que es la vía canónica.

## 2. Duplicación: el dato real (medido)

```
módulos en rust-core/src: 76      líneas totales: 81.522
funciones con cuerpo DUPLICADO EXACTO: 12 (en 5 grupos)
```

| duplicado | veces | dónde | arreglo |
|---|---|---|---|
| `walk_files` | 3 | `backup_full.rs`, `backup_vault.rs`, `call_recorder.rs` | → `util_fs.rs::walk_files` |
| `hex` | 3 | `apk_inventory.rs`, `dex_decompile.rs`, `signing_vault.rs` | → `util_text.rs::hex` |
| `now_iso` | 2 | `agent_voice.rs`, `github_connector.rs` | → `util_text.rs::now_iso` |
| `format_iso8601` / `epoch_to_iso` | 2 | `signing.rs`, `repo_updates.rs` | → `util_text.rs::format_iso8601` |
| `parse_apk_paths` / `parse_package_names` | 2 | `apk_inventory.rs` (interno) | → un solo parser |

⇒ **La "duplicación" real es de 12 funciones**, no de miles. El desorden real es de otro tipo:

### 2.1 Concentración (medido)

| fichero | líneas |
|---|---|
| `signing.rs` | **5.887** |
| `dex_decompile.rs` | 4.995 |
| `webui.rs` | 4.767 |
| `messaging.rs` | 3.655 |
| `profile_analysis.rs` | 2.820 |
| `delegate.rs` | 2.636 |
| `rom_migration.rs` | 2.549 |
| `battery_advanced.rs` | 2.384 |
| `seere_v11.rs` | 2.348 |
| `repo_updates.rs` | 2.336 |

**El top-10 = ~37.000 líneas = 45 % del crate.** Eso sí es el "desorden" que hay que atacar:
`signing.rs` con 5.887 líneas es un monolithio.

### 2.2 Qué hacer (por orden de relación esfuerzo/beneficio)

1. **`util_text.rs` / `util_fs.rs`**: absorber los 12 duplicados exactos (mecánico, sin riesgo,
   ~1 hora). El crate ya tiene `util_text.rs`: es el sitio natural.
2. **Trocear `signing.rs`** por dominio: `signing/keys.rs`, `signing/pkcs.rs`, `signing/vault_io.rs`,
   `signing/format.rs`. Sin cambiar la API pública (`pub use` en `signing/mod.rs`).
3. **`dex_decompile.rs`**: el parser DEX ya lo sabe hacer otro nodo (`tools/dex_deobf.py`): unificar
   el parser Rust con el que usa el pipeline Python ⇒ un solo formato de error y una sola tabla de
   strings. (El bug `encoded_value 0x17 = VALUE_ARRAY` se arregla de una vez para todos.)
4. **`webui.rs`**: 4.767 líneas de UI en un módulo ⇒ separar en `webui/{cockpit,mesh,store,logs}.rs`.
5. **Limpieza de módulos "vivos" que ya no lo están**: `seere_v11.rs` (2.348) es de una versión
   anterior del runtime; `delegate.rs` (2.636) y `profile_analysis.rs` (2.820) deben revisarse:
   ¿siguen invocados por `mcp_ext.rs`? Si no, se archivan en `legacy/` con su test.
6. **Tool registry declarativa**: hoy cada tool se cablea a mano en `mcp_ext.rs` (53 tools). Con un
   macro/tabla única, añadir una tool = una entrada, y "lo que hace más cosas" deja de ser un
  接线 manual de 50 líneas por tool.

## 3. "Mega repo / store manager universal" (la idea, con diseño)

Objetivo del usuario: que MegaDaemon sea un **gestor universal de repos/stores**, **super ligero**,
con GUI más adelante.

### 3.1 Diseño: un `store` es un origen + un índice + un instalador

```
StoreDescriptor {
   id, nombre, tipo (magisk_module | zygisk_module | apk | rom | payload | tool),
   base_url, indice (json), firma (sha256 por artefacto), 
   instalador: Magisk | pm (APK) |odin/payload | nativo
}
```

- **Índice**: un `catalog.json` por store, firmado; el daemon cachea el índice en
  `state/stores/<id>/catalog.json` y lo refresca con `--ttl`.
- **Instaladores**: `magisk_module` (copiar a `/data/adb/modules/<id>` + `module.prop`),
  `apk` (`pm install` con `REQUEST_INSTALL_PACKAGES` ya gestionado por `app_control`),
  `payload` ( Odin/`cscfeature.xml` con `rom_migration`), `tool` (binario nativo en el vault).
- **Transporte**: el daemon **no** necesita Google Play ni nada de Google: son índices HTTP
  firmados. Eso lo hace **ligero** (nada de SDKs, nada de OAuth salvo que el store lo pida).
- **Verificación**: sha256 por artefacto + verificación de la firma del índice (el maestro ya tiene
  `signing_vault.rs` y `signing.rs` para esto).
- **Un solo cliente HTTP**: el crate ya usa `ureq` (con `rustls`) ⇒ cero dependencias nuevas.

### 3.2 "Super ligero": qué significa aquí

| decisión | efecto |
|---|---|
| sin SDK de Google, sin rclone, sin binario externo | binario pequeño y arranque instantáneo |
| índices cacheados en JSON plano | arranque sin red |
| subida de estado por *diff* (NDJSON append) | 1 escritura por ciclo |
| `delegar` el trabajo pesado al vault/sandbox | el daemonCoordina, no ejecuta |

⇒ El binario actual (8,95 MB) con el cliente de Drive nativo, un registry declarativo y sin
dependencias nuevas **no crece**: el cliente de Drive cabe en `backup_vault.rs` (~200 líneas).

### 3.3 GUI: más adelante, pero el contrato ya

- Los stores exponen un **JSON estable** (`/api/stores`, `/api/stores/<id>/catalog`,
  `/api/stores/<id>/install`).
- La GUI (web o app) sólo consume ese JSON ⇒ no depende de la UI interna.
- Regla: **la GUI nunca toca el disco del daemon**; todo pasa por la API.

## 4. Qué queda **fuera** de mi alcance (y por qué)

- **No** toco el bootloader ni el KVB del usuario (decisión suya, no mía).
- **No** hago flasheo de ROM (fuera del encargo original del proyecto).
- **No** borro nada del móvil: el backup es *upload primero, limpieza después*, y la limpieza
  final la decide el usuario.

## 5. Checklist de este plan

- [x] medir ramas y decidir qué se borra (`publish/*` fusionadas, `fix/megadaemon-stability` obsoleta)
- [x] medir duplicación real (12 funciones exactas) y concentración (top-10 = 45 % del crate)
- [x] definir el diseño de `StoreDescriptor` + instaladores + verificación
- [ ] absorber los 12 duplicados en `util_text.rs`/`util_fs.rs` (mecánico)
- [ ] trocear `signing.rs` (5.887 líneas) en sub-módulos
- [ ] unificar el parser DEX Rust con `tools/dex_deobf.py` (arregla el bug 0x17)
- [ ] registry declarativa de tools (de 53 cableados a 1 tabla)
- [ ] cliente de Drive nativo ampliado a "carpeta + 100 % + stream" (ver `BACKUP_100_STREAM.md`)
- [x] **borrar las ramas fusionadas y dejar una sola rama de trabajo** — ejecutado: `publish/drive-handoff-20261119` (0 commits únicos) y `publish/state-sync` (commit ya presente en `fix/robustness` como `9358f4e`) **borradas**. De 4 ramas → 3 (`main` + 2 PRs vivos).
- [x] **`fix/megadaemon-stability-20261119` rebaseada sobre `main`**: de *ahead 10 / behind 253* → **ahead 2 / behind 0** (los 9 commits de estado ya estaban en `main`; sólo quedan los 2 commits de código de `peers.rs`). PR #6 actualizada con force-push.
