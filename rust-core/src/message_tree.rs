//! # Patrón 3 — La conversación es un **BOSQUE**
//!
//! Implementa el **patrón 3** de `docs/PATRONES_CLI_RUST_DEFINICION.md:93-124`:
//! la conversación **no** es una lista, es un **bosque** de nodos; se puede
//! **bifurcar** (`fork`) y se puede **revertir** **sin borrar nunca nada**.
//!
//! ## Fuente MEDIDA
//!
//! * El bosque: `message_nodes.parent_node_id` (la tabla **medida** con
//!   `UNIQUE(session_id, node_id)` y `parent_node_id` nulo en la raíz) —
//!   `docs/RE_DEVIN_CLI_RUST.md:243-251` y §4.2 (`docs/RE_DEVIN_CLI_RUST.md:292`).
//! * El revert es un **fork del bosque**: la referencia expone
//!   `cognition.ai/revert/{listSteps,preview,execute,forkFromStep}`
//!   (`docs/RE_DEVIN_CLI_RUST.md:292`).  De ahí sale el **revert por pasos**:
//!   listar los puntos de retorno y ejecutar el elegido.
//! * La época: `sessions.current_epoch` + `messages.compaction_epoch` con
//!   `UNIQUE(session_id, compaction_epoch, sequence_number)`
//!   (`docs/RE_DEVIN_CLI_RUST.md:218-241`).
//! * El contrato del patrón (nodo `{id, padre, secuencia, epoch}`, `fork` con
//!   historial **copiado**, `revert` que **no borra**, bosque con **varias
//!   raíces**): `docs/PATRONES_CLI_RUST_DEFINICION.md:100-117`.
//!
//! ## Lo que está NO MEDIDO
//!
//! * **Cómo** guarda la referencia la copia del historial de un fork: NO
//!   MEDIDO.  Aquí se deduce del DDL medido (ver abajo).
//! * **El formato** de la etiqueta de un paso de revert: NO MEDIDO.  Aquí es
//!   `"{rol}: {contenido}"` recortado, y `"punto de retorno"` para los nodos
//!   que son un fork.
//! * **El contenido** de `chat_message`: NO MEDIDO (la referencia sólo mide
//!   que es `TEXT NOT NULL`).  Aquí es **nuestro** JSON.
//! * Si un fork de la referencia abre una época nueva: NO MEDIDO, pero
//!   **forzado** por el DDL medido (ver abajo).
//!
//! ## Por qué un fork **obliga** a abrir una época (derivado del DDL medido)
//!
//! `messages` medido tiene `UNIQUE(session_id, compaction_epoch,
//! sequence_number)`.  El fork **copia** el historial del padre, y la copia
//! tiene **los mismos `sequence_number`** que el original (el `seq` es global
//! y no se reinicia, igual que en `compaction_epoch.rs`).  Si la copia viviera
//! en la **misma** época, chocaría con el `UNIQUE` del padre ⇒ o se pisaba el
//! original (prohibido: el revert **no borra**) o no podía existir.
//!
//! ⇒ **El fork abre una época nueva** para su copia.  Es la misma semántica de
//! época que ya usa `compaction_epoch.rs`: una época es un tramo del registro,
//! y cambiar de época **no borra** el anterior (`revertir_a(epoch)`).
//!
//! ## Modelo (puro, sin E/S)
//!
//! * [`Nodo`] = `{id, padre, secuencia, epoch}` + el **historial materializado**
//!   (la copia) + su mensaje propio.
//! * [`Estado`] = el bosque: los nodos, la **cabeza** (el nodo vivo), la época
//!   actual y el objetivo de revert preparado.
//! * Un nodo **nunca** se muta: añadir un mensaje crea un **hijo**; por eso
//!   el historial de un nodo es siempre el mismo después de tener hijos, y el
//!   historial copiado de un fork **no** cambia aunque el padre siga vivo.
//!
//! ## Persistencia
//!
//! Se guarda en SQLite con las tablas **medidas** del patrón 10
//! (`crate::board_schema`):
//!
//! | dato | tabla medida | columnas |
//! |---|---|---|
//! | topología del bosque | `message_nodes` | `session_id`, `node_id`, `parent_node_id`, `chat_message`, `created_at` |
//! | registro de mensajes por época | `messages` | `sequence_number`, `role`, `content`, `timestamp`, `compaction_epoch` |
//! | época viva | `sessions` | `current_epoch` |
//!
//! `chat_message` guarda el **mensaje propio** del nodo en JSON (nuestro
//! formato, NO MEDIDO).  El historial de un nodo **se reconstruye** al cargar
//! recorriendo la cadena de padres (invariante: el historial de un nodo = los
//! mensajes de su camino desde la raíz).
//!
//! No hace falta ninguna tabla extra: `app_state` (tabla **medida**) guarda la
//! cabeza viva con la clave `message_tree.head/{session_id}` (el nombre de la
//! clave es **nuestro**).
//!
//! ## Reparto de las tablas medidas (patrón 10)
//!
//! | tabla | dueño | qué escribe |
//! |---|---|---|
//! | `message_nodes` | **el bosque** | la topología (un nodo = una fila) |
//! | `app_state` | **el bosque** | la cabeza viva (`message_tree.head/<id>`) |
//! | `sessions` | el store de sesiones | la fila de la sesión (`current_epoch`) |
//! | `messages` | el store de sesiones | los mensajes (con el `Value` ACP exacto) |
//!
//! [`guardar_bosque`] es el escritor **de la topología** (lo usa el store de
//! sesiones ACP, que ya es el dueño de `sessions` y de `messages`);
//! [`guardar`] escribe además esas dos, y es el escritor completo del módulo.

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Tabla medida donde vive la topología del bosque.
pub const TABLA_NODOS: &str = "message_nodes";
/// Tabla medida donde vive el registro de mensajes por época.
pub const TABLA_MENSAJES: &str = "messages";
/// Tabla medida donde vive la época viva de la sesión.
pub const TABLA_SESIONES: &str = "sessions";
/// Prefijo de la clave de `app_state` (tabla medida) con la cabeza viva.
pub const PREFIJO_CABEZA: &str = "message_tree.head/";

// ── El modelo puro ────────────────────────────────────────────────────────

/// Un mensaje del historial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mensaje {
    /// Número de secuencia **global**: nunca se reinicia (igual que en
    /// `compaction_epoch.rs`).
    pub seq: u64,
    /// La época a la que pertenece este mensaje.
    pub epoch: u32,
    pub rol: String,
    pub contenido: String,
}

