//! P2-13 · Lint de fusiones de palabras en comentarios y documentación.
//!
//! Un modelo que escribe prosa mixta ES/EN deja artefactos como
//! `no es unydployer` (= "un" + "deployer").  Un comentario así se propaga a
//! todos los agentes que leen el repo, así que se detecta en CI.
//!
//! Reglas (calibradas contra el corpus real del repo, no a ojo):
//!
//! * **R1 · CJK pegado a ASCII**: un ideograma CJK inmediatamente al lado de una
//!   letra o dígito ASCII (sin espacio) ⇒ siempre error.  Cero falsos positivos.
//! * **R2 · Conector español pegado a palabra sólo-inglesa**: un token que NO
//!   está en el léxico ES, ni en el inglés, ni en el vocabulario del proyecto, y
//!   que se puede partir en `conector_español` + `palabra_inglesa` (≥4 letras)
//!   ⇒ error **sólo si aparece una única vez en todo el corpus** (hapax): un
//!   artefacto de fusión aparece una vez; una palabra real aparece varias.
//!   El filtro de "sólo-inglesa" es lo que evita los ~289 falsos positivos del
//!   enfoque ingenuo ("aportas" = a+portas, "determina" = de+termina, ...).
//!
//! Decisiones de diseño (para no romper a los demás agentes):
//!
//! * Los ficheros **modificados en el working tree** se saltan: si otro agente
//!   está editando a media PULSA, este lint no puede poner en rojo el árbol
//!   compartido.  En CI el árbol está limpio ⇒ todo se escanea.
//! * Hay una **línea base** (`tests/data/fusion_baseline.txt`): los hallazgos
//!   conocidos se aceptan; sólo fallan los **nuevos**.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Conectores **españoles** (y sus equivalentes) que, pegados a una palabra
/// inglesa, producen un artefacto de fusión.  Lista corta a propósito: "sand" +
/// "boxes" = "sandboxes" no es una fusión, por eso "sand" no está aquí.
const CONNECTORS: &[&str] = &[
    "un", "una", "uno", "el", "la", "lo", "los", "las", "al", "del", "de", "en", "y", "o", "no", "se",
    "sin", "con", "por", "para", "que", "debe", "deben", "puede", "pueden", "hay", "esta", "está", "ser",
];

/// Términos del dominio que no aparecen en los léxicos pero que NO son fusiones.
const PROJECT_TERMS: &[&str] = &[
    "megadaemon", "megadaemon-core", "md-coord", "md-verify", "termux", "shizuku", "exynos", "android",
    "samsung", "bootloader", "selinux", "magisk", "keystore", "keymaster", "keymint", "vbmeta", "dtbo",
    "ndjson", "jsonrpc", "mcp", "sha", "sha256", "blake3", "uuid", "base64", "adb", "fastboot", "regex",
    "openssl", "sqlite", "wal", "shm", "apk", "aab", "dex", "smali", "elf", "xml", "yaml", "toml",
    "deployer", "bootstrapper", "watchdog", "launcher", "payload", "handler", "supervisor",
];

fn repo_root() -> PathBuf {
    if let Ok(r) = std::env::var("MD_LINT_ROOT") {
        return PathBuf::from(r);
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rust-core/..")
        .to_path_buf()
}

fn read_lines(p: &Path) -> Option<Vec<String>> {
    fs::read_to_string(p)
        .ok()
        .map(|s| s.lines().map(|l| l.to_string()).collect())
}

fn norm(w: &str) -> String {
    w.chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF)
}

struct Lint {
    es: HashSet<String>,
    en: HashSet<String>,
    vocab: HashSet<String>,
    /// Vecindario de borrado: cada palabra inglesa con una letra menos.
    /// Permite detectar "unydployer" (= "un" + "dployer", a "deployer" con la
    /// 'e' comida): sin esto, una fusión con una letra desaparecida NO la
    /// detecta ni el mejor diccionario.
    del1: HashSet<String>,
}

impl Lint {
    fn load(root: &Path) -> Lint {
        let mut es = HashSet::new();
        let mut en = HashSet::new();
        if let Some(l) = read_lines(&root.join("rust-core/tests/data/lexicon_es.txt")) {
            for w in l {
                let w = norm(w.trim());
                if !w.is_empty() {
                    es.insert(w);
                }
            }
        }
        if let Some(l) = read_lines(&root.join("rust-core/tests/data/lexicon_en.txt")) {
            for w in l {
                let w = norm(w.trim());
                if !w.is_empty() {
                    en.insert(w);
                }
            }
        }
        let mut vocab: HashSet<String> = PROJECT_TERMS.iter().map(|w| norm(w)).collect();
        vocab.extend(project_vocab(root));
        let mut del1: HashSet<String> = HashSet::new();
        for w in en.iter().chain(vocab.iter()) {
            if w.len() >= 5 {
                let b = w.as_bytes();
                for i in 0..b.len() {
                    let mut v = w.clone();
                    v.remove(i);
                    del1.insert(v);
                }
            }
        }
        Lint { es, en, vocab, del1 }
    }

