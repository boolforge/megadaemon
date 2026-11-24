//! MegaDaemon v16 — CARGADOR DE SKILLS (carpetas de instrucciones).
//!
//! ## Qué es una skill
//!
//! Una skill es una **carpeta con un `SKILL.md`**: un `frontmatter` (metadatos
//! en YAML) + un cuerpo en Markdown que son las instrucciones que el agente
//! sigue cuando la skill aplica.  El formato NO es inventado aquí: es el que
//! miden las dos referencias:
//!
//! * **Claude Code** (fuente recuperada de `@anthropic-ai/claude-code@2.1.88`,
//!   `src/skills/loadSkillsDir.ts` y `src/skills/bundledSkills.ts`): campos
//!   `name`, `description`, `aliases`, `whenToUse`, `argumentHint`,
//!   `allowedTools`, `model`, `disableModelInvocation`, `userInvocable`,
//!   `hooks`, `context` (`inline` | `fork`), `agent` y `files`.
//! * **Codex** (`codex-rs/skills`, `openai/plugins`): manifiesto
//!   `.codex-plugin/plugin.json` con `skills/` y el estándar `agentskills.io`.
//!
//! ## Decisiones de diseño (y por qué)
//!
//! * **`context: inline | fork`** (medido en Claude Code): una skill grande se
//!   puede ejecutar en un **subagente** (fork) para no meter su texto en el
//!   contexto principal.  En MegaDaemon eso es lo que permite una skill
//!   enorme sin que el contexto se desborde.
//! * **`files` + la línea "Base directory for this skill: <dir>"**: el cuerpo
//!   de la skill no lleva los ficheros de referencia dentro; se le dice al
//!   agente DÓNDE están y los lee cuando los necesita.
//! * **Sin dependencias**: el `frontmatter` se parsea aquí (subconjunto
//!   YAML: `clave: valor`, `clave: [a, b]`, `clave: "texto"`, `#` de
//!   comentario) porque el crate no depende de un parser de YAML y una
//!   skill rota tiene que dar un error legible, no un panic.
//! * **Descubrimiento por varios raíz y deduplicación por nombre** (medido):
//!   las skills personales, las del proyecto y las de directorios extra.
//!
//! Ejecutar (lo compila y testea el root, ver docs/PROTOCOLO_FLOCKA.md):
//!   cd rust-core && cargo test --test skills -- --nocapture

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// El nombre del fichero de una skill.
pub const SKILL_FILE: &str = "SKILL.md";

/// Contexto en el que se ejecuta la skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contexto {
    /// La skill se ejecuta en el contexto principal (su texto entra ahí).
    Inline,
    /// La skill se ejecuta en un subagente (fork): su texto NO ensucia el
    /// contexto principal.
    Fork,
}

impl Contexto {
    /// Convierte el valor del frontmatter (`inline`/`fork`).
    pub fn de(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "inline" => Some(Contexto::Inline),
            "fork" => Some(Contexto::Fork),
            _ => None,
        }
    }
}

/// Una skill cargada.
#[derive(Debug, Clone, PartialEq)]
pub struct Skill {
    /// Identificador (único por catálogo).
    pub nombre: String,
    /// Cuándo aplicar la skill (lo lee el agente para decidir).
    pub descripcion: String,
    /// Herramientas que la skill puede usar (`allowed-tools`).
    pub herramientas: Vec<String>,
    /// Modelo sugerido (`model`).
    pub modelo: Option<String>,
    /// Contexto de ejecución (`context`).
    pub contexto: Contexto,
    /// Pista de argumentos (`argument-hint`).
    pub pista_argumentos: Option<String>,
    /// `disable-model-invocation`: el modelo NO puede invocarla sola.
    pub solo_usuario: bool,
    /// `license`.
    pub licencia: Option<String>,
    /// `version`.
    pub version: Option<String>,
    /// Carpeta donde vive la skill (su "directorio base").
    pub dir: PathBuf,
    /// Cuerpo de la skill (el Markdown sin el frontmatter).
    pub cuerpo: String,
}

impl Skill {
    /// La línea que el agente recibe para poder leer los ficheros de
    /// referencia bajo demanda (convención medida en Claude Code).
    pub fn linea_directorio_base(&self) -> String {
        format!(
            "Base directory for this skill: {}",
            self.dir.to_string_lossy()
        )
    }

