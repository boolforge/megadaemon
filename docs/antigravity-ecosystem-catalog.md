# Catálogo técnico del ecosistema Antigravity

> **Regla de este documento**: ninguna URL ni dato inventado. Cada repo se ha
> verificado contra la API de GitHub (existencia, lenguaje, licencia, stars y
> descripción real). Cuando el repo **no existe**, se dice explícitamente.
> Fecha de verificación: **2026-11-19**.

Formato: URL en **texto plano** (sin enlaces Markdown), ficha por repo con
categoría, qué modifica, stack, plataformas y advertencias.

---

## 0. Tabla resumen

| Proyecto | Categoría | Lenguaje | Licencia | Método | SO |
|---|---|---|---|---|---|
| AvenCores-open-antigravity-patcher | Elegibilidad/Región | Python | GPL-3.0 | Script + parche | Win/mac/Linux |
| eligibility-antigravity-patcher | Elegibilidad/Región | Python | MIT | Gestor de entorno | Win/mac/Linux |
| antigravity-cleaner | Elegibilidad/Región | Go | Apache-2.0 | Binario 1-click | Win/mac/Linux |
| antigravity-patch | Interfaz/Límites | CSS (+JS) | MIT | Parche de autorización | Win/mac/Linux |
| antigravity-bypass-russia | Geofencing | Rust | MIT | DNS/región | Win/macOS |
| antigravity-patch-proxy-remote | Proxy/Red | TypeScript | Apache-2.0 | Proxy local | Win/mac/Linux |
| antigravity-add-model | Modelos BYOK | TypeScript | Apache-2.0 | Proxy + parche | Win/mac/Linux |
| antigravity_bypass | Límites MCP | Go | AGPL-3.0 | Binario | Win/mac/Linux |
| antigravity-rtl (mmnaderi) | Interfaz RTL | JavaScript | MIT | CSS/JS | Win/mac/Linux |
| Antigravity-RTL (4klgtv) | Interfaz RTL | HTML | — | HTML/CSS | Win/mac/Linux |
| antigravity-rtl-support | Interfaz RTL | JavaScript | MIT | Extensión | Win/mac/Linux |
| antigravity-android | Android/Termux | Java | NOASSERTION | Empaquetador + Ghidra ARM64 | Android/Termux |
| reverse-engineering-antigravity | Ingeniería inversa | Python | — | Análisis/documentación | — |
| Velocity | Alternativa open source | TypeScript | GPL-2.0 | Clone funcional | Win/mac/Linux |
| antigravity-cli (oficial) | CLI oficial | — | — | Binario oficial | Win/mac/Linux |
| OMCDecoder | CSC Samsung | C++ | — | Codec CSC | Android/Termux/Linux |
| UN1CA | ROM custom | Shell | GPL-3.0 | Flash de firmware | Android |
| nexmon | Monitor mode | C | GPL-3.0 | Patch de firmware WiFi | Linux/Android |
| SOLOW-Weaponized-Kernel | Kernel hacking | — | — | Kernel pre-rooteado | Android (Fold 3) |

---

## Bloque 1 — Elegibilidad y bloqueo regional

### 1.1 Open AG Patcher
https://github.com/trustybird/AvenCores-open-antigravity-patcher
- **Categoría**: Elegibilidad/Región.
- **Propósito**: parcheador de elegibilidad regional. La descripción del repo
  es tersa, así que el mecanismo exacto (¿binario? ¿JSON de configuración?)
  **debe verificarse leyendo el repo**: no se afirma nada que no esté en la
  ficha.
- **Stack**: Python · **Licencia**: GPL-3.0 · **★ 425**.
- **Plataformas**: las del propio Antigravity (Windows/macOS/Linux).
- **Advertencias**: GPL-3.0 ⇒ los derivados deben ir también con GPL.
  Los parcheadores de elegibilidadtocan la verificación de región del cliente;
  pueden romper al actualizarse Antigravity.

### 1.2 eligibility-antigravity-patcher (agy-manager)
https://github.com/QNIX-Dev/eligibility-antigravity-patcher
- **Categoría**: Elegibilidad/Región + gestión de entorno.
- **Propósito** (descripción real): *"Cross-platform environment manager for
  Antigravity CLI, Manager & IDE — eligibility patching"*. Es decir: un gestor
  que instala/parchea el CLI, el Manager y el IDE, aplicando el parche de
  elegibilidad.
