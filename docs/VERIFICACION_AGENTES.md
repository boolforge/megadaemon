# Verificación de los agentes de MegaDaemon (MEDIDO 2026-11-27)

> Por qué este documento: el encargo pedía **verificar todos los agentes
> existentes más allá de los ya testados**.  Verificar a un agente leyendo su
> transcripción cuesta tokens y no dice nada del código que dejó.  Aquí se
> verifica **lo que hay en el repo**: ¿existe el módulo que el agente
> declara? ¿de cuánto código? ¿tiene tests?
>
> MEDIDO: los números salen de contar líneas y `#[test]` en `rust-core/src/`
> en el commit `04e977f`.

## 1. El roster y su módulo

El roster tiene **22 agentes** (`General`, `Coder`, `Verifier` y 19
`MegaDaemon-*`).  Todos los que declaran un módulo propio lo tienen en `main`:

| agente | módulo | líneas | `pub fn` | tests |
|---|---|---:|---:|---:|
| `MegaDaemon-APK` | `rust-core/src/apk_inventory.rs` | 4383 | 24 | 26 |
| `MegaDaemon-Alerts` | `rust-core/src/alerts.rs` | 413 | 8 | 4 |
| `MegaDaemon-Alerts` | `rust-core/src/task_scheduler.rs` | 992 | 22 | 10 |
| `MegaDaemon-Android` | `rust-core/src/android.rs` | 1497 | 21 | 9 |
| `MegaDaemon-Android` | `rust-core/src/root_stealth.rs` | 251 | 3 | 3 |
| `MegaDaemon-Battery` | `rust-core/src/battery_saver.rs` | 262 | 5 | 4 |
| `MegaDaemon-Battery` | `rust-core/src/power_policy.rs` | 315 | 5 | 2 |
| `MegaDaemon-Bookmarks` | `rust-core/src/bookmarks.rs` | 468 | 8 | 5 |
| `MegaDaemon-Brillo` | `rust-core/src/brightness.rs` | 3544 | 65 | 30 |
| `MegaDaemon-Capability` | `rust-core/src/capability.rs` | 469 | 9 | 5 |
| `MegaDaemon-Capability` | `rust-core/src/unifier.rs` | 171 | 1 | 0 |
| `MegaDaemon-Capability` | `rust-core/src/agent_core.rs` | 611 | 6 | 6 |
| `MegaDaemon-Cockpit` | `rust-core/src/cockpit_server.rs` | 4228 | 20 | 16 |
| `MegaDaemon-Cockpit` | `rust-core/src/mcp_server.rs` | 498 | 4 | 0 |
| `MegaDaemon-Cockpit` | `rust-core/src/mcp_ext.rs` | 4247 | 2 | 4 |
| `MegaDaemon-Git` | `rust-core/src/git_ops.rs` | 184 | 5 | 4 |
| `MegaDaemon-Identity` | `rust-core/src/service_identity.rs` | 595 | 11 | 7 |
| `MegaDaemon-Identity` | `rust-core/src/root_stealth.rs` | 251 | 3 | 3 |
| `MegaDaemon-Messaging` | `rust-core/src/messaging.rs` | 3862 | 49 | 42 |
| `MegaDaemon-NPU` | `rust-core/src/npu_bridge.rs` | 371 | 4 | 0 |
| `MegaDaemon-NPU` | `rust-core/src/npu_cv.rs` | 427 | 4 | 0 |
| `MegaDaemon-NPU` | `rust-core/src/npu_nnapi.rs` | 1447 | 19 | 3 |
| `MegaDaemon-NPU` | `rust-core/src/accelerator_policy.rs` | 271 | 2 | 5 |
| `MegaDaemon-Orchestrator` | `rust-core/src/task_scheduler.rs` | 992 | 22 | 10 |
| `MegaDaemon-Orchestrator` | `rust-core/src/alerts.rs` | 413 | 8 | 4 |
| `MegaDaemon-Orchestrator` | `rust-core/src/sync.rs` | 797 | 12 | 5 |
| `MegaDaemon-Orchestrator` | `rust-core/src/sync_web.rs` | 986 | 6 | 5 |
| `MegaDaemon-Profile` | `rust-core/src/profile_analysis.rs` | 2820 | 10 | 15 |
| `MegaDaemon-Providers` | `rust-core/src/providers.rs` | 2938 | 50 | 5 |
| `MegaDaemon-Providers` | `rust-core/src/provider_catalog.rs` | 316 | 4 | 0 |
| `MegaDaemon-Providers` | `rust-core/src/api_key.rs` | 998 | 24 | 18 |
| `MegaDaemon-RE` | `rust-core/src/binwalk_die.rs` | 758 | 2 | 0 |
| `MegaDaemon-RE` | `rust-core/src/bindiff.rs` | 91 | 1 | 0 |
| `MegaDaemon-RE` | `rust-core/src/deobfuscate.rs` | 191 | 2 | 0 |
| `MegaDaemon-RE` | `rust-core/src/disasm.rs` | 377 | 2 | 0 |
| `MegaDaemon-RE` | `rust-core/src/kaitai_imhex.rs` | 326 | 3 | 0 |
| `MegaDaemon-RE` | `rust-core/src/tile_vram.rs` | 261 | 1 | 0 |
| `MegaDaemon-RE` | `rust-core/src/crypto_scan.rs` | 194 | 1 | 0 |
| `MegaDaemon-RE` | `rust-core/src/entropy.rs` | 102 | 3 | 0 |
| `MegaDaemon-RE` | `rust-core/src/cfg_lifter.rs` | 245 | 1 | 0 |
| `MegaDaemon-Remote` | `rust-core/src/sshd.rs` | 1564 | 18 | 17 |
| `MegaDaemon-Remote` | `rust-core/src/sync.rs` | 797 | 12 | 5 |
| `MegaDaemon-Remote` | `rust-core/src/sync_web.rs` | 986 | 6 | 5 |
| `MegaDaemon-Repo` | `rust-core/src/repo_updates.rs` | 2316 | 15 | 21 |

