//! MegaDaemon — **cola de mensajes**: la cola de envío de una sesión.
//!
//! ## Qué capacidad implementa
//!
//! La cola de mensajes **encolados** de una sesión: encolar un mensaje,
//! listar la cola, actualizar un mensaje encolado, borrarlo y **redirigir**
//! ("steer") uno que ya está encolado para que tome el turno.  Cada cambio
//! deja su razón (el "por qué") en el historial y se puede publicar por el
//! bus, de modo que el cliente recibe la **actualización de información** de
//! la cola (`messages_in_queued`).
//!
//! ## Fuentes MEDIDAS (documentadas; NO transcritas)
//!
//! | qué | dónde se midió |
//! |---|---|
//! | la cola de envío se lee de `messages_in_queued` (`cognition.ai/queuedMessages` en `_meta`) | `docs/RE_ACP.md:331` (fila 5 de la tabla §6.1) |
//! | convergencia `mcode/session/queue/{list,enqueue,update,delete,steer}` ≈ `cognition.ai/queuedMessagesInfoUpdate` | `docs/RE_ACP_COMPARADO.md:307` y `docs/EXTENSIONES_PROPIETARIAS.md:39` |
//! | "la cola ES el protocolo" (no es un añadido nuestro) | `docs/EXTENSIONES_PROPIETARIAS.md:39` |
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
//! ## Nombres
//!
//! * Las **acciones** van en castellano, como las de las tools del repo
//!   (`plugin_data`: `listar`/`escribir`/`purgar`; `acp_sesion`:
//!   `nueva`/`cancelar`/`reanudar`): `listar`, `encolar`, `actualizar`,
//!   `borrar`, `redirigir`.
//! * Los **métodos de extensión** van en inglés con namespace, como los de
//!   `protocol_acp.rs` (`policy/evaluate`, `board/publish`): `queue/list`,
//!   `queue/enqueue`, `queue/update`, `queue/delete`, `queue/steer`.
//!
//! ## Cómo se cuelga del bus real
//!
//! El catálogo del repo asigna esta capacidad a `service_bus.rs`
//! (`docs/EXTENSIONES_PROPIETARIAS.md:39`), y ese fichero no se toca aquí.
//! Por eso este módulo deja el **enchufe** preparado y no una copia del bus:
//!
//! * la cola es un modelo **puro** (sin disco, sin red): todo lo decidible
//!   se decide aquí y se puede testear sin E/S;
//! * la E/S es **fina**: [`Cola::guarda`]/[`Cola::carga`] (un fichero JSON)
//!   y [`BusEventos`], un *trait* de un solo método que el root implementa
//!   sobre el bus que ya hay (el `service_bus` o los eventos del tablón
//!   `board.rs`).  Trae dos implementaciones de serie: [`BusEnMemoria`] (para
//!   tests) y [`BusNdjson`] (un journal por línea, como los del repo);
//! * [`opera`] es el **despachador** fino: recibe la acción y sus parámetros
//!   en JSON y devuelve el mismo JSON.  El root lo engancha tal cual a la
//!   extensión `mdaemon/queue/*` o a la tool MCP.
//!
//! ### El enganche que le toca al root
//!
//! ```ignore
//! // 1) El bus real se suscribe a los eventos de la cola (en `service_bus.rs`):
//! impl cola_mensajes::BusEventos for ServiceBus {
//!     fn registra(&mut self, ev: &cola_mensajes::Evento) -> Result<(), String> {
//!         self.publica_en_el_tablon("cola", &ev.a_json()); // nombre ilustrativo
//!         Ok(())
//!     }
//! }
//!
//! // 2) Y el despachador responde a `mdaemon/queue/*` (en el router de ACP):
//! "mdaemon/queue/enqueue" => cola_mensajes::opera(&mut cola, "encolar", &params, ahora_s()),
//! "mdaemon/queue/list"    => cola_mensajes::opera(&mut cola, "listar", &params, ahora_s()),
//! ```
//!
//! Las extensiones a registrar salen de [`extensiones`] (o de [`anuncios`],
//! si el cliente ha declarado [`CAPACIDAD_COLA`]).
//!
//! ## Lo que NO está medido
//!
//! * La referencia publica el estado de la cola, pero **no** publica un
//!   historial de cambios: el **historial** de este módulo es una decisión
//!   NUESTRA (para que borrar un mensaje no borre su traza) y por eso el
//!   módulo lo dice en vez de fingir que está medido.
//! * La persistencia en fichero JSON es NUESTRA: la referencia lleva la
//!   cola en el protocolo, no en un fichero.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Capacidad que el cliente declara para poder usar la cola.
///
/// MEDIDO: las capacidades del dialecto propietario son **opt-in** (el agente
/// sólo anuncia la que el cliente pide) — `docs/RE_ACP.md:320`.
pub const CAPACIDAD_COLA: &str = "mdaemon/queue";

// ── Estados ───────────────────────────────────────────────────────────────

/// Estado de un mensaje dentro de la cola.
///
/// La cola tiene **un** estado "en línea" (`Encolado`) y tres estados
/// terminales.  Los terminales **no** salen de la estructura: el registro se
/// conserva para poder preguntar por él (el mensaje ya consumido, el que
/// falló, el que se canceló), pero dejan de estar *en línea*.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoMensaje {
    /// En la cola, esperando su turno.
    Encolado,
    /// Ya se entregó: salió de la cola, pero el registro se conserva.
    Consumido,
    /// La entrega falló (queda el motivo).
    Fallido,
    /// Retirado de la cola a petición (el registro se conserva).
    Cancelado,
}

impl EstadoMensaje {
    /// Nombre estable (el que va al disco y al JSON).
    pub fn as_str(&self) -> &'static str {
        match self {
            EstadoMensaje::Encolado => "encolado",
            EstadoMensaje::Consumido => "consumido",
            EstadoMensaje::Fallido => "fallido",
            EstadoMensaje::Cancelado => "cancelado",
        }
    }

    /// ¿El estado ya no admite más cambios?
    pub fn es_terminal(&self) -> bool {
        !matches!(self, EstadoMensaje::Encolado)
    }
}

/// Qué pasó con un mensaje (lo que sale en el historial).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accion {
    /// Entró en la cola.
    Encolado,
    /// Cambió su texto o su prioridad.
    Actualizado,
    /// Tomó el turno (redirección).
    Redirigido,
    /// Se entregó.
    Consumido,
    /// La entrega falló.
    Fallido,
    /// Se retiró de la cola sin entregarse.
    Cancelado,
    /// Se borró de la cola (sólo queda la traza).
    Eliminado,
}

impl Accion {
    /// Nombre estable (el que va al disco y al JSON).
    pub fn as_str(&self) -> &'static str {
        match self {
            Accion::Encolado => "encolado",
            Accion::Actualizado => "actualizado",
            Accion::Redirigido => "redirigido",
            Accion::Consumido => "consumido",
            Accion::Fallido => "fallido",
            Accion::Cancelado => "cancelado",
            Accion::Eliminado => "eliminado",
        }
    }
}

