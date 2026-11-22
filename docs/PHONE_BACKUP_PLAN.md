# PLAN DE RESPALDO DEL S21 ULTRA — POR FASES (TODO MEDIDO)

> Todos los pesos y tiempos de este plan salen de la medición del móvil del 2026-11-22
> (ver `docs/PHONE_INVENTORY.md`). Las duraciones son **proyecciones calculadas con la tasa
> de subida MEDIDA de 304.742 B/s** (no un hecho: el transporte real puede cambiar).
> Lo que no se midió está marcado **NO VERIFICADO**.

## 0. El encuadre, en cifras

| Magnitud | Valor medido |
|---|---|
| Datos que hay que respaldar (fases 1–6) | **205.600.475 KB = 196.08 GiB** |
| Datos que NO hay que respaldar (regenerables + derivados) | **9.139.120 KB = 8.72 GiB** |
| Espacio libre en el móvil | **10.732.152 KB (10,23 GiB)** |
| Espacio libre en Drive | **3,751 TiB** |
| Peso que ocuparía la copia en Drive | **196.08 GiB = 0,019 TiB = 0,50 % de la cuota libre** |
| Tiempo de subida con la tasa medida (304.742 B/s) | **191.9 h ≈ 8.0 días** |
| Mismo backup, si el `client_id` compartido de rclone deja de funcionar | **0,0 h — no se puede hacer** (ver §6) |

**El dato que decide todo el plan: en el móvil sólo hay 10,23 GiB libres.**
El respaldo completo (196,08 GiB) **NO se puede etapaizar en el móvil**. Tiene que ir **directo**
de móvil a Drive con `rclone copy` (streaming), que es exactamente lo que ya hace el pipeline vivo.

---

## 1. Las 6 fases, con su peso medido

| Fase | Qué entra | Peso (KB) | Peso (GiB) | Horas @304.742 B/s | Días |
|---|---|---|---|---|---|
| **F1** | Irreemplazable: fotos. vídeo y multimedia de WhatsApp | 25.656.310 | 24.47 | 23.9 | 1.0 |
| **F2** | Los APK: reinstalables. pero 1.262 apps que no están en la tienda | 80.078.194 | 76.37 | 74.7 | 3.1 |
| **F3** | Datos de las apps (todo /data/user/0 menos cachés) | 48.243.910 | 46.01 | 45.0 | 1.9 |
| **F4** | Descargas (ya las está subiendo el loop vivo) | 36.184.304 | 34.51 | 33.8 | 1.4 |
| **F5** | Datos de terceros en /sdcard/Android/data | 12.238.399 | 11.67 | 11.4 | 0.5 |
| **F6** | Root. perfiles secundarios y misceláneo | 3.199.358 | 3.05 | 3.0 | 0.1 |
| **Total** | | **205.600.475** | **196.08** | **191.9** | **8.0** |

### Desglose de cada fase (medido)

**F1 — Irreemplazable (25.656.310 KB = 24.47 GiB, 23.9 h)**

| Contenido | KB | GiB |
|---|---|---|
| `/sdcard/DCIM` (Camera 15.538.492 + Screenshots 1.997.176 + Screen recordings 622.248 + resto) | 18.207.664 | 17,36 |
| `/sdcard/Android/media/com.whatsapp/WhatsApp` | 7.029.622 | 6,71 |
| `/sdcard/Pictures` | 316.237 | 0,30 |
| `/sdcard/Movies` + `Music` + `Documents` + `Books` | 101.787 | 0,10 |

**F2 — APKs (80.078.194 KB = 76.37 GiB, 74.7 h)**

| Contenido | KB | GiB |
|---|---|---|
| `/data/app/*/*/base.apk` + `split*.apk` (**2.776 ficheros** en 1.342 carpetas de paquete) | 66.967.724 | 63,87 |
| `/data/app/*/*/lib/` (**1.066 carpetas** de librerías nativas por ABI) | 13.110.470 | 12,50 |

Quedan fuera: `oat/` (7,28 GiB) y `*.dm` (0,41 GiB) — artefactos de ART, se regeneran.

**F3 — Datos de apps (48.243.910 KB = 46.01 GiB, 45.0 h)**

| Contenido | KB | GiB |
|---|---|---|
| `/data/user/0` completo | 50.978.411 | 48,62 |
| − cachés regenerables (4.701 directorios medidos) | −2.734.501 | −2,61 |
| **= a respaldar** | **48.243.910** | **46.01** |

