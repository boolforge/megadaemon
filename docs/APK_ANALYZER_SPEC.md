# Especificación del escáner de APK (`apk_inventory`)

> Módulo: `rust-core/src/apk_inventory.rs` · Motor de veredictos: `rust-core/src/app_intel.rs`
> Versión del documento: 1.0 (2026-11-21) · Idioma del documento: español, identificadores en inglés.

## 1. Qué es y qué NO es

`apk_inventory` dejó de ser un *listado* de APK instaladas: ahora es un **escáner**.  Por cada
APK mide ofuscación, empaquetador, frameworks de parche, permisos por riesgo, endpoints,
librerías nativas, firma y componentes exportados sin permiso; y por cada app instalada calcula
un **ranking de riesgo** y un **veredicto** accionable.

| Lo que NO hace | Por qué |
|---|---|
| No reimplementa el ZIP | Se reutiliza el crate `zip` 2.2 (features `deflate`) sobre un lector `Read + Seek` propio (`DeviceFile`) |
| No reimplementa el desensamblador ni el análisis de DEX | Se **consume** la API pública de `crate::dex_decompile` (`parse_dex`, `analyze_obfuscation`) |
| No reimplementa la detección de toolkits de parcheo | Se **consume** `crate::app_intel::detect_patcher_toolkit_detail` |
| No inventa datos | Si una medición falla, la clave va vacía o `PENDIENTE`, nunca "probablemente" |
| No entra en panic | Todas las funciones devuelven `serde_json::Value` y tragan los errores |

Dependencias **nuevas**: ninguna.  Todo lo que hace falta ya estaba en `Cargo.toml`
(`zip 2.2`, `flate2`, `sha2`, `serde_json`).  crates.io no es visible desde el sandbox, así que
`Cargo.toml` **no se ha tocado**.

## 2. API pública (contrato)

Las dos funciones que el resto del código llama **no cambian de firma**:

| Función | Firma | Estado |
|---|---|---|
| `inventory` | `inventory(limit: usize) -> serde_json::Value` | **intacta** (la usan `main.rs`, `mcp_ext.rs`, `agent_core.rs`, `repo_updates.rs`, `sshd.rs`) |
| `analyze_apk` | `analyze_apk(path: &str) -> serde_json::Value` | **intacta**; las claves nuevas se **añaden** al mismo objeto, ninguna se renombra ni se borra |
| `dump_all` | `dump_all(out_dir: &str) -> serde_json::Value` | intacta |
| `drift` | `drift(known: &Value) -> serde_json::Value` | intacta |

Funciones **nuevas** (aditivas):

| Función | Qué hace | Coste en el dispositivo |
|---|---|---|
| `scan_installed(limit)` | Escanea las apps instaladas: rutas (`pm list packages -3 -f`), ficha por paquete (`dumpsys package`), manifiesto de cada APK y **ranking de riesgo** | 2 llamadas + 1 lectura de manifiesto por APK |
| `rank_risk(limit)` | La misma información, ordenada por riesgo descendente (acción `rank`) | idem |
| `verdict_installed(limit)` | Lo anterior + **veredicto** por app (`KEEP` / `REIMPLEMENTAR EN RUST` / `DESINSTALAR`) y módulo Rust destino (acción `verdict`) | idem |
| `parse_dumpsys_packages(stdout)` | **Función pura**: `dumpsys package` → fichas por app (testeable sin móvil) | 0 |
| `parse_package_paths(stdout)` | **Función pura**: `pm list packages -f` → `(paquete, ruta)` | 0 |
| `parse_installers(stdout)` | **Funión pura**: `pm list packages -i` → `(paquete, instalador)` | 0 |
| `score_installed_app(rec)` | **Función pura**: ficha de app → `{risk_score, risk_level, reasons[]}` | 0 |
| `permission_risk_class(perm)` | Clase de riesgo de un permiso | 0 |
| `permission_risk_groups(manifest)` | Permisos del manifiesto agrupados por riesgo | 0 |
| `exported_without_permission(manifest)` | Componentes `exported` sin permiso (hallazgo de seguridad) | 0 |

