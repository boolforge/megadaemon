# ORDEN IMPERATIVA GLOBAL — cuenta MiniMax (MCode) en el proyecto MegaDaemon

> **Esto es una orden, no una sugerencia.** El usuario la pidió así el
> 2026-11-26: *«debería de ser una orden imperativa global para mi cuenta
> minimax»*.
>
> Ámbito: **toda** la flota de agentes que trabaja en `boolforge/megadaemon`
> (root, subagentes, agentes externos, sesiones nuevas). Si un agente la
> contradice, el trabajo se considera **mal hecho** aunque compile.
>
> Léela al empezar y antes de commitear. Si algo de esta orden choca con una
> instrucción puntual del usuario en ese momento, **manda el usuario**.

---

## 0. La regla que manda sobre las demás

**No perder el hilo ni el foco.**

El fallo histórico del enjambre no es de código: es de **continuidad**. Cada
sesión nueva releyó todo desde cero, repitió cosas ya hechas, y el backlog
vivía en la cabeza de alguien. Por eso:

1. **El backlog está en el repo, en un fichero, con estados.** Nada de "lo
   tengo en la cabeza" ni "quedó en el chat anterior". (§2: la memoria en
   disco es también la forma de no gastar tokens dos veces.)
2. **Todo lo que se empieza, se anota antes de empezar.**
3. **Al terminar, se actualiza el estado** (hecho / en curso / bloqueado y
   por qué). Un ítem que no se actualiza cuenta como **abandonado**.

Ficheros de referencia (los tres son la fuente de verdad):

| fichero | para qué |
|---|---|
| `docs/IDEAS_BACKLOG.md` | **el índice maestro de TODO lo pedido**, con estado y enlace al detalle |
| `docs/COORDINATION.md` | **el tablón**: quién tiene qué fichero, y el cremallero de mensajes |
| `docs/MASTER_PLAN_V16.md` | la estrategia de largo plazo y las fases |

---

## 1. Nada de "fantasmadas"

Cada afirmación en un informe, commit, mensaje o documento va con:

* una **medición real** (el comando y su salida), o
* una **referencia `fichero:línea`**, o
* la marca explícita **`NO VERIFICADO`** + cómo verificarlo.

Un dato de internet sin abrir la página es `NO VERIFICADO`. Una captura de
pantalla ajena es `NO VERIFICADO`. "Debería funcionar" es `NO VERIFICADO`.

Si algo no se pudo comprobar, **se dice**. Un trabajo honesto a medias vale
mucho más que uno completo y falso: el segundo cuesta días.

---

## 2. Eficiencia de tokens: automatizar y reducir el consumo, sin perder eficacia

> Imperativo del usuario (2026-11-26): *«hacer análisis y todo tipo de
> procesos ultra automatizados pero ultra eficientes con técnicas de reducción
> masiva de consumo de tokens pero maximizando eficacia y optimizando siempre
> para aumentar eficacia y reducción de consumo de tokens en esto, e
> implementación de todo lo empíricamente comprobado de forma óptima en
> MegaDaemon»*.

El objetivo **no** es "gastar menos tokens" por sí mismo: es **no volver a
pagar dos veces por la misma información**. Un token gastado en releer un
fichero que ya se procesó es un token que no se gasta en encontrar el bug
siguiente.

### 2.1 Reglas

1. **Un script, no un volcado.** Si hay que mirar 4.000 líneas, el script
   devuelve las 20 que importan. El agente lee la **salida del script**, no el
   fichero.
2. **Filtros en la propia herramienta**, no en el contexto: `grep`, `head`,
   `--max-count`, `| tail`, `jq` para lo estructurado. Lo que no se imprime,
   no se paga.
3. **Salida compacta y estable** en todo lo automatizado: JSON con campos
   fijos y un resumen de una línea, nunca el volcado crudo. Un informe de
   10.000 tokens se reduce a 40 con las cifras y los `fichero:línea`.
4. **Memoización de lo caro**: cachear en disco lo que ya se midió (hashes,
   inventarios, pools de strings) y reutilizarlo. Si un cálculo de 8 minutos
   se repite porque nadie guardó el resultado, es un bug de proceso.
5. **Una sola pasada**: si hay que analizar N ficheros, un bucle que emite una
   tabla, no N llamadas que devuelven N volcados.
6. **El resultado del análisis va a un fichero**, no al chat: el chat lleva el
   resumen y el `fichero:línea`; el detalle se consulta bajo demanda.
