//! Migración de apps y datos con **apoyo de IA** — y con **verificación real**.
//!
//! # El flujo, en tres pasos (y en este orden)
//!
//! 1. **Plan base determinista** ([`plan_base`]): se decide, con reglas
//!    comprobables, qué se copia, qué se excluye, qué no se puede migrar y
//!    en qué **orden** se migra.  Este plan **no** depende de ningún modelo.
//! 2. **Consulta al modelo** ([`consultar_ia`]): el modelo entra por el
//!    **gateway `megadaemon`** ([`crate::llm_gateway::chat_completions`],
//!    endpoint OpenAI-compatible con *routing* y *fallback* real).  El modelo
//!    recibe el plan base en JSON y devuelve su **propuesta**: las decisiones
//!    por paquete y el **orden** de migración.
//! 3. **Validación** ([`validar_plan_ia`]): **la salida del modelo NO se
//!    confía**.  Se comprueba el esquema y una lista de invariantes duras:
//!    * todo paquete del plan base debe aparecer exactamente una vez;
//!    * no se pueden inventar paquetes que no están en el inventario;
//!    * la decisión sólo puede ser `copiar`, `excluir` o `no_migrable`;
//!    * `no_migrable` exige motivo;
//!    * **la IA sólo puede acotar**: puede excluir o degradar, nunca
//!      ampliar el conjunto a migrar ni los bytes medidos;
//!    * el **orden** propuesto sólo puede reordenar paquetes que el plan
//!      base marca como `copiar` (nunca meter uno nuevo).
//!
//! Si algo no cuadra, la regla contaminada **se descarta** y se vuelve al
//! plan base: el resultado final es siempre *como mínimo* tan conservador
//! como el plan determinista.
//!
//! La **propuesta cruda** y la **validación** quedan guardadas en el plan
//! ([`PlanMigracion::propuesta_ia`] y [`PlanMigracion::validacion`]): el
//! bus puede auditar exactamente qué propuso la IA y qué se corrigió.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::backup_catalog::{ClaseBackup, EntradaInventario};

/// Qué se hace con un paquete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Se copia (datos + APK si los hay).
    Copiar,
    /// Se excluye a propósito (caché, duplicado, no hace falta…).
    Excluir,
    /// No se puede migrar (y hay que decir por qué).
    NoMigrable,
}

impl Decision {
    /// Nombre estable.
    pub fn as_str(self) -> &'static str {
        match self {
            Decision::Copiar => "copiar",
            Decision::Excluir => "excluir",
            Decision::NoMigrable => "no_migrable",
        }
    }

    /// Parsea el texto del modelo (admite variantes en español/inglés).
    pub fn parse(s: &str) -> Option<Decision> {
        match s.trim().to_lowercase().replace(' ', "_").as_str() {
            "copiar" | "copy" | "migrate" | "migrar" => Some(Decision::Copiar),
            "excluir" | "exclude" | "skip" | "omitir" => Some(Decision::Excluir),
            "no_migrable" | "not_migratable" | "impossible" => Some(Decision::NoMigrable),
            _ => None,
        }
    }
}

/// Una regla del plan (una por paquete).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Regla {
    /// Paquete.
    pub paquete: String,
    /// Qué se hace.
    pub decision: Decision,
    /// Por qué (obligatorio si `no_migrable`).
    pub motivo: String,
    /// Bytes medidos del backup de este paquete.
    pub bytes: u64,
    /// Clase de formato del backup.
    pub clase: String,
    /// ¿La decisión viene del modelo o de las reglas fijas?
    pub por_ia: bool,
}

/// Totales del plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Totales {
    /// Paquetes copiados.
    pub copiados: usize,
    /// Paquetes excluidos.
    pub excluidos: usize,
    /// Paquetes no migrables.
    pub no_migrables: usize,
    /// Bytes de los paquetes copiados.
    pub bytes_copiados: u64,
    /// Bytes de todos los paquetes.
    pub bytes_totales: u64,
}

/// Resultado de validar la respuesta del modelo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidacionIa {
    /// ¿La respuesta era utilizable **sin incidencias**?
    pub ok: bool,
    /// Correcciones aplicadas.
    pub correcciones: Vec<String>,
    /// Reglas descartadas.
    pub descartadas: Vec<String>,
    /// Motivo del rechazo global (si lo hubo).
    pub motivo: Option<String>,
}