En `crate::app_intel`:

| Función | Qué hace |
|---|---|
| `verdict_for(record)` | Veredicto de una app a partir de su ficha medida + `VERDICT_CATALOG` |
| `verdict_summary(records)` | Resumen `{count, counts{}, packages[]}` |
| `catalog_entry(package)` | Ficha de catálogo (nombre, categoría, veredicto, módulo, motivo) |
| `is_system_app(apk_path)` | ¿Vive en la partición de sistema? (no se desinstala) |
| `VERDICT_CATALOG` | Catálogo curado de 71 apps: `(paquete, nombre, categoría, veredicto, módulo Rust, motivo)` |
| `RUST_TARGETS` | Tabla de módulos destino: `(id, categoría, módulo, qué hace)` |

## 3. Bloques que devuelve `analyze_apk(path)`

Además de las claves de siempre (`entries`, `dex_files`, `native_libs`, `abis`, `assets`,
`manifest`, `fingerprint`, `sha256`), el escáner añade:

| Clave | Contenido |
|---|---|
| `obfuscation` | Fingerprint de ofuscación (ver §4) |
| `packers` | Empaquetadores/proteores detectados (ver §5) |
| `patch_frameworks` | Frameworks de parche y de hook detectados (ver §5) |
| `patcher_toolkit` | Detalle del toolkit de parcheo (API de `app_intel`) |
| `permissions_risk` | Permisos agrupados por clase de riesgo (ver §6) |
| `endpoints` | URLs, dominios e IPs del DEX y de los assets (ver §7) |
| `native_libraries` | Una entrada por `.so`: ABI, tamaño, exports, `stripped`, máquina (ver §8) |
| `signature` | v1/v2/v3, esquemas, firmante y SHA-256 del certificado (ver §9) |
| `exported_without_permission` | Componentes exportados sin permiso (ver §10) |
| `security_findings` | Hallazgos del manifiesto con severidad (`debuggable`, `allowBackup`, cleartext…) |

### 3.1 Estructura del manifiesto (ampliada, retro-compatible)

`manifest_axml` **añade** claves; `manifest_canonical` (y por tanto `manifest_fingerprint`) **no
cambia**, así que las huellas guardadas siguen siendo comparables:

```jsonc
{
  "permissions": [...],              // uses-permission (igual que antes)
  "declared_permissions": [          // NUEVO: <permission> declarados por la app
    { "name": "com.mi.app.PRIVADO", "protection_level": "signature" }
  ],
  "meta_data": [                     // NUEVO: <meta-data> (sirve para xposedmodule…)
    { "name": "xposedmodule", "value": "true" }
  ],
  "application": {                   // NUEVO: atributos de <application>
    "debuggable": false, "allow_backup": true, "uses_cleartext_traffic": null,
    "test_only": false, "network_security_config": null, "has_code": true
  },
  "activities": [                    // cada componente añade:
    { "name": "...", "exported": true,
      "permission": null,            // NUEVO: android:permission
      "read_permission": null,       // NUEVO
      "write_permission": null,      // NUEVO
      "authorities": [],             // NUEVO
      "grant_uri_permissions": null, // NUEVO
      "intent_filters": [...] }
  ]
}
```

## 4. Fingerprint de ofuscación

Todo se mide sobre el DEX parseado con `crate::dex_decompile`:

| Métrica | Definición | Por qué importa |
|---|---|---|
| `classes_total` / `classes_short` | Clases del DEX / clases cuyo nombre simple tiene 1-2 caracteres | El renombrado aggressive es la huella del ofuscador |
| `classes_short_ratio` | `classes_short / classes_total` | Ratio comparable entre APK |
| `fields_short_ratio`, `methods_short_ratio` | Igual para campos y métodos | Un ratio alto en los tres = minificación |
| `encrypted_strings` / `encrypted_strings_ratio` | Strings que `analyze_obfuscation` marca como cifrados | Cifrado de strings (Stringer, ZKM) |
| `renamed_types`, `renamed_methods` | Lo que el detector de `dex_decompile` ya marca | Heredado del carril de MegaDaemon-RE |
| `control_flow_obfuscation` | Técnicas de ofuscación de flujo detectadas | ofuscación de control de flujo |
| `source_file_present` / `source_file_ratio` | Clases con atributo `SourceFile` | Si casi no hay, el build es minificado (R8) |
| `line_number_table_present` / `…_ratio` | Métodos con `debug_info_off != 0` (⇒ `LineNumberTable`) | Su ausencia en release es normal; su presencia en un APK "ofuscado a mano" es anómala |
| `score` / `level` | 0..100 y `nula`/`baja`/`media`/`alta` | Comparación entre APK |