7. **Nada de releer lo que ya está en el repo**: antes de abrir un fichero,
   `git log`/`grep` para saber si ya se leyó. El backlog (§0) es la memoria.
8. **Delegar lo acotado**: un subagente con un brief de 10 líneas hace el
   trabajo largo y devuelve 3 líneas. El contexto del root no se llena con los
   detalles de un barrido de 300.000 líneas.

### 2.2 Lo que ya está medido en este repo (y hay que reusar)

| Técnica | Dónde | Qué ahorra |
|---|---|---|
| Lectura de NDJSON por UID + barrido acotado | `tools/leer-diagnostico.py` | no leer 473 líneas de instrumentación en crudo |
| Publicación nativa a Drive desde Rust | `drive_vfs::write_local_file`, `drive_native::*` | no pasar por `curl` ni por el chat |
| Cola y bus como protocolo (no chats) | `task_scheduler.rs`, `resonance.rs` | no re-transportar contexto entre agentes |
| Un solo `bus_ejecutor::ciclo()` en vez de N llamadas | `bus_ejecutor.rs` | un ciclo = un ciclo de contexto |
| Autodetección de "listo" | `telemetria::estado_listo()` | no preguntar "¿ya estás?" |
| Informes con `NO VERIFICADO` explícito | esta orden | no discutir sobre lo que nadie comprobó |
| Núcleo forense propio en C (48 KB, sin deps) | `tools/mddoctor.c` | diagnostica en **<1 s** donde el build de Rust se muere por RAM |
| Diagnóstico del móvil en C (sin `curl` ni `wget`) | `mddoctor salud` | el shell de Android no trae esas herramientas |
| Tool MCP `doctor` (diagnóstico + recuperación) | `mcp_ext.rs` → `doctor.rs` | diagnosticar por la API, sin ssh ni ADB |
| Tablón del enjambre en un comando | `tools/estado.sh` | no releer el repo para saber dónde está el trabajo |
| Ejecutar un binario ARM64 de verdad | `tools/verificar-arm64.sh` (+ `qemu-aarch64-static`) | "compila" ⇒ "funciona", sin esperar al CI |

### 2.3 Lo que NO se optimiza

- **La corrección.** Reducir tokens nunca justifica saltarse una verificación.
  Si un test cuesta 40 s, se tarda 40 s.
- **La seguridad.** Nada de "es sólo un dato" para subirlo a un repo público.
- **La honestidad del informe.** Un resumen corto que miente es peor que
  un volcado largo.


## 3. No saturar el entorno

El sandbox tiene **2 CPU y 3 GB de RAM sin swap**; un `cargo test --lib` mata
el enlazador (OOM). Reglas obligatorias:

| regla | cómo |
|---|---|
| **1 build por agente** | `CARGO_TARGET_DIR` exclusivo por agente (`/workspace/md-target-<agente>`): el `target/` compartido se corrompe |
| **Nunca en paralelo lo pesado** | un `cargo` a la vez en toda la flota; los demás esperan |
| **Builds con `-j 1`** | `CARGO_BUILD_JOBS=1 CODEGEN_UNITS=16`, LTO apagado |
| **Comprobar antes de lanzar** | RAM libre, carga, espacio en NFS y número de subagentes vivos, **antes** de arrancar trabajo pesado |
| **Los subagentes esperan** | si no hay recursos, **no arrancan**: se documenta el bloqueo en vez de competing |
| **Nada de instalar paquetes pesados** | el repo no puede depender de la red de pip/npm |

Si una tarea no cabe en los recursos actuales, se **documenta el bloqueo**
(`⛔ bloqueado: falta RAM`) y se sigue con otra. Perder la sesión entera
por un OOM es el peor resultado posible.

---

## 4. Coordinación: no pisar el trabajo de otro

Las reglas de `docs/COORDINATION.md` siguen vigentes. En resumen:

1. **Un agente = un módulo Rust.** Nadie toca `lib.rs`, `main.rs`,
   `Cargo.toml` ni el módulo de otro sin acuerdo en el tablón.
2. **El root es el único integrador.** Los subagentes proponen ficheros; el
   root cablea, commitea y publica.
3. **`git add <ruta exacta>`**, nunca `-A` ni `.`. Un commit = un ámbito.
   Nunca `--force`, nunca reescribir `main`.
4. **Doble auth** en lo que toque el dispositivo: `X-API-Key` + `X-Seere-Auth-Token`.
   Toda acción irreversible exige `confirm: true`.
5. **Gate antes de commitear**: `bash tools/check-sintaxis.sh` (debe dar 0
   errores) y `python3 tools/yaml_lint.py .github/workflows/*.yml`.