- **Stack**: Python · **Licencia**: MIT · **★ 301**.
- **Advertencias**: al ser un *gestor de entorno*, puede descargar binarios
  de terceros: verificar siempre el SHA-256 antes de ejecutar.

### 1.3 Antigravity Cleaner
https://github.com/dalroot/antigravity-cleaner
- **Categoría**: Elegibilidad/Región + limpieza.
- **Propósito** (descripción real): *"The Best 1-Click Fixer for Google
  Antigravity: Bypass Region Lock & Sanctions. Remove re…"* (descripción
  truncada en la API). Quita residues/telemetría además del bloqueo regional.
- **Stack**: Go · **Licencia**: Apache-2.0 · **★ 373**.
- **Advertencias**: binario de un solo clic ⇒ verificar hash; puede tocar
  credenciales locales de Google.

### 1.4 antigravity-patch (CLI agy)
https://github.com/kakajan/antigravity-patch
- **Categoría**: Interfaz/Límites · parche del **CLI `agy`**.
- **Propósito** (descripción real): *"Google Antigravity Patch for
  authorization"*. Parchea el flujo de autorización del CLI.
- **Stack**: CSS (el repo es principalmente assets de interfaz) · **Licencia**:
  MIT · **★ 114**.
- **Advertencias**: al tocar *authorization*, puede invalidar tokens ya
  emitidos; reaplicar tras cada actualización del CLI.

### 1.5 antigravity-bypass-russia
https://github.com/vezlin1/antigravity-bypass-russia
- **Categoría**: Geofencing / bloqueo regional.
- **Propósito** (descripción real): *"Google Antigravity & Gemini bypass in
  Russia (Windows & macOS, No VPN, SmartDNS NRPT & SIM…)"*. Usa SmartDNS NRPT
  y SIM, no VPN: sirve como **referencia técnica de técnicas de geofencing**.
- **Stack**: Rust · **Licencia**: MIT · **★ 202**.
- **Advertencias**: método basado en DNS ⇒ depende de que el operador no
  bloquee el resolvedor usado.

---

## Bloque 2 — Enrutamiento de red y proxies

### 2.1 Google Antigravity Proxy Patch (remote)
https://github.com/Aminetwiti/antigravity-patch-proxy-remote
- **Categoría**: Proxy/Red + control remoto.
- **Propósito** (descripción real): *"One IDE, every model. Custom LLM proxy
  for Google Antigravity IDE and 2.0 and control remo…"* (truncada). Es un
  **proxy local** que intercepta las peticiones del IDE y las enruta a
  proveedores externos, con control remoto.
- **Stack**: TypeScript · **Licencia**: Apache-2.0 · **★ 38**.
- **Advertencias**: un proxy ve **todas** las peticiones y respuestas
  (incluidas las claves del proveedor): audítalo, úsalo sólo con endpoints
  propios y no lo expongas a la LAN.

---

## Bloque 3 — Modelos personalizados (BYOK)

### 3.1 antigravity-add-model
https://github.com/vahapogut/antigravity-add-model
- **Categoría**: Modelos BYOK.
- **Propósito** (descripción real): *"Proxy patch for Google Antigravity that
  intercepts internal Cloud Code API and adds suppor…"* (truncada). Intercepta
  la API interna de Cloud Code y añade soporte de modelos propios.
- **Stack**: TypeScript · **Licencia**: Apache-2.0 · **★ 207**.
- **Plataformas**: las del IDE · el repo incluye `node_modules` propio.
- **Advertencias**: interceptar la API interna ⇒ **se rompe con cada
  actualización de Antigravity** (de ahí la importance del selector por
  *proxy* y no por parche binario). Es la vía recomendada en el prompt
  maestro para conmutar de proveedor "en caliente".

### 3.2 Antigravity-Proxy (slug sin verificar)
- El prompt lo listaba como *"https://github.com (verificar slug real)"*.
  **No se ha encontrado un repo con ese nombre exacto y no se inventa**:
  la vía equivalente y verificada es `antigravity-patch-proxy-remote`
  (ver 2.1) y `antigravity-add-model` (ver 3.1).