/// Un nodo del bosque: `{id, padre, secuencia, epoch}`.
///
/// El **historial** va materializado dentro del nodo: por eso el `fork` puede
/// copiarlo y el padre **no** cambia (ni un mensaje).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nodo {
    /// Id del nodo dentro del bosque de la sesión (`message_nodes.node_id`).
    pub id: i64,
    /// El padre (`message_nodes.parent_node_id`); `None` en una raíz.
    pub padre: Option<i64>,
    /// Secuencia del último mensaje del historial.
    pub secuencia: u64,
    /// La época en la que nació el nodo.
    pub epoch: u32,
    /// El mensaje **propio** del nodo (`None` en un punto de retorno / fork).
    pub mensaje: Option<Mensaje>,
    /// El historial **materializado** (copiado en el fork).
    pub historial: Vec<Mensaje>,
    /// Cuándo se creó (`message_nodes.created_at`).
    pub creado_en: i64,
}

impl Nodo {
    /// ¿Es un punto de retorno (un fork, sin mensaje propio)?
    pub fn es_fork(&self) -> bool {
        self.mensaje.is_none()
    }
}

/// Los datos de la sesión que la tabla `sessions` (medida) guarda y que este
/// módulo no decide (el directorio de trabajo, el backend, el modelo...).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatosSesion {
    pub working_directory: String,
    pub backend_type: String,
    pub model: String,
    pub permission_mode: String,
    pub total_tokens: i64,
}

impl Default for DatosSesion {
    fn default() -> Self {
        DatosSesion {
            working_directory: String::new(),
            backend_type: "megadaemon".to_string(),
            model: String::new(),
            permission_mode: "default".to_string(),
            total_tokens: 0,
        }
    }
}

/// El estado del bosque.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Estado {
    pub session_id: String,
    /// Los nodos, por id.
    pub nodos: BTreeMap<i64, Nodo>,
    /// El nodo vivo (la cabeza de la rama en la que se está trabajando).
    pub cabeza: Option<i64>,
    /// La época viva (`sessions.current_epoch`).
    pub current_epoch: u32,
    /// El siguiente id de nodo.
    pub siguiente_id: i64,
    /// El siguiente número de secuencia **global**.
    pub siguiente_seq: u64,
    /// El objetivo de un revert por pasos (lo fija [`Estado::preparar_revert`]).
    pub revert_objetivo: Option<i64>,
    /// Los datos de la sesión (tabla `sessions`).
    pub sesion: DatosSesion,
}

/// Alias: el estado **es** el bosque.
pub type Bosque = Estado;

/// Un punto de retorno posible de un revert por pasos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasoRevert {
    /// Posición en la lista de pasos (0 = el más cercano a la cabeza).
    pub indice: usize,
    /// El nodo al que se puede volver.
    pub nodo_id: i64,
    /// Cómo se le describe al usuario (NO MEDIDO: formato nuestro).
    pub etiqueta: String,
    /// Cuántos mensajes se deshacen al elegir este paso.
    pub distancia: usize,
    /// La época del nodo.
    pub epoch: u32,
}

impl Estado {
    /// Un bosque nuevo (vacío) para una sesión.
    pub fn nuevo(session_id: impl Into<String>) -> Self {
        Estado {
            session_id: session_id.into(),
            nodos: BTreeMap::new(),
            cabeza: None,
            current_epoch: 0,
            siguiente_id: 1,
            siguiente_seq: 1,
            revert_objetivo: None,
            sesion: DatosSesion::default(),
        }
    }

    // ── Consultas (puras) ────────────────────────────────────────────────

    /// Las **raíces** del bosque: los nodos sin padre.
    ///
    /// Con más de una raíz el árbol es un **bosque**, no una lista.
    pub fn raices(&self) -> Vec<i64> {
        let mut v: Vec<i64> = self
            .nodos
            .values()
            .filter(|n| n.padre.is_none())
            .map(|n| n.id)
            .collect();
        v.sort_unstable();
        v
    }

    /// ¿Es un bosque de verdad (más de una raíz)?
    pub fn es_bosque(&self) -> bool {
        self.raices().len() > 1
    }

    /// Los hijos de un nodo, en orden.
    pub fn hijos(&self, nodo_id: i64) -> Vec<i64> {
        let mut v: Vec<i64> = self
            .nodos
            .values()
            .filter(|n| n.padre == Some(nodo_id))
            .map(|n| n.id)
            .collect();
        v.sort_unstable();
        v
    }

    /// El historial de un nodo (su copia materializada).
    pub fn historial_de(&self, nodo_id: i64) -> Vec<Mensaje> {
        self.nodos
            .get(&nodo_id)
            .map(|n| n.historial.clone())
            .unwrap_or_default()
    }

    /// Lo que el agente ve ahora: el historial de la cabeza.
    pub fn vista_viva(&self) -> Vec<Mensaje> {
        match self.cabeza {
            Some(c) => self.historial_de(c),
            None => Vec::new(),
        }
    }

    /// **Todos** los mensajes del bosque (los que existen en `messages`),
    /// sin repetir: se deduplican por `(epoch, seq)`.
    ///
    /// Éste es el "historial completo" del criterio 2: tras un revert sigue
    /// consultable **todo**, porque el revert no borra nada.
    pub fn todos_los_mensajes(&self) -> Vec<Mensaje> {
        let mut vistos: BTreeSet<(u32, u64)> = BTreeSet::new();
        let mut out: Vec<Mensaje> = Vec::new();
        for nodo in self.nodos.values() {
            for m in &nodo.historial {
                if vistos.insert((m.epoch, m.seq)) {
                    out.push(m.clone());
                }
            }
        }
        out.sort_by_key(|m| (m.epoch, m.seq));
        out
    }

    /// Los mensajes de una época concreta.
    pub fn mensajes_de_epoch(&self, epoch: u32) -> Vec<Mensaje> {
        self.todos_los_mensajes()
            .into_iter()
            .filter(|m| m.epoch == epoch)
            .collect()
    }

    /// El camino desde una raíz hasta un nodo (incluidos los dos).
    pub fn ruta(&self, nodo_id: i64) -> Vec<i64> {
        let mut r = Vec::new();
        let mut cur = Some(nodo_id);
        let mut guard = 0usize;
        while let Some(id) = cur {
            r.push(id);
            guard += 1;
            if guard > self.nodos.len() + 1 {
                break; // ciclo defensivo
            }
            cur = self.nodos.get(&id).and_then(|n| n.padre);
        }
        r.reverse();
        r
    }

