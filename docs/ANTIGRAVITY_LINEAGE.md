# Genealogía de nombres de Antigravity: ¿era «Jetski»? ¿Siempre fue de Google?

> Documento de trabajo · 2026-11-26 · workspace canónico `/workspace/antigravity/`
> Todo lo que va marcado **[MEDIDO]** está verificado contra el binario que analizamos
> (`/workspace/agy-re/liblanguage_server.so`, 181 358 800 B, sha256 `f25135e4…0ff27`,
> crc32 `6207a1b8`, idéntico al `assets/runtime/bin/liblanguage_server.so` del APK
> `antigravity-region-bypass.apk` v2.4.1).
> Lo marcado **[FUENTE]** procede de terceros y se cita la URL.
> Lo marcado **[NO VERIFICADO]** no ha podido comprobarse aquí y se dice por qué.

---

## 0. Respuesta corta

| Pregunta | Respuesta | Estado |
|---|---|---|
| ¿Antigravity se llamaba antes *Jetski*? | **No como nombre de producto.** *Jetski* es el **nombre interno del árbol de código** (`google3/third_party/jetski/`) que Google heredó del equipo de Windsurf/Codeium. El producto se Called **Antigravity** desde su anuncio público. | **[MEDIDO]** + **[FUENTE]** |
| ¿Fue publicado alguna vez como «Jetski»? | **No consta ninguna publicación con ese nombre.** No hay release, paquete ni registro de nombre «jetski» en ningún canal oficial (Play Store, VS Code Marketplace, Homebrew, npm, PyPI, bucket `antigravity-public`). | **[MEDIDO]** (ausencia en 48 artefactos catalogados + 102 versiones de CLI + 9 plataformas) |
| ¿Siempre fue de Google? | **No.** El linaje es Codeium (empresa, 2022) → Windsurf (rebranding, 2024) → Google DeepMind (licencia de tecnología + contratación, 11 jul 2025) → Antigravity (preview pública, 18 nov 2025). El *language server* que analisamos es **el LS de Codeium/Windsurf forkado**, no código original de Google. | **[FUENTE]** + **[MEDIDO]** (los símbolos del binario lo delatan) |
| ¿Ha tenido otros nombres? | **Sí, muchos, y de tres clases**: nombres de producto (`Antigravity` → `Antigravity IDE` + `Antigravity` Hub + `Antigravity CLI`/`agy`), nombres internos de código (`jetski`, `jetbox`, `cortex`, `cascade`, `exa`, `gaia`, `stubby`, `modelproxy`) y predecesores vivos (`gemini-cli`, Codeium/Windsurf). | **[MEDIDO]** |

---

## 1. Lo que el binario delata (evidencia primaria, [MEDIDO])

Conteos de ocurrencias literales sobre `liblanguage_server.so` (ELF64 aarch64 PIE, Go):

| Literal | Ocurrencias | Qué demuestra |
|---|---:|---|
| `jetski` | **77 079** | El árbol de código se llama así dentro de google3 |
| `cortex` | **29 048** | El agente se llama **Cortex** (paquete `exa.cortex_pb`, `devtools/jetski/telemetry/extensions/Cortex*`) |
| `codeium` | **11 216** | Herencia directa de Codeium (`exa.codeium_common_pb`, `codeium_common_go_proto`) |
| `v1internal` | **7 360** | La API interna de Cloud Code (`/v1internal:*`) |
| `Cascade` | **4 655** | El agente **Cascade** de Windsurf sobrevive como nombre |
| `exa.` (namespaces proto) | **5 480** | «exa» = *codename* de Codeium para su infraestructura |
| `gemini_coder` | **2 464** | Paquete proto propio de Google (`third_party/gemini_coder/proto/trajectory.proto`) |
| `jetbox` | **1 123** | Nombre interno del **Hub** (`jetbox_state_pb`, `jetbox_summaries.pb`) |
| `cloudcode` | **277** | Backend Cloud Code de Google |
| `gaia` | **503** | OAuth interno de Google (`//gaiastaging.corp.google.com/o/oauth2/token`) |
| `stubby` | **76** | Proveedor de autenticación «Stubby» |
| `modelproxy` | **91** | `cloud/developer_experience/cloudcode/pkg/modelproxy` — **paquete interno de Google** |
| `windsurf` | **6** | **Sólo** como identificador de modelo (`gemini-2.5-pro-windsurf`), nunca como marca |

Rutas literales recuperadas del binario (muestra representativa):

