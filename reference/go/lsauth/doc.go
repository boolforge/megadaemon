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
//	/workspace/out/LS_AUTH_ANALYSIS.md   (informe, v4)
//	/workspace/out/VERIFICATION_LEDGER.md (libro mayor de verificación)
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
// La asignación del "slot 5" de la interfaz `AuthProvider` del receptor a
// `GetEndpointURL` es una INFERENCIA (no se reconstruyó la tabla de métodos
// del itab). Todo lo demás que este paquete afirma está respaldado por
// desensamblado, por el descriptor proto, o por el barrido exhaustivo del
// binario; cuando algo no se pudo determinar, se marca explícitamente como
// NO VERIFICADO en el comentario correspondiente.
package lsauth
