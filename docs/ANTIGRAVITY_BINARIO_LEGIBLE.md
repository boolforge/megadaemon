# El binario de Antigravity, legible al 100 %

Fuente: `go.funcs.tsv` = export del **pclntab del propio binario**
(`/workspace/agy-re/liblanguage_server.so`, 181.358.800 B, sha256 `f25135e4…70ff27`).
Cada línea trae **dirección, símbolo Go y fichero:línea ORIGINAL**: el binario
**no está ofuscado** (Go conserva los nombres en el `pclntab`), así que esto no es
una aproximación: es el índice que el programa lleva dentro.

## 1. Arquitectura

- **161682 funciones** en **1741 paquetes**, 5383 ficheros Go.
- Antigravity es el producto **Google «Jetski»** (todo bajo `google3/`).

| paquete | funciones |
|---|---:|
| `google3/third_party/golang/connectrpc_com/connect/v/v1/connect` | 7764 |
| `google3/third_party/jetski/language_server_pb/language_server_go_proto` | 6312 |
| `google3/third_party/jetski/cortex_pb/cortex_go_proto` | 5769 |
| `google3/third_party/golang/github_com/openai/openai_go/v/v3/openai` | 4424 |
| `google3/third_party/jetski/codeium_common_pb/codeium_common_go_proto` | 3525 |
| `google3/google/api/service_config_go_proto` | 2652 |
| `google3/third_party/golang/github_com/openai/openai_go/v/v3/responses/responses` | 2262 |
| `google3/google/ai/generativelanguage/v1main_generative_service_go_proto` | 2233 |
| `google3/third_party/jetski/api_server_pb/api_server_go_proto` | 2077 |
| `google3/third_party/jetski/language_server_pb/language_server_go_grpc` | 1969 |
| `google3/third_party/golang/grpc/grpc` | 1890 |
| `google3/third_party/jetski/seat_management_pb/seat_management_go_proto` | 1833 |
| `google3/google/internal/cloud/code/v1internal/v1internal_go_proto` | 1643 |
| `runtime` | 1637 |
| `google3/third_party/golang/google_golang_org/genai/v/v0/google/genai/interactions/models/interactions/interactions` | 1393 |
| `google3/third_party/golang/protobuf/v2/internal/impl/impl` | 1287 |
| `google3/learning/genai/api/localharness/localharness_go_proto` | 1282 |
| `google3/third_party/golang/github_com/openai/openai_go/v/v3/packages/param/param` | 1210 |
| `google3/google/gca/aicode/v1main_remote_agent_proxy_go_proto` | 1142 |
| `google3/google/cloud/aiplatform/master/content_go_proto` | 1131 |
| `type:` | 1099 |
| `google3/google/gca/aicode/v1main_prediction_service_go_proto` | 1081 |
| `google3/google/cloud/aiplatform/v1beta1/content_go_proto` | 1075 |
| `google3/third_party/golang/github_com/modelcontextprotocol/go_sdk/v/v0/mcp/mcp` | 996 |
| `google3/third_party/golang/github_com/antlr4_go/antlr/v/v4/antlr` | 945 |
| `google3/third_party/jetski/language_server/language_server` | 907 |
| `google3/third_party/jetski/extension_server_pb/extension_server_go_proto` | 887 |
| `google3/google/ai/generativelanguage/v1main_content_go_proto` | 859 |

### Los paquetes que importan para el control

- `google3/third_party/jetski/language_server/language_server` — 907 funciones — **el Language Server** (el proceso que habla con la GUI)
- `google3/third_party/jetski/gateway/gateway` — 81 funciones — **el gateway**: `admin_settings.go` (catálogo de modelos, rutas) y `auth_provider.go`
- `google3/google/internal/cloud/code/v1internal/v1internal_go_proto` — 1643 funciones — **el servicio `v1internal`** (el `generateContent` real)
- `google3/google/gca/aicode/v1main_prediction_service_go_proto` — 1081 funciones — **el servicio de predicción** (el modelo que responde)
- `google3/google/gca/aicode/v1main_remote_agent_proxy_go_proto` — 1142 funciones — **el proxy remoto** de agentes
- `google3/third_party/golang/modelcontextprotocol/go_sdk/v0/mcp/mcp` — 0 funciones — **el SDK de MCP** (el plugin `megadaemon-minimax`)
- `google3/third_party/golang/github_com/openai/openai_go/v3/openai` — 0 funciones — **el SDK de OpenAI**: el LS habla OpenAI-compatible
- `google3/third_party/jetski/language_server_pb/language_server_go_proto` — 6312 funciones — **los RPC del LS** (GetAvailableModels, JetboxWriteState, RefreshMcpServers…)

## 2. El catálogo de modelos está en `gateway/admin_settings.go`

