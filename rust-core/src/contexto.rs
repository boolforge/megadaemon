//! Patrón 6 — **construir el contexto** (aquí) ≠ **conducir el agente** (allí).
//!
//! ## Qué hace este módulo
//!
//! `construir_contexto(estado, entrada) -> Contexto` **arma el prompt**:
//! devuelve los **bloques** que lo componen y los **metadatos** de esos
//! bloques.  Es **puro**: no habla con el modelo, no ejecuta herramientas, no
//! hace E/S (ni red ni disco).  Todo lo que necesita llega ya resuelto en el
//! `EstadoContexto`; quien habla con el modelo es el "conductor"
//! (`conducir`), que en MegaDaemon todavía no existe como módulo: hoy el
//! prompt se arma dentro del bucle (`agent_loop.rs`).
//!
//! ## De dónde sale la estructura (MEDIDO)
//!
//! * La **separación** viene medida del producto de referencia:
//!   `fx/context.rs` (arma el contexto: reglas, skills, memoria, ficheros) y
//!   `fx/driver.rs` (conduce la ejecución) — §2.5 de
//!   `docs/RE_DEVIN_CLI_RUST.md`, y patrón 6 de
//!   `docs/PATRONES_CLI_RUST_DEFINICION.md`.
//! * Los **bloques** de MegaDaemon salen de lo que hoy se arma en
//!   `rust-core/src/agent_loop.rs` (`build_prompt`, líneas 827-856): sistema,
//!   formatos admitidos, tarea, proveedores y memoria de fallos.
//! * La **vista viva por época** sale de `rust-core/src/compaction_epoch.rs`:
//!   el historial que ve el agente es el de la época **actual**
//!   (`Historial::vista_viva`), no el historial entero (que no se borra nunca).
//!
//! ## Lo que NO está medido (y por tanto se dice, no se disimula)
//!
//! * El bloque **FICHEROS**: la referencia lo tiene (`fx/context.rs` mete
//!   ficheros), pero **el prompt de MegaDaemon hoy no mete ficheros**.  El
//!   bloque existe porque el estado puede traer ficheros ya leídos (aquí sólo
//!   se formatean, para que la construcción siga siendo pura), pero **no hay
//!   ningún prompt medido que lo use**: su contenido es una convención de este
//!   módulo.
//! * Las **etiquetas** (`[SISTEMA]`, `[REGLAS]`, ...) **no están medidas**: el
//!   prompt plano de `agent_loop.rs` no lleva etiquetas.  Aquí se ponen para
//!   que cada bloque sea identificable en el prompt y para que los tests
//!   puedan comprobar el prompt exacto.
//! * El **orden** de los bloques es el canónico de este módulo (sistema,
//!   reglas, memoria, ficheros, época, historial, entrada); el prompt plano
//!   actual intercala "Tarea" entre las reglas y los proveedores.
//! * El **presupuesto** de caracteres no está medido: `presupuesto_chars` es
//!   `None` por defecto (sin límite ⇒ no se recorta nada).  El umbral por
//!   modelo vive en `model_catalog::umbral_compactacion` y lo pasa el
//!   llamador, que es quien sabe qué modelo se va a usar.
//!
//! ## Contrato de recorte (nada se borra en silencio)
//!
//! Si el contexto no cabe en el presupuesto, el recorte **se anota**: qué
//! bloque se ha tocado, cuántas unidades se han quitado y **cuántos
//! caracteres se han ahorrado** (medidos: largo original - largo final).  El
//! ahorro se cuenta en **caracteres** (escalares Unicode, como
//! `compaction_epoch::Mensaje::chars`), nunca en tokens inventados.

use crate::compaction_epoch::Historial;
use std::collections::BTreeMap;

/// La clase de un bloque del prompt.
///
/// El orden de la enumeración es el **orden canónico** de los bloques en el
/// prompt (y el orden en que se recorren los metadatos).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClaseBloque {
    /// Quién es el agente y qué tiene que hacer.
    Sistema,
    /// Los formatos admitidos y las reglas de enrutado (incluida la línea de
    /// proveedores disponibles).
    Reglas,
    /// Lo que ya falló: el agente no debe repetirlo.
    Memoria,
    /// Los ficheros que el agente puede ver (contenido ya leído por el
    /// llamador).
    Ficheros,
    /// La época viva de la compactación.
    Epoca,
    /// El historial de la época actual (la vista viva).
    Historial,
    /// La entrada del usuario: la pregunta a la que hay que responder.
    Entrada,
}

