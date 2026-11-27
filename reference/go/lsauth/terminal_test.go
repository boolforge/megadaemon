package lsauth

import "testing"

// TestTerminalCredentialRejection es la prueba que fija la regla más
// importante del paquete (la que el usuario pidió cerrar): ¿el cliente
// distingue 401/403 de 404?
//
// La tabla está sacada del desensamblado de
// `isTerminalCredentialRejection` (auth_client/auth_client.go:303,
// 0x70e1c10) y de la capa HTTP (`cmp x11, 0xc8` en 0x66e8b38).
func TestTerminalCredentialRejection(t *testing.T) {
	t.Parallel()

	const (
		reasonInvalidGrant = "invalid_grant"
		reasonOther        = "access_denied"
	)

	cases := []struct {
		name   string
		reason string
		status int
		want   bool
	}{
		// 4xx: rechazo TERMINAL (incluido 404, que es lo que se pedía).
		{"401 unauthorized", reasonOther, 401, true},
		{"403 forbidden", reasonOther, 403, true},
		{"404 not found = igual que 401/403", reasonOther, 404, true},
		{"400 bad request", reasonOther, 400, true},
		{"429Quota es transitorio", reasonOther, 429, false},
		{"408 timeout es transitorio", reasonOther, 408, false},
		{"499 sigue siendo 4xx terminal", reasonOther, 499, true},
		{"399 no es 4xx", reasonOther, 399, false},
		{"500 no es 4xx", reasonOther, 500, false},
		{"502 no es 4xx", reasonOther, 502, false},
		{"200 no es 4xx", reasonOther, 200, false},
		{"0 no es 4xx", reasonOther, 0, false},

		// "invalid_grant" es terminal con cualquier status (el backend a
		// veces lo devuelve con 200 en un cuerpo de error).
		{"invalid_grant con 400", reasonInvalidGrant, 400, true},
		{"invalid_grant con 200", reasonInvalidGrant, 200, true},
		{"invalid_grant con 500", reasonInvalidGrant, 500, true},

		// Un reason distinto NO fuerza terminalidad por sí solo.
		{"otro reason con 429 sigue transitorio", reasonOther, 429, false},
	}

	for _, tc := range cases {
		tc := tc
		t.Run(tc.name, func(t *testing.T) {
			t.Parallel()
			got := IsTerminalCredentialRejection(tc.reason, tc.status)
			if got != tc.want {
				t.Fatalf("IsTerminalCredentialRejection(%q, %d) = %v; se esperaba %v",
					tc.reason, tc.status, got, tc.want)
			}
		})
	}
}

// TestInvalidGrantLength comprueba la longitud EXACTA que el binario
// compara (13 bytes, immediate `cmp x3, 0xd` en 0x70e1c74).
func TestInvalidGrantLength(t *testing.T) {
	t.Parallel()
	if got := len(InvalidGrantReason); got != 13 {
		t.Fatalf("len(%q) = %d; el binario compara con 13 (cmp x3, 0xd @0x70e1c74)", InvalidGrantReason, got)
	}
}

// TestClassifyFromError comprueba la envoltura de error real.
func TestClassifyFromError(t *testing.T) {
	t.Parallel()

	terminal := NewBackendError(403, "POST", "https://x/v1internal:loadCodeAssist", "forbidden", "body")
	if !Classify(terminal) {
		t.Fatal("un 403 debe clasificarse como terminal")
	}

	transient := NewBackendError(429, "POST", "https://x/v1internal:loadCodeAssist", "quota", "body")
	if Classify(transient) {
		t.Fatal("un 429 NO debe clasificarse como terminal")
	}

	// Un corte de red (error sin status) es transitorio: el LS original no
	// puede distinguirlo de un rechazo y no debe tirar la sesión.
	if Classify(nil) {
		t.Fatal("nil no es terminal")
	}
}

// TestHandleBackendErrorTransientVsTerminal es la diferencia de
// comportamiento que importa al usuario: con 404 el LS cierra sesión; con
// 429 la mantiene.
func TestHandleBackendErrorTransientVsTerminal(t *testing.T) {
	t.Parallel()

	e404 := NewBackendError(404, "POST", "https://x/v1internal:loadCodeAssist", "not found", "")
	out404 := HandleBackendError(e404)
	if out404.Valid {
		t.Fatal("un 404 no puede ser un resultado válido")
	}
	if out404.Transient {
		t.Fatal("un 404 es terminal: no debe marcarse como transitorio")
	}
	if out404.Result().HasValidAuth {
		t.Fatal("un 404 debe dejar has_valid_auth=false")
	}
	if out404.Result().FailureDetails == nil {
		t.Fatal("un 404 debe traer failure_details")
	}

	e429 := NewBackendError(429, "POST", "https://x/v1internal:loadCodeAssist", "quota", "")
	out429 := HandleBackendError(e429)
	if !out429.Transient {
		t.Fatal("un 429 debe ser transitorio (la sesión se conserva)")
	}
}
