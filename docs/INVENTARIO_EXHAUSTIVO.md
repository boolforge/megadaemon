# INVENTARIO EXHAUSTIVO del móvil — qué se recoge, de dónde y cómo

Objetivo: que el respaldo del S21 Ultra sea **mucho más completo** que Swift
Backup y Lucky Patcher. Esos dos sólo hacen APK + (a veces) datos; lo que
sigue es la lista completa de lo que hay en un Android 13 real y de dónde
sale cada cosa.

Todo es del **dispositivo del propio usuario**, con root, y va a la carpeta
**privada** de Drive (`state/backup_target.json` → `megadaemon-backup`).
El repo (público) sólo guarda el **plan**, los inventarios sin valores y el
código.

Regla de oro del inventario: **el valor nunca se escribe en el repo, ni en
el chat, ni en el tablón**. De los secretos sólo sale su *sha16* y su
*tamaño*. Los valores van al contenedor cifrado en la carpeta privada.

---

## P0 · Identidad y acceso (lo que más duele perder)

| Qué | De dónde sale (root) | Nota |
|---|---|---|
| Claves WiFi (SSID+clave) | `/data/misc/wifi/WifiConfigStore.xml`, `/data/misc/apexdata/com.android.wifi/WifiConfigStore.xml`, `/data/misc/wifi/softap.conf` | El XML trae la clave en claro o en PSK hex |
| Certificados de cliente WiFi | `/data/misc/apexdata/com.android.wifi/`, `/data/misc/keychain/` | |
| Configuración VPN | `/data/misc/vpn/`, `/data/data/com.android.vpnmanager/` | |
| Credenciales de desbloqueo | `/data/system/gatekeeper.password.key`, `gatekeeper.pattern.key`, `gatekeeper.password_wallpaper.key` | **root**: leerlas habilita el desbloqueo => se guarda cifrado y aparte |
| AccountsManager (Google y otras) | `/data/system/users/0/accounts.db`, `/data/system_ce/0/accounts_ce.db` | Contiene **tokens de autenticación**: es lo más sensible que hay |
| Autenticadores 2FA | `com.google.android.apps.authenticator2/databases/`, `com.azure.authenticator/databases/`, `com.beem`, Aegis | Bases SQLite con los TOTP |
| Gestores de contraseñas | `com.androidkeepass/files/`, `com.x8bit.bitwarden/databases/` | |
| Certificados del usuario | `/data/misc/keychain/user_0/`, `/data/misc/keystore/user_0/` | PKCS#12 con su contraseña |
| Token de sesión propio | `/data/local/tmp/megadaemon/state/megadaemon_api.key`, `.keys` | El de la flota |

## P1 · Navegadores (lo que nadie más copia bien)

Por cada navegador instalado (Chrome, Brave, Firefox, Edge, Samsung
Internet, Opera, Vivaldi, Whale…): directorio de perfil bajo
`/data/user/0/<pkg>/app_webview/Default/` o `files/`:

| Fichero | Qué contiene |
|---|---|
| `Login Data` (SQLite) | **Contraseñas** (con IV y salt; las de las apps de contraseña no se cifran, las de Google sí) |
| `Cookies` | Cookies de sesión (con `encrypted_value`: cifradas con la master key del SO ⇒ descifrables desde el móvil) |
| `History` | Historial completo con URLs y títulos |
| `Bookmarks` | Marcadores |
| `Web Data` | Tarjetas, autofill, datos de tarjetas |
| `Sessions`/`Current Session`/`Current Tabs` | **Pestañas abiertas** (lo que el usuario tenía abierto) |
| `Preferences` | Preferencias, extensiones, búsqueda por defecto, perfiles |
| `Service Worker`/`Cache` | Web-apps offline (opcional) |
| `Local Storage`, `IndexedDB`, `Session Storage` | Estado web |
| Extensions (`Default/Extensions/<id>/`) | Extensiones instaladas con sus permisos |

## P2 · Apps: APK + datos (1719 paquetes)

