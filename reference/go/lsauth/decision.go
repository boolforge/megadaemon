package lsauth

// Profile es el "perfil" de autenticación que decide qué validador se usa.
//
// # La decisión de perfil (VERIFICADA por desensamblado)
//
// El auxiliar invocado por GetAuthStatus (0x70f08e0) parte la URL del
// endpoint por "://" (literal de 3 bytes en 0x423757a) y compara el
// PREFIXO con "gemini-api" (10 bytes, construido como "gemini-a" + "pi"):
//
//	0x70f091c  cmp  x5, 0xa            ; longitud 10
//	0x70f0928  mov  x6, 0x6567        ; 'g'
//	0x70f092c  movk x6, 0x696d, lsl 16 ; 'm','i'
//	0x70f0930  movk x6, 0x696e, lsl 32 ; 'n','-'
//	0x70f0934  movk x6, 0x612d, lsl 48 ; 'a'
//	0x70f0938  cmp  x5, x6
//	0x70f0940  ldrh w4, [x4, 8]
//	0x70f0944  mov  x5, 0x6970        ; 'p','i'
//	0x70f094c  b.ne 0x70f0960
//	0x70f0950  orr  x1, xzr, 6        ; longitud 6
//	0x70f0954  adrp x2, 0x4254000 ; add x2, x2, 0x138   ; -> "gemini"
//	0x70f0960  orr  x1, xzr, 4        ; longitud 4
//	0x70f0964  adrp x2, 0x423d000 ; add x2, x2, 0xa7d   ; -> "ccpa"
//
// ⇒ Si el esquema del endpoint es "gemini-api" se usa el literal "gemini";
// en cualquier otro caso "ccpa" (Code Companion Personal Account).
type Profile int

const (
	// ProfileUnknown es el perfil por defecto cuando no se puede decidir.
	ProfileUnknown Profile = iota
	// ProfilePersonal es el perfil personal ("ccpa").
	ProfilePersonal
	// ProfileEnterprise es el perfil de empresa ("gemini").
	ProfileEnterprise
	// ProfileInternal es el perfil interno (el validador
	// (*InternalAuthValidator), que se usa en entornos de Google).
	ProfileInternal
)

// String implementa fmt.Stringer.
func (p Profile) String() string {
	switch p {
	case ProfilePersonal:
		return "personal"
	case ProfileEnterprise:
		return "enterprise"
	case ProfileInternal:
		return "internal"
	default:
		return "unknown"
	}
}

// Tier es el nivel de servicio del usuario (UserTier) devuelto por el
// backend.
//
// Los identificadores observados en el descriptor son, entre otros, los
// valores que el LS compara contra `allowed_tiers`.
type Tier struct {
	// ID es el identificador del tier (p. ej. "free-tier", "legacy-tier").
	ID string
	// Name es el nombre legible del tier.
	Name string
	// UsesGcpTos indica si el tier obliga a aceptar los TOS de Google Cloud.
	//
	// CRUCE SEMÁNTICO VERIFICADO: `AuthResult.is_gcp_tos` ↔
	// `UserTier.uses_gcp_tos` ⇒ es el mismo dato, traveling del backend al
	// estado de auth.
	UsesGcpTos bool
	// Allowed indica si el tier concede acceso al servicio.
	Allowed bool
}

// UserTierResponse modela la respuesta de tier/entitlement del backend.
//
// Es la parte de `UserTier` que el LS consume para decidir validez y para
// rellenar el `AuthResult`.
type UserTierResponse struct {
	// CurrentTier es el tier del usuario (nil si el backend no lo devolvió).
	CurrentTier *Tier
	// IneligibleTiers son los tiers que NO dan acceso.
	//
	// CRUCE SEMÁNTICO VERIFICADO: `AuthResult.ineligible` (IneligibleInfo)
	// ↔ `LoadCodeAssistResponse.ineligible_tiers` (IneligibleTier) ⇒ es el
	// backend el que produce el motivo "ineligible" del estado de auth.
	IneligibleTiers []Tier
	// AllowedTiers son los tiers que SÍ dan acceso.
	AllowedTiers []Tier
	// ProjectID es el "cloudaicompanion_project" (campo 3 de
	// LoadCodeAssistResponse).
	ProjectID string
	// Location es la región del proyecto.
	Location string
	// ProjectValidationError es el `google.rpc.Status` de validación del
	// proyecto: si tiene error, el proyecto no vale.
	ProjectValidationError string
}

// ValidationOutcome es el veredicto de la validación de autenticación.
//
// Es exactamente lo que el LS acaba publicando en el `AuthResult`, más el
// detalle de por qué se ha tomado (para logs y para el prompt de la app).
type ValidationOutcome struct {
	// Valid indica si la autenticación es válida.
	//
	// Equivale a `AuthResult.has_valid_auth`.
	Valid bool
	// Failure es el motivo del fallo (nil si Valid).
	Failure FailureDetails
	// UIMessage es el texto que la app puede pintar (opcional).
	UIMessage string
	// AuthMode es el modo de autenticación resuelto.
	AuthMode AuthMode
	// ProjectID es el proyecto resuelto (si lo hay).
	ProjectID string
	// Location es la región del proyecto (si lo hay).
	Location string
	// IsGcpTos indica si el usuario aceptó los TOS.
	IsGcpTos bool
	// Transient marca el veredicto como "no pude comprobar", en vez de
	// "comprobado y falso". Es la distinción que hace el LS entre un
	// rechazo terminal (408/429/red ⇒ no se invalida la sesión) y un
	// rechazo de credenciales.
	Transient bool
}

