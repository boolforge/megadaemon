package lsauth

import (
	"errors"
	"fmt"
	"net/http"
)

// BackendError es el error que devuelve la capa HTTP del backend de Code
// Assist cuando la respuesta no es 200.
//
// # Lo que la v3 NO sabía y la v4 sí (VERIFICADO)
//
// En todo el paquete `code_assist_client` hay UNA SOLA comparación contra un
// código HTTP, y es contra 200:
//
//	doPostAndUnmarshal (http_helpers.go:403, 0x66e8b38)
//	    0x66e8b38  cmp x11, 0xc8        ; x11 = http.Response.StatusCode
//	    0x66e8b3c  b.eq 0x66e8d84       ; 200 -> camino feliz
//	    ...                           ; cualquier otro -> este error
//
//	doGetAndUnmarshal (http_helpers.go:374, 0x66e8f9c)
//	    ... cmp x11, 0xc8 ...
//
// Es decir: el backend NO distingue 401 de 403 de 404. Todos caen en el mismo
// `else` y producen el mismo error. La única información que sobrevive es el
// código, que se conserva aquí para poder clasificarlo después (ver
// IsTerminalCredentialRejection).
type BackendError struct {
	// Status es el código HTTP de la respuesta.
	Status int
	// Method es el verbo HTTP de la petición fallida.
	Method string
	// URL es la URL pedida (ya construida: host + "/v1internal:" + método).
	URL string
	// Body es el cuerpo de la respuesta, recortado. El LS original lo
	// registra entero en el log; aquí se limita para no arrastrar megabytes.
	Body string
	// Reason es el motivo legible que el backend devolvió (vacío si no lo
	// hubo). Es el campo que la regla de IsTerminalCredentialRejection
	// compara con "invalid_grant".
	Reason string
}

// NewBackendError construye el error de una respuesta no-200.
//
// `body` se recorta a maxBodyBytes para no arrastrar payloads grandes.
func NewBackendError(status int, method, url, reason, body string) *BackendError {
	const maxBodyBytes = 4096
	if len(body) > maxBodyBytes {
		body = body[:maxBodyBytes] + "…(recortado)"
	}
	return &BackendError{
		Status: status,
		Method: method,
		URL:    url,
		Body:   body,
		Reason: reason,
	}
}

// Error implementa error.
func (e *BackendError) Error() string {
	if e.Body == "" {
		return fmt.Sprintf("backend %s %s: HTTP %d", e.Method, e.URL, e.Status)
	}
	return fmt.Sprintf("backend %s %s: HTTP %d: %s", e.Method, e.URL, e.Status, e.Body)
}

// ErrNotOK es el error genérico que el LS original produce cuando el status
// no es 200. Se mantiene separado para poder comparar con errores
// tipados sin depender del texto.
var ErrNotOK = errors.New("lsauth: respuesta del backend con status distinto de 200")

// IsNotOK devuelve true si err representa una respuesta no-200 del backend.
//
// Equivale a `errors.Is(err, ErrNotOK)` para los errores que produce este
// paquete, y además reconoce un *BackendError construido a mano.
func IsNotOK(err error) bool {
	if errors.Is(err, ErrNotOK) {
		return true
	}
	var be *BackendError
	return errors.As(err, &be)
}

// StatusOf extrae el código HTTP de un error del backend, o 0 si no lo tiene.
func StatusOf(err error) int {
	var be *BackendError
	if errors.As(err, &be) {
		return be.Status
	}
	return 0
}

// ReasonOf extrae el "reason" de un error del backend.
//
// El backend de Google devuelve el motivo OAuth en el cuerpo de la respuesta
// (típicamente `error_description` o el campo `reason` de un
// `google.rpc.Status`), y el LS lo propaga en el campo `reason` de la
// variante `general_error`. Aquí se modela como un campo explícito en el
// error, que es lo que permite luego aplicar la regla de
// IsTerminalCredentialRejection.
func ReasonOf(err error) string {
	var be *BackendError
	if errors.As(err, &be) {
		return be.Reason
	}
	return ""
}

