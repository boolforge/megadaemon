/* ============================================================
   GENERADOR DE PROMPT "AGENTE EXTERNO" (MegaDaemon)
   Botón flotante que genera, en el propio móvil, un prompt listo para
   copiar/pegar a un agente externo: datos de conexión, catálogo de tools,
   estado del nodo y últimos hallazgos del enjambre.
   100% cliente: no toca la lógica de la UI.
   ============================================================ */
(function () {
  "use strict";
  if (window.__MD_PROMPT_GEN__) return;
  window.__MD_PROMPT_GEN__ = true;

  var MD_KEY_STORE = "md_apikey";
  var TUNNEL_HINT = "https://pot-implied-approach-input.trycloudflare.com"; // lee /data/local/tmp/megadaemon/tunnel.url (rota)
  var TOOLS_FALLBACK = ["adb","agent","alerts","apk_inventory","app_control","app_intel","battery_saver","bookmarks","capabilities","conversation_stats","dex_decompile","find_duplicates","fs_read","fs_write","get_conversation","git_ops","harvest_sessions","identity_and_web","ingest_export","kernel_and_blocks","list_conversations","magisk_module","megadaemon_cirx_analyze","megadaemon_die_binwalk","megadaemon_mba_simplify","megadaemon_npu_cv","megadaemon_rizin","messaging","minimax_chat","profile","props_and_settings","repo_updates","root_exec","root_hide","root_stealth","screen_and_ui","search_conversations","selinux_control","signing_vault","ssh_remote","swarm_chat","sync_conversations","sync_web_conversations","system_overview","task_queue"];
  var P1P3_FALLBACK = [
    "P1 NPU/EDEN (11 hallazgos): runtime EDEN v1.6.20; el driver /dev/vertex10 usa interfaz VS4L (type 'V'=0x56), NO el ioctl clasico de Samsung; 9 codigos VS4L / 20 llamadas reales (la tabla inicial de 38/4.384 era un falso positivo del extractor y ya se corrigio); el .rc hace chmod 0644 /dev/vertex10 (el nodo del NPU queda legible por cualquier app); el compilador EDEN viaja en el producto (9 libnpuc_* + libnpu_compiler.so + libsait_npu_compiler.so); NNAPI HAL eden-drv sobre NNAPI 1.3; modelos .nnc eden167/eden162c.",
    "P2 TEE (9 hallazgos, solo estructura): el mundo seguro es TEEGris (no GreenTea) - tzsw.img con TEEGRIS ver, P_tee_, Root Task; tzar.img es el TrustZone AR e incluye /secureos/tls.c (la pila TLS vive en el mundo seguro); keystore.bin (firma SLSI) es el contenedor por dispositivo SM-G998B_EUR_XX_EKEY0 (formato, no claves); RPMB (/dev/rpmb) y FBE (FBE_s) se resuelven en el mundo seguro; en userspace keymaster@4.0-service, keymaster@4.0_strongbox-service, gatekeeper.exynos2100.so (especifico del Exynos 2100), keystore2, gatekeeperd.",
    "P3 boot chain (4 hallazgos): paquete de bootloader de stock del G998B = 12 imagenes (sboot.bin=SBL, tzar, tzsw, ldfw, harx, keystore, ssp, dpm=U-Boot d00dfeed, uh=GREENTEA, up_param=android_, vddcal_fw); el SBL implementa AVB encadenado (chained vbmeta image has non-zero flags, DEBUG_VBMETA, $(ANDROID_VBMETA_PARTUUID), $(ANDROID_VERITY_MODE)) y FMM Lock: el unlock esta gated por manid y warranty, con 'try jtag unlock' como via de servicio; el boot.img de One UI 8 del VN-ROM va reempaquetado con androidboot.selinux=permissive (no es stock) - los flags AVB reales hay que leerlos en el movil."
  ];

  function el(id) { return document.getElementById(id); }
  function storedKey() {
    try { return window.localStorage.getItem(MD_KEY_STORE) || ""; } catch (e) { return ""; }
  }
  function authHeaders(extra) {
    var h = Object.assign({}, extra || {});
    if (window.MEGADAEMON_TOKEN) h["X-Seere-Auth-Token"] = window.MEGADAEMON_TOKEN;
    var k = storedKey();
    if (k) h["X-API-Key"] = k;
    return h;
  }
  function mcp(method, params) {
    var k = storedKey();
    var body = { jsonrpc: "2.0", id: 1, method: method, params: params || {} };
    var h = { "Content-Type": "application/json" };
    if (k) h["X-API-Key"] = k;
    if (window.MEGADAEMON_TOKEN) h["X-Seere-Auth-Token"] = window.MEGADAEMON_TOKEN;
    return fetch("/mcp", { method: "POST", headers: h, body: JSON.stringify(body), cache: "no-store" })
      .then(function (r) { return r.ok ? r.json() : Promise.reject(new Error("HTTP " + r.status)); });
  }
  function getJson(url) {
    return fetch(url, { cache: "no-store", headers: authHeaders() })
      .then(function (r) { return r.ok ? r.json() : Promise.reject(new Error("HTTP " + r.status)); });
  }

  function buildPrompt(opts) {
    var tools = (opts.tools && opts.tools.length) ? opts.tools : TOOLS_FALLBACK;
    var find = opts.findings && opts.findings.length ? opts.findings : null;
    var ov = opts.overview || "";
    var L = [];
    L.push("# ENCARGO: nodo RE para el enjambre MegaDaemon — Samsung S21 Ultra (Exynos 2100)");
    L.push("");
    L.push("## 0. Quién eres");
    L.push("Eres un NODO WORKER del enjambre MegaDaemon. El nodo MAESTRO es un Samsung S21 Ultra");
    L.push("(SM-G998B / p3s, SoC Exynos 2100, 5 nm, Mali-G78 MP14, NPU ~4 TOPS) con root + Magisk");
    L.push("27.2-kitsune y SELinux Enforcing, y hace de maestro de la web y del estado compartido.");
    L.push("Dos mitades: (A) RE de prioridad sobre ese Exynos 2100. (B) BROADCAST: todo lo que");
    L.push("descubras tiene que llegar a TODOS los nodos.");
    L.push("");
    L.push("## 1. Conectar (esto es lo primero, no preguntes nada)");
    L.push("Endpoint MCP (tunel publico del movil):");
    L.push("    " + TUNNEL_HINT + "/mcp");
    L.push("    (si da 530 o error de DNS, el quick tunnel ha rotado: leelo del movil en");
    L.push("     /data/local/tmp/megadaemon/tunnel.url y prueba de nuevo)");
    L.push("Cabeceras obligatorias en cada POST:");
    if (opts.includeSecrets) {
      L.push("    X-API-Key: " + opts.apiKey);
      L.push("    X-Seere-Auth-Token: " + opts.token);
      L.push("    (el token de sesion tambien esta en el movil: /data/local/tmp/megadaemon/state/cockpit_token)");
    } else {
      L.push("    X-API-Key: <la API key de la instancia>");
      L.push("    X-Seere-Auth-Token: <el token de sesion>");
    }
    L.push("    Content-Type: application/json");
    L.push("    OJO (medido): con SOLO X-API-Key el MCP devuelve 403");
    L.push("    {\"error\":\"unauthorized\",\"hint\":\"falta la cabecera X-Seere-Auth-Token\"}.");
    L.push("    Las DOS cabeceras son obligatorias en cada POST, tambien tools/list:");
    L.push("    el gate de API key y el de token de sesion son independientes.");
    L.push("    El token de sesion ROTA en cada reinicio del daemon => si el MCP");
    L.push("    empieza a dar 403, canjea otra vez (§1.b), no lo reintentes con la key.");
    L.push("El body es JSON-RPC 2.0 puro:");
    L.push("    {\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\",\"params\":{}}");
    L.push("    {\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"system_overview\",\"arguments\":{}}}");
    L.push("Si el tunel cae, la ruta estable dentro del movil es http://127.0.0.1:9090/mcp.");
    L.push("PRIMER MOVIMIENTO OBLIGATORIO: tools/list completo y luego system_overview.");
    L.push("");
    L.push("## 1.b. Canje de credenciales (si el MCP da 403)");
    L.push("    POST <URL>/api/session/exchange");
    L.push("    Content-Type: application/json");
    L.push("    {\"api_key\":\"<tu API key>\"}   # la key va EN EL CUERPO, no solo en la cabecera");
    L.push("    => 200 {\"ok\":true,\"token\":\"<token de sesion>\"}");
    L.push("Usa ese token como X-Seere-Auth-Token. Si da 403 con la key correcta,");
    L.push("la key no es la vigente: no la pegues en ningun sitio, pidela por el bus.");
    L.push("");
    L.push("## 2. Tu catalogo de tools (" + tools.length + ")");
    L.push("  " + tools.join(", "));
    L.push("  RE: megadaemon_die_binwalk, megadaemon_rizin, dex_decompile, apk_inventory,");
    L.push("      megadaemon_cirx_analyze, megadaemon_mba_simplify, megadaemon_npu_cv");
    L.push("  Sistema: root_exec, fs_read, fs_write, app_control, app_intel, adb, props_and_settings,");
    L.push("      selinux_control, magisk_module, kernel_and_blocks, root_hide, root_stealth");
    L.push("  Enjambre: task_queue, capabilities, agent, alerts, swarm_chat, git_ops, signing_vault");
    L.push("Si necesitas una tool que no esta: no la inventes, dilo y sigue con root_exec.");
    L.push("");
    L.push("## 3. REGLA #1 - BROADCAST DE TODO (lo mas importante)");
    L.push("El estado compartido vive DENTRO del movil, en /data/local/tmp/megadaemon/state/:");
    L.push("    mesh_board.json        tablon: peers / claims / tasks / agents  (+ .lock: flock)");
    L.push("    mesh_journal.ndjson    journal de eventos: una linea JSON por evento  <-- BROADCAST");
    L.push("    sync/re_<nodo>.ndjson  una linea JSON por hallazgo (NDJSON)");
    L.push("    agents/<nodo>.json     identidad del nodo");
    L.push("    task_queue.json        cola de tareas");
    L.push("Cada hallazgo tuyo sale por los canales 1-4 (identidad, sync, journal, tablon) y");
    L.push("opcionalmente swarm_chat con un resumen de 10 lineas. Formato de una linea:");
    L.push("  {\"ts\":\"...\",\"node\":\"<tu-nodo>\",\"kind\":\"reverse|npu|tee|bootloader\",\"target\":\"/ruta\",");
    L.push("   \"sha256\":\"...\",\"size\":123,\"title\":\"...\",\"finding\":\"...\",");
    L.push("   \"offsets\":{\"hex\":\"0x1234\"},\"artifacts\":[\"<ruta>\"],\"confidence\":\"high|medium|low\"}");
    L.push("Reglas: toma mesh_board.json.lock (flock, NUNCA editor) y sueltalo; no pises una");
    L.push("tarea reclamada por otro nodo; pagina (64 KB por tool, 300 s max); si algo falla,");
    L.push("reporta el estado literal y no inventes; NINGUN secreto en el estado compartido");
    L.push("(ni tokens, ni cookies, ni claves) ni en el repo.");
    L.push("");
    L.push("## 4. TU ENCARGO RE - PRIORIDAD EXYNOS 2100");
    L.push("P1 NPU/EDEN (vertex10, ioctls VS4L, runtime EDEN, tensores) · P2 TEE/secure (solo");
    L.push("  estructura, nunca material criptografico) · P3 boot chain (SBL, boot.img/vbmeta,");
    L.push("  flags AVB) · P4 vendor (.so de camera/ISP/DRM/audio, HAL de /vendor/bin/hw,");
    L.push("  governors, ioctls propios del Exynos) · P5 apps Samsung nativas (anti-root/anti-debug,");
    L.push("  mapa de desofuscacion, plan de parche) · P6 radio/ROM (solo inventario).");
    L.push("Para CADA artefacto: ruta, sha256, tamano, tipo (DiE) y el hallazgo con offset/funcion.");
    L.push("");
    L.push("## 5. Estado real del movil (medido, no te lo inventes)");
    L.push("  " + ov.replace(/\n/g, "\n  "));
    L.push("  RAM 10575 MB con ~391 MB libres; disco /storage/emulated al 98% (5,9 GB libres).");
    L.push("  => NADA de volcados in situ: 32 GB no caben. Todo por Drive + broadcast con sha256.");
    L.push("");
    L.push("## 6. Lo que ya sabe el enjambre (no lo repitas desde cero)");
    for (var i = 0; i < P1P3_FALLBACK.length; i++) L.push("  - " + P1P3_FALLBACK[i]);
    if (find && find.length) {
      L.push("");
      L.push("## 7. Ultimos hallazgos del enjambre (vivos, " + find.length + ")");
      for (var j = 0; j < find.length && j < 25; j++) {
        var f = find[j];
        L.push("  - [" + (f.agent || "?") + "/" + (f.kind || "?") + "] " + (f.title || "") +
               (f.target ? "  (" + f.target + ")" : ""));
      }
    }
    L.push("");
    L.push("## 8. Orden de tus primeros movimientos");
    L.push("  1. tools/list -> catalogo vivo");
    L.push("  2. system_overview -> ficha del movil");
    L.push("  3. registra tu identidad + primer hallazgo en los 4 canales (con lock)");
    L.push("  4. P1 completo en trozos, con broadcast por hallazgo");
    L.push("  5. P2..P6 sin saltarte el broadcast");
    L.push("Todo por MegaDaemon: nada de chat, correo ni issues publicos para coordinar estado.");
    return L.join("\n");
  }

  function collect() {
    var out = { tools: [], findings: [], overview: "" };
    var p = mcp("tools/list").then(function (j) {
      var ts = (j && j.result && j.result.tools) || [];
      out.tools = ts.map(function (t) { return t.name; });
    }).catch(function () {});
    var p2 = mcp("tools/call", { name: "system_overview", arguments: {} }).then(function (j) {
      var c = (j && j.result && j.result.content) || [];
      out.overview = c.map(function (x) { return x.text || ""; }).join("\n").slice(0, 1500);
    }).catch(function () {});
    var p3 = getJson("/api/mesh_findings").then(function (j) {
      out.findings = (j && j.findings) || [];
    }).catch(function () {});
    return Promise.all([p, p2, p3]).then(function () { return out; });
  }

  function buildUI() {
    var btn = document.createElement("button");
    btn.id = "mdPromptBtn";
    btn.textContent = "📋 Prompt agente externo";
    btn.style.cssText = "position:fixed;right:10px;bottom:14px;z-index:99999;padding:9px 13px;" +
      "border-radius:999px;border:1px solid #3a4a66;background:#101827;color:#e8eefc;font:13px system-ui;" +
      "box-shadow:0 4px 14px rgba(0,0,0,.45);cursor:pointer";
    btn.onclick = function () { openModal(); };
    document.body.appendChild(btn);

    var wrap = document.createElement("div");
    wrap.id = "mdPromptModal";
    wrap.style.cssText = "position:fixed;inset:0;z-index:100000;display:none;background:rgba(0,0,0,.72);" +
      "padding:14px;box-sizing:border-box";
    wrap.innerHTML =
      '<div style="max-width:900px;margin:0 auto;background:#0d1420;border:1px solid #2b3a52;border-radius:12px;' +
      'padding:14px;height:92vh;display:flex;flex-direction:column;gap:8px">' +
      '<div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap">' +
      '<b style="color:#e8eefc;font:14px system-ui">Prompt para un agente externo</b>' +
      '<label style="color:#9fb3d1;font:12px system-ui"><input type="checkbox" id="mdPromptSecrets"> incluir API key y token</label>' +
      '<label style="color:#9fb3d1;font:12px system-ui">key: <input id="mdPromptKey" style="width:150px;background:#0a111c;color:#cfe;border:1px solid #2b3a52;border-radius:6px;padding:3px"></label>' +
      '<button id="mdPromptCopy" style="margin-left:auto;padding:6px 12px;border-radius:8px;border:1px solid #3a4a66;background:#16233a;color:#e8eefc;cursor:pointer">Copiar</button>' +
      '<button id="mdPromptDl" style="padding:6px 12px;border-radius:8px;border:1px solid #3a4a66;background:#16233a;color:#e8eefc;cursor:pointer">Descargar .md</button>' +
      '<button id="mdPromptClose" style="padding:6px 10px;border-radius:8px;border:1px solid #3a4a66;background:#2a1620;color:#e8eefc;cursor:pointer">Cerrar</button>' +
      '</div>' +
      '<div id="mdPromptStatus" style="color:#9fb3d1;font:12px system-ui">recolectando catalogo, estado y hallazgos…</div>' +
      '<textarea id="mdPromptText" spellcheck="false" style="flex:1;width:100%;background:#080e17;color:#d7e3f4;border:1px solid #2b3a52;border-radius:8px;padding:10px;font:12px ui-monospace,monospace;resize:none"></textarea>' +
      '</div>';
    document.body.appendChild(wrap);

    wrap.querySelector("#mdPromptClose").onclick = function () { wrap.style.display = "none"; };
    wrap.addEventListener("click", function (e) { if (e.target === wrap) wrap.style.display = "none"; });
    wrap.querySelector("#mdPromptKey").value = storedKey();
    wrap.querySelector("#mdPromptCopy").onclick = function () {
      var t = wrap.querySelector("#mdPromptText");
      t.select();
      try {
        if (navigator.clipboard && navigator.clipboard.writeText) navigator.clipboard.writeText(t.value);
        else document.execCommand("copy");
        wrap.querySelector("#mdPromptStatus").textContent = "copiado al portapapeles ✓";
      } catch (e) { wrap.querySelector("#mdPromptStatus").textContent = "no se pudo copiar: selecciona el texto a mano"; }
    };
    wrap.querySelector("#mdPromptDl").onclick = function () {
      var t = wrap.querySelector("#mdPromptText");
      var blob = new Blob([t.value], { type: "text/markdown;charset=utf-8" });
      var a = document.createElement("a");
      a.href = URL.createObjectURL(blob);
      a.download = "prompt_agente_externo_megadaemon.md";
      document.body.appendChild(a); a.click(); a.remove();
      wrap.querySelector("#mdPromptStatus").textContent = "descargado ✓";
    };
  }

  function openModal() {
    var wrap = el("mdPromptModal");
    if (!wrap) return;
    wrap.style.display = "block";
    var st = wrap.querySelector("#mdPromptStatus");
    collect().then(function (data) {
      wrap.querySelector("#mdPromptText").value = buildPrompt({
        tools: data.tools,
        findings: data.findings,
        overview: data.overview,
        apiKey: (wrap.querySelector("#mdPromptKey").value || storedKey()).trim(),
        token: window.MEGADAEMON_TOKEN || "",
        includeSecrets: wrap.querySelector("#mdPromptSecrets").checked
      });
      st.textContent = "listo · " + (data.tools.length || TOOLS_FALLBACK.length) + " tools · " +
        data.findings.length + " hallazgos vivos" + (data.overview ? " · overview ok" : " · overview no disponible");
    }).catch(function (e) {
      wrap.querySelector("#mdPromptText").value = buildPrompt({
        tools: [], findings: [], overview: "",
        apiKey: storedKey(), token: window.MEGADAEMON_TOKEN || "",
        includeSecrets: wrap.querySelector("#mdPromptSecrets").checked
      });
      st.textContent = "generado con datos estaticos (el MCP no respondio: " + (e && e.message) + ")";
    });
  }

  function boot() { try { buildUI(); } catch (e) { console.warn("md prompt gen:", e); } }
  if (document.body) boot();
  else document.addEventListener("DOMContentLoaded", boot);
})();
