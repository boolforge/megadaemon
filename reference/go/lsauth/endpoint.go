package lsauth

import "strings"

// V1InternalPrefix es el segmento de ruta que el LS antepone a todos los
// métodos del backend de Code Assist.
//
// # Dónde vive en el binario (VERIFICADO)
//
// No es un literal en `.rodata` referenciado por el código: es una variable
// GLOBAL de cadena de Go (un header {ptr,len}) en `0x799ff50`:
//
//	0x799ff50  puntero -> 0x42a68b1   (relocación R_AARCH64_RELATIVE)
//	0x799ff58  longitud = 10
//
// y los bytes en 0x42a68b1 son "v1internal", fusionados con el literal mayor
// "v1internalos_type=%s%s/%s (%s)User-Agent" (Go fusiona cadenas contiguas).
//
// La referencia se hace en 4 funciones, y en ninguna más:
//
//	0x66c66cc  constructCodeAssistEndpoint (http_helpers.go:153)
//	0x66c699c  constructResourceURL      (http_helpers.go:177)
//	0x66c6be4  constructJetskiEndpoint    (http_helpers.go:204)
//	0x66c6cf4  constructJetskiResourceURL (http_helpers.go:213)
//
// Las cuatro hacen, literalmente:
//
//	adrp x4, 0x799f000
//	add  x4, x4, 0xf50        ; -> 0x799ff50
const V1InternalPrefix = "v1internal"

// Templates de construcción de URL (VERBATIM del binario).
const (
	// TemplateMethod es la plantilla de `constructCodeAssistEndpoint`
	// (0x66c6680) y de `constructJetskiEndpoint` (0x66c6ba0):
	//
	//	0x66c66ec  adrp x0, 0x4279000
	//	0x66c66f0  add  x0, x0, 0xa33   ; -> 0x4279a33
	//	0x66c66f4  orr  x1, xzr, 8      ; longitud 8
	//
	// Los 8 bytes en 0x4279a33 son "%s/%s:%s".
	TemplateMethod = "%s/%s:%s"

	// TemplateResource es la plantilla de `constructResourceURL`
	// (0x66c6950) y de `constructJetskiResourceURL` (0x66c6cb0):
	//
	//	0x66c69bc  adrp x0, 0x4279000
	//	0x66c69c0  add  x0, x0, 0xa3b   ; -> 0x4279a3b
	//	0x66c69c4  orr  x1, xzr, 8
	//
	// Los 8 bytes en 0x4279a3b son "%s/%s/%s".
	TemplateResource = "%s/%s/%s"
)

// BackendMethod es un método del backend de Code Assist.
//
// # El nombre del método es una CONSTANTE GO (VERIFICADO)
//
// Cada llamada al backend pasa el nombre del método como literal `.rodata`.
// Las literals de Go se fusionan en blobs, así que la VA apunta al inicio
// del nombre dentro de un blob mayor; la longitud es la del nombre, no la
// del blob.
//
// La tabla de abajo se ha verificado contra el par (VA, longitud) que el
// código realmente usa: la longitud se lee del inmediato que acompaña al
// ADRP+ADD (p. ej. `orr x5, xzr, 0x18` = 24 para
// "retrieveUserQuotaSummary").
type BackendMethod struct {
	// Name es el nombre del método, tal y como va en la URL.
	Name string
	// LiteralVA es la VA real del literal en `.rodata`.
	LiteralVA uint64
	// LiteralLen es la longitud del literal que usa el código.
	LiteralLen int
	// BuiltBy es la función del LS que lo pasa.
	BuiltBy string
	// FileLine es el fichero:línea original del Go de Google.
	FileLine string
}

