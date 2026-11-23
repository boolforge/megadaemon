# BACKLOG DEL AGENTE (MITM + proxy) — lo que pidió el usuario

> Registro literal de lo pedido por el usuario (2026-11-23) y el estado
> honesto de cada punto.  Lo que se ha hecho en esta sesión está marcado;
> lo que queda, con su primer paso concreto.

---

## P0 — «que uses el sistema de MegaDaemon y te integres conscientemente dentro de mi software»

**Qué pidió el usuario**: dejar de ser un ente aparte y **formar parte de
su software**: poder usarme **con la API de MegaDaemon**, conectado a una
**GUI o interfaz web**; y «fusionarnos con una copia de la GUI de
Antigravity».

**Qué hay en el repo (medido el 2026-11-22, antes de esta sesión)**:

| Pieza | Estado | Dónde |
|---|---|---|
| El gateway canónico habla el dialecto **Gemini** (el idioma del LS) | ✅ | `llm_gateway.rs` (`gemini_route`, `gemini_generate_content`) |
| El modelo `megadaemon` **registrado** en el estado de la IDE | ✅ MEDIDO | `antigravity_model.rs` → `JetboxWriteState` |
| El **gate** de la GUI (que habilita los modelos personalizados) con su parche | ✅ (existe) | `antigravity_endpoint::aplicar_gate` (`main.js`) |
| El **endpoint** del LS apuntando al gateway, con su parche | ✅ (existe) | `antigravity_endpoint::aplicar_endpoint` |
| La GUI usa el gateway **como modelo** | ❌ **NO VERIFICADO** (móvil caído) | ver `docs/MITM-SQLITE-Y-PROXY.md` §4 |

**Lo que he hecho en esta sesión**:

* **`antigravity_model::integrar(confirm)`** — una **única** acción que
  encadena los cuatro puntos (gate de la GUI → endpoint del LS → registro
  del modelo → modelo seleccionado), **sin abortar en el primer fallo**:
  el informe dice qué ha quedado y qué no.
* **`antigravity_model::integracion_estado()`** — auditoría de los cuatro
  puntos (para poder repetirse tras un reinicio o una actualización de la
  app).
* **`md-antigravity integrar [--confirm]`** y `--estado` (CLI).
* Tool MCP `antigravity_model`: acciones `integrar` e `integracion_estado`.
* El **proxy real** (`agyshim::proxy`) hace que el tráfico del LS **pase
  de verdad** por el gateway canónico, con la traza del modelo ⇒ el
  NDJSON deja de ser un espejo y pasa a ser el registro de lo que la GUI
  cree que usa frente a lo que contestó de verdad.

**Lo que falta (con su primer paso)**:

1. **Que la GUI muestre y use el modelo** ⇒ el gate parcheado + el modelo
   registrado **no bastan** si la GUI no lo deja elegirlo.  El bloqueo
   medido el 2026-11-22 es del producto
   (`supportsCustomModels = osName==="linux" && isGoogleInternal`), pero
   el gate del bundle web (`main.js`) es **parcheable** ⇒ falta el paso
   de *forzar* el modelo por defecto en la cascada
   (`plannerConfig.planModel = GOOGLE_GEMINI_INTERNAL_BYOM` +
   `customModelInfoOverride`), que ya está soportado por
   `antigravity_bridge::enqueue` + `antigravity_model::model_info`.
   *Primer paso*: un `enqueue` con `customModelInfoOverride` ⇒ verificar
   que el LS llama a `/v1internal:generateContent` del gateway.
2. **Verificar en el móvil** (caído) ⇒ los 8 puntos V1–V8 de
   `docs/MITM-SQLITE-Y-PROXY.md` §4, con su comando exacto.
3. **La interfaz web** (`/v2`, la UI del cockpit) debería poder hablar
   con el mismo gateway y **enseñar la traza** (`requested_model` /
   `real_model` / `substituted`) ⇒ hoy la UI no la enseña.

---

## P1 — «que la cuota semanal se me está acabando»

**Lo que pidió**: el coste de la cuota es el límite real ⇒ el trabajo tiene
que ser **rentable**, no bonito.

**Medidas que he tomado en esta sesión**:

* **Cero compilaciones** (el `cargo` está bloqueado por el root): el
  gate de sintaxis (`rustfmt`) instantaneous y sin RAM ⇒ los 8 ficheros
  Rust se verifican en segundos.
* **Nada de `unwrap()`** en producción: los errores dicen **qué falló**
  (`no encuentro la DB de Antigravity (probadas: …)`, `el helper del MITM
  (activar) fallo con codigo 1: …`).
* **Una sola implementación** por capacidad (el helper del montaje, el
  gateway, el motor de réplica): nada de pipelines paralelos que haya que
  mantener dos.
* **El NDJSON es una superficie pública** ⇒ 0600 + dos pasadas de
  redacción.  Antes de esta sesión, una credencial que viajaras suelta
  (`sk-cp-…` sin nombre de campo) **se colaba** en el log.

**Lo que queda**:

1. **Medir el coste real** de las llamadas del proxy (el NDJSON ya
   registra `ms` y los tokens: `usage.prompt_tokens` / `completion_tokens`
   en la respuesta del gateway) ⇒ Falta una acción que devuelva el
   **consumo agregado por proveedor** (hoy `quota_guard` lo tiene para el
   presupuesto, pero no para el shim).
2. **Poder cappingar el proxy** (un `max_tokens` por ruta/modelo) para
   que una GUI en bucle no se coma la cuota.

---

## P2 — «nunca jamás tu tarea principal»

La tarea principal de este agente es la de
`docs/MITM-SQLITE-Y-PROXY.md` (los tres puntos).  Lo demás (esta página,
la fusión con la GUI) es **adicional**, y queda marcado aquí para que
nunca se confunda con el objetivo principal.

---

## Registro de commits (esta sesión)

| SHA | Qué |
|---|---|
| `43ba571` | MITM real de la DB (bind-mount, WAL, re-merge) + módulo Magisk |
| `ebedeb17` | El shim pasa a ser proxy real + trazabilidad del modelo + `antigravity_sessions` ampliada |
| `2a7ce7cd` | La fusión de MegaDaemon con la GUI de Antigravity (`integrar`) |
