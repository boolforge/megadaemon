/* ===========================================================================
   ANTIGRAVITY: el motor agéntico del móvil, servido por la API de MegaDaemon.
   Las rutas las sirve el daemon (rust-core/src/webui.rs) y hablan con el
   language server local de Antigravity (127.0.0.1:45157, HTTPS autofirmado)
   usando el token CSRF que publica el SPA del propio IDE.
   =========================================================================== */
function agyText(el, v) { var n = document.getElementById(el); if (n) n.textContent = (v === undefined || v === null || v === "") ? "--" : String(v); }

async function loadAntigravity() {
  try {
    var d = await apiGet("/api/v2/antigravity");
    if (!d || d.ok === false) { agyText("agyLs", "ERROR"); agyText("agyModelsInfo", d && d.error ? d.error : "error"); return; }
    var st = d.status || {};
    agyText("agyLs", st.language_server ? "vivo" : "--");
    agyText("agyAuth", st.auth && st.auth.authResult && st.auth.authResult.hasValidAuth ? "valida" : "no");
    agyText("agyModel", st.model || "--");
    agyText("agyConvCount", st.conversations !== undefined ? st.conversations : "--");
    agyText("agyCsrf", "CSRF " + (st.csrf_sha16 || "--"));
    agyText("agyDefault", "DEFAULT " + (d.default_model || "--"));
    agyText("agyModelCount", "MODELOS " + (d.model_count || 0));
    agyText("agyEnqModel", "MODELO " + (st.model || "--"));
    agyRenderModels(d.models || []);
    agyRenderTasks(d.tasks || []);
    if (!(window.__agyConvLoaded)) { await agyConversations(); }
  } catch (e) {
    agyText("agyLs", "SIN SEÑAL");
    agyText("agyModelsInfo", String(e));
  }
}

function agyRenderModels(models) {
  var tb = document.getElementById("agyModelsBody");
  if (!tb) return;
  if (!models.length) { tb.innerHTML = '<tr><td colspan="5" style="color:var(--ink-3)">catálogo vacío</td></tr>'; return; }
  var html = "";
  for (var i = 0; i < models.length; i++) {
    var m = models[i];
    var q = (m.quota_remaining_fraction === null || m.quota_remaining_fraction === undefined) ? "--" : (Math.round(m.quota_remaining_fraction * 100) + "%");
    var name = (m.display_name || "") + (m.recommended ? " ★" : "");
    html += "<tr><td>" + (m.id || "?") + "</td><td>" + (m.placeholder || "-") + "</td><td>" + name + "</td><td>" + (m.provider || "-") + "</td><td>" + q + "</td></tr>";
  }
  tb.innerHTML = html;
  agyText("agyModelsInfo", models.length + " modelos");
}

function agyRenderTasks(tasks) {
  var tb = document.getElementById("agyTasksBody");
  if (!tb) return;
  if (!tasks.length) { tb.innerHTML = '<tr><td colspan="5" style="color:var(--ink-3)">sin tareas</td></tr>'; agyText("agyTasksInfo", "0"); return; }
  var html = "";
  for (var i = 0; i < tasks.length; i++) {
    var t = tasks[i];
    html += "<tr><td>" + (t.id || "?") + "</td><td>" + (t.name || "") + "</td><td>" + (t.trigger || "-") + "</td><td>" + (t.interval_sec || "-") + "s</td><td>" + (t.enabled ? "activo" : "parado") + "</td></tr>";
  }
  tb.innerHTML = html;
  agyText("agyTasksInfo", tasks.length + " tareas");
}

async function agyConversations() {
  try {
    var d = await apiGet("/api/v2/antigravity/conversations");
    if (!d || d.ok === false) { agyText("agyConvNote", (d && d.error) || "error"); return; }
    window.__agyConvLoaded = true;
    var arr = d.conversations || [];
    var tb = document.getElementById("agyConvBody");
    var html = "";
    for (var i = 0; i < arr.length; i++) {
      var c = arr[i];
      html += "<tr><td>" + (c.cascade_id || "?") + "</td><td>" + (c.title || "") + "</td><td>" + (c.status || "") + "</td><td>" + (c.step_count || 0) + "</td></tr>";
    }
    tb.innerHTML = html || '<tr><td colspan="4" style="color:var(--ink-3)">sin conversaciones</td></tr>';
    agyText("agyConvNote", arr.length + " conversaciones");
    agyText("agyConvCount", arr.length);
  } catch (e) { agyText("agyConvNote", String(e)); }
}

async function agySearch() {
  var q = (document.getElementById("agySearchBox") || {}).value || "";
  if (!q.trim()) { agyText("agyConvNote", "falta el texto de búsqueda"); return; }
  try {
    var d = await apiGet("/api/v2/antigravity/search?q=" + encodeURIComponent(q) + "&limit=20");
    if (!d || d.ok === false) { agyText("agyConvNote", (d && d.error) || "error"); return; }
    var arr = d.results || [];
    var tb = document.getElementById("agyConvBody");
    var html = "";
    for (var i = 0; i < arr.length; i++) {
      var c = arr[i];
      html += "<tr><td>" + (c.cascadeId || c.cascade_id || "?") + "</td><td>" + (c.title || "") + "</td><td>" + (c.workspaceName || "") + "</td><td></td></tr>";
    }
    tb.innerHTML = html || '<tr><td colspan="4" style="color:var(--ink-3)">sin resultados</td></tr>';
    agyText("agyConvNote", arr.length + " resultados para «" + q + "»");
  } catch (e) { agyText("agyConvNote", String(e)); }
}

async function agyEnqueue() {
  var p = (document.getElementById("agyPrompt") || {}).value || "";
  if (!p.trim()) { agyText("agyEnqState", "falta el prompt"); return; }
  agyText("agyEnqState", "encolando...");
  try {
    var d = await apiPost("/api/v2/antigravity/enqueue", { prompt: p, confirm: true });
    if (!d || d.ok === false) { agyText("agyEnqState", "error"); agyText("agyEnqNote", (d && d.error) || "error"); return; }
    agyText("agyEnqState", "encolado");
    agyText("agyEnqNote", "cascade_id=" + (d.cascade_id || "?"));
    await agyConversations();
  } catch (e) { agyText("agyEnqState", "error"); agyText("agyEnqNote", String(e)); }
}