**Límites declarados**:

- El score es una **heurística ponderada**, no una clasificación.  Un APK con ProGuard normal
  puede dar `baja`; un packer con el DEX cifrado da `nula` (el DEX real no está en
  `classes.dex`).  Por eso los packs se detectan **por marcador**, no por el score.
- `source_file_ratio < 0,05` con más de 50 clases ⇒ se suma +10 al score (típico R8).  No
  distingue R8 de ProGuard ni de un build con `strip`.

## 5. Empaquetadores y frameworks de parche

Dos tablas de marcadores literales (id, nombre, marcador).  El marcador se busca en el texto
minúsculas de: nombres de entrada del ZIP + strings del DEX + nombres de assets + `meta-data`.

**Empaquetadores / protectores** (`PACKER_MARKERS`):

| id | Marcadores |
|---|---|
| `dexguard` | `dexguard`, `com.guardsquare` |
| `r8` | `com.android.tools.r8` |
| `allatori` | `allatori` |
| `zkm` | `zelix`, `stringencryption` |
| `stringer` | `stringer` |
| `bangcle` / `secneo` | `bangcle`, `secshell`, `libdexhelper` |
| `jiagu` (360) | `libjiagu`, `360sec` |
| `legu` (Tencent) | `libshella`, `libshellx` |
| `baidu` | `libbaiduprotect` |
| `ijiami` | `ijiami`, `libexecmain` |
| `tencent` | `libtersafe` |
| `alibaba` | `libmobisec`, `libpreverify1` |
| `nagain` | `libnagain` |

**Frameworks de parche y de hook** (`PATCH_FRAMEWORK_MARKERS`):

| id | Marcadores | Ancla |
|---|---|---|
| `lspatch` | `lspatch` | APK parcheado; clase `com.lsposed.lspatch` |
| `knoxpatch` | `knoxpatch` | Parche con soporte Knox (Samsung) |
| `edxposed` | `edxposed` | Gestor del framework de hooks |
| `monopatch` | `monopatch` | Parche de un solo DEX |
| `trustme` | `trustme` | Módulo de confianza/permisos |
| `lsposed` | `lsposed` | Framework de hooks |
| `xposed` | `com.xposed`, `xposed_init` | `meta-data xposedmodule` + `assets/xposed_init` |
| `frida-gadget` | `frida-gadget`, `libfrida-gadget.so` | `.so` instrumentado en el APK |
| `frida-server` | `frida-server` | Binario de instrumentación |
| `sandhook`, `whale`, `yahfa`, `epic`, `pine` | idem | Hook en runtime |
| `substrate` | `com.saurik.substrate` | Hook clásico |
| `lucky-patcher` | `luckypatcher`, `com.chelpus` | Lucky Patcher reempaquetado (sube el APK a un tercero) |

**Límite**: un marcador prueba que el artefacto **está dentro** del APK, no que esté activo.
`patcher_toolkit` (de `app_intel`) añade el detalle de severidad del caso Lucky Patcher.

## 6. Permisos por riesgo