// ── Errores ───────────────────────────────────────────────────────────────

/// Los errores de la cola, **tipados** (cada uno con su código).
///
/// Los cuatro que exige el contrato medido son [`ErrorCola::MensajeDuplicado`],
/// [`ErrorCola::SesionDesconocida`], [`ErrorCola::ColaVacia`] y
/// [`ErrorCola::MensajeYaConsumido`]; los demás cubren los huecos que deja
/// una API con estados (mensaje que no existe, mensaje que ya no se puede
/// tocar, acción desconocida, petición incompleta).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorCola {
    /// Ya hay un mensaje encolado con ese id.
    MensajeDuplicado {
        /// Id del mensaje que se repite.
        id: String,
    },
    /// La sesión no está registrada en la cola.
    SesionDesconocida {
        /// La sesión que no existe.
        sesion: String,
    },
    /// No hay ningún mensaje que listar o que tomar.
    ColaVacia,
    /// El mensaje ya se entregó: no admite más cambios.
    MensajeYaConsumido {
        /// Id del mensaje ya consumido.
        id: String,
    },
    /// Ese id no está en la cola.
    MensajeDesconocido {
        /// Id que no existe.
        id: String,
    },
    /// El mensaje está en un estado terminal que no admite este cambio.
    MensajeNoEditable {
        /// Id del mensaje.
        id: String,
        /// Estado en el que está.
        estado: EstadoMensaje,
    },
    /// La acción no existe.
    AccionDesconocida {
        /// La acción que no se reconoce.
        accion: String,
    },
    /// Falta un campo obligatorio de la petición.
    PeticionIncompleta {
        /// El campo que falta.
        campo: String,
    },
}

impl ErrorCola {
    /// Código estable del error (el que va en el JSON).
    pub fn code(&self) -> &'static str {
        match self {
            ErrorCola::MensajeDuplicado { .. } => "duplicado",
            ErrorCola::SesionDesconocida { .. } => "sesion_inexistente",
            ErrorCola::ColaVacia => "cola_vacia",
            ErrorCola::MensajeYaConsumido { .. } => "ya_consumido",
            ErrorCola::MensajeDesconocido { .. } => "desconocido",
            ErrorCola::MensajeNoEditable { .. } => "no_editable",
            ErrorCola::AccionDesconocida { .. } => "accion_desconocida",
            ErrorCola::PeticionIncompleta { .. } => "peticion_incompleta",
        }
    }

    /// Explicación para humanos (en castellano).
    pub fn mensaje(&self) -> String {
        match self {
            ErrorCola::MensajeDuplicado { id } => {
                format!("ya hay un mensaje encolado con el id '{}'", id)
            }
            ErrorCola::SesionDesconocida { sesion } => {
                format!("la sesión '{}' no existe", sesion)
            }
            ErrorCola::ColaVacia => "la cola está vacía".to_string(),
            ErrorCola::MensajeYaConsumido { id } => {
                format!("el mensaje '{}' ya se consumió", id)
            }
            ErrorCola::MensajeDesconocido { id } => {
                format!("no hay ningún mensaje con el id '{}'", id)
            }
            ErrorCola::MensajeNoEditable { id, estado } => format!(
                "el mensaje '{}' está en estado '{}' y no admite más cambios",
                id,
                estado.as_str()
            ),
            ErrorCola::AccionDesconocida { accion } => {
                format!("acción desconocida: '{}'", accion)
            }
            ErrorCola::PeticionIncompleta { campo } => {
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

/// Un mensaje de la cola.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mensaje {
    /// Id estable del mensaje dentro de la cola.
    pub id: String,
    /// Sesión a la que pertenece (la cola es multi-sesión).
    pub sesion: String,
    /// Cuerpo del mensaje.
    pub texto: String,
    /// Estado actual.
    pub estado: EstadoMensaje,
    /// Prioridad (mayor = antes, dentro de la misma banda).
    pub prioridad: i32,
    /// ¿Tomó el turno (redirección)?  Los redirigidos van delante.
    pub dirigido: bool,
    /// Posición **en línea** (1 = el primero de la cola).  0 si ya no está
    /// en línea (consumido, fallido o cancelado).
    pub posicion: usize,
    /// Orden de llegada (desempate estable dentro de la misma prioridad).
    pub llegada: u64,
    /// Momento de encolado (epoch).
    pub creado_s: u64,
    /// Momento del último cambio (epoch).
    pub actualizado_s: u64,
    /// **Por qué** cambió de estado por última vez.
    pub motivo: Option<String>,
}

impl Mensaje {
    /// El mensaje en el formato JSON de la cola.
    pub fn a_json(&self) -> Value {
        json!({
            "id": self.id,
            "session_id": self.sesion,
            "position": self.posicion,
            "priority": self.prioridad,
            "state": self.estado.as_str(),
            "text": self.texto,
            "created_at": self.creado_s,
            "updated_at": self.actualizado_s,
            "reason": self.motivo,
        })
    }
}

/// Un evento del historial: el "por qué" de cada cambio de la cola.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evento {
    /// Secuencia monótona (permite publicar sólo lo nuevo).
    pub seq: u64,
    /// Mensaje afectado.
    pub mensaje_id: String,
    /// Sesión del mensaje.
    pub sesion: String,
    /// Qué pasó.
    pub accion: Accion,
    /// Por qué.
    pub motivo: String,
    /// Epoch del cambio.
    pub momento_s: u64,
}

impl Evento {
    /// El evento en JSON.
    pub fn a_json(&self) -> Value {
        json!({
            "seq": self.seq,
            "message_id": self.mensaje_id,
            "session_id": self.sesion,
            "action": self.accion.as_str(),
            "reason": self.motivo,
            "at": self.momento_s,
        })
    }
}

/// Petición de encolado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeticionEncolar {
    /// Sesión destino (debe existir).
    pub sesion: String,
    /// Cuerpo del mensaje.
    pub texto: String,
    /// Prioridad (mayor = antes).
    pub prioridad: i32,
    /// Id opcional (si no, la cola genera uno estable).
    pub id: Option<String>,
}

impl PeticionEncolar {
    /// Petición mínima: sesión y texto.
    pub fn nueva(sesion: &str, texto: &str) -> Self {
        PeticionEncolar {
            sesion: sesion.to_string(),
            texto: texto.to_string(),
            prioridad: 0,
            id: None,
        }
    }

    /// Con prioridad.
    pub fn con_prioridad(mut self, prioridad: i32) -> Self {
        self.prioridad = prioridad;
        self
    }

    /// Con id explícito (el root lo usa para el `clientMessageId`).
    pub fn con_id(mut self, id: &str) -> Self {
        self.id = Some(id.to_string());
        self
    }
}

/// Petición de actualización de un mensaje encolado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeticionActualizar {
    /// Id del mensaje.
    pub id: String,
    /// Texto nuevo (si no se cambia, se conserva).
    pub texto: Option<String>,
    /// Prioridad nueva (si no se cambia, se conserva).
    pub prioridad: Option<i32>,
}

