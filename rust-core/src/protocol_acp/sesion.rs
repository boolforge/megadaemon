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
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use super::super::acp_schema::{ContentBlock, SessionId, StopReason};

/// El almacén de sesiones SQLite **por hilo**.
///
/// MEDIDO (2026-11-28): en los TESTS `state_dir()` es un temporal **por
/// hilo** ⇒ el almacén tiene que ser por hilo también: si fuese global, el
/// primer hilo que lo abriera fijaría el camino y los demás tests
/// escribirían en el estado de otro (el mismo bug que ya corrigió
/// `state_dir.rs`).  En producción hay un único estado y SQLite abre la misma
/// base desde varios hilos sin problema (WAL + `busy_timeout`).
thread_local! {
    static STORE: RefCell<Option<crate::acp_sesion_store::AcpSesionStore>> =
        RefCell::new(None);
}

/// Abre (o reutiliza) el almacén de sesiones y ejecuta `f` con él.
///
/// La persistencia es un **efecto secundario, nunca un bloqueo**: si el
/// almacén no se puede abrir, se avisa por stderr y el ACP sigue **en
/// memoria** (devuelve `None`).
fn con_store<R>(f: impl FnOnce(&crate::acp_sesion_store::AcpSesionStore) -> R) -> Option<R> {
    STORE.with(|celda| {
        let mut c = celda.borrow_mut();
        if c.is_none() {
            match crate::acp_sesion_store::AcpSesionStore::abrir_en_state_dir() {
                Ok(s) => *c = Some(s),
                Err(e) => {
                    eprintln!("acp-sesion: no se pudo abrir el almacen: {}", e);
                    return None;
                }
            }
        }
        c.as_ref().map(f)
    })
}

/// El registro **global** del proceso.
///
/// MEDIDO (2026-11-27): el daemon es un proceso largo y `session/cancel`,
/// `session/resume` y `session/fork` son llamadas SUELTAS ⇒ el registro
/// tiene que sobrevivir entre llamadas; si no, cada una vería un registro
/// vacío.
pub fn global() -> &'static Mutex<Registro> {
    static G: OnceLock<Mutex<Registro>> = OnceLock::new();
    // MEDIDO (2026-11-28): el registro se RESTAURA del almacen SQLite
    // (`acp_sesion_store`): las sesiones y su historial sobreviven a un
    // reinicio del daemon.
    G.get_or_init(|| Mutex::new(Registro::restaurar()))
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
    /// El **BOSQUE** de cada sesión (patrón 3): los nodos de mensajes.
    ///
    /// MEDIDO (2026-11-28): el bosque se **persiste** en la tabla medida
    /// `message_nodes` (`acp_sesion_store::guardar_bosque`) y se
    /// **restaura** al arrancar ([`Registro::restaurar`]).  Antes el bosque
    /// sólo existía en memoria del proceso y cada llamada lo reconstruía
    /// desde los mensajes que le pasaban (`capacidades.rs::revert`): al
    /// reiniciar el daemon se perdían las ramas.
    bosques: BTreeMap<String, crate::message_tree::Estado>,
}

impl Registro {
    /// Registro vacío.
    pub fn nuevo() -> Self {
        Registro::default()
    }

