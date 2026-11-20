# MegaDaemon v15 — Auditoría adversarial EXHAUSTIVA

> **Autor**: agente *General (auditoría adversarial v15)*.
> **Alcance**: `rust-core/src/**` (69 ficheros, 64.918 líneas), `web/cockpit_template.html`,
> `deploy/**`, `tools/**`, `docs/**`.
> **NO solapa** con `AUDIT_V13_1/2/4`, `AUDIT_V14_UI_HTTP` (UI-00…UI-15),
> `AUDIT_ENJAMBRE_V14`, `AUDIT_DELEGACION_V14`, `AUDIT_HARNESS_V13_4`.
> Todo lo que esas auditorías ya diagnosticaron **y sigue vivo** se cita como
> referencia cruzada (`↩ UI-xx`), pero **no se vuelve a contar como hallazgo
> nuevo**: aquí se cuenta lo que nadie miró.
>
> **Nota de redacción (2026-11-19 22:45)**: las **credenciales reales** que
> se citan como evidencia en `B-01` y `B-02` se han **redactado** de este
> fichero antes de subirlo al repo: la push protection de GitHub bloqueó el
> push por *secret scanning* (`Highnote SK Live Key`). Los **hallazgos no
> cambian** (siguen siendo P0), pero redactar **no invalida** las
> credenciales: hay que **rotarlas** (ver `B-01` y `B-02`).
>
> **Método (obligatorio, sin excepción)**: cada defecto lleva una de estas tres
> pruebas, y se dice cuál:
>   * **(T)** un test que lo reproduce → `rust-core/tests/audit_adversarial_v15.rs`
>     (fichero nuevo de esta auditoría, **no cableado** en `lib.rs`);
>   * **(M)** una medición/lectura del móvil vía MCP (no disponible en esta
>     sesión: el dispositivo está en manos de otro agente → se marca
>     `NO VERIFICADO EN RUNTIME` y no se afirma nada sobre el móvil);
>   * **(C)** una cita exacta `fichero:línea` que lo demuestra por inspección.
>
> **Regla de honestidad**: lo que no se ha verificado se marca
> `NO VERIFICADO (hipótesis)`. No hay "fantasmadas".

---

## 0. Resumen ejecutivo

### 0.1 Los tres hallazgos que cambian el plan

1. **La web v14 se arregló a medias y el arreglo abre una puerta peor.**
   Durante esta auditoría (21:xx) se detectó que el "arreglo de seguridad" de
   v14 (quitar el token del HTML público) dejaba la web **entera en 403**:
   el intercambio `POST /api/session/exchange` estaba registrado en la rama
   **GET** y `requires_auth()` lo exigía (círculo vicioso). **El owner del
   cockpit lo corrigió a las 22:48** (UI-16: la ruta está en POST, se exime
   de `requires_auth` y se introduce `local_browser`). **Pero el arreglo
   entrega el token root a cualquier petición que "parezca local"** — y
   "parecer local" se decide con el `Host` y con la ausencia de cabeceras de
   proxy, **que son falsificables**: el puente web (que escucha en
   `0.0.0.0`) las reescribe ⇒ **RCE root desde la wifi del usuario**
   (A-18, reproducido con test a las 22:59).
2. **Hay dos RCE root nuevas y encadenadas** que no estaban en ninguna
   auditoría previa: el heredoc de `fs_write` (T-A01) y el `stat` que AWK
   construye con el nombre del fichero en `find_duplicates` (T-A02). Ambas se
   ejecutan **como root real** (`su -mm -c`) y ambas son alcanzables desde el
   MCP (`fs_write`, `find_duplicates`), que es el canal que usa el agente
   externo.
3. **El puente web regala root a toda la wifi.** `deploy/web_bridge.py:201-204`
   inyecta `X-API-Key` **y** `X-Seere-Auth-Token` a cualquier petición que no
   traiga `Origin`, y escucha en `0.0.0.0` en los puertos 80/3000/3030/8080/8090.
   Un `curl` desde otro equipo de la wifi ejecuta root en el móvil. El
   parche UI-00 (CORS restringido) sólo cierra el vector de *navegador*.

### 0.2 Recuento por clase

| Clase | P0 | P1 | P2 | P3 | Total |
|---|---:|---:|---:|---:|---:|
| SEGURIDAD | 5 | 5 | 5 | 1 | **16** |
| CORRECTITUD | 0 | 1 | 9 | 0 | **10** |
| FIABILIDAD | 0 | 4 | 4 | 0 | **8** |
| RENDIMIENTO | 0 | 3 | 5 | 0 | **8** |
| MANTENIBILIDAD | 0 | 0 | 2 | 1 | **3** |
| USABILIDAD | 0 | 1 | 5 | 0 | **6** |
| ARQUITECTURA | 0 | 3 | 6 | 0 | **9** |
| COBERTURA | 0 | 2 | 3 | 1 | **6** |
| **Total** | **5** | **19** | **39** | **3** | **66** |

### 0.3 Recuento por severidad

| Severidad | Nº | Significado en este repo |
|---|---:|---|
| **P0** | 5 | RCE root, o deja el daemon/UI muerto en el S21 Ultra |
| **P1** | 19 | rompe el servicio o la seguridad en el uso real del móvil |
| **P2** | 39 | degradación, coste o mentira técnica que se nota a la semana |
| **P3** | 3 | higiene |

### 0.4 Los P0

| ID | Qué | Por qué es P0 |
|---|---|---|
| **A-01** | `fs_write` ejecuta el contenido como shell (heredoc) | RCE root vía MCP, sin `confirm` |
| **A-02** | `find_duplicates` ejecuta el nombre del fichero (AWK→shell) | RCE root en 2 pasos, sin `confirm` |
| **A-04** | `MEGADAEMON_REQUIRE_API_KEY=1` no fuerza nada si no hay key | fail-open: el túnel público queda abierto |
| **A-07** | el intercambio de token de sesión es inalcanzable | la web v14 no tiene token ⇒ 403 en todo |
| **A-15** | el puente web inyecta key+token a cualquier cliente de la LAN | root remoto desde la wifi del usuario |

### 0.5 Salud del repo en una frase

64.918 líneas de Rust, 69 módulos, **31 módulos sin un solo test**, un
`Cargo.toml` con `panic = "abort"` (un panic en cualquier hilo tumba el
daemon), y un plan (`MASTER_PLAN.md`) que dice "33.800 líneas, 50 módulos": el
repo **duplica su tamaño en tres días** y ni el plan ni la suite lo han
detectado.

---

## 0.6 Ficha del arnés de pruebas

`rust-core/tests/audit_adversarial_v15.rs` (nuevo, de esta auditoría,
**no cableado en `lib.rs`**):

| Test | Defecto | Tipo |
|---|---|---|
| `adv_a01_fs_write_inyecta_comandos_por_el_heredoc` | A-01 | (T) |
| `adv_a02_find_duplicates_inyecta_comandos_por_el_nombre_de_fichero` | A-02 | (T) |
| `adv_a03_la_politica_destructiva_se_esquiva` | A-03 | (T) |
| `adv_a04_require_api_key_se_ignora_sin_key_configurada` | A-04 | (T) |
| `adv_a05_el_rate_limit_no_aplica_al_loopback` | A-05 | (T) |
| `adv_a06_escribir_en_data_adb_no_pide_confirmacion` | A-06 | (T) |
| `adv_a18_la_exencion_local_entrega_el_token_root` | **A-18** | (T) |
| `adv_a19_el_tunel_no_recibe_el_token_root` | A-18 (control positivo) | (T) |
| `adv_a08_post_a_ruta_inexistente_devuelve_200_ok` | A-08 | (T) |
| `adv_a09_brain_status_true_por_cualquier_proceso_en_8080` | A-09 | (T) |
| `adv_a10_8080_no_esta_en_los_puertos_alias` | A-10 | (T) |
| `adv_c01_el_estimador_de_tokens_usa_chars_por_4` | C-01 | (T) |
| `adv_c02_hw_mmio_convierte_un_error_de_lectura_en_vacio` | C-02 | (T) |
| `adv_e01_la_config_de_api_key_se_relee_en_cada_request` | E-01 | (T) |
| `adv_e02_el_techo_de_memoria_del_cockpit_supera_la_ram_libre` | E-16/E-03 | (T) |
| `adv_h01_el_conversor_duplica_el_valor_en_la_colision_signal_class` | H-01 | (T) |

---

## A. SEGURIDAD