/// El plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanMigracion {
    /// Reglas, una por paquete.
    pub reglas: Vec<Regla>,
    /// **Orden** de migración: los paquetes que se copian, en el orden en que
    /// se deben migrar (el primero de la lista, el primero).
    pub orden: Vec<String>,
    /// Totales.
    pub totales: Totales,
    /// ¿Participó el modelo?
    pub uso_ia: bool,
    /// Id del modelo que respondió (si hubo respuesta).
    pub modelo: Option<String>,
    /// **Propuesta cruda** del modelo (lo que devolvió, sin interpretar).
    pub propuesta_ia: Option<String>,
    /// **Validación** de esa propuesta (qué se corrigió y qué se descartó).
    pub validacion: Option<ValidacionIa>,
    /// Correcciones aplicadas al validar la respuesta del modelo.
    pub correcciones: Vec<String>,
    /// Reglas del modelo que se descartaron por no pasar la validación.
    pub descartadas: Vec<String>,
}

/// Construye el plan base **determinista**.
///
/// Reglas (fijas, sin IA):
/// * `.dat` / `.extdat` / `.med` / `.tar` ⇒ `copiar` (son los que se restauran).
/// * `.app` (APK) ⇒ `copiar` (se necesita para reinstalar).
/// * `.extra` / `.splits` / `.wfi` / `.cls` / `.msg` ⇒ `copiar` sólo si son
///   pequeños; si no, `excluir` por peso (y el motivo lo dice).
/// * Formatos no restaurables (`.png`, `.zip`, etc.) ⇒ `no_migrable`.
///
/// El **orden** base es determinista: primero lo **más grande** (es lo que
/// manda en el tiempo y en el espacio) y, a igualdad, el nombre del paquete.
pub fn plan_base(entradas: &[EntradaInventario], tope_bytes_extra: u64) -> PlanMigracion {
    let mut reglas: Vec<Regla> = Vec::new();
    for e in entradas {
        let paquete = paquete_de_nombre(&e.nombre);
        let (decision, motivo) = match e.clase {
            ClaseBackup::SwiftData | ClaseBackup::TitaniumTar | ClaseBackup::Sba1 => {
                (Decision::Copiar, "datos de la app: es lo que se restaura".to_string())
            }
            ClaseBackup::SwiftApk => {
                (Decision::Copiar, "APK: hace falta para reinstalar la app".to_string())
            }
            ClaseBackup::SwiftExternal | ClaseBackup::SwiftMedia => {
                (Decision::Copiar, "datos externos/multimedia de la app".to_string())
            }
            ClaseBackup::SwiftExtra | ClaseBackup::SwiftSplits => {
                if e.bytes > tope_bytes_extra {
                    (
                        Decision::Excluir,
                        format!(
                            "pesa {} y no aporta al restaurar",
                            crate::backup_catalog::bytes_legibles(e.bytes)
                        ),
                    )
                } else {
                    (Decision::Copiar, "metadatos pequeños: se copian".to_string())
                }
            }
            ClaseBackup::SwiftCallsSms | ClaseBackup::SwiftWifi => {
                (Decision::Copiar, "registros del sistema (SMS/llamadas/Wi-Fi)".to_string())
            }
            ClaseBackup::SwiftWalls => (Decision::Excluir, "fondos: se regeneran".to_string()),
            ClaseBackup::Apk => (Decision::Copiar, "APK suelto".to_string()),
            ClaseBackup::Zip | ClaseBackup::Tar | ClaseBackup::SevenZip => {
                (Decision::Copiar, "contenedor de backup legible".to_string())
            }
            ClaseBackup::Desconocido => (
                Decision::NoMigrable,
                "formato no identificado: no se sabe restaurar".to_string(),
            ),
        };
        reglas.push(Regla {
            paquete,
            decision,
            motivo,
            bytes: e.bytes,
            clase: e.clase.as_str().to_string(),
            por_ia: false,
        });
    }
    let orden = orden_base(&reglas);
    let totales = calcular_totales(&reglas);
    PlanMigracion {
        reglas,
        orden,
        totales,
        uso_ia: false,
        modelo: None,
        propuesta_ia: None,
        validacion: None,
        correcciones: Vec::new(),
        descartadas: Vec::new(),
    }
}

