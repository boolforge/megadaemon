//! **Ciclo de vida de sesión ACP** (módulo `session.rs` del plan de
//! `protocol.rs`, `docs/DECISION_ACP.md` §3).
//!
//! MEDIDO (registro ACP oficial, 2026-11-26, 41 agentes que inicializan):
//! `loadSession` 39/41 · `sessionList` 35/41 · `sessionResume` 25/41 ·
//! `sessionFork` **18/41** · `setModel` 2/41.
//!
//! ⇒ El **núcleo** es `session/new` + `session/prompt` + `session/cancel`;
//! `load`/`list`/`resume` son de uso mayoritario y `fork` es una de las
//! cuatro extensions que sí se implementan (por eso el "bosque": un fork es
//! un hijo que conserva el historial del padre, como el patrón 3 del doc de
//! patrones).
//!
//! MEDIDO (2026-11-27): `sessionStop` lo usan **0/41** ⇒ NO es mínimo común
//! y no se implementa como método del núcleo; el estado "terminada" se
//! alcanza por `cancel` o por el `stopReason` de `session/prompt`.

use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};

use super::super::acp_schema::{ContentBlock, SessionId, StopReason};

/// El registro **global** del proceso.
///
/// MEDIDO (2026-11-27): el daemon es un proceso largo y `session/cancel`,
/// `session/resume` y `session/fork` son llamadas SUELTAS ⇒ el registro
/// tiene que sobrevivir entre llamadas; si no, cada una vería un registro
/// vacío.
pub fn global() -> &'static Mutex<Registro> {
    static G: OnceLock<Mutex<Registro>> = OnceLock::new();
    G.get_or_init(|| Mutex::new(Registro::nuevo()))
}

/// Coge el registro global tolerando un mutex envenenado (un panic previo
/// no puede dejar el daemon sin ACP).
pub fn con_global<R>(f: impl FnOnce(&mut Registro) -> R) -> R {
    let mut g = match global().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    f(&mut g)
}

/// Estado de una sesión.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// Activa: admite `session/prompt`.
    Activa,
    /// Cancelada: se puede `session/resume`.
    Cancelada,
    /// Terminada: el turno ended con un `stopReason`; no admite más prompts.
    Terminada,
}

impl Estado {
    /// El nombre estable (para el JSON).
    pub fn nombre(self) -> &'static str {
        match self {
            Estado::Activa => "activa",
            Estado::Cancelada => "cancelada",
            Estado::Terminada => "terminada",
        }
    }

    /// ¿Admite un `session/prompt`?
    pub fn admite_prompt(self) -> bool {
        matches!(self, Estado::Activa)
    }
}

/// Una sesión ACP.
#[derive(Debug, Clone, PartialEq)]
pub struct Sesion {
    /// Identificador (opaco para el cliente).
    pub id: String,
    /// Directorio de trabajo de la sesión.
    pub cwd: String,
    /// Modo actual (ACP v1 tiene modos; el registro los prueba poco).
    pub modo: Option<String>,
    /// La sesión de la que forked (el BOSQUE).
    pub padre: Option<String>,
    /// Estado.
    pub estado: Estado,
    /// Los prompts y las respuestas (historial conservado).
    pub mensajes: Vec<Value>,
    /// Turnos completados.
    pub turnos: u32,
}

impl Sesion {
    /// Identificador ACP tipado.
    pub fn session_id(&self) -> SessionId {
        SessionId(self.id.clone())
    }

    /// El resumen JSON (lo que ve el cliente).
    pub fn resumen(&self) -> Value {
        json!({
            "sessionId": self.id,
            "cwd": self.cwd,
            "estado": self.estado.nombre(),
            "modo": self.modo,
            "padre": self.padre,
            "turnos": self.turnos,
            "mensajes": self.mensajes.len(),
        })
    }
}

/// El registro de sesiones (en memoria).
///
/// MEDIDO: ACP no dice dónde se guardan las sesiones (el cliente trae el
/// historial con `session/load`).  Aquí viven en memoria del proceso; la
/// persistencia (SQLite, `board.rs`) es otro módulo.
#[derive(Debug, Default)]
pub struct Registro {
    /// Las sesiones, por id.
    sesiones: Vec<Sesion>,
    /// Contador para los ids.
    siguiente: u64,
}