| ID | Sev | Ubicación | Defecto | Verificación | Impacto en el móvil | Corrección propuesta | Coste |
|---|---|---|---|---|---|---|---|
| **A-01** | **P0** | `rust-core/src/android.rs:762-786` (heredoc en `:779`) | `fs_write` mete el **contenudo crudo** del usuario dentro de `cat > tmp <<'__MD_EOF__' … __MD_EOF__`. Si el contenido contiene una línea `__MD_EOF__`, el heredoc termina antes de tiempo y **las líneas siguientes se ejecutan como shell como root** (`su -mm -c`). La rama `base64=true` sí usa `sh_quote`; la de texto plano no. | **(T)** `adv_a01_…`: el fichero marcador aparece ⇒ se ejecutó como comando. Además (C) `android.rs:779` muestra el `<<'__MD_EOF__'` con `{c}` sin escapar. | RCE root desde el MCP (`fs_write`) sin `confirm_destructive`. El agente externo lo usa para subir artefactos: basta un `REPORT.md` con `__MD_EOF__` dentro. | (a) Prohibir la línea delimitadora: `if content.contains("__MD_EOF__") → error`. (b) Pasar **siempre** por base64 (`printf %s <b64> \| base64 -d`). (c) Nunca interpolar el contenido en el argv del shell: escribir con `std::fs::write` como root y hacer el `chmod` aparte. | S (2 h) |
| **A-02** | **P0** | `rust-core/src/android.rs:806-812` (cmd del AWK en `:808`) | `find_duplicates` genera `cmd="stat -c %s \"" p "\" 2>/dev/null"` dentro del AWK y lo ejecuta con `getline`. `p` es el **nombre del fichero** que devuelve `sha256sum`: un nombre con `"` o `;` inyecta comandos de shell **como root**. | **(T)** `adv_a02_…`: el marcador aparece al escanear un directorio con un fichero llamado `x";touch …;echo "`. (C) `android.rs:808`. | RCE root en 2 pasos (subir un fichero con nombre hostil vía `fs_write`/`chat_upload` y después llamar a `find_duplicates`). | Sustituir el `stat` por `awk` puro (`p` ya trae el tamaño si se usa `sha256sum` con `%s`) o usar `find -printf '%s %p\0'` y no volver a invocar la shell desde AWK. Si hace falta `stat`, hacerlo con `system()` sobre una lista escapada. | M (½ día) |
| **A-03** | P1 | `rust-core/src/android.rs:35-46` (marcadores), `:58-61` (`is_destructive`) | La política destructiva es *substring matching* sobre 11 cadenas. Se evade con un espacio de más, con una opción larga, o con otra forma de borrar. `dd if=/dev/zero of=/dev/block/sda` sólo salta porque existe el marcador literal `dd if=`; con dos espacios (`dd  if=`) no salta. | **(T)** `adv_a03_…`: se comprueban 6 comandos que **no** se clasifican como destructivos. (C) `android.rs:35-46`. | El móvil puede quedarse sin particiones (`cat /dev/zero > /dev/block/sda`) o con un fork bomb, sin `confirm_destructive`. | Normalizar el comando (colapsar espacios, unificar guiones) y añadir patrones por *efecto* (`/dev/block`, `wipe`, `reboot`, `fork bomb`), no por sintaxis exacta. Complementar con un **allow-list** de binarios (`rm` sólo con flags concretos). | M (½ día) |
| **A-04** | **P0** | `rust-core/src/api_key.rs:311-318` (`decide`) | `if !cfg.enabled { return Decision::Allow; }` está **antes** de mirar `cfg.forced`. `MEGADAEMON_REQUIRE_API_KEY=1` sólo obliga cuando *hay* key: si `<STATE_DIR>/megadaemon_api.key` falta o el env se borra, el middleware se desactiva entero y **el túnel público sirve el daemon sin credencial**. | **(T)** `adv_a04_…`: `forced=true, enabled=false` ⇒ `Allow` para una IP externa sin cabeceras. (C) `api_key.rs:311-318`. | Si se desinstala la app (se borra `/data/user/0/com.agy/…/state/`) o se pierde `megadaemon.env`, el cockpit queda **abierto a internet** por el túnel, con `root_exec` detrás. | Invertir el orden: si `forced` ⇒ exigir key (y **autogenerarla** si falta, con aviso ruidoso). Añadir `Decision::Unauthorized` + "key no configurada" al log. | S (1 h) |
| **A-05** | P1 | `rust-core/src/api_key.rs:53-61` (`rate_applies_to`), `:316-318` | El rate limit (60/min) **no se aplica al loopback por defecto**. En el móvil TODO el tráfico entra por 127.0.0.1 (Chrome, el túnel Cloudflare, los agentes) ⇒ **no queda ningún cubo limitado**: el límite v13.3.0 que provocaba los 429 en cascada se ha desactivado sin sustituirlo. | **(T)** `adv_a05_…`: `rate_applies_to(127.0.0.1) == false` y 5000 peticiones seguidas de loopback ⇒ ninguna `RateLimited`. (C) `api_key.rs:274`. | Un agente con un bucle agresivo (o la UI con 20 peticiones por recarga) puede tumbar el móvil; y a la inversa, nadie puede afirmar que hay protección. | Limitar **por ruta** y por **agente** (cabecera `X-Agent-Id` → cubo propio), no por IP de origen; y poner el límite por defecto en el propio `X-Seere-Auth-Token` (que sí se puede exigir al loopback). | M (1 día) |
| **A-06** | P1 | `rust-core/src/android.rs:712-718` (`write_requires_confirm`) | Sólo `/dev/block` y `/system` exigen `confirm`. **`/data/adb` no**: escribir un `post-fs-data.sh` en `/data/adb/modules/` es persistencia de código que se auto-arranca en cada arranque, sin confirmación. | **(T)** `adv_a06_…`: `fs_write("/data/adb/modules/…/post-fs-data.sh")` **no** devuelve `needs_confirm` y se ejecuta. (C) `android.rs:712-718`. | Backdoor permanente en el móvil vía MCP, sin `confirm_destructive`. | Ampliar la lista: `/data/adb`, `/data/local/tmp/megadaemon`, el directorio de la app, `/sdcard/Android`, y exigir `confirm` en cualquier escritura fuera de `uploads/`, `extracted/` y `state/`. | S (2 h) |
| **A-07** | ✅ CORREGIDO | `rust-core/src/cockpit_server.rs:217-219` (+ `:183-228`) | **Detectado por esta auditoría a las 21:xx sobre el código de las 21:09** y **corregido por el owner del cockpit a las 22:48** (UI-16) mientras esta auditoría corría. El defecto era: `/api/session/exchange` estaba en la rama **GET**, `requires_auth()` lo exigía (círculo vicioso) y el HTML público ya no llevaba token ⇒ con el binario v14 **toda la UI devolvía 403**. El arreglo: la ruta existe en POST, `requires_auth()` la exime y se añade el concepto de `local_browser`. | (T) el test `adv_a18` **falla por otro motivo** (ver A-18): el deadlock sí está resuelto, pero el arreglo abrió una puerta peor. (C) `cockpit_server.rs:217-219`. | **Si se hubiera desplegado el ARM64 v14 sin este arreglo, la web del móvil quedaba muerta.** Ya no aplica. | — (hecho) | — |
| **A-18** | **P0** | `rust-core/src/cockpit_server.rs:846` (`local_browser`), `:855` (gate de API key), `:921` (gate de token), `:959-966` (HTML con token), `:1380-1400` (intercambio), `:129-165` (`host_is_loopback` / `has_proxy_headers`) | **Nuevo P0 introducido por el arreglo de A-07 (22:48).** La exención `local_browser` se decide con (a) el **IP de peer** loopback, (b) la **cabecera `Host`** loopback y (c) la **ausencia** de cabeceras de proxy — **las tres son falsificables**. Y el efecto es: (1) `POST /api/session/exchange` con **cualquier** `api_key` no vacío devuelve el **token de sesión root** (`local_browser && !provided.is_empty()` ⇒ la key real **no se comprueba**); (2) `GET /` incrusta el **token root** en el HTML; (3) **todos** los `GET /api/*` responden sin token (el gate de la API key también se salta: `Decision::Unauthorized if … \|\| local_browser`). El **puente web** (`0.0.0.0:80/3000/3030/8080/8090`) entra desde loopback (**`SRC_IP = 127.0.0.2`**, `web_bridge.py:129`, que `api_key::is_loopback` considera loopback porque mira `octets()[0] == 127`) y **reescribe el `Host` a `127.0.0.1:9090`** (`web_bridge.py:166-199`: `host` está en `HOP_BY_HOP` ⇒ `http.client` pone el suyo) ⇒ cualquier equipo de la wifi encaja **exactamente** en el perfil "local". | **(T)** `adv_a18_la_exencion_local_entrega_el_token_root`: POST con `api_key:"esta-no-es-la-key"` ⇒ **200 `ok:true` + el token root real**; `GET /` ⇒ el HTML **contiene el token root**; `GET /api/status` sin token ⇒ **200**. (T) `adv_a19_el_tunel_no_recibe_el_token_root` (control positivo): con `Host` de dominio + `CF-Connecting-IP` el túnel **sí** queda fuera ⇒ el discriminante es **una cabecera**, y el puente la borra. | **RCE root desde la wifi del usuario** (y desde cualquier proceso del móvil): `curl http://192.168.1.124/api/session/exchange -d '{"api_key":"x"}'` → token root → `POST /api/chat {"message":"$ id"}` → root. El túnel público está a salvo **por suerte** (cabeceras de proxy), no por diseño. | (a) `local_browser` **no** puede depender del `Host` ni de "no hay cabeceras de proxy" (se falsifican); (b) el intercambio **debe** comprobar la API key real **siempre** (`provided == cfg.key`), sin la rama `local_browser`; (c) el token que se entrega debe ser **de sesión** (con caducidad y sin permisos root), no el token root; (d) el puente debe dejar de reescribir el `Host` a loopback o dejar de existir (A-15). | M (½ día) |
| **A-08** | P2 | `rust-core/src/cockpit_server.rs:1795-1805` | El `catch-all` del router POST responde **HTTP 200** con `{"status":"executed","endpoint":…}` a **cualquier** ruta inexistente. | **(T)** `adv_a08_…`: `POST /api/no-existe-esta-ruta` ⇒ 200 + `"status":"executed"`. (C) `cockpit_server.rs:1795`. | Un cliente (o un agente) que escriba a una ruta mal escrita believes que la operación se ejecutó. | Responder **404** (`{"error":"not_found"}`) y reservar el 200 para rutas implementadas. | S (30 min) |
| **A-09** | P2 | `rust-core/src/cockpit_server.rs:453-459` (`check_llama_online`), `:1016` (uso), `:1948` (`ALIAS_PORTS`) | `brain_active` se calcula **conectando a `127.0.0.1:8080`**. En el móvil el 8080 lo sirve el **puente web** (o cualquier proceso) ⇒ el indicador dice "cerebro activo" aunque no haya ningún LLM. | **(T)** `adv_a09_…`: con un `TcpListener` vacío en 8080, `/api/brain_status` devuelve `"brain_active":true`. (C) `cockpit_server.rs:453-459`. | La UI miente sobre el estado del LLM ⇒ el usuario diagnostica mal por qué el chat no responde. | Sondear el **PID** de `llama-server` (como ya hace `npu_bridge.rs:260`) o leer el socket real del daemon; nunca el 8080 genérico. | S (1 h) |
| **A-10** | P2 | `rust-core/src/cockpit_server.rs:1948` vs `deploy/99-megadaemon.sh:116-125` y `deploy/web_bridge.py:104` | `ALIAS_PORTS = [3000, 3030, 8090, 9090]` — **sin 8080**. Pero el supervisor (`alias_ports_listening` mira 0050/0BB8/0BC6/1F90/1F9A) y el puente (puertos `3000,3030,8090,8080,80`) y `write_url` asumen que 8080 sirve la UI. | **(T)** `adv_a10_…`: `!ALIAS_PORTS.contains(&8080)`. (C) `cockpit_server.rs:1948` vs `99-megadaemon.sh:116`. | Al desplegar v14, `http://localhost:8080` vuelve a estar muerto (justo la URL que el supervisor anuncia como OK) ⇒ otro "la web no me funciona". | Una **única** fuente de verdad para la lista de puertos (`ALIAS_PORTS` exportada al shell vía `/health`), y el supervisor la lee de ahí en vez de hardcodearla. | S (2 h) |
| **A-11** | P1 | `rust-core/src/seere_v11.rs:1290-1296` (`exec_root`), `:2006-2018` (atajo `$ `), `rust-core/src/cockpit_server.rs:1452` (`/api/chat`), `:1483` (`/api/llm_agent`) | El arsenal de SEERE (`Arsenal::exec_root`) **no consulta `is_destructive`**: el atajo `"$ <orden>"` de `/api/chat` ejecuta **como root sin `confirm`**, mientras que la *misma* orden por la tool MCP `root_exec` **sí** exige `confirm_destructive`. Dos políticas distintas para el mismo poder. | (C) `seere_v11.rs:1290-1296` (no hay ninguna llamada a `android::is_destructive` en el camino) frente a `android.rs:821-836` (`root_exec_tool` sí la llama). | Escribir `rm -rf /` en el chat del cockpit borra el móvil sin preguntar. El gate de la UI es el único que impide que sea "por accidente". | Que `Arsenal::exec_root` delegue en `android::root_exec_tool` (o aplique la misma `is_destructive` + `confirm`) ⇒ **una sola política de destructividad en todo el daemon**. | S (3 h) |
| **A-12** | P2 | `rust-core/src/cockpit_server.rs:1369-1377` (`/api/sync/ingest`) + `rust-core/src/sync_web.rs:844-870` | `/api/sync/ingest` acepta **cualquier ruta absoluta** y la lee **como root** (`head -c 2000000 <ruta> \| base64`); si el formato no es de conversación, devuelve los **primeros 4.000 caracteres del fichero** en el campo `head`. No pasa por el sandbox de adjuntos que AUDIT-V14 propone para los endpoints RE. | (C) `sync_web.rs:844-855` y `:870-876` (`head` en la respuesta de error). | Lectura arbitraria de ficheros del móvil (incluidos `*.db` de apps) con el token; el arreglo de UI-04 **no** cubre esta ruta. | Aplicar el mismo `providers::attachment_allowed()` que en los endpoints RE, o restringirse a `incoming/`, `uploads/` y `state/`. | S (2 h) |
| **A-13** | P3 | `rust-core/src/cockpit_server.rs:111-120` (`auth_ok`) | El token de sesión se compara con `==` (no constante en el tiempo), mientras que la API key **sí** se compara en tiempo constante (`api_key.rs:247-257`). Inconsistencia dentro del mismo fichero de auth. | (C) `cockpit_server.rs:119` vs `api_key.rs:247`. | Fuga de temporización muy débil (red local). Bajo riesgo real, pero contradice el estándar que el propio repo se fija. | Reusar `api_key::constant_time_eq` en `auth_ok`. | S (15 min) |
| **A-14** | P2 | `deploy/99-megadaemon.sh:22` (`LOG=/sdcard/megadaemon.log`), `rust-core/src/android.rs:191-215` (`audit` → `audit.ndjson`) | El log del supervisor y el registro de auditoría se escriben en **`/sdcard`** (almacenamiento compartido, legible por cualquier app con `READ_EXTERNAL_STORAGE` en Android < 13 y por cualquiera con acceso al PC). El `audit.ndjson` registra IP, ruta, user-agent y **el comando root ejecutado** (`android.rs:213`: `{"cmd": command}`). | (C) `99-megadaemon.sh:22` y `android.rs:203-213`. | Otro proceso del móvil (o una app con permiso) lee el historial de comandos root ejecutados. Además, escribir en FUSE sdcard en cada evento es lento. | Mover el log a `<STATE_DIR>/logs/` (privado, 0700) y dejar en `/sdcard` **sólo** un enlace al log actual; redactar el comando completo en la auditoría (hash + prefijo). | S (2 h) |
| **A-15** | **P0** | `deploy/web_bridge.py:201-204` (inyección de cabeceras), `:265-270` (`MD_WEB_BIND` por defecto `0.0.0.0`), `:104` (puertos `3000,3030,8090,8080,80`) | El puente inyecta `X-API-Key` **a cualquier** petición que no la traiga (**sin comprobar `Origin`**, al contrario que el token en la línea siguiente) y el `X-Seere-Auth-Token` cuando no hay `Origin` (un `curl` no lleva `Origin`). Escucha en **todas las interfaces** en el puerto **80**. | (C) `web_bridge.py:201-204` y `:265-270`. El parche UI-00 (`AUDIT_V14_UI_HTTP.md`) sólo restringió el **CORS de lectura**, no la inyección de credenciales. | **Cualquier equipo de la wifi del usuario** (y cualquier persona con la IP `192.168.1.124`, publicada en `docs/CONNECT_EXTERNAL_AGENT_ANDROID_RE.md:99`) hace `curl http://192.168.1.124/mcp` y ejecuta root en el móvil. El repo es público: la topología está escrita. | (a) El puente **no debe inyectar el token de sesión**; sólo la API key, y sólo si la petición viene de loopback. (b) `MD_WEB_BIND` por defecto `127.0.0.1` (el 80 en `0.0.0.0` es lo que abre la LAN). (c) Cuando el daemon v14 sirva los alias, **borrar el puente** (ya está en el plan de despliegue). | M (½ día) |
| **A-16** | P1 | `rust-core/src/cockpit_server.rs:695-704` (tope de 8 MB), `:628-631` + `:1919-1928` (64 hilos) | El tope de concurrencia (64) no acota la **memencia**: cada conexión reserva hasta **8 MB** de cuerpo (`buf.resize(header_end + content_length)`) ⇒ **64 × 8 MB = 512 MB** sólo de buffers, más 64 pilas de 2 MB (128 MB). El móvil tiene ~300 MB libres (10,2/10,5 GB). | **(T)** `adv_e02_…` (la aritmética del techo, con las constantes reales del código); (C) `cockpit_server.rs:695-718` y `:1919-1932`. | 64 subidas simultáneas ⇒ OOM-killer ⇒ el daemon y el puente mueren; en el S21 Ultra eso es "la web se cae sola". | (a) Tope de cuerpo **acumulado** (p. ej. 32 MB en total, no por conexión). (b) `Content-Length` obligatorio y comprobado **antes** de reservar. (c) `stacker::grow` / lectura por trozos sin `resize` a `content_length`. (d) Bajar `MAX_CLIENT_THREADS` a 32 en el móvil. | M (1 día) |