// BackendMethods es el catálogo de nombres de método verificados.
//
// Cubre el camino de autenticación completo más las llamadas de generación
// (que comparten el mismo constructor de URL).
var BackendMethods = []BackendMethod{
	{"loadCodeAssist", 0x430116e, 14, "(*CodeAssistClient).fetchLoadCodeAssistResponse", "user_management.go:115"},
	{"onboardUser", 0x42be8e4, 11, "(*CodeAssistClient).OnboardUser", "user_management.go:250"},
	{"fetchUserInfo", 0x42ec2f7, 13, "(*CodeAssistClient).fetchUserInfo", "user_management.go:513"},
	{"listExperiments", 0x4313d0c, 15, "(*CodeAssistClient).ListExperiments", "code_assist_client.go:128"},
	{"fetchAvailableModels", 0x4366c57, 20, "(*CodeAssistClient).fetchAvailableModels", "model_configs.go:277"},
	{"fetchAdminControls", 0x4348106, 18, "(*CodeAssistClient).fetchAdminControls", "user_management.go:364"},
	{"fetchConfig", 0x42be860, 11, "(*CodeAssistClient).fetchBAICAdminControls", "businessaicode.go:246"},
	{"entitlement.userTier", 0x4366c1b, 20, "(*CodeAssistClient).fetchBAICAdminControls", "businessaicode.go:246"},
	{"generateContent", 0x4313ca3, 15, "(*CodeAssistClient).generateContentFromCCPA", "generation.go:1407"},
	{"streamGenerateContent", 0x4375583, 21, "(*CodeAssistClient).streamGenerateContentFromCCPA", "generation.go:1216"},
	{"setUserSettings", 0x4313d75, 15, "(*CodeAssistClient).SetUserSettings", "user_management.go:400"},
	{"rewriteUri", 0x42a690b, 10, "(*CodeAssistClient).GetWebSearchRedirect", "web.go:38"},
	{"retrieveUserQuotaSummary", 0x43a6942, 24, "(*CodeAssistClient).fetchRetrieveUserQuotaSummary", "user_management.go:629"},
}

// MethodNames devuelve el conjunto de nombres de método del catálogo.
func MethodNames() []string {
	out := make([]string, 0, len(BackendMethods))
	for _, m := range BackendMethods {
		out = append(out, m.Name)
	}
	return out
}

// EndpointURL construye la URL de un método del backend de Code Assist.
//
// Es el equivalente exacto de `constructCodeAssistEndpoint`
// (http_helpers.go:153, 0x66c6680):
//
//	fmt.Sprintf("%s/%s:%s", baseURL, "v1internal", method)
//
// es decir:
//
//	https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist
//
// # El host base NO está cableado en el binario (VERIFICADO)
//
// "cloudcode-pa.googleapis.com" (0x440be2c, 0x442f8af) y
// "cloudcode-pa.sandbox.googleapis.com" (0x445fac6, 0x446fdfc) tienen CERO
// referencias por ADRP desde el código y CERO relocaciones que los apunten:
// el host entra como PARÁMETRO en el constructor del proveedor
// (`NewStandaloneAuthProvider`, 0x66afcd0, lo copia al campo +0x50, que es
// el que devuelve `GetEndpointURL`).
//
// ⇒ El host real depende de la configuración o del Antigravity Hub, no del
// binario. Por eso `base` es un parámetro y no una constante.
func EndpointURL(base, method string) string {
	return base + "/" + V1InternalPrefix + ":" + method
}

// ResourceURL construye la URL de un recurso del backend.
//
// Es el equivalente de `constructResourceURL` (http_helpers.go:177,
// 0x66c6950):
//
//	fmt.Sprintf("%s/%s/%s", baseURL, "v1internal", resource)
func ResourceURL(base, resource string) string {
	return base + "/" + V1InternalPrefix + "/" + resource
}

// BackendHost describe el host effective contra el que se habla.
type BackendHost struct {
	// Base es la URL base, p. ej. "https://cloudcode-pa.googleapis.com".
	Base string
}

// EndpointURL construye la URL de un método sobre este host.
func (h BackendHost) EndpointURL(method string) string { return EndpointURL(h.Base, method) }

// ResourceURL construye la URL de un recurso sobre este host.
func (h BackendHost) ResourceURL(resource string) string { return ResourceURL(h.Base, resource) }

// IsMethodName dice si name es un método conocido del backend.
//
// Útil para validar que un proxy que reescribe URLs no está construyendo
// rutas raras.
func IsMethodName(name string) bool {
	for _, m := range BackendMethods {
		if m.Name == name {
			return true
		}
	}
	return false
}

// MethodFromURL extrae el nombre de método de una URL del backend.
//
// Reconoce el patrón "<base>/v1internal:<método>". Devuelve ("", false) si
// la URL no es de ese tipo.
func MethodFromURL(rawURL string) (string, bool) {
	i := strings.LastIndex(rawURL, "/"+V1InternalPrefix+":")
	if i < 0 {
		return "", false
	}
	return rawURL[i+len(V1InternalPrefix)+2:], true
}
