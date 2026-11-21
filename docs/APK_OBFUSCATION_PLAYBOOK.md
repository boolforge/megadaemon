# Playbook de ofuscación, empaquetado y parcheo en Android — las firmas

> Referencia operativa del escáner (`rust-core/src/apk_deobfuscator.rs`).
> Cada firma dice **qué se busca**, **dónde** y **cómo se ancla**, con la
> confianza que devuelve el motor.
>
> Criterio de este documento: **no se inventa**.  Una firma que no se puede
> comprobar con bytes del APK **no se escribe aquí**; cuando sólo hay
> indicios, se dice "heurístico" y se baja la confianza.

---

## 1. Cómo decide el motor

```
ZIP ─► AndroidManifest.xml (AXML) ─► classes*.dex ─► tabla de clases/métodos/campos
     ─► strings (MUTF-8) ─► grafo de llamadas (xrefs) ─► informe JSON
     ─► lib/**/*.so ─► ELF (secciones, símbolos, imports, DT_NEEDED, stripped)
```

Tres familias de evidencia, con confianza distinta:

| Familia | Qué la produce | Confianza |
|---|---|---|
| **Ruta del APK** | `assets/`, `lib/`, `META-INF/` | 88-90 (prueba directa) |
| **Clase del DEX** | prefijo de descriptor (`Lcom/xyz/`) | 85 |
| **Cadena** (DEX o `.so`) | subcadena, con marcador *específico* | 78 |
| **Estructura** | % de clases cortas, `SourceFile` | calculada (60-97) |

⚠️ **Los marcadores de cadena cortos o genéricos se prohíben**: `"360"`,
`"ZKM"`, `"tencent"` dan falsos positivos (aparecen en cadenas de versión o
de SDK legítimos).  Por eso la tabla de §3 sólo usa marcadores largos y
distintivos.

---

## 2. Ofuscadores (renombrado de código)

### 2.1 R8 / ProGuard — **detección estructural** (no hay marcador fijo)

R8 y ProGuard **no dejan una firma**: se detectan por la **forma** del
código.

| Señal | Regla | Confianza |
|---|---|---|
| Clases con nombre de 1-3 caracteres (`a`, `ab`, `a05`, `y87`) | `^[A-Za-z]{1,2}[0-9]{0,2}$` | + |
| % de clases cortas sobre el total | **> 40 %** ⇒ ofuscado | `40 + ratio*55` |
| `SourceFile` ausente | **< 20 %** de clases lo conservan | +6 |

**Prueba definitiva de R8**: R8 escribe el hash de su mapa de renombrado en
el atributo `SourceFile`:

```
SourceFile = "r8-map-id-c74bca89aa2371ea52ef6c1a26dacf4301f91c70b84806dec6e25bdc8fc9d5ca"
```

⇒ si aparece `r8-map-id-<sha256>` ⇒ **confianza 99**.  Medido en:
Swift Backup 5.1.1 (14.122 clases), TrustMe (4.548 clases), LSPatch
(3.355/4.207 = 80 %), LSPosed daemon (162/187 = 87 %).

### 2.2 DexGuard (GuardSquare)

| Tipo | Buscar en | Confianza |
|---|---|---|
| cadenas | `DexGuard`, `guardsquare`, `guardianprotection` | 78 |
| clases | `Lcom/guardianprotection/`, `Lcom/guardsquare/` | 85 |
| nativas | `libdexguard*`, `libguardian*` | 88 |

*Nota*: DexGuard es ProGuard + cifrado de strings + anti-debug nativo; sin
la marca comercial, se degrade a "R8/ProGuard + cifrado de strings".

### 2.3 Allatori

| Tipo | Buscar en |
|---|---|
| cadenas | `Allatori Obfuscator`, `allatori` |
| clases | `Lcom/allatori/` |

### 2.4 ZKM / Zelix KlassMaster

| Tipo | Buscar en |
|---|---|
| cadenas | `Zelix KlassMaster`, `zelix.classloader`, `StringProtector` |
| clases | `Lcom/zelix/` |