// Result convierte el veredicto en el `AuthResult` que consume la app.
//
// Equivale a lo que hacen los tres validadores y `buildErrorResp`:
//   - camino válido  ⇒ `has_valid_auth = true` y sin `failure_details`;
//   - camino de error ⇒ `has_valid_auth = false` (por valor cero del
//     `newobject`) y `failure_details` con la variante que corresponda.
//
// Nótese que en el camino de error el booleano NO se escribe nunca: queda en
// `false` porque el struct acaba de salir de `runtime.newobject`. Esa es la
// razón por la que `buildErrorResp` no aparece en la lista de escritores de
// `has_valid_auth`.
func (o ValidationOutcome) Result() AuthResult {
	r := AuthResult{
		HasValidAuth: o.Valid,
		UIMessage:    o.UIMessage,
		IsGcpTos:     o.IsGcpTos,
		AuthMode:     o.AuthMode,
		ProjectID:    o.ProjectID,
		Location:     o.Location,
	}
	if !o.Valid {
		r.FailureDetails = o.Failure
	}
	return r
}

// ValidateTierResponse decide si la respuesta de tier/entitlement implica
// autenticación válida.
//
// # Reglas (VERIFICADAS; son las que el LS aplica a la respuesta del backend)
//
// Válido ⇔
//   - hay `current_tier`;
//   - su `id` NO está en `ineligible_tiers`;
//   - `allowed_tiers` contiene el tier del usuario;
//   - `cloudaicompanion_project` (project_id) no está vacío;
//   - `project_validation_error` (google.rpc.Status) no tiene error;
//   - `project_id` y `location` están rellenos.
//
// Si el tier está en `ineligible_tiers` ⇒ `failure_details = ineligible`
// (la variante IneligibleInfo, itab 0x8f12448).
func ValidateTierResponse(resp *UserTierResponse) ValidationOutcome {
	if resp == nil {
		return ValidationOutcome{
			Failure:   NewGeneralError("respuesta de tier vacía"),
			UIMessage: "no se pudo leer la información de servicio",
		}
	}

	// 1) ¿Hay tier?
	if resp.CurrentTier == nil {
		return ValidationOutcome{
			Failure:   NewGeneralError("no current tier"),
			UIMessage: "no se pudo determinar el nivel de servicio",
		}
	}

	// 2) ¿El tier está en la lista de no elegibles?
	for _, in := range resp.IneligibleTiers {
		if in.ID == resp.CurrentTier.ID {
			return ValidationOutcome{
				Failure:   NewIneligible(),
				UIMessage: "tu cuenta no es elegible para este servicio",
				IsGcpTos:  resp.CurrentTier.UsesGcpTos,
			}
		}
	}

	// 3) ¿allowed_tiers contiene el tier del usuario?
	allowed := false
	for _, t := range resp.AllowedTiers {
		if t.ID == resp.CurrentTier.ID {
			allowed = true
			break
		}
	}
	if !allowed {
		return ValidationOutcome{
			Failure:   NewGeneralError("tier not allowed"),
			UIMessage: "tu nivel de servicio no incluye este producto",
			IsGcpTos:  resp.CurrentTier.UsesGcpTos,
		}
	}

	// 4) ¿El proyecto está validado?
	if resp.ProjectValidationError != "" {
		return ValidationOutcome{
			Failure:   NewGeneralError(resp.ProjectValidationError),
			UIMessage: "el proyecto no está disponible",
			IsGcpTos:  resp.CurrentTier.UsesGcpTos,
		}
	}

	// 5) ¿Proyecto y región rellenos?
	if resp.ProjectID == "" || resp.Location == "" {
		return ValidationOutcome{
			Failure:   NewGeneralError("project or location missing"),
			UIMessage: "no hay proyecto configurado",
			IsGcpTos:  resp.CurrentTier.UsesGcpTos,
		}
	}

	// Camino válido.
	return ValidationOutcome{
		Valid:     true,
		IsGcpTos:  resp.CurrentTier.UsesGcpTos,
		ProjectID: resp.ProjectID,
		Location:  resp.Location,
	}
}

// HandleBackendError convierte un error del backend en el veredicto que el
// LS publicaría.
//
// Regla (VERIFICADA): un rechazo TERMINAL (ver IsTerminalCredentialRejection)
// implica sesión cerrada: `Logout()` + `buildErrorResp()` ⇒ el resultado
// lleva `has_valid_auth=false` y `general_error` con el motivo.
//
// Un error TRANSITORIO (408, 429, corte de red) NO invalida la sesión: el
// LS mantiene el estado previo y reintenta. En ese caso esta función
// devuelve un veredicto con `Valid=false` pero `Transient=true`, para que el
// llamador pueda distinguir "no autenticado" de "no he podido comprobarlo".
func HandleBackendError(err error) ValidationOutcome {
	if err == nil {
		return ValidationOutcome{Valid: true}
	}
	if !Classify(err) {
		// Transitorio: no se toca el estado de sesión.
		return ValidationOutcome{
			Valid:     false,
			Transient: true,
			Failure:   NewGeneralError(err.Error()),
			UIMessage: "no se pudo contactar con el servicio; reintentando",
		}
	}
	// Terminal: el usuario tiene que volver a autenticarse.
	return ValidationOutcome{
		Valid:     false,
		Transient: false,
		Failure:   NewGeneralError(ReasonOf(err)),
		UIMessage: "la sesión ha caducado; vuelve a iniciar sesión",
	}
}