impl PeticionActualizar {
    /// Actualización mínima: sólo el id.
    pub fn nueva(id: &str) -> Self {
        PeticionActualizar {
            id: id.to_string(),
            texto: None,
            prioridad: None,
        }
    }

    /// Con texto nuevo.
    pub fn con_texto(mut self, texto: &str) -> Self {
        self.texto = Some(texto.to_string());
        self
    }

    /// Con prioridad nueva.
    pub fn con_prioridad(mut self, prioridad: i32) -> Self {
        self.prioridad = Some(prioridad);
        self
    }
}

/// Filtro de listado de la cola.
#[derive(Clone, Debug, Default)]
pub struct FiltroCola {
    /// Sólo los mensajes de esta sesión (si no se pone, todas).
    pub sesion: Option<String>,
    /// Tope de filas (0 = sin tope).
    pub limite: usize,
}

// ── La cola ───────────────────────────────────────────────────────────────

/// La cola de mensajes de una o varias sesiones.
///
/// Es un modelo **puro**: no toca el disco ni la red.  Toda la E/S vive en
/// [`Cola::guarda`]/[`Cola::carga`] y en [`BusEventos`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Cola {
    /// Los mensajes, **en orden de cola** (el primero es el que sale).
    mensajes: Vec<Mensaje>,
    /// Las sesiones conocidas: una sesión que no está aquí no existe.
    sesiones: Vec<String>,
    /// Historial de cambios (auditoría; decisión NUESTRA, ver la cabecera).
    historial: Vec<Evento>,
    /// Contador de mensajes (ids y orden de llegada).
    contador: u64,
    /// Contador de eventos del historial.
    secuencia: u64,
}

/// Orden de la cola: primero los **redirigidos** (toman el turno), luego por
/// prioridad (mayor primero) y, a igualdad, por orden de llegada.
fn cmp_mensajes(a: &Mensaje, b: &Mensaje) -> Ordering {
    b.dirigido
        .cmp(&a.dirigido)
        .then(b.prioridad.cmp(&a.prioridad))
        .then(a.llegada.cmp(&b.llegada))
}

impl Cola {
    /// Cola vacía (sin sesiones y sin mensajes).
    pub fn nueva() -> Self {
        Cola::default()
    }

    /// Cola con las sesiones que ya existen.
    pub fn con_sesiones(sesiones: &[&str]) -> Self {
        let mut c = Cola::nueva();
        for s in sesiones {
            c.registrar_sesion(s);
        }
        c
    }

    /// Registra una sesión.  Devuelve `false` si ya estaba (no duplica).
    pub fn registrar_sesion(&mut self, sesion: &str) -> bool {
        if self.sesion_existe(sesion) {
            return false;
        }
        self.sesiones.push(sesion.to_string());
        true
    }

    /// ¿Existe esa sesión?
    pub fn sesion_existe(&self, sesion: &str) -> bool {
        self.sesiones.iter().any(|s| s == sesion)
    }

    /// Las sesiones conocidas.
    pub fn sesiones(&self) -> &[String] {
        &self.sesiones
    }

    /// Todos los mensajes (encolados y no).
    pub fn mensajes(&self) -> &[Mensaje] {
        &self.mensajes
    }

    /// El historial de cambios.
    pub fn historial(&self) -> &[Evento] {
        &self.historial
    }

    /// Cuántos mensajes hay **en línea**.
    pub fn total(&self) -> usize {
        self.mensajes
            .iter()
            .filter(|m| m.estado == EstadoMensaje::Encolado)
            .count()
    }

    /// Un mensaje por id (encolado o no).
    pub fn buscar(&self, id: &str) -> Option<&Mensaje> {
        self.mensajes.iter().find(|m| m.id == id)
    }

    // ── Encolar ───────────────────────────────────────────────────────────

    /// Encola un mensaje.
    ///
    /// El mensaje entra **al final de su banda de prioridad**: si hay
    /// mensajes con más prioridad delante, el nuevo se coloca detrás de
    /// ellos; a igual prioridad, detrás de los que ya estaban (FIFO).
    ///
    /// Errores: [`ErrorCola::SesionDesconocida`] si la sesión no existe y
    /// [`ErrorCola::MensajeDuplicado`] si el id ya está encolado.
    pub fn encolar(
        &mut self,
        peticion: PeticionEncolar,
        ahora_s: u64,
    ) -> Result<Mensaje, ErrorCola> {
        if !self.sesion_existe(&peticion.sesion) {
            return Err(ErrorCola::SesionDesconocida {
                sesion: peticion.sesion.clone(),
            });
        }
        self.contador += 1;
        let id = match peticion.id.as_deref().map(str::trim) {
            Some(i) if !i.is_empty() => i.to_string(),
            _ => format!("m-{:06}", self.contador),
        };
        if self.mensajes.iter().any(|m| m.id == id) {
            return Err(ErrorCola::MensajeDuplicado { id });
        }
        let mensaje = Mensaje {
            id: id.clone(),
            sesion: peticion.sesion.clone(),
            texto: peticion.texto,
            estado: EstadoMensaje::Encolado,
            prioridad: peticion.prioridad,
            dirigido: false,
            posicion: 0,
            llegada: self.contador,
            creado_s: ahora_s,
            actualizado_s: ahora_s,
            motivo: Some("mensaje encolado".to_string()),
        };
        self.mensajes.push(mensaje);
        self.ordena();
        let mensaje = match self.mensajes.iter().find(|m| m.id == id) {
            Some(m) => m.clone(),
            None => return Err(ErrorCola::MensajeDesconocido { id }),
        };
        self.registra(
            &mensaje,
            Accion::Encolado,
            &format!("prioridad {}", mensaje.prioridad),
            ahora_s,
        );
        Ok(mensaje)
    }

    // ── Listar ────────────────────────────────────────────────────────────

    /// Lista los mensajes **en línea**, en orden de cola.
    ///
    /// Devuelve [`ErrorCola::ColaVacia`] si no hay nada que listar (ni
    /// messages quelistar con ese filtro).
    pub fn listar(&self, filtro: &FiltroCola) -> Result<Vec<Mensaje>, ErrorCola> {
        if let Some(s) = &filtro.sesion {
            if !self.sesion_existe(s) {
                return Err(ErrorCola::SesionDesconocida { sesion: s.clone() });
            }
        }
        let mut out: Vec<Mensaje> = self
            .mensajes
            .iter()
            .filter(|m| m.estado == EstadoMensaje::Encolado)
            .filter(|m| match &filtro.sesion {
                Some(s) => &m.sesion == s,
                None => true,
            })
            .cloned()
            .collect();
        if filtro.limite > 0 && out.len() > filtro.limite {
            out.truncate(filtro.limite);
        }
        if out.is_empty() {
            return Err(ErrorCola::ColaVacia);
        }
        Ok(out)
    }

