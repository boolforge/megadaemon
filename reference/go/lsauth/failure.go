package lsauth

import "fmt"

// Itab es la tabla de métodos ("itab") de una implementación concreta de
// una interfaz en Go. Para los tipos concretos (no interfaz) el runtime usa
// el mismo layout, así que la VA identifica unívocamente a la variante.
type Itab uint64

// FailureVariant describe una de las 7 variantes del oneof `failure_details`.
//
// VERIFICADO por itab: cada variante tiene un itab distinto y único en el
// binario. La VA del itab es el identificador estable; el nombre del tipo
// concreto se obtuvo de los getters generados y del blob de tipos del
// runtime (`types_base = 0x7df5f18`).
type FailureVariant struct {
	// Kind es el discriminante lógico de la variante.
	Kind FailureKind
	// Itab es la VA real del itab de la implementación.
	Itab Itab
	// ConcreteType es el nombre del tipo concreto en el Go de Google.
	ConcreteType string
	// BuiltAt son las VAs donde el LS CONSTRUYE esta variante (puede estar
	// vacía si la variante nunca se construye en este binario).
	BuiltAt []uint64
	// BuiltAtNote describe dónde se construye.
	BuiltAtNote string
	// NeverBuilt vale true si nadie la construye en el LS (la tendría que
	// haber enviado el servidor).
	NeverBuilt bool
}

// FailureVariants es el catálogo verificado de las 7 variantes.
//
// Lectura de cada variante en el binario: type-switch en 0x70e19ec,
// 0x70e1ac0 y 0x70e2200 (rama de error de validateCurrentAuth) y en
// 0x70e63bc (rama de onboarding).
var FailureVariants = []FailureVariant{
	{
		Kind:         FailureIneligible,
		Itab:         0x8f12448,
		ConcreteType: "exa.language_server_pb.IneligibleInfo",
		BuiltAt:      []uint64{0x70e63bc},
		BuiltAtNote:  "ValidateAndOnboardAccount (validator.go:185): el tier del usuario está en ineligible_tiers",
	},
	{
		Kind:         FailureVerificationRequired,
		Itab:         0x8f12468,
		ConcreteType: "exa.language_server_pb.VerificationRequiredInfo",
		BuiltAt:      []uint64{0x70e5874},
		BuiltAtNote:  "buildErrorResp (auth_client.go): falta verificación de identidad",
	},
	{
		Kind:         FailureTosViolation,
		Itab:         0x8f12488,
		ConcreteType: "exa.language_server_pb.TosViolationInfo",
		BuiltAt:      []uint64{0x70e5920},
		BuiltAtNote:  "buildErrorResp (auth_client.go): violación de términos de servicio",
	},
	{
		Kind:         FailureGeneralError,
		Itab:         0x8f124a8,
		ConcreteType: "exa.language_server_pb.GeneralErrorInfo",
		BuiltAt:      []uint64{0x70e5490},
		BuiltAtNote:  "buildErrorResp (auth_client.go): error genérico; lleva `reason` libre",
	},
	{
		Kind:         FailureProjectRequired,
		Itab:         0x8f124c8,
		ConcreteType: "exa.language_server_pb.ProjectRequiredInfo",
		BuiltAt:      []uint64{0x70e5cd4},
		BuiltAtNote:  "(*EnterpriseAuthValidator).Validate (validator.go:122): falta proyecto",
	},
	{
		Kind:         FailureHeadlessAuthRequired,
		Itab:         0x8f124e8,
		ConcreteType: "exa.language_server_pb.HeadlessAuthRequiredInfo",
		BuiltAt:      nil,
		BuiltAtNote:  "NUNCA se construye en el LS: si llega, vino del servidor",
		NeverBuilt:   true,
	},
	{
		Kind:         FailureLicenseRequired,
		Itab:         0x8f12508,
		ConcreteType: "exa.language_server_pb.LicenseRequiredInfo",
		BuiltAt:      []uint64{0x70e5e4c},
		BuiltAtNote:  "(*EnterpriseAuthValidator).Validate (validator.go:122): falta licencia",
	},
}

// IneligibleInfo es la variante `ineligible`: el tier del usuario no da
// derecho al servicio.
//
// El proto no define campos propios (sólo el discriminante), así que el
// detalle vive en el `ui_message` del AuthResult.
type IneligibleInfo struct{}

// VerificationRequiredInfo es la variante `verification_required`.
type VerificationRequiredInfo struct{}

// TosViolationInfo es la variante `tos_violation`.
type TosViolationInfo struct{}

// GeneralErrorInfo es la variante `general_error`: lleva un `reason` libre.
type GeneralErrorInfo struct {
	// Reason es el texto del motivo (campo 1 del mensaje).
	Reason string
}

// ProjectRequiredInfo es la variante `project_required`.
//
// Sin campos ⇒ el motivo concreto va en `ui_message` o en el `reason` de un
// GeneralError. (Punto que sigue NO VERIFICADO en §9 del informe: qué texto
// exacto pone el LS cuando el motivo es "proyecto requerido").
type ProjectRequiredInfo struct{}

