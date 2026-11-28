//! Arnés de validación AISLADO del módulo `autorreparacion`.
//!
//! NUNCA se compila el crate real aquí (2 CPU, 3 GB sin swap ⇒ el OOM
//! killer).  Este crate incluye los **MISMOS ficheros fuente** del repo
//! con `#[path]` y ** stubs** para los módulos que el daemon sí tiene.
//! Así se comprueba que el módulo nuevo TIPA y que sus tests PASSAN, sin
//! tocar el crate real.

#![allow(dead_code)]

// --- Stubs: lo que el daemon tiene y aqui se simula -----------------------
pub mod util_fs {
    use std::path::Path;
    pub fn write_atomic(_ruta: &Path, _datos: &[u8], _modo: u32) -> Result<(), String> {
        Ok(())
    }
    pub fn set_mode(_ruta: &Path, _modo: u32) {}
}

pub mod api_key {
    pub struct ApiKeyConfig;
    impl ApiKeyConfig {
        pub fn generate_and_store(_state_dir: &str) -> Result<String, String> {
            Ok("mdaemon_sk_live_0123456789abcdef0123456789abcdef".to_string())
        }
    }
}

pub mod cockpit_server {
    pub const EMBEDDED_TEMPLATE: &str = "<html></html>";
    pub fn state_dir() -> String {
        "/tmp/valida-state".to_string()
    }
}

pub mod supervisor {
    pub const WATCHDOG_SH: &str = "#!/system/bin/sh\n# supervisor\n";
    pub const KEEPER_SH: &str = "#!/system/bin/sh\n# keeper\n";
    pub fn es_android() -> bool {
        false
    }
}

pub mod config_efectiva {
    pub fn u64_de(_ruta: &str, defecto: u64) -> u64 {
        defecto
    }
    pub fn bool_de(_ruta: &str, defecto: bool) -> bool {
        defecto
    }
}

pub mod battery_advanced {
    pub fn modo_ahorro_activo() -> bool {
        false
    }
}

pub mod self_update {
    use std::path::Path;
    pub fn sha256_de_fichero(_ruta: &Path) -> Option<String> {
        Some("sha-de-disco".to_string())
    }
    pub fn sha_de_arranque() -> &'static str {
        "sha-en-servicio"
    }
}

pub mod channel {
    use serde_json::{json, Value};
    pub fn probe_url(_url: &str) -> (u16, bool) {
        (200, true)
    }
    pub fn es_canal_muerto(code: u16) -> bool {
        matches!(code, 0 | 502 | 503 | 504 | 530)
    }
    // MEDIDO (2026-11-28): el stub debe ser FIEL al crate real: leer del
    // DISCO (no devolver un JSON de mentira).  Con el stub mentiroso el
    // modulo nunca veia el "canal roto" => el falso positivo no se
    // detectaba en el banco.
    pub fn read_channel_json(state_dir: Option<&str>) -> Option<Value> {
        let base = state_dir
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "/tmp/valida-state".to_string());
        let raw = std::fs::read_to_string(std::path::PathBuf::from(base).join("channel.json")).ok()?;
        serde_json::from_str::<Value>(&raw).ok()
    }
    // MEDIDO (2026-11-28): el modulo `autorreparacion` lee el canal por la
    // abstraccion `sis` (no por el disco real) => el stub necesita la ruta.
    pub const CHANNEL_JSON: &str = "channel.json";
    pub fn channel_path(state_dir: Option<&str>, name: &str) -> std::path::PathBuf {
        let base = state_dir
            .map(|s| s.to_string())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "/tmp/valida-state".to_string());
        std::path::PathBuf::from(base).join(name)
    }
    pub fn publish(
        _state_dir: Option<&str>,
        _quick: Option<&str>,
        _duckdns: Option<&str>,
        _ip: Option<&str>,
        _pids: Option<&str>,
    ) -> Value {
        json!({"ok": true, "published": true})
    }
}

pub mod telemetria {
    use serde_json::{json, Value};
    use std::path::PathBuf;
    pub fn publicar(_state_dir: Option<&str>) -> Result<Value, String> {
        Ok(json!({"ok": true}))
    }
    pub fn ruta_indice(state_dir: Option<&str>) -> PathBuf {
        let base = state_dir.unwrap_or("/tmp/valida-state");
        PathBuf::from(base).join("telemetria").join("indice.json")
    }
}

pub mod coordination {
    pub struct TaskBoard;
    impl TaskBoard {
        pub fn open(_path: &str) -> Result<TaskBoard, String> {
            Ok(TaskBoard)
        }
    }
}

pub mod webui {
    pub fn coord_board_dirs() -> Vec<String> {
        vec!["/tmp/valida-state".to_string()]
    }
}

pub mod stack {
    use serde_json::{json, Value};

    #[derive(Debug, Clone)]
    pub struct Config {
        pub prefijo: String,
        pub android: bool,
    }
    impl Config {
        pub fn del_entorno() -> Config {
            Config {
                prefijo: "/data/adb/megadaemon".to_string(),
                android: false,
            }
        }
    }

    #[derive(Debug, Clone, Default)]
    pub struct Informe {
        pub escritos: Vec<String>,
        pub fallidos: Vec<(String, String)>,
    }

    pub fn reconciliar(_cfg: &Config) -> Informe {
        Informe {
            escritos: vec!["99-megadaemon.sh reescrito".to_string()],
            fallidos: vec![],
        }
    }

    pub fn estado_json(_cfg: &Config) -> Value {
        json!({"desfasados": 0})
    }
}

// --- Los MISMOS ficheros fuente del repo ----------------------------------
#[path = "../../../rust-core/src/reparar_entorno.rs"]
pub mod reparar_entorno;

#[path = "../../../rust-core/src/autorreparacion.rs"]
pub mod autorreparacion;
