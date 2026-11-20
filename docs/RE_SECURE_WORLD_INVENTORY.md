# P2-bis — Inventario del mundo seguro del S21 Ultra (rutas + hashes, **sin** material de claves)

**Condición del principal cumplida**: aquí sólo hay **rutas, tamaños y hashes**. El
**contenido** de las particiones y de los ficheros de clave **no se ha copiado, extraído
ni ha salido del móvil**: los sha256 se calcularon **dentro del dispositivo** (`root_exec`,
streaming de 1 MiB, sólo lectura). No hay material criptográfico en el journal, ni en
Drive, ni en el repo.

- Dispositivo: SM-G998B (Exynos 2100), Android 14
- Fecha: 2026-11-20T00:47Z
- Método: `root_exec` → script Python on-device → sha256 en streaming → **sólo el
  resultado (rutas/tamaños/hashes)** sale del móvil

## 1. Las 7 particiones del mundo seguro (sha256 real)

| Partición | Dispositivo real | Bytes | sha256 |
|---|---|---|---|
| `keystorage` | `/dev/block/sda6` | 524.288 | `5dd8511af3f914b3a17e5ec57969c6cafc654406a8db2157f88302fa1a07235c` |
| `keyrefuge` | `/dev/block/sda21` | 54.525.952 | `1f02bc470347f979f284443605ead1abae2c3f715901375aa41bb59e658b6793` |
| `sec_efs` | `/dev/block/sda2` | 20.971.520 | `51c4ac95370f442baf7a166754a15028887636a123dd478645c4495fefd24ac8` |
| `uh` | `/dev/block/sda9` | 2.097.152 | `c516562faf272e61d336d32d8e751e8e23352985052e340014c193bfac1a3a61` |
| `uhcfg` | `/dev/block/sda10` | 4.194.304 | `bb9f8df61474d25e71fa00722318cd387396ca1736605e1248821cc0de3d3af8` |
| `spu` | `/dev/block/sda33` | 52.428.800 | `5ce506471f50e68725125c9266867e844dd5cf87c431a70873eb9a9cac00e5ad` |
| `prism` | `/dev/block/sda29` | 268.435.456 | `29292381f6b4a1498aee6df80ae3f045e505ee71ae77be5bf6d28f1dbc78a712` |

**Para W3/W4 (backup antes de tocar ROM)**: `keystorage`, `keyrefuge`, `sec_efs`, `uh`,
`uhcfg` y `spu` son las que hay que **respaldar con verificación de hash**. `prism` son
256 MiB (partición de contenido, no de claves) ⇒ **decidir si entra en el backup**: cabe,
pero es la más grande y la menos crítica para el arranque.

## 2. Inventario del EFS (`/mnt/vendor/efs`) — rutas y tamaños, **sin contenido**

| Ruta | Contenido (nombre + tamaño) |
|---|---|
| `/DAK/` | `GAK_EC.private` (312 B) · `GAK_EC.private.val` (103 B) · `GAK_RSA.private` (1.963 B) · `GAK_RSA.private.val` (416 B) · `SAK_EC.private` (463 B) · `SAK_EC.private.val` (136 B) · `gakeccert0..2.der` · `gakrsacert0..2.der` · `sakeccert0..2.der` · `sgakeccert0..2.der` · `sgakrsacert0..2.der` · `sak_secnvm` (9 B) · `sgakresultgsi` (1 B) |
| `/tee/srt/<uuid>-2147483648/<id>/` | `obj_info` (128 B) + `data_chunk` (16.428 B) — **almacén seguro de la TEE** (Keymaster/FBE) |
| `/prov_data/dev_root/` | `dev_root.dat` (2.556 B) — raíz de provisioning |
| `/prov/` | `issued.log` (230 B) · `diag.dat` (65 B) |
| `/root/` | `FLL.bin` (144 B) — **rollback protection** · `LRHistory.txt` · `ReworkHistory.txt` · `sc.log` (342 B) |
| `/taas/` | `taas` (8 B) · `taaslog` (9.152 B) — Samsung TA |
| `/` | `wv.keys` (168 B) — Widevine · `nv_data.bin` (1 MiB) + `.md5` — NV de radio · `factory.prop` (96 B) · `telephony.prop` (63 B) · `mps_code.dat` (3 B) · `nv.log` (9.234 B) · `.nv_state` · `.nv_data.bak` · `wifi/.mac.info` (17 B) · `bluetooth/bt_addr` (17 B) |

### Lo que esto dice de la TEE de Samsung

1. **El material de claves por dispositivo está en dos sitios distintos**, y no es el
   mismo mecanismo:
   · **`/DAK/`** = cadena de **atestación** (Device Attestation Key + Samsung
     Attestation Key, EC y RSA) ⇒ es lo que permite al servidor de Samsung *atribuir* el
     dispositivo. Sus claves privadas son `.private` y van con un `.val` (valor de
     verificación) ⇒ **son clavesIv+IV encadenadas**, no exportables en claro.
   · **`/tee/srt/`** = **almacén seguro de la TEE** (objetos con `obj_info` + `data_chunk`),
     que es donde Keymaster/FBE guarda las claves de las apps.
   ⇒ **Conclusión operativa**: para re-construir el estado de claves de un dispositivo
   **no basta el EFS**: hace falta además el *keystore2* (motor del sistema, en
   `/system/bin`) y la TEE (`fkeymaster-service`). **Un backup de EFS sin keystore2 es
   un backup incompleto** ⇒ esto es un requisito nuevo para W3.
2. **`FLL.bin` (144 B) = rollback protection**: si el bootloader o la TEE lo verifican,
   un downgrade de bootloader/ROM puede dejar el móvil inservible ⇒ **W4 debe
   respaldarlo y comprobarlo antes de tocar nada**.
3. `spu` (50 MiB) es probablemente el almacén de la SPU (Sensor Processing Unit) y
   `prism` (256 MiB) contenido de usuario/varios ⇒ **pendiente de identificar** (P4/P6).

## 3. Qué NO se ha hecho (a propósito)

- No se ha copiado ni un byte de estas particiones al sandbox, a Drive ni al repo.
- No se ha abierto ningún `.private` ni ningún `data_chunk`.
- Los sha256 se calcularon **dentro del móvil**; el resultado (hash) sí ha salido, que es
  un resumen criptográfico, no el material.

## 4. Broadcast

- `state/mesh_journal.ndjson` y `state/sync/re_cloud-sandbox.ndjson`: entrada
  `kind=p2bis_secure_world_inventory` con las 7 particiones (sha256) y el inventario EFS.
- `state/mesh_board.json`: mensaje `cloud-sandbox-p2bis-*` + bloque `p2_p3_p4_live.p2bis_secure_world`
  con `content_extracted:false`.
