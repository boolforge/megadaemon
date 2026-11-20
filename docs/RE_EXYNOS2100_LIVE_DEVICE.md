# RE Exynos 2100 — artefactos reales del S21 Ultra (P2 TEE · P3 boot chain · P4 NPU)

**Todo lo de esta página está medido sobre el dispositivo vivo**, no sobre una copia ni
sobre un volcado de ROM: `root_exec` con lectura directa de los ficheros y de los
dispositivos de bloque, sha256 por streaming (1 MiB de trozo) para no cargar la RAM.

- **Dispositivo**: SM-G998B (p3s), Exynos 2100, Android 14 (SDK 34), patch 2024-06-01, CSC AMO
- **Fecha**: 2026-11-20T00:31Z
- **Método**: sólo lectura. Ni builds, ni polling agresivo, ni escrituras grandes
- **Credenciales**: ninguna en esta página (sólo rutas y hashes)

## P2 — TEE / keystore / keymaster (14 binarios, sha256 real)

| Ruta | sha256 | Bytes |
|---|---|---|
| `/system/bin/keystore2` | `3012205acc44a8c7d077fdeea398488f2c783802fd6679763b9120e8c8f6eac0` | 2.134.920 |
| `/system/bin/gatekeeperd` | `1ebf380914d2898c4ab4b5ab98f0a70c831d045644f4e06ed77c8a6d03a3f239` | 92.360 |
| `/system/bin/keystore_cli_v2` | `b20fa395f2a6129a02fc6d5a74d355f9743c088a44af22d9d684deb122d9bdda` | 125.672 |
| `/vendor/lib64/hw/gatekeeper.exynos2100.so` | `efa211760ed0a23ecdfe55154bcf86064a629c8277737d218d0d55cc123723e4` | 11.696 |
| `/vendor/bin/vendor.samsung.hardware.security.fkeymaster-service` | `979f4864b1ddfd3d22aafb0b36d867690b2bda000e93eda66ad909ce9128629a` | 51.208 |
| `/vendor/lib64/libkeymaster4.so` | `f73b58435a155c1921df83dcb2fab58edd9774ed3d9a120b727eff3b7ac4b68f` | 87.672 |
| `/vendor/lib64/libkeymaster4_1support.so` | `021953efb00665f2d923614f664b6b46a24e05a94aa806d95d92d543e21c5063` | 121.192 |
| `/vendor/lib64/libkeymaster4support.so` | `a01e53592d37ec8d86f44c666463cab4105a0cf345bad3294989aaaef4445760` | 99.176 |
| `/vendor/lib64/libkeymaster_helper.so` | `79579ee60450c4990de983757933728e028bf358b14a2584ed349fb5f9f55257` | 199.680 |
| `/vendor/lib64/libkeymaster_messages.so` | `29522cfce010f8edaded6e312affbd720502c49ce828721a45d34651ab1bae79` | 98.208 |
| `/vendor/lib64/libkeymaster_portable.so` | `a248a82f5bbce40be2211daed95c0f9214b3fe8100fe131a0185677c0ab072dd` | 342.056 |
| `/vendor/lib64/libskeymaster4device.so` | `ee8fef80fdca04953a6c7fa38af23af1804f14af51b2c86422a7578151a62792` | 53.176 |
| `/vendor/lib64/libpuresoftkeymasterdevice.so` | `dc753e8b4b5d77ac2d73131d117964c554e20e9c3698bd597877ef4a9a0364a2` | 47.096 |
| `/vendor/lib64/libteecl.so` | `4bac41ddcf454d278aa4bd5c8c942a136f80cad5fd9030f82ff4e2b5eadf9aa1` | 55.592 |
| `/vendor/lib64/vendor.samsung.hardware.security.fkeymaster-V1-ndk_platform.so` | `97f592938b60614acd70fdb5978fd142a5e5cf2f3bd98e07c049a2092bc7f42e` | 55.808 |

**Puntos que corrigen suposiciones anteriores:**

- El binario **exynos2100-specific** es `hw/gatekeeper.exynos2100.so` (11,7 KB) — no un
  `gatekeeper` genérico. Es el punto de entrada real de la TEE en este SoC.
- La TEE de Samsung es **`fkeymaster-service`** (NO `hw_keymaster`: en este firmware no
  existe `/vendor/bin/hw_keymaster`; el nombre del servicio es
  `vendor.samsung.hardware.security.fkeymaster-service`, con su cliente NDK
  `fkeymaster-V1-ndk_platform.so`).
- `keystore2` (2,1 MB) vive en **`/system/bin`**, no en `/vendor/bin`: el motor de claves
  es de sistema, el *daemon* de gatekeeper también.
- `libteecl.so` es el cliente de la TEE (55 KB) — el punto de entrada para hablar con el
  mundo seguro desde userspace.

## P3 — boot chain (sha256 de los dispositivos de bloque, leídos enteros)