    /// ¿Es `ancestro` un antepasado de `nodo` (o el mismo)?
    pub fn es_ancestro(&self, ancestro: i64, nodo: i64) -> bool {
        let mut cur = Some(nodo);
        let mut guard = 0usize;
        while let Some(id) = cur {
            if id == ancestro {
                return true;
            }
            guard += 1;
            if guard > self.nodos.len() + 1 {
                return false;
            }
            cur = self.nodos.get(&id).and_then(|n| n.padre);
        }
        false
    }

    // ── Operaciones (puras) ───────────────────────────────────────────────

    /// Añade un mensaje a la rama viva: crea un **hijo** de la cabeza.
    ///
    /// El padre **no** se muta (por eso su historial no cambia nunca).
    /// Devuelve el id del nodo nuevo.
    pub fn anadir_mensaje(
        &mut self,
        rol: impl Into<String>,
        contenido: impl Into<String>,
    ) -> Result<i64, String> {
        self.anadir_mensaje_en(rol, contenido, ahora())
    }

    /// Igual que [`Estado::anadir_mensaje`] pero con la marca de tiempo
    /// explícita (para tests deterministas).
    pub fn anadir_mensaje_en(
        &mut self,
        rol: impl Into<String>,
        contenido: impl Into<String>,
        creado_en: i64,
    ) -> Result<i64, String> {
        let padre = self.cabeza;
        let base = match padre {
            Some(p) => self
                .nodos
                .get(&p)
                .ok_or_else(|| format!("el nodo vivo {} no existe", p))?
                .historial
                .clone(),
            None => Vec::new(),
        };
        let seq = self.siguiente_seq;
        let mensaje = Mensaje {
            seq,
            epoch: self.current_epoch,
            rol: rol.into(),
            contenido: contenido.into(),
        };
        let mut historial = base;
        historial.push(mensaje.clone());

        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.siguiente_seq += 1;
        self.nodos.insert(
            id,
            Nodo {
                id,
                padre,
                secuencia: seq,
                epoch: self.current_epoch,
                mensaje: Some(mensaje),
                historial,
                creado_en,
            },
        );
        self.cabeza = Some(id);
        Ok(id)
    }

    /// Añade un mensaje con su **época** y su **`seq`** **explícitos**.
    ///
    /// Para **reconstruir** un bosque desde un historial ya persistido: el
    /// `seq` es el `sequence_number` **medido** de `messages` y la época es
    /// su `compaction_epoch` ⇒ el bosque reconstruido tiene exactamente las
    /// mismas claves que las filas de `messages` (y por tanto el mismo
    /// `UNIQUE(session_id, compaction_epoch, sequence_number)`).
    ///
    /// El padre es la **cabeza viva** (igual que [`Estado::anadir_mensaje`]):
    /// el mensaje se cuelga del nodo vivo.
    pub fn anadir_mensaje_completo(
        &mut self,
        rol: impl Into<String>,
        contenido: impl Into<String>,
        epoch: u32,
        seq: u64,
        creado_en: i64,
    ) -> Result<i64, String> {
        let padre = self.cabeza;
        let base = match padre {
            Some(p) => self
                .nodos
                .get(&p)
                .ok_or_else(|| format!("el nodo vivo {} no existe", p))?
                .historial
                .clone(),
            None => Vec::new(),
        };
        let mensaje = Mensaje {
            seq,
            epoch,
            rol: rol.into(),
            contenido: contenido.into(),
        };
        let mut historial = base;
        historial.push(mensaje.clone());

        let id = self.siguiente_id;
        self.siguiente_id += 1;
        // El `seq` es global: nunca se reinicia.
        if seq.saturating_add(1) > self.siguiente_seq {
            self.siguiente_seq = seq.saturating_add(1);
        }
        // La época viva es la de la rama viva.
        if epoch > self.current_epoch {
            self.current_epoch = epoch;
        }
        self.nodos.insert(
            id,
            Nodo {
                id,
                padre,
                secuencia: seq,
                epoch,
                mensaje: Some(mensaje),
                historial,
                creado_en,
            },
        );
        self.cabeza = Some(id);
        Ok(id)
    }

    /// Crea una **raíz nueva**: una cadena independiente dentro de la misma
    /// sesión.  Es lo que hace que el árbol sea un **bosque** y no una lista.
    pub fn nueva_raiz(
        &mut self,
        rol: impl Into<String>,
        contenido: impl Into<String>,
    ) -> Result<i64, String> {
        let id = self.siguiente_id;
        self.siguiente_id += 1;
        let seq = self.siguiente_seq;
        self.siguiente_seq += 1;
        let mensaje = Mensaje {
            seq,
            epoch: self.current_epoch,
            rol: rol.into(),
            contenido: contenido.into(),
        };
        self.nodos.insert(
            id,
            Nodo {
                id,
                padre: None,
                secuencia: seq,
                epoch: self.current_epoch,
                mensaje: Some(mensaje.clone()),
                historial: vec![mensaje],
                creado_en: ahora(),
            },
        );
        self.cabeza = Some(id);
        Ok(id)
    }

    /// **Bifurca**: crea un hijo de `nodo_id` con el **historial copiado**.
    ///
    /// * El padre **no cambia** (ni un mensaje): es el criterio 1.
    /// * El hijo abre una **época nueva** (obligatorio: el `UNIQUE` medido de
    ///   `messages` no dejaría convivir la copia con el original en la misma
    ///   época ⇒ ver el doc-comment del módulo).
    /// * La cabeza pasa al hijo (la rama nueva es la que sigue viva).
    pub fn fork(&mut self, nodo_id: i64) -> Result<i64, String> {
        let padre = self
            .nodos
            .get(&nodo_id)
            .ok_or_else(|| format!("no existe el nodo {}", nodo_id))?;
        let nueva_epoch = self.current_epoch + 1;
        // La COPIA: los mensajes se replican en la época nueva.
        let historial: Vec<Mensaje> = padre
            .historial
            .iter()
            .map(|m| Mensaje {
                epoch: nueva_epoch,
                ..m.clone()
            })
            .collect();
        let secuencia = historial.last().map(|m| m.seq).unwrap_or(0);

        let id = self.siguiente_id;
        self.siguiente_id += 1;
        self.nodos.insert(
            id,
            Nodo {
                id,
                padre: Some(nodo_id),
                secuencia,
                epoch: nueva_epoch,
                mensaje: None,
                historial,
                creado_en: ahora(),
            },
        );
        self.current_epoch = nueva_epoch;
        self.cabeza = Some(id);
        self.revert_objetivo = None;
        Ok(id)
    }

