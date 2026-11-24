//! `md-telecom` — CLI de telefonía del propio usuario: **OTP del buzón**,
//! **filtro antispam** y **simulación de la política de reintento**.
//!
//! ```text
//! md-telecom --help
//! md-telecom otp      --file <dump> [--min-confidence 0.5] [--json]
//! md-telecom antispam --file <dump> [--contacts <fichero>] [--state-dir <dir>] [--json]
//! md-telecom call     --number <n> [--simulate busy|no_answer|answered] [--attempts 3]
//! ```
//!
//! ⚠️ **Fuente de verdad única**: este binario **no reimplementa** nada;
//! delega en `crate::telecom`, `crate::otp` y `crate::antispam` (los mismos
//! módulos que usan los tests del repo) ⇒ la CLI y la librería no pueden
//! divergir.
//!
//! ## Dónde sale el buzón (Termux, en el móvil)
//!
//! ```bash
//! su -mm -c 'content query --uri content://sms/inbox --projection _id:address:body:date' > inbox.txt
//! ```
//!
//! Después, en el PC: `md-telecom otp --file inbox.txt`.
//!
//! ## Privacidad
//!
//! Los códigos se imprimen **sólo en la stdout de esta CLI local**: no se
//! mandan a ningún canal compartido, ni al tablón, ni a un chat. El usuario
//! los ve en su pantalla y los usa.
//!
//! Formato de entrada aceptado:
//! * **dump de `content query`** (el de arriba), una fila por línea;
//! * **JSON**: un array de `{id, sender, body, timestamp}`.

use megadaemon_core::antispam::{classify_sms, InboundSms, ScanContext, SpamLists, SpamVerdict};
use megadaemon_core::otp::{extract_otp_in_inbox, OtpCandidate, OtpMessage};
use megadaemon_core::telecom::{
    call_with_retry, parse_content_sms, CallOutcome, CallStatus, RetryCondition, RetryPolicy,
    SimClock, SimTelecom,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;

/// Texto de ayuda.
const AYUDA: &str = "\
md-telecom — OTP del buzon propio, filtro antispam entrante y politica de reintento

USO:
  md-telecom otp      --file <fichero> [--min-confidence <0..1>] [--json]
  md-telecom antispam --file <fichero> [--contacts <fichero>] [--state-dir <dir>] [--json]
  md-telecom call     --number <numero> [--simulate busy|no_answer|answered|failed]
                       [--attempts N] [--initial-delay-ms N] [--max-delay-ms N]
  md-telecom --help

ENTRADA (--file):
  - dump de 'content query' (una fila por linea):
      Row: 0 _id=3, address=+34600111222, body=hola, date=1712345678000
  - JSON: array de {id, sender, body, timestamp}

NOTAS:
  - Los codigos OTP se imprimen SOLO en la stdout local: no van a ningun canal.
  - 'otp' NO inventa nada: sin palabra clave el codigo sale con confianza baja
    y marcado como dudoso; los telefonos y los numeros de pedido se descartan.
  - 'antispam' es DEFENSIVO: clasifica lo que entra (accept/quarantine/block).
  - 'call' NO marca: simula la politica de reintento y ensena el registro.
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{AYUDA}");
        std::process::exit(if args.is_empty() { 1 } else { 0 });
    }

    let sub = args[0].clone();
    let opts = parse_opts(&args[1..]);

    let code = match sub.as_str() {
        "otp" => cmd_otp(&opts),
        "antispam" => cmd_antispam(&opts),
        "call" => cmd_call(&opts),
        otra => {
            eprintln!("subcomando desconocido: {otra}\n");
            eprint!("{AYUDA}");
            2
        }
    };
    std::process::exit(code);
}

/// Parsea `--clave valor` y `--clave=valor` en un mapa.
fn parse_opts(args: &[String]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut i = 0usize;
    while i < args.len() {
        let a = &args[i];
        if let Some(rest) = a.strip_prefix("--") {
            if let Some((k, v)) = rest.split_once('=') {
                out.insert(k.to_string(), v.to_string());
                i += 1;
                continue;
            }
            let value = args.get(i + 1).cloned().unwrap_or_default();
            if !value.starts_with("--") {
                out.insert(rest.to_string(), value);
                i += 2;
                continue;
            }
            out.insert(rest.to_string(), String::new());
        }
        i += 1;
    }
    out
}

/// ¿Está puesta la bandera `--json`?
fn flag_json(args: &[String]) -> bool {
    args.iter().any(|a| a == "--json")
}

/// Lee el fichero de entrada (o stdin si es `-`).
fn leer_entrada(opts: &HashMap<String, String>) -> Result<String, String> {
    match opts.get("file").map(|s| s.as_str()) {
        None | Some("-") => {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
                .map_err(|e| format!("leyendo stdin: {e}"))?;
            Ok(buf)
        }
        Some(path) => fs::read_to_string(path).map_err(|e| format!("leyendo {path}: {e}")),
    }
}

