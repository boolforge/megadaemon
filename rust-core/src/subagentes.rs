//! MegaDaemon — **subagentes**: el lead y los subagentes que dependen de él.
//!
//! ## Qué capacidad implementa
//!
//! Un **lead** (el agente principal) puede tener varios **subagentes**, cada
//! uno con su tarea, su presupuesto y su resultado.  El cliente puede
//! listarlos, obtener uno y **controlarlos**: promover a primer plano
//! (`foreground`), mandar a segundo plano (`background`) y cancelar
//! (`cancel`).  Sólo se anuncia si el cliente lo ha pedido (**opt-in
//! mutuo**).
//!
//! ## Fuentes MEDIDAS (documentadas; NO transcritas)
//!
//! | qué | dónde se midió |
//! |---|---|
//! | `cognition.ai/subagentSupport` — habilita los subagentes | `docs/RE_ACP.md:332` (fila 6 de la tabla §6.1) |
//! | `cognition.ai/subagentControl` — métodos `cognition.ai/subagent/{foreground,background,cancel}` (**opt-in mutuo**) | `docs/RE_ACP.md:333` (fila 7 de la tabla §6.1) |
//! | convergencia `mcode/session/delegation` ≈ `cognition.ai/subagentControl` | `docs/EXTENSIONES_PROPIETARIAS.md:37` y `docs/RE_ACP_COMPARADO.md:312` |
//! | las capacidades `cognition.ai/*` son **opt-in**: el agente sólo anuncia la que el cliente pide | `docs/RE_ACP.md:320` |
//!
//! ## Implementación NUESTRA
//!
//! Esto es **OUR Rust**, con el procedimiento AST → Rust del repo
//! (`docs/EXTENSIONES_PROPIETARIAS.md` §4, líneas 51-78): los nombres, la
//! estructura, la máquina de estados, los errores y el formato JSON son
//! **nuestros**.  De la referencia sólo se ha medido el **comportamiento
//! observable** (mismas entradas aceptadas y rechazadas, misma salida, mismos
//! efectos secundarios) y **no** se ha copiado ni transcrito código de ningún
//! producto de terceros.
//!
//! ## Qué NO se duplica
//!
//! * `agent_core.rs` es el planificador del lead (objetivo → tareas → cola):
//!   no sabe de subagentes, y este módulo no mete tareas en su cola.
//! * `meta_agent.rs` traduce una petición en lenguaje natural en un plan de
//!   ejecución: tampoco sabe de subagentes.
//! * `delegate.rs` delega en **cuentas** de otros proveedores (a qué cuenta,
//!   en qué orden, a qué coste): no es lo mismo que un subagente, que es un
//!   turno de trabajo **dentro** de este daemon.
//!
//! Este módulo es, por tanto, la **única** implementación de subagentes: el
//! resto habla con él.
//!
//! ## Nombres
//!
//! * Las **acciones** van en castellano, como las de las tools del repo
//!   (`acp_sesion`: `nueva`/`cancelar`/`reanudar`): `listar`, `obtener`,
//!   `foreground`, `background`, `cancelar`.
//! * Los **métodos de extensión** van en inglés con namespace, como los de
//!   `protocol_acp.rs` (`policy/evaluate`, `board/publish`):
//!   `subagent/list`, `subagent/get`, `subagent/foreground`,
//!   `subagent/background`, `subagent/cancel`.
//!
//! ## Reglas del turno
//!
//! * **Un subagente en primer plano ocupa el turno**: sólo puede haber uno,
//!   y mientras lo hay el lead no corre.
//! * **Cancelar (o terminar, o quedarse sin presupuesto) al subagente que
//!   tiene el turno devuelve el control al lead**.
//! * **Presupuesto agotado ⇒ el subagente se detiene solo** y queda dicho por
//!   qué (motivo + parte).
//!
//! ## El enganche que le toca al root
//!
//! ```ignore
//! // El router de ACP llama al despachador con las capacidades del cliente:
//! let cliente = subagentes::CapacidadesCliente::desde_meta(&meta_inicial);
//! "mdaemon/subagent/foreground" =>
//!     subagentes::opera(&mut subagentes, &cliente, "foreground", &params, ahora_s()),
//!
//! // Y las extensiones que se anuncian son las que el cliente ha pedido:
//! let exts = subagentes::anuncios(&cliente); // vacío si no ha declarado nada
//! ```
//!
//! Las extensiones a registrar salen de [`extensiones`] (la lista completa) o
//! de [`anuncios`] (sólo las que el cliente ha pedido: el opt-in medido).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// Capacidad que el cliente declara para poder **usar** subagentes.
pub const CAPACIDAD_SOPORTE: &str = "mdaemon/subagent/support";

/// Capacidad que el cliente declara para poder **controlarlos**.
pub const CAPACIDAD_CONTROL: &str = "mdaemon/subagent/control";

// ── Estados ───────────────────────────────────────────────────────────────

/// Estado de un subagente.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoSubagente {
    /// Vivo, sin turno: espera trabajo.
    Vivo,
    /// En segundo plano (trabaja, pero no tiene el turno).
    Fondo,
    /// En primer plano: **ocupa el turno**.
    PrimerPlano,
    /// Cancelado.
    Cancelado,
    /// Terminado (con resultado, o porque se quedó sin presupuesto).
    Terminado,
}

impl EstadoSubagente {
    /// Nombre estable (el que va al disco y al JSON).
    pub fn as_str(&self) -> &'static str {
        match self {
            EstadoSubagente::Vivo => "vivo",
            EstadoSubagente::Fondo => "fondo",
            EstadoSubagente::PrimerPlano => "primer_plano",
            EstadoSubagente::Cancelado => "cancelado",
            EstadoSubagente::Terminado => "terminado",
        }
    }

    /// ¿El estado ya no admite más cambios?
    pub fn es_terminal(&self) -> bool {
        matches!(
            self,
            EstadoSubagente::Cancelado | EstadoSubagente::Terminado
        )
    }
}