    /// El siguiente mensaje de una sesión, **sin** consumirlo.
    pub fn siguiente(&self, sesion: &str) -> Result<Mensaje, ErrorCola> {
        if !self.sesion_existe(sesion) {
            return Err(ErrorCola::SesionDesconocida {
                sesion: sesion.to_string(),
            });
        }
        self.mensajes
            .iter()
            .find(|m| m.sesion == sesion && m.estado == EstadoMensaje::Encolado)
            .cloned()
            .ok_or(ErrorCola::ColaVacia)
    }

    // ── Actualizar ────────────────────────────────────────────────────────

    /// Actualiza un mensaje encolado (su texto y/o su prioridad).
    ///
    /// El mensaje **no se pierde**: conserva su id, su sesión, su momento de
    /// creación y su texto si no se cambia.  Lo que sí puede cambiar es su
    /// posición en la cola (si cambia la prioridad).
    ///
    /// Errores: [`ErrorCola::MensajeDesconocido`],
    /// [`ErrorCola::MensajeYaConsumido`] y [`ErrorCola::MensajeNoEditable`].
    pub fn actualizar(
        &mut self,
        peticion: PeticionActualizar,
        ahora_s: u64,
    ) -> Result<Mensaje, ErrorCola> {
        let PeticionActualizar {
            id,
            texto,
            prioridad,
        } = peticion;
        {
            let mensaje = self
                .mensajes
                .iter_mut()
                .find(|m| m.id == id)
                .ok_or_else(|| ErrorCola::MensajeDesconocido { id: id.clone() })?;
            if mensaje.estado == EstadoMensaje::Consumido {
                return Err(ErrorCola::MensajeYaConsumido { id });
            }
            if mensaje.estado.es_terminal() {
                return Err(ErrorCola::MensajeNoEditable {
                    id,
                    estado: mensaje.estado,
                });
            }
            let mut motivos: Vec<String> = Vec::new();
            if let Some(t) = texto {
                mensaje.texto = t;
                motivos.push("texto actualizado".to_string());
            }
            if let Some(p) = prioridad {
                if p != mensaje.prioridad {
                    mensaje.prioridad = p;
                    motivos.push(format!("prioridad {}", p));
                }
            }
            if motivos.is_empty() {
                motivos.push("sin cambios".to_string());
            }
            mensaje.actualizado_s = ahora_s;
            mensaje.motivo = Some(motivos.join("; "));
        }
        self.ordena();
        let mensaje = self
            .mensajes
            .iter()
            .find(|m| m.id == id)
            .cloned()
            .ok_or(ErrorCola::MensajeDesconocido { id })?;
        let motivo = mensaje.motivo.clone().unwrap_or_default();
        self.registra(&mensaje, Accion::Actualizado, &motivo, ahora_s);
        Ok(mensaje)
    }

    // ── Borrar ────────────────────────────────────────────────────────────

    /// Borra un mensaje encolado.
    ///
    /// El mensaje **no deja rastro en la cola** (desaparece de ella), pero
    /// **sí** en el historial: queda el evento `eliminado` con su motivo.
    /// Esa traza es una decisión NUESTRA (ver la cabecera): el historial no
    /// está medido en la referencia.
    ///
    /// Errores: [`ErrorCola::MensajeDesconocido`],
    /// [`ErrorCola::MensajeYaConsumido`] y [`ErrorCola::MensajeNoEditable`].
    pub fn borrar(&mut self, id: &str, ahora_s: u64) -> Result<Mensaje, ErrorCola> {
        let indice = self
            .mensajes
            .iter()
            .position(|m| m.id == id)
            .ok_or_else(|| ErrorCola::MensajeDesconocido { id: id.to_string() })?;
        if self.mensajes[indice].estado == EstadoMensaje::Consumido {
            return Err(ErrorCola::MensajeYaConsumido { id: id.to_string() });
        }
        if self.mensajes[indice].estado.es_terminal() {
            return Err(ErrorCola::MensajeNoEditable {
                id: id.to_string(),
                estado: self.mensajes[indice].estado,
            });
        }
        let mut mensaje = self.mensajes.remove(indice);
        mensaje.estado = EstadoMensaje::Cancelado;
        mensaje.posicion = 0;
        mensaje.actualizado_s = ahora_s;
        mensaje.motivo = Some("borrado de la cola".to_string());
        self.renumera();
        self.registra(&mensaje, Accion::Eliminado, "borrado de la cola", ahora_s);
        Ok(mensaje)
    }

    // ── Redirigir ("steer") ──────────────────────────────────────────────

    /// Redirige un mensaje encolado: le cambia la prioridad y lo pone
    /// **delante** (toma el turno).
    ///
    /// El mensaje **no se pierde**: conserva su id, su sesión, su texto y su
    /// momento de creación; sólo cambian su prioridad, su posición y el
    /// motivo.
    ///
    /// Errores: [`ErrorCola::MensajeDesconocido`],
    /// [`ErrorCola::MensajeYaConsumido`] y [`ErrorCola::MensajeNoEditable`].
    pub fn redirigir(
        &mut self,
        id: &str,
        prioridad: i32,
        ahora_s: u64,
    ) -> Result<Mensaje, ErrorCola> {
        let indice = self
            .mensajes
            .iter()
            .position(|m| m.id == id)
            .ok_or_else(|| ErrorCola::MensajeDesconocido { id: id.to_string() })?;
        if self.mensajes[indice].estado == EstadoMensaje::Consumido {
            return Err(ErrorCola::MensajeYaConsumido { id: id.to_string() });
        }
        if self.mensajes[indice].estado.es_terminal() {
            return Err(ErrorCola::MensajeNoEditable {
                id: id.to_string(),
                estado: self.mensajes[indice].estado,
            });
        }
        {
            let mensaje = &mut self.mensajes[indice];
            mensaje.prioridad = prioridad;
            mensaje.dirigido = true;
            mensaje.actualizado_s = ahora_s;
            mensaje.motivo = Some(format!("redirigido al frente (prioridad {})", prioridad));
        }
        self.ordena();
        let mensaje = self
            .mensajes
            .iter()
            .find(|m| m.id == id)
            .cloned()
            .ok_or_else(|| ErrorCola::MensajeDesconocido { id: id.to_string() })?;
        let motivo = mensaje.motivo.clone().unwrap_or_default();
        self.registra(&mensaje, Accion::Redirigido, &motivo, ahora_s);
        Ok(mensaje)
    }

    // ── Consumir / fallar / cancelar ──────────────────────────────────────