impl ClaseBloque {
    /// La etiqueta que abre el bloque dentro del prompt.
    pub fn etiqueta(&self) -> &'static str {
        match self {
            ClaseBloque::Sistema => "[SISTEMA]",
            ClaseBloque::Reglas => "[REGLAS]",
            ClaseBloque::Memoria => "[MEMORIA]",
            ClaseBloque::Ficheros => "[FICHEROS]",
            ClaseBloque::Epoca => "[EPOCA]",
            ClaseBloque::Historial => "[HISTORIAL]",
            ClaseBloque::Entrada => "[ENTRADA]",
        }
    }
}

/// Un fichero que el agente puede ver en el prompt.
///
/// El **contenido ya viene leído**: la construcción del contexto es pura y no
/// puede tocar el disco.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FicheroContexto {
    /// Nombre o ruta (informativo: el contenido ya está aquí).
    pub nombre: String,
    /// El contenido del fichero, ya leído por el llamador.
    pub contenido: String,
}

/// Un proveedor que el agente puede usar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProveedorContexto {
    /// Identificador del proveedor.
    pub id: String,
    /// Si es el proveedor **local** (el último recurso).
    pub local: bool,
}

/// La entrada del usuario: lo que hay que responder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entrada {
    /// El texto del usuario (la "tarea" del bucle).
    pub texto: String,
}

impl Entrada {
    /// Entrada nueva a partir de un texto.
    pub fn nueva(texto: impl Into<String>) -> Self {
        Entrada {
            texto: texto.into(),
        }
    }
}

impl From<&str> for Entrada {
    fn from(s: &str) -> Self {
        Entrada::nueva(s)
    }
}

/// Todo lo que el contexto necesita, **ya resuelto**.
///
/// Quien llama es responsable de leer lo que haga falta (ficheros, memoria de
/// fallos, historial) y de traerlo aquí **ya en memoria**: `construir_contexto`
/// no lee nada de ningún sitio.
#[derive(Clone, Debug)]
pub struct EstadoContexto {
    /// La instrucción base del agente (sistema).
    pub sistema: String,
    /// Las reglas: una por línea, en el orden en que se han de cumplir.
    pub reglas: Vec<String>,
    /// La memoria: lo que ya falló, una línea por entrada.
    pub memoria: Vec<String>,
    /// Los ficheros que el agente puede ver.
    pub ficheros: Vec<FicheroContexto>,
    /// El historial con épocas: de aquí sale la vista viva.
    pub historial: Historial,
    /// Los proveedores disponibles (se renderizan como una regla más).
    pub proveedores: Vec<ProveedorContexto>,
    /// El presupuesto de caracteres del prompt.  `None` = sin límite: **no
    /// se recorta nada** (no se inventa un número).
    pub presupuesto_chars: Option<usize>,
}

impl Default for EstadoContexto {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl EstadoContexto {
    /// Estado vacío: sin sistema, sin reglas, sin memoria, sin ficheros y con
    /// el historial en la época 0.
    pub fn nuevo() -> Self {
        EstadoContexto {
            sistema: String::new(),
            reglas: Vec::new(),
            memoria: Vec::new(),
            ficheros: Vec::new(),
            historial: Historial::nuevo("sesion"),
            proveedores: Vec::new(),
            presupuesto_chars: None,
        }
    }

    /// Fija el presupuesto de caracteres del prompt.
    pub fn con_presupuesto(mut self, presupuesto: Option<usize>) -> Self {
        self.presupuesto_chars = presupuesto;
        self
    }
}

/// Un bloque del prompt: su clase y su texto final.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BloqueContexto {
    /// De qué clase es el bloque.
    pub clase: ClaseBloque,
    /// El texto del bloque, tal y como va al prompt (incluye su etiqueta).
    pub texto: String,
}

/// Lo que se ha recortado de un bloque, y por qué.
///
/// El ahorro se mide en **caracteres**: largo original del bloque - largo
/// final.  Si el bloque desapareció entero, el ahorro es su largo completo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecorteContexto {
    /// El bloque afectado.
    pub clase: ClaseBloque,
    /// Caracteres ahorrados (medidos, no estimados).
    pub chars_ahorrados: usize,
    /// Cuántas unidades (líneas de contenido) se han quitado.
    pub unidades_eliminadas: usize,
    /// Si el bloque ha desaparecido entero del prompt.
    pub bloque_eliminado: bool,
    /// Por qué se ha recortado (texto legible: el presupuesto y lo que sobraba).
    pub motivo: String,
}

