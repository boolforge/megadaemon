//! Patrón 1 — El bucle del agente como **manejador de efectos**.
//!
//! **Fuente medida**: `affogato/agent/effects/{handler,inference,tasks,tools}.rs`
//! (el crate `affogato` del CLI Devin en Rust, medido en
//! `docs/RE_DEVIN_CLI_RUST.md` §2.1) y el contrato del
//! `docs/PATRONES_CLI_RUST_DEFINICION.md` §"Patrón 1" (líneas 15-53).
//!
//! **Contrato**
//! * Entrada: un **evento** (`UserInput`, `ToolResult`, `TaskDone`, `Tick`).
//! * Salida: `(estado', efectos[])` — el reductor **NO ejecuta nada**.
//! * Cada efecto lo despacha **un manejador**, que devuelve el evento
//!   siguiente.
//!
//! ```text
//! bucle(state, evento):
//!     (state, efectos) = reducir(state, evento)
//!     for e in efectos:
//!         evento' = manejar(e)      # inferencia | herramientas | tareas
//!         state = bucle(state, evento')
//! ```
//!
//! Aquí la recursión del pseudocódigo es una **pila explícita** (`bucle`):
//! mismo orden (profundidad, efecto a efecto) sin desbordar la pila del
//! proceso, y con un **tope duro de eventos** que garantiza que termina.
//!
//! **Reglas duras**
//! * [`reducir`] es **PURO**: no llama al modelo, no llama a las tools y no
//!   hace E/S.  Sólo devuelve la lista de efectos.
//! * Quien llama al modelo es [`manejar_inferencia`]; quien llama a las tools
//!   es [`manejar_herramientas`]; quien lanza trabajo asíncrono (y consulta el
//!   criterio fijo) es [`manejar_tareas`].
//! * El mismo bucle sirve para el agente **interactivo**, el **programático**
//!   y el **subagente**: lo único que cambia es el [`Perfil`] (el "modo").
//! * La E/S del diario de fallos la hace el **driver** ([`bucle`]) a través
//!   del [`Diario`], nunca el reductor.
//!
//! **Dependencias**: este módulo usa **sólo** tipos del propio crate
//! (`crate::`), concretamente de `crate::agent_loop` ([`Action`], [`Cost`],
//! [`PriceBook`], [`ChatBackend`], [`ToolExecutor`], [`Router`], …): son los
//! datos que el bucle ya manejaba antes del refactor.  **No** importa nada
//! de `providers.rs` ni de ninguna tool: el modelo y las tools entran por los
//! traits inyectados, así que el módulo se puede testear solo.
//!
//! **Punto de cableado** (medido 2026-11-28, refactor aplicado):
//! `rust-core/src/agent_loop.rs:935` (`AgentLoop::tick`) es el único punto
//! que toca el bucle.  Antes del refactor ese método llamaba al modelo
//! (`backend.chat(...)`, entonces en `agent_loop.rs:941`) y a las tools
//! (`executor.call(...)`, entonces en `agent_loop.rs:1131`) **en línea**.
//! Ahora `AgentLoop::tick` sólo:
//!   1. construye el [`Estado`] puro (`agent_loop.rs:946`),
//!   2. llama a [`bucle`] (`agent_loop.rs:962`),
//!   3. vuelca el estado y devuelve el [`Informe`] (`agent_loop.rs:971`,
//!      `agent_loop.rs:975`).
//! Las llamadas al modelo y a las tools están **sólo** en
//! [`manejar_inferencia`] (`efectos.rs:957`) y [`manejar_herramientas`]
//! (`efectos.rs:980`).
//!
//! **REQUISITO DE INTEGRACIÓN**: este módulo tiene que declararse en
//! `rust-core/src/lib.rs` (`pub mod efectos;`), junto a `pub mod agent_loop;`.
//! Sin esa línea el crate no compila (`agent_loop.rs` usa `crate::efectos`).

use std::collections::HashMap;

use serde_json::json;

use crate::agent_loop::{
    parse_decision, Action, AgentLoopConfig, BreakerState, ChatBackend, ChatOutcome,
    CircuitBreaker, Cost, FailureEntry, HourBudget, PriceBook, ProviderRef, Router, ToolExecutor,
};

/// Modo del agente.  El **mismo** bucle los sirve a los tres: lo único que
/// cambia es el [`Perfil`] (topes y permisos).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modo {
    /// Agente de consola/chat: habla con la persona y puede delegar.
    Interactivo,
    /// Agente programático (sin UI): devuelve un resultado, no habla.
    Programatico,
    /// Subagente: trabajo acotado y **sin** delegar (evita la recursión).
    Subagente,
}

impl Modo {
    /// Nombre estable (informes y logs).
    pub fn as_str(&self) -> &'static str {
        match self {
            Modo::Interactivo => "interactivo",
            Modo::Programatico => "programatico",
            Modo::Subagente => "subagente",
        }
    }
}

/// Topes y permisos del bucle para un modo concreto.
#[derive(Clone, Debug)]
pub struct Perfil {
    /// Modo (informativo: los topes son los que mandan).
    pub modo: Modo,
    /// Tope duro de llamadas al modelo por tick.
    pub max_calls_per_tick: u32,
    /// Tope duro de eventos procesados por vuelta de bucle (evita el cuelgue).
    pub max_eventos: u32,
    /// Tope duro de procesos de shell por tick (el móvil es el recurso escaso).
    pub max_shell_spawns_per_tick: u32,
    /// Ventana (en ticks) en la que un error se considera "repetido".
    pub ventana_fallos_ticks: u32,
    /// ¿Puede el agente delegar en otro nodo?
    pub permite_delegar: bool,
    /// ¿Puede el agente hablar (difusión por el bus)?
    pub permite_hablar: bool,
    /// ¿Se persiste el diario de fallos en disco?
    pub persiste_fallos: bool,
}

impl Perfil {
    /// Perfil por defecto de un modo.
    pub fn for_modo(modo: Modo) -> Self {
        match modo {
            Modo::Interactivo => Perfil {
                modo,
                max_calls_per_tick: 3,
                max_eventos: 64,
                max_shell_spawns_per_tick: 4,
                ventana_fallos_ticks: 5,
                permite_delegar: true,
                permite_hablar: true,
                persiste_fallos: true,
            },
            Modo::Programatico => Perfil {
                modo,
                max_calls_per_tick: 3,
                max_eventos: 48,
                max_shell_spawns_per_tick: 4,
                ventana_fallos_ticks: 5,
                permite_delegar: true,
                permite_hablar: false,
                persiste_fallos: true,
            },
            Modo::Subagente => Perfil {
                modo,
                max_calls_per_tick: 2,
                max_eventos: 32,
                max_shell_spawns_per_tick: 2,
                ventana_fallos_ticks: 3,
                permite_delegar: false,
                permite_hablar: false,
                persiste_fallos: false,
            },
        }
    }

    /// Aplica los topes de la configuración del bucle.  Los permisos del
    /// modo (delegar / hablar / persistir) **no** se tocan: los decide el
    /// modo, no la configuración numérica.
    pub fn con_cfg(mut self, cfg: &AgentLoopConfig) -> Self {
        self.max_calls_per_tick = cfg.max_calls_per_tick;
        self.max_shell_spawns_per_tick = cfg.max_shell_spawns_per_tick;
        self.ventana_fallos_ticks = cfg.failure_window_ticks;
        self
    }
}

/// Evento de entrada del bucle.
#[derive(Clone, Debug)]
pub enum Evento {
    /// Entrada de la persona (o la tarea del turno): abre un tick nuevo.
    UserInput { texto: String, now: u64 },
    /// Latido del planificador: continúa el trabajo pendiente; si **no** hay
    /// nada pendiente no produce ningún efecto.
    Tick { now: u64 },
    /// El modelo respondió (o falló).
    InferenciaTerminada {
        provider: ProviderRef,
        resultado: Result<ChatOutcome, String>,
        now: u64,
    },
    /// Una tool terminó.
    ToolResult {
        accion: Action,
        ok: bool,
        salida: String,
        error: Option<String>,
        now: u64,
    },
    /// Una tarea asíncrona terminó (llega de fuera del bucle).
    TaskDone {
        id: String,
        ok: bool,
        detalle: String,
        now: u64,
    },
    /// El router resolvió una delegación.
    RutaResuelta {
        provider: String,
        task_type: String,
        destino: Option<String>,
        now: u64,
    },
    /// El criterio fijo devolvió una tarea libre (o ninguna).
    CriterioFijo { tarea: Option<String>, now: u64 },
    /// La tarea asíncrona quedó integrada: el bucle puede seguir.
    TareaCerrada { id: String, ok: bool, now: u64 },
    /// Evento sin efecto (un manejador recibió un efecto que no era suyo).
    Inerte { now: u64 },
}

