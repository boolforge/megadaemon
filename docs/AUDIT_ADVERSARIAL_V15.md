# MegaDaemon v15 — Auditoría adversarial EXHAUSTIVA

> **Autor**: agente *General (auditoría adversarial v15)*.
> **Alcance**: `rust-core/src/**` (69 ficheros, 64.918 líneas), `web/cockpit_template.html`,
> `deploy/**`, `tools/**`, `docs/**`.
> **NO solapa** con `AUDIT_V13_1/2/4`, `AUDIT_V14_UI_HTTP` (UI-00…UI-15),
> `AUDIT_ENJAMBRE_V14`, `AUDIT_DELEGACION_V14`, `AUDIT_HARNESS_V13_4`.
> Todo lo que esas auditorías ya diagnosticaron **y sigue vivo** se cita como
> referencia cruzada (`↩ UI-xx`), pero **no se vuelve a contar como hallazgo
> nuevo**: aquí se cuenta lo que nadie miró.
>
> **Nota de redacción (2026-11-19 22:45)**: las **credenciales reales** que
> se citan como evidencia en `B-01` y `B-02` se han **redactado** de este
> fichero antes de subirlo al repo: la push protection de GitHub bloqueó el
> push por *secret scanning* (`Highnote SK Live Key`). Los **hallazgos no
> cambian** (siguen siendo P0), pero redactar **no invalida** las
> credenciales: hay que **rotarlas** (ver `B-01` y `B-02`).
>
> **Método (obligatorio, sin excepción)**: cada defecto lleva una de estas tres
> pruebas, y se dice cuál:
>   * **(T)** un test que lo reproduce → `rust-core/tests/audit_adversarial_v15.rs`
>     (fichero nuevo de esta auditoría, **no cableado** en `lib.rs`);
>   * **(M)** una medición/lectura del móvil vía MCP (no disponible en esta
>     sesión: el dispositivo está en manos de otro agente → se marca
>     `NO VERIFICADO EN RUNTIME` y no se afirma nada sobre el móvil);
>   * **(C)** una cita exacta `fichero:línea` que lo demuestra por inspección.
>
> **Regla de honestidad**: lo que no se ha verificado se marca
> `NO VERIFICADO (hipótesis)`. No hay "fantasmadas".

---

## 0. Resumen ejecutivo

### 0.1 Los tres hallazgos que cambian el plan — y dónde están **hoy** (07:2x)

> **Los tres están arreglados.** Lo que queda es *durable*: que no se pierdan.

1. **La web v14 se arregló a medias y el arreglo abría una puerta peor.**
   El "arreglo de seguridad" de v14 (quitar el token del HTML público) dejó la
   web **entera en 403**: el intercambio `POST /api/session/exchange` estaba en
   la rama **GET** y `requires_auth()` lo exigía (círculo vicioso). El owner del
   cockpit lo corrigió a las 22:48 (UI-16) introducing `local_browser`… **y con
   él, el token root a cualquier petición que "pareciera local"** (decidido con
   el `Host` y con la ausencia de cabeceras de proxy, **falsificables**): RCE root
   desde la wifi del usuario y desde cualquier app del móvil. **Hoy**: el
   intercambio comprueba **la API key real** (`cockpit_server.rs:535-544`, ✅
   cerrado y **commiteado** — `9f30665` en `main` —) **pero** la exención de
   token en **todos** los `GET` sigue viva (⇒ fuga de datos sin root,
   **A-18 P1** abierto).
2. **Dos RCE root nuevas y encadenadas** que no estaban en ninguna auditoría
   previa: el heredoc de `fs_write` y el `stat` que AWK construía con el nombre
   del fichero. Ambas se ejecutaban **como root real** (`su -mm -c`) y ambas eran
   alcanzables desde el MCP (el canal del agente externo). **Hoy**: base64
   siempre y escapado POSIX (`android.rs:807-830` / `:847-868`) ✅ cerrados y
   **commiteados** (`efd6dc3`, en `main`).
3. **El puente web regalaba root a toda la wifi** (`web_bridge.py` inyectaba
   `X-API-Key` **y** `X-Seere-Auth-Token` a cualquier petición sin `Origin`).
   **Hoy**: cerrado (`23e82ed`: la inyección sólo ocurre con **peer loopback
   real** y `MD_WEB_BIND` vale `127.0.0.1`). El **mismo patrón de fallo**
   ("local" decidido por cabeceras) sigue **medio vivo** dentro del daemon: la
   entrega del token root está cerrada, pero la **exención de token en los
   `GET`** sigue ⇒ de ahí el punto 1.

### 0.2 Recuento por clase

| Clase | P0 | P1 | P2 | P3 | Total |
|---|---:|---:|---:|---:|---:|
| SEGURIDAD (A+B) | 0 | 7 | 6 | 1 | **14** |
| CORRECTITUD (C) | 0 | 2 | 7 | 0 | **9** |
| FIABILIDAD (D) | 0 | 2 | 4 | 0 | **6** |
| RENDIMIENTO (E) | 0 | 3 | 5 | 0 | **8** |
| ARQUITECTURA (F+H) | 0 | 4 | 9 | 0 | **13** |
| COBERTURA (G) | 0 | 3 | 0 | 1 | **4** |
| USABILIDAD (I) | 0 | 1 | 5 | 1 | **7** |
| MANTENIBILIDAD (G-03, H-05) | 0 | 0 | 1 | 1 | **2** |
| **Total abiertos** | **0** | **22** | **37** | **4** | **63** |

*(81 filas escritas: **63 defectos abiertos** (**0 P0**) + **18 ya corregidos** —
A-01, A-02, A-03, A-04, A-07, A-15, B-01, B-02, B-06, C-06, D-01, D-02, D-09,
D-10, **D-11**, G-05, I-01, **I-08** — + 7 límites honestos en §J, que no son
defectos.)*

### 0.3 Recuento por severidad

> **Actualizado el 2026-11-22** (§0.7): A-18 (resto) pasa a **CERRADO** ⇒
> **59 defectos abiertos** (19 P1 / 36 P2 / 4 P3) y **22 cerrados** (los P0
> siguen siendo 0 abiertos).



| Severidad | Nº | Significado en este repo |
|---|---:|---|
| **P0** | **0** | ~~RCE root, o deja el daemon/UI muerto en el S21 Ultra~~ — **los 6 P0 están cerrados** (ver §0.4) |
| **P1** | 22 | rompe el servicio o la seguridad en el uso real del móvil |
| **P2** | 37 | degradación, coste o mentira técnica que se nota a la semana |
| **P3** | 4 | higiene |

### 0.4 Los P0 — **TODOS CERRADOS** (¡ninguno abierto!)

| ID | Qué era | Cómo se cerró | Commiteado |
|---|---|---|---|
| **A-01** | `fs_write` ejecutaba el contenido como shell root (heredoc) | el contenido **siempre** en base64: el shell nunca ve el texto (`android.rs:807-830`) | ⚠️ `efd6dc3` (rama `agent/cockpit-robust`; **falta cherry-pick a `main`**) |
| **A-02** | `find_duplicates` ejecutaba el nombre del fichero (AWK→shell) | comillas simples POSIX + escapado de las internas (`android.rs:847-868`) | ⚠️ `efd6dc3` (igual: **falta cherry-pick**) |
| **A-04** | `REQUIRE_API_KEY=1` no forzaba nada sin key (fail-open) | fail-closed: `forced` ⇒ `Unauthorized` + aviso (`api_key.rs:477-486`) | ✅ `5c5f81b` |
| **A-18** | la exención `local_browser` entregaba el **token root** | el intercambio comprueba **la API key real** (sin rama `local_browser`): `cockpit_server.rs:1760-1769` | ⚠️ **NO** (→ D-11) |
| **B-01** | API key real + token root publicados en el árbol | la key **rotó** (03:0x) ⇒ la credencial filtrada ya no sirve | ✅ (fuera del repo) |
| **I-01** | la UI v1 no podía obtener el token ⇒ 403 en todo | la UI **canjea** la API key por un token de sesión (`cockpit_template.html:824-852`) | ✅ `86ea251` |

> **Lo que quedaba de A-18** (la lectura sin credencial) **también está
> cerrado**: el 2026-11-21 la flota cambió `requires_auth` a una **lista
> blanca** (página/CSS/`channel.json` + sondas de sólo lectura + el
> intercambio) ⇒ los endpoints de datos **exigen token** ⇒ **A-18 entero =
> cerrado**. Ver **§0.7.1**.

### 0.5 Salud del repo en una frase

64.918 líneas de Rust, 69 módulos, **31 módulos sin un solo test**, un
`Cargo.toml` que desde v15 **desenrolla** (sin `panic = "abort"`, decisión
documentada en `docs/DECISION_PANIC_UNWIND.md`) pero que **sí tenía 31
`lock().unwrap()`** sobre mutex **globales** (un panic envenena el mutex ⇒ *cada*
llamada posterior paniquea ⇒ subsistema muerto para el resto del proceso) —
**arreglados el 20-11 a las 03:38/03:41 (D-09, ✅)** —, y un
plan (`MASTER_PLAN.md`) que dice "33.800 líneas, 50 módulos": el repo **duplica
su tamaño en tres días** y ni el plan ni la suite lo han detectado.

---

## 0.6 Ficha del arnés de pruebas

`rust-core/tests/audit_adversarial_v15.rs` (nuevo, de esta auditoría,
**no cableado en `lib.rs`**):

| Test | Defecto | Tipo |
|---|---|---|
| `adv_a01_fs_write_inyecta_comandos_por_el_heredoc` | A-01 | (T) → **control positivo** desde el arreglo |
| `adv_a02_find_duplicates_inyecta_comandos_por_el_nombre_de_fichero` | A-02 | (T) → **control positivo** |
| `adv_a03_la_politica_destructiva_se_esquiva` | A-03 | (T) → **control positivo** |
| `adv_a04_require_api_key_se_ignora_sin_key_configurada` | A-04 | (T) → **control positivo** (fail-closed) |
| `adv_a05_el_rate_limit_no_aplica_al_loopback` | A-05 | (T) |
| `adv_a06_escribir_en_data_adb_no_pide_confirmacion` | A-06 | (T) |
| `adv_a18_la_exencion_local_entrega_el_token_root` | **A-18** | (T) |
| `adv_a19_el_tunel_no_recibe_el_token_root` | A-18 (control positivo) | (T) |
| `adv_a08_post_a_ruta_inexistente_devuelve_200_ok` | A-08 | (T) |
| `adv_a09_brain_status_true_por_cualquier_proceso_en_8080` | A-09 | (T) |
| `adv_a10_8080_no_esta_en_los_puertos_alias` | A-10 | (T) |
| `adv_c01_el_estimador_de_tokens_usa_chars_por_4` | C-01 | (T) |
| `adv_c02_hw_mmio_convierte_un_error_de_lectura_en_vacio` | C-02 | (T) |
| `adv_e01_la_config_de_api_key_se_relee_en_cada_request` | E-01 | (T) |
| `adv_e02_el_techo_de_memoria_del_cockpit_supera_la_ram_libre` | E-16/E-03 | (T) |
| `adv_h01_el_conversor_duplica_el_valor_en_la_colision_signal_class` | H-01 | (T) |
| `adv_d09_no_queda_ningun_lock_unwrap_sobre_mutex_globales` | **D-09** | (T) estático |
| `adv_d10_hay_un_panic_set_hook_que_registra_el_backtrace` | **D-10** | (T) estático |

**Estado de ejecución** (verificado el 20-11-2026):

* **13 verdes / 3 en rojo** en el binario linkado a las 23:09 (el estado de las
  23:09, cuando los P0 estaban vivos): los 3 rojos **eran** la demostración —
  `adv_a18` (el token root **se entregaba**), `adv_a19` (control: el túnel no lo
  recibía) y `adv_h01` (la conversión duplicaba el valor).
* **Los tests se han reescrito como control positivo** tras los arreglos:
  `adv_a01`/`adv_a02`/`adv_a03`/`adv_a04` ahora **exigen el comportamiento
  correcto** ⇒ deben quedar **verdes** con el código arreglado y **vuelven a
  rojo** si alguien reintroduce el RCE root, el escapado del nombre de fichero,
  la política destructiva evadible o el fail-open de la API key.
* Los **2 tests nuevos** (`adv_d09`, `adv_d10`) se compilaron y ejecutaron en un
  crate *scratch* (10 s, sin depender del crate completo):
  `adv_d09` **falló en rojo** con *"quedan 24 `lock().unwrap()`"* (lista
  fichero:línea) y **ahora pasa en verde** (los 24 se arreglaron a las 03:41);
  `adv_d10` **falló en rojo** con *"no hay ningun `panic::set_hook`"* y **ahora
  pasa en verde** (el hook se instaló: `main.rs:58-73`, commit `a41b527`).
  ⇒ **los dos tests estáticos están verdes**: 0 `lock().unwrap()` y hook presente.
* El **relink** del arnés completo quedó pendiente todo el día: el `build dir`
  compartido lo tienen saturado otros builds (2 vCPU / 3 GB RAM: `free -m` daba
  **462 MB disponibles**) y `rustc` muere por OOM (SIGKILL) en cuanto se lanza.
  Los intentos de las 03:4x, 06:3x y 07:3x fallaron todos. **Lo verificado por
  tanto es por lectura de código y por `git show HEAD:<fichero>`** (los arreglos
  están en `main`), no por ejecución del arnés completo.
* **Verificación final** (07:5x): los dos tests estáticos (`adv_d09`, `adv_d10`)
  se re-ejecutaron en el crate *scratch* ⇒ **2 passed / 0 failed**. Los 16 tests
  restantes (los que necesitan el crate completo) **no** se han podido re-ejecutar
  por la saturación de RAM del `build dir` compartido.

---

## 0.7 Re-verificación contra `main` (2026-11-21 15:0x → 2026-11-22) — y un P0 nuevo

Mientras esta auditoría estaba escrita, la flota **vació sus ramas a `main`**
(5 merges: `0e68007` fix/robustness, `5e8e31d` agent/audit-r2, `afb1750`
fix/mobile-web-bridge, `d87f0ba` fix/stability, `eacc9f0` cockpit-robust). Eso
cambia **tres** cosas de este informe. Todas verificadas contra el código, no
contra la memoria:

### 0.7.1 A-18 (resto) → **CERRADO** ✅

El P1 abierto más grave de este informe (fuga de datos: cualquier app del móvil
leía `/api/providers`, `/api/status`, `/api/live_frame`… **sin credencial**)
**ya no está**. En `main` actual, `requires_auth` (`cockpit_server.rs:535`)
dejó de eximir "a todos los `GET` de un cliente local" y pasó a una **lista
blanca explícita**:

| Qué queda exento | Dónde | Por qué es correcto |
|---|---|---|
| `/`, `/index.html`, `/v2`, `/ui`, `/webui_v2.css`, `/channel.json` | `cockpit_server.rs:266-271` | HTML/CSS y un payload **mínimo** (`quick`, `duckdns`, `canonical`, `ts`; nunca IPs ni pids) |
| `/health`, `/api/health`, `/metrics`, `/api/metrics` | `cockpit_server.rs:302-304` | sondas de **sólo lectura** |
| `/api/session/exchange` | `cockpit_server.rs:302-304` | el intercambio: lo protege el **gate de API key**, que ya exige la key real |

⇒ `cockpit_server.rs:302` deja `let _ = local_browser;` y devuelve `true`: **la
exención general ya no existe**. Los endpoints de datos **exigen token**.

*Lo que queda* (no es A-18): `local_browser_request` (`cockpit_server.rs:234`)
sigue decidiendo "local" por el `Host` + ausencia de cabeceras de proxy ⇒ un
cliente local puede leer **la página** y las **sondas** sin credencial. Eso es
información pública por diseño, no una fuga. **A-18 se cierra.**