/// Quién tiene el turno ahora mismo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Turno {
    /// El lead (nadie en primer plano).
    Lead,
    /// Un subagente en primer plano (ocupa el turno).
    Subagente(String),
}

impl Default for Turno {
    /// Por defecto, el turno es del lead.
    fn default() -> Self {
        Turno::Lead
    }
}

// ── Errores ───────────────────────────────────────────────────────────────

/// Los errores de los subagentes, **tipados** (cada uno con su código).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorSubagente {
    /// No hay ningún subagente con ese id.
    SubagenteDesconocido {
        /// Id que no existe.
        id: String,
    },
    /// El padre indicado no es el lead de esta sesión.
    PadreDesconocido {
        /// El padre que no es el lead.
        padre: String,
    },
    /// El cliente no ha declarado la capacidad: la operación no se anuncia
    /// ni se atiende (opt-in mutuo).
    CapacidadNoAnunciada {
        /// El método que el cliente no pidió.
        metodo: String,
    },
    /// El subagente está en un estado terminal: no admite este cambio.
    EstadoTerminal {
        /// Id del subagente.
        id: String,
        /// Estado en el que está.
        estado: EstadoSubagente,
    },
    /// La operación no existe.
    OperacionDesconocida {
        /// La operación que no se reconoce.
        operacion: String,
    },
    /// Falta un campo obligatorio de la petición.
    PeticionIncompleta {
        /// El campo que falta.
        campo: String,
    },
}

impl ErrorSubagente {
    /// Código estable del error (el que va en el JSON).
    pub fn code(&self) -> &'static str {
        match self {
            ErrorSubagente::SubagenteDesconocido { .. } => "desconocido",
            ErrorSubagente::PadreDesconocido { .. } => "padre_desconocido",
            ErrorSubagente::CapacidadNoAnunciada { .. } => "capacidad_no_anunciada",
            ErrorSubagente::EstadoTerminal { .. } => "estado_terminal",
            ErrorSubagente::OperacionDesconocida { .. } => "operacion_desconocida",
            ErrorSubagente::PeticionIncompleta { .. } => "peticion_incompleta",
        }
    }

    /// Explicación para humanos (en castellano).
    pub fn mensaje(&self) -> String {
        match self {
            ErrorSubagente::SubagenteDesconocido { id } => {
                format!("no hay ningún subagente con el id '{}'", id)
            }
            ErrorSubagente::PadreDesconocido { padre } => {
                format!("'{}' no es el lead de esta sesión", padre)
            }
            ErrorSubagente::CapacidadNoAnunciada { metodo } => format!(
                "el cliente no ha declarado la capacidad de '{}': no se anuncia ni se atiende",
                metodo
            ),
            ErrorSubagente::EstadoTerminal { id, estado } => format!(
                "el subagente '{}' está '{}' y no admite más cambios",
                id,
                estado.as_str()
            ),
            ErrorSubagente::OperacionDesconocida { operacion } => {
                format!("operación desconocida: '{}'", operacion)
            }
            ErrorSubagente::PeticionIncompleta { campo } => {
                format!("falta el campo '{}' en la petición", campo)
            }
        }
    }

    /// El error en JSON (el `error` de una respuesta fallida).
    pub fn a_json(&self) -> Value {
        json!({ "code": self.code(), "message": self.mensaje() })
    }
}

// ── Modelo ────────────────────────────────────────────────────────────────

/// Un subagente del lead.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Subagente {
    /// Id estable del subagente.
    pub id: String,
    /// El padre (el lead que lo creó).
    pub padre: String,
    /// La tarea que tiene asignada.
    pub tarea: String,
    /// Estado actual.
    pub estado: EstadoSubagente,
    /// Presupuesto de tokens.
    pub presupuesto_tokens: u32,
    /// Tokens ya gastados.
    pub gastado_tokens: u32,
    /// Resultado (si terminó).
    pub resultado: Option<Value>,
    /// **Por qué** está en este estado (el motivo del último cambio).
    pub motivo: Option<String>,
    /// Epoch de creación.
    pub creado_s: u64,
    /// Epoch del último cambio.
    pub actualizado_s: u64,
}

impl Subagente {
    /// El subagente en JSON.
    pub fn a_json(&self) -> Value {
        json!({
            "id": self.id,
            "parent_id": self.padre,
            "task": self.tarea,
            "state": self.estado.as_str(),
            "budget_tokens": self.presupuesto_tokens,
            "spent_tokens": self.gastado_tokens,
            "result": self.resultado,
            "reason": self.motivo,
            "created_at": self.creado_s,
            "updated_at": self.actualizado_s,
        })
    }

    /// ¿Le queda presupuesto?
    pub fn tiene_presupuesto(&self) -> bool {
        self.gastado_tokens < self.presupuesto_tokens
    }
}

/// Qué pasó con un subagente (lo que sale en el historial).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accion {
    /// Se creó.
    Creado,
    /// Tomó el turno.
    Promovido,
    /// Dejó el turno (a segundo plano).
    Apartado,
    /// Se canceló.
    Cancelado,
    /// Terminó (con resultado).
    Terminado,
    /// Terminó porque se agotó el presupuesto.
    PresupuestoAgotado,
}

impl Accion {
    /// Nombre estable (el que va al disco y al JSON).
    pub fn as_str(&self) -> &'static str {
        match self {
            Accion::Creado => "creado",
            Accion::Promovido => "promovido",
            Accion::Apartado => "apartado",
            Accion::Cancelado => "cancelado",
            Accion::Terminado => "terminado",
            Accion::PresupuestoAgotado => "presupuesto_agotado",
        }
    }
}