Código legible real (símbolo → fichero Go:línea original):


### `third_party/jetski/gateway/ca_cert.go` (5 funciones)

```go
// línea    36  @0x6d95a08  LoadCACertPool
// línea    68  @0x6d95c68  LoadProxyURL
// línea    88  @0x6d95ec8  NewHTTPTransportWithOptions
// línea   119  @0x6d9b1e8  func1
// línea   160  @0x6d96128  ConfigureDefaultTransport
```

### `third_party/jetski/gateway/auth_provider.go` (46 funciones)

```go
// línea    63  @0x6d93dc8  NewAuthProviderWithOptions
// línea    99  @0x6d94188  NewAuthProviderFromSettings
// línea   127  @0x6d94288  AdminSettings
// línea   132  @0x6d9aba8  deferwrap1
// línea   136  @0x6d94388  setLastDiscoveredCatalog
// línea   141  @0x6d9ac28  deferwrap1
// línea   145  @0x6d94448  getLastDiscoveredCatalog
// línea   150  @0x6d9ac68  deferwrap1
// línea   174  @0x6d94548  splitHeaderEntries
// línea   203  @0x6d94998  ParseCustomHeaders
// línea   230  @0x6d94b88  GetTokenSource
// línea   235  @0x6d94ba8  GetCAICProject
// línea   240  @0x6d94bc8  GetGCPLocation
// línea   245  @0x6d94be8  IsBusinessPayGo
// línea   250  @0x6d94bf8  IsBusinessEnterprise
// línea   255  @0x6d94c08  ResolveAuthMode
// línea   260  @0x6d94c28  GetHTTPClient
// línea   265  @0x6d9ace8  deferwrap1
// línea   270  @0x6d94d58  GetCACertPool
// línea   272  @0x6d9ad68  deferwrap1
// línea   277  @0x6d94e48  GetCACertPath
// línea   279  @0x6d9ade8  deferwrap1
// línea   284  @0x6d94f38  GetProxyURL
// línea   286  @0x6d9ae68  deferwrap1
// línea   291  @0x6d95028  GetProxyURLString
// línea   293  @0x6d9aee8  deferwrap1
// línea   298  @0x6d95118  SetHTTPClient
// línea   303  @0x6d9af68  deferwrap1
// línea   308  @0x6d951d8  GetUserAgentName
// línea   314  @0x6d95208  GetAuthMethod
// línea   319  @0x6d95218  GetWIFProvider
// línea   324  @0x6d95228  GetUserTier
// línea   328  @0x6d95248  SetUserTier
// línea   331  @0x6d95258  GetEndpointURL
// línea   336  @0x6d9afa8  deferwrap1
// línea   341  @0x6d95368  SetEndpointURL
// línea   346  @0x6d9b028  deferwrap1
// línea   351  @0x6d95438  GetWireProtocol
// línea   353  @0x6d9b068  deferwrap1
// línea   358  @0x6d95528  SetWireProtocol
// línea   360  @0x6d9b0e8  deferwrap1
// línea   366  @0x6d955e8  IsEval
// línea   371  @0x6d955f8  ShouldUseCAICProjectAsOverride
// línea   375  @0x6d95608  SetHTTPHeaders
// línea   380  @0x6d9b128  deferwrap1
// línea   395  @0x6d959f8  GetEntitlement
```

### `third_party/jetski/gateway/admin_settings.go` (16 funciones)

```go
// línea    66  @0x6d92368  IsEnabled
// línea    71  @0x6d92388  IsRouteWebFetch
// línea    77  @0x6d923a8  IsGatewayEnabled
// línea    85  @0x6d923d8  AdminFileSetsModels
// línea   117  @0x6d923f8  defaultAdminSettingsPathForOS
// línea   137  @0x6d92528  defaultUserSettingsPath
// línea   149  @0x6d925c8  Validate
// línea   187  @0x6d92a88  ParseAdminSettings
// línea   202  @0x6d92c28  ParseEnvOverrides
// línea   262  @0x6d92ee8  MergeSettings
// línea   303  @0x6d93048  mergeInto
// línea   315  @0x6d93178  mergeGateway
// línea   349  @0x6d934f8  mergeModels
// línea   384  @0x6d93858  LoadDefaultEffectiveSettings
// línea   392  @0x6d938e8  LoadEffectiveSettings
// línea   427  @0x6d93ce8  loadSettingsFile
```

### `third_party/jetski/gateway/model_discovery.go` (14 funciones)

