# Verificación de firma electrónica sobre artefacto REAL del Samsung S21 Ultra

**Fecha:** 2026-11-20 · **Autor:** agente General (nodo `cloud-sandbox`) · **Motor:** `rust-core/src/signing.rs` (Rust puro, sin JVM)

Este documento deja constancia de una firma **real** de un artefacto **real** extraído del
Samsung SM-G998B (Exynos 2100, Android 14) conectado al enjambre, y de la salida **real** del
verificador externo (`openssl cms -verify`).  No hay fixtures: el contenido firmado es el firmware
NPU del propio dispositivo.

---

## 1. El artefacto

| Dato | Valor |
|---|---|
| Ruta en el dispositivo | `/vendor/firmware/NPU.bin` |
| Tamaño | 1.506.272 bytes |
| sha256 (medido **en el móvil**) | `1a17e3b3a41b2da83618c899517086f056d05fe9628dc44d7f4d179c1aad22ba` |
| sha256 (medido **en el sandbox**, tras el trasvase) | `1a17e3b3a41b2da83618c899517086f056d05fe9628dc44d7f4d179c1aad22ba` |
| ¿Coincide? | **Sí** ⇒ el trasvase por el canal MCP es íntegro |

Cómo se midió en el móvil (canal MCP, tool `root_exec`):

```
$ ls -la /vendor/firmware/NPU.bin
-rw-r--r-- 1 root root 1506272 2008-12-31 16:00 /vendor/firmware/NPU.bin
$ sha256sum /vendor/firmware/NPU.bin
1a17e3b3a41b2da83618c899517086f056d05fe9628dc44d7f4d179c1aad22ba  /vendor/firmware/NPU.bin
```

El trasvase se hizo en 4 trozos de 393.216 bytes (la respuesta del bridge MCP está capada a
~384 KB) con `head -c`/`tail -c` en el propio móvil, y se reensambló en el sandbox; la
**igualdad de sha256 es la prueba de integridad** del trasvase.

## 2. Las tres firmas y su verificación externa

El motor `src/signing.rs` firma con una clave RSA-2048 de software (la clave de prueba embebida en
el propio módulo, `TEST_RSA_KEY_PEM`; el sandbox **no** tiene token PKCS#11/OpenSC — ver §4).

| Artefacto | Qué es |
|---|---|
| `NPU.bin.cms.der` | CMS/CAdES **con contenido adjunto** sobre el firmware (1.507.966 bytes) |
| `NPU.bin.detached.cms.der` | CMS/CAdES **desprendido** (el perfil que usa PAdES) (1.678 bytes) |
| `NPU.bin.pades.pdf` | PDF con **PAdES-B-B** (`/SubFilter /ETSI.CAdES.detached`) que certifica el sha256 del firmware |

Salida **real** del verificador externo (OpenSSL 3.0.20), sin filtros:

```
$ sha256sum NPU.bin
1a17e3b3a41b2da83618c899517086f056d05fe9628dc44d7f4d179c1aad22ba  NPU.bin

$ openssl cms -verify -inform DER -in NPU.bin.cms.der -content NPU.bin -binary -CAfile NPU.bin.cert.pem -out /dev/null
CMS Verification successful
exit=0

$ openssl cms -verify -inform DER -in NPU.bin.detached.cms.der -content NPU.bin -binary -CAfile NPU.bin.cert.pem -out /dev/null
CMS Verification successful
exit=0

$ openssl cms -verify -inform DER -in NPU.bin.pdf.cms.der -content NPU.bin.pdf_signed_bytes.bin -binary -CAfile NPU.bin.cert.pem -out /dev/null
CMS Verification successful
exit=0
```

Atributos CAdES presentes en el CMS (verificado con `openssl cms -cmsout -print`):

```
      eContentType: pkcs7-data (1.2.840.113549.1.7.1)
            object: contentType (1.2.840.113549.1.9.3)
            object: signingTime (1.2.840.113549.1.9.5)
            object: id-smime-aa-ets-commitmentType (1.2.840.113549.1.9.16.2.16)
            object: messageDigest (1.2.840.113549.1.9.4)
            object: id-smime-aa-signingCertificateV2 (1.2.840.113549.1.9.16.2.47)
```

El PDF firmado también pasa las herramientas de PDF del sistema:

```
$ qpdf --check NPU.bin.pades.pdf
No syntax or stream encoding errors found; the file may still contain
errors that qpdf cannot detect
$ pdfinfo NPU.bin.pades.pdf
Form:            AcroForm
Pages:           1
```

y `pypdf` lee el campo `Signature1` con `/FT /Sig`, `/SubFilter /ETSI.CAdES.detached`,
`/ByteRange [0, 991, 33761, 224]` y extrae el CMS, que `openssl cms -verify` valida sobre los bytes
del `ByteRange`.

## 3. Qué queda SIN implementar (dicho sin adornos)

* **XAdES-T / XAdES-LT**: no implementados (exigen TSA y OCSP/CRL). Tampoco DSS/LTV.
* **PKCS#11 real**: la capa está escrita (dlopen/dlsym, sesión, login, `C_FindObjects`, `C_Sign`
  con `CKM_RSA_PKCS` y `CKM_ECDSA`) pero **no se ha podido ejercitar contra un token real**: el
  sandbox no tiene OpenSC ni tarjeta.  La firma de este documento usa la **impl software**.
