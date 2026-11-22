//! `md-backup`: el CLI del motor de backup nativo de apps.
//!
//! El código del CLI **no está duplicado**: el fichero del crate principal
//! (`rust-core/src/bin/md-backup.rs`) se compila **como módulo** aquí (con
//! `#[path]`) ⇒ lo que se prueba en el móvil es exactamente lo que va en el
//! crate grande del daemon.
//!
//! La lib de este crate se llama `megadaemon_core` (igual que la del crate
//! principal) ⇒ los `use megadaemon_core::...` del CLI resuelven sin tocar una
//! línea del CLI.

#[path = "../../src/bin/md-backup.rs"]
mod cli;

fn main() -> std::process::ExitCode {
    cli::main()
}
