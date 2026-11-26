package lsauth

// AuthMode es el "modo de autenticación" que el Language Server publica en
// el campo `auth_mode` (15) del `AuthResult`.
//
// Verificado contra el descriptor proto embebido en el binario
// (`exa.language_server_pb`), no contra nombres de símbolos: el enum vive
// como bytes en el `FileDescriptorProto` raw, así que la lista de valores de
// abajo es la que decodifica el runtime de protobuf.
type AuthMode int32

const (
	// AuthModeUnspecified es el valor cero: el LS no sabe (o no quiere
	// decir) en qué modo está.
	AuthModeUnspecified AuthMode = 0
	// AuthModePersonalConsumer es la cuenta personal del usuario final.
	AuthModePersonalConsumer AuthMode = 1
	// AuthModeBusinessLicensedEnterprise es una organización con licencia
	// de empresa (Gemini Enterprise / Code Assist Enterprise).
	AuthModeBusinessLicensedEnterprise AuthMode = 2
	// AuthModeBusinessAgentPlatformPayGo es "Agent Platform" con pago por
	// uso (pay-as-you-go).
	//
	// Nota de versión: una versión anterior de este informe asignaba el
	// valor 5 a este modo. Es un error: el valor 5 es
	// AuthModeEnterpriseGateway, que es lo que escriben los stubs de
	// (*GatewayAuthClient) (0x70e1748, 0x70e1798, 0x70e18a8, 0x70e1918).
	AuthModeBusinessAgentPlatformPayGo AuthMode = 3
	// AuthModeGeminiAPIKey es autenticación por clave de API de Gemini.
	AuthModeGeminiAPIKey AuthMode = 4
	// AuthModeEnterpriseGateway es el modo del gateway de empresa
	// (Enterprise Gateway): un proxy corporativo en medio del camino.
	AuthModeEnterpriseGateway AuthMode = 5
)

// String devuelve el nombre del enum tal y como aparece en el descriptor
// proto. Es el mismo texto que el runtime de protobuf generaría para el
// enum, y por tanto el que un cliente vería en un volcado textual.
func (m AuthMode) String() string {
	switch m {
	case AuthModeUnspecified:
		return "AUTH_MODE_UNSPECIFIED"
	case AuthModePersonalConsumer:
		return "AUTH_MODE_PERSONAL_CONSUMER"
	case AuthModeBusinessLicensedEnterprise:
		return "AUTH_MODE_BUSINESS_LICENSED_ENTERPRISE"
	case AuthModeBusinessAgentPlatformPayGo:
		return "AUTH_MODE_BUSINESS_AGENT_PLATFORM_PAYGO"
	case AuthModeGeminiAPIKey:
		return "AUTH_MODE_GEMINI_API_KEY"
	case AuthModeEnterpriseGateway:
		return "AUTH_MODE_ENTERPRISE_GATEWAY"
	default:
		return "AUTH_MODE_UNKNOWN"
	}
}

