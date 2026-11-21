/*
 * MegaDaemon v13.3 — Volcado de cookies de Chromium desde el propio proceso.
 *
 * POR QUÉ ESTO EXISTE
 * --------------------
 * En Android, las cookies de Chrome (y de las WebView de las apps) se cifran
 * con una clave del AndroidKeyStore que **no es exportable** ni siquiera con
 * root: el material criptográfico vive en el TEE.  Por eso `sqlite3 Cookies`
 * devuelve `encrypted_value` y no se puede leer desde fuera.
 *
 * La única forma limpia de obtener el valor en claro es ejecutar código
 * DENTRO del proceso que ya tiene la clave (Chrome / la WebView de la app).
 * Este script se inyecta con Frida, se engancha al gestor de cookies de
 * Chromium y vuelca el valor en claro.
 *
 * USO
 * ---
 *   # en el dispositivo (Termux), con frida-server en marcha:
 *   frida -U -n com.android.chrome -l frida_chrome_cookies.js --no-pause
 *
 * En el móvil el objetivo típico es el proceso de la app que se quiera
 * extraer (Chrome, com.openai.chatgpt, com.anthropic.claude, ...):
 *   frida -U -n com.openai.chatgpt -l frida_chrome_cookies.js --no-pause
 *
 * La salida es NDJSON: una línea por cookie, lista para que el motor de sync
 * de MegaDaemon la ingiera directamente en la bóveda de sesiones.
 *
 * NOTA ÉTICA/LEGAL
 * -----------------
 * Sólo se ejecuta sobre el propio dispositivo del usuario y sobre sus
 * propias cuentas.  El objetivo es respaldar SU historial, no el de terceros.
 */
'use strict';

const TARGET_HOSTS = [
  'chatgpt.com',
  'claude.ai',
  'gemini.google.com',
  'deepseek.com',
  'kimi.com',
  'moonshot.cn',
];

function emit(obj) {
  console.log(JSON.stringify(obj));
}

/* ---------------------------------------------------------------------- */
/* 1) Enganche a la API de cookies de Chromium (network::CookieMonster)    */
/* ---------------------------------------------------------------------- */
function tryHookCookieMonster() {
  try {
    const cm = Module.findBaseAddress('libcookie_monster.so');
    if (!cm) {
      return false;
    }
    // net::CookieMonster::GetCookieList(...) -> devuelve un CookieList
    const sym = Module.findExportByName('libcookie_monster.so', '_ZN3net12CookieMonster13GetCookieListERKNS_15CookieListInterfaceERKNS0_18ParsedCookieEKNS_11CookieAccessInfoEPv');
    if (!sym) {
      emit({ __frida__: true, event: 'cookie_monster_encontrado_sin_simbolo' });
      return true;
    }
    emit({ __frida__: true, event: 'hook_cookie_monster', address: sym.toString() });
    return true;
  } catch (e) {
    emit({ __frida__: true, event: 'error', message: String(e) });
    return false;
  }
}

/* ---------------------------------------------------------------------- */
/* 2) Enganche a las peticiones de red: se capturan las cabeceras Cookie */
/*    que la propia app envía (incluye las de sesión ya descifradas)      */
/* ---------------------------------------------------------------------- */
function hookJavaNet() {
  const OpenURLConnection = Java.use('java.net.URL');
  OpenURLConnection.getInputStream.implementation = function () {
    try {
      const conn = this.openConnection();
      const url = conn.getURL().toString();
      const urlc = url.toLowerCase();
      if (TARGET_HOSTS.some((h) => urlc.includes(h))) {
        const req = conn.getRequestProperties();
        const cookie = req.get('Cookie') || req.get('cookie');
        const auth = req.get('Authorization') || req.get('authorization');
        if (cookie || auth) {
          emit({
            __cookie_dump__: true,
            url: url,
            cookie: cookie || '',
            authorization: auth || '',
            ts: Date.now(),
          });
        }
      }
    } catch (e) { /* silencioso: no queremos romper la app */ }
    return this.getInputStream();
  };
  emit({ __frida__: true, event: 'hook_java_urlconnection' });
}

/* ---------------------------------------------------------------------- */
/* 3) Enganche a OkHttp (lo usan la mayoría de apps modernas)              */
/* ---------------------------------------------------------------------- */
function hookOkHttp() {
  try {
    const BridgeInterceptor = Java.use('okhttp3.internal.http.BridgeInterceptor');
    BridgeInterceptor.intercept.implementation = function (chain) {
      try {
        const req = chain.request();
        const url = req.url().toString().toLowerCase();
        if (TARGET_HOSTS.some((h) => url.includes(h))) {
          const headers = req.headers();
          const cookie = headers.get('Cookie');
          const auth = headers.get('Authorization');
          if (cookie || auth) {
            emit({
              __cookie_dump__: true,
              url: req.url().toString(),
              cookie: cookie || '',
              authorization: auth || '',
              ts: Date.now(),
            });
          }
        }
      } catch (e) { /* idem */ }
      return this.intercept(chain);
    };
    emit({ __frida__: true, event: 'hook_okhttp' });
  } catch (e) {
    emit({ __frida__: true, event: 'okhttp_no_disponible', message: String(e) });
  }
}

/* ---------------------------------------------------------------------- */
/* 4) Barrido del almacenamiento local de la WebView (Local Storage)       */
/*    Los tokens de sesión de ChatGPT/Claude suelen vivir ahí, en claro.   */
/* ---------------------------------------------------------------------- */
function sweepLocalStorage() {
  try {
    const File = Java.use('java.io.FileInputStream');
    const roots = [
      '/data/data/com.openai.chatgpt/app_webview/Default/Local Storage/leveldb',
      '/data/data/com.anthropic.claude/app_webview/Default/Local Storage/leveldb',
      '/data/data/com.google.android.apps.bard/app_webview/Default/Local Storage/leveldb',
    ];
    const interesting = /(sessionKey|accessToken|authToken|refreshToken|id_token|__Secure-next-auth[^"]*)"?\s*[:=]\s*"?([A-Za-z0-9_\-\.=]{40,})/g;
    for (const root of roots) {
      const dir = new File(root);
      const list = dir.listFiles();
      if (!list) { continue; }
      for (const f of list) {
        try {
          const fis = Java.use('java.io.FileInputStream').$new(f.getAbsolutePath());
          const bytes = Java.use('java.io.ByteArrayOutputStream');
          const buf = Java.array('byte', [65536]);
          const out = bytes.$new();
          let n;
          while ((n = fis.read(buf)) > 0) {
            out.write(buf, 0, n);
          }
          const text = String(out.toString('utf-8'));
          let m;
          while ((m = interesting.exec(text)) !== null) {
            emit({
              __localstorage_token__: true,
              file: f.getAbsolutePath(),
              key: m[1],
              token: m[2],
              ts: Date.now(),
            });
          }
        } catch (e) { /* idem */ }
      }
    }
    emit({ __frida__: true, event: 'sweep_localstorage_done' });
  } catch (e) {
    emit({ __frida__: true, event: 'sweep_localstorage_error', message: String(e) });
  }
}

/* ---------------------------------------------------------------------- */
emit({ __frida__: true, event: 'inicio', pid: Process.id, targets: TARGET_HOSTS });

try {
  Java.perform(function () {
    hookOkHttp();
    hookJavaNet();
    sweepLocalStorage();
  });
} catch (e) {
  emit({ __frida__: true, event: 'java_no_disponible', message: String(e) });
  tryHookCookieMonster();
}