// HeadlessAuthRequiredInfo es la variante `headless_auth_required`: pide
// autenticación en navegador.
//
// El proto declara un campo `auth_url`; el LS no la construye nunca.
type HeadlessAuthRequiredInfo struct {
	// AuthURL es la URL a la que el usuario debe ir.
	AuthURL string
}

// LicenseRequiredInfo es la variante `license_required`.
//
// Sin campos ⇒ el motivo va en `ui_message` o en `GeneralError.reason`.
type LicenseRequiredInfo struct{}

// Kind implementa FailureDetails.
func (IneligibleInfo) Kind() FailureKind { return FailureIneligible }

// Kind implementa FailureDetails.
func (VerificationRequiredInfo) Kind() FailureKind { return FailureVerificationRequired }

// Kind implementa FailureDetails.
func (TosViolationInfo) Kind() FailureKind { return FailureTosViolation }

// Kind implementa FailureDetails.
func (GeneralErrorInfo) Kind() FailureKind { return FailureGeneralError }

// Kind implementa FailureDetails.
func (ProjectRequiredInfo) Kind() FailureKind { return FailureProjectRequired }

// Kind implementa FailureDetails.
func (HeadlessAuthRequiredInfo) Kind() FailureKind { return FailureHeadlessAuthRequired }

// Kind implementa FailureDetails.
func (LicenseRequiredInfo) Kind() FailureKind { return FailureLicenseRequired }

// reason implementa FailureDetails.
func (IneligibleInfo) reason() string { return "" }

// reason implementa FailureDetails.
func (VerificationRequiredInfo) reason() string { return "" }

// reason implementa FailureDetails.
func (TosViolationInfo) reason() string { return "" }

// reason implementa FailureDetails.
func (g GeneralErrorInfo) reason() string { return g.Reason }

// reason implementa FailureDetails.
func (ProjectRequiredInfo) reason() string { return "" }

// reason implementa FailureDetails.
func (h HeadlessAuthRequiredInfo) reason() string { return h.AuthURL }

// reason implementa FailureDetails.
func (LicenseRequiredInfo) reason() string { return "" }

// Reason devuelve el motivo legible de la variante, o "" si no lo tiene.
//
// Es la versión pública de `reason()`: los consumidores del paquete (y el
// propio MegaDaemon) no deberían necesitar type switches para leer el
// motivo de un fallo.
func Reason(f FailureDetails) string {
	if f == nil {
		return ""
	}
	return f.reason()
}

// VariantOf devuelve el catálogo de la variante que implementa f.
//
// Si f es nil devuelve la variante FailureNone con Itab 0 (que no existe en
// el binario: es el caso "sin fallo").
func VariantOf(f FailureDetails) FailureVariant {
	if f == nil {
		return FailureVariant{Kind: FailureNone, ConcreteType: "<nil>"}
	}
	for _, v := range FailureVariants {
		if v.Kind == f.Kind() {
			return v
		}
	}
	return FailureVariant{Kind: f.Kind(), ConcreteType: "<desconocida>"}
}

// NewGeneralError crea la variante `general_error` con su motivo.
//
// Es la variante que buildErrorResp fabrica en la rama de error genérico
// (0x70e5490), que es la más frecuente de todas.
func NewGeneralError(reason string) FailureDetails {
	return GeneralErrorInfo{Reason: reason}
}

// NewVerificationRequired crea la variante `verification_required`.
func NewVerificationRequired() FailureDetails { return VerificationRequiredInfo{} }

// NewTosViolation crea la variante `tos_violation`.
func NewTosViolation() FailureDetails { return TosViolationInfo{} }

// NewIneligible crea la variante `ineligible`.
func NewIneligible() FailureDetails { return IneligibleInfo{} }

// NewProjectRequired crea la variante `project_required`.
func NewProjectRequired() FailureDetails { return ProjectRequiredInfo{} }

// NewLicenseRequired crea la variante `license_required`.
func NewLicenseRequired() FailureDetails { return LicenseRequiredInfo{} }

// NewHeadlessAuthRequired crea la variante `headless_auth_required`.
//
// OJO: el LS original NUNCA construye esta variante (itab 0x8f124e8 sin
// ningún punto de construcción). Se ofrece aquí para poder modelar un
// `AuthResult` DECODIFICADO del servidor, que sí puede traerla.
func NewHeadlessAuthRequired(authURL string) FailureDetails {
	return HeadlessAuthRequiredInfo{AuthURL: authURL}
}

// String implementa fmt.Stringer para las variantes, con su nombre de proto
// y su itab: es la forma más cómoda de depurar un `AuthResult` entero.
func (v FailureVariant) String() string {
	if v.Kind == FailureNone {
		return "none"
	}
	return fmt.Sprintf("%s (itab 0x%x)", v.Kind, uint64(v.Itab))
}
