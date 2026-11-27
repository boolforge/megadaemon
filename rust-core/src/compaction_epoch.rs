//! **Compactación por época, sin borrar** (patrón 4 de
//! `docs/DISENO_PATRONES_CLI_RUST.md`).
//!
//! MEDIDO en el producto de referencia: `messages.compaction_epoch` +
//! `sessions.current_epoch`.  La compactación **no borra**: **marca una
//! época**.  El historial anterior sigue consultable.
//!
//! MEDIDO también el esquema (SQL literal de la referencia):
//!
//! ```sql
//! CREATE TABLE messages (
//!   id INTEGER PRIMARY KEY,
//!   session_id INTEGER NOT NULL,
//!   compaction_epoch INTEGER NOT NULL DEFAULT 0,
//!   sequence_number INTEGER NOT NULL,
//!   UNIQUE(session_id, compaction_epoch, sequence_number)
//! );
//! CREATE INDEX idx_messages_session_epoch ON messages(session_id, compaction_epoch);
//! ```
//!
//! En MegaDaemon el historial vive en `agent_loop.rs` / `board.rs`; este
//! módulo es el **modelo** de épocas: el mismo criterio, pero sin SQL y
//! testeable sin base de datos.
//!
//! Por qué aquí y no en un script: la regla del proyecto es que toda
//! capacidad nueva vive en Rust dentro del daemon y se expone por CLI y por
//! tool MCP.
//!
//! MEDIDO 2026-11-28: implementación propia; el esquema del producto de
//! referencia es material propietario y NO se ha copiado (sólo la idea y la
//! forma del DDL, que es la parte medida del análisis).

use serde_json::{json, Value};

/// El DDL medido en la referencia (documentado, no aplicado aquí).
pub const DDL_MEDIDO: &str = "\
CREATE TABLE messages (
  id INTEGER PRIMARY KEY,
  session_id INTEGER NOT NULL,
  compaction_epoch INTEGER NOT NULL DEFAULT 0,
  sequence_number INTEGER NOT NULL,
  UNIQUE(session_id, compaction_epoch, sequence_number)
);
CREATE INDEX idx_messages_session_epoch ON messages(session_id, compaction_epoch);";

/// Un mensaje del historial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mensaje {
    /// Número de secuencia GLOBAL (crece siempre; no se reinicia al
    /// compactar).
    pub seq: u64,
    /// La época a la que pertenece este mensaje.
    pub epoch: u32,
    pub rol: String,
    pub contenido: String,
    /// Si este mensaje es el RESUMEN que abrió su época.
    ///
    /// MEDIDO: el resumen es lo que el agente ve al empezar la época nueva.
    pub es_resumen: bool,
}

impl Mensaje {
    /// Caracteres del contenido (lo que mide el presupuesto de contexto).
    pub fn chars(&self) -> usize {
        self.contenido.chars().count()
    }
}

/// Lo que devolvió una compactación.
#[derive(Clone, Debug, PartialEq)]
pub struct Compactacion {
    /// La época nueva (la anterior queda sellada).
    pub epoch_nueva: u32,
    /// La época que se ha sellado.
    pub epoch_sellada: u32,
    /// Cuántos mensajes se han movido a la época anterior (NO se borran:
    /// se quedan donde estaban, con su epoch).
    pub mensajes_sellados: usize,
    /// Caracteres que el resumen ahorra en el prompt.
    pub chars_ahorrados: usize,
    /// El historial COMPLETO sigue siendo el mismo número de mensajes.
    pub mensajes_totales: usize,
}

/// El historial de una sesión, con épocas.
#[derive(Clone, Debug, PartialEq)]
pub struct Historial {
    pub session_id: String,
    /// La época en la que se está trabajando AHORA.
    pub current_epoch: u32,
    /// Todos los mensajes, en orden de `seq`.  Compactar NO borra ninguno.
    pub mensajes: Vec<Mensaje>,
}

impl Historial {
    /// Historial nuevo (época 0).
    pub fn nuevo(session_id: impl Into<String>) -> Self {
        Historial {
            session_id: session_id.into(),
            current_epoch: 0,
            mensajes: Vec::new(),
        }
    }

    /// Añade un mensaje a la época actual.
    pub fn anadir(&mut self, rol: impl Into<String>, contenido: impl Into<String>) -> u64 {
        let seq = self.mensajes.len() as u64 + 1;
        self.mensajes.push(Mensaje {
            seq,
            epoch: self.current_epoch,
            rol: rol.into(),
            contenido: contenido.into(),
            es_resumen: false,
        });
        seq
    }

