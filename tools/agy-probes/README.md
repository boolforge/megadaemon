# Probes de Frida sobre el language server de Antigravity

Genera el **plan de hooks** que consume el payload `frida-gum` que MegaDaemon
ya instala en el móvil (`rust-core/src/antigravity_intercept.rs`).

## El plan

El payload lee un fichero de texto tabulado y engancha `base + va_elf` en el
módulo vivo:

```
RUTA_DEL_MODULO <TAB> VA_ELF_HEX <TAB> ID <TAB> TAREA [TAB> VALOR_HEX]
```

* `TAREA` = `observar` (loguea la llamada) o `sustituir_url` (cambia el
  retorno; el valor va en hexadecimal).
* ⚠️ `VA_ELF_HEX` tiene que ser la dirección **del pclntab** (la del ELF), NO
  la del índice de Ghidra: celle está 1 MiB por encima (medido, 93 % de las
  funciones) ⇒ un plan hecho con Ghidra apunta 1 MiB equivocado.

## De dónde salen las direcciones

Del export del **pclntab** (`go.funcs.tsv`): nombre Go plano, ruta del
fichero original y línea.  Es la misma fuente que usa
`antigravity_intercept::resolver_objetivos`.

## Uso

```sh
# 180 hooks en el proveedor de autenticación del LS
python3 tools/agy-probes/generar_plan_hooks.py /ruta/go.funcs.tsv \
    --fichero "jetski/language_server/code_assist_client/auth_provider.go" \
    --listar

# el plan, listo para /data/local/tmp/megadaemon/hooks-plan.txt
python3 tools/agy-probes/generar_plan_hooks.py /ruta/go.funcs.tsv \
    --fichero "jetski/language_server" \
    --salida hooks-plan.txt
```

Ejemplo de salida (con la línea **original** del Go, que es lo que hace útil
una sonda):

```
hooks seleccionados: 180  tarea=observar
  0x66a6188  third_party/jetski/language_server/code_assist_client/auth_provider.go:151  ...RefreshingTokenSource
  0x66a6328  third_party/jetski/language_server/code_assist_client/auth_provider.go:207  ...__IDEAuthProvider_.GetTokenSource
  0x66a6878  third_party/jetski/language_server/code_assist_client/auth_provider.go:245  ...__IDEAuthProvider_.GetCAICProject
  ...
```

## Por qué esto importa para el login

El log del LS en el móvil muestra exactamente dónde decide:

```
auth_provider.go:1255] [AuthProvider] SetProjectID called with projectID: ""
auth_provider.go:1263] [AuthProvider] SetLocation called with location: ""
auth_provider.go:1271] [AuthProvider] SetUserTier called with userTier: "", tierDisplayName: ""
```

Esos `Set*` están **en el mismo fichero** que el plan de hooks de arriba: con
`observar` se ve, en vivo, qué valor llega en cada llamada y cuándo se
vacía.  Es la sonda que faltaba para cerrar el bucle de login sin adivinar.
