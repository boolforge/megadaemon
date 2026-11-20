//! Imprime el plan de subida de un fichero a Drive (trozos + comandos curl).
//!
//! Uso (en el móvil o en el sandbox):
//!   cargo run --example drive_plan -- <ruta> <folder_id> [chunk_bytes]
//!
//! La credencial **no** se imprime ni se guarda: el ejemplo genera las
//! peticiones con un token de relleno para enseñar la forma exacta.

#[path = "../src/drive_client.rs"]
mod drive_client;

use drive_client::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).cloned().unwrap_or_else(|| "/data/media/0/ejemplo.mp4".into());
    let folder = args.get(2).cloned().unwrap_or_default();
    let chunk: u64 = args
        .get(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(8 * 1024 * 1024);

    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "fichero".into());

    let creds = DriveCredentials {
        access_token: "<TOKEN-DEL-ALMACEN-DE-SECRETOS>".to_string(),
    };
    let target = DriveTarget::folder(&folder);

    println!("fichero : {path} ({size} bytes)");
    println!("trozo   : {chunk} bytes (múltiplo de 256 KiB: {})", chunk % DRIVE_CHUNK_GRANULARITY == 0);
    let plan = chunk_plan(size, chunk);
    println!("trozos  : {}", plan.len());
    println!("\n-- abrir sesion --");
    println!("{}", open_upload_session(&creds, &name, size, &target).curl_command(None));
    let first = plan.first();
    if let Some(r) = first {
        println!("\n-- primer trozo --");
        println!(
            "{}",
            upload_chunk_request(&creds, "<URL-DE-LA-SESION>", r, size).curl_command(Some(&path))
        );
    }
    println!("\n-- listado --");
    for (i, r) in plan.iter().take(5).enumerate() {
        println!("  {i}: bytes {}-{} ({} bytes)", r.start, r.end, r.len());
    }
    if plan.len() > 5 {
        println!("  ... (+{} trozos)", plan.len() - 5);
    }
}
