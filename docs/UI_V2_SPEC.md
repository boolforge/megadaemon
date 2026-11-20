# UI v2 del cockpit — diseño y flujo de datos

> Especificación de `web/cockpit_v2.html` + `web/webui_v2.css`.
> Companion de `docs/API_V2.md` (el contrato de datos) y de
> `docs/COORDINATION.md` (el tablón compartido).
>
> **Estado**: implementado y compilando.  **Pendiente de cableado**: publicar la
> página (`/v2`) y registrar `pub mod webui;` en `lib.rs` (ver `API_V2.md` §2).

---

## 1. Qué cambia respecto a v1 y por qué

La v1 (`web/cockpit_template.html`, 1.779 líneas) es un cockpit de **una sola
cosa**: el chat con el agente y un puñado de herramientas RE.  Todo lo demás
(RE, NPU, cámara, bidders) se pintó después, sin sistema de diseño.

| # | Queja del usuario | Diagnóstico | Cambio en la v2 |
|---|---|---|---|
| 1 | "es una mierda" / "muy feo y muy penoso" | HTML de 88 KB con `<style>` en línea, valores sueltos, sin modo claro, sin tokens | `web/webui_v2.css`: sistema de diseño con tokens (paleta, tipografía fluida, espaciado 4/8, radios, sombras), **modo claro/oscuro** (`prefers-color-scheme` + toggle manual), componentes (cards, chips, tablas, badges, tabs, toasts, modales, formularios), foco visible, AA, `prefers-reduced-motion` |
| 2 | "le faltan muchísimas cosas" | 5 vistas (chat, RE, NPU, bidders, cámara) | **10 vistas**: Resumen, Nodos, Tablero, Agente, Delegación, Alertas, RE/Apps, Archivos, Reparto, Sistema |
| 3 | "no muestra nodos conectados" | no existía endpoint de nodos | vista **Nodos** con `GET /api/v2/nodes` + 3 acciones reales (asignar / mensaje / migrar) |
| 4 | "no busca los LLM de forma agéntica" | el chat mandaba un prompt y pintaba la respuesta | vista **Agente**: objetivo → plan (con **origen**: LLM local o heurística, y por qué) → ejecución; catálogo real de proveedores con **ping** y **consumo por proveedor** |
| 5 | "falla muchisimo" | ~20 fetch por vista, cada uno una fuente de fallo | **1 fetch por vista** (agregadores `/api/v2/*`); los errores se muestran como estado, no como pantalla rota; auth tolerante (401 → prompt de API key → reintento) |
| 6 | "es un triste chatbot" | sin coordinación, sin auto-mejora, sin ficheros compartidos | **Tablero de tareas** (con export/import para el sistema distribuido), **bucle agéntico local** (5 fases, notas, ajustes), **ficheros compartidos** con vault + `MANIFEST.ndjson` + sha256 |
| 7 | "colaboración entre agentes" | inexistente | el bus es de lectura/escritura real: `mesh` (tablón) + `coordination` (SQLite con leases); las acciones escriben de verdad en el tablón |
| 8 | "colaboración con compartición de archivos" | inexistente | **Archivos compartidos**: subida por chunks, hash por trozo y por fichero, manifiesto NDJSON, blobs direccionados por hash (dos nodos no se pisan) |

Lo que **NO** cambia: la v1 sigue viva y sirve las mismas rutas.  La v2 es
aditiva; el `dispatch` de `webui` sólo atiende `/api/v2/*`.

---

## 2. Sistema de diseño (`web/webui_v2.css`)

### 2.1 Tokens

* **Paleta**: escala de 10 pasos (`--bg`, `--bg-elev`, `--bg-elev-2`,
  `--bg-inset`, `--surface`, `--surface-hover`, `--border`,
  `--border-strong`) + texto (`--text`, `--text-muted`, `--text-dim`) +
  acentos semánticos (`--accent`, `--info`, `--warn`, `--danger`, `--violet`)
  con su variante `-soft` (fondos translúcidos).
* **Tipografía fluida** con `clamp()`: `--fs-xs` … `--fs-metric` (métricas de
  26→40 px).  Números siempre con `tabular-nums` (las columnas de cifras
  bailan si no).
* **Espaciado** en múltiplos de 4: `--sp-1` … `--sp-7`.
* **Radios** `--r-xs` … `--r-pill`; **sombras** `--shadow-1` … `--shadow-3`.
* **Objetivo táctil** `--tap: 44px` (mínimo, no negociable en un móvil).

### 2.2 Modo claro / oscuro