---

## B. CREDENCIALES EN CLARO

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **B-01** | **P0** | `docs/EXTERNAL_AGENT_PROMPT_FINAL.md` (líneas 37, 63 y 64 en la versión de las 21:21; el fichero fue **redactado a las 21:28** por otro agente) | Se publicó en el árbol de trabajo la **API key real** `mdaemon_sk_live_<REDACTADO: la credencial real está en el árbol de trabajo y hay que rotarla>` y el **token de sesión root** `c17398e1…<REDACTADO>`. | (C) el fichero con `git status` como **untracked** (`?? docs/EXTERNAL_AGENT_PROMPT_FINAL.md`) ⇒ **nunca se commiteó**; el texto sí estuvo ~7 min en el árbol compartido (leído por cualquier agente). | La API key es la **única** barrera del túnel público (con `MEGADAEMON_REQUIRE_API_KEY=1`). Con ella, `POST /mcp {root_exec}` = root. | **Rotar las dos credenciales** (la redacción del fichero NO las invalida) y añadir un escáner de secretos al CI (`gitleaks`/`detect-secrets`) que bloquee el push. | S (1 h) + despliegue |
| **B-02** | **P0** | `tools/upload_bin.py:10-11` (fichero **versionado** en git) | URL y **token de un túnel Cloudflare** en claro: `ENDPOINT="https://lou-larry-telecharger-changed.trycloudflare.com/mcp"` (URL del túnel), `TOKEN="<REDACTADO>"`. Ese token es la credencial del **proxy de subida**: da acceso root al móvil. Además usa `Authorization: Bearer` (el daemon acepta `X-API-Key` o `Bearer` → funciona). | (C) `tools/upload_bin.py:10-11`; `git ls-files tools/upload_bin.py` lo confirma versionado. | Cualquiera que clone el repo (público) puede subir y ejecutar cosas en el móvil mientras el túnel siga vivo. | Borrar el fichero (o moverlo a `tools/` **sin** credenciales y leerlas del entorno). Rotar el token del túnel. Añadir `tools/upload_bin.py` al `.gitignore` si se conserva. | S (1 h) |
| **B-03** | P1 | `docs/CONNECT_PRIMARY_SAMSUNG.md:17`, `docs/COORDINATION.md:71`, `docs/EXTERNAL_AGENT_PROMPT_FINAL.md:35`, `docs/CONNECT_EXTERNAL_AGENT_ANDROID_RE.md:99` | El repo publica la **URL del túnel público**, la **IP de la LAN** (`192.168.1.124`) y la lista de puertos alias. Con A-15, esa información es un **manual de RCE**. | (C) las líneas citadas. | Reduce el effort del atacante a "curl a la IP de la wifi". | Publicar sólo la URL del túnel **rotada** y enmascarar la IP (`<ip-del-movil>`); mover el detalle operativo a un fichero fuera del repo. | S (1 h) |
| **B-04** | P1 | `rust-core/src/api_key.rs:113-121` (búsqueda de la key), `rust-core/src/cockpit_server.rs:37-45` (`state_dir`) vs `deploy/99-megadaemon.sh:19-21` | El **cofre** de la API key se busca en `<STATE_DIR>/megadaemon_api.key` = `/data/user/0/com.agy/files/megadaemon/state/` (privado de la app), pero el supervisor carga la key del **env** en `/data/local/tmp/megadaemon/megadaemon.env`. Si el env se pierde (o se reinstala la app), `ApiKeyConfig::load` cae en `enabled=false` ⇒ **fail-open** (ver A-04). | (C) `api_key.rs:101-121` + `99-megadaemon.sh:174-181`; además `web_bridge.py:48-53` busca la key en **dos** directorios distintos. | El fallo de auth es silencioso y sólo se ve cuando el túnel ya está abierto. | Un único origen de la key (fichero 0600 en `<STATE_DIR>`), y el supervisor lo **genera** si falta (con `api_key::generate_and_store`) en vez de arrancar sin key. | M (3 h) |
| **B-05** | P2 | `rust-core/src/api_key.rs:129` y `:182` | El registro de claves (`megadaemon_api.key` / `.keys`) se escribe con `fs::write` (truncar+escribir), **sin atomicidad**: un corte de luz en mitad deja el registro vacío ⇒ **todos** los agentes (y el usuario) pierden el acceso a la vez. | (C) `api_key.rs:129`, `:182` (comparar con `coordination.rs:127-134`, que **sí** hace tmp+rename). | Un corte de luz deja la instancia inaccesible por remoto hasta regenerar la key a mano. | Escribir a `.tmp` + `fsync` + `rename` (el patrón ya existe en `coordination.rs`). | S (1 h) |

---

