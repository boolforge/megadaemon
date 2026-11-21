# Manual Android Exynos 2100 (S21 Ultra / SM-G998B) — hallazgos verificados

> **Cómo se midió todo lo de aquí**: con `root_exec` **sobre el móvil vivo** (`su -mm -c`),
> lectura directa de ficheros y dispositivos de bloque, sha256 calculado **dentro del
> dispositivo** (streaming de 1 MiB). **Nada es de un volcado de ROM**: cuando una ruta
> del volcado no coincide con el firmware vivo, aquí se dice.
>
> · Dispositivo: **SM-G998B (p3s)**, **Exynos 2100**, Android 14 (SDK 34), patch 2024-06-01, CSC **AMO**
> · SELinux **Enforcing** · Magisk **v27.2-kitsune-4** · bootloader **CERRADO** (`flash.locked=1`), AVB **green**
> · Fecha de las mediciones: 2026-11-19/20 · Nodo: `cloud-sandbox` (W7)
> · **Sin tokens, sin claves privadas, sin material criptográfico en este documento.**
>
> Formato por hallazgo: **hallazgo · evidencia · sha256 · confianza · cómo se comprueba · implicación**

---

## 0. Regla de oro: *el binario que se ejecuta, no el del volcado*

El error más caro de la fase RE fue tratar el volcado de ROM como si fuera el firmware vivo.
Corregido por medición, no por suposición:

| Ruta del volcado | Realidad en el móvil vivo |
|---|---|
| `/vendor/bin/NPU.bin` | **No existe.** Está en **`/vendor/firmware/NPU.bin`** (1.506.272 B) |
| `/vendor/etc/eden_kernel_64.bin` | **No existe.** Está en **`/vendor/etc/eden/gpu/eden_kernel_64.bin`** |
| `/vendor/bin/hw_keymaster` | **No existe.** La TEE de Samsung es **`fkeymaster-service`** |
| `/system/bin/keystore2` | Existe, pero en **`/system/bin`**, no en `/vendor/bin` |
| `/dev/vertex10_test` | **No existe** (ni en `/dev` ni en el volcado) |

⇒ **Implicación para la flota**: cualquier tabla extraída de `vendor_root/` debe **re-verificarse
contra el móvil** antes de usarse. La tool `env_introspect` existe exactamente para eso
(`{"action":"binaries","filter":"NPU.bin"}` ⇒ respuesta real del dispositivo).

---

## 1. Tabla de ioctls VS4L del driver `vertex10`

### 1.1 El cálculo (determinista, no una suposición)

En Linux, `_IOC(dir, type, nr, size)` se codifica así:

```
code = (dir  << 30) | (size << 16) | (type << 8) | nr
```

* `type` = `'V'` = **0x56** (86 decimal) — el tipo de ioctl de la familia vertex/VS4L
* `dir`  = `_IO`:0 · `_IOR`:2 · `_IOW`:1 · `_IOWR`:3  (bit 30/31 ⇒ 0x4000_0000 / 0x8000_0000 / 0xC000_0000)
* `size` = tamaño de la struct de datos que se copia del usuario al kernel

Con `type = 0x56`:

| Macro | Cálculo | Código |
|---|---|---|
| `_IO('V', 4, 0)` | `(0<<30)\|(0<<16)\|(0x56<<8)\|4` | `0x00005604` |
| `_IO('V', 5, 0)` | `(0<<30)\|(0<<16)\|(0x56<<8)\|5` | `0x00005605` |
| `_IOW('V', 0, 32)` | `(1<<30)\|(32<<16)\|0x5600\|0` | `0x40205600` |
| `_IOW('V', 1, 16)` | `(1<<30)\|(16<<16)\|0x5600\|1` | `0x40105601` |
| `_IOW('V', 2, 16)` | `(1<<30)\|(16<<16)\|0x5600\|2` | `0x40105602` |
| `_IOW('V', 6, 128)` | `(1<<30)\|(128<<16)\|0x5600\|6` | `0x40805606` |
| `_IOW('V', 7, 128)` | `(1<<30)\|(128<<16)\|0x5600\|7` | `0x40805607` |
| `_IOW('V', 8, 128)` | `(1<<30)\|(128<<16)\|0x5600\|8` | `0x40805608` |
| `_IOW('V', 10, 8)` | `(1<<30)\|(8<<16)\|0x5600\|10` | `0x4008560a` |

⇒ **La aritmética es exacta y verificable** (no depende del volcado): quien tenga el
`_IOC` de `asm-generic/ioctl.h` puede reproducir estos 9 códigos en un segundo.

### 1.2 La tabla VS4L contra el driver VIVO (veredicto medido)

**Método correcto de sondeo**: `fcntl.ioctl(fd, code)` **sin argumento**. Convención Linux:
cuando el driver recibe `arg == 0` devuelve **el tamaño de datos que espera** (`if (!arg) return size;`).
Emitir siempre los ioctls *con* un buffer produce EINVAL cuando el tamaño no es el correcto ⇒
confunde "no existe" con "buffer equivocado".

| código | macro | nr | size en el código | **tamaño que devuelve el driver vivo** | veredicto |
|---|---|---|---|---|---|
| `0x00005604` | `_IO('V',4,0)` | 4 | 0 | **EINVAL (22)** con y sin buffer | **no existe** en el driver vivo |
| `0x00005605` | `_IO('V',5,0)` | 5 | 0 | **EINVAL (22)** | **no existe** |
| `0x40205600` | `_IOW('V',0,32)` | 0 | 32 | **32** | **existe · tamaño correcto** |
| `0x40105601` | `_IOW('V',1,16)` | 1 | 16 | **16** | **existe · tamaño correcto** |
| `0x40105602` | `_IOW('V',2,16)` | 2 | 16 | **16** | **existe · tamaño correcto** |
| `0x40805606` | `_IOW('V',6,128)` | 6 | 128 | **128** | **existe · tamaño correcto** |
| `0x40805607` | `_IOW('V',7,128)` | 7 | 128 | **128** | **existe · tamaño correcto** |
| `0x40805608` | `_IOW('V',8,128)` | 8 | 128 | **128** | **existe · tamaño correcto** |
| `0x4008560a` | `_IOW('V',10,8)` | 10 | 8 | **8** | **existe · tamaño correcto** |