| Qué | De dónde |
|---|---|
| APK | `pm path <pkg>` → copiar el `.apk` (o `pm install-existing` desde el APK) |
| Datos | `/data/user/0/<pkg>/` completo: `databases/`, `shared_prefs/`, `files/`, `no_backup/`, `cache/`(excluido), `code_cache/`(excluido) |
| APK de sistema | `/system/app/`, `/system/priv-app/`, `/product/app/`, `/vendor/app/` (el APK "optimizado" de cada partición) |
| Datos de sistema | `/data/system/packages.xml`, `/data/system/users/0/`, `/data/prop/`, `/data/vendor/` |
| Bluetooth | `/data/misc/bluedroid/bt_config.conf`, `btopp/`, **`/data/misc/bluedroid/` (claves de enlace de los dispositivos emparejados)**, `/data/data/com.android.bluetooth/` |

## P3 · Comunicaciones y multimedia

| Qué | De dónde |
|---|---|
| SMS/MMS | `content://sms`, `/data/data/com.android.providers.telephony/databases/mmssms.db` |
| RCS | `/data/data/com.google.android.rcs/databases/rcs.db` (¡los chats de Google Messages!) |
| Llamadas | `content://call_log`, `contacts2.db` |
| Contactos | `/data/data/com.android.providers.contacts/databases/contacts2.db` |
| WhatsApp | `/data/data/com.whatsapp/databases/msgstore.db` (cifrado), `media/`, `Databases/` |
| Telegram | `/data/data/org.telegram.messenger/databases/` (cifrado con su clave), `files/` |
| Signal | `/data/data/org.thoughtcrime.securesms/databases/` (SQLCipher) |
| Discord/Slack/Element | sus `databases/` y `files/` |
| Fotos y vídeos | `/sdcard/DCIM`, `Pictures`, `Movies`, `Documents`, `Download`, `Android/data/<pkg>`, `Android/obb/<pkg>` |
| Grabaciones del móvil | `/sdcard/Recordings`, `/sdcard/Sounds`, `/sdcard/Music` |
| Teclado (diccionario aprendido) | `/data/data/com.google.android.inputmethod.latin/databases/` |

## P4 · Estado del sistema y del propio MegaDaemon

| Qué | De dónde |
|---|---|
| Ajustes | `/data/system/settings_*.xml`, `secure_*.xml`, `global_*.xml` |
| Props | `/data/prop/default.prop`, `build.prop`, `default.prop` |
| Magisk/Módulos | `/data/adb/modules/`, `/data/adb/magisk.db` |
| CSC (config del operador) | el `cscfeature.xml` (OMC) + `contents.db` |
| Estado de MegaDaemon | `<state_dir>/` (key, tunnel.url, channel.json, board.sqlite, audit.ndjson) |
| Antigravity | `/data/user/0/com.agy/files/.gemini/antigravity-app/` (backlog, brain/, conversations/, mcp/) |
| Nu1ca/UN1CA (si existe) | el módulo y su estado |

## P5 · Lo que NO se copia (y por qué)

- `cache/`, `code_cache/`, `no_backup/`, `thumbnails/`, `*.tmp`, `*.crdownload`
  → regenerables; ocupan más que los datos útiles.
- Bases ya volcadas en otro sitio (no duplicar).
- Nada de `/data/system/users/0/*.key` sin cifrar: van dentro del contenedor.

---

## Cómo se transporta

1. **Recogida**: con root, `tar` por bloques con `MANIFEST` (sha256 por
   fichero) y cifrado (AEGIS-256 ya implementado en el repo) por trozos.
2. **Subida**: a la carpeta **privada** de Drive, por trozos de 256 KiB con
   `Content-Range` (reanudable) y **subiendo mientras se recoge** (no
   esperar a terminar) ⇒ la subida empieza antes.
3. **Verificación**: sha256 por trozo + manifiesto global; un trozo que no
   cuadra se re-baja y se rehace.
4. **Restauración**: por app/perfil, con `--confirm`; nunca sobrescribir sin
   que el usuario lo pida.

## Lo que esto NO puede hacer

- **Las bases cifradas con clave que sólo tiene la app** (WhatsApp,
  Telegram, Signal, RCS de otro dispositivo) se copian, pero no se abren:
  para abrirlas hace falta la clave o el teléfono que las emparejó.
  Copiar ≠ descifrar. Se documenta el límite, no se finge.
- Los **tokens de Google en `accounts_ce.db`** se copian sí, pero su uso
  depende de que el token siga vivo en el servidor.