    fn known(&self, w: &str) -> bool {
        self.es.contains(w) || self.en.contains(w) || self.vocab.contains(w)
    }

    /// R2: ¿este token es una fusión "conector español + palabra inglesa"?
    /// `freq` = apariciones del token en el corpus (sólo se juzgan los hapax).
    ///
    /// El prefijo puede ser un conector **más una letra**: el artefacto real es
    /// `unydployer` (= "un" + "y" + "deployer"), no "undeployer".  Se exige que
    /// el token sea minúsculas en el original: así los identificadores en
    /// mayúsculas (`NOFOLLOW`) quedan fuera.
    fn is_fusion(&self, token: &str, freq: usize) -> Option<(String, String)> {
        if token.chars().any(|c| c.is_ascii_uppercase()) {
            return None; // identificador, no prosa
        }
        let w = norm(token);
        if w.len() < 4 || freq != 1 || self.known(&w) {
            return None;
        }
        for c in CONNECTORS {
            for extra in 0..=1usize {
                let cut = c.len() + extra;
                if !w.starts_with(c) || w.len() <= cut + 3 {
                    continue;
                }
                // la letra extra sólo puede ser vocal: "uny"+"deployer" sí es una
                // fusión, pero "des"+"crib" es un prefijo español normal.
                if extra == 1 && !matches!(w.as_bytes()[c.len()], b'a' | b'e' | b'i' | b'o' | b'u' | b'y') {
                    continue;
                }
                let suffix = &w[cut..];
                let englishish = self.en.contains(suffix) || self.vocab.contains(suffix);
                if englishish && !self.es.contains(suffix) {
                    return Some((format!("{}{}", c, &w[c.len()..cut]), suffix.to_string()));
                }
                // R3 · la palabra pegada le han comido una letra ("dployer" es
                // "deployer" sin la 'e').  Sólo con conector+vocal y palabras
                // largas: si no, cualquier palabra española terminada cerca de
                // una inglesa ("dependa", "deducen") salta.
                if extra == 1 && suffix.len() >= 6 && self.del1.contains(suffix) && !self.es.contains(suffix) {
                    return Some((
                        format!("{}{}", c, &w[c.len()..cut]),
                        format!("{}~", suffix),
                    ));
                }
            }
        }
        None
    }

    /// R1: ideograma CJK pegado a ASCII.
    fn has_cjk_glue(&self, line: &str) -> bool {
        let chars: Vec<char> = line.chars().collect();
        for i in 0..chars.len() {
            if !is_cjk(chars[i]) {
                continue;
            }
            let prev = if i > 0 { Some(chars[i - 1]) } else { None };
            let next = chars.get(i + 1).copied();
            if prev.map(|c| c.is_ascii_alphanumeric()).unwrap_or(false)
                || next.map(|c| c.is_ascii_alphanumeric()).unwrap_or(false)
            {
                return true;
            }
        }
        false
    }
}

/// Vocabulario del propio proyecto: identificadores del código (enteros y
/// partes de snake_case / camelCase).  Así "megadaemon" o "ndjson" no se
/// confunden con una fusión.
fn project_vocab(root: &Path) -> HashSet<String> {
    let mut set = HashSet::new();
    let mut stack = vec![root.join("rust-core/src")];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if !p.extension().map(|x| x == "rs").unwrap_or(false) {
                continue;
            }
            let lines = match read_lines(&p) {
                Some(l) => l,
                None => continue,
            };
            for line in lines {
                // Sólo el CÓDIGO: si se metieran los comentarios, cada palabra
                // de un comentario entraría en el vocabulario y el detector se
                // anularía a sí mismo (el artefacto "unydployer" se declararía
                // "palabra conocida" por estar escrito en el comentario).
                for id in code_part(&line)
                    .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                {
                    let id = id.trim_end_matches('_');
                    if id.len() < 3 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                        continue;
                    }
                    set.insert(norm(id));
                    let mut cur = String::new();
                    for ch in id.chars() {
                        if ch == '_' {
                            if cur.len() >= 3 {
                                set.insert(norm(&cur));
                            }
                            cur.clear();
                            continue;
                        }
                        if ch.is_ascii_uppercase() && !cur.is_empty() {
                            if cur.len() >= 3 {
                                set.insert(norm(&cur));
                            }
                            cur.clear();
                        }
                        cur.push(ch);
                    }
                    if cur.len() >= 3 {
                        set.insert(norm(&cur));
                    }
                }
            }
        }
    }
    set
}

