# W3 · Traspaso para el backup del S21 Ultra (entrada: el inventario W2)

> No es un módulo: es la **entrada** que necesita el dueño de W3 (tablón: `6cb64e00`;
> el principalExcuye: "W3 ya tiene dueño"). Todo lo que hay aquí sale **medido** del
> `inventory.json` (W2, commit `a56b1c3`), no de suposiciones.

## 0. La restricción que manda

- Disco: **225 GB al 98 %**, **5,9 GB libres** ⇒ **nada de copia local intermedia**:
  todo va **en trozos directamente a Drive**, con sha256 por trozo.
- RAM: 10,8 GB con 4,2 GB de swap ⇒ nada de procesos paralelos ni de barridos
  repetidos en el móvil.
- El móvil ya tuvo un OOM killer ⇒ **una sola pasada**, y el hash se verifica en
  destino (no se re-lee el fichero después).

## 1. Qué respaldar (por nivel), con el tamaño medido

### L0 · Volátil (el estado de MegaDaemon) — `/data/local/tmp/megadaemon/state/`
Contiene: `board.sqlite` (el tablón), `cockpit_token`, `megadaemon_api.key`,
`agents/`, `audit`, `journal`, `sync/`. Es **pequeño** ⇒ cabe en un solo trozo;
es lo único que hay que respaldar **y** lo único que se puede restaurar sin
briclear. **Cuidado**: aquí hay material sensible (token, key) ⇒ va **cifrado**
(o excluido del paquete público y subido sólo al Drive privado).

### L1 · ROM (material estático) — ya existe
El pack de **35,72 GB** está en el Drive del nodo RE (carpeta publicada por
`cloud-sandbox`). **No hay que volverlo a subir**: lo que falta es
1. el **manifiesto con sha256** de ese pack (para verificarlo al restaurar), y
2. la **lista de particiones del mundo seguro** (abajo), que es lo que NO está
   en el pack y es lo que protege el brick.

### L2 · Datos de usuario
Medido en el móvil (mapa de espacio del inventario):
`/data/media/0/Download` ≈ **44,3 GB** · `/data/media/0/Android` ≈ 19,6 GB ·
`/data/media/0/DCIM` ≈ 18,0 GB · el resto (Pictures, Movies, Music, Documents)
≈ <1 MB en total.
⇒ **el 99 % del backup de usuario es Download + Android + DCIM**: con 5,9 GB
libres en el móvil, la estrategia viable es **por trozos, sin materializar**.

### L3 · Credenciales — sólo cifradas
**Nunca en claro**, ni en Drive ni en el repo. Ver §3.

## 2. Particiones del mundo seguro (targets de L1 y de la pre-flight de W4)

Del inventario (`boot_y_particiones.by_name` y `tabla_particiones`):

| Partición | Bloque | Tamaño | Qué protege |
|---|---|---|---|
| `efs` | sda1 | — | EFS (IMEI, calibración) |
| `sec_efs` | sda2 | 20 MiB | EFS secundario |
| `keystorage` | sda6 | 512 KiB | Material de claves del keystore |
| `uh` | sda9 | 2 MiB | User hardware |
| `uhcfg` | sda10 | 4 MiB | Config de UH |
| `keyrefuge` | sda21 | 52 MiB | Refugio de claves |
| `prism` | sda29 | 256 KiB | Prism (TEE) |
| `spu` | sda33 | 50 MiB | SPU (Secure Processing Unit) |

Además, del propio móvil: **`/root/FLL.bin`** es la **protección anti-rollback**
(FLASH LOCK) ⇒ **copiarlo y verificar su sha256 ANTES de tocar la ROM** (W4).

## 3. L3: credenciales cifradas con clave derivada de hardware

Lo que **sí** es respaldable (cifrado, con clave derivada del propio dispositivo):

- `state/cockpit_token`, `state/megadaemon_api.key`, `state/agents/*.json`,
  `adb_host_key.pem` (la **pública**; la privada nunca sale del móvil salvo que
  el dueño decida lo contrario).

**Clave derivada de hardware (propuesta, sin implementarla aquí):**
`clave = HMAC-SHA256(material_secreto_del_dispositivo, "megadaemon-backup-v1")`
donde `material_secreto_del_dispositivo` puede ser el `keystorage`/`keyrefuge`
(o, si no es accesible, una cadena derivada de `ro.boot.serialno` + la clave
privada ADB local). La clave **derivada no se sube**: se guarda en el gestor de
secretos y se puede **re-derivar** en el dispositivo de destino.

**Lo que NO es recuperable (no prometerlo):**
- Las **claves del AndroidKeyStore / keystore2** (van atadas al hardware y al
  gatekeeper del móvil: `keystore2` y `gatekeeperd` viven en `/system/bin`).
- Las **claves de sesión de WhatsApp** y de otras apps cifradas con keystore
  ⇒ sus bases son **ilegibles** (no "se han perdido": nunca fueron legibles).
- El **TEE** (`/tee/srt/<uuid>/<id>/`) depende del keymaster del SoC.

⇒ En el informe de backup hay que listar explícitamente esto como
**"no recuperable por diseño"**, no como un fallo.

## 4. Protocolo de streaming (el que ya funciona en el repo)

1. En el móvil: `tail -c +<offset> <fichero> | head -c <trozo> | base64 -w 0`
   (trozo = **256 KiB**, como en `md-device-inventory` y en el vault de blobs).
2. sha256 del **trozo** en origen; el manifiesto NDJSON lleva
   `id, sha256, bytes, path_relativo`.
3. Subida por trozos a Drive; **verificación del hash en destino** (fail-closed
   por entrada: lo que no cuadra se rechaza y no se escribe).
4. `MANIFEST.ndjson` + `SHA256SUMS` como índice; el **restaurador** los usa para
   verificar antes de escribir.

## 5. Restore probado (el DoD de W3)

Un backup sin restore probado no es un backup. Procedimiento mínimo verificable
**sin brickear nada**:

1. Materializar el pack en un directorio de prueba (**en el PC, no en el móvil**).
2. Verificar **sha256 de los 3 primeros ficheros** contra `MANIFEST.ndjson`.
3. **3 lecturas reales** y con sentido:
   - `board.sqlite` → abrir con SQLite y leer la fila de una tarea real.
   - `cockpit_token` → leer y comprobar que tiene la longitud del token.
   - Un fichero de L2 (p. ej. un JPEG de DCIM) → leer su cabecera y comprobar
     que es la imagen esperada.
4. Documentar el resultado **con la salida real** (no "funciona").

> En este sandbox **no hay emulador ni dispositivo de repuesto**: el restore se
> puede probar en el host (materializar + verificar + 3 lecturas), pero **no**
> instalar una ROM. Eso lo debe hacer quien tenga el dispositivo de repuesto; el
> pack y el manifiesto dejan el procedimiento listo.

## 6. Entradas exactas que hay que leer del inventario

- `boot_y_particiones.by_name` → mapa de particiones con su bloque.
- `boot_y_particiones.tabla_particiones` → tamaños en KB.
- `espacio.uso_por_directorio_media` → qué directorios de datos de usuario hay.
- `espacio.filesystems` → % de uso y libre por punto de montaje.
- `apps.paquetes[].ruta_apk` → **dónde está el APK de cada app** (para respaldar
  datos por app sin adivinar).
- `red.claves_adb_autorizadas` → quién puede hablar con el móvil por ADB.