// AuthResult es el estado de autenticación que el Language Server devuelve a
// la aplicación (el IDE) por el RPC GetAuthStatus.
//
// # Layout en memoria (VERIFICADO, no supuesto)
//
// El struct real en el binario mide 168 bytes (0xA8) y sus campos están en
// estos offsets. No es una suposición: sale de tres fuentes independientes
// que se cruzaron (ver LS_AUTH_ANALYSIS.md §11):
//
//  1. El `runtime._type` de *AuthResult (0x8ddad20): Size_ = 0xA8 = 168,
//     y su slice `Fields` (+56) con la tabla de offsets.
//  2. Los *getters* generados (`GetHasValidAuth`, `GetUiMessage`, …), cada
//     uno leyendo exactamente su offset.
//  3. `(*AuthResult).Reset` (0x5b36380), que hace `memclr` de 164 bytes
//     (168 − 4 del sizeCache final).
//
// | Offset | Campo                | Tipo Go                    |
// |--------|----------------------|----------------------------|
// | 0x00   | state                | protoimpl.MessageState (8) |
// | 0x08   | has_valid_auth       | bool                       |
// | 0x10   | ui_message           | string (16 B)              |
// | 0x20   | is_gcp_tos           | bool                       |
// | 0x28   | project_id           | string (16 B)              |
// | 0x38   | location             | string (16 B)              |
// | 0x48   | failure_details      | interfaz (oneof, 16 B)    |
// | 0x58   | granted_scopes       | []string (24 B)            |
// | 0x70   | wif_provider         | string (16 B)              |
// | 0x80   | auth_mode            | int32                      |
// | 0x88   | unknown_fields       | []byte (24 B)              |
// | 0xA0   | size_cache           | int32                      |
//
// La cabecera protobuf NO va al principio: el `MessageState` ocupa los
// primeros 8 bytes y `sizeCache`/`unknownFields` van AL FINAL. (La hipótesis
// de una cabecera de 40 bytes era falsa.)
//
// # Tags proto
//
//	has_valid_auth = 1, ui_message = 2, is_gcp_tos = 3, project_id = 11,
//	location = 12, failure_details = 13 (oneof), granted_scopes = 14,
//	wif_provider = 15, auth_mode = 16
type AuthResult struct {
	// HasValidAuth es el booleano que la aplicación lee para decidir si
	// mostrar el prompt de login.
	//
	// VERIFICADO: lo escriben 12 sitios del propio LS (ver
	// lsauth.HasValidAuthWriters). La versión v2 de este informe decía que
	// el LS no lo escribía; era un fallo del barrido, no del binario.
	HasValidAuth bool `protobuf:"varint,1,opt,name=has_valid_auth,json=hasValidAuth,proto3" json:"has_valid_auth,omitempty"`

	// UIMessage es el texto que la aplicación puede pintar al usuario.
	// No es un mensaje de error estructurado: es texto libre.
	UIMessage string `protobuf:"bytes,2,opt,name=ui_message,json=uiMessage,proto3" json:"ui_message,omitempty"`

	// IsGcpTos indica si el usuario ha aceptado los Términos de Servicio
	// de Google Cloud.
	//
	// Se copia de `OAuthTokenInfo.IsGcpTos`, que vive en el offset +0x40
	// de ese mensaje (desensamblado de `(*Server).GetAuthStatus`,
	// 0x70f94dc: `ldrb w3,[x0,0x40]`).
	IsGcpTos bool `protobuf:"varint,3,opt,name=is_gcp_tos,json=isGcpTos,proto3" json:"is_gcp_tos,omitempty"`

	// ProjectID es el identificador del proyecto de Google Cloud con el
	// que se trabaja (el "cloudaicompanion_project" del backend).
	ProjectID string `protobuf:"bytes,11,opt,name=project_id,json=projectId,proto3" json:"project_id,omitempty"`

	// Location es la región del proyecto (p. ej. "us-central1").
	Location string `protobuf:"bytes,12,opt,name=location,proto3" json:"location,omitempty"`

	// FailureDetails es el "oneof" con el motivo concreto del fallo.
	// Es nil cuando el resultado es válido.
	//
	// VERIFICADO: hay 7 variantes, con itab en 0x8f12448…0x8f12508
	// (ver lsauth.FailureKind y el catálogo de variantes).
	FailureDetails FailureDetails `protobuf_oneof:"failure_details"`

	// GrantedScopes son los scopes OAuth concedidos.
	GrantedScopes []string `protobuf:"bytes,14,rep,name=granted_scopes,json=grantedScopes,proto3" json:"granted_scopes,omitempty"`

	// WifProvider identifica al proveedor de credenciales de workstation
	// (Workstation Identity Federation) cuando ése es el mecanismo.
	WifProvider string `protobuf:"bytes,15,opt,name=wif_provider,json=wifProvider,proto3" json:"wif_provider,omitempty"`

	// AuthMode es el modo de autenticación (§AuthMode).
	AuthMode AuthMode `protobuf:"varint,16,opt,name=auth_mode,json=authMode,proto3,enum=exa.language_server_pb.AuthMode" json:"auth_mode,omitempty"`
}

// FailureDetails es la interfaz del "oneof" `failure_details`.
//
// En el binario es una interfaz con 7 implementaciones concretas, cada una
// con su itab. La interfaz se define aquí en Go puro: no hace falta protobuf
// para modelar el comportamiento, y así el paquete es utilizable desde
// MegaDaemon sin arrastrar el runtime de protobuf.
type FailureDetails interface {
	// Kind devuelve el discriminante del oneof.
	Kind() FailureKind
	// reason devuelve el motivo legible, si la variante lo tiene.
	//
	// En el binario esto NO es un método: el runtime de protobuf genera
	// un getter por campo (`GetReason`, `GetAuthURL`, …). Se modela aquí
	// como un método para que el código que consume el estado no tenga que
	// hacer type switches.
	reason() string
}

// FailureKind enumera las variantes del oneof `failure_details`.
type FailureKind int32

