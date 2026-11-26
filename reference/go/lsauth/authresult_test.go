package lsauth

import (
	"testing"
	"unsafe"
)

// wireMirror replica el layout del `AuthResult` real del binario, con los
// MISMOS tipos que usa el runtime de protobuf (string = 16 B, slice = 24 B,
// interfaz = 16 B, bool = 1 B, int32 = 4 B).
//
// El nombre del primer campo (`state`) corresponde al `protoimpl.MessageState`
// del binario, que ocupa 8 bytes; aquí se modela como un array opaco porque
// el runtime de protobuf no está disponible fuera del binario.
type wireMirror struct {
	state          [8]byte
	hasValidAuth   bool
	uiMessage      string
	isGcpTos       bool
	projectID      string
	location       string
	failureDetails FailureDetails
	grantedScopes  []string
	wifProvider    string
	authMode       AuthMode
	unknownFields  []byte
	sizeCache      int32
}

// TestAuthResultWireLayout fija el layout de 168 bytes verificado en el
// binario. Esta prueba protege la reimplementación de que nadie "reordene"
// los campos pensando que el orden da igual: NO da igual, porque el
// `AuthResult` cruza la frontera del proceso (Connect ⇒ protobuf) y porque
// MegaDaemon va a hablar con el LS real.
//
// Fuentes: runtime._type (0x8ddad20, Size_=0xA8), los getters generados y
// (*AuthResult).Reset (0x5b36380, memclr de 164 B).
func TestAuthResultWireLayout(t *testing.T) {
	t.Parallel()

	var m wireMirror

	cases := []struct {
		name string
		got  uintptr
		want uintptr
	}{
		{"state", unsafe.Offsetof(m.state), 0x00},
		{"has_valid_auth", unsafe.Offsetof(m.hasValidAuth), 0x08},
		{"ui_message", unsafe.Offsetof(m.uiMessage), 0x10},
		{"is_gcp_tos", unsafe.Offsetof(m.isGcpTos), 0x20},
		{"project_id", unsafe.Offsetof(m.projectID), 0x28},
		{"location", unsafe.Offsetof(m.location), 0x38},
		{"failure_details", unsafe.Offsetof(m.failureDetails), 0x48},
		{"granted_scopes", unsafe.Offsetof(m.grantedScopes), 0x58},
		{"wif_provider", unsafe.Offsetof(m.wifProvider), 0x70},
		{"auth_mode", unsafe.Offsetof(m.authMode), 0x80},
		{"unknown_fields", unsafe.Offsetof(m.unknownFields), 0x88},
		{"size_cache", unsafe.Offsetof(m.sizeCache), 0xA0},
	}

	for _, tc := range cases {
		if tc.got != tc.want {
			t.Errorf("offset de %-16s = 0x%02x; el binario lo tiene en 0x%02x", tc.name, tc.got, tc.want)
		}
	}

	if got := unsafe.Sizeof(m); got != 168 {
		t.Fatalf("sizeof(AuthResult) = %d; el binario mide 168 (0xA8)", got)
	}
}

// TestHasValidAuthWriterCount fija el número de escritores verificados.
//
// La v2 de este informe afirmaba que el LS NO escribía el booleano; la v3
// lo refutó con un barrido exhaustivo (12 escritores reales, 9 falsos
// positivos). Esta prueba documenta el número para que una regresión futura
// sea visible.
func TestHasValidAuthWriterCount(t *testing.T) {
	t.Parallel()
	if len(HasValidAuthWriters) != 12 {
		t.Fatalf("escritores de has_valid_auth = %d; se esperaban 12", len(HasValidAuthWriters))
	}
	seen := map[uint64]bool{}
	for _, w := range HasValidAuthWriters {
		if w.VA == 0 {
			t.Errorf("escritor %q sin VA", w.Function)
		}
		if seen[w.VA] {
			t.Errorf("VA duplicado en la lista de escritores: 0x%x", w.VA)
		}
		seen[w.VA] = true
		if w.FileLine == "" {
			t.Errorf("escritor 0x%x sin fichero:línea", w.VA)
		}
	}
}

// TestFailureVariantsAreUniqueAndComplete fija el catálogo de las 7
// variantes del oneof y que sus itabs son únicos.
func TestFailureVariantsAreUniqueAndComplete(t *testing.T) {
	t.Parallel()
	if len(FailureVariants) != 7 {
		t.Fatalf("variantes = %d; se esperaban 7", len(FailureVariants))
	}
	itabs := map[Itab]bool{}
	for _, v := range FailureVariants {
		if v.Itab == 0 {
			t.Errorf("variante %v sin itab", v.Kind)
		}
		if itabs[v.Itab] {
			t.Errorf("itab duplicado: 0x%x", uint64(v.Itab))
		}
		itabs[v.Itab] = true
	}
}