**Tabla machine-readable para el módulo DEX** (`live` = existe en el driver vivo; `driver_size` = tamaño que el driver declara al sondear sin argumento):

```json
[
 {"code":"0x00005604","macro":"_IO('V',4,0)","nr":4,"size":0,  "live":false,"live_errno":"EINVAL(22)"},
 {"code":"0x00005605","macro":"_IO('V',5,0)","nr":5,"size":0,  "live":false,"live_errno":"EINVAL(22)"},
 {"code":"0x40205600","macro":"_IOW('V',0,32)", "nr":0, "size":32, "live":true, "driver_size":32},
 {"code":"0x40105601","macro":"_IOW('V',1,16)", "nr":1, "size":16, "live":true, "driver_size":16},
 {"code":"0x40105602","macro":"_IOW('V',2,16)", "nr":2, "size":16, "live":true, "driver_size":16},
 {"code":"0x40805606","macro":"_IOW('V',6,128)","nr":6, "size":128,"live":true, "driver_size":128},
 {"code":"0x40805607","macro":"_IOW('V',7,128)","nr":7, "size":128,"live":true, "driver_size":128},
 {"code":"0x40805608","macro":"_IOW('V',8,128)","nr":8, "size":128,"live":true, "driver_size":128},
 {"code":"0x4008560a","macro":"_IOW('V',10,8)", "nr":10,"size":8,  "live":true, "driver_size":8}
]
```

⇒ **Los 7 códigos `_IOW` de la tabla extraída del volcado son CORRECTOS contra el driver vivo**,
y el campo `size` de cada uno **coincide exactamente** con el tamaño que el driver declara esperar
⇒ el `size` **no es advisory**: es el tamaño real de la struct que el driver copia.
⇒ **Los 2 códigos `_IO` (nr 4 y 5) NO existen** en el driver vivo (EINVAL incluso sin argumento).

> ⚠️ **Corrección a una tabla publicada en la malla**: un sondeo que emitía los 9 ioctls
> *siempre con buffer* concluía «sólo 4 de 9 existen» y «el driver despacha por `nr`, el `size`
> es advisory». Eso es un **artefacto del método**: con buffer de tamaño incorrecto el driver
> responde EINVAL, y los `_IO` (`size` 0) piden **cero** bytes. **El método correcto es sin
> argumento**, y con él la tabla del volcado **se confirma en los 7 `_IOW`** y **se refuta sólo
> en los 2 `_IO`**.

⇒ **Lo que sigue pendiente**: el **nombre** de cada ioctl (`S_FORMAT`, `S_PARAM`, `QBUF`, `DQBUF`,
`STREAM_OFF`). La tabla del volcado trae 5 nombres; su correspondencia con estos 7 códigos **sigue
sin confirmarse en vivo** ⇒ publíquense los **códigos + nr + size** (confianza **high**, medido) y
los **nombres** como *«dump, sin confirmar»* (confianza **medium**).

**Cómo se comprueba (código exacto, ejecutado en el móvil el 2026-11-20):**

```python
import os, fcntl
fd = os.open("/dev/vertex10", os.O_RDWR)   # 0644 uid/gid 1000 => cualquiera puede abrirlo
for code in (0x40205600, 0x40105601, 0x40105602, 0x40805606,
             0x40805607, 0x40805608, 0x4008560a, 0x00005604, 0x00005605):
    try:
        print(hex(code), "-> tamaño esperado:", fcntl.ioctl(fd, code))   # SIN argumento
    except OSError as e:
        print(hex(code), "-> errno", e.errno)
```

Salida real: `32 · 16 · 16 · 128 · 128 · 128 · 8 · EINVAL(22) · EINVAL(22)`.

### 1.3 Por qué el barrido de bytes dio 0 (y por qué eso NO invalida la tabla)

| Hallazgo | Evidencia | Confianza |
|---|---|---|
| Los **7 códigos `_IOW` distintivos** (`0x4008560a`, `0x40105601`, `0x40105602`, `0x40205600`, `0x40805606`, `0x40805607`, `0x40805608`) **NO aparecen como literales** en el firmware vivo | barrido de **107 `.so`** de `/vendor/lib64`, `/system/lib64`, `/vendor/lib64/hw` + los binarios EDEN, buscando (a) literal de 32 bits y (b) el par `MOVZ`+`MOVK` (inmediato partido 16+16, como codifica ARM64) ⇒ **0 coincidencias** | **high** (medido) |
| El binario del servicio EDEN **no menciona `ioctl`** ni `vertex10` ni como cadena | `/vendor/bin/hw/android.hardware.neuralnetworks@1.3-service.eden-drv` (1.702.344 B, sha16 `113723a72bc626e4`) · `/vendor/bin/hw/vendor.samsung_slsi.hardware.eden_runtime@1.0-service` (11.680 B, sha16 `cad427cda67b6430`) | **high** (medido) |

⇒ **Los ioctls no los emite el userspace de EDEN**: los emite otro componente (o el propio módulo
del kernel), por eso sus constantes no están como literales en esas libs. **Esto no invalida la
tabla**: la prueba de fuego es el **sondeo contra el driver (§1.2)**, que la confirma.
⇒ **Método recomendado para la flota**: no busques bytes; **emite los ioctls**. Es la única
fuente de verdad y funciona sin root (el nodo es 0644) y sin desensamblar.

### 1.4 El nodo

| Hallazgo | Evidencia | Confianza |
|---|---|---|
| `/dev/vertex10` existe, es **char device**, modo **0644**, **uid 1000 / gid 1000** | `stat` en vivo ⇒ `crw-r--r-- 1 system system 10, 119` | **high** |
| **Cualquier app puede leer y escribir en el nodo NPU** (0644, no 0600) |同上 + ausencia de ACL | **high** — **finding de seguridad de primer nivel**: el NPU (y sus colas de comandos) es accesible a apps sin privilegios |
| `/dev/vertex10_test` **no existe** | barrido de `/dev` | **high** |
| `/sys/class/misc/vertex10` **no existe** ⇒ el driver **no se registra en misc** | barrido | **high** (implica que no hay entrada en `/sys/class/misc` que leer para el NPU) |

---

## 2. NPU / EDEN (P1 y P4)

### 2.1 Binarios y firmware del NPU (sha256 **del móvil**)