| Partición | sha256 | Bytes |
|---|---|---|
| `/dev/block/by-name/boot` | `f96cd410e6f048739fe36737f047e2ede6d229d428e1d0c74de80017c8beb4fd` | 67.108.864 |
| `/dev/block/by-name/vendor_boot` | `b55ecfd0744ae8ae5404635b3345b846764e55eca8349a655e71aebd52d0b2fb` | 67.108.864 |
| `/dev/block/by-name/dtbo` | `c2cc0f21784007dde7ae154887b2fc3c69085c73b185ba847ae2e9b29b3ee106` | 8.388.608 |
| `/dev/block/by-name/vbmeta` | `b1ef8b5108b1a333c13e6db5cf1e2c1446a4e017e81d0fe9b9392de70a1238c9` | 65.536 |
| `/dev/block/by-name/vbmeta_system` | `254732e930687a6d5a3c0c82965f84c9a4a36767f6aa39eeacabf53af6aba5c6` | 65.536 |
| `/dev/block/by-name/recovery` | `777350644d9f5c345970fef5f92ea5efdb1fbc9d2b0005b4a8b62dcb90703f19` | 67.108.864 |

**Ausentes (con el nombre exacto):** `init_boot`, `vbmeta_vendor`, `sec`.
⇒ **Este firmware NO usa `init_boot` ni `vbmeta_vendor`** ⇒ la cadena AVB es
`vbmeta` + `vbmeta_system` (2 niveles), no 3. Es un dato de estructura del chain, no
una suposición.

**Particiones presentes que SÍ son del mundo seguro / claves:**
`sec_efs`, `keystorage`, `keyrefuge`, `uh`, `uhcfg`, `spu`, `prism`, `harx`, `fmp_selftest`,
`param`, `metadata`, `persistent`, `dqmdbg`, `nad_refer`, `omr`, `reserved`, `bota`, `blcmd`.
⇒ `keystorage` + `keyrefuge` son las candidatas reales al almacén de claves por
dispositivo (Keymaster/FBE); `uh`/`uhcfg` es el *user header* de Samsung.

**Estado AVB / bootloader (props):**

| Prop | Valor |
|---|---|
| `ro.boot.verifiedbootstate` | `green` |
| `ro.boot.flash.locked` | `1` (bootloader **cerrado**) |
| `ro.bootloader` / `ro.boot.bootloader` | `G998BXXSBGXED` |
| `ro.boot.hardware` | `exynos2100` |
| `ro.boot.dynamic_partitions` | `true` (super) |
| `ro.boot.slot_suffix` | vacío (slot A) |
| `ro.boot.veritymode` | vacío |

⇒ **Bootloader cerrado + AVB green con Magisk root ⇒ el bootloader NO está desbloqueado**:
Magisk parchea la imagen de arranque y firma con la clave original, de modo que la
verificación sigue en `green`. Esto **descarta** el escenario "bootloader abierto con
AVB verde" que daba por hecho el análisis previo.

## P4 — NPU EDEN (estado real del nodo)

| Ruta | Estado |
|---|---|
| `/dev/vertex10` | **modo 0644**, uid 1000, gid 1000 (confirma el 0644 del volcado) |
| `/dev/vertex10_test` | **NO existe** |
| `/sys/class/misc/vertex10` | **NO existe** (el driver no se registra en misc) |

| Binario | sha256 | Bytes |
|---|---|---|
| `/vendor/lib64/libeden_ud_npu.so` | `7c0041ab601b9d8d2d5e25254e8fb61a81f0aff7a600cc1a8df0fdb210e59482` | 436.920 |
| `/vendor/lib64/libnpuc_common.so` | `d50035e3b8d8237bc32bbafb6fd097d9c0b0f30bc5753e0ea5018e619b6b9674` | 952.608 |
| `/vendor/etc/init/vendor.samsung_slsi.hardware.eden_runtime@1.0-service.rc` | `b1f111905a6857c5bd79c45ef38ddc18d9ba3c67f8b1462e32fe073e9ae40c92` | 317 |

**Corrección de rutas**: `NPU.bin` **no está** en `/vendor/bin` ni `eden_kernel_64.bin` en
`/vendor/etc` ⇒ esas rutas del volcado de ROM **no corresponden a este firmware en vivo**.
Lo que sí hay: `libeden_ud_{npu,cpu,dsp,gpu}.so`, `libeden_{nn,rt,model,osal,profiler,gb_on_vendor,nn_on_vendor}.so`,
`libeden_wrapper{,_sdk}.so`, `libeden_xtool.so`, `libnpu_{compiler,api,backend,cmdq,controller,frontend,graph,template}.so`,
`libsait_npu_compiler.so`, y un directorio `eden/`.

⇒ **Consecuencia para el trabajo P1**: los hashes del volcado de ROM y los del firmware en
vivo **no coinciden** en nomenclatura ⇒ cualquier tabla de ioctls sacada del volcado debe
validarse contra `libeden_ud_npu.so` **del dispositivo** (sha `7c0041ab…`), que es el
objeto que realmente carga el driver `vertex10`.

## Estado de la fase

- P1 NPU/EDEN: 11 hallazgos · P2 TEE: 9 → **+15 con hash real** · P3 boot chain: 4 → **+6 con hash real** · P4 vendor: en curso
- Broadcast a los 4 canales: `mesh_journal.ndjson`, `sync/re_cloud-sandbox.ndjson`, `mesh_board.json` (bajo flock) y este documento
- `master_url` del tablón corregido a la URL vigente (la anterior estaba **caducada**)