/// (fichero, línea, texto) de los comentarios del código y del markdown.
fn corpus(root: &Path) -> Vec<(String, usize, String)> {
    let mut out = Vec::new();
    let mut stack = vec![root.join("rust-core/src")];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if !p.extension().map(|x| x == "rs").unwrap_or(false) {
                continue;
            }
            if let Some(lines) = read_lines(&p) {
                for (i, line) in lines.iter().enumerate() {
                    if line.trim_start().starts_with("//") {
                        out.push((rel(root, &p), i + 1, line.clone()));
                    }
                }
            }
        }
    }
    let mut stack = vec![root.join("docs")];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().map(|x| x == "md").unwrap_or(false) {
                if let Some(lines) = read_lines(&p) {
                    for (i, line) in lines.iter().enumerate() {
                        out.push((rel(root, &p), i + 1, line.clone()));
                    }
                }
            }
        }
    }
    out
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Ficheros modificados en el working tree (git) ⇒ se saltan.
fn dirty_files(root: &Path) -> HashSet<String> {
    if std::env::var("MD_LINT_SKIP_DIRTY").ok().as_deref() == Some("1") {
        return HashSet::new();
    }
    let mut set = HashSet::new();
    if let Ok(out) = Command::new("git")
        .args(["-C", &root.to_string_lossy(), "status", "--porcelain"])
        .output()
    {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                if line.len() > 3 {
                    let p = line[3..].trim().to_string();
                    let p = p.split(" -> ").last().unwrap_or(&p).to_string();
                    set.insert(p.trim_matches('"').to_string());
                }
            }
        }
    }
    set
}

fn baseline(root: &Path) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Some(lines) = read_lines(&root.join("rust-core/tests/data/fusion_baseline.txt")) {
        for l in lines {
            let l = l.trim().to_string();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            set.insert(l);
        }
    }
    set
}

/// Trocea una línea en tokens ASCII alfanuméricos con su columna.
fn tokens(line: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut start = 0usize;
    for (i, c) in line.char_indices() {
        if c.is_ascii_alphanumeric() {
            if cur.is_empty() {
                start = i;
            }
            cur.push(c);
        } else if !cur.is_empty() {
            out.push((start, std::mem::take(&mut cur)));
        }
    }
    if !cur.is_empty() {
        out.push((start, cur));
    }
    out
}

/// Devuelve la parte de código de una línea: sin comentarios y sin literales
/// de cadena.
fn code_part(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut quote: Option<char> = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == '\\' {
                    chars.next();
                } else if c == q {
                    quote = None;
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                } else if c == '/' && chars.peek() == Some(&'/') {
                    break;
                } else {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// Fragmento estable alrededor del ideograma (para la clave de la línea base).
fn cjk_fragment(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    for i in 0..chars.len() {
        if is_cjk(chars[i]) {
            let a = i.saturating_sub(6);
            let b = (i + 7).min(chars.len());
            return chars[a..b].iter().collect();
        }
    }
    String::new()
}




/// Analiza el repositorio y devuelve los hallazgos (sin baseline).
pub fn analizar(root: &Path) -> Vec<String> {

    let root = root.to_path_buf();
    let lint = Lint::load(&root);
    let dirty = dirty_files(&root);
    let base = baseline(&root);
    let corpus = corpus(&root);

    let mut freq: HashMap<String, usize> = HashMap::new();
    for (_, _, text) in &corpus {
        for (_, tok) in tokens(text) {
            *freq.entry(norm(&tok)).or_insert(0) += 1;
        }
    }

    let mut findings: Vec<(String, String)> = Vec::new();
    for (path, line, text) in &corpus {
        if dirty.contains(path) {
            continue; // trabajo en vuelo de otro agente: no lo juzgamos
        }
        if lint.has_cjk_glue(text) {
            let frag = cjk_fragment(text);
            findings.push((
                format!("{}:{}", path, frag),
                format!("{}:{}: [R1] CJK pegado a ASCII: {}", path, line, text.trim()),
            ));
            continue;
        }
        for (col, tok) in tokens(text) {
            if let Some((c, suffix)) = lint.is_fusion(&tok, freq.get(&norm(&tok)).copied().unwrap_or(0))
            {
                findings.push((
                    format!("{}:{}", path, tok),
                    format!(
                        "{}:{}:{}: [R2] fusión {:?} = {:?} + {:?}",
                        path, line, col, tok, c, suffix
                    ),
                ));
            }
        }
    }

    let nuevos: Vec<&String> = findings
        .iter()
        .filter(|(key, _)| !base.contains(key))
        .map(|(_, msg)| msg)
        .collect();

    // En vez de panicar (el test) devolvemos los hallazgos nuevos: el binario
    // los imprime y sale con codigo 1; el test los compara con el baseline.
    let mut salida: Vec<String> = nuevos.into_iter().cloned().collect();
    salida.sort();
    salida
}

fn main() {
    let root = repo_root();
    let hallazgos = analizar(&root);
    if hallazgos.is_empty() {
        println!("OK: sin fusiones ni CJK pegado en comentarios y docs.");
        return;
    }
    for h in &hallazgos {
        println!("{h}");
    }
    eprintln!("\n{} hallazgo(s). Si son falsos positivos, añade 'ruta:token' a", hallazgos.len());
    eprintln!("rust-core/tests/data/fusion_baseline.txt");
    std::process::exit(1);
}