// IsTerminalCredentialRejection decide si un error del backend debe
// tratarse como RECHAZO TERMINAL de las credenciales.
//
// # La regla exacta (VERIFICADA por desensamblado)
//
//	isTerminalCredentialRejection (auth_client/auth_client.go:303, 0x70e1c10)
//
//	0x70e1c74  cmp  x3, 0xd            ; len(reason) == 13 ?
//	0x70e1c80  mov  x4, 0x6e69         ; "in"
//	0x70e1c84  movk x4, 0x6176, lsl 16 ; "va"
//	0x70e1c88  movk x4, 0x696c, lsl 32 ; "li"
//	0x70e1c8c  movk x4, 0x5f64, lsl 48 ; "d_"
//	0x70e1c90  cmp  x3, x4              ; "invalid_"
//	0x70e1c9c  mov  x4, 0x7267         ; "gr"
//	0x70e1ca0  movk x4, <"an">, lsl 16
//	0x70e1ca4  cmp  w3, w4              ; "gran"
//	0x70e1cb0  cmp  w2, 0x74            ; 't'
//	0x70e1cb8  orr  x0, xzr, 1          ; -> VEREDICTO: terminal
//
//	0x70e1cd4  sub  x2, x1, 0x190       ; status - 400
//	0x70e1cd8  cmp  x2, 0x64            ; (status-400) < 100 ?   [sin signo]
//	0x70e1cdc  b.hs 0x70e1cf4           ; fuera de 4xx -> NO terminal
//	0x70e1ce0  cmp  x1, 0x1ad           ; 429 (cuota) -> NO terminal
//	0x70e1ce4  b.eq 0x70e1cf4
//	0x70e1ce8  cmp  x1, 0x198           ; 408 (timeout)
//	0x70e1cec  cset x0, ne              ; terminal si status != 408
//
// Es decir:
//
//	terminal  <=>  reason == "invalid_grant"
//	            OR  (400 <= status <= 499 AND status != 429 AND status != 408)
//
// # Consecuencias importantes
//
//   - **404 se trata EXACTAMENTE como 401 y 403**: los tres son 4xx, los tres
//     son rechazo terminal. Un 404 del backend (proyecto que ya no existe,
//     método no desplegado en la región, etc.) se le presenta al usuario como
//     "sin autenticación" y provoca un Logout().
//   - **Sólo 408 (timeout) y 429 (cuota) son transitorios**: no disparan el
//     Logout(), de modo que un pico de 429 no tira la sesión del usuario.
//   - El código HTTP **no se propaga** al `AuthResult`: el usuario ve
//     `has_valid_auth=false` y un `general_error`, sin saber si fue un 401 o
//     un 404.
func IsTerminalCredentialRejection(reason string, status int) bool {
	// "invalid_grant" (13 bytes) es el motivo canónico de un refresh token
	// revocado o expirado: no tiene sentido reintentar.
	if reason == InvalidGrantReason {
		return true
	}
	// Rango 4xx (400..499) comprobado SIN signo: `status-400` debe ser < 100.
	if status < 400 || status > 499 {
		return false
	}
	// 429 = cuota: transitorio. 408 = timeout: transitorio.
	if status == http.StatusTooManyRequests || status == http.StatusRequestTimeout {
		return false
	}
	return true
}

// InvalidGrantReason es el motivo OAuth que el LS trata como rechazo
// terminal aunque el status HTTP no sea 4xx.
//
// La comparación en el binario es por longitud (13) y por bytes, no por
// comparación de cadenas completa: el runtime optimiza "invalid_grant" a una
// comparación de 8 bytes + 4 bytes + 1 byte.
const InvalidGrantReason = "invalid_grant"

// TerminalRejection es el resultado de clasificar un error del backend.
type TerminalRejection bool

// Booleano del resultado.
func (t TerminalRejection) Bool() bool { return bool(t) }

// String implementa fmt.Stringer.
func (t TerminalRejection) String() string {
	if t {
		return "terminal"
	}
	return "transitorio"
}

// Classify clasifica un error del backend en rechazo terminal o transitorio.
//
// Es la envoltura de uso real de IsTerminalCredentialRejection: extrae el
// status y el reason del error y devuelve el veredicto.
//
// Un error que no viene del backend (p. ej. un fallo de red) se considera
// TRANSITORIO: el LS original no puede distinguir un corte de red de un
// rechazo, y tratar un corte de red como rechazo terminal tiraría la
// sesión del usuario sin motivo.
func Classify(err error) TerminalRejection {
	if err == nil {
		return false
	}
	return TerminalRejection(IsTerminalCredentialRejection(ReasonOf(err), StatusOf(err)))
}
