# Telecom, SMS, OTP y antispam — qué funciona de verdad y qué queda por medir

> Módulos: `rust-core/src/telecom.rs`, `rust-core/src/otp.rs`,
> `rust-core/src/antispam.rs`.
> Dispositivo objetivo: **Galaxy S21 Ultra (Exynos 2100), Android 14, Magisk
> Kitsune**, con root (`su -mm -c`).

---

## 0. Alcance (del proyecto)

* Todo lo de aquí es para el **dispositivo del propio usuario** y sus
  **propios contactos/consentimiento**: llamadas de prueba a números propios,
  reintento de una llamada que quedó ocupada, SMS a números propios, lectura
  del buzón propio, lectura de OTP de cuentas propias y un **filtro antispam
  defensivo** (clasificar y bloquear lo que **entra**).
* **Fuera de alcance** (documentado, no moralizado): campañas de spam, envío
  masivo a terceros y llamada automatizada a desconocidos. Ninguna de las tres
  herramientas de este documento hace eso: `telecom_call` marca **un** número
  que se le pase, `telecom_sms` abre **el compositor** de **un** número, y
  `antispam_scan` sólo **lee y clasifica**.

---

## 1. Estado real: qué funciona y qué NO

| Capa | Mecanismo | Estado | Nota |
|---|---|---|---|
| **Leer el buzón** | `content query --uri content://sms/inbox` (root) | **FUNCIONA (según el repo)** | Es la vía que ya usa el daemon en `rust-core/src/messaging.rs` (`collect_sms_records()` → `crate::android::root_exec`, que es `su -mm -c`). Documentado en `docs/ROADMAP_V14.md` ("SMS/MMS ✅ `content query --uri content://sms`"). **NO MEDIDO por mí en el móvil**: falta confirmar el formato exacto de las filas en este firmware. |
| **Marcar (llamar)** | `am start -a android.intent.action.CALL -d tel:<num>` | **NO VERIFICADO** | Requiere el permiso `CALL_PHONE`. Hay que medir si el intent lo atiende el usuario *shell* o root, y si Android 14 lo deniega (`SecurityException` / `Permission Denial`). El código **no inventa** el resultado: devuelve `status = unknown`, `ok = true` (el intent se lanzó) y `verified = false`. |
| **Enviar SMS** | `service call isms <tx> …` (root) | **NO VERIFICADO** | En Android 14 el Binder `isms` puede no existir, puede pedir otro número de transacción o puede rechazar la llamada. El código sólo da el envío por bueno si **aparece la fila en `content://sms/sent`** (y entonces el `_id` de esa fila es el identificador de transacción **real**). Si no aparece: `status = unknown`, `ok = false`, `verified = false`. |
| **Enviar SMS (alternativa)** | `am start -a android.intent.action.SENDTO -d sms:<num>` | **NO VERIFICADO** | Abre **el compositor** con el número y el texto puestos. **NO envía**: el envío lo hace el usuario pulsando "enviar". El código lo refleja: `status = composer_opened`, `ok = false`. |
| **Filtro antispam / OTP / reintentos** | lógica pura en Rust | **FUNCIONA** (tests deterministas) | No toca el móvil: se prueban con proveedores simulados y un reloj simulado (sin red, sin dormir). |

> Todo lo que no está medido en el móvil está marcado **NO VERIFICADO** en el
> código (`verified: false`, `detail` con el motivo) y en la tabla de §6.

---

## 2. Cómo se mide cada cosa en el móvil

Todos los comandos de abajo se ejecutan **desde el móvil** (Termux con root o
`adb shell`), con `su -mm -c` cuando hace falta root.

### 2.1 Leer el buzón (la vía que sí funciona como root)

```bash
su -mm -c 'content query --uri content://sms/inbox --projection _id:address:body:date'
```

Variante con más campos (la que usa el código):

```bash
su -mm -c 'content query --uri content://sms/inbox --projection _id:address:body:date:type:read'
```