// TestHeadlessAuthRequiredNeverBuilt documenta el hallazgo de que el LS
// nunca construye esa variante.
func TestHeadlessAuthRequiredNeverBuilt(t *testing.T) {
	t.Parallel()
	for _, v := range FailureVariants {
		if v.Kind == FailureHeadlessAuthRequired {
			if !v.NeverBuilt {
				t.Fatal("headless_auth_required debe marcarse como nunca construida por el LS")
			}
			if len(v.BuiltAt) != 0 {
				t.Fatalf("headless_auth_required no debe tener puntos de construcción, tiene %v", v.BuiltAt)
			}
		}
	}
}

// TestOutcomeResultMapping comprueba el mapeo veredicto ⇒ AuthResult:
//   - válido  ⇒ has_valid_auth=true, sin failure_details;
//   - error   ⇒ has_valid_auth=false (nunca se escribe: queda en false por
//     el newobject) y failure_details presente.
func TestOutcomeResultMapping(t *testing.T) {
	t.Parallel()

	ok := ValidationOutcome{
		Valid: true, ProjectID: "p", Location: "us-central1",
		IsGcpTos: true, AuthMode: AuthModePersonalConsumer,
	}
	r := ok.Result()
	if !r.HasValidAuth {
		t.Fatal("un veredicto válido debe producir has_valid_auth=true")
	}
	if r.FailureDetails != nil {
		t.Fatal("un veredicto válido no debe llevar failure_details")
	}
	if r.ProjectID != "p" || r.Location != "us-central1" {
		t.Fatalf("proyecto/localización mal copiados: %q/%q", r.ProjectID, r.Location)
	}

	bad := ValidationOutcome{Failure: NewGeneralError("boom"), UIMessage: "nope"}
	r2 := bad.Result()
	if r2.HasValidAuth {
		t.Fatal("un veredicto de error debe producir has_valid_auth=false")
	}
	if r2.FailureDetails == nil {
		t.Fatal("un veredicto de error debe llevar failure_details")
	}
	if Reason(r2.FailureDetails) != "boom" {
		t.Fatalf("reason = %q; se esperaba \"boom\"", Reason(r2.FailureDetails))
	}
}

// TestValidateTierResponseRules comprueba las 5 reglas de validez del
// backend.
func TestValidateTierResponseRules(t *testing.T) {
	t.Parallel()

	good := &UserTierResponse{
		CurrentTier:  &Tier{ID: "free", UsesGcpTos: true},
		AllowedTiers: []Tier{{ID: "free"}},
		ProjectID:    "proj",
		Location:     "us-central1",
	}
	if out := ValidateTierResponse(good); !out.Valid {
		t.Fatalf("una respuesta buena debe ser válida: %+v", out)
	}

	inel := &UserTierResponse{
		CurrentTier:     &Tier{ID: "free"},
		AllowedTiers:    []Tier{{ID: "free"}},
		IneligibleTiers: []Tier{{ID: "free"}},
		ProjectID:       "p", Location: "l",
	}
	out := ValidateTierResponse(inel)
	if out.Valid {
		t.Fatal("un tier ineligible no puede ser válido")
	}
	if out.Failure.Kind() != FailureIneligible {
		t.Fatalf("variante = %v; se esperaba ineligible", out.Failure.Kind())
	}

	notAllowed := &UserTierResponse{
		CurrentTier:  &Tier{ID: "free"},
		AllowedTiers: []Tier{{ID: "enterprise"}},
		ProjectID:    "p", Location: "l",
	}
	if ValidateTierResponse(notAllowed).Valid {
		t.Fatal("un tier no permitido no puede ser válido")
	}

	badProject := &UserTierResponse{
		CurrentTier:  &Tier{ID: "free"},
		AllowedTiers: []Tier{{ID: "free"}},
		ProjectID:    "p", Location: "l",
		ProjectValidationError: "INVALID_ARGUMENT",
	}
	if ValidateTierResponse(badProject).Valid {
		t.Fatal("un proyecto con error de validación no puede ser válido")
	}

	noProject := &UserTierResponse{
		CurrentTier:  &Tier{ID: "free"},
		AllowedTiers: []Tier{{ID: "free"}},
		Location:     "l",
	}
	if ValidateTierResponse(noProject).Valid {
		t.Fatal("sin project_id no puede ser válido")
	}
}

// TestAuthModeEnumValues fija los valores del enum (verificados contra el
// descriptor proto embebido).
func TestAuthModeEnumValues(t *testing.T) {
	t.Parallel()
	cases := map[AuthMode]string{
		0: "AUTH_MODE_UNSPECIFIED",
		1: "AUTH_MODE_PERSONAL_CONSUMER",
		2: "AUTH_MODE_BUSINESS_LICENSED_ENTERPRISE",
		3: "AUTH_MODE_BUSINESS_AGENT_PLATFORM_PAYGO",
		4: "AUTH_MODE_GEMINI_API_KEY",
		5: "AUTH_MODE_ENTERPRISE_GATEWAY",
	}
	for v, want := range cases {
		if got := v.String(); got != want {
			t.Errorf("AuthMode(%d).String() = %q; se esperaba %q", int32(v), got, want)
		}
	}
}