    /// ¿Puede el modelo invocarla sin que el usuario la pida?
    pub fn invocable_por_el_modelo(&self) -> bool {
        !self.solo_usuario
    }
}

/// Error de carga de una skill (SIEMPRE legible, nunca panic).
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorSkill {
    pub ruta: PathBuf,
    pub motivo: String,
}

impl std::fmt::Display for ErrorSkill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.ruta.to_string_lossy(), self.motivo)
    }
}

/// Divide el texto en `(frontmatter, cuerpo)`.
///
/// Acepta el frontmatter con `---` al principio y al final.  Si no hay
/// frontmatter, el mapa va vacío y todo el texto es el cuerpo.
pub fn partir_frontmatter(texto: &str) -> (BTreeMap<String, String>, String) {
    let t = texto.trim_start_matches('\u{feff}');
    if !t.starts_with("---") {
        return (BTreeMap::new(), t.to_string());
    }
    // La primera línea tiene que ser exactamente `---`.
    let primera_fin = match t.find('\n') {
        Some(i) => i,
        None => return (BTreeMap::new(), t.to_string()),
    };
    if t[..primera_fin].trim() != "---" {
        return (BTreeMap::new(), t.to_string());
    }
    let resto = &t[primera_fin + 1..];
    // Buscar el `---` de cierre.
    let mut fin = None;
    let mut pos = 0usize;
    for linea in resto.split_inclusive('\n') {
        let limpia = linea.trim_end_matches(['\r', '\n']).trim();
        if limpia == "---" {
            fin = Some(pos);
            break;
        }
        pos += linea.len();
    }
    let fin = match fin {
        Some(f) => f,
        None => return (BTreeMap::new(), t.to_string()),
    };
    let bloque = &resto[..fin];
    let cuerpo = &resto[fin..];
    // Saltar el `---` de cierre.
    let cuerpo = match cuerpo.find('\n') {
        Some(i) => cuerpo[i + 1..].to_string(),
        None => String::new(),
    };
    (parsear_frontmatter(bloque), cuerpo)
}

/// Parsea el subconjunto de YAML que usan las skills.
///
/// * `clave: valor`   → texto (se quita las comillas).
/// * `clave: [a, b]`  → lista (la primera coma separa; se quitan comillas).
/// * `clave:`         → vacío.
/// * `#` al principio  → comentario.
/// * `clave:` con lista multilínea `- item` → se ignora (queda vacía); no es
///   un error: una skill con un frontmatter raro sigue siendo utilizable.
pub fn parsear_frontmatter(bloque: &str) -> BTreeMap<String, String> {
    let mut mapa = BTreeMap::new();
    for linea in bloque.lines() {
        let l = linea.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with('-') {
            continue;
        }
        let (clave, valor) = match l.split_once(':') {
            Some((c, v)) => (c.trim().to_string(), v.trim().to_string()),
            None => continue,
        };
        if clave.is_empty() {
            continue;
        }
        let valor = if valor.starts_with('[') && valor.ends_with(']') {
            let dentro = &valor[1..valor.len() - 1];
            let partes: Vec<String> = dentro
                .split(',')
                .map(|p| p.trim().trim_matches(['"', '\'']).to_string())
                .filter(|p| !p.is_empty())
                .collect();
            partes.join(", ")
        } else {
            valor.trim_matches(['"', '\'']).to_string()
        };
        mapa.insert(clave, valor);
    }
    mapa
}