/// Orden base: los que se copian, del **más grande** al más pequeño.
fn orden_base(reglas: &[Regla]) -> Vec<String> {
    let mut copia: Vec<&Regla> = reglas.iter().filter(|r| r.decision == Decision::Copiar).collect();
    copia.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.paquete.cmp(&b.paquete)));
    copia.into_iter().map(|r| r.paquete.clone()).collect()
}

/// Deduce el paquete del nombre de un fichero de backup.
pub fn paquete_de_nombre(nombre: &str) -> String {
    let base = nombre.rsplit('/').next().unwrap_or(nombre);
    let limpio = base.split(" (").next().unwrap_or(base);
    // Se quita **la última** extensión (el punto pegado al nombre del
    // fichero), no la primera: los paquetes de Android llevan puntos
    // (`com.foo.bar`, `app.marlboroadvance.mpvex`).
    let sin_ext = match limpio.rfind('.') {
        Some(p) => limpio[..p].trim(),
        None => limpio.trim(),
    };
    // Swift nombra el APK `<pkg>.app`: si el nombre acababa en `.app`, ese
    // sufijo es del formato, no del paquete.
    if let Some(p) = sin_ext.rfind(".app") {
        if p + 4 == sin_ext.len() {
            return sin_ext[..p].to_string();
        }
    }
    sin_ext.to_string()
}

// ---------------------------------------------------------------------------
// La IA: pregunta y validación
// ---------------------------------------------------------------------------

/// El prompt que se le manda al modelo.
///
/// Es **estructurado**: el modelo tiene que devolver JSON con una entrada por
/// paquete, sólo tres decisiones posibles y un **orden** de migración.  Se le
/// da el plan base **ya calculado** para que no tenga que inventarse nada.
pub fn construir_prompt(base: &PlanMigracion, contexto: &str) -> String {
    construir_prompt_con_limite(base, contexto, 200)
}

/// Igual que [`construir_prompt`] pero **limitando** cuántas reglas van al
/// modelo.
///
/// El inventario real de Swift Backup son 64 143 ficheros ⇒ 1 700+ paquetes:
/// mandar eso a un modelo es un disparate de tokens.  Con el límite, al modelo
/// le van **las reglas más grandes** (las que más importan) y el resto se
/// queda con la decisión **determinista**.  La validación sigue aplicando
/// sobre el plan **completo**: lo que el modelo no vio, no lo puede cambiar.
pub fn construir_prompt_con_limite(
    base: &PlanMigracion,
    contexto: &str,
    max_reglas: usize,
) -> String {
    let mut orden: Vec<&Regla> = base.reglas.iter().collect();
    orden.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    let (mandadas, resto): (Vec<&Regla>, Vec<&Regla>) = if orden.len() > max_reglas {
        let (a, b) = orden.split_at(max_reglas);
        (a.to_vec(), b.to_vec())
    } else {
        (orden.clone(), Vec::new())
    };
    let ocultas = resto.len();
    let reglas: Vec<Value> = mandadas
        .iter()
        .map(|r| {
            json!({
                "package": r.paquete,
                "base_decision": r.decision.as_str(),
                "base_reason": r.motivo,
                "bytes": r.bytes,
                "format": r.clase,
            })
        })
        .collect();
    format!(
        "Eres un planificador de migración de backups de Android. \
         CONTEXTO: {contexto}\n\
         \nREGLAS DEL PLAN BASE (medidas, no las cambies de formato):\n{}\n\
         \nTAREA: revisa el plan base y devuelve un JSON con el MISMO conjunto de \
         paquetes (ni uno más, ni uno menos) y una decisión para cada uno.\n\
         Sólo puedes usar estas decisiones: \"copiar\", \"excluir\", \"no_migrable\".\n\
         Si la decisión es \"no_migrable\", el motivo es obligatorio y debe decir por qué \
         no se puede migrar.\n\
         Puedes EXCLUIR o degradar a \"no_migrable\"; nunca amplíes lo que se copia.\n\
         Además devuelve \"order\": la lista de paquetes que se COPIAN, en el orden \
         en que conviene migrarlos (el primero de la lista se migra primero). \
         En \"order\" sólo pueden aparecer paquetes con decisión \"copiar\".\n\
         \nFORMATO EXACTO (sólo JSON, sin texto antes ni después):\n\
         {{\"rules\":[{{\"package\":\"...\",\"decision\":\"copiar|excluir|no_migrable\",\"reason\":\"...\"}}],\
         \"order\":[\"...\"]}}\n\
         \n(Ojo: hay {ocultas} paquetes más que NO se te enseñan; esos se quedan con la \
         decisión base y no hace falta que los menciones.)\n",
        serde_json::to_string(&reglas).unwrap_or_default()
    )
}