    /// Consume (entrega) el primer mensaje encolado de una sesión.
    ///
    /// El mensaje pasa a `consumido` (deja de estar en línea) pero su
    /// registro se conserva: por eso volver a tocarlo da el error tipado
    /// [`ErrorCola::MensajeYaConsumido`] y no "no existe".
    ///
    /// Errores: [`ErrorCola::SesionDesconocida`] y [`ErrorCola::ColaVacia`].
    pub fn consumir(&mut self, sesion: &str, ahora_s: u64) -> Result<Mensaje, ErrorCola> {
        if !self.sesion_existe(sesion) {
            return Err(ErrorCola::SesionDesconocida {
                sesion: sesion.to_string(),
            });
        }
        let indice = self
            .mensajes
            .iter()
            .position(|m| m.sesion == sesion && m.estado == EstadoMensaje::Encolado)
            .ok_or(ErrorCola::ColaVacia)?;
        let mut mensaje = self.mensajes[indice].clone();
        mensaje.estado = EstadoMensaje::Consumido;
        mensaje.posicion = 0;
        mensaje.actualizado_s = ahora_s;
        mensaje.motivo = Some(format!("entregado a la sesión {}", sesion));
        self.mensajes[indice] = mensaje.clone();
        self.renumera();
        let motivo = format!("entregado a la sesión {}", sesion);
        self.registra(&mensaje, Accion::Consumido, &motivo, ahora_s);
        Ok(mensaje)
    }

    /// Marca la entrega de un mensaje como fallida (el motivo se guarda).
    pub fn fallar(&mut self, id: &str, motivo: &str, ahora_s: u64) -> Result<Mensaje, ErrorCola> {
        self.cambia_estado(id, EstadoMensaje::Fallido, motivo, Accion::Fallido, ahora_s)
    }

    /// Retira un mensaje de la cola sin consumirlo (el registro se conserva).
    pub fn cancelar(&mut self, id: &str, motivo: &str, ahora_s: u64) -> Result<Mensaje, ErrorCola> {
        self.cambia_estado(
            id,
            EstadoMensaje::Cancelado,
            motivo,
            Accion::Cancelado,
            ahora_s,
        )
    }

    /// Cambio de estado terminal (consumido/fallido/cancelado).
    fn cambia_estado(
        &mut self,
        id: &str,
        estado: EstadoMensaje,
        motivo: &str,
        accion: Accion,
        ahora_s: u64,
    ) -> Result<Mensaje, ErrorCola> {
        let indice = self
            .mensajes
            .iter()
            .position(|m| m.id == id)
            .ok_or_else(|| ErrorCola::MensajeDesconocido { id: id.to_string() })?;
        if self.mensajes[indice].estado == EstadoMensaje::Consumido {
            return Err(ErrorCola::MensajeYaConsumido { id: id.to_string() });
        }
        if self.mensajes[indice].estado.es_terminal() {
            return Err(ErrorCola::MensajeNoEditable {
                id: id.to_string(),
                estado: self.mensajes[indice].estado,
            });
        }
        let mut mensaje = self.mensajes[indice].clone();
        mensaje.estado = estado;
        mensaje.posicion = 0;
        mensaje.actualizado_s = ahora_s;
        mensaje.motivo = Some(motivo.to_string());
        self.mensajes[indice] = mensaje.clone();
        self.renumera();
        self.registra(&mensaje, accion, motivo, ahora_s);
        Ok(mensaje)
    }

    // ── Internos ──────────────────────────────────────────────────────────

    /// Ordena la cola y renumera las posiciones.
    fn ordena(&mut self) {
        self.mensajes.sort_by(cmp_mensajes);
        self.renumera();
    }

    /// Renumera las posiciones **en línea** (1 = el primero).
    fn renumera(&mut self) {
        let mut posicion = 0usize;
        for mensaje in self.mensajes.iter_mut() {
            if mensaje.estado == EstadoMensaje::Encolado {
                posicion += 1;
                mensaje.posicion = posicion;
            } else {
                mensaje.posicion = 0;
            }
        }
    }

    /// Anota un evento en el historial.
    fn registra(&mut self, mensaje: &Mensaje, accion: Accion, motivo: &str, ahora_s: u64) {
        self.secuencia += 1;
        self.historial.push(Evento {
            seq: self.secuencia,
            mensaje_id: mensaje.id.clone(),
            sesion: mensaje.sesion.clone(),
            accion,
            motivo: motivo.to_string(),
            momento_s: ahora_s,
        });
    }

    // ── E/S (fina) ────────────────────────────────────────────────────────

    /// La **actualización de información** de la cola.
    ///
    /// MEDIDO: la cola de envío se lee de `messages_in_queued`
    /// (`docs/RE_ACP.md:331`), así que ese es el campo que lleva la lista.
    pub fn info(&self) -> Value {
        let encolados: Vec<Value> = self
            .mensajes
            .iter()
            .filter(|m| m.estado == EstadoMensaje::Encolado)
            .map(|m| m.a_json())
            .collect();
        json!({
            "ok": true,
            "messages_in_queued": encolados,
            "total": encolados.len(),
            "sessions": self.sesiones,
        })
    }

    /// Guarda la cola en un fichero JSON (decisión NUESTRA: la referencia
    /// lleva la cola en el protocolo, no en un fichero).
    pub fn guarda(&self, ruta: &Path) -> Result<(), String> {
        let texto = serde_json::to_string_pretty(self)
            .map_err(|e| format!("no se pudo serializar la cola: {}", e))?;
        if let Some(dir) = ruta.parent() {
            if !dir.as_os_str().is_empty() {
                fs::create_dir_all(dir)
                    .map_err(|e| format!("no se pudo crear {}: {}", dir.display(), e))?;
            }
        }
        fs::write(ruta, texto).map_err(|e| format!("no se pudo escribir {}: {}", ruta.display(), e))
    }

    /// Carga la cola de un fichero JSON.  Si el fichero no existe, cola
    /// vacía (no es un error: la cola empieza vacía).
    pub fn carga(ruta: &Path) -> Result<Cola, String> {
        match fs::read_to_string(ruta) {
            Ok(texto) => serde_json::from_str(&texto)
                .map_err(|e| format!("no se pudo leer la cola de {}: {}", ruta.display(), e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Cola::nueva()),
            Err(e) => Err(format!("no se pudo leer {}: {}", ruta.display(), e)),
        }
    }

    /// Publica en el bus los eventos **nuevos** (los de `seq` mayor que
    /// `desde_seq`) y devuelve cuántos publicó.
    pub fn publica(&self, bus: &mut dyn BusEventos, desde_seq: u64) -> Result<usize, String> {
        let nuevos: Vec<Evento> = self
            .historial
            .iter()
            .filter(|e| e.seq > desde_seq)
            .cloned()
            .collect();
        for evento in &nuevos {
            bus.registra(evento)?;
        }
        Ok(nuevos.len())
    }
}

// ── El bus (el enchufe para el bus real) ─────────────────────────────────

/// Puerto de eventos: por dónde salen los cambios de la cola.
///
/// Es **un solo método** a propósito: el root lo implementa sobre el bus que
/// ya hay (`service_bus.rs` o los eventos del tablón `board.rs`) sin tocar
/// este módulo.
pub trait BusEventos {
    /// Registra un evento de la cola.  `Err` si el bus no pudo con él.
    fn registra(&mut self, evento: &Evento) -> Result<(), String>;
}

/// Bus de memoria: guarda los eventos (para tests y para inspeccionar).
#[derive(Clone, Debug, Default)]
pub struct BusEnMemoria {
    /// Los eventos registrados, en orden.
    pub eventos: Vec<Evento>,
}

impl BusEventos for BusEnMemoria {
    fn registra(&mut self, evento: &Evento) -> Result<(), String> {
        self.eventos.push(evento.clone());
        Ok(())
    }
}

/// Bus de fichero: un journal NDJSON (una línea por evento), como los del
/// repo.
#[derive(Clone, Debug)]
pub struct BusNdjson {
    /// Fichero del journal.
    pub ruta: PathBuf,
}

impl BusNdjson {
    /// Journal en `ruta`.
    pub fn new(ruta: &Path) -> Self {
        BusNdjson {
            ruta: ruta.to_path_buf(),
        }
    }
}

impl BusEventos for BusNdjson {
    fn registra(&mut self, evento: &Evento) -> Result<(), String> {
        use std::io::Write;
        if let Some(dir) = self.ruta.parent() {
            if !dir.as_os_str().is_empty() {
                fs::create_dir_all(dir)
                    .map_err(|e| format!("no se pudo crear {}: {}", dir.display(), e))?;
            }
        }
        let mut fichero = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.ruta)
            .map_err(|e| format!("no se pudo abrir {}: {}", self.ruta.display(), e))?;
        let linea = serde_json::to_string(evento)
            .map_err(|e| format!("no se pudo serializar el evento: {}", e))?;
        writeln!(fichero, "{}", linea)
            .map_err(|e| format!("no se pudo escribir {}: {}", self.ruta.display(), e))
    }
}