/// Lee una lista separada por comas (para `allowed-tools`).
fn lista(valor: &str) -> Vec<String> {
    valor
        .split(',')
        .map(|s| s.trim().trim_matches(['"', '\'']).to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Carga una skill de un fichero `SKILL.md`.
pub fn cargar_fichero(ruta: &Path) -> Result<Skill, ErrorSkill> {
    let texto = std::fs::read_to_string(ruta).map_err(|e| ErrorSkill {
        ruta: ruta.to_path_buf(),
        motivo: format!("no se pudo leer: {e}"),
    })?;
    let (fm, cuerpo) = partir_frontmatter(&texto);
    let nombre = match fm.get("name") {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => {
            return Err(ErrorSkill {
                ruta: ruta.to_path_buf(),
                motivo: "falta `name` en el frontmatter".to_string(),
            })
        }
    };
    let descripcion = match fm.get("description") {
        Some(d) if !d.trim().is_empty() => d.trim().to_string(),
        _ => {
            return Err(ErrorSkill {
                ruta: ruta.to_path_buf(),
                motivo: "falta `description` en el frontmatter".to_string(),
            })
        }
    };
    let contexto = match fm.get("context").and_then(|c| Contexto::de(c)) {
        Some(c) => c,
        None => Contexto::Inline,
    };
    let dir = ruta
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(Skill {
        nombre,
        descripcion,
        herramientas: fm
            .get("allowed-tools")
            .or_else(|| fm.get("allowedTools"))
            .map(|v| lista(v))
            .unwrap_or_default(),
        modelo: fm.get("model").cloned(),
        contexto,
        pista_argumentos: fm
            .get("argument-hint")
            .or_else(|| fm.get("argumentHint"))
            .cloned(),
        solo_usuario: fm
            .get("disable-model-invocation")
            .or_else(|| fm.get("disableModelInvocation"))
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes"))
            .unwrap_or(false),
        licencia: fm.get("license").cloned(),
        version: fm.get("version").cloned(),
        dir,
        cuerpo,
    })
}

/// Carga todas las skills de un directorio (cada subcarpeta con `SKILL.md`).
///
/// Devuelve `(skills, errores)`: una skill rota **no** rompe el resto.
pub fn cargar_directorio(dir: &Path) -> (Vec<Skill>, Vec<ErrorSkill>) {
    let mut skills = Vec::new();
    let mut errores = Vec::new();
    let entradas = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            errores.push(ErrorSkill {
                ruta: dir.to_path_buf(),
                motivo: format!("no se pudo leer el directorio: {e}"),
            });
            return (skills, errores);
        }
    };
    let mut rutas: Vec<PathBuf> = Vec::new();
    for e in entradas.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let skill = p.join(SKILL_FILE);
        if skill.is_file() {
            rutas.push(skill);
        }
    }
    rutas.sort();
    for r in rutas {
        match cargar_fichero(&r) {
            Ok(s) => skills.push(s),
            Err(e) => errores.push(e),
        }
    }
    (skills, errores)
}