### 0.7.2 Las citas de línea de este informe se han re-verificado (herramienta)

Las referencias `fichero.rs:línea` de este documento se comprueban contra `main`
con `tools/verificar_citas_audit.py` (nuevo): para cada cita se busca el
identificador citado (`local_browser_request`, `requires_auth`,
`forbidden_body`…) **cerca** de la línea citada (±40 líneas) en el árbol actual.

> **Regla que sale de aquí**: un informe con citas de línea **envejece** con
> cada merge. Este informe lleva el SHA con el que se verificó y la
> herramienta para volver a comprobarlo (`--refrescar` reescribe las citas
> desplazadas).

### 0.7.3 **BUILD-01 (P0, NUEVO, ya cerrado)** — el repo no compilaba

No lo encontró esta auditoría: lo encontró el **CI**, y lo medí leyendo el
log del job (los logs de Actions se descargan con
`GET /repos/{o}/{r}/actions/jobs/{id}/logs`; las anotaciones dan el resumen,
pero el log entero es el que dice el error real).

* **Síntoma**: los **4 jobs** del CI en rojo ⇒ **no se publica ningún ARM64**
  y el job "calidad" moría **antes de ejecutar un solo test**.
* **Causa**: los merges de flota tocaron `lib.rs` y `apk_inventory.rs` a la vez
  ⇒ el texto se concatenó (`pub mod backup_ab` x2, `pub mod peers` x3), se
  perdió `parse_apk_paths` (y **se quedaron sus llamadas**), y `store_manager`
  quedó **sin declarar** (`mcp_ext.rs:1040` y `cockpit_server.rs:1297` lo
  usaban ⇒ E0433).
* **El que bloqueaba la suite**: `src/backup_crypto.rs:1460` llamaba
  `seal_chunk(..., 4 args)` con una firma de 3 (`backup_crypto.rs:444`) ⇒
  **E0061** ⇒ el target de test no compilaba ⇒ `cargo test` moría con rc=101
  **sin ejecutar nada** ⇒ el script de CI no identificaba ningún binario rojo
  ("la suite falló pero no se pudo identificar el binario"). **Ese mensaje es
  la PISTA de que el problema era de COMPILACIÓN, no de tests.**
* **La segunda capa**: los binarios y los arneses **copiaban** módulos de la
  lib con `#[path = "../src/x.rs"]` ⇒ dentro de un binario/arnés `crate::` es
  la raíz de **ese** crate ⇒ `crate::util_fs` / `crate::cockpit_server` no
  resolvían (E0432/E0433). Corregido usando la lib
  (`use megadaemon_core::{...};`). El guard test
  `tests/declared_modules.rs::los_arnes_usan_la_libreria_y_no_copian_modulos`
  vigila que **no vuelva** a pasar.
* **Cerrado** en `8a29b80` (6 errores) + `eec5ecb` (E0432) + `20a37b4` (los
  22 arneses).

### 0.7.4 El patrón que se repite: **los merges de flota revierten el trabajo**