    /// **Restaura** las sesiones del almacén SQLite.
    ///
    /// MEDIDO (2026-11-28): la persistencia de sesiones (el store
    /// `acp_sesion_store`) era un módulo sin usar; ahora las sesiones
    /// sobreviven a un reinicio del daemon: al arrancar se recuperan del
    /// almacén con su historial completo.
    ///
    /// Si el almacén no está (o no se puede leer) se devuelve un registro
    /// vacío: el ACP tiene que funcionar aunque no haya persistencia.
    pub fn restaurar() -> Registro {
        let mut r = Registro::nuevo();
        con_store(|store| {
            let Ok(resumenes) = crate::acp_sesion_store::listar(store.conexion()) else {
                return;
            };
            for res in resumenes {
                let historial =
                    crate::acp_sesion_store::cargar_historial(store.conexion(), &res.id).ok();
                // Los mensajes **detallados** (con su época y su `seq`): son
                // los que permiten **reconstruir** el bosque si no estuviera
                // guardado.
                let detallados = historial
                    .as_ref()
                    .map(|h| h.mensajes.clone())
                    .unwrap_or_default();
                let (modo, padre, estado, turnos, mensajes) = match historial {
                    Some(h) => (
                        h.sesion.modo.clone(),
                        h.sesion.padre.clone(),
                        h.sesion.estado,
                        h.sesion.turnos,
                        h.sesion.mensajes.clone(),
                    ),
                    None => (
                        res.modo.clone(),
                        res.padre.clone(),
                        res.estado,
                        res.turnos,
                        Vec::new(),
                    ),
                };
                r.sesiones.push(Sesion {
                    id: res.id.clone(),
                    cwd: res.cwd.clone(),
                    modo,
                    padre,
                    estado: match estado.nombre() {
                        "cancelada" => Estado::Cancelada,
                        "terminada" => Estado::Terminada,
                        _ => Estado::Activa,
                    },
                    mensajes,
                    turnos,
                });
                // EL BOSQUE (patrón 3): se restaura de `message_nodes` ⇒ la
                // topología (padre/hijo, ramas) sobrevive al reinicio.
                r.bosques.insert(
                    res.id.clone(),
                    Self::restaurar_bosque(store, &res.id, &detallados),
                );
                // El contador de ids sigue DESPUES de las ids restauradas,
                // para que una sesion nueva no repita una id.
                if let Some(n) = res
                    .id
                    .strip_prefix("ses-")
                    .and_then(|x| x.parse::<u64>().ok())
                {
                    if n >= r.siguiente {
                        r.siguiente = n;
                    }
                }
            }
        });
        r
    }

    fn id_nuevo(&mut self) -> String {
        self.siguiente += 1;
        format!("ses-{:06}", self.siguiente)
    }

    /// La sesión en el formato del STORE (lo que se persiste).
    ///
    /// MEDIDO (2026-11-28): el modelo en memoria y el modelo persistido son
    /// casi el mismo; el store añade la época actual y las marcas de
    /// tiempo.  La conversión vive aquí para que el store no dependa del
    /// ACP y el ACP no dependa del store.
    fn a_sesion_store(&self, s: &Sesion) -> crate::acp_sesion_store::SesionAcp {
        let ahora = crate::acp_sesion_store::ahora();
        crate::acp_sesion_store::SesionAcp {
            id: s.id.clone(),
            cwd: s.cwd.clone(),
            modo: s.modo.clone(),
            padre: s.padre.clone(),
            estado: crate::acp_sesion_store::EstadoSesion::desde_nombre(s.estado.nombre()),
            // La época la usa el store para la compactacion; el ACP no
            // compacta (el historial se conserva entero) ⇒ siempre 0.
            current_epoch: 0,
            turnos: s.turnos,
            mensajes: s.mensajes.clone(),
            created_at: ahora,
            updated_at: ahora,
        }
    }

    /// **Persiste** la sesión en SQLite (patrón: la persistencia es otro
    /// módulo, `acp_sesion_store`).
    ///
    /// MEDIDO (2026-11-28): si el almacén no se puede abrir o escribir, la
    /// sesión ACP **sigue funcionando** (está en memoria): la persistencia
    /// es un efecto secundario, nunca un bloqueo.  El fallo se avisa por
    /// stderr para que se pueda ver.
    ///
    /// MEDIDO (`acp_sesion_store::guardar_sesion`): el guardado es un
    /// **upsert** que anexa **sólo el sufijo** del historial que faltaba
    /// (prefijo común) ⇒ guardar dos veces la misma sesión **no** duplica
    /// mensajes.
    fn persistir(&mut self, id: &str) {
        let Some(s) = self.obtener(id) else {
            return;
        };
        let cwd = s.cwd.clone();
        let modo = s.modo.clone();
        let mut sesion = self.a_sesion_store(s);
        // El bosque conoce los datos de su sesión (los que guarda la tabla
        // medida `sessions`): se sincronizan aquí ⇒ el bosque **restaurado**
        // es igual al que había en memoria.
        if let Some(b) = self.bosques.get_mut(id) {
            b.sesion.working_directory = cwd;
            b.sesion.backend_type = modo.unwrap_or_default();
            b.sesion.model = String::new();
            b.sesion.permission_mode = String::new();
            b.sesion.total_tokens = 0;
        }
        // La época viva es la del BOSQUE (patrón 3): una bifurcación (un
        // revert por pasos) abre una época nueva y la fila medida
        // `sessions.current_epoch` la guarda.
        let bosque = self.bosques.get(id);
        if let Some(b) = bosque {
            sesion.current_epoch = b.current_epoch;
        }
        con_store(|store| {
            if let Err(e) = crate::acp_sesion_store::guardar_sesion(store.conexion(), &sesion) {
                eprintln!(
                    "acp-sesion: no se pudo guardar la sesion {}: {}",
                    sesion.id, e
                );
            }
            // El BOSQUE en la tabla medida `message_nodes`.  Si falla, la
            // sesión sigue **en memoria** (la persistencia es un efecto
            // secundario, nunca un bloqueo) y el fallo se avisa por stderr.
            if let Some(b) = bosque {
                if let Err(e) = crate::acp_sesion_store::guardar_bosque(store.conexion(), b) {
                    eprintln!(
                        "acp-sesion: no se pudo guardar el bosque de {}: {}",
                        b.session_id, e
                    );
                }
            }
        });
    }