## C. CORRECCIÓN

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **C-01** | P1 | `rust-core/src/delegate.rs:864-872` (`estimate_task_tokens`) | La estimación de tokens es `chars/4`. El **propio repo** lo dice: `tools/token_budget.py:6-8` — *"los 'chars/4' están equivocados entre un 15% y un 30% entre tokenizers, y cada decisión de optimización en SEERE es una decisión de presupuesto de tokens"*. El enrutado de la delegación **decide por "menor coste estimado"** (`delegate.rs:978`) y ese coste se compara con los precios reales por millón (`delegate.rs:701-706`). | **(T)** `adv_c01_…`: con texto multibyte la estimación queda por debajo de `chars/2`. (C) `delegate.rs:865` + `tools/token_budget.py:6-8`. | La decisión 5 del `MASTER_PLAN.md` §8 ("presupuesto de tokens: coste real, no estimado a ojo") se sigue calculando **a ojo**. El router puede enviar a un provider caro believing que es barato. | Tokenizador real por modelo (el que usa `token_budget.py`), o al menos una tabla de "tokens por carácter" por familia de modelo, con la fuente y la fecha (igual que `PRICING_SOURCES`). | M (1 día) |
| **C-02** | P2 | `rust-core/src/cockpit_server.rs:1244-1251` (`/api/hw_mmio`) | `fs::read_to_string(c_path).map(truncate).unwrap_or_default()` convierte "no se pudo leer" en "fichero vacío" ⇒ el análisis responde "no hay MMIO" en lugar de "no existe el fichero". | **(T)** `adv_c02_…`: el endpoint contesta `ok` aunque el fichero no exista. (C) `cockpit_server.rs:1247-1249`. | Diagnóstico engañoso: el usuario cree que el firmware no tiene MMIO. | Propagar el error con `match` y responder `{"status":"error","message":…}`. | S (30 min) |
| **C-03** | P2 | `rust-core/src/cockpit_server.rs:937` (`/health`), `:960` (`/api/ping`), `:1295` (`/api/remote_auth`) | El **mismo daemon** reporta tres versiones: `13.4.0` (`env!("CARGO_PKG_VERSION")`), `"13.1.0-zenith"` (`/api/ping`) y `"13.0.0"` (`/api/remote_auth`, que además devuelve `"authenticated": true` **siempre**, sin comprobar nada). | (C) las tres líneas. | "¿qué versión está desplegada?" — la pregunta que bloqueó el despliegue del ARM64 — no tiene una respuesta fiable. | Que las tres rutas usen `env!("CARGO_PKG_VERSION")`; y `/api/remote_auth` que **verifique** el token antes de decir `authenticated: true`. | S (30 min) |
| **C-04** | P2 | `rust-core/src/cockpit_server.rs:490-498` (`status_text`) | El mapa de estados HTTP no conoce 429 ni 503 ⇒ un **429** se etiqueta `429 Internal Server Error` y un **503**, `503 Internal Server Error`. | (C) `cockpit_server.rs:491-498`. | Los 429 que el enjambre interpretó como "la instancia está caída" se ven todavía peor ("Internal Server Error"). El cliente MCP ve un error de servidor donde hay una límite de ritmo. | Tabla completa de status texts (429 → "Too Many Requests", 503 → "Service Unavailable", 405, 401…). | S (20 min) |
| **C-05** | P2 | `rust-core/src/cockpit_server.rs:1919-1928` (respuesta 503) | El 503 por exceso de clientes se escribe **a mano**, sin `Content-Length` correcto… sí lo tiene, pero **sin `Access-Control-Allow-Origin` ni `Vary`**: un navegador que recibe 503 lo ve como error de red (CORS), no como "servidor saturado". | (C) `cockpit_server.rs:1922-1927` (respuesta cruda, sin pasar por `send_http_response`). | El diagnóstico "estoy saturado" se pierde justo en el momento en que hace falta. | Usar `send_http_response` con un 503 real. | S (20 min) |
| **C-06** | P2 | `rust-core/src/cockpit_server.rs:1588` (`mutex.lock().unwrap()`) | El `Mutex` global del cockpit se toma con `unwrap()`. Un panic previo **envenena** el mutex ⇒ el siguiente `POST /api/runners/control` hace panic ⇒ con `panic = "abort"` (`Cargo.toml:44`) **aborta el proceso entero**. El `send_json` ocurre **con el lock tomado** (I/O de red bajo lock). | (C) `cockpit_server.rs:1587-1588` + `Cargo.toml` `[profile.release] panic = "abort"`. | Un panic aislado (p. ej. `serde_json::to_value` fallando) tumba el daemon y el móvil se queda sin cockpit hasta que el supervisor lo reinicie (y con el límite de 3 fallos rápidos, A/D-01, puede no volver). | `lock().unwrap_or_else(\|p\| p.into_inner())` (el patrón ya usado en `api_key.rs:277-280`), sacar el `send_json` fuera del lock, y `panic = "unwind"` + `catch_unwind` en el hilo de cada conexión. | S (1 h) |
| **C-07** | P2 | `rust-core/src/cockpit_server.rs:1129,1141,1165,1176,1189,1206,1219,1226,1235,1242,1251,1258,1565,1574,1582,1675,1684,1691,1707` | 19 × `serde_json::to_value(rep).unwrap()` en rutas servidas por HTTP. `to_value` falla si la estructura contiene un mapa con clave no-string (o un `f64` NaN). Con `panic = "abort"`, **una** de esas 19 es suficiente para tumbar el daemon. | (C) la lista de líneas; `Cargo.toml` `panic = "abort"`. | El fallo es "improbable" hasta que ocurre en el móvil, y entonces es fatal y sin diagnóstico. | `unwrap_or_else(|_| json!({"status":"error"}))` en las 19 + `catch_unwind` en `handle_client`. | S (1 h) |
| **C-08** | P1 | `rust-core/src/android.rs:131-144` (hilos lectores) y `:164-165` (`join()`) | `root_exec` mata el hijo al expirar el timeout, pero **espera sin límite** a los dos hilos lectores (`out_handle.join()`): si el comando deja un hijo que hereda stdout (típico `nohup … &`), el pipe no cierra ⇒ **la llamada MCP se queda colgada para siempre**, con su hilo y su pipe. | (C) `android.rs:131-144` y `:164-165`. | Un `root_exec` colgado **consume un hilo del cockpit para siempre** (y con `MAX_CLIENT_THREADS=64`, 64 colgados ⇒ el cockpit deja de aceptar). | `join` con timeout (canal + `recv_timeout`) y cerrar los pipes al expirar. | M (3 h) |
| **C-09** | P2 | `rust-core/src/providers.rs:76-95` (`load_vault`, copia `.corrupt` en `:85`) | Si `providers.json` está **corrupto**, `load_vault()` escribe una copia `.corrupt-<ts>` en cada llamada y **nunca repara el fichero**. Como `load_vault()` se llama en cada request que toque proveedores (`/api/providers`, `/api/brain_status`, `/api/chat`…), el cockpit crea **un fichero por petición** hasta llenar la partición. | (C) `providers.rs:85-91` (la copia se escribe dentro de la rama de error, que se repite en cada llamada). | Un `providers.json` a medias (escritura interrumpida) ⇒ el móvil genera ~7.000 ficheros/día (1 cada 12 s de refresco) y acaba sin espacio ⇒ el daemon deja de arrancar. | Escribir la copia **una sola vez** (mover el corrupto a `.corrupt` y dejar que `load_vault()` use el default en silencio), y hacer `save_vault` **atómico** (tmp+rename, ver UI-06). | S (1 h) |
| **C-10** | P2 | `rust-core/src/cockpit_server.rs:692-704` (comentario "100 MB" vs `MAX_BODY_BYTES = 8 MB`) y `:1486-1498` (`chat_upload`); `web/cockpit_template.html:976-985` (base64 en el navegador) | El comentario dice *"Cap single HTTP body at 100 MB"* pero el tope real son **8 MB**, y como la UI sube en **base64**, el fichero real máximo es **~6 MB**. Un APK de 60 MB ⇒ **413** y la UI no explica por qué. | (C) `cockpit_server.rs:692-695` + `:1495-1498` + `cockpit_template.html:976-981`. | "Subir un APK" (el caso de uso del agente externo) falla de forma opaca en el móvil. | Subida por trozos (`fs_write` ya lo hace bien: `tools/upload_bin.py` trocea a 400 KB) o subir el tope a 64 MB **por ruta autenticada** y con tope acumulado (ver A-16). | M (½ día) |

---

## D. FIABILIDAD

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **D-01** | P1 | `deploy/99-megadaemon.sh:195-212` + `:247-256` | Tras **3 fallos de arranque seguidos** el supervisor hace `exit 0` y **no vuelve a intentarlo** hasta el próximo reinicio del móvil. Un `EADDRINUSE` transitorio (el puente occupying 9090, el daemon antiguo sin morir todavía), un binario truncado o un `/system` lleno ⇒ **el móvil se queda sin daemon hasta que se reinicia**. | (C) `99-megadaemon.sh:198-212` (`fast_fails`, `MAX_FAST_FAILS=3`, `return 2` → `exit 0`). | El fallo clásico "el móvil va JUSTO de RAM" (OOM al arrancar) deja el dispositivo sin herramienta y **sin aviso** salvo un fichero de estado en `/sdcard`. | No `exit 0`: seguir con backoff largo (5-30 min) y **marcar `degraded`**; reservar el `exit` para un fallo de *código* inequívoco (p. ej. `Exec format error`), y reiniciar el servicio de Magisk con `stop`/`start` en vez de morirse. | S (2 h) |
| **D-02** | P1 | `deploy/install-phone.sh:110` (copia `.bak`) y `:175-177` (`verify \|\| true`) | Se guarda el binario anterior como `.bak`… y **nunca se restaura**. Además `verify \|\| true` hace que **el despliegue termine con código 0 aunque el cockpit no responda**. | (C) `install-phone.sh:110` (crea el `.bak`), `grep -n "bak"` → **no hay ninguna restauración**; `install-phone.sh` final: `verify \|\| true`. | Un despliegue con un binario roto deja el móvil **sin daemon y con el script diciendo "hecho"**. La cadena automática (que el enjambre usa) no se entera. | Rollback real: si `verify` falla ⇒ `cp $BIN.bak $BIN` + reiniciar + reintentar; y `verify` **sin** `\|\| true` (o `exit 1`). | M (3 h) |
| **D-03** | P1 | `deploy/99-megadaemon.sh:249` y `:274` (`pgrep -f "megadaemon-core cockpit"`) | El supervisor comprueba **que el proceso exista**, no que **sirva**. El estado "vivo pero mudo" (justo el que motivó `cockpit_doctor.rs`) sigue sin detección automática. | (C) `99-megadaemon.sh:249`, `:274`; `cockpit_doctor.rs:348` (`diagnose`) existe pero **no está cableado** (F-03). | El móvil puede tener el proceso vivo y **cero puertos abiertos** — el "ERR_CONNECTION_REFUSED" que el usuario reportó tres veces. | El supervisor debe consultar `/health` (no `pgrep`) y, si falla N veces, **reiniciar** el daemon. Idealmente invocar `cockpit_doctor::diagnose` (que ya sabe mirar puertos, plantilla, token y log). | M (½ día) |
| **D-04** | P1 | `deploy/99-megadaemon.sh:264-277` | El bucle interno duerme 5 s y en cada vuelta lanza `ensure_web_bridge()` → `pgrep -f` + `grep` sobre `/proc/net/tcp{,6}` ⇒ **3-4 procesos fork cada 5 segundos, para siempre** (día y noche). | (C) `99-megadaemon.sh:269-277` → `:135-162` (`pgrep`, `grep`). | En un móvil, un `pgrep` cada 5 s mantiene el wakelock del process_refresher y **gasta batería**; el `grep` de `/proc/net/tcp` es I/O de procfs. | Un solo `sh` que comprueba `/proc/net/tcp` con `read` (sin fork) y un `pgrep` cada 60 s; o un watchdog en Rust (que ya está en el daemon). | S (2 h) |
| **D-05** | P2 | `deploy/99-megadaemon.sh:22-23` (`LOG=/sdcard/...`), `:61-79` (`rotate`), `:247-248` | El log del daemon va a **`/sdcard`** (FUSE: cada `println!` del daemon paga el FUSE) y la rotación sólo se evalúa **una vez por vuelta del bucle exterior (≈60 s)**: entre rotaciones el log puede pasar de 10 MB a 60 MB. | (C) `99-megadaemon.sh:22`, `:247-248`, `:66-79`. | Escrituras lentas al log = I/O extra en un dispositivo con 78 % de iow; y rotación tardía. | Log en `<STATE_DIR>/logs/` (ext4 interno) y rotación por **tamaño en el propio daemon** (no desde el shell). | S (2 h) |
| **D-06** | P2 | `rust-core/src/coordination.rs:237` (`Connection::open`) | El tablero abre la SQLite **sin `busy_timeout`** (el de SQLite es 0) ⇒ dos procesos que escriben a la vez reciben `database is locked` **al instante**, sin esperar. Con WAL activo (`coordination.rs:266`) el problema persiste en las escrituras. | (C) `coordination.rs:237-245` (no hay `busy_timeout`) frente a `coordination.rs:266-267` (sí hay WAL). | El tablero (el sitio donde "vive el plan") falla de forma intermitente y **pierde claims** justo cuando hay más de un agente. | `PRAGMA busy_timeout=5000` + reintento con backoff en las escrituras. | S (1 h) |
| **D-07** | P2 | `tools/findings_from_manifest.py:565-570` | La salida (`findings.ndjson`) se escribe con `open(out_path, "w")`: **trunca y luego escribe**. Si el proceso muere a mitad, el fichero queda truncado y el daemon (o el agente) leerá NDJSON a medias. | (C) `findings_from_manifest.py:567`. | Un `Ctrl-C` o un OOM dejan el fichero de hallazgos corrupto y **no hay forma de distinguir "vacío" de "truncado"**. | Escribir a `.tmp` + `os.replace` (atómico en el mismo FS). | S (20 min) |
| **D-08** | P2 | `rust-core/src/cockpit_server.rs:1822-1941` | `run_server` nunca devuelve: su bucle final es `loop { sleep(3600) }` y el `Ok(())` de la línea 1940 es **código muerto**. No hay ruta de **apagado ordenado** (ni `SIGTERM`): el daemon siempre muere por `killall`. | (C) `cockpit_server.rs:1936-1940`. | Cada reinicio es un `kill -9` en pleno I/O: se pueden perder escrituras (NDJSON, `providers.json`, `board.sqlite`). | Manejar `SIGTERM`/`SIGINT` → cerrar el listener, vaciar buffers y salir 0; y hacer el supervisor `kill -TERM` antes de `kill -9`. | M (½ día) |