Qué hay que anotar de la salida:
1. ¿Sale `Row: 0 _id=…, address=…, body=…, date=…`? (formato que parsea
   `telecom::parse_content_sms`).
2. ¿La columna `date` viene en **milisegundos** o en segundos? (el código
   detecta ambos: si el valor es < 1e11, lo multiplica por 1000).
3. ¿El `body` puede traer comas y saltos de línea? (el parser corta por
   claves conocidas, no por comas).
4. ¿`content://sms/inbox` existe tal cual, o hay que usar `content://sms` +
   filtrar por `type=1`?

### 2.2 Confirmar un envío (carpeta de enviados)

```bash
su -mm -c 'content query --uri content://sms/sent --projection _id:address:body:date'
```

Esto es lo que usa `IsmsTelecom` para **no inventar** un "enviado": si el
cuerpo y el destinatario aparecen aquí, el envío es real y el `_id` es el
identificador de transacción.

### 2.3 El Binder `isms` (SMS por root) — NO VERIFICADO

```bash
# 1) ¿Existe el servicio?
su -mm -c 'service list | grep -i isms'

# 2) ¿Qué transacciones declara?
su -mm -c 'dumpsys isms 2>/dev/null | head -40'

# 3) Intento de envío (el que genera el código; el nº de transacción es el
#    único parámetro que quizá haya que cambiar):
su -mm -c "service call isms 4 i32 1 s16 'texto de prueba' s16 '+34600111222' i32 0 i32 0"

# 4) ¿Ha salido? (debe aparecer una fila nueva en `sent`)
su -mm -c 'content query --uri content://sms/sent --projection _id:address:body:date'
```

Si el paso 3 falla, probar con otras transacciones (`1`, `2`, `3`, `5`…) y
anotar cuál funciona: se cambia en `IsmsTelecom::transaction` (o en
`IsmsTelecom::send_command`, que es pública y devuelve la orden exacta).

### 2.4 Llamada de prueba a un número propio

```bash
# A) Como root (el daemon vive como root: su -mm -c)
su -mm -c 'am start -a android.intent.action.CALL -d tel:+34600111222'

# B) Como el usuario shell (a veces el intent de llamada lo atiende shell)
am start -a android.intent.action.CALL -d tel:+34600111222

# C) ¿Se concede el intent? (mirar el logcat justo después de B)
su -mm -c 'logcat -d -t 300 | grep -i -E "SecurityException|Permission Denial|ActivityTaskManager.*CALL"'

# D) ¿Hay una llamada en curso / en qué estado?
su -mm -c 'dumpsys telecom 2>/dev/null | grep -i -E "Call \{|mCalls|mCurrentCall" | head -20'
su -mm -c 'dumpsys telephony.registry 2>/dev/null | head -40'

# E) Historial real de llamadas (el resultado que la red sí conoce)
su -mm -c 'content query --uri content://call_log --projection _id:number:duration:date:type'
```

Y el estado del permiso:

```bash
su -mm -c 'dumpsys package com.android.shell | grep -i -E "CALL_PHONE|granted"'
```

**Qué anotar**: si (A) o (B) abren el marcador y marcan, si el logcat enseña un
`Permission Denial`, y qué dice `dumpsys telecom` mientras la llamada está en
curso. Con eso se puede pasar `CallStatus::Unknown` a un estado real y poner
`verified = true`.

### 2.5 Contactos del usuario (para el detector de números confundibles)

```bash
# Candidatos (NO VERIFICADO: el URI exacto hay que medirlo)
su -mm -c 'content query --uri content://com.android.contacts/data/phones --projection display_name:data1'
su -mm -c 'content query --uri content://contacts/phones --projection display_name:number'
su -mm -c 'dumpsys contact 2>/dev/null | head -40'
```

De aquí sale la lista de contactos que se pasa a `antispam::ScanContext::contacts`.

---

## 3. Los tres módulos