/// Un evento del historial de los subagentes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventoSubagente {
    /// Secuencia monótona.
    pub seq: u64,
    /// Subagente afectado.
    pub subagente_id: String,
    /// Qué pasó.
    pub accion: Accion,
    /// Por qué.
    pub motivo: String,
    /// Epoch del cambio.
    pub momento_s: u64,
}

impl EventoSubagente {
    /// El evento en JSON.
    pub fn a_json(&self) -> Value {
        json!({
            "seq": self.seq,
            "subagent_id": self.subagente_id,
            "action": self.accion.as_str(),
            "reason": self.motivo,
            "at": self.momento_s,
        })
    }
}

/// Resultado de gastar presupuesto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gasto {
    /// Todavía queda presupuesto: el subagente sigue.
    Continua,
    /// Se agotó el presupuesto: el subagente se detiene solo.
    Agotado,
}

/// Filtro de listado de subagentes.
#[derive(Clone, Debug, Default)]
pub struct FiltroSubagentes {
    /// Sólo los subagentes en este estado (si no se pone, todos).
    pub estado: Option<EstadoSubagente>,
    /// Tope de filas (0 = sin tope).
    pub limite: usize,
}

// ── El modelo (puro) ──────────────────────────────────────────────────────

/// Los subagentes de un lead.
///
/// Modelo **puro**: no toca el disco ni la red.
#[derive(Clone, Debug, Default)]
pub struct Subagentes {
    /// El lead (el padre de todos).
    lead: String,
    /// Los subagentes.
    lista: Vec<Subagente>,
    /// Quién tiene el turno.
    turno: Turno,
    /// Contador de subagentes (ids).
    contador: u64,
    /// Contador de eventos.
    secuencia: u64,
    /// Historial de cambios.
    eventos: Vec<EventoSubagente>,
}

impl Subagentes {
    /// Subagentes de un lead (sin ninguno todavía).
    pub fn nuevo(lead: &str) -> Self {
        Subagentes {
            lead: lead.to_string(),
            lista: Vec::new(),
            turno: Turno::Lead,
            contador: 0,
            secuencia: 0,
            eventos: Vec::new(),
        }
    }

    /// El lead.
    pub fn lead(&self) -> &str {
        &self.lead
    }

    /// Quién tiene el turno.
    pub fn turno(&self) -> &Turno {
        &self.turno
    }

    /// Todos los subagentes.
    pub fn subagentes(&self) -> &[Subagente] {
        &self.lista
    }

    /// El historial de cambios.
    pub fn historial(&self) -> &[EventoSubagente] {
        &self.eventos
    }

