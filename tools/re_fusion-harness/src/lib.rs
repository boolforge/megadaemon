//! Arnés de prueba para `re_fusion.rs` con stubs de los motores pesados.
//! Permite ejecutar los tests reales del modulo y la fusion real con los
//! artefactos, sin enlazar la lib enorme de MegaDaemon (que no cabe en 3 GB).
use serde_json::{json, Value};

pub mod ghidra {
    use super::*;
    #[derive(Clone, Debug, Default)]
    pub struct GhidraConfig {
        pub ghidra_dir: Option<std::path::PathBuf>,
        pub java_home: Option<std::path::PathBuf>,
        pub project_dir: std::path::PathBuf,
        pub project_name: String,
        pub timeout: std::time::Duration,
        pub queue_wait: std::time::Duration,
        pub max_memory: String,
        pub script_dir: std::path::PathBuf,
    }
    #[derive(Clone, Debug, Default)]
    pub struct GhidraTool;
    #[derive(Clone, Debug, Default, serde::Serialize)]
    pub struct GhidraReport {
        pub stage: String, pub ok: bool, pub exit_code: Option<i32>,
        pub elapsed_secs: u64, pub message: String,
        pub functions: Option<u64>, pub symbols_applied: Option<u64>,
        pub functions_decompiled: Option<u64>, pub outputs: Vec<String>,
    }
    #[derive(Clone, Debug, Default)]
    pub struct DecompileOutcome { pub report: GhidraReport, pub files: Vec<String>, pub rows: Vec<String> }
    #[derive(Debug)]
    pub struct GhidraError(pub String);
    impl std::fmt::Display for GhidraError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.0) }
    }
    pub type GhidraResult<T> = Result<T, GhidraError>;
    pub fn import_binary(_t: &GhidraTool, _c: &GhidraConfig, _b: &std::path::Path) -> GhidraResult<GhidraReport> {
        Err(GhidraError("stub".into()))
    }
    pub fn apply_symbols(_t: &GhidraTool, _c: &GhidraConfig, _s: &std::path::Path) -> GhidraResult<GhidraReport> {
        Err(GhidraError("stub".into()))
    }
    pub fn decompile_anchors(_t: &GhidraTool, _c: &GhidraConfig, _a: &std::path::Path, _o: &std::path::Path, _f: &str) -> GhidraResult<DecompileOutcome> {
        Err(GhidraError("stub".into()))
    }
    impl GhidraTool {
        pub fn detect(_c: &GhidraConfig) -> GhidraResult<GhidraTool> { Err(GhidraError("stub".into())) }
    }
}
pub mod frida_dbi {
    use super::*;
    pub fn run_rizin_analysis(_f: &str, _m: &str, _a: Option<&str>) -> Value { json!({"status":"error","message":"stub"}) }
}
pub mod binwalk_die {
    use super::*;
    #[derive(Clone, Debug, Default, serde::Serialize)]
    pub struct SectionInfo { pub name: String }
    #[derive(Clone, Debug, Default, serde::Serialize)]
    pub struct EmbeddedSignature { pub name: String }
    #[derive(Clone, Debug, Default, serde::Serialize)]
    pub struct BulkArtifact { pub kind: String }
    #[derive(Clone, Debug, Default, serde::Serialize)]
    pub struct DieBinwalkReport {
        pub filename: String, pub format_family: String,
        pub sections: Vec<SectionInfo>,
        pub embedded_signatures: Vec<EmbeddedSignature>,
        pub bulk_artifacts: Vec<BulkArtifact>,
    }
    pub fn analyze_binary_full(_f: &str) -> DieBinwalkReport { DieBinwalkReport::default() }
}
pub mod re_engine {
    use super::*;
    #[derive(Clone, Debug, Default)]
    pub struct OpcionesAnalisis;
    #[derive(Clone, Debug, Default)]
    pub struct OpcionesDeofuscacion;
    pub fn analizar(_p: &std::path::Path, _o: &OpcionesAnalisis) -> Result<Value, String> { Ok(json!({"funciones": 3, "cadenas": 7})) }
    pub fn deobfuscar(_p: &std::path::Path, _o: &OpcionesDeofuscacion) -> Result<Value, String> { Ok(json!({"cadenas_cifradas": 2, "predicados_opacos": 1, "chains": 0})) }
}

pub mod re_strings {
    use super::*;
    #[derive(Clone, Debug)]
    pub enum StrEncoding { Ascii, Utf16Le }
    #[derive(Clone, Debug)]
    pub struct StringHit { pub offset: u64, pub length: usize, pub encoding: StrEncoding, pub text: String }
    /// Extrae los strings imprimibles (stub minimo pero real: ASCII de >=4).
    pub fn extract_strings(data: &[u8], min_len: usize, limit: usize) -> Vec<StringHit> {
        let mut out = Vec::new();
        let mut i = 0usize;
        while i < data.len() && out.len() < limit {
            if (32..127).contains(&data[i]) {
                let ini = i;
                while i < data.len() && (32..127).contains(&data[i]) { i += 1; }
                if i - ini >= min_len {
                    out.push(StringHit {
                        offset: ini as u64,
                        length: i - ini,
                        encoding: StrEncoding::Ascii,
                        text: String::from_utf8_lossy(&data[ini..i]).to_string(),
                    });
                }
            } else { i += 1; }
        }
        out
    }
}

#[path = "../../../rust-core/src/re_fusion.rs"]
pub mod re_fusion;
