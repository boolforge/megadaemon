// El binario es un envoltorio fino: toda la lógica vive en la lib
// (megadaemon_core), que es lo que permite `cargo test`.
use megadaemon_core::*;

use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};

fn print_usage() {
    eprintln!("MegaDaemon Native Rust Core v13.1 Zenith (Pure Rust ARM64 + Exynos 2100 NPU + LTO-Decomp v3.4 CIRX)");
    eprintln!("Usage: megadaemon-core <command> [args...]");
    eprintln!("Commands:");
    eprintln!("  cockpit [port]                              Launch 100% Pure-Rust Multi-Threaded Cockpit HTTP Server (default: 3000)");
    eprintln!("  mcp-server                                  Run Antigravity JSON-RPC 2.0 MCP Server over stdio (MiniMax + RE Tools)");
    eprintln!("  chat <model_id> <prompt>                    Query any catalog provider (MiniMax M3.1-Flash/M3, Groq, OpenRouter, Google AI Studio, ...) or Local SEERE");
    eprintln!("  die-binwalk <file>                          Detect It Easy + Binwalk + Scalpel + Bulk Extractor + Comtype Scanner");
    eprintln!("  cirx-ssa [mas]                              LTO-Decomp v3.4 CIRX Micro-RTL SSA, Risk Score S/R & SMT-LIB2 Export");
    eprintln!("  mba <expr>                                  D810/Goomba Mixed Boolean-Arithmetic (MBA) Algebraic Simplifier");
    eprintln!("  kaitai <file> [template] [offset]           ImHex/Kaitai Struct Declarative Binary Parser");
    eprintln!(
        "  rizin <file> [mode] [arch]                  Native ARM64 Rizin 0.9.1 Multi-OS Inspector"
    );
    eprintln!("  frida <mode> <module> <sym_or_off> [proc]   Frida DBI Script Generator (attach|stalker|native_call|android_jni|tenet_mem)");
    eprintln!("  npu-cv                                      Exynos 2100 NPU + Mali-G78 + VLA Computer Vision ROI Telemetry");
    eprintln!("  unifier                                     Scan & Unify All 9 Projects Across the Device Ecosystem");
    eprintln!("  rea <plat> [query] [max]                    REA Ghidra 2.7MB C Corpus Indexer & ScummVM C++ Transpiler");
    eprintln!("  cfg <plat> [offset] [count]                 Native Control Flow Graph (CFG) Lifter & Mermaid Generator");
    eprintln!(
        "  bindiff <plat>                              Multi-ISA Cross-Platform Function Matcher"
    );
    eprintln!("  hw-mmio <plat>                              Hardware MMIO Register Annotator & Auto-Struct Reconstructor");
    eprintln!("  crypto-scan <plat>                          Shift-JIS String Decoder & Konami LZSS/RLE Scanner");
    eprintln!("  seere <query> [--max-turns N] [--env-in-prompt]   Execute SEERE v13.1 GBNF Local Harness (0 tokens if possible)");
    eprintln!("  seere-prompt [es|en]                         Dump the static SEERE system prompt (for token accounting)");
    eprintln!("  seere-grammar                                Dump the active GBNF grammar");
    eprintln!("  power                                        Show battery/charging/thermal state and the active policy");
    eprintln!("  seere-selftest                              Run SEERE v12 Assertion Suites & Export Shared GBNF/Prompts");
    eprintln!("  npu-probe                                   Probe Exynos 2100 NPU (/dev/vertex10, EDEN, KleidiAI) & Pin Cores 4-7");
    eprintln!("  npu-probe --vs4l                            Sondea el driver VS4L/NPU (ioctl SIN argumento => devuelve el tamano que espera)");
    eprintln!("  csc-verify                                Verifica en el movil si la feature de grabacion nativa (CSC) esta activa");
    eprintln!("  rootcheck <paquete>[,...]                 Banco de pruebas de ocultacion de root: veredicto + evidencia");
    eprintln!("  rootcheck --state                        Que senales de root quedan a la vista ahora mismo");
    eprintln!(
        "  rootcheck --exec <paquete>               Arranca la app y captura su logcat filtrado"
    );
    eprintln!("  magisk list|enable|disable|install       Gestion real de modulos Magisk (install valida el ZIP)");
    eprintln!("  magisk shamiko [<zip>]                   Ficha de la release oficial + estado real en el movil");
    eprintln!("  locate-roms [refresh]                       Native Multi-Platform ROM & Binary Auto-Discovery");
    eprintln!("  disasm <plat> <file> [offset] [count]       Native Multi-Arch Disassembler");
    eprintln!("  hex <file> [offset] [length]                Native Hex & ASCII Inspector");
    eprintln!(
        "  deobfuscate <plat> <file>                   Native Konami XOR Script Deobfuscator"
    );
    eprintln!("  dumper <plat> <file>                        Native Asset Signature Extractor");
    eprintln!("  entropy <file> [block_size]                 Shannon Entropy Block Classifier");
    eprintln!("  pattern <file> <pattern_str> [max_matches]  Wildcard AOB Pattern Scanner");
    eprintln!("  pointers <file> <base_hex> [is_be] [max]    32-bit Pointer Table Miner");
    eprintln!("  diff <file_a> <file_b> [base_hex] [max]     Memory Snapshot Differential Engine");
    eprintln!("  shm-status                                  Zero-Copy VLA SHM Bus Header Status");
    eprintln!(
        "  shm-frame-bmp                               Render Live 24-bit BMP Frame from SHM"
    );
}

/// D-10 (P0): hook de panic con backtrace.
///
/// El release usa `panic = "unwind"` (v15), asi que un panic en un hilo de
/// peticion lo captura `catch_unwind` (=> la peticion responde 500 y el
/// daemon sigue) pero un panic en un hilo de fondo **mata el proceso**
/// (=> el "134" que se vio en el movil, que NO puede ser un panic Rust).
/// Con este hook cada panic deja rastro real: mensaje, hilo y backtrace.
///
/// El backtrace se captura SIEMPRE (no solo con RUST_BACKTRACE=1): en el
/// movil nadie pone esa variable y sin esto el 134 es indescifrable.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let hilo = std::thread::current()
            .name()
            .unwrap_or("<sin nombre>")
            .to_string();
        let backtrace = std::backtrace::Backtrace::force_capture();
        eprintln!(
            "[megadaemon][PANIC] hilo={} panic={}\n{}",
            hilo, info, backtrace
        );
    }));
}