    /// **Compacta SIN BORRAR**: sella la época actual y abre una nueva con
    /// un resumen.
    ///
    /// * Los mensajes anteriores **no se borran**: siguen en `mensajes` con
    ///   su `epoch` de siempre ⇒ el historial sigue siendo consultable.
    /// * La época nueva empieza con el resumen, que es lo que el agente ve.
    /// * `seq` es global y no se reinicia.
    /// * Si la vista viva NO pasa del presupuesto, NO se compacta (crear una
    ///   epoch para eso gastaria mas de lo que ahorra).
    pub fn compactar(&mut self, resumen: impl Into<String>, presupuesto_chars: usize) -> Compactacion {
        let resumen = resumen.into();
        let epoch_sellada = self.current_epoch;
        let chars_vivos = self.chars_vivos();
        if chars_vivos <= presupuesto_chars {
            // No hace falta compactar: crear una epoca para esto gastaria
            // mas de lo que ahorra.
            return Compactacion {
                epoch_nueva: epoch_sellada,
                epoch_sellada,
                mensajes_sellados: 0,
                chars_ahorrados: 0,
                mensajes_totales: self.mensajes.len(),
            };
        }
        // MEDIDO (2026-11-28): el recuento se hace ANTES del `push` (si no,
        // el prestamo de `sellados` sigue vivo y el `push` no compila:
        // E0502 "cannot borrow self.mensajes as mutable").
        let (mensajes_sellados, chars_sellados) = {
            let sellados: Vec<&Mensaje> = self
                .mensajes
                .iter()
                .filter(|m| m.epoch == epoch_sellada && !m.es_resumen)
                .collect();
            let n = sellados.len();
            let c: usize = sellados.iter().map(|m| m.chars()).sum();
            (n, c)
        };
        let chars_resumen = resumen.chars().count();

        self.current_epoch += 1;
        let seq = self.mensajes.len() as u64 + 1;
        self.mensajes.push(Mensaje {
            seq,
            epoch: self.current_epoch,
            rol: "resumen".to_string(),
            contenido: resumen,
            es_resumen: true,
        });

        Compactacion {
            epoch_nueva: self.current_epoch,
            epoch_sellada,
            mensajes_sellados,
            // El ahorro es lo que el resumen deja de gastar, menos lo que
            // el resumen cuesta: si el resumen es peor que el historial,
            // el ahorro es 0 (no se inventa un numero).
            chars_ahorrados: chars_sellados.saturating_sub(chars_resumen),
            mensajes_totales: self.mensajes.len(),
        }
    }

    /// Los mensajes de una época concreta.
    pub fn mensajes_de_epoch(&self, epoch: u32) -> Vec<&Mensaje> {
        self.mensajes.iter().filter(|m| m.epoch == epoch).collect()
    }

    /// La lista de épocas que existen (en orden).
    pub fn epochs(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self.mensajes.iter().map(|m| m.epoch).collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// La vista **viva**: lo que el agente ve ahora (la época actual).
    pub fn vista_viva(&self) -> Vec<&Mensaje> {
        self.mensajes_de_epoch(self.current_epoch)
    }

    /// El historial completo, de principio a fin (nunca se pierde).
    pub fn historial_completo(&self) -> &[Mensaje] {
        &self.mensajes
    }

    /// Caracteres de la vista viva (lo que ocupa el prompt).
    pub fn chars_vivos(&self) -> usize {
        self.vista_viva().iter().map(|m| m.chars()).sum()
    }

    /// Caracteres del historial entero (lo que ocuparía sin compactar).
    pub fn chars_totales(&self) -> usize {
        self.mensajes.iter().map(|m| m.chars()).sum()
    }

    /// **Revierte** a una epoch anterior: el historial sigue intacto, sólo
    /// cambia la época en la que se está trabajando.
    ///
    /// MEDIDO: es la operación que el patrón 3 (la conversación como bosque)
    /// necesita; aquí se hace al nivel de época.
    pub fn revertir_a(&mut self, epoch: u32) -> Result<(), String> {
        if !self.epochs().contains(&epoch) {
            return Err(format!("no existe la epoch {}", epoch));
        }
        self.current_epoch = epoch;
        Ok(())
    }

    /// El DDL medido, como documentation.
    pub fn ddl(&self) -> &'static str {
        DDL_MEDIDO
    }
}

// =============================================================================
// CLI
// =============================================================================
/// `megadaemon-core compaction-epoch <epochs|vista|revertir>`
pub fn run_cli(args: &[String]) -> i32 {
    match run_cli_res(args) {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            0
        }
        Err(e) => {
            eprintln!("compaction-epoch error: {}", e);
            1
        }
    }
}

fn run_cli_res(args: &[String]) -> Result<Value, String> {
    let sub = args.first().map(|s| s.as_str()).unwrap_or("");
    let h = Historial::nuevo("demo");
    match sub {
        "epochs" => Ok(json!({
            "session_id": h.session_id,
            "current_epoch": h.current_epoch,
            "epochs": h.epochs(),
            "ddl": DDL_MEDIDO,
        })),
        "vista" => Ok(json!({
            "current_epoch": h.current_epoch,
            "vista_viva": h.vista_viva().len(),
            "historial_completo": h.mensajes.len(),
        })),
        "revertir" => {
            let epoch: u32 = args
                .get(1)
                .and_then(|s| s.parse::<u32>().ok())
                .ok_or_else(|| "falta la epoch (número)".to_string())?;
            let mut h = h;
            h.revertir_a(epoch)?;
            Ok(json!({ "ok": true, "current_epoch": h.current_epoch }))
        }
        "" => Err("uso: compaction-epoch <epochs|vista|revertir> [epoch]".to_string()),
        otro => Err(format!("subcomando desconocido: {}", otro)),
    }
}