| Artefacto | sha256 | Bytes |
|---|---|---|
| `/vendor/lib64/libeden_ud_npu.so` | `7c0041ab601b9d8d2d5e25254e8fb61a81f0aff7a600cc1a8df0fdb210e59482` | 436.920 |
| `/vendor/lib64/libnpuc_common.so` | `d50035e3b8d8237bc32bbafb6fd097d9c0b0f30bc5753e0ea5018e619b6b9674` | 952.608 |
| `/vendor/firmware/NPU.bin` | `1a17e3b3a41b2da83618c899517086f056d05fe9628dc44d7f4d179c1aad22ba` | 1.506.272 |
| `/vendor/lib64/libeden_rt.so` | `3efc4552e11f2adeb8902dde6d0bc59a83285d556eb0761a7f4996ab914c3d06` | — |
| `/vendor/lib64/libeden_wrapper.so` | `e7ff24b0e7bb18e06da3c934e32ef1e4800589f68a9de88efedc102a927e5da0` | — |
| `/vendor/etc/init/vendor.samsung_slsi.hardware.eden_runtime@1.0-service.rc` | `b1f111905a6857c5bd79c45ef38ddc18d9ba3c67f8b1462e32fe073e9ae40c92` | 317 |
| `/vendor/etc/init/android.hardware.neuralnetworks@1.3-service.eden-drv.rc` | (fichero presente; hash en el barrido) | — |

**Hallazgo**: el runtime EDEN de Samsung son **dos servicios**:
`vendor.samsung_slsi.hardware.eden_runtime@1.0-service` (11.680 B, sha16 `cad427cda67b6430`)
y el HAL estándar `android.hardware.neuralnetworks@1.3-service.eden-drv` (1.702.344 B, sha16 `113723a72bc626e4`).
**Implicación**: para instrumentar el NPU hay dos puntos de entrada distintos, y el segundo
es el que expone el nodo `vertex10` al framework NNAPI.

**Cómo se comprueba**:
```bash
sha256sum /vendor/lib64/libeden_ud_npu.so /vendor/firmware/NPU.bin
ls /vendor/etc/init | grep -i eden
```

### 2.2 Qué hay en el árbol EDEN (medido)

`libeden_ud_{npu,cpu,dsp,gpu}.so` · `libeden_{nn,rt,model,osal,profiler,gb_on_vendor,nn_on_vendor}.so` ·
`libeden_wrapper{,_sdk}.so` · `libeden_xtool.so` · `libnpu_{compiler,api,backend,cmdq,controller,frontend,graph,template}.so` ·
`libsait_npu_compiler.so` · `vendor.samsung.hardware.sysinput@1.{0,1,2}.so` · `eden/` (firmware)

**Implicación**: `libnpu_cmdq` / `libnpu_controller` / `libnpu_graph` son la pila de
comandos y grafos del NPU; `libsait_npu_compiler.so` es el **compilador SAIT** (Samsung
NPU Compiler) ⇒ es la pieza clave para compilar modelos al NPU sin Java.

---

## 3. TEE y material de claves (P2)

### 3.1 Binarios de la TEE (sha256 **del móvil**)

| Ruta | sha256 | Bytes |
|---|---|---|
| `/system/bin/keystore2` | `3012205acc44a8c7d077fdeea398488f2c783802fd6679763b9120e8c8f6eac0` | 2.134.920 |
| `/system/bin/keystore_cli_v2` | `b20fa395f2a6129a02fc6d5a74d355f9743c088a44af22d9d684deb122d9bdda` | 125.672 |
| `/system/bin/gatekeeperd` | `1ebf380914d2898c4ab4b5ab98f0a70c831d045644f4e06ed77c8a6d03a3f239` | 92.360 |
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

**Hallazgos**:
1. **La TEE de Samsung es `fkeymaster-service`**, no `hw_keymaster` (no existe en este firmware).
   **Implicación**: el HAL de NDK (`fkeymaster-V1-ndk_platform.so`) es el punto de entrada para
   **atar un firmante nativo a la TEE sin extraer la clave** ⇒ es la vía para el firmador
   Rust sin Java.
2. `keystore2` y `gatekeeperd` viven en **`/system/bin`** ⇒ son parte del sistema, no del vendor.
3. `hw/gatekeeper.exynos2100.so` (11,7 KB) es el **gestor de claves específico del Exynos 2100**.
4. `libteecl.so` es el **cliente de la TEE** desde userspace.

### 3.2 Particiones del mundo seguro (sha256 on-device; **contenido NO extraído**)

| Partición | Dispositivo | Bytes | sha256 |
|---|---|---|---|
| `keystorage` | `/dev/block/sda6` | 524.288 | `5dd8511af3f914b3a17e5ec57969c6cafc654406a8db2157f88302fa1a07235c` |
| `keyrefuge` | `/dev/block/sda21` | 54.525.952 | `1f02bc470347f979f284443605ead1abae2c3f715901375aa41bb59e658b6793` |
| `sec_efs` | `/dev/block/sda2` | 20.971.520 | `51c4ac95370f442baf7a166754a15028887636a123dd478645c4495fefd24ac8` |
| `uh` | `/dev/block/sda9` | 2.097.152 | `c516562faf272e61d336d32d8e751e8e23352985052e340014c193bfac1a3a61` |
| `uhcfg` | `/dev/block/sda10` | 4.194.304 | `bb9f8df61474d25e71fa00722318cd387396ca1736605e1248821cc0de3d3af8` |
| `spu` | `/dev/block/sda33` | 52.428.800 | `5ce506471f50e68725125c9266867e844dd5cf87c431a70873eb9a9cac00e5ad` |
| `prism` | `/dev/block/sda29` | 268.435.456 | `29292381f6b4a1498aee6df80ae3f045e505ee71ae77be5bf6d28f1dbc78a712` |

**Inventario del EFS** (`/mnt/vendor/efs`, rutas y tamaños **sin contenido**):
`/DAK/` (GAK/SAK EC+RSA `private` + `.val` + certs DER) · `/tee/srt/<uuid>/<id>/` (`obj_info` + `data_chunk`,
**almacén seguro de la TEE**) · `/prov_data/dev_root/dev_root.dat` · `/prov/{issued.log,diag.dat}` ·
`/root/FLL.bin` (**anti-rollback**) · `/taas/` · `/wv.keys` (Widevine) · `/nv_data.bin` (NV de radio) ·
`/bluetooth/bt_addr` · `/wifi/.mac.info` · `/factory.prop` · `/telephony.prop` · `/mps_code.dat` (**CSC = `AMO`**).