---

## E. RENDIMIENTO Y RECURSOS (el S21 Ultra va JUSTO de RAM)

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **E-01** | P1 | `rust-core/src/cockpit_server.rs:745` (`api_key::ApiKeyConfig::load(&state_dir())` **dentro** de `handle_client`) | **En cada petición** se releen del disco `megadaemon_api.key` + `megadaemon_api.keys` (2 ficheros) y se relee el env. No hay caché (ni por fichero ni con TTL). | **(T)** `adv_e01_…`: escribir la key **después** de arrancar la hace efectiva en la petición siguiente ⇒ no hay caché. (C) `cockpit_server.rs:745`. | Con 20 peticiones por recarga de UI y un refresco cada 12 s: **~60 lecturas de fichero por minuto** en flash, con el móvil ya al 78 % de iow. | Cachear la config con TTL (5-10 s) + un `mtime` para invalidar; o leerla una vez y recargarla con `SIGHUP`. | S (2 h) |
| **E-02** | P1 | `rust-core/src/providers.rs:76` (`load_vault()`), llamadas en `cockpit_server.rs:965`, `:1017`, `:1452` | La **bóveda de proveedores** (que contiene las API keys) se relee y se re-parsea en cada request que la toca, y `POST /api/providers` la relee **y** la reescribe sin lock (ya está en ↩ UI-06). | (C) `providers.rs:76-95` + las tres llamadas. | Coste de I/O y de CPU por request en un móvil; y una **carrera de escritura** que pierde keys (↩ UI-06, sigue viva). | Caché en memoria con `RwLock` + escritura atómica (el arreglo de UI-06). | S (2 h) |
| **E-03** | P1 | `cockpit_server.rs:695-718` + `:628-631` + `:1919-1932` | El techo de memoria real del cockpit es **512 MB de buffers + 128 MB de pilas** con 64 clientes. | **(T)** `adv_e02_…` (aritmética con las constantes reales). (C) las líneas. | OOM-killer en el S21 Ultra (10,2/10,5 GB usados) ⇒ el daemon **y** el puente mueren a la vez. | Tope **acumulado** de cuerpos + `Content-Length` obligatorio + menos hilos. | M (1 día) |
| **E-04** | P2 | `cockpit_server.rs:1910-1935` | Un hilo por conexión **y** un hilo por listener (×2 por dual-stack: `0.0.0.0` y `[::1]` ⇒ **8 hilos de escucha** para 4 puertos). Cada hilo de conexión reserva su pila de 2 MB. | (C) `cockpit_server.rs:1863-1872` (bind IPv6) + `:1910-1934`. | 8 hilos de escucha + 64 de clientes = 72 hilos ⇒ ~144 MB de pilas virtuales y schedulación extra en un móvil. | Un solo hilo con `poll`/epoll o un `TcpListener` por puerto compartido entre v4/v6 (dual-stack con `IPV6_V6ONLY=0`). | M (1 día) |
| **E-05** | P2 | `web/cockpit_template.html:1588` (`setInterval(refreshHardware, 12000)`), `:1616` (`setInterval(refreshPower, 20000)`) | Sondeo **ciego** cada 12 s y 20 s, sin parar cuando la pestaña está oculta, sin backoff si falla, y **sin usuario** (A-05: el rate limit no cubre el loopback). | (C) `cockpit_template.html:1588`, `:1616`. | 5 peticiones/minuto permanentes desde el navegador del móvil; con la UI abierta en el fondo, el daemon nunca está quieto. | `document.visibilityState` + backoff exponencial + `AbortController` para cancelar la anterior (esto último arregla también la carrera UI-13). | S (2 h) |
| **E-06** | P2 | `rust-core/src/android.rs:191-215` (`audit`) | `audit.ndjson` se **añade** (append) en cada llamada y **nunca rota**. Cada `root_exec` escribe una línea con el comando completo. | (C) `android.rs:203-213` (`.append(true)`, sin rotación en el fichero). | Crecimiento ilimitado en el almacenamiento del móvil (con la app: `/data/user/0/…`, que Android limpia al desinstalar, pero no mientras se usa). | Rotar por tamaño (5 MB × 3) dentro de `audit()` + redactor del comando. | S (1 h) |
| **E-07** | P2 | `rust-core/src/seere_v11.rs:1313-1320` | Cada `exec_root` prueba hasta **3 runners** (`su`, `rish`, `sh`), y cada intento lanza **una sonda** (`Command::new(prog).arg("exit 0")`) ⇒ hasta **6 forks** por orden ejecutada, más el comando. | (C) `seere_v11.rs:1313-1320`. | Un chat con 10 turnos = 60 forks en el móvil (y `su` pide a Magisk: cada `su` es un diálogo con el gestor de root). | Cachear qué runner funciona (detectado una vez por sesión) y saltarse la sonda si ya se sabe. | S (2 h) |
| **E-08** | P2 | `cockpit_server.rs:1015-1040` (`/api/brain_status`) | Cada refresco de la UI llama a `brain_status`, que **relee la bóveda de proveedores** (E-02) + **abre un socket TCP** (`check_llama_online`, 250 ms de timeout) + sondea el NPU (con caché de 5 s). | (C) `cockpit_server.rs:1016-1018`. | Cada 12 s: 1 lectura de fichero + 1 conexión + trabajo de CPU, con el móvil al límite. | Un endpoint de "resumen" que reúna todo lo que la UI pinta cada 12 s (y que el daemon cachee 5-10 s). | M (½ día) |

---

## F. ARQUITECTURA

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **F-01** | P1 | `rust-core/src/mesh.rs:62` (`BOARD_FILE = "mesh_board.json"`), `:64` (`mesh_journal.ndjson`), `:342-348` vs `rust-core/src/coordination.rs:237` (`board.sqlite`) | **Dos tableros de tareas** con formatos, bloqueos y modelos de datos distintos: `mesh.rs` (JSON + fichero de lock + `MeshTask`/`AgentTask`/`TaskClaim`) y `coordination.rs` (SQLite + WAL + `BoardTask`/`TaskStatus`). El `MASTER_PLAN.md` §6 ya lo marca como "P0.6 a resolver" y §8 pregunta qué hacer: **sigue sin resolverse dos meses después**, y el módulo `webui.rs` (v2) lee `board.sqlite` mientras `mesh` sigue vivo. | (C) `mesh.rs:56-64` y `:337-348`; `coordination.rs:226-245`. Búsqueda: `grep -rn "board.sqlite\|mesh_board.json"` → 2 implementaciones distintas. | Dos fuentes de verdad ⇒ las tareas del plan existen en un sitio y las del enjambre en otro; el tablero "único" no lo es. | Decidir (la propuesta del plan: `coordination.rs` = estado, `mesh.rs` = transporte) y **apagar** el tablero de `mesh` (borrar el módulo o dejar sólo discovery/tunnel). | M (1-2 días) |
| **F-02** | P1 | `rust-core/src/mcp_server.rs:20-34` (`tool_definitions`) vs `rust-core/src/mcp_ext.rs:15-235` | El catálogo MCP expone `core + android + sync + adb` = **27 tools**. Las **17 herramientas v14** (`capabilities`, `task_queue`, `alerts`, `agent`, `apk_inventory`, `dex_decompile`, `app_intel`, `root_hide`, `repo_updates`, `profile`, `messaging`, `bookmarks`, `battery_saver`, `git_ops`, `identity_and_web`, `root_stealth`, `ssh_remote`) **sólo** se alcanzan por `POST /api/tool/<nombre>`, una ruta REST propia que un cliente MCP **no puede descubrir**. `mcp_ext::tool_definitions()` **sólo** lo llama `webui.rs`, que a su vez no está cableado (F-05). | (C) `mcp_server.rs:20-34` (no incluye `mcp_ext`), `cockpit_server.rs:1329-1340` (la ruta REST), `grep -rn "mcp_ext::tool_definitions"` → **una** llamada, en `webui.rs`. | El agente externo (que habla MCP) **no puede usar** el decompilador DEX, el inventario de APKs ni la intel de apps: exactamente lo que el prompt le pide hacer (H-03). | Fusionar `mcp_ext::tool_definitions()` en `mcp_server::tool_definitions()` y el `dispatch` en `dispatch_tool` (con el mismo contrato de `confirm`). | S (½ día) |
| **F-03** | P1 | `rust-core/src/cockpit_doctor.rs:518` (`pub fn run_doctor_cli`) vs `rust-core/src/main.rs:55-330` (sin subcomando `doctor`) | El módulo **`cockpit_doctor` existe y funciona**, pero `main.rs` **no lo cablea**: no hay `megadaemon-core doctor`. El tablón dice "el principal cablea `pub mod cockpit_doctor;` y el subcomando `doctor`" — la primera mitad se hizo, la segunda no. | (C) `cockpit_doctor.rs:518` (la función existe) + `grep -n '"doctor"' rust-core/src/main.rs` → **0 resultados**. | La herramienta creada **exactamente** para diagnosticar "el proceso está vivo pero /health no responde" **no se puede ejecutar**. Es el instrumento que faltaba en D-03. | Añadir el subcomando `doctor` a `main.rs` (5 líneas) y que el supervisor lo use cuando `/health` falle. | S (1 h) |
| **F-04** | P2 | `rust-core/src/bin/md-coord.rs:59-180` vs `docs/MASTER_PLAN.md:213-222` | El CLI tiene `register/heartbeat/add/claim/report/tasks/instances/migrate/snapshot` pero **no `sync`**, que es el comando documentado (`md-coord sync --peer http://<ip>:9099 --token <t>`). | (C) `grep -n '"sync"' rust-core/src/bin/md-coord.rs` → 0 (sólo aparece `instances`). | El sync peer-a-peer documentado no se puede lanzar; la documentación miente sobre la herramienta. | Añadir el subcomando `sync` (la función `sync_with_peer` ya existe en `coordination.rs`). | S (2 h) |
| **F-05** | P2 | `rust-core/src/lib.rs:9-70` (62 `pub mod`), ficheros `rust-core/src/webui.rs` (93 KB) y `rust-core/src/signing_vault.rs` (27 KB) sin declarar | **120 KB de código escrito que no se compila**: `webui.rs` (la UI v2) y `signing_vault.rs` (el vault de firma) están en disco pero **no** en `lib.rs` ⇒ no compilan, no se prueban y no se despliegan. | (C) `lib.rs` completo (70 líneas, sin `webui` ni `signing_vault`); `git status` los marca como `??` (untracked). | La "v2 de la UI" que el plan da por en marcha **no existe como código compilable**; y el vault de firma tampoco. Regla del propio repo: *"código sin cablear es código que no existe"*. | Declarar los módulos **con su arnés de test** (el patrón `#[path]` de `tests/`), o borrarlos del árbol hasta que haya tiempo. | M (1 día) |
| **F-06** | P2 | `rust-core/src/cockpit_server.rs:37-45` (`state_dir()` → `/data/user/0/com.agy/files/megadaemon/state`) vs `rust-core/src/bin/md-coord.rs:52-55` (`/data/local/tmp/megadaemon`) vs `rust-core/src/mesh.rs:320` (`default_state_dir`) | **Tres** directorios de estado distintos según por dónde se entre. En el móvil: el token y la key viven en el dir privado de la app, el tablero en `/data/local/tmp`, el log en `/sdcard`. | (C) las tres líneas citadas + `web_bridge.py:48-53` (que busca la key en **dos** sitios). | Un fichero "no aparece" en un sitio y sí en otro; el `state_dir` del daemon no es el que usan las tools de coordinación. | **Una** variable `MEGADAEMON_STATE_DIR` como única fuente, leída de `megadaemon.env` por todos (daemon, md-coord, puente, supervisor). | S (3 h) |
| **F-07** | P2 | `rust-core/src/android.rs:1-20` y 40 módulos con rutas `/data/user/0/com.agy/...` | El núcleo está atado a **rutas absolutas de Android**: `cockpit_server.rs:26-29` (`MEGADAEMON_DIR`, `RE_DIR`, `SCUMMVM_TOOLS_DIR`, `STATE_DIR`), `shm.rs:5` (`SHM_PATH`), `seere_v11.rs` (STATE_DIR), `sync.rs`, `messaging.rs`… En un PC Linux esas rutas no existen → las tools devuelven errores crípticos ("no such file or directory") en lugar de "esta capacidad sólo existe en Android". | (C) `cockpit_server.rs:26-29`; `android.rs:719-735` (`fs_read` de una ruta de Android). | El PC Linux (el "worker" del plan) es casi inservible: la mitad de las tools fallan con errores que no explican la causa. | Un `paths.rs` con resolución por entorno (`MEGADAEMON_HOME`) + un `capability::verify()` que **diga** "requiere Android" antes de que la tool falle. | M (1-2 días) |
| **F-08** | P2 | `rust-core/src/android.rs:101-140` (`root_exec` → `Command::new("su")`) | En un PC Linux sin `su`, `root_exec` devuelve `stderr: "no se pudo ejecutar 'su': ..."` — un error **sin contexto** ("¿estoy en el móvil?"). `root_exec_tool` lo envuelve en `{"ok": false, "result": {...}}` con `code: null`. | (C) `android.rs:110-120` (mensaje literal) y `:829-835`. | El agente externo en el PC no sabe si falló el comando o el entorno. | `Err("no_android_runtime")` explícito + `platform` en la respuesta de error. | S (1 h) |
| **F-09** | P2 | `rust-core/src/cockpit_server.rs:850-1304` (GET), `:1310-1806` (POST) | El router son **dos `match` gigantes** con ~90 rutas, duplicados entre GET y POST (p.ej. `/api/kaitai_parse`, `/api/memory_freeze` existen en los dos arms con cuerpos distintos) y un `catch-all` que **no falla** (A-08). No hay tabla de rutas ni *feature test* que la recorra. | (C) `cockpit_server.rs:851` y `:1310`; duplicados: `/api/kaitai_parse` en `:1122` y `:1554`; `/api/memory_freeze` en `:1213` y `:1709`. | Un cambio de ruta se hace en un arm y se olvida en el otro ⇒ endpoints " fantasma" (GET y POST con semántica distinta). | Tabla de rutas (`&[(&str, HttpMethod, Handler)]`) generada desde una única fuente, con test que recorra **todas** las rutas y compruebe auth+status. | M (1-2 días) |