⇒ el cifrado de strings de ZKM produce el patrón **"cadena + routine de
descifrado"** que §5 describe.

### 2.5 Stringer

| Tipo | Buscar en |
|---|---|
| cadenas | `Stringer obfuscator`, `com.stringer` |
| clases | `Lcom/stringer/` |

---

## 3. Empaquetadores / protectores (el DEX no es el DEX)

Un empaquetador **sustituye** el DEX original por uno cifrado y lo
descifra en `attachBaseContext` (o en el `Application.onCreate`).  La firma
delatora es un **fichero raro dentro del APK**.

### 3.1 Bangcle / SecShell (SecNeo)

| Tipo | Buscar en | Confianza |
|---|---|---|
| rutas | `assets/libsecmain.so`, `assets/libsecexe.so`, `assets/bangcle_classes.jar`, `assets/classes.jar` | 90 |
| nativas | `libsecmain.so`, `libsecexe.so`, `libSecShell.so` | 88 |
| cadenas | `bangcle`, `SecShell`, `secneo`, `secexe` | 78 |

⇒ **ancla**: el `Application` de la app descomprime `assets/classes.jar` en
el directorio de la app y llama a `DexClassLoader`.

### 3.2 Tencent Legu

| Tipo | Buscar en |
|---|---|
| rutas | **`assets/o0oooOO0ooOO.dat`** (marcador característico), `assets/0OO00l111l1l`, `assets/af.bin` |
| nativas | `libshell.so`, `libshella*`, `libtup.so`, `libtosso*` |
| cadenas | `Legu`, `libshella`, `o0oooOO0ooOO` |

### 3.3 360 Jiagu (Qihoo)

| Tipo | Buscar en |
|---|---|
| nativas | `libjiagu.so`, `libjiagu_art.so`, `libjiagu64.so`, `libjiagu_x86.so` |
| rutas | `assets/.app_protect`, `assets/.appkey` |
| cadenas | `jiagu`, `qihoo360`, `libjiagu` |

### 3.4 Ijiami

| Tipo | Buscar en |
|---|---|
| nativas | `libexec.so`, `libexecmain.so`, `libijiami.so` |
| rutas | `assets/ijiami.dat`, `assets/ijiamiCa.keystore` |
| cadenas | `ijiami`, `ijiamiCa` |

### 3.5 Cómo se distingue un empaquetador de un ofuscador

| | Ofuscador | Empaquetador |
|---|---|---|
| El DEX original ¿está? | **sí** (renombrado) | **no** (cifrado) |
| ¿Hay `.so` sospechosa? | no | **sí** |
| ¿Hay `assets/` raro? | no | **sí** |
| Se puede leer el `strings` del DEX | sí | **sólo el stub** |

⇒ **prueba rápida**: si el DEX tiene > 1.000 clases pero casi ninguna tiene
métodos con cuerpo ⇒ es un stub de empaquetador.

---

## 4. Frameworks de parcheo / hooking

Éstos **no** empaquetan: **modifican** el APK o se enganchan al proceso.

| Framework | Ancla | Firmas | Confianza |
|---|---|---|---|
| **LSPatch** | `assets/lspatch/` | `LSPatch`, `Lcom/lspatch/hook/`, `liblspatch.so` | 88-90 |
| **KnoxPatch** | `assets/knoxpatch/` | `KnoxPatch`, `Lcom/tsng/knoxpatch/` | 85-90 |
| **EdXposed** | `assets/edxposed/` | `EdXposed`, `Lorg/edxposed/hook/`, `libedxposed.so` | 85-90 |
| **MonoPatch** | — | `MonoPatch`, `Lcom/monopatch/` | 85-90 |
| **TrustMe** | — | `TrustMe`, `Lcom/topjohnwu/trustme/` | 85-90 |
| **Xposed / LSPosed** | **`assets/xposed_init`** | `XposedBridge`, `xposedmodule`, `de.robv.android.xposed`, `Landroid/content/pm/XposedHelpers;` | 88-90 |
| **Frida Gadget** | `lib/*/libfrida-gadget.so` | `frida-gadget`, `frida-agent`, `gadget.config`, `libgadget.config.so` | 88-90 |

