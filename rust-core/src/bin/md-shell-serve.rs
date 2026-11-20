//! F3 — servicio de **shell root persistente**.
//!
//! El bucle agéntico no puede abrir un `su` por llamada: en el móvil cada
//! llamada crea 2+ procesos y la máquina va al límite (load 17,65 · 876
//! procesos).  Este binario levanta **un** shell root vivo y habla con él por
//! un socket unix (0600), de modo que las llamadas del bucle cuestan **0
//! procesos**.
//!
//! Uso (en el móvil, con root):
//! ```sh
//! su -mm -c '/data/local/tmp/megadaemon/md-shell-serve \
//!             /data/local/tmp/megadaemon/state/shell.sock'
//! ```
//!
//! El supervisor (`99-megadaemon.sh`) es quien lo relanza si muere.

use megadaemon_core::persistent_shell::PersistentShell;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

/// Ruta por defecto del socket.
pub const DEFAULT_SOCKET: &str = "/data/local/tmp/megadaemon/state/shell.sock";
/// Comando con el que se arma el shell root en el móvil.
pub const DEFAULT_SHELL_CMD: &str = "su -mm -c sh -s";

fn main() {
    let mut args = std::env::args().skip(1);
    let socket = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SOCKET));
    let shell_cmd = args.next().unwrap_or_else(|| DEFAULT_SHELL_CMD.to_string());

    let shell = match PersistentShell::start(&socket, &shell_cmd) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("md-shell-serve: no se pudo arrancar el shell: {}", e);
            std::process::exit(1);
        }
    };
    eprintln!(
        "md-shell-serve: shell persistente en {} (cmd: {})",
        socket.display(),
        shell_cmd
    );

    // El servicio vive hasta que lo maten: el supervisor lo vigila.
    loop {
        thread::sleep(Duration::from_secs(30));
        if shell.is_broken() {
            // El shell interno murio (timeout): se rearma para que el proximo
            // cliente no encuentre el socket dead.
            if let Err(e) = shell.rearm() {
                eprintln!("md-shell-serve: rearme fallo: {}", e);
            }
        }
    }
}