```
google3/third_party/jetski/cortex/state/state
google3/third_party/jetski/cortex/tools/tools
google3/third_party/jetski/cortex/utils/utils
third_party/jetski/remoting_pb/remoting.proto
devtools/jetski/telemetry/extensions/CortexExecutionMet…
devtools_jetski_boq_api_proto.InitiateMeshSessionResponse
cloud/developer_experience/cloudcode/pkg/modelproxy/protos/extra_fields.proto
bin/cloud/developer_experience/cloudcode/pkg/modelproxy/mperrors.go
```

**Conclusión**: `jetski` **nunca fue un producto**; es el nombre del *third_party* de google3 que entra con el equipo. El nombre de producto es *Antigravity*. Y `windsurf` apenas sobrevive (6 hits, todos ids de modelo), lo que confirma que el rebranding fue de **capa profunda**.

---

## 2. La cadena de nombres, por capas

### 2.1 Capa de código (la que pesa 77 079 ocurrencias)

| Nombre | Origen | Estado actual |
|---|---|---|
| **exa** | *codename* interno de Codeium para su infra de extensión | Vive en los namespaces proto: `exa.language_server_pb`, `exa.cortex_pb`, `exa.codeium_common_pb`, `exa.analytics_pb`, `exa.jetski_cortex_pb`, `exa.gemini_coder.*` |
| **codeium** | Nombre comercial de la empresa (2022) | Vive en `codeium_common_pb` / `codeium_common_go_proto` |
| **cascade** | Agente de Windsurf (2024) | 4 655 hits; es el ancestro funcional de «Cortex» |
| **windsurf** | Nombre comercial de Codeium tras el rebranding | Prácticamente extinto como marca (6 hits, ids de modelo) |
| **jetski** | *third_party* de google3 | Dominante: 77 079 hits; es el «sello» del código heredado dentro de Google |
| **cortex** | Renombrado del agente en la tránsición a Antigravity | 29 048 hits; el agente actual se llama Cortex |
| **jetbox** | Nombre interno del producto «Hub»/escritorio | `jetbox_state_pb`, `jetbox_summaries.pb`, `--jetbox` |
| **gaia** | OAuth interno de Google (corp) | Flag `-use_test_gaia`, `gaiastaging.corp.google.com` |
| **stubby** | Proveedor de auth alternativo | Flag `-use_stubby_auth`, `-stubby_auth_mode` |
| **modelproxy** | Proxy de modelos de Cloud Code | `cloud/developer_experience/cloudcode/pkg/modelproxy` |
| **gemini_coder** | Paquete Google para el trajectory.proto | `third_party/gemini_coder/proto/trajectory.proto` |

### 2.2 Capa de producto (lo que ve el usuario)

| Superficie | Nombre público | Binario | Contenedor / ID |
|---|---|---|---|
| Editor (fork de VS Code) | **Antigravity IDE** | Electron + `language_server*` | `com.google.antigravity-ide` |
| Escritorio / «Hub» | **Antigravity** (2.0) | Electron + `Resources/bin/language_server` | `com.google.antigravity` |
| Terminal | **Antigravity CLI** (`agy`) | Go monolítico, LS **enlazado dentro** | 9 plataformas |
| Extensión VS Code | **Antigravity** (Google) | `extension.js` minificado | `google.google-antigravity` |
| Móvil | **Antigravity Mobile** (APK de terceros) | `assets/runtime/bin/liblanguage_server.so` | `com.google.antigravity` (paquete Android) |

**Matiz histórico medido en el catálogo**: antes de 2.0, «Antigravity» designaba **el IDE**; a partir de 2.0 el nombre se reparte: «Antigravity IDE» (el fork de VS Code) y «Antigravity» (la app de escritorio/Hub). El Hub macOS que descargamos (`antigravity-hub` 2.35.2) lleva `Antigravity.app/Contents/Resources/bin/language_server` (**sí** lleva LS, a diferencia del IDE 2.8.0, que **no** lo lleva — verificado descargando y listando el contenido).

### 2.3 Capa de predecesores

```
Codeium (empresa, 2022)
  └─► Windsurf (rebranding del editor, 2024)
        └─► Google compra licencia de tecnología + equipo (11 jul 2025, 2.400 MUSD)
              └─► Antigravity (preview pública, 18 nov 2025, junto a Gemini 3 Pro)
                    ├─► Antigravity IDE / Antigravity (Hub) / Antigravity CLI (agy)
                    └─► Antigravity CLI sustituye a gemini-cli (migración anunciada)
```