### 4.1 La firma universal: `assets/xposed_init`

Todo módulo Xposed (y LSPosed, EdXposed, MonoPatch, TrustMe, Lucky
Patcher) lleva **`assets/xposed_init`** con la clase que se carga en el
proceso destino.

⇒ **regla de una línea**: si `assets/xposed_init` existe ⇒ la app **inyecta
código en otras apps** ⇒ hallazgo de supply-chain (`PATCH_FRAMEWORK_PRESENT`,
severidad alta), porque el binario instalado difiere del publicado.

### 4.2 Frida dentro de una `.so`

En el interior de las bibliotecas nativas se buscan marcas de instrumentación:
`frida`, `xposed`, `substrate`, `sandhook`, `whale`, `epic`, `yahfa`,
`riru`, `zygisk`, `lsposed`, `liberty`, `dobby`, `shadowhook`.

Y **superficie de hook**: si la `.so` importa de libc
`open`/`openat`/`fopen`/`readlink`/`stat`/`access`/`getenv`/`ptrace`/
`dlopen`/`dlsym`/`mprotect`/`mmap`/`fork`/`system`/`execve`/`popen`/`kill`/
`signal`/`sigaction`/`connect`/`send`/`recv`/`strcmp`/`strncmp`/`memcmp`/
`__system_property_get`/`prctl`/`syscall` ⇒ son los puntos que un framework
de hook intercepta.

⚠️ **Importar `system` no es "ejecutar comandos"**: la comprobación de
ejecución real usa **los imports** (`system`, `execve`, `execl*`, `popen`,
`posix_spawn`) y **cadenas exactas** (`/system/bin/sh`, `/system/xbin/su`,
`magisk`, `frida-server`, `com.saurik.substrate`).  Buscar la *subcadena*
`"system"` en todas las cadenas daba falsos positivos con
`java.lang.System` y `System.currentTimeMillis` (32 falsos positivos en un
APK normal de Crashlytics).

---

## 5. Cadenas cifradas: el patrón "cadena + routine de descifrado"

### 5.1 Cómo se detecta el **descifrador**

Una clase es un descifrador de strings si cumple **las tres**:
1. tiene un **campo estático** de tipo `[I`, `[B`, `[C` o `[Ljava/lang/String;`
   (la tabla de sustitución),
2. algún método suyo usa **XOR** (`xor-int`, `xor-int/2addr`, `xor-long`,
   `xor-long/2addr`),
3. ese método **construye un `String`** (`new-instance Ljava/lang/String;` +
   `invoke-direct Ljava/lang/String;-><init>`).

⇒ es el patrón canónico de R8/ZKM/Allatori/DexGuard.

### 5.2 Cómo se detecta el **punto de carga dinámica**

Si un método invoca `DexClassLoader`, `PathClassLoader` o
`InMemoryDexClassLoader` **con una constante de texto** ⇒ hay un punto de
carga de código, y la constante es lo que se descifra.

> Ejemplo medido (Swift Backup 5.1.1):
> `Lza4;-><clinit>` → `const-class Lsun/misc/Unsafe;` →
> `const-string "getUnsafe"` → `Method.invoke` →
> `sput-object Lza4;.a:Lsun/misc/Unsafe;` →
> `const-string "java.boot.class.path"` → `String.split(":")` →
> `new-instance Loz1;` → `PathClassLoader.<init>` → `loadClass`.
> ⇒ **R8 con bootclasspath spoofing**: construye un `PathClassLoader`
> sobre `java.boot.class.path` para falsear las clases del framework.

### 5.3 Qué se reporta

```json
{ "class": "...", "method": "...", "kind": "string-decryptor|dynamic-loader",
  "constant": "...", "confidence": 86,
  "evidence": "clase X -> metodo Y -> invoca DexClassLoader con la constante \"Z\"" }
```

---

## 6. Renombrado heurístico (por patrón de uso, nunca por adivinar)

