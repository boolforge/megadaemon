# CONTRATO 8 — LICENCIA, PRECIOS Y CONDICIONES

Fuente: `usr/share/cursor/resources/app/LICENSE.txt` (25 725 B),
`usr/share/cursor/resources/app/ThirdPartyNotices.txt` (185 562 B),
`usr/share/cursor/resources/app/product.json` (56 407 B).

---

## 8.1 ⚠️ HALLAZGO CRÍTICO: la EULA prohíbe explícitamente la ingeniería inversa

`LICENSE.txt`, cabecera literal:
```
Cursor
End User License Agreement
Last Modified: June 13, 2023
```
```
This End User License Agreement (this "Agreement") is a binding agreement between you and
Anysphere Inc. ("Cursor", "we", "our", or "us").
```

### §1.2 Limited License — literal

> *"Subject to this Agreement, you are hereby granted a limited, non-exclusive,
> **non-transferable**, and freely revocable license to install and use the executable form of
> the Software made available by Cursor **solely for your personal use only**, and strictly as
> permitted by the features of the Software. Cursor may terminate this license at any time for
> any reason or no reason."*

### §1.3 Use Restrictions — literal (el párrafo clave)

> *"Except as expressly specified in this Agreement, you may not:
> **(a) copy, modify, adapt, translate, prepare derivative works from, reverse engineer,
> disassemble, or decompile the Software** or otherwise attempt to discover any proprietary
> source code or trade secrets related to the Software;
> **(b) license, sell, resell, assign, transfer, sublicense, lease, lend, rent or otherwise
> distribute the Software to any third party**;
> (c) remove any copyright or other proprietary notices in the Software;
> (d) circumvent any mechanisms in the Software intended to limit its use;
> (e) use the Software in any manner that exceeds the scope of use expressly permitted in this
> Agreement; or (f) permit any third party to do any of the foregoing."*

### Qué significa esto para MegaDaemon (distinción importante)

| Actividad | ¿ Permitido por la EULA de Cursor? | Razonamiento |
|---|---|---|
| **Leer** el paquete para entender **formatos de fichero** (hooks.json, mcp.json, frontmatter de SKILL.md) | **Sí, en principio** | No es "copiar código ni hacer obras derivadas": es analizar la interfaz observable. Un formato de configuración es una *ficha* (facts), no expresión protegida. |
| **Reimplementar** un formato de fichero desde cero (hooks.json con 22 eventos) | **Sí, en principio** | Es escribir un lector/escritor independiente, no copiar la implementación. El resultado es un *formato*, no una copia del Software. |
| **Copiar fragmentos de código** de los bundles al repo de MegaDaemon | **NO** | §1.3(a) prohíbe copiar/adaptar/derivar, y §1.3(b) prohíbe redistribuir. |
| **Redistribuir** los ficheros extraídos (`contratos/ficheros/*`) | **NO** | §1.3(b). Deben quedarse en zona de artefactos privados, no en un repo público. |
| **Extraer las herramientas/los nombres** de tool y reimplementarlas | **Zona gris a favor** | Los nombres son hechos funcionales ( interoperabilidad), no expresión. Pero redactar "nombres de 73 tools + tipos de error" en un doc **público** es微量 redistribution de información del producto → **recomendación: mantenerlo en artefactos privados**. |

### Otras cláusulas relevantes (resumen honesto del resto)

- **§9.2 Arbitraje individual obligatorio** y **§9.3 renuncia a acción colectiva / jurado**
  (destacado en mayúsculas en el preámbulo).
- Distribución prohibida (b) → los binarios de Cursor no se pueden redistribuir.
- Limitado a "personal use only" → no hay licencia de reutilización comercial del producto.

---

## 8.2 ThirdPartyNotices — qué terceros hay dentro

`ThirdPartyNotices.txt` (185 562 B). El daemon trae además su propio fichero de licencias:
`extensions/cursor-agent-host/dist/agent-host-daemon/dist/bin/daemon.cjs.LEGAL.txt` (11 025 B).

Componentes de terceros identificados **con licencia literal** en `daemon.cjs.LEGAL.txt`:

| Componente | Licencia |
|---|---|
| `undici/lib/web/fetch/body.js` → formdata-polyfill | MIT (Jimmy Wärting) |
| `undici/lib/web/websocket/frame.js` → ws | MIT (Einar Otto Stangvik) |
| `tough-cookie` | BSD-3-Clause (Salesforce) |
| `re2js` v1.2.2 | MIT (Alexey Vasiliev) — *"JavaScript port of RE2, a regular expression engine that provides linear time matching"* |
| `is-extendable` | MIT (Jonas Schlinkert) |
| `strip-bom-string` | MIT (Jonas Schlinkert) |
| `ieee754` | BSD-3-Clause (Feross Aboukhizeh) |
| `image-q` | MIT |