    /// Un subagente por id.
    pub fn obtener(&self, id: &str) -> Result<&Subagente, ErrorSubagente> {
        self.lista
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| ErrorSubagente::SubagenteDesconocido { id: id.to_string() })
    }

    /// Lista los subagentes (con filtro opcional).
    pub fn listar(&self, filtro: &FiltroSubagentes) -> Vec<Subagente> {
        let mut out: Vec<Subagente> = self
            .lista
            .iter()
            .filter(|s| match filtro.estado {
                Some(e) => s.estado == e,
                None => true,
            })
            .cloned()
            .collect();
        if filtro.limite > 0 && out.len() > filtro.limite {
            out.truncate(filtro.limite);
        }
        out
    }

    // ── Crear ─────────────────────────────────────────────────────────────

    /// Crea un subagente del lead.
    ///
    /// El padre **tiene que ser el lead**: un subagente de otro subagente no
    /// existe en este modelo (ver [`ErrorSubagente::PadreDesconocido`]).
    pub fn crear(
        &mut self,
        padre: &str,
        tarea: &str,
        presupuesto_tokens: u32,
        ahora_s: u64,
    ) -> Result<Subagente, ErrorSubagente> {
        if padre != self.lead {
            return Err(ErrorSubagente::PadreDesconocido {
                padre: padre.to_string(),
            });
        }
        self.contador += 1;
        let id = format!("sa-{:04}", self.contador);
        let subagente = Subagente {
            id: id.clone(),
            padre: padre.to_string(),
            tarea: tarea.to_string(),
            estado: EstadoSubagente::Vivo,
            presupuesto_tokens,
            gastado_tokens: 0,
            resultado: None,
            motivo: Some("creado".to_string()),
            creado_s: ahora_s,
            actualizado_s: ahora_s,
        };
        self.lista.push(subagente.clone());
        self.registra(&subagente, Accion::Creado, "subagente creado", ahora_s);
        Ok(subagente)
    }

    // ── Controlar ─────────────────────────────────────────────────────────

    /// Promueve un subagente a **primer plano**: toma el turno.
    ///
    /// Sólo puede haber uno en primer plano: si lo había, ése baja a `vivo`
    /// (con su motivo) y el nuevo pasa a primer plano.
    pub fn foreground(&mut self, id: &str, ahora_s: u64) -> Result<Subagente, ErrorSubagente> {
        self.comprueba_vivo(id)?;
        let objetivo = self
            .lista
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| ErrorSubagente::SubagenteDesconocido { id: id.to_string() })?;
        if self.lista[objetivo].estado == EstadoSubagente::PrimerPlano {
            // Ya está en primer plano: no hay nada que cambiar.
            return Ok(self.lista[objetivo].clone());
        }
        // El que estaba en primer plano baja.
        let indice_previo = self
            .lista
            .iter()
            .position(|s| s.estado == EstadoSubagente::PrimerPlano);
        if let Some(previo) = indice_previo {
            let saliente = self.lista[previo].clone();
            self.lista[previo].estado = EstadoSubagente::Vivo;
            self.lista[previo].motivo = Some("degradado: otro subagente tomó el turno".to_string());
            self.lista[previo].actualizado_s = ahora_s;
            self.registra(
                &saliente,
                Accion::Apartado,
                "degradado: otro subagente tomó el turno",
                ahora_s,
            );
        }
        {
            let mensaje = &mut self.lista[objetivo];
            mensaje.estado = EstadoSubagente::PrimerPlano;
            mensaje.motivo = Some("promovido a primer plano".to_string());
            mensaje.actualizado_s = ahora_s;
        }
        self.turno = Turno::Subagente(id.to_string());
        let promovido = self.lista[objetivo].clone();
        self.registra(&promovido, Accion::Promovido, "toma el turno", ahora_s);
        Ok(promovido)
    }

    /// Manda un subagente a **segundo plano**.
    ///
    /// Si tenía el turno, el control vuelve al lead.
    pub fn background(&mut self, id: &str, ahora_s: u64) -> Result<Subagente, ErrorSubagente> {
        self.comprueba_vivo(id)?;
        let indice = self
            .lista
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| ErrorSubagente::SubagenteDesconocido { id: id.to_string() })?;
        if self.lista[indice].estado == EstadoSubagente::Fondo {
            return Ok(self.lista[indice].clone());
        }
        {
            let mensaje = &mut self.lista[indice];
            mensaje.estado = EstadoSubagente::Fondo;
            mensaje.motivo = Some("mandado a segundo plano".to_string());
            mensaje.actualizado_s = ahora_s;
        }
        let tenia_el_turno = self.turno == Turno::Subagente(id.to_string());
        if tenia_el_turno {
            self.turno = Turno::Lead;
        }
        let apartado = self.lista[indice].clone();
        self.registra(&apartado, Accion::Apartado, "a segundo plano", ahora_s);
        Ok(apartado)
    }

    /// Cancela un subagente.
    ///
    /// Si tenía el turno, **el lead recupera el control**.
    pub fn cancelar(
        &mut self,
        id: &str,
        motivo: &str,
        ahora_s: u64,
    ) -> Result<Subagente, ErrorSubagente> {
        self.comprueba_vivo(id)?;
        let indice = self
            .lista
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| ErrorSubagente::SubagenteDesconocido { id: id.to_string() })?;
        {
            let mensaje = &mut self.lista[indice];
            mensaje.estado = EstadoSubagente::Cancelado;
            mensaje.motivo = Some(motivo.to_string());
            mensaje.actualizado_s = ahora_s;
        }
        let tenia_el_turno = self.turno == Turno::Subagente(id.to_string());
        if tenia_el_turno {
            self.turno = Turno::Lead;
        }
        let cancelado = self.lista[indice].clone();
        self.registra(&cancelado, Accion::Cancelado, motivo, ahora_s);
        Ok(cancelado)
    }

    // ── Gastar y terminar ─────────────────────────────────────────────────

    /// Gasta presupuesto de un subagente.
    ///
    /// Si el presupuesto se agota, el subagente **se detiene solo**: queda
    /// en `terminado` y el motivo dice por qué (`presupuesto agotado`).  Si
    /// tenía el turno, el control vuelve al lead.
    pub fn consumir(
        &mut self,
        id: &str,
        tokens: u32,
        ahora_s: u64,
    ) -> Result<Gasto, ErrorSubagente> {
        self.comprueba_vivo(id)?;
        let indice = self
            .lista
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| ErrorSubagente::SubagenteDesconocido { id: id.to_string() })?;
        let agotado;
        {
            let mensaje = &mut self.lista[indice];
            mensaje.gastado_tokens = mensaje.gastado_tokens.saturating_add(tokens);
            agotado = mensaje.gastado_tokens >= mensaje.presupuesto_tokens;
            if agotado {
                mensaje.estado = EstadoSubagente::Terminado;
                mensaje.motivo = Some(format!(
                    "presupuesto agotado ({} de {} tokens)",
                    mensaje.gastado_tokens, mensaje.presupuesto_tokens
                ));
                mensaje.actualizado_s = ahora_s;
            }
        }
        if agotado {
            let tenia_el_turno = self.turno == Turno::Subagente(id.to_string());
            if tenia_el_turno {
                self.turno = Turno::Lead;
            }
            let parada = self.lista[indice].clone();
            let motivo = parada.motivo.clone().unwrap_or_default();
            self.registra(&parada, Accion::PresupuestoAgotado, &motivo, ahora_s);
            Ok(Gasto::Agotado)
        } else {
            let mensaje = self.lista[indice].clone();
            self.registra(&mensaje, Accion::Apartado, "presupuesto consumido", ahora_s);
            Ok(Gasto::Continua)
        }
    }

    /// Termina un subagente con su resultado.
    ///
    /// Si tenía el turno, el control vuelve al lead.
    pub fn terminar(
        &mut self,
        id: &str,
        resultado: Value,
        ahora_s: u64,
    ) -> Result<Subagente, ErrorSubagente> {
        self.comprueba_vivo(id)?;
        let indice = self
            .lista
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| ErrorSubagente::SubagenteDesconocido { id: id.to_string() })?;
        {
            let mensaje = &mut self.lista[indice];
            mensaje.estado = EstadoSubagente::Terminado;
            mensaje.resultado = Some(resultado);
            mensaje.motivo = Some("terminado".to_string());
            mensaje.actualizado_s = ahora_s;
        }
        let tenia_el_turno = self.turno == Turno::Subagente(id.to_string());
        if tenia_el_turno {
            self.turno = Turno::Lead;
        }
        let terminado = self.lista[indice].clone();
        self.registra(&terminado, Accion::Terminado, "terminado", ahora_s);
        Ok(terminado)
    }

    // ── Internos ──────────────────────────────────────────────────────────

    /// Comprueba que el subagente existe y no está en un estado terminal.
    fn comprueba_vivo(&self, id: &str) -> Result<(), ErrorSubagente> {
        let subagente = self.obtener(id)?;
        if subagente.estado.es_terminal() {
            return Err(ErrorSubagente::EstadoTerminal {
                id: id.to_string(),
                estado: subagente.estado,
            });
        }
        Ok(())
    }

    /// Anota un evento en el historial.
    fn registra(&mut self, subagente: &Subagente, accion: Accion, motivo: &str, ahora_s: u64) {
        self.secuencia += 1;
        self.eventos.push(EventoSubagente {
            seq: self.secuencia,
            subagente_id: subagente.id.clone(),
            accion,
            motivo: motivo.to_string(),
            momento_s: ahora_s,
        });
    }

    // ── Informe ───────────────────────────────────────────────────────────

    /// El estado de los subagentes en JSON (la vista que ve el cliente).
    pub fn info(&self) -> Value {
        json!({
            "ok": true,
            "lead": self.lead,
            "turn": match &self.turno {
                Turno::Lead => json!({"kind": "lead"}),
                Turno::Subagente(id) => json!({"kind": "subagent", "id": id}),
            },
            "subagents": self.lista.iter().map(|s| s.a_json()).collect::<Vec<_>>(),
            "total": self.lista.len(),
        })
    }
}

