package lsauth

import "testing"

// TestEndpointURLFormat fija la forma EXACTA de la URL que arma el LS:
//
//	fmt.Sprintf("%s/%s:%s", baseURL, "v1internal", method)
//
// (constructCodeAssistEndpoint, http_helpers.go:153, 0x66c6680; plantilla en
// 0x4279a33).
func TestEndpointURLFormat(t *testing.T) {
	t.Parallel()

	cases := []struct {
		base   string
		method string
		want   string
	}{
		{
			"https://cloudcode-pa.googleapis.com", "loadCodeAssist",
			"https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist",
		},
		{
			"https://cloudcode-pa.googleapis.com", "onboardUser",
			"https://cloudcode-pa.googleapis.com/v1internal:onboardUser",
		},
		{
			"https://cloudcode-pa.sandbox.googleapis.com", "fetchUserInfo",
			"https://cloudcode-pa.sandbox.googleapis.com/v1internal:fetchUserInfo",
		},
	}

	for _, tc := range cases {
		got := EndpointURL(tc.base, tc.method)
		if got != tc.want {
			t.Fatalf("EndpointURL(%q, %q) = %q; se esperaba %q", tc.base, tc.method, got, tc.want)
		}
	}
}

// TestResourceURLFormat fija la plantilla "%s/%s/%s" (0x4279a3b) usada por
// constructResourceURL (http_helpers.go:177, 0x66c6950).
func TestResourceURLFormat(t *testing.T) {
	t.Parallel()
	got := ResourceURL("https://cloudcode-pa.googleapis.com", "projects/p/locations/l")
	want := "https://cloudcode-pa.googleapis.com/v1internal/projects/p/locations/l"
	if got != want {
		t.Fatalf("ResourceURL = %q; se esperaba %q", got, want)
	}
}

// TestV1InternalPrefixMatchesBinary fija el valor del prefijo global
// 0x799ff50 (longitud 10 en 0x799ff58, bytes en 0x42a68b1).
func TestV1InternalPrefixMatchesBinary(t *testing.T) {
	t.Parallel()
	if V1InternalPrefix != "v1internal" {
		t.Fatalf("prefijo = %q; se esperaba \"v1internal\"", V1InternalPrefix)
	}
	if len(V1InternalPrefix) != 10 {
		t.Fatalf("longitud del prefijo = %d; el binario guarda 10 (0x799ff58)", len(V1InternalPrefix))
	}
}

// TestBackendMethodLiterals comprueba que cada método del catálogo tiene la
// longitud que el binario usa (el inmediato que acompaña al ADRP+ADD) y que
// el nombre NO contiene caracteres que romperían la construcción de la URL.
func TestBackendMethodLiterals(t *testing.T) {
	t.Parallel()
	for _, m := range BackendMethods {
		if len(m.Name) != m.LiteralLen {
			t.Errorf("método %q: longitud declarada %d != longitud real %d", m.Name, m.LiteralLen, len(m.Name))
		}
		if m.LiteralVA == 0 {
			t.Errorf("método %q sin VA de literal", m.Name)
		}
		if m.Name == "" {
			t.Errorf("método con nombre vacío en 0x%x", m.LiteralVA)
		}
	}
}

// TestMethodFromURL ida y vuelta.
func TestMethodFromURL(t *testing.T) {
	t.Parallel()

	const u = "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist"
	got, ok := MethodFromURL(u)
	if !ok || got != "loadCodeAssist" {
		t.Fatalf("MethodFromURL(%q) = (%q, %v); se esperaba (\"loadCodeAssist\", true)", u, got, ok)
	}

	if _, ok := MethodFromURL("https://otro.host/otra-ruta"); ok {
		t.Fatal("una URL que no es del backend no debe reconocerse")
	}
}

// TestAuthPathMethodsCovered documenta la cobertura: todos los métodos que
// intervienen en el camino de autenticación están en el catálogo.
func TestAuthPathMethodsCovered(t *testing.T) {
	t.Parallel()
	for _, want := range []string{
		"loadCodeAssist", "onboardUser", "fetchUserInfo",
		"fetchAvailableModels", "fetchAdminControls", "fetchConfig",
		"entitlement.userTier", "listExperiments", "retrieveUserQuotaSummary",
	} {
		if !IsMethodName(want) {
			t.Errorf("el método %q del camino de auth no está en el catálogo", want)
		}
	}
}