// =============================================================================
// TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn historial_con_turnos() -> Historial {
        let mut h = Historial::nuevo("s1");
        h.anadir("user", "hola");
        h.anadir("assistant", "buenas");
        h.anadir("user", "mas cosas");
        h
    }

    #[test]
    fn al_principio_todo_esta_en_la_epoch_0() {
        let h = historial_con_turnos();
        assert_eq!(h.current_epoch, 0);
        assert_eq!(h.epochs(), vec![0]);
        assert_eq!(h.vista_viva().len(), 3);
    }

    #[test]
    fn compactar_no_borra_nada() {
        let h0 = historial_con_turnos();
        let antes = h0.mensajes.len();
        let mut h = h0;
        let c = h.compactar("resumen de la conversacion", 10);
        // Los mensajes NO desaparecen: hay uno MÁS (el resumen).
        assert_eq!(h.mensajes.len(), antes + 1);
        assert_eq!(c.mensajes_totales, antes + 1);
        assert_eq!(c.epoch_sellada, 0);
        assert_eq!(c.epoch_nueva, 1);
        assert_eq!(c.mensajes_sellados, 3);
    }

    #[test]
    fn la_epoch_anterior_sigue_consultable() {
        let mut h = historial_con_turnos();
        h.compactar("resumen", 10);
        assert_eq!(h.mensajes_de_epoch(0).len(), 3);
        assert_eq!(h.mensajes_de_epoch(1).len(), 1);
        assert_eq!(h.mensajes_de_epoch(1)[0].es_resumen, true);
    }

    #[test]
    fn la_vista_viva_solo_muestra_la_epoch_actual() {
        let mut h = historial_con_turnos();
        h.compactar("resumen", 10);
        assert_eq!(h.vista_viva().len(), 1);
        assert_eq!(h.vista_viva()[0].rol, "resumen");
        // Pero el historial entero sigue ahí.
        assert_eq!(h.historial_completo().len(), 4);
    }

    #[test]
    fn el_seq_es_global_y_no_se_reinicia() {
        let mut h = historial_con_turnos();
        h.compactar("resumen", 10);
        h.anadir("user", "nuevo");
        let seqs: Vec<u64> = h.mensajes.iter().map(|m| m.seq).collect();
        assert_eq!(seqs, vec![1, 2, 3, 4, 5]);
        assert_eq!(h.mensajes[4].epoch, 1);
    }

    #[test]
    fn compactar_ahorra_contexto_solo_si_el_resumen_merece_la_pena() {
        let mut h = historial_con_turnos();
        h.anadir("assistant", "una respuesta muy larga ".repeat(50));
        let antes = h.chars_vivos();
        let c = h.compactar("resumen", 10);
        assert_eq!(c.chars_ahorrados, antes.saturating_sub("resumen".len()));
        assert!(h.chars_vivos() < h.chars_totales());
    }

    #[test]
    fn un_resumen_peor_que_el_historial_no_ahorra_nada() {
        let mut h = Historial::nuevo("s2");
        h.anadir("user", "hi");
        let c = h.compactar("un resumen larguisimo que no compensa de nada", 10);
        assert_eq!(c.chars_ahorrados, 0);
    }

    #[test]
    fn revertir_cambia_de_epoch_pero_no_toca_el_historial() {
        let mut h = historial_con_turnos();
        h.compactar("resumen", 10);
        let antes = h.mensajes.len();
        h.revertir_a(0).unwrap();
        assert_eq!(h.current_epoch, 0);
        assert_eq!(h.vista_viva().len(), 3);
        assert_eq!(h.mensajes.len(), antes);
        assert!(h.revertir_a(99).is_err());
    }

    #[test]
    fn las_epochs_listadas_no_se_repiten() {
        let mut h = historial_con_turnos();
        h.compactar("r1", 10);
        // MEDIDO: para que la segunda compactacion ocurra, la vista viva
        // tiene que pasar el presupuesto (si no, no se crea epoch nueva).
        h.anadir("user", "una mensaje larga de verdad");
        h.compactar("r2", 10);
        assert_eq!(h.epochs(), vec![0, 1, 2]);
    }

    #[test]
    fn bajo_presupuesto_no_se_compacta() {
        let mut h = Historial::nuevo("s4");
        h.anadir("user", "hi");
        let antes = h.mensajes.len();
        let c = h.compactar("resumen", 1_000);
        assert_eq!(c.epoch_nueva, c.epoch_sellada);
        assert_eq!(c.mensajes_sellados, 0);
        assert_eq!(c.chars_ahorrados, 0);
        assert_eq!(h.mensajes.len(), antes);
        assert_eq!(h.current_epoch, 0);
    }

    #[test]
    fn el_ddl_medido_esta_documentado() {
        let h = Historial::nuevo("s3");
        assert!(h.ddl().contains("compaction_epoch"));
        assert!(h.ddl().contains("UNIQUE(session_id, compaction_epoch, sequence_number)"));
        assert!(h.ddl().contains("idx_messages_session_epoch"));
    }
}