El nombre ofuscado no dice nada; **lo que se llama sí**.  SeConstruction
la tabla de roles en `ROLE_RULES` (60 reglas) y de métodos en
`METHOD_RULES` (22 reglas).

### 6.1 Cómo se puntúa

```
confianza = base_de_la_regla
          + 6   si casa con > 1 rol distinto
          + 4   si la API se llama más de 2 veces
          + 5   si el nombre de la clase es ofuscado
          - 40  si el nombre de la clase YA es legible
          - 12  si `Cipher.doFinal` no permite distinguir cifrar de descifrar
          (tope 95)
```

### 6.2 La regla de oro: **por debajo del umbral no se renombra**

`RENAME_CONFIDENCE_THRESHOLD = 60`.  Por debajo:

```json
{ "obfuscated": "La05;", "suggested": "a05", "role": "dbHelper",
  "confidence": 38, "applied": false,
  "reason": "confianza 38 < 60: se conserva el nombre original" }
```

⇒ **el mapa siempre propone, pero nunca impone**; y el campo `applied`
distingue lo renombrable de lo que se deja como estaba.

### 6.3 Distinguir **cifrar** de **descifrar**

`Cipher.doFinal` no dice si la clase cifra o descifra, pero **el modo que se
pasó a `Cipher.init` sí**: `1` = `ENCRYPT_MODE`, `2` = `DECRYPT_MODE`.  El
motor sigue la constante de entero entre el `const/4`/`const/16` y la
invocación ⇒

* modo 1 ⇒ el método se propone **`encrypt`**
* modo 2 ⇒ **`decrypt`**
* ambos o ninguno ⇒ **`transform`** (y −12 de confianza)

### 6.4 Ejemplos reales medidos

| Ofuscado | Propuesto | Confianza | Evidencia |
|---|---|---|---|
| `La32;->h` | `decodeBase64` | 88 | `android.util.Base64->decode` |
| `La36;->b` | `resolveClass` | 86 | `java.lang.Class->forName` |
| `La05;` | `dbHelper` | 83 | `SQLiteDatabase->execSQL` |
| `La72;` | `socialSdk` | 93 | `com.facebook.*` |
| `La17;` | `httpClient` | 83 | `HttpURLConnection` |
| `La72;->b` | `encrypt` | 84 | `Cipher.doFinal` + `Cipher.init(ENCRYPT_MODE)` |

---

## 7. Huellas del propio formato (para el escáner)

| Indicador | Lectura |
|---|---|
| `SourceFile` = `r8-map-id-<sha256>` | R8 (confianza 99) |
| Clases con nombre Unicode confusable (`Lʼˎ/ᵢ;`) | ofuscación agresiva / herramientas de pago |
| `% de clases cortas` > 40 % | ProGuard/R8 |
| Cero `lib/*.so` y > 1.000 clases | empaquetador puro (el código está en la nativa) |

---

## 8. Tabla rápida de decision

```
¿assets/xposed_init?              → framework de parcheo (hallazgo alto)
¿libjiagu/libshell/libexec?       → empaquetador (360 / Legu / Ijiami)
¿assets/o0oooOO0ooOO.dat?         → Tencent Legu
¿assets/libsecmain.so?            → Bangcle/SecShell
¿SourceFile = r8-map-id-…?        → R8 (confianza 99)
¿clases con nombre Unicode?        → ofuscación anti-lectura
¿clase con array estático + XOR
  + construcción de String?        → descifrador de strings
¿DexClassLoader con constante?    → punto de carga dinámica
```

---

## 9. Límites que se documen a propósito

* **No** se distingue "R8" de "ProGuard" cuando no hay `r8-map-id`: se dice
  `ProGuard/R8` con la confianza calculada.
* **No** se afirma el ofuscador de un APKwhose classes están renamed pero
  que no encaja en ningún patrón conocido.
* Las firmas de **DexGuard** y otros propietarios son **heurísticas**: si
  sólo hay una cadena suelta, la confianza es 78, no 99.
* La detección **no** intenta descifrar las cadenas: **describe dónde está
  el descifrador**, que es lo que un auditor necesita y lo que la ley
  permite documentar.