/// Convierte la entrada en mensajes del buzón.
fn parse_mensajes(raw: &str) -> Result<Vec<OtpMessage>, String> {
    let trimmed = raw.trim_start();
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        // JSON: array de {id, sender, body, timestamp}
        let v: Value = serde_json::from_str(raw).map_err(|e| format!("JSON inválido: {e}"))?;
        let arr = v
            .as_array()
            .ok_or("el JSON debe ser un array de mensajes")?;
        let mut out = Vec::new();
        for (i, item) in arr.iter().enumerate() {
            out.push(OtpMessage::new(
                item.get("id").and_then(|x| x.as_str()).unwrap_or_default(),
                item.get("sender")
                    .or_else(|| item.get("address"))
                    .and_then(|x| x.as_str())
                    .unwrap_or_default(),
                item.get("body")
                    .and_then(|x| x.as_str())
                    .unwrap_or_default(),
                item.get("timestamp").and_then(|x| x.as_i64()).unwrap_or(0),
            ));
            let _ = i;
        }
        return Ok(out);
    }
    // Dump de `content query`
    let msgs = parse_content_sms(raw, usize::MAX);
    Ok(msgs
        .iter()
        .map(|m| OtpMessage::new(&m.id, &m.sender, &m.body, m.timestamp))
        .collect())
}