// ── Capacidades (opt-in) ──────────────────────────────────────────────────

/// Lo que el cliente ha declarado que extiende.
///
/// MEDIDO: las capacidades son **opt-in**; el agente sólo anuncia la que el
/// cliente pide (`docs/RE_ACP.md:320`).  Este módulo lleva **su propio** juego
/// de capacidades para no depender de `subagentes.rs`: el root construye las
/// dos desde el mismo `_meta` del `initialize`.
#[derive(Clone, Debug, Default)]
pub struct Capacidades {
    declaradas: BTreeSet<String>,
}

impl Capacidades {
    /// Sin nada declarado (nada se anuncia).
    pub fn nueva() -> Self {
        Capacidades::default()
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
    /// Acepta las dos formas: una clave por capacidad (`{"mdaemon/queue":
    /// true}`) y una lista (`{"capacidades": ["mdaemon/queue"]}`).
    pub fn desde_meta(meta: &Value) -> Self {
        let mut c = Capacidades::nueva();
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

// ── Extensiones (las registra el root) ────────────────────────────────────

/// Las extensiones **propias** que propone este módulo.
///
/// Devuelve la lista `(método, para qué)`; el root la convierte en
/// `protocol_acp::Extension { namespace: "mdaemon", metodo, para_que }` (ver
/// `rust-core/src/protocol_acp.rs`).  Se devuelve una lista **propia** (sin
/// tipos del crate) para que este módulo no dependa de nada.
pub fn extensiones() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "queue/list",
            "listar los mensajes encolados de una sesión (actualización de información de la cola)",
        ),
        ("queue/enqueue", "encolar un mensaje para una sesión"),
        (
            "queue/update",
            "cambiar el texto o la prioridad de un mensaje encolado",
        ),
        (
            "queue/delete",
            "borrar un mensaje encolado (queda la traza en el historial)",
        ),
        (
            "queue/steer",
            "redirigir un mensaje ya encolado al frente de la cola",
        ),
    ]
}

/// Los métodos que se anuncian a un cliente que ha declarado
/// [`CAPACIDAD_COLA`].  Sin declaración, **no** se anuncia nada.
pub fn anuncios(capacidades: &Capacidades) -> Vec<(&'static str, &'static str)> {
    if capacidades.pide(CAPACIDAD_COLA) {
        extensiones()
    } else {
        Vec::new()
    }
}

// ── El despachador (la capa fina que el root enchufa) ─────────────────────

/// Las acciones que este módulo sabe hacer.
pub const ACCIONES: &[&str] = &["listar", "encolar", "actualizar", "borrar", "redirigir"];

/// Ejecuta una acción sobre la cola.
///
/// Es la capa fina: recibe la acción y sus parámetros en JSON y devuelve el
/// mismo JSON (`{"ok": true, ...}` o el error tipado).  El root la engancha
/// tal cual a `mdaemon/queue/*` o a la tool MCP.
pub fn opera(
    cola: &mut Cola,
    accion: &str,
    params: &Value,
    ahora_s: u64,
) -> Result<Value, ErrorCola> {
    match accion {
        "listar" => {
            let filtro = filtro_desde_json(params);
            let lista = cola.listar(&filtro)?;
            let mensajes: Vec<Value> = lista.iter().map(|m| m.a_json()).collect();
            Ok(json!({
                "ok": true,
                "messages_in_queued": mensajes,
                "total": lista.len(),
            }))
        }
        "encolar" => {
            let peticion = peticion_desde_json(params)?;
            let mensaje = cola.encolar(peticion, ahora_s)?;
            Ok(json!({ "ok": true, "message": mensaje.a_json() }))
        }
        "actualizar" => {
            let peticion = peticion_actualizar_desde_json(params)?;
            let mensaje = cola.actualizar(peticion, ahora_s)?;
            Ok(json!({ "ok": true, "message": mensaje.a_json() }))
        }
        "borrar" => {
            let id = texto_obligatorio(params, "id")?;
            let mensaje = cola.borrar(&id, ahora_s)?;
            Ok(json!({ "ok": true, "message": mensaje.a_json() }))
        }
        "redirigir" => {
            let id = texto_obligatorio(params, "id")?;
            let prioridad = params.get("priority").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let mensaje = cola.redirigir(&id, prioridad, ahora_s)?;
            Ok(json!({ "ok": true, "message": message_json(&mensaje) }))
        }
        otro => Err(ErrorCola::AccionDesconocida {
            accion: otro.to_string(),
        }),
    }
}

/// El mensaje en JSON (ayudante del despachador).
fn message_json(mensaje: &Mensaje) -> Value {
    mensaje.a_json()
}

/// Lee un campo de texto obligatorio.
fn texto_obligatorio(params: &Value, campo: &str) -> Result<String, ErrorCola> {
    params
        .get(campo)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| ErrorCola::PeticionIncompleta {
            campo: campo.to_string(),
        })
}

/// El filtro de listado, desde JSON.
fn filtro_desde_json(params: &Value) -> FiltroCola {
    FiltroCola {
        sesion: params
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        limite: params.get("limit").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
    }
}

/// La petición de encolado, desde JSON.
fn peticion_desde_json(params: &Value) -> Result<PeticionEncolar, ErrorCola> {
    let sesion = texto_obligatorio(params, "session_id")?;
    let texto = texto_obligatorio(params, "text")?;
    let prioridad = params.get("priority").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Ok(PeticionEncolar {
        sesion,
        texto,
        prioridad,
        id,
    })
}