impl Registro {
    /// Registro vacío.
    pub fn nuevo() -> Self {
        Registro::default()
    }

    fn id_nuevo(&mut self) -> String {
        self.siguiente += 1;
        format!("ses-{:06}", self.siguiente)
    }

    /// `session/new`.
    pub fn nueva(&mut self, cwd: &str, modo: Option<String>) -> Sesion {
        let id = self.id_nuevo();
        let s = Sesion {
            id: id.clone(),
            cwd: cwd.to_string(),
            modo,
            padre: None,
            estado: Estado::Activa,
            mensajes: Vec::new(),
            turnos: 0,
        };
        self.sesiones.push(s.clone());
        s
    }

    /// Busca una sesión por id.
    pub fn obtener(&self, id: &str) -> Option<&Sesion> {
        self.sesiones.iter().find(|s| s.id == id)
    }

    fn obtener_mut(&mut self, id: &str) -> Option<&mut Sesion> {
        self.sesiones.iter_mut().find(|s| s.id == id)
    }

    /// `session/list`: el resumen de todas.
    pub fn listar(&self) -> Vec<Value> {
        self.sesiones.iter().map(|s| s.resumen()).collect()
    }

    /// `session/cancel`: cancela una sesión activa.
    ///
    /// MEDIDO: `sessionStop` no existe en el registro (0/41) ⇒ la
    /// cancelación es el estado terminal que sí existe.
    pub fn cancelar(&mut self, id: &str) -> Result<Estado, String> {
        match self.obtener_mut(id) {
            None => Err(format!("no existe la sesión {}", id)),
            Some(s) => {
                s.estado = Estado::Cancelada;
                Ok(s.estado)
            }
        }
    }

    /// `session/resume`: reactiva una sesión cancelada.
    pub fn reanudar(&mut self, id: &str) -> Result<Estado, String> {
        match self.obtener_mut(id) {
            None => Err(format!("no existe la sesión {}", id)),
            Some(s) => {
                if s.estado == Estado::Terminada {
                    return Err(format!(
                        "la sesión {} está terminada y no se puede reanudar",
                        id
                    ));
                }
                s.estado = Estado::Activa;
                Ok(s.estado)
            }
        }
    }

    /// `session/fork`: una sesión HIJA que conserva el historial del padre.
    ///
    /// MEDIDO: el registro oficial lo prueba en 18/41 ⇒ es una extensión
    /// real, no una invención.  Y es la convergencia con el patrón 3 del
    /// doc de patrones (la conversación como bosque: el revert es un fork).
    pub fn fork(&mut self, id_padre: &str, cwd: Option<&str>) -> Result<Sesion, String> {
        let padre = self
            .obtener(id_padre)
            .ok_or_else(|| format!("no existe la sesión {}", id_padre))?
            .clone();
        let id = self.id_nuevo();
        let hija = Sesion {
            id: id.clone(),
            cwd: cwd.map(|c| c.to_string()).unwrap_or(padre.cwd.clone()),
            modo: padre.modo.clone(),
            padre: Some(padre.id.clone()),
            estado: Estado::Activa,
            // El historial se COPIA: el padre no se toca (deshacer sin
            // borrar).
            mensajes: padre.mensajes.clone(),
            turnos: padre.turnos,
        };
        self.sesiones.push(hija.clone());
        Ok(hija)
    }

    /// Registra un prompt en la sesión (para el historial).
    pub fn registrar_prompt(&mut self, id: &str, bloques: &[ContentBlock]) -> Result<(), String> {
        let s = self
            .obtener_mut(id)
            .ok_or_else(|| format!("no existe la sesión {}", id))?;
        if !s.estado.admite_prompt() {
            return Err(format!(
                "la sesión {} está {} y no admite un prompt",
                id,
                s.estado.nombre()
            ));
        }
        s.mensajes.push(json!({
            "role": "user",
            "content": bloques,
        }));
        Ok(())
    }

    /// Registra un **update** de la sesión (notificación `session/update`).
    ///
    /// MEDIDO (2026-11-28, `docs/DECISION_ACP.md` §3.2): el dispatch de los
    /// updates no estaba cableado.  El update se guarda en el **historial**
    /// de la sesión (que es lo que `session/load` devuelve) ⇒ el cliente que
    /// recarga la sesión ve también lo que pasó en ella.
    pub fn registrar_update(&mut self, id: &str, update: &Value) -> Result<(), String> {
        let s = self
            .obtener_mut(id)
            .ok_or_else(|| format!("no existe la sesión {}", id))?;
        s.mensajes.push(update.clone());
        Ok(())
    }