> **Nombres**: el proyecto pide identificadores en inglés y comentarios en
> español, así que los campos que en el enunciado estaban en español tienen
> nombre inglés en el código:
>
> | Enunciado | En el código |
> |---|---|
> | `OtpCandidate.valor` | `value` |
> | `OtpCandidate.confianza` | `confidence` (0..=1) |
> | `OtpCandidate.mensaje_id` | `message_id` |
> | `OtpCandidate.remitente` | `sender` |
> | `OtpCandidate.ts` | `timestamp` (ms epoch) |
> | `OtpCandidate.motivo` | `reason` (+ `dubious` para el caso dudoso) |
> | `RetryPolicy.intentos` | `attempts` (intentos **totales**) |
> | `RetryPolicy.espera_inicial` | `initial_delay_ms` |
> | `RetryPolicy.espera_max` | `max_delay_ms` |
> | `RetryPolicy.solo_si` | `retry_on: Vec<RetryCondition>` (`Busy`, `NoAnswer`, `TemporaryFailure`) |
> | `Signal.regla/peso/detalle` | `rule` / `weight` / `detail` |

### 3.1 `telecom` — capas de proveedor (deterministas)

```rust
pub trait TelecomProvider {
    fn place_call(&self, numero: &str) -> Result<CallOutcome, String>;
    fn send_sms(&self, numero: &str, texto: &str) -> Result<SmsOutcome, String>;
    fn read_inbox(&self, limite: usize) -> Result<Vec<SmsMessage>, String>;
}
```

| Tipo | Qué hace |
|---|---|
| `AmTelecom` | `am start … CALL` / `SENDTO`. Requiere `CALL_PHONE`. El SMS **no se envía**: abre el compositor (`SmsStatus::ComposerOpened`, `ok = false`). |
| `IsmsTelecom` | `service call isms` (root). **NO VERIFICADO** en Android 14. Verifica el envío leyendo `content://sms/sent`; si no aparece, `ok = false`. |
| `ContentTelecom` | Lee el buzón con `content query --uri content://sms/inbox` (**la vía que funciona como root**). No marca ni envía. |
| `SimTelecom` | Proveedor simulado (guion de resultados) para tests y modo simulación. |

Infraestructura de soporte:

* `CommandRunner` (inyectable): `RootShellRunner` (`su -mm -c`),
  `PlainShellRunner` (`sh -c`) y `ScriptedRunner` (guion + registro de órdenes).
* `Clock` (inyectable): `RealClock` y `SimClock` (**no duerme**: anota las
  esperas y avanza el reloj).
* `RetryPolicy { attempts, initial_delay_ms, max_delay_ms, retry_on, backoff_factor }`
  con backoff exponencial **y tope**; `RetryCondition` = `Busy`, `NoAnswer`,
  `TemporaryFailure`.
* `CallLog` / `CallAttempt`: número, marca de tiempo, duración, resultado y
  **motivo de reintento** por intento.
* `call_with_retry(provider, numero, policy, clock) -> CallLog`:
  * error del proveedor (`Err`) = **permanente**, no se reintenta (número
    inválido);
  * `Busy` / `NoAnswer` / `Failed` se reintentan **sólo** si la política lo
    permite y quedan intentos;
  * `Unknown` **nunca** se reintenta (no se sabe qué pasó);
  * `Answered` / `Rejected` no se reintentan.

### 3.2 `otp` — códigos de un solo uso del buzón propio

* Patrones de **4 a 8 dígitos**, con o sin separador (`123456`, `123 456`,
  `483-921`).
* Palabras clave: `código`, `code`, `verification`, `OTP`, `2FA`, `token`,
  `clave`, `pin`, `contraseña`… (lista completa en `otp::KEYWORDS`).
* `OtpCandidate { value, confidence, message_id, sender, timestamp, reason, dubious }`
  — la confianza es `0..=1` y **`reason` explica por qué** (qué palabra clave,
  a cuántos caracteres, de qué lado).