| Clase | Significado | Ejemplos |
|---|---|---|
| `peligroso` | Permiso de runtime con datos personales | `CAMERA`, `RECORD_AUDIO`, `ACCESS_FINE_LOCATION`, `READ_SMS`, `READ_CONTACTS`, `READ_EXTERNAL_STORAGE`, `READ_MEDIA_*` |
| `alto` | Poder sobre el sistema | `REQUEST_INSTALL_PACKAGES`, `SYSTEM_ALERT_WINDOW`, `MANAGE_EXTERNAL_STORAGE`, `QUERY_ALL_PACKAGES`, `CAPTURE_AUDIO_OUTPUT`, `BIND_ACCESSIBILITY_SERVICE` |
| `medio` | Red, arranque, primer plano | `INTERNET`, `ACCESS_NETWORK_STATE`, `FOREGROUND_SERVICE`, `RECEIVE_BOOT_COMPLETED` |
| `bajo` | El resto | — |

Salida:

```jsonc
"permissions_risk": {
  "total": 24,
  "counts": { "peligroso": 8, "alto": 0, "medio": 1, "bajo": 15 },
  "by_risk": { "peligroso": [...], "alto": [...], "medio": [...], "bajo": [...] }
}
```

## 7. Endpoints

- **URLs**: `crate::app_intel::extract_endpoints` (API pública ya existente) sobre el texto
  concatenado de los strings del DEX + los assets de texto + los `meta-data`.
- **Dominios**: host de cada URL + barrido de dominios sueltos con TLD conocido
  (`KNOWN_TLDS`), longitud ≤ 253 y ≤ 63 por etiqueta.
- **IPs**: cuádruplas IPv4 literales válidas (0-255 por octeto, sin digitos pegados).

**Límites**: heurística de texto.  Los dominios de los assets binarios (DEX empaquetado,
imágenes) **no** se ven; las IPs privadas (`10.x`, `192.168.x`) **sí** salen y hay que
filtrarlas antes deOX un "telemetría".

## 8. Librerías nativas

Por cada `lib/<abi>/<nombre>.so`:

| Clave | Origen |
|---|---|
| `abi`, `name`, `path` | Ruta dentro del ZIP |
| `size`, `compressed_size` | Cabecera del ZIP (`zip` crate) |
| `stripped` | El ELF **no** tiene sección `.symtab` |
| `exports` | Símbolos de `.dynsym` (`sh_size / sh_entsize`) |
| `has_debug` | Tiene `.debug_info` / `.debug_line` / `.zdebug_info` |
| `machine`, `elf_class`, `sections` | `e_machine` y cabecera ELF |

El parser ELF es propio (≈100 líneas) y **sólo** little-endian: es el único que usan las
`.so` de Android.  Se leen como mucho `MAX_SO_ANALYZED` (8) `.so` por APK; el resto se lista
sin abrir (`stripped`/`exports` quedan `null`).

## 9. Firma de la APK

| Clave | Significado |
|---|---|
| `v1` | Hay `META-INF/*.RSA|.DSA|.EC` (firma JAR) |
| `v2` / `v3` | Hay un **APK Signing Block** con el par 0x7109871a / 0xf05368c0 |
| `schemes` | Lista de esquemas detectados (`v2`, `v3`, `v3.1`) |
| `source` | De dónde salió el certificado (`v2`, `v3`, `v1`, `ninguna`) |
| `certificate.signer` | `CN=..., O=..., C=...` del firmante |
| `certificate.cert_sha256` | SHA-256 del X.509 completo (64 hex) |
| `certificate.cert_sha256_short` | Los 8 primeros (el "sha16" que va al tablón) |

Cómo se lee:

1. Se busca el **EOCD** al final del APK y se lee el offset del directorio central.
2. Justo antes está el *APK Signing Block*: magic `APK Sig Block 42` + tamaño de 64 bits.
3. Se recorren los pares `(len, id, value)`; `0x7109871a` = v2, `0xf05368c0` = v3.
4. Dentro del valor, el certificado X.509 se localiza recorriendo las sub-bloques con longitud
   `u32` (el formato de v2/v3) y validando la estructura DER del certificado.