```css
:root { color-scheme: dark; /* … */ }          /* por defecto */
@media (prefers-color-scheme: light) {
  :root:not([data-theme="dark"]) { /* … */ }    /* el sistema manda */
}
:root[data-theme="light"] { /* … */ }           /* el usuario manda */
:root[data-theme="dark"]  { /* … */ }           /* el usuario manda */
```

El toggle escribe `data-theme` en `<html>` y lo guarda en `localStorage`
(`md_theme`): `auto` → `dark` → `light` → `auto`.  Con `auto` se **quita** el
atributo y decide el sistema.

### 2.3 Mobile-first

* Layout de una columna; a partir de **900 px** la barra de pestañas se
  convierte en rail lateral y las rejillas pasan a 2-3 columnas
  (`repeat(auto-fit, minmax(…, 1fr))`).
* `100dvh` + `env(safe-area-inset-*)` para no quedar bajo el notch ni el
  gesto de la barra de gestos.
* Barra de pestañas horizontal con scroll y *snap* (9 vistas no caben en 5
  iconos sin sacrificar legibilidad).
* Tablas con scroll horizontal (`overflow-x: auto`), cabeceras `sticky`.

### 2.4 Accesibilidad

* Foco visible siempre (`:focus-visible` con doble anillo) y **nunca**
  `outline: none` sin sustituto.
* `skip-link` al contenido, `aria-label` en los botones de sólo icono,
  `role="tab"/"tabpanel"` en la navegación, `aria-selected` en tabs y en la
  command palette, `aria-live="polite"` en los toasts, `role="dialog"` +
  `aria-modal` en modales y palette.
* Contraste AA en texto y en los bordes de foco (tokens validados a mano para
  los dos modos).
* `prefers-reduced-motion: reduce` desactiva animaciones y transiciones.
* `prefers-contrast: more` refuerza los bordes.
* Navegación por teclado completa: `Ctrl/Cmd+K` (palette), `Esc` (cierra),
  flechas + `Enter` en la palette, `R` para refrescar (fuera de campos).

### 2.5 Cero dependencias

Ni CDN, ni fuentes remotas, ni librerías JS.  Los iconos son SVG inline; el
SHA-256 del cliente está implementado en JS puro (≈60 líneas) para que
`crypto.subtle` **no** haga falta (en `http://192.168.x.x` no existe
`crypto.subtle`: el navegador sólo lo da en contexto seguro).

---

## 3. Arquitectura de la UI

```
┌─ appbar ───────────────────────────────────────────────────────────┐
│ marca + estado │ nodo activo │ bus │ rol │ reloj │ tema │ ⟳ │ 🔍  │
└───────────────────────────────────────────────────────────────────┘
┌─ views ───────────────────────────────────────────────────────────┐
│  view-resumen   view-nodos    view-tablero  view-agente           │
│  view-delegacion view-alertas  view-re      view-archivos         │
│  view-sistema                                                     │
└───────────────────────────────────────────────────────────────────┘
┌─ tabbar (9 pestañas, scroll) ─────────────────────────────────────┐
```

* **Estado global** en un objeto `state`: API key, token, tema, vista activa,
  nodo activo, última respuesta de cada vista, marca de sincronización.
* **Caché por vista**: `state.data.<vista>` evita repintar de más y permite
  que un modal use datos ya cargados (p. ej. "aplicar ajustes" usa
  `state.data.delegacion.adjustments`).
* **Auto-refresco**: cada 30 s se recarga **la vista activa** (y sólo si la
  pestaña del navegador está visible: el móvil no puede gastar batería
  refrescando en segundo plano).  El reloj de la cabecera muestra la última
  sincronización.
* **Errores**: cada vista captura su fallo y lanza un toast con el mensaje
  real (`HTTP 403`, `hash_mismatch`…).  La vista **no** se queda en blanco:
  conserva lo último que se pintó.

---

## 4. Las 9 vistas

### 4.1 Resumen — `view-resumen`

* **Endpoint**: `GET /api/v2/dashboard` (1 fetch).
* **Muestra**: uptime del daemon, % de CPU (muestra real de 120 ms sobre
  `/proc/stat`), RAM usada/total, batería %, temperatura de batería y CPU,
  thermal hot, governor, política de energía, estado del NPU (soc, VS4L,
  NNAPI, EDEN y si el offload está implementado de verdad), alertas activas
  (crit/warn/info), tamaño de la cola por estado, capacidades sanas,
  proveedores LLM listos, ficheros compartidos, y el **estado real del bus**
  con su `reason` (si no está desplegado, un aviso ámbar con el path que
  falta).