/// Descubre skills en varios directorios raíz y deduplica por nombre.
///
/// El **primer** directorio que trae una skill con ese nombre gana (el
/// personal manda sobre el del proyecto), que es el orden que usa Claude
/// Code (medido: `deduplicatedSkills`).
pub fn descubrir(raices: &[PathBuf]) -> (Vec<Skill>, Vec<ErrorSkill>) {
    let mut skills: Vec<Skill> = Vec::new();
    let mut errores: Vec<ErrorSkill> = Vec::new();
    for raiz in raices {
        let (s, e) = cargar_directorio(raiz);
        for skill in s {
            if skills.iter().any(|x| x.nombre == skill.nombre) {
                continue;
            }
            skills.push(skill);
        }
        errores.extend(e);
    }
    (skills, errores)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn escribir(dir: &Path, nombre: &str, contenido: &str) {
        let d = dir.join(nombre);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(SKILL_FILE), contenido).unwrap();
    }

    fn temporal(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "megadaemon-skills-test-{}-{}",
            nombre,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn el_frontmatter_se_parte_en_campos_y_cuerpo() {
        let texto = "---\nname: revisar\ndescription: Revisa el codigo\nallowed-tools: Read, Grep\n---\n\nEl cuerpo va aqui.\n";
        let (fm, cuerpo) = partir_frontmatter(texto);
        assert_eq!(fm.get("name").unwrap(), "revisar");
        assert_eq!(fm.get("allowed-tools").unwrap(), "Read, Grep");
        assert!(cuerpo.contains("El cuerpo va aqui."));
        assert!(!cuerpo.contains("name:"));
    }

    #[test]
    fn sin_frontmatter_todo_es_cuerpo() {
        let (fm, cuerpo) = partir_frontmatter("solo texto\n");
        assert!(fm.is_empty());
        assert_eq!(cuerpo, "solo texto\n");
    }

    #[test]
    fn las_listas_y_las_comillas_se_parsean() {
        let fm = parsear_frontmatter("a: [x, y, z]\nb: \"con espacio\"\nc: sin\n# comentario\nd:\n");
        assert_eq!(fm.get("a").unwrap(), "x, y, z");
        assert_eq!(fm.get("b").unwrap(), "con espacio");
        assert_eq!(fm.get("c").unwrap(), "sin");
        assert_eq!(fm.get("d").unwrap(), "");
        assert!(!fm.contains_key("comentario"));
    }

    #[test]
    fn una_skill_se_carga_con_sus_campos() {
        let d = temporal("carga");
        escribir(
            &d,
            "revisar",
            "---\nname: revisar\ndescription: Revisa el codigo\nallowed-tools: Read, Grep\ncontext: fork\nmodel: grande\nargument-hint: <ruta>\ndisable-model-invocation: true\nlicense: MIT\nversion: 1.2.3\n---\n\nCuerpo.\n",
        );
        let s = cargar_fichero(&d.join("revisar").join(SKILL_FILE)).unwrap();
        assert_eq!(s.nombre, "revisar");
        assert_eq!(s.herramientas, vec!["Read", "Grep"]);
        assert_eq!(s.contexto, Contexto::Fork);
        assert_eq!(s.modelo.as_deref(), Some("grande"));
        assert_eq!(s.pista_argumentos.as_deref(), Some("<ruta>"));
        assert!(s.solo_usuario);
        assert!(!s.invocable_por_el_modelo());
        assert_eq!(s.licencia.as_deref(), Some("MIT"));
        assert_eq!(s.version.as_deref(), Some("1.2.3"));
        assert!(s.cuerpo.contains("Cuerpo."));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn una_skill_sin_name_o_description_es_un_error_legible() {
        let d = temporal("errores");
        escribir(&d, "sin_nombre", "---\ndescription: hola\n---\ntexto\n");
        escribir(&d, "sin_desc", "---\nname: x\n---\ntexto\n");
        let e = cargar_fichero(&d.join("sin_nombre").join(SKILL_FILE)).unwrap_err();
        assert!(e.motivo.contains("name"), "{}", e.motivo);
        let e2 = cargar_fichero(&d.join("sin_desc").join(SKILL_FILE)).unwrap_err();
        assert!(e2.motivo.contains("description"), "{}", e2.motivo);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn el_descubrimiento_ignora_lo_que_no_es_skill_y_no_rompe_con_una_rota() {
        let d = temporal("descubrimiento");
        escribir(&d, "buena", "---\nname: buena\ndescription: ok\n---\ntexto\n");
        escribir(&d, "rota", "---\ndescription: sin nombre\n---\ntexto\n");
        std::fs::write(d.join("README.md"), "no es una skill").unwrap();
        let (skills, errores) = cargar_directorio(&d);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].nombre, "buena");
        assert_eq!(errores.len(), 1);
        assert!(errores[0].to_string().contains("rota"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn el_primer_raiz_gana_cuando_hay_dos_skills_del_mismo_nombre() {
        let personal = temporal("personal");
        let proyecto = temporal("proyecto");
        escribir(&personal, "comun", "---\nname: comun\ndescription: personal\n---\n");
        escribir(&proyecto, "comun", "---\nname: comun\ndescription: proyecto\n---\n");
        escribir(&proyecto, "propia", "---\nname: propia\ndescription: solo proyecto\n---\n");
        let (skills, _) = descubrir(&[personal.clone(), proyecto.clone()]);
        assert_eq!(skills.len(), 2);
        let comun = skills.iter().find(|s| s.nombre == "comun").unwrap();
        assert_eq!(comun.descripcion, "personal");
        let _ = std::fs::remove_dir_all(&personal);
        let _ = std::fs::remove_dir_all(&proyecto);
    }

    #[test]
    fn la_linea_del_directorio_base_apunta_a_la_carpeta_de_la_skill() {
        let d = temporal("base");
        escribir(&d, "guia", "---\nname: guia\ndescription: g\n---\n");
        let s = cargar_fichero(&d.join("guia").join(SKILL_FILE)).unwrap();
        let linea = s.linea_directorio_base();
        assert!(linea.starts_with("Base directory for this skill: "), "{linea}");
        assert!(linea.contains("guia"), "{linea}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_directorio_inexistente_da_error_y_no_panica() {
        let (skills, errores) = cargar_directorio(Path::new("/no/existe/jamas"));
        assert!(skills.is_empty());
        assert_eq!(errores.len(), 1);
    }
}