Dentro de F3, dos bloques son **re-obtenibles del repo Git, no de la tienda** (decisión del usuario):
`com.agy/files/usr` 5.072.152 KB (4,84 GiB) y `com.agy/files/tools` 1.771.667 KB (1,69 GiB).
Quitándolos, F3 baja a **41.400.091 KB (39.48 GiB, 38.6 h)**.

**F4 — Descargas (36.184.304 KB = 34.51 GiB, 33.8 h)**

| Contenido | KB | GiB |
|---|---|---|
| `/sdcard/Download` (Ordenado 18,09 + Snatcher-Collection 10,79 + Carpetas 4,13 + 1DMP 0,71 + resto) | 36.184.304 | 34,51 |

**Ya está en marcha**: el proceso `rclone copy` vivo (PID 19350) está subida `Carpetas` ahora mismo,
y `YTDLnis` ya está subida y verificada («0 differences found», 34 ficheros).

**F5 — `/sdcard/Android/data` de terceros (12.238.399 KB = 11.67 GiB, 11.4 h)**

| Carpeta | KB | GiB | Por qué importa |
|---|---|---|---|
| `zed.rainxch.githubstore/files` | 7.777.947 | 7,42 | clones de repos: reconstruible re-clonando, pero conviene la lista de repos |
| `com.revanced.net.revancedmanager/files` | 1.335.059 | 1,27 | **APKs parcheados con ReVanced: NO hay de dónde volver a sacarlos** |
| `com.github.axet.audiorecorder` | 515.809 | 0,49 | |
| `ru.drjmwdme.cgwbfzijr` | 444.743 | 0,42 | MarinaPatcher / Lucky Patcher |
| `com.deniscerri.ytdl` | 370.524 | 0,35 | descargas de yt-dlp |
| `org.woheller69.whisper` | 361.547 | 0,34 | modelos Whisper locales |
| resto (11 carpetas más) | 1.524.820 | 1,45 | |

**F6 — Root, perfiles y misceláneo (3.199.358 KB = 3.05 GiB, 3.0 h)**

| Contenido | KB | GiB |
|---|---|---|
| `/data/adb` (módulos Magisk 1.331.199 + `modules---` 40.490 + `rvhc` 119.420 + `magisk.db` 65.536 B + LSPosed `lspd`) | 1.502.572 | 1,43 |
| `/data/user_de/0` (**perfil secundario**: TTS 518.809 + GMS 307.310 + Magisk 20.176 + …) | 946.112 | 0,90 |
| `/data/local` (`tmp`: mis ficheros de medición y logs) | 496.074 | 0,47 |
| `/data/misc` + `/data/log` | 254.600 | 0,24 |

### Lo que NO entra en ninguna fase

| Zona | KB | GiB | Motivo |
|---|---|---|---|
| `/data/app/*/*/oat/` | 7.636.287 | 7,28 | AOT de ART, regenerable |
| `/data/app/*/*/*.dm` | 422.044 | 0,41 | metadatos de dex, regenerables |
| `/data/dalvik-cache/arm64` | 272.047 | 0,26 | caché dalvik, regenerable |
| cachés de apps (4.701 dirs) | 2.734.501 | 2,61 | `cache`, `code_cache`, `noBackup`/`no_backup`, `thumbnails`, `Service Worker`, GPU caches |
| `/sdcard/iLovePDF/processed` | 808.742 | 0,77 | PDF convertidos: derivados sin valor (los originales ya no están) |
| **Total** | **9.139.120** | **8.72** | |

---

## 2. El plan, fase por fase

### FASE 0 — Manifiesto y Credentials (0 bytes, 0 h)

Antes de mover un byte:

1. Congelar el inventario: copiar `/data/local/tmp/inv/*.tsv` (los 1.721 paquetes con su tamaño,
   los 457 del sistema con su ruta de APK, la lista de cachés) al Drive, junto con
   `docs/PHONE_INVENTORY.md`. Es la lista de lo que *tiene* que haber en la restauración.
2. **Crear un `client_id` propio de Google Drive.** rclone avisa por escrito:
   *«This remote uses rclone's shared Google Drive client_id, which is being retired and will
   stop working during 2026»*. Con el `client_id` compartido, **F2+F3+F4+F5+F6 no se pueden
   subir**: el plan entero depende de que el destino funcione.