---

## Bloque 4 — Interfaz (RTL) y límites técnicos

### 4.1 antigravity_bypass (límites de herramientas MCP)
https://github.com/Futureppo/antigravity_bypass
- **Categoría**: Límites/Utilidades.
- **Propósito** (descripción real): *"Google Antigravity MCP tool limit patch:
  raise local limits to 8192 in Antigravity IDE and…"* (truncada). Sube el
  límite local de herramientas MCP a 8192.
- **Stack**: Go · **Licencia**: **AGPL-3.0** · **★ 71**.
- **Advertencias**: **AGPL** ⇒ si lo integras o lo modificas, tu obra
  derivada debepublicar el código. Al ser un parche de **límite local**, sube el
  techo del cliente, no la cuota del servidor: el servidor puede seguir
  limitando.

### 4.2 antigravity-rtl (mmnaderi)
https://github.com/mmnaderi/antigravity-rtl
- **Categoría**: Interfaz RTL (árabe, hebreo, farsi).
- **Propósito** (descripción real): *"Smart RTL (Right-to-Left) UI patcher for
  Antigravity & Antigravity IDE with Persian, Arabi…"* (truncada). Parchea la
  interfaz para RTL y fuentes propias.
- **Stack**: JavaScript · **Licencia**: MIT · **★ 408**.

### 4.3 Antigravity-RTL (4klgtv)
https://github.com/4klgtv/Antigravity-RTL
- **Categoría**: Interfaz RTL.
- **Propósito** (descripción real): *"Smart Right-to-Left (RTL) alignment and
  custom font patcher for Antigravity and major IDEs"*.
- **Stack**: HTML · **Licencia**: sin SPDX detectable · **★ 6**.
- **Advertencias**: licencia no declarada en GitHub ⇒ uso sujeto a la
  "sin licencia" (todos los derechos reservados) salvo que el repo indique
  otra cosa.

### 4.4 antigravity-rtl-support
https://github.com/AhmedMahmoud113/antigravity-rtl-support
- **Categoría**: Interfaz RTL.
- **Propósito** (descripción real): *"Fix Arabic (RTL) text rendering in
  Google Antigravity's chat panel and Markdown Preview"*.
- **Stack**: JavaScript · **Licencia**: MIT · **★ 1**.
- **Advertencias**: proyecto muy nuevo (1 estrella) ⇒ auditar antes de usarlo.

### 4.5 Slugs no confirmados
`vscode-antigravity-rtl`, `antigravity-chat-rtl-extension`,
`antigravity-chinese`, `Antigravity-Hans`: el prompt los daba truncados como
`https://github.com`. **No se han verificado** (no aparecen en la búsqueda
del API con esos nombres exactos). No se inventan: los sustitutos
verificados con la misma función son 4.1–4.4.

---

## Bloque 5 — Android, Termux y móvil

### 5.1 antigravity-android
https://github.com/Yhoanes/antigravity-android
- **Categoría**: Android/Termux.
- **Propósito** (descripción real): *"Antigravity for Android (by Yhoanes) —
  Standalone Google Antigravity IDE & Native ARM64 Re…"* (truncada). Empaqueta
  el IDE como app **standalone** para Android, con toolchain **nativa ARM64**
  (incluye Ghidra para ingeniería inversa en el propio móvil).
- **Stack**: Java · **Licencia**: NOASSERTION (sin SPDX claro) · **★ 7**.
- **Advertencias**: licence ambigua ⇒ leer el LICENSE del repo antes de
  redistribuir derivados. Es el que permite **decompilar `liblanguage_server.so`
  en el propio móvil** sin PC.

> **En este dispositivo** ya está instalado y en uso: `/data/data/com.agy/`
  (paquete `com.agy`), versión **2.11.0**, con
> `liblse_compat.so` precargado sobre `liblanguage_server.so --standalone
> --override_ide_name antigravity --subclient_type hub
> --override_ide_version 2.11.0 --https_server_port 45157 --app_data_dir
> antigravity-app --web_bundle_path /data/data/com.agy/files/web
> --enable_sidecars`.
> El toolchain nativo está en `/data/data/com.agy/files/usr` (proot-distro,
> JVM, rustlib, node) — **5,8 GB**.