    /// El historial de una sesión (los mensajes y los updates).
    pub fn historial(&self, id: &str) -> Vec<Value> {
        match self.obtener(id) {
            Some(s) => s.mensajes.clone(),
            None => Vec::new(),
        }
    }

    /// Registra la respuesta de un turno y cierra el turno.
    pub fn cerrar_turno(&mut self, id: &str, razon: StopReason) -> Result<(), String> {
        let s = self
            .obtener_mut(id)
            .ok_or_else(|| format!("no existe la sesión {}", id))?;
        s.mensajes.push(json!({
            "role": "assistant",
            "stopReason": format!("{:?}", razon),
        }));
        s.turnos += 1;
        // MEDIDO: `sessionStop` no existe en ACP v1 (0/41) ⇒ una sesión
        // que termina por el `stopReason` del turno queda TERMINADA.
        if razon == StopReason::EndTurn {
            s.estado = Estado::Terminada;
        }
        Ok(())
    }

    /// El árbol de sesiones (padre → hijos): el BOSQUE.
    pub fn bosque(&self) -> Value {
        let raices: Vec<&Sesion> = self.sesiones.iter().filter(|s| s.padre.is_none()).collect();
        let hijos = |id: &str| -> Vec<Value> {
            self.sesiones
                .iter()
                .filter(|s| s.padre.as_deref() == Some(id))
                .map(|s| s.resumen())
                .collect()
        };
        // El mapa se construye FUERA del `json!`: `serde_json::Map` no es
        // `Serialize` como tipo, sólo sus valores.
        let mapa_hijos: serde_json::Map<String, Value> = raices
            .iter()
            .map(|s| (s.id.clone(), Value::Array(hijos(&s.id))))
            .collect();
        json!({
            "raices": raices.iter().map(|s| s.resumen()).collect::<Vec<_>>(),
            "hijos": mapa_hijos,
        })
    }
}

// =============================================================================
// CLI
// =============================================================================
/// `megadaemon-core acp-sesion <nueva|listar|cancelar|reanudar|fork|bosque>`
pub fn run_cli(args: &[String]) -> i32 {
    match run_cli_res(args) {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            0
        }
        Err(e) => {
            eprintln!("acp-sesion error: {}", e);
            1
        }
    }
}

fn run_cli_res(args: &[String]) -> Result<Value, String> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("");
    let mut r = Registro::nuevo();
    match sub {
        "nueva" => {
            let cwd = args.get(1).map(|s| s.as_str()).unwrap_or("/tmp");
            let s = r.nueva(cwd, None);
            Ok(json!({ "ok": true, "sesion": s.resumen() }))
        }
        "listar" => Ok(json!({ "ok": true, "sesiones": r.listar() })),
        "cancelar" | "reanudar" | "fork" => {
            let id = args.get(1).map(|s| s.as_str()).unwrap_or("");
            // Para que el CLI tenga algo que operar se crea la sesión.
            let s = r.nueva("/tmp", None);
            let _ = s;
            match sub {
                "cancelar" => {
                    let e = r.cancelar(id)?;
                    Ok(json!({ "ok": true, "estado": e.nombre() }))
                }
                "reanudar" => {
                    let e = r.reanudar(id)?;
                    Ok(json!({ "ok": true, "estado": e.nombre() }))
                }
                _ => {
                    let h = r.fork(id, None)?;
                    Ok(json!({ "ok": true, "sesion": h.resumen() }))
                }
            }
        }
        "bosque" => Ok(json!({ "ok": true, "bosque": r.bosque() })),
        "" => Err("uso: acp-sesion <nueva|listar|cancelar|reanudar|fork|bosque> [id]".to_string()),
        otro => Err(format!("subcomando desconocido: {}", otro)),
    }
}