→ **`re2js` (RE2 lineal) es usado por Cursor para el matcher de los hooks.** Es la razón por la que
el matcher de `hooks.json` puede ser una regex sin riesgo de ReDoS. **Recomendación directa
para MegaDaemon**: usar `re2` (crate de Rust) en vez de `regex` para los matchers de hooks.

---

## 8.3 `product.json` — metadatos del producto

`product.json` (56 407 B). Campos de interés para el RE (nombres de clave observados):

- Identidad del producto y del editor base (VS Code fork).
- **URLs de marketplace/extensions**, telemetry, updates y del servicio de cuentas.
- Flags de producto y configuración del update service.

**NO MEDIDO con detalle**: no se ha extraído el bloque literal de `product.json` en este informe
(marca de tiempo y contenido exactos pendientes). Lo que sí está medido:

- `build-info.json` del daemon (`extensions/cursor-agent-host/dist/agent-host-daemon/dist/bin/build-info.json`):
  ```json
  {"cursorVersion":"3.32.4","commit":"0925c4b8226c56947c43f0b555fb98e187a7e029",
   "buildDate":"2026-11-24T06:34:58.000Z","agentHarnessCdnUrl":"https://downloads.cursor.com/agent-host-harness/"}
  ```
  → **`agentHarnessCdnUrl`**: el harness del agente se descarga de un CDN. En un entorno offline
  (MegaDaemon), esto significa que el runtime del agente NO está en el paquete → **NO MEDIDO**.

---

## 8.4 Precios y planes

**NO MEDIDO / NO PRESENTE EN EL PAQUETE.** Razón medible: en los bundles analizados
(`workbench.desktop.main.js`, `cursor-agent-host`, `cursor-mcp`, `molten`) los literales de
pricing/plan no aparecen como constantes de precio; el producto **pregunta al servidor**.
Lo que sí se puede afirmar con evidencia:

- `molten/dist/renderer/assets/teams-DPBRQVc3.js:2` enlaza a `https://cursor.com/privacy` y
  `https://cursor.com/terms-of-service` (la UI de teams lleva a los términos legales).
- `molten/dist/renderer/assets/page-lznzVes8.js:2` enlaza a
  `https://cursor.com/docs/bugbot#usage-based-billing` → **confirma el modelo de billing
  "usage-based"** de Bugbot, pero **no la cifra**.
- `molten/dist/renderer/assets/new-agent-pane-1EkwHgJt.js:4` enlaza a
  `https://cursor.com/dashboard?tab=integrations` → la config de integraciones/MCP vive en el
  dashboard web, no en el cliente.

→ **Los precios, los límites de uso, el número de requests incluidos y las condiciones del plan
NO son recuperables del paquete.** Hay que leerlos en `cursor.com/pricing` (con `curl`, no
`web_search`). **Pendiente para el root.**

---

## 8.5 Valoración para MegaDaemon

| Aspecto | Valoración | Motivo |
|---|---|---|
| **EULA como restricción del RE** | **CRÍTICO — leer antes de redistribuir** | §1.3(a)+(b). El informe con citas **es** análisis (uso legítimo), pero **no** debe ir a un repo público junto con código copiado. |
| Formatos de fichero como "contratos de interoperabilidad" | **ADAPTABLE (con Caution)** | Reimplementar el *formato* sí; copiar la *implementación* no. |
| Lista de licencias de terceros | **COPIABLE tal cual** | Hechos de licencia, no expresión protegida. |
| `re2js` → usar RE2 en Rust | **COPIABLE tal cual (recomendación)** | Evitar ReDoS en matchers de hooks. |
| Precios / planes / límites de cuota | **NO MEDIDO** | No están en el paquete. Requiere `curl` a cursor.com. |
| `agentHarnessCdnUrl` (runtime descargado del CDN) | **NO MEDIDO** | El runtime no está en el paquete; no es analizable offline. |
| Términos de privacidad / TOS | **NO MEDIDO** | Solo el enlace; el texto está en la web. |

**Destino en MegaDaemon**: no aplica a `rust-core/src/` (esto es política/legal).
→ **Acción para el root**: decidir si `docs/RE_CURSOR_CONTRATOS.md` (con citas literales del
producto) puede vivir en el repo, o si debe ir solo a la Drive privada
(`15FRlRwDQdAicoZcfKFOTzeWqJQMjXIC0`, `tools/drive_push.py`).