/// Los metadatos del contexto: lo que se mide **sin coste** (sin red, sin
/// disco, sin modelo).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetaContexto {
    /// Número de bloques del prompt.
    pub bloques: usize,
    /// Caracteres de todos los bloques juntos (sin los separadores).
    pub chars_totales: usize,
    /// Caracteres por clase de bloque.
    pub chars_por_clase: BTreeMap<ClaseBloque, usize>,
    /// El presupuesto que se aplicó (`None` = sin presupuesto).
    pub presupuesto_chars: Option<usize>,
    /// Si el contexto **cabe** en el presupuesto (tras el recorte).
    pub cabe: bool,
    /// Si ha habido que recortar algo.
    pub recortado: bool,
    /// Lo que se ha recortado, bloque a bloque (vacío si no ha hecho falta).
    pub recortes: Vec<RecorteContexto>,
    /// La época viva de la compactación.
    pub epoch: u32,
    /// Mensajes de la vista viva que tenía el historial.
    pub mensajes_vista_viva: usize,
    /// Mensajes de la vista viva que han cabido en el bloque de historial.
    pub mensajes_visibles: usize,
    /// Caracteres que tenía el contexto **antes** de recortar (la referencia
    /// para medir cuánto se ha ahorrado).
    pub total_inicial_chars: usize,
}

/// El contexto construido: los bloques del prompt y sus metadatos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contexto {
    /// Los bloques, en el orden canónico.
    pub bloques: Vec<BloqueContexto>,
    /// Los metadatos de esos bloques.
    pub metadatos: MetaContexto,
}

impl Contexto {
    /// Ensambla el prompt final: los bloques unidos por una línea en blanco.
    ///
    /// **Determinista**: mismo contexto ⇒ mismo prompt, byte a byte.  No hay
    /// marcas de tiempo, números de tick ni azar en el prompt.
    pub fn to_prompt(&self) -> String {
        let partes: Vec<&str> = self.bloques.iter().map(|b| b.texto.as_str()).collect();
        partes.join("\n\n")
    }

    /// El bloque de una clase (si está).
    pub fn bloque(&self, clase: ClaseBloque) -> Option<&BloqueContexto> {
        self.bloques.iter().find(|b| b.clase == clase)
    }

    /// El texto de un bloque (si está).
    pub fn texto_de(&self, clase: ClaseBloque) -> Option<&str> {
        self.bloque(clase).map(|b| b.texto.as_str())
    }
}

/// Construye el contexto: **puro**.
///
/// No llama al modelo, no ejecuta herramientas y no hace E/S.  Devuelve los
/// bloques del prompt y sus metadatos.  Si el contexto no cabe en el
/// presupuesto, se recorta y **se dice qué se ha recortado y cuánto se ha
/// ahorrado** (en caracteres).
pub fn construir_contexto(estado: &EstadoContexto, entrada: &Entrada) -> Contexto {
    let mut internos = bloques_internos(estado, entrada);
    let total_inicial = chars_totales(&internos);
    let recortes = match estado.presupuesto_chars {
        None => Vec::new(),
        Some(p) => recortar(&mut internos, p, total_inicial),
    };

    let bloques: Vec<BloqueContexto> = internos
        .iter()
        .map(|b| BloqueContexto {
            clase: b.clase,
            texto: b.texto(),
        })
        .collect();

    let mensajes_visibles = internos
        .iter()
        .find(|b| b.clase == ClaseBloque::Historial)
        .map(|b| b.unidades.len())
        .unwrap_or(0);

    let metadatos = metadatos(&bloques, estado, recortes, total_inicial, mensajes_visibles);
    Contexto { bloques, metadatos }
}

// =============================================================================
// INTERNO
// =============================================================================

/// Un bloque en construcción: la etiqueta + sus unidades (líneas de
/// contenido).  El recorte trabaja sobre las unidades, no sobre el texto.
struct BloqueInterno {
    clase: ClaseBloque,
    unidades: Vec<String>,
}

impl BloqueInterno {
    fn nuevo(clase: ClaseBloque, unidades: Vec<String>) -> Self {
        BloqueInterno { clase, unidades }
    }

    /// El texto final del bloque (etiqueta + unidades).
    fn texto(&self) -> String {
        if self.unidades.is_empty() {
            return self.clase.etiqueta().to_string();
        }
        let mut s = String::with_capacity(self.clase.etiqueta().len() + 16);
        s.push_str(self.clase.etiqueta());
        s.push('\n');
        s.push_str(&self.unidades.join("\n"));
        s
    }

    /// Caracteres del bloque (escalares Unicode).
    fn chars(&self) -> usize {
        self.texto().chars().count()
    }
}