* Falsos positivos filtrados: teléfonos (9-15 dígitos, `+`, "llama/tel" delante),
  números de pedido/factura (>11 dígitos, "pedido/orden/factura" delante),
  números sueltos < 4 dígitos. Un número suelto **sin palabra clave** sale
  con confianza baja (`0.20`) y `dubious = true`.
* Normalización **sin dependencias nuevas** (`unicode-normalization` **no** está
  en `Cargo.toml`: verificado) → ASCII a mano: dígitos de ancho completo,
  letras de ancho completo, dígitos de otros alfabetos, acentos, espacios
  raros y ancho cero. Cada carácter se mapea a uno, así las distancias siguen
  siendo exactas.
* `extract_otp(&OtpMessage)`, `extract_otp_text(&str)`,
  `extract_otp_in_inbox(&[OtpMessage])` y `extract_otp_detailed(...)`, que
  además devuelve los **descartes con su motivo** (`RejectedToken`).

### 3.3 `antispam` — filtro entrante (SMS y llamadas)

* `SpamScore { total, signals: Vec<Signal{rule, weight, detail}> }`: el total
  es la suma de los pesos y **el desglose está en `signals`** (`explain()`
  devuelve el texto legible).
* `Decision { Accept, Quarantine, Block }` con umbrales configurables
  (`quarantine_threshold = 0.35`, `block_threshold = 0.80`).
* Reglas: `blacklist`, `whitelist`, `short_code`, `premium_prefix`,
  `phishing_template`, `urgency_threat`, `shortened_url`, `generic_url`,
  `frequency`, `confusable_number`, `low_coherence`.
* Número confundible: Levenshtein contra los contactos del usuario, comparando
  los números crudos **y** los canónicos (`0/O`, `1/l/I`, `5/S`, `2/Z`, `8/B`,
  `6/G`, `rn/m`). La canonización se aplica **antes** de recortar la parte
  nacional (si no, un número con una `O` se alinea mal con su contacto).
* Listas persistentes en `<state_dir>/antispam_lists.json` con permisos
  **0600**; cada entrada guarda **el motivo**. La **lista blanca tiene
  prioridad** (si el remitente está en ella, la decisión es `Accept` aunque el
  mensaje sea basura); la **lista negra** fuerza `Block`.

---

## 4. Cómo se integra con el resto (tool MCP)

`lib.rs` tiene que declarar los tres módulos (**NO lo he tocado**: lo cablea
el dueño del repo):

```rust
pub mod telecom;
pub mod otp;
pub mod antispam;
```

Y las herramientas MCP (en `rust-core/src/mcp_ext.rs`, que **tampoco** he
tocado) se añaden en dos sitios: el catálogo `tool_definitions()` y el
dispatcher `dispatch(name, args)`.

### 4.1 `telecom_call` — marcar un número propio

```json
{
  "name": "telecom_call",
  "description": "Marca un numero (propio o de contacto consentido) con la politica de reintento. Devuelve el registro real de intentos: NO inventa que la llamada salio.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "number": { "type": "string", "description": "Numero a marcar (normalizado a digitos)" },
      "provider": { "type": "string", "enum": ["am", "sim"], "default": "am" },
      "attempts": { "type": "integer", "default": 3, "description": "Intentos totales (1 = sin reintento)" },
      "initial_delay_ms": { "type": "integer", "default": 5000 },
      "max_delay_ms": { "type": "integer", "default": 60000 },
      "retry_on": { "type": "array", "items": { "type": "string", "enum": ["busy", "no_answer", "temporary_failure"] } },
      "confirm": { "type": "boolean", "default": false }
    },
    "required": ["number", "confirm"]
  }
}
```

Implementación (resumen):

```rust
let provider = match provider {
    "sim" => Box::new(telecom::SimTelecom::new()) as Box<dyn telecom::TelecomProvider>,
    _      => Box::new(telecom::AmTelecom::root()),
};
let policy = telecom::RetryPolicy { attempts, initial_delay_ms, max_delay_ms, retry_on, backoff_factor: 2 };
let mut clock = telecom::RealClock;
let log = telecom::call_with_retry(provider.as_ref(), &number, &policy, &mut clock);
json!({ "ok": true, "log": log, "verified": false, "nota": "estado real NO VERIFICADO en el movil" })
```