    /// El **bosque de mensajes** de una sesión (si lo hay): la topología de
    /// sus nodos (patrón 3, tabla medida `message_nodes`).
    ///
    /// No confundir con [`Registro::bosque`]: aquel es el árbol de
    /// **sesiones** (padre → hijos) y éste el de **mensajes** de una sesión.
    pub fn bosque_mensajes(&self, id: &str) -> Option<&crate::message_tree::Estado> {
        self.bosques.get(id)
    }

    /// El bosque de una sesión, **creándolo** si no está (para que siempre
    /// haya uno donde colgar los mensajes).
    fn bosque_mut(&mut self, id: &str) -> &mut crate::message_tree::Estado {
        self.bosques
            .entry(id.to_string())
            .or_insert_with(|| crate::message_tree::Estado::nuevo(id.to_string()))
    }

    /// **Añade un nodo** al bosque de la sesión.
    ///
    /// El rol y el contenido salen del **mismo** `Value` ACP que se persiste
    /// en `messages` ⇒ el nodo del bosque y la fila del mensaje dicen
    /// exactamente lo mismo.
    fn anadir_nodo(&mut self, id: &str, v: &Value) {
        let rol = crate::acp_sesion_store::rol_de_json(v);
        let contenido = crate::acp_sesion_store::contenido_de_json(v);
        let b = self.bosque_mut(id);
        if let Err(e) = b.anadir_mensaje(rol, contenido) {
            eprintln!(
                "acp-sesion: no se pudo anadir el nodo al bosque de {}: {}",
                id, e
            );
        }
    }

    /// **Restaura** el bosque de una sesión del almacén.
    ///
    /// * Si el bosque está guardado (`message_nodes`) ⇒ se carga **tal cual**
    ///   (con sus ramas).
    /// * Si **no** está (la sesión se guardó antes de que existiera la
    ///   persistencia del bosque) ⇒ se **reconstruye** del historial
    ///   persistido ⇒ no se pierde nada y el bosque queda completo desde el
    ///   primer arranque.
    fn restaurar_bosque(
        store: &crate::acp_sesion_store::AcpSesionStore,
        id: &str,
        historial: &[crate::acp_sesion_store::MensajeAcp],
    ) -> crate::message_tree::Estado {
        match crate::acp_sesion_store::cargar_bosque(store.conexion(), id) {
            Ok(Some(b)) => b,
            Ok(None) => {
                let b = Self::bosque_desde_historial(id, historial);
                if let Err(e) = crate::acp_sesion_store::guardar_bosque(store.conexion(), &b) {
                    eprintln!(
                        "acp-sesion: no se pudo guardar el bosque reconstruido de {}: {}",
                        id, e
                    );
                }
                b
            }
            Err(e) => {
                eprintln!("acp-sesion: no se pudo leer el bosque de {}: {}", id, e);
                // Sin bosque en el almacén: se reconstruye igual (en memoria)
                // ⇒ la sesión sigue funcionando.
                Self::bosque_desde_historial(id, historial)
            }
        }
    }