---

## G. COBERTURA DE TESTS

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **G-01** | P1 | `rust-core/tests/` (listado) | **No existe `audit_scout_ui.rs`**, que es el fichero que `docs/AUDIT_V14_UI_HTTP.md` cita como prueba de sus 15 hallazgos ("*cada hallazgo se reproduce con un test ejecutable que FALLA con el código actual (`rust-core/tests/audit_scout_ui.rs`)*"). Tampoco existe `cockpit_doctor_smoke.rs` (7 tests). | (C) `ls rust-core/tests/` → `audit_scout_auth.rs, audit_scout_hw.rs, audit_scout_seere.rs, audit_v13.rs, delegate.rs, fake_nnapi.c, harness_policy.rs, npu_nnapi.rs, root_hide_app_intel.rs, signing.rs, signing_vault.rs, zz_mesh_harness_tmp.rs, zz_tmp_dex_decompile.rs`. | Los arreglos de la auditoría UI (UI-01…UI-15) **no son verificables**: cualquiera puede decir "ya está arreglado" sin que exista el test. Es el mismo problema que esta auditoría va a repetir si el arnés no se versiona. | Versionar el arnés de cada auditoría **en el mismo commit** que el arreglo, y que CI lo ejecute. | S (2 h) |
| **G-02** | P1 | 31 módulos de `rust-core/src/*.rs` **sin un solo `#[cfg(test)]`**: `seere_v11.rs` (2.348 líneas), `cockpit_server.rs` (1.948), `providers.rs` (759), `cirx_rtl.rs` (759), `binwalk_die.rs` (758), `npu_cv.rs` (427), `disasm.rs` (377), `npu_bridge.rs` (375), `kaitai_imhex.rs` (326), `provider_catalog.rs` (294), `mcp_server.rs` (291), `frida_dbi.rs` (275), `shm.rs` (267), `rea_assistant.rs` (267), `tile_vram.rs` (261), `power_policy.rs` (250), `cfg_lifter.rs` (245), `adb_tool.rs` (209), `hw_mapper.rs` (203), `crypto_scan.rs` (194), `rom_locator.rs` (193), `freezer.rs` (192), `deobfuscate.rs` (191), `unifier.rs` (171), `snapshot.rs` (128), `entropy.rs` (102), `pointers.rs` (97), `pattern.rs` (94), `bindiff.rs` (91) | (C) `for f in src/*.rs; do grep -q "cfg(test)" $f || echo $f; done` → la lista de arriba; los que sí tienen tests: `messaging` (33), `delegate` (25), `battery_advanced` (23), `repo_updates` (21), `mesh` (21), `dex_decompile` (21), `sshd` (17), `root_hide` (16), `app_intel` (16)… | El núcleo que **decide** (SEERE, el cockpit, los proveedores, el MCP) no tiene ni un test unitario. Los 121 tests del repo cubren parsers, no control de flujo. | Prioridad por riesgo: (1) `cockpit_server` (auth, rutas, errores), (2) `seere_v11` (política de exec), (3) `providers` (bóveda, precios), (4) `mcp_server` (JSON-RPC). Mínimo 5 tests por módulo. | L (varias olas) |
| **G-03** | P2 | `rust-core/tests/zz_mesh_harness_tmp.rs`, `zz_tmp_dex_decompile.rs` | Ficheros de test **temporales** (`zz_*_tmp`) que nadie ha borrado: 0 tests, sólo incluyen el módulo por `#[path]`. Uno de ellos (`zz_mesh_harness_tmp.rs`) incluye `mesh.rs` **fuera de `lib.rs`**, es decir, compila una **tercera copia** del tablero (F-01). | (C) `ls -la rust-core/tests/` + `cat rust-core/tests/zz_mesh_harness_tmp.rs` (325 B, 0 tests). | Ruido en la suite y copia divergente de `mesh.rs` que "compila verde" sin ser la que se despliega. | Borrarlos (o renombrarlos con el prefijo del módulo y convertirlos en tests reales). | S (15 min) |
| **G-04** | P1 | `rust-core/tests/` (ninguno prueba el gate de auth del cockpit) | Ningún test cubre la **matriz de autenticación** del cockpit: qué rutas son públicas, cuáles exigen token, cuáles exigen API key, qué hace el CORS. Es la superficie que produce el P0 A-07. | (C) `grep -rn "X-Seere-Auth-Token\|requires_auth\|cors_allowed" rust-core/tests/` → sin cobertura del servidor. | Los cambios en auth (que son los de v14) se despliegan **sin red de seguridad**: por eso A-07 ha podido colarse. | Un test de integración que arranque el cockpit y recorra **todas** las rutas con y sin token (este arnés lo hace ya, con 4 aserciones). | M (½ día) |
| **G-05** | P2 | `rust-core/Cargo.toml:44` (`panic = "abort"`) | Con `panic = "abort"` **no hay `catch_unwind`** y un test de panic aborta todo el binario de test. No hay ningún test que compruebe que las rutas HTTP **no** paniquean (C-06, C-07). | (C) `Cargo.toml` `[profile.release] panic = "abort"`; `grep -rn "catch_unwind" src/` → 0. | Un panic en el móvil = daemon muerto + supervisor que puede no volver (D-01). | (a) `catch_unwind` por conexión en `handle_client` (con `AssertUnwindSafe`); (b) o `panic = "unwind"` en release + `catch_unwind` en los hilos. | S (2 h) |
| **G-06** | P3 | `rust-core/tests/` + `Cargo.toml` | Coste de la suite: **102 tests** en 12 ficheros de integración + ~387 tests unitarios. Con `CARGO_TARGET_DIR` local el build completo del crate + tests tardó **33 s** (medido por otro agente, `docs/AUDIT_DELEGACION_V14.md` §4 Q-1), pero el **coste del binario** crece: `signing.rs` (207 KB), `dex_decompile.rs` (179 KB) y `profile_analysis.rs` (100 KB) compilan **siempre**, aunque el móvil no los use. | (C) `grep -c '#\[test\]' rust-core/tests/*.rs` → 102; `wc -l src/*.rs` → 64.918 líneas. | Cada agente paga el build completo; el binario del móvil pesa más de lo necesario (LTO + `codegen-units=1` sobre 69 módulos). | Features por binario (`--no-default-features` + features `re`, `signing`, `mesh`) para el móvil. | M (1 día) |

---

