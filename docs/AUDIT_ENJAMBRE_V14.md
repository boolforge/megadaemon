# Auditoría del enjambre v14 — por qué "no avanza" y qué se hace ahora

> Salida de la reunión virtual del 2026-11-19 (puesto: agente de delegación /
> P1.3). Todo lo de aquí está **medido** en esta sesión; lo que no se ha
> medido aparece marcado como `pendiente`. Objetivo: que la siguiente
> reunión empiece con un plan que se pueda ejecutar, no con una lista de
> intenciones.

## 0. Resumen ejecutivo (5 líneas)

1. **El enjambre no tiene un problema de trabajo: tiene un problema de
   E/S.** `/workspace` está montado por **NFS v3 contra una NAS** y **no se ha
   ableado ni un solo `.rlib` en todo el workspace** después de horas de
   build (medido en los 15 `target/`).
2. **El workspace también es un NAS**, así que el historial de git, los
   ficheros de estado y los artefactos compiten por la misma I/O. Regla
   propuesta: **`CARGO_TARGET_DIR` en disco local**, no en el workspace.
3. Hay **23 ficheros de código sin commitear** (~60.000 líneas) y `main`
   **divergido** de `origin/main` (behind 10 / ahead 4): el trabajo está en
   peligro de perderse hoy mismo.
4. El **tablero no existe**: no hay ningún `board.sqlite` en el repo ni en
   el PC. "El plan vive en el demonio" sigue siendo una intención.
5. P1.3 (delegación) queda **cerrado y commiteado** (`e512461`) con tests y
   con los precios re-verificados contra las fuentes oficiales.

## 1. La causa raíz: el build ocurre sobre un NAS

### 1.1 Lo medido

| Medida | Valor |
|---|---|
| Tipo de `/workspace` | `nfs` (vers=3, `nolock`, `hard`, TCP) → NAS Alibaba |
| Disco local del contenedor | overlay/ext4, 30 GB, **26 GB libres** |
| `CARGO_HOME` | `/workspace/.home/.cargo` (**también en el NAS**) |
| `.rlib`produced en todo el workspace | **0** (15 target dirs revisados) |
| Procesos `cargo`/`rustc` simultáneos | 65 → 128 |
| Load average (2 CPUs) | 28-39 |
| `cp -a` del registro de crates (NAS → local) | **3,7 MB en 4 minutos** |
| RAM | 3 GB totales, ~700 MB libres |

### 1.2 Por qué esto rompe el enjambre

- **Los `.rlib` no llegan a escribirse.** Cargo escribe el artefacto final
  con muchas operaciones aleatorias sobre el mismo fichero; sobre NFS eso
  falla o se queda colgado. De ahí los "failed to copy bitcode to object
  file" y "rust-lld: cannot open ...rcgu.o" que ya documentaba
  `tools/build-queue.sh`.
- **"database is locked"** no es contención real: el índice de crates es un
  SQLite sobre NFS con `nolock`. Dos builds que no se pisan de verdad se
  expulsan mutuamente.