    /// Reconstruye el bosque de una sesión a partir de su historial
    /// **persistido** (los mensajes con su época y su `seq`).
    ///
    /// El `seq` y la época son los **medidos** de `messages`
    /// (`sequence_number` y `compaction_epoch`) ⇒ el bosque reconstruido
    /// tiene las mismas claves que las filas del historial.
    fn bosque_desde_historial(
        id: &str,
        historial: &[crate::acp_sesion_store::MensajeAcp],
    ) -> crate::message_tree::Estado {
        let mut b = crate::message_tree::Estado::nuevo(id.to_string());
        for m in historial {
            if let Err(e) = b.anadir_mensaje_completo(
                m.rol.clone(),
                m.contenido.clone(),
                m.compaction_epoch,
                m.sequence_number,
                m.created_at,
            ) {
                eprintln!(
                    "acp-sesion: no se pudo reconstruir el nodo {} de {}: {}",
                    m.sequence_number, id, e
                );
                break;
            }
        }
        b
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
        // El bosque empieza vacío: la sesión todavía no tiene mensajes.
        self.bosques
            .insert(id.clone(), crate::message_tree::Estado::nuevo(id.clone()));
        self.persistir(&s.id);
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
        let estado = match self.obtener_mut(id) {
            None => return Err(format!("no existe la sesión {}", id)),
            Some(s) => {
                s.estado = Estado::Cancelada;
                s.estado
            }
        };
        self.persistir(id);
        Ok(estado)
    }

    /// `session/resume`: reactiva una sesión cancelada.
    pub fn reanudar(&mut self, id: &str) -> Result<Estado, String> {
        let estado = match self.obtener_mut(id) {
            None => return Err(format!("no existe la sesión {}", id)),
            Some(s) => {
                if s.estado == Estado::Terminada {
                    return Err(format!(
                        "la sesión {} está terminada y no se puede reanudar",
                        id
                    ));
                }
                s.estado = Estado::Activa;
                s.estado
            }
        };
        self.persistir(id);
        Ok(estado)
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
        // El BOSQUE del hijo: una **COPIA** del del padre (patrón 3: el
        // fork copia el historial y el padre **no** cambia, ni un mensaje).
        let bosque = match self.bosques.get(id_padre) {
            Some(b) => b.copiar_para(id.clone()),
            None => crate::message_tree::Estado::nuevo(id.clone()),
        };
        self.bosques.insert(id.clone(), bosque);
        self.persistir(&hija.id);
        Ok(hija)
    }

    /// Registra un prompt en la sesión (para el historial).
    pub fn registrar_prompt(&mut self, id: &str, bloques: &[ContentBlock]) -> Result<(), String> {
        let v = json!({
            "role": "user",
            "content": bloques,
        });
        {
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
            s.mensajes.push(v.clone());
        }
        // El BOSQUE: un nodo **hijo** de la cabeza viva.
        self.anadir_nodo(id, &v);
        self.persistir(id);
        Ok(())
    }

    /// Registra un **update** de la sesión (notificación `session/update`).
    ///
    /// MEDIDO (2026-11-28, `docs/DECISION_ACP.md` §3.2): el dispatch de los
    /// updates no estaba cableado.  El update se guarda en el **historial**
    /// de la sesión (que es lo que `session/load` devuelve) ⇒ el cliente que
    /// recarga la sesión ve también lo que pasó en ella.
    pub fn registrar_update(&mut self, id: &str, update: &Value) -> Result<(), String> {
        {
            let s = self
                .obtener_mut(id)
                .ok_or_else(|| format!("no existe la sesión {}", id))?;
            s.mensajes.push(update.clone());
        }
        // El BOSQUE: un nodo **hijo** de la cabeza viva.
        self.anadir_nodo(id, update);
        self.persistir(id);
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
        let v = json!({
            "role": "assistant",
            "stopReason": format!("{:?}", razon),
        });
        {
            let s = self
                .obtener_mut(id)
                .ok_or_else(|| format!("no existe la sesión {}", id))?;
            s.mensajes.push(v.clone());
            s.turnos += 1;
            // MEDIDO: `sessionStop` no existe en ACP v1 (0/41) ⇒ una sesión
            // que termina por el `stopReason` del turno queda TERMINADA.
            if razon == StopReason::EndTurn {
                s.estado = Estado::Terminada;
            }
        }
        // El BOSQUE: un nodo **hijo** de la cabeza viva.
        self.anadir_nodo(id, &v);
        self.persistir(id);
        Ok(())
    }