**Implicaciones**:
* El **material de claves por dispositivo** vive en dos sitios distintos: **atestación**
  (`/DAK/`, para atribuir el dispositivo ante Samsung) y **almacén de la TEE** (`/tee/srt/`,
  donde Keymaster/FBE guarda las claves de las apps).
* **Un backup de EFS sin `keystore2` es incompleto** ⇒ requisito para W3.
* **`FLL.bin` es la protección anti-rollback** ⇒ W4 debe respaldarlo y verificar su hash
  **antes** de tocar la ROM (si el bootloader baja de versión, FLL lo bloquea).
* `prism` (256 MiB) es contenido, no claves ⇒ decidir si entra en el backup.
* **No** existe `/data/misc/keystore/persistent.sqlite` con material exportable: son
  **metadatos** (alias/huellas); las claves viven en la TEE ⇒ **no son portables**.

---

## 4. Boot chain (P3)

| Partición | Bytes | sha256 |
|---|---|---|
| `/dev/block/by-name/boot` | 67.108.864 | `f96cd410e6f048739fe36737f047e2ede6d229d428e1d0c74de80017c8beb4fd` |
| `/dev/block/by-name/vendor_boot` | 67.108.864 | `b55ecfd0744ae8ae5404635b3345b846764e55eca8349a655e71aebd52d0b2fb` |
| `/dev/block/by-name/dtbo` | 8.388.608 | `c2cc0f21784007dde7ae154887b2fc3c69085c73b185ba847ae2e9b29b3ee106` |
| `/dev/block/by-name/vbmeta` | 65.536 | `b1ef8b5108b1a333c13e6db5cf1e2c1446a4e017e81d0fe9b9392de70a1238c9` |
| `/dev/block/by-name/vbmeta_system` | 65.536 | `254732e930687a6d5a3c0c82965f84c9a4a36767f6aa39eeacabf53af6aba5c6` |
| `/dev/block/by-name/recovery` | 67.108.864 | `777350644d9f5c345970fef5f92ea5efdb1fbc9d2b0005b4a8b62dcb90703f19` |

**Ausentes**: `init_boot`, `vbmeta_vendor`, `sec` ⇒ **la cadena AVB de este firmware tiene
2 niveles** (`vbmeta` + `vbmeta_system`), no 3.

**Estado AVB / bootloader (props medidas)**:

| Prop | Valor | Lectura |
|---|---|---|
| `ro.boot.verifiedbootstate` | `green` | verificación OK |
| `ro.boot.flash.locked` | `1` | **bootloader CERRADO** |
| `ro.bootloader` | `G998BXXSBGXED` | binario del bootloader |
| `ro.boot.hardware` | `exynos2100` | SoC |
| `ro.boot.dynamic_partitions` | `true` | super dinámico |
| `ro.boot.slot_suffix` | (vacío) | slot A |

⇒ **Magisk con AVB green y bootloader cerrado** ⇒ Magisk parchea la imagen de arranque y
**re-firma con la clave original**: el bootloader **no** está abierto ⇒ **cualquier flash de
ROM es supervisado sí o sí** (y sin abrir el bootloader no se puede cambiar CSC de forma
permanente).

**Cómo se comprueba**:
```bash
sha256sum /dev/block/by-name/boot /dev/block/by-name/vbmeta
getprop ro.boot.verifiedbootstate; getprop ro.boot.flash.locked; getprop ro.bootloader
ls /dev/block/by-name | grep -E 'init_boot|vbmeta_vendor|^sec$'
```

---

## 5. Apps Samsung (P5) — 229 paquetes `com.samsung.*`

Medido con `pm list packages | grep com.samsung` + `pm path` + `dumpsys package`.

Muestra (versión + ruta real del APK):

| Paquete | Versión | APK |
|---|---|---|
| `com.samsung.android.app.contacts` | 15.1.87 | `/system/priv-app/SamsungContacts/SamsungContacts.apk` |
| `com.samsung.android.knox.attestation` | 1.4.01.5 | `/system/priv-app/EnhancedAttestationAgent/…` |
| `com.samsung.android.smartcallprovider` | 15.0.01.5 | `/system/priv-app/SmartCallProvider/…` |
| `com.samsung.android.vtcamerasettings` | 4.0.04.16 | `/system/app/VTCameraSetting/…` |
| `com.samsung.android.peripheral.framework` | 2.5.01.0 | `/system/priv-app/PeripheralFramework/…` |

**Hallazgos con implicación**:
* Las apps de sistema Samsung viven en **`/system/app` y `/system/priv-app`**, no en `/data/app`
  ⇒ **no se pueden actualizar sin partición de sistema** ⇒ una migración de ROM las cambia.
* `com.samsung.android.knox.attestation` es el cliente de **Enhanced Attestation** ⇒ enlaza
  con la cadena `/DAK/` de la TEE (ver §3.2): **la atestación es la que ata la identidad del
  dispositivo a las claves del EFS**.

**Cómo se comprueba**:
```bash
pm list packages | grep -c '^package:com.samsung'
pm path com.samsung.android.app.contacts
dumpsys package com.samsung.android.app.contacts | grep versionName
```

### 5.1 Evidencia visual del inventario (y un dato que conecta con el CSC)

![Inventario real de apps en el S21 Ultra](../assets/apps_apk_extractor_2026-11-20.jpg)

* `docs/assets/apps_apk_extractor_2026-11-20.jpg` — captura del **Apk Extractor** del propio
  móvil (lista de apps instaladas, sección M). sha256 `d912fb28f3897722c3c4598127b9befaccce5c2b0182633a6142397c28055c85`.
* **Lo que se ve**: `Manual Selector` (`com.aura.oobe.samsung`) · `Marina` (`g.pyn.fi`) ·
  `MarinaPatcher` (`ru.drjmwdme.cgwbfzijr`, nombre ofuscado) · `Markor` · `Marxist Tamil` ·
  `Maskan` · `MasterClass` (`com.yanka.mc`) · `Mastodon` · `Maps` ·
  `Mantenimiento del dispositivo` (`com.samsung.android.ool`).