fn main() {
    install_panic_hook();
    // MEDIDO (2026-11-25, movil): el daemon anunciaba el SHA del binario de
    // DISCO mientras ejecutaba otro en memoria (7 horas de desfase, login
    // roto).  La foto del binario en servicio se toma lo primero, antes de
    // que nada pueda sustituir el fichero.
    megadaemon_core::self_update::fijar_identidad_de_arranque();
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    match args[1].as_str() {
        "cockpit" => {
            let port = args
                .get(2)
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(3000);
            // v15 (W0): bucle autónomo de la voz.  Opt-in explícito
            // (MEGADAEMON_VOICE=1): 1 acción por tick, cooldown, presupuesto
            // por hora y NUNCA reinicios ni procesos nuevos.
            megadaemon_core::voice_runtime::spawn_autonomous_loop();
            // v18: auto-actualización PERMANENTE ("que se auto actualice
            // siempre").  El hilo comprueba el manifiesto de la release,
            // descarga sólo si hay versión nueva, verifica el SHA-256 y
            // deja el anterior en `.prev`.  Se desactiva con
            // MD_NO_AUTO_UPDATE=1.  Tras instalar, sale con código 10 si
            // hay supervisor (para que lo relance con el binario nuevo).
            {
                let ruta = std::env::var("MEGADAEMON_BIN").ok().unwrap_or_else(|| {
                    if cfg!(target_os = "android") {
                        // MEDIDO (2026-11-23, movil): la ruta estaba en
                        // `/data/local/tmp/megadaemon/megadaemon-core`, que es
                        // VOLATIL: el binario ya vive en
                        // `/data/adb/megadaemon/megadaemon-core` => el
                        // auto-actualizador no encontraba el fichero y
                        // respondia "no se pudo calcular el sha256 del
                        // binario instalado" => el movil NO se actualizaba
                        // nunca (siguió en d7d72f1 con el release de 6d8bc56
                        // ya publicado).
                        // Ahora la ruta sale del mismo sitio que el
                        // supervisor y que el reparador de entorno.
                        megadaemon_core::reparar_entorno::Config::del_entorno().binario()
                    } else {
                        std::env::current_exe()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|_| "megadaemon-core".to_string())
                    }
                });
                // La ruta se copia antes de consumirla: el lanzador se la
                // queda (move) y la bitácora la quiere para el evento de
                // arranque.
                let binario_registrado = ruta.clone();
                megadaemon_core::self_update::lanzar_auto_actualizador(
                    std::path::PathBuf::from(ruta),
                    // 30 min: el movil esta al dia con la ultima build sin
                    // depender de reinicios.  Se puede alargar con
                    // MD_AUTO_UPDATE_INTERVAL_MIN.
                    30,
                );
                // El watchdog se instala SOLO (el daemon corre como root en
                // /data/local/tmp): Magisk lo ejecuta en cada boot y relanza
                // el daemon cuando la auto-actualizacion lo deja con
                // codigo 10.  Sin esto, la actualizacion se instala pero no
                // entra en servicio hasta el siguiente reinicio.
                megadaemon_core::supervisor::asegurar_supervisor();
                // MEDIDO (2026-11-23): la bitácora de desarrollo estaba
                // vacía al arrancar (sólo se escribía al auto-actualizar y
                // al instrumentar) => la UI pintaba «Sin eventos» aunque el
                // daemon llevaba horas vivo.  Se registra el arranque.
                megadaemon_core::devlog::append(
                    "arranque",
                    serde_json::json!({
                        "build": megadaemon_core::self_update::build_commit(),
                        "binario": binario_registrado,
                        "puerto": port,
                        "version": env!("CARGO_PKG_VERSION"),
                    }),
                );
                // MEDIDO (2026-11-23, auditoría de calidad): la cola de tareas
                // se guardaba en disco (`task_scheduler::persist`) pero
                // `task_scheduler::restore` NO se llamaba NUNCA en todo el
                // crate => cada reinicio del daemon (o cada auto-actualización
                // con código 10) arrancaba con la cola VACÍA.  El trabajo en
                // curso se perdía en cada reinicio, contra lo que promete el
                // doc del módulo ("un reinicio no pierde el trabajo en curso").
                let tareas = megadaemon_core::task_scheduler::restore();
                if tareas > 0 {
                    eprintln!("task_scheduler: {tareas} tarea(s) restauradas de la cola en disco");
                }
                // MEDIDO (2026-11-23): la URL del tunel publico es EFIMERA
                // (trycloudflare cambia en cada arranque) y quien coordina
                // desde fuera no tiene forma de encontrarla.  El movil se
                // ANUNCIA aqui: publica un "latido" con su URL en el tablon
                // (Drive) => la flota lo encuentra siempre, aunque el
                // internet se caiga y la URL cambie al volver.
                if let Ok(dir) = std::env::var("MEGADAEMON_STATE_DIR") {
                    // MEDIDO (2026-11-23): el supervisor escribe la URL en
                    // `$PREFIX/tunnel.url` y el daemon la buscaba en
                    // `$STATE_DIR/tunnel.url` => NUNCA se anunciaba el móvil
                    // en el tablón => la flota no lo encontraba.
                    // Se miran los DOS sitios (el del supervisor manda).
                    let prefijo = std::env::var("MEGADAEMON_PREFIX")
                        .unwrap_or_else(|_| "/data/adb/megadaemon".to_string());
                    let candidatas = [
                        std::path::Path::new(&prefijo).join("tunnel.url"),
                        std::path::Path::new(&dir).join("tunnel.url"),
                    ];
                    for tunel in candidatas.iter() {
                        let Ok(txt) = std::fs::read_to_string(tunel) else {
                            continue;
                        };
                        let url = txt.trim().to_string();
                        if url.starts_with("http") {
                            let cuerpo = serde_json::json!({
                                "url": url,
                                "puerto": 9090,
                            });
                            if let Ok(mut t) = megadaemon_core::resonance::Tablon::abrir(&dir) {
                                t.publicar("latido", cuerpo);
                            }
                            break;
                        }
                    }
                }
                // v21: ANUNCIO CONTINUO DEL TÚNEL.
                //
                // MEDIDO (2026-11-25): el quick tunnel de Cloudflare genera
                // una URL ALEATORIA nueva cada vez que se reconecta (y se
                // reconecta al cambiar de WiFi a datos, al perder cobertura
                // o al morir cloudflared).  Antes la URL solo se publicaba
                // al ARRANCAR el daemon => el root se quedaba horas con una
                // URL muerta sin saber que habia otra viva.
                //
                // Aqui se vigila `tunnel.url` y se publica en el tablón de
                // Drive en cuanto cambia (y cada 5 min para que el "latido"
                // siga fresco).  El root lo lee con tools/ver-tunel.py y se
                // conecta solo: esto es el equivalente a "mosh" aqui, donde
                // lo que migra es la URL, no la IP.
                {
                    let dir = std::env::var("MEGADAEMON_STATE_DIR")
                        .unwrap_or_else(|_| megadaemon_core::cockpit_server::state_dir());
                    let dir2 = dir.clone();
                    std::thread::Builder::new()
                        .name("anuncio-tunel".to_string())
                        .spawn(move || {
                            let mut ultima: String = String::new();
                            loop {
                                // La URL vigente: la del daemon, o la del
                                // watchdog si el daemon no la ha escrito.
                                let mut url = String::new();
                                for cand in [
                                    format!("{dir2}/tunnel.url"),
                                    format!("{dir2}/../tunnel.url"),
                                    "/data/adb/megadaemon/tunnel.url".to_string(),
                                ] {
                                    if let Ok(t) = std::fs::read_to_string(&cand) {
                                        let t = t.trim().to_string();
                                        if t.starts_with("https://") {
                                            url = t;
                                            break;
                                        }
                                    }
                                }
                                if !url.is_empty() && url != ultima {
                                    ultima = url.clone();
                                    let cuerpo = serde_json::json!({
                                        "url": url,
                                        "puerto": 9090,
                                        "build": megadaemon_core::self_update::build_commit(),
                                    });
                                    if let Ok(mut t) =
                                        megadaemon_core::resonance::Tablon::abrir(&dir2)
                                    {
                                        t.publicar("latido", cuerpo);
                                        eprintln!("tunel anunciado: {url}");
                                    }
                                    // Y el MARCADOR DE PRESENCIA en Drive: es
                                    // lo que sigue el root (tools/ver-tunel.py)
                                    // para migrar solo cuando la URL cambia.
                                    match megadaemon_core::diagnostico::publicar_presencia(&url) {
                                        Ok(_) => eprintln!("presencia publicada: {url}"),
                                        Err(e) => eprintln!("presencia: no se pudo publicar: {e}"),
                                    }
                                }
                                // Cada 60 s: el hilo comprueba la URL y, si
                                // ha cambiado, la republica (por si el
                                // primer intento fallo sin red).
                                std::thread::sleep(std::time::Duration::from_secs(60));
                            }
                        })
                        .map_err(|e| eprintln!("anuncio de tunel: no se pudo arrancar: {e}"));
                }
                // v18: sincroniza el runtime unificado de Antigravity al arrancar.
                if let Err(e) = megadaemon_core::antigravity_clone::unificar_runtime() {
                    eprintln!("antigravity: runtime no unificado: {e}");
                }
                // v20: el BUS DE COORDINACION se despliega al arrancar.
                // MEDIDO (2026-11-23): `Mesh::attach` no lo llamaba nadie =>
                // el fichero `mesh_board.json` no se creaba nunca y TODAS las
                // instancias decian "Bus de coordinacion no desplegado": el
                // enjambre no existia aunque el codigo del bus estuviera
                // entero.  Ahora arrancar el daemon ES desplegar el bus.
                {
                    let dir = std::env::var("MEGADAEMON_STATE_DIR")
                        .unwrap_or_else(|_| megadaemon_core::cockpit_server::state_dir());
                    match megadaemon_core::mesh::desplegar_bus(&dir) {
                        Ok(v) => eprintln!(
                            "bus: desplegado (instancia {}, rol {})",
                            v.get("instancia").and_then(|x| x.as_str()).unwrap_or("?"),
                            v.get("rol").and_then(|x| x.as_str()).unwrap_or("?")
                        ),
                        Err(e) => eprintln!("bus: no se pudo desplegar: {e}"),
                    }
                    // Latido: cada 45 s se renueva la presencia en el tablero,
                    // para que los demas nodos vean a esta instancia viva.
                    std::thread::Builder::new()
                        .name("mesh-latido".to_string())
                        .spawn(move || loop {
                            std::thread::sleep(std::time::Duration::from_secs(45));
                            let _ = megadaemon_core::mesh::latido(&dir);
                        })
                        .map_err(|e| eprintln!("bus: no se pudo arrancar el latido: {e}"));
                }

                // v19: la instrumentacion de Antigravity es AUTOMATICA.
                // El daemon comprueba cada minuto (barato: metadatos, no
                // hashes de 181 MB) y vuelve a instrumentar lo que falte:
                // si la app restaura su `.so`, se repone sin que nadie toque
                // nada.  Es lo que faltaba para que "arrancar Antigravity"
                // se notase (queja del usuario, 2026-11-23).
                if megadaemon_core::config_efectiva::bool_de("instrumentacion_agy.automatica", true)
                {
                    let puerto =
                        megadaemon_core::config_efectiva::u64_de("general.puerto_gateway", 9090)
                            as u16;
                    std::thread::Builder::new()
                        .name("instrumentacion-agy".to_string())
                        .spawn(move || {
                            megadaemon_core::instrumentacion_agy::bucle_automatico(puerto)
                        })
                        .map(|_| {
                            eprintln!(
                                "instrumentación AGY: ciclo automático cada 60 s (configurable)"
                            );
                        })
                        .map_err(|e| {
                            eprintln!("instrumentación AGY: no se pudo arrancar el ciclo: {e}");
                        });
                }
            }
            // v21: el daemon PUBLICA SU DIAGNOSTICO en Drive cada N min.
            //
            // MEDIDO (2026-11-25): el diagnóstico dependía de que el
            // usuario pegase texto en el chat.  Con esto el móvil publica
            // su estado (procesos, puertos, /health, stack, binario, log y
            // NDJSON del shim) en `MegaDaemon/diagnostico/<nodo>/` y el
            // root lo lee desde el sandbox con tools/leer-diagnostico.py:
            // sin ADB, sin túnel y sin pegar nada.
            {
                let min = megadaemon_core::config_efectiva::u64_de("diagnostico.publicar_min", 30);
                if min > 0 {
                    megadaemon_core::diagnostico::publicar_periodico(min);
                    eprintln!("diagnostico: publicación en Drive cada {min} min");
                }
            }
            // v21: el daemon RECONCILIA SU PROPIO STACK al arrancar y cada
            // 10 min.
            //
            // MEDIDO (2026-11-25, movil): el binario se actualizaba pero el
            // supervisor se quedaba viejo (con el prefijo VOLATIL) => no
            // encontraba el binario => no podia relanzar el daemon => movil
            // sin daemon y en bucle de reinicios.  Ademas la Web UI servida
            // era la V1 (7 vistas, sin Antigravity) porque el HTML en disco
            // era viejo.
            //
            // Aqui el daemon lleva DENTRO el supervisor, el keeper, los
            // scripts de Antigravity y la Web UI (ver `stack`), y reescribe
            // lo que difiera.  Es idempotente y no hace nada si todo esta
            // en su sitio.  Si el supervisor cambio, se le mata para que
            // cargue la version nueva (lo revive el keeper).
            {
                std::thread::Builder::new()
                    .name("stack-reconciliacion".to_string())
                    .spawn(|| {
                        let cfg = megadaemon_core::stack::Config::del_entorno();
                        // En un PC (no Android) no se toca /data/adb.
                        let puede = cfg.android
                            || std::fs::create_dir_all(&cfg.prefijo).is_ok();
                        if !puede {
                            return;
                        }
                        loop {
                            let inf = megadaemon_core::stack::reconciliar(&cfg);
                            if inf.cambio() {
                                eprintln!("stack: {}", inf.linea());
                                for e in &inf.escritos {
                                    eprintln!("stack: escrito {e}");
                                }
                                for (r, m) in &inf.fallidos {
                                    eprintln!("stack: FALLO {r}: {m}");
                                }
                                if inf.supervisor_cambiado {
                                    let ok = megadaemon_core::stack::reiniciar_supervisor();
                                    eprintln!(
                                        "stack: el supervisor cambio y se reinicio={ok} (lo revive el keeper)"
                                    );
                                }
                            }
                            // MEDIDO (2026-11-25, movil): con la bateria en
                            // "seal" (<20 % sin cargador) la reconciliacion
                            // se espacia: es trabajo discrecional y solo
                            // importa que el stack este en su sitio.
                            let espera = if megadaemon_core::battery_advanced::modo_ahorro_activo() {
                                1800
                            } else {
                                600
                            };
                            std::thread::sleep(std::time::Duration::from_secs(espera));
                        }
                    })
                    .map_err(|e| eprintln!("stack: no se pudo arrancar la reconciliacion: {e}"));
            }
            if let Err(e) = cockpit_server::run_server(port) {
                eprintln!("Cockpit server error: {}", e);
                std::process::exit(1);
            }
        }

        "mcp-server" => {
            mcp_server::run_stdio_mcp_server();
        }

        // v17 — auto-actualizacion SIN curl ni wget (el movil no los tiene).
        //
        // `megadaemon-core update`         -> comprueba e instala
        // `megadaemon-core update --check` -> solo comprueba (1 peticion)
        //
        // Codigo de salida 10 = "se instalo una version nueva" => el
        // supervisor reinicia el daemon con el binario nuevo.
        // `megadaemon-core self-test` — AUTOCOMPROBACIÓN de arranque.
        //
        // MEDIDO 2026-11-25 (el usuario: "que no se rompa en cada
        // auto-update"): el auto-actualizador verificaba el SHA-256 del
        // binario descargado, pero NO que el binario ARRANQUE.  Un asset
        // corrupto en larelease (o un binario que no arranca en ese móvil)
        // se instalaba y el móvil se quedaba sin daemon: la copia al
        // `.prev` existía, pero nadie la restauraba.
        //
        // Este comando hace un arranque mínimo (state dir, escritura,
        // configuracion, SHA-256) SIN red y SIN abrir el cockpit, e imprime
        // una marca.  El auto-actualizador lo ejecuta sobre el `.new` ANTES
        // de instalarlo: si no arranca, no se instala nada.
        "self-test" => match megadaemon_core::self_update::autocomprobacion() {
            Ok(marca) => {
                println!("{marca}");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("SELFTEST-FALLO: {e}");
                std::process::exit(1);
            }
        },

        "update" => {
            let solo_comprobar = args.iter().any(|a| a == "--check");
            let ruta = std::env::var("MEGADAEMON_BIN")
                .ok()
                .unwrap_or_else(|| "/data/local/tmp/megadaemon/megadaemon-core".to_string());
            let rc = megadaemon_core::self_update::cli(std::path::Path::new(&ruta), solo_comprobar);
            std::process::exit(rc);
        }

        // v17 — `probe` y `channel` en el binario PRINCIPAL.
        //
        // MEDIDO el 2026-11-21: el móvil NO tiene curl, NO tiene wget
        // (`toybox wget` -> "Unknown command", rc=127) y `channel-publish`
        // no se podía instalar (su descarga también usaba wget).  Resultado:
        // el sondeo de salud del script de recuperación daba SIEMPRE 0 y el
        // canal nunca se publicaba => el móvil parecía muerto estando vivo.
        //
        // Ahora el propio binario del daemon hace de sonda y de publicador:
        // cero dependencias externas en el móvil.
        "probe" => {
            // `megadaemon-core probe <url>`  -> imprime el codigo HTTP.
            let url = args.get(2).map(|s| s.as_str()).unwrap_or("");
            if url.is_empty() {
                eprintln!("uso: megadaemon-core probe <url>");
                std::process::exit(2);
            }
            let (code, es_daemon, detalle) = megadaemon_core::channel::probe_url_detalle(url);
            // En stdout, SOLO el numero: es lo que leen los scripts.
            println!("{code}");
            if !detalle.is_null() {
                eprintln!("{}", serde_json::to_string(&detalle).unwrap_or_default());
            }
            std::process::exit(if es_daemon { 0 } else { 1 });
        }

        "channel" => {
            // `megadaemon-core channel publish`  -> publica el estado del canal.
            // `megadaemon-core channel probe <url>` -> alias de `probe`.
            let sub = args.get(2).map(|s| s.as_str()).unwrap_or("publish");
            match sub {
                "probe" => {
                    let url = args.get(3).map(|s| s.as_str()).unwrap_or("");
                    let (code, es_daemon, _) = megadaemon_core::channel::probe_url_detalle(url);
                    println!("{code}");
                    std::process::exit(if es_daemon { 0 } else { 1 });
                }
                "publish" => {
                    let sd = std::env::var("MEGADAEMON_STATE_DIR").ok();
                    let out =
                        megadaemon_core::channel::publish(sd.as_deref(), None, None, None, None);
                    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
                }
                _ => {
                    eprintln!("uso: megadaemon-core channel [publish|probe <url>]");
                    std::process::exit(2);
                }
            }
        }

        // v16 — Capa de automatización (CLI de los 5 módulos nuevos).
        //   megadaemon-core automation <list|add|remove|enable|disable|poll|events|fire|journal>
        //   megadaemon-core cron       <list|add|remove|tick|plan|resume>
        //   megadaemon-core energy    <status|snapshot|profiles|apply|set_profile> [perfil]
        //   megadaemon-core comms     <status|consent|decide|handle|journal> [evento] [peer] [texto]
        //   megadaemon-core bridge    <detect|report|fire|inbox> [paquete] [accion]
        "automation" | "cron" | "energy" | "comms" | "bridge" => {
            let cmd = args[1].as_str();
            let action = args.get(2).cloned().unwrap_or_else(|| "list".to_string());
            let rest: Vec<String> = args.iter().skip(3).cloned().collect();
            let r: serde_json::Value = match cmd {
                "automation" => match action.as_str() {
                    "add" => {
                        // La regla llega como JSON en el último argumento.
                        let raw = rest.join(" ");
                        let rule: serde_json::Value = serde_json::from_str(&raw).unwrap_or_else(
                            |e| serde_json::json!({"ok": false, "error": e.to_string()}),
                        );
                        match megadaemon_core::automation_runtime::automation_add(&rule) {
                            Ok(v) => v,
                            Err(e) => serde_json::json!({"ok": false, "error": e}),
                        }
                    }
                    "remove" => match megadaemon_core::automation_runtime::automation_remove(
                        rest.first().map(|s| s.as_str()).unwrap_or(""),
                    ) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    "enable" => match megadaemon_core::automation_runtime::automation_toggle(
                        rest.first().map(|s| s.as_str()).unwrap_or(""),
                        true,
                    ) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    "disable" => match megadaemon_core::automation_runtime::automation_toggle(
                        rest.first().map(|s| s.as_str()).unwrap_or(""),
                        false,
                    ) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    "poll" => megadaemon_core::automation_runtime::automation_poll(false),
                    "events" => serde_json::json!({
                        "ok": true,
                        "events": megadaemon_core::automation_runtime::poll_events().iter().map(|e| serde_json::json!({
                            "kind": e.kind.as_str(), "key": e.key, "value": e.value, "ts": e.ts
                        })).collect::<Vec<_>>(),
                    }),
                    "fire" => {
                        let kind = rest.first().cloned().unwrap_or_default();
                        let key = rest.get(1).cloned().unwrap_or_default();
                        let text = rest.get(2).cloned().unwrap_or_default();
                        match megadaemon_core::automation_runtime::automation_fire(
                            &kind, &key, None, &text, false,
                        ) {
                            Ok(v) => v,
                            Err(e) => serde_json::json!({"ok": false, "error": e}),
                        }
                    }
                    "journal" => megadaemon_core::automation_runtime::automation_journal(50),
                    _ => megadaemon_core::automation_runtime::automation_list(),
                },
                "cron" => match action.as_str() {
                    "add" => {
                        let raw = rest.join(" ");
                        let job: serde_json::Value = serde_json::from_str(&raw).unwrap_or_else(
                            |e| serde_json::json!({"ok": false, "error": e.to_string()}),
                        );
                        match megadaemon_core::automation_runtime::cron_add(&job) {
                            Ok(v) => v,
                            Err(e) => serde_json::json!({"ok": false, "error": e}),
                        }
                    }
                    "remove" => match megadaemon_core::automation_runtime::cron_remove(
                        rest.first().map(|s| s.as_str()).unwrap_or(""),
                    ) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    "tick" => megadaemon_core::automation_runtime::cron_tick(false),
                    "plan" => megadaemon_core::automation_runtime::cron_plan(),
                    "resume" => megadaemon_core::automation_runtime::cron_resume(),
                    _ => megadaemon_core::automation_runtime::cron_list(),
                },
                "energy" => match action.as_str() {
                    "snapshot" => serde_json::json!({
                        "ok": true,
                        "raw": megadaemon_core::automation_runtime::probe_energy_text(),
                        "snapshot": megadaemon_core::energy_automation::snapshot_to_json(
                            &megadaemon_core::automation_runtime::energy_snapshot(),
                        ),
                    }),
                    "profiles" => megadaemon_core::automation_runtime::energy_profiles(),
                    "apply" => megadaemon_core::automation_runtime::energy_apply(
                        rest.first().map(|s| s.as_str()).unwrap_or(""),
                    ),
                    "set_profile" => match megadaemon_core::automation_runtime::energy_set_profile(
                        rest.first().map(|s| s.as_str()).unwrap_or(""),
                    ) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    _ => megadaemon_core::automation_runtime::energy_status(),
                },
                "comms" => match action.as_str() {
                    "decide" => match megadaemon_core::automation_runtime::comms_decide(
                        rest.first().map(|s| s.as_str()).unwrap_or(""),
                        rest.get(1).map(|s| s.as_str()).unwrap_or(""),
                        rest.get(2).map(|s| s.as_str()).unwrap_or(""),
                    ) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    "handle" => match megadaemon_core::automation_runtime::comms_handle(
                        rest.first().map(|s| s.as_str()).unwrap_or(""),
                        rest.get(1).map(|s| s.as_str()).unwrap_or(""),
                        rest.get(2).map(|s| s.as_str()).unwrap_or(""),
                    ) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    "journal" => megadaemon_core::automation_runtime::comms_journal(50),
                    _ => megadaemon_core::automation_runtime::comms_status(),
                },
                _ => match action.as_str() {
                    "report" => megadaemon_core::automation_runtime::bridge_report(),
                    "fire" => match megadaemon_core::automation_runtime::bridge_fire(
                        rest.first().map(|s| s.as_str()).unwrap_or(""),
                        rest.get(1).map(|s| s.as_str()).unwrap_or(""),
                        &[],
                    ) {
                        Ok(v) => v,
                        Err(e) => serde_json::json!({"ok": false, "error": e}),
                    },
                    "inbox" => megadaemon_core::automation_runtime::bridge_inbox(),
                    _ => megadaemon_core::automation_runtime::bridge_detect(),
                },
            };
            println!("{}", serde_json::to_string_pretty(&r).unwrap_or_default());
        }

        // v15 (W0): la voz a mano (status | plan | act | whoami | catalog).
        // Útil para depurar el bucle autónomo sin esperar al siguiente tick.
        "voice" => {
            let action = args.get(2).cloned().unwrap_or_else(|| "status".to_string());
            let mut payload = serde_json::Map::new();
            payload.insert("action".into(), serde_json::Value::String(action.clone()));
            for a in args.iter().skip(3) {
                if let Some((k, v)) = a.split_once('=') {
                    let key = k.trim_start_matches("--").to_string();
                    match key.as_str() {
                        "llm" | "broadcast" => {
                            payload.insert(
                                key,
                                serde_json::Value::Bool(v.trim() == "1" || v.trim() == "true"),
                            );
                        }
                        "args" => {
                            let parsed: serde_json::Value =
                                serde_json::from_str(v).unwrap_or(serde_json::Value::Null);
                            payload.insert(key, parsed);
                        }
                        _ => {
                            payload.insert(key, serde_json::Value::String(v.to_string()));
                        }
                    }
                }
            }
            let r = megadaemon_core::voice_runtime::handle(
                &action,
                &serde_json::Value::Object(payload),
            );
            println!("{}", serde_json::to_string_pretty(&r).unwrap_or_default());
        }

        // v13.3 — Motor de sincronización de conversaciones (CLI).
        "sync" => {
            let services: Option<Vec<String>> = args.get(2).map(|s| {
                s.split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect()
            });
            let cookies = args.iter().any(|a| a == "--cookies");
            let r = megadaemon_core::sync::run_sync(services, cookies);
            println!("{}", serde_json::to_string_pretty(&r).unwrap_or_default());
        }

        "conv-search" => {
            let q = args.get(2).cloned().unwrap_or_default();
            let svc = args.get(3).cloned();
            let r = megadaemon_core::sync::search(&q, svc.as_deref(), 20);
            println!("{}", serde_json::to_string_pretty(&r).unwrap_or_default());
        }

        "conv-stats" => {
            let r = megadaemon_core::sync::status();
            println!("{}", serde_json::to_string_pretty(&r).unwrap_or_default());
        }

        "daad" => {
            let sub = args.get(2).map(|s| s.as_str()).unwrap_or("scan");
            if sub == "catalog" {
                let platform = args.get(3).map(|s| s.as_str()).filter(|s| !s.is_empty());
                let query = args.get(4).map(|s| s.as_str()).filter(|s| !s.is_empty());
                match megadaemon_core::daad::DAADCatalog::open_default() {
                    Ok(cat) => {
                        let stats = cat.get_stats().ok();
                        let games = cat.query_games(platform, query).unwrap_or_default();
                        let out = serde_json::json!({
                            "ok": true,
                            "stats": stats,
                            "count": games.len(),
                            "games": games,
                        });
                        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
                    }
                    Err(e) => {
                        eprintln!("Error abriendo catalogo DAAD: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                let target_path = args.get(3).cloned().unwrap_or_default();
                if target_path.is_empty() {
                    eprintln!("Uso: megadaemon daad <scan|embedded|media|disasm|catalog> [fichero|plataforma] [filtro]");
                    std::process::exit(1);
                }
                let data = match std::fs::read(&target_path) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("Error leyendo {}: {}", target_path, e);
                        std::process::exit(1);
                    }
                };
                match sub {
                    "scan" => {
                        let analysis = megadaemon_core::daad::DAADBytecodeParser::parse_ddb(
                            &data,
                            &target_path,
                        );
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&analysis).unwrap_or_default()
                        );
                    }
                    "embedded" => {
                        if let Some((offset, size, analysis)) =
                            megadaemon_core::daad::DAADBytecodeParser::find_embedded_ddb(&data)
                        {
                            println!(
                                "Encontrado DDB embebido en offset 0x{:08X} ({} bytes):",
                                offset, size
                            );
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&analysis).unwrap_or_default()
                            );
                        } else {
                            println!("No se encontró DDB embebido.");
                        }
                    }
                    "media" => {
                        let media = megadaemon_core::daad::MediaInspector::inspect_media(
                            &data,
                            &target_path,
                        );
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&media).unwrap_or_default()
                        );
                    }
                    "disasm" => {
                        let dialect = args.get(4).map(|s| s.as_str()).unwrap_or("v2");
                        let (condacts, next_offset) =
                            megadaemon_core::daad::CondActDisassembler::disassemble_stream(
                                &data, 0, dialect,
                            );
                        let out = serde_json::json!({
                            "dialect": dialect,
                            "count": condacts.len(),
                            "next_offset": next_offset,
                            "condacts": condacts,
                        });
                        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
                    }
                    _ => {
                        eprintln!("Comando daad desconocido: {}", sub);
                        std::process::exit(1);
                    }
                }
            }
        }

        "chat" => {
            // v16 (DEFECTO-10): el razonamiento interno (CoT) ya NO se
            // imprime por defecto.  Antes el CLI volcaba el `reasoning`
            // crudo a stdout (y de ahí al log del móvil).  Ahora hace falta
            // pedirlo: `chat <modelo> <prompt> --reasoning`.
            let mut model: Option<&str> = None;
            let mut want_reasoning = false;
            let mut rest: Vec<&str> = Vec::new();
            for a in args.iter().skip(2) {
                match a.as_str() {
                    "--reasoning" => want_reasoning = true,
                    _ => rest.push(a.as_str()),
                }
            }
            // Compatibilidad: `chat <modelo> <prompt...>` (el modelo, el primero).
            if !rest.is_empty() {
                model = Some(rest[0]);
                rest.remove(0);
            }
            let prompt = if rest.is_empty() {
                "Hello".to_string()
            } else {
                rest.join(" ")
            };
            let mut res = providers::execute_chat(&prompt, model, None, None, 1024);
            if !want_reasoning {
                res.reasoning = String::new();
            }
            println!("{}", serde_json::to_string_pretty(&res).unwrap());
        }

        "code" => {
            let mut workspace =
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let mut model: Option<String> = None;
            let mut session_id: Option<String> = None;
            let mut max_turns: usize = 20;
            let mut allow_outside = false;
            let mut goal_parts: Vec<String> = Vec::new();
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--workspace" | "--cwd" => {
                        if let Some(v) = args.get(i + 1) {
                            workspace = std::path::PathBuf::from(v);
                            i += 2;
                            continue;
                        }
                    }
                    "--model" => {
                        if let Some(v) = args.get(i + 1) {
                            model = Some(v.clone());
                            i += 2;
                            continue;
                        }
                    }
                    "--session" => {
                        if let Some(v) = args.get(i + 1) {
                            session_id = Some(v.clone());
                            i += 2;
                            continue;
                        }
                    }
                    "--max-turns" => {
                        if let Some(v) = args.get(i + 1).and_then(|s| s.parse::<usize>().ok()) {
                            max_turns = v;
                            i += 2;
                            continue;
                        }
                    }
                    "--allow-outside" => {
                        allow_outside = true;
                        i += 1;
                        continue;
                    }
                    other => {
                        goal_parts.push(other.to_string());
                    }
                }
                i += 1;
            }
            let goal = goal_parts.join(" ");
            if goal.trim().is_empty() {
                eprintln!("Uso: megadaemon code [--workspace <dir>] [--model <id>] [--session <id>] [--max-turns <n>] <objetivo>");
                std::process::exit(1);
            }
            let cfg = megadaemon_core::agent_coding::CodingAgentConfig {
                workspace_root: workspace,
                max_turns,
                max_context_chars: 120_000,
                model,
                session_id,
                allow_outside_workspace: allow_outside,
            };
            let report = megadaemon_core::agent_coding::run_coding_session_live(&goal, &cfg);
            println!(
                "{}",
                serde_json::to_string_pretty(&report).unwrap_or_default()
            );
        }

        "go-retool" => {
            let sub = args.get(2).map(|s| s.as_str()).unwrap_or("analyze");
            let fp = args.get(3).map(|s| s.as_str()).unwrap_or("");
            if fp.is_empty() {
                eprintln!("Uso: megadaemon go-retool <analyze|container|functions|ghidra_json> <binario> [filtro]");
                std::process::exit(1);
            }
            let query = args.get(4).map(|s| s.as_str());
            let rep = megadaemon_core::go_retool::handle_action(
                sub,
                std::path::Path::new(fp),
                query,
                500,
            );
            println!("{}", serde_json::to_string_pretty(&rep).unwrap_or_default());
        }

        "eid" => {
            let sub = args.get(2).map(|s| s.as_str()).unwrap_or("help");
            match sub {
                "inspect-p12" => {
                    let fp = args.get(3).map(|s| s.as_str()).unwrap_or("");
                    let pass = args.get(4).map(|s| s.as_str()).unwrap_or("");
                    if fp.is_empty() {
                        eprintln!("Uso: megadaemon eid inspect-p12 <archivo.p12> [password]");
                        std::process::exit(1);
                    }
                    match megadaemon_core::es_eid::load_pkcs12(std::path::Path::new(fp), pass) {
                        Ok(id) => println!(
                            "{}",
                            serde_json::to_string_pretty(&id.inspection).unwrap_or_default()
                        ),
                        Err(e) => {
                            eprintln!("eid inspect-p12 error: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                "dnie-atr" => {
                    let hex_str = args.get(3).map(|s| s.as_str()).unwrap_or("");
                    let bytes: Vec<u8> = (0..hex_str.len())
                        .step_by(2)
                        .filter_map(|i| {
                            hex_str
                                .get(i..i + 2)
                                .and_then(|h| u8::from_str_radix(h, 16).ok())
                        })
                        .collect();
                    let res = megadaemon_core::es_eid::inspect_dnie_atr(&bytes);
                    println!("{}", serde_json::to_string_pretty(&res).unwrap_or_default());
                }
                _ => {
                    println!("Uso: megadaemon eid <inspect-p12|dnie-atr> ...");
                }
            }
        }

        "ast" => {
            let fp = args.get(2).map(|s| s.as_str()).unwrap_or("");
            if fp.is_empty() {
                eprintln!("Uso: megadaemon ast <archivo.rs>");
                std::process::exit(1);
            }
            match megadaemon_core::code_ast::analyze_file(std::path::Path::new(fp)) {
                Ok(rep) => println!("{}", serde_json::to_string_pretty(&rep).unwrap_or_default()),
                Err(e) => {
                    eprintln!("ast error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        "re-disasm" => {
            let arch = args.get(2).map(|s| s.as_str()).unwrap_or("arm64");
            let hex_str = args.get(3).map(|s| s.as_str()).unwrap_or("");
            if hex_str.is_empty() {
                eprintln!("Uso: megadaemon re-disasm <arm64|arm|x86_64|x86_32|m68k|6800|6809|ppc|mips|riscv64> <hex_bytes>");
                std::process::exit(1);
            }
            let bytes: Vec<u8> = (0..hex_str.len())
                .step_by(2)
                .filter_map(|i| {
                    hex_str
                        .get(i..i + 2)
                        .and_then(|h| u8::from_str_radix(h, 16).ok())
                })
                .collect();
            let res = if arch == "x86_32" || arch == "x86_64" {
                let bits = if arch == "x86_32" { 32 } else { 64 };
                Ok(megadaemon_core::re_disasm::disasm_x86_iced(
                    &bytes, bits, 0x400000, 128,
                ))
            } else {
                megadaemon_core::re_disasm::disasm_capstone(&bytes, arch, 0x400000, 128)
            };
            match res {
                Ok(rep) => println!("{}", serde_json::to_string_pretty(&rep).unwrap_or_default()),
                Err(e) => {
                    eprintln!("disasm error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        "rhai" => {
            let script = args.get(2).map(|s| s.as_str()).unwrap_or("40 + 2");
            let rep = megadaemon_core::script_sandbox::eval_sandboxed(script, 50_000);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap_or_default());
        }

        "jobs" => {
            let db = std::path::Path::new(&megadaemon_core::cockpit_server::state_dir())
                .join("megadaemon_jobs.db");
            let q = megadaemon_core::job_queue::JobQueue::open(&db).expect("open job queue");
            let list = q.list(50).unwrap_or_default();
            println!(
                "{}",
                serde_json::to_string_pretty(&list).unwrap_or_default()
            );
        }

        "audit" => {
            let db = std::path::Path::new(&megadaemon_core::cockpit_server::state_dir())
                .join("megadaemon_audit.db");
            let rows = megadaemon_core::tool_policy::AuditJournal::open(&db)
                .and_then(|j| j.recent(50))
                .unwrap_or_default();
            println!(
                "{}",
                serde_json::to_string_pretty(&rows).unwrap_or_default()
            );
        }

        //   megadaemon-core agy-logs [--json] [--n 200]
        //
        //   Los logs de la PROPIA app de Antigravity (donde esta el error
        //   real del login) y el estado de la cuenta.  Se pueden volcar a
        //   stdout o subir a Drive con `--publicar` (util desde el movil,
        //   sin ADB ni que el root tenga que pegar texto).
        "agy-logs" => {
            let json = args[2..].iter().any(|a| a == "--json");
            let publicar = args[2..].iter().any(|a| a == "--publicar");
            let n: usize = args[2..]
                .iter()
                .position(|a| a == "--n")
                .and_then(|i| args[2 + i + 1].parse().ok())
                .unwrap_or(200);
            let v = megadaemon_core::diagnostico::logs_de_la_app_publicos(n);
            if publicar {
                match megadaemon_core::diagnostico::publicar_logs_agy(&v) {
                    Ok(r) => println!("{}", r),
                    Err(e) => {
                        eprintln!("agy-logs: {e}");
                        std::process::exit(1);
                    }
                }
            } else if json {
                println!("{}", v);
            } else {
                for f in v["ficheros"].as_array().cloned().unwrap_or_default() {
                    println!("== {}", f["fichero"].as_str().unwrap_or("?"));
                    for l in f["lineas"].as_array().cloned().unwrap_or_default() {
                        println!("{}", l.as_str().unwrap_or(""));
                    }
                }
            }
            std::process::exit(0);
        }

        "drive-vfs" => {
            let sub = args.get(2).map(|s| s.as_str()).unwrap_or("ls");
            let vpath = args.get(3).map(|s| s.as_str()).unwrap_or("drive:/");
            let st = megadaemon_core::cockpit_server::state_dir();
            let res: Result<serde_json::Value, String> = match sub {
                "ls" => megadaemon_core::drive_vfs::ls(vpath, Some(&st), 100)
                    .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null)),
                "stat" => megadaemon_core::drive_vfs::stat_path(vpath, Some(&st))
                    .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null)),
                "find" => megadaemon_core::drive_vfs::find_by_name(vpath, Some(&st), 100)
                    .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null)),
                "mkdir" => megadaemon_core::drive_vfs::mkdir_p(vpath, Some(&st))
                    .map(|id| serde_json::json!({"ok": true, "folder_id": id})),
                "cat" | "read" => megadaemon_core::drive_vfs::read_file_bytes(vpath, 65536, Some(&st))
                    .map(|b| serde_json::json!({"ok": true, "bytes": b.len(), "text": String::from_utf8_lossy(&b)})),
                _ => Err("Uso: megadaemon drive-vfs <ls|stat|find|mkdir|cat> <drive:/path|query>".to_string()),
            };
            match res {
                Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default()),
                Err(e) => {
                    eprintln!("drive-vfs error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        //   megadaemon-core reparar [--diagnostico] [--json]
        //     Diagnostica y REPARA el entorno del propio nodo (prefijo
        //     persistente, binario, watchdog, keeper, API key, credenciales
        //     de Drive y plantilla web).  Es idempotente.
        "reparar" | "repair" => {
            let rc = megadaemon_core::reparar_entorno::run_cli(&args[2..]);
            std::process::exit(rc);
        }

        //   megadaemon-core stack [--reconciliar] [--json]
        //
        //   El daemon lleva DENTRO el supervisor, el keeper, cloudflared,
        //   los scripts de Antigravity y la Web UI.  "stack" dice si lo que
        //   hay en disco es exactamente lo que lleva el binario;
        //   "--reconciliar" reescribe lo que difiere (idempotente) y
        //   reinicia el supervisor si este cambio.
        //
        //   MEDIDO (2026-11-25, movil): el supervisor instalado era una
        //   version vieja con el prefijo VOLATIL => no encontraba el
        //   binario => no podia relanzar el daemon => movil sin daemon.
        "stack" => {
            let cfg = megadaemon_core::stack::Config::del_entorno();
            let json = args[2..].iter().any(|a| a == "--json");
            let reconciliar = args[2..]
                .iter()
                .any(|a| a == "--reconciliar" || a == "reconciliar" || a == "reparar");
            if !reconciliar {
                let v = megadaemon_core::stack::estado_json(&cfg);
                if json {
                    println!("{}", v);
                } else {
                    println!("stack (prefijo {}):", cfg.prefijo);
                    for f in v["ficheros"].as_array().cloned().unwrap_or_default() {
                        println!(
                            "  [{}] {}  (origen {})",
                            if f["ok"].as_bool().unwrap_or(false) {
                                "ok"
                            } else {
                                "DESFASADO"
                            },
                            f["ruta"].as_str().unwrap_or("?"),
                            f["origen"].as_str().unwrap_or("?")
                        );
                    }
                    println!(
                        "  {} de {} correctos",
                        v["correctos"].as_u64().unwrap_or(0),
                        v["total"].as_u64().unwrap_or(0)
                    );
                }
                std::process::exit(0);
            }
            let inf = megadaemon_core::stack::reconciliar(&cfg);
            let reiniciado = if inf.supervisor_cambiado {
                megadaemon_core::stack::reiniciar_supervisor()
            } else {
                false
            };
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "ok": inf.ok(),
                        "informe": inf.a_json(),
                        "supervisor_reiniciado": reiniciado,
                        "estado": megadaemon_core::stack::estado_json(&cfg),
                    })
                );
            } else {
                println!("{}", inf.linea());
                for e in &inf.escritos {
                    println!("  escrito: {e}");
                }
                for (r, m) in &inf.fallidos {
                    eprintln!("  FALLO {r}: {m}");
                }
                if inf.supervisor_cambiado {
                    println!(
                        "  el supervisor cambio: reiniciado={reiniciado} (lo revive el keeper)"
                    );
                }
            }
            std::process::exit(if inf.ok() { 0 } else { 1 });
        }

        //   megadaemon-core bundle [--instalar] [--json]
        //
        //   Descarga el ARTEFACTO META (un unico .tar.gz con el binario, el
        //   supervisor, el keeper, los scripts de Antigravity y la Web UI),
        //   verifica el SHA-256 de CADA fichero contra su manifest.json y lo
        //   instala todo.  Si un solo hash no coincide, no instala nada.
        //
        //   Es la via de instalacion/actualizacion COMPLETA; el stack
        //   autocontenido (comando `stack`) es la via de autorreparacion
        //   offline.
        "bundle" => {
            let json = args[2..].iter().any(|a| a == "--json");
            let instalar = args[2..]
                .iter()
                .any(|a| a == "--instalar" || a == "instalar" || a == "install");
            if !instalar {
                let url = megadaemon_core::bundle::url_bundle();
                match megadaemon_core::bundle::sha256_bundle() {
                    Ok(sha) => {
                        if json {
                            println!("{}", serde_json::json!({"url": url, "sha256": sha}));
                        } else {
                            println!("bundle: {url}");
                            println!("  sha256: {sha}");
                            println!("  usa: megadaemon-core bundle --instalar");
                        }
                        std::process::exit(0);
                    }
                    Err(e) => {
                        eprintln!("bundle: no se pudo consultar: {e}");
                        std::process::exit(1);
                    }
                }
            }
            match megadaemon_core::bundle::instalar() {
                Ok(inf) => {
                    if json {
                        println!("{}", inf.a_json());
                    } else {
                        println!("bundle instalado (build {})", inf.build);
                        for i in &inf.instalados {
                            println!("  instalado: {i}");
                        }
                        for (r, m) in &inf.fallidos {
                            eprintln!("  FALLO {r}: {m}");
                        }
                    }
                    std::process::exit(if inf.ok() { 0 } else { 1 });
                }
                Err(e) => {
                    eprintln!("bundle: {e}");
                    std::process::exit(1);
                }
            }
        }

        //   megadaemon-core diagnostico [--publicar] [--json]
        //
        //   Recoge el estado completo del nodo y, con --publicar, lo sube a
        //   Drive (MegaDaemon/diagnostico/<nodo>/).  Asi el root ve el
        //   estado del movil sin ADB y sin pegar texto en el chat.
        "diagnostico" | "diagnostics" => {
            let json = args[2..].iter().any(|a| a == "--json");
            let publicar = args[2..]
                .iter()
                .any(|a| a == "--publicar" || a == "publicar" || a == "publish");
            if publicar {
                match megadaemon_core::diagnostico::publicar() {
                    Ok(v) => {
                        if json {
                            println!("{}", v);
                        } else {
                            println!(
                                "diagnostico publicado en Drive: {}",
                                v["vfs"].as_str().unwrap_or("?")
                            );
                        }
                        std::process::exit(0);
                    }
                    Err(e) => {
                        eprintln!("diagnostico: {e}");
                        std::process::exit(1);
                    }
                }
            }
            let v = megadaemon_core::diagnostico::recoger();
            if json {
                println!("{}", v);
            } else {
                println!("{}", megadaemon_core::diagnostico::a_markdown(&v));
            }
            std::process::exit(0);
        }

        "retro-tools" => {
            let sub = args.get(2).map(|s| s.as_str()).unwrap_or("axml");
            let fp = args.get(3).map(|s| s.as_str()).unwrap_or("");
            if fp.is_empty() {
                eprintln!("Uso: megadaemon retro-tools <axml|font-1bpp|saturn-pal|scummvm-save|dex-origins> <archivo>");
                std::process::exit(1);
            }
            let bytes = std::fs::read(fp).unwrap_or_else(|e| {
                eprintln!("read {}: {}", fp, e);
                std::process::exit(1);
            });
            let out = match sub {
                "axml" => match megadaemon_core::axml_parser::parse_axml(&bytes) {
                    Ok(r) => serde_json::to_value(r).unwrap_or_default(),
                    Err(e) => {
                        eprintln!("axml error: {}", e);
                        std::process::exit(1);
                    }
                },
                "font-1bpp" => megadaemon_core::retro_gfx::extract_1bpp_glyphs(&bytes, 64),
                "saturn-pal" => megadaemon_core::retro_gfx::inspect_saturn_palette(&bytes, true),
                "scummvm-save" => megadaemon_core::scummvm_save::parse_scummvm_savegame(&bytes),
                "dex-origins" => megadaemon_core::dex_origins::scan_dex_origins(&bytes),
                other => {
                    eprintln!("subcomando desconocido '{}'", other);
                    std::process::exit(1);
                }
            };
            println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        }

        "drive-sync" => {
            let local_dir = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let remote_name = args.get(3).map(|s| s.as_str()).unwrap_or("MegaDaemon_Sync");
            let parent_id = args
                .windows(2)
                .find(|w| w[0] == "--parent")
                .map(|w| w[1].as_str())
                .unwrap_or("");
            let workers = args
                .windows(2)
                .find(|w| w[0] == "--workers")
                .and_then(|w| w[1].parse::<usize>().ok())
                .unwrap_or(8);
            let verify_delete = args.iter().any(|a| a == "--verify-delete");
            if local_dir.is_empty() {
                eprintln!("Uso: megadaemon drive-sync <local_dir> <remote_folder> [--parent <id>] [--workers 8] [--verify-delete]");
                std::process::exit(1);
            }
            let st = megadaemon_core::cockpit_server::state_dir();
            match megadaemon_core::drive_native::upload_dir_parallel(
                std::path::Path::new(local_dir),
                remote_name,
                parent_id,
                workers,
                verify_delete,
                Some(&st),
            ) {
                Ok(rep) => println!("{}", serde_json::to_string_pretty(&rep).unwrap_or_default()),
                Err(e) => {
                    eprintln!("drive-sync error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        "die-binwalk" => {
            let fp = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let rep = binwalk_die::analyze_binary_full(fp);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "cirx-ssa" => {
            let mas = args
                .get(2)
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.75);
            let rep = cirx_rtl::run_reference_pipeline(mas, 10, 0.05, 3, 1, 0);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "mba" => {
            let expr = if args.len() > 2 {
                args[2..].join(" ")
            } else {
                "(x ^ y) + 2*(x & y)".into()
            };
            let rep = kaitai_imhex::simplify_mba_expression(&expr);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "kaitai" => {
            let fp = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let tmpl = args
                .get(3)
                .map(|s| s.as_str())
                .unwrap_or("sega_m68k_vectors");
            let off = args.get(4).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            match kaitai_imhex::parse_struct_at_offset(fp, tmpl, off, None) {
                Ok(rep) => println!("{}", serde_json::to_string_pretty(&rep).unwrap()),
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }

        "rizin" => {
            let fp = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let mode = args.get(3).map(|s| s.as_str()).unwrap_or("info");
            let arch = args.get(4).map(|s| s.as_str());
            let rep = frida_dbi::run_rizin_analysis(fp, mode, arch);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "frida" => {
            let mode = args.get(2).map(|s| s.as_str()).unwrap_or("attach");
            let module = args.get(3).map(|s| s.as_str()).unwrap_or("libtarget.so");
            let sym = args.get(4).map(|s| s.as_str()).unwrap_or("0x1000");
            let proc_n = args
                .get(5)
                .map(|s| s.as_str())
                .unwrap_or("com.example.target");
            let rep = frida_dbi::generate_frida_script(mode, module, sym, proc_n);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "npu-cv" => {
            let rep = npu_cv::analyze_vla_frame_and_accel();
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "unifier" => {
            let rep = unifier::scan_unified_ecosystem();
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "rea" => {
            let plat = args.get(2).map(|s| s.as_str()).unwrap_or("segacd");
            let query = args.get(3).map(|s| s.as_str()).unwrap_or("");
            let max_r = args
                .get(4)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(5);
            let rep = rea_assistant::analyze_ghidra_corpus(plat, query, max_r);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        // v21: `megadaemon-core config-set <ruta.punteada> <json>`
        //
        // MEDIDO (2026-11-25, movil): la instrumentacion de Antigravity
        // hubo que DESACTIVAR a mano (por HTTP) para que el language server
        // arrancara, y reactivarla exigia el token de sesion.  Con este
        // CLI el script de recuperacion la reactiva sin HTTP ni token.
        //
        //   megadaemon-core config-set instrumentacion_agy.activa true
        //   megadaemon-core config-set instrumentacion_agy.modo_so '"falso"'
        //   megadaemon-core config-get instrumentacion_agy.activa
        "config-set" | "config-get" => {
            let ruta = args.get(2).cloned().unwrap_or_default();
            if ruta.is_empty() {
                eprintln!(
                    "uso: megadaemon-core <config-set|config-get> <ruta.punteada> [valor-json]"
                );
                std::process::exit(2);
            }
            if args[1] == "config-get" {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &megadaemon_core::config_efectiva::valor(&ruta)
                            .unwrap_or(serde_json::Value::Null)
                    )
                    .unwrap_or_default()
                );
                return;
            }
            let valor_txt = args.get(3).cloned().unwrap_or_default();
            let valor: serde_json::Value = serde_json::from_str(&valor_txt)
                .unwrap_or_else(|_| serde_json::Value::String(valor_txt.clone()));
            // De "a.b.c" a {"a":{"b":{"c":valor}}}.
            let mut anidado = valor.clone();
            let mut partes: Vec<&str> = ruta.split('.').rev().collect();
            let primera = partes.remove(0);
            for p in partes.into_iter().rev() {
                anidado = serde_json::json!({ p: anidado });
            }
            let cambios = serde_json::json!({ primera: anidado });
            match megadaemon_core::config_efectiva::escribir(&cambios) {
                Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default()),
                Err(e) => {
                    eprintln!("config-set error: {e}");
                    std::process::exit(1);
                }
            }
        }
        "cfg" => {
            let plat = args.get(2).map(|s| s.as_str()).unwrap_or("segacd");
            let offset = args.get(3).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            let count = args
                .get(4)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(28);
            if let Some(rom) = rom_locator::resolve_binary(plat) {
                match cfg_lifter::lift_cfg(
                    plat,
                    &rom.platform_name,
                    &rom.arch,
                    &rom.path,
                    offset,
                    count,
                ) {
                    Ok(rep) => println!("{}", serde_json::to_string_pretty(&rep).unwrap()),
                    Err(e) => {
                        eprintln!("CFG error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("Binary for platform '{}' not found", plat);
                std::process::exit(1);
            }
        }

        "bindiff" => {
            let plat = args.get(2).map(|s| s.as_str()).unwrap_or("segacd");
            let rep = bindiff::run_cross_platform_bindiff(plat);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "hw-mmio" => {
            let plat = args.get(2).map(|s| s.as_str()).unwrap_or("segacd");
            let (_, _, _, c_path, _) = rom_locator::platform_metadata(&plat.to_lowercase());
            let sample = fs::read_to_string(c_path)
                .map(|s| megadaemon_core::util_text::truncate_chars(&s, 65536))
                .unwrap_or_default();
            let rep = hw_mapper::analyze_text_for_mmio_and_structs(plat, &sample);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "crypto-scan" => {
            let plat = args.get(2).map(|s| s.as_str()).unwrap_or("segacd");
            if let Some(rom) = rom_locator::resolve_binary(plat) {
                let rep = crypto_scan::scan_crypto_and_encodings(plat, &rom.path);
                println!("{}", serde_json::to_string_pretty(&rep).unwrap());
            } else {
                eprintln!("Binary for platform '{}' not found", plat);
                std::process::exit(1);
            }
        }

        "seere" => {
            if args.len() < 3 {
                eprintln!("Usage: megadaemon-core seere <query>");
                std::process::exit(1);
            }
            let mut query_parts: Vec<String> = Vec::new();
            let mut max_turns = 4usize;
            let mut env_in_prompt = false;
            // BUG v13.2 (HW-09): con "--max-turns 8", el "8" acababa en la
            // consulta del LLM (la bandera se saltaba pero su valor no).
            // Ahora se consume el valor de la bandera.
            let rest = &args[2..];
            let mut i = 0usize;
            while i < rest.len() {
                let a = &rest[i];
                if a == "--env-in-prompt" {
                    env_in_prompt = true;
                } else if a == "--max-turns" {
                    if let Some(v) = rest.get(i + 1) {
                        max_turns = v.parse().unwrap_or(4);
                        i += 1;
                    }
                } else if let Some(v) = a.strip_prefix("--max-turns=") {
                    max_turns = v.parse().unwrap_or(4);
                } else {
                    query_parts.push(a.clone());
                }
                i += 1;
            }
            let query = query_parts.join(" ");
            // v13.2 (S02): el guardián de taint bloquea la ejecución y la
            // delegación cuando el contexto viene de contenido no fiable
            // (web/fichero).  Escape hatch explícito del operador.
            let allow_tainted = env::var("SEERE_ALLOW_TAINTED")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            let cfg = seere_v11::SeereConfig {
                max_turns,
                allow_tools: true,
                bin_mode: false,
                plan_mode: false,
                allow_exec: true,
                allow_external: false,
                env_in_prompt,
                re_opcodes: false,
                allow_tainted,
            };
            let res = seere_v11::run_seere(&query, &cfg);
            println!("{}", serde_json::to_string(&res).unwrap());
        }

        "power" => {
            let (st, pol) = power_policy::current_policy();
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "policy": pol.name, "max_turns": pol.max_turns,
                    "prefer_local": pol.prefer_local, "allow_heavy": pol.allow_heavy,
                    "max_tokens": pol.max_tokens, "reason": pol.reason,
                    "charging": st.charging, "battery_pct": st.capacity_pct,
                    "battery_temp_c": st.battery_temp_c, "cpu_temp_c": st.cpu_temp_c,
                    "thermal_hot": st.thermal_hot, "system_power_save": st.system_power_save,
                    "governor": st.governor
                }))
                .unwrap()
            );
        }
        "seere-prompt" => {
            let lang = if args.len() > 2 {
                args[2].clone()
            } else {
                "es".to_string()
            };
            println!("{}", seere_v11::sys_prompt("REACT", &lang));
        }
        "seere-grammar" => {
            println!("{}", seere_v11::build_act_grammar());
        }
        "seere-selftest" => match seere_v11::selftest() {
            Ok(msg) => println!("{}", msg),
            Err(e) => {
                eprintln!("SEERE v12 selftest failed: {}", e);
                std::process::exit(1);
            }
        },

        "npu-probe" => {
            // v15: `--vs4l` sondea el driver NPU con el metodo correcto
            // (ioctl SIN argumento => el driver devuelve el tamano que espera).
            if args.iter().any(|a| a == "--vs4l" || a == "-v") {
                let rep = npu_vs4l::probe();
                println!("{}", serde_json::to_string_pretty(&rep).unwrap());
            } else {
                let rep = npu_bridge::probe_exynos_npu();
                println!("{}", serde_json::to_string_pretty(&rep).unwrap());
            }
        }

        "csc-verify" => {
            // Verificador post-parche del CSC: la feature de grabacion nativa
            // esta realmente activa en el movil? (no "deberia estarlo").
            // `csc-verify --expect <valor>` exige un valor EXACTO (p.ej.
            // "RecordingAllowed" o "RecordingAllowedByMenu,disclaimer_latin").
            let expect = args
                .windows(2)
                .find(|w| w[0] == "--expect")
                .map(|w| w[1].to_string());
            let rep = csc_verify::verify_expecting(expect.as_deref());
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "csc-patch" => {
            // Aplica (o SIMULA) un valor nuevo para una feature del CSC y
            // VERIFICA que el XML re-codificado cambia de sha256 y que el
            // valor queda aplicado.
            //
            //   megadaemon-core csc-patch --value RecordingAllowedByMenu,disclaimer_latin
            //   megadaemon-core csc-patch --value RecordingAllowed --apply
            //
            // Sin `--apply` es una SIMULACIÓN: no toca el móvil (por defecto no
            // se cambia el valor legal de fábrica).
            let value = args
                .windows(2)
                .find(|w| w[0] == "--value")
                .map(|w| w[1].to_string());
            let feature = args
                .windows(2)
                .find(|w| w[0] == "--feature")
                .map(|w| w[1].to_string())
                .unwrap_or_else(|| "CscFeature_VoiceCall_ConfigRecording".to_string());
            let apply = args.iter().any(|a| a == "--apply");
            let Some(valor) = value else {
                eprintln!(
                    "Uso: megadaemon-core csc-patch --value <valor> [--feature <nombre>] [--apply]"
                );
                std::process::exit(2);
            };
            let csc = csc_patch::active_csc().unwrap_or_else(|| "AMO".to_string());
            let modulo = if apply {
                std::path::PathBuf::from(csc_patch::MODULE_DIR)
            } else {
                std::path::PathBuf::from("/data/local/tmp/megadaemon/csc_patch_dryrun")
            };
            let antes = csc_patch::read_csc_xml(&csc)
                .ok()
                .and_then(|_| {
                    let ruta = csc_patch::xml_candidates(&csc)
                        .into_iter()
                        .find(|p| p.exists())
                        .unwrap_or_default();
                    std::fs::read(&ruta)
                        .ok()
                        .map(|b| root_occlusion::sha256_hex(&b))
                })
                .unwrap_or_else(|| "?".to_string());
            let rep = match csc_patch::patch(&csc, &feature, &valor, &modulo) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("csc-patch error: {}", e);
                    std::process::exit(1);
                }
            };
            // Verificación: se vuelve a leer el fichero escrito, se calcula su
            // sha256, se descodifica (OMC) y se comprueba el valor.
            let bytes = std::fs::read(&rep.modulo).unwrap_or_default();
            let despues = root_occlusion::sha256_hex(&bytes);
            let leido = root_occlusion::read_device_file(&rep.modulo, 1 << 20).and_then(|b| {
                let (xml, _enc, _err) = csc_verify::decode_csc_file(&b);
                xml.and_then(|x| rom_migration::csc_feature_get(&x, &feature))
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "modo": if apply { "aplicado" } else { "simulacion" },
                    "csc": rep.csc,
                    "feature": rep.feature,
                    "valor_anterior": rep.valor_anterior,
                    "valor_pedido": rep.valor_nuevo,
                    "valor_leido_del_fichero_re_codificado": leido,
                    "valor_aplicado": leido.as_deref() == Some(rep.valor_nuevo.as_str()),
                    "sha256_antes": antes,
                    "sha256_despues": despues,
                    "sha256_cambia": antes != despues,
                    "bytes": rep.bytes_escritos,
                    "fichero": rep.modulo,
                    "contents_db_actualizado": rep.contents_db_actualizado,
                    "aviso": if apply {
                        "reinicia para que el framework lea el fichero; sin --apply esto es solo una simulacion"
                    } else {
                        "SIMULACION: no se ha escrito nada en el modulo"
                    },
                }))
                .unwrap()
            );
        }

        "rootcheck" => {
            // Banco de pruebas de ocultacion de root:
            //   megadaemon-core rootcheck <paquete> [<paquete>...]
            //   megadaemon-core rootcheck --state      (que se ve ahora mismo)
            //   megadaemon-core rootcheck --exec <pkg>  (prueba de ejecucion)
            //   megadaemon-core rootcheck --static <pkg>
            // Salida: JSON con la tabla app -> senal -> veredicto.
            let mut pkgs: Vec<String> = Vec::new();
            let mut mode = "audit";
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--state" => mode = "state",
                    "--exec" => mode = "exec",
                    "--static" => mode = "static",
                    "--csc" => mode = "csc",
                    other => pkgs.push(other.to_string()),
                }
                i += 1;
            }
            if pkgs.is_empty() && mode == "audit" {
                eprintln!("Usage: megadaemon-core rootcheck <paquete>[,<paquete>...] | --state | --exec <pkg> | --static <pkg> | --csc");
                std::process::exit(2);
            }
            let packages = serde_json::json!(pkgs);
            let rep = android::root_occlusion_audit(&packages, "", mode);
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "magisk" => {
            // Gestion real de modulos Magisk:
            //   megadaemon-core magisk list
            //   megadaemon-core magisk enable <id> | disable <id>
            //   megadaemon-core magisk validate <zip>
            //   megadaemon-core magisk install <zip> [--enable] [--dry-run]
            //   megadaemon-core magisk shamiko [<zip>] [--enable] [--dry-run]
            let sub = args.get(2).map(|s| s.as_str()).unwrap_or("list");
            let rest: Vec<String> = args.iter().skip(3).cloned().collect();
            let has = |f: &str| rest.iter().any(|a| a == f);
            let positional: Vec<String> = rest
                .iter()
                .filter(|a| !a.starts_with("--"))
                .cloned()
                .collect();
            let enable = has("--enable");
            let dry_run = if has("--dry-run") {
                true
            } else {
                !has("--real")
            };
            let rep = match sub {
                "list" => android::magisk_module("list", "", "", "", "", enable, dry_run),
                "enable" | "disable" => {
                    let id = positional.first().cloned().unwrap_or_default();
                    android::magisk_module(sub, &id, "", "", "", enable, dry_run)
                }
                "validate" => {
                    let zip = positional.first().cloned().unwrap_or_default();
                    android::magisk_module("validate_zip", "", "", "", &zip, enable, dry_run)
                }
                "install" => {
                    let zip = positional.first().cloned().unwrap_or_default();
                    android::magisk_module("install", "", "", "", &zip, enable, dry_run)
                }
                "shamiko" => {
                    let zip = positional.first().cloned().unwrap_or_default();
                    android::magisk_module("shamiko", "", "", "", &zip, enable, dry_run)
                }
                other => {
                    eprintln!("Subcomando desconocido: {}", other);
                    std::process::exit(2);
                }
            };
            println!("{}", serde_json::to_string_pretty(&rep).unwrap());
        }

        "locate-roms" => {
            let refresh = args
                .get(2)
                .map(|s| s == "1" || s == "true")
                .unwrap_or(false);
            let roms = rom_locator::locate_all(refresh);
            println!("{}", serde_json::to_string(&roms).unwrap());
        }

        "disasm" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core disasm <plat> <file> [offset] [count]");
                std::process::exit(1);
            }
            let plat = &args[2];
            let file_path = &args[3];
            let offset = args.get(4).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            let count = args
                .get(5)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(16);
            let (plat_name, arch, _, _, _) = rom_locator::platform_metadata(plat);
            match disasm::disassemble_file(plat, plat_name, arch, file_path, offset, count) {
                Ok(rep) => println!("{}", serde_json::to_string(&rep).unwrap()),
                Err(e) => {
                    eprintln!("Disasm error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        "hex" => {
            if args.len() < 3 {
                eprintln!("Usage: megadaemon-core hex <file> [offset] [length]");
                std::process::exit(1);
            }
            let file_path = &args[2];
            let offset = args.get(3).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            let length = args
                .get(4)
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(256);
            match disasm::inspect_hex(file_path, "ARM64/Retro", offset, length) {
                Ok(rep) => println!("{}", serde_json::to_string(&rep).unwrap()),
                Err(e) => {
                    eprintln!("Hex error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        "deobfuscate" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core deobfuscate <plat> <file>");
                std::process::exit(1);
            }
            let rep = deobfuscate::deobfuscate_binary(&args[2], &args[3], 32);
            println!("{}", serde_json::to_string(&rep).unwrap());
        }

        "dumper" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core dumper <plat> <file>");
                std::process::exit(1);
            }
            let rep = deobfuscate::extract_assets_scan(&args[2], &args[3]);
            println!("{}", serde_json::to_string(&rep).unwrap());
        }

        "entropy" => {
            if args.len() < 3 {
                eprintln!("Usage: megadaemon-core entropy <file> [block_size]");
                std::process::exit(1);
            }
            let file_path = &args[2];
            let block_size = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1024);

            let mut f = match File::open(file_path) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Error opening file: {}", e);
                    std::process::exit(1);
                }
            };
            let mut data = Vec::new();
            if let Err(e) = f.read_to_end(&mut data) {
                eprintln!("Error reading file: {}", e);
                std::process::exit(1);
            }

            let res = entropy::scan_entropy(&data, block_size);
            println!("{}", serde_json::to_string(&res).unwrap());
        }

        "pattern" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core pattern <file> <pattern_str> [max_matches]");
                std::process::exit(1);
            }
            let file_path = &args[2];
            let pat_str = &args[3];
            let max_matches = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(128);

            let mut f = match File::open(file_path) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Error opening file: {}", e);
                    std::process::exit(1);
                }
            };
            let mut data = Vec::new();
            if let Err(e) = f.read_to_end(&mut data) {
                eprintln!("Error reading file: {}", e);
                std::process::exit(1);
            }

            match pattern::scan_pattern(&data, pat_str, max_matches) {
                Ok(res) => println!("{}", serde_json::to_string(&res).unwrap()),
                Err(err) => {
                    eprintln!("Pattern error: {}", err);
                    std::process::exit(1);
                }
            }
        }

        "pointers" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core pointers <file> <base_vaddr_hex> [is_be] [max]");
                std::process::exit(1);
            }
            let file_path = &args[2];
            let base_hex = args[3].trim_start_matches("0x");
            let base_vaddr = u32::from_str_radix(base_hex, 16).unwrap_or(0);
            let is_be = args
                .get(4)
                .map(|s| s == "1" || s == "true")
                .unwrap_or(false);
            let max_res = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(128);

            let mut f = match File::open(file_path) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Error opening file: {}", e);
                    std::process::exit(1);
                }
            };
            let mut data = Vec::new();
            if let Err(e) = f.read_to_end(&mut data) {
                eprintln!("Error reading file: {}", e);
                std::process::exit(1);
            }

            let res = pointers::scan_pointers_32(&data, base_vaddr, is_be, max_res);
            println!("{}", serde_json::to_string(&res).unwrap());
        }

        "diff" => {
            if args.len() < 4 {
                eprintln!("Usage: megadaemon-core diff <file_a> <file_b> [base_vaddr_hex] [max]");
                std::process::exit(1);
            }
            let file_a_path = &args[2];
            let file_b_path = &args[3];
            let base_hex = args
                .get(4)
                .map(|s| s.trim_start_matches("0x"))
                .unwrap_or("0");
            let base_vaddr = u32::from_str_radix(base_hex, 16).unwrap_or(0);
            let max_res = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(256);

            let mut f_a = match File::open(file_a_path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error opening file_a: {}", e);
                    std::process::exit(1);
                }
            };
            let mut buf_a = Vec::new();
            f_a.read_to_end(&mut buf_a).unwrap();

            let mut f_b = match File::open(file_b_path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Error opening file_b: {}", e);
                    std::process::exit(1);
                }
            };
            let mut buf_b = Vec::new();
            f_b.read_to_end(&mut buf_b).unwrap();

            let res = snapshot::diff_snapshots(&buf_a, &buf_b, base_vaddr, max_res);
            println!("{}", serde_json::to_string(&res).unwrap());
        }

        "shm-status" => match shm::read_shm_status() {
            Ok(hdr) => println!("{}", serde_json::to_string(&hdr).unwrap()),
            Err(e) => {
                eprintln!("SHM Error: {}", e);
                std::process::exit(1);
            }
        },

        "shm-frame-bmp" => {
            // v13.2 (HW-08): la cabecera la escribe la app y puede ser
            // incoherente (1000x1000 en un fichero de 1 MB) -> el slice
            // reventaba con panic.  Ahora se usa la instantánea validada.
            let bmp_bytes = match shm::read_frame_snapshot() {
                Ok(snap) if snap.header.width > 0 && snap.header.height > 0 => shm::encode_bmp_24(
                    snap.header.width as u32,
                    snap.header.height as u32,
                    &snap.rgb,
                ),
                _ => shm::generate_tactical_hud_bmp(682, 448),
            };
            io::stdout().write_all(&bmp_bytes).unwrap();
        }

        cmd => {
            eprintln!("Unknown command: {}", cmd);
            print_usage();
            std::process::exit(1);
        }
    }
}