// =============================================================================
// TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp_schema::TextContent;

    fn bloque(t: &str) -> ContentBlock {
        ContentBlock::Text(TextContent {
            text: t.to_string(),
            annotations: None,
            meta: None,
        })
    }

    #[test]
    fn una_sesion_nueva_esta_activa_y_tiene_id() {
        let mut r = Registro::nuevo();
        let s = r.nueva("/tmp", None);
        assert!(s.id.starts_with("ses-"));
        assert_eq!(s.estado, Estado::Activa);
        assert!(s.estado.admite_prompt());
        assert!(r.obtener(&s.id).is_some());
    }

    #[test]
    fn los_ids_son_distintos() {
        let mut r = Registro::nuevo();
        let a = r.nueva("/tmp", None);
        let b = r.nueva("/tmp", None);
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn cancelar_y_reanudar_cambian_el_estado() {
        let mut r = Registro::nuevo();
        let s = r.nueva("/tmp", None);
        assert_eq!(r.cancelar(&s.id).unwrap(), Estado::Cancelada);
        assert_eq!(r.reanudar(&s.id).unwrap(), Estado::Activa);
    }

    #[test]
    fn cancelar_una_sesion_que_no_existe_es_un_error() {
        let mut r = Registro::nuevo();
        assert!(r.cancelar("nope").is_err());
        assert!(r.reanudar("nope").is_err());
        assert!(r.fork("nope", None).is_err());
    }

    #[test]
    fn el_fork_crea_un_hijo_con_el_historial_del_padre() {
        let mut r = Registro::nuevo();
        let p = r.nueva("/tmp", None);
        r.registrar_prompt(&p.id, &[bloque("hola")]).unwrap();
        let h = r.fork(&p.id, None).unwrap();
        assert_eq!(h.padre.as_deref(), Some(p.id.as_str()));
        assert_eq!(h.mensajes.len(), 1);
        // El padre NO se toca.
        assert_eq!(r.obtener(&p.id).unwrap().mensajes.len(), 1);
        assert_eq!(r.obtener(&p.id).unwrap().turnos, 0);
    }

    #[test]
    fn el_fork_es_un_bosque_y_no_un_bosque_ciclico() {
        let mut r = Registro::nuevo();
        let p = r.nueva("/tmp", None);
        let h1 = r.fork(&p.id, None).unwrap();
        let h2 = r.fork(&h1.id, None).unwrap();
        assert_eq!(h2.padre.as_deref(), Some(h1.id.as_str()));
        let b = r.bosque();
        let raices = b["raices"].as_array().unwrap();
        assert_eq!(raices.len(), 1);
        assert_eq!(raices[0]["sessionId"], json!(p.id));
    }

    #[test]
    fn una_sesion_cancelada_no_admite_prompts() {
        let mut r = Registro::nuevo();
        let s = r.nueva("/tmp", None);
        r.cancelar(&s.id).unwrap();
        let e = r.registrar_prompt(&s.id, &[bloque("hola")]).unwrap_err();
        assert!(e.contains("cancelada"), "{}", e);
    }

    #[test]
    fn cerrar_el_turno_registra_el_motivo_y_termina_la_sesion() {
        let mut r = Registro::nuevo();
        let s = r.nueva("/tmp", None);
        r.registrar_prompt(&s.id, &[bloque("hola")]).unwrap();
        r.cerrar_turno(&s.id, StopReason::EndTurn).unwrap();
        // MEDIDO: el prestamo de `r.obtener(...)` se acaba en cada
        // sentencia; si se guarda la referencia, el `r.reanudar` de abajo
        // no compila (E0502).
        let turnos = r.obtener(&s.id).unwrap().turnos;
        let estado = r.obtener(&s.id).unwrap().estado;
        assert_eq!(turnos, 1);
        assert_eq!(estado, Estado::Terminada);
        // Y una sesión terminada NO se puede reanudar.
        assert!(r.reanudar(&s.id).is_err());
    }

    #[test]
    fn el_historial_conserva_prompt_y_respuesta() {
        let mut r = Registro::nuevo();
        let s = r.nueva("/tmp", None);
        r.registrar_prompt(&s.id, &[bloque("hola")]).unwrap();
        r.cerrar_turno(&s.id, StopReason::EndTurn).unwrap();
        let s = r.obtener(&s.id).unwrap();
        assert_eq!(s.mensajes.len(), 2);
        assert_eq!(s.mensajes[0]["role"], json!("user"));
        assert_eq!(s.mensajes[1]["role"], json!("assistant"));
    }
}