* **Cadena de confianza / revocación / políticas**: `verify_cms` valida digest y firma contra el
  certificado que se le pase; no valida la cadena ni consulta CRL/OCSP.
* El PDF firmado **no** lleva `DocMDP` ni flujo de apariencia (SAV).

## 4. keystore2 / PKCS#11 en el móvil: qué se puede y qué no (evidencia medida)

Medido en el SM-G998B (Android 14, SELinux **Enforcing**, Magisk 27.2-kitsune) por el canal MCP:

```
=== keystore2: servicio y HAL ===
-rwxr-xr-x 1 root shell   11832 android.hardware.keymaster@4.0-service
-rwxr-xr-x 1 root shell  111392 android.hardware.keymaster@4.0_strongbox-service
keystore2                   keystore2 /data/misc/keystore
android.hardware.keymaster@4.0-service android.hardware.keymaster@4.0-service
android.hardware.keymaster@4.0_strongbox-service android.hardware.keymaster@4.0_strongbox-service
vendor.samsung.hardware.security.fkeymaster-service vendor.samsung.hardware.security.fkeymaster-service
[init.svc.vendor.fkeymaster-default]: [running]
[init.svc.vendor.keymaster-4-0]: [running]
[init.svc.vendor.keymaster-4-0_strongbox]: [running]

=== PKCS#11 en todo el sistema de ficheros legible ===
find / -xdev -iname "*pkcs11*" 2>/dev/null | head -10
(vacío ⇒ NO hay ningún módulo PKCS#11)

=== termux: herramientas pkcs11 disponibles ===
ls /data/data/com.termux/files/usr/bin | grep -i -E "pkcs11|opensc|softhsm"
(vacío)
```

**Conclusión honesta:** el keystore2 de Android **no expone una interfaz PKCS#11**.  No hay
módulo PKCS#11 en el dispositivo, ni OpenSC, ni SoftHSM, ni en Termux.  Lo que hay es:

| Pieza | Estado | Implicación para firmar |
|---|---|---|
| `keystore2` (servicio) | vivo | las claves del usuario se generan y viven **dentro del TEE** |
| Keymaster 4.0 HAL | vivo | es la puerta al hardware, pero **no** es PKCS#11 |
| `keymaster@4.0_strongbox-service` | vivo | es el **stub** del HAL: este móvil **no** tiene Secure Element físico ⇒ "StrongBox" no aporta aislamiento extra |
| `vendor.samsung.hardware.security.fkeymaster` | vivo | HAL de Samsung para huellas |
| **PKCS#11** | **ausente** | **no hay forma de firmar desde el TEE con la API PKCS#11** |

Vías reales para firmar en el TEE, ninguna de las cuales es PKCS#11:

1. **Cliente nativo de keystore2** (`libkeystore2_crypto.so` / `android.system.keystore2-V3-ndk.so`,
   presentes en `/system/lib64`) con una clave creada con `setKeyEntry` bajo un UID propio.
   Es la vía correcta, pero es **AIDL/ndk, no PKCS#11**, y por tanto **NO** la cubre
   `src/signing.rs` tal como está.
2. **Shim PKCS#11 propio** que hable con keystore2 por AIDL y exponga `C_Sign`: no existe ninguno
   publicado para Android; sería trabajo nuevo (y el HAL de keystore2 no es un token genérico).
3. **DNIe** (tarjeta física): ahí sí aplica mi capa PKCS#11 con OpenSC **en el PC**, no en el
   teléfono; el DNIe se apoya en OpenSC + una tarjeta, no en keystore2.

⇒ **Conclusión operativa:** el motor `src/signing.rs` (PKCS#11) sirve para el DNIe y para
tarjetas/HSM del PC o del móvil con OpenSC instalado; **para el keystore2 del propio S21 hace
falta un proveedor nuevo** (un `SignatureProvider` que hable AIDL con `keystore2`), no OpenSC.

## 5. Reclasificación del vault (`src/signing_vault.rs`)

Regla del enjambre: *lo material privado nunca sale del dispositivo; lo público (hashes,
manifiestos, firmas) sí, y se replica con `sha256` en el MANIFEST.*

Estado medido del código:

| Pieza | Comportamiento | Veredicto |
|---|---|---|
| `Zone::Public` / `Zone::Private` | dos zonas en disco (`vault/public/`, `vault/private/`) | ✅ |
| `export_public()` | exporta **sólo** las entradas `zone == "public"`, con `name`, `sha256`, `size_bytes`, `mime`, `origin`, `ts` y `content_b64` | ✅ el contenido privado no sale |
| `import_public()` | importa **sólo** a la zona pública y **verifica el sha256 antes de escribir** (fail-closed por entrada) | ✅ |
| `get()` | verifica el sha256 contra el manifiesto antes de devolver | ✅ |
| `put()` | escritura atómica (tmp + `rename`), manifestando `sha256` | ✅ |
| **`write_manifest()`** | escribe **todas** las entradas (públicas **y privadas**) en un único `manifest.ndjson` | ⚠️ **hueco** |

**El hueco concreto:** el fichero `manifest.ndjson` mezcla ambas zonas, así que las **entradas
privadas** quedan descritas en el mismo fichero que el que se replica: su `name`, `sha256`,
`size_bytes`, `mime` y `origin` travels con el manifiesto.  El *contenido* privado no sale (el
export filtra por zona ✅), pero **la metadata privada sí viaja** si el manifiesto se replica tal
cual.

Arreglo propuesto (2 líneas de verdad, para el dueño del módulo):

* Separar el manifiesto: `manifest.ndjson` = **sólo entradas públicas** (es el que se replica) y
  `manifest.private.ndjson` = entradas privadas (local, nunca exportado); o
* si se quiere un único fichero, marcar cada entrada con `"replicable": true|false` y que **el
  replicador filtre por ese flag** antes de subirlo (igual que hace `export_public`).

Con eso, la regla queda: **contenido privado + metadata privada = local**; **contenido público +
hashes + firmas = replicados**, y el MANIFEST público lleva el `sha256` de cada pieza.

## 6. Reparto de ficheros

| Fichero | Dónde | Qué es |
|---|---|---|
| `NPU.bin` | `/workspace/sign_verify/` (fuera del repo) | el firmware real, pulled del S21 |
| `NPU.bin.cms.der` | ídem | CMS/CAdES adjunto |
| `NPU.bin.detached.cms.der` | ídem | CMS/CAdES desprendido |
| `NPU.bin.pades.pdf` | ídem | PDF con PAdES-B-B |
| `NPU.bin.pdf_signed_bytes.bin` | ídem | los bytes firmados (los dos tramos del `ByteRange`) |
| `NPU.bin.cert.pem` | ídem | certificado del firmante (clave de prueba del módulo) |

Los binarios **no** se commitean (1,5 MB + derivados); se regeneran con el módulo.