* **Interacción**: botones "Ver todas" / "Ver tablero" saltan a las vistas
  correspondientes.

### 4.2 Nodos — `view-nodos`

* **Endpoint**: `GET /api/v2/nodes`; acciones `POST /api/v2/nodes/assign`,
  `POST /api/v2/nodes/message`, `POST /api/v2/nodes/migrate`.
* **Muestra**: estado del bus (desplegado / no desplegado + motivo +
  `board_path`), lista de instancias con **rol** (primary/worker/observer),
  dispositivo, plataforma, versión, **carga**, **capacidades** y **último
  latido**; tabla de tareas publicadas en el tablón con su claim y su
  propietario; eventos del tablero SQLite si existe.
* **Acciones**:
  * *Asignar tarea*: nodo + tipo + capacidad + payload JSON → se publica en
    el tablón del bus (y en el tablero SQLite si está desplegado).
  * *Enviar mensaje*: texto + datos en base64 → buzón del nodo
    (`<bus>/inbox/<nodo>.ndjson`) + tarea de capacidad `message`.
  * *Migración*: plan de migración con el primary el último.
* **Si el bus no está desplegado**: la lista queda vacía **a propósito** y se
  muestra el motivo; las acciones devuelven `bus_not_deployed`.

### 4.3 Tablero de tareas — `view-tablero`

* **Endpoints**: `GET /api/v2/tasks`; acciones `POST /api/tool/task_queue`
  (submit/run/cancel/retry) y `POST /api/v2/tasks/export` /
  `POST /api/v2/tasks/import` (reparto entre instancias).
* **Muestra**: total, pendientes, en curso, hechas, fallidas y entradas de
  journal; tabla con **id, tipo, prioridad, estado, dependencias, nodo,
  intentos** y acciones por fila (cancelar / reintentar); journal de
  auditoría (últimas 30 entradas); si el tablero SQLite está desplegado, su
  estado.
* **Acciones**: encolar (tipo + prioridad + payload JSON), ejecutar lote,
  exportar (descarga JSON), importar (fichero JSON).

### 4.4 Agente — `view-agente`

* **Endpoints**: `GET /api/v2/llm`; `POST /api/tool/agent` (plan/run);
  `POST /api/providers/test` (ping).
* **Muestra**: campo de objetivo + switch "preferir LLM local"; el plan
  resultante con **el origen** (`local-seere` vs `heurística`) y **las notas
  del planificador** (por qué ha elegido esas tareas); el catálogo real de
  proveedores con estado *listo/sin key* y botón **Probar** (ping real,
  muestra respuesta, latencia y si hubo *fallback*); el **consumo por
  proveedor** (tokens de entrada/salida, coste y tasa de éxito por cuenta).
* **Honestidad**: si no hay proveedores en el vault, la vista dice que el
  agente caerá en el planificador heurístico (0 tokens) en lugar de fingir
  una lista.

### 4.5 Delegación — `view-delegacion`

* **Endpoints**: `GET /api/v2/delegation`; `POST /api/v2/delegation/apply`.
* **Muestra**: cuentas (proveedor, modelo, nivel, **cuota restante**,
  **presupuesto diario**, **score** de calidad, activada), **plan de reparto**
  (qué tarea va a qué cuenta, tokens y coste estimados y **por qué**), y los
  **ajustes que propone el motor** (`delegate::adjustments`) con su razón.
* **Acciones**: "Aplicar todos" (todo o nada, con modal de confirmación).
* **Si el motor no está desplegado**: aviso con el path del fichero que
  falta y las cuentas/ajustes vacíos (no inventados).

### 4.6 Alertas — `view-alertas`

* **Endpoints**: `GET /api/v2/alerts`; `POST /api/tool/alerts`
  (check/ack/prune).
* **Muestra**: activas / críticas / avisos / info / emitidas y la **ventana de
  dedup**; el **canal** de entrega (si hay webhook, su host — nunca la URL
  completa, puede llevar credenciales); la lista de alertas con severidad,
  regla, **contador de dedup** (`×3`), cuerpo, timestamps y botón **Acusar**.
* **Acciones**: evaluar reglas, acusar una alerta.

### 4.7 RE / Apps — `view-re`

* **Endpoint**: `GET /api/v2/re`; `POST /api/tool/apk_inventory` (analyze),
  `POST /api/tool/root_stealth` (status).