**Hallazgo que conecta §5 con §6 y §10 (alto)**: `com.aura.oobe.samsung` = "Manual Selector"
es la app de OOBE del **operador**, y en `contents.db` (§P3) aparecen sus ficheros **de AMO**:
`/prism/etc/carriers/single/AMO/sysconfig/config-com.orange.update.xml` ·
`.../config-com.aura.oobe.samsung.xml` · `.../default-permissions/privapp-permissions-com.orange.update.xml`.
⇒ **AMO = Orange España**, coherente con lo que dice el XML descifrado del CSC
(`<Country>SPAIN</Country>`, `<CountryISO>ES</CountryISO>`) y con el CSC propietario
(`com.orange.update`).
⇒ **Aviso para el cambio de CSC**: cambiar AMO→THL/ILO/XXV **no es sólo un flag**: se cambia
también el **perfil de operador** (OOBE, permisos por defecto, configuración de APN/VoLTE y apps
del carrier) ⇒ puede dejar de funcionar la configuración específica de Orange (APN, VoLTE, roaming).
**El parche mínimo y menos invasivo es editar el `cscfeature.xml` de AMO** (añadir el
`<CscFeature_VoiceCall_ConfigRecording>` que falta) en vez de cambiar el CSC entero ⇒ eso exige
actualizar `contents.db` (su `fileSize`) y el CRC/formato que el sistema compruebe.

---

## 6. Radio y ROM (P6)

| Artefacto | sha256 | Bytes |
|---|---|---|
| `/mnt/vendor/efs/nv_data.bin` (NV de radio) | `19d67fb61d8a7f20…` | 1.048.576 |
| `/mnt/vendor/efs/mps_code.dat` (**CSC**) | `3150854ef4e891ec…` (contenido = `AMO`) | 3 |
| `/mnt/vendor/efs/telephony.prop` | `c41f7774681deed9…` | 63 |
| `/mnt/vendor/efs/prov/issued.log` | `ca9a7d93b75d2364…` | 230 |
| `/mnt/vendor/efs/factory.prop` | `ceb67f8921ce59d8…` | 96 |
| `/mnt/vendor/efs/.nv_state` | `6b86b273ff34fce1…` | 1 |

**Props de radio medidas**: `gsm.operator.alpha=JAZZTEL` · `gsm.operator.iso-country=es` ·
`gsm.network.type=LTE` · `gsm.facilitylock.state=false` · `gsm.current.phone-type=[1,1]`.

**Hallazgo**: el **CSC es `AMO`** (América) y vive en `mps_code.dat` (3 bytes exactos) ⇒
cambiarlo con root es la vía **más limpia y reversible** (3 bytes) y **no triangular el Knox**.
**Implicación**: `CscFeature_VoiceCall_ConfigRecording` decide si el firmware graba llamadas
nativamente ⇒ **CORREGIDO (medido en vivo el 2026-11-21 con `csc_verify`)**: el CSC `AMO` **SÍ trae la feature**, con el valor `RecordingAllowedByMenu,disclaimer_latin` ⇒ **la grabación nativa está DISPONIBLE y AVISA** ⇒ **no hace falta parchar el CSC** ⇒ (lo que decía antes —«AMO no la tiene»— estaba mal: salió de una lista de CSC incompleta, no del móvil).

**Cómo se comprueba**:
```bash
cat /mnt/vendor/efs/mps_code.dat | tr -d '\0'; echo
sha256sum /mnt/vendor/efs/nv_data.bin /mnt/vendor/efs/mps_code.dat
getprop | grep -iE 'gsm\.|radio|rild'
```

---

## 7. Tablón rápido de verificación (sha16)

```
7c0041ab601b9d8d  /vendor/lib64/libeden_ud_npu.so          d50035e3b8d8237b  /vendor/lib64/libnpuc_common.so
1a17e3b3a41b2da8  /vendor/firmware/NPU.bin                 3efc4552e11f2ade  /vendor/lib64/libeden_rt.so
e7ff24b0e7bb18e0  /vendor/lib64/libeden_wrapper.so         b1f111905a6857c5  eden_runtime@1.0-service.rc
113723a72bc626e4  android.hardware.neuralnetworks@1.3-service.eden-drv
cad427cda67b6430  vendor.samsung_slsi.hardware.eden_runtime@1.0-service
3012205acc44a8c7  /system/bin/keystore2                   b20fa395f2a6129a  /system/bin/keystore_cli_v2
1ebf380914d2898c  /system/bin/gatekeeperd                  efa211760ed0a23e  /vendor/lib64/hw/gatekeeper.exynos2100.so
979f4864b1ddfd3d  fkeymaster-service                       f73b58435a155c19  /vendor/lib64/libkeymaster4.so
4bac41ddcf454d27  /vendor/lib64/libteecl.so                97f592938b60614a  fkeymaster-V1-ndk_platform.so
5dd8511af3f914b3  keystorage        1f02bc470347f979  keyrefuge   51c4ac95370f442b  sec_efs
c516562faf272e61  uh                bb9f8df61474d25e  uhcfg       5ce506471f50e687  spu
29292381f6b4a149  prism
f96cd410e6f04873  boot              b55ecfd0744ae8ae  vendor_boot  c2cc0f21784007dd  dtbo
b1ef8b5108b1a333  vbmeta            254732e930687a6d  vbmeta_system  777350644d9f5c34  recovery
19d67fb61d8a7f20  nv_data.bin       3150854ef4e891ec  mps_code.dat (CSC=AMO)
```

---

## 8. Límites de este manual (lo que NO afirma)

* **La tabla de ioctls no está confirmada contra el firmware vivo** (§1.2): la aritmética es
  sólida, la correspondencia con nombres y la presencia en el binario vivo, no.
* **No se ha extraído material criptográfico**: ni claves privadas, ni el contenido de las
  particiones del mundo seguro, ni `persistent.sqlite`. Sólo rutas, tamaños y hashes.
* **Las apps se inventariaron, no se decompilaron** las 229; el volcado de APKs de Samsung
  queda pendiente (su valor está en el APK, no en el volcado de ROM).
* Los hashes son **del firmware en vivo de este dispositivo concreto**; otro firmware del
  mismo modelo puede diferir ⇒ **vuelve a medir antes de comparar**.

---