## H. EL AGENTE EXTERNO Y EL CONVERSOR DE FINDINGS

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **H-01** | P2 | `tools/findings_from_manifest.py:439-445` (mueve `signal`→`class`) y `:499` (escribe `src.get("signal")`, **no** la variable local) | El conversor declara resolver la colisión histórica DEX/app_intel *"se mueve (y se anota) en vez de duplicarse"*. En realidad **duplica**: cuando un `signal` con valor de enum de `class` se mueve a `class`, `signal` **sigue con el mismo valor** (porque el `out` lee `src`, no la variable local ya anulada). | **(T)** `adv_h01_…`: `normalize({'signal':'fs_path','class':'unknown'})` → `class="fs_path"` **y** `signal="fs_path"`. Reproducción directa: `python3 -c "import findings_from_manifest as F; print(F.normalize({'package':'com.x','signal':'fs_path','class':'unknown'},{},[]))"`. | El esquema cerrado v1.2 queda **violado en el campo que el bus de coordinación usa para decidir**: un valor de clase duplicado como "valor crudo" hace que el consumidor crea que se observó una señal `/sbin/.magisk` cuando en realidad era una etiqueta de clase. | Escribir el `out` con las **variables locales** (`signal`, `method_raw`) y no con `src`. | S (30 min) |
| **H-02** | P2 | `docs/CONNECT_EXTERNAL_AGENT_ANDROID_RE.md:22` y `:70` vs `rust-core/src/cockpit_server.rs:123-151` | El doc dice que el token de escritura *"sólo se necesita en POST/PUT"*, pero **toda** llamada MCP es un POST a `/mcp` y `requires_auth()` **no mira el método**: el token es obligatorio **siempre**, también para `tools/list`. | (C) `cockpit_server.rs:123` (`fn requires_auth(path: &str, _method: &str)` — el método se **ignora**) + `:1418` (`/mcp` en la rama POST). | El agente externo que siga la lectura literal ("tools/list sin token") recibe **403** y puede concluir que la instancia está caída. | Corregir el doc ("el token es obligatorio en **todas** las llamadas MCP") y, mejor, eximir `tools/list` del token (es metadato, no estado). | S (30 min) |
| **H-03** | P1 | `docs/CONNECT_EXTERNAL_AGENT_ANDROID_RE.md:59,108-134` (26 tools) vs `rust-core/src/mcp_server.rs:20-34` (27) vs `rust-core/src/mcp_ext.rs:15-235` (17 más) | El contrato dice "26 tools, verificado contra el binario desplegado v13.3.0" y advierte que el repo tiene más. Al desplegar v14 el MCP expone **27** (nuevo: `adb`), y las **17 herramientas v14 siguen inalcanzables por MCP** (F-02). El prompt le pide al agente "no los busques en tools/list: si no están, no los uses" ⇒ **el agente renuncia a DEX/APK/app_intel justo cuando lo que el usuario le ha pedido es eso**. | (C) `mcp_server.rs:20-34` (core 7 + android 11 + sync 8 + adb 1 = 27) y `mcp_ext.rs` (17 más, sólo REST). | El agente externo no puede hacer el trabajo para el que se le contrató. | F-02 (fusionar los catálogos) + regenerar el prompt desde `tools/list` **real** (un script que lo descargue del móvil y lo pegue en el doc). | M (1 día) |
| **H-04** | P2 | `tools/mdmcp.py:35-45` (`timeout=900`), `:48-53` (`init()` en cada `call`) | Cada llamada MCP hace **dos** viajes (`initialize` + la llamada) y usa un timeout de **900 s** sin reintentos ni backoff. Con el rate limit por IP (A-05 lo desactiva para loopback, pero el túnel viene de 127.0.0.1 ⇒ tampoco aplica)… | (C) `mdmcp.py:35`, `:56-58`. | Con la tasa de 429 que el enjambre sufrió, el cliente espera 900 s y el agente interpreta "cayó la instancia". | Reintentos con backoff exponencial ante 429/5xx, `initialize` **una vez** por sesión, timeout adaptativo (30 s por defecto). | S (1 h) |
| **H-05** | P3 | `tools/findings_from_manifest.py:233-237` | La docstring de `load_manifest` dice *"-> (artefactos, indice_evidence)"* pero la función **devuelve 3** valores (`artifacts, index, broken`). La contradicción ya costó un bug (`broken` sin usar). | (C) `findings_from_manifest.py:234` vs `:261`. | Ruido menor, pero es el tipo de cosa que hace que el siguiente agente lea el contrato equivocado. | Corregir la docstring. | S (5 min) |

---

## I. UX DE LA UI v1 (`web/cockpit_template.html`)

> Diagnóstico de la **v1**. La v2 (`rust-core/src/webui.rs`, aún sin cablear) es otro tema.

| ID | Sev | Ubicación | Defecto | Verificación | Impacto | Corrección | Coste |
|---|---|---|---|---|---|---|---|
| **I-01** | **P0** | `web/cockpit_template.html:795-801` (`authHeaders`) + `rust-core/src/cockpit_server.rs:872` (token vacío inyectado) | La v1 **sólo** sabe usar `window.MEGADAEMON_TOKEN`, y la v14 lo inyecta **vacío**. No hay ninguna otra vía: la plantilla no menciona `X-API-Key` ni `/api/session/exchange` (búsqueda literal: **0 coincidencias**). | **(T)** `adv_a07_…`: con el binario v14, `GET /api/status` sin token ⇒ 403, y la plantilla no tiene forma de obtenerlo. (C) `cockpit_template.html:795-801`. | **La web v1 con el binario v14 no muestra un solo dato**: todas las pestañas (SWARM/NPU/INSPECT/VRAM/SYSTEM) quedan vacías con un "HTTP 403" en cada `catch`. | Resolver A-07 + añadir a la v1 un campo de API key con llamada al intercambio al arrancar (o reusar el mecanismo de la v2). | M (1 día) |
| **I-02** | P1 | `web/cockpit_template.html` (todo el fichero) | **No hay forma de introducir la API key ni de autenticar**; tampoco indicador de "401/403" (los `catch` sólo pintan el texto del error en la caja). | (C) `grep -n "X-API-Key\|api_key\|session/exchange\|401" cockpit_template.html` → 0 coincidencias (el único `api_key` es el del *provider vault*). | El usuario ve cajas con "HTTP 403" sin ninguna pista de qué hacer. | Un banner de estado de auth con campo de API key (y que se guarde en `localStorage`, no en el repo). | S (3 h) |
| **I-03** | P2 | `cockpit_template.html:976-985` (base64 en el navegador) + `cockpit_server.rs:695` (8 MB) | La subida **falla a partir de ~6 MB** y el error es un 413 sin explicación; el comentario del servidor dice "100 MB" y el endpoint se anuncia como "100% UNRESTRICTED". | (C) `cockpit_template.html:976-981` + `cockpit_server.rs:692-695`. | El caso de uso real (subir un APK) falla en el móvil sin diagnóstico. | Trocear la subida en el navegador (o subir por `fs_write` por trozos, que ya funciona) + mensaje explícito del límite. | M (½ día) |
| **I-04** | P2 | `cockpit_template.html` (sin versión, sin estado del daemon, sin contadores) | La UI **no muestra la versión del daemon**, ni el estado del servicio, ni el consumo (RAM/CPU del propio daemon), ni si la API key está activa, ni el rate limit. Hay un `/metrics` en el servidor que nadie usa. | (C) `grep -n "version" cockpit_template.html` → 0 usos del `version` del daemon; `cockpit_server.rs:943-953` (`/metrics`) sin consumidor. | "La web no me funciona" se queda sin diagnosticar desde la propia web (que es justo lo que motivó `cockpit_doctor`). | Una tira de estado arriba (versión, uptime, RAM del daemon, clave activa, puertos) leyendo `/health` + `/metrics`. | S (3 h) |
| **I-05** | P2 | `cockpit_template.html:1588` y `:1616` | El sondeo no se detiene con la pestaña oculta, no tiene backoff si falla, y **no cancela** la petición anterior (la carrera UI-13 ya auditada sigue viva). En un móvil, la UI abierta en segundo plano mantiene el daemon en_requests. | (C) las dos líneas + `cockpit_template.html:1528` (la carrera, ↩ UI-13). | Consumo de batería y de red del móvil; y estados obsoletos en pantalla. | `visibilitychange` + `AbortController` + contador de secuencia (el arreglo que propose UI-13). | S (2 h) |
| **I-06** | P2 | `cockpit_template.html` (5 pestañas: swarm/npu/inspect/vram/system) | **Nada de lo nuevo es visible**: no hay vista del **tablero de tareas**, ni de las **alertas**, ni del **perfil**, ni de la **delegación**, ni de los **reportes de findings** del agente externo. 44 capacidades v14 viven en la API y 0 en la pantalla. | (C) `grep -n 'id="view-' cockpit_template.html` → 5 vistas; `mcp_ext.rs` define 17 herramientas. | El producto se ve como "un visor de ROMs y un chat"; el 80 % del trabajo del enjambre es invisible para el usuario. | Priorizar 2-3 vistas nuevas en la v2 (Estado del enjambre / Tareas / Hallazgos). | L (1-2 olas) |
| **I-07** | P3 | `cockpit_template.html` (34 `catch (e)` que pintan el error en la caja) | Cada error se muestra en la caja de la sección, sin distinguir "daemon caído" de "403" de "respuesta vacía", y **sin reintento ni acción**. El usuario ve "Error: HTTP 403" sin contexto. | (C) `grep -c "catch (e)" cockpit_template.html` → 34; no hay ningún manejador global. | Diagnóstico lento: el usuario tiene que abrir el log del móvil a mano. | Un manejador global de errores con toast + reintento + enlace a `/health`. | S (2 h) |

---

## J. LO QUE NO SE PUEDE HACER (límites honestos)

Esto **no son defectos**: son límites del sistema. Se listan porque el plan los
hereda y porque el usuario debe saber qué **no** se puede prometer.

| # | Límite | Por qué | Qué haría falta | Impacto en el plan |
|---|---|---|---|---|
| **J-01** | **Play Integrity / attestation** | La firma es del hardware; `ro.boot.verifiedbootstate` etc. son **atestaciones del TEE**, no propiedades de `system.prop`. `root_hide.rs` sólo manipula props y presence de ficheros: **no puede** falsificar la atestación, y el propio repo lo dice (`docs/MASTER_PLAN.md` P3.2). | Hardware firmado (Keymaster/StrongBox con clave del fabricante) o un **TEE attestation agent** real. | **P1.4 (ocultación de root) nunca dará apps de banca**. Debe venderse como "reduce la superficie detectable", no como "pasa Play Integrity". |
| **J-02** | **Cookies de Chrome** | Android las cifra con una clave del **AndroidKeyStore** en el TEE; ni root ni ADB las exportan (`docs/MASTER_PLAN.md` P3.1, `docs/AUDIT_ENJAMBRE_V14.md` §6). | Inyección en proceso (Frida) o una exportación oficial del fabricante. | `sync.rs --cookies` **nunca** admite cookies de Chrome: hay que decirlo en la UI, no esconderlo. |
| **J-03** | **WhatsApp / cifrado de la base** | El almacén está cifrado con la clave del keystore del proceso; el `messaging.rs` lo reporta como "no legible" (correcto). | Decrypt con Frida dentro del proceso, o un backup del proveedor. | `backup_full.rs` sólo puede entregar **metadatos**, no mensajes. |
| **J-04** | **Google Play sin API de instalación** | No hay API pública para instalar/actualizar desde Play; automatizarlo es automatizar la cuenta del usuario. | `Obtainium`/F-Droid (ya está en `repo_updates.rs`) o la API de Play Console para apps propias. | `repo_updates` cubre **F-Droid/Obtainium**, no Play. Decirlo en la UI. |
| **J-05** | **FNMT-RCM (Java)** | El configurador exige Java 17; no hay reimplementación abierta. | El atajo ya descrito: la app oficial de la FNMT en Android emite el `.p12` leyendo el DNIe por NFC; a partir del `.p12`, todo es Rust (`signing.rs` + `signing_vault.rs`). | `signing_vault.rs` **no está cableado** (F-05): la parte que sí puede hacerse (emitir/guardar `.p12`) no está en el binario. |
| **J-06** | **Historial de Gemini / modelos cloud** | Vive en el servidor, no en local; sin API oficial de exportación para la cuenta. | La API oficial (con coste) o el export manual. | `app_intel.rs` puede decir **dónde** está, no **leer** lo que hay en el servidor. |
| **J-07** | **Portabilidad del daemon a PC** | ~40 módulos asumen rutas y herramientas de Android (`su`, `magisk`, `getprop`). En Linux fallan con errores sin contexto (F-07/F-08). | `capability::verify()` por plataforma + un modo "host" que degrade con mensajes claros. | El PC Linux es un **worker de tareas simples**, no un MegaDaemon completo. |