* **Muestra**: inventario de **APKs** (paquete, versión, tamaño, sha256),
  estado de la **decompilación DEX**, **señales anti-root** (targets del
  perfil de ocultación), **planes de export/import** (cola, repos, volcado de
  APKs) y el **vault de firma** (ficheros de clave con su hash).
* **Si no hay inventario** (PC sin root): estado vacío con el motivo real.

### 4.8 Archivos compartidos — `view-archivos`

* **Endpoints**: `GET /api/v2/share`, `GET /api/v2/share/get?path=…`,
  `POST /api/v2/share/put`, `POST /api/v2/share/chunk`,
  `POST /api/v2/share/commit`.
* **Muestra**: inventario del vault con **sha256** por fichero y su estado
  real (`verificado` / `corrupto` / `ausente`), el contenido del
  `MANIFEST.ndjson` y el path del vault en disco.
* **Subida**: por trozos de 64 KiB (cada trozo con su hash; el commit verifica
  el hash del fichero completo) o de una vez.  El progreso se muestra en
  vivo.
* **Descarga**: `GET` con verificación; el toast dice si el hash cuadró.

### 4.9 Reparto — `view-reparto`

* **Endpoints**: `GET /api/v2/market`, `GET /api/v2/market/rules?node=…`,
  `POST /api/v2/market/rules`, `POST /api/v2/market/route`.
* **Qué resuelve**: el usuario pidió "una forma de distribuir trabajo entre
  los nodos: inferencia, compilación, cómputo, red… cada nodo debe
  contribuir, automatizado por reglas locales según lo que el dispositivo
  pueda dar".  Esta vista es ese motor, hecho visible.
* **Muestra**:
  * **Oferta**: por nodo, los tipos de trabajo que ofrece (`inference`,
    `build`, `compute`, `network`, `storage`), la **capacidad libre** que
    ofrece y **por qué** comparte o no (batería, cargador, temperatura,
    modo *dedicada*, concurrencia).
  * **Demanda y plan**: cada tarea pendiente con su tipo de trabajo, el nodo
    que debería cogerla, la puntuación y **la razón** del reparto; y los
    candidatos descartados con su motivo.
  * **Contribución**: tareas hechas/falladas por nodo, % del total y
    veredicto; los nodos vivos con capacidad libre **sin trabajo** aparecen
    como "deben contribuir".
  * **Mis reglas locales**: editor de las reglas del nodo activo
    (dedicada, sólo con cargador, compartir inferencia, batería mínima,
    temperatura máxima, concurrencia, fracción reservada, sesgo de
    prioridad y tipos ofrecidos).
  * **Estado del dispositivo** que manda en las reglas (batería, carga,
    temperatura, política de energía).
* **Acciones**: guardar reglas (validadas por el daemon), cargar las reglas
  de un nodo y "Repartir" una demanda concreta (el motor elige nodo y
  explica por qué; con `publish` también se publica en el bus).
* **Sin nodos**: la vista lo dice y no inventa oferta.

### 4.10 Sistema — `view-sistema` `view-sistema`

* **Endpoints**: `GET /api/v2/agentic` y `GET /api/v2/routes`.
* **Muestra**: el **bucle agéntico local** en sus 5 fases (observar →
  planificar → ejecutar → medir → ajustar) con la fuente real de cada fase,
  las **notas de aprendizaje** que deja el bus y los **ajustes pendientes**;
  la **tabla completa de la API v2** (autogenerada por el daemon) y la
  **política de auth**.

---

## 5. Cabecera global

| Elemento | Fuente | Comportamiento |
|---|---|---|
| Marca + punto de estado | `bus.deployed` | verde = bus desplegado, ámbar = no desplegado |
| Selector de nodo activo | `nodes[]` | fija el destino de las acciones del bus; se refleja en los formularios |
| Chip "bus" | `bus.deployed`, `bus.peers_alive` | "bus: 2 vivo(s)" / "bus: no desplegado" |
| Chip de rol | `bus.master` | "maestra" / "rol: worker" |
| Reloj | `state.lastSync` | hora local + marca de la última sincronización |
| Tema | `state.theme` | auto → claro → oscuro |
| Refrescar | — | recarga la vista activa |
| Command palette | `Ctrl/Cmd+K` | navegación + acciones |

**Command palette** (14 acciones): 9 de navegación + refrescar + evaluar
reglas + ejecutar lote + exportar cola + cambiar tema + cambiar API key.
Filtra por texto, se navega con flechas y `Enter`, se cierra con `Esc`.

---

## 6. Autenticación en la UI