/// Los bloques en el orden canónico.  Un bloque **sin contenido no aparece**
/// (no hay bloques vacíos en el prompt).
fn bloques_internos(estado: &EstadoContexto, entrada: &Entrada) -> Vec<BloqueInterno> {
    let mut v: Vec<BloqueInterno> = Vec::new();

    // 1) Sistema.
    let sistema = if estado.sistema.is_empty() {
        Vec::new()
    } else {
        vec![estado.sistema.clone()]
    };
    v.push(BloqueInterno::nuevo(ClaseBloque::Sistema, sistema));

    // 2) Reglas.  La línea de proveedores es una regla más: dice a quién puede
    //    llamar el agente (y cuál es el último recurso).
    let mut reglas = estado.reglas.clone();
    if !estado.proveedores.is_empty() {
        reglas.push(linea_proveedores(&estado.proveedores));
    }
    v.push(BloqueInterno::nuevo(ClaseBloque::Reglas, reglas));

    // 3) Memoria (lo que ya falló).
    v.push(BloqueInterno::nuevo(
        ClaseBloque::Memoria,
        estado.memoria.clone(),
    ));

    // 4) Ficheros (contenido ya leído por el llamador).
    let ficheros: Vec<String> = estado
        .ficheros
        .iter()
        .map(|f| format!("--- {} ---\n{}", f.nombre, f.contenido))
        .collect();
    v.push(BloqueInterno::nuevo(ClaseBloque::Ficheros, ficheros));

    // 5) Época viva.  Sólo si hay historial: sin mensajes no hay época que
    //    anunciar, y el prompt no lleva bloques de adorno.
    if !estado.historial.historial_completo().is_empty() {
        v.push(BloqueInterno::nuevo(
            ClaseBloque::Epoca,
            vec![linea_epoca(estado)],
        ));
    }

    // 6) Historial: **sólo la vista viva** (la época actual).  El historial
    //    entero no se borra (sigue en su sitio), pero aquí no se enseña.
    let historial: Vec<String> = estado
        .historial
        .vista_viva()
        .iter()
        .map(|m| format!("{}: {}", m.rol, m.contenido))
        .collect();
    v.push(BloqueInterno::nuevo(ClaseBloque::Historial, historial));

    // 7) Entrada del usuario: va la última (es lo que hay que responder).
    let ent = if entrada.texto.is_empty() {
        Vec::new()
    } else {
        vec![entrada.texto.clone()]
    };
    v.push(BloqueInterno::nuevo(ClaseBloque::Entrada, ent));

    v.retain(|b| !b.unidades.is_empty());
    v
}

/// La línea de proveedores: los ids, y el local marcado como último recurso.
fn linea_proveedores(proveedores: &[ProveedorContexto]) -> String {
    let lista: Vec<String> = proveedores
        .iter()
        .map(|p| {
            if p.local {
                format!("{} (local, ultimo recurso)", p.id)
            } else {
                p.id.clone()
            }
        })
        .collect();
    format!("Proveedores disponibles: {}", lista.join(", "))
}

/// La línea de la época: qué época está viva y cuánta historia se ve.
fn linea_epoca(estado: &EstadoContexto) -> String {
    let h = &estado.historial;
    let epochs: Vec<String> = h.epochs().iter().map(|e| e.to_string()).collect();
    format!(
        "Epoch viva: {} | Vista viva: {} mensajes | Historial completo: {} mensajes | Epochs: {}",
        h.current_epoch,
        h.vista_viva().len(),
        h.historial_completo().len(),
        epochs.join(",")
    )
}

/// Caracteres de todos los bloques.
fn chars_totales(bloques: &[BloqueInterno]) -> usize {
    bloques.iter().map(|b| b.chars()).sum()
}

/// Orden de recorte: **el primero es el que se recorta antes**.
///
/// `Sistema`, `Epoca` y `Entrada` no se recortan nunca: son la identidad del
/// agente, el estado de la compactación y la pregunta; sin ellos no hay
/// prompt que valga.
const ORDEN_RECORTE: [ClaseBloque; 4] = [
    ClaseBloque::Historial,
    ClaseBloque::Ficheros,
    ClaseBloque::Memoria,
    ClaseBloque::Reglas,
];