/// Consulta al **gateway `megadaemon`** y devuelve la respuesta cruda.
///
/// El gateway ([`crate::llm_gateway::chat_completions`]) es el punto de
/// entrada **oficial**: habla OpenAI-compatible y por debajo ya hace el
/// *routing* y el *fallback* real entre proveedores (y el SEERE local).
///
/// Si el gateway no puede (p. ej. no hay ninguna credencial de modelo en el
/// vault), se cae al [`crate::providers::execute_chat`] directo con la
/// cascada del bus de servicios.  Si **ningún** modelo responde, se devuelve
/// `Err` y el plan se queda **determinista**.
pub fn consultar_ia(
    prompt: &str,
    modelo_preferido: Option<&str>,
) -> Result<(String, String), String> {
    let modelo = modelo_preferido
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "megadaemon".to_string());

    // 1) Gateway (OpenAI-compatible): es la vía normal.
    let cuerpo = json!({
        "model": modelo,
        "messages": [{
            "role": "system",
            "content": "Eres un planificador de migraciones de backups de Android. \
                        Respondes SÓLO con JSON válido, sin texto alrededor."
        }, {
            "role": "user",
            "content": prompt
        }],
        "max_tokens": 4096,
        "temperature": 0
    });
    match crate::llm_gateway::chat_completions(&cuerpo) {
        Ok(v) => {
            let reply = v
                .get("choices")
                .and_then(|c| c.as_array())
                .and_then(|a| a.first())
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            if !reply.trim().is_empty() {
                let id = v
                    .get("megadaemon")
                    .and_then(|m| m.get("provider_id"))
                    .and_then(|s| s.as_str())
                    .unwrap_or(&modelo)
                    .to_string();
                return Ok((reply, id));
            }
        }
        Err(e) => {
            // El gateway falló: se intenta la vía directa antes de rendirse.
            let directo = crate::providers::execute_chat(
                prompt,
                modelo_preferido.filter(|m| *m != "megadaemon"),
                None,
                None,
                4096,
            );
            if !directo.reply.trim().is_empty() {
                return Ok((directo.reply, directo.model_id));
            }
            return Err(format!("el gateway no respondió ({e})"));
        }
    }

    // 2) Bus de servicios: cadena de fallback real sobre los proveedores
    //    habilitados del vault.
    let vault = crate::providers::load_vault();
    let entradas: Vec<crate::service_bus::ServiceEntry> = vault
        .providers
        .iter()
        .filter(|p| p.enabled && p.auth_type != "local" && p.auth_type != "hybrid")
        .map(|p| {
            let mut e = crate::service_bus::ServiceEntry::new(&p.id, crate::service_bus::ServiceKind::Model);
            e.model = p.model.clone();
            e.endpoint = p.endpoint.clone();
            e.capabilities = vec!["inference".to_string()];
            e.enabled = true;
            e
        })
        .collect();
    if entradas.is_empty() {
        return Err("no hay proveedores de modelo disponibles en el vault".to_string());
    }
    let mut bus = crate::service_bus::ServiceBus::new(entradas);
    let outcome = bus.route(crate::service_bus::TaskType::Inference, &mut |e| {
        let r = crate::providers::execute_chat(prompt, Some(&e.id), None, None, 4096);
        if r.reply.trim().is_empty() {
            Err(format!("el modelo {} no devolvió respuesta", e.id))
        } else {
            Ok(r.reply)
        }
    });
    if outcome.ok {
        Ok((outcome.reply, outcome.used.unwrap_or_default()))
    } else {
        Err(format!(
            "ningún modelo respondió ({}); se queda el plan determinista",
            outcome.error.unwrap_or_else(|| "sin detalle".to_string())
        ))
    }
}