---

## 5. Secretos: extraer sí, subir no

* El usuario **pide** extraer credenciales para su uso. Se pueden extraer.
* El repo es **PÚBLICO**. **Nada** de credenciales, tokens ni datos del
  dispositivo va a GitHub, ni en un commit, ni en un `.env`, ni en un log.
* Los secretos viven en ficheros **0600** fuera del repo (en
  `/workspace/md-secrets/`, en el state dir del móvil) y **nunca se imprimen**
  en una salida: se muestran como `sha16`.
* Los respaldos van a la carpeta **privada** de Drive
  (`15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0`), no a una carpeta compartida.
* **PII tampoco**: MEDIDO (2026-11-26) los 9 PDF de `Facturas jazztel`
  llevan **el NIF del titular en el nombre del fichero**. En cualquier
  informe del repo, documento o mensaje, ese nombre va **enmascarado**
  (`<NIF>_J….pdf`), igual que el resto de datos personales de la factura
  (importe, IBAN, número de contrato). Los originales se quedan en la
  carpeta privada y no se resuben.
* Los ficheros que Google restringió (`com.ketan.romtools21.app`,
  `Facturas jazztel`) **no se vuelven a subir**: el flujo los **omite con
  aviso** en vez de fallar. Pedir la revisión es un formulario web: lo puede
  hacer una persona, no un agente.

---

## 6. Unificar lo que es lo mismo

Antes de crear algo nuevo, **buscar si ya existe**:

* El daemon ya tiene: cliente de Drive **nativo** (`drive_native`), cliente
  HTTP (`http_transfer`), diagnóstico (`diagnostico`), cola de tareas
  (`task_scheduler`), tablero (`board`), bus (`mesh`), autenticación de
  Google (`google_auth`), telemetría (`telemetria`).
* Antigravity, el language server, el CLI de Antigravity, Go y Jetski son
  **el mismo entorno**: un runtime, un bus, un modelo de credenciales.
  Lo que se toca en uno se toca en los otros.

**Un módulo nuevo sólo si no existe otro que haga lo mismo**, y sólo entonces
con su entrada en `docs/IDEAS_BACKLOG.md`.

---

## 7. El canal manda: nada de conexiones dependientes de un humano

MEDIDO (2026-11-26): el *quick tunnel* de Cloudflare **cambia de hostname en
cada relanzamiento** y el relé canónico se cae ⇒ el sandbox se queda ciego y
la única salida es que alguien se conecte a mano. Eso **no puede** ser el
mecanismo normal.

**Regla**: el sistema se sostiene **sin que el root esté conectado**.

* El nodo **empuja** su estado a Drive (`telemetria::publicar_periodico`).
* El root **tira** de él cuando quiera (`tools/leer-diagnostico.py`).
* Los datos de conexión (URL, puerto, build, `sha16` de las claves) se
  publican **junto** con el estado: quien lee el estado, sabe a dónde
  conectarse.
* La instrumentación y los artefactos se suben a **Drive**, no a GitHub.

Un agente que necesita "que alguien mire el móvil" para avanzar, **no ha
terminado**: le falta el push o el pull.

---

## 8. Batería: el móvil es un recurso compartido y limitado

MEDIDO: el móvil estaba al 14 % de batería con la carga al 98 % de RAM.

* Todo lo que use red o CPU en el móvil **respeta el modo ahorro**
  (`battery_advanced::modo_ahorro_activo()`): el periodo se duplica.
* Nada de bucles cerrados y agresivos contra el móvil.
* Nada de "golpear al maestro por el túnel" desde varios agentes a la vez:
  cada petición MCP crea procesos (`su -mm -c sh -c …`) y el móvil lo nota.

---

## 9. Qué se espera de un agente al terminar

1. **Estado real** en el backlog: `✅ hecho` / `🟡 en curso` / `⛔ bloqueado
   (por qué)`.
2. **Evidencia**: el comando y su salida, o `fichero:línea`.
3. **Un mensaje** en el cremallero de `docs/COORDINATION.md` (al final; no se
   editan filas ajenas).
4. **Los huecos que quedan**, nombrados: lo que no se hizo y por qué.

---

## 10. La frase que resume todo

> **Apunta todo en documentos, no pierdas el hilo ni el foco, automatiza y no
> repitas trabajo (que es lo que cuesta tokens), no satures la máquina,
> coordina antes de tocar, no subas secretos a GitHub, y deja el sistema
> funcionando aunque nadie esté mirando.**