**[FUENTE]** el traspaso de 2.400 MUSD y la fecha (11 jul 2025) y el anuncio de Antigravity (18 nov 2025) están corroborados por
[aiwiki.ai/wiki/antigravity](https://aiwiki.ai/wiki/antigravity/raw), [afengblog.com](https://afengblog.com/en/blog/antigravity-256k-context-discrepancy.html)
y [seanthegeek.net/doubleagent](https://seanthegeek.net/doubleagent/research/antigravity.html).
La conversación oficial *«Transitioning Gemini CLI to Antigravity CLI»* está en
[google-gemini/gemini-cli#27274](https://github.com/google-gemini/gemini-cli/discussions/27274).

### 2.4 Lo que **NO** se puede afirmar

- **[NO VERIFICADO]** Si hubo algún *build* interno o beta con el nombre visible «Jetski». No hay ninguna publicación con ese nombre en los canales que hemos barrido, pero no puedo descartar artefactos internos no públicos.
- **[NO VERIFICADO]** La fecha exacta en que Google renombró `cascade` → `cortex` dentro del árbol: el binario contiene ambos, lo que prueba coexistencia, no la fecha.
- **[NO VERIFICADO]** Si el *hub* 2.x y el *IDE* 1.x usaron el mismo `language_server` byte a byte: el catálogo dice que hay 14 LS distintos, pero la atribución exacta por superficie requiere cruzar cada payload.

---

## 3. Por qué importa: el LS es el producto

Medido y documentado en `docs/LS_AUTH_ANALYSIS.md`:

- El LS **no** es un componente accesorio: es el núcleo. La app de escritorio es un *shell* Electron que arranca el LS en modo `standalone`/`hub` y abre un `BrowserWindow` contra el puerto local que el LS anuncia.
- Protocolo: **Connect-RPC v1** (gRPC-Web sobre HTTP/2) contra `127.0.0.1`, con token CSRF por sesión.
- Servicio principal: `exa.language_server_pb.LanguageServerService` — **188 métodos** (reconstrucción de la comunidad) — sobre protos `exa.*`, `gemini_coder.*` y `google.internal.cloud.code.v1internal.*`.
- Backend: `/v1internal:fetchAvailableModels`, `/v1internal:streamGenerateContent`, `/v1internal:fetchUserInfo` contra `daily-cloudcode-pa.googleapis.com`.

**Implicación práctica**: cuando la comunidad parchea «el binario `language_server` para forzar `hasValidAuth`», está parcheando **el producto**, no un plugin. Y nuestro parche (15 B en 4 tramos; `LDRB wN,[x0,#8]` → `MOV wN,#1` en `(*PersonalAuthValidator).Validate`, `validator.go:79`) hace exactamente eso: **forzar `AuthResult.HasValidAuth = true`**.

---

## 4. Tabla resumen de equivalencias

| Lo que oyes | Nombre real (código) | Dónde se ve |
|---|---|---|
| «Antigravity» | `jetski` (`google3/third_party/jetski`) | 77 079 hits en el LS |
| «el agente» | `cortex` / `cascade` | 29 048 / 4 655 hits |
| «el LS» | `exa.language_server_pb.LanguageServerService` | 1 210 hits de `exa.language_server` |
| «la app de escritorio» | `jetbox` | `jetbox_state_pb`, `--jetbox` |
| «Windsurf» | `codeium` → `windsurf` (extinto como marca) | 11 216 / 6 hits |
| «la API de modelos» | `cloudcode` + `modelproxy` | 277 / 91 hits |
| «el login» | `gaia` / `stubby` | 503 / 76 hits |

---

## 5. Reproducir las measurements

```bash
cd /workspace/agy-re
for p in jetski cortex codeium cascade exa. gemini_coder jetbox cloudcode gaia stubby modelproxy windsurf; do
  printf '%-14s %s\n' "$p" "$(LC_ALL=C grep -a -o -E "$p" liblanguage_server.so | wc -l)"
done
LC_ALL=C grep -a -o -E '[A-Za-z0-9_./-]{0,40}jetski[A-Za-z0-9_./-]{0,40}' liblanguage_server.so | sort -u | head -25
```

Resultado esperado (2026-11-26): `jetski 77079`, `cortex 29048`, `codeium 11216`,
`cascade 4655` (sin distinguir mayúscula: 6 233), `exa. 5480`, `gemini_coder 2464`,
`jetbox 1123`, `cloudcode 277`, `gaia 503`, `stubby 76`, `modelproxy 91`, `windsurf 6`.