    /// **Copia** el bosque para otra sesión (el *fork*).
    ///
    /// La topología y los mensajes se copian tal cual —el historial se
    /// copia y el padre **no** cambia ni un mensaje (criterio 1 del patrón
    /// 3)— y lo único que cambia es la sesión a la que pertenecen los
    /// nodos: las filas van a `message_nodes` con el `session_id` nuevo.
    ///
    /// NO MEDIDO: la referencia no dice si el fork **del bosque** copia la
    /// topología o sólo los mensajes.  Aquí se copia la topología entera
    /// porque es lo que hace el fork del patrón 3 (`fork` = hijo con el
    /// historial copiado) y porque el `parent_node_id` medido es **dentro**
    /// de la sesión (`UNIQUE(session_id, node_id)`).
    pub fn copiar_para(&self, session_id: impl Into<String>) -> Estado {
        let mut copia = self.clone();
        copia.session_id = session_id.into();
        // El objetivo de revert no se hereda: es de otra rama.
        copia.revert_objetivo = None;
        copia
    }

    /// **Revierte** a un nodo: la cabeza pasa a ese nodo **sin borrar nada**.
    ///
    /// Esto es el "revert a un nodo": mueve el punto vivo dentro del bosque.
    /// (El "revert **por pasos**" es [`Estado::aplicar_paso`], que además
    /// **bifurca** desde el punto elegido.)
    pub fn revertir_a(&mut self, nodo_id: i64) -> Result<(), String> {
        let nodo = self
            .nodos
            .get(&nodo_id)
            .ok_or_else(|| format!("no existe el nodo {}", nodo_id))?;
        self.cabeza = Some(nodo_id);
        // La época viva es la del nodo al que se vuelve (como
        // `compaction_epoch.rs::revertir_a`): el historial sigue entero.
        self.current_epoch = nodo.epoch;
        self.revert_objetivo = None;
        Ok(())
    }

    /// Prepara un **revert por pasos**: dice cuál es el objetivo (el nodo al
    /// que se quiere volver) para que [`pasos_revert`] pueda listar los puntos
    /// de retorno y [`Estado::aplicar_paso`] pueda ejecutar el elegido.
    pub fn preparar_revert(&mut self, objetivo: i64) -> Result<(), String> {
        let cabeza = self.cabeza.ok_or("no hay ningún nodo vivo")?;
        if !self.nodos.contains_key(&objetivo) {
            return Err(format!("no existe el nodo {}", objetivo));
        }
        if !self.es_ancestro(objetivo, cabeza) {
            return Err(format!(
                "el nodo {} no está en el camino del nodo vivo {}",
                objetivo, cabeza
            ));
        }
        self.revert_objetivo = Some(objetivo);
        Ok(())
    }
}

/// Los **pasos** de un revert por pasos: los puntos de retorno posibles entre
/// el nodo vivo y el `objetivo`, con su etiqueta y su distancia.
///
/// * Se listan **del más cercano al más lejano** (distancia 1, 2, 3...): así
///   "deshacer 1 paso" es el primero.
/// * Si el `objetivo` no está en el camino del nodo vivo, **no hay pasos**.
///
/// Es una función **pura**: no toca disco ni el estado.
pub fn pasos_revert(estado: &Estado, objetivo: i64) -> Vec<PasoRevert> {
    let Some(cabeza) = estado.cabeza else {
        return Vec::new();
    };
    if !estado.nodos.contains_key(&objetivo) {
        return Vec::new();
    }
    // El camino del objetivo hasta la cabeza (ambos incluidos).
    let Some(camino) = camino_entre(estado, objetivo, cabeza) else {
        return Vec::new();
    };
    if camino.len() < 2 {
        return Vec::new();
    }
    // Los pasos son los nodos del camino EXCLUYENDO la cabeza (que es donde
    // ya estamos) e INCLUYENDO el objetivo.
    let candidatos: Vec<i64> = camino[..camino.len() - 1].to_vec();
    let total = estado.vista_viva().len();

    let mut pasos: Vec<PasoRevert> = candidatos
        .iter()
        .map(|id| {
            let nodo = &estado.nodos[id];
            PasoRevert {
                indice: 0,
                nodo_id: *id,
                etiqueta: etiqueta_de(nodo),
                distancia: total.saturating_sub(nodo.historial.len()),
                epoch: nodo.epoch,
            }
        })
        .collect();
    // Del más cercano (el último del camino) al más lejano (el objetivo).
    pasos.reverse();
    for (i, p) in pasos.iter_mut().enumerate() {
        p.indice = i;
    }
    pasos
}

/// **Ejecuta** el paso de revert elegido.
///
/// A diferencia de [`Estado::revertir_a`], esto **bifurca** desde el punto
/// elegido (como el `forkFromStep` de la referencia) y mueve la cabeza allí:
/// el paso elegido se convierte en un punto de retorno nuevo y **la rama
/// original sigue intacta** (nada se borra).
///
/// Devuelve el estado nuevo; el original no se toca.
pub fn aplicar_paso(estado: &Estado, indice_paso: usize) -> Result<Estado, String> {
    let objetivo = estado
        .revert_objetivo
        .ok_or("no hay ningún objetivo de revert preparado (llama antes a preparar_revert)")?;
    let pasos = pasos_revert(estado, objetivo);
    let paso = pasos.get(indice_paso).ok_or_else(|| {
        format!(
            "el paso {} no existe (hay {} pasos)",
            indice_paso,
            pasos.len()
        )
    })?;
    let mut nuevo = estado.clone();
    nuevo.fork(paso.nodo_id)?;
    nuevo.revert_objetivo = None;
    Ok(nuevo)
}

/// El camino entre dos nodos (de `desde` hasta `hasta`, ambos incluidos), o
/// `None` si `hasta` no es un descendiente de `desde`.
fn camino_entre(estado: &Estado, desde: i64, hasta: i64) -> Option<Vec<i64>> {
    let mut r: Vec<i64> = Vec::new();
    let mut cur = Some(hasta);
    let mut guard = 0usize;
    while let Some(id) = cur {
        r.push(id);
        if id == desde {
            r.reverse();
            return Some(r);
        }
        guard += 1;
        if guard > estado.nodos.len() + 1 {
            return None;
        }
        cur = estado.nodos.get(&id).and_then(|n| n.padre);
    }
    None
}

/// La etiqueta de un nodo (formato nuestro, NO MEDIDO).
fn etiqueta_de(nodo: &Nodo) -> String {
    match &nodo.mensaje {
        Some(m) if !m.rol.is_empty() => {
            let contenido: String = m.contenido.chars().take(60).collect();
            format!("{}: {}", m.rol, contenido)
        }
        _ => "punto de retorno (fork)".to_string(),
    }
}