1. `localStorage["md_api_key"]` → cabecera `X-API-Key`.
2. `sessionStorage["md_token"]` (o `window.MEGADAEMON_TOKEN`, token embebido
   por el servidor en los binarios que lo hacen) → cabecera
   `X-Seere-Auth-Token`.
3. Un **401** (sólo 401) dispara el prompt de API key → `POST
   /api/session/exchange` → reintento de la misma llamada.
   * El canje **exige un token real** (`ok === true` y `token` no vacío): un
     `200 {"status":"executed"}` del dispatcher genérico **no** es una sesión.
   * Si ya hay sesión, **no** se pregunta nada.
   * Como mucho se pregunta **una vez**: si el canje falló, la UI no insiste.
   * Un **403** (token que falta) o un **404** (endpoint inexistente) **no**
     abren el prompt: son errores de otra naturaleza.
4. **Diagnóstico adaptativo** al arrancar (chip en la cabecera + banner en
   Resumen), con tres estados:
   * `sesión: ok` — canje correcto y lecturas bien.
   * `sólo lectura` — el canje falla pero la lectura va: las escrituras
     fallarán (UI-16 abierto).
   * `sin sesión` — ni canje ni lectura: el daemon exige token de sesión.
     El mensaje **nombra el endpoint que falló** y su código.
   La sonda de lectura es `GET /api/status` (ligero, pasa por el gate).
5. Los errores de cualquier llamada se muestran con **endpoint + código**:
   `"/api/v2/llm → HTTP 403 · falta la cabecera X-Seere-Auth-Token"`.

La API key se guarda **sólo** en el navegador y sólo se envía al propio
daemon.  La UI nunca la imprime en logs ni en la URL.

---

## 7. Flujo de datos (resumen)

```
navegador                         daemon (cockpit_server + webui)
    │  GET /api/v2/dashboard  ──────────▶  dashboard()   ─┐
    │  GET /api/v2/nodes      ──────────▶  nodes()        │
    │  GET /api/v2/tasks      ──────────▶  task_board()   │  un fetch
    │  GET /api/v2/llm        ──────────▶  llm_catalog()  │  por vista
    │  GET /api/v2/delegation ──────────▶  delegation()   │
    │  GET /api/v2/alerts     ──────────▶  alerts_view()  │
    │  GET /api/v2/re         ──────────▶  re_apps()       │
    │  GET /api/v2/agentic    ──────────▶  agentic_loop_status()
    │  GET /api/v2/share      ──────────▶  shared_files() │
    │  GET /api/v2/routes     ──────────▶  routes_doc()   ─┘
    │                                            │
    │  ◀────── JSON con ok/error/reason ─────────┘
    ▼
 render(view) → HTML con escape + toasts + estado real
```

Las **escrituras** van al dispatcher genérico de la v1 (`/api/tool/<nombre>`)
o a los POST de la v2 (`nodes/*`, `share/*`, `delegation/apply`,
`tasks/export|import`), y siempre terminan en un toast con el resultado real.

---

## 8. Qué se ha hecho / qué no (honestidad)

**Hecho**

* Sistema de diseño completo con modo claro/oscuro y a11y.
* 10 vistas con datos reales (0 mocks).
* API v2 agregadora (24 rutas) con `dispatch` listo para cablear.
* Reparto de trabajo entre nodos con reglas locales y equidad (todo nodo debe contribuir).
* Verificación de hash real en los ficheros compartidos (ida y vuelta).
* Tests de Rust que atan la UI a la tabla de rutas en las dos direcciones.

**No hecho (y por qué)**

* **No se ha tocado `cockpit_server.rs`** (conflict map): publicar `/v2` y
  llamar a `webui::dispatch` son 4 líneas que hace el dueño del fichero
  (`docs/API_V2.md` §2).  Hasta entonces la v2 se sirve como fichero estático
  y sus endpoints no existen.
* **No se ha tocado `lib.rs`**: falta `pub mod webui;` (1 línea, misma razón).
* **El bus de coordinación no se despliega desde la UI**: la vista dice el
  estado real; el despliegue es una decisión de despliegue, no de interfaz.
* **Sin tests de navegador**: no hay Playwright/Selenium en el repo.  La
  lógica de hash del cliente se ha verificado contra `hashlib` con 9
  vectores (vacío, 1, 3, 55, 56, 63, 64 y 1000 bytes).
* **Sin PWA/offline**: la UI necesita el daemon (es un cockpit, no una app
  offline).  El CSS no depende de la red, pero los datos sí.