`confirm: true` es obligatorio (marca un teléfono: es la misma política que el
resto de tools que modifican el dispositivo).

### 4.2 `telecom_sms` — SMS a un número propio

```json
{
  "name": "telecom_sms",
  "description": "Envia un SMS a un numero propio. 'am' abre el COMPOSITOR (no envia); 'isms' usa el Binder isms de root (NO VERIFICADO en Android 14) y solo da el envio por bueno si aparece la fila en content://sms/sent.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "number": { "type": "string" },
      "text": { "type": "string" },
      "provider": { "type": "string", "enum": ["am", "isms", "sim"], "default": "am" },
      "confirm": { "type": "boolean", "default": false }
    },
    "required": ["number", "text", "confirm"]
  }
}
```

La respuesta **debe** incluir `status`, `ok` y `verified` tal cual: si el
proveedor es `am`, el estado será `composer_opened` y `ok = false` (no se ha
enviado nada).

### 4.3 `sms_read` — leer el buzón

```json
{
  "name": "sms_read",
  "description": "Lee el buzon propio por content provider (content://sms/inbox, la via que funciona como root) y devuelve los mensajes con id, remitente, cuerpo y fecha.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "limit": { "type": "integer", "default": 50 }
    }
  }
}
```

```rust
let prov = telecom::ContentTelecom::root();
let msgs = prov.read_inbox(limit)?;
```

### 4.4 `otp_read` — códigos del buzón propio

```json
{
  "name": "otp_read",
  "description": "Extrae codigos de un solo uso del buzon propio con su confianza y su motivo. Los codigos de cuentas del propio usuario.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "limit": { "type": "integer", "default": 20 },
      "min_confidence": { "type": "number", "default": 0.5 }
    }
  }
}
```

```rust
let msgs = telecom::ContentTelecom::root().read_inbox(limit)?;
let items: Vec<otp::OtpMessage> = msgs.iter().map(|m| otp::OtpMessage::new(
    &m.id, &m.sender, &m.body, m.timestamp)).collect();
let cands = otp::extract_otp_in_inbox(&items);
```

### 4.5 `antispam_scan` — clasificar lo que entra

```json
{
  "name": "antispam_scan",
  "description": "Clasifica un SMS o una llamada entrante con el filtro antispam defensivo y devuelve el desglose de señales y la decision (accept/quarantine/block). No envia nada.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "kind": { "type": "string", "enum": ["sms", "call"], "default": "sms" },
      "sender": { "type": "string" },
      "body": { "type": "string" },
      "transcript": { "type": "string" },
      "timestamp": { "type": "integer" },
      "action": { "type": "string", "enum": ["scan", "blacklist_add", "blacklist_remove", "whitelist_add", "whitelist_remove", "lists"], "default": "scan" },
      "reason": { "type": "string", "description": "Motivo de la entrada en la lista (obligatorio)" }
    },
    "required": ["kind", "sender"]
  }
}
```

```rust
let state_dir = crate::cockpit_server::state_dir();
let mut lists = antispam::SpamLists::load(&state_dir);
// listas negra/blanca con motivo, en <state_dir>/antispam_lists.json (0600)
let mut ctx = antispam::ScanContext::with_lists(lists);
ctx.contacts = contactos_del_usuario();      // ver §2.5
ctx.history = ultimos_sms_del_buzon();      // para la regla de frecuencia
let verdict = antispam::classify_sms(&sms, &ctx);
json!({ "ok": true, "decision": verdict.decision.label(), "score": verdict.score, "desglose": verdict.score.explain() })
```