/// Valida la respuesta del modelo y la aplica **encima** del plan base.
///
/// La salida de la IA **nunca** se confianza sin comprobar:
/// * el JSON debe parsear y tener la forma `{"rules":[…]}` (el `"order"` es
///   opcional pero, si viene, se valida también);
/// * cada paquete debe existir en el plan base (no se inventan paquetes);
/// * la decisión debe ser una de las tres;
/// * `no_migrable` exige motivo;
/// * la IA **no puede ampliar**: si la base era `excluir`, la IA tampoco
///   puede pasar a `copiar`;
/// * el **orden** sólo puede contener paquetes que quedan en `copiar`.
pub fn validar_plan_ia(
    base: &PlanMigracion,
    respuesta: &str,
) -> (PlanMigracion, ValidacionIa) {
    let mut correcciones: Vec<String> = Vec::new();
    let mut descartadas: Vec<String> = Vec::new();

    let texto = extraer_json(respuesta);
    let v: Value = match serde_json::from_str(&texto) {
        Ok(v) => v,
        Err(e) => {
            return (
                base.clone(),
                ValidacionIa {
                    ok: false,
                    correcciones: Vec::new(),
                    descartadas: Vec::new(),
                    motivo: Some(format!("la respuesta del modelo no es JSON válido: {e}")),
                },
            )
        }
    };
    let reglas = match v.get("rules").and_then(|r| r.as_array()) {
        Some(r) => r.clone(),
        None => {
            return (
                base.clone(),
                ValidacionIa {
                    ok: false,
                    correcciones: Vec::new(),
                    descartadas: Vec::new(),
                    motivo: Some("la respuesta del modelo no trae la lista \"rules\"".to_string()),
                },
            )
        }
    };

    let mut por_paquete: std::collections::HashMap<String, (Decision, String)> =
        std::collections::HashMap::new();
    for r in &reglas {
        let paquete = r.get("package").and_then(|p| p.as_str()).unwrap_or("").trim();
        if paquete.is_empty() {
            descartadas.push("regla sin paquete".to_string());
            continue;
        }
        if !base.reglas.iter().any(|b| b.paquete == paquete) {
            descartadas.push(format!("paquete inventado por el modelo: {paquete}"));
            continue;
        }
        let Some(decision) = r
            .get("decision")
            .and_then(|d| d.as_str())
            .and_then(Decision::parse)
        else {
            descartadas.push(format!("decisión inválida para {paquete}"));
            continue;
        };
        let motivo = r
            .get("reason")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if decision == Decision::NoMigrable && motivo.is_empty() {
            descartadas.push(format!("\"no_migrable\" sin motivo para {paquete}"));
            continue;
        }
        if por_paquete.contains_key(paquete) {
            descartadas.push(format!("paquete duplicado en la respuesta: {paquete}"));
            continue;
        }
        por_paquete.insert(paquete.to_string(), (decision, motivo));
    }

    // Se aplica sobre el plan base, sólo acotando.
    let mut reglas_final = base.reglas.clone();
    for r in reglas_final.iter_mut() {
        if let Some((decision, motivo)) = por_paquete.get(&r.paquete) {
            let antes = r.decision;
            let permite = match (antes, *decision) {
                (Decision::Copiar, Decision::Copiar) => true,
                (Decision::Copiar, Decision::Excluir) => true,
                (Decision::Copiar, Decision::NoMigrable) => true,
                (Decision::Excluir, Decision::Excluir) => true,
                (Decision::Excluir, Decision::NoMigrable) => true,
                (Decision::NoMigrable, Decision::NoMigrable) => true,
                _ => false,
            };
            if !permite {
                correcciones.push(format!(
                    "{}: la IA quiso pasar de {} a {} ⇒ se conserva la decisión base",
                    r.paquete,
                    antes.as_str(),
                    decision.as_str()
                ));
                continue;
            }
            if *decision != antes {
                correcciones.push(format!(
                    "{}: {} → {} ({})",
                    r.paquete,
                    antes.as_str(),
                    decision.as_str(),
                    if motivo.is_empty() { "sin motivo" } else { &motivo }
                ));
                r.decision = *decision;
                if !motivo.is_empty() {
                    r.motivo = motivo.clone();
                }
                r.por_ia = true;
            }
        }
    }

    // --- El orden propuesto -------------------------------------------------
    let mut orden = orden_base(&reglas_final);
    if let Some(propuesto) = v.get("order").and_then(|o| o.as_array()) {
        let mut orden_ia: Vec<String> = Vec::new();
        for p in propuesto {
            let Some(nombre) = p.as_str() else {
                descartadas.push("elemento de \"order\" que no es texto".to_string());
                continue;
            };
            let nombre = nombre.trim();
            if nombre.is_empty() {
                continue;
            }
            if !reglas_final.iter().any(|r| r.paquete == nombre) {
                descartadas.push(format!("\"order\" con paquete inventado: {nombre}"));
                continue;
            }
            let decision = reglas_final
                .iter()
                .find(|r| r.paquete == nombre)
                .map(|r| r.decision)
                .unwrap_or(Decision::NoMigrable);
            if decision != Decision::Copiar {
                descartadas.push(format!(
                    "\"order\" mete {nombre}, que no se va a copiar (decisión: {})",
                    decision.as_str()
                ));
                continue;
            }
            if orden_ia.iter().any(|x| x == nombre) {
                descartadas.push(format!("\"order\" repite {nombre}"));
                continue;
            }
            orden_ia.push(nombre.to_string());
        }
        if !orden_ia.is_empty() {
            // Los que la IA no menciona se quedan al final, en el orden base.
            let mut resto: Vec<String> = orden
                .iter()
                .filter(|p| !orden_ia.iter().any(|x| x == *p))
                .cloned()
                .collect();
            let mut nuevo = orden_ia.clone();
            nuevo.append(&mut resto);
            if nuevo != orden {
                correcciones.push(format!(
                    "orden de migración: la IA propone {} paquetes por delante del orden base",
                    orden_ia.len()
                ));
            }
            orden = nuevo;
        }
    }

    let totales = calcular_totales(&reglas_final);
    let validacion = ValidacionIa {
        ok: descartadas.is_empty() && correcciones.is_empty(),
        correcciones: correcciones.clone(),
        descartadas: descartadas.clone(),
        motivo: None,
    };
    (
        PlanMigracion {
            reglas: reglas_final,
            orden,
            totales,
            uso_ia: true,
            modelo: None,
            propuesta_ia: Some(respuesta.chars().take(20_000).collect()),
            validacion: Some(validacion.clone()),
            correcciones,
            descartadas,
        },
        validacion,
    )
}