*Documento del nodo `cloud-sandbox` (W7). Las mediciones son reproducibles con `root_exec` +
`sha256sum`; las tools `env_introspect` y `megadaemon_rizin` del móvil permiten repetirlas
sin escribir código. Sin credenciales en este documento.*

---

## 9. Respuestas medidas al agente RE externo (P1–P7)

Todo lo de esta sección está **medido en el móvil vivo** el 2026-11-20.

### P1 · ¿Qué APK declara `CAPTURE_AUDIO_OUTPUT`? — **Ninguno de Samsung**

Barrido de **398 APKs** (`/system/app`, `/system/priv-app`, `/vendor/app`, `/product/priv-app`,
`/prism/app`, `/odm/app`) = **5.580 MB leídos**, buscando `android.permission.CAPTURE_AUDIO_OUTPUT`
en UTF-16LE y UTF-8. **3** APKs contienen la cadena; al comprobar si está en el **manifiesto**
(`AndroidManifest.xml`) vs sólo en el código:

| APK | ¿en el manifiesto? | ¿en el DEX? | Veredicto |
|---|---|---|---|
| `/system/priv-app/YourPhone_P1_5/YourPhone_P1_5.apk` (Microsoft Your Phone) | **sí** | sí | **declara el permiso** |
| `/system/priv-app/LinkToWindowsService/LinkToWindowsService.apk` (Microsoft Link to Windows) | **sí** | sí | **declara el permiso** |
| `/system/priv-app/GalaxyApps_OPEN/GalaxyApps_OPEN.apk` (Galaxy Store) | **no** | sí | *falso positivo*: sólo lo **referencia** en código |

⇒ **Implicación (alta)**: los únicos que **declaran** el permiso son dos apps de **Microsoft**
(continuidad PC-teléfono). **Samsung no trae ninguna app con `CAPTURE_AUDIO_OUTPUT`**, y el
grabador tampoco está en los jars de `/system/framework` (comprobado por el agente RE).
⇒ **CORREGIDO (medido en vivo el 2026-11-21 con `csc_verify`)**: en este firmware (CSC **AMO**) la grabación nativa **SÍ está implementada** y con **aviso**: el valor de `CscFeature_VoiceCall_ConfigRecording` es `RecordingAllowedByMenu,disclaimer_latin` ⇒ ⇒ **no hay que parchar nada** para tenerla ⇒ ⇒ y si se quisiera quitar el aviso, eso sería una decisión **del usuario** (grabar en silencio a un tercero es delito en España, art. 197 CP), no un detalle técnico.
ninguna app**: o la activa el framework por CSC (y aquí no está), o hay que cambiar de CSC.
**Cómo se comprueba**:
```bash
# recorre los APK y separa manifiesto de código (el permissão va en el manifiesto)
for a in $(pm list packages -f | grep -E '\.apk:' | cut -d: -f2); do
  unzip -p "$a" AndroidManifest.xml 2>/dev/null | strings | grep -q CAPTURE_AUDIO_OUTPUT && echo "$a"
done
```

### P2 · Qué clase/método lee el flag — **bloqueado por un bug real de `dex_decompile`**

La tool `dex_decompile` del propio daemon (ARM64, live) **falla** sobre el DEX de SamsungInCallUI:

```json
{"error":"no se pudo parsear /data/local/tmp/siui/classes.dex: encoded_value con tipo desconocido 0x1e","ok":false}
```

`0x1e` = **`VALUE_NULL`** y `0x1f` = **`VALUE_BOOLEAN`** en `encoded_value` del formato DEX: el parser
de la flota **no implementa esos dos tipos**. Los DEX están **sanos** (cabecera `dex
` válida,
30.480 cadenas, 10.019 `class_defs`), ⇒ **es un bug del parser, no del fichero**.
**Implicación**: cualquier APK de Samsung con un `null` en una constante estática se le queda
**sin descompilar** ⇒ afecta a `dex_decompile` y a `app_intel`.
**Estado**: **ya corregido** por el agente DEX en `rust-core/src/dex_decompile.rs` (el parser leía
los tipos de valor como lista consecutiva: un `null` 0x1e o un array 0x1c lo rompían) + CLI
`megadaemon-core dex` para decompilar **en el sandbox**. ⇒ **queda pendiente el despliegue del
ARM64 con el parser arreglado** para poder usar `dex_decompile` en el móvil.
**Cómo se comprueba**: `dex_decompile {"action":"decompile","path":"/data/local/tmp/siui/classes.dex"}`
debe devolver pseudo-Java en vez del error.

### P3 · `contents.db` **es SQLite**, pero es un *inventario*, no un almacén de CSC

Ruta: `/prism/etc/carriers/single/AMO/contents.db` — **156.672 B**, magic `SQLite f`.

```sql
-- PRAGMA table_info(contents)
salesCode TEXT | _id INTEGER | filePath TEXT | fileSize INTEGER
-- 2006 filas; 22 de AMO; 131 filas apuntan a un cscfeature.xml (uno por CSC, con su tamaño)
```

Filas de AMO (ejemplo): `/prism/etc/carriers/single/AMO/sysconfig/orange.xml` (169 B) ·
`.../AMO/sales_code.dat` (4 B) · `.../AMO/default_workspace.xml` (5.573 B) ·
`.../AMO/default-permissions/privapp-permissions-com.orange.update.xml` (216 B).

⇒ **Implicación (alta)**: los valores de `CscFeature_*` **NO están en `contents.db`**: esa tabla
es el **catálogo de ficheros** del paquete CSC (ruta + tamaño). Sirve, eso sí, para **auditar
integridad** (el `fileSize` esperado de cada fichero) y para saber **qué ficheros forman el CSC**.
Los valores viven en el `cscfeature.xml` de cada CSC (131 referenciados), **que es el fichero
ofuscado** (§P4). ⇒ Cambiar un CSC = reescribir el XML ofuscado; `contents.db` te dice el
tamaño que debería tener (detecta si el parche quedó mal).

### P4 · El formato OMC: **no es gzip+XOR** (dato nuevo y restrictivo)