### 5.2 Slugs no confirmados
`antigravity-on-android`, `Antigravity-for-Android`, `antigravity-cli-termux`,
`PocketGravity`, `Antigravity-Mobile`: dados truncados en el prompt; **no
verificados**, no se inventan. El equivalente verificado es 5.1.

---

## Bloque 6 — Gestores e índices

### 6.1 AntigravityManager
- Prompt: *"verificar slug real"*. **No verificado**; no se inventa.
  El candidato verificado con esa función es 1.2
  (`eligibility-antigravity-patcher`, "Cross-platform environment manager").

### 6.2 Tópico oficial de herramientas
https://github.com/topics/antigravity-tools
- Índice de la comunidad. Se usa para descubrir proyectos que no estén en
  esta lista; **no es software**, es un catálogo.

---

## Bloque 7 — Complementarios (fuera del ecosistema Antigravity, pero en el plan)

| Proyecto | URL | Para qué | Licencia |
|---|---|---|---|
| **OMCDecoder** | https://github.com/soulr344/OMCDecoder | Decodificar/parchear `cscfeature.xml` (CSC Samsung) | — |
| **UN1CA** | https://github.com/salvogiangri/UN1CA | Firmware custom para Samsung (One UI 8) | GPL-3.0 |
| **nexmon** | https://github.com/seemoo-lab/nexmon | Patch de firmware Broadcom para monitor mode + injection | GPL-3.0 |
| **SOLOW Weaponized Kernel** | https://github.com/o0sayed0o-code/SOLOW-Weaponized-Kernel | Kernel con monitor mode para Z Fold 3 (referencia) | — |
| **PlayIntegrityFork** | https://github.com/osm0sis/PlayIntegrityFork | Fingerprint de Play Integrity (<A13 verdicts) | GPL-3.0 |
| **TrickyStore** | https://github.com/5ec1cff/TrickyStore | Attestation de hardware + keybox | — |
| **Zygisk Assistant** | https://github.com/snake-4/Zygisk-Assistant | Ocultar root | MIT |
| **HMA-OSS** | https://github.com/frknkrc44/HMA-OSS | Ocultar lista de apps | AGPL-3.0 |
| **anti-detect** | https://github.com/Johnny520/anti-detect | Capa extra Zygisk | MIT |
| **ZygiskNext** | https://github.com/Dr-TSNG/ZygiskNext | Zygisk alternativo | — |
| **snapdragon-npu-llm** | https://github.com/avisre/snapdragon-npu-llm | LLM en Hexagon (sólo Qualcomm) | NOASSERTION |

---

## Recomendaciones de seguridad y mantenimiento

1. **Verifica el SHA-256 de cada binario antes de ejecutarlo.** Los parcheadores
   "1-click" de la categoría 1 son la vía más fácil de meter malware.
2. **Prefiere el proxy al parche binario** para lo que cambia rápido
   (modelos BYOK, telemetría). El proxy se puede reconfigurar sin volver a
   compilar; el parche binario se rompe con cada actualización.
3. **Cuidado con las licencias**: `antigravity_bypass` es AGPL-3.0 y
   `HMA-OSS` AGPL-3.0 ⇒Derivados deben ser AGPL. `Antigravity-RTL` (4klgtv)
   no declara licencia.
4. **Tus claves de proveedor no van en el config versionado**: guárdalas en
   un fichero 600 y léalas con `set -a; . fichero; set +a`.
5. **Play Integrity**: los parcheadores de integridad (PlayIntegrityFork,
   TrickyStore) y los de ocultación (Shamiko, Zygisk Assistant, HMA) son
   capas independientes: una sola no basta y se estropean entre sí con el
   tiempo. Documenta la combinación que funciona por app.
6. **Telemetría**: `antigravity-cleaner` y los proxies tocan credenciales de
   Google; revisa qué sale del dispositivo con el proxy activo.
7. **Actualizaciones**: los offsets fijos (parcheadores binarios) son la
   parte frágil. Anota siempre la versión exacta sobre la que parcheaste.
8. **En Android**, el propio IDE es una app con un root proot de 5,8 GB:
   cada copia de repos, `node_modules`, runtime o toolchain duplicada se
   paga en espacio (que es el recurso escaso del dispositivo).