3. Decidir el cifrado: `/data/user/0` contiene material sensible real
   (`files/.gemini/jetski-standalone-oauth-token`, `magisk.db`, cookies de WebView, cuentas).
   **Cifrar ANTES de subir**, no en Drive.
4. Guardar el mapa de UIDs (`pm list packages -U` → **1.590 uids distintos**): sin él no se puede
   restaurar `/data/user/0` con el propietario correcto (`chown -R <uid>:<gid>`).

### FASE 1 — Lo irreemplazable (F1: 25.656.310 KB = 24,47 GiB, ~23,9 h medidos-proyectados)

DCIM + WhatsApp + Pictures/Movies/Music/Documents/Books. Es lo único que **no se puede volver a
hacer**: fotos, vídeos de pantalla, y los adjuntos que WhatsApp ya borró del chat.

```
rclone copy /data/media/0/DCIM            gdrive:Backup_Android/media/DCIM   --transfers 4 --checkers 8 --stats 30s
rclone copy /data/media/0/Android/media/com.whatsapp gdrive:Backup_Android/media/whatsapp --transfers 4 --stats 30s
rclone copy /data/media/0/Pictures gdrive:Backup_Android/media/Pictures
rclone copy /data/media/0/Movies   gdrive:Backup_Android/media/Movies
rclone copy /data/media/0/Music     gdrive:Backup_Android/media/Music
rclone copy /data/media/0/Documents gdrive:Backup_Android/media/Documents
rclone copy /data/media/0/Books     gdrive:Backup_Android/media/Books
rclone check /data/media/0/DCIM gdrive:Backup_Android/media/DCIM --one-way
```

**NO usar el `rm -rf` del loop actual en esta fase.** El origen se queda hasta que exista una
**segunda verificación independiente** (p. ej. `rclone check` desde otro cliente, o comparar
`sha256sum` de una muestra contra el hash guardado en Drive).

### FASE 2 — Los APK (F2: 80.078.194 KB = 76,37 GiB, ~74,8 h)

De las 1.262 apps de usuario, una parte no está en Google Play (F-Droid, NeoStore, APKs
parcheados con Lucky Patcher / MarinaPatcher / ReVanced) ⇒ **sin los APK no hay app**.
Se copia **el directorio del paquete entero**, no los ficheros sueltos:

```
rclone copy /data/app gdrive:Backup_Android/app --include '*/' --include '*.apk' --include 'lib/**' --exclude '*' --stats 30s
```

Filtro equivalente: dentro de `/data/app/<hash>/<pkg>-<hash>/` copiar `base.apk`, `split*.apk` y
`lib/`, y **excluir `oat/` y `*.dm`** (7,69 GiB que se regeneran).

**Restauración (medido: 2.776 `.apk` en 1.342 carpetas ⇒ la mayoría son *splits*):**
instalar con `pm install-multiple -r <base.apk> <split_*.apk…>`; con `pm install` a secas
falla. Un APK con firma distinta de la del sistema ⇒ `INSTALL_FAILED_UPDATE_INCOMPATIBLE`
⇒ hay que desinstalar antes (y con eso se pierde el dato de F3: **orden: F3 antes de desinstalar**).

### FASE 3 — Los datos de las apps (F3: 48.243.910 KB = 46,01 GiB, ~45,0 h)

```
rclone copy /data/user/0 gdrive:Backup_Android/user0 --exclude '**/cache/**' --exclude '**/code_cache/**' \
  --exclude '**/noBackup/**' --exclude '**/no_backup/**' --exclude '**/thumbnails/**' \
  --exclude '**/Service Worker/**' --exclude '**/GPUCache/**' --stats 30s
```

Eso quita **2.734.501 KB (2,61 GiB)** de cachés regenerables (medido).

**Restauración (orden obligatorio):**

1. `pm install-multiple` de F2 para que el paquete exista y tenga uid.
2. `adb shell` con root → copiar el árbol de vuelta a `/data/user/0/<pkg>`.
3. `chown -R <uid>:<gid> /data/user/0/<pkg>` y `restorecon -RF` (el contexto SELinux lo
   gobierna; sin `restorecon` la app no abre sus bases de datos).
4. `am force-stop <pkg>` y abrir la app.

### FASE 4 — Descargas (F4: 36.184.304 KB = 34,51 GiB, ~33,8 h)

