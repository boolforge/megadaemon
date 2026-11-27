# RE masivo por app — priorización, presupuesto y digest

> **Estado**: 2026-11-26 · rama `rama/re-masivo-2026-11-26`
>
> Lo **MEDIDO** y lo **NO VERIFICADO** están separados. Este módulo **no se
> ha compilado en el sandbox** (3 GB de RAM: prohibido por
> `docs/PROTOCOLO_FLOCKA.md`); lo compila y testea el CI.

---

## 1. El problema que resuelve

El repo ya tenía las piezas sueltas:

| pieza | qué hacía | qué le faltaba |
|-------|-----------|-----------------|
| `apk_inventory` | inventario y ficha de los APK instalados | volcar el JSON completo |
| `app_intel` | perfiles, endpoints, secretos, almacenamiento | no prioriza |
| `root_intel::detectors` | **apps que vigilan el root**, con categorías | no decía cuál mirar primero ni por qué |
| `root_occlusion::scan_apk` | señales de detección (12 familias) en DEX y `.so` | estático, sin contexto de app |
| `dex_decompile` | pseudo-Java, desensamblado, mapa de desofuscación | por app, a mano |

El problema real: **meter eso en el contexto de un modelo son megabytes por
app**. Con 50 apps el presupuesto de tokens se agota antes de mirar la
tercera. Y nadie tenía el "**por qué** esta app es interesante" en una línea.

`mass_re.rs` es la **capa de orquestación** que faltaba:

1. **Worklist** — qué apps mirar primero y **por qué** (puntaje + motivos).
2. **Unidad por app** — cada app se analiza con un **presupuesto** y su
   **bitácora NDJSON** propia (si una revienta, no arrastra a las demás).
3. **Digest** — una **tabla** (una línea por app) que es lo que va al
   contexto del modelo. Los volcados completos se quedan en el disco.

---

## 2. La heurística (puntaje) y por qué está donde está

`mass_re::puntuar(&Evidencia) -> Puntaje` es una **función pura** (no toca
disco ni red) → se testea sola y se ajustan los pesos sin tocar el resto.

| evidencia | puntos | por qué |
|-----------|-------:|---------|
| veredicto `detector_fuerte` | +45 | la app tiene un detector de root de verdad |
| veredicto `detector_medio` | +28 | |
| veredicto `detector_debil` | +12 | |
| familia `attestation` (Play Integrity) | +18 | **no se puede evadir por hooks**: hay que responder como un móvil limpio |
| familia `hook_framework` (Xposed/Frida) | +16 | detecta el hook, no el root |
| familia `root_api` | +12 | la app trae su propio `isRooted()` |
| familia `root_library` | +10 | |
| `su_binary` / `magisk_binary` | +8 | |
| `build_props` / `proc_mounts` | +6 | |
| 3+ familias distintas | +10 | muchas técnicas = difícil de tapar |
| 5+ familias distintas | +16 | |
| ofuscación >80% de clases | +26 | el código hay que reconstruirlo |
| ofuscación >50% | +18 | |
| ofuscación >25% | +8 | |
| 10+ permisos peligrosos | +15 | superficie de datos |
| 3+ permisos peligrosos | +8 | |
| nombre de app sensible (banca/pagos) | +20 | |
| código nativo presente | +6 | la RE cuesta más y suele esconder el detector |

Niveles: `>=75` **crítico**, `>=50` **alto**, `>=28` **medio**, resto **bajo**.

**La ofuscación se mide de verdad**: ratio de clases del DEX con nombre de
1-2 caracteres (`Lcom/x/a;`) sobre el total (`dex_decompile::load_dex`).

---

## 3. El presupuesto (por qué no se cuelga)

```jsonc
{ "max_apps": 25, "max_por_app_ms": 20000,
  "max_por_app_bytes": 201326592, "max_senales_por_app": 12 }
```

* **Por app**: si se agotan los bytes o el tiempo, el paso caro (parsear el
  DEX) **no se hace** y la bitácora lo dice (`presupuesto agotado`).
* **Por tanda**: `max_apps` corta la lista.
* Un `0` en el JSON **no** desactiva el límite (sería un agujero): se usa el
  valor por defecto.  Hay un test que lo fija.

---

## 4. La bitácora (el log ultra detallado)

Por app: `<state_dir>/re_masivo/<paquete>.ndjson`, una línea por paso:

```json
{"ts":1764144000123,"paquete":"com.bbva.bbvacontigo","paso":"scan_deteccion","ok":true,"ms":812,"bytes":41234432,"detalle":"14 señales, 3 dex, 2 nativas"}
```

Pasos: `ruta_apk` → `scan_deteccion` → `ficha_apk` → `ofuscacion` →
`resumen`.

Global: `<state_dir>/re_masivo/batch.ndjson`, **una línea por app** (el
resumen).  Y `<state_dir>/re_masivo/digest.md` con la tabla (0600).

---

## 5. Uso

### Tool MCP `re_masivo`

```jsonc
{ "action": "worklist", "limite": 40 }              // prioriza, con el POR QUE
{ "action": "analizar", "paquete": "com.bbva.bbvacontigo" }
{ "action": "analizar_lote", "limite": 10 }         // worklist + análisis + digest
{ "action": "digest" }                              // la tabla compacta
{ "action": "bitacora", "paquete": "com.bbva.bbvacontigo", "lineas": 100 }
```

### Salida del digest (esto es lo que va al modelo)

```markdown
| app | nivel | pts | coste | por que |
|---|---|---:|---|---|
| com.bbva.bbvacontigo | critico | 112 | alto | +45 detector de root FUERTE; +26 ofuscacion fuerte (>80% de clases); +20 app sensible (banca/pagos/seguridad) |
| com.revolut.mobile | alto | 74 | medio | +28 detector de root MEDIO; +18 vigilancia de 'attestation'; +15 10+ permisos peligrosos |
```

Una app = **una línea**. 200 apps = ~20 KiB (frente a los cientos de MiB de
los volcados).

---

## 6. Qué falta (NO VERIFICADO / pendiente)

| # | qué | cómo se verifica |
|---|-----|------------------|
| 1 | Que el crate compile (el módulo usa `root_intel`, `root_occlusion`, `apk_inventory`, `dex_decompile`) | el CI (`calidad`) |
| 2 | Que la worklist devuelva las apps con detector en **este** móvil | tool `re_masivo` con `action=worklist` en el móvil |
| 3 | Que el presupuesto aguante una app de 200 MB | `action=analizar` con `max_por_app_bytes` bajo: la bitácora debe decir `presupuesto agotado` |
| 4 | Que el digest ordering sea el esperado | `action=analizar_lote` y comparar con la worklist |
| 5 | El **fan-out "un agente por app"** (cola con trabajo por app, reintentos yWorkers) | **NO IMPLEMENTADO**: hoy el lote es secuencial dentro de la tool.  La cola que ya existe (`job_queue`, `board`, `task_scheduler`) es el sitio natural; queda pendiente |
| 6 | Un **modo de escritura por app** (volcar el DEX decompilado a disco por app, para poder bucear después) | **NO IMPLEMENTADO**: `apk_inventory::dump_all` existe pero no está enganchado al pipeline |

---

## 7. Ficheros

| fichero | qué es |
|---------|--------|
| `rust-core/src/mass_re.rs` | el módulo (puntaje, worklist, análisis por app, bitácora, digest, tool MCP) |
| `rust-core/tests/re_masivo.rs` | arnés (tool declarada, dispatch, puntuación determinista, digest vacío, presupuesto) |
| `rust-core/src/mcp_ext.rs` | alta de la tool en el catálogo MCP (+ dispatch) |