/// El reloj (segundos).
fn ahora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ── Persistencia (SQLite) ─────────────────────────────────────────────────

/// Asegura el esquema medido (idempotente) antes de tocar la base.
pub fn asegurar_esquema(conn: &Connection) -> Result<(), rusqlite::Error> {
    crate::board_schema::aplicar(conn).map(|_| ())
}

/// Guarda el bosque en SQLite (tablas **medidas** del patrón 10).
///
/// * `message_nodes`: la topología (un nodo = una fila).
/// * `messages`: el registro de mensajes por época (`INSERT OR IGNORE`: la
///   copia del fork vive en su propia época y **nunca** pisa la del padre).
/// * `sessions`: la época viva (`current_epoch`).
/// * `app_state`: la cabeza viva.
pub fn guardar(conn: &Connection, estado: &Estado) -> Result<(), rusqlite::Error> {
    asegurar_esquema(conn)?;

    // La sesión (tabla medida).
    conn.execute(
        "INSERT INTO sessions(id, working_directory, backend_type, model, permission_mode,
                              created_at, last_activity_at, total_tokens, current_epoch, hidden)
         VALUES(?1,?2,?3,?4,?5,?6,?6,?7,?8,0)
         ON CONFLICT(id) DO UPDATE SET
            working_directory=excluded.working_directory,
            backend_type=excluded.backend_type,
            model=excluded.model,
            permission_mode=excluded.permission_mode,
            last_activity_at=excluded.last_activity_at,
            total_tokens=excluded.total_tokens,
            current_epoch=excluded.current_epoch",
        params![
            estado.session_id,
            estado.sesion.working_directory,
            estado.sesion.backend_type,
            estado.sesion.model,
            estado.sesion.permission_mode,
            ahora(),
            estado.sesion.total_tokens,
            estado.current_epoch as i64
        ],
    )?;

    // El registro de mensajes, por época.  `INSERT OR IGNORE` porque el
    // UNIQUE medido es (session_id, compaction_epoch, sequence_number) y la
    // copia del fork tiene los MISMOS seq que el padre (en otra época).
    for nodo in estado.nodos.values() {
        for m in &nodo.historial {
            conn.execute(
                "INSERT OR IGNORE INTO messages(session_id, sequence_number, role, content,
                                                tool_call_id, tool_calls_json, timestamp,
                                                compaction_epoch)
                 VALUES(?1,?2,?3,?4,NULL,NULL,?5,?6)",
                params![
                    estado.session_id,
                    m.seq as i64,
                    m.rol,
                    m.contenido,
                    nodo.creado_en,
                    m.epoch as i64
                ],
            )?;
        }
    }

    // La topología del bosque y la cabeza viva.
    guardar_bosque(conn, estado)?;

    Ok(())
}

/// Guarda **sólo la topología** del bosque: la tabla medida `message_nodes`
/// y la cabeza viva en `app_state`.
///
/// MEDIDO (patrón 10): el DDL medido no tiene ninguna columna para la
/// cabeza ⇒ la cabeza vive en `app_state` (la tabla medida de clave/valor)
/// con la clave `message_tree.head/<session_id>`.  **No se le inventan
/// columnas al esquema medido**.
///
/// POR QUÉ NO ESCRIBE `sessions` NI `messages`
/// ------------------------------------------
/// Esas filas son **de otro dueño**: el store de sesiones ACP
/// (`acp_sesion_store`) escribe la fila de `sessions` (con `created_at`,
/// `last_activity_at` y `total_tokens`) y los mensajes (con el `Value` ACP
/// exacto en `tool_calls_json`).  Si el bosque los sobrescribiera:
///
/// * se perderían `total_tokens` y `last_activity_at` (el bosque no los
///   conoce: su [`DatosSesion`] va por defecto), y
/// * los mensajes del bosque (sin `tool_calls_json`) chocarían con el
///   `UNIQUE(session_id, compaction_epoch, sequence_number)` **medido** ⇒
///   el guardado del mensaje fallaría.
///
/// ⇒ El bosque escribe **su** tabla (`message_nodes`) y **su** clave
/// (`app_state`); el contenido de los mensajes es del store de sesiones.
///
/// Es **idempotente**: un `upsert` por nodo ⇒ guardar el mismo bosque dos
/// veces no duplica ni corrompe nada.
pub fn guardar_bosque(conn: &Connection, estado: &Estado) -> Result<(), rusqlite::Error> {
    asegurar_esquema(conn)?;

    // Los nodos (la topología del bosque).
    for nodo in estado.nodos.values() {
        conn.execute(
            "INSERT INTO message_nodes(session_id, node_id, parent_node_id, chat_message, created_at)
             VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(session_id, node_id) DO UPDATE SET
                parent_node_id=excluded.parent_node_id,
                chat_message=excluded.chat_message,
                created_at=excluded.created_at",
            params![
                estado.session_id,
                nodo.id,
                nodo.padre,
                nodo_a_json(nodo).to_string(),
                nodo.creado_en
            ],
        )?;
    }

    // La cabeza viva, en `app_state` (tabla medida).
    let clave = format!("{}{}", PREFIJO_CABEZA, estado.session_id);
    let cabeza = estado.cabeza.map(|c| c.to_string()).unwrap_or_default();
    conn.execute(
        "INSERT INTO app_state(key,value) VALUES(?1,?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![clave, cabeza],
    )?;

    Ok(())
}