const (
	// FailureNone es "no hay motivo" (resultado válido).
	FailureNone FailureKind = 0
	// FailureIneligible: el tier del usuario no es elegible para el
	// servicio (el backend devolvió `ineligible_tiers`).
	FailureIneligible FailureKind = 1
	// FailureVerificationRequired: falta verificación de identidad.
	FailureVerificationRequired FailureKind = 2
	// FailureTosViolation: se ha violado un término de servicio.
	FailureTosViolation FailureKind = 3
	// FailureGeneralError: error genérico, con `reason` libre.
	FailureGeneralError FailureKind = 4
	// FailureProjectRequired: falta elegir/crear proyecto.
	FailureProjectRequired FailureKind = 5
	// FailureHeadlessAuthRequired: se requiere autenticación en navegador.
	FailureHeadlessAuthRequired FailureKind = 6
	// FailureLicenseRequired: falta licencia.
	FailureLicenseRequired FailureKind = 7
)

// String devuelve el nombre de la variante tal como lo nombra el proto.
func (k FailureKind) String() string {
	switch k {
	case FailureNone:
		return "none"
	case FailureIneligible:
		return "ineligible"
	case FailureVerificationRequired:
		return "verification_required"
	case FailureTosViolation:
		return "tos_violation"
	case FailureGeneralError:
		return "general_error"
	case FailureProjectRequired:
		return "project_required"
	case FailureHeadlessAuthRequired:
		return "headless_auth_required"
	case FailureLicenseRequired:
		return "license_required"
	default:
		return "unknown"
	}
}

// HasValidAuthWriter es un sitio del binario que escribe el booleano
// `has_valid_auth` del `AuthResult`.
type HasValidAuthWriter struct {
	// VA es la dirección real (PBI) de la instrucción que escribe.
	VA uint64
	// Function es el símbolo Go (nombre completo) del sitio.
	Function string
	// FileLine es el fichero:línea original del Go de Google.
	FileLine string
	// Note explica el papel del sitio.
	Note string
}

// HasValidAuthWriters son los 12 sitios verificados que escriben el
// booleano `has_valid_auth`.
//
// VERIFICADO por un barrido exhaustivo de las instrucciones `STRB [Xn,#8]`
// de todo `.text` (645 546 ADRP revisados; 21 candidatos en el área de auth,
// de los cuales 12 son escritores reales y 9 falsos positivos). Los falsos
// positivos que se descartaron son, para que quede constancia:
//
//	0x70e20c0, 0x70e2830  longitud de una `string` de 16 B
//	0x70e1ed4, 0x70e2014  longitud de una `string`
//	0x70e47c8            longitud de un campo `string`
//	0x70e64e0            store a la pila (no a un AuthResult)
//	0x70f99dc, 0x70f9b30, 0x70f9c7c  otro mensaje, no el AuthResult
//	0x712c78c            store a la pila
//
// `buildErrorResp` NO está en la lista: su `AuthResult` nace a cero y por
// tanto `has_valid_auth` queda en `false` sin escribir nada.
var HasValidAuthWriters = []HasValidAuthWriter{
	{0x70f9514, "(*Server).GetAuthStatus", "rpcs_auth.go:15",
		"punto de entrada de la app: escribe true de forma incondicional si hay OAuthTokenInfo con token"},
	{0x70fae5c, "(*Server).cliSessionAuthResult", "rpcs_auth.go",
		"sesión CLI: construye el resultado para el modo línea de comandos"},
	{0x70e1748, "(*GatewayAuthClient).<stub>", "auth_client/auth_client.go",
		"stub del gateway de empresa: bool=true, is_gcp_tos=true, AuthMode=5 (ENTERPRISE_GATEWAY)"},
	{0x70e1798, "(*GatewayAuthClient).<stub>", "auth_client/auth_client.go",
		"stub del gateway de empresa (2º punto de entrada)"},
	{0x70e18a8, "(*GatewayAuthClient).<stub>", "auth_client/auth_client.go",
		"stub del gateway de empresa (3º punto de entrada)"},
	{0x70e1918, "(*GatewayAuthClient).<stub>", "auth_client/auth_client.go",
		"stub del gateway de empresa (4º punto de entrada)"},
	{0x70e4234, "(*AuthClient).ValidateProject", "auth_client/auth_client.go:696",
		"el proyecto elegido por el usuario es válido: el booleano lo pone el camino de éxito"},
	{0x70e527c, "(*AuthClient).SelectLicense", "auth_client/auth_client.go:860",
		"selección de licencia correcta"},
	{0x70e59f4, "(*InternalAuthValidator).Validate", "auth_client/validator.go:39",
		"perfil interno: credenciales válidas"},
	{0x70e5b54, "(*PersonalAuthValidator).Validate", "auth_client/validator.go:79",
		"perfil personal: credenciales válidas"},
	{0x70e5d68, "(*EnterpriseAuthValidator).Validate", "auth_client/validator.go:122",
		"perfil empresa: credenciales válidas"},
	{0x70e62c0, "ValidateAndOnboardAccount", "auth_client/validator.go:185",
		"onboarding completado: cuenta onboarded y con tier válido"},
}