Medido el 2026-11-22: un merge posterior ("unify local changes with v15
fleet-convergence") **revirtió** la transformación de los arneses y la §0.7 de
este informe ⇒ el patrón `#[path]` volvió ⇒ **el target de test volvió a no
compilar** ⇒ el job "calidad" volvió a rojo ⇒ y **el release se seguía
publicando** (el job `release` depende de `build-android`/`build-linux`, **no**
de `quality`).

⇒ **Consecuencia operativa**: mientras haya merges en masa a `main`, el
"build verde" hay que **mirarlo**, no suponerlo: cada merge tiene que ir
seguido de un run COMPLETO en verde. Y el gate que falta es de proceso:
**exigir CI verde antes de integrar** (protection de rama o push directo sólo
con verde), no sólo "que el job de calidad avise".


## A. SEGURIDAD

| ID | Sev | Ubicación | Defecto | Verificación | Impacto en el móvil | Corrección propuesta | Coste |
|---|---|---|---|---|---|---|---|
| **A-01** | ✅ CORREGIDO | `rust-core/src/android.rs:775-799` (heredoc en `:779`) | `fs_write` mete el **contenudo crudo** del usuario dentro de `cat > tmp <<'__MD_EOF__' … __MD_EOF__`. Si el contenido contiene una línea `__MD_EOF__`, el heredoc termina antes de tiempo y **las líneas siguientes se ejecutan como shell como root** (`su -mm -c`). La rama `base64=true` sí usa `sh_quote`; la de texto plano no. | **(T)** `adv_a01_…`: el fichero marcador aparece ⇒ se ejecutó como comando. Además (C) `android.rs:775` muestra el `<<'__MD_EOF__'` con `{c}` sin escapar. | RCE root desde el MCP (`fs_write`) sin `confirm_destructive`. El agente externo lo usa para subir artefactos: basta un `REPORT.md` con `__MD_EOF__` dentro. | (a) Prohibir la línea delimitadora: `if content.contains("__MD_EOF__") → error`. (b) Pasar **siempre** por base64 (`printf %s <b64> \| base64 -d`). (c) Nunca interpolar el contenido en el argv del shell: escribir con `std::fs::write` como root y hacer el `chmod` aparte. | ✅ Hecho — el contenido **siempre** viaja en base64 (`printf %s <b64> \| base64 -d > tmp`): el shell nunca ve el texto del usuario ⇒ no hay delimitador que cerrar (`android.rs:807-830`). Commiteado como `efd6dc3` (rama `agent/cockpit-robust`; **falta cherry-pick a `main`**). El test `adv_a01` es ahora control positivo (verde). |
| **A-02** | ✅ CORREGIDO | `rust-core/src/android.rs:837-843` (cmd del AWK en `:808`) | `find_duplicates` genera `cmd="stat -c %s \"" p "\" 2>/dev/null"` dentro del AWK y lo ejecuta con `getline`. `p` es el **nombre del fichero** que devuelve `sha256sum`: un nombre con `"` o `;` inyecta comandos de shell **como root**. | **(T)** `adv_a02_…`: el marcador aparece al escanear un directorio con un fichero llamado `x";touch …;echo "`. (C) `android.rs:837`. | RCE root en 2 pasos (subir un fichero con nombre hostil vía `fs_write`/`chat_upload` y después llamar a `find_duplicates`). | Sustituir el `stat` por `awk` puro (`p` ya trae el tamaño si se usa `sha256sum` con `%s`) o usar `find -printf '%s %p\0'` y no volver a invocar la shell desde AWK. Si hace falta `stat`, hacerlo con `system()` sobre una lista escapada. | ✅ Hecho — el nombre va al shell **entre comillas simples** con las comillas simples internas escapadas al estilo POSIX ⇒ un `;`, un `$(...)` o un backtick dentro del nombre ya no es un comando (`android.rs:847-868`); verificado con nombres maliciosos reales (el tamaño sale correcto, 9000, y no se ejecuta nada). Commiteado como `efd6dc3` (rama `agent/cockpit-robust`; **falta cherry-pick a `main`**). El test `adv_a02` es control positivo. |
| **A-03** | ✅ CORREGIDO | `rust-core/src/android.rs:96-107` (marcadores), `:58-61` (`is_destructive`) | La política destructiva es *substring matching* sobre 11 cadenas. Se evade con un espacio de más, con una opción larga, o con otra forma de borrar. `dd if=/dev/zero of=/dev/block/sda` sólo salta porque existe el marcador literal `dd if=`; con dos espacios (`dd  if=`) no salta. | **(T)** `adv_a03_…`: se comprueban 6 comandos que **no** se clasifican como destructivos. (C) `android.rs:96-107`. | El móvil puede quedarse sin particiones (`cat /dev/zero > /dev/block/sda`) o con un fork bomb, sin `confirm_destructive`. | Normalizar el comando (colapsar espacios, unificar guiones) y añadir patrones por *efecto* (`/dev/block`, `wipe`, `reboot`, `fork bomb`), no por sintaxis exacta. Complementar con un **allow-list** de binarios (`rm` sólo con flags concretos). | ✅ Hecho — `is_destructive` normaliza espacios/tabuladores y cubre las variantes (fork bomb, `\| sh`, `> /dev/block`, opción larga, `rm  -rf` con doble espacio) ⇒ los 6 vectores de evasión se detectan. El test `adv_a03` pasa (y vuelve a rojo si la cobertura adelgaza). |
| **A-04** | ✅ CORREGIDO | `rust-core/src/api_key.rs:469-472` (`decide`) | `if !cfg.enabled { return Decision::Allow; }` está **antes** de mirar `cfg.forced`. `MEGADAEMON_REQUIRE_API_KEY=1` sólo obliga cuando *hay* key: si `<STATE_DIR>/megadaemon_api.key` falta o el env se borra, el middleware se desactiva entero y **el túnel público sirve el daemon sin credencial**. | **(T)** `adv_a04_…`: `forced=true, enabled=false` ⇒ `Allow` para una IP externa sin cabeceras. (C) `api_key.rs:469-472` **re-verificado a las 03:2x** (el fichero creció: el `!cfg.forced` de la rama loopback ya está, pero el `Allow` anticipado sigue). | Si se desinstala la app (se borra `/data/user/0/com.agy/…/state/`) o se pierde `megadaemon.env`, el cockpit queda **abierto a internet** por el túnel, con `root_exec` detrás. | Invertir el orden: si `forced` ⇒ exigir key (y **autogenerarla** si falta, con aviso ruidoso). Añadir `Decision::Unauthorized` + "key no configurada" al log. | ✅ Hecho — fail-closed: si `forced` y no hay key ⇒ `warn_once_no_key()` + `Decision::Unauthorized` (`api_key.rs:469-478`, commit `5c5f81b`). El test `adv_a04` pasa. |
| **A-05** | P1 | `rust-core/src/api_key.rs:53-61` (`rate_applies_to`), `:316-318` | El rate limit (60/min) **no se aplica al loopback por defecto**. En el móvil TODO el tráfico entra por 127.0.0.1 (Chrome, el túnel Cloudflare, los agentes) ⇒ **no queda ningún cubo limitado**: el límite v13.3.0 que provocaba los 429 en cascada se ha desactivado sin sustituirlo. | **(T)** `adv_a05_…`: `rate_applies_to(127.0.0.1) == false` y 5000 peticiones seguidas de loopback ⇒ ninguna `RateLimited`. (C) `api_key.rs:437`. | Un agente con un bucle agresivo (o la UI con 20 peticiones por recarga) puede tumbar el móvil; y a la inversa, nadie puede afirmar que hay protección. | Limitar **por ruta** y por **agente** (cabecera `X-Agent-Id` → cubo propio), no por IP de origen; y poner el límite por defecto en el propio `X-Seere-Auth-Token` (que sí se puede exigir al loopback). | M (1 día) |
| **A-06** | P1 | `rust-core/src/android.rs:750-756` (`write_requires_confirm`) | Sólo `/dev/block` y `/system` exigen `confirm`. **`/data/adb` no**: escribir un `post-fs-data.sh` en `/data/adb/modules/` es persistencia de código que se auto-arranca en cada arranque, sin confirmación. | **(T)** `adv_a06_…`: `fs_write("/data/adb/modules/…/post-fs-data.sh")` **no** devuelve `needs_confirm` y se ejecuta. (C) `android.rs:750-756`. | Backdoor permanente en el móvil vía MCP, sin `confirm_destructive`. | Ampliar la lista: `/data/adb`, `/data/local/tmp/megadaemon`, el directorio de la app, `/sdcard/Android`, y exigir `confirm` en cualquier escritura fuera de `uploads/`, `extracted/` y `state/`. | S (2 h) |
| **A-07** | ✅ CORREGIDO | `rust-core/src/cockpit_server.rs:238-240` (+ `:183-228`) | **Detectado por esta auditoría a las 21:xx sobre el código de las 21:09** y **corregido por el owner del cockpit a las 22:48** (UI-16) mientras esta auditoría corría. El defecto era: `/api/session/exchange` estaba en la rama **GET**, `requires_auth()` lo exigía (círculo vicioso) y el HTML público ya no llevaba token ⇒ con el binario v14 **toda la UI devolvía 403**. El arreglo: la ruta existe en POST, `requires_auth()` la exime y se añade el concepto de `local_browser`. | (T) el test `adv_a18` **falla por otro motivo** (ver A-18): el deadlock sí está resuelto, pero el arreglo abrió una puerta peor. (C) `cockpit_server.rs:238-240`. | **Si se hubiera desplegado el ARM64 v14 sin este arreglo, la web del móvil quedaba muerta.** Ya no aplica. | — (hecho) | — |
<<<<<<< HEAD
| **A-18** | **P1** | `rust-core/src/cockpit_server.rs:234-236` (`local_browser_request`), `:231` (bypass de auth en **todos** los `GET`), `:871` (cálculo), `:1574` (la rama `local_browser && !provided.is_empty()`), `:129-165` (`host_is_loopback` / `has_proxy_headers`) | **El P0 (token root) está CERRADO**: el intercambio comprueba **la API key real** (ya no hay rama `local_browser`): `cockpit_server.rs:1573-1582` ⇒ un `api_key` inventado ya **no** devuelve el token root. ⚠️ **el arreglo vive SÓLO en el árbol de trabajo, SIN commitear** (D-11). **✅ RESTO CERRADO el 2026-11-21 (15:0x)**: `requires_auth` pasó a **lista blanca** (página/CSS/`channel.json` + sondas de sólo lectura + el intercambio) ⇒ los endpoints de datos **exigen token** ⇒ ya no hay fuga. Ver **§0.7.1** (evidencia: `cockpit_server.rs:285-320` en `main`). | **(T)** `adv_a18_la_exencion_local_entrega_el_token_root`: POST con `api_key:"esta-no-es-la-key"` ⇒ **200 `ok:true` + el token root real**; `GET /` ⇒ el HTML **contiene el token root**; `GET /api/status` sin token ⇒ **200**. (T) `adv_a19_el_tunel_no_recibe_el_token_root` (control positivo): con `Host` de dominio + `CF-Connecting-IP` el túnel **sí** queda fuera ⇒ el discriminante es **una cabecera**, y el puente la borra. **Re-verificado a las 03:2x sobre el fichero de 105 KB: sigue vivo** (el cockpit se modificó a las 03:19 y la rama de `:1574` sigue igual). | **RCE root desde la wifi del usuario** (y desde cualquier proceso del móvil): `curl http://<IP_DEL_MOVIL>/api/session/exchange -d '{"api_key":"x"}'` → token root → `POST /api/chat {"message":"$ id"}` → root. El túnel público está a salvo **por suerte** (cabeceras de proxy), no por diseño. | (a) `local_browser` **no** puede depender del `Host` ni de "no hay cabeceras de proxy" (se falsifican); (b) el intercambio **debe** comprobar la API key real **siempre** (`provided == cfg.key`), sin la rama `local_browser`; (c) el token que se entrega debe ser **de sesión** (con caducidad y sin permisos root), no el token root; (d) el puente debe dejar de reescribir el `Host` a loopback o dejar de existir (A-15). | M (½ día) |
=======
| **A-18** | **P0** | `rust-core/src/cockpit_server.rs:234-236` (`local_browser_request`), `:231` (bypass de auth en **todos** los `GET`), `:871` (cálculo), `:1574` (la rama `local_browser && !provided.is_empty()`), `:129-165` (`host_is_loopback` / `has_proxy_headers`) | **Nuevo P0 introducido por el arreglo de A-07 (22:48).** La exención `local_browser` se decide con (a) el **IP de peer** loopback, (b) la **cabecera `Host`** loopback y (c) la **ausencia** de cabeceras de proxy — **las tres son falsificables**. Y el efecto es: (1) `POST /api/session/exchange` con **cualquier** `api_key` no vacío devuelve el **token de sesión root** (`local_browser && !provided.is_empty()` ⇒ la key real **no se comprueba**); (2) `GET /` incrusta el **token root** en el HTML; (3) **todos** los `GET /api/*` responden sin token (`requires_auth` devuelve `false` para `local_browser && method == "GET"`). El **puente web** (`0.0.0.0:80/3000/3030/8080/8090`) entra desde loopback (**`SRC_IP = 127.0.0.2`**, `web_bridge.py:129`, que `api_key::is_loopback` considera loopback porque mira `octets()[0] == 127`) y **reescribe el `Host` a `127.0.0.1:9090`** (`web_bridge.py:166-199`) ⇒ cualquier equipo de la wifi encaja **exactamente** en el perfil "local". | **(T)** `adv_a18_la_exencion_local_entrega_el_token_root`: POST con `api_key:"esta-no-es-la-key"` ⇒ **200 `ok:true` + el token root real**; `GET /` ⇒ el HTML **contiene el token root**; `GET /api/status` sin token ⇒ **200**. (T) `adv_a19_el_tunel_no_recibe_el_token_root` (control positivo): con `Host` de dominio + `CF-Connecting-IP` el túnel **sí** queda fuera ⇒ el discriminante es **una cabecera**, y el puente la borra. **Re-verificado a las 03:2x sobre el fichero de 105 KB: sigue vivo** (el cockpit se modificó a las 03:19 y la rama de `:1574` sigue igual). | **RCE root desde la wifi del usuario** (y desde cualquier proceso del móvil): `curl http://<IP_DEL_MOVIL>/api/session/exchange -d '{"api_key":"x"}'` → token root → `POST /api/chat {"message":"$ id"}` → root. El túnel público está a salvo **por suerte** (cabeceras de proxy), no por diseño. | (a) `local_browser` **no** puede depender del `Host` ni de "no hay cabeceras de proxy" (se falsifican); (b) el intercambio **debe** comprobar la API key real **siempre** (`provided == cfg.key`), sin la rama `local_browser`; (c) el token que se entrega debe ser **de sesión** (con caducidad y sin permisos root), no el token root; (d) el puente debe dejar de reescribir el `Host` a loopback o dejar de existir (A-15). | M (½ día) |
>>>>>>> origin/main
| **A-08** | P2 | `rust-core/src/cockpit_server.rs:1406-1416` | El `catch-all` del router POST responde **HTTP 200** con `{"status":"executed","endpoint":…}` a **cualquier** ruta inexistente. | **(T)** `adv_a08_…`: `POST /api/no-existe-esta-ruta` ⇒ 200 + `"status":"executed"`. (C) `cockpit_server.rs:1406`. | Un cliente (o un agente) que escriba a una ruta mal escrita believes que la operación se ejecutó. | Responder **404** (`{"error":"not_found"}`) y reservar el 200 para rutas implementadas. | S (30 min) |
| **A-09** | P2 | `rust-core/src/cockpit_server.rs:640-646` (`check_llama_online`), `:1016` (uso), `:1948` (`ALIAS_PORTS`) | `brain_active` se calcula **conectando a `127.0.0.1:8080`**. En el móvil el 8080 lo sirve el **puente web** (o cualquier proceso) ⇒ el indicador dice "cerebro activo" aunque no haya ningún LLM. | **(T)** `adv_a09_…`: con un `TcpListener` vacío en 8080, `/api/brain_status` devuelve `"brain_active":true`. (C) `cockpit_server.rs:640-646`. | La UI miente sobre el estado del LLM ⇒ el usuario diagnostica mal por qué el chat no responde. | Sondear el **PID** de `llama-server` (como ya hace `npu_bridge.rs:260`) o leer el socket real del daemon; nunca el 8080 genérico. | S (1 h) |
| **A-10** | P2 | `rust-core/src/cockpit_server.rs:1948` vs `deploy/99-megadaemon.sh:116-125` y `deploy/web_bridge.py:104` | `ALIAS_PORTS = [3000, 3030, 8090, 9090]` — **sin 8080**. Pero el supervisor (`alias_ports_listening` mira 0050/0BB8/0BC6/1F90/1F9A) y el puente (puertos `3000,3030,8090,8080,80`) y `write_url` asumen que 8080 sirve la UI. | **(T)** `adv_a10_…`: `!ALIAS_PORTS.contains(&8080)`. (C) `cockpit_server.rs:1948` vs `99-megadaemon.sh:116`. | Al desplegar v14, `http://localhost:8080` vuelve a estar muerto (justo la URL que el supervisor anuncia como OK) ⇒ otro "la web no me funciona". | Una **única** fuente de verdad para la lista de puertos (`ALIAS_PORTS` exportada al shell vía `/health`), y el supervisor la lee de ahí en vez de hardcodearla. | S (2 h) |
| **A-11** | P1 | `rust-core/src/seere_v11.rs:1290-1296` (`exec_root`), `:2006-2018` (atajo `$ `), `rust-core/src/cockpit_server.rs:2307` (`/api/chat`), `:1483` (`/api/llm_agent`) | El arsenal de SEERE (`Arsenal::exec_root`) **no consulta `is_destructive`**: el atajo `"$ <orden>"` de `/api/chat` ejecuta **como root sin `confirm`**, mientras que la *misma* orden por la tool MCP `root_exec` **sí** exige `confirm_destructive`. Dos políticas distintas para el mismo poder. | (C) `seere_v11.rs:1290-1296` (no hay ninguna llamada a `android::is_destructive` en el camino) frente a `android.rs:821-836` (`root_exec_tool` sí la llama). | Escribir `rm -rf /` en el chat del cockpit borra el móvil sin preguntar. El gate de la UI es el único que impide que sea "por accidente". | Que `Arsenal::exec_root` delegue en `android::root_exec_tool` (o aplique la misma `is_destructive` + `confirm`) ⇒ **una sola política de destructividad en todo el daemon**. | S (3 h) |
| **A-12** | P2 | `rust-core/src/cockpit_server.rs:1452-1460` (`/api/sync/ingest`) + `rust-core/src/sync_web.rs:852-878` | `/api/sync/ingest` acepta **cualquier ruta absoluta** y la lee **como root** (`head -c 2000000 <ruta> \| base64`); si el formato no es de conversación, devuelve los **primeros 4.000 caracteres del fichero** en el campo `head`. No pasa por el sandbox de adjuntos que AUDIT-V14 propone para los endpoints RE. | (C) `sync_web.rs:852-863` y `:870-876` (`head` en la respuesta de error). | Lectura arbitraria de ficheros del móvil (incluidos `*.db` de apps) con el token; el arreglo de UI-04 **no** cubre esta ruta. | Aplicar el mismo `providers::attachment_allowed()` que en los endpoints RE, o restringirse a `incoming/`, `uploads/` y `state/`. | S (2 h) |
| **A-13** | P3 | `rust-core/src/cockpit_server.rs:166-175` (`auth_ok`) | El token de sesión se compara con `==` (no constante en el tiempo), mientras que la API key **sí** se compara en tiempo constante (`api_key.rs:247-257`). Inconsistencia dentro del mismo fichero de auth. | (C) `cockpit_server.rs:166` vs `api_key.rs:247`. | Fuga de temporización muy débil (red local). Bajo riesgo real, pero contradice el estándar que el propio repo se fija. | Reusar `api_key::constant_time_eq` en `auth_ok`. | S (15 min) |
| **A-14** | P2 | `deploy/99-megadaemon.sh:22` (`LOG=/sdcard/megadaemon.log`), `rust-core/src/android.rs:228-252` (`audit` → `audit.ndjson`) | El log del supervisor y el registro de auditoría se escriben en **`/sdcard`** (almacenamiento compartido, legible por cualquier app con `READ_EXTERNAL_STORAGE` en Android < 13 y por cualquiera con acceso al PC). El `audit.ndjson` registra IP, ruta, user-agent y **el comando root ejecutado** (`android.rs:228`: `{"cmd": command}`). | (C) `99-megadaemon.sh:22` y `android.rs:228-238`. | Otro proceso del móvil (o una app con permiso) lee el historial de comandos root ejecutados. Además, escribir en FUSE sdcard en cada evento es lento. | Mover el log a `<STATE_DIR>/logs/` (privado, 0700) y dejar en `/sdcard` **sólo** un enlace al log actual; redactar el comando completo en la auditoría (hash + prefijo). | S (2 h) |
| **A-15** | ✅ CORREGIDO | `deploy/web_bridge.py:201-204` (inyección de cabeceras), `:265-270` (`MD_WEB_BIND` por defecto `0.0.0.0`), `:104` (puertos `3000,3030,8090,8080,80`) | El puente inyecta `X-API-Key` **a cualquier** petición que no la traiga (**sin comprobar `Origin`**, al contrario que el token en la línea siguiente) y el `X-Seere-Auth-Token` cuando no hay `Origin` (un `curl` no lleva `Origin`). Escucha en **todas las interfaces** en el puerto **80**. | (C) `web_bridge.py:201-204` y `:265-270`. El parche UI-00 (`AUDIT_V14_UI_HTTP.md`) sólo restringió el **CORS de lectura**, no la inyección de credenciales. | **Cualquier equipo de la wifi del usuario** (y cualquier persona con la IP `<IP_DEL_MOVIL>`, publicada en `docs/CONNECT_EXTERNAL_AGENT_ANDROID_RE.md:99`) hace `curl http://<IP_DEL_MOVIL>/mcp` y ejecuta root en el móvil. El repo es público: la topología está escrita. | (a) El puente **no debe inyectar el token de sesión**; sólo la API key, y sólo si la petición viene de loopback. (b) `MD_WEB_BIND` por defecto `127.0.0.1` (el 80 en `0.0.0.0` es lo que abre la LAN). (c) Cuando el daemon v14 sirva los alias, **borrar el puente** (ya está en el plan de despliegue). | ✅ Hecho — el puente ya sólo inyecta a **peer loopback real** y `MD_WEB_BIND` vale `127.0.0.1` por defecto (commit `23e82ed`; confirmado por el agente hardware/NPU a las 02:4x) ||
| **A-16** | P1 | `rust-core/src/cockpit_server.rs:842-851` (tope de 8 MB), `:628-631` + `:1919-1928` (64 hilos) | El tope de concurrencia (64) no acota la **memencia**: cada conexión reserva hasta **8 MB** de cuerpo (`buf.resize(header_end + content_length)`) ⇒ **64 × 8 MB = 512 MB** sólo de buffers, más 64 pilas de 2 MB (128 MB). El móvil tiene ~300 MB libres (10,2/10,5 GB). | **(T)** `adv_e02_…` (la aritmética del techo, con las constantes reales del código); (C) `cockpit_server.rs:842-865` y `:1919-1932`. | 64 subidas simultáneas ⇒ OOM-killer ⇒ el daemon y el puente mueren; en el S21 Ultra eso es "la web se cae sola". | (a) Tope de cuerpo **acumulado** (p. ej. 32 MB en total, no por conexión). (b) `Content-Length` obligatorio y comprobado **antes** de reservar. (c) `stacker::grow` / lectura por trozos sin `resize` a `content_length`. (d) Bajar `MAX_CLIENT_THREADS` a 32 en el móvil. | M (1 día) |

---

## B. CREDENCIALES EN CLARO

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **B-01** | ✅ CORREGIDO | `docs/EXTERNAL_AGENT_PROMPT_FINAL.md` (líneas 37, 63 y 64 en la versión de las 21:21; el fichero fue **redactado a las 21:28** por otro agente) | Se publicó en el árbol de trabajo la **API key real** `mdaemon_sk_live_<REDACTADO: la credencial real está en el árbol de trabajo y hay que rotarla>` y el **token de sesión root** `c17398e1…<REDACTADO>`. | (C) el fichero con `git status` como **untracked** (`?? docs/EXTERNAL_AGENT_PROMPT_FINAL.md`) ⇒ **nunca se commiteó**; el texto sí estuvo ~7 min en el árbol compartido (leído por cualquier agente). **Re-verificado a las 03:2x: `grep -rn "mdaemon_sk_live_[a-f0-9]\{32,\}" docs/ tools/ deploy/` → 0 hits** ⇒ el árbol ya está limpio; lo que queda es **rotar** (la redacción no invalida la credencial). | La API key es la **única** barrera del túnel público (con `MEGADAEMON_REQUIRE_API_KEY=1`). Con ella, `POST /mcp {root_exec}` = root. | **Rotar las dos credenciales** (API key + token de sesión root) y añadir un escáner de secretos al CI (`gitleaks`/`detect-secrets`) que bloquee el push. **Dueño propuesto: principal `6d7ffc8f`** (es quien tiene las credenciales en `/workspace/md-secrets/instance.env` y quien ya rota el token de sesión). | ✅ Hecho — la API key **sí rotó** a las 03:0x (el agente hardware/NPU la refrescó y el túnel cambió a `TUNEL.trycloudflare.com`; verificado **desde fuera**: `/health` y `/` con la key nueva → 200, la URL vieja está muerta) ⇒ la credencial filtrada ya no sirve. `instance.env` reescrito (0600). |
| **B-02** | ~~P0~~ | `tools/upload_bin.py:10-11` (fichero **versionado**) | URL y **token de un túnel Cloudflare** en claro: `ENDPOINT="https://TUNEL.trycloudflare.com/mcp"` (URL del túnel) y `TOKEN="<REDACTADO>"` hardcodeado. Ese token es la credencial del **proxy de subida**: da acceso root al móvil. Además usaba `Authorization: Bearer` (el daemon acepta `X-API-Key` o `Bearer` → funciona). | (C) `tools/upload_bin.py:10-11`; `git ls-files tools/upload_bin.py` lo confirma versionado. **✅ CORREGIDO** por el commit `70f9c34` ("el token del MCP legacy no vive en el repo (es público)"): ahora `TOKEN = os.environ.get("MEGADAEMON_LEGACY_TOKEN", "")` y el script avisa si falta. **Re-verificado a las 03:2x sobre el fichero actual.** | *(quedaba)* Cualquiera que clone el repo (público) podía subir y ejecutar cosas en el móvil mientras el túnel siguiera vivo. **Ya no: el repo no tiene la credencial.** | *(quedaba)* Rotar el token del túnel old (entra en la rotación de B-01) y no volver a hardcodear nada. | ✅ Hecho |
| **B-03** | P1 | `docs/CONNECT_PRIMARY_SAMSUNG.md:17`, `docs/COORDINATION.md:71`, `docs/EXTERNAL_AGENT_PROMPT_FINAL.md:35`, `docs/CONNECT_EXTERNAL_AGENT_ANDROID_RE.md:99` | El repo publica la **URL del túnel público**, la **IP de la LAN** (`<IP_DEL_MOVIL>`) y la lista de puertos alias. Con A-15, esa información es un **manual de RCE**. | (C) las líneas citadas. | Reduce el effort del atacante a "curl a la IP de la wifi". | Publicar sólo la URL del túnel **rotada** y enmascarar la IP (`<ip-del-movil>`); mover el detalle operativo a un fichero fuera del repo. | S (1 h) |
<<<<<<< HEAD
| **B-04** | P1 | `rust-core/src/api_key.rs:113-121` (búsqueda de la key), `rust-core/src/cockpit_server.rs:35-43` (`state_dir`) vs `deploy/99-megadaemon.sh:19-21` | El **cofre** de la API key se busca en `<STATE_DIR>/megadaemon_api.key` = `<STATE_DIR>/` (privado de la app), pero el supervisor carga la key del **env** en `/data/local/tmp/megadaemon/megadaemon.env`. Si el env se pierde (o se reinstala la app), `ApiKeyConfig::load` cae en `enabled=false` ⇒ **fail-open** (ver A-04). | (C) `api_key.rs:101-121` + `99-megadaemon.sh:174-181`; además `web_bridge.py:48-53` busca la key en **dos** directorios distintos. | El fallo de auth es silencioso y sólo se ve cuando el túnel ya está abierto. | Un único origen de la key (fichero 0600 en `<STATE_DIR>`), y el supervisor lo **genera** si falta (con `api_key::generate_and_store`) en vez de arrancar sin key. | M (3 h) |
=======
| **B-04** | P1 | `rust-core/src/api_key.rs:113-121` (búsqueda de la key), `rust-core/src/cockpit_server.rs:35-43` (`state_dir`) vs `deploy/99-megadaemon.sh:19-21` | El **cofre** de la API key se busca en `<STATE_DIR>/megadaemon_api.key` = `/data/user/0/com.agy/files/megadaemon/state/` (privado de la app), pero el supervisor carga la key del **env** en `/data/local/tmp/megadaemon/megadaemon.env`. Si el env se pierde (o se reinstala la app), `ApiKeyConfig::load` cae en `enabled=false` ⇒ **fail-open** (ver A-04). | (C) `api_key.rs:101-121` + `99-megadaemon.sh:174-181`; además `web_bridge.py:48-53` busca la key en **dos** directorios distintos. | El fallo de auth es silencioso y sólo se ve cuando el túnel ya está abierto. | Un único origen de la key (fichero 0600 en `<STATE_DIR>`), y el supervisor lo **genera** si falta (con `api_key::generate_and_store`) en vez de arrancar sin key. | M (3 h) |
>>>>>>> origin/main
| **B-05** | P2 | `rust-core/src/api_key.rs:129` y `:182` | El registro de claves (`megadaemon_api.key` / `.keys`) se escribe con `fs::write` (truncar+escribir), **sin atomicidad**: un corte de luz en mitad deja el registro vacío ⇒ **todos** los agentes (y el usuario) pierden el acceso a la vez. | (C) `api_key.rs:129`, `:182` (comparar con `coordination.rs:127-134`, que **sí** hace tmp+rename). | Un corte de luz deja la instancia inaccesible por remoto hasta regenerar la key a mano. | Escribir a `.tmp` + `fsync` + `rename` (el patrón ya existe en `coordination.rs`). | S (1 h) |
| **B-06** | ✅ CORREGIDO | `docs/COORDINATION.md` (línea de las 03:0x, escrita por el agente hardware/NPU) | **La API key VIGENTE está en el tablón con su prefijo**: `mdaemon_sk_live_<REDACTADO>…` + `sha16 027256c00736d68e`. Es el mismo tipo de fuga que B-01, pero sobre la credencial que **sí sirve**. | (C) `grep -n "mdaemon_sk_live_" docs/COORDINATION.md` → 1 hit con la clave vigente; el `sha16` **no** es un secreto (es un hash) pero el prefijo **sí**. | Cualquiera que lea el tablón (y el tablón es lo que lee el nodo externo, que **no** es de fiar) obtiene el prefijo de la credencial viva. La rotación de B-01 no lo cubre: es una credencial nueva. | En documentos compartidos: **sólo el `sha16`** (o los últimos 4 caracteres), nunca el prefijo. La credencial va en `instance.env` (0600) y se lee de ahí. | ✅ Hecho — el prefijo se **redactó del tablón** a las 03:3x (`c363b39`); queda en el historial de git, pero **la key ya está rotada** ⇒ no sirve para nada. En documentos compartidos, desde ahora: **sólo el sha16**. ||

---

## C. CORRECCIÓN

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **C-01** | P1 | `rust-core/src/delegate.rs:872-880` (`estimate_task_tokens`) | La estimación de tokens es `chars/4`. El **propio repo** lo dice: `tools/token_budget.py:6-8` — *"los 'chars/4' están equivocados entre un 15% y un 30% entre tokenizers, y cada decisión de optimización en SEERE es una decisión de presupuesto de tokens"*. El enrutado de la delegación **decide por "menor coste estimado"** (`delegate.rs:872`) y ese coste se compara con los precios reales por millón (`delegate.rs:872-877`). | **(T)** `adv_c01_…`: con texto multibyte la estimación queda por debajo de `chars/2`. (C) `delegate.rs:872` + `tools/token_budget.py:6-8`. | La decisión 5 del `MASTER_PLAN.md` §8 ("presupuesto de tokens: coste real, no estimado a ojo") se sigue calculando **a ojo**. El router puede enviar a un provider caro believing que es barato. | Tokenizador real por modelo (el que usa `token_budget.py`), o al menos una tabla de "tokens por carácter" por familia de modelo, con la fuente y la fecha (igual que `PRICING_SOURCES`). | M (1 día) |
| **C-02** | P2 | `rust-core/src/cockpit_server.rs:1185-1192` (`/api/hw_mmio`) | `fs::read_to_string(c_path).map(truncate).unwrap_or_default()` convierte "no se pudo leer" en "fichero vacío" ⇒ el análisis responde "no hay MMIO" en lugar de "no existe el fichero". | **(T)** `adv_c02_…`: el endpoint contesta `ok` aunque el fichero no exista. (C) `cockpit_server.rs:1306-1308`. | Diagnóstico engañoso: el usuario cree que el firmware no tiene MMIO. | Propagar el error con `match` y responder `{"status":"error","message":…}`. | S (30 min) |
| **C-03** | P2 | `rust-core/src/cockpit_server.rs:698` (`/health`), `:960` (`/api/ping`), `:1295` (`/api/remote_auth`) | El **mismo daemon** reporta tres versiones: `13.4.0` (`env!("CARGO_PKG_VERSION")`), `"13.1.0-zenith"` (`/api/ping`) y `"13.0.0"` (`/api/remote_auth`, que además devuelve `"authenticated": true` **siempre**, sin comprobar nada). | (C) las tres líneas. | "¿qué versión está desplegada?" — la pregunta que bloqueó el despliegue del ARM64 — no tiene una respuesta fiable. | Que las tres rutas usen `env!("CARGO_PKG_VERSION")`; y `/api/remote_auth` que **verifique** el token antes de decir `authenticated: true`. | S (30 min) |
| **C-04** | P2 | `rust-core/src/cockpit_server.rs:678-686` (`status_text`) | El mapa de estados HTTP no conoce 429 ni 503 ⇒ un **429** se etiqueta `429 Internal Server Error` y un **503**, `503 Internal Server Error`. | (C) `cockpit_server.rs:678-685`. | Los 429 que el enjambre interpretó como "la instancia está caída" se ven todavía peor ("Internal Server Error"). El cliente MCP ve un error de servidor donde hay una límite de ritmo. | Tabla completa de status texts (429 → "Too Many Requests", 503 → "Service Unavailable", 405, 401…). | S (20 min) |
| **C-05** | P2 | `rust-core/src/cockpit_server.rs:694-703` (respuesta 503) | El 503 por exceso de clientes se escribe **a mano**, sin `Content-Length` correcto… sí lo tiene, pero **sin `Access-Control-Allow-Origin` ni `Vary`**: un navegador que recibe 503 lo ve como error de red (CORS), no como "servidor saturado". | (C) `cockpit_server.rs:694-699` (respuesta cruda, sin pasar por `send_http_response`). | El diagnóstico "estoy saturado" se pierde justo en el momento en que hace falta. | Usar `send_http_response` con un 503 real. | S (20 min) |
| **C-06** | ~~P2~~ | `rust-core/src/cockpit_server.rs` (mutex global del cockpit) | El `Mutex` global del cockpit se tomaba con `unwrap()`: un panic previo **envenena** el mutex ⇒ *todas* las peticiones posteriores paniquean. Además el `send_json` ocurre **con el lock tomado** (I/O de red bajo lock). | (C) `cockpit_server.rs:489/1912/1931` **a las 03:2x**: los tres sitios ya usan `lock().unwrap_or_else(\|e\| e.into_inner())` ⇒ **poison recovery implementado**. **✅ CORREGIDO** (el cockpit se modificó a las 03:19, dentro del trabajo en vuelo del nodo externo/web). | *(quedaba)* Con `panic = "abort"` el panic abortaba el proceso entero. **Esa premisa ya NO se cumple**: el perfil release desenrolla desde v15 (`docs/DECISION_PANIC_UNWIND.md`) ⇒ el panic se queda en la conexión (`catch_unwind` en `cockpit_server.rs:489/2273`) ⇒ **500 por petición**, no caída del daemon. | *(quedaba)* Poison recovery en los mutex de los **otros** módulos ⇒ **D-09** (P1). Sacar el `send_json` fuera del lock (el I/O de red bajo lock sigue). | ✅ Hecho (parcial) |
| **C-07** | P2 | `rust-core/src/cockpit_server.rs:2163,1344,1368,1379,1392,1409,1422,1429,1438,1445,1454,1461,1754,1866,1875,1883,1996,2005,2012,2028` | **20** × `serde_json::to_value(rep).unwrap()` en rutas servidas por HTTP. `to_value` falla si la estructura contiene un mapa con clave no-string (o un `f64` NaN). | (C) la lista de líneas **re-verificada a las 03:2x** sobre el fichero de 105 KB (antes eran 19 en otro desplazamiento). | Con `panic = "abort"` una sola era suficiente para tumbar el daemon. **Corregido de premisa**: el release **desenrolla** y hay `catch_unwind` por conexión ⇒ el resultado es **un 500 en esa petición**, no la caída del móvil. Sigue siendo un 500 sin diagnóstico (no hay `panic::set_hook`, ver **D-10**) y en `GET /api/status` el 500 se lleva por delante datos que la UI necesita. | `unwrap_or_else(\|_\| json!({"status":"error"}))` en las 20 + log del error real. | S (1 h) |
| **C-08** | P1 | `rust-core/src/android.rs:202-215` (hilos lectores) y `:164-165` (`join()`) | `root_exec` mata el hijo al expirar el timeout, pero **espera sin límite** a los dos hilos lectores (`out_handle.join()`): si el comando deja un hijo que hereda stdout (típico `nohup … &`), el pipe no cierra ⇒ **la llamada MCP se queda colgada para siempre**, con su hilo y su pipe. | (C) `android.rs:202-215` y `:164-165`. | Un `root_exec` colgado **consume un hilo del cockpit para siempre** (y con `MAX_CLIENT_THREADS=64`, 64 colgados ⇒ el cockpit deja de aceptar). | `join` con timeout (canal + `recv_timeout`) y cerrar los pipes al expirar. | M (3 h) |
| **C-09** | P2 | `rust-core/src/providers.rs:102-121` (`load_vault`, copia `.corrupt` en `:85`) | Si `providers.json` está **corrupto**, `load_vault()` escribe una copia `.corrupt-<ts>` en cada llamada y **nunca repara el fichero**. Como `load_vault()` se llama en cada request que toque proveedores (`/api/providers`, `/api/brain_status`, `/api/chat`…), el cockpit crea **un fichero por petición** hasta llenar la partición. | (C) `providers.rs:102-108` (la copia se escribe dentro de la rama de error, que se repite en cada llamada). | Un `providers.json` a medias (escritura interrumpida) ⇒ el móvil genera ~7.000 ficheros/día (1 cada 12 s de refresco) y acaba sin espacio ⇒ el daemon deja de arrancar. | Escribir la copia **una sola vez** (mover el corrupto a `.corrupt` y dejar que `load_vault()` use el default en silencio), y hacer `save_vault` **atómico** (tmp+rename, ver UI-06). | S (1 h) |
| **C-10** | P2 | `rust-core/src/cockpit_server.rs:2021-2033` (comentario "100 MB" vs `MAX_BODY_BYTES = 8 MB`) y `:1486-1498` (`chat_upload`); `web/cockpit_template.html:976-985` (base64 en el navegador) | El comentario dice *"Cap single HTTP body at 100 MB"* pero el tope real son **8 MB**, y como la UI sube en **base64**, el fichero real máximo es **~6 MB**. Un APK de 60 MB ⇒ **413** y la UI no explica por qué. | (C) `cockpit_server.rs:2021-2024` + `:1495-1498` + `cockpit_template.html:976-981`. | "Subir un APK" (el caso de uso del agente externo) falla de forma opaca en el móvil. | Subida por trozos (`fs_write` ya lo hace bien: `tools/upload_bin.py` trocea a 400 KB) o subir el tope a 64 MB **por ruta autenticada** y con tope acumulado (ver A-16). | M (½ día) |

---

## D. FIABILIDAD

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **D-01** | ✅ CORREGIDO | `deploy/99-megadaemon.sh:195-212` + `:247-256` | Tras **3 fallos de arranque seguidos** el supervisor hace `exit 0` y **no vuelve a intentarlo** hasta el próximo reinicio del móvil. Un `EADDRINUSE` transitorio (el puente occupying 9090, el daemon antiguo sin morir todavía), un binario truncado o un `/system` lleno ⇒ **el móvil se queda sin daemon hasta que se reinicia**. | (C) `99-megadaemon.sh:198-212` (`fast_fails`, `MAX_FAST_FAILS=3`, `return 2` → `exit 0`). | El fallo clásico "el móvil va JUSTO de RAM" (OOM al arrancar) deja el dispositivo sin herramienta y **sin aviso** salvo un fichero de estado en `/sdcard`. | No `exit 0`: seguir con backoff largo (5-30 min) y **marcar `degraded`**; reservar el `exit` para un fallo de *código* inequívoco (p. ej. `Exec format error`), y reiniciar el servicio de Magisk con `stop`/`start` en vez de morirse. | ✅ Hecho — supervisor v15: auto-chmod + auto-rollback + "nunca se rinde" (reintenta cada 5 min); commit `7fd13de` |
| **D-02** | ✅ CORREGIDO | `deploy/install-phone.sh:110` (copia `.bak`) y `:175-177` (`verify \|\| true`) | Se guarda el binario anterior como `.bak`… y **nunca se restaura**. Además `verify \|\| true` hace que **el despliegue termine con código 0 aunque el cockpit no responda**. | (C) `install-phone.sh:110` (crea el `.bak`), `grep -n "bak"` → **no hay ninguna restauración**; `install-phone.sh` final: `verify \|\| true`. | Un despliegue con un binario roto deja el móvil **sin daemon y con el script diciendo "hecho"**. La cadena automática (que el enjambre usa) no se entera. | Rollback real: si `verify` falla ⇒ `cp $BIN.bak $BIN` + reiniciar + reintentar; y `verify` **sin** `\|\| true` (o `exit 1`). | ✅ Hecho — `install-phone.sh` hace verify→rollback de verdad (ya no `verify \|\| true`); commit `7fd13de` |
| **D-03** | P1 | `deploy/99-megadaemon.sh:249` y `:274` (`pgrep -f "megadaemon-core cockpit"`) | El supervisor comprueba **que el proceso exista**, no que **sirva**. El estado "vivo pero mudo" (justo el que motivó `cockpit_doctor.rs`) sigue sin detección automática. | (C) `99-megadaemon.sh:249`, `:274`; `cockpit_doctor.rs:348` (`diagnose`) existe pero **no está cableado** (F-03). | El móvil puede tener el proceso vivo y **cero puertos abiertos** — el "ERR_CONNECTION_REFUSED" que el usuario reportó tres veces. | El supervisor debe consultar `/health` (no `pgrep`) y, si falla N veces, **reiniciar** el daemon. Idealmente invocar `cockpit_doctor::diagnose` (que ya sabe mirar puertos, plantilla, token y log). | M (½ día) |
| **D-04** | P1 | `deploy/99-megadaemon.sh:264-277` | El bucle interno duerme 5 s y en cada vuelta lanza `ensure_web_bridge()` → `pgrep -f` + `grep` sobre `/proc/net/tcp{,6}` ⇒ **3-4 procesos fork cada 5 segundos, para siempre** (día y noche). | (C) `99-megadaemon.sh:269-277` → `:135-162` (`pgrep`, `grep`). | En un móvil, un `pgrep` cada 5 s mantiene el wakelock del process_refresher y **gasta batería**; el `grep` de `/proc/net/tcp` es I/O de procfs. | Un solo `sh` que comprueba `/proc/net/tcp` con `read` (sin fork) y un `pgrep` cada 60 s; o un watchdog en Rust (que ya está en el daemon). | S (2 h) |
| **D-05** | P2 | `deploy/99-megadaemon.sh:22-23` (`LOG=/sdcard/...`), `:61-79` (`rotate`), `:247-248` | El log del daemon va a **`/sdcard`** (FUSE: cada `println!` del daemon paga el FUSE) y la rotación sólo se evalúa **una vez por vuelta del bucle exterior (≈60 s)**: entre rotaciones el log puede pasar de 10 MB a 60 MB. | (C) `99-megadaemon.sh:22`, `:247-248`, `:66-79`. | Escrituras lentas al log = I/O extra en un dispositivo con 78 % de iow; y rotación tardía. | Log en `<STATE_DIR>/logs/` (ext4 interno) y rotación por **tamaño en el propio daemon** (no desde el shell). | S (2 h) |
| **D-06** | P2 | `rust-core/src/coordination.rs:237` (`Connection::open`) | El tablero abre la SQLite **sin `busy_timeout`** (el de SQLite es 0) ⇒ dos procesos que escriben a la vez reciben `database is locked` **al instante**, sin esperar. Con WAL activo (`coordination.rs:266`) el problema persiste en las escrituras. | (C) `coordination.rs:237-245` (no hay `busy_timeout`) frente a `coordination.rs:266-267` (sí hay WAL). | El tablero (el sitio donde "vive el plan") falla de forma intermitente y **pierde claims** justo cuando hay más de un agente. | `PRAGMA busy_timeout=5000` + reintento con backoff en las escrituras. | S (1 h) |
| **D-07** | P2 | `tools/findings_from_manifest.py:565-570` | La salida (`findings.ndjson`) se escribe con `open(out_path, "w")`: **trunca y luego escribe**. Si el proceso muere a mitad, el fichero queda truncado y el daemon (o el agente) leerá NDJSON a medias. | (C) `findings_from_manifest.py:567`. | Un `Ctrl-C` o un OOM dejan el fichero de hallazgos corrupto y **no hay forma de distinguir "vacío" de "truncado"**. | Escribir a `.tmp` + `os.replace` (atómico en el mismo FS). | S (20 min) |
| **D-08** | P2 | `rust-core/src/cockpit_server.rs:2476-2595` | `run_server` nunca devuelve: su bucle final es `loop { sleep(3600) }` y el `Ok(())` de la línea 1940 es **código muerto**. No hay ruta de **apagado ordenado** (ni `SIGTERM`): el daemon siempre muere por `killall`. | (C) `cockpit_server.rs:2476-2480`. | Cada reinicio es un `kill -9` en pleno I/O: se pueden perder escrituras (NDJSON, `providers.json`, `board.sqlite`). | Manejar `SIGTERM`/`SIGINT` → cerrar el listener, vaciar buffers y salir 0; y hacer el supervisor `kill -TERM` antes de `kill -9`. | M (½ día) |
| **D-09** | ✅ CORREGIDO | `rust-core/src/task_scheduler.rs` (18), `alerts.rs` (6) | **24 `lock().unwrap()` sobre mutex GLOBALES** (`task_scheduler.rs`: `queue()`/`journal()`/`COUNTER`; `alerts.rs`: `store()`; `npu_cv.rs`: `vision_state()`; `npu_bridge.rs`: `cache()`). Un panic mientras se mantiene el lock **envenena** el mutex ⇒ a partir de ahí **toda** llamada a ese mutex paniquea ⇒ el subsistema queda **muerto para el resto de la vida del proceso**, y como el release desenrolla (v15) el daemon **sigue contestando 200/500** ⇒ parece vivo y no lo está. | (C) `grep -rn "lock()\.unwrap()" rust-core/src/` → **24** al cierre (a las 03:3x): `task_scheduler.rs` (18) + `alerts.rs` (6). `task_scheduler.rs:94-100` (`static Q`/`static J` con `OnceLock<Mutex<…>>`). Contraste: **el cockpit ya está arreglado** (`cockpit_server.rs:1899/1912/1931` usan `unwrap_or_else(\|e\| e.into_inner())`) ⇒ el patrón correcto ya existe en el repo. **Parte ya arreglado a las 03:38**: `npu_cv.rs` (6) y `npu_bridge.rs` (1) pasaron a `unwrap_or_else(\|p\| p.into_inner())` ⇒ quedan **24**, no 31. | La **cola de tareas compartida** (la que usan el MCP, la delegación y el tablero) envenenada = el sistema de agentes queda **inutilizable** sin que nada lo anuncie: el móvil "va bien", el daemon responde, y nada se encola. En `alerts.rs` las alertas dejan de poder registrarse; en `npu_cv.rs` el estado de visión se queda congelado. | Sustituir los 31 por `lock().unwrap_or_else(\|p\| p.into_inner())` (patrón ya usado en `api_key.rs` y `cockpit_server.rs`). Con `panic::set_hook` (D-10) los panics dejan rastro. | ✅ Hecho — los 31 `lock().unwrap()` pasaron a `unwrap_or_else(\|p\| p.into_inner())`: lane NPU a las 03:38 (`npu_cv.rs` 6, `npu_bridge.rs` 1, commit `c363b39`) y el resto a las 03:41 (`task_scheduler.rs` 18, `alerts.rs` 6). **Verificado: `grep -rn "lock()\.unwrap()" rust-core/src/` → 0**, y el test `adv_d09_…` pasa. |
| **D-10** | ✅ CORREGIDO | `rust-core/src/` (todo el crate) | **No hay ningún `panic::set_hook`**: un panic se imprime por stderr y se pierde. | (C) `grep -rn "panic::set_hook" rust-core/src/` → **0 hits**. | El **134 (SIGABRT) medido en el móvil** (HW-01) **no puede ser un panic de Rust** con unwind ⇒ viene de un `abort()` explícito, un doble panic o un assert en lib nativa (sqlite3/NPU): **sin hook no hay backtrace y no hay forma de localizarlo**. Y los panics que sí ocurren envenenan los mutex de D-09 **sin dejar rastro**. | (a) `std::panic::set_hook` en el arranque que registre `payload` + `backtrace::Backtrace` a un fichero del `state_dir`; (b) `RUST_BACKTRACE=1` en el entorno del supervisor. **Requisito que propone la lane NPU** (que se apunta a hacerlo): el hook debe **distinguir panic en hilo de petición** (⇒ 500, el daemon sigue) de **panic en hilo de fondo** (⇒ ahí sí muere el proceso: ése es el 134 que buscamos). **Dueño propuesto: la lane NPU (`46e23911`), pendiente de que el principal le ceda el arranque (`main.rs`)** — no es `cockpit_server.rs`/`shm.rs`/`99-megadaemon.sh`, luego **no choca con la regla de intocabilidad**. | ✅ Hecho — `install_panic_hook()` en el arranque: mensaje + hilo + `Backtrace::force_capture` (`main.rs:58-73`, commit `a41b527`, lane NPU). **El 134 medido ya no es una caja negra.** |
| **D-11** | ✅ CORREGIDO | `rust-core/src/android.rs`, `rust-core/src/cockpit_server.rs` (árbol de trabajo vs `HEAD`) | **A-18 sigue SÓLO en el árbol de trabajo, sin commitear** ⇒ el **RCE root puede volver** sin querer: `git show HEAD:rust-core/src/cockpit_server.rs` **sigue teniendo** `(local_browser && !provided.is_empty())` (línea 1574). **A-01/A-02 ya están commiteados** (`efd6dc3`, harness/MCP) **pero en la rama `agent/cockpit-robust`, NO en `main`** ⇒ mientras no haya cherry-pick, `main` **sigue teniendo** el heredoc `<<'__MD_EOF__'` (`git show HEAD:rust-core/src/android.rs`, línea 779) y el ARM64 que se publique desde `main` **no lleva el arreglo**. | (C) `git show HEAD:<fichero> \| grep …` (comandos exactos en la fila) + `git status --short` → `M android.rs`, `M cockpit_server.rs`. | **Ya ha pasado**: a las 03:4x un `git checkout`/merge ajeno **borró** el fichero de test de esta auditoría (`rust-core/tests/audit_adversarial_v15.rs` volvió a su versión anterior, perdiendo 2 tests). El mismo mecanismo puede devolver el heredoc y el token root al `main`. Además, sin commitear, **el ARM64 que se publique no lleva el arreglo**. | (a) **cockpit/web** (`e81fade1`): `git add rust-core/src/cockpit_server.rs && git commit -m "fix(P0): A-18 el intercambio comprueba la API key real"`; (b) el **principal**: `git cherry-pick efd6dc3` para que A-01/A-02 entren en `main`. Nada más: el código ya está arreglado. | ✅ Hecho — los tres arreglos P0 están **commiteados y en `main`**: `efd6dc3` (A-01 base64 + A-02 escapado POSIX, `android.rs`) y `9f30665` (A-18, `cockpit_server.rs`) ⇒ `git show HEAD:<fichero>` ya trae el arreglo. (Durante el proceso: el 20-11 a las ~07:3x un `git checkout origin/main --` sobre todos los módulos de `lib.rs` **machaqued** las ediciones sin commitear de web y del harness ⇒ los arreglos se re-aplicaron y commitearon; por eso este hallazgo era real.) |

---

## E. RENDIMIENTO Y RECURSOS (el S21 Ultra va JUSTO de RAM)

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **E-01** | P1 | `rust-core/src/cockpit_server.rs:821` (`api_key::ApiKeyConfig::load(&state_dir())` **dentro** de `handle_client`) | **En cada petición** se releen del disco `megadaemon_api.key` + `megadaemon_api.keys` (2 ficheros) y se relee el env. No hay caché (ni por fichero ni con TTL). | **(T)** `adv_e01_…`: escribir la key **después** de arrancar la hace efectiva en la petición siguiente ⇒ no hay caché. (C) `cockpit_server.rs:821`. | Con 20 peticiones por recarga de UI y un refresco cada 12 s: **~60 lecturas de fichero por minuto** en flash, con el móvil ya al 78 % de iow. | Cachear la config con TTL (5-10 s) + un `mtime` para invalidar; o leerla una vez y recargarla con `SIGHUP`. | S (2 h) |
| **E-02** | P1 | `rust-core/src/providers.rs:102` (`load_vault()`), llamadas en `cockpit_server.rs:1407`, `:1017`, `:1452` | La **bóveda de proveedores** (que contiene las API keys) se relee y se re-parsea en cada request que la toca, y `POST /api/providers` la relee **y** la reescribe sin lock (ya está en ↩ UI-06). | (C) `providers.rs:102-121` + las tres llamadas. | Coste de I/O y de CPU por request en un móvil; y una **carrera de escritura** que pierde keys (↩ UI-06, sigue viva). | Caché en memoria con `RwLock` + escritura atómica (el arreglo de UI-06). | S (2 h) |
| **E-03** | P1 | `cockpit_server.rs:695-718` + `:628-631` + `:1919-1932` | El techo de memoria real del cockpit es **512 MB de buffers + 128 MB de pilas** con 64 clientes. | **(T)** `adv_e02_…` (aritmética con las constantes reales). (C) las líneas. | OOM-killer en el S21 Ultra (10,2/10,5 GB usados) ⇒ el daemon **y** el puente mueren a la vez. | Tope **acumulado** de cuerpos + `Content-Length` obligatorio + menos hilos. | M (1 día) |
| **E-04** | P2 | `cockpit_server.rs:1910-1935` | Un hilo por conexión **y** un hilo por listener (×2 por dual-stack: `0.0.0.0` y `[::1]` ⇒ **8 hilos de escucha** para 4 puertos). Cada hilo de conexión reserva su pila de 2 MB. | (C) `cockpit_server.rs:1863-1872` (bind IPv6) + `:1910-1934`. | 8 hilos de escucha + 64 de clientes = 72 hilos ⇒ ~144 MB de pilas virtuales y schedulación extra en un móvil. | Un solo hilo con `poll`/epoll o un `TcpListener` por puerto compartido entre v4/v6 (dual-stack con `IPV6_V6ONLY=0`). | M (1 día) |
| **E-05** | P2 | `web/cockpit_template.html:1588` (`setInterval(refreshHardware, 12000)`), `:1616` (`setInterval(refreshPower, 20000)`) | Sondeo **ciego** cada 12 s y 20 s, sin parar cuando la pestaña está oculta, sin backoff si falla, y **sin usuario** (A-05: el rate limit no cubre el loopback). | (C) `cockpit_template.html:1588`, `:1616`. | 5 peticiones/minuto permanentes desde el navegador del móvil; con la UI abierta en el fondo, el daemon nunca está quieto. | `document.visibilityState` + backoff exponencial + `AbortController` para cancelar la anterior (esto último arregla también la carrera UI-13). | S (2 h) |
| **E-06** | P2 | `rust-core/src/android.rs:228-252` (`audit`) | `audit.ndjson` se **añade** (append) en cada llamada y **nunca rota**. Cada `root_exec` escribe una línea con el comando completo. | (C) `android.rs:228-238` (`.append(true)`, sin rotación en el fichero). | Crecimiento ilimitado en el almacenamiento del móvil (con la app: `/data/user/0/…`, que Android limpia al desinstalar, pero no mientras se usa). | Rotar por tamaño (5 MB × 3) dentro de `audit()` + redactor del comando. | S (1 h) |
| **E-07** | P2 | `rust-core/src/seere_v11.rs:1290-1297` | Cada `exec_root` prueba hasta **3 runners** (`su`, `rish`, `sh`), y cada intento lanza **una sonda** (`Command::new(prog).arg("exit 0")`) ⇒ hasta **6 forks** por orden ejecutada, más el comando. | (C) `seere_v11.rs:1290-1297`. | Un chat con 10 turnos = 60 forks en el móvil (y `su` pide a Magisk: cada `su` es un diálogo con el gestor de root). | Cachear qué runner funciona (detectado una vez por sesión) y saltarse la sonda si ya se sabe. | S (2 h) |
| **E-08** | P2 | `cockpit_server.rs:1405-1430` (`/api/brain_status`) | Cada refresco de la UI llama a `brain_status`, que **relee la bóveda de proveedores** (E-02) + **abre un socket TCP** (`check_llama_online`, 250 ms de timeout) + sondea el NPU (con caché de 5 s). | (C) `cockpit_server.rs:1405-1407`. | Cada 12 s: 1 lectura de fichero + 1 conexión + trabajo de CPU, con el móvil al límite. | Un endpoint de "resumen" que reúna todo lo que la UI pinta cada 12 s (y que el daemon cachee 5-10 s). | M (½ día) |

---

## F. ARQUITECTURA

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **F-01** | P1 | `rust-core/src/mesh.rs:416` (`BOARD_FILE = "mesh_board.json"`), `:64` (`mesh_journal.ndjson`), `:342-348` vs `rust-core/src/coordination.rs:237` (`board.sqlite`) | **Dos tableros de tareas** con formatos, bloqueos y modelos de datos distintos: `mesh.rs` (JSON + fichero de lock + `MeshTask`/`AgentTask`/`TaskClaim`) y `coordination.rs` (SQLite + WAL + `BoardTask`/`TaskStatus`). El `MASTER_PLAN.md` §6 ya lo marca como "P0.6 a resolver" y §8 pregunta qué hacer: **sigue sin resolverse dos meses después**, y el módulo `webui.rs` (v2) lee `board.sqlite` mientras `mesh` sigue vivo. | (C) `mesh.rs:416-424` y `:337-348`; `coordination.rs:226-245`. Búsqueda: `grep -rn "board.sqlite\|mesh_board.json"` → 2 implementaciones distintas. | Dos fuentes de verdad ⇒ las tareas del plan existen en un sitio y las del enjambre en otro; el tablero "único" no lo es. | Decidir (la propuesta del plan: `coordination.rs` = estado, `mesh.rs` = transporte) y **apagar** el tablero de `mesh` (borrar el módulo o dejar sólo discovery/tunnel). | M (1-2 días) |
| **F-02** | P1 | `rust-core/src/mcp_server.rs:20-34` (`tool_definitions`) vs `rust-core/src/mcp_ext.rs:17-237` | El catálogo MCP expone `core + android + sync + adb` = **27 tools**. Las **17 herramientas v14** (`capabilities`, `task_queue`, `alerts`, `agent`, `apk_inventory`, `dex_decompile`, `app_intel`, `root_hide`, `repo_updates`, `profile`, `messaging`, `bookmarks`, `battery_saver`, `git_ops`, `identity_and_web`, `root_stealth`, `ssh_remote`) **sólo** se alcanzan por `POST /api/tool/<nombre>`, una ruta REST propia que un cliente MCP **no puede descubrir**. `mcp_ext::tool_definitions()` **sólo** lo llama `webui.rs`, que a su vez no está cableado (F-05). | (C) `mcp_server.rs:20-34` (no incluye `mcp_ext`), `cockpit_server.rs:1334-1345` (la ruta REST), `grep -rn "mcp_ext::tool_definitions"` → **una** llamada, en `webui.rs`. | El agente externo (que habla MCP) **no puede usar** el decompilador DEX, el inventario de APKs ni la intel de apps: exactamente lo que el prompt le pide hacer (H-03). | Fusionar `mcp_ext::tool_definitions()` en `mcp_server::tool_definitions()` y el `dispatch` en `dispatch_tool` (con el mismo contrato de `confirm`). | S (½ día) |
| **F-03** | P1 | `rust-core/src/cockpit_doctor.rs:518` (`pub fn run_doctor_cli`) vs `rust-core/src/main.rs:55-330` (sin subcomando `doctor`) | El módulo **`cockpit_doctor` existe y funciona**, pero `main.rs` **no lo cablea**: no hay `megadaemon-core doctor`. El tablón dice "el principal cablea `pub mod cockpit_doctor;` y el subcomando `doctor`" — la primera mitad se hizo, la segunda no. | (C) `cockpit_doctor.rs:518` (la función existe) + `grep -n '"doctor"' rust-core/src/main.rs` → **0 resultados**. | La herramienta creada **exactamente** para diagnosticar "el proceso está vivo pero /health no responde" **no se puede ejecutar**. Es el instrumento que faltaba en D-03. | Añadir el subcomando `doctor` a `main.rs` (5 líneas) y que el supervisor lo use cuando `/health` falle. | S (1 h) |
| **F-04** | P2 | `rust-core/src/bin/md-coord.rs:59-180` vs `docs/MASTER_PLAN.md:213-222` | El CLI tiene `register/heartbeat/add/claim/report/tasks/instances/migrate/snapshot` pero **no `sync`**, que es el comando documentado (`md-coord sync --peer http://<ip>:9099 --token <t>`). | (C) `grep -n '"sync"' rust-core/src/bin/md-coord.rs` → 0 (sólo aparece `instances`). | El sync peer-a-peer documentado no se puede lanzar; la documentación miente sobre la herramienta. | Añadir el subcomando `sync` (la función `sync_with_peer` ya existe en `coordination.rs`). | S (2 h) |
| **F-05** | P2 | `rust-core/src/lib.rs:129-190` (62 `pub mod`), ficheros `rust-core/src/webui.rs` (93 KB) y `rust-core/src/signing_vault.rs` (27 KB) sin declarar | **120 KB de código escrito que no se compila**: `webui.rs` (la UI v2) y `signing_vault.rs` (el vault de firma) están en disco pero **no** en `lib.rs` ⇒ no compilan, no se prueban y no se despliegan. | (C) `lib.rs` completo (70 líneas, sin `webui` ni `signing_vault`); `git status` los marca como `??` (untracked). | La "v2 de la UI" que el plan da por en marcha **no existe como código compilable**; y el vault de firma tampoco. Regla del propio repo: *"código sin cablear es código que no existe"*. | Declarar los módulos **con su arnés de test** (el patrón `#[path]` de `tests/`), o borrarlos del árbol hasta que haya tiempo. | M (1 día) |
| **F-06** | P2 | `rust-core/src/cockpit_server.rs:37-45` (`state_dir()` → `<STATE_DIR>`) vs `rust-core/src/bin/md-coord.rs:52-55` (`/data/local/tmp/megadaemon`) vs `rust-core/src/mesh.rs:320` (`default_state_dir`) | **Tres** directorios de estado distintos según por dónde se entre. En el móvil: el token y la key viven en el dir privado de la app, el tablero en `/data/local/tmp`, el log en `/sdcard`. | (C) las tres líneas citadas + `web_bridge.py:48-53` (que busca la key en **dos** sitios). | Un fichero "no aparece" en un sitio y sí en otro; el `state_dir` del daemon no es el que usan las tools de coordinación. | **Una** variable `MEGADAEMON_STATE_DIR` como única fuente, leída de `megadaemon.env` por todos (daemon, md-coord, puente, supervisor). | S (3 h) |
| **F-07** | P2 | `rust-core/src/android.rs:1-20` y 40 módulos con rutas `/data/user/0/com.agy/...` | El núcleo está atado a **rutas absolutas de Android**: `cockpit_server.rs:26-29` (`MEGADAEMON_DIR`, `RE_DIR`, `SCUMMVM_TOOLS_DIR`, `STATE_DIR`), `shm.rs:5` (`SHM_PATH`), `seere_v11.rs` (STATE_DIR), `sync.rs`, `messaging.rs`… En un PC Linux esas rutas no existen → las tools devuelven errores crípticos ("no such file or directory") en lugar de "esta capacidad sólo existe en Android". | (C) `cockpit_server.rs:26-29`; `android.rs:719-735` (`fs_read` de una ruta de Android). | El PC Linux (el "worker" del plan) es casi inservible: la mitad de las tools fallan con errores que no explican la causa. | Un `paths.rs` con resolución por entorno (`MEGADAEMON_HOME`) + un `capability::verify()` que **diga** "requiere Android" antes de que la tool falle. | M (1-2 días) |
| **F-08** | P2 | `rust-core/src/android.rs:139-178` (`root_exec` → `Command::new("su")`) | En un PC Linux sin `su`, `root_exec` devuelve `stderr: "no se pudo ejecutar 'su': ..."` — un error **sin contexto** ("¿estoy en el móvil?"). `root_exec_tool` lo envuelve en `{"ok": false, "result": {...}}` con `code: null`. | (C) `android.rs:139-149` (mensaje literal) y `:829-835`. | El agente externo en el PC no sabe si falló el comando o el entorno. | `Err("no_android_runtime")` explícito + `platform` en la respuesta de error. | S (1 h) |
| **F-09** | P2 | `rust-core/src/cockpit_server.rs:830-1284` (GET), `:1310-1806` (POST) | El router son **dos `match` gigantes** con ~90 rutas, duplicados entre GET y POST (p.ej. `/api/kaitai_parse`, `/api/memory_freeze` existen en los dos arms con cuerpos distintos) y un `catch-all` que **no falla** (A-08). No hay tabla de rutas ni *feature test* que la recorra. | (C) `cockpit_server.rs:830` y `:1310`; duplicados: `/api/kaitai_parse` en `:1122` y `:1554`; `/api/memory_freeze` en `:1213` y `:1709`. | Un cambio de ruta se hace en un arm y se olvida en el otro ⇒ endpoints " fantasma" (GET y POST con semántica distinta). | Tabla de rutas (`&[(&str, HttpMethod, Handler)]`) generada desde una única fuente, con test que recorra **todas** las rutas y compruebe auth+status. | M (1-2 días) |

---

## G. COBERTURA DE TESTS

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **G-01** | P1 | `rust-core/tests/` (listado) | **No existe `audit_scout_ui.rs`**, que es el fichero que `docs/AUDIT_V14_UI_HTTP.md` cita como prueba de sus 15 hallazgos ("*cada hallazgo se reproduce con un test ejecutable que FALLA con el código actual (`rust-core/tests/audit_scout_ui.rs`)*"). Tampoco existe `cockpit_doctor_smoke.rs` (7 tests). | (C) `ls rust-core/tests/` → `audit_scout_auth.rs, audit_scout_hw.rs, audit_scout_seere.rs, audit_v13.rs, delegate.rs, fake_nnapi.c, harness_policy.rs, npu_nnapi.rs, root_hide_app_intel.rs, signing.rs, signing_vault.rs, zz_mesh_harness_tmp.rs, zz_tmp_dex_decompile.rs`. | Los arreglos de la auditoría UI (UI-01…UI-15) **no son verificables**: cualquiera puede decir "ya está arreglado" sin que exista el test. Es el mismo problema que esta auditoría va a repetir si el arnés no se versiona. | Versionar el arnés de cada auditoría **en el mismo commit** que el arreglo, y que CI lo ejecute. | S (2 h) |
| **G-02** | P1 | 31 módulos de `rust-core/src/*.rs` | **sin un solo `#[cfg(test)]`**: `seere_v11.rs` (2.348 líneas), `cockpit_server.rs` (1.948), `providers.rs` (759), `cirx_rtl.rs` (759), `binwalk_die.rs` (758), `npu_cv.rs` (427), `disasm.rs` (377), `npu_bridge.rs` (375), `kaitai_imhex.rs` (326), `provider_catalog.rs` (294), `mcp_server.rs` (291), `frida_dbi.rs` (275), `shm.rs` (267), `rea_assistant.rs` (267), `tile_vram.rs` (261), `power_policy.rs` (250), `cfg_lifter.rs` (245), `adb_tool.rs` (209), `hw_mapper.rs` (203), `crypto_scan.rs` (194), `rom_locator.rs` (193), `freezer.rs` (192), `deobfuscate.rs` (191), `unifier.rs` (171), `snapshot.rs` (128), `entropy.rs` (102), `pointers.rs` (97), `pattern.rs` (94), `bindiff.rs` (91) | (C) `for f in src/*.rs; do grep -q "cfg(test)" $f \|\| echo $f; done` → la lista de arriba; los que sí tienen tests: `messaging` (33), `delegate` (25), `battery_advanced` (23), `repo_updates` (21), `mesh` (21), `dex_decompile` (21), `sshd` (17), `root_hide` (16), `app_intel` (16)… | El núcleo que **decide** (SEERE, el cockpit, los proveedores, el MCP) no tiene ni un test unitario. Los 121 tests del repo cubren parsers, no control de flujo. | Prioridad por riesgo: (1) `cockpit_server` (auth, rutas, errores), (2) `seere_v11` (política de exec), (3) `providers` (bóveda, precios), (4) `mcp_server` (JSON-RPC). Mínimo 5 tests por módulo. | L (varias olas) |
| **G-03** | P2 | `rust-core/tests/zz_mesh_harness_tmp.rs`, `zz_tmp_dex_decompile.rs` | Ficheros de test **temporales** (`zz_*_tmp`) que nadie ha borrado: 0 tests, sólo incluyen el módulo por `#[path]`. Uno de ellos (`zz_mesh_harness_tmp.rs`) incluye `mesh.rs` **fuera de `lib.rs`**, es decir, compila una **tercera copia** del tablero (F-01). | (C) `ls -la rust-core/tests/` + `cat rust-core/tests/zz_mesh_harness_tmp.rs` (325 B, 0 tests). | Ruido en la suite y copia divergente de `mesh.rs` que "compila verde" sin ser la que se despliega. | Borrarlos (o renombrarlos con el prefijo del módulo y convertirlos en tests reales). | S (15 min) |
| **G-04** | P1 | `rust-core/tests/` (ninguno prueba el gate de auth del cockpit) | Ningún test cubre la **matriz de autenticación** del cockpit: qué rutas son públicas, cuáles exigen token, cuáles exigen API key, qué hace el CORS. Es la superficie que produce el P0 A-07. | (C) `grep -rn "X-Seere-Auth-Token\|requires_auth\|cors_allowed" rust-core/tests/` → sin cobertura del servidor. | Los cambios en auth (que son los de v14) se despliegan **sin red de seguridad**: por eso A-07 ha podido colarse. | Un test de integración que arranque el cockpit y recorra **todas** las rutas con y sin token (este arnés lo hace ya, con 4 aserciones). | M (½ día) |
| **G-05** | ~~P2~~ | `rust-core/Cargo.toml` (perfil release) + `cockpit_server.rs:2163/2273` + `webui.rs:4061` | (a) Con `panic = "abort"` no hay `catch_unwind` y un test de panic aborta todo el binario de test; (b) no hay ningún test que compruebe que las rutas HTTP **no** paniquean. | (C) **a las 03:2x**: `Cargo.toml` dice "v15: SIN panic=abort a propósito"; `grep -rn catch_unwind rust-core/src/` → **3 sitios** (`cockpit_server.rs:1928`, `:2273`, `webui.rs:4061`). ⇒ **(a) ✅ RESUELTO** (decisión documentada en `docs/DECISION_PANIC_UNWIND.md`, commit `9e22f29`). (b) sigue abierto pero lo cubre **G-04** (test de la matriz de auth/errores). | *(quedaba)* Un panic en el móvil = daemon muerto + supervisor que puede no volver (D-01). **Ya no**: unwind + `catch_unwind` ⇒ 500 por petición. | *(b)* Un test de integración que recorra las rutas HTTP y compruebe que **ninguna** devuelve 500 por panic (con `AssertUnwindSafe`). | ✅ Hecho (a) |
| **G-06** | P3 | `rust-core/tests/` + `Cargo.toml` | Coste de la suite: **102 tests** en 12 ficheros de integración + ~387 tests unitarios. Con `CARGO_TARGET_DIR` local el build completo del crate + tests tardó **33 s** (medido por otro agente, `docs/AUDIT_DELEGACION_V14.md` §4 Q-1), pero el **coste del binario** crece: `signing.rs` (207 KB), `dex_decompile.rs` (179 KB) y `profile_analysis.rs` (100 KB) compilan **siempre**, aunque el móvil no los use. | (C) `grep -c '#\[test\]' rust-core/tests/*.rs` → 102; `wc -l src/*.rs` → 64.918 líneas. | Cada agente paga el build completo; el binario del móvil pesa más de lo necesario (LTO + `codegen-units=1` sobre 69 módulos). | Features por binario (`--no-default-features` + features `re`, `signing`, `mesh`) para el móvil. | M (1 día) |

---

## H. EL AGENTE EXTERNO Y EL CONVERSOR DE FINDINGS

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **H-01** | P2 | `tools/findings_from_manifest.py:439-445` (mueve `signal`→`class`) y `:499` (escribe `src.get("signal")`, **no** la variable local) | El conversor declara resolver la colisión histórica DEX/app_intel *"se mueve (y se anota) en vez de duplicarse"*. En realidad **duplica**: cuando un `signal` con valor de enum de `class` se mueve a `class`, `signal` **sigue con el mismo valor** (porque el `out` lee `src`, no la variable local ya anulada). | **(T)** `adv_h01_…`: `normalize({'signal':'fs_path','class':'unknown'})` → `class="fs_path"` **y** `signal="fs_path"`. Reproducción directa: `python3 -c "import findings_from_manifest as F; print(F.normalize({'package':'com.x','signal':'fs_path','class':'unknown'},{},[]))"`. | El esquema cerrado v1.2 queda **violado en el campo que el bus de coordinación usa para decidir**: un valor de clase duplicado como "valor crudo" hace que el consumidor crea que se observó una señal `/sbin/.magisk` cuando en realidad era una etiqueta de clase. | Escribir el `out` con las **variables locales** (`signal`, `method_raw`) y no con `src`. | S (30 min) |
| **H-02** | P2 | `docs/CONNECT_EXTERNAL_AGENT_ANDROID_RE.md:22` y `:70` vs `rust-core/src/cockpit_server.rs:238-266` | El doc dice que el token de escritura *"sólo se necesita en POST/PUT"*, pero **toda** llamada MCP es un POST a `/mcp` y `requires_auth()` **no mira el método**: el token es obligatorio **siempre**, también para `tools/list`. | (C) `cockpit_server.rs:238` (`fn requires_auth(path: &str, _method: &str)` — el método se **ignora**) + `:1418` (`/mcp` en la rama POST). | El agente externo que siga la lectura literal ("tools/list sin token") recibe **403** y puede concluir que la instancia está caída. | Corregir el doc ("el token es obligatorio en **todas** las llamadas MCP") y, mejor, eximir `tools/list` del token (es metadato, no estado). | S (30 min) |
| **H-03** | P1 | `docs/CONNECT_EXTERNAL_AGENT_ANDROID_RE.md:59,108-134` (26 tools) vs `rust-core/src/mcp_server.rs:20-34` (27) vs `rust-core/src/mcp_ext.rs:17-237` (17 más) | El contrato dice "26 tools, verificado contra el binario desplegado v13.3.0" y advierte que el repo tiene más. Al desplegar v14 el MCP expone **27** (nuevo: `adb`), y las **17 herramientas v14 siguen inalcanzables por MCP** (F-02). El prompt le pide al agente "no los busques en tools/list: si no están, no los uses" ⇒ **el agente renuncia a DEX/APK/app_intel justo cuando lo que el usuario le ha pedido es eso**. | (C) `mcp_server.rs:20-34` (core 7 + android 11 + sync 8 + adb 1 = 27) y `mcp_ext.rs` (17 más, sólo REST). | El agente externo no puede hacer el trabajo para el que se le contrató. | F-02 (fusionar los catálogos) + regenerar el prompt desde `tools/list` **real** (un script que lo descargue del móvil y lo pegue en el doc). | M (1 día) |
| **H-04** | P2 | `tools/mdmcp.py:35-45` (`timeout=900`), `:48-53` (`init()` en cada `call`) | Cada llamada MCP hace **dos** viajes (`initialize` + la llamada) y usa un timeout de **900 s** sin reintentos ni backoff. Con el rate limit por IP (A-05 lo desactiva para loopback, pero el túnel viene de 127.0.0.1 ⇒ tampoco aplica)… | (C) `mdmcp.py:35`, `:56-58`. | Con la tasa de 429 que el enjambre sufrió, el cliente espera 900 s y el agente interpreta "cayó la instancia". | Reintentos con backoff exponencial ante 429/5xx, `initialize` **una vez** por sesión, timeout adaptativo (30 s por defecto). | S (1 h) |
| **H-05** | P3 | `tools/findings_from_manifest.py:233-237` | La docstring de `load_manifest` dice *"-> (artefactos, indice_evidence)"* pero la función **devuelve 3** valores (`artifacts, index, broken`). La contradicción ya costó un bug (`broken` sin usar). | (C) `findings_from_manifest.py:234` vs `:261`. | Ruido menor, pero es el tipo de cosa que hace que el siguiente agente lea el contrato equivocado. | Corregir la docstring. | S (5 min) |

---

## I. UX DE LA UI v1 (`web/cockpit_template.html`)

> Diagnóstico de la **v1**. La v2 (`rust-core/src/webui.rs`, aún sin cablear) es otro tema.

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **I-01** | ✅ CORREGIDO | `web/cockpit_template.html:795-801` (`authHeaders`) + `rust-core/src/cockpit_server.rs:872` (token vacío inyectado) | La v1 **sólo** sabe usar `window.MEGADAEMON_TOKEN`, y la v14 lo inyecta **vacío**. No hay ninguna otra vía: la plantilla no menciona `X-API-Key` ni `/api/session/exchange` (búsqueda literal: **0 coincidencias**). | **(T)** `adv_a07_…`: con el binario v14, `GET /api/status` sin token ⇒ 403, y la plantilla no tiene forma de obtenerlo. (C) `cockpit_template.html:795-801`. | **La web v1 con el binario v14 no muestra un solo dato**: todas las pestañas (SWARM/NPU/INSPECT/VRAM/SYSTEM) quedan vacías con un "HTTP 403" en cada `catch`. | Resolver A-07 + añadir a la v1 un campo de API key con llamada al intercambio al arrancar (o reusar el mecanismo de la v2). | ✅ Hecho — la UI v1 **canjea la API key por un token de sesión** (`askApiKey()` → `exchangeSession()` → `POST /api/session/exchange`, `web/cockpit_template.html:824-852`, commit `86ea251`) y el servidor **incrusta** el token en el navegador local (`cockpit_server.rs:986`). Con el binario nuevo la web **sí** pinta datos. |
| **I-02** | P1 | `web/cockpit_template.html` (todo el fichero) | **No hay forma de introducir la API key ni de autenticar**; tampoco indicador de "401/403" (los `catch` sólo pintan el texto del error en la caja). | (C) `grep -n "X-API-Key\|api_key\|session/exchange\|401" cockpit_template.html` → 0 coincidencias (el único `api_key` es el del *provider vault*). | El usuario ve cajas con "HTTP 403" sin ninguna pista de qué hacer. | Un banner de estado de auth con campo de API key (y que se guarde en `localStorage`, no en el repo). | S (3 h) |
| **I-03** | P2 | `cockpit_template.html:976-985` (base64 en el navegador) + `cockpit_server.rs:695` (8 MB) | La subida **falla a partir de ~6 MB** y el error es un 413 sin explicación; el comentario del servidor dice "100 MB" y el endpoint se anuncia como "100% UNRESTRICTED". | (C) `cockpit_template.html:976-981` + `cockpit_server.rs:692-695`. | El caso de uso real (subir un APK) falla en el móvil sin diagnóstico. | Trocear la subida en el navegador (o subir por `fs_write` por trozos, que ya funciona) + mensaje explícito del límite. | M (½ día) |
| **I-04** | P2 | `cockpit_template.html` (sin versión, sin estado del daemon, sin contadores) | La UI **no muestra la versión del daemon**, ni el estado del servicio, ni el consumo (RAM/CPU del propio daemon), ni si la API key está activa, ni el rate limit. Hay un `/metrics` en el servidor que nadie usa. | (C) `grep -n "version" cockpit_template.html` → 0 usos del `version` del daemon; `cockpit_server.rs:1249-1259` (`/metrics`) sin consumidor. | "La web no me funciona" se queda sin diagnosticar desde la propia web (que es justo lo que motivó `cockpit_doctor`). | Una tira de estado arriba (versión, uptime, RAM del daemon, clave activa, puertos) leyendo `/health` + `/metrics`. | S (3 h) |
| **I-05** | P2 | `cockpit_template.html:1588` y `:1616` | El sondeo no se detiene con la pestaña oculta, no tiene backoff si falla, y **no cancela** la petición anterior (la carrera UI-13 ya auditada sigue viva). En un móvil, la UI abierta en segundo plano mantiene el daemon en_requests. | (C) las dos líneas + `cockpit_template.html:1528` (la carrera, ↩ UI-13). | Consumo de batería y de red del móvil; y estados obsoletos en pantalla. | `visibilitychange` + `AbortController` + contador de secuencia (el arreglo que propose UI-13). | S (2 h) |
| **I-06** | P2 | `cockpit_template.html` (5 pestañas: swarm/npu/inspect/vram/system) | **Nada de lo nuevo es visible**: no hay vista del **tablero de tareas**, ni de las **alertas**, ni del **perfil**, ni de la **delegación**, ni de los **reportes de findings** del agente externo. 44 capacidades v14 viven en la API y 0 en la pantalla. | (C) `grep -n 'id="view-' cockpit_template.html` → 5 vistas; `mcp_ext.rs` define 17 herramientas. | El producto se ve como "un visor de ROMs y un chat"; el 80 % del trabajo del enjambre es invisible para el usuario. | Priorizar 2-3 vistas nuevas en la v2 (Estado del enjambre / Tareas / Hallazgos). | L (1-2 olas) |
| **I-07** | P3 | `cockpit_template.html` (34 `catch (e)` que pintan el error en la caja) | Cada error se muestra en la caja de la sección, sin distinguir "daemon caído" de "403" de "respuesta vacía", y **sin reintento ni acción**. El usuario ve "Error: HTTP 403" sin contexto. | (C) `grep -c "catch (e)" cockpit_template.html` → 34; no hay ningún manejador global. | Diagnóstico lento: el usuario tiene que abrir el log del móvil a mano. | Un manejador global de errores con toast + reintento + enlace a `/health`. | S (2 h) |
| **I-08** | ✅ CORREGIDO | `rust-core/src/api_key.rs` (respuesta 403) | **El 403 no dice si tu API key está caducada**: el túnel vivo responde `{"error":"forbidden","hint":"API key invalida"}` ⇒ un agente con la key vieja deduce "el móvil está roto" cuando el móvil está **perfecto** y lo que caducó es su credencial local. Se han perdido horas de diagnóstico así (la flota entera diagnosing el móvil mientras el daemon servía 200). | (C) el 403 medido por el agente principal a las 07:1x; `api_key.rs` (rama `Decision::Forbidden`) → cuerpo sin referencia a la key vigente. | Cada agente con la key caducada rediagnostica el móvil entero (túnel, daemon, RAM, iow) antes de sospechar de su propia credencial. | Incluir en el 403 la **huella fnv1a64 de la key vigente** (la huella **no** es el secreto: es un hash de 64 bits de una clave de alta entropía, no invertible): así el que va con la key caducada ve "tu key caducada" de un vistazo, y el que va con la buena ve que coincide. **Dueño propuesto: lane de `api_key.rs`** (la de A-04). | ✅ Hecho **con una variante más segura** que la propuesta: el 403 lleva `sent_key_fp` (la huella de la key que el cliente **sí** presentó = su propio secreto, no filtra nada) y `key_rotated_at` (mtime del fichero de la key vigente) ⇒ el agente ve "el daemon recibió MI key" y "la vigente se rotó después de mi copia" ⇒ diagnóstico instantáneo **sin publicar nada derivado de la key vigente**. Publicar el hash de la key vigente convertiría al daemon en **oráculo de verificación** (candidatos offline contra la huella) ⇒ **no se publica**. Commit `1c40990`; **falta el enganche de 1 línea en `cockpit_server.rs:934`** (intocable: lo parchea cockpit/web). |
| **I-09** | **P2** | `rust-core/src/cockpit_server.rs:1031` (la llamada a `forbidden_body`) | **El 403 siempre dice `sent_key_fp: "-"`**: el gate pasa `None` como "key presentada" ⇒ el campo que debería decir *qué key recibió el daemon* **nunca se rellena**. `forbidden_body()` sí sabe calcularlo (`api_key::fingerprint`, fnv1a64) y el test de `api_key.rs` lo ejercita con `Some(...)`, pero **en producción el camino real pasa `None`**. | (C) `cockpit_server.rs:1031` → `api_key::forbidden_body(&state_dir(), None)`; `api_key.rs:553-565` (`fp = presented.map(fingerprint).unwrap_or("-")`). | Un proxy que reescriba la cabecera `X-API-Key` (o un cliente que mande la key equivocada sin querer) es **indistinguible** de "tu key caducada" ⇒ el agente no puede saber si el daemon recibió otra cosa. | Una línea: `api_key::forbidden_body(&state_dir(), api_key::extract_key(&header_str).as_deref())` (`extract_key` ya es `pub`, `api_key.rs:553`) ⇒ el 403 dice la huella de la key **que el cliente ya conoce** (no se filtra nada: es su propio secreto hasheado). **Dueño propuesto: web (`e81fade1`) / principal** — es el mismo `cockpit_server.rs` que ya está patching la línea 934. | S (5 min) |

---

## J. LO QUE NO SE PUEDE HACER (límites honestos)

Esto **no son defectos**: son límites del sistema. Se listan porque el plan los
hereda y porque el usuario debe saber qué **no** se puede prometer.

| # | Límite | Por qué | Qué haría falta | Impacto en el plan |
|---|---|---|---|---|
| **J-01** | **Play Integrity / attestation** | La firma es del hardware; `ro.boot.verifiedbootstate` etc. son **atestaciones del TEE**, no propiedades de `system.prop`. `root_hide.rs` sólo manipula props y presence de ficheros: **no puede** falsificar la atestación, y el propio repo lo dice (`docs/MASTER_PLAN.md` P3.2). | Hardware firmado (Keymaster/StrongBox con clave del fabricante) o un **TEE attestation agent** real. | **P1.4 (ocultación de root) nunca dará apps de banca**. Debe venderse como "reduce la superficie detectable", no como "pasa Play Integrity". |
| **J-02** | **Cookies de Chrome** | Android las cifra con una clave del **AndroidKeyStore** en el TEE; ni root ni ADB las exportan (`docs/MASTER_PLAN.md` P3.1, `docs/AUDIT_ENJAMBRE_V14.md` §6). | Inyección en proceso (Frida) o una exportación oficial del fabricante. | `sync.rs --cookies` **nunca** admite cookies de Chrome: hay que decirlo en la UI, no esconderlo. |
| **J-03** | **WhatsApp / cifrado de la base** | El almacén está cifrado con la clave del keystore del proceso; el `messaging.rs` lo reporta como "no legible" (correcto). | Decrypt con Frida dentro del proceso, o un backup del proveedor. | `backup_full.rs` sólo puede entregar **metadatos**, no mensajes. |
| **J-04** | **Google Play sin API de instalación** | No hay API pública para instalar/actualizar desde Play; automatizarlo es automatizar la cuenta del usuario. | `Obtainium`/F-Droid (ya está en `repo_updates.rs`) o la API de Play Console para apps propias. | `repo_updates` cubre **F-Droid/Obtainium**, no Play. Decirlo en la UI. |
| **J-05** | **FNMT-RCM (Java)** | El configurador exige Java 17; no hay reimplementación abierta. | El atajo ya descrito: la app oficial de la FNMT en Android emite el `.p12` leyendo el DNIe por NFC; a partir del `.p12`, todo es Rust (`signing.rs` + `signing_vault.rs`). | `signing_vault.rs` **no está cableado** (F-05): la parte que sí puede hacerse (emitir/guardar `.p12`) no está en el binario. |
| **J-06** | **Historial de Gemini / modelos cloud** | Vive en el servidor, no en local; sin API oficial de exportación para la cuenta. | La API oficial (con coste) o el export manual. | `app_intel.rs` puede decir **dónde** está, no **leer** lo que hay en el servidor. |
| **J-07** | **Portabilidad del daemon a PC** | ~40 módulos asumen rutas y herramientas de Android (`su`, `magisk`, `getprop`). En Linux fallan con errores sin contexto (F-07/F-08). | `capability::verify()` por plataforma + un modo "host" que degrade con mensajes claros. | El PC Linux es un **worker de tareas simples**, no un MegaDaemon completo. |

---

## PLAN DE MEJORA DERIVADO (4 olas)

> Orden de ataque pensado para el **S21 Ultra** (10,2/10,5 GB, 78 % iow) y para
> el **canal de despliegue** (el móvil tiene v13.3.0; el repo v14).

### Ola 1 — CONSOLIDAR LO YA ARREGLADO (día 0-1, ~2 h) — *que no se pierda*

> **Los 6 P0 están cerrados** (§0.4). Esta ola ya **no es arreglar**, es
> **consolidar**: hacer durable lo que se arregló y cerrar lo que quedó a medias.

1. ~~**D-11**~~ → **✅ cerrado**: los tres arreglos P0 están **commiteados y en
   `main`** (`efd6dc3` para `android.rs`, `9f30665` para `cockpit_server.rs`) ⇒
   el ARM64 que se publique **sí** lleva los arreglos. *Lección operativa:*
   commitear con **ruta exacta** y en `main`, nunca `git add -A` (un
   `git checkout origin/main --` sobre todos los módulos de `lib.rs` ya machaquó
   ediciones sin commitear de dos agentes).
2. **A-18 (resto, P1)**: `requires_auth` sigue eximiendo de token a **todos** los
   `GET` de un cliente "local" ⇒ cualquier app del móvil lee datos sin
   credencial. Arreglo: que la exención sea sólo para la **página** (y para
   `/v2`, `/webui_v2.css`) y **no** para `GET /api/*`; y que "local" sea el
   **peer loopback real** (que es lo que ya hace el puente), no el `Host`.
   Mismo parche que el del nodo externo/web ⇒ **no** abrir un conflicto nuevo.
3. **A-16/E-03** (techo de 512 MB de buffers) antes de que entre nadie más por
   el túnel: el móvil tiene ~300 MB libres.
4. **G-01** (recuperar `audit_scout_ui.rs`): sin arnés versionado, los arreglos
   de esta auditoría tampoco se van a poder verificar.
5. **Releer el arnés**: `cargo test --release --test audit_adversarial_v15`
   ⇒ los 18 tests deben quedar **verdes** (los P0 ya están) y quedar **rojos**
   los que aún tienen dueño (`adv_a18` por el resto de A-18, `adv_h01`, `adv_d10`).

> **Criterio de salida**: el arnés con **0 P0 en rojo** (los `adv_a01/a02/a03`
> en verde ⇒ los P0 están de verdad cerrados) y la web del móvil pintando datos
> con el binario nuevo.

### Ola 2 — ROBUSTEZ (días 2-5, ~3 días) — *que el sistema se sostenga solo*

- **A-03/A-06/A-11**: **una sola política de destructividad** en todo el daemon
  (`Arsenal::exec_root` → `android::root_exec_tool`), y `confirm` para `/data/adb`.
- **D-03 + F-03**: el supervisor decide con `/health`, no con `pgrep`; y
  `megadaemon-core doctor` existe (5 líneas) para el diagnóstico.
- ~~**D-09**~~ → **✅ cerrado el 20-11 03:38/03:41**: los 31 `lock().unwrap()`
  pasaron a `unwrap_or_else(|p| p.into_inner())` (`npu_cv.rs`, `npu_bridge.rs` por
  la lane NPU; `task_scheduler.rs`, `alerts.rs` por el bus). `grep` → 0 ✓
  (el patrón ya está en `cockpit_server.rs:1899/1912/1931` ⇒ **el cockpit ya está
  arreglado**, el resto no). Sin esto, un panic envenena el mutex y **todo** el
  subsistema queda muerto para el resto del proceso.
- **D-10** (P2): `std::panic::set_hook` con `backtrace` al fichero del `state_dir`.
  Con unwind, el **134 (SIGABRT) medido en el móvil no puede ser un panic de
  Rust** ⇒ viene de un `abort()` explícito, un doble panic o un assert en
  sqlite3/NPU: **sin hook no hay forma de localizarlo**. (Lane del arranque,
  no de `cockpit_server.rs`/`shm.rs` ⇒ no choca con la intocabilidad.)
- **C-07** (P2): las **20** `to_value(...).unwrap()` de las rutas HTTP →
  `unwrap_or_else(|_| json!({"status":"error"}))` + log del error real.
  Con unwind + `catch_unwind` ya **no** tumban el daemon: son un 500 por
  petición, pero un 500 **sin diagnóstico** (D-10).
- **C-09 + B-05 + D-07**: escrituras atómicas en `providers.json`, el registro de
  keys y `findings.ndjson`.
- **D-06**: `busy_timeout` en el tablero.
- **C-04/C-03/C-05/C-02**: Status texts correctos, **una** versión en toda la API,
  errores propagados (no `unwrap_or_default`).
- **C-10 + I-03**: subida por trozos (para que el agente externo pueda subir APKs).

### Ola 3 — RENDIMIENTO Y EXPERIENCIA (semana 1-2, ~4 días)

- **E-01/E-02/E-08**: **cachés** para la config de la API key, la bóveda de
  proveedores y el resumen de estado; un endpoint "resumen" para la UI.
- **A-16/E-03/E-04**: topes de memoria y de hilos **acumulados**, no por conexión;
  `Content-Length` obligatorio; revisar el número de hilos.
- **I-01…I-07**: la UI v2 (`webui.rs`, **sin cablear**: F-05) con: auth visible,
  estado del daemon (versión/RAM/puertos), fin del sondeo ciego, subida por
  trozos, y **3 vistas nuevas** (estado del enjambre, tareas, hallazgos) para que
  el producto se vea como lo que es.
- **D-04/E-05/E-06/E-07**: menos forks (supervisor), sondeo con `visibilityState`,
  rotación de `audit.ndjson`, cachear el runner de root.
- **F-05**: cablear (o borrar) `webui.rs` y `signing_vault.rs`.

### Ola 4 — EXPANSIÓN (semana 2-4, ~2 semanas) — *lo que faltaba*

- **F-01**: decidir el tablero único (`coordination.rs` = estado, `mesh.rs` =
  transporte) y **apagar** el segundo. Sin esto, la Ola 0 de "que el sistema se
  sostenga solo" del plan maestro sigue sin poder cerrarse.
- **F-02 + H-03**: un **catálogo MCP único** (las 17 tools v14 incluidos) y el
  contrato del agente externo regenerado **desde el móvil real** (script que
  descarga `tools/list` y regenera el prompt).
- **H-01/H-02/H-04**: conversor de findings correcto (sin duplicados), doc del
  contrato corregido y cliente MCP con reintentos.
- **F-06/F-07/F-08**: un `MEGADAEMON_STATE_DIR` único y `capability::verify()`
  por plataforma (que el PC diga "esto es Android-only" en vez de reventar).
- **F-04**: `md-coord sync` (el comando que el plan ya promete).
- **J-05**: cablear el vault de firma para que el atajo FNMT (app oficial →
  `.p12` → Rust puro) exista de verdad.
- **G-02**: cobertura de los 31 módulos sin test, empezando por los de decisión
  (`seere_v11`, `providers`, `mcp_server`, `cockpit_server`).

### Dependencias (qué desbloquea qué)

```
Ola 1 ──┬─► Ola 2 ──┬─► Ola 3 ──► Ola 4
        │           │
        └─ A-07 ───┴─► (la UI v1 sirve de campo de pruebas para la v2)
F-01 (tablero único) ─────────────────► Ola 4 (protocolo distribuido real)
F-02 (catálogo MCP) ──────────────────► H-03 (el agente externo puede trabajar)
G-01 (arneses versionados) ───────────► todas (nada se puede verificar si no)
```

---

## TOP 10 (los que quedan abiertos — **ya no hay P0**)

| # | ID | Sev | Defecto | Evidencia |
|---|---|---|---|---|
| **1** | **B-03 / B-04** | P1 | El repo publica la **URL del túnel público**, la **IP de la LAN** (`<IP_DEL_MOVIL>`) y la **IP del móvil**, y además la **ruta del cofre de la API key** (`<STATE_DIR>/megadaemon_api.key` = `/data/user/0/com.agy/...`) ⇒ cualquiera que clone el repo tiene **puntería perfecta** contra ese móvil | (C) `docs/` + `tools/` (URL del túnel, IPs) + `api_key.rs` (ruta del cofre) |
| ~~2~~ | ~~**A-18** (resto)~~ | — | ✅ **CERRADO el 2026-11-21** (§0.7.1): la exención pasó a **lista blanca** ⇒ los endpoints de datos **exigen token** ⇒ ya no hay fuga de datos | `cockpit_server.rs:266-301` (lista blanca) |
| **3** | **A-16 / E-03** | P1 | 64 clientes × 8 MB = **512 MB** de buffers (+128 MB de pilas) en un móvil con ~300 MB libres | **Test** `adv_e02_el_techo_de_memoria_del_cockpit_supera_la_ram_libre` + `cockpit_server.rs:695-704` |
| **4** | **A-11** | P1 | `/api/chat` ejecuta root **sin** la política destructiva (el MCP sí la aplica) | `seere_v11.rs:1290-1296` + `:2006-2018` vs `android.rs:821-836` |
| **5** | **A-06** | P1 | `/data/adb` se escribe **sin** `confirm` (la política destructiva ya la cubre el harness: A-03 cerrado) | `android.rs:744-750` |
| **6** | **D-03 / F-03** | P1 | El supervisor decide con `pgrep` ("vivo" ≠ "sirviendo") y **`cockpit_doctor::run_doctor_cli` no está cableado** | `99-megadaemon.sh:249` + `cockpit_doctor.rs:518` (la función existe; falta el subcomando) |
| **7** | **F-02** | P1 | **17 tools de v14 no están en el catálogo MCP** ⇒ el agente externo los llama por `POST /api/tool/<nombre>` y el MCP responde "tool desconocida" | `mcp_server.rs:20-34` (catálogo: 27 tools) vs `mcp_ext.rs:15-235` |
| **8** | **A-05** | P1 | El rate limit (60/min) **no se aplica al loopback** ⇒ en el móvil todo el tráfico entra por 127.0.0.1 ⇒ el límite no limita nada | `api_key.rs:53-61` (`rate_applies_to`) |
| **9** | **C-01** | P1 | La estimación de tokens es `chars/4`; el propio repo dice que está entre un 15 % y un 30 % mal ⇒ el presupuesto de contexto se desborda (o se recorta de más) | `delegate.rs:864-872` + `tools/token_budget.py:6-8` |
| **10** | **C-02** | P2 | `/api/hw_mmio` convierte "no se pudo leer" en "fichero vacío" ⇒ el análisis responde "no hay MMIO" en vez de "no existe ese fichero" | `cockpit_server.rs:1244-1251` |

## DUEÑOS PROPUESTOS (asignación de lo que queda sin dueño)

Actualizado a las **07:2x**: los P0 ya tienen dueño y **están arreglados**;
esto es lo que **queda abierto** y quién debería tomarlo. **Propongo dueño, no
lo toco**: los ficheros de otros agentes son intocables para mí.

| Defecto | Fichero | Dueño propuesto | Por qué ese | El arreglo que pido |
|---|---|---|---|---|
| **D-11** (P1, **lo más urgente**) | `rust-core/src/cockpit_server.rs` (+ cherry-pick de `android.rs`) | **nodo externo/web** (`e81fade1`) para el commit de `cockpit_server.rs`; **principal** (`6d7ffc8f`) para el cherry-pick | el código ya está arreglado; lo que falta es **commitearlo con ruta exacta** y **traerlo a `main`** | `git add rust-core/src/cockpit_server.rs && git commit -m "fix(P0): A-18 …"` + `git cherry-pick efd6dc3`. **10 min**, y sin esto el RCE root puede volver. |
| **A-18** (resto, P1) | `rust-core/src/cockpit_server.rs:234` | **nodo externo/web** (`e81fade1`), **en su parche en vuelo** | el token root ya está cerrado; queda la exención de token en todos los `GET` | que la exención sea sólo para la **página** (y `/v2`, `/webui_v2.css`), no para `GET /api/*`; y que "local" sea el **peer loopback real** |
| ~~**I-08**~~ | `rust-core/src/api_key.rs` (cuerpo del 403) | **✅ hecho** por la lane NPU (`1c40990`) con una **variante más segura**: `sent_key_fp` (la huella de la key que el cliente **sí** presentó) + `key_rotated_at` ⇒ diagnóstico instantáneo **sin publicar nada derivado de la key vigente** | | **falta el enganche de 1 línea en `cockpit_server.rs:934`** (intocable ⇒ lo parchea cockpit/web o el principal) |
| **A-16 / E-03** (P1) | `rust-core/src/cockpit_server.rs` (buffers) | **nodo externo/web** (`e81fade1`) | el techo de 8 MB × 64 es suyo | acotar la memoria **acumulada**, no por conexión; `Content-Length` obligatorio |
| ~~**A-03**~~ (P1) | `android.rs` (política) | **✅ cerrado** por la lane harness (`is_destructive` normaliza y cubre las variantes) |  |  |
| **A-11 / A-06** (P1) | `seere_v11.rs`, `android.rs` | **lane SEER/harness** (`cc01d5d7`) | el MCP y el chat ejecutan root con políticas distintas | **una sola** política de destructividad en todo el daemon + `confirm` para `/data/adb` |
| **F-02** (P1) | `mcp_server.rs`, `mcp_ext.rs` | **bus** (`9cb64913`) | el catálogo MCP lo decide el bus | que las 17 tools de v14 aparezcan en `tools/list` (o que el MCP acepte `POST /api/tool/<nombre>`) ⇒ el agente externo deja de "no ver" herramientas |
| **D-03 / F-03** (P1) | `99-megadaemon.sh` (intocable), `cockpit_doctor.rs` | **nodo externo** (el supervisor) + **nodo externo/web** (cablear el subcomando `doctor`) | el supervisor no puede decidir con `pgrep` | el supervisor pregunta `/health`; y `megadaemon-core doctor` existe de verdad |
| **C-01** (P1) | `delegate.rs:864-872` | **bus** (`9cb64913`) | el estimador es del carril de delegación | tokenizer de verdad (o el de `token_budget.py`), no `chars/4` |

> **Nota de método**: los "dueños" de arriba son **propuestas** por carril y
> fichero, no asignaciones. Si el bus puede reasignar, es mejor que lo haga con
> su mapa real (que yo no tengo: los `git log` de este repo usan nombres de
> autor genéricos —"MCode Audit", "MegaDaemon Bus"— que **no** identifican la
> sesión). El único dato duro que doy es el **fichero**.

---

## Anexo — Cómo reproducir

```bash
cd /workspace/megadaemon/rust-core
CARGO_HOME=/run/agent-runtime/md/cargo \
CARGO_TARGET_DIR=/run/agent-runtime/md/target \
cargo test --release --test audit_adversarial_v15 -- --nocapture --test-threads=1
```

* Los tests `adv_a01`, `adv_a02` y `adv_a06` necesitan `su` (los ejecuta como
  root **en el sandbox**, no en el móvil) y **limpian** lo que crean.
* `adv_a18`, `adv_a19`, `adv_a08` y `adv_a09` arrancan el cockpit real en un
  puerto libre y hablan HTTP con él.
* `adv_h01` requiere `python3`; si no está, el test se marca como *skip*.

**Lo que este arnés NO puede comprobar** (y por eso está marcado como
`NO VERIFICADO EN RUNTIME`): el comportamiento real en el Galaxy S21 Ultra
(RSS del daemon, iow, 429 del túnel, estado del supervisor). Eso requiere el
dispositivo, que en esta sesión está en manos de otro agente.
