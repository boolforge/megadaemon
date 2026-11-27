# Datos medidos — RE de contratos de Cursor 3.32.4

Estos ficheros son la **parte medida** del RE del paquete oficial
`cursor_3.32.4_amd64.deb` (linux-x64, 230 561 828 B).

**Qué hay aquí.** Por cada contrato, las **rutas y líneas exactas** dentro del paquete, las
tablas de campos, los catálogos de nombres (eventos, tools, enums, campos) y los mensajes de
validación literales. Los bloques de código largos se han recortado: este repositorio lleva
el **análisis y las rutas**, y el material íntegro vive en la **Drive privada** del proyecto.

**Qué NO hay aquí.** Nada de código de Cursor. Ni un solo fragmento de más de tres líneas.

| Fichero | Contrato |
|---|---|
| `01_manifiesto_extensiones.md` | Las 22 extensiones de Anysphere/Cursor, puntos de entrada y el runtime del producto |
| `02_rules_skills_modes.md` | Agent rules, `SKILL.md`, y los custom modes (`agent.v1.CustomSubagent`) |
| `03_hooks.md` | Los 22 eventos de hook, el matcher, las respuestas JSON y la política deny>ask>allow |
| `04_mcp.md` | El esquema de `mcp.json`, `placement`, los enums de política y las allowlists |
| `05_tools.md` | Los 73 nombres de tool del agente y la taxonomía de errores |
| `06_endpoints_contexto.md` | Endpoints, auth, telemetría OTel GenAI, política de red y `RequestContext` |
| `08_licencia_precios.md` | La EULA de Cursor (§1.2-1.3), licencias de terceros y el estado de los precios |
| `09_daemon_protocolo.md` | El agent host daemon: arranque, flags, `environment-file`, deadlines y reintentos |
| `10_protocolo_mcp_stateless.md` | **Contrato de referencia**: el protocolo MCP stateless (JSON-RPC 2.0) método a método |

`_scripts/` contiene las dos herramientas de medición (reproducibles): `harvest.py` extrae
literales con su ruta y línea; `trim_verbatim.py` aplica el recorte de extractos largos.

**El informe que los resume** (con la tabla maestro «contrato → ¿copiable? → fichero destino
en `rust-core/src/`») está en `docs/RE_CURSOR_CONTRATOS.md`.

**Licencia.** Cursor es proprietary (Anysphere Inc.). Aquí sólo hay análisis de formato y
rutas; ningún material redistribuido. Ver `08_licencia_precios.md` para la cita literal de la
EULA y el análisis de qué es reimplementación de formato y qué sería redistribute software.