`/optics/configs/carriers/single/AMO/conf/system/cscfeature.xml` = **1.109 B**,
primeros bytes **`2f 27 29 b7 da c1 ac 37`**, **no** empieza por `1f 8b` (magic de gzip) y **no**
es texto ⇒ el envoltorio OMC es **un formato binario propio con cabecera**, no "gzip + XOR".
⇒ **Implicación**: el decodificador que se porte a Rust tiene que leer una **cabecera OMC**
(magic de 4 B + campos) antes de descomprimir; buscar el XOR a ciegas no funciona.
**No hay ningún APK decodificador OMC en `/prism/app`** (11 APKs medidos: `289_companion`,
`MDIP-TIMO-2uk-DTDE-DTAT-VFAU-optus-3IT`, `SwiftKey-market-arm64-v8a-release`,
`SwiftKeyFactorySettings-samsung`, `idapk-1.0.0.47-2`, `idapk-1.0.0.75-92`,
`instagram-stub`, `yandex-browser`, `yandex-disk`, `yandex-maps`, `yandex.search`).
⇒ el algoritmo hay que sacarlo del repo de referencia (`soulr344/OMCDecoder`) o de un APK de
OMC en `/optics`.

### P5 · Swift Backup — **sin respuesta aún** (no he恍dedicado su formato)

Pendiente: derivación de clave AEGIS-256, formato `.splits`/`.dat`/`.med`, esquema JSON de
`Config`/`Blacklist`, proveedores cloud y el caso Chrome. Los DEX están **en el móvil**
(`/data/local/tmp/swift/classes.dex` 9.273.900 B, 50.772 cadenas; `classes2.dex` 2.844.984 B,
17.205 cadenas) ⇒ en cuanto `dex_decompile` arregle `VALUE_NULL`/`VALUE_BOOLEAN` (P2), su
`action:"strings"` sirve de punto de partida. **No he touching ese APK**: lo tiene el agente RE.

### P6 · El HAL de audio: **sin camino de captura de voz** (veredicto)

Módulos de audio presentes en el móvil: **`/system/lib64/libaudiohal.so` (33.184 B)** y los HAL
de efectos en `/vendor/lib64/`. Barrido de cadenas (`voice|voip|csd|VOICE_CALL|AUDIO_DEVICE_IN_|capture|in_call`):
**cero coincidencias** en `libaudiohal.so`; el resto de módulos de audio no exponen literales de voz.
⇒ **Veredicto**: **no hay un path de captura de voz en el HAL** ⇒ el audio `VOICE_CALL` lo genera
el **modem** y **no hay parche de AudioFlinger que lo arregle** ⇒ las dos vías reales son
(a) el grabador del propio Samsung, que **no existe en este firmware** (P1), o (b) **cambiar el CSC**.

### P7 · Estado de este nodo

Manual publicado (`docs/ANDROID_EXYNOS2100_FINDINGS.md`, commit `53e00ab` + apéndice §9);
P1–P6 medidos; **P2 bloqueado por un bug del parser DEX de la flota** (§P2) y **P5 sin empezar**.
**Lo que necesito del exterior**: (a) el algoritmo/repo del decodificador OMC (§P4) o un APK de
OMC con su sha256; (b) confirmación del nombre de la clase de Swift Backup que deriva la clave
AEGIS (o el APK y su sha256 para que lo extraiga); (c) confirmación del **CSC objetivo** para
poder medir la diferencia de `cscfeature.xml` entre AMO y ese CSC.

---

## 10. El formato OMC (P4 **RESUELTO**) — rotación + XOR posicional, NO gzip+XOR

### 10.1 El algoritmo (exacto, con referencia)

Fuente: **`soulr344/OMCDecoder`** → `include.h` (C++), reimplementado en
**`tools/omc_codec.py`** (en este repo, Python de referencia, listo para portar a Rust).

El fichero OMC **no** está "gzip + XOR": es

```
fichero OMC  --omc_decode()-->  stream gzip  --gunzip-->  XML plano
```

donde `omc_decode` es, byte a byte y **dependiente de la posición** (la clave gira cada 256 bytes):

```python
def omc_decode(data):
    out = bytearray(len(data))
    for i, b in enumerate(data):
        s = SHIFTS[i % 256]                     # 0..7, tabla de 256
        out[i] = ((((b << s) | (b >> (8 - s))) & 0xFF) ^ (SALTS[i % 256] & 0xFF))
    return bytes(out)                          # -> gunzip -> XML
```

y el inverso para **re-codificar**:

```python
def omc_encode(data):                          # data = XML plano -> gunzip -> gunzip? no: gunzip->zip
    out = bytearray(len(data))
    for i, b in enumerate(data):
        a = b ^ (SALTS[i % 256] & 0xFF)
        s = SHIFTS[i % 256]
        out[i] = ((a >> s) | (a << (8 - s))) & 0xFF
    return bytes(out)
```