---

## PLAN DE MEJORA DERIVADO (4 olas)

> Orden de ataque pensado para el **S21 Ultra** (10,2/10,5 GB, 78 % iow) y para
> el **canal de despliegue** (el móvil tiene v13.3.0; el repo v14).

### Ola 1 — PARAR EL SANGRADO (día 0-1, ~6 h) — *desbloquea el despliegue*

Objetivo: que desplegar v14 no tumbe el móvil ni regale root.

1. **A-18** (el arreglo de A-07 entrega el token root a lo "local") — es el
   P0 más grave del repo ahora mismo y **es una regresión de hace 10 minutos**:
   el intercambio **debe** exigir la API key real siempre, y `local_browser`
   **no** puede depender del `Host` ni de la ausencia de cabeceras de proxy.
   (A-07 ya lo arregló el owner del cockpit a las 22:48.)
2. **A-01** y **A-02** (RCE root por `fs_write` y por `find_duplicates`) — 2 vectores
   de ejecución como root alcanzables desde el MCP que usa el agente externo.
   Mitigación inmediata: base64 siempre en `fs_write` + `stat` fuera de AWK.
3. **A-04** (fail-open sin key) + **B-01/B-02** (rotar credenciales expuestas) +
   **A-15** (el puente regala root a la LAN).
4. **A-16/E-03** (techo de 512 MB) antes de que entre nadie por el túnel.
5. **D-01/D-02** (el supervisor se apaga; el despliegue nunca falla) — sin esto, un
   despliegue a medias deja el móvil sin daemon **y sin aviso**.
6. **G-01** (recuperar `audit_scout_ui.rs`): sin arnés versionado, los arreglos
   de esta auditoría tampoco se van a poder verificar.

> **Criterio de salida**: los 5 P0 con test en rojo→verde en
> `rust-core/tests/audit_adversarial_v15.rs`, el token rotado, y
> `GET /` + `GET /api/status` funcionando en el móvil con el binario nuevo.

### Ola 2 — ROBUSTEZ (días 2-5, ~3 días) — *que el sistema se sostenga solo*

- **A-03/A-06/A-11**: **una sola política de destructividad** en todo el daemon
  (`Arsenal::exec_root` → `android::root_exec_tool`), y `confirm` para `/data/adb`.
- **D-03 + F-03**: el supervisor decide con `/health`, no con `pgrep`; y
  `megadaemon-core doctor` existe (5 líneas) para el diagnóstico.
- **A-11/C-06/C-07/G-05**: `catch_unwind` por conexión + `lock().unwrap_or_else`:
  **que ningún panic tumbe el daemon** (`panic = "abort"` es una bomba de relojería).
- **C-09 + B-05 + D-07**: escrituras atómicas en `providers.json`, el registro de
  keys y `findings.ndjson`.
- **D-06**: `busy_timeout` en el tablero.
- **C-04/C-03/C-05/C-02**: Status texts correctos, **una** versión en toda la API,
  errores propagados (no `unwrap_or_default`).
- **C-10 + I-03**: subida por trozos (para que el agente externo pueda subir APKs).

### Ola 3 — RENDIMIENTO Y EXPERIENCIA (semana 1-2, ~4 días)

- **E-01/E-02/E-08**: **cachés** para la config de la API key, la bóveda de
  proveedores y el resumen de estado; un endpoint "resumen" para la UI.
- **A-16/E-03/E-04**: topes de memoria y de hilos **acumulados**, no por conexión;
  `Content-Length` obligatorio; revisar el número de hilos.
- **I-01…I-07**: la UI v2 (`webui.rs`, **sin cablear**: F-05) con: auth visible,
  estado del daemon (versión/RAM/puertos), fin del sondeo ciego, subida por
  trozos, y **3 vistas nuevas** (estado del enjambre, tareas, hallazgos) para que
  el producto se vea como lo que es.
- **D-04/E-05/E-06/E-07**: menos forks (supervisor), sondeo con `visibilityState`,
  rotación de `audit.ndjson`, cachear el runner de root.
- **F-05**: cablear (o borrar) `webui.rs` y `signing_vault.rs`.

### Ola 4 — EXPANSIÓN (semana 2-4, ~2 semanas) — *lo que faltaba*

- **F-01**: decidir el tablero único (`coordination.rs` = estado, `mesh.rs` =
  transporte) y **apagar** el segundo. Sin esto, la Ola 0 de "que el sistema se
  sostenga solo" del plan maestro sigue sin poder cerrarse.
- **F-02 + H-03**: un **catálogo MCP único** (las 17 tools v14 incluidos) y el
  contrato del agente externo regenerado **desde el móvil real** (script que
  descarga `tools/list` y regenera el prompt).
- **H-01/H-02/H-04**: conversor de findings correcto (sin duplicados), doc del
  contrato corregido y cliente MCP con reintentos.
- **F-06/F-07/F-08**: un `MEGADAEMON_STATE_DIR` único y `capability::verify()`
  por plataforma (que el PC diga "esto es Android-only" en vez de reventar).
- **F-04**: `md-coord sync` (el comando que el plan ya promete).
- **J-05**: cablear el vault de firma para que el atajo FNMT (app oficial →
  `.p12` → Rust puro) exista de verdad.
- **G-02**: cobertura de los 31 módulos sin test, empezando por los de decisión
  (`seere_v11`, `providers`, `mcp_server`, `cockpit_server`).

### Dependencias (qué desbloquea qué)

```
Ola 1 ──┬─► Ola 2 ──┬─► Ola 3 ──► Ola 4
        │           │
        └─ A-07 ───┴─► (la UI v1 sirve de campo de pruebas para la v2)
F-01 (tablero único) ─────────────────► Ola 4 (protocolo distribuido real)
F-02 (catálogo MCP) ──────────────────► H-03 (el agente externo puede trabajar)
G-01 (arneses versionados) ───────────► todas (nada se puede verificar si no)
```

---

## TOP 10 P0/P1 (numerados, con su evidencia)

| # | ID | Sev | Defecto | Evidencia |
|---|---|---|---|---|
| **1** | **A-01** | P0 | `fs_write` ejecuta el contenido del usuario como shell root (heredoc `__MD_EOF__`) | **Test** `adv_a01_fs_write_inyecta_comandos_por_el_heredoc` (el marcador aparece) + `android.rs:779` |
| **2** | **A-02** | P0 | `find_duplicates` ejecuta el **nombre del fichero** (AWK → shell) como root | **Test** `adv_a02_find_duplicates_inyecta_comandos_por_el_nombre_de_fichero` + `android.rs:808` |
| **3** | **A-18** | P0 | La exención `local_browser` entrega el **token root** a cualquier petición "local" (`Host` falsificable) ⇒ **cualquier app del móvil** se lo lleva por `127.0.0.1:9090` | **Test** `adv_a18_la_exencion_local_entrega_el_token_root` + `cockpit_server.rs:846/855/921/959/1390` |
| **4** | **A-04** | P0 | `MEGADAEMON_REQUIRE_API_KEY=1` no fuerza nada si no hay key ⇒ fail-open del túnel | **Test** `adv_a04_require_api_key_se_ignora_sin_key_configurada` + `api_key.rs:311-318` |
| **5** | **B-01** | P0 | API key real (`mdaemon_sk_live_<prefijo redactado>`) y token de sesión root (`c17398e1…`) publicados en el árbol de trabajo | `docs/EXTERNAL_AGENT_PROMPT_FINAL.md` (líneas 37/63/64 a las 21:21; redactado a las 21:28) → **rotar** |
| **6** | **B-02** | P0 | Token de un túnel Cloudflare en claro en un fichero **versionado** | `tools/upload_bin.py:10-11` |
| **7** | **I-01** | P0 | La UI v1 sólo sabe usar `window.MEGADAEMON_TOKEN`, que el servidor nuevo inyecta **vacío** ⇒ con el binario v14 **no pinta un solo dato** (403 en cada `/api/*`) | `cockpit_template.html:795-801` + `cockpit_server.rs:959-966` + **Test** `adv_a18` (el token no llega) |
| **8** | **A-16 / E-03** | P1 | 64 clientes × 8 MB = **512 MB** de buffers (+128 MB de pilas) en un móvil con ~300 MB libres | **Test** `adv_e02_el_techo_de_memoria_del_cockpit_supera_la_ram_libre` + `cockpit_server.rs:695-718` |
| **9** | **A-11** | P1 | `/api/chat` ejecuta root **sin** la política destructiva (el MCP sí la aplica) | `seere_v11.rs:1290-1296` + `:2006-2018` vs `android.rs:821-836` |
| **10** | **D-03 / F-03** | P1 | El supervisor decide con `pgrep` ("vivo" ≠ "sirviendo") y **`cockpit_doctor::run_doctor_cli` no está cableado**: la herramienta para diagnosticar "vivo pero mudo" no se puede ejecutar | `99-megadaemon.sh:249` + `cockpit_server.rs` (sin subcomando `doctor`; la función sí existe en `cockpit_doctor.rs:518`) |

> **Ya corregidos durante esta auditoría** (no cuentan en el TOP 10, pero se
> documentan): **A-15** (el puente inyectaba key+token a la LAN → commit
> `23e82ed`), **A-07** (el intercambio era inalcanzable → UI-16 del owner del
> cockpit, aunque su arreglo introdujo A-18), **D-01** y **D-02** (el supervisor se
> apagaba y el despliegue no hacía rollback → auto-rollback + "no se rinde",
> `7fd13de`).

---

## Anexo — Cómo reproducir

```bash
cd /workspace/megadaemon/rust-core
CARGO_HOME=/run/agent-runtime/md/cargo \
CARGO_TARGET_DIR=/run/agent-runtime/md/target \
cargo test --release --test audit_adversarial_v15 -- --nocapture --test-threads=1
```

* Los tests `adv_a01`, `adv_a02` y `adv_a06` necesitan `su` (los ejecuta como
  root **en el sandbox**, no en el móvil) y **limpian** lo que crean.
* `adv_a18`, `adv_a19`, `adv_a08` y `adv_a09` arrancan el cockpit real en un
  puerto libre y hablan HTTP con él.
* `adv_h01` requiere `python3`; si no está, el test se marca como *skip*.

**Lo que este arnés NO puede comprobar** (y por eso está marcado como
`NO VERIFICADO EN RUNTIME`): el comportamiento real en el Galaxy S21 Ultra
(RSS del daemon, iow, 429 del túnel, estado del supervisor). Eso requiere el
dispositivo, que en esta sesión está en manos de otro agente.