    /// Los **pasos de revert** de una sesión: los puntos a los que se puede
    /// volver (patrón 3, `message_tree::pasos_revert`).
    ///
    /// MEDIDO: la referencia expone `cognition.ai/revert/listSteps` ⇒ los
    /// puntos de retorno se **listan** y luego se ejecuta el elegido.
    pub fn pasos_de_revert(&self, id: &str, objetivo: i64) -> Vec<crate::message_tree::PasoRevert> {
        match self.bosques.get(id) {
            Some(b) => crate::message_tree::pasos_revert(b, objetivo),
            None => Vec::new(),
        }
    }

    /// **Revierte** a un punto del bosque **sin borrar nada** y **bifurca**
    /// (patrón 3): la rama nueva es la que sigue viva y la rama anterior
    /// sigue consultable.
    ///
    /// MEDIDO: el revert de la referencia es un **fork** del bosque
    /// (`cognition.ai/revert/forkFromStep`) ⇒ el punto elegido se convierte
    /// en un punto de retorno nuevo y **nada se borra**.
    ///
    /// Devuelve cuántos mensajes ve ahora el agente (la vista viva de la
    /// rama nueva).
    pub fn revertir_a_paso(
        &mut self,
        id: &str,
        objetivo: i64,
        indice: usize,
    ) -> Result<usize, String> {
        let bosque = self
            .bosques
            .get(id)
            .ok_or_else(|| format!("no existe la sesión {}", id))?
            .clone();
        let mut preparado = bosque.clone();
        preparado.preparar_revert(objetivo)?;
        let nuevo = crate::message_tree::aplicar_paso(&preparado, indice)?;
        let vista = nuevo.vista_viva().len();
        self.bosques.insert(id.to_string(), nuevo);
        // El BOSQUE cambia ⇒ se persiste (la rama nueva queda en
        // `message_nodes`).
        self.persistir(id);
        Ok(vista)
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
    fn la_sesion_se_persiste_y_vuelve_del_almacen() {
        // MEDIDO (2026-11-28): la persistencia de sesiones (el store
        // `acp_sesion_store`) ya no es un modulo sin usar: lo que se crea
        // queda en el almacen y `restaurar()` lo devuelve.
        let mut r = Registro::nuevo();
        let s = r.nueva("/tmp/mi-proyecto", Some("code".to_string()));
        r.registrar_prompt(&s.id, &[bloque("hola")]).unwrap();

        // 1) En el almacen.
        let store = crate::acp_sesion_store::AcpSesionStore::abrir_en_state_dir()
            .expect("el almacen tiene que abrir");
        let cargada = crate::acp_sesion_store::cargar_sesion(store.conexion(), &s.id)
            .expect("la sesion tiene que estar en el almacen");
        assert_eq!(cargada.cwd, "/tmp/mi-proyecto");
        assert_eq!(cargada.modo.as_deref(), Some("code"));

        // 2) Y al restaurar el registro vuelve con su historial.
        let r2 = Registro::restaurar();
        let s2 = r2.obtener(&s.id).expect("la sesion restaurada");
        assert_eq!(s2.cwd, "/tmp/mi-proyecto");
        assert_eq!(s2.modo.as_deref(), Some("code"));
        assert_eq!(s2.mensajes.len(), 1, "el historial se conserva");
        assert!(
            !s2.mensajes.is_empty(),
            "el historial no se pierde: {:?}",
            s2.mensajes
        );
    }

    #[test]
    fn guardar_dos_veces_no_duplica_el_historial() {
        // MEDIDO (2026-11-28): `guardar_sesion` anexa SOLO el sufijo que
        // faltaba (prefijo comun) ⇒ persistir la misma sesion varias
        // veces no duplica mensajes.
        let mut r = Registro::nuevo();
        let s = r.nueva("/tmp/mi-proyecto", None);
        r.registrar_prompt(&s.id, &[bloque("hola")]).unwrap();
        r.registrar_prompt(&s.id, &[bloque("adios")]).unwrap();

        let r2 = Registro::restaurar();
        let s2 = r2.obtener(&s.id).expect("la sesion restaurada");
        assert_eq!(s2.mensajes.len(), 2, "dos prompts, dos mensajes");
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

    // ── EL BOSQUE (patrón 3) ───────────────────────────────────────────

    #[test]
    fn el_bosque_se_persiste_y_vuelve_al_arrancar() {
        // MEDIDO (2026-11-28): el bosque (patrón 3) se **persiste** en la
        // tabla medida `message_nodes` y se **restaura** al arrancar: al
        // reiniciar el daemon el árbol sigue ahí (antes sólo vivía en
        // memoria y se reconstruía en cada llamada).
        let mut r = Registro::nuevo();
        let s = r.nueva("/tmp/mi-proyecto", None);
        r.registrar_prompt(&s.id, &[bloque("hola")]).unwrap();
        r.registrar_update(
            &s.id,
            &json!({
                "sessionUpdate": "agent_message_chunk",
                "content": {"type": "text", "text": "buenas"},
            }),
        )
        .unwrap();

        // 1) En el almacén, en la tabla MEDIDA.
        let store = crate::acp_sesion_store::AcpSesionStore::abrir_en_state_dir()
            .expect("el almacen tiene que abrir");
        let filas: i64 = store
            .conexion()
            .query_row(
                "SELECT COUNT(*) FROM message_nodes WHERE session_id=?1",
                [&s.id],
                |r| r.get(0),
            )
            .expect("cuenta los nodos");
        assert_eq!(filas, 2, "un nodo por mensaje");

        // 2) Al arrancar de nuevo, el bosque vuelve con su topología.
        let r2 = Registro::restaurar();
        let b = r2.bosque_mensajes(&s.id).expect("el bosque restaurado");
        assert_eq!(b.nodos.len(), 2, "los dos nodos vuelven");
        assert_eq!(b.raices().len(), 1, "una raíz");
        let cabeza = b.cabeza.expect("cabeza viva");
        assert_eq!(
            b.nodos[&cabeza].padre,
            Some(1),
            "el segundo cuelga del primero"
        );
        assert_eq!(
            b.vista_viva().len(),
            2,
            "la vista viva son los dos mensajes"
        );
        // Y el bosque restaurado es IGUAL al que había en memoria.
        assert_eq!(b, r.bosque_mensajes(&s.id).expect("el bosque vivo"));
    }

    #[test]
    fn el_revert_por_pasos_bifurca_el_bosque_y_la_rama_se_persiste() {
        // MEDIDO (patrón 3): el revert es un **fork** del bosque: la rama
        // original **no** se toca y la nueva es la que sigue viva.
        let mut r = Registro::nuevo();
        let s = r.nueva("/tmp", None);
        r.registrar_prompt(&s.id, &[bloque("uno")]).unwrap();
        r.registrar_prompt(&s.id, &[bloque("dos")]).unwrap();
        r.registrar_prompt(&s.id, &[bloque("tres")]).unwrap();
        assert_eq!(r.bosque_mensajes(&s.id).unwrap().nodos.len(), 3);

        // Los puntos de retorno, del más cercano al más lejano.
        let pasos = r.pasos_de_revert(&s.id, 1);
        assert_eq!(pasos.len(), 2, "dos puntos de retorno");
        assert_eq!(pasos[0].nodo_id, 2, "el más cercano primero");
        assert_eq!(pasos[1].nodo_id, 1);

        // Se ejecuta el paso elegido: el bosque se bifurca (y no borra).
        let vista = r.revertir_a_paso(&s.id, 1, 1).expect("revierte");
        assert_eq!(vista, 1, "la rama nueva ve sólo el primer mensaje");
        let b = r.bosque_mensajes(&s.id).expect("el bosque");
        assert_eq!(b.nodos.len(), 4, "tres mensajes y el nodo de bifurcación");
        assert_eq!(b.raices().len(), 1, "sigue siendo un árbol con una raíz");
        assert_eq!(b.historial_de(3).len(), 3, "la rama original sigue intacta");

        // Al arrancar de nuevo, la bifurcación sigue ahí.
        let r2 = Registro::restaurar();
        let b2 = r2.bosque_mensajes(&s.id).expect("el bosque restaurado");
        assert_eq!(b2.nodos, b.nodos, "la rama vuelve igual");
        assert_eq!(b2.cabeza, b.cabeza);
        assert_eq!(b2.current_epoch, b.current_epoch, "la época viva también");
    }

    #[test]
    fn el_fork_copia_el_bosque_y_el_padre_no_cambia() {
        // MEDIDO (patrón 3, criterio 1): tras un fork el padre **no**
        // cambia (ni un mensaje) y el hijo tiene el historial copiado.
        let mut r = Registro::nuevo();
        let p = r.nueva("/tmp", None);
        r.registrar_prompt(&p.id, &[bloque("hola")]).unwrap();
        let antes = r.bosque_mensajes(&p.id).expect("bosque del padre").clone();

        let h = r.fork(&p.id, None).expect("fork");
        let hijo = r.bosque_mensajes(&h.id).expect("bosque del hijo");
        assert_eq!(hijo.nodos.len(), 1, "el hijo tiene su copia");
        assert_eq!(hijo.nodos[&1].historial.len(), 1);
        // El padre no se ha movido.
        assert_eq!(
            r.bosque_mensajes(&p.id).expect("bosque del padre"),
            &antes,
            "el padre no cambia"
        );
        // Y al arrancar, los dos bosques vuelven.
        let r2 = Registro::restaurar();
        assert_eq!(r2.bosque_mensajes(&p.id).expect("padre"), &antes);
        assert_eq!(r2.bosque_mensajes(&h.id).expect("hijo").nodos.len(), 1);
    }

    #[test]
    fn una_sesion_sin_bosque_se_reconstruye_al_arrancar() {
        // Si la sesión se guardó **antes** de que existiera la persistencia
        // del bosque, al arrancar se **reconstruye** del historial
        // persistido ⇒ no se pierde nada (y queda guardado para la
        // siguiente vez).
        let mut r = Registro::nuevo();
        let s = r.nueva("/tmp", None);
        r.registrar_prompt(&s.id, &[bloque("hola")]).unwrap();
        r.registrar_update(
            &s.id,
            &json!({
                "sessionUpdate": "agent_message_chunk",
                "content": {"type": "text", "text": "buenas"},
            }),
        )
        .unwrap();

        // Se borra el bosque (como si la sesión se hubiera guardado antes).
        let store = crate::acp_sesion_store::AcpSesionStore::abrir_en_state_dir()
            .expect("el almacen tiene que abrir");
        store
            .conexion()
            .execute("DELETE FROM message_nodes WHERE session_id=?1", [&s.id])
            .expect("borra el bosque");
        assert!(
            crate::acp_sesion_store::cargar_bosque(store.conexion(), &s.id)
                .expect("carga")
                .is_none()
        );

        // Al arrancar, el bosque vuelve (reconstruido del historial).
        let r2 = Registro::restaurar();
        let b = r2.bosque_mensajes(&s.id).expect("el bosque reconstruido");
        assert_eq!(b.nodos.len(), 2, "los dos mensajes vuelven al bosque");
        assert_eq!(b.vista_viva().len(), 2);
        assert_eq!(b.raices().len(), 1);
        // Y queda guardado: la próxima vez viene de `message_nodes`.
        assert!(
            crate::acp_sesion_store::cargar_bosque(store.conexion(), &s.id)
                .expect("carga")
                .is_some()
        );
    }

    #[test]
    fn si_el_almacen_no_abre_la_sesion_sigue_en_memoria() {
        // MEDIDO (2026-11-28): la persistencia es un efecto secundario: si
        // el almacén no se puede abrir, el ACP sigue **en memoria** y el
        // fallo se avisa por stderr (no se pierde nada).
        // Se rompe el almacén poniendo un DIRECTORIO donde debería estar la
        // base (`board.sqlite`): SQLite no puede abrirlo.
        let dir = crate::state_dir::state_dir();
        std::fs::create_dir_all(&dir).expect("crea el estado");
        std::fs::create_dir_all(format!("{}/{}", dir, crate::acp_sesion_store::ACPS_DB_FILE))
            .expect("rompe la base");
        assert!(
            crate::acp_sesion_store::AcpSesionStore::abrir_en_state_dir().is_err(),
            "el almacen tiene que estar caido para que este test signifique algo"
        );

        let mut r = Registro::nuevo();
        let s = r.nueva("/tmp", None);
        r.registrar_prompt(&s.id, &[bloque("hola")]).unwrap();
        r.registrar_update(
            &s.id,
            &json!({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "hola"}}),
        )
        .unwrap();

        // La sesión sigue en memoria, con su historial...
        let sesion = r.obtener(&s.id).expect("la sesion sigue en memoria");
        assert_eq!(sesion.mensajes.len(), 2, "el historial no se pierde");
        // ...y su bosque.
        let b = r
            .bosque_mensajes(&s.id)
            .expect("el bosque sigue en memoria");
        assert_eq!(b.nodos.len(), 2, "los dos nodos siguen en memoria");
        assert_eq!(b.vista_viva().len(), 2);
        assert_eq!(b.raices().len(), 1);
    }
}