/// Recorta los bloques hasta que el total quepa en el presupuesto.
///
/// Política (documentada, determinista):
///
/// 1. Se recorta por **bloque**, en el orden de `ORDEN_RECORTE`.
/// 2. Dentro de un bloque se quitan unidades **por antigüedad**, empezando
///    por la segunda: la primera (el resumen de la época) se conserva
///    mientras el bloque sobreviva, y lo que se conserva al final son las
///    unidades **más recientes**.
/// 3. Si con dos unidades aún no cabe, se quita la primera; si con una
///    tampoco, el bloque **desaparece entero** (y se anota).
///
/// Lo que se quita **se anota**: bloque, unidades quitadas y caracteres
/// ahorrados (medidos).
fn recortar(
    bloques: &mut Vec<BloqueInterno>,
    presupuesto: usize,
    total_inicial: usize,
) -> Vec<RecorteContexto> {
    let mut recortes: Vec<RecorteContexto> = Vec::new();
    if total_inicial <= presupuesto {
        return recortes;
    }

    // Largo original de cada clase, para medir el ahorro exacto.
    let mut original: BTreeMap<ClaseBloque, usize> = BTreeMap::new();
    for b in bloques.iter() {
        *original.entry(b.clase).or_insert(0usize) += b.chars();
    }

    for clase in ORDEN_RECORTE {
        let mut unidades_eliminadas = 0usize;
        let mut bloque_eliminado = false;
        loop {
            if chars_totales(bloques) <= presupuesto {
                break;
            }
            let Some(i) = bloques.iter().position(|b| b.clase == clase) else {
                break;
            };
            if bloques[i].unidades.is_empty() {
                break;
            }
            // La primera unidad (resumen de la época) se conserva mientras el
            // bloque sobreviva; las del medio caen por antigüedad.
            let idx = if bloques[i].unidades.len() >= 3 { 1 } else { 0 };
            bloques[i].unidades.remove(idx);
            unidades_eliminadas += 1;
            if bloques[i].unidades.is_empty() {
                bloques.remove(i);
                bloque_eliminado = true;
                break;
            }
        }
        if unidades_eliminadas > 0 {
            let final_chars: usize = bloques
                .iter()
                .filter(|b| b.clase == clase)
                .map(|b| b.chars())
                .sum();
            let orig = *original.get(&clase).unwrap_or(&0usize);
            let mut motivo = format!(
                "el contexto ({} chars) no cabia en el presupuesto ({} chars)",
                total_inicial, presupuesto
            );
            if bloque_eliminado {
                motivo.push_str("; el bloque se ha eliminado del prompt");
            }
            recortes.push(RecorteContexto {
                clase,
                chars_ahorrados: orig.saturating_sub(final_chars),
                unidades_eliminadas,
                bloque_eliminado,
                motivo,
            });
        }
    }
    recortes
}

