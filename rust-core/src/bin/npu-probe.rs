//! `npu-probe` — CLI de campo para sondear los ioctl **VS4L** del driver NPU
//! del Exynos 2100 (`/dev/vertex10`).
//!
//! ```text
//! npu-probe --vs4l [--device /dev/vertex10] [--pretty]
//! ```
//!
//! ⚠️ **Fuente de verdad única**: este binario **no implementa el sondeo**;
//! delega en `crate::npu_vs4l` (el mismo módulo que usa la tool MCP
//! `npu_vs4l_probe`). Así la CLI de campo —que corre **sin daemon**— y la
//! tool MCP devuelven **exactamente lo mismo** y no pueden divergir.
//!
//! · El **método** (emitir el ioctl **sin argumento** ⇒ el driver devuelve el
//!   tamaño que espera ⇒ ése es el test de existencia) está documentado en el
//!   módulo: `EINVAL` **no** significa "no existe" (significa "reconocido pero
//!   argumento inválido"); con buffer se_falsearía.
//! · Es **sólo lectura**: no se emiten los ioctl de tipo `SET` (alterarían el
//!   estado del NPU), por eso **no hay modo `--deep`**.
//! · La tabla de códigos es `crate::dex_decompile::VS4L_IOCTLS` (medida en
//!   vivo el 2026-11-20 en el S21 Ultra).
//!
//! Salida: **JSON por stdout**, resumen legible por stderr. Código de salida
//! 0 si el driver vivo coincide con la tabla, 1 si no.

use megadaemon_core::dex_decompile::{VS4L_IOCTLS, VS4L_LIVE_CODES};
use megadaemon_core::npu_vs4l;
use serde_json::{json, Value};
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 0;
    while i < args.len() {
        if args[i] == name && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        if let Some(v) = args[i].strip_prefix(&format!("{}=", name)) {
            return Some(v.to_string());
        }
        i += 1;
    }
    None
}

fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

/// Sondea un dispositivo alternativo usando el **módulo** (misma lógica).
fn probe_device(path: &str) -> Option<Value> {
    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .or_else(|_| OpenOptions::new().read(true).open(path))
        .ok()?;
    let fd = f.as_raw_fd();
    let mut codes: Vec<Value> = Vec::new();
    let mut live = 0u32;
    for (name, code, _nr, _size, note) in VS4L_IOCTLS.iter() {
        let (is_live, driver_size, errno) = npu_vs4l::probe_code(fd, *code);
        if is_live {
            live += 1;
        }
        codes.push(json!({
            "name": name,
            "code": format!("0x{:08x}", code),
            "code_dec": code,
            "macro": npu_vs4l::ioc_macro(*code),
            "nr": npu_vs4l::ioc_nr(*code),
            "size_in_code": npu_vs4l::ioc_size(*code),
            "live": is_live,
            "driver_size": driver_size,
            "size_match": match driver_size {
                Some(d) => Value::from(d as u16 == npu_vs4l::ioc_size(*code)),
                None => Value::Null,
            },
            "errno": errno,
            "errno_name": errno.map(npu_vs4l::errno_name).unwrap_or(""),
            "note": note,
        }));
    }
    Some(json!({
        "ok": true,
        "tool": "npu-probe",
        "mode": "no_arg",
        "node": path,
        "probed": codes.len(),
        "live": live,
        "expected_live": VS4L_LIVE_CODES,
        "matches_expected": live == VS4L_LIVE_CODES,
        "method": "ioctl(fd, code, NULL) => el driver devuelve el tamaño que espera (sólo lectura)",
        "codes": codes,
    }))
}

fn main() -> std::process::ExitCode {
    if !flag("--vs4l") {
        eprintln!("uso: npu-probe --vs4l [--device <ruta>] [--pretty]");
        eprintln!("  --device <ruta>  nodo del dispositivo (por defecto: /dev/vertex10)");
        eprintln!("  --pretty         JSON indentado");
        eprintln!("(la lógica de sondeo vive en crate::npu_vs4l, la misma que la tool MCP)");
        return std::process::ExitCode::from(2);
    }

    let pretty = flag("--pretty");
    let out: Value = match arg("--device") {
        Some(ruta) => match probe_device(&ruta) {
            Some(v) => v,
            None => json!({
                "ok": false,
                "error": "no_device",
                "detail": format!("no se pudo abrir {ruta}"),
                "hint": "hace falta root y que el nodo exista (/dev/vertex10, modo 0644)",
            }),
        },
        None => {
            // Informe del módulo (misma fuente que la tool MCP `npu_vs4l_probe`).
            let rep = npu_vs4l::probe();
            let mut v = serde_json::to_value(&rep).unwrap_or_else(|_| json!({}));
            if let Some(o) = v.as_object_mut() {
                o.insert("ok".to_string(), json!(rep.live > 0));
                o.insert("tool".to_string(), json!("npu-probe"));
                o.insert("mode".to_string(), json!("no_arg"));
            }
            v
        }
    };

    let texto = if pretty {
        serde_json::to_string_pretty(&out).unwrap_or_else(|_| out.to_string())
    } else {
        out.to_string()
    };
    println!("{texto}");

    let live = out.get("live").and_then(|v| v.as_u64()).unwrap_or(0);
    let esperado = out
        .get("expected_live")
        .and_then(|v| v.as_u64())
        .unwrap_or(VS4L_LIVE_CODES as u64);
    eprintln!("npu-probe: {live}/{esperado} ioctl vivos");

    let ok = out.get("ok").and_then(|v| v.as_bool()) == Some(true);
    std::process::ExitCode::from(if ok && live == esperado { 0 } else { 1 })
}