```go
// línea    54  @0x6d96228  redactURL
// línea    93  @0x6d962a8  GetModelCatalog
// línea   168  @0x6d969a8  AdminFileSetsModels
// línea   172  @0x6d969f8  isImageModel
// línea   184  @0x6d96c28  findKnownModelInfo
// línea   204  @0x6d96cc8  isSupportedMimeType
// línea   224  @0x6d96df8  modelEntryToModelDetails
// línea   406  @0x6d978e8  buildResponseFromAdminModelList
// línea   556  @0x6d98a28  effectivePort
// línea   570  @0x6d98af8  shouldAttachGatewayHeaders
// línea   592  @0x6d98c68  fetchRemoteModelCatalog
// línea   623  @0x6d9b1a8  deferwrap1
// línea   654  @0x6d994a8  fetchGatewayDiscoveredModels
// línea   858  @0x6d9aad8  buildDefaultGatewayCatalog
```

## 3. Los RPC del Language Server (los que usa el `md-antigravity`)

- **GetAvailableModels** — `google3_third_party_jetski_language_server_pb_language_server_go_proto._LanguageServerService_GetAvailableModels_Handler` @`0x5b977f8` (blaze-out/arm-opt/bin/third_party/jetski/language_server_pb/language_server.pb.go:89030)
- **GetCascadeModelConfigData** — `google3_third_party_jetski_language_server_pb_language_server_go_proto._LanguageServerService_GetCascadeModelConfigData_Handler` @`0x5b83aa8` (blaze-out/arm-opt/bin/third_party/jetski/language_server_pb/language_server.pb.go:86277)
- **GetAllCascadeTrajectories** — `google3_third_party_jetski_language_server_pb_language_server_go_proto._LanguageServerService_GetAllCascadeTrajectories_Handler` @`0x5b84f48` (blaze-out/arm-opt/bin/third_party/jetski/language_server_pb/language_server.pb.go:86457)
- **GetCascadeTrajectorySteps** — `google3_third_party_jetski_language_server_pb_language_server_go_proto._LanguageServerService_GetCascadeTrajectorySteps_Handler` @`0x5b8cc18` (blaze-out/arm-opt/bin/third_party/jetski/language_server_pb/language_server.pb.go:87538)
- **GetCascadeNuxes** — `google3_third_party_jetski_language_server_pb_language_server_go_proto._LanguageServerService_GetCascadeNuxes_Handler` @`0x5b91188` (blaze-out/arm-opt/bin/third_party/jetski/language_server_pb/language_server.pb.go:88143)
- **GetCapabilities** — `google3_third_party_jetski_language_server_pb_language_server_go_proto._LanguageServerService_GetCapabilities_Handler` @`0x5b9ac38` (blaze-out/arm-opt/bin/third_party/jetski/language_server_pb/language_server.pb.go:89484)
- **GetUserSettings** — `google3_third_party_jetski_language_server_pb_language_server_go_proto._LanguageServerService_GetUserSettings_Handler` @`0x5b7e2a8` (blaze-out/arm-opt/bin/third_party/jetski/language_server_pb/language_server.pb.go:85510)
- **SetUserSettings** — `google3_third_party_jetski_language_server_pb_language_server_go_proto._LanguageServerService_SetUserSettings_Handler` @`0x5b7e4b8` (blaze-out/arm-opt/bin/third_party/jetski/language_server_pb/language_server.pb.go:85528)
- **RegisterGdmUser** — `google3_third_party_jetski_language_server_pb_language_server_go_proto._LanguageServerService_RegisterGdmUser_Handler` @`0x5b73b98` (blaze-out/arm-opt/bin/third_party/jetski/language_server_pb/language_server.pb.go:84055)
- **JetboxWriteState** — `google3_third_party_jetski_language_server_pb_language_server_go_proto._LanguageServerService_JetboxWriteState_Handler` @`0x5b95648` (blaze-out/arm-opt/bin/third_party/jetski/language_server_pb/language_server.pb.go:88741)
- **RefreshMcpServers** — `google3_third_party_jetski_language_server_pb_language_server_go_proto._LanguageServerService_RefreshMcpServers_Handler` @`0x5b871a8` (blaze-out/arm-opt/bin/third_party/jetski/language_server_pb/language_server.pb.go:86753)

## 4. Cómo se usa (el motor `re_engine` de MegaDaemon)

```bash
md-re catalogo                       # qué sabe hacer el motor
md-re identify  <binario>            # formato + evidencia
md-re analyze   <binario>            # informe uniforme
md-re deobfuscate <binario>          # MBA + CFF + crypto + opacos
```
`rust-core/src/gobook.rs` es el **libro**: `mapa()` (arquitectura),
`buscar()` (anchors por nombre), `esqueleto()` (el código por fichero y línea).
Motor de deofuscación: `re_engine` (el del agente) + `re_strings`, `re_ir`,
`re_cff` (control-flow flattening con prueba diferencial), `re_crypto` y
`re_opaque`, con tests contra **fragmentos reales** de los binarios.
