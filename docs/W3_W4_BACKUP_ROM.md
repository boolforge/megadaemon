# W3 · Backup ultra completo · W4 · Migración de ROM

> Dueño: agente General (sesión `79abadea-4f74-4211-b805-9c9a5e976dd3`).
> Ficheros propios: `rust-core/src/backup_vault.rs`, `rust-core/src/backup_full.rs`,
> `rust-core/src/rom_migration.rs`, `rust-core/tests/backup_full_smoke.rs`,
> `rust-core/tests/rom_migration_smoke.rs`, `rust-core/examples/rom_plan.rs`,
> `deploy/rom_migrate.sh`, `deploy/backup_phone.sh`.
>
> Todo lo que hay aquí está **medido** en el S21 Ultra o es una regla que
> aborta cuando no se puede comprobar. Nada de "fantasmadas": lo que no se
> puede leer sin hook, o no existe en el firmware, se declara explícitamente.

---

## 1. W3 · Las cuatro capas

| Nivel | Qué es | Zona | Se replica a las instancias |
|---|---|---|---|
| **L0** volátil | `state/` (tablón, journal, sync, audit), `board.sqlite`, config de la UI | pública | sí |
| **L1** ROM | boot/init/etc de fábrica, particiones de sólo lectura, **las 7 particiones del mundo seguro** | pública | sí |
| **L2** datos de usuario | DCIM, Downloads, Documents, `/sdcard`, lo recuperable de apps | pública | sí |
| **L3** credenciales | tokens de sesión, material PKCS#11, claves de firma | **privada** | **nunca** |

Los secretos de L0 **no** viajan en claro: viven en `md-secrets/` (0600) o
dentro de la bóveda cifrada.

### La línea roja (zona pública vs zona privada)

`backup_full::classify_zone()` decide la zona de cada ruta. **Lo privado
manda sobre lo público**: si una ruta contiene material de clave
(`private`, `privkey`, `nsec`, `seed`, `keystore2`, `pkcs11`, `token`, `.key`,
`.p12`, `.jks`, `auth_key`…) es privada aunque esté dentro de `public/`.

`backup_full::build_pack()` **excluye** la zona privada del plan y deja
constancia en `excluded_private`. El nivel L3 es privado entero: su plan
sirve para cifrar, no para subir en claro.

Esto es lo que hace posible **compartir los archivos de firma sincronizada a
todas las instancias**: lo público (certificados, CMS, PDF firmados) se
replica con el SHA-256 verificado en el destino; lo privado no sale.

### Lo que NO es recuperable (y hay que decirlo)

`backup_full::not_recoverable()` lo declara con motivo y con **qué se salva**:

- WhatsApp (`msgstore.db`): cifrado con clave del Android Keystore
  hardware-backed ⇒ sólo metadata y multimedia; el texto exige hook.
- Telegram (`tdata/`): AES-IGE atado al `auth_key`.
- Signal: la base local sí va cifrada; el `.backup` oficial sí es recuperable
  porque usa la contraseña de backup del usuario.
- `keystore2` / AndroidKeyStore: hardware-backed, no extraíble. **Es el
  límite duro del backup de credenciales.**
- Tokens PKCS#11 y claves privadas: por definición no salen del dispositivo.

### Streaming y restore probado

- `backup_vault::seal_dir_streaming()` sella **sin cargar el fichero entero**
  en memoria: dos pasadas sobre el origen (hash + cifrado), chunk a chunk de
  1 MiB, escribiendo el mismo formato `MDV1` que `seal_with_iterations()`
  (las bóvedas son intercambiables).
- `backup_vault::open_streaming()` restaura chunk a chunk directamente al
  destino, con la memoria acotada a un chunk y el SHA-256 comprobado por
  fichero contra el manifiesto.
- `backup_vault::restore_verify()` **relee del destino** los ficheros que se
  le pidan (las 3 lecturas reales mínimas) y compara con el manifiesto.
  **Un backup sin restore probado no es un backup.**
- `backup_vault::drive_chunk_plan()` + `drive_chunk_command()`: subida a
  Drive **por rangos**, con SHA-256 por trozo, pipeando con `dd` → `curl`.
  **Ninguna copia local intermedia** (el móvil tiene 5,9 GB libres y el pack
  son decenas de GB).
- `backup_vault::derive_hardware_key()`: deriva la clave de L3 con PBKDF2
  sobre un secreto ligado al hardware que obtiene el llamante **en el
  dispositivo**. El módulo sólo deriva: nunca lee, guarda ni transmite el
  secreto.

`deploy/backup_phone.sh` es el driver: genera el manifiesto (SHA-256 por
fichero y por trozo), sube por streaming y con `--verify` relee de Drive y
compara los hashes.

---

## 2. W4 · Migración de ROM (pre-flight que aborta)

### Lo que se midió en el móvil

