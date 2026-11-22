# GO RE + Antigravity + el CI rojo: lo medido el 2026-11-22

Todo lo de aquí está **medido**, con el comando o la línea de código que lo
sustenta. Lo que no se pudo medir está marcado **NO VERIFICADO**.

---

## 1. El CI estaba rojo por 5 bugs REALES del motor de backup (ya arreglados)

El job `calidad` llevaba varios pushes en rojo. Con el log del job
(`137034337821`, run `44965490063`) se identificaron 9 dianas fallidas; 5 eran
bugs de verdad, no tests mal escritos:

| # | Bug | Síntoma | Dónde |
|---|---|---|---|
| 1 | **El lector de tar no saltaba el relleno a 512** (formato USTAR) | La cabecera siguiente se leía desplazada: en un tar de 3 entradas se leían 5 (una vacía y `nsitive.key` en vez de `KEYS/sensitive.key`) | `rust-core/src/backup_formats.rs` (`TarStream`) |
| 2 | **`read_payload` leía la longitud del trozo pero no la copiaba al marco** | Los 4 primeros bytes del marco quedaban a 0 ⇒ el AAD se calculaba con longitud 0 ⇒ el motor rechazaba **su propio contenedor** con *"la etiqueta no cuadra"* | `rust-core/src/backup_engine.rs` |
| 3 | **`capture` hasheaba cabecera + payload; `verify` hasheaba sólo el payload** | `payload_ok=false` siempre | `HashingWriter` + `reset_hash()` |
| 4 | **`fake_device()` usaba el mismo directorio temporal para todos los tests** | Los tests se borraban entre sí y el de "corrupción" alteraba el contenedor del vecino | `rust-core/src/backup_engine.rs` (tests) |
| 5 | **El patrón de exclusión `/thumbnails/` no cubría `/.thumbnails`** | Android nombra la carpeta de miniaturas del usuario con punto delante | `rust-core/src/backup_inventory.rs` |

**Verificación**: `cargo test --lib` → **1101 passed, 0 failed** (antes
1094/7). Push `abc5c8e`.

Diagnóstico del propio CI (por si vuelve a pasar):
* el log de cargo trae **códigos ANSI** ⇒ `grep '^error'` no casa nunca;
* `cargo test --lib --tests --no-fail-fast` + conteo de binarios **muertos**
  (`error: test failed|signal:|process didn't exit|linking with`);
* el job `calidad` compila en **release SIN LTO** (`CARGO_PROFILE_RELEASE_LTO=false`),
  porque con LTO + `codegen-units=1` el `rustc` muere por OOM (medido:
  `rustc … (signal: 9, SIGKILL: kill)`).

---

## 2. go-retool: el parser de `pclntab` propio, YA FUNCIONA contra Go 1.28

El crate venía **sin compilar** (el paquete que lo generó no tenía `rustc`).
Al compilarlo salieron **2 bugs de formato** y faltaba lo esencial: la base
de texto.

### 2.1 Bugs de formato corregidos

| Bug | Detalle |
|---|---|
| `//go:embed` | El registro es `{nameData[2]uintptr, dataData[2]uintptr, hash [sha256.Size]byte}` ⇒ **64 B**, no 48. El código asumía un hash de 16 B ⇒ truncaba el SHA-256. |
| `stackmap` | El test esperaba que el bit 8 fuese 1 con un byte `0b0000_0010`, pero `runtime.bitvector.ptrbit` ordena **LSB-first** ⇒ ese byte pone a 1 el bit 9. **El test estaba mal; el decoder era correcto.** |

### 2.2 La base de texto (lo que faltaba de verdad)

* `pcHeader.textStart` vale **0** en una **biblioteca compartida**: el linker
  no puede conocer la VA final del texto.
* En el binario real la base es **`0x4d06cb8`** = la sección **`.text.hot`
  (de TAMAÑO 0)**, **NO** la de `.text` (`0x4d0f000`). Usar `.text` desplaza
  todas las VAs ~`0x8330` B y los nombres **no cuadran** con las direcciones.
* Implementado: `container::text_base_hint()` = **mínima** dirección entre las
  secciones `.text*` (incluidas las de tamaño 0); el CLI la usa cuando el
  llamante no pasa `--text-base`, y el JSON lo dice explícitamente
  (`estimated-from-container-sections (runtime.text NOT verified)`).

### 2.3 Medición contra el binario real

`liblanguage_server.so` (181.358.800 B, ELF64 AArch64 PIE **stripped**, Go
**1.28-20260921-RC02**, layout de pclntab **1.20**, `minLC=4`, pclntab **en
`.lrodata`** @ `0x1449ff0`, **sin** `.gopclntab`):

| Métrica | Valor |
|---|---|
| Candidatos pclntab | 1 (`0x1449ff0`, magic `0xfffffff1`, `min_lc=4`) |
| Funciones | **161.682** |
| Nombres resueltos | **161.682 / 161.682** (100 %) |
| Base de texto | `0x4d06cb8` |

**Validación cruzada** (dos implementaciones independientes):
* contra la tabla del agente de RE (`go.funcs.tsv`, 161.682 entradas):
  **161.682/161.682 direcciones idénticas**;
* contra `unstrip` (Rust, riven-labs): mismo text base; el oráculo pide el PC
  y devuelve la función correcta.

**Herramientas estándar que NO soportan este binario** (por eso hace falta el
parser propio):
* `redress moduledata` ⇒ *"Could not retrieve the file's moduledata: no goversion found"*;
* `GoReSym v3.4.1` ⇒ rc=0 con salida vacía.

---

## 3. Antigravity: la API real (medida en el binario Go)

### 3.1 Endpoints