**Ya está en marcha** (proceso vivo). Lo único que hay que decidir es si el `rm -rf` del loop
se mantiene. Si Drive es el destino único, el `rm -rf` es correcto (es lo que ha freeingado
`YTDLnis`); si se quiere **copia doble**, hay que desactivar ese `rm` **antes** de que el loop
llegue a `1DMP`, `Snatcher-Collection` y `Ordenado` (34,51 GiB en juego).

**Medición relevante:** la deduplicación ya la hizo el usuario (876 pares idénticos detectados,
3.511 líneas de undo). Los ficheros `luupatch.7z.t0.00{1..4}` **NO** son duplicados
(4 SHA-256 distintos: son trozos). No volver a "limpiar" eso.

### FASE 5 — `/sdcard/Android/data` de terceros (F5: 12.238.399 KB = 11,67 GiB, ~11,4 h)

Prioridad dentro de la fase, por valor de:
`com.revanced.net.revancedmanager` (1,27 GiB, APKs parcheados) > `ru.drjmwdme.cgwbfzijr`
(0,42 GiB, MarinaPatcher) > `com.deniscerri.ytdl` (0,35 GiB) > `org.woheller69.whisper`
(0,34 GiB, modelos) > `zed.rainxch.githubstore` (7,42 GiB, reconstruible re-clonando).

### FASE 6 — Root y perfiles (F6: 3.199.358 KB = 3,05 GiB, ~3,0 h)

`/data/adb` (módulos Magisk + LSPosed), `/data/user_de/0` (perfil secundario) y misceláneo.
Va al final porque es lo más fácil de re-crear, pero **los 1.331.199 KB de módulos Magisk
incluyen customizaciones que no están en ningún repositorio** ⇒ hay que copiarlos sí o sí.

---

## 3. Presupuesto de espacio y tiempo (todo medido)

| | Medido |
|---|---|
| Libre en el móvil | **10.732.152 KB (10,23 GiB)** |
| Etapa local posible | **1 sola tanda de ≤ ~9 GiB** (dejando aire) |
| Etapa local **recomendada** | **0** — subir en streaming con `rclone copy` |
| Libre en Drive | **3,751 TiB** |
| Necesario en Drive | **196.08 GiB** |
| Occupación de la cuota libre | **0,50 %** |
| Tiempo de subida | **191.9 h = 8.0 días** a 304.742 B/s |
| Tiempo de subida si el `check` repite la lectura | ×~1,5–2 (el `check --one-way` relee origen y destino) |
| Tiempo de **bajada** (restaurar) | **10.3 h** a 5.668.249 B/s (medido) |

**El cuello de botella no es Drive: es la subida.** La bajada desde Drive va a 5,41 MiB/s
(medido), 18× más rápido que la subida (297,6 KiB/s). Subir a 5 MiB/s lo dejaría en ~11 h.

---

## 3-bis. Cómo encaja con el motor de backup que la flota YA está construyendo

Este plan **no inventa una herramienta nueva**: `rust-core/src/bin/md-backup.rs` (y
`md-backup-perfil.rs`) ya existen en el repo y ya exponen exactamente las operaciones que
hacen falta. La fases de abajo son *el qué*; el motor es *el cómo*.

| Fase de este plan | Comando del motor (`md-backup`) | Perfíl de `state/backup_target.json` |
|---|---|---|
| F0 · manifiesto | `md-backup inventory --json` (contraste contra los 1.721 TSV medidos aquí) | — |
| F2 · APKs + F3 · datos | `md-backup create --out <set> --password … --max-bytes … --segment-bytes …` | **P0-apps** |
| F3 · restaurar | `md-backup restore <set> --package <pkg> --install-apk --confirm` (verifica SHA-256 antes de escribir, y arregla el `ownership`) | **P0-apps** |
| F1 + F5 · multimedia y `Android/data` | `rclone copy` (volumen, no va en un contenedor por app) | **P2-sdcard** |
| F6 · root, perfiles, navegadores | `md-backup-perfil` | **P1-perfil** |
| — | (fuera de este encargo) | **P3-raw** (volcado RAW de particiones) |

**El destino ya está decidido en el repo**: `state/backup_target.json` fija la carpeta **privada**
de Drive `megadaemon-backup` (`drive_folder_id 15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0`), y
`md-backup upload --folder <id>` sube ahí. El repo es **público**: los datos, los certificados y
los cookies van **sólo** a esa carpeta, nunca al repo.

