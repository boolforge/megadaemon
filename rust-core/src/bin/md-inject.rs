//! `md-inject` — inyector `ptrace` para el móvil.
//!
//! Se ejecuta **en el móvil** y **como root** (`su -mm -c`): el proceso
//! objetivo (Antigravity) pertenece a otra aplicación, así que sin root no
//! hay `ptrace`.
//!
//! ## Por qué un binario aparte y no una tool del daemon
//!
//! El daemon corre como usuario normal y usa `su -mm -c` para lo que necesita
//! root.  `ptrace` necesita el **permiso del kernel** (CAP_SYS_PTRACE +
//! `SELinux`), y en Android ese permiso se pide desde el dominio `magisk`:
//! ejecutarlo dentro del daemon (dominio de la app) fallaría.  Por eso el
//! inyector es un binario que el daemon **copia al móvil y ejecuta con
//! `root_exec`**.
//!
//! ## Uso
//!
//! ```text
//! md-inject list [filtro]                     lista los procesos (JSON)
//! md-inject status <pid>                      ¿está cargada la .so? ¿hay hooks?
//! md-inject find-dlopen <pid>                 dirección de dlopen en el objetivo
//! md-inject inject <pid|proceso> <ruta.so> [--mem=ptrace|mem]
//! ```
//!
//! Toda la salida es **JSON por stdout** (el daemon la parsea) y los errores
//! también salen en JSON (`{"ok":false,"error":"..."}`) con el `errno` cuando
//! lo hay.

use megadaemon_core::inject;
use serde_json::{json, Value};

/// Uso por pantalla.
const USO: &str = "\
md-inject — inyector ptrace (carga una .so en un proceso vivo)

  md-inject list [filtro]
  md-inject status <pid>
  md-inject find-dlopen <pid>
  md-inject inject <pid|proceso> <ruta.so> [--mem=ptrace|mem]
  md-inject ayuda
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let codigo = ejecutar(&args);
    std::process::exit(codigo);
}

/// Ejecuta la orden y devuelve el código de salida (0 = todo bien).
fn ejecutar(args: &[String]) -> i32 {
    if args.is_empty() {
        println!("{}", json!({"ok": false, "error": "faltan argumentos", "uso": USO}));
        return 2;
    }
    match args[0].as_str() {
        "list" | "lista" | "ps" => listar(&args[1..]),
        "status" | "estado" => estado(&args[1..]),
        "find-dlopen" => buscar_dlopen(&args[1..]),
        "inject" | "inyectar" => inyectar(&args[1..]),
        "ayuda" | "help" | "--help" | "-h" => {
            println!("{}", json!({"ok": true, "uso": USO}));
            0
        }
        otra => {
            println!(
                "{}",
                json!({"ok": false, "error": format!("orden desconocida: {}", otra), "uso": USO})
            );
            2
        }
    }
}

/// `list [filtro]`
fn listar(args: &[String]) -> i32 {
    let filtro = args.first().map(|s| s.as_str()).unwrap_or("");
    let procs = inject::listar_procesos(filtro);
    let lista: Vec<Value> = procs
        .iter()
        .map(|p| json!({"pid": p.pid, "nombre": p.nombre, "cmdline": p.cmdline}))
        .collect();
    println!(
        "{}",
        json!({"ok": true, "filtro": filtro, "procesos": lista, "total": lista.len()})
    );
    0
}

/// `status <pid>`: ¿está la `.so` cargada? ¿qué hay en el NDJSON de hooks?
fn estado(args: &[String]) -> i32 {
    let pid = match args.first().and_then(|s| s.parse::<i32>().ok()) {
        Some(p) => p,
        None => {
            println!("{}", json!({"ok": false, "error": "hace falta el PID"}));
            return 2;
        }
    };
    let cmdline = inject::leer_cmdline(pid).unwrap_or_default();
    let mapas = inject::leer_maps(pid);
    let libs: Vec<String> = {
        let mut v: Vec<String> = Vec::new();
        for m in &mapas {
            if m.ruta.is_empty() || v.contains(&m.ruta) {
                continue;
            }
            // Sólo las que "suenan" a payload de hooks.
            if m.ruta.contains("hooks") || m.ruta.contains("megadaemon") {
                v.push(m.ruta.clone());
            }
        }
        v
    };
    println!(
        "{}",
        json!({
            "ok": true,
            "pid": pid,
            "cmdline": cmdline,
            "payload_cargado": libs,
            "cargado": !libs.is_empty(),
            "ndjson_hooks": inject::NDJSON_HOOKS,
            "nota": "los hooks se escriben en el NDJSON del payload; el daemon lo lee con la tool inject_hooks",
        })
    );
    0
}

/// `find-dlopen <pid>`: dónde está `dlopen` en el objetivo (diagnóstico).
fn buscar_dlopen(args: &[String]) -> i32 {
    let pid = match args.first().and_then(|s| s.parse::<i32>().ok()) {
        Some(p) => p,
        None => {
            println!("{}", json!({"ok": false, "error": "hace falta el PID"}));
            return 2;
        }
    };
    match inject::resolver_dlopen(pid) {
        Ok(d) => {
            println!("{}", json!({"ok": true, "pid": pid, "dlopen": format!("0x{:x}", d)}));
            0
        }
        Err(e) => {
            println!("{}", json!({"ok": false, "pid": pid, "error": e}));
            1
        }
    }
}

/// `inject <pid|proceso> <ruta.so> [--mem=...]`
fn inyectar(args: &[String]) -> i32 {
    let mut posicionales: Vec<&str> = Vec::new();
    let mut via = inject::ViaMemoria::Ptrace;
    for a in args {
        if let Some(v) = a.strip_prefix("--mem=") {
            match inject::ViaMemoria::desde_texto(v) {
                Some(x) => via = x,
                None => {
                    println!(
                        "{}",
                        json!({"ok": false, "error": format!("vía de memoria desconocida: {}", v)})
                    );
                    return 2;
                }
            }
        } else {
            posicionales.push(a.as_str());
        }
    }
    if posicionales.len() < 2 {
        println!(
            "{}",
            json!({"ok": false, "error": "uso: md-inject inject <pid|proceso> <ruta.so>"})
        );
        return 2;
    }
    let objetivo = posicionales[0];
    let biblioteca = posicionales[1];

    let pid = match inject::resolver_pid(objetivo) {
        Ok(p) => p,
        Err(e) => {
            println!("{}", json!({"ok": false, "error": e}));
            return 1;
        }
    };
    let informe = inject::inyectar(pid, biblioteca, via);
    let ok = informe.ok;
    let texto = serde_json::to_string_pretty(&informe).unwrap_or_else(|_| "{}".to_string());
    println!("{}", texto);
    if ok {
        0
    } else {
        1
    }
}