**Resultado: 0 módulos ausentes.**  Todo agente con módulo asignado entregó
fichero real, en `main`, con contenido sustantivo.

## 2. El hallazgo: la mitad de la superficie NO está testeada

| medida | valor |
|---|---|
| módulos en `rust-core/src/` | **229** |
| módulos con **algún** test | 184 |
| módulos **sin un solo test** | **45** |
| de esos, los que pasan de 150 líneas | **38** |

Los 38 módulos sin test y con más de 150 líneas (MEDIDO, ordenados por tamaño):

`backup_apps.rs` (3322), `main.rs` (2459), `seere_v11.rs` (2431), `cert_vault.rs` (1974), `re_mba.rs` (1194), `automation_runtime.rs` (1160), `device_inventory.rs` (1048), `root_intel.rs` (758), `backup_formats.rs` (677), `re_strenc.rs` (608), `argon2.rs` (576), `brillo_events.rs` (532), `cert_android.rs` (513), `mcp_server.rs` (499), `deobf.rs` (457), `drive_client.rs` (436), `npu_cv.rs` (428), `disasm.rs` (378), `npu_bridge.rs` (372), `swift_sba1.rs` (325), `recording_vault.rs` (319), `provider_catalog.rs` (317), `shm.rs` (291), `re_strings.rs` (285), `frida_dbi.rs` (276), `aegis.rs` (269), `rea_assistant.rs` (268), `tile_vram.rs` (262), `zstd.rs` (253), `cfg_lifter.rs` (246), `lib.rs` (239), `adb_tool.rs` (210), `hw_mapper.rs` (204), `freezer.rs` (197), `crypto_scan.rs` (195), `rom_locator.rs` (194), `deobfuscate.rs` (192), `unifier.rs` (172).

> MEDIDO 2026-11-28 (actualización): **`cirx_rtl.rs` (759), `kaitai_imhex.rs`
> (326) y `binwalk_die.rs` (759) ya tienen tests** (28, 9 y 15
> respectivamente) ⇒ el hueco baja de 48 a **45** módulos sin test (y de 41
> a **38** los que pasan de 150 líneas).  Los tres son casi `SOLO_STD`: sus
> tests se validan **offline**, sin red y sin el crate entero.

### Por qué la familia RE y NPU sale con 0

Los 9 módulos de RE (`binwalk_die`, `bindiff`, `deobfuscate`, `disasm`,
`tile_vram`, `crypto_scan`, `entropy`, `cfg_lifter`) y dos de
NPU (`npu_bridge`, `npu_cv`) tienen **0 tests**.  No es casualidad: son los
módulos que **nunca llegaron a compilar en el CI** (el crate entero no
compilaba desde el merge del snapshot v16), así que su suite nunca pudo
existir.  La causa raíz es la misma que la del CI rojo.

MEDIDO 2026-11-28: `kaitai_imhex` (9 tests), `cirx_rtl` (28) y
`binwalk_die` (15) **ya tienen suite** ⇒ la razón de fondo (el crate no
compilaba) quedó sin efecto para esos tres, y los demás quedan en la cola.

## 3. Qué significa y qué NO significa

* **Significa**: el código está integrado y es real (líneas, funciones
  públicas, lógica), no un esqueleto.
* **NO significa**: que esté *probado*.  Un módulo de 3.000 líneas sin un
  solo test es un módulo cuya corrección no está medida; la verificación de
  esta tabla es de **presencia y volumen**, no de **calidad**.
* **NO se ha medido**: el comportamiento en runtime de esos 45 módulos en el
  móvil.  Eso exige el dispositivo.

## 4. Orden de trabajo que sale de esta verificación

1. Que el CI **compile** (en curso): sin compilar, los tests no existen.
2. `backup_apps.rs` (3.321) y `seere_v11.rs` (2.430): son los que más
   superficie tocan y los que un fallo destruyen datos.
3. `cert_vault.rs` (1.973) y `security`: toca credenciales.
4. La familia RE (9 módulos) y NPU (2): son código de análisis, se testea
   con *fixtures* y no con red.

Cada test que se añada debe ser **sin red y determinista**: el banco de
pruebas del repo ya tiene el patrón (`tools/test-oneliner.sh` para el
instalador, `rust-core/tests/llm_mock/` para el LLM).