/// `md-telecom otp`
fn cmd_otp(opts: &HashMap<String, String>) -> i32 {
    let raw = match leer_entrada(opts) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    let msgs = match parse_mensajes(&raw) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    let min = opts
        .get("min-confidence")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.0);
    let cands: Vec<OtpCandidate> = extract_otp_in_inbox(&msgs)
        .into_iter()
        .filter(|c| c.confidence >= min)
        .collect();

    if flag_json(&std::env::args().collect::<Vec<String>>()) {
        let out: Vec<Value> = cands
            .iter()
            .map(|c| {
                json!({
                    "value": c.value,
                    "confidence": c.confidence,
                    "confidence_label": c.confidence_label(),
                    "dubious": c.dubious,
                    "reason": c.reason,
                    "message_id": c.message_id,
                    "sender": c.sender,
                    "timestamp": c.timestamp,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    } else {
        if cands.is_empty() {
            println!("(sin codigos OTP por encima de la confianza {min})");
        }
        for c in &cands {
            println!(
                "{}  confianza {:.2} ({}){}",
                c.value,
                c.confidence,
                c.confidence_label(),
                if c.dubious { "  [DUDOSO]" } else { "" }
            );
            println!("    {}", c.reason);
        }
    }
    0
}

/// Recorta a `n` **caracteres** sin partir un UTF-8 por la mitad.
///
/// Un `&s[..n]` a pelo revienta (panic) si `n` cae dentro de un carácter
/// multibyte: un remitente o un cuerpo con acentos o emoji lo harían.
fn recortar(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut out: String = s.chars().take(n).collect();
    out.push('…');
    out
}

/// `md-telecom antispam`
fn cmd_antispam(opts: &HashMap<String, String>) -> i32 {
    let raw = match leer_entrada(opts) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    let msgs = match parse_mensajes(&raw) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };

    let mut ctx = match opts.get("state-dir") {
        Some(dir) => ScanContext::with_lists(SpamLists::load(dir)),
        None => ScanContext::new(),
    };
    if let Some(path) = opts.get("contacts") {
        if let Ok(txt) = fs::read_to_string(path) {
            for line in txt.lines() {
                let n = line.trim();
                if !n.is_empty() && !n.starts_with('#') {
                    ctx.add_contact(n);
                }
            }
        }
    }
    // Historial = los propios mensajes (para la regla de frecuencia).
    let history: Vec<InboundSms> = msgs
        .iter()
        .map(|m| InboundSms::new(&m.sender, &m.body, m.timestamp))
        .collect();
    ctx.history = history;

    let veredictos: Vec<SpamVerdict> = msgs
        .iter()
        .map(|m| {
            let sms = InboundSms::new(&m.sender, &m.body, m.timestamp);
            classify_sms(&sms, &ctx)
        })
        .collect();

    if flag_json(&std::env::args().collect::<Vec<String>>()) {
        let out: Vec<Value> = msgs
            .iter()
            .zip(veredictos.iter())
            .map(|(m, v)| {
                json!({
                    "id": m.id,
                    "sender": m.sender,
                    "body": m.body,
                    "timestamp": m.timestamp,
                    "decision": v.decision.label(),
                    "total": v.score.total,
                    "signals": v.score.signals.iter().map(|s| json!({
                        "rule": s.rule, "weight": s.weight, "detail": s.detail
                    })).collect::<Vec<Value>>(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
    } else {
        for (m, v) in msgs.iter().zip(veredictos.iter()) {
            println!(
                "{:<10} {:<22} {:.2}  {}",
                v.decision.label(),
                recortar(&m.sender, 22),
                v.score.total,
                recortar(&m.body, 60)
            );
            for s in &v.score.signals {
                println!("             {:+.2} {:<20} {}", s.weight, s.rule, s.detail);
            }
        }
    }
    0
}

/// `md-telecom call` — simulación de la política de reintento (NO marca).
fn cmd_call(opts: &HashMap<String, String>) -> i32 {
    let numero = match opts.get("number") {
        Some(n) if !n.is_empty() => n.clone(),
        _ => {
            eprintln!("falta --number");
            return 2;
        }
    };
    let policy = RetryPolicy {
        attempts: opts
            .get("attempts")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(3),
        initial_delay_ms: opts
            .get("initial-delay-ms")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5_000),
        max_delay_ms: opts
            .get("max-delay-ms")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60_000),
        retry_on: vec![
            RetryCondition::Busy,
            RetryCondition::NoAnswer,
            RetryCondition::TemporaryFailure,
        ],
        backoff_factor: 2,
    };

    // Proveedor simulado: el guion decide lo que devuelve cada intento.
    let prov = SimTelecom::new();
    let guion = opts
        .get("simulate")
        .cloned()
        .unwrap_or_else(|| "busy".to_string());
    for _ in 0..policy.attempts {
        let (status, detalle) = match guion.as_str() {
            "answered" => (CallStatus::Answered, "contestada"),
            "no_answer" => (CallStatus::NoAnswer, "sin respuesta"),
            "failed" => (CallStatus::Failed, "fallo temporal"),
            _ => (CallStatus::Busy, "linea ocupada"),
        };
        prov.push_call(CallOutcome::new(
            &numero,
            status,
            status == CallStatus::Answered,
            detalle,
        ));
    }

    let mut clock = SimClock::new(1_700_000_000_000);
    let log = call_with_retry(&prov, &numero, &policy, &mut clock);

    if flag_json(&std::env::args().collect::<Vec<String>>()) {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "number": log.number,
                "final_status": log.final_status.map(|s| s.label()),
                "attempts": log.attempts.len(),
                "total_wait_ms": log.total_wait_ms,
                "waits_ms": clock.sleeps,
                "retried": log.retried,
                "detail": log.attempts.iter().map(|a| json!({
                    "attempt": a.attempt,
                    "status": a.status.label(),
                    "wait_before_ms": if a.attempt > 0 { policy.delay_ms(a.attempt - 1) } else { 0 },
                    "retry_reason": a.retry_reason,
                })).collect::<Vec<Value>>(),
            }))
            .unwrap_or_default()
        );
    } else {
        println!("numero:      {}", log.number);
        println!(
            "estado final: {}",
            log.final_status.map(|s| s.label()).unwrap_or("—")
        );
        println!("intentos:    {}", log.attempts.len());
        println!("espera total: {} ms", log.total_wait_ms);
        println!("esperas:     {:?}", clock.sleeps);
        for a in &log.attempts {
            println!(
                "  #{} {} -> {}",
                a.attempt,
                a.status.label(),
                a.retry_reason.clone().unwrap_or_else(|| "—".to_string())
            );
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_opts_acepta_guiones_y_igual() {
        let args: Vec<String> = ["--file", "x.txt", "--min-confidence=0.5", "--json"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let o = parse_opts(&args);
        assert_eq!(o.get("file").map(|s| s.as_str()), Some("x.txt"));
        assert_eq!(o.get("min-confidence").map(|s| s.as_str()), Some("0.5"));
        assert!(o.contains_key("json"));
    }

    #[test]
    fn parse_mensajes_acepta_el_dump_de_content_query() {
        let raw =
            "Row: 0 _id=3, address=+34600111222, body=Tu código es 123456, date=1712345678000";
        let m = parse_mensajes(raw).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].id, "3");
        assert_eq!(m[0].sender, "+34600111222");
        assert!(m[0].body.contains("123456"));
    }

    #[test]
    fn parse_mensajes_acepta_json() {
        let raw =
            r#"[{"id":"7","sender":"BANCO","body":"código 654321","timestamp":1712345678000}]"#;
        let m = parse_mensajes(raw).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].sender, "BANCO");
        assert_eq!(m[0].timestamp, 1_712_345_678_000);
    }

    #[test]
    fn el_json_invalido_da_error_y_no_panic() {
        assert!(parse_mensajes("[{\"id\":}").is_err());
    }

    #[test]
    fn recortar_no_parte_un_utf8_por_la_mitad() {
        // Con acentos y emoji: un `&s[..n]` a pelo reventaría.
        assert_eq!(recortar("áéíóú", 3), "áéí…");
        assert_eq!(recortar("hola", 10), "hola");
        assert_eq!(recortar("👍👍👍", 2), "👍👍…");
    }
}