- **Cada agente recompila lo mismo.** Con la regla actual ("target aislado
  por agente"), `serde`, `serde_json`, `rusqlite`, `zip`, `flate2`, `sha2` se
  compilan **15 veces**, una por agente, cada una escribiendo y releyendo
  cientos de MB por red. El trabajo no escala: se multiplica.
- **Git también paga el peaje**: `index.lock` saltar (me pasó dos veces
  mientras commiteaba), pushes lentos, `git status` sobre el NAS tarda.

### 1.3 La regla propuesta (sustituye a la regla 5 del plan)

> **Regla 5-bis — "El build ocurre en disco local".**
> `CARGO_TARGET_DIR` **nunca** dentro de `/workspace` (es NAS).
> Destino canónico: `/run/agent-runtime/md/target` (ext4 local, 26 GB).
> `CARGO_HOME` puede seguir en el NAS (se lee, no se escribe mucho) pero
> con `--offline` para no tocar el índice.
> Todo build pasa por `tools/build-queue.sh`, que además serializa con
> `flock` (2 slots) y tiene guardia de memoria.

```bash
# recipe única para el enjambre
CARGO_TARGET_DIR=/run/agent-runtime/md/target \
  tools/build-queue.sh build --release --offline
```

El objetivo medible: **el primer build tarda minutos, el segundo segundos**
(artefactos reutilizados). Hoy no hay segundo build.

**Alternativa si el disco local no es compartible entre agentes**: un único
target compartido en local + `flock` es exactamente lo que ya hace
`tools/build-queue.sh`; sólo hay que cambiar `MD_BUILD_TARGET` al disco local.

## 2. Lo que está en peligro ahora mismo

| # | Riesgo | Medida | Acción |
|---|---|---|---|
| R-1 | 23 ficheros de código untracked (~60.000 líneas) | `git status` | Cada agente: `git add <su fichero>` + commit. **Lo más barato del enjambre** |
| R-2 | `main` divergido: behind 10 / ahead 4 de `origin/main` | `git branch -vv` | Reconciliar con rebase y aviso; **nunca `--force`** |
| R-3 | Ningún `board.sqlite` existe | `find` en repo y PC | Crear el tablero en el primary (`md-coord register --primary`): es el rendez-vous de la reunión |
| R-4 | `lib.rs` cablea 9 módulos que dormían sin compilar | `git show 93e0503` vs `lib.rs` | Correcto ya, **pero cada módulo nuevo debe entrar con su arnés de test** |
| R-5 | Un módulo nuevo rompió el build de todos (E0425 en `delegate.rs:561`) | logs de build | El arnés por `#[path]` (tests/*.rs) evita que esto vuelva: **el módulo se prueba antes de entrar en `lib.rs`** |

## 3. Reglas de operación (propuesta para el plan, versión 2)

| # | Regla | Por qué |
|---|---|---|
| 1 | Un agente = un módulo Rust (+ su arnés de test) | evita colisiones |
| 2 | `git add <ruta exacta>`, un commit por ámbito, nunca `-A` | 23 ficheros untracked son el resultado de lo contrario |
| 3 | **Build en disco local** (§1.3) | sin esto no hay artefacto |
| 4 | Builds por `tools/build-queue.sh` (flock, 2 slots) | mata la contención y los errores falsos |
| 5 | Todo módulo entra en `lib.rs` **con su arnés `#[path]` verde** | el fallo aparece en el módulo, no en el enjambre |
| 6 | Informe de 3 líneas + 1 línea en el tablón | el chat no es cola |
| 7 | Doble auth + `confirm: true` en acciones irreversibles | seguridad |
| 8 | Evidencia o nada: test, medición o `pendiente` | nada de "creo que funciona" |

## 4. Ondas revisadas (propuesta para el plan)

### Ola A — "que el enjambre pueda trabajar" (hoy, horas)
1. Adoptar el build en disco local + cola (§1.3). **Sin esto, todo lo demás
   es teatro.**
2. Commitear los 23 ficheros untracked (cada agente los suyos).
3. Cablear en `lib.rs` los módulos con arnés verde y **verificar que compila
   el lib entero** (1 build local).
4. Crear el tablero en la instancia(primary) y publicar la primera ola ahí.
5. Reconciliar `main` con `origin/main`.

### Ola B — "lo que el usuario pidió" (días 1-5)
Firma sin Java (P1.1) · DEX (P1.2) · **delegación (P1.3, cerrad hoy)** ·
ocultación root + intel (P1.4) · **migración a Drive con verificación por
hash (P1.5, sin empezar)** · batería/Magisk (P1.6) · backups + grabador
(P1.7) · **migrar las tareas de las cuentas al demonio (P1.8, el cargador de
inventario ya está)**.
Salida: cada P1 con test verde + evidencia en el dispositivo + línea en el
tablón.

### Ola C — "que nada se pierda" (semana 2)
Backups completos · motor de navegador (wry) · migración de tareas de las
cuentas de IA · adjuntos binarios de las apps.

### Ola D — "el móvil como herramienta" (semana 3-4)
Conector de banco (con consentimiento explícito) · firma en trámites reales ·
actualizaciones de Play Store (limitado: sin API pública).

### Bloqueos honestos (P3) — no se tocan
Play Integrity no se falsea · cookies de Chrome viven en el TEE del
AndroidKeyStore · almacén de WhatsApp cifrado con la clave del keystore ·
Play Store sin API de instalación · historial de Gemini en el servidor, no en
local · **FNMT-RCM necesita Java 17**: el atajo es la app oficial de la FNMT
en Android, que emite el `.p12` leyendo el DNIe por NFC; a partir del
`.p12`, todo es Rust puro.

## 5. Lo que este módulo aporta al plan

- **P1.3 cerrado** (`e512461`): enrutado siempre a la cuenta gratuita con las
  de pago reservadas, cuenta quemada sin tareas, **precios con fecha y
  fuente**, más el cargador de inventario real (la base de P1.8).
- **La decisión 5 del §8 del plan maestro** ("presupuesto de tokens: coste
  real, no estimado a ojo") tiene ya respuesta técnica: doble contador
  `cost_usd` / `shadow_cost_usd` y detector de precio rancio.
- **Lección reutilizable para todos los módulos**: un módulo que entra en
  `lib.rs` sin arnés de test rompe el build de 15 agentes a la vez (pasó hoy
  con `E0425`). El arnés `#[path]` es el seguro: 5 líneas por módulo.

## 6. Pendiente (honesto)

| # | Qué falta | Quién |
|---|---|---|
| Q-1 | ~~Confirmar con medición que el build en disco local da artefacto~~ | **RESUELTO y MEDIDO el 2026-11-19**: con `CARGO_HOME` y `CARGO_TARGET_DIR` en disco local, el build completo de un crate + sus tests tardó **33 s** y quedó verde (25+30 tests). Con el target en el NAS: nunca acaba. La receta exacta está en `docs/AUDIT_DELEGACION_V14.md` §4 |
| Q-2 | Precio de `perplexity` (4 cuentas reales) ⇒ hoy queda fuera del enrutado | hardware/NPU + P1.3 |
| Q-3 | Modelo "preferido" **medido** por cuenta (no el de por defecto) | hardware/NPU |
| Q-4 | Estado real del `/health` del Samsung y existencia de `board.sqlite` en el móvil | hardware/NPU |
| Q-5 | Decidir `coordination.rs` (estado) vs `mesh.rs` (transporte) — sigue abierto | reunión |
| Q-6 | Rotar el token de GitHub expuesto en el chat | principal |
| Q-7 | **P1.5 (migración a Drive con verificación por hash) no lo ha tomado nadie** | sin dueño |
| Q-8 | `wry` (motor de navegador) no está en `Cargo.toml` y no compila para Android ⇒ el valor en el móvil es nulo: allí navega el Chrome del propio móvil | P2.1 |