**Son dos destinos de Drive distintos, y hay que no confundirlos:**

| Destino | Qué contiene | Estado medido |
|---|---|---|
| `gdrive:Backup_Android/` | el offload del loop vivo (Download, grandes) | **62,863 GiB**, 11.396 objetos |
| `gdrive:megadaemon-backup/` (id `15FRl…`) | destino **privado** del motor `md-backup` | recién creado (aún sin objetos medidos) |

**Cifrado:** el motor ya trae `backup_crypto` (KDF + AEGIS-256, el mismo algoritmo que usa Swift Backup,
ver `docs/BACKUP_FORMATS_CATALOG.md` §1.1) ⇒ **F3 no debe subirse en claro**, ni a la carpeta
privada ni a la otra.

---

## 4. Por qué esto es «mucho mejor que Swift Backup y Lucky Patcher» (con números medidos)

| | Swift Backup (medido en el móvil) | Lucky Patcher | Este plan |
|---|---|---|---|
| Espacio en el móvil | `org.swiftapps.swiftbackup/databases` = **703.072 KB (687 MiB)** | **NO VERIFICADO** (no instalado / no medido) | **0 KB** (streaming) |
| Qué cubre | sólo lo que el usuario marque | parcheo, no respaldo | **196,08 GiB, las 1.719 apps, todos los datos** |
| Verificación | no medida | n/a | `rclone check --one-way` real ya ejecutada con éxito (34 ficheros, 0 diferencias) |
| Datos de sistema | no | no | **457 apps del sistema** con ruta y tamaño, incl. 80 actualizadas |
| Copia en la nube | n/a | n/a | **62,863 GiB ya en Drive**, 3,751 TiB libres |

Swift Backup está instalado y ocupa 687 MiB con **sus** respaldos: la razón de su límite es
que trabaja **en el móvil**, y el móvil sólo tiene 10,23 GiB libres. Cualquier respaldador que
escriba en local tiene ese techo. Éste no.

---

## 5. Riesgos medidos

| # | Riesgo | Evidencia medida | Mitigación |
|---|---|---|---|
| 1 | El `client_id` compartido de rclone se retira en 2026 | aviso de rclone en cada invocación | **Fase 0.2**: client_id propio ANTES de subir 196 GiB |
| 2 | El loop vivo borra el origen tras verificar | `rm -rf "$SRC"` en el comando del proceso 18952 | decidir ANTES de que llegue a `Ordenado` (18,09 GiB) |
| 3 | La subida es 18× más lenta que la bajada | 297,6 KiB/s vs 5,41 MiB/s | empezar por F1 (24,47 GiB) y luego el resto |
| 4 | Restaurar APKs con splits falla con `pm install` | 2.776 `.apk` en 1.342 carpetas | `pm install-multiple -r` |
| 5 | Restaurar sin `chown`/`restorecon` deja apps rotas | 1.590 uids distintos | guardar el mapa de uids en Fase 0.4 |
| 6 | F3 después de desinstalar pierde los datos | orden natural de las fases | **F3 antes de cualquier desinstalación** |
| 7 | El superloop puede volver a dispararse | `interval_sec=300`, `enabled=true`, 72 ejecuciones de media 365,3 s | el plan no depende de él; si vuelve, medir su CPU en un ciclo real (§4 del inventario) |
| 8 | `/data/user/0` filtrar credenciales a Drive | `jetski-standalone-oauth-token`, `magisk.db`, cookies de WebView | cifrar antes de subir (Fase 0.3) |

---

## 6. Lo que NO está verificado en este plan

* **La tasa de subida aislada.** La única medición es el promedio de una invocación `rclone`
  real en curso (304.742 B/s, incluye listado y `check`). Las horas de las fases son
  **proyecciones sobre esa tasa medida**, no hechos.
* **La tasa de subida con un `client_id` propio** (probablemente mejor; no medido).
* **La duración real de la copia de `Carpetas`** (estaba al 91 % al terminar la medición).
* **El coste de CPU de un ciclo del superloop** (no se ejecutó: muta el governor del sistema).
* **Que el conjunto de 1.719 apps se restaure bien.** No se hizo ninguna prueba de restauración.
* **Si los 687 MiB de la base de datos de Swift Backup son respaldos válidos** (no se abrió).
* **El estado en la nube de Telegram y Signal** (en disco sólo hay 665 KB).

