//! Arnés de la instrumentación por hooks (`inject_hooks` + inyector `ptrace`).
//!
//! Qué fija:
//! * la tool MCP `inject_hooks` está **declarada** con sus cuatro acciones;
//! * su `dispatch` responde con un **error claro** (nunca un panic) cuando
//!   falta el payload o el inyector;
//! * el inyector resuelve PID y nombres de proceso leyendo `/proc`;
//! * la **llamada remota** (`dlopen` en el proceso objetivo) se construye con
//!   los registros correctos.
//!
//! MEDIDO vs NO VERIFICADO: aquí no se puede **inyectar** de verdad (haría
//! falta un proceso ajeno y `ptrace` con permisos); lo que se comprueba aquí
//! es la **construcción** de la llamada y el **comportamiento del dispatch**.
//! La inyección real se verifica en el móvil (ver
//! `docs/INYECCION-HOOKS.md`).

use megadaemon_core::android;
use megadaemon_core::inject;
use megadaemon_core::inject::ViaMemoria;
use serde_json::json;

#[test]
fn la_tool_inject_hooks_esta_declarada() {
    let defs = android::tool_definitions();
    let arr = defs.as_array().expect("tool_definitions devuelve una lista");
    let d = arr
        .iter()
        .find(|d| d["name"] == "inject_hooks")
        .expect("la tool inject_hooks tiene que estar declarada");
    let acciones = d["inputSchema"]["properties"]["action"]["enum"]
        .as_array()
        .expect("la acción tiene que ser un enum");
    let lista: Vec<&str> = acciones.iter().filter_map(|v| v.as_str()).collect();
    for a in ["estado", "instalar", "inyectar", "leer"] {
        assert!(lista.contains(&a), "falta la acción {}", a);
    }
    assert_eq!(d["inputSchema"]["required"][0], "action");
}

#[test]
fn el_dispatch_de_una_accion_desconocida_no_panea() {
    let r = android::dispatch("inject_hooks", &json!({"action": "accion_que_no_existe"}));
    assert_eq!(r["ok"], json!(false));
    assert!(r["error"].is_string(), "el error debe ser una cadena: {r}");
}

#[test]
fn el_dispatch_sin_accion_no_panea() {
    let r = android::dispatch("inject_hooks", &json!({}));
    assert_eq!(r["ok"], json!(false));
    assert_eq!(r["error"], "falta_action");
}

#[test]
fn el_dispatch_de_inyectar_sin_payload_da_un_error_claro() {
    // En esta máquina no hay móvil: el inyector no está instalado ⇒ la tool
    // tiene que decirlo con claridad (y NO petar).
    let r = android::dispatch(
        "inject_hooks",
        &json!({"action": "inyectar", "package": "com.agy.inexistente", "pid": 999999}),
    );
    assert_eq!(r["ok"], json!(false));
    let err = r["error"].as_str().unwrap_or("");
    assert!(
        matches!(
            err,
            "falta_inyector" | "falta_payload" | "proceso_no_encontrado" | "via_desconocida"
        ),
        "error inesperado: {r}"
    );
    assert!(
        r["hint"].as_str().unwrap_or("").len() > 10,
        "el error tiene que explicar qué hacer: {r}"
    );
}

#[test]
fn el_inyector_resuelve_pids_y_nombres_desde_proc() {
    let dir = std::env::temp_dir().join(format!("md-inject-arnes-{}", std::process::id()));
    let crear = |pid: i32, cmd: &str| {
        let d = dir.join(pid.to_string());
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join("cmdline"), cmd.as_bytes());
    };
    crear(4242, "com.agy.antigravity\0");
    crear(4243, "com.agy.antigravity:remote\0");
    crear(4244, "system_server\0");

    let todos = inject::listar_procesos_en(&dir, "");
    assert_eq!(todos.len(), 3, "deben verse los tres procesos de mentira");

    let filtrados = inject::listar_procesos_en(&dir, "antigravity");
    assert_eq!(filtrados.len(), 2);

    assert_eq!(
        inject::resolver_pid_en(&dir, "4242").unwrap_or(-1),
        4242,
        "un PID numérico se resuelve tal cual"
    );
    // Con dos candidatos por nombre hay que decir cuáles son (o fallar con
    // el error que los lista), pero nunca devolver un PID inventado.
    match inject::resolver_pid_en(&dir, "antigravity") {
        Ok(p) => assert!(p == 4242 || p == 4243),
        Err(e) => assert!(e.contains("case con"), "error poco claro: {e}"),
    }
    let e = inject::resolver_pid_en(&dir, "no-existe-este").unwrap_err();
    assert!(e.contains("no hay ningún proceso"), "error poco claro: {e}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn la_llamada_remota_se_construye_con_los_registros_correctos() {
    // MEDIDO (2026-11-23): con `default()` el SP vale 0 y la comprobacion
    // "la ruta va por debajo del SP" no puede cumplirse => test rojo por un
    // motivo artificial.  Aqui hay registros REALISTAS.
    let mut regs = inject::Registros::default();
    #[cfg(target_arch = "aarch64")]
    {
        regs.sp = 0x7fff_0000_0000;
    }
    #[cfg(target_arch = "x86_64")]
    {
        regs.rsp = 0x7fff_0000_0000;
    }
    let llamada = inject::preparar_llamada(&regs, 0x7fff_1234_5678, 0x1000);
    assert_eq!(llamada.pc, 0x7fff_1234_5678, "el PC debe ser dlopen");
    assert_eq!(llamada.arg0, 0x1000, "el primer argumento es la ruta");
    assert_eq!(llamada.arg1, 0x2 | 0x100, "RTLD_NOW|RTLD_GLOBAL");
    assert_eq!(llamada.magico, 0, "la dirección de retorno es la mágica (0)");

    // La ruta se escribe POR DEBAJO del SP original (memoria libre).
    #[cfg(target_arch = "aarch64")]
    assert!(llamada.arg0 < regs.sp);
    #[cfg(target_arch = "x86_64")]
    assert!(llamada.arg0 < regs.rsp);

    // Al aplicarla, el PC pasa a dlopen y el SP se mueve a la ranura.
    let mut r = regs;
    llamada.aplicar(&mut r);
    assert_eq!(inject::pc_actual(&r), 0x7fff_1234_5678);
    assert_eq!(inject::sp_actual(&r), llamada.sp);
}

#[test]
fn las_vias_de_memoria_se_conmutan() {
    assert_eq!(ViaMemoria::desde_texto("ptrace"), Some(ViaMemoria::Ptrace));
    assert_eq!(ViaMemoria::desde_texto("mem"), Some(ViaMemoria::ProcMem));
    assert_eq!(ViaMemoria::desde_texto("otra"), None);
}