Las cuatro reglas de lista (`blacklist_add`, `blacklist_remove`,
`whitelist_add`, `whitelist_remove`) usan
`SpamLists::add_blacklist_in(state_dir, numero, motivo)` /
`add_whitelist_in(...)` / `remove_blacklist_in(...)` /
`remove_whitelist_in(...)`: el motivo es **obligatorio** (para poder revisar la
decisión después).

---

## 5. Tests (deterministas, sin red y sin dormir)

Todos van **dentro** de los módulos (`#[cfg(test)] mod tests`), así que
`cargo test --lib` los recoge en cuanto `lib.rs` declara los módulos.

**`telecom` (20 tests)**: reintento con ocupado N veces (esperas crecientes y
parada en el tope), parada al agotar intentos, contestado → sin reintento,
sin respuesta → según política, error permanente → sin reintento, tope de
espera total, reloj de simulación que no duerme, estado desconocido → sin
reintento, intent `CALL` con el número normalizado, compositor que no declara
envío, número inválido, `isms` sin verificación, `isms` confirmado por la
carpeta de enviados, lectura y límite del buzón, parser con comas, registro de
órdenes del runner, normalización de números, `shell_quote`.

**`otp` (18 tests)**: "Tu código es 123456", "Código: 483-921", "123 456",
"verification code", "OTP", "2FA", código **después** del número, teléfono
(9 dígitos) → no extraído, teléfono con `+` → no extraído, número de pedido
largo → no extraído, sin palabra clave → confianza baja + `dubious`, dos
códigos en el mismo mensaje, números de 3 dígitos → no extraídos, ancho
completo, acentos y dígitos extranjeros, longitud de caracteres conservada,
"barcode" no es "code", código repetido → uno solo, buzón completo aplanado y
ordenado.

**`antispam` (18 tests)**: phishing con enlace acortado → `Block`, SMS de
banco sin URL → `Accept`, número cercano a un contacto → `Quarantine`, lista
blanca con prioridad, lista negra → `Block`, número corto solo → `Accept`,
frecuencia anómala, urgencia con amenazas, mensaje normal de un contacto →
`Accept` (sin señales), prefijo premium, voz con coherencia baja, voz normal,
desglose con detalle, persistencia con permiso 0600, Levenshtein, confundibles,
clave nacional, hosts de URLs.

Total: **56 tests** en los tres módulos (verificados con un arnés aparte,
fuera del crate, mientras `lib.rs` aún no los declara).

---

## 6. Lo que queda `NO VERIFICADO` (pendiente de medir en el móvil)

1. **Llamada por intent** (`am start … CALL`): ¿la atiende shell o root?
   ¿Android 14 lo deniega sin `CALL_PHONE`? ¿Qué devuelve `dumpsys telecom`
   durante la llamada? Hasta medirlo, `CallStatus::Unknown` y `verified = false`.
2. **Envío por Binder `isms`**: número de transacción válido, existencia del
   Binder en Android 14 y si la verificación por `content://sms/sent` es
   fiable. Hasta medirlo, `ok = false` y `verified = false`.
3. **Envío por compositor** (`SENDTO`): que el texto arrive prellenado y que
   el envío manual sea el único camino.
4. **Formato exacto de las filas** de `content query` en este firmware
   (milisegundos vs segundos, comas en el cuerpo, `content://sms/inbox` vs
   `content://sms`).
5. **URI de contactos** para la regla de confundibles (§2.5).
6. **Regla de frecuencia**: el umbral (3 mensajes / 10 min) es un punto de
   partida elegido a ojo; hay que ajustarlo con el volumen real del usuario.
7. **Listas de phishing y de urgencia**: los pesos y las palabras clave son
   un punto de partida; con el uso real habrá que ajustarlos (y el usuario
   puede meter sus remitentes en la lista blanca, que tiene prioridad).
8. **Confirmación de llamada contestada**: no hay forma fiable de saber si la
   llamada se contestó desde el intent; si hace falta, el estado real está en
   `content://call_log` (duración) — falta el cableado de esa lectura.