/// Carga el bosque de una sesión desde SQLite.
///
/// Reconstruye el historial de cada nodo recorriendo la cadena de padres
/// (invariante: el historial de un nodo = los mensajes de su camino desde la
/// raíz).
pub fn cargar(conn: &Connection, session_id: &str) -> Result<Estado, rusqlite::Error> {
    let mut estado = Estado::nuevo(session_id);

    // La sesión.
    let sesion: Option<(String, String, String, String, i64, i64)> = conn
        .query_row(
            "SELECT working_directory, backend_type, model, permission_mode, total_tokens, current_epoch
             FROM sessions WHERE id=?1",
            params![session_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .optional()?;
    let mut hay_sesion = false;
    if let Some((wd, bt, modelo, pm, tokens, epoch)) = sesion {
        estado.sesion = DatosSesion {
            working_directory: wd,
            backend_type: bt,
            model: modelo,
            permission_mode: pm,
            total_tokens: tokens,
        };
        estado.current_epoch = epoch.max(0) as u32;
        hay_sesion = true;
    }

    // Los nodos.
    let mut crudos: Vec<(i64, Option<i64>, String, i64)> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT node_id, parent_node_id, chat_message, created_at
             FROM message_nodes WHERE session_id=?1 ORDER BY node_id",
        )?;
        let filas = stmt.query_map(params![session_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?;
        for f in filas {
            crudos.push(f?);
        }
    }

    for (node_id, padre, chat, creado_en) in crudos {
        let v: Value = serde_json::from_str(&chat).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let epoch = v.get("epoch").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
        let seq = v.get("seq").and_then(|x| x.as_u64()).unwrap_or(0);
        let rol = v
            .get("role")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let contenido = v
            .get("content")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let es_fork = v.get("fork").and_then(|x| x.as_bool()).unwrap_or(false);
        let mensaje = if es_fork {
            None
        } else {
            Some(Mensaje {
                seq,
                epoch,
                rol,
                contenido,
            })
        };
        estado.nodos.insert(
            node_id,
            Nodo {
                id: node_id,
                padre,
                secuencia: seq,
                epoch,
                mensaje,
                historial: Vec::new(),
                creado_en,
            },
        );
        if node_id >= estado.siguiente_id {
            estado.siguiente_id = node_id + 1;
        }
    }

    // Reconstruir el historial de cada nodo (de abajo arriba).
    let mut ids: Vec<i64> = estado.nodos.keys().copied().collect();
    ids.sort_unstable();
    let mut resueltos: BTreeSet<i64> = BTreeSet::new();
    let mut progreso = true;
    while progreso {
        progreso = false;
        for id in ids.iter() {
            if resueltos.contains(id) {
                continue;
            }
            let (padre, mensaje, epoch) = match estado.nodos.get(id) {
                Some(n) => (n.padre, n.mensaje.clone(), n.epoch),
                None => continue,
            };
            let base = match padre {
                Some(p) => {
                    if !resueltos.contains(&p) {
                        continue; // el padre todavía no está resuelto
                    }
                    estado
                        .nodos
                        .get(&p)
                        .map(|n| n.historial.clone())
                        .unwrap_or_default()
                }
                None => Vec::new(),
            };
            let mut historial = base;
            // Los mensajes heredados **adoptan la época de este nodo**: es lo
            // que hace el fork (la copia vive en la época nueva).  Así el
            // historial reconstruido es EXACTAMENTE el que había en memoria.
            for m in historial.iter_mut() {
                m.epoch = epoch;
            }
            if let Some(m) = mensaje {
                historial.push(m);
            }
            if let Some(n) = estado.nodos.get_mut(id) {
                n.historial = historial;
                n.epoch = epoch;
            }
            resueltos.insert(*id);
            progreso = true;
        }
    }

    // La secuencia global siguiente.
    let max_seq = estado
        .nodos
        .values()
        .flat_map(|n| n.historial.iter())
        .map(|m| m.seq)
        .max()
        .unwrap_or(0);
    estado.siguiente_seq = max_seq + 1;

    // La cabeza viva.
    let clave = format!("{}{}", PREFIJO_CABEZA, session_id);
    let cabeza: Option<String> = conn
        .query_row(
            "SELECT value FROM app_state WHERE key=?1",
            params![clave],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(c) = cabeza {
        if let Ok(id) = c.parse::<i64>() {
            if estado.nodos.contains_key(&id) {
                estado.cabeza = Some(id);
            }
        }
    }
    if estado.cabeza.is_none() {
        // Sin cabeza guardada: la hoja más reciente.
        estado.cabeza = ids.last().copied();
    }
    if !hay_sesion {
        // Sin fila de sesión, la época viva es la del nodo vivo.
        if let Some(c) = estado.cabeza {
            if let Some(n) = estado.nodos.get(&c) {
                estado.current_epoch = n.epoch;
            }
        }
    }

    Ok(estado)
}

/// El JSON de un nodo (nuestro formato; NO MEDIDO).
fn nodo_a_json(nodo: &Nodo) -> Value {
    json!({
        "seq": nodo.secuencia,
        "epoch": nodo.epoch,
        "fork": nodo.mensaje.is_none(),
        "role": nodo.mensaje.as_ref().map(|m| m.rol.clone()).unwrap_or_default(),
        "content": nodo.mensaje.as_ref().map(|m| m.contenido.clone()).unwrap_or_default(),
    })
}

// ── TESTS ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn bd() -> Connection {
        Connection::open_in_memory().expect("bd en memoria")
    }

    /// Una conversación de 3 mensajes (usuario, asistente, usuario).
    fn conversacion() -> Estado {
        let mut e = Estado::nuevo("s1");
        e.anadir_mensaje_en("user", "hola", 1).unwrap();
        e.anadir_mensaje_en("assistant", "buenas", 2).unwrap();
        e.anadir_mensaje_en("user", "mas cosas", 3).unwrap();
        e
    }

    #[test]
    fn una_conversacion_vacia_no_tiene_nodos() {
        let e = Estado::nuevo("s0");
        assert!(e.nodos.is_empty());
        assert!(e.cabeza.is_none());
        assert!(e.raices().is_empty());
        assert!(e.vista_viva().is_empty());
        assert_eq!(e.current_epoch, 0);
    }

    #[test]
    fn anadir_mensajes_crea_la_cadena_con_sus_padres() {
        let e = conversacion();
        assert_eq!(e.nodos.len(), 3);
        let raices = e.raices();
        assert_eq!(raices.len(), 1, "una cadena normal tiene una raíz");
        let raiz = raices[0];
        assert_eq!(e.nodos[&raiz].padre, None);
        let hijos = e.hijos(raiz);
        assert_eq!(hijos.len(), 1);
        assert_eq!(e.nodos[&hijos[0]].padre, Some(raiz));
        // El historial de la cabeza tiene los 3 mensajes.
        assert_eq!(e.vista_viva().len(), 3);
        // El nodo raíz sólo tiene su propio mensaje.
        assert_eq!(e.historial_de(raiz).len(), 1);
    }

    #[test]
    fn tras_un_fork_el_padre_no_cambia_ni_un_mensaje() {
        // Criterio de aceptación 1.
        let mut e = conversacion();
        let padre = e.cabeza.unwrap();
        let antes_padre = e.nodos[&padre].clone();
        let antes_whole = e.nodos.clone();

        let hijo = e.fork(padre).expect("fork");

        // El padre: idéntico, mensaje a mensaje.
        assert_eq!(e.nodos[&padre], antes_padre, "el padre cambió tras el fork");
        assert_eq!(
            e.nodos[&padre].historial, antes_padre.historial,
            "el padre perdió o ganó mensajes"
        );
        // Y ningún otro nodo cambió tampoco.
        for (id, nodo) in antes_whole.iter() {
            assert_eq!(&e.nodos[id], nodo, "el nodo {} cambió", id);
        }
        // El hijo tiene el historial COPIADO (mismo contenido, otra época).
        assert_eq!(e.nodos[&hijo].padre, Some(padre));
        assert_eq!(e.nodos[&hijo].historial.len(), 3);
        assert_eq!(e.nodos[&hijo].secuencia, antes_padre.secuencia);
        assert_eq!(e.nodos[&hijo].epoch, 1, "el fork abre una época nueva");
        assert!(e.nodos[&hijo].es_fork());
        // Los mensajes copiados tienen el mismo seq (global) pero otra época.
        for (m, c) in e.nodos[&hijo]
            .historial
            .iter()
            .zip(antes_padre.historial.iter())
        {
            assert_eq!(m.seq, c.seq);
            assert_eq!(m.contenido, c.contenido);
            assert_eq!(m.epoch, 1);
            assert_eq!(c.epoch, 0);
        }
    }

    #[test]
    fn el_fork_no_altera_el_historial_copiado_del_padre_despues() {
        // La copia es una COPIA: seguir en el padre no la cambia.
        let mut e = conversacion();
        let padre = e.cabeza.unwrap();
        let hijo = e.fork(padre).unwrap();
        let copia = e.nodos[&hijo].historial.clone();
        // El padre sigue vivo y añade un mensaje más.
        e.anadir_mensaje_en("user", "y ahora otra cosa", 4).unwrap();
        assert_eq!(
            e.nodos[&hijo].historial, copia,
            "la copia del fork cambió al seguir en el padre"
        );
        assert_eq!(e.nodos[&hijo].historial.len(), 3);
        assert_eq!(e.nodos[&e.cabeza.unwrap()].historial.len(), 4);
    }

    #[test]
    fn tras_un_revert_el_historial_completo_sigue_consultable() {
        // Criterio de aceptación 2: el revert NO borra.
        let mut e = conversacion();
        let total_antes = e.todos_los_mensajes().len();
        let mensajes_antes = e.todos_los_mensajes();
        let raiz = e.raices()[0];

        e.revertir_a(raiz).expect("revert");

        // La cabeza es el nodo al que se ha vuelto...
        assert_eq!(e.cabeza, Some(raiz));
        // ...pero NADA se ha borrado: el historial completo sigue ahí.
        let despues = e.todos_los_mensajes();
        assert_eq!(despues.len(), total_antes, "el revert borró mensajes");
        for m in mensajes_antes.iter() {
            assert!(
                despues.contains(m),
                "desapareció el mensaje {}: {}",
                m.seq,
                m.contenido
            );
        }
        // Los nodos siguen todos.
        assert_eq!(e.nodos.len(), 3);
    }

    #[test]
    fn dos_raices_constituyen_un_bosque() {
        // Criterio de aceptación 3: el árbol es un bosque, no una lista.
        let mut e = conversacion();
        assert!(!e.es_bosque(), "una sola cadena no es un bosque");
        let r2 = e.nueva_raiz("user", "otra conversación").unwrap();
        let raices = e.raices();
        assert_eq!(raices.len(), 2);
        assert!(e.es_bosque());
        assert_eq!(e.nodos[&r2].padre, None);
        // Son ramas independientes.
        e.anadir_mensaje_en("assistant", "respuesta aparte", 5)
            .unwrap();
        assert_eq!(e.vista_viva().len(), 2);
        // La otra rama no se ha tocado.
        assert_eq!(e.historial_de(raices[0]).len(), 1);
    }

    #[test]
    fn el_revert_por_pasos_lista_los_puntos_de_retorno() {
        let mut e = conversacion();
        let raiz = e.raices()[0];
        e.preparar_revert(raiz).expect("preparar");

        let pasos = pasos_revert(&e, raiz);
        // La cabeza es el 3º mensaje; los pasos son los 2 anteriores + la raíz.
        assert_eq!(pasos.len(), 2, "pasos: {:?}", pasos);
        // Del más cercano al más lejano.
        assert_eq!(pasos[0].distancia, 1);
        assert_eq!(pasos[1].distancia, 2);
        assert_eq!(pasos[1].nodo_id, raiz);
        // Con etiqueta.
        assert!(pasos[0].etiqueta.contains("buenas"));
        assert_eq!(pasos[0].indice, 0);
        assert_eq!(pasos[1].indice, 1);
        // Y son puros: el estado no cambia.
        assert_eq!(e.cabeza, e.nodos.keys().last().copied());
    }

    #[test]
    fn el_revert_por_pasos_ejecuta_el_paso_elegido() {
        let mut e = conversacion();
        let raiz = e.raices()[0];
        e.preparar_revert(raiz).expect("preparar");
        let pasos = pasos_revert(&e, raiz);
        let elegido = pasos[0].nodo_id;

        let nuevo = aplicar_paso(&e, 0).expect("aplicar el paso");

        // La cabeza está en el paso elegido (que ahora es una bifurcación).
        let nuevo_head = nuevo.cabeza.unwrap();
        assert_eq!(nuevo.nodos[&nuevo_head].padre, Some(elegido));
        // El original sigue intacto: nada se ha borrado.
        assert_eq!(nuevo.nodos.len(), e.nodos.len() + 1);
        for m in e.todos_los_mensajes().iter() {
            assert!(
                nuevo.todos_los_mensajes().contains(m),
                "desapareció el mensaje {}",
                m.seq
            );
        }
        // Y el paso abre su propia época (como un fork).
        assert!(nuevo.current_epoch > e.current_epoch);
    }

    #[test]
    fn un_paso_que_no_existe_da_error() {
        let e = conversacion();
        // Sin objetivo preparado.
        let err = aplicar_paso(&e, 0).unwrap_err();
        assert!(err.contains("objetivo"), "error: {}", err);

        let mut e2 = conversacion();
        let raiz = e2.raices()[0];
        e2.preparar_revert(raiz).unwrap();
        let err = aplicar_paso(&e2, 99).unwrap_err();
        assert!(err.contains("no existe"), "error: {}", err);
    }

    #[test]
    fn un_objetivo_que_no_es_ancestro_no_tiene_pasos() {
        let mut e = conversacion();
        let raiz = e.raices()[0];
        e.nueva_raiz("user", "otra rama").unwrap();
        // La raíz NO está en el camino de la cabeza (que está en la otra rama).
        assert!(pasos_revert(&e, raiz).is_empty());
        assert!(e.preparar_revert(raiz).is_err());
    }

    #[test]
    fn el_fork_obligatorio_a_una_epoca_nueva_y_el_padro_no_se_pisa() {
        // Derivado del UNIQUE medido: la copia y el original no pueden
        // convivir en la misma época.
        let mut e = conversacion();
        let padre = e.cabeza.unwrap();
        let epoch_padre = e.current_epoch;
        e.fork(padre).unwrap();
        assert_eq!(e.current_epoch, epoch_padre + 1);
        assert_eq!(e.mensajes_de_epoch(epoch_padre).len(), 3);
        assert_eq!(e.mensajes_de_epoch(epoch_padre + 1).len(), 3);
        // Los mensajes de la época nueva son copias (mismo seq, misma cosa).
        for (a, b) in e
            .mensajes_de_epoch(epoch_padre)
            .iter()
            .zip(e.mensajes_de_epoch(epoch_padre + 1).iter())
        {
            assert_eq!(a.seq, b.seq);
            assert_eq!(a.contenido, b.contenido);
        }
    }

    #[test]
    fn la_secuencia_global_no_se_reinicia() {
        let mut e = conversacion();
        let ultimo = e.vista_viva().last().unwrap().seq;
        e.fork(e.cabeza.unwrap()).unwrap();
        e.anadir_mensaje("user", "tras el fork").unwrap();
        let nuevo = e.vista_viva().last().unwrap().seq;
        assert!(
            nuevo > ultimo,
            "el seq se reinició: {} -> {}",
            ultimo,
            nuevo
        );
        // Tras un revert tampoco se reutiliza.
        e.revertir_a(e.raices()[0]).unwrap();
        e.anadir_mensaje("user", "otra vez").unwrap();
        assert!(e.vista_viva().last().unwrap().seq > nuevo);
    }

    #[test]
    fn guardar_y_cargar_conserva_el_bosque() {
        let conn = bd();
        let mut e = conversacion();
        e.nueva_raiz("user", "otra cadena").unwrap();
        e.anadir_mensaje("assistant", "en la otra cadena").unwrap();
        guardar(&conn, &e).expect("guardar");

        let c = cargar(&conn, "s1").expect("cargar");
        assert_eq!(c.nodos.len(), e.nodos.len());
        assert_eq!(c.raices(), e.raices());
        assert_eq!(c.cabeza, e.cabeza);
        assert_eq!(c.current_epoch, e.current_epoch);
        // El historial de cada nodo se reconstruye igual.
        for (id, nodo) in e.nodos.iter() {
            assert_eq!(
                &c.nodos[id].historial, &nodo.historial,
                "el historial del nodo {} no se reconstruyó bien",
                id
            );
        }
        assert_eq!(c.todos_los_mensajes(), e.todos_los_mensajes());
    }

    #[test]
    fn el_historial_se_persiste_en_messages_y_el_padro_no_se_borra() {
        let conn = bd();
        let mut e = conversacion();
        let padre = e.cabeza.unwrap();
        guardar(&conn, &e).expect("guardar");
        let antes: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE session_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(antes, 3);

        e.fork(padre).unwrap();
        guardar(&conn, &e).expect("guardar tras el fork");

        // Los 3 mensajes del padre siguen en su época...
        let padre_epoch: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE session_id='s1' AND compaction_epoch=0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(padre_epoch, 3, "el padre perdió sus mensajes");
        // ...y la copia del fork está en la época nueva.
        let copia: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE session_id='s1' AND compaction_epoch=1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(copia, 3);
        // El UNIQUE medido impide el mismo (época, seq) dos veces.
        let dupe = conn.execute(
            "INSERT INTO messages(session_id,sequence_number,role,content,timestamp,compaction_epoch)
             VALUES('s1',1,'user','x',1,0)",
            [],
        );
        assert!(dupe.is_err());
    }

    #[test]
    fn el_bosque_sobrevive_a_un_viaje_de_id_y_vuelta() {
        let conn = bd();
        let mut e = conversacion();
        let padre = e.cabeza.unwrap();
        e.fork(padre).unwrap();
        e.nueva_raiz("user", "tercera cadena").unwrap();
        guardar(&conn, &e).unwrap();
        let c = cargar(&conn, "s1").unwrap();
        assert_eq!(c.raices().len(), 2);
        assert_eq!(c.nodos.len(), e.nodos.len());
        // La copia del fork se reconstruye con la MISMA historia (mismos
        // mensajes) pero en la época del fork.
        let padre_en_carga = c.nodos[&padre].historial.clone();
        let hijo = c
            .nodos
            .values()
            .find(|n| n.padre == Some(padre) && n.es_fork())
            .expect("el nodo del fork");
        assert_eq!(hijo.historial.len(), padre_en_carga.len());
        for (a, b) in hijo.historial.iter().zip(padre_en_carga.iter()) {
            assert_eq!(a.seq, b.seq);
            assert_eq!(a.contenido, b.contenido);
            assert_eq!(a.epoch, hijo.epoch, "la copia no está en la época del fork");
            assert_eq!(b.epoch, padre_en_carga[0].epoch);
        }
        // Y la siguiente secuencia sigue siendo global.
        assert_eq!(c.siguiente_seq, e.siguiente_seq);
    }

    #[test]
    fn un_nodo_inexistente_da_error() {
        let mut e = conversacion();
        assert!(e.fork(999).is_err());
        assert!(e.revertir_a(999).is_err());
        assert!(e.preparar_revert(999).is_err());
    }

    #[test]
    fn revertir_a_un_nodo_no_crea_nodos_ni_borra_ninguno() {
        let mut e = conversacion();
        let antes = e.nodos.len();
        let raiz = e.raices()[0];
        e.revertir_a(raiz).unwrap();
        assert_eq!(e.nodos.len(), antes);
        assert_eq!(e.cabeza, Some(raiz));
        // La época viva es la del nodo al que se vuelve.
        assert_eq!(e.current_epoch, e.nodos[&raiz].epoch);
    }

    #[test]
    fn el_historial_de_un_nodo_es_el_camino_desde_la_raiz() {
        let e = conversacion();
        let raiz = e.raices()[0];
        let camino = e.ruta(e.cabeza.unwrap());
        assert_eq!(camino.len(), 3);
        assert_eq!(camino[0], raiz);
        // La suma de los mensajes propios del camino = el historial.
        let cuenta: usize = camino
            .iter()
            .map(|id| e.nodos[id].historial.len())
            .collect::<Vec<_>>()
            .last()
            .copied()
            .unwrap_or(0);
        assert_eq!(cuenta, 3);
    }
}