impl Evento {
    /// Reloj lógico del evento (epoch en segundos).
    pub fn now(&self) -> u64 {
        match self {
            Evento::UserInput { now, .. }
            | Evento::Tick { now }
            | Evento::InferenciaTerminada { now, .. }
            | Evento::ToolResult { now, .. }
            | Evento::TaskDone { now, .. }
            | Evento::RutaResuelta { now, .. }
            | Evento::CriterioFijo { now, .. }
            | Evento::TareaCerrada { now, .. }
            | Evento::Inerte { now } => *now,
        }
    }

    /// Nombre estable del evento (informes y logs).
    pub fn as_str(&self) -> &'static str {
        match self {
            Evento::UserInput { .. } => "user_input",
            Evento::Tick { .. } => "tick",
            Evento::InferenciaTerminada { .. } => "inferencia_terminada",
            Evento::ToolResult { .. } => "tool_result",
            Evento::TaskDone { .. } => "task_done",
            Evento::RutaResuelta { .. } => "ruta_resuelta",
            Evento::CriterioFijo { .. } => "criterio_fijo",
            Evento::TareaCerrada { .. } => "tarea_cerrada",
            Evento::Inerte { .. } => "inerte",
        }
    }
}

/// Efecto: una acción que el bucle **propone** y que ejecuta un manejador.
/// El reductor nunca los ejecuta: sólo los enumera.
#[derive(Clone, Debug)]
pub enum Efecto {
    /// Pedir el siguiente turno al modelo.
    Inferencia {
        provider: ProviderRef,
        prompt: String,
        now: u64,
    },
    /// Ejecutar una tool (o hablar por el bus).
    Herramientas { accion: Action, tick: u32, now: u64 },
    /// Lanzar trabajo asíncrono.
    Tareas { tarea: Tarea, now: u64 },
}

/// Trabajo asíncrono que lanza el bucle.
#[derive(Clone, Debug)]
pub enum Tarea {
    /// Enrutar a otro nodo (meta-agente): el router decide el destino.
    Enrutar { provider: String, task_type: String },
    /// Consultar el criterio fijo (reclamar una tarea del tablero).
    CriterioFijo,
    /// Integrar el resultado de una tarea terminada.
    Cerrar {
        id: String,
        ok: bool,
        detalle: String,
    },
}

/// Estado del bucle.  **Puro**: no tiene ninguna referencia al modelo, a las
/// tools ni al disco.  Todo lo que el reductor necesita está aquí.
#[derive(Clone, Debug)]
pub struct Estado {
    /// Topes y permisos del modo.
    pub perfil: Perfil,
    /// Presupuesto por hora (ventana rodante).
    pub presupuesto: HourBudget,
    /// Libro de precios (con fuente; sin fuente ⇒ `NO VERIFICADO`).
    pub precios: PriceBook,
    /// Proveedores del enjambre (el local es el último recurso).
    pub proveedores: Vec<ProviderRef>,
    /// Circuit breakers (uno por proveedor).
    pub breakers: HashMap<String, CircuitBreaker>,
    /// Diario de fallos **en memoria** (el reductor sólo lo lee y lo escribe).
    pub fallos: Vec<FailureEntry>,
    /// Tope de entradas del diario en memoria.
    pub max_registros: usize,
    /// Entradas decididas por el reductor y **pendientes de persistir**
    /// (las vacía el driver).
    pub pendientes: Vec<FailureEntry>,
    /// Número de tick.
    pub tick: u32,
    /// Reloj lógico del evento en curso.
    pub now: u64,
    /// Tarea/prompt activo.
    pub tarea: String,
    /// ¿Queda trabajo pendiente (para el latido `Tick`)?
    pub trabajo_pendiente: bool,
    /// Llamadas al modelo en este tick.
    pub llamadas: u32,
    /// Tokens gastados en este tick.
    pub tokens: u64,
    /// Coste real del último turno (con su fuente o `NO VERIFICADO`).
    pub coste: Option<Cost>,
    /// Narración del tick (lo que ve la persona).
    pub acciones: Vec<String>,
    /// Última acción ejecutada.
    pub ultima_accion: Option<Action>,
    /// Proveedor usado en este tick.
    pub proveedor_usado: Option<String>,
    /// ¿El modelo devolvió basura y hubo que degradar?
    pub degradado: bool,
    /// Motivo de parada (si lo hay).
    pub detenido: Option<String>,
    /// Último error del tick (se usa como `stopped_reason` si no hay otro).
    pub ultimo_error: Option<String>,
    /// Proveedores ya intentados en este tick (no se repiten).
    pub probados: Vec<String>,
    /// Procesos de shell gastados en este tick.
    pub shells: u32,
    /// Procesos que crea cada llamada (lo informa el ejecutor).
    pub spawns_por_llamada: u32,
    /// ¿Terminó el tick?
    pub fin_del_tick: bool,
    /// Estado de los breakers **al empezar** el tick (informe).
    pub breakers_inicio: Vec<(String, BreakerState)>,
}

impl Estado {
    /// Estado nuevo.  El diario se siembra con las entradas ya conocidas
    /// (las lee el driver del [`Diario`]).
    pub fn nuevo(
        perfil: Perfil,
        presupuesto: HourBudget,
        precios: PriceBook,
        proveedores: Vec<ProviderRef>,
        breakers: HashMap<String, CircuitBreaker>,
        fallos: Vec<FailureEntry>,
        tarea: String,
    ) -> Self {
        let mut breakers = breakers;
        for p in &proveedores {
            breakers
                .entry(p.id.clone())
                .or_insert_with(|| CircuitBreaker::new(&p.id));
        }
        let breakers_inicio = breakers
            .iter()
            .map(|(k, v)| (k.clone(), v.state()))
            .collect();
        Estado {
            perfil,
            presupuesto,
            precios,
            proveedores,
            breakers,
            fallos,
            max_registros: 500,
            pendientes: Vec::new(),
            tick: 0,
            now: 0,
            tarea,
            trabajo_pendiente: false,
            llamadas: 0,
            tokens: 0,
            coste: None,
            acciones: Vec::new(),
            ultima_accion: None,
            proveedor_usado: None,
            degradado: false,
            detenido: None,
            ultimo_error: None,
            probados: Vec::new(),
            shells: 0,
            spawns_por_llamada: 0,
            fin_del_tick: false,
            breakers_inicio,
        }
    }

    /// Reinicia los contadores de un tick (el estado global — breakers,
    /// presupuesto, diario — **no** se reinicia).
    fn reiniciar_tick(&mut self) {
        self.llamadas = 0;
        self.tokens = 0;
        self.coste = None;
        self.acciones.clear();
        self.ultima_accion = None;
        self.proveedor_usado = None;
        self.degradado = false;
        self.detenido = None;
        self.ultimo_error = None;
        self.probados.clear();
        self.shells = 0;
        self.fin_del_tick = false;
        self.breakers_inicio = self
            .breakers
            .iter()
            .map(|(k, v)| (k.clone(), v.state()))
            .collect();
    }

    /// Registra un fallo en el diario **en memoria** (puro).  La escritura en
    /// disco la hace el driver (`bucle`) a través del [`Diario`].
    fn registrar_fallo(&mut self, entrada: FailureEntry) {
        self.pendientes.push(entrada.clone());
        self.fallos.push(entrada);
        if self.fallos.len() > self.max_registros {
            let drop = self.fallos.len() - self.max_registros;
            self.fallos.drain(0..drop);
        }
    }
}

/// Informe de una vuelta del bucle.
#[derive(Clone, Debug, Default)]
pub struct Informe {
    /// Número de tick.
    pub tick: u32,
    /// Llamadas al modelo.
    pub llamadas: u32,
    /// Tokens gastados.
    pub tokens: u64,
    /// Coste real del último turno.
    pub coste: Option<Cost>,
    /// Narración del tick.
    pub acciones: Vec<String>,
    /// Última acción ejecutada.
    pub ultima_accion: Option<Action>,
    /// ¿Hubo degradación (el modelo no devolvió una acción utilizable)?
    pub degradado: bool,
    /// Estado de los breakers al empezar el tick.
    pub breakers: Vec<(String, BreakerState)>,
    /// Proveedor usado.
    pub proveedor_usado: Option<String>,
    /// Motivo de parada.
    pub detenido: Option<String>,
}

/// Sumidero del diario de fallos: **la E/S vive aquí**, nunca en el
/// reductor.  El reductor sólo *decide* qué se registra.
pub trait Diario {
    /// Entradas ya registradas (semilla del estado puro).
    fn entradas(&self) -> &[FailureEntry];
    /// Persiste una entrada ya decidida por el reductor.
    fn registrar(&mut self, entrada: &FailureEntry);
}