- `flash.locked=1` **y** AVB `green` ⇒ **el bootloader está CERRADO**. Con
  FMM Lock el unlock va por manid y garantía; la única vía alternativa es la
  de servicio (`try jtag unlock`). Por eso el pre-flight **pregunta si el
  unlock es viable** en vez de asumirlo.
- La cadena AVB de este firmware tiene **2 niveles, no 3**: `init_boot`,
  `vbmeta_vendor` y `sec` **no existen**. `EFS_NON_EXISTENT` los lista para
  que nadie los prometa ni los respalde.
- La `boot.img` reempaquetada de la VN-ROM con
  `androidboot.selinux=permissive` **no es stock** ⇒ el pre-flight aborta.
- CSC real: **AMO**, parche 2024-06-01, Android 14. El "latest" de Samsung no
  vale si cambia de CSC.

### Los 15 checks del pre-flight

`unlock_viability`, `frp`, `omi`, `battery` (≥80 %), `temperature` (0–40 °C),
`space`, `odin`, `rom_checksums`, `csc_coherence`, `patch_not_older`,
`bootloader_version`, `bootimg_stock`, `pit_coherence`, `efs_partitions`,
`android_version` (aviso).

Cualquier **blocker** que falle ⇒ `report.abort == true` ⇒ no se flashea.

### Selección de ROM

`select_rom()` con criterios explícitos (`SelectionCriteria`): mismo CSC,
parche ≥ el del móvil, `boot.img` stock, PIT coherente; a igualdad de parche
gana la misma región de módem. **Cada descarte queda justificado** en
`rejected`.

### Flash guardado en 4 pasos

| Paso | Escribe | Guarda |
|---|---|---|
| 1 · WIPE DATA | HOME_CSC, CSC | exige **copia de EFS verificada**; sin ella el paso queda bloqueado |
| 2 · BOOTLOADER | BOOTLOADER | hash leído del móvil antes y después |
| 3 · AP + CP + CSC | AP, CP, CSC | el CSC tiene que ser el real del móvil |
| 4 · HOME_CSC | HOME_CSC | hash antes y después |

### Anti-brick

Las **7 particiones reales** del mundo seguro: `sec_efs`, `keystorage`,
`keyrefuge`, `uh`, `uhcfg`, `spu`, `prism`. `efs_backup_plan()` las exige
todas, avisa de las que falten y trae el **plan de recuperación** (TWRP por
sideload → `heimdall` → `fastboot`). "Erase Download" **sin copia verificada
está PROHIBIDO**: el paso queda bloqueado y el plan aborta.

### Post-flash

Primer arranque (`boot_completed`), re-espera del root, reinstalación de los
módulos Magisk y **re-ejecución del inventario W2** con comparación
antes/después (`compare_inventories`).

### Dry-run primero

`rust-core/examples/rom_plan.rs` genera el plan (JSON o texto) desde los
hechos medidos; `deploy/rom_migrate.sh` es el driver:

```sh
# 1. Medir y planificar (NO escribe nada)
deploy/rom_migrate.sh --rom-dir /rom --rom-manifest /rom/rom.json --text

# 2. Copia del mundo seguro con SHA-256, verificada contra el propio móvil
deploy/rom_migrate.sh --rom-dir /rom --rom-manifest /rom/rom.json \
    --efs-dir ~/efs-backup

# 3. Flash (con una persona delante)
deploy/rom_migrate.sh --rom-dir /rom --rom-manifest /rom/rom.json \
    --efs-dir ~/efs-backup --confirm --stage bootloader
```

Reglas del guion: por defecto es dry-run; `--confirm` **y** TTY son
obligatorios; "Erase Download" exige además `--wipe-confirm` **y** la copia
de EFS verificada; nunca escribe directamente sobre un bloque de particiones.

---

## 3. Qué falta para que esto entre en el binario

`src/lib.rs` es intocable sin acuerdo, así que los módulos nuevos se prueban
con el arnés `#[path]` (igual que `tests/signing_vault.rs`). Para que el
daemon los use hacen falta estas dos líneas en `rust-core/src/lib.rs`:

```rust
pub mod backup_full;
pub mod rom_migration;
```

`backup_vault` ya está declarada. Mientras tanto:

```sh
cargo test --release --test rom_migration_smoke -j 1
cargo test --release --test backup_full_smoke -j 1
cargo test --release --lib backup_vault -j 1
```

## 4. Trampa del sandbox

El repo está en **NFS**: cargo puede **reutilizar artefactos viejos por
mtime** y devolver un "verde" de hace tres commits (tecla: si un cambio "no
hace nada", `touch src/<fichero>` + rebuild, o `cargo clean -p
megadaemon-core`, que no borra las dependencias). Con `CARGO_HOME` y
`CARGO_TARGET_DIR` en disco local (`/run/agent-runtime/md/...`), `-j 1`,
`LTO=false` y `codegen-units=16` el build entra en los 3 GB de RAM.
