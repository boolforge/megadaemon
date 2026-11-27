// Package lsauth reimplementa, en Go limpio y legible, la semántica de
// autenticación del Language Server de Antigravity (LS).
//
// Este paquete NO es una traducción literal del binario: es una
// reimplementación *limpia* (clean-room) de las reglas que se han
// verificado por desensamblado sobre
// /workspace/agy-re/liblanguage_server.so (ELF64 aarch64 PIE, stripped).
//
// # Procedencia de cada regla
//
// Cada regla de este paquete lleva, en su comentario, la evidencia con la
// que fue verificada: VA real (dirección virtual correcta del binario PIE),
// fichero:línea original del Go de Google y, cuando aplica, el desensamblado
// que la demuestra.
//
// La tabla completa de evidencia vive en:
//
//	/workspace/out/LS_AUTH_ANALYSIS.md   (informe, v5)
//	/workspace/out/VERIFICATION_LEDGER.md (libro mayor de verificación)
//	/workspace/out/ANTIGRAVITY_BINARIES.md (catálogo de binarios y parches)
//
// # Identidad del binario analizado
//
// El fichero /workspace/agy-re/liblanguage_server.so NO es un build
// desconocido: es `assets/runtime/bin/liblanguage_server.so` del
// `antigravity-region-bypass.apk` de la etiqueta v2.4.1 del repositorio
// werdio325-png/antigravity-android (publicado 2026-10-22). El sha256 del
// `.so` descargado de ese APK es exactamente
// f25135e4b4a2c4a8e4cb319dc36b13eb98fa4718fa80aaae5a7819935370ff27.
//
// # El parche «region-bypass» (VERIFICADO, 15 bytes)
//
// Comparado con el vanilla de la MISMA versión (CRC-32 56ecf991), el
// binario parcheado difiere en 15 bytes repartidos en 4 tramos. Los tres
// puntos reescriben la misma instrucción:
//
//	LDRB wN, [x0, #8]   ->   MOV wN, #1
//
// El punto semánticamente relevante está en
// `(*PersonalAuthValidator).Validate` (validator.go:79):
//
//	0x070e5b50  ldrb w3, [x0, #8]    ; vanilla: w3 = AuthResult.HasValidAuth
//	0x070e5b54  tbz  w3, #0, ...    ; vanilla: si !HasValidAuth -> error
//	0x070e5b50  mov  w3, #1          ; parche: forzar el flag
//	0x070e5b54  strb w3, [x0, #8]    ; parche: AuthResult.HasValidAuth = true
//
// Es decir: el parche hace que el LS acepte como autenticado un AuthResult
// que el backend marcó con has_valid_auth = false. Los otros dos puntos
// (0x05353a7c, en `grpc/internal/buffer.(*Unbounded[...]).Load`, y
// 0x06ce96f4, en `completion_provider.(*DocumentCacheState).CheckAndUpdateRefreshStateForStart`)
// son colaterales del mismo patrón genérico.
//
// En el LS v2.11.0 (build «core») el parche equivalente son 7 bytes en un
// único punto, 0x0656dfbc, en la MISMA función
// `(*PersonalAuthValidator).Validate` (validator.go:79): el objetivo del
// parche es estable entre versiones.
//
// # Las tres correcciones que cambiaron el diseño
//
//  1. El LS SÍ escribe `has_valid_auth` (12 sitios). La versión v2 del
//     informe afirmaba lo contrario por un fallo de método (barrido de
//     `STRB [Xn,#8]` mal filtrado). Ver authresult.go.
//
//  2. Las rutas `/v1internal:*` existen en DOS copias: la del descriptor
//     proto (extensión `google.api.http`, metadato declarativo que NADIE
//     lee en este binario) y la de las constantes Go (la que el LS usa en
//     runtime para construir las URLs). Ver endpoint.go.
//
//  3. La capa HTTP del backend sólo comprueba `status == 200`; la
//     diferenciación entre "rechazo terminal" (401/403/404...) y
//     "transitorio" (408/429) ocurre un nivel más arriba, en
//     `isTerminalCredentialRejection`. Ver terminal.go.
//
// # Qué NO está verificado
//
// Nada de lo que este paquete afirma sobre la semántica de autenticación
// queda como inferencia: cada afirmación está respaldada por desensamblado,
// por el descriptor proto, o por el barrido exhaustivo del binario.
//
// En particular, el método que devuelve la URL base del backend es el
// **slot 2** (offset `0x28`) de la interfaz `AuthProvider`, y esto se
// verificó reconstruyendo los SIETE itabs de proveedor del binario: en los
// siete, `itab+24+8*2` apunta a la implementación de `GetEndpointURL`.
//
//	itab base (VA)   proveedor                fun[2] = GetEndpointURL
//	0x8f853b8        IDEAuthProvider          0x66aeec0
//	0x8f851c0        HostAuthProvider         0x66af970
//	0x8f85310        StandaloneAuthProvider   0x66b0180
//	0x8f84fc8        CLIAuthProvider          0x66b2a90
//	0x8f85268        AntigravityHubAuthProvider 0x66b5650
//	0x8f84f20        GeminiAPIKeyAuthProvider 0x66be650
//	0x8f85118        StubbyAuthProvider       0x66b6370
//
// El *itab* de Go tiene una cabecera de 24 bytes, así que el índice de
// método es `(offset − 24) / 8`: `0x28` ⇒ **slot 2**, no 5.
//
// Lo que sí queda marcado como NO VERIFICADO en los puntos concretos
// (texto exacto de mensajes, entradas de `failure_details` que el LS nunca
// construye) está señalado así en el comentario de cada elemento.
package lsauth