// ── Capacidades (opt-in mutuo) ───────────────────────────────────────────

/// Lo que el cliente ha declarado que extiende.
///
/// MEDIDO: el opt-in es **mutuo** (`docs/RE_ACP.md:333`): el cliente declara
/// la capacidad y el agente sólo anuncia la que se le ha pedido
/// (`docs/RE_ACP.md:320`).  Si el cliente no declara nada, **no** se anuncia
/// nada.
///
/// Este módulo lleva **su propio** juego de capacidades para no depender de
/// `cola_mensajes.rs`: el root construye las dos desde el mismo `_meta` del
/// `initialize`.
#[derive(Clone, Debug, Default)]
pub struct CapacidadesCliente {
    declaradas: BTreeSet<String>,
}

impl CapacidadesCliente {
    /// Sin nada declarado (nada se anuncia).
    pub fn nueva() -> Self {
        CapacidadesCliente::default()
    }

    /// Declara una capacidad.  Devuelve `false` si ya estaba.
    pub fn declara(&mut self, capacidad: &str) -> bool {
        self.declaradas.insert(capacidad.to_string())
    }

    /// ¿El cliente ha declarado esta capacidad?
    pub fn pide(&self, capacidad: &str) -> bool {
        self.declaradas.contains(capacidad)
    }

    /// Las capacidades declaradas (ordenadas).
    pub fn declaradas(&self) -> Vec<String> {
        self.declaradas.iter().cloned().collect()
    }

    /// Lee las declaraciones de un `_meta` de ACP.
    ///
    /// Acepta las dos formas: una clave por capacidad
    /// (`{"mdaemon/subagent/control": true}`) y una lista
    /// (`{"capacidades": ["mdaemon/subagent/control"]}`).
    pub fn desde_meta(meta: &Value) -> Self {
        let mut c = CapacidadesCliente::nueva();
        if let Some(obj) = meta.as_object() {
            for (clave, valor) in obj {
                if clave == "capacidades" || clave == "capabilities" {
                    if let Some(lista) = valor.as_array() {
                        for entrada in lista {
                            if let Some(s) = entrada.as_str() {
                                c.declara(s);
                            }
                        }
                    }
                } else if valor.as_bool() == Some(true) {
                    c.declara(clave);
                }
            }
        }
        c
    }
}

/// Las capacidades que **nuestro** agente sabe ofrecer.
pub fn capacidades_agente() -> Vec<&'static str> {
    vec![CAPACIDAD_SOPORTE, CAPACIDAD_CONTROL]
}

// ── Extensiones (las registra el root) ────────────────────────────────────

/// Las extensiones **propias** que propone este módulo.
///
/// Devuelve la lista `(método, para qué)`; el root la convierte en
/// `protocol_acp::Extension { namespace: "mdaemon", metodo, para_que }` (ver
/// `rust-core/src/protocol_acp.rs`).  Se devuelve una lista **propia** (sin
/// tipos del crate) para que este módulo no dependa de nada.
pub fn extensiones() -> Vec<(&'static str, &'static str)> {
    vec![
        ("subagent/list", "listar los subagentes del lead"),
        ("subagent/get", "obtener un subagente por id"),
        (
            "subagent/foreground",
            "promover un subagente a primer plano (toma el turno)",
        ),
        (
            "subagent/background",
            "mandar un subagente a segundo plano (el lead recupera el turno)",
        ),
        (
            "subagent/cancel",
            "cancelar un subagente (el lead recupera el control)",
        ),
    ]
}

/// Los métodos que se anuncian a un cliente, según lo que haya declarado.
///
/// * Sin declaraciones: **nada** se anuncia (opt-in medido).
/// * Con [`CAPACIDAD_SOPORTE`]: se anuncian las consultas.
/// * Con [`CAPACIDAD_CONTROL`]: se anuncian además las de control.
pub fn anuncios(cliente: &CapacidadesCliente) -> Vec<(&'static str, &'static str)> {
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
    if cliente.pide(CAPACIDAD_SOPORTE) {
        out.push(("subagent/list", "listar los subagentes del lead"));
        out.push(("subagent/get", "obtener un subagente por id"));
    }
    if cliente.pide(CAPACIDAD_CONTROL) {
        out.push((
            "subagent/foreground",
            "promover un subagente a primer plano (toma el turno)",
        ));
        out.push((
            "subagent/background",
            "mandar un subagente a segundo plano (el lead recupera el turno)",
        ));
        out.push((
            "subagent/cancel",
            "cancelar un subagente (el lead recupera el control)",
        ));
    }
    out
}

/// ¿Se puede anunciar (y atender) este método a este cliente?
pub fn permitido(cliente: &CapacidadesCliente, metodo: &str) -> bool {
    anuncios(cliente).iter().any(|(m, _)| *m == metodo)
}

// ── El despachador (la capa fina que el root enchufa) ─────────────────────

