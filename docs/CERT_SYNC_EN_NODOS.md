# Sincronización de certificados entre nodos: despliegue, uso y revocación

> Documento operativo para el harness MegaDaemon (`rust-core`). Complementa
> `docs/CERTIFICADOS_Y_SEDES_ELECTRONICAS.md` (qué sistemas existen y qué se
> puede automatizar) con **cómo** se despliega un certificado del usuario en un
> nodo que lo necesita (por ejemplo, un nodo Linux en casa que hace trámites del
> usuario).
>
> Implementación de referencia: `rust-core/src/cert_vault.rs` (bóveda de
> certificados) y `rust-core/tests/cert_vault_smoke.rs` (arnés determinista).

---

## 0. La frontera, antes de nada

**Los certificados electrónicos son personales e intransferibles.** Un
certificado de persona física identifica a **una** persona ante las
Administraciones Públicas y su uso implica responsabilidad de esa
persona. Lo que el harness replica **no es el certificado de otra persona**
(si lo fuera, el harness sería un intermediario de suplantación), sino:

1. **Los certificados del propio usuario**, para que **sus** trámites se
   puedan hacer desde **sus** nodos (móvil, PC de casa, servidor propio).
2. **Las referencias a claves no exportables** (alias del Android Keystore,
   token PKCS#11, TPM): punteros, no material.
3. El **certificado público** de los certificados exportables (que es público
   por definición) y, **sólo con confirmación explícita**, el blob **cifrado**
   de la clave privada.

Todo lo que salga de un nodo y entre en otro pasa por:

* **consentimiento explícito** del usuario en el momento de la operación
  (`confirm: true`), y
* **traza de auditoría** (`cert_vault/audit.log`).

Y hay una frontera más, no negociable:

> **Los datos de salud son categoría especial (RGPD art. 9).** Aunque el
> certificado habilite a leer la historia clínica, **el certificado no da base
> jurídica** para tratar ese dato. Un nodo que lee datos de salud necesita su
> **propia base jurídica**, su **registro de actividad** y **minimización**;
> la clave privada no es el problema legal: lo es el dato.

---

## 1. Los nodos y qué credencial lleva cada uno

| Nodo | Credencial natural | ¿Clave exportable? | Por qué |
|---|---|---|---|
| **Móvil (Android con root)** — instancia MAESTRA | DNIe (chip), Cl@ve Móvil, certificado en **Android Keystore** | **No exportable** | El Keystore/TEE no deja salir el material: la bóveda guarda **sólo el alias** |
| **PC Linux en casa** — instancia WORKER | Certificado FNMT de persona física (`.p12`) o Cl@ve Móvil/Permanente | **Exportable** (el `.p12` se puede copiar) | Es el nodo que hace los trámites «de oficina»: necesita el certificado **completo** |
| **Servidor propio / VPS** | Cl@ve Móvil (preferible) o certificado exportable | Exportable sólo si el usuario lo decide | Cuanto más expuesto el nodo, menos material en él |

Regla práctica: **el material de clave se queda en el nodo menos expuesto**.
El móvil es el que lleva el DNIe; el PC de casa lleva el `.p12` cifrado; el
VPS, idealmente, **nada** (usa Cl@ve Móvil contra el móvil del usuario).

---

## 2. Flujo completo: inventario → cifrado → transferencia → despliegue → uso → revocación

### Fase 0 · Inventario (qué hay)

Cada nodo tiene una bóveda en `<state_dir>/cert_vault/`:

```
<state_dir>/cert_vault/
    inventory.json      metadatos (0600)
    manifest.json       manifiesto de sincronización (0600)
    audit.log           traza NDJSON (0600, append-only)
    certs/<id>.der      certificado público en DER (0600)
    keys/<id>.key       clave privada CIFRADA AEGIS-256 (0600)
```

El inventario guarda, por certificado: `id`, `label`, `issuer`, `subject`,
`serial_number`, `not_before`, `not_after`, `signature_algorithm`,
`public_key_algorithm`, `fingerprint_sha256` (SHA-256 **del DER**), `der_file` y
cómo está guardada la clave (`key`):

| `key.kind` | Significado | ¿Hay bytes de clave en disco? |
|---|---|---|
| `non_exportable` | La clave vive en un almacén (Keystore/TPM/PKCS#11); aquí sólo su **alias** | **No** (no existe ningún fichero de clave) |
| `exportable` | La clave está **cifrada** en `keys/<id>.key` | **Sí, pero cifrada** (AEGIS-256) |
| `remote_exportable` | El certificado está aquí, la clave **vive en otro nodo** | **No** (sólo su SHA-256) |

API: `CertVault::new(state_dir)`, `list()`, `get(id)`, `build_manifest()`,
`refresh_manifest()`, `expiry_warnings(now, warn_days)`.

### Fase 1 · Alta y cifrado

```rust
let vault = CertVault::new(state_dir)?;

// (a) Clave NO exportable (DNIe, token PKCS#11, TPM): NO se escribe material.
vault.add_certificate(
    "dnie",
    "DNI electronico (chip)",
    &der,
    KeyMaterial::NonExportable {
        backend: "android_keystore".into(),
        handle: "alias-dnie-1".into(),
    },
    None,                       // no hace falta frase de paso
    KdfParams::default(),
)?;

// (b) Clave exportable (.p12 del usuario): se cifra con AEGIS-256 y la clave
//     de cifrado sale de Argon2id con sal aleatoria POR CLAVE.
vault.add_certificate(
    "fnmt",
    "FNMT persona fisica",
    &der,
    KeyMaterial::Exportable {
        backend: "archivo".into(),
        key_bytes: p12_key_material,   // el material que se importa
    },
    Some(passphrase),           // frase de paso del usuario
    KdfParams::default(),       // 3 pases, 64 MiB, 4 carriles (RFC 9106)
)?;
```

Garantías (fijadas por tests en `rust-core/tests/cert_vault_smoke.rs`):

* En el disco **no** aparece ni la clave en claro ni la frase de paso
  (test `el_fichero_en_disco_no_contiene_la_clave_en_claro`: busca el patrón de
  bytes en **todos** los ficheros de la bóveda).
* La clave cifrada va con **0600** y la bóveda con **0700**.
* El `notAfter` se parsea **del DER** (sin crates: lector DER propio) y de ahí
  sale el estado `valid` / `expiring` / `expired`
  (test `el_estado_de_caducidad_se_calcula_desde_el_der`).

### Fase 2 · Transferencia (manifiesto + paquete)

**Manifiesto** (`manifest.json`): una entrada por certificado con
`der_sha256`, `der_len`, `not_before`/`not_after` y la referencia de la clave.
Su **digest** es el SHA-256 del cuerpo canónico (entradas ordenadas por `id`,
sin marcas de tiempo) ⇒ **mismo contenido ⇒ mismo SHA-256**, en cualquier nodo
y en cualquier momento (test `el_manifiesto_es_estable_mismo_contenido_mismo_sha256`).

**Paquete de sincronización** (`MDCS`): cabecera JSON + blobs en streaming.

```rust
let mut bundle = std::fs::File::create("/tmp/paquete.mdcs")?;
let manifest = vault.write_sync_bundle(
    &mut bundle,
    "pc-casa",                                   // nodo destino
    SyncOptions {
        include_exportable_keys: false,           // POR DEFECTO: no
        confirm: false,
    },
)?;
```

* **Por defecto** viajan **certificados públicos + referencias de clave no
  exportable**. La clave privada exportable **no viaja**.
* Para replicar la clave privada exportable hacen falta **las dos** cosas:
  `include_exportable_keys: true` **y** `confirm: true`. Con la primera sin la
  segunda, la función **falla** y no replica nada a medias
  (test `la_sincronizacion_replica_claves_exportables_solo_con_confirm`).
* El destino importa en streaming y **verifica el SHA-256 de cada elemento
  antes de escribirlo**: un elemento corrupto se rechaza y **no toca el disco**
  (fail-closed por entrada, test `un_paquete_con_sha256_rotado_se_rechaza_fail_closed`).
* La importación es **idempotente**: el mismo paquete dos veces ⇒ la segunda
  vez `imported = 0`, `unchanged = N` y **ningún fichero cambia**
  (test `la_sincronizacion_es_idempotente`). Si el `id` ya existe **con otro
  contenido**, la entrada se **rechaza** (no se pisa un certificado que el
  usuario ya tenía).

```rust
let mut f = std::fs::File::open("/tmp/paquete.mdcs")?;
let report = vault_destino.read_sync_bundle(&mut f)?;
```

### Fase 3 · Despliegue en el nodo que lo requiere

1. El nodo destino recibe el paquete **por un canal del usuario** (LAN, canal
   del harness, unidad USB, `scp`): el paquete no lleva material en claro, pero
   **no es un canal.confidencial** por sí solo: el blob de la clave va
   cifrado, y el digest del manifiesto permite comprobar que no se ha tocado.
2. La **frase de paso** de la bóveda **no viaja en el paquete**: se traslada
   **fuera de banda** (el usuario la teclea en el nodo destino, o se entrega
   por un canal distinto). Sin ella, el nodo destino tiene el certificado
   pero **no puede abrir** la clave (`unlock_private_key` falla con un mensaje
   explícito).
3. El nodo despliega la credencial en **su** almacén:
   * Android → el material va al **Android Keystore** (alias), no a un
     fichero; la bóveda guarda el alias (`non_exportable`).
   * Linux → el `.p12` se importa en un almacén del sistema (por ejemplo
     `p11-kit`/`opensc` con un token, o un almacén de usuario protegido). El
     harness **no** escribe el `.p12` en claro: lo que guarda es el blob
     AEGIS-256 y lo abre en memoria.
4. Antes de usar el certificado en una sede, el nodo **comprueba**:
   * `fingerprint_sha256` == el esperado por el usuario (evita el "certificado
     sustituto"),
   * `not_after` > ahora (estado `valid`, no `expiring`/`expired`),
   * que el certificado está **admitido** por la sede (lista oficial de
     certificados admitidos: ver `docs/CERTIFICADOS_Y_SEDES_ELECTRONICAS.md` §6.1).

### Fase 4 · Uso

```rust
// Sólo para claves exportables; el material vuelve EN MEMORIA.
let p12 = vault.unlock_private_key("fnmt", &passphrase)?;
```

Reglas de uso (no negociables):

* El material **no** se escribe en disco, **no** va a logs, **no** se
  cachea, **no** se copia a otro nodo sin `confirm`.
* Cada uso contra una sede deja **traza**: qué certificado (huella), qué
  sistema, qué hora, con qué resultado.
* **Nunca** desde un nodo desatendido para: prescribir, dispensar, firmar en
  nombre del usuario, presentar escritos sin confirmación, ni acceder a datos
  de salud sin base jurídica y con la sesión del usuario.
* Para **Cl@ve Móvil** el nodo **no** guarda credencial: abre el flujo y el
  usuario confirma en su móvil (QR + biometría). Es la única vía cleanly
  reutilizable desde un nodo.

### Fase 5 · Revocación

Hay **tres** revocaciones distintas y no hay que confundirlas:

| Qué se revoca | Cómo | Consecuencia |
|---|---|---|
| **El certificado ante su emisor** | El usuario lo pide a la FNMT (o al DNIe). Ojo: pedir otro por la app **revoca** el anterior automáticamente | El nodo que lo tenía deja de poder autenticarse; el harness debe marcarlo `expired`/`revoked` y **borrar** el blob local |
| **La copia en un nodo** | `vault.remove_certificate(id, confirm = true)` → borra el DER y el blob cifrado y deja traza | Ese nodo ya no tiene la credencial; los demás nodos siguen (hasta que se revoque el original) |
| **La autorización entre nodos** | La sincronización es **opt-in por operación**: sin `confirm: true` no sale nada | Un nodo comprometido no puede exportar claves que no ha pedido |

Procedimiento recomendado ante sospecha de robo:

1. `remove_certificate(id, confirm=true)` en **todos** los nodos.
2. Revocar el original ante el emisor (FNMT / DNIe) — esto es lo que deja de
   valer de verdad.
3. Audit log: exportar `audit.log` de cada nodo (sólo metadatos: `id`,
   huella, acción, hora) para saber **dónde** estuvo la credencial.

---

## 3. Qué se audita (y qué **no**)

`cert_vault/audit.log` (NDJSON, 0600, append-only):

| `event` | Se escribe cuando | Contenido |
|---|---|---|
| `add` | Se da de alta un certificado | `id`, tipo de clave, huella |
| `remove` | Se borra un certificado | `id` |
| `export` | Se exporta un certificado | `id`, si incluía material privado |
| `export_denied` | Se **intenta** exportar una clave privada **sin** `confirm` | `id`, motivo |
| `import` / `import_denied` | Importación (o intento sin `confirm`) | `id`, si incluía material |
| `sync_export` / `sync_import` | Paquete de sincronización | nodo destino/origen, nº de entradas, si incluía material privado |

**Nunca** se escribe en el log: la frase de paso, los bytes de la clave, ni el
contenido del `.p12` (test `la_auditoria_no_guarda_la_frase_de_paso`).

Lo que el log **no** cubre (y hay que cubrir por otra vía):

* **Qué se hizo con el certificado** en la sede (la sede tiene su propia
  traza; el nodo debe correlacionar por `fingerprint_sha256` + hora).
* **Quién pidió** una operación: el `cert_vault` sabe **qué** y **cuándo**, no
  **quién** (el usuario). Para eso está el `api_key`/canal del harness, que ya
  tiene su propio registro de auditoría.
* El **contenido** de los datos que se han descargado (historia clínica,
  informes): eso lo audita la aplicación que los usa, no la bóveda.

---

## 4. Matriz de consentimiento (qué exige `confirm: true`)

| Operación | `confirm` | Motivo |
|---|---|---|
| Listar inventario, ver caducidades, ver manifiesto | **No** | Sólo metadatos |
| Exportar el **certificado público** | **No** | Es público |
| Exportar una clave **no exportable** | **Imposible** | El material no existe fuera del almacén |
| Exportar una clave **exportable** (o el paquete que la lleva) | **Sí** | Sale material de clave (cifrado) del nodo |
| Importar un paquete **con** material de clave | **Sí** | Entra material de clave en el nodo |
| Replicar claves exportables a otro nodo | **Sí** (y `include_exportable_keys`) | Es una exportación con otro destino |
| Borrar un certificado | **Sí** | Destruye una credencial |

---

## 5. Modelo de amenazas (qué protege la bóveda y qué no)

| Amenaza | ¿La cubre `cert_vault`? | Nota |
|---|---|---|
| Robo de un fichero de la bóveda | **Sí** | La clave privada va cifrada (AEGIS-256) y la clave de cifrado sale de Argon2id con sal aleatoria por clave. Sin la frase de paso no se abre. |
| Volcado de disco del nodo | **Sí** (parcial) | Sale el blob cifrado, no la clave. La frase de paso **no** está en el disco: no la escribas ahí. |
| Alguien con acceso de lectura al `inventory.json` | **Sí** | Sólo metadatos (emisor, sujeto, caducidad, huella). |
| Robo de la frase de paso | **No** | Si la roban, obtienen la clave: usar una frase larga y **distinta por bóveda**. |
| Nodo comprometido que pide exportar claves | **Sí, parcialmente** | Sin `confirm: true` no exporta nada; pero `confirm` es un flag del proceso, no una prueba de identidad del humano: **el control real es quién puede llamar a la función** (API key + canal autenticado) |
| Copia de la clave a otro nodo sin permiso | **Sí** | La política de `confirm` + traza; el blob va cifrado |
| **Suplantación del usuario ante una sede** | **No** (y no debe automatizarse) | Es el límite legal y contractual; ver §0 |
| Clave usada contra una sede **fuera** del ámbito del usuario | **No** | La bóveda no valida contra qué sede se usa: eso lo hace el agente, con la lista de sedes permitidas |

---

## 6. Datos de salud: la frontera legal dentro del nodo

* Los datos de salud son **categoría especial** (RGPD art. 9): su tratamiento
  está **prohibido** salvo base jurídica del art. 9.2 (consentimiento
  explícito del interesado, ejecución de contrato con el interesado,
  obligación legal, etc.).
* Para el usuario que consulta **su propia** historia, la base típica es el
  **consentimiento explícito** y el **derecho de acceso** (arts. 12–15 RGPD).
* Consecuencias de diseño para el harness:
  1. El nodo **no** guarda la historia clínica en la bóveda de certificados:
     la bóveda guarda **credenciales**, no **datos clínicos**.
  2. El acceso a datos de salud exige **identificación avanzada del usuario
     en esa sesión** (certificado, DNIe o Cl@ve con registro avanzado) — lo
     verificado en la documentación oficial de SALUD Andalucía.
  3. Todo acceso a datos de salud deja **traza**: qué certificado (huella),
     qué usuario, qué dato, qué hora.
  4. **Minimización**: se descarga **lo que el usuario pide**, no «la historia
     clínica entera» por sistema.
  5. **Retención**: los datos clínicos descargados no se replican entre nodos
     por el mecanismo de certificados; si se cachean, con caducidad y cifrado.
* `NO VERIFICADO`: la base jurídica concreta de cada SAD del SAS y el régimen
  exacto de responsabilidad (encargado vs autónomo) hay que leerlos en el aviso
  de privacidad de la app «Salud Andalucía» y en la política de protección de
  datos del SAS: no están en las páginas públicas que he abierto.

---

## 7. Por qué NO se automatiza (y qué se automatiza en su lugar)

| NO se automatiza | Por qué | Qué se automatiza en su lugar |
|---|---|---|
| **Prescripción** (Receta Electrónica / DIRAYA) | Es una decisión clínico-terapéutica profesional | Recordatorio al paciente de qué tiene que recoger y hasta cuándo |
| **Dispensación** (MCD en la farmacia) | Se autentica al paciente con su tarjeta/DNIe **en la farmacia**, delante del farmacéutico acreditado | Consulta de la medicación activa y de las dispensaciones ya hechas |
| **Firma en nombre del usuario** (escritos, solicitudes, PAdES) | La firma avanzada es un acto personal con efectos jurídicos; automatizarla es suplantación | Preparar el documento, Mostrar el resumen, y que el usuario firme (o confirme explícitamente) |
| **Presentar escritos** desatendidos | Obliga al usuario a estar delante (notificación, plazos) | Dejar el borrador preparado y avisar |
| **Acceder a datos de salud sin sesión del usuario** | Art. 9 RGPD + identidad avanzada obligatoria | Nada: es una frontera, no una funcionalidad |

Justificación adicional en `docs/CERTIFICADOS_Y_SEDES_ELECTRONICAS.md` §2.4,
§3.2 y §4.2.

---

## 8. Runbook mínimo (operación)

### Instalar un certificado en el nodo Linux de casa

```bash
# 1. El usuario exporta desde el móvil (o desde el nodo que lo tenga):
#    export_certificate(id, passphrase, confirm = true, KdfParams::default())
#      ->  paquete MDC1
# 2. Se copia el paquete al nodo destino (el material va cifrado).
# 3. En el nodo destino:
#      import_certificate(bundle, Some(passphrase), confirm = true)
#    La frase de paso se teclea en el nodo: NO viaja en el paquete.
# 4. Verificación:
#      list()                    -> inventario con estado de caducidad
#      expiry_warnings(now, 30)  -> 'expiring' / 'expired'
#      unlock_private_key(id, passphrase) -> material en memoria (nunca en disco)
```

### Sincronizar el móvil → PC

```bash
# En el móvil:
write_sync_bundle(f, "pc-casa", SyncOptions { include_exportable_keys: false, confirm: false })
# En el PC:
read_sync_bundle(f)   # idempotente; verifica SHA-256 por entrada
```

### Despliegue del certificado en el navegador/sede

El certificado **no** se instala "en el navegador" desde el harness: el
navegador lo pide al almacén del sistema. En Linux, el harness deja el blob
cifrado y lo abre en memoria; el agente TLS del nodo es quien lo usa
(`rust-core/src/signing.rs` ya tiene la capa PKCS#11/OpenSC para el material no
exportable). `NO VERIFICADO`: la integración concreta navegador↔almacén del
nodo Linux **no** está implementada en este repo: el harness carga el
certificado en la sesión TLS del propio agente, no en el navegador del
usuario.

---

## 9. Lo que queda sin verificar

1. `NO VERIFICADO`: la **integración real** de un certificado exportable con
   un cliente mTLS dentro del harness (hoy `cert_vault` abre el material;
   el handshake TLS contra una sede **no** está implementado).
2. `NO VERIFICADO`: la **emisión y descarga** del `.p12` de forma
   automatizada (la FNMT exige navegador + vídeo-identificación **por el
   usuario**; no hay API pública documentada para hacerlo desde un nodo).
3. `NO VERIFICADO`: el **ciclo de revocación** de un `.p12` copiado entre
   nodos (la revocación real es ante el emisor).
4. `NO VERIFICADO`: si la FNMT o las sedes rechazan un certificado cuyo
   dispositivo/contraseña se ha movido de equipo (el proceso de descarga
   exige «el mismo equipo y el mismo usuario»: por eso la opción con vídeo o
   acreditación presencial es la que genera un `.p12` portable).
5. `NO VERIFICADO`: **retención y cifrado** de los documentos descargados de
   las sedes (informes, analíticas): fuera del módulo de certificados.