* `https://127.0.0.1:45157/` ⇒ SPA (`window.__APP_CONFIG__` con
  `csrfToken`); `/mcp` y `/tools` son el **catch-all** de la SPA (200 pero no
  son un MCP).
* El servidor real es **gRPC**: `POST
  /exa.language_server_pb.LanguageServerService/<Método>` ⇒ **401** sin
  autenticación de Google (el CSRF **no** sirve: probados `Authorization`,
  `X-CSRF-Token`, `Cookie`, `X-Goog-Api-Key` ⇒ 401 en todos).

### 3.2 Métodos RPC descubiertos en el binario

```
/exa.language_server_pb.LanguageServerService/GetAvailableModels
/exa.language_server_pb.LanguageServerService/GetCascadeModelConfigData
/exa.language_server_pb.LanguageServerService/GetAllCascadeTrajectories
/exa.language_server_pb.LanguageServerService/GetCascadeTrajectorySteps
/exa.language_server_pb.LanguageServerService/GetCascadeNuxes
/exa.language_server_pb.LanguageServerService/GetCapabilities
/exa.language_server_pb.LanguageServerService/GetUserSettings
/exa.language_server_pb.LanguageServerService/SetUserSettings
/exa.language_server_pb.LanguageServerService/RegisterGdmUser
/exa.language_server_pb.LanguageServerService/StreamCascadePanelReactiveUpdates
/exa.extension_server_pb.ExtensionServerService/{CreateTerminal,ReadTerminal,ShowTerminal,OpenTerminal,EditNotebook,ReadNotebook,SaveDocument,BroadcastConversationDeletion}
/exa.api_server_pb.ApiServerService/ApplyTrajectoryHeuristics
```

### 3.3 El catálogo de modelos: 4 tiers (cadenas del binario)

```
[ModelConfig] Resolving models from Tier 1 (Inline admin_settings models.list)
[ModelConfig] Tier 1 models.list parse failed: %v. Falling back to next tier.
[ModelConfig] Tier 2 remote catalog fetch failed: %v. Falling back to Tier 3.
[ModelConfig] Resolving models from Tier 4 (Built-in default catalog fallback)
models.list[%d] has an empty modelId
```

* **Tier 1**: `admin_settings.json` — rutas que busca el binario:
  `/etc/antigravity/admin_settings.json` (Linux),
  `Antigravity\admin_settings.json` (Windows),
  `/Library/Application Support/Antigravity/admin_settings.json` (macOS).
  En Android ⇒ dentro del sandbox de la app.
* El `admin_settings.json` **embebido** en el binario lleva la tabla de
  placeholders: `MODEL_PLACEHOLDER_M50`, `M124`, `M319`, `M526`… (Google
  ofusca los ids de modelo en esta build).
* Tipo Go: `gateway.AdminSettings`, `gateway.GatewayConfig`,
  `ModelID json:"modelId"`.
* El binario **NO contiene la cadena "minimax"** ⇒ MiniMax sólo puede entrar
  por el BYOK del sandbox.

### 3.4 El BYOK ya está en el móvil (MEDIDO)

`/data/user/0/com.agy/files/.gemini/antigravity/custom_models.json` (0600,
289 B, 2026-11-19 10:27):

```json
{ "models": [ { "provider": "minimax", "modelId": "MiniMax-M3.1-Flash",
                "displayName": "MiniMax M3.1 Flash", "apiKey": "<secreto>" } ] }
```

* El módulo `rust-core/src/antigravity_intel.rs` del repo ya sabe mapear
  `("minimax","MiniMax-M3.1-Flash") → "minimax-m3.1-flash"` (el id del
  proveedor en `providers.rs`).
* `antigravity_state.pbtxt` guarda `last_selected_agent_model:
  MODEL_PLACEHOLDER_M319` y luego `M526` ⇒ el modelo **elegido** sigue siendo
  un placeholder de Google; el BYOK de MiniMax es aparte.
* El plugin `mcp/megadaemon-minimax` de Antigravity declara **52 tools** de
  MegaDaemon ⇒ el puente Antigravity ⇄ MegaDaemon **ya existe** y es
  bidireccional.

### 3.5 NO VERIFICADO

* Que MiniMax aparezca en el **selector** de la UI: la API que lo responde es
  gRPC y exige la autenticación de Google del usuario (401 sin ella). Falta
  consultarla con la sesión autenticada.
* El formato exacto del `admin_settings.json` de Android (sólo se conoce el
  de Linux/macOS/Windows del binario).
* Dónde acaba el log del language server (para leer los mensajes
  `[ModelConfig] Tier N …`).

---

## 4. El "congelamiento" del móvil NO era del backup

Medido en el S21 Ultra (load 24-27, 844 procesos, 12 ejecutables, 15 en I/O,
0 zombies):

| Proceso | CPU instantánea (jiffies/5 s) |
|---|---|
| `com.android.chrome:privileged_process0` | **6.752 ⇒ 2,7 núcleos** ← el culpable |
| `surfaceflinger` | 4.831 |
| `app.morphe.android.youtube` (YouTube mod) | 3.672 |
| `samsung.hardware…` | 1.531 |
| F-Droid | 1.001 |

Matando el proceso runaway de Chrome (que Chrome respawna) el móvil se
descongestiona. El resto (Antigravity 309-673 MB RSS, la app de YouTube,
Chrome) es carga legítima del usuario.
**NO VERIFICADO**: el efecto medido a largo plazo (no se hizo un perfil de
10 minutos).

> Nota: `/dev/tcp` **no existe** en el `sh` de Android ⇒ probar puertos con
> `(echo > /dev/tcp/127.0.0.1/$p)` da **falsos negativos**. Usar `curl`.