/// La petición de actualización, desde JSON.
fn peticion_actualizar_desde_json(params: &Value) -> Result<PeticionActualizar, ErrorCola> {
    let id = texto_obligatorio(params, "id")?;
    let texto = params
        .get("text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let prioridad = params
        .get("priority")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);
    Ok(PeticionActualizar {
        id,
        texto,
        prioridad,
    })
}

/// El epoch de ahora (para el despachador).
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

    /// Cola de una sola sesión con tres mensajes.
    fn cola_de_prueba() -> Cola {
        let mut cola = Cola::con_sesiones(&["s1"]);
        cola.encolar(PeticionEncolar::nueva("s1", "hola"), 10)
            .unwrap();
        cola.encolar(PeticionEncolar::nueva("s1", "qué tal"), 20)
            .unwrap();
        cola.encolar(PeticionEncolar::nueva("s1", "adios"), 30)
            .unwrap();
        cola
    }

    #[test]
    fn encolar_y_listar_mantiene_el_orden() {
        let cola = cola_de_prueba();
        let lista = cola.listar(&FiltroCola::default()).unwrap();
        assert_eq!(lista.len(), 3);
        assert_eq!(lista[0].texto, "hola");
        assert_eq!(lista[1].texto, "qué tal");
        assert_eq!(lista[2].texto, "adios");
        // Las posiciones son 1, 2, 3 (en línea, en orden de cola).
        assert_eq!(lista[0].posicion, 1);
        assert_eq!(lista[2].posicion, 3);
        assert_eq!(cola.total(), 3);
    }

    #[test]
    fn la_prioridad_manda_al_encolar() {
        let mut cola = Cola::con_sesiones(&["s1"]);
        cola.encolar(PeticionEncolar::nueva("s1", "normal"), 1)
            .unwrap();
        cola.encolar(PeticionEncolar::nueva("s1", "urgente").con_prioridad(9), 2)
            .unwrap();
        let lista = cola.listar(&FiltroCola::default()).unwrap();
        assert_eq!(lista[0].texto, "urgente");
        assert_eq!(lista[1].texto, "normal");
    }

    #[test]
    fn actualizar_no_pierde_el_mensaje() {
        let mut cola = cola_de_prueba();
        let id = cola.listar(&FiltroCola::default()).unwrap()[0].id.clone();
        let actualizado = cola
            .actualizar(
                PeticionActualizar::nueva(&id).con_texto("hola de nuevo"),
                50,
            )
            .unwrap();
        // El mensaje sigue en la cola, con su id y su sesión.
        assert_eq!(actualizado.id, id);
        assert_eq!(actualizado.sesion, "s1");
        assert_eq!(actualizado.texto, "hola de nuevo");
        assert_eq!(
            actualizado.creado_s, 10,
            "no se pierde el momento de creación"
        );
        let lista = cola.listar(&FiltroCola::default()).unwrap();
        assert_eq!(lista.len(), 3);
        assert!(lista
            .iter()
            .any(|m| m.id == id && m.texto == "hola de nuevo"));
    }

    #[test]
    fn borrar_no_deja_rastro_en_la_cola_pero_si_en_el_historial() {
        let mut cola = cola_de_prueba();
        let id = cola.listar(&FiltroCola::default()).unwrap()[0].id.clone();
        let borrado = cola.borrar(&id, 40).unwrap();
        assert_eq!(borrado.id, id);
        // 1) En la cola ya no está.
        assert!(cola.buscar(&id).is_none());
        let lista = cola.listar(&FiltroCola::default()).unwrap();
        assert_eq!(lista.len(), 2);
        assert!(!lista.iter().any(|m| m.id == id));
        // 2) En el historial sí está, con su motivo.
        let evento = cola
            .historial()
            .iter()
            .find(|e| e.mensaje_id == id && e.accion == Accion::Eliminado)
            .expect("el borrado deja traza en el historial");
        assert_eq!(evento.motivo, "borrado de la cola");
    }

    #[test]
    fn redirigir_pone_el_mensaje_al_frente_sin_perderlo() {
        let mut cola = cola_de_prueba();
        let id = cola.listar(&FiltroCola::default()).unwrap()[2].id.clone();
        let antes = cola.buscar(&id).cloned().unwrap();
        let redirigido = cola.redirigir(&id, 5, 60).unwrap();
        // El mensaje NO se pierde: conserva id, sesión, texto y creación.
        assert_eq!(redirigido.id, antes.id);
        assert_eq!(redirigido.sesion, antes.sesion);
        assert_eq!(redirigido.texto, antes.texto);
        assert_eq!(redirigido.creado_s, antes.creado_s);
        // Pero cambia la prioridad y pasa al frente.
        assert_eq!(redirigido.prioridad, 5);
        assert!(redirigido.dirigido);
        assert_eq!(redirigido.posicion, 1);
        let lista = cola.listar(&FiltroCola::default()).unwrap();
        assert_eq!(lista[0].id, id);
        assert_eq!(lista.len(), 3, "siguen estando los mismos mensajes");
    }

    #[test]
    fn el_mensaje_duplicado_es_un_error_tipado() {
        let mut cola = cola_de_prueba();
        let id = cola.listar(&FiltroCola::default()).unwrap()[0].id.clone();
        let err = cola
            .encolar(PeticionEncolar::nueva("s1", "otro").con_id(&id), 70)
            .unwrap_err();
        assert_eq!(err, ErrorCola::MensajeDuplicado { id: id.clone() });
        assert_eq!(err.code(), "duplicado");
    }

    #[test]
    fn la_sesion_inexistente_es_un_error_tipado() {
        let mut cola = Cola::con_sesiones(&["s1"]);
        let err = cola
            .encolar(PeticionEncolar::nueva("s9", "hola"), 80)
            .unwrap_err();
        assert_eq!(
            err,
            ErrorCola::SesionDesconocida {
                sesion: "s9".into()
            }
        );
        assert_eq!(err.code(), "sesion_inexistente");
        // Y al listar por una sesión que no existe, también.
        let filtro = FiltroCola {
            sesion: Some("s9".to_string()),
            limite: 0,
        };
        assert_eq!(
            cola.listar(&filtro).unwrap_err().code(),
            "sesion_inexistente"
        );
    }

    #[test]
    fn la_cola_vacia_es_un_error_tipado() {
        let cola = Cola::con_sesiones(&["s1"]);
        let err = cola.listar(&FiltroCola::default()).unwrap_err();
        assert_eq!(err, ErrorCola::ColaVacia);
        assert_eq!(err.code(), "cola_vacia");
        // Y al pedir el siguiente mensaje.
        assert_eq!(cola.siguiente("s1").unwrap_err(), ErrorCola::ColaVacia);
    }

    #[test]
    fn el_mensaje_ya_consumido_es_un_error_tipado() {
        let mut cola = cola_de_prueba();
        let id = cola.listar(&FiltroCola::default()).unwrap()[0].id.clone();
        let consumido = cola.consumir("s1", 90).unwrap();
        assert_eq!(consumido.id, id);
        assert_eq!(consumido.estado, EstadoMensaje::Consumido);
        // Tocarlo después de consumirlo da el error tipado.
        let err = cola
            .actualizar(PeticionActualizar::nueva(&id), 91)
            .unwrap_err();
        assert_eq!(err, ErrorCola::MensajeYaConsumido { id: id.clone() });
        assert_eq!(err.code(), "ya_consumido");
        // Tampoco se puede borrar ni redirigir.
        assert_eq!(cola.borrar(&id, 92).unwrap_err().code(), "ya_consumido");
        assert_eq!(
            cola.redirigir(&id, 1, 93).unwrap_err().code(),
            "ya_consumido"
        );
    }

    #[test]
    fn el_historial_explica_cada_cambio_de_estado() {
        let mut cola = cola_de_prueba();
        let id = cola.listar(&FiltroCola::default()).unwrap()[0].id.clone();
        cola.actualizar(PeticionActualizar::nueva(&id).con_texto("nuevo"), 100)
            .unwrap();
        cola.redirigir(&id, 3, 110).unwrap();
        cola.consumir("s1", 120).unwrap();
        let mio: Vec<&Evento> = cola
            .historial()
            .iter()
            .filter(|e| e.mensaje_id == id)
            .collect();
        assert_eq!(mio.len(), 4);
        assert_eq!(mio[0].accion, Accion::Encolado);
        assert_eq!(mio[1].accion, Accion::Actualizado);
        assert!(mio[1].motivo.contains("texto"));
        assert_eq!(mio[2].accion, Accion::Redirigido);
        assert!(mio[2].motivo.contains("prioridad 3"));
        assert_eq!(mio[3].accion, Accion::Consumido);
        // La secuencia es monótona.
        assert!(mio[0].seq < mio[1].seq && mio[1].seq < mio[3].seq);
    }

    #[test]
    fn la_cola_se_guarda_y_se_carga_sin_perder_nada() {
        let cola = cola_de_prueba();
        let ruta = std::env::temp_dir().join(format!("md-cola-{}.json", std::process::id()));
        let _ = fs::remove_file(&ruta);
        cola.guarda(&ruta).unwrap();
        let cargada = Cola::carga(&ruta).unwrap();
        let lista = cargada.listar(&FiltroCola::default()).unwrap();
        assert_eq!(lista.len(), 3);
        assert_eq!(lista[0].texto, "hola");
        assert_eq!(cargada.sesiones(), &["s1".to_string()]);
        // Un fichero que no existe = cola vacía (no es un error).
        let ruta_ausente = std::env::temp_dir().join("md-cola-no-existe-este-fichero.json");
        let _ = fs::remove_file(&ruta_ausente);
        assert_eq!(Cola::carga(&ruta_ausente).unwrap().total(), 0);
        let _ = fs::remove_file(&ruta);
    }

    #[test]
    fn los_cambios_salen_por_el_bus() {
        let cola = cola_de_prueba();
        let mut bus = BusEnMemoria::default();
        let publicados = cola.publica(&mut bus, 0).unwrap();
        assert_eq!(publicados, 3);
        assert_eq!(bus.eventos.len(), 3);
        assert_eq!(bus.eventos[0].accion, Accion::Encolado);
        // Publicar otra vez con el mismo `desde_seq` repite; con el último
        // `seq` no repite nada.
        let nada = cola.publica(&mut bus, 3).unwrap();
        assert_eq!(nada, 0);
        assert_eq!(bus.eventos.len(), 3);
    }

    #[test]
    fn la_info_de_la_cola_trae_los_mensajes_encolados() {
        let mut cola = cola_de_prueba();
        let id = cola.listar(&FiltroCola::default()).unwrap()[0].id.clone();
        cola.consumir("s1", 130).unwrap();
        let info = cola.info();
        assert_eq!(info["ok"], json!(true));
        let encolados = info["messages_in_queued"].as_array().unwrap();
        assert_eq!(encolados.len(), 2);
        // El consumido ya no está en la cola (pero su registro sigue ahí).
        assert!(!encolados.iter().any(|m| m["id"] == json!(id)));
        assert!(cola.buscar(&id).is_some());
    }

    #[test]
    fn el_despachador_atiende_las_cinco_acciones() {
        let mut cola = Cola::con_sesiones(&["s1"]);
        let encolado = opera(
            &mut cola,
            "encolar",
            &json!({"session_id": "s1", "text": "hola"}),
            200,
        )
        .unwrap();
        let id = encolado["message"]["id"].as_str().unwrap().to_string();
        let actualizado = opera(
            &mut cola,
            "actualizar",
            &json!({"id": id, "text": "hola2"}),
            201,
        )
        .unwrap();
        assert_eq!(actualizado["message"]["text"], json!("hola2"));
        let redirigido = opera(
            &mut cola,
            "redirigir",
            &json!({"id": id, "priority": 7}),
            202,
        )
        .unwrap();
        assert_eq!(redirigido["message"]["priority"], json!(7));
        let listado = opera(&mut cola, "listar", &json!({}), 203).unwrap();
        assert_eq!(listado["total"], json!(1));
        let borrado = opera(&mut cola, "borrar", &json!({"id": id}), 204).unwrap();
        assert_eq!(borrado["ok"], json!(true));
        // Acción desconocida y petición incompleta: errores tipados.
        assert_eq!(
            opera(&mut cola, "inventada", &json!({}), 205)
                .unwrap_err()
                .code(),
            "accion_desconocida"
        );
        assert_eq!(
            opera(&mut cola, "borrar", &json!({}), 206)
                .unwrap_err()
                .code(),
            "peticion_incompleta"
        );
    }

    #[test]
    fn sin_declaracion_del_cliente_no_se_anuncia_la_cola() {
        // MEDIDO: el opt-in es mutuo (`docs/RE_ACP.md:320`).
        let vacio = Capacidades::nueva();
        assert!(anuncios(&vacio).is_empty());
        let mut declara = Capacidades::nueva();
        assert!(declara.declara(CAPACIDAD_COLA));
        let anuncios = anuncios(&declara);
        assert_eq!(anuncios.len(), 5);
        assert!(anuncios.iter().any(|(m, _)| *m == "queue/steer"));
    }

    #[test]
    fn las_extensiones_propias_cuadran_con_las_acciones() {
        let ext = extensiones();
        assert_eq!(ext.len(), ACCIONES.len());
        let nombres: Vec<&str> = ext.iter().map(|(m, _)| *m).collect();
        assert!(nombres.contains(&"queue/list"));
        assert!(nombres.contains(&"queue/enqueue"));
        assert!(nombres.contains(&"queue/update"));
        assert!(nombres.contains(&"queue/delete"));
        assert!(nombres.contains(&"queue/steer"));
        // Ninguno puede repetir método.
        let mut ordenados = nombres.clone();
        ordenados.sort();
        let antes = ordenados.len();
        ordenados.dedup();
        assert_eq!(antes, ordenados.len());
    }
}