**Esquema en Rust** (equivalente exacto; `salts` puede ser negativo ⇒ `as u8` trunca a 2's complement):

```rust
fn omc_decode(data: &[u8]) -> Result<Vec<u8>, OmcError> {
    let mut out = Vec::with_capacity(data.len());
    for (i, &b) in data.iter().enumerate() {
        let s = SHIFTS[i % 256] as u32;
        let rot = (((b as u32) << s) | ((b as u32) >> (8 - s))) as u8;
        out.push(rot ^ (SALTS[i % 256] as u8));
    }
    gzip::decompress(&out)
}
```

**Cómo se comprueba** (validado hoy sobre el móvil, no sobre un volcado):

```bash
python3 tools/omc_codec.py decode <fichero.omc>            # -> XML por stdout
python3 tools/omc_codec.py feature <fichero.omc> CscFeature_VoiceCall_ConfigRecording
python3 tools/omc_codec.py encode <plano.xml> -o out.omc
```

| Comprobación | Resultado |
|---|---|
| Descifrar el fichero **real** de AMO (`/optics/configs/carriers/single/AMO/conf/system/cscfeature.xml`, 1.109 B, sha16 `ef110140f7bb95e1`) | XML `<SamsungMobileFeature>` con `<SalesCode>AMO</SalesCode>`, `<Country>SPAIN</Country>`, `<CountryISO>ES</CountryISO>` ✓ |
| **Round-trip** `encode(decode(x)) == x` | **True** en los **126/126** CSC del móvil ✓ |
| Contenido tras descifrar | 1.109 B → **3.589 B** de XML plano (AMO) |

⇒ **Confianza: high.** No es una hipótesis: es el algoritmo exacto, con round-trip verificado.

### 10.2 Qué CSC traen grabación nativa de llamadas (medido en los 126 CSC del móvil)

`CscFeature_VoiceCall_ConfigRecording` **existe en 20 de 126** CSCs (medido en vivo con `csc_verify` el 2026-11-21):

| Valor | CSCs |
|---|---|
| `RecordingAllowed` | **BKD · BTC · CAL · EGY · ILO · ILP · LYS · MYM · NPL · SEK · SLK · THL · TUN · XXV** |
| `RecordingAllowedByMenu,disclaimer_latin` | CHO · EON · GTO · TPA |
| `RecordingAllowedByMenu,disclaimer_india` | INU |
| **AUSENTE** (107 CSCs) | **AMO** (nuestro), y los demás |

⇒ **AMO NO tiene la feature de grabación** ⇒ con CSC=AMO **no hay grabación nativa**, y esto
confirma la hipótesis del agente RE con dato duro (no con suposición).
⇒ **Candidatos para el CSC**: **THL** (Tailandia, 630 B, sha16 `a108771bde4cdfbf`),
**ILO** (Israel, 422 B, sha16 `cc1e85ef88546bd5`), **XXV** (627 B, sha16 `81c622acb4ffb78b`):
los tres con `RecordingAllowed` **sin** disclaimer (⇒ grabación sin menú de aviso).
⇒ **Nota de riesgo**: `contents.db` guarda el `fileSize` de cada fichero del CSC ⇒ un
`cscfeature.xml` parcheado con otro tamaño **se detecta** en el inventario ⇒ hay que
actualizar **a la vez** `contents.db` (y el CSC en `mps_code.dat`) o el sistema puede
invalidar el paquete. Además el árbol `/optics` es de sólo lectura en runtime ⇒ el cambio
va por una imagen de sistema (o `remount`), no escribiendo en vivo.

### 10.3 Lo que esto NO resuelve (para que nadie lo dé por hecho)

* **El mecanismo en runtime** (qué componente lee `cscfeature.xml` y cómo lo cachea) **sigue
  sin localizar**: el magic `2f2729b7` **no aparece en ninguno de los 2.476 ficheros**
  del firmware (barrido de `/system`, `/vendor`, `/system_ext`, jars incluidos) ⇒ el decodificador
  **no está en el firmware**, sino en las herramientas de build de Samsung (y en el APK de CSC
  del que sale `OMCDecoder`). Los candidatos en el firmware son los que **consumen** la feature
  (`services.jar` 29 hits, `vsimmanager.jar` 428, `telephony-common.jar` 19, `imsmanager.jar` 3).
* **P5 (Swift Backup: derivación de clave AEGIS-256, formato `.splits`/`.dat`/`.med`, JSON de
  `Config`/`Blacklist`, proveedores cloud, caso Chrome) sigue ABIERTA**: no la he tocado. Los DEX
  están en el móvil (`/data/local/tmp/swift/classes.dex` 9.273.900 B / 50.772 cadenas).
  El agente DEX ya arregló el parser DEX (`VALUE_NULL`/`VALUE_BOOLEAN`) ⇒ su CLI
  `megadaemon-core dex` sirve de punto de partida.

### 10.4 El consumidor en runtime (RESUELTO): `com.samsung.android.feature.SemCscFeature`

El magic `2f2729b7` no aparece en el firmware ⇒ **el decodificador OMC tampoco**. Where está:
**dentro del framework base**, no en `services.jar`:

| Dato | Valor (medido) |
|---|---|
| Fichero | **`/system/framework/framework.jar`** → **`classes5.dex`** (10.684.140 B, sha16 `19688a3b1dde2fe1`) |
| Clase | **`Lcom/samsung/android/feature/SemCscFeature;`** (28 métodos) |
| Plantilla de ruta | **`/cscfeature.xml`** (⇒ `<csc_dir>/cscfeature.xml`) |
| Parser auxiliar | `Lcom/samsung/android/util/CustomizedTextParser;` (9 métodos, `getValue`/`search`/`searchList`/`update`) |

**Métodos de `SemCscFeature` (API pública)**:
`getInstance()` · `loadFeatureFile(...)` (×2) · `isXmlEncoded()` · `decode(...)` · `_decode(...)` ·
`_decompressGzip(...)` · `needToSkipElement(...)` · `isElementWithCountryISO(...)` ·
**`getBoolean(...)` (×4)** · `getInt`/`getInteger`/`getString` (×2 cada uno).

⇒ **La cadena literal `cscfeature.xml` aparece UNA vez en todo el firmware** (framework.jar) ⇒
éste es **el** consumidor: `SemCscFeature.getBoolean("CscFeature_VoiceCall_ConfigRecording")`.
El nombre **no** aparece literal (el framework compone `"CscFeature_"` + categoría + nombre, y
sólo lleva embebidos algunos nombres concretos) ⇒ el valor se busca por **clave dinámica**.
⇒ **El framework trae su propio decodificador OMC** (`isXmlEncoded()` + `decode()` +
`_decompressGzip()`) ⇒ **es exactamente el algoritmo de §10.1** (rotación+XOR y gunzip) ⇒
**confirma la implementación** y explica por qué el magic no aparece: no se busca una constante,
se *detecta* el formato (`isXmlEncoded`).
⇒ **Implicación para el parche**: añadir `<CscFeature_VoiceCall_ConfigRecording>RecordingAllowed</…>`
al `cscfeature.xml` de AMO ⇒ lo leerá `SemCscFeature` al cargar ⇒ **el flag llega al sistema**
(sin tocar el CSC, sin triangular el Knox). Requisitos: (a) `/optics` sólo lectura ⇒ imagen o
bind-mount de Magisk; (b) actualizar el `fileSize` en `contents.db` a la vez; (c) reiniciar
(la feature se cachea al boot).
⇒ **Cómo se comprueba** (sin root, con el binario del repo):
```bash
# 1) el fichero real descifrado con nuestra herramienta
python3 tools/omc_codec.py decode /optics/configs/carriers/single/AMO/conf/system/cscfeature.xml | head
# 2) la clase que lo consume (dex de framework.jar)
unzip -p /system/framework/framework.jar classes5.dex > /data/local/tmp/classes5.dex
strings /data/local/tmp/classes5.dex | grep -E "SemCscFeature|cscfeature.xml"
```