/// Las operaciones que este módulo sabe hacer.
pub const OPERACIONES: &[&str] = &["listar", "obtener", "foreground", "background", "cancelar"];

/// El método de extensión que corresponde a cada operación.
fn metodo_de(operacion: &str) -> &'static str {
    match operacion {
        "listar" => "subagent/list",
        "obtener" => "subagent/get",
        "foreground" => "subagent/foreground",
        "background" => "subagent/background",
        "cancelar" => "subagent/cancel",
        _ => "",
    }
}

/// Ejecuta una operación sobre los subagentes **respetando el opt-in**.
///
/// Si el cliente no ha declarado la capacidad, la operación **no se atiende**
/// y se devuelve el error tipado [`ErrorSubagente::CapacidadNoAnunciada`]:
/// es el mismo comportamiento que no tener el método en el protocolo.
pub fn opera(
    subagentes: &mut Subagentes,
    cliente: &CapacidadesCliente,
    operacion: &str,
    params: &Value,
    ahora_s: u64,
) -> Result<Value, ErrorSubagente> {
    let metodo = metodo_de(operacion);
    if metodo.is_empty() {
        return Err(ErrorSubagente::OperacionDesconocida {
            operacion: operacion.to_string(),
        });
    }
    if !permitido(cliente, metodo) {
        return Err(ErrorSubagente::CapacidadNoAnunciada {
            metodo: metodo.to_string(),
        });
    }
    match operacion {
        "listar" => {
            let filtro = FiltroSubagentes {
                estado: params
                    .get("state")
                    .and_then(|v| v.as_str())
                    .and_then(estado_desde_str),
                limite: params.get("limit").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
            };
            let lista = subagentes.listar(&filtro);
            Ok(json!({
                "ok": true,
                "subagents": lista.iter().map(|s| s.a_json()).collect::<Vec<_>>(),
                "total": lista.len(),
                "turn": match subagentes.turno() {
                    Turno::Lead => json!({"kind": "lead"}),
                    Turno::Subagente(id) => json!({"kind": "subagent", "id": id}),
                },
            }))
        }
        "obtener" => {
            let id = texto_obligatorio(params, "id")?;
            let subagente = subagentes.obtener(&id)?;
            Ok(json!({ "ok": true, "subagent": subagente.a_json() }))
        }
        "foreground" => {
            let id = texto_obligatorio(params, "id")?;
            let subagente = subagentes.foreground(&id, ahora_s)?;
            Ok(json!({ "ok": true, "subagent": subagente.a_json() }))
        }
        "background" => {
            let id = texto_obligatorio(params, "id")?;
            let subagente = subagentes.background(&id, ahora_s)?;
            Ok(json!({ "ok": true, "subagent": subagente.a_json() }))
        }
        "cancelar" => {
            let id = texto_obligatorio(params, "id")?;
            let motivo = params
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("cancelado por el cliente")
                .to_string();
            let subagente = subagentes.cancelar(&id, &motivo, ahora_s)?;
            Ok(json!({ "ok": true, "subagent": subagente.a_json() }))
        }
        otro => Err(ErrorSubagente::OperacionDesconocida {
            operacion: otro.to_string(),
        }),
    }
}

/// El estado desde su nombre estable.
fn estado_desde_str(s: &str) -> Option<EstadoSubagente> {
    match s {
        "vivo" => Some(EstadoSubagente::Vivo),
        "fondo" => Some(EstadoSubagente::Fondo),
        "primer_plano" => Some(EstadoSubagente::PrimerPlano),
        "cancelado" => Some(EstadoSubagente::Cancelado),
        "terminado" => Some(EstadoSubagente::Terminado),
        _ => None,
    }
}

/// Lee un campo de texto obligatorio.
fn texto_obligatorio(params: &Value, campo: &str) -> Result<String, ErrorSubagente> {
    params
        .get(campo)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| ErrorSubagente::PeticionIncompleta {
            campo: campo.to_string(),
        })
}