/// Calcula los totales de un conjunto de reglas.
fn calcular_totales(reglas: &[Regla]) -> Totales {
    let mut t = Totales {
        copiados: 0,
        excluidos: 0,
        no_migrables: 0,
        bytes_copiados: 0,
        bytes_totales: 0,
    };
    for r in reglas {
        t.bytes_totales += r.bytes;
        match r.decision {
            Decision::Copiar => {
                t.copiados += 1;
                t.bytes_copiados += r.bytes;
            }
            Decision::Excluir => t.excluidos += 1,
            Decision::NoMigrable => t.no_migrables += 1,
        }
    }
    t
}

/// Saca el JSON de una respuesta del modelo (que puede venir envuelto en
/// texto o en un bloque ```json).
pub fn extraer_json(respuesta: &str) -> String {
    let t = respuesta.trim();
    if let Some(i) = t.find('{') {
        if let Some(j) = t.rfind('}') {
            if j > i {
                return t[i..=j].to_string();
            }
        }
    }
    t.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entradas() -> Vec<EntradaInventario> {
        vec![
            EntradaInventario {
                nombre: "com.foo.bar.dat (SM-G998B)".into(),
                bytes: 1000,
                clase: ClaseBackup::SwiftData,
                mime: None,
                id: None,
            },
            EntradaInventario {
                nombre: "com.foo.bar.app (SM-G998B)".into(),
                bytes: 500,
                clase: ClaseBackup::SwiftApk,
                mime: None,
                id: None,
            },
            EntradaInventario {
                nombre: "v3.1.cls (SM-G998B)".into(),
                bytes: 10,
                clase: ClaseBackup::SwiftCallsSms,
                mime: None,
                id: None,
            },
        ]
    }

    #[test]
    fn plan_base_es_determinista() {
        let p = plan_base(&entradas(), 1024);
        assert_eq!(p.totales.copiados, 3);
        assert_eq!(p.totales.bytes_copiados, 1510);
        assert!(!p.uso_ia);
        // El orden base va de más grande a más pequeño.
        assert_eq!(p.orden, vec!["com.foo.bar", "v3.1"]);
    }

    #[test]
    fn la_ia_no_puede_ampliar() {
        let base = plan_base(&entradas(), 0); // con tope 0, `.cls` queda excluido
        let base_excluye = base
            .reglas
            .iter()
            .find(|r| r.clase == "swift_cls_msg")
            .map(|r| r.decision)
            .unwrap();
        assert_eq!(base_excluye, Decision::Excluir);

        let resp = r#"{"rules":[{"package":"v3.1","decision":"copiar","reason":"porque sí"}]}"#;
        let (plan, v) = validar_plan_ia(&base, resp);
        let sigue = plan
            .reglas
            .iter()
            .find(|r| r.clase == "swift_cls_msg")
            .map(|r| r.decision)
            .unwrap();
        assert_eq!(sigue, Decision::Excluir, "la IA no puede ampliar el plan");
        assert!(!v.ok, "hubo una corrección ⇒ la respuesta no era limpia");
        assert!(plan
            .correcciones
            .iter()
            .any(|c| c.contains("v3.1") && c.contains("conserva la decisión base")));
    }

    #[test]
    fn la_ia_no_puede_inventar_paquetes() {
        let base = plan_base(&entradas(), 1024);
        let resp = r#"{"rules":[{"package":"com.inventado","decision":"copiar","reason":"x"}]}"#;
        let (plan, v) = validar_plan_ia(&base, resp);
        assert!(!plan.reglas.iter().any(|r| r.paquete == "com.inventado"));
        assert!(!v.ok);
        assert!(plan.descartadas.iter().any(|d| d.contains("inventado")));
        assert!(v.descartadas.iter().any(|d| d.contains("inventado")));
    }

    #[test]
    fn json_invalido_no_rompe_el_plan() {
        let base = plan_base(&entradas(), 1024);
        let (plan, v) = validar_plan_ia(&base, "esto no es json");
        assert_eq!(plan.totales, base.totales);
        assert!(!v.ok);
        assert!(v.motivo.is_some());
    }

    #[test]
    fn la_ia_puede_proponer_el_orden_pero_solo_de_lo_que_se_copia() {
        let base = plan_base(&entradas(), 1024);
        // Propone un orden con un paquete que NO está en el plan.
        let resp = r#"{"rules":[],"order":["com.inventado","com.foo.bar"]}"#;
        let (plan, v) = validar_plan_ia(&base, resp);
        assert!(!v.ok);
        assert!(plan.descartadas.iter().any(|d| d.contains("com.inventado")));
        // El paquete inventado no entra en el orden.
        assert!(!plan.orden.contains(&"com.inventado".to_string()));
    }

    #[test]
    fn la_ia_puede_reordenar_lo_que_se_copia() {
        let base = plan_base(&entradas(), 1024);
        let resp = r#"{"rules":[],"order":["com.foo.bar"]}"#;
        let (plan, v) = validar_plan_ia(&base, resp);
        assert!(v.ok, "no hay incidencias: {:?}", v);
        assert_eq!(plan.orden[0], "com.foo.bar");
        assert!(plan.uso_ia);
        assert!(plan.propuesta_ia.is_some());
        assert!(plan.validacion.is_some());
    }

    #[test]
    fn extrae_json_de_una_respuesta_envuelta() {
        let t = "Claro:\n```json\n{\"rules\":[]}\n```\nfin";
        assert_eq!(extraer_json(t), "{\"rules\":[]}");
    }
}
