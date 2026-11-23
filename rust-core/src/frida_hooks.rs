//! Hooks de Frida para observar Antigravity (SQLite, red y ficheros) **sin
//! tocar sus datos**.
//!
//! Se inyectan con `frida_dbi.rs` (`generate_frida_script` genera el
//! cargador; estos son los ganchos concretos).  Todo lo que sale de aquí
//! pasa por el shim (`agyshim.rs`), que **redacta credenciales** antes de
//! escribirlo en el NDJSON.
//!
//! Reglas:
//! * **observar**, no modificar: los ganchos sólo leen y registran;
//! * **nunca** volcar el contenido de las consultas: sólo la sentencia y
//!   los parámetros *redactados*;
//! * **nada de secretos**: si la SQL o la ruta huele a credencial, se marca
//!   `[REDACTADO]` en vez del valor.

/// Hook de **SQLite**: registra qué tablas/consultas toca el proceso.
///
/// Objetivo real: la DB de sesiones de Antigravity.  Con esto sabemos qué
/// escribe la app (y desde qué tabla) **sin** bloquearla ni escribir en ella.
pub const HOOK_SQLITE_JS: &str = r#"
// hook_sqlite.js — observador de SQLite (solo lectura)
'use strict';
var MAX_EVENTOS = 500;
var eventos = [];

function redact(sql) {
    if (!sql) return sql;
    // credenciales: se marca el valor, no se imprime
    var s = sql.replace(/('[^']*(?:key|token|secret|password|passwd|api[_-]?key|credential)[^']*')\s*=\s*'[^']*'/gi,
                         "$1='[REDACTADO]'");
    return s.length > 400 ? s.slice(0, 400) + '...' : s;
}

function registrar(tipo, detalle) {
    eventos.push({ t: Date.now(), tipo: tipo, detalle: detalle });
    if (eventos.length > MAX_EVENTOS) eventos.shift();
    send({ hook: 'sqlite', tipo: tipo, detalle: detalle });
}

function hook(nombre, alEntrar, alSalir) {
    var p = Module.findExportByName(null, nombre);
    if (!p) { registrar('ausente', nombre); return; }
    Interceptor.attach(p, {
        onEnter: function (args) { try { this.ctx = alEntrar ? alEntrar(args) : null; } catch (e) {} },
        onLeave: function (ret) { try { if (alSalir) alSalir(this.ctx, ret); } catch (e) {} }
    });
}

hook('sqlite3_open', function (args) {
    try { return { ruta: args[0].readCString() }; } catch (e) { return null; }
}, function (ctx) {
    if (ctx && ctx.ruta) registrar('open', ctx.ruta);
});

hook('sqlite3_prepare_v2', function (args) {
    try { return { sql: args[1].readCString() }; } catch (e) { return null; }
}, function (ctx) {
    if (ctx && ctx.sql) registrar('prepare', redact(ctx.sql));
});

hook('sqlite3_step', function () { return {}; }, function (ctx) {
    registrar('step', 'stmt');
});

hook('sqlite3_finalize', function () { return {}; }, function () {
    registrar('finalize', 'stmt');
});

"#;

/// Hook de **red**: qué hosts contacta el proceso y con qué URL.
///
/// Sirve para VERIFICAR el parche del endpoint: si tras parchear sigue
/// apareciendo `daily-cloudcode-pa.googleapis.com`, el parche no está aplicado.
pub const HOOK_RED_JS: &str = r#"
// hook_red.js — observador de red (solo lectura)
'use strict';
var MAX = 500;

function registrar(tipo, detalle) {
    send({ hook: 'red', tipo: tipo, detalle: detalle });
}

function hook(nombre, alEntrar) {
    var p = Module.findExportByName(null, nombre);
    if (!p) return;
    Interceptor.attach(p, { onEnter: function (args) { try { alEntrar(args); } catch (e) {} } });
}

// connect(): host:puerto de cada socket
hook('connect', function (args) {
    try {
        var sa = args[1];
        var fam = sa.readU16();
        var host = fam === 2 ? sa.add(4).readCString() : sa.add(4).readCString();
        var puerto = fam === 2 ? (sa.add(2).readU8() << 8) | sa.add(3).readU8() : 0;
        registrar('connect', host + ':' + puerto);
    } catch (e) {}
});

// SSL_write / SSL_read (BoringSSL): los bytes de la petición
function sslhook(nombre) {
    var p = Module.findExportByName(null, nombre);
    if (!p) return;
    Interceptor.attach(p, {
        onEnter: function (args) {
            try {
                var n = args[2].toInt32();
                if (n > 0 && n < 2048) {
                    var b = args[1].readByteArray(n);
                    registrar(nombre, b);
                }
            } catch (e) {}
        }
    });
}
sslhook('SSL_write');
sslhook('SSL_read');
"#;

/// Hook de **ficheros**: qué ficheros abre el proceso (y cuáles escribe).
///
/// Permite ver si Antigravity reescribe su estado (BYOM) o borra algo que
/// MegaDaemon controla.
pub const HOOK_FICHEROS_JS: &str = r#"
// hook_ficheros.js — observador de ficheros (solo lectura)
'use strict';
function registrar(tipo, ruta) {
    send({ hook: 'ficheros', tipo: tipo, detalle: ruta });
}

function hook(nombre, conFd) {
    var p = Module.findExportByName(null, nombre);
    if (!p) return;
    Interceptor.attach(p, {
        onEnter: function (args) {
            try {
                var ruta = args[conFd ? 1 : 0].readCString();
                if (ruta && ruta.length > 1 && ruta.indexOf('\u0000') < 0) {
                    registrar(nombre, ruta);
                }
            } catch (e) {}
        }
    });
}
hook('open', false);
hook('open64', false);
hook('openat', true);
hook('fopen', false);
hook('unlink', false);
hook('rename', false);
"#;

/// Lista de hooks disponibles (nombre -> script).
pub fn hooks_disponibles() -> Vec<(&'static str, &'static str)> {
    vec![
        ("sqlite", HOOK_SQLITE_JS),
        ("red", HOOK_RED_JS),
        ("ficheros", HOOK_FICHEROS_JS),
    ]
}

/// El script completo de un hook por su nombre.
pub fn hook_por_nombre(nombre: &str) -> Option<&'static str> {
    hooks_disponibles()
        .into_iter()
        .find(|(n, _)| *n == nombre)
        .map(|(_, s)| s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_tres_hooks_existen_y_redactan() {
        assert!(hook_por_nombre("sqlite").is_some());
        assert!(hook_por_nombre("red").is_some());
        assert!(hook_por_nombre("ficheros").is_some());
        assert!(hook_por_nombre("inexistente").is_none());
        // el hook de SQLite redacta credenciales
        assert!(HOOK_SQLITE_JS.contains("REDACTADO"));
        // ninguno modifica la memoria: sólo mandan mensajes
        assert!(HOOK_SQLITE_JS.contains("send("));
        assert!(!HOOK_SQLITE_JS.contains(".write("));
    }

    #[test]
    fn el_hook_de_red_registra_el_host() {
        assert!(HOOK_RED_JS.contains("connect"));
        assert!(HOOK_RED_JS.contains("readCString"));
    }
}
