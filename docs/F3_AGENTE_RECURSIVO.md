# F3 — LLM agéntico recursivo (meta-agente)

> Referencia única del plan: [`MASTER_PLAN_V16.md`](MASTER_PLAN_V16.md) (fase **F3**).
> Este documento **no** es un plan: es la ficha técnica de lo implementado.

## Qué es

Un bucle **decidir → llamar tool → evaluar → se corregir**, con cuatro cosas
que un bucle "de una llamada" no tiene y que aquí son requisito:

| Pieza | Fichero | Qué garantiza |
|---|---|---|
| Memoria de fallos | `rust-core/src/agent_loop.rs` | `failures.ndjson` (acción, error, qué funcionó). El mismo error **no se repite** en el siguiente ciclo: se consulta antes de actuar y el resumen va en el prompt del modelo. |
| Presupuesto **por hora** | `agent_loop.rs` | Tokens + llamadas + coste con **ventana rodante** (no por acción). |
| Coste real | `agent_loop.rs` (`PriceBook`) | Tokens y latencia **medidos**; el precio **con su fuente**. Sin fuente ⇒ `NO VERIFICADO` (nunca un número inventado). |
| Degradación honesta | `agent_loop.rs` | Si el modelo devuelve basura **no se inventa la acción**: se aplica el criterio fijo (reclamar tarea > callar), igual que el `RuleBrain` de la voz (que no se toca). |
| Meta-agente | `rust-core/src/agent_adapters.rs` | `ServiceBusRouter` enruta la petición al nodo/servicio correcto vía `service_bus::select(TaskType)`. |
| Circuit breaker | `agent_loop.rs` | 3 estados (cerrado/abierto/semiabierto), backoff exponencial y **ventana de recuperación medida**. Si cae el proveedor barato cae al siguiente; **SEERE local es el último recurso** ⇒ el enjambre nunca se queda sin voz. |
| Tope duro por tick | `agent_loop.rs` | El móvil tiene load 17,65 · 128 % de IRQ · 876 procesos y cada `su` crea 2+ procesos ⇒ el bucle **no** puede llamar tools en bucle cerrado sin tope. |
| Shell root persistente | `rust-core/src/persistent_shell.rs` | **Un** shell vivo por socket unix ⇒ **0 procesos por llamada**. Timeout ⇒ mata al hijo, lo marca roto y lo rearma. |

## Restricciones respetadas

* **No toca `providers.rs`**: el backend de chat se inyecta (`ChatBackend`) y
  el adaptador (`ProvidersBackend`) lo *usa* sin modificarlo.
* **El prompt nunca lleva la API key ni el token de sesión** (test propio).
* **No inventa acciones**: si la decisión no es interpretable, criterio fijo.
* **Topes duros**: `max_calls_per_tick` y `max_shell_spawns_per_tick`.

## Tests

* `rust-core/src/agent_loop.rs` → 8 unitarios.
* `rust-core/src/persistent_shell.rs` → 4 unitarios.
* `rust-core/tests/agent_loop.rs` → 7 de integración (uno por criterio de
  aceptación).
* `rust-core/tests/persistent_shell.rs` → 3 de integración.

Comprobación rápida (crate scratch, segundos; el crate completo tarda 20-40 min
en este entorno y hay que compilarlo con `CARGO_TARGET_DIR` en disco local):

```bash
mkdir -p /workspace/f3scratch/src && cd /workspace/f3scratch
cat > Cargo.toml <<'EOF'
[package]
name = "f3scratch"
version = "0.1.0"
edition = "2021"
[dependencies]
serde_json = "1"
EOF
cat > src/lib.rs <<'EOF'
#[path = "/ruta/al/rust-core/src/agent_loop.rs"]
pub mod agent_loop;
EOF
CARGO_HOME=/run/agent-runtime/md/cargo CARGO_TARGET_DIR=/run/agent-runtime/md/target-f3 \
  cargo test --offline -j 2
```

## Cómo se usa

```rust
use megadaemon_core::agent_loop::*;
use megadaemon_core::agent_adapters::*;

let cfg = AgentLoopConfig { max_calls_per_tick: 3, ..Default::default() };
let mut loop_ag = AgentLoop::new(
    cfg,
    FailureMemory::new(AgentLoop::default_failures_path(&state_dir)),
    PriceBook::new().with_source("modelo", 0.0001, 0.0002, "url-de-la-tarifa"),
    vec![ProviderRef::remote("remoto", "modelo"), ProviderRef::local("seere", "local")],
);
let mut backend = ProvidersBackend::default();
let mut executor = ShellToolExecutor::new(shell_persistente);
let mut router = ServiceBusRouter::new(bus);
let informe = loop_ag.tick(now, "instala el apk de prueba", &mut backend, &mut executor, &mut router);
```

## Pendiente

1. **Cablear al daemon**: endpoint `/api/agent` + tool MCP `megadaemon_agent`.
   (Requiere `cockpit_server.rs` / `mcp_server.rs`: lanes de otros agentes.)
2. **Tabla de precios canónica** de la flota (el `PriceBook` exige `source`).
3. **Arrancar el shell persistente** como servicio del supervisor (socket en
   `state/`) cuando haya un consumidor.