/// Calcula los metadatos del contexto.
fn metadatos(
    bloques: &[BloqueContexto],
    estado: &EstadoContexto,
    recortes: Vec<RecorteContexto>,
    total_inicial: usize,
    mensajes_visibles: usize,
) -> MetaContexto {
    let mut chars_por_clase: BTreeMap<ClaseBloque, usize> = BTreeMap::new();
    let mut chars_totales = 0usize;
    for b in bloques {
        let c = b.texto.chars().count();
        chars_totales += c;
        *chars_por_clase.entry(b.clase).or_insert(0usize) += c;
    }
    let presupuesto = estado.presupuesto_chars;
    let cabe = match presupuesto {
        None => true,
        Some(p) => chars_totales <= p,
    };
    MetaContexto {
        bloques: bloques.len(),
        chars_totales,
        chars_por_clase,
        presupuesto_chars: presupuesto,
        cabe,
        recortado: !recortes.is_empty(),
        recortes,
        epoch: estado.historial.current_epoch,
        mensajes_vista_viva: estado.historial.vista_viva().len(),
        mensajes_visibles,
        total_inicial_chars: total_inicial,
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Estado de ejemplo: reproduce las piezas que hoy arma `agent_loop.rs`.
    fn estado_demo() -> EstadoContexto {
        let mut historial = Historial::nuevo("s1");
        historial.anadir("user", "hola");
        historial.anadir("assistant", "buenas");

        let mut e = EstadoContexto::nuevo();
        e.sistema = "Eres el meta-agente de MegaDaemon. Responde SOLO con una accion.".to_string();
        e.reglas = vec![
            "Formatos admitidos:".to_string(),
            "  {\"action\":\"tool\",\"name\":\"<tool>\",\"args\":{...}}".to_string(),
        ];
        e.memoria = vec!["decision -> timeout (tick 3)".to_string()];
        e.proveedores = vec![
            ProveedorContexto {
                id: "cloud".to_string(),
                local: false,
            },
            ProveedorContexto {
                id: "seere".to_string(),
                local: true,
            },
        ];
        e.historial = historial;
        e
    }

    fn entrada_demo() -> Entrada {
        Entrada::nueva("Tarea: instalar el launcher")
    }

    /// Un estado con bastante historial (para probar el recorte).
    fn estado_con_historial_largo() -> EstadoContexto {
        let mut e = estado_demo();
        e.memoria = vec![
            "decision -> timeout (tick 3)".to_string(),
            "tool leer -> permiso denegado (tick 4)".to_string(),
        ];
        e.ficheros = vec![FicheroContexto {
            nombre: "notas.txt".to_string(),
            contenido: "contenido del fichero".to_string(),
        }];
        let mut historial = Historial::nuevo("s1");
        historial.anadir("user", "primera pregunta del usuario");
        historial.anadir("assistant", "primera respuesta del asistente");
        historial.anadir("user", "segunda pregunta del usuario");
        historial.anadir("assistant", "segunda respuesta del asistente");
        e.historial = historial;
        e
    }

    // -------------------------------------------------------------------------
    // Criterio de aceptación 1: el prompt exacto es estable.
    // -------------------------------------------------------------------------

    #[test]
    fn el_prompt_exacto_es_estable_byte_a_byte() {
        let e = estado_demo();
        let entrada = entrada_demo();
        let c1 = construir_contexto(&e, &entrada);
        let c2 = construir_contexto(&e, &entrada);
        // Mismo estado => mismo prompt, byte a byte.
        assert_eq!(c1.to_prompt(), c2.to_prompt());
        // Y el prompt es EXACTO (comparado a mano, sin marcas de tiempo).
        let esperado = r#"[SISTEMA]
Eres el meta-agente de MegaDaemon. Responde SOLO con una accion.

[REGLAS]
Formatos admitidos:
  {"action":"tool","name":"<tool>","args":{...}}
Proveedores disponibles: cloud, seere (local, ultimo recurso)

[MEMORIA]
decision -> timeout (tick 3)

[EPOCA]
Epoch viva: 0 | Vista viva: 2 mensajes | Historial completo: 2 mensajes | Epochs: 0

[HISTORIAL]
user: hola
assistant: buenas

[ENTRADA]
Tarea: instalar el launcher"#;
        assert_eq!(c1.to_prompt(), esperado);
    }

    #[test]
    fn el_prompt_no_cambia_entre_dos_construcciones_del_mismo_estado() {
        let e = estado_con_historial_largo().con_presupuesto(Some(400));
        let c1 = construir_contexto(&e, &entrada_demo());
        let c2 = construir_contexto(&e, &entrada_demo());
        assert_eq!(c1.to_prompt().as_bytes(), c2.to_prompt().as_bytes());
        assert_eq!(c1.metadatos, c2.metadatos);
    }

    // -------------------------------------------------------------------------
    // Criterio de aceptación 2: no se llama al modelo ni a las tools.
    // -------------------------------------------------------------------------

    #[test]
    fn la_construccion_no_llama_al_modelo_ni_a_las_tools() {
        // Se mira el CÓDIGO del fichero (los comentarios se quitan antes:
        // si no, la propia documentación del módulo daría falsos positivos).
        let codigo = sin_comentarios(include_str!("contexto.rs"));
        // Las palabras prohibidas se construyen en runtime para que este test
        // no las lleve escritas en el fichero (si no, el grep se encontraría
        // a sí mismo).
        let prohibidas: Vec<String> = vec![
            ["provider", "s::"].concat(),
            ["execute_", "chat"].concat(),
            ["execute_", "tool"].concat(),
            ["run_", "tool"].concat(),
        ];
        for palabra in &prohibidas {
            assert!(
                !codigo.contains(palabra.as_str()),
                "el modulo de contexto referencia '{}'",
                palabra
            );
        }
    }

    #[test]
    fn el_prompt_no_depende_del_reloj_ni_del_azar() {
        let codigo = sin_comentarios(include_str!("contexto.rs"));
        let prohibidas: Vec<String> = vec![
            ["Instant", "::now"].concat(),
            ["SystemTime", "::now"].concat(),
            ["rand", "::"].concat(),
        ];
        for palabra in &prohibidas {
            assert!(
                !codigo.contains(palabra.as_str()),
                "el modulo de contexto usa '{}' (el prompt debe ser determinista)",
                palabra
            );
        }
    }

    // -------------------------------------------------------------------------
    // El presupuesto: con holgura no se recorta; con poco, se recorta y se dice.
    // -------------------------------------------------------------------------

    #[test]
    fn con_presupuesto_amplio_no_se_recorta() {
        let e = estado_con_historial_largo().con_presupuesto(Some(100_000));
        let ctx = construir_contexto(&e, &entrada_demo());
        assert!(ctx.metadatos.cabe);
        assert!(!ctx.metadatos.recortado);
        assert!(ctx.metadatos.recortes.is_empty());
        assert_eq!(
            ctx.metadatos.chars_totales,
            ctx.metadatos.total_inicial_chars
        );
    }

    #[test]
    fn sin_presupuesto_no_se_recorta_nunca() {
        let e = estado_con_historial_largo();
        let ctx = construir_contexto(&e, &entrada_demo());
        assert!(ctx.metadatos.cabe);
        assert!(!ctx.metadatos.recortado);
        assert_eq!(ctx.metadatos.presupuesto_chars, None);
    }

    #[test]
    fn con_presupuesto_corto_se_recorta_y_se_dice_que_bloque_y_cuantos_chars() {
        let e = estado_con_historial_largo();
        let sin_recorte = construir_contexto(&e, &entrada_demo());
        let hist_chars = sin_recorte.metadatos.chars_por_clase[&ClaseBloque::Historial];
        // Presupuesto: todo menos el historial, más 10 chars de colchón.
        let presupuesto = sin_recorte.metadatos.chars_totales - hist_chars + 10;
        let e2 = e.clone().con_presupuesto(Some(presupuesto));
        let ctx = construir_contexto(&e2, &entrada_demo());

        assert!(ctx.metadatos.recortado);
        assert!(!ctx.metadatos.recortes.is_empty());
        let r = ctx.metadatos.recortes[0].clone();
        // Se dice QUÉ bloque.
        assert_eq!(r.clase, ClaseBloque::Historial);
        // Se dice CUÁNTOS chars se han ahorrado (medidos, no estimados).
        assert!(r.chars_ahorrados > 0);
        assert!(r.unidades_eliminadas > 0);
        // Y se dice POR QUÉ.
        assert!(r.motivo.contains("presupuesto"));
        // El resultado cabe.
        assert!(ctx.metadatos.cabe);
        assert!(ctx.metadatos.chars_totales <= presupuesto);
        // Y el historial se ha recortado de verdad.
        assert!(ctx.metadatos.mensajes_visibles < ctx.metadatos.mensajes_vista_viva);
    }

    #[test]
    fn el_ahorro_se_mide_en_caracteres_y_suma_exacto() {
        let e = estado_con_historial_largo();
        let sin_recorte = construir_contexto(&e, &entrada_demo());
        let presupuesto = sin_recorte.metadatos.chars_totales / 2;
        let e2 = e.clone().con_presupuesto(Some(presupuesto));
        let ctx = construir_contexto(&e2, &entrada_demo());

        let ahorro: usize = ctx
            .metadatos
            .recortes
            .iter()
            .map(|r| r.chars_ahorrados)
            .sum();
        assert_eq!(
            ahorro,
            ctx.metadatos.total_inicial_chars - ctx.metadatos.chars_totales
        );
        // Y el ahorro por bloque es el largo original menos el final.
        for r in &ctx.metadatos.recortes {
            let final_chars = ctx
                .bloques
                .iter()
                .filter(|b| b.clase == r.clase)
                .map(|b| b.texto.chars().count())
                .sum::<usize>();
            let original = sin_recorte
                .bloques
                .iter()
                .filter(|b| b.clase == r.clase)
                .map(|b| b.texto.chars().count())
                .sum::<usize>();
            assert_eq!(r.chars_ahorrados, original - final_chars);
        }
    }

    // -------------------------------------------------------------------------
    // La historia por época: sólo la vista viva.
    // -------------------------------------------------------------------------

    #[test]
    fn la_historia_usa_la_vista_viva_de_la_epoca_actual() {
        let mut historial = Historial::nuevo("s1");
        historial.anadir("user", "pregunta vieja");
        historial.anadir("assistant", "respuesta vieja");
        historial.compactar("resumen de la conversacion", 10);
        historial.anadir("user", "pregunta de la epoca nueva");

        let mut e = estado_demo();
        e.historial = historial;
        let ctx = construir_contexto(&e, &entrada_demo());

        // La época viva es la nueva (la 1).
        assert_eq!(ctx.metadatos.epoch, 1);
        let bloque = ctx.texto_de(ClaseBloque::Historial).unwrap();
        // La vista viva: el resumen + lo dicho en la época actual.
        assert!(bloque.contains("resumen de la conversacion"));
        assert!(bloque.contains("pregunta de la epoca nueva"));
        // Lo de la época vieja NO está en el prompt.
        assert!(!bloque.contains("pregunta vieja"));
        assert!(!bloque.contains("respuesta vieja"));
        // Pero el historial COMPLETO sigue intacto (compactar no borra).
        assert_eq!(e.historial.historial_completo().len(), 4);
    }

    // -------------------------------------------------------------------------
    // La entrada del usuario va en su bloque.
    // -------------------------------------------------------------------------

    #[test]
    fn la_entrada_del_usuario_va_en_su_bloque_y_es_el_ultimo() {
        let ctx = construir_contexto(&estado_demo(), &entrada_demo());
        let entrada = ctx.bloque(ClaseBloque::Entrada).expect("bloque de entrada");
        assert!(entrada.texto.contains("Tarea: instalar el launcher"));
        // Y es el último bloque del prompt (es lo que hay que responder).
        assert_eq!(ctx.bloques.last().unwrap().clase, ClaseBloque::Entrada);
        // La entrada no se ha colado en otros bloques.
        assert!(!ctx
            .texto_de(ClaseBloque::Sistema)
            .unwrap()
            .contains("instalar el launcher"));
    }

    // -------------------------------------------------------------------------
    // Los metadatos son coherentes con los bloques.
    // -------------------------------------------------------------------------

    #[test]
    fn los_metadatos_coinciden_con_los_textos_de_los_bloques() {
        let e = estado_con_historial_largo().con_presupuesto(Some(600));
        let ctx = construir_contexto(&e, &entrada_demo());
        let mut suma = 0usize;
        for b in &ctx.bloques {
            let c = b.texto.chars().count();
            suma += c;
            assert_eq!(
                ctx.metadatos.chars_por_clase[&b.clase], c,
                "los chars por clase no cuadran para {:?}",
                b.clase
            );
        }
        assert_eq!(ctx.metadatos.chars_totales, suma);
        assert_eq!(ctx.metadatos.bloques, ctx.bloques.len());
        // Los chars del prompt con separadores son los del contenido + 2 por
        // separación entre bloques.
        assert_eq!(
            ctx.to_prompt().chars().count(),
            ctx.metadatos.chars_totales + 2 * (ctx.bloques.len().saturating_sub(1))
        );
    }

    #[test]
    fn un_bloque_sin_contenido_no_aparece_en_el_prompt() {
        let mut e = estado_demo();
        e.memoria.clear();
        e.ficheros.clear();
        let ctx = construir_contexto(&e, &entrada_demo());
        assert!(ctx.bloque(ClaseBloque::Memoria).is_none());
        assert!(ctx.bloque(ClaseBloque::Ficheros).is_none());
        assert!(!ctx.to_prompt().contains("[MEMORIA]"));
        assert!(!ctx.to_prompt().contains("[FICHEROS]"));
        // Los metadatos tampoco los inventan.
        assert!(!ctx
            .metadatos
            .chars_por_clase
            .contains_key(&ClaseBloque::Memoria));
    }

    #[test]
    fn los_bloques_cumplen_el_orden_canonico() {
        let ctx = construir_contexto(&estado_con_historial_largo(), &entrada_demo());
        let orden: Vec<ClaseBloque> = ctx.bloques.iter().map(|b| b.clase).collect();
        assert_eq!(
            orden,
            vec![
                ClaseBloque::Sistema,
                ClaseBloque::Reglas,
                ClaseBloque::Memoria,
                ClaseBloque::Ficheros,
                ClaseBloque::Epoca,
                ClaseBloque::Historial,
                ClaseBloque::Entrada,
            ]
        );
    }

    #[test]
    fn el_bloque_de_ficheros_muestra_el_contenido_que_le_pasan() {
        let e = estado_con_historial_largo();
        let ctx = construir_contexto(&e, &entrada_demo());
        let bloque = ctx.texto_de(ClaseBloque::Ficheros).unwrap();
        assert!(bloque.contains("notas.txt"));
        assert!(bloque.contains("contenido del fichero"));
    }

    #[test]
    fn un_estado_vacio_solo_tiene_la_entrada() {
        // Sin sistema, reglas, memoria ni ficheros no hay bloques que enseñar;
        // y sin historial tampoco hay época que anunciar (MEDIDO 2026-11-28:
        // el bloque [EPOCA] con 0 mensajes era ruido en el prompt).
        let ctx = construir_contexto(&EstadoContexto::nuevo(), &entrada_demo());
        assert_eq!(ctx.bloques.len(), 1);
        assert_eq!(ctx.bloques[0].clase, ClaseBloque::Entrada);
        assert_eq!(
            ctx.metadatos.chars_totales,
            ctx.bloques[0].texto.chars().count()
        );
    }

    /// Quita los comentarios del código fuente (`//` y `/* */`), respetando
    /// las cadenas de texto y los literales de carácter.
    ///
    /// Sirve para comprobar el **código** y no su documentación.  Limitación
    /// conocida: las cadenas crudas (`r#"..."#`) con comillas dentro no se
    /// tratan como cadenas crudas (este módulo no las usa).
    fn sin_comentarios(src: &str) -> String {
        let c: Vec<char> = src.chars().collect();
        let mut out = String::with_capacity(src.len());
        let mut i = 0usize;
        while i < c.len() {
            let ch = c[i];
            // Comentario de línea.
            if ch == '/' && i + 1 < c.len() && c[i + 1] == '/' {
                i += 2;
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            // Comentario de bloque (anidado).
            if ch == '/' && i + 1 < c.len() && c[i + 1] == '*' {
                let mut nivel = 1usize;
                i += 2;
                while i < c.len() && nivel > 0 {
                    if c[i] == '/' && i + 1 < c.len() && c[i + 1] == '*' {
                        nivel += 1;
                        i += 2;
                        continue;
                    }
                    if c[i] == '*' && i + 1 < c.len() && c[i + 1] == '/' {
                        nivel -= 1;
                        i += 2;
                        continue;
                    }
                    if c[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
                continue;
            }
            // Cadena de texto o literal de carácter: se copian enteros.
            if ch == '"' || ch == '\'' {
                let delim = ch;
                out.push(ch);
                i += 1;
                while i < c.len() {
                    let d = c[i];
                    out.push(d);
                    i += 1;
                    if d == '\\' && i < c.len() {
                        out.push(c[i]);
                        i += 1;
                        continue;
                    }
                    if d == delim {
                        break;
                    }
                }
                continue;
            }
            out.push(ch);
            i += 1;
        }
        out
    }
}