/// El epoch de ahora.
pub fn ahora_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Lead con dos subagentes.
    fn subagentes_de_prueba() -> Subagentes {
        let mut s = Subagentes::nuevo("lead-1");
        s.crear("lead-1", "busca el bug", 1000, 10).unwrap();
        s.crear("lead-1", "escribe el informe", 1000, 20).unwrap();
        s
    }

    /// Cliente que ha declarado sólo el soporte (consultas).
    fn cliente_soporte() -> CapacidadesCliente {
        let mut c = CapacidadesCliente::nueva();
        c.declara(CAPACIDAD_SOPORTE);
        c
    }

    /// Cliente que ha declarado las dos capacidades.
    fn cliente_completo() -> CapacidadesCliente {
        let mut c = cliente_soporte();
        c.declara(CAPACIDAD_CONTROL);
        c
    }

    #[test]
    fn crear_deja_al_subagente_vivo_y_con_presupuesto() {
        let s = subagentes_de_prueba();
        assert_eq!(s.subagentes().len(), 2);
        let sub = &s.subagentes()[0];
        assert_eq!(sub.estado, EstadoSubagente::Vivo);
        assert_eq!(sub.padre, "lead-1");
        assert_eq!(sub.tarea, "busca el bug");
        assert_eq!(sub.presupuesto_tokens, 1000);
        assert_eq!(sub.gastado_tokens, 0);
        assert!(sub.resultado.is_none());
        assert_eq!(s.turno(), &Turno::Lead);
    }

    #[test]
    fn foreground_promueve_y_toma_el_turno() {
        let mut s = subagentes_de_prueba();
        let id = s.subagentes()[1].id.clone();
        let promovido = s.foreground(&id, 30).unwrap();
        assert_eq!(promovido.estado, EstadoSubagente::PrimerPlano);
        assert_eq!(s.turno(), &Turno::Subagente(id.clone()));
        // Y el lead ya no tiene el turno.
        assert_ne!(s.turno(), &Turno::Lead);
    }

    #[test]
    fn foreground_baja_al_que_ya_estaba_en_primer_plano() {
        let mut s = subagentes_de_prueba();
        let primero = s.subagentes()[0].id.clone();
        let segundo = s.subagentes()[1].id.clone();
        s.foreground(&primero, 40).unwrap();
        s.foreground(&segundo, 50).unwrap();
        // Sólo puede haber uno en primer plano.
        let en_primer_plano: Vec<&Subagente> = s
            .subagentes()
            .iter()
            .filter(|x| x.estado == EstadoSubagente::PrimerPlano)
            .collect();
        assert_eq!(en_primer_plano.len(), 1);
        assert_eq!(en_primer_plano[0].id, segundo);
        // El que bajó queda vivo y con su motivo.
        let bajado = s.obtener(&primero).unwrap();
        assert_eq!(bajado.estado, EstadoSubagente::Vivo);
        assert!(bajado.motivo.as_ref().unwrap().contains("degradado"));
    }

    #[test]
    fn background_manda_a_segundo_plano_y_devuelve_el_turno() {
        let mut s = subagentes_de_prueba();
        let id = s.subagentes()[0].id.clone();
        s.foreground(&id, 60).unwrap();
        let apartado = s.background(&id, 70).unwrap();
        assert_eq!(apartado.estado, EstadoSubagente::Fondo);
        // Al apartado, el lead recupera el turno.
        assert_eq!(s.turno(), &Turno::Lead);
    }

    #[test]
    fn cancelar_un_subagente_en_primer_plano_devuelve_el_control_al_lead() {
        let mut s = subagentes_de_prueba();
        let id = s.subagentes()[0].id.clone();
        s.foreground(&id, 80).unwrap();
        assert_eq!(s.turno(), &Turno::Subagente(id.clone()));
        let cancelado = s.cancelar(&id, "cancelado por el usuario", 90).unwrap();
        assert_eq!(cancelado.estado, EstadoSubagente::Cancelado);
        assert_eq!(
            cancelado.motivo.as_deref(),
            Some("cancelado por el usuario")
        );
        // El lead recupera el control.
        assert_eq!(s.turno(), &Turno::Lead);
        // Y no se puede volver a tocar.
        let err = s.foreground(&id, 100).unwrap_err();
        assert_eq!(err.code(), "estado_terminal");
    }

    #[test]
    fn el_presupuesto_agotado_lo_detiene_y_reporta_el_motivo() {
        let mut s = Subagentes::nuevo("lead-1");
        s.crear("lead-1", "tarea corta", 100, 110).unwrap();
        let id = s.subagentes()[0].id.clone();
        s.foreground(&id, 120).unwrap();
        // Todavía queda presupuesto.
        assert_eq!(s.consumir(&id, 40, 130).unwrap(), Gasto::Continua);
        assert_eq!(s.obtener(&id).unwrap().gastado_tokens, 40);
        assert_eq!(s.obtener(&id).unwrap().estado, EstadoSubagente::PrimerPlano);
        // Al agotarlo se detiene solo y dice por qué.
        assert_eq!(s.consumir(&id, 60, 140).unwrap(), Gasto::Agotado);
        let parado = s.obtener(&id).unwrap();
        assert_eq!(parado.estado, EstadoSubagente::Terminado);
        assert!(parado
            .motivo
            .as_ref()
            .unwrap()
            .contains("presupuesto agotado"));
        // Y el lead recupera el control.
        assert_eq!(s.turno(), &Turno::Lead);
    }

    #[test]
    fn sin_declaracion_del_cliente_no_se_anuncia_nada() {
        // MEDIDO: opt-in (`docs/RE_ACP.md:320` y `:333`).
        let vacio = CapacidadesCliente::nueva();
        assert!(anuncios(&vacio).is_empty());
        assert!(!permitido(&vacio, "subagent/list"));
        assert!(!permitido(&vacio, "subagent/foreground"));
    }

    #[test]
    fn con_declaracion_se_anuncian_las_extensiones() {
        let cliente = cliente_completo();
        let anuncios = anuncios(&cliente);
        assert_eq!(anuncios.len(), 5);
        let metodos: Vec<&str> = anuncios.iter().map(|(m, _)| *m).collect();
        assert!(metodos.contains(&"subagent/list"));
        assert!(metodos.contains(&"subagent/get"));
        assert!(metodos.contains(&"subagent/foreground"));
        assert!(metodos.contains(&"subagent/background"));
        assert!(metodos.contains(&"subagent/cancel"));
        // Y cada uno lleva su "para qué".
        for (_, para_que) in &anuncios {
            assert!(!para_que.is_empty());
        }
    }

    #[test]
    fn la_capacidad_de_control_depende_de_su_declaracion() {
        // Sólo soporte => sólo consultas.
        let solo_soporte = cliente_soporte();
        let metodos: Vec<&str> = anuncios(&solo_soporte).iter().map(|(m, _)| *m).collect();
        assert_eq!(metodos.len(), 2);
        assert!(metodos.contains(&"subagent/list"));
        assert!(!metodos.contains(&"subagent/foreground"));
        // Con control además => los tres de control.
        let completo = cliente_completo();
        assert!(permitido(&completo, "subagent/foreground"));
        assert!(permitido(&completo, "subagent/background"));
        assert!(permitido(&completo, "subagent/cancel"));
    }

    #[test]
    fn la_llamada_sin_capacidad_se_rechaza() {
        let mut s = subagentes_de_prueba();
        let id = s.subagentes()[0].id.clone();
        let vacio = CapacidadesCliente::nueva();
        // Ni se anuncia ni se atiende.
        let err = opera(&mut s, &vacio, "foreground", &json!({"id": id}), 150).unwrap_err();
        assert_eq!(
            err,
            ErrorSubagente::CapacidadNoAnunciada {
                metodo: "subagent/foreground".to_string()
            }
        );
        assert_eq!(err.code(), "capacidad_no_anunciada");
        // Y el estado no ha cambiado.
        assert_eq!(s.obtener(&id).unwrap().estado, EstadoSubagente::Vivo);
    }

    #[test]
    fn listar_y_obtener_dan_los_subagentes() {
        let mut s = subagentes_de_prueba();
        let id = s.subagentes()[0].id.clone();
        let lista = s.listar(&FiltroSubagentes::default());
        assert_eq!(lista.len(), 2);
        assert_eq!(s.obtener(&id).unwrap().id, id);
        // Filtrando por estado.
        s.foreground(&id, 160).unwrap();
        let solo = s.listar(&FiltroSubagentes {
            estado: Some(EstadoSubagente::PrimerPlano),
            limite: 0,
        });
        assert_eq!(solo.len(), 1);
        assert_eq!(solo[0].id, id);
        // Un id que no existe: error tipado.
        assert_eq!(s.obtener("no-existe").unwrap_err().code(), "desconocido");
    }

    #[test]
    fn terminar_un_subagente_devuelve_el_turno_al_lead() {
        let mut s = subagentes_de_prueba();
        let id = s.subagentes()[0].id.clone();
        s.foreground(&id, 170).unwrap();
        let terminado = s.terminar(&id, json!({"ok": true}), 180).unwrap();
        assert_eq!(terminado.estado, EstadoSubagente::Terminado);
        assert_eq!(terminado.resultado, Some(json!({"ok": true})));
        assert_eq!(s.turno(), &Turno::Lead);
    }

    #[test]
    fn el_despachador_atiende_las_cinco_operaciones() {
        let mut s = subagentes_de_prueba();
        let cliente = cliente_completo();
        let id = s.subagentes()[0].id.clone();
        let listado = opera(&mut s, &cliente, "listar", &json!({}), 190).unwrap();
        assert_eq!(listado["total"], json!(2));
        let obtenido = opera(&mut s, &cliente, "obtener", &json!({"id": id}), 191).unwrap();
        assert_eq!(obtenido["subagent"]["id"], json!(id));
        let promovido = opera(&mut s, &cliente, "foreground", &json!({"id": id}), 192).unwrap();
        assert_eq!(promovido["subagent"]["state"], json!("primer_plano"));
        let apartado = opera(&mut s, &cliente, "background", &json!({"id": id}), 193).unwrap();
        assert_eq!(apartado["subagent"]["state"], json!("fondo"));
        let cancelado = opera(
            &mut s,
            &cliente,
            "cancelar",
            &json!({"id": id, "reason": "ya no hace falta"}),
            194,
        )
        .unwrap();
        assert_eq!(cancelado["subagent"]["state"], json!("cancelado"));
        // Operación desconocida.
        assert_eq!(
            opera(&mut s, &cliente, "inventada", &json!({}), 195)
                .unwrap_err()
                .code(),
            "operacion_desconocida"
        );
        // Petición incompleta.
        assert_eq!(
            opera(&mut s, &cliente, "obtener", &json!({}), 196)
                .unwrap_err()
                .code(),
            "peticion_incompleta"
        );
    }

    #[test]
    fn las_capacidades_se_leen_del_meta_del_cliente() {
        // Forma "una clave por capacidad".
        let meta = json!({ "mdaemon/subagent/support": true, "otra": false });
        let cliente = CapacidadesCliente::desde_meta(&meta);
        assert!(cliente.pide(CAPACIDAD_SOPORTE));
        assert!(!cliente.pide(CAPACIDAD_CONTROL));
        // Forma "lista".
        let meta2 = json!({ "capacidades": ["mdaemon/subagent/control"] });
        let cliente2 = CapacidadesCliente::desde_meta(&meta2);
        assert!(cliente2.pide(CAPACIDAD_CONTROL));
        assert!(!cliente2.pide(CAPACIDAD_SOPORTE));
        // Sin meta: nada declarado.
        let cliente3 = CapacidadesCliente::desde_meta(&json!({}));
        assert!(cliente3.declaradas().is_empty());
        assert!(anuncios(&cliente3).is_empty());
    }

    #[test]
    fn el_historial_explica_cada_cambio() {
        let mut s = subagentes_de_prueba();
        let id = s.subagentes()[0].id.clone();
        s.foreground(&id, 200).unwrap();
        s.background(&id, 210).unwrap();
        s.cancelar(&id, "fin", 220).unwrap();
        let mio: Vec<&EventoSubagente> = s
            .historial()
            .iter()
            .filter(|e| e.subagente_id == id)
            .collect();
        assert_eq!(mio.len(), 4);
        assert_eq!(mio[0].accion, Accion::Creado);
        assert_eq!(mio[1].accion, Accion::Promovido);
        assert_eq!(mio[2].accion, Accion::Apartado);
        assert_eq!(mio[3].accion, Accion::Cancelado);
        assert_eq!(mio[3].motivo, "fin");
    }

    #[test]
    fn las_extensiones_propias_cuadran_con_las_operaciones() {
        let ext = extensiones();
        assert_eq!(ext.len(), OPERACIONES.len());
        let nombres: Vec<&str> = ext.iter().map(|(m, _)| *m).collect();
        assert!(nombres.contains(&"subagent/list"));
        assert!(nombres.contains(&"subagent/get"));
        assert!(nombres.contains(&"subagent/foreground"));
        assert!(nombres.contains(&"subagent/background"));
        assert!(nombres.contains(&"subagent/cancel"));
        // Con el cliente completo, los anuncios coinciden con la lista propia.
        let cliente = cliente_completo();
        let metodos: Vec<&str> = anuncios(&cliente).iter().map(|(m, _)| *m).collect();
        assert_eq!(metodos, nombres);
    }
}