5. Si no hay bloque de firma, se recurre a `META-INF/*.RSA` (PKCS#7) y se busca el X.509 dentro
   recorriendo el árbol DER.
6. El X.509 se parsea con un lector DER propio: `issuer` y `subject` (RDN con OID → nombre
   corto: CN, O, OU, C, L, ST, STREET, emailAddress).

**Límites**: no se valida la cadena de confianza (no hay `apksigner`): se identifica el
certificado, no se verifica que la firma sea válida.  El `cert_sha256` es el del X.509, que es
lo que Android expone en `PackageManager` (y por eso el inventario lo guarda truncado a 4
bytes).

## 10. Componentes exportados sin permiso

Un componente con `android:exported="true"` y **sin** `android:permission` (o, en un
`provider`, sin `readPermission`/`writePermission`) lo puede invocar **cualquier** app
instalada.  Es el hallazgo de seguridad más típico y el más accionable:

```jsonc
"exported_without_permission": [
  { "kind": "activity", "name": "com.ejemplo.A", "actions": ["android.intent.action.MAIN"],
    "authorities": [] }
]
```

`security_findings` lo recoge con severidad `alto`, junto con `debuggable`,
`allowBackup`, `usesCleartextTraffic` y `testOnly`.

## 11. Modelo de riesgo (`score_installed_app`)

Función ** pura**: la suma de pesos, con la lista de motivos que la explica.

| Motivo | Peso | Condición |
|---|---|---|
| `dangerous_permissions` | 3 por permiso (máx. 30) | Permiso de clase `peligroso` |
| `high_permissions` | 4 por permiso (máx. 20) | Permiso de clase `alto` |
| `exported_without_permission` | 6 por componente (máx. 18) | Componente exportado sin permiso |
| `sideloaded` | 4 | Sin instalador registrado (`null` o vacío) |
| `third_party_installer` | 2 | Instalada desde algo que no es la Play Store |
| `legacy_target_sdk` | 3 | `targetSdk < 26` (los permisos se conceden al instalar) |
| `too_many_permissions` | 2 | Más de 40 permisos pedidos |
| `disabled` | 1 | `pm` la tiene deshabilitada |
| `debuggable` | 3 | `android:debuggable=true` |
| `patch_framework` | 8 | Framework de parche/hook dentro del APK |
| `packer` | 5 | Empaquetador/protector dentro del APK |

Niveles: `critico` ≥ 25 · `alto` ≥ 15 · `medio` ≥ 8 · `bajo` > 0 · `sin_riesgo` = 0.

## 12. Motor de veredictos (`crate::app_intel`)

```
VERDICT_CATALOG (71 apps curadas)  ──►  veredicto + módulo Rust destino
                │
                ▼ (lo que no está en el catálogo)
reglas medidas: reempaquetado con subida a un tercero → DESINSTALAR
                relicto deshabilitado con ≥3 permisos peligrosos → DESINSTALAR
                targetSdk < 26 con ≥3 permisos peligrosos → DESINSTALAR
                app de sistema (/system, /product, /vendor, /apex, /prism) → KEEP
                material de RE (packer o framework) → KEEP
                resto → KEEP ("sin evidencia medida en contra")
```

El catálogo es la **fuente única**: el documento `docs/SOFTWARE_INVENTORY_VERDICT.md` se
genera con `tools/verdict_inventory.py` leyéndolo del propio `.rs`, así que el código y el
documento no pueden discrepar.

Módulos destino ya existentes en el repo: `recording_vault.rs`, `backup_formats.rs`,
`dex_decompile.rs`, `adb_client.rs`, `magisk_module.rs`, `bookmarks.rs`,
`antigravity_intel.rs`, `apk_inventory.rs`, `app_intel.rs`, `device_inventory.rs`,
`signing_vault.rs`, `root_stealth.rs`, `csc_verify.rs`, `messaging.rs`, `npu_cv.rs`,
`lab_kit.rs`.  Los marcados **(nuevo)** (`backup_engine.rs`) son la pieza que falta.

## 13. Presupuesto de carga en el móvil

El móvil va cargado (load 17,6) ⇒ el escáner es **acotado por diseño**:

| Paso | Coste |
|---|---|
| `pm list packages -3 -f` | 1 llamada |
| `dumpsys package` | 1 llamada (hasta 16 MiB de salida) |
| Manifiesto de cada APK | 1 apertura de ZIP + lectura de 1 entrada por APK (≈3 viajes por APK) |
| DEX | **no** se leen en `scan_installed` (sólo en `analyze_apk` de una APK concreta) |

Topes: `MAX_SCAN_APPS = 60` apps por barrido (o el `limit` que se pase, siempre ≤ 60),
`MAX_DEX_ANALYZED = 3`, `MAX_SO_ANALYZED = 8`, `MAX_ASSETS_READ = 24`.

`scan_installed` **no** calcula el hash de las APK ni lee los DEX: es un barrido de
*riesgo*, no de *contenido*.  El hash completo es trabajo de `inventory`.

## 14. Pruebas

Deterministas, sin red y sin dispositivo (`cargo test --lib`):

| Prueba | Qué fija |
|---|---|
| `los_permisos_se_agrupan_por_clase_de_riesgo` | La clasificación de permisos |
| `los_componentes_exportados_sin_permiso_se_detectan` | El hallazgo de seguridad |
| `el_escaner_de_una_apk_sintetica_devuelve_todos_los_bloques` | Que `analyze_apk` tiene los 7 bloques nuevos |
| `el_escaner_detecta_el_componente_exportado_en_una_apk_sintetica` | El caso real, con APK construida en el test |
| `el_parser_de_pm_list_packages_f_extrae_paquete_y_ruta` | El formato real de `pm` |
| `el_parser_de_instaladores_extrae_el_instalador` | El formato real de `pm -i` |
| `el_parser_de_dumpsys_extrae_la_measured_de_cada_paquete` | El formato real de `dumpsys package` |
| `la_puntuacion_de_riesgo_suma_los_motivos` | Que la suma de pesos cuadra con el score |
| `los_endpoints_salen_de_urls_dominios_e_ips` | La extracción de endpoints |
| `los_dominios_falsos_no_se_aceptan` | Los falsos positivos (versiones, TLD inventados) |
| `los_marcadores_de_pack_y_de_parche_se_detectan` | Las dos tablas de marcadores |
| `el_elf_se_lee_y_dice_si_esta_stripped` | El parser ELF (con un ELF64 mínimo construido en el test) |
| `el_veredicto_por_defecto_es_keep` / `el_veredicto_medido_saca_los_relictos_y_deja_el_sistema` | El motor de veredictos |

Las APK sintéticas se construyen con el crate `zip` (método `Stored`) ⇒ **no** hace falta red
ni el dispositivo.

```bash
CARGO_TARGET_DIR=/run/agent-runtime/md/target-apk \
CARGO_PROFILE_DEV_LTO=false CARGO_PROFILE_DEV_CODEGEN_UNITS=16 \
cargo test --lib -j 1 --offline apk_inventory app_intel
```

## 15. Lo que queda fuera (y a quién hay que pedirlo)

| Necesidad | Estado | Dónde va |
|---|---|---|
| Tool MCP `apk_inventory` con acciones `scan` / `rank` / `verdict` | **Las tres acciones existen en el módulo** (`scan_installed`, `rank_risk`, `verdict_installed`); falta el cableado en el dispatcher MCP | `rust-core/src/mcp_ext.rs` — **lo tiene cockpit/web**, no se toca aquí |
| Cifrado real de strings del DEX (descifrar, no sólo detectar) | Detecta el ratio; el descifrado es del carril de RE | `rust-core/src/deobfuscate.rs` / `dex_decompile.rs` — **MegaDaemon-RE** |
| Tamaño exacto por APK | El escáner lo tiene (`size` de la entrada del ZIP) pero el volcado W2 no lo guardaba | `scan_installed` lo rellena cuando vuelva el canal |
| Telemetría por app (quién llama a quién) | `endpoints` da los candidatos; falta correlación con el tráfico real | Requeriría captura de red en vivo |

## 16. Historial de cambios

| Fecha | Cambio |
|---|---|
| 2026-11-21 | v1.0: escáner completo (ofuscación, packs, parcheo, permisos, endpoints, `.so`, firma, exportados), `scan_installed`/`rank_risk`/`verdict_installed`, motor de veredictos en `app_intel`, catálogo de 71 apps y documento de veredicto |