/// Dependencias inyectadas del bucle: el modelo, las tools y el router.
pub struct Deps<'a> {
    /// Backend de chat (el modelo).  Lo invoca **sólo** [`manejar_inferencia`].
    pub backend: &'a mut dyn ChatBackend,
    /// Ejecutor de tools.  Lo invoca **sólo** [`manejar_herramientas`].
    pub executor: &'a mut dyn ToolExecutor,
    /// Router del meta-agente.  Lo invoca **sólo** [`manejar_tareas`].
    pub router: &'a mut dyn Router,
    /// Sumidero del diario de fallos (`None` = sólo memoria).
    pub diario: Option<&'a mut dyn Diario>,
}

/// Paso del driver: un evento que hay que reducir, o los efectos que quedan
/// por despachar de una reducción anterior.
enum Paso {
    Evento(Evento),
    Efectos(Vec<Efecto>),
}

// ---------------------------------------------------------------------------
// El reductor (PURO)
// ---------------------------------------------------------------------------

/// **El reductor**: `(estado, evento) -> (estado, efectos)`.
///
/// Puro: **no** llama al modelo, **no** llama a las tools y **no** hace E/S.
/// Sólo decide qué efectos hay que despachar y con qué datos.
pub fn reducir(estado: Estado, evento: Evento) -> (Estado, Vec<Efecto>) {
    let mut estado = estado;
    let now = evento.now();
    estado.now = now;
    match evento {
        Evento::UserInput { texto, .. } => {
            // Una entrada nueva abre un tick: contadores a cero.
            estado.tick = estado.tick.saturating_add(1);
            estado.reiniciar_tick();
            estado.tarea = texto;
            estado.trabajo_pendiente = true;
            let efectos = planificar_inferencia(&mut estado, now);
            (estado, efectos)
        }
        Evento::Tick { .. } => {
            // Latido: si no hay nada pendiente, **ningún** efecto.
            estado.tick = estado.tick.saturating_add(1);
            if !estado.trabajo_pendiente {
                return (estado, Vec::new());
            }
            let efectos = planificar_inferencia(&mut estado, now);
            (estado, efectos)
        }
        Evento::InferenciaTerminada {
            provider,
            resultado,
            now,
        } => {
            estado.llamadas = estado.llamadas.saturating_add(1);
            match resultado {
                Err(e) => {
                    if let Some(br) = estado.breakers.get_mut(&provider.id) {
                        br.on_failure(now);
                    }
                    estado.ultimo_error = Some(e.clone());
                    estado.registrar_fallo(FailureEntry {
                        ts: now,
                        tick: estado.tick,
                        provider: provider.id.clone(),
                        action: "decision".to_string(),
                        error: e,
                        ok: false,
                        detail: "el proveedor fallo antes de decidir".to_string(),
                    });
                    // Siguiente proveedor de la cadena (el local es el último).
                    let efectos = continuar_o_parar(&mut estado, now);
                    (estado, efectos)
                }
                Ok(outcome) => {
                    if let Some(br) = estado.breakers.get_mut(&provider.id) {
                        br.on_success(now);
                    }
                    // Coste real: tokens medidos + precio **con fuente**.
                    let cost = estado.precios.cost_of(
                        &outcome.model_id,
                        outcome.prompt_tokens,
                        outcome.tokens_used,
                    );
                    estado.presupuesto.spend(now, outcome.tokens_used, &cost);
                    estado.tokens = estado.tokens.saturating_add(outcome.tokens_used);
                    estado.coste = Some(cost);
                    match parse_decision(&outcome.reply) {
                        // No interpretable ⇒ **criterio fijo** (nunca inventar).
                        None => {
                            estado.degradado = true;
                            estado.registrar_fallo(FailureEntry {
                                ts: now,
                                tick: estado.tick,
                                provider: provider.id.clone(),
                                action: "decision".to_string(),
                                error: "respuesta no interpretable".to_string(),
                                ok: false,
                                detail: outcome.reply.chars().take(120).collect::<String>(),
                            });
                            // El criterio fijo lo consulta el manejador de
                            // tareas: el reductor no pregunta al router.
                            (
                                estado,
                                vec![Efecto::Tareas {
                                    tarea: Tarea::CriterioFijo,
                                    now,
                                }],
                            )
                        }
                        Some(accion) => {
                            // Memoria de fallos: NO repetir el mismo error.
                            let previa = crate::agent_loop::should_avoid_in(
                                &estado.fallos,
                                &accion.key(),
                                estado.tick,
                                estado.perfil.ventana_fallos_ticks,
                            );
                            if let Some(prev) = previa {
                                estado.acciones.push(format!(
                                    "evitada {} (ya fallo con: {})",
                                    accion.key(),
                                    prev
                                ));
                                estado.registrar_fallo(FailureEntry {
                                    ts: now,
                                    tick: estado.tick,
                                    provider: provider.id.clone(),
                                    action: accion.key(),
                                    error: prev,
                                    ok: false,
                                    detail: "evitada por memoria de fallos".to_string(),
                                });
                                // El modelo ve la memoria en el siguiente prompt.
                                let efectos = continuar_o_parar(&mut estado, now);
                                (estado, efectos)
                            } else {
                                match plan_accion(&mut estado, accion.clone(), now) {
                                    Some(efecto) => (estado, vec![efecto]),
                                    None => {
                                        let efectos = accion_descartada(&mut estado, &accion, now);
                                        (estado, efectos)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Evento::ToolResult {
            accion,
            ok,
            salida,
            error,
            now,
        } => {
            let provider = estado.proveedor_usado.clone().unwrap_or_default();
            match &error {
                Some(e) => {
                    estado
                        .acciones
                        .push(format!("{} -> ERROR {}", accion.describe(), e));
                    estado.registrar_fallo(FailureEntry {
                        ts: now,
                        tick: estado.tick,
                        provider: provider.clone(),
                        action: accion.key(),
                        error: e.clone(),
                        ok: false,
                        detail: "error al ejecutar la tool".to_string(),
                    });
                }
                None => {
                    let short: String = salida.chars().take(160).collect();
                    estado
                        .acciones
                        .push(format!("{} -> {}", accion.describe(), short));
                }
            }
            // En la vía degradada el tick ya terminaba con el fallback: no se
            // duplica el registro del resultado de la acción.
            if !estado.degradado {
                estado.registrar_fallo(FailureEntry {
                    ts: now,
                    tick: estado.tick,
                    provider,
                    action: accion.key(),
                    error: if ok {
                        String::new()
                    } else {
                        "error de la tool".to_string()
                    },
                    ok,
                    detail: accion.describe(),
                });
            }
            estado.ultima_accion = Some(accion);
            if !ok {
                if estado.degradado {
                    // El criterio fijo manda: el tick termina igual.
                    estado.fin_del_tick = true;
                    return (estado, Vec::new());
                }
                estado.ultimo_error = Some("la tool fallo".to_string());
                let efectos = continuar_o_parar(&mut estado, now);
                return (estado, efectos);
            }
            estado.fin_del_tick = true;
            (estado, Vec::new())
        }
        Evento::TaskDone {
            id,
            ok,
            detalle,
            now,
        } => {
            estado
                .acciones
                .push(format!("tarea {} terminada: {}", id, detalle));
            estado.trabajo_pendiente = true;
            (
                estado,
                vec![Efecto::Tareas {
                    tarea: Tarea::Cerrar { id, ok, detalle },
                    now,
                }],
            )
        }
        Evento::RutaResuelta {
            provider,
            task_type,
            destino,
            now,
        } => {
            let accion = Action::Delegate {
                provider: provider.clone(),
                task_type: task_type.clone(),
            };
            let ok = match &destino {
                Some(d) => {
                    estado
                        .acciones
                        .push(format!("enrutado a {} ({})", d, task_type));
                    true
                }
                None => {
                    estado.acciones.push(format!(
                        "sin destino para {} ({}): delegacion rechazada",
                        provider, task_type
                    ));
                    false
                }
            };
            estado.registrar_fallo(FailureEntry {
                ts: now,
                tick: estado.tick,
                provider: estado.proveedor_usado.clone().unwrap_or_default(),
                action: accion.key(),
                error: if ok {
                    String::new()
                } else {
                    "error de la tool".to_string()
                },
                ok,
                detail: accion.describe(),
            });
            estado.ultima_accion = Some(accion);
            if !ok {
                estado.ultimo_error = Some("la tool fallo".to_string());
                let efectos = continuar_o_parar(&mut estado, now);
                return (estado, efectos);
            }
            estado.fin_del_tick = true;
            (estado, Vec::new())
        }
        Evento::CriterioFijo { tarea, now } => {
            let fallback = match &tarea {
                Some(t) => Action::Tool {
                    name: "claim_task".to_string(),
                    args: json!({ "task": t }).to_string(),
                },
                None => Action::Idle {
                    reason: "el modelo no devolvio una accion utilizable: criterio fijo = callar"
                        .to_string(),
                },
            };
            estado.acciones.push(format!(
                "respuesta no interpretable -> criterio fijo: {}",
                fallback.describe()
            ));
            match plan_accion(&mut estado, fallback.clone(), now) {
                Some(efecto) => (estado, vec![efecto]),
                None => {
                    let efectos = accion_descartada(&mut estado, &fallback, now);
                    (estado, efectos)
                }
            }
        }
        Evento::TareaCerrada { now, .. } => {
            // La tarea quedó integrada: el bucle sigue (el modelo ve el
            // resultado en el siguiente prompt).
            let efectos = continuar_o_parar(&mut estado, now);
            (estado, efectos)
        }
        Evento::Inerte { .. } => (estado, Vec::new()),
    }
}

/// Decide el siguiente paso: pedir otro turno al modelo o parar el tick.
fn continuar_o_parar(estado: &mut Estado, now: u64) -> Vec<Efecto> {
    if estado.llamadas >= estado.perfil.max_calls_per_tick {
        estado.fin_del_tick = true;
        return Vec::new();
    }
    planificar_inferencia(estado, now)
}

/// Planea la llamada al modelo: presupuesto ⇒ prompt ⇒ proveedor.
///
/// `None` = no se puede llamar (presupuesto o cadena de proveedores
/// agotada): el tick se detiene con su motivo.
fn planificar_inferencia(estado: &mut Estado, now: u64) -> Vec<Efecto> {
    // 0) Tope duro de llamadas por tick.
    if estado.llamadas >= estado.perfil.max_calls_per_tick {
        estado.fin_del_tick = true;
        return Vec::new();
    }
    // 1) ¿Queda presupuesto de la hora?
    let probe = Cost::unverified("antes de llamar no se conoce el coste");
    if let Err(e) = estado.presupuesto.allows(now, 0, &probe) {
        let motivo = e.to_string();
        estado.detenido = Some(motivo.clone());
        estado.ultima_accion = Some(Action::Idle { reason: motivo });
        estado.fin_del_tick = true;
        return Vec::new();
    }
    // 2) Prompt (con la memoria de fallos; **nunca** con credenciales).
    let prompt = construir_prompt(&estado.tarea, &estado.proveedores, &resumen_fallos(estado));
    // 3) Proveedor: cadena con breaker; el local es el último recurso.
    let Some(provider) = elegir_proveedor(estado, now) else {
        estado.detenido = Some("todos los proveedores en abierto".to_string());
        estado.ultima_accion = Some(Action::Idle {
            reason: "circuit breaker: no hay proveedor disponible".to_string(),
        });
        estado.fin_del_tick = true;
        return Vec::new();
    };
    estado.proveedor_usado = Some(provider.id.clone());
    estado.probados.push(provider.id.clone());
    vec![Efecto::Inferencia {
        provider,
        prompt,
        now,
    }]
}

/// Elige proveedor: cadena con breaker; el local (SEERE) es el último
/// recurso y un proveedor ya fallido en este tick no se reintenta.
fn elegir_proveedor(estado: &mut Estado, now: u64) -> Option<ProviderRef> {
    let order: Vec<ProviderRef> = estado
        .proveedores
        .iter()
        .filter(|p| !p.local)
        .cloned()
        .chain(estado.proveedores.iter().filter(|p| p.local).cloned())
        .collect();
    for p in order {
        if estado.probados.iter().any(|t| *t == p.id) {
            continue;
        }
        let br = estado
            .breakers
            .entry(p.id.clone())
            .or_insert_with(|| CircuitBreaker::new(&p.id));
        if br.allows(now) {
            return Some(p);
        }
    }
    None
}

/// Decide cómo se ejecuta una acción: aplica el **tope duro de shell** y
/// devuelve el efecto.  `None` = la acción se descarta (tope o permiso).
fn plan_accion(estado: &mut Estado, accion: Action, now: u64) -> Option<Efecto> {
    // Tope duro de procesos de shell por tick (el móvil es el recurso escaso:
    // cada `su` son 2+ procesos).
    let spawns = estado.spawns_por_llamada;
    if estado.shells.saturating_add(spawns) > estado.perfil.max_shell_spawns_per_tick {
        estado.acciones.push(format!(
            "tope de shell del tick alcanzado ({} + {} > {}): accion descartada",
            estado.shells, spawns, estado.perfil.max_shell_spawns_per_tick
        ));
        return None;
    }
    estado.shells = estado.shells.saturating_add(spawns);
    match accion {
        Action::Delegate {
            provider,
            task_type,
        } => {
            if !estado.perfil.permite_delegar {
                estado.acciones.push(format!(
                    "delegacion no permitida en modo {}: {} ({}) descartada",
                    estado.perfil.modo.as_str(),
                    provider,
                    task_type
                ));
                return None;
            }
            Some(Efecto::Tareas {
                tarea: Tarea::Enrutar {
                    provider,
                    task_type,
                },
                now,
            })
        }
        Action::Speak { text } => {
            if !estado.perfil.permite_hablar {
                estado.acciones.push(format!(
                    "hablar no permitido en modo {}: {} descartado",
                    estado.perfil.modo.as_str(),
                    text
                ));
                return None;
            }
            Some(Efecto::Herramientas {
                accion: Action::Speak { text },
                tick: estado.tick,
                now,
            })
        }
        otra => Some(Efecto::Herramientas {
            accion: otra,
            tick: estado.tick,
            now,
        }),
    }
}

/// Una acción **no** se pudo ejecutar (tope de shell o permiso): se registra
/// como fallida y el tick continúa (el modelo se corrige).
fn accion_descartada(estado: &mut Estado, accion: &Action, now: u64) -> Vec<Efecto> {
    estado.ultima_accion = Some(accion.clone());
    estado.registrar_fallo(FailureEntry {
        ts: now,
        tick: estado.tick,
        provider: estado.proveedor_usado.clone().unwrap_or_default(),
        action: accion.key(),
        error: "error de la tool".to_string(),
        ok: false,
        detail: accion.describe(),
    });
    estado.ultimo_error = Some("la tool fallo".to_string());
    continuar_o_parar(estado, now)
}

/// La instrucción de sistema del meta-agente.
const SISTEMA_METAAGENTE: &str = "Eres el meta-agente de MegaDaemon. Responde SOLO con una accion.";

/// Los formatos que el meta-agente puede emitir (una regla).
const FORMATOS_ADMITIDOS: &str = "Formatos admitidos:\n  {\"action\":\"tool\",\"name\":\"<tool>\",\"args\":{...}}\n  {\"action\":\"delegate\",\"provider\":\"<id>\",\"task_type\":\"<tipo>\"}\n  {\"action\":\"speak\",\"text\":\"...\"}";

/// La regla del último recurso (el proveedor local).
const ULTIMO_RECURSO: &str =
    "Provider local (SEERE) es el ultimo recurso: solo si los demas fallan.";

/// El estado del contexto del meta-agente, **ya resuelto**.
///
/// MEDIDO (2026-11-28, patrón 6): el prompt **no** se escribe a mano aquí:
/// se le pide al módulo de contexto (`crate::contexto`) que lo arme.  Esta
/// función sólo **traduce** lo que el bucle tiene (la tarea, los
/// proveedores, la memoria de fallos) al modelo del contexto; el módulo
/// `contexto` es el que decide el orden de los bloques, los recorta si
/// llega un presupuesto y devuelve los metadatos (chars por bloque).
fn estado_de_contexto(
    tarea: &str,
    proveedores: &[ProviderRef],
    fallos: &[String],
) -> crate::contexto::EstadoContexto {
    use crate::contexto::{EstadoContexto, ProveedorContexto};
    // La memoria: la cabecera explica el bloque y las entradas son lo que
    // ya falló.  Sin fallos **no hay bloque** (no hay bloques de adorno).
    let memoria = if fallos.is_empty() {
        Vec::new()
    } else {
        let mut m = vec!["Lo que YA fallo (no lo repitas):".to_string()];
        m.extend(fallos.iter().map(|s| format!("  - {}", s)));
        m
    };
    EstadoContexto {
        sistema: SISTEMA_METAAGENTE.to_string(),
        reglas: vec![FORMATOS_ADMITIDOS.to_string(), ULTIMO_RECURSO.to_string()],
        memoria,
        ficheros: Vec::new(),
        // El bucle de efectos no lleva historial de compactación: la
        // memoria de fallos va en su propio bloque.
        historial: crate::compaction_epoch::Historial::nuevo("metaagente"),
        proveedores: proveedores
            .iter()
            .map(|p| ProveedorContexto {
                id: p.id.clone(),
                local: p.local,
            })
            .collect(),
        // Sin presupuesto: el recorte por presupuesto es del llamador (el
        // que sabe el modelo que se va a usar).
        presupuesto_chars: None,
    }
}

/// Construye el prompt.  **Nunca** lleva API key, token ni secretos: sólo
/// la tarea, la memoria de fallos y la lista de proveedores.
///
/// MEDIDO (2026-11-28, patrón 6): el prompt sale de
/// `crate::contexto::construir_contexto` (puro: sin red, sin disco, sin
/// modelo) ⇒ los bloques son los canónicos (`[SISTEMA]`, `[REGLAS]`,
/// `[MEMORIA]`, `[ENTRADA]`) y el prompt es **determinista** (mismo estado
/// ⇒ mismo prompt, byte a byte).
pub fn construir_prompt(tarea: &str, proveedores: &[ProviderRef], fallos: &[String]) -> String {
    let estado = estado_de_contexto(tarea, proveedores, fallos);
    let entrada = crate::contexto::Entrada::nueva(format!("Tarea: {}", tarea));
    crate::contexto::construir_contexto(&estado, &entrada).to_prompt()
}

/// Resumen de lo que ya falló (para el prompt): "acción → error (n veces)".
fn resumen_fallos(estado: &Estado) -> Vec<String> {
    crate::agent_loop::summary_in(
        &estado.fallos,
        estado.tick,
        estado.perfil.ventana_fallos_ticks,
    )
}

// ---------------------------------------------------------------------------
// Los manejadores (SÍ hacen E/S)
// ---------------------------------------------------------------------------

/// Manejador del efecto de **inferencia**: pide el siguiente turno al
/// modelo.  Devuelve el evento siguiente (`InferenciaTerminada`).
///
/// El estado no hace falta aquí: el manejador sólo **devuelve** el evento;
/// el reductor es quien actualiza el estado.
pub fn manejar_inferencia(_estado: &mut Estado, efecto: &Efecto, deps: &mut Deps<'_>) -> Evento {
    match efecto {
        Efecto::Inferencia {
            provider,
            prompt,
            now,
        } => {
            let resultado = deps.backend.chat(provider, prompt);
            Evento::InferenciaTerminada {
                provider: provider.clone(),
                resultado,
                now: *now,
            }
        }
        _ => Evento::Inerte { now: 0 },
    }
}

/// Manejador del efecto de **herramientas**: ejecuta la tool.  Devuelve el
/// evento siguiente (`ToolResult`).
///
/// El estado no hace falta aquí: el manejador sólo **devuelve** el evento;
/// el reductor es quien actualiza el estado.
pub fn manejar_herramientas(_estado: &mut Estado, efecto: &Efecto, deps: &mut Deps<'_>) -> Evento {
    match efecto {
        Efecto::Herramientas { accion, tick, now } => {
            let resultado = deps.executor.call(accion, *tick);
            match resultado {
                Ok(salida) => Evento::ToolResult {
                    accion: accion.clone(),
                    ok: true,
                    salida,
                    error: None,
                    now: *now,
                },
                Err(e) => Evento::ToolResult {
                    accion: accion.clone(),
                    ok: false,
                    salida: String::new(),
                    error: Some(e),
                    now: *now,
                },
            }
        }
        _ => Evento::Inerte { now: 0 },
    }
}

/// Manejador del efecto de **tareas**: enruta a otro nodo, consulta el
/// criterio fijo o integra el resultado de una tarea terminada.  Devuelve el
/// evento siguiente.
pub fn manejar_tareas(estado: &mut Estado, efecto: &Efecto, deps: &mut Deps<'_>) -> Evento {
    match efecto {
        Efecto::Tareas {
            tarea:
                Tarea::Enrutar {
                    provider,
                    task_type,
                },
            now,
        } => {
            let destino = deps.router.route(provider, task_type);
            Evento::RutaResuelta {
                provider: provider.clone(),
                task_type: task_type.clone(),
                destino,
                now: *now,
            }
        }
        Efecto::Tareas {
            tarea: Tarea::CriterioFijo,
            now,
        } => {
            let tarea = deps.router.fixed_fallback_task();
            Evento::CriterioFijo { tarea, now: *now }
        }
        Efecto::Tareas {
            tarea: Tarea::Cerrar { id, ok, detalle },
            now,
        } => {
            // Integrar el resultado de la tarea terminada (aquí sí se
            // escribe: el manejador es la parte con E/S).
            estado.registrar_fallo(FailureEntry {
                ts: *now,
                tick: estado.tick,
                provider: estado.proveedor_usado.clone().unwrap_or_default(),
                action: format!("task:{}", id),
                error: if *ok {
                    String::new()
                } else {
                    "la tarea fallo".to_string()
                },
                ok: *ok,
                detail: detalle.clone(),
            });
            Evento::TareaCerrada {
                id: id.clone(),
                ok: *ok,
                now: *now,
            }
        }
        _ => Evento::Inerte { now: 0 },
    }
}

/// Despacha un efecto a su manejador.
fn despachar(estado: &mut Estado, efecto: &Efecto, deps: &mut Deps<'_>) -> Evento {
    match efecto {
        Efecto::Inferencia { .. } => manejar_inferencia(estado, efecto, deps),
        Efecto::Herramientas { .. } => manejar_herramientas(estado, efecto, deps),
        Efecto::Tareas { .. } => manejar_tareas(estado, efecto, deps),
    }
}

// ---------------------------------------------------------------------------
// El bucle (driver)
// ---------------------------------------------------------------------------

/// **El bucle**: reduce, despacha cada efecto a su manejador y mete el
/// evento siguiente de vuelta.  Es **iterativo** (pila explícita, no
/// recursión) y tiene un tope duro de eventos ⇒ **termina siempre**.
pub fn bucle(estado: Estado, evento_inicial: Evento, deps: &mut Deps<'_>) -> (Estado, Informe) {
    let mut estado = estado;
    let mut pila: Vec<Paso> = vec![Paso::Evento(evento_inicial)];
    let mut procesados: u32 = 0;
    let max_eventos = estado.perfil.max_eventos;
    while let Some(paso) = pila.pop() {
        match paso {
            Paso::Evento(evento) => {
                if estado.fin_del_tick {
                    break;
                }
                if procesados >= max_eventos {
                    estado.detenido = Some(format!(
                        "tope de eventos del bucle alcanzado ({}): el tick se corta",
                        max_eventos
                    ));
                    estado.fin_del_tick = true;
                    break;
                }
                procesados += 1;
                // El ejecutor informa de cuántos procesos crea cada llamada:
                // el reductor lo necesita para el tope duro (lectura, no
                // efecto: no se ejecuta ninguna tool aquí).
                estado.spawns_por_llamada = deps.executor.spawns_per_call();
                let (nuevo, efectos) = reducir(estado, evento);
                estado = nuevo;
                let mut efectos = efectos;
                efectos.reverse();
                pila.push(Paso::Efectos(efectos));
            }
            Paso::Efectos(mut efectos) => {
                if let Some(efecto) = efectos.pop() {
                    let evento = despachar(&mut estado, &efecto, deps);
                    // El evento del manejador se procesa **antes** que el
                    // resto de efectos (orden del pseudocódigo).
                    pila.push(Paso::Efectos(efectos));
                    pila.push(Paso::Evento(evento));
                }
            }
        }
        // La E/S del diario la hace el driver, nunca el reductor.
        if estado.perfil.persiste_fallos {
            if let Some(diario) = deps.diario.as_deref_mut() {
                for entrada in estado.pendientes.drain(..) {
                    diario.registrar(&entrada);
                }
            }
        }
    }
    if estado.acciones.is_empty() {
        estado.acciones.push(format!(
            "sin llamadas (tope por tick: {})",
            estado.perfil.max_calls_per_tick
        ));
    }
    if let Some(e) = estado.ultimo_error.clone() {
        if estado.detenido.is_none() {
            estado.detenido = Some(e);
        }
    }
    let informe = Informe {
        tick: estado.tick,
        llamadas: estado.llamadas,
        tokens: estado.tokens,
        coste: estado.coste.clone(),
        acciones: estado.acciones.clone(),
        ultima_accion: estado.ultima_accion.clone(),
        degradado: estado.degradado,
        breakers: estado.breakers_inicio.clone(),
        proveedor_usado: estado.proveedor_usado.clone(),
        detenido: estado.detenido.clone(),
    };
    (estado, informe)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Backend falso: cuenta las llamadas al modelo.
    struct FakeBackend {
        replies: Vec<Result<String, String>>,
        prompts: Vec<String>,
        calls: usize,
    }

    impl FakeBackend {
        fn new(replies: Vec<Result<String, String>>) -> Self {
            FakeBackend {
                replies,
                prompts: Vec::new(),
                calls: 0,
            }
        }
    }

    impl ChatBackend for FakeBackend {
        fn chat(&mut self, provider: &ProviderRef, prompt: &str) -> Result<ChatOutcome, String> {
            self.calls += 1;
            self.prompts.push(prompt.to_string());
            let idx = self.calls - 1;
            let r = self.replies.get(idx).cloned().unwrap_or_else(|| {
                Ok("{\"action\":\"idle\",\"reason\":\"sin respuestas\"}".to_string())
            });
            r.map(|reply| ChatOutcome {
                reply,
                tokens_used: 100,
                prompt_tokens: 50,
                latency_ms: 12.0,
                model_id: provider.model.clone(),
            })
        }
    }

    /// Ejecutor falso: cuenta las llamadas a las tools.
    struct FakeExecutor {
        calls: Vec<String>,
        spawns: u32,
        fail_on: Option<String>,
    }

    impl FakeExecutor {
        fn new() -> Self {
            FakeExecutor {
                calls: Vec::new(),
                spawns: 2,
                fail_on: None,
            }
        }

        fn fallando(name: &str) -> Self {
            FakeExecutor {
                calls: Vec::new(),
                spawns: 2,
                fail_on: Some(name.to_string()),
            }
        }
    }

    impl ToolExecutor for FakeExecutor {
        fn call(&mut self, action: &Action, _tick: u32) -> Result<String, String> {
            let k = action.key();
            self.calls.push(k.clone());
            if let Some(f) = &self.fail_on {
                if *f == k {
                    return Err("boom".to_string());
                }
            }
            Ok("hecho".to_string())
        }

        fn spawns_per_call(&self) -> u32 {
            self.spawns
        }
    }

    /// Router falso.
    struct FakeRouter {
        fallback: Option<String>,
    }

    impl Router for FakeRouter {
        fn route(&self, provider: &str, task_type: &str) -> Option<String> {
            Some(format!("{}::{}", provider, task_type))
        }

        fn fixed_fallback_task(&self) -> Option<String> {
            self.fallback.clone()
        }
    }

    /// Diario en memoria (implementa [`Diario`] sin tocar el disco).
    #[derive(Default)]
    struct FakeDiario {
        entradas: Vec<FailureEntry>,
    }

    impl Diario for FakeDiario {
        fn entradas(&self) -> &[FailureEntry] {
            &self.entradas
        }

        fn registrar(&mut self, entrada: &FailureEntry) {
            self.entradas.push(entrada.clone());
        }
    }

    fn estado_de_prueba(modo: Modo) -> Estado {
        Estado::nuevo(
            Perfil::for_modo(modo),
            HourBudget::new(120_000, Some(1.0), 120),
            PriceBook::new().with_source("m1", 0.0001, 0.0002, "precio de prueba (fuente: test)"),
            vec![
                ProviderRef::remote("p1", "m1"),
                ProviderRef::local("seere", "m-local"),
            ],
            HashMap::new(),
            Vec::new(),
            "tarea".to_string(),
        )
    }

    fn deps<'a>(
        backend: &'a mut FakeBackend,
        executor: &'a mut FakeExecutor,
        router: &'a mut FakeRouter,
        diario: &'a mut FakeDiario,
    ) -> Deps<'a> {
        Deps {
            backend,
            executor,
            router,
            diario: Some(diario),
        }
    }

    // --- Criterio 2: el reductor no ejecuta nada -------------------------

    #[test]
    fn el_reductor_es_puro_no_toca_el_modelo_ni_las_tools() {
        let estado = estado_de_prueba(Modo::Interactivo);
        let backend = FakeBackend::new(vec![Ok("{\"action\":\"idle\"}".to_string())]);
        let executor = FakeExecutor::new();
        let _router = FakeRouter { fallback: None };
        let diario = FakeDiario::default();

        // El reductor NO recibe backend ni executor: no puede llamarlos.
        let (_estado, efectos) = reducir(
            estado,
            Evento::UserInput {
                texto: "tarea".to_string(),
                now: 100,
            },
        );
        assert_eq!(efectos.len(), 1, "debe proponer exactamente un efecto");
        assert!(matches!(efectos[0], Efecto::Inferencia { .. }));

        // Y de verdad: ni el modelo ni las tools se han llamado.
        assert_eq!(backend.calls, 0, "el reductor no puede llamar al modelo");
        assert!(
            executor.calls.is_empty(),
            "el reductor no puede llamar a las tools"
        );
        assert!(
            diario.entradas.is_empty(),
            "el reductor no puede escribir en disco"
        );
    }

    #[test]
    fn una_lista_de_efectos_se_puede_dar_sin_ejecutar_nada() {
        let estado = estado_de_prueba(Modo::Interactivo);
        let backend = FakeBackend::new(vec![Ok(
            "{\"action\":\"tool\",\"name\":\"ls\",\"args\":{}}".to_string(),
        )]);
        let executor = FakeExecutor::new();
        let _router = FakeRouter { fallback: None };
        let diario = FakeDiario::default();

        // Se pide **sólo** la lista de efectos.
        let (_estado, efectos) = reducir(
            estado,
            Evento::UserInput {
                texto: "tarea".to_string(),
                now: 100,
            },
        );
        assert_eq!(efectos.len(), 1);
        assert!(matches!(efectos[0], Efecto::Inferencia { .. }));

        // Nadie se ha ejecutado: ni modelo, ni tools, ni disco.
        assert_eq!(backend.calls, 0);
        assert!(executor.calls.is_empty());
        assert!(diario.entradas.is_empty());
    }

    // --- El reductor decide, los manejadores ejecutan ---------------------

    #[test]
    fn user_input_produce_el_efecto_de_inferencia_esperado() {
        let estado = estado_de_prueba(Modo::Interactivo);
        let (estado, efectos) = reducir(
            estado,
            Evento::UserInput {
                texto: "busca el fallo".to_string(),
                now: 100,
            },
        );
        assert_eq!(efectos.len(), 1);
        match &efectos[0] {
            Efecto::Inferencia {
                provider,
                prompt,
                now,
            } => {
                assert_eq!(provider.id, "p1");
                assert!(prompt.contains("busca el fallo"));
                assert_eq!(*now, 100);
            }
            otro => panic!("se esperaba inferencia, obtuve {:?}", otro),
        }
        assert_eq!(estado.tick, 1);
    }

    #[test]
    fn el_manejador_de_inferencia_es_el_que_llama_al_modelo() {
        let mut backend = FakeBackend::new(vec![Ok("{\"action\":\"idle\"}".to_string())]);
        let mut executor = FakeExecutor::new();
        let mut router = FakeRouter { fallback: None };
        let mut diario = FakeDiario::default();
        let mut estado = estado_de_prueba(Modo::Interactivo);

        let efecto = Efecto::Inferencia {
            provider: ProviderRef::remote("p1", "m1"),
            prompt: "hola".to_string(),
            now: 100,
        };
        let evento = manejar_inferencia(
            &mut estado,
            &efecto,
            &mut deps(&mut backend, &mut executor, &mut router, &mut diario),
        );
        assert_eq!(backend.calls, 1, "el manejador llama al modelo");
        assert!(executor.calls.is_empty());
        match evento {
            Evento::InferenciaTerminada {
                provider,
                resultado,
                ..
            } => {
                assert_eq!(provider.id, "p1");
                assert!(resultado.is_ok());
            }
            otro => panic!("se esperaba inferencia terminada, obtuve {:?}", otro),
        }
    }

    #[test]
    fn tool_result_produce_el_efecto_de_herramientas_y_devuelve_el_evento_siguiente() {
        // 1) El modelo pide una tool: el reductor propone el efecto.
        let mut backend = FakeBackend::new(vec![Ok(
            "{\"action\":\"tool\",\"name\":\"ls\",\"args\":{}}".to_string(),
        )]);
        let mut executor = FakeExecutor::new();
        let mut router = FakeRouter { fallback: None };
        let mut diario = FakeDiario::default();
        let mut estado = estado_de_prueba(Modo::Interactivo);

        let (_, efectos) = reducir(
            estado.clone(),
            Evento::UserInput {
                texto: "tarea".to_string(),
                now: 100,
            },
        );
        assert_eq!(efectos.len(), 1);
        assert!(matches!(efectos[0], Efecto::Inferencia { .. }));

        // El modelo responde pidiendo la tool.
        let (_, efectos) = reducir(
            estado.clone(),
            Evento::InferenciaTerminada {
                provider: ProviderRef::remote("p1", "m1"),
                resultado: Ok(ChatOutcome {
                    reply: "{\"action\":\"tool\",\"name\":\"ls\",\"args\":{}}".to_string(),
                    tokens_used: 10,
                    prompt_tokens: 5,
                    latency_ms: 1.0,
                    model_id: "m1".to_string(),
                }),
                now: 101,
            },
        );
        assert_eq!(efectos.len(), 1, "el reductor propone la tool");
        let efecto = match &efectos[0] {
            Efecto::Herramientas { accion, .. } => {
                assert_eq!(accion.key(), "tool:ls");
                efectos[0].clone()
            }
            otro => panic!("se esperaba herramientas, obtuve {:?}", otro),
        };

        // 2) El manejador la ejecuta y devuelve el evento siguiente.
        let evento = manejar_herramientas(
            &mut estado,
            &efecto,
            &mut deps(&mut backend, &mut executor, &mut router, &mut diario),
        );
        assert_eq!(executor.calls, vec!["tool:ls".to_string()]);
        match &evento {
            Evento::ToolResult { ok, accion, .. } => {
                assert!(ok);
                assert_eq!(accion.key(), "tool:ls");
            }
            otro => panic!("se esperaba tool result, obtuve {:?}", otro),
        }

        // 3) El `ToolResult` vuelve al bucle: el reductor decide el siguiente
        //    efecto.  Con la tool **OK** el tick termina (0 efectos); con la
        //    tool **fallida** el bucle se corrige y vuelve a preguntar.
        let (estado, efectos) = reducir(estado, evento);
        assert!(
            efectos.is_empty(),
            "con la tool OK el tick termina: {:?}",
            efectos
        );
        assert_eq!(
            estado.ultima_accion.as_ref().map(|a| a.key()),
            Some("tool:ls".to_string())
        );
        assert!(estado.fin_del_tick);

        // Y con la tool fallida, el bucle sigue (efecto de inferencia).
        let (estado, efectos) = reducir(
            estado,
            Evento::ToolResult {
                accion: Action::Tool {
                    name: "ls".to_string(),
                    args: "{}".to_string(),
                },
                ok: false,
                salida: String::new(),
                error: Some("boom".to_string()),
                now: 102,
            },
        );
        assert_eq!(efectos.len(), 1);
        assert!(
            matches!(efectos[0], Efecto::Inferencia { .. }),
            "tras una tool fallida el bucle vuelve a preguntar al modelo: {:?}",
            efectos
        );
        assert_eq!(estado.ultimo_error.as_deref(), Some("la tool fallo"));
    }

    #[test]
    fn task_done_produce_el_efecto_de_tareas() {
        let estado = estado_de_prueba(Modo::Interactivo);
        let (estado, efectos) = reducir(
            estado,
            Evento::TaskDone {
                id: "t-7".to_string(),
                ok: true,
                detalle: "hecho".to_string(),
                now: 200,
            },
        );
        assert_eq!(efectos.len(), 1);
        match &efectos[0] {
            Efecto::Tareas { tarea, now } => {
                assert_eq!(*now, 200);
                match tarea {
                    Tarea::Cerrar { id, ok, detalle } => {
                        assert_eq!(id, "t-7");
                        assert!(ok);
                        assert_eq!(detalle, "hecho");
                    }
                    otro => panic!("se esperaba cerrar la tarea, obtuve {:?}", otro),
                }
            }
            otro => panic!("se esperaba el efecto de tareas, obtuve {:?}", otro),
        }
        assert!(estado
            .acciones
            .iter()
            .any(|a| a.contains("t-7") && a.contains("hecho")));
    }

    #[test]
    fn tick_no_produce_efectos_si_no_hay_nada_pendiente() {
        let estado = estado_de_prueba(Modo::Interactivo);
        let (_estado, efectos) = reducir(estado, Evento::Tick { now: 300 });
        assert!(
            efectos.is_empty(),
            "sin trabajo pendiente no debe haber efectos: {:?}",
            efectos
        );
    }

    #[test]
    fn tick_produce_inferencia_si_hay_trabajo_pendiente() {
        let mut estado = estado_de_prueba(Modo::Interactivo);
        let (nuevo, _) = reducir(
            estado.clone(),
            Evento::UserInput {
                texto: "tarea".to_string(),
                now: 100,
            },
        );
        estado = nuevo;
        // La entrada abrió el tick: el latido continúa el trabajo.
        estado.trabajo_pendiente = true;
        let (_, efectos) = reducir(estado, Evento::Tick { now: 101 });
        assert_eq!(efectos.len(), 1);
        assert!(matches!(efectos[0], Efecto::Inferencia { .. }));
    }

    #[test]
    fn el_bucle_termina_con_una_secuencia_finita_de_eventos() {
        let mut backend = FakeBackend::new(vec![
            Ok("{\"action\":\"tool\",\"name\":\"ls\",\"args\":{}}".to_string()),
            Ok("{\"action\":\"tool\",\"name\":\"ls\",\"args\":{}}".to_string()),
            Ok("{\"action\":\"tool\",\"name\":\"ls\",\"args\":{}}".to_string()),
            Ok("{\"action\":\"tool\",\"name\":\"ls\",\"args\":{}}".to_string()),
            Ok("{\"action\":\"tool\",\"name\":\"ls\",\"args\":{}}".to_string()),
            Ok("{\"action\":\"tool\",\"name\":\"ls\",\"args\":{}}".to_string()),
        ]);
        let mut executor = FakeExecutor::fallando("tool:ls");
        let mut router = FakeRouter { fallback: None };
        let mut diario = FakeDiario::default();
        // Tres proveedores: la cadena se agota justo en el tope de llamadas.
        let mut estado = estado_de_prueba(Modo::Interactivo);
        estado.proveedores.push(ProviderRef::remote("p2", "m1"));

        let (_estado, informe) = bucle(
            estado,
            Evento::UserInput {
                texto: "tarea".to_string(),
                now: 100,
            },
            &mut deps(&mut backend, &mut executor, &mut router, &mut diario),
        );

        // Terminó (no se cuelga) y respetó el tope de llamadas por tick.
        assert!(
            informe.llamadas <= 3,
            "no debe pasar del tope: {}",
            informe.llamadas
        );
        assert_eq!(informe.llamadas, 3);
        // La tool que falla no se repite (memoria de fallos) y el diario se
        // escribió (el driver, no el reductor).
        assert_eq!(executor.calls, vec!["tool:ls".to_string()]);
        assert!(!diario.entradas.is_empty());
        assert!(informe
            .acciones
            .iter()
            .any(|a| a.contains("evitada tool:ls")));
    }

    #[test]
    fn la_cadena_de_proveedores_agotada_detiene_el_tick() {
        // Mismo comportamiento que el bucle original: un proveedor ya
        // probado en este tick no se reintenta; cuando la cadena se agota,
        // el tick para con su motivo.
        let mut backend = FakeBackend::new(vec![
            Err("sin credenciales".to_string()),
            Err("sin credenciales".to_string()),
        ]);
        let mut executor = FakeExecutor::new();
        let mut router = FakeRouter { fallback: None };
        let mut diario = FakeDiario::default();

        let (_estado, informe) = bucle(
            estado_de_prueba(Modo::Interactivo),
            Evento::UserInput {
                texto: "tarea".to_string(),
                now: 100,
            },
            &mut deps(&mut backend, &mut executor, &mut router, &mut diario),
        );
        assert_eq!(informe.llamadas, 2, "p1 y el local: dos intentos");
        assert_eq!(
            informe.detenido.as_deref(),
            Some("todos los proveedores en abierto")
        );
        assert_eq!(informe.proveedor_usado.as_deref(), Some("seere"));
    }

    #[test]
    fn el_mismo_bucle_sirve_para_los_tres_modos() {
        for modo in [Modo::Interactivo, Modo::Programatico, Modo::Subagente] {
            let mut backend = FakeBackend::new(vec![Ok(
                "{\"action\":\"tool\",\"name\":\"ls\",\"args\":{}}".to_string(),
            )]);
            let mut executor = FakeExecutor::new();
            let mut router = FakeRouter { fallback: None };
            let mut diario = FakeDiario::default();
            let estado = estado_de_prueba(modo);

            let (estado, informe) = bucle(
                estado,
                Evento::UserInput {
                    texto: "tarea".to_string(),
                    now: 100,
                },
                &mut deps(&mut backend, &mut executor, &mut router, &mut diario),
            );

            // Los tres modos usan **el mismo** bucle y llegan al mismo punto.
            assert_eq!(informe.llamadas, 1, "modo {:?}", modo);
            assert_eq!(informe.acciones.len(), 1, "modo {:?}", modo);
            assert!(informe.acciones[0].contains("tool ls"), "modo {:?}", modo);
            assert_eq!(
                executor.calls,
                vec!["tool:ls".to_string()],
                "modo {:?}",
                modo
            );
            assert_eq!(estado.perfil.modo, modo);
        }
    }

    #[test]
    fn el_subagente_no_delega_y_el_interactivo_si() {
        // Delegación en modo interactivo: el router decide el destino.
        let mut backend = FakeBackend::new(vec![Ok(
            "{\"action\":\"delegate\",\"provider\":\"seere\",\"task_type\":\"build\"}".to_string(),
        )]);
        let mut executor = FakeExecutor::new();
        let mut router = FakeRouter { fallback: None };
        let mut diario = FakeDiario::default();
        let (estado, informe) = bucle(
            estado_de_prueba(Modo::Interactivo),
            Evento::UserInput {
                texto: "tarea".to_string(),
                now: 100,
            },
            &mut deps(&mut backend, &mut executor, &mut router, &mut diario),
        );
        assert!(
            informe.acciones.iter().any(|a| a.contains("enrutado a")),
            "el interactivo delega: {:?}",
            informe.acciones
        );
        assert_eq!(
            estado.ultima_accion.map(|a| a.key()),
            Some("delegate:seere".to_string())
        );

        // Mismo bucle, modo subagente: la delegación se rechaza.
        let mut backend = FakeBackend::new(vec![Ok(
            "{\"action\":\"delegate\",\"provider\":\"seere\",\"task_type\":\"build\"}".to_string(),
        )]);
        let mut executor = FakeExecutor::new();
        let mut router = FakeRouter { fallback: None };
        let mut diario = FakeDiario::default();
        let (_estado, informe) = bucle(
            estado_de_prueba(Modo::Subagente),
            Evento::UserInput {
                texto: "tarea".to_string(),
                now: 100,
            },
            &mut deps(&mut backend, &mut executor, &mut router, &mut diario),
        );
        assert!(
            informe.acciones.iter().any(|a| a.contains("no permitida")),
            "el subagente no delega: {:?}",
            informe.acciones
        );
    }

    #[test]
    fn la_degradacion_honesta_no_inventa_acciones() {
        // El modelo devuelve basura: el criterio fijo decide (callar si no hay
        // tarea libre), y el bucle **no** inventa una tool.
        let mut backend = FakeBackend::new(vec![Ok("asdfghjkl no es json".to_string())]);
        let mut executor = FakeExecutor::new();
        let mut router = FakeRouter { fallback: None };
        let mut diario = FakeDiario::default();
        let (estado, informe) = bucle(
            estado_de_prueba(Modo::Interactivo),
            Evento::UserInput {
                texto: "tarea".to_string(),
                now: 100,
            },
            &mut deps(&mut backend, &mut executor, &mut router, &mut diario),
        );
        assert!(informe.degradado, "debe marcar degradación");
        assert!(informe
            .acciones
            .iter()
            .any(|a| a.contains("respuesta no interpretable")));
        assert!(
            !diario.entradas.is_empty(),
            "la memoria de fallos se escribe"
        );
        assert!(estado.degradado);
    }

    #[test]
    fn el_tope_duro_de_shell_descarta_la_accion() {
        let mut backend = FakeBackend::new(vec![Ok(
            "{\"action\":\"tool\",\"name\":\"ls\",\"args\":{}}".to_string(),
        )]);
        let mut executor = FakeExecutor::new();
        let mut router = FakeRouter { fallback: None };
        let mut diario = FakeDiario::default();
        let mut estado = estado_de_prueba(Modo::Interactivo);
        // Cada llamada abre 2 procesos y el tope es 1 ⇒ la acción se descarta.
        estado.perfil.max_shell_spawns_per_tick = 1;
        let (_, informe) = bucle(
            estado,
            Evento::UserInput {
                texto: "tarea".to_string(),
                now: 100,
            },
            &mut deps(&mut backend, &mut executor, &mut router, &mut diario),
        );
        assert!(executor.calls.is_empty(), "no debe ejecutar la tool");
        assert!(informe.acciones.iter().any(|a| a.contains("tope de shell")));
    }

    // -----------------------------------------------------------------------
    // Patrón 6: el prompt lo arma el módulo de contexto
    // -----------------------------------------------------------------------

    /// Un proveedor de prueba.
    fn prov(id: &str, local: bool) -> ProviderRef {
        ProviderRef {
            id: id.to_string(),
            model: "m".to_string(),
            local,
        }
    }

    #[test]
    fn el_prompt_lo_arma_el_modulo_de_contexto() {
        // MEDIDO (2026-11-28, patrón 6): el prompt del meta-agente sale de
        // `contexto::construir_contexto` => los bloques son los canónicos.
        let p = construir_prompt("la tarea", &[prov("seere", true)], &[]);
        assert!(p.contains("[SISTEMA]"), "{}", p);
        assert!(p.contains("[REGLAS]"), "{}", p);
        assert!(p.contains("[ENTRADA]"), "{}", p);
        assert!(p.contains("Eres el meta-agente de MegaDaemon"), "{}", p);
        assert!(p.contains("Formatos admitidos:"), "{}", p);
        assert!(
            p.contains("Proveedores disponibles: seere (local, ultimo recurso)"),
            "{}",
            p
        );
        assert!(p.contains("Tarea: la tarea"), "{}", p);
    }

    #[test]
    fn la_memoria_de_fallos_va_en_su_bloque() {
        // MEDIDO (2026-11-28): la frase que el bucle usa ("YA fallo") se
        // mantiene: el bloque [MEMORIA] la lleva como cabecera.
        let fallos = vec!["tool ls: no such file".to_string()];
        let p = construir_prompt("la tarea", &[], &fallos);
        assert!(p.contains("[MEMORIA]"), "{}", p);
        assert!(p.contains("Lo que YA fallo (no lo repitas):"), "{}", p);
        assert!(p.contains("  - tool ls: no such file"), "{}", p);
    }

    #[test]
    fn sin_fallos_no_hay_bloque_de_memoria() {
        // MEDIDO (2026-11-28): no hay bloques de adorno: sin fallos, el
        // bloque [MEMORIA] no aparece (igual que antes del cableado).
        let p = construir_prompt("la tarea", &[], &[]);
        assert!(!p.contains("[MEMORIA]"), "{}", p);
        assert!(!p.contains("[EPOCA]"), "{}", p);
        assert!(!p.contains("[HISTORIAL]"), "{}", p);
        assert!(!p.contains("[FICHEROS]"), "{}", p);
    }

    #[test]
    fn el_prompt_es_determinista_byte_a_byte() {
        // MEDIDO (2026-11-28): mismo estado => mismo prompt (el módulo de
        // contexto no mete el reloj ni el azar).
        let a = construir_prompt("la tarea", &[prov("seere", true)], &["x".to_string()]);
        let b = construir_prompt("la tarea", &[prov("seere", true)], &["x".to_string()]);
        assert_eq!(a, b);
    }

    #[test]
    fn el_prompt_no_lleva_credenciales() {
        // El prompt nunca lleva API key, token ni secretos.
        let p = construir_prompt("la tarea", &[prov("seere", true)], &[]);
        for prohibido in [
            "X-Seere-Auth-Token",
            "X-API-Key",
            "mdaemon_sk",
            "MEGADAEMON_API_KEY",
            "MEGADAEMON_COCKPIT_TOKEN",
        ] {
            assert!(!p.contains(prohibido), "{} en el prompt", prohibido);
        }
    }

    #[test]
    fn el_prompt_y_el_contexto_coinciden_byte_a_byte() {
        // MEDIDO (2026-11-28): el prompt **es** el contexto (no hay dos
        // fuentes de verdad): se puede comparar con el prompt exacto que
        // arma `contexto::construir_contexto` sin red.
        let proveedores = [prov("seere", true)];
        let fallos = vec!["x".to_string()];
        let p = construir_prompt("la tarea", &proveedores, &fallos);
        let estado = estado_de_contexto("la tarea", &proveedores, &fallos);
        let entrada = crate::contexto::Entrada::nueva("Tarea: la tarea");
        let ctx = crate::contexto::construir_contexto(&estado, &entrada);
        assert_eq!(p, ctx.to_prompt());
    }
}
