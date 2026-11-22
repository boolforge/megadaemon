# Ingeniería inversa de binarios Go: investigación técnica, auditoría adversaria y arquitectura definitiva

**Edición:** 2.0 — revisión adversaria, especificación de implementación y workspace Rust  
**Fecha de corte documental:** 22 de noviembre de 2026  
**Idioma:** español  
**Ámbito:** recuperación de metadatos, análisis estático/dinámico, desofuscación, reconstrucción semántica, automatización Rust y parcheo controlado de ejecutables Go (ELF, PE, Mach-O y WebAssembly).

> **Advertencia de alcance:** este documento consolida y audita los dos documentos técnicos anteriores y la investigación ampliada disponibles en el proyecto. Se ha contrastado la documentación y el código fuente público enlazado. No se ha ejecutado aquí una batería de binarios de prueba contra todas las versiones/arquitecturas ni se ha compilado cada herramienta. Por tanto, se distingue expresamente entre compatibilidad declarada, evidencia de código/documentación y compatibilidad comprobada de forma independiente. No se presenta una prueba de laboratorio inexistente como si se hubiera realizado.

---

## 0. Veredicto ejecutivo

La conclusión estratégica es distinta de la que sugerían las primeras versiones: **no conviene construir el sistema alrededor de un parcheador de cadenas ni escoger un único parser como autoridad**. El producto correcto es una plataforma de recuperación estructural y semántica con varias fuentes de evidencia, parsers versionados, importadores para descompiladores, corpus reproducible, pruebas diferenciales y una etapa de escritura/patching separada y desactivada por defecto.

El cambio más importante que faltaba en la investigación anterior es la reorganización de metadatos de tipos e interfaces introducida durante el ciclo de Go 1.27. No es un simple cambio de offsets:

- Los descriptores de tipos y los itabs pasan a una región dedicada `.go.type`.
- Se eliminan las listas `typelinks` e `itablinks` del diseño antiguo de `moduledata`.
- El runtime puede recorrer los descriptores de tipos contiguos mediante límites y tamaños; la ubicación y enumeración de itabs también cambia.
- El campo que antes contenía `textStart` en `pcHeader` deja de almacenar esa dirección: el espacio se conserva como campo reservado para mantener el layout, y el runtime usa `moduledata.text`/`textAddr`.

Por ello, un parser que trate Go 1.27 como «Go 1.20 con unos offsets distintos» puede extraer funciones y aun así fracasar al reconstruir tipos, interfaces o direcciones de código. La compatibilidad de `pclntab` y la compatibilidad de `moduledata`/tipos deben evaluarse por separado.

### Recomendación final de herramientas

| Función | Primera elección | Complemento / contraste | Advertencia esencial |
|---|---|---|---|
| Parser general y recuperación de funciones | **GoReSym** | GoRE, `debug/gosym`, `gobin` | GoReSym documenta layouts `pclntab` hasta 1.20; no asumir que su parser de tipos entiende el modelo Go 1.27 V5. |
| Tipos, interfaces y firmas modernas | **`gobin` 0.4.1** para investigar Go 1.27; **unstrip** para su rango declarado | GoRE, Ghidra GolangAnalyzerExtension | `gobin` es nuevo y requiere validación independiente; `unstrip` declara Go 1.18–1.25, no 1.27. |
| Integración en Ghidra | **GolangAnalyzerExtension v1.4.0** | GoReSym JSON / GoResolver plugin | La extensión declara Go 1.6–1.27; verificar versión exacta de Ghidra y resultados sobre el fixture real. |
| Recuperación de nombres obfuscados por similitud | **GoResolver** | BSim, BinDiff/Diaphora, corpus propio | Un score de 0,9 no es una probabilidad calibrada ni prueba identidad semántica. |
| Garble `-literals` | **`disrobe` / GoStringUngarbler según versión y arquitectura** | análisis dinámico, `garble reverse` si hay fuente/semilla | Ningún extractor de literales es universal. GoStringUngarbler está limitado a Garble 0.11–0.13 y Go 1.21–1.23. |
| Automatización y parsers propios | Rust: `object` + parser propio; `gobin` como referencia | `goblin`, `gimli`, `addr2line`, `iced-x86`, `capstone` | Separar parsing del contenedor, metadata Go, disassembly y escritura. |
| Análisis dinámico | Delve + Frida según objetivo | eBPF profiler, Qiling/Unicorn | Un hook en una función runtime no captura necesariamente todas las cadenas ni todas las rutas. |
| Decompilación | Ghidra como base abierta | IDA / Binary Ninja / Rizin | Ningún descompilador recupera automáticamente la semántica original perdida por optimización/obfuscación. |

### Principios no negociables

1. El binario original es evidencia: nunca se modifica en el primer análisis.
2. Un parser nunca debe convertir una hipótesis en un símbolo definitivo sin registrar su procedencia y confianza.
3. No se deduce la versión exacta de Go solo a partir del magic de `pclntab` cuando varias releases comparten layout.
4. Los offsets relativos, file offsets y VAs son dominios distintos y deben tener tipos distintos en el código.
5. Los formatos ELF, PE, Mach-O y Wasm no se reducen a «buscar bytes y sumar offsets».
6. Los parsers procesan entradas hostiles: límites, overflow, contadores, varints, ciclos y recursos deben estar acotados.
7. La desofuscación no equivale a descompilación; la recuperación de símbolos no equivale a reconstrucción del código fuente.
8. El LLM propone hipótesis y explica evidencia; no decide por sí solo el nombre definitivo ni escribe directamente en el binario.

---

## 1. Qué se ha auditado y cómo leer las conclusiones

Se han consolidado estos materiales del proyecto:

- `go_rebranding_deobfuscacion_guia_completa.md`: estrategia de herramientas, parcheo in-place de `funcnametab`, código Rust y script Ghidra.
- `Guía Avanzada de Ingeniería Inversa y Patching en Rust.md`: elección de Ghidra/Rizin/Frida, ecosistema Rust, parser/patcher y automatización.
- `go_reverse_engineering_research_expanded_2026-11-22.md`: catálogo de herramientas, cambios históricos de Go, auditoría inicial y arquitectura de reconstrucción.

### Escala de evidencia utilizada

- **A — fuente primaria:** código fuente oficial de Go, especificación oficial, documentación del proyecto mantenedor o tests/código del propio proyecto.
- **B — fuente técnica secundaria:** documentación de proyectos de profiling, investigación técnica, artículos de ingeniería inversa o historial de cambios con enlaces a commits.
- **C — afirmación del proyecto:** README, tabla de compatibilidad o roadmap del propio autor sin fixture independiente disponible.
- **D — hipótesis de ingeniería:** propuesta razonable que requiere laboratorio y no debe tratarse como hecho demostrado.

En las tablas, «soporta» significa normalmente «el proyecto lo declara»; solo se usa «verificado» cuando se identifica una matriz de tests/fixtures o una fuente primaria que demuestra el comportamiento concreto.

---

## 2. Auditoría adversaria: qué estaba mal, incompleto o exagerado

Esta sección prevalece sobre las afirmaciones contradictorias de los documentos anteriores.

### 2.1 Corrección crítica: el runtime sí comprueba la cabecera `pcHeader`

Una guía anterior afirmaba que el runtime no valida el magic de `pclntab` y sugería que se podía reparar la firma para hacer funcionar el ejecutable. Esa afirmación es incorrecta.

El runtime contiene `moduledataverify1`, que comprueba `magic`, `pad1`, `pad2`, `minLC` y `ptrSize`, y lanza `throw("invalid function symbol table")` cuando la cabecera no es válida. También comprueba que los offsets de entrada de la tabla de funciones estén ordenados y que `minpc`/`maxpc` concuerden con la tabla. Véase:

- https://github.com/golang/go/blob/master/src/runtime/symtab.go
- https://go.dev/src/runtime/symtab.go

GoReSym documenta una técnica para localizar `pclntab` mediante una firma asociada a `runtime_modulesinit` y reparar el magic **para continuar el análisis del parser**. Eso no demuestra que el ejecutable original pueda ejecutarse con el magic alterado. La reparación de análisis debe hacerse en una copia/buffer de trabajo, y el resultado debe etiquetarse como «metadatos recuperados tras normalización», nunca como binario runtime-reparado sin prueba de ejecución.

Referencia del proyecto y cambios declarados:

- https://github.com/mandiant/GoReSym
- https://github.com/mandiant/GoReSym/releases

**Regla:** la búsqueda por firma y la reparación de cabecera son técnicas de recuperación forense. No deben venderse como un mecanismo general de desempaquetado ni como garantía de que el programa volverá a arrancar.

### 2.2 Corrección crítica: Go 1.27 elimina `typelinks` e `itablinks`

Las versiones previas describían el cambio, pero no lo convertían en una condición arquitectónica suficiente. Los cambios del linker/runtime de Go 1.27 reorganizan el almacenamiento:

- Se introduce `.go.type` como región dedicada para descriptores de tipos y datos relacionados.
- Se elimina la lista linker-generated `typelinks`; el runtime recorre el rango de descriptores.
- Los itabs pasan a `.go.type`; `itablinks` deja de ser la lista canónica anterior.
- `moduledata` deja de tener los campos antiguos de slices para esas listas y usa campos de límites/tamaños para los datos nuevos.

Fuentes y commits:

- Historial técnico con commits concretos: https://www.golang.design/history/runtime-compiler/linker.html
- Cambio oficial `cmd/link, runtime: remove typelinks`: https://go.googlesource.com/go/+/481ab86aafe0cac177df793c9946c5ef2126137c
- GoRE: https://github.com/goretk/gore
- `gobin` 0.4.1, que declara soporte explícito del layout V5: https://docs.rs/crate/gobin/0.4.1

En el `moduledata` del runtime nuevo aparecen campos de límites/tamaños como `types`, `typedesclen`, `etypes`, `itaboffset` e `itabsize` (además de otros campos de módulo). No se deben reinterpretar como las slices antiguas ni deducir sus semánticas solo por el nombre: hay que seguir el código de la release exacta en `runtime` y `cmd/link`. La región `.go.type` pasa a ser la fuente estructural central para recorrer tipos e itabs.

El cambio también afecta a la interpretación de los descriptores: Go 1.26 ya había movido el bit de direct/indirect interface desde el high bit de `Kind` a `TFlag`. Un parser que aplique el mask de `Kind` de Go 1.25 a Go 1.26+ puede clasificar mal tipos aunque haya encontrado su dirección correctamente. El historial técnico enlaza el cambio y las actualizaciones necesarias en herramientas de debugging.

Fuentes adicionales:

- https://www.golang.design/history/runtime-compiler/data-structures.html
- https://github.com/golang/go/blob/master/src/internal/abi/type.go
- https://github.com/golang/go/blob/master/src/runtime/runtime2.go

**Consecuencia:** un extractor de funciones que siga encontrando `pclntab` en Go 1.27 no queda por ello validado para tipos/itabs. La compatibilidad se debe publicar como matriz separada:

- `pclntab/functions`
- `moduledata`
- `type descriptors`
- `interface tables / itabs`
- `buildinfo`
- `inline metadata`

### 2.3 Corrección crítica: en Go 1.27 `textStart` ya no es una dirección válida dentro de `pcHeader`

En versiones anteriores el campo `textStart` era la base para sumar `entryoff`. En el runtime actual, ese espacio se conserva como un campo reservado (`_ uintptr`) porque almacenar la dirección requería una relocación; el comentario oficial indica que debe usarse `moduledata.text`.

Fuente primaria:

- https://github.com/golang/go/blob/master/src/runtime/symtab.go
- https://go.dev/src/cmd/link/internal/ld/pcln.go

El linker actual escribe cero en ese slot reservado (`SetUintptr(..., 0)`). Por tanto:

- En Go 1.27 no se debe calcular `function_va = header.textStart + entryoff` leyendo el header.
- La base debe obtenerse de `moduledata.text` y, cuando existan varias secciones de texto, la traducción debe respetar `textsectmap`/`textAddr`.
- Si `moduledata` no se ha localizado, el parser puede informar offsets relativos y marcar la VA como no resuelta, en vez de inventar una dirección.

La documentación OpenTelemetry sobre el header Go 1.20 sigue siendo útil para el formato histórico, pero no debe extrapolarse sin matices al nuevo layout:

- https://github.com/open-telemetry/opentelemetry-ebpf-profiler/blob/main/doc/gopclntab.md

### 2.4 Corrección: `unstrip` no debe anunciarse como parser Go 1.27

Una versión anterior de la investigación atribuía a `unstrip` un rango Go 1.16–1.27. El README actual del repositorio consultado declara **Go 1.18–1.25**. Además especifica:

- ELF, Mach-O y PE; amd64 y arm64.
- Mach-O tiene rutas de código cubiertas por tests unitarios, pero el README declara que falta un fixture de integración macOS.
- No se ha ejecutado una campaña estructurada de fuzzing.
- No recupera firmas completas de funciones libres de nivel superior en el caso general sin DWARF.
- El rewriting de símbolos es una función separada; el patching in-place está limitado.

Fuentes:

- https://github.com/riven-labs/unstrip
- https://docs.rs/unstrip/latest/unstrip/
- https://docs.rs/unstrip/latest/unstrip/rewrite/

La recomendación correcta es usar `unstrip` como una herramienta muy interesante para el rango declarado y como fuente de ideas de implementación, no como solución universal ni como parser V5 de Go 1.27.

### 2.5 Hallazgo nuevo: `gobin` 0.4.1 sí declara soporte Go 1.16–1.27 y layout V5

El crate `gobin` de ATRAPS LLC no debe confundirse con el módulo `gobin` que aparece dentro de la API de `unstrip`.

`gobin` 0.4.1 declara:

- Go 1.16–1.27.
- ELF, Mach-O, PE y WebAssembly.
- Parsing de `pclntab`, build info, build IDs, tipos e itabs.
- Tratamiento explícito del layout Go 1.27 V5, sin slices `typelinks`/`itablinks`.
- Reconstrucción de memoria lineal para los segmentos de datos de Wasm.
- Clasificación de confianza High/Medium/Low.

Fuentes:

- https://docs.rs/crate/gobin/0.4.1
- https://docs.rs/gobin/latest/gobin/
- https://github.com/ATRAPSLLC/gobin
- https://docs.rs/crate/gobin/0.4.1/source/Cargo.toml

**Valoración adversaria:** es el hallazgo más prometedor para cubrir la brecha Go 1.27 en Rust, pero no merece una calificación de «fiable» solo por declarar soporte. El crate es joven, fija `rust-version = 1.88`, depende directamente de `goblin 0.10.7` y no aporta por sí solo una validación independiente. Hay que inspeccionar fixtures, tests, commits, errores de parser y reproducir muestras de Go 1.16–1.27 antes de convertirlo en autoridad.

### 2.6 Corrección: GoReSym no equivale a compatibilidad completa con Go 1.27

GoReSym es una herramienta de primera línea para extraer metadatos Go y es especialmente valiosa por su cercanía al código del runtime/toolchain. Sin embargo, su README enumera layouts `pclntab` pre-1.2, 1.2, 1.16, 1.18 y 1.20, y documenta soporte para Go 1.24 en los cambios recientes consultados. Su parser de tipos se basa en `moduledata`/`typelinks` clásicos.

Fuentes:

- https://github.com/mandiant/GoReSym
- https://github.com/mandiant/GoReSym/releases
- https://github.com/mandiant/GoReSym/blob/master/GoReSym.proto

Distinción necesaria:

- **Extracción de funciones:** el layout `pclntab` Go 1.20 continúa siendo relevante en Go 1.27; GoReSym puede ser una primera pasada útil, pero no debe etiquetarse como Go 1.27 validado sin un fixture específico.
- **Extracción de tipos e interfaces:** no asumir soporte V5; el esquema de salida todavía representa `Typelinks`/`ITablinks` antiguos.
- **Reparación de magic:** recuperación para análisis, no reparación demostrada del ejecutable runtime.

### 2.7 Corrección: Ghidra y los importadores

El documento previo afirmaba la existencia de un `goresym_ghidra.py` oficial. En el repositorio oficial de GoReSym, el importador citado es **IDAPython** (`IDAPython/goresym_rename.py`), no un importador Ghidra oficial equivalente.

Fuentes:

- GoReSym README: https://github.com/mandiant/GoReSym
- Importador IDA: https://github.com/mandiant/GoReSym/blob/master/IDAPython/goresym_rename.py
- GoResolver sí incluye plugins para IDA y Ghidra: https://github.com/volexity/GoResolver

Para Ghidra, la opción especializada que hay que evaluar es `Ghidra_GolangAnalyzerExtension`, cuyo release v1.4.0 declara soporte Go 1.6–1.27 y añade análisis de funciones, argumentos, líneas, tipos y Wasm con plugins adicionales:

- https://github.com/mooncat-greenpy/Ghidra_GolangAnalyzerExtension
- https://github.com/mooncat-greenpy/Ghidra_GolangAnalyzerExtension/releases

No confundir «el proyecto declara soporte» con «la versión instalada de Ghidra y el binario concreto han sido validados». La issue de Ghidra relativa a cambios de tipos Go 1.26 ilustra el riesgo de desfase entre snapshots del runtime y analizadores:

- https://github.com/NationalSecurityAgency/ghidra/issues/9219

### 2.8 Corrección: el ejemplo Frida/ABI contenía un comentario invertido

La función runtime tiene la firma:

```go
func slicebytetostring(buf *tmpBuf, ptr *byte, n int) string
```

En amd64 ABIInternal, los primeros argumentos enteros se asignan a `RAX, RBX, RCX, ...`; por tanto, en esa firma concreta:

- `RAX`: `buf` (`*tmpBuf`)
- `RBX`: `ptr` (`*byte`)
- `RCX`: `n` (`int`)

El código anterior leía `RBX` y `RCX`, lo que corresponde a `ptr` y `n`, pero el comentario decía que `RAX` era el puntero y `RBX` el buffer. El comentario era incorrecto.

Fuentes:

- ABIInternal: https://go.dev/src/cmd/compile/abi-internal
- Runtime string helpers: https://go.dev/src/runtime/string.go

Además, ese hook solo es válido para amd64 con esa ABI y esa firma. En arm64 cambian los registros; en otros targets también. El helper puede no ser llamado en todos los caminos, puede haber optimizaciones/inlining y muchas conversiones ordinarias producirán ruido. No se debe describir como un «hook universal que captura todos los literales Garble».

### 2.9 Corrección: el catálogo de herramientas debe separar capacidades declaradas y alcance real

- `GoStringUngarbler`: Windows PE y Linux ELF; Garble v0.11.0–v0.13.0; Go 1.21–1.23. El proyecto declara que necesita nuevos patrones para versiones posteriores y deja «otras arquitecturas» como TODO. No generalizar a arm64, Mach-O o Go moderno.
- `disrobe`: el README consultado declara Go 1.15–1.26; el soporte de recuperación de literales Garble es más estrecho y debe comprobarse por arquitectura. No extrapolar a Go 1.27.
- Garble: el README actual requiere Go 1.26 o posterior; el changelog v0.18.0 declara soporte Go 1.27. `-literals` y `-tiny` tienen efectos distintos; el control-flow obfuscation está documentado como experimental.

Fuentes:

- https://github.com/mandiant/gostringungarbler
- https://github.com/1-3-7/disrobe
- https://github.com/burrowers/garble
- https://github.com/burrowers/garble/blob/master/CHANGELOG.md

---

## 3. Modelo técnico de los metadatos Go

### 3.1 Qué sobrevive al stripping y por qué

Go necesita metadatos internos para stack traces, GC, reflexión, dispatch de interfaces y resolución de PCs. Por eso un binario compilado con `-ldflags="-s -w"` puede conservar `pclntab`, `moduledata` y tipos que el runtime aún necesita. Pero no significa que conserve todo:

- `-s` elimina la tabla de símbolos/debug asociada al linker.
- `-w` omite DWARF.
- El runtime sigue necesitando parte de la información de funciones.
- La información de tipos retenida depende de reachability, reflexión, interfaces, métodos y decisiones del linker.
- `-trimpath` altera rutas de origen.
- Garble puede ofuscar nombres, rutas, literales y eliminar metadatos adicionales.

Fuentes:

- https://go.dev/cmd/link/
- https://go.dev/doc/gc-guide
- https://go.dev/src/debug/gosym/pclntab.go
- https://developers.redhat.com/articles/2024/04/03/how-add-debug-support-go-stripped-binaries

### 3.2 Evolución de `pclntab`

| Familia | Magic | Cambio relevante | Riesgo para parsers |
|---|---|---|---|
| Go 1.2–1.15 | `0xfffffffb` | Formato inicial; `functab` con campos `uintptr`; nombres indexados mediante tabla inicial | No aplicar layout moderno. |
| Go 1.16–1.17 | `0xfffffffa` | Cambios de cabecera/tablas; tabla de nombres pasa a strings concatenadas y offsets | Layout distinto de 1.2; campos/offsets versionados. |
| Go 1.18–1.19 | `0xfffffff0` | `entryoff` relativo y `funcoff`; cambios asociados a ABIInternal y generics | `functab` moderno de 2 × uint32, no dos `uintptr`. |
| Go 1.20+ | `0xfffffff1` | Nombre de símbolos cambia con `:`; `startLine` en `_func`; familia de layout moderna | El magic no distingue 1.20, 1.21… 1.27. |
| Go 1.27 | `0xfffffff1` | La familia `pclntab` sigue usando magic 1.20+, pero `textStart` deja de ser una VA almacenada; la reorganización de tipos ocurre fuera del parser básico de `pclntab` | No deducir `textStart` del slot reservado; types/itabs requieren V5. |

Fuentes:

- https://go.dev/src/internal/abi/symtab.go
- https://go.dev/src/debug/gosym/pclntab.go
- https://github.com/open-telemetry/opentelemetry-ebpf-profiler/blob/main/doc/gopclntab.md
- https://github.com/golang/go/blob/master/src/runtime/symtab.go

**Tamaño de cabecera:** no codificar una cifra única sin versionar el layout. Para la familia moderna, el encabezado conserva los campos dependientes del tamaño de puntero y el linker actual reserva el slot que antes era `textStart`. El código oficial del linker calcula el tamaño con `8 + 8*PtrSize`; esto da 72 bytes en 64-bit y 40 bytes en 32-bit. El parser debe validar exactamente ese tamaño antes de leer campos. Para formatos anteriores, implementar estructuras y límites separados según magic.

Fuente primaria del generador de cabecera:

- https://go.dev/src/cmd/link/internal/ld/pcln.go

### 3.3 `functab` y `_func`: la distinción que más errores evita

Desde Go 1.18, cada registro de la tabla de funciones contiene dos valores de 32 bits:

```c
struct functab_go118_plus {
    uint32_t entryoff;  // offset del código respecto a la base de texto
    uint32_t funcoff;   // offset hacia el registro _func
};
```

El segundo campo **no es `nameOff`**. Es un offset a `_func`. Dentro de `_func`, después de `entryOff`, está `nameOff`, que apunta a `funcnametab`.

Camino correcto:

```text
pcHeader
  └── pcln/functab region
        ├── entryoff (u32)
        └── funcoff  (u32) ──> _func
                                ├── entryOff
                                ├── nameOff ──> funcnametab
                                ├── pcsp / pcfile / pcln
                                ├── npcdata / cuOffset / startLine
                                └── pcdata[] / funcdata[]
```

Para Go 1.18+, el tamaño de campo de `functab` es 4 bytes incluso en un proceso de 64 bits. Cada registro mide 8 bytes; la tabla incluye una entrada final/sentinel para cerrar el rango del último símbolo. La cantidad de registros y el límite de lectura deben calcularse con overflow checks y el layout concreto.

Fuente primaria:

- https://github.com/golang/go/blob/master/src/debug/gosym/pclntab.go
- https://github.com/open-telemetry/opentelemetry-ebpf-profiler/blob/main/doc/gopclntab.md

### 3.4 Nombres, archivos, líneas e inlining

- `funcnametab`: strings terminadas en NUL. Desde Go 1.16, los nombres se concatenan y se referencian mediante offsets de bytes.
- `cutab`: tabla de unidades de compilación que participa en la resolución de archivos.
- `filetab`: rutas de archivo embebidas, potencialmente afectadas por `-trimpath` o Garble.
- `pctab`: programas compactos que codifican cambios de PC y valores (línea, archivo, stack delta, etc.). No son arrays planos de enteros.
- `FUNCDATA_InlTree` y `PCDATA_InlTreeIndex`: permiten recuperar parte de la información de funciones inline. Una función inline no tiene por qué aparecer como una función física independiente en el código máquina.

El parser debe exponer por separado:

- símbolo físico (función compilada)
- inline frame (función lógica representada en un rango de PC)
- nombre observado (texto literal de metadata)
- nombre normalizado (presentación/sanitización)
- nombre inferido (hipótesis)

Fuente de estructura e inlining:

- https://github.com/open-telemetry/opentelemetry-ebpf-profiler/blob/main/doc/gopclntab.md
- https://go.dev/src/runtime/symtab.go
- https://developers.redhat.com/articles/2024/04/03/how-add-debug-support-go-stripped-binaries

### 3.5 `moduledata`: no es una estructura estable de ABI pública

`moduledata` lo escribe el linker y lo consume el runtime. El propio código fuente dice que cualquier cambio debe mantenerse sincronizado con `cmd/link/internal/ld/symtab.go`. No es una ABI pública estable. Las herramientas externas mantienen copias de estructuras internas y deben seguir los cambios de Go.

Consecuencias:

- No serializar `moduledata` como un struct C universal.
- Mantener layouts por rango/version/arquitectura.
- Distinguir slices Go (data/len/cap) de campos uintptr.
- Resolver relocations y VAs del contenedor antes de interpretar punteros.
- En Go 1.27, no esperar `typelinks`/`itablinks` clásicos.
- Respetar `textsectmap` si hay varias secciones de texto.

Fuentes:

- https://github.com/golang/go/blob/master/src/runtime/symtab.go
- https://www.golang.design/history/runtime-compiler/linker.html

### 3.6 Build info, build ID y versión

El build info puede incluir versión de Go, path del módulo principal, dependencias, reemplazos y settings. En Go 1.18+ cambió el formato inline; las versiones antiguas tenían otra representación. `debug/buildinfo` y `go version -m` son primeras herramientas de referencia.

No confundir:

- build ID con identidad criptográfica del ejecutable;
- versión declarada con prueba de que todo el binario fue compilado exactamente con esa toolchain (puede haber cgo, plugins, objetos externos o datos manipulados);
- rutas de fuente con rutas reales del sistema de construcción.

Fuentes:

- https://pkg.go.dev/debug/buildinfo
- https://go.dev/src/debug/buildinfo/buildinfo.go
- https://go.dev/cmd/go/#hdr-Print_Go_version_and_module_information

---

## 4. Catálogo técnico y valoración de herramientas Go

### 4.1 GoReSym — Mandiant

**URL:** https://github.com/mandiant/GoReSym  
**Licencia:** MIT (según repositorio).  
**Lenguaje:** Go.  
**Rol:** parser/recuperador de metadata Go, no descompilador.

Extrae arquitectura, sistema operativo, endianness, versión, funciones, direcciones, nombres, rutas, líneas, tipos e interfaces. Reutiliza/copia estructuras del código interno de Go y mantiene sus propias adaptaciones. Declara soporte ELF/PE/Mach-O, x86/x64 y ARM64, además de variantes de endian.

**Fortalezas**

- Se apoya en la implementación upstream del runtime/compiler, lo que reduce la cantidad de suposiciones inventadas.
- Maneja stripped, secciones partidas y ciertos binarios desempaquetados/malformados.
- Escanea `pclntab` y `moduledata` cuando los símbolos/section names no bastan.
- JSON de salida práctico para integrarlo con herramientas RE.
- El importador oficial disponible en el repo es IDAPython.

**Debilidades y límites**

- Copiar código `internal` de Go implica mantener forks sincronizados con upstream; el propio README reconoce ese coste.
- El README enumera layouts `pclntab` hasta 1.20; no tomar la frase «aim to support latest» como prueba de soporte Go 1.27 V5.
- El esquema `ModuleData` del protobuf conserva `Typelinks` e `ITablinks`, lo que exige revisar cómo representa el nuevo modelo.
- El override manual `-v` puede producir resultados falsos si se usa una versión incorrecta.
- La recuperación de magic para parsing no valida que el ejecutable parcheado arranque.

**Recomendación:** mantenerlo como extractor independiente de funciones/metadata y referencia de runtime. En Go 1.27, usarlo en paralelo con `gobin`/GoRE y comparar resultados; no dejar que su JSON sustituya a la versión del parser propio.

Fuentes:

- https://github.com/mandiant/GoReSym
- https://github.com/mandiant/GoReSym/blob/master/GoReSym.proto
- https://github.com/mandiant/GoReSym/blob/master/IDAPython/goresym_rename.py
- https://github.com/mandiant/GoReSym/releases

### 4.2 GoRE — `goretk/gore`

**URL:** https://github.com/goretk/gore  
**Rol:** biblioteca Go para análisis de binarios Go.

Es una biblioteca, no un CLI idéntico a GoReSym. Es valiosa para construir herramientas y para entender las diferencias entre versiones. Su historial/documentación recoge layouts modernos y la reorganización de tipos V5. Hay que revisar los paquetes concretos y el branch/tag usado; `develop` no equivale automáticamente a una release estable.

**Uso recomendado:** parser de contraste y biblioteca de referencia para el modelo de datos. No acoplar la arquitectura entera a una única API interna sin tests propios.

### 4.3 Redress — `goretk/redress`

**URL:** https://github.com/goretk/redress  
**Rol:** CLI de análisis Go basado en el ecosistema GoRE.

Útil para extracción rápida de metadatos y triage; conviene compararlo con GoReSym y `gobin`. Su condición de herramienta CLI y la relación con GoRE facilitan automatización, pero el rango exacto de Go, los formatos y la calidad de los tipos deben tomarse del tag/README actual y validarse con fixtures propios.

### 4.4 `unstrip` — Riven Labs

**URL:** https://github.com/riven-labs/unstrip  
**Crate:** https://docs.rs/unstrip/latest/unstrip/  
**Rol:** recuperar funciones, tipos, interfaces, firmas de métodos, inline frames y xrefs desde binarios stripped.

**Rango declarado consultado:** Go 1.18–1.25; ELF/Mach-O/PE; amd64/arm64; PIE y non-PIE. La matriz de integración declarada cubre Linux ELF amd64/arm64 y Windows PE amd64; Mach-O tiene unit tests, pero falta fixture de integración macOS según README.

**Capacidades destacadas**

- Funciones, archivos y líneas.
- Firmas de métodos en sintaxis Go cuando el linker conserva el tipo.
- Tipos y layout de structs.
- Pares interface/concrete a través de itabs.
- PC lookup con inline call stacks.
- `--xref` con CALL/BL directos y dispatch indirecto por itab.
- Exporters IDA/Ghidra/Binary Ninja.
- Módulos nuevos para `diff`, `fingerprint`, `dispatch`, `goroutines`, `inline`, `probe`, `strings`, `rewrite`, etc., visibles en la documentación del crate.

**Debilidades declaradas**

- No cubre Go anterior a 1.18.
- No cubre Go 1.26/1.27 según README consultado.
- Las rutas Mach-O no tienen aún fixture de integración completa.
- No declara campaña estructurada de fuzzing.
- No recupera firmas de funciones libres top-level de forma general sin DWARF.
- Algunos campos de tipo (p. ej. package paths y argumentos de function types) tienen huecos documentados.
- La escritura de símbolos no equivale a reparar el runtime ni a regenerar la metadata Go.

**Rewriting:** el módulo `rewrite` puede generar `.symtab`/`.strtab` sintéticos en ELF y tabla COFF en PE; Mach-O no figura como implementado en esa versión. El modo in-place es una operación destructiva y tiene limitaciones adicionales. Debe ejecutarse sobre una copia.

**Recomendación:** muy buena opción para análisis profundo de Go 1.18–1.25 y para aprender una arquitectura Rust moderna. No es el parser de referencia para Go 1.27 V5.

Fuentes:

- https://github.com/riven-labs/unstrip
- https://docs.rs/unstrip/latest/unstrip/
- https://docs.rs/unstrip/latest/unstrip/rewrite/
- https://docs.rs/unstrip/latest/unstrip/inline/

### 4.5 `gobin` 0.4.1 — ATRAPS LLC

**URL:** https://github.com/ATRAPSLLC/gobin  
**Docs:** https://docs.rs/gobin/latest/gobin/  
**Crate:** https://docs.rs/crate/gobin/0.4.1

**Rango declarado:** Go 1.16–1.27.  
**Formatos declarados:** ELF, Mach-O, PE, Wasm.  
**Rol:** biblioteca Rust de extracción de metadata Go.

**Capacidades declaradas**

- detección de formato Go;
- build ID/build info;
- `pclntab`, funciones, rutas y líneas;
- descriptores `abi.Type`;
- itabs;
- recursos `//go:embed` (paths y bytes);
- orden de inicialización (`moduledata.inittasks`);
- datos FIPS (`GOFIPS140`, `__go_fipsinfo`);
- dependencias, replace directives y go.sum hashes;
- confidence scoring;
- Go 1.27 V5 sin `typelinks`/`itablinks` clásicos;
- Wasm con reconstrucción de memoria lineal desde segmentos.

**Fortalezas potenciales**

- Diseño por capas: `BinaryContext`, formatos y parsers de estructuras.
- Zero-copy y traducción VA.
- Dependencia principal `goblin` acotada a ELF/PE/Mach-O.
- Lints estrictos para evitar `unwrap`, `panic`, indexing inseguro y overflow aritmético.
- Es la opción del catálogo que declara explícitamente V5/Go 1.27 y Wasm.

**Riesgos**

- Proyecto joven y sin validación externa independiente en esta investigación.
- Requiere Rust 1.88 y edition 2024 según `Cargo.toml` publicado.
- La política de lints no prueba por sí sola ausencia de bugs.
- La cobertura real de cada arquitectura/formato/version debe verificarse con fixtures.
- El confidence scoring necesita calibración: un nivel High debe tener una definición auditable, no ser un número decorativo.

**Recomendación:** candidato prioritario para laboratorio Go 1.27; comparar sus salidas con GoRE, runtime/linker y fixtures propios. No reemplazar automáticamente GoReSym/unstrip hasta pasar las pruebas de la sección 12.

Fuentes:

- https://docs.rs/crate/gobin/0.4.1
- https://docs.rs/crate/gobin/0.4.1/source/Cargo.toml
- https://docs.rs/crate/gobin/0.4.1/source/README.md
- https://github.com/ATRAPSLLC/gobin

### 4.6 `disrobe` — `1-3-7/disrobe`

**URL:** https://github.com/1-3-7/disrobe  
**Rol:** análisis/desofuscación de binarios Go, con tratamiento específico de Garble.

El README consultado declara Go 1.15–1.26. Tiene valor como herramienta adicional para Garble y para recuperar metadata en casos donde otros parsers fallen.

**Límites:** la recuperación de literales Garble tiene un rango más estrecho que el parser general y se ha descrito para x86-64; no asumir soporte de literales en ARM64, Go 1.27 o cualquier variante de Garble sin fixtures. La recuperación de nombres hash no debe confundirse con la restauración de nombres originales: sin fuente/seed o un corpus equivalente, los hashes pueden seguir siendo irreversibles.

**Recomendación:** mantenerlo como herramienta especializada y comparar sus resultados con GoStringUngarbler y ejecución controlada. No usarlo como único parser.

Fuente:

- https://github.com/1-3-7/disrobe

### 4.7 GoResolver — Volexity

**URL:** https://github.com/volexity/GoResolver  
**Rol:** recuperación de nombres mediante extracción de símbolos y similitud de CFG.

Declara soporte Windows/macOS/Linux; PE/ELF/Mach-O; x86, x64, ARM y ARM64. Requiere Go 1.20.6 o superior para generar/usar referencias. Incluye plugins IDA y Ghidra.

El umbral por defecto documentado es 0.9 y el CLI permite separar extracción (`--extract`) de comparación de grafos (`--graph`).

**Qué hace bien**

- Aporta una técnica distinta a la lectura de metadata: compara estructura de control de funciones con referencias compiladas.
- Puede identificar funciones de runtime/stdlib o dependencias cuando los nombres se han reemplazado.
- Puede acelerar la clasificación de grandes cantidades de funciones repetidas.

**Qué no demuestra**

- Un CFG parecido no prueba que el comportamiento sea idéntico.
- Cambios de versión Go, arquitectura, optimización, inlining, build tags, cgo y flags alteran el grafo.
- El umbral 0.9 no es una probabilidad estadística calibrada.
- Una colisión de CFG puede proponer un nombre falso pero plausible.
- No recupera mágicamente los nombres privados originales de la lógica de negocio.

**Estrategia recomendada:** corpus de referencia por toolchain, GOOS/GOARCH, build tags y flags. Registrar top-k candidatos, score bruto, características y discrepancias. Aplicar nombres automáticamente solo cuando haya evidencia adicional (strings, xrefs, tipos, comportamiento, firma de llamadas) y nunca si el candidato compite con otro casi igual.

Fuente:

- https://github.com/volexity/GoResolver

### 4.8 GoStringUngarbler — Mandiant

**URL:** https://github.com/mandiant/gostringungarbler  
**Rol:** extracción de literales transformados por `garble -literals`.

El README declara Windows PE y Linux ELF, Garble 0.11.0–0.13.0 y Go 1.21–1.23. Utiliza patrones/regex para localizar rutinas, Unicorn para emular y Capstone para desensamblar; depende de `pefile`.

**Mecanismo:** localiza rutinas de descifrado, las emula y puede parchear el binario insertando el literal descifrado y un stub que invoca `runtime_slicebytetostring`. El parcheo modifica el ejecutable de salida.

**Crítica:** no es un extractor general de strings, no cubre todas las versiones de Garble y el README deja explícitamente otras arquitecturas como trabajo futuro. La heurística regex puede romperse con cambios del compilador/obfuscador.

**Recomendación:** usarlo únicamente después de identificar con evidencia la versión de Garble/Go y la arquitectura. Preservar el original, revisar cada parche y comparar ejecución/bytes.

Fuente:

- https://github.com/mandiant/gostringungarbler

### 4.9 Garble — comprender el generador de la ofuscación

**URL:** https://github.com/burrowers/garble  
**Changelog:** https://github.com/burrowers/garble/blob/master/CHANGELOG.md

Garble envuelve el toolchain de Go y transforma identificadores, package paths y posiciones. Con `-literals`, transforma literales y archivos embed; con `-tiny`, elimina más información y puede suprimir stack traces/posiciones.

Hechos que condicionan la estrategia:

- El README consultado requiere Go 1.26 o posterior.
- El changelog v0.18.0 declara soporte Go 1.27.
- `-literals` es opt-in; literales usados en expresiones constantes no pueden ser transformados del mismo modo.
- Los contenidos embed pueden decodificarse en runtime; los nombres de archivo embed siguen visibles.
- La clave de decodificación está en el ejecutable: obfuscation, not encryption.
- `-tiny` elimina información de posiciones y runtime diagnostics; no es simplemente «strings cifradas».
- La ofuscación de control flow está documentada como experimental.
- `garble reverse` es especialmente útil si se conserva el código fuente y la semilla/inputs necesarios; no es un deobfuscator universal de binarios ajenos.

**Recomendación:** versionar Garble junto al Go toolchain en los fixtures. Probar separadamente `-literals`, `-tiny`, `-seed`, build tags y control-flow directives.

Fuente:

- https://github.com/burrowers/garble
- https://github.com/burrowers/garble/blob/master/CHANGELOG.md
- https://github.com/burrowers/garble/blob/master/docs/CONTROLFLOW.md

### 4.10 `ungarble_bn` y scripts Binary Ninja

El documento anterior mencionaba `ungarble_bn` como si su mecanismo LLIL estuviera validado y fuese un componente estable. No se ha encontrado en esta pasada evidencia documental suficiente para asignarle una matriz de versiones/arquitecturas robusta. Se conserva como **experimento a inspeccionar**, no como recomendación de producción.

Antes de integrarlo, exigir:

- repositorio y commit exactos;
- licencia;
- fixtures Garble por versión;
- soporte real de x86-64/ARM64;
- prueba de que el emulador LLIL respeta flags, memoria, calling convention y efectos laterales;
- comparación del literal reconstruido con una ejecución real del binario.

No usarlo como base de una arquitectura propia hasta superar estos puntos.

---

## 5. Herramientas generales: qué aporta cada una y qué no

### 5.1 Toolchain oficial de Go

| Herramienta/API | Uso real | Límite |
|---|---|---|
| `go version -m` | versión, módulo, dependencias y build settings cuando se conservan | puede faltar o estar manipulado; no recupera lógica de negocio. |
| `debug/buildinfo` | API programática para build info | no parsea toda la metadata runtime. |
| `debug/gosym` | parsear PC/line tables y resolver funciones/PCs | no es un framework universal de tipos/itabs; depende de cómo se le suministra el contenedor/section address. |
| `go tool nm` | símbolos presentes en tabla de símbolos | con `-s` muchos símbolos se pierden; no equivale a pclntab. |
| `go tool objdump` | desensamblado por función/símbolo cuando hay información | no es un decompilador ni resuelve automáticamente nombres hash. |
| `go tool addr2line` | PC a función/archivo/línea según datos disponibles | su utilidad cae cuando faltan metadata o el PC no está en el módulo correcto. |
| `go tool compile -S` | inspección del output del compilador para fixtures controlados | necesita fuente/toolchain y no recupera el original de un binario desconocido. |

URLs:

- https://pkg.go.dev/debug/gosym
- https://pkg.go.dev/debug/buildinfo
- https://go.dev/cmd/nm/
- https://go.dev/cmd/objdump/
- https://go.dev/cmd/addr2line/
- https://go.dev/src/cmd/compile/abi-internal

### 5.2 Ghidra

**URL:** https://github.com/NationalSecurityAgency/ghidra  
**Extensión Go:** https://github.com/mooncat-greenpy/Ghidra_GolangAnalyzerExtension

Ghidra es la base abierta más atractiva para una pipeline automatizada con decompilación, P-Code, scripts Java/Python, análisis headless y data type manager.

**Ventajas**

- Decompilador gratuito y automatizable.
- Modelo de programa y tipos relativamente accesible.
- P-Code permite análisis intermedio y análisis de llamadas.
- La extensión GolangAnalyzerExtension declara soporte Go 1.6–1.27 y ofrece renombrado, comentarios de fuente/línea, argumentos y tipos.

**Riesgos**

- El soporte Go nativo y el de la extensión no son idénticos.
- Las snapshots de tipos runtime pueden quedarse desactualizadas; issue #9219 muestra un fallo relacionado con Go 1.26.
- No asumir que la convención de llamada detectada por Ghidra está bien aplicada en todos los sitios; validar funciones, wrappers ABI0/ABIInternal y cgo.
- Las funciones inline pueden crear diferencias entre símbolos físicos y frames lógicos.
- Los nombres de funciones deben importarse con source type y provenance correctos, no como nombres manuales indistinguibles.

**Recomendación:** usar Ghidra como entorno de análisis primario si se prioriza software libre y scripting. Instalar la extensión compatible con la versión exacta de Ghidra, importar metadata desde al menos dos parsers y registrar las diferencias.

### 5.3 IDA Pro

**URL:** https://hex-rays.com/ida-pro/

IDA ofrece un entorno maduro de análisis, scripting y tipos. GoReSym proporciona un importador IDAPython oficial. GoResolver tiene plugin IDA. Es una opción excelente cuando se dispone de licencia y se necesita comparar resultados de un segundo motor.

**Crítica:** no atribuir al producto capacidades concretas de parser Go de una versión sin probarlas; el soporte puede depender del loader, plugins y versiones. El importador GoReSym tiene lógica que altera funciones/tipos del IDB y debe ejecutarse sobre una copia del proyecto.

### 5.4 Binary Ninja

**URL:** https://binary.ninja/

Su IL y API pueden ser útiles para análisis estructural, dataflow y emulación limitada. `ungarble_bn` se debe tratar como plugin experimental hasta verificar alcance. No confundir un script que reconstruye literales en LLIL con un sistema que restaura tipos, funciones y fuente.

### 5.5 Rizin / Cutter

**URLs:** https://rizin.re/ — https://cutter.re/ — https://github.com/rizinorg/rizin

Rizin es un framework CLI/scriptable; Cutter ofrece GUI. Es útil para automatizar búsquedas, inspeccionar instrucciones, generar CFGs y hacer patching manual. No hay que confundir estas capacidades generales con un parser Go completo integrado: la presencia de un comando de análisis o de un plugin de terceros no demuestra soporte para layouts Go 1.27.

**`rz-frida` sí existe** como plugin de integración de Frida con Rizin y Cutter: https://github.com/rizinorg/rz-frida. Es un puente de instrumentación dinámica, no un parser de `pclntab`, `moduledata` o `.go.type`. No asumir que está instalado por defecto ni que todas las funciones descritas en una rama de desarrollo están presentes en el release estable.

No asumir que `rzpipe-rs` es una ventaja automática frente a un proceso CLI/JSON: medir estabilidad, compatibilidad de API y coste de mantenimiento.

### 5.6 Frida

**URL:** https://frida.re/  
**Bindings Rust:** https://github.com/frida/frida-rust

Frida permite observar memoria y ejecución real, pero no «resiste todos los packers» ni sustituye al desempaquetado. Si el programa no llega a ejecutar el código de interés, si detecta instrumentación, si la cadena se reconstruye por un camino no interceptado o si la función objetivo está inlineada, el hook no sirve.

Usos de alto valor:

- capturar strings en el momento de decodificación;
- registrar llamadas a APIs de red/archivos/crypto;
- localizar inicializadores y rutas de ejecución;
- observar dispatch de interfaces cuando se conoce el sitio de llamada;
- comparar comportamiento de un binario original y uno parcheado.

Evitar hooks «universales» en helpers runtime sin controlar arquitectura, versión, calling convention, firma y condiciones de llamada.

### 5.7 Unicorn, Capstone, Qiling, angr, Miasm, Triton y PANDA

| Herramienta | Aporta | No aporta / riesgo |
|---|---|---|
| Unicorn | emulación de CPU para ejecutar rutinas pequeñas de descifrado | no modela por sí sola el SO, runtime Go, syscalls o GC. |
| Capstone | desensamblado multi-arquitectura | no realiza análisis semántico de alto nivel. |
| Qiling | emulación de proceso/OS con loaders y syscalls | configuración y cobertura dependen del target; no es un runtime Go completo. |
| angr | CFG, symbolic execution, dataflow y análisis de binarios | path explosion, CFG incorrectos y modelos incompletos. |
| Miasm | IR, lifting, symbolic execution y análisis | exige conocimiento de arquitectura y modelos; no recupera fuente automáticamente. |
| Triton | dynamic symbolic execution y taint sobre instrucciones | depende de ejecución y de la calidad de instrumentación. |
| PANDA | análisis dinámico basado en QEMU y record/replay | mayor coste operativo; útil para casos complejos, no primer paso. |

Estas herramientas deben activarse cuando una pregunta concreta lo justifique (p. ej. descifrar un decoder, entender una condición anti-debug, reconstruir una transformación), no desplegarse todas por defecto.

### 5.8 Herramientas auxiliares y candidatos de menor prioridad

Estas herramientas aparecían en el inventario técnico o son útiles para completar el ecosistema. Se incluyen para evitar que una lista larga se interprete como una recomendación de instalarlo todo.

**AlphaGolang — scripts IDAPython paso a paso**  
URL: https://github.com/SentineLabs/AlphaGolang  
Colección de scripts para reconstruir `pclntab`, descubrir/renombrar funciones, analizar strings y extraer tipos. Es valiosa didácticamente porque separa tareas en etapas pequeñas en lugar de un script monolítico. El README original apunta a IDA 7.5/7.6+, pero no ofrece por sí solo una matriz moderna equivalente a Go 1.27 V5. Usarlo como material de estudio y comparar sus supuestos con el runtime actual; no tomarlo como parser actualizado sin auditar el código.

**GhidraMCP y GhidrAssistMCP — control de Ghidra por agentes**  
URLs: https://github.com/bethington/ghidra-mcp y https://github.com/symgraph/GhidrAssistMCP  
Proporcionan una interfaz MCP para consultar/operar Ghidra desde herramientas externas. No añaden un parser Go nuevo: el conocimiento de Go sigue procediendo de Ghidra, extensiones, scripts o parsers importados. Su valor está en automatizar tareas repetitivas y consultar el programa. Riesgo importante: algunos endpoints pueden modificar símbolos, tipos, funciones o proyectos. Usar snapshots del proyecto, allowlist de herramientas de escritura y una capa de aprobación para cambios. No permitir que un LLM renombre cientos de funciones sin provenance ni diff.

**YARA-X — fingerprints y reglas**  
URLs: https://github.com/VirusTotal/yara-x y https://virustotal.github.io/yara-x/  
Motor de reglas/pattern matching en Rust, útil para identificar familias de binarios, firmas de Garble, packers, secciones y patrones runtime conocidos. No interpreta por sí mismo estructuras Go ni reconstruye tipos. Las reglas deben tener tests positivos y negativos y evitar patrones demasiado genéricos. YARA-X declara alta compatibilidad con reglas YARA clásicas, pero conviene validar las reglas utilizadas en ambos motores.

**RetDec — decompilador general basado en LLVM**  
URL: https://github.com/avast/retdec  
Puede servir como segundo decompilador/contraste de arquitectura y formato, pero no es una solución especializada para recuperar `pclntab`, `moduledata`, tipos o nombres Garble. El README oficial indica que está en mantenimiento limitado por falta de recursos. No lo pondría en el camino crítico frente a Ghidra/IDA/Binary Ninja para este proyecto; sí puede aportar un contraste ocasional si la arquitectura/ISA está soportada.

**LLVM MC — ensamblador/decoder de bajo nivel**  
URLs: https://llvm.org/docs/CommandGuide/llvm-mc.html y https://llvm.org/docs/CodeGenerator.html  
La capa MC representa instrucciones y secciones a nivel de máquina. `llvm-mc --disassemble` puede servir como oráculo adicional para bytes de instrucciones y codificación. No es un decompilador ni recupera metadata Go. Es útil para validar instrucciones raras o contrastar un decoder propio, no para sustituir Ghidra.

**Yaxpeax — decoders Rust**  
URLs: https://docs.rs/yaxpeax-arch/latest/yaxpeax_arch/ , https://docs.rs/yaxpeax-x86/latest/yaxpeax_x86/ , https://docs.rs/yaxpeax-arm/latest/yaxpeax_arm/  
`yaxpeax-x86` es un decoder Rust interesante y rápido; el proyecto publica fuzzing/differential-testing y reconoce el uso de `unsafe` por rendimiento. `yaxpeax-arm` incluye ARMv7/Thumb y AArch64, pero la matriz de `yaxpeax-arch` señala limitaciones: NEON/SVE y algunas partes no están completas. Por ello, no elegirlo como único decoder multiplataforma para un sistema que deba analizar Go ARM64 moderno. Compararlo con Capstone e iced-x86.

**`rz-frida` y `frida-rs`**  
`rz-frida` se integra con Rizin/Cutter; `frida-rs` es un binding Rust de Frida. Ninguno sustituye el modelo de memoria, las firmas ABI ni la lógica de hooks específicos de Go. El plugin puede ser útil para la UI y sesiones, mientras que `frida-rs` puede facilitar automatización en Rust; elegir uno u otro según el lenguaje del orquestador y medir el coste de integración.

### 5.9 Delve y eBPF profilers

Delve es un debugger Go y ha tenido que adaptar el hallazgo de `moduledata` para cambios recientes. Su changelog 1.26.1 menciona un fix para localizar `moduledata` en Go 1.26+ y ajustes para Go 1.27. Esto lo convierte en una fuente útil de compatibilidad práctica, aunque no sustituye un parser forense offline.

- https://github.com/go-delve/delve
- https://github.com/go-delve/delve/blob/master/CHANGELOG.md
- https://github.com/go-delve/delve/tree/master/pkg/proc

OpenTelemetry eBPF Profiler documenta `gopclntab` con detalle y muestra cómo se usa para simbolizar ejecutables stripped en producción. Es excelente referencia para funciones, archivos y líneas, pero el propio documento se centra en ELF Linux y señala que el runtime/linker son las implementaciones de referencia para inline metadata.

- https://github.com/open-telemetry/opentelemetry-ebpf-profiler
- https://github.com/open-telemetry/opentelemetry-ebpf-profiler/blob/main/doc/gopclntab.md

Pyroscope y Parca pueden ayudar a comprender symbolization y perfiles de Go, no son herramientas de decompilación:

- https://github.com/grafana/pyroscope
- https://github.com/parca-dev/parca

---

## 6. Auditoría detallada del parser/patcher Rust original

El código original es un prototipo didáctico, no una herramienta segura para modificar binarios. Debe reescribirse antes de usarlo con muestras reales.

### 6.1 Problemas de parsing

1. **Validación de longitud insuficiente.** El bucle comprueba `offset + 64 <= data.len()` pero el camino de 64 bits lee hasta `offset + 72`. Un archivo truncado puede causar panic/out-of-bounds.
2. **Versiones antiguas tratadas como layout moderno.** Reconoce magic Go 1.2 y 1.16, pero aplica una interpretación de cabecera moderna. Cada familia necesita un parser separado.
3. **Solo little-endian.** `LittleEndian` está fijado; el magic debe usarse para determinar byte order y las lecturas deben respetarlo.
4. **No valida `nfiles` ni todos los offsets.** Solo comprueba `funcname_off < cu_off < filetab_off`; no valida `pctab_off`, `pcln_off`, monotonicidad completa, límites del segmento ni solapamientos.
5. **No valida overflow.** Conversiones `u64 -> usize`, sumas de offsets y `nfunc * stride` deben comprobar overflow y límites.
6. **No acota asignaciones/iteraciones con un presupuesto de recursos.** Un `nfunc` manipulado puede provocar coste desproporcionado.
7. **No valida el terminador/sentinel de la tabla.** El parser debe verificar que la tabla de funciones incluye su última entrada y que los offsets de `_func` están dentro del área correspondiente.
8. **No valida `funcoff` hacia `_func`.** Antes de leer `nameOff`, hay que validar el offset, longitud mínima del registro y sus arrays variables.
9. **No valida `nameOff`.** Un offset fuera de `funcnametab` o una cadena sin NUL dentro del límite debe producir error estructurado.
10. **No tiene modelo de VA/offset de archivo.** El escaneo por bytes y la suma de offsets asumen contigüidad; esto falla con segmentos/secciones divididos o layouts especiales.
11. **No resuelve Go 1.27 `textStart`.** El slot reservado no debe usarse como base de código.
12. **No maneja varias secciones de texto.** `textsectmap` puede requerir traducción por sección.
13. **El filtro `nfunc > 10` descarta ejecutables Go pequeños legítimos.** No es un criterio de validez.
14. **No valida de forma robusta `minLC`, `ptrSize`, magic y endianness como conjunto.** Una coincidencia accidental puede ser aceptada.
15. **Escaneo lineal de todo el archivo sin candidatos priorizados.** Puede ser costoso y produce falsos positivos; empezar por secciones conocidas y luego usar búsqueda estructural global.

### 6.2 Problemas de patching

1. **Búsqueda de subcadena en toda `funcnametab`.** No asocia la cadena a un `nameOff` concreto de `_func`; puede tocar coincidencias parciales, sufijos, nombres repetidos o strings que no representan el símbolo que el usuario quería.
2. **No valida que la coincidencia sea un inicio de nombre.** Un target puede coincidir en mitad de `package.function`.
3. **No ofrece selección por VA/símbolo/función.** El usuario no puede exigir que se parcheen solo funciones concretas.
4. **No rechaza target vacío.** Si el bucle avanza por `target.len()`, puede no avanzar nunca.
5. **Riesgo de underflow/overflow en los límites de búsqueda.** Deben usarse operaciones checked y slices ya validados.
6. **No rechaza NUL en replacement.** Un NUL embebido truncaría el nombre observado.
7. **No comprueba codificación/representación.** Los bytes UTF-8 deben tratarse como bytes; no se debe asumir longitud de caracteres igual a bytes.
8. **No crea backup ni salida separada.** Escribe sobre el archivo de entrada.
9. **No usa transacción.** Un error después de varias escrituras deja el archivo parcialmente parcheado.
10. **No valida la salida.** No vuelve a parsear, comparar hashes, verificar secciones ni ejecutar tests después de escribir.
11. **No conserva permisos/metadata/firmas del contenedor de forma explícita.** En PE, Authenticode puede invalidarse; en Mach-O, firmas/codesign y load commands requieren tratamiento específico.
12. **No separa patch de metadata de un patch de código.** Deben ser subsistemas distintos con políticas de seguridad distintas.

### 6.3 El `dry-run` no es funcional como está escrito

El código abre el fichero con `write(false)` en dry-run y después crea `MmapMut::map_mut`. Un mapping mutable requiere permisos de escritura compatibles; el modo dry-run debería usar `Mmap` de solo lectura o cargar bytes en memoria sin escribir. No basta con no ejecutar el bloque `copy_from_slice`.

### 6.4 La idea de null-padding es válida, pero la implementación no

El principio de reemplazar un nombre por otro de igual o menor longitud de bytes puede conservar offsets si se escribe sobre el slot exacto y se termina el nuevo string con NUL. Pero el método correcto es:

1. Resolver `functab[i].funcoff` a `_func`.
2. Leer `nameOff` del campo correcto de `_func`.
3. Resolver `funcnametab + nameOff`.
4. Leer el string dentro de límites.
5. Verificar que la cadena esperada coincide exactamente con el nombre completo o con una política explícita de reemplazo.
6. Comprobar longitud en bytes, NUL, caracteres válidos para el consumidor y capacidad del slot.
7. Crear patch plan inmutable con bytes originales/nuevos, offsets, VA, función y razón.
8. Aplicar sobre copia temporal, hacer flush/fsync y renombrado atómico.
9. Volver a parsear con dos parsers y comparar el diff.
10. Ejecutar pruebas runtime si el objetivo es que el binario siga ejecutándose.

**Recomendación preferente:** no reescribir `funcnametab` salvo que haya una razón real. Para análisis, aplicar nombres a la base de datos del descompilador. Para exponer símbolos a herramientas ELF/PE, considerar una tabla de símbolos sintética en una copia, por ejemplo con `unstrip`, sabiendo que no es lo mismo que cambiar la metadata interna de Go.

---

## 7. Auditoría del script Ghidra original

El script del documento anterior tiene errores estructurales que invalidan el renombrado.

### 7.1 `functab` mal interpretada

El script hace:

```python
entry_off = memory.getLong(functab_addr.add(i * 16))
name_off  = memory.getLong(functab_addr.add(i * 16 + 8))
```

Para Go 1.18+, los campos son `uint32`, y cada registro mide 8 bytes. Además, el segundo campo es `funcoff`, no `nameOff`.

### 7.2 No sigue el puntero lógico hacia `_func`

El flujo correcto es:

```text
functab[i].funcoff -> _func -> _func.nameOff -> funcnametab[nameOff]
```

No existe un `nameOff` directamente en el segundo campo de `functab`.

### 7.3 Base de texto errónea para Go 1.27

El script lee `header_addr + 24` como `textStart`. Para Go 1.27, ese slot está reservado y el linker escribe cero. Hay que resolver `moduledata.text`/`textsectmap` o dejar la VA sin resolver.

### 7.4 Búsqueda y validación insuficientes

- Solo busca magic 1.18/1.20 en little-endian.
- Devuelve la primera coincidencia sin validar estructura completa.
- No comprueba bloques de memoria ni límites antes de leer strings.
- No trata el sentinel ni el rango final de funciones.
- No controla colisiones de nombres ni errores de `createFunction`.
- Usa `USER_DEFINED`, mezclando metadata importada con nombres puestos manualmente por el analista.
- No conserva procedencia ni evidencia.
- Limita a 5000 funciones sin explicar por qué; puede dejar fuera la mayoría de un ejecutable real.

### 7.5 Arquitectura de importador recomendada

El importador no debe volver a parsear a ciegas el binario si ya existe un JSON estructurado y validado. Debe consumir un schema versionado con:

- `image_id` y hash del fichero;
- formato, arquitectura, endianness y mapping de segmentos;
- Go version estimada y evidencia;
- función: VA/offset, rango, nombre observado, archivo/línea, parser de origen;
- tipos y relaciones con confianza;
- inline frames;
- warnings/unsupported features;
- estado de colisión y política de nombre.

El importador debe aplicar etiquetas con `SourceType.ANALYSIS` o categoría equivalente y añadir comentarios de procedencia. Un nombre inferido por CFG o LLM nunca debe sobrescribir un nombre observado directamente sin una acción explícita.

---

## 8. Recuperación semántica: de metadata a comprensión del programa

### 8.1 Niveles de recuperación

| Nivel | Resultado | Fuente típica | Qué no implica |
|---|---|---|---|
| L0 | Container/arquitectura/segmentos | ELF/PE/Mach-O/Wasm parser | No prueba que sea Go. |
| L1 | Go fingerprint, versión aproximada | buildinfo, magic, runtime strings | No identifica la versión exacta en todos los casos. |
| L2 | Funciones, archivos, líneas | pclntab | No recupera variables locales completas ni fuente. |
| L3 | Tipos y métodos retenidos | moduledata/.go.type/DWARF | No implica que todos los tipos sobrevivan al linker. |
| L4 | Interfaces/dispatch | itabs, call sites, type descriptors | No recupera toda la semántica de interfaz. |
| L5 | CFG, xrefs, inline frames | disassembler, pclntab, funcdata | No prueba equivalencia funcional. |
| L6 | comportamiento observado | Frida, Delve, eBPF, emulación | Solo cubre caminos ejecutados. |
| L7 | semántica de negocio | análisis humano + LLM + evidencia | Sigue siendo una reconstrucción, no el fuente original. |

### 8.2 Tipos: lo recuperable y lo que no

Los descriptores runtime pueden proporcionar nombres, tamaños, alineación, kind, flags, campos de structs, métodos y referencias a tipos relacionados. En Go 1.27, el layout y recorrido de los tipos cambia, por lo que la extracción debe seguir la versión real del runtime/linker.

No prometer:

- todos los tipos declarados en el source;
- variables locales y parámetros exactos de cada función;
- firmas de todas las funciones libres top-level;
- comentarios, nombres de variables locales o estructura de paquetes originales cuando se ofuscaron;
- types que el linker eliminó por dead-code elimination.

`unstrip` documenta que recupera firmas de métodos cuando se conservó el registro de tipo, pero no las firmas de funciones libres top-level en el caso general sin DWARF. Esa limitación debe aparecer en cualquier informe.

### 8.3 Interfaces e itabs

Los itabs conectan un tipo concreto con una interfaz y contienen entradas de métodos. Son valiosos para:

- identificar implementaciones concretas de interfaces;
- resolver dispatch indirecto;
- reconstruir familias de tipos;
- priorizar funciones con semántica conocida (`io.Reader`, `http.Handler`, `crypto.Signer`, etc.).

Pero hay que tener en cuenta:

- Go 1.27 reorganiza el almacenamiento;
- el linker puede eliminar tipos/implementaciones no alcanzables;
- un método wrapper puede ser distinto del cuerpo lógico;
- las interfaces vacías (`any`) y las no vacías tienen representaciones diferentes;
- el dispatch observado en un binario no representa todas las implementaciones del source.

### 8.4 Generics y nombres de instanciación

Los nombres de símbolos pueden contener información de shape/dictionaries y el compilador puede compartir código entre instanciaciones. No asumir que cada instanciación genérica tiene una copia de código o un tipo runtime independiente. La normalización de nombres debe preservar el símbolo original y ofrecer una vista normalizada aparte.

### 8.5 Inlining y optimización

Un decompilador puede mostrar un bloque de código dentro de una función física que proviene de una función fuente inlineada. El `pclntab` y `FUNCDATA_InlTree` pueden recuperar parte de esa relación. Para analizar una función:

1. resolver PC físico;
2. resolver `_func` y sus pcdata;
3. comprobar `PCDATA_InlTreeIndex`;
4. resolver `FUNCDATA_InlTree`;
5. reconstruir frames lógicos;
6. preservar que el código es compartido/inlineado y no una función física independiente.

No crear funciones independientes para cada inline frame salvo que el descompilador lo requiera como anotación virtual.

### 8.6 CFG y recuperación de nombres

GoResolver debe usarse como generador de candidatos. Mejorar la señal con:

- versión/arquitectura exacta de compilación;
- tamaño de función y distribución de bloques;
- dominadores, loops y branch topology;
- llamadas directas e indirectas;
- constantes y strings asociadas;
- tipos de entrada/salida inferidos;
- referencias a APIs stdlib;
- firmas de métodos e itabs;
- patrones de error handling, defer y panic;
- datos de ejecución cuando existan.

Cada resultado debería ser una tupla:

```json
{
  "function_va": "0x...",
  "observed_name": "pkg.a1b2c3",
  "candidate_name": "net/http.(*Client).Do",
  "method": "cfg_similarity",
  "score_raw": 0.93,
  "reference_build": "go1.26.2/linux/amd64/-trimpath=...",
  "supporting_evidence": ["call_edges", "string_xrefs", "type_context"],
  "contradictions": [],
  "status": "candidate_not_confirmed"
}
```

El sistema no debe convertir automáticamente `score_raw >= 0.9` en «confirmado».

---

## 9. Strings, Garble, embed y ejecución dinámica

### 9.1 Clasificar antes de descifrar

No toda cadena ausente está ofuscada. Clasificar:

1. literal plaintext en `.rodata`;
2. string formada por concatenación/formatting;
3. stack string;
4. bytes decodificados desde un array;
5. literal Garble `-literals`;
6. contenido `//go:embed`;
7. string cifrada por la aplicación;
8. dato comprimido/serializado;
9. string recibida desde red/archivo/IPC.

### 9.2 Garble `-literals`

El README actual de Garble explica que sustituye expresiones literales por expresiones más complejas que producen el mismo valor en runtime. Algunas constantes no pueden transformarse porque se resuelven en compile time. `//go:embed` se trata también; los paths de embed siguen visibles y el ejecutable contiene el decoder/key. Es obfuscation, no cifrado robusto.

Fuentes:

- https://github.com/burrowers/garble
- https://github.com/burrowers/garble/blob/master/README.md

### 9.3 Estrategia de recuperación

Orden recomendado:

1. Extraer strings estáticas.
2. Enumerar funciones con referencias a memoria/decoder sospechoso.
3. Identificar versión de Garble y Go.
4. Ejecutar GoStringUngarbler o disrobe solo si su matriz coincide.
5. Emular rutinas pequeñas con Unicorn en un entorno aislado.
6. Comparar salida con ejecución real controlada.
7. Usar Frida para capturar buffers solo cuando la ruta se ejecute.
8. Volcar `embed.FS` o datos decodificados con límites de tamaño y hash.
9. Guardar cada literal con VA, función, callsite, longitud, codificación y confianza.

### 9.4 Hook correcto a `runtime.slicebytetostring`

La firma actual es `slicebytetostring(buf *tmpBuf, ptr *byte, n int) string`. En amd64 ABIInternal, los argumentos enteros se asignan a RAX/RBX/RCX en ese orden. El hook debe:

- validar que el módulo y la versión coinciden;
- resolver la VA desde metadata y confirmar bytes de función;
- capturar `RBX` como puntero y `RCX` como longitud en amd64;
- validar `n` con límite razonable y comprobar memoria legible;
- no asumir que todo buffer es literal ofuscado;
- guardar caller PC y stack/contexto;
- manejar arm64 por separado;
- tolerar que la función no se invoque o que el compilador use otro camino.

Fuente:

- https://go.dev/src/runtime/string.go
- https://go.dev/src/cmd/compile/abi-internal

### 9.5 Goroutines, panic y runtime hooks

`runtime.newproc`, `runtime.gopanic`, `runtime.deferproc` y otros helpers pueden ofrecer puntos de observación, pero sus firmas y layouts cambian. Un hook debe basarse en el binario exacto, no en un offset estático copiado de otro Go. El número de goroutines observado es dependiente de inputs y timing; no equivale al grafo estático completo.

### 9.6 Entorno seguro

- VM desechable sin credenciales personales.
- Red desactivada o simulada salvo que la prueba la requiera.
- Snapshot previo y posterior.
- Límites CPU/RAM/tiempo/disco.
- Captura de syscalls y cambios de archivos.
- No ejecutar muestras desconocidas en el host de trabajo.
- Separar «análisis» de «reproducción de comportamiento malicioso».

---

## 10. Arquitectura definitiva del sistema propio

La propuesta es una plataforma por capas, no un monolito.

### 10.1 Pipeline de alto nivel

```text
             INPUT BINARIO (SHA-256 + evidencia)
                          |
                          v
       [1] Container adapters: ELF / PE / Mach-O / Wasm
                          |
                          v
       [2] Go fingerprint + buildinfo + version hypotheses
                          |
                          v
       [3] Parsers independientes y versionados
           | GoReSym | GoRE | gobin | unstrip | own parser
                          |
                          v
       [4] Normalized evidence graph (JSON/SQLite)
           functions / types / itabs / files / inline / strings
                          |
             +------------+-------------+
             |                          |
             v                          v
       [5] Disassembly/CFG          [6] Dynamic evidence
       Ghidra/IDA/BN/Rizin          Delve/Frida/eBPF/Unicorn
             |                          |
             +------------+-------------+
                          |
                          v
       [7] Candidate resolution + confidence/provenance
       CFG corpus / xrefs / type context / string evidence / LLM
                          |
                          v
       [8] Analyst review + symbol/type importers
                          |
                          v
       [9] Optional rewrite/patch in separate copy
                          |
                          v
       [10] Reparse + differential tests + runtime validation
```

### 10.2 Capas y responsabilidades

**A. Container layer**

- parsear headers y segmentos;
- mapear file offset ↔ RVA/VA ↔ memoria cargada;
- soportar PIE/ASLR y relocations;
- detectar solapamientos y regiones sin bytes de archivo;
- soportar Mach-O fat/universal y PE overlays;
- Wasm: memoria lineal, segmentos de datos y direcciones virtuales Go.

**B. Go fingerprint layer**

- build ID;
- build info;
- pclntab magic y campos;
- arquitectura y endianness;
- runtime strings;
- firmas de compiler/linker;
- nivel de confianza de versión.

**C. Versioned metadata parsers**

- `pclntab` layouts pre-1.2, 1.2, 1.16, 1.18, 1.20+;
- `moduledata` layouts por release y architecture;
- tipos legacy y Go 1.27 V5;
- itabs legacy y Go 1.27 V5;
- buildinfo legacy/inline;
- `funcdata`, `pcdata`, inline trees;
- `textsectmap` y Wasm.

**D. Evidence graph**

No almacenar solo nombres en una tabla. Usar nodos y relaciones:

- Binary, Segment, Section, Function, InlineFrame, Type, Method, Interface, Itab, String, SourceFile, BuildSetting, ReferenceBuild, CandidateName, Patch.
- Edge types: `contains`, `points_to`, `implements`, `calls`, `references_string`, `inline_of`, `resolved_by`, `derived_from`, `contradicts`.

**E. RE adapters**

- Ghidra: funciones, símbolos, comentarios, data types y bookmarks.
- IDA: functions, names, types, comments y structures.
- Binary Ninja: symbols, types, tags y IL metadata.
- Rizin: flags, functions, comments y JSON.

**F. Resolver**

- exact metadata first;
- version-aware runtime/std library map;
- CFG similarity second;
- type/callsite context third;
- dynamic observations fourth;
- LLM candidate explanation last.

**G. Patcher**

- metadata patching;
- synthetic symbol-table writing;
- code patching;
- resource/string rewriting;
- signature regeneration/invalidations.

Cada una es una operación distinta con dry-run real, patch plan y rollback.

### 10.3 Modelo de datos mínimo

Ejemplo de esquema lógico (no schema final de serialización):

```json
{
  "schema_version": "1.1",
  "binary": {
    "sha256": "...",
    "format": "ELF",
    "arch": "amd64",
    "endianness": "little",
    "go_version": {"value": "go1.27.x", "confidence": "high", "evidence": ["buildinfo"]}
  },
  "parser_runs": [
    {"tool": "gobin", "version": "0.4.1", "status": "success", "warnings": []},
    {"tool": "GoReSym", "version": "3.1.1", "status": "partial", "warnings": ["type layout V5 not validated"]}
  ],
  "modules": [
    {
      "module_index": 0,
      "module_identity": "main-image",
      "pclntab_file_offset": "0x123400",
      "text_base_va": null,
      "text_base_status": "unresolved; offsets only",
      "text_sections": [],
      "functions": [
        {
          "start_va": null,
          "end_va": null,
          "text_section_index": null,
          "entryoff": 1234,
          "funcoff": 456,
          "observed_name": "main.a1b2",
          "name_status": "observed",
          "evidence": [{"source": "pclntab", "parser": "gobin", "confidence": "high"}]
        }
      ]
    }
  ]
}
```

Reglas:

- Las direcciones se serializan como strings hex para no perder precisión en consumidores JSON/JavaScript.
- Cada campo derivado incluye fuente y parser.
- Un conflicto no se resuelve silenciosamente: se registra.
- El nombre observado no se sobrescribe por una inferencia.
- Los offsets y VAs no comparten el mismo tipo interno.

### 10.4 Arquitectura Rust recomendada

Crates por función:

| Crate | Papel |
|---|---|
| `object` 0.41 | lectura de ELF/PE/Mach-O/Wasm, secciones y símbolos; primera opción si cubre las necesidades. Su release actual requiere Rust 1.85 para la feature `read`. |
| `goblin` | alternativa para parsing de formatos, usada por gobin; comparar ergonomía y cobertura. |
| `gimli` | DWARF parser. |
| `addr2line` | resolución de direcciones a fuente usando DWARF. |
| `iced-x86` | decoder/encoder x86/x64 de alto nivel. |
| `yaxpeax-*` | parsers/decoders alternativos por arquitectura. |
| `capstone` / `capstone-rs` | disassembly multi-ISA. |
| `pelite` | PE-specific parsing. |
| `pdb` | PDB parsing para Windows cuando exista. |
| `memmap2` | memory mapping; usar inmutable en parsing y mutable solo en writer controlado. |
| `nom` / `binrw` | parser combinator o lectura declarativa; escoger uno por módulo, no ambos indiscriminadamente. |
| `serde` | serialización de resultados y schema versionado. |
| `thiserror` / `anyhow` | errores tipados en librería y contexto en CLI. |
| `proptest` / `cargo-fuzz` | property tests y fuzzing estructurado. |
| `criterion` | benchmarks reproducibles. |

Fuentes:

- https://crates.io/crates/object
- https://docs.rs/object/0.41.0/object/
- https://docs.rs/object/latest/object/
- https://crates.io/crates/goblin
- https://docs.rs/goblin/latest/goblin/
- https://crates.io/crates/gimli
- https://docs.rs/gimli/latest/gimli/
- https://crates.io/crates/addr2line
- https://docs.rs/addr2line/latest/addr2line/
- https://crates.io/crates/iced-x86
- https://docs.rs/iced-x86/latest/iced_x86/
- https://crates.io/crates/capstone
- https://docs.rs/capstone/latest/capstone/
- https://crates.io/crates/pelite
- https://docs.rs/pelite/latest/pelite/
- https://crates.io/crates/pdb
- https://crates.io/crates/memmap2
- https://crates.io/crates/nom
- https://crates.io/crates/binrw
- https://github.com/rust-fuzz/cargo-fuzz
- https://github.com/proptest-rs/proptest
- https://bheisler.github.io/criterion.rs/book/

**No mezclar responsabilidades:** `object`/`goblin` son contenedores; `gimli` es DWARF; `iced-x86`/Capstone son decoders; el parser Go debe ser propio o reutilizar una librería especializada. Una dependencia no hace el trabajo de la otra.

### 10.5 ¿Construir parser propio o integrar `gobin`?

Recomendación: ambas cosas, pero con alcance limitado.

1. Usar `gobin` como baseline para Go 1.27 V5.
2. Usar GoRE/GoReSym como parsers de contraste.
3. Implementar un núcleo propio pequeño para los formatos y estructuras que realmente se necesiten.
4. No copiar masivamente `runtime` sin automatizar la extracción de snapshots versionados.
5. Generar estructuras por release desde fuentes Go taggeadas o mantener fixtures/offset tables explícitas y revisadas.
6. Comparar con el propio runtime/linker cuando una discrepancia aparezca.

---

## 11. Patching, rebranding y escritura de símbolos

### 11.1 Separar objetivos

«Rebranding» puede significar operaciones incompatibles entre sí:

- cambiar nombres en la base de datos del descompilador;
- agregar símbolos de análisis a una copia del ejecutable;
- cambiar cadenas visibles de producto;
- modificar buildinfo;
- cambiar nombres de funciones en `funcnametab`;
- modificar recursos PE/Info.plist/metadata de paquete;
- alterar comportamiento/código.

Cada objetivo necesita un patcher distinto. No debe existir un botón «rebrand» que haga todas las operaciones sin explicar qué cambia.

### 11.2 Preferir símbolos sintéticos antes que alterar `pclntab`

Para ELF/PE, una tabla de símbolos sintética puede hacer que herramientas estándar vean nombres sin modificar la metadata que usa el runtime. `unstrip` documenta escritura de `.symtab`/`.strtab` ELF y COFF en PE. Esto es generalmente más reversible que cambiar strings internas.

Limitaciones:

- la tabla añadida no recupera tipos ni firmas;
- el formato PE COFF symbol table tiene restricciones y no es equivalente a PDB;
- Mach-O requiere otra implementación y firma/loader considerations;
- las firmas digitales pueden invalidarse;
- algunos loaders/tools pueden ignorar o rechazar metadata añadida.

Fuentes:

- https://docs.rs/unstrip/latest/unstrip/rewrite/
- https://github.com/riven-labs/unstrip

### 11.3 Escritura transaccional

Un patch plan debe registrar:

- SHA-256 del input;
- versión del parser;
- offset de archivo, VA, segmento y sección;
- bytes originales y bytes nuevos;
- longitud y motivo;
- restricciones de firma/loader;
- verificación posterior esperada.

Aplicación:

1. Crear copia temporal con permisos conservados.
2. Aplicar todos los cambios en memoria o a un archivo temporal.
3. Verificar que el input no ha cambiado desde el análisis.
4. Volver a parsear el output.
5. Ejecutar validaciones del contenedor.
6. Comparar hash de regiones no modificadas.
7. Si es ejecutable, probar arranque/funciones relevantes en sandbox.
8. `fsync` y renombrado atómico.
9. Mantener backup y log de rollback.

### 11.4 No parchear nombres de `funcnametab` con búsqueda ciega

Solo permitir escritura por identidad de función y `nameOff` resuelto. La política de patch debe permitir:

- exact name replacement;
- same-length replacement;
- shorter replacement with NUL termination;
- no growth unless se reconstruyen regiones y offsets en un writer completo.

No permitir `target=""`, reemplazos con NUL ni offsets que no hayan pasado verificación estructural.

### 11.5 Firmas y formatos

- ELF: section headers, program headers, build ID, notes, alignments, dynamic loader, PIE.
- PE: section table, COFF symbol table, certificate table/AuthentiCode, checksum, overlays, RVA vs file offset.
- Mach-O: load commands, fat slices, code signature, `LC_SYMTAB`, dyld constraints.
- Wasm: section sizes, data segments, memory index/offset, custom sections.

No afirmar que una modificación es inocua porque solo «añade bytes al final». Hay que validar el loader y el formato exacto.

---

## 12. Plan de pruebas que sí permite llamar robusto al sistema

### 12.1 Matriz mínima de toolchains

Generar fixtures controlados para cada combinación disponible:

| Dimensión | Valores mínimos |
|---|---|
| Go version | 1.16, 1.17, 1.18, 1.19, 1.20, 1.21, 1.22, 1.23, 1.24, 1.25, 1.26, 1.27 |
| Arquitectura | amd64, arm64, 386, arm, riscv64, ppc64le, s390x; añadir wasm donde proceda |
| Formato | ELF, PE, Mach-O, Wasm según GOOS/GOARCH soportado |
| Link mode | internal/external, PIE/non-PIE cuando aplique |
| Strip | normal, `-s`, `-w`, `-s -w`, `strip` externo |
| Paths | normal y `-trimpath` |
| CGO | on/off |
| Optimización | default, `-N -l`, optimizaciones normales |
| Garble | none, default, `-literals`, `-tiny`, `-seed`, control-flow experimental |
| Generics/reflection | genéricos, reflect, interfaces, plugins, embed |
| Multi-section text | fixture específico si la plataforma/linker lo genera |

No hace falta compilar todas las combinaciones posibles desde el día uno; sí hay que declarar qué celdas existen y cuáles no.

### 12.2 Programa fixture

El fixture debe incluir:

- funciones libres, métodos value/pointer receivers y closures;
- generics con varias instanciaciones;
- interfaces vacías y no vacías;
- tipos con structs embebidos, tags, arrays, maps, slices, channels;
- `//go:embed` de string, bytes y `embed.FS`;
- panic/defer/recover;
- goroutines y select;
- cgo opcional;
- strings en plaintext, stack strings y conversiones byte→string;
- `runtime.Callers`/stack traces;
- funciones susceptibles de inlining y funciones marcadas `//go:noinline`.

Conservar source, toolchain exacta, flags, build logs y hash del binario.

### 12.3 Oráculos de verdad

No comparar parser A con parser B y declarar que ambos tienen razón. Usar oráculos independientes:

1. metadata del compilador/linker generado durante la compilación del fixture;
2. `go tool nm`, `go tool objdump`, `debug/gosym`, `debug/buildinfo`;
3. runtime `runtime.FuncForPC`, `runtime.CallersFrames`, `reflect` y stack traces en la ejecución de fixture;
4. DWARF cuando no se haya eliminado;
5. salidas de GoReSym, GoRE, unstrip, gobin, disrobe;
6. comparación del disassembly/CFG con fuente conocida.

### 12.4 Métricas

- precisión/recall de funciones;
- exactitud de start/end VA;
- exactitud de `nameOff`, `funcoff` y función asociada;
- exactitud de archivo/línea por PC;
- exactitud de inline frames;
- cobertura de tipos por kind;
- exactitud de offsets/campos de structs;
- exactitud de itab/interface dispatch;
- tasa de parser crash/panic;
- tasa de falsos positivos en magic scans;
- tiempo, RAM y bytes leídos;
- porcentaje de warnings y fallbacks;
- tasa de éxito de round-trip (parse → serialize → parse);
- porcentaje de parches que sobreviven a reparse y runtime tests.

### 12.5 Pruebas negativas y fuzzing

Fuzzear:

- cabeceras truncadas;
- magic con endianness inversa;
- offsets fuera de rango;
- nfunc/nfiles enormes;
- `funcoff` duplicados, desordenados y fuera de rango;
- `_func` truncado y arrays variables corruptos;
- cadenas sin NUL;
- varints que no terminan o exceden bits;
- secciones solapadas;
- segmentos no mapeados;
- punteros que parecen VA pero son file offsets;
- módulo circular/corrupto;
- PE overlays, Mach-O fat slices y Wasm data segments malformados.

El parser debe terminar con error tipado y presupuesto de recursos, no panic, hang o asignación ilimitada.

### 12.6 Diferential testing

- Ejecutar todas las herramientas contra el mismo fixture.
- Normalizar nombres, VAs, tipos y warnings.
- Comparar resultados y generar un diff.
- Clasificar divergencia: parser bug, versión no soportada, metadata no retenida, parser heurístico o interpretación distinta.
- Añadir cada divergencia resuelta como regresión.

---

## 13. Laboratorios prioritarios y orden de implementación

### Laboratorio A — Go 1.27 V5, la prueba más importante

Objetivo: confirmar `.go.type`, desaparición de `typelinks`/`itablinks`, base de texto y parsing de funciones.

Herramientas:

- Go 1.27.x toolchain;
- `gobin 0.4.1`;
- GoRE;
- GoReSym;
- Ghidra GolangAnalyzerExtension v1.4.0;
- Delve actualizado.

Pasos:

1. Compilar fixture con tipos, interfaces, métodos, reflect y generics.
2. Compilar normal y `-s -w`.
3. Extraer buildinfo y `pclntab`.
4. Comparar `entryoff` con `moduledata.text` y `textsectmap`.
5. Verificar que el slot `textStart` del header no se trata como VA.
6. Enumerar tipos e itabs desde `.go.type`.
7. Contrastar con `reflect` y runtime.
8. Registrar falsos positivos y huecos de cada parser.

**Criterio de éxito:** recuperar de forma reproducible funciones y una muestra de tipos/itabs, sin asumir `typelinks` antiguos ni usar una base de texto inventada.

### Laboratorio B — GoReSym vs gobin vs GoRE vs unstrip

Objetivo: crear una matriz por versiones y separar parser de funciones de parser de tipos.

Resultado esperado: tabla por release y arquitectura con columnas `functions`, `files/lines`, `types`, `itabs`, `inline`, `buildinfo`, `wasm`, `warnings`.

### Laboratorio C — Garble literal extraction

Objetivo: no confundir Garble default, `-literals` y `-tiny`.

1. Construir misma aplicación con y sin Garble.
2. Construir Garble `-literals` y `-tiny` por separado.
3. Ejecutar GoStringUngarbler solo con sus versiones compatibles.
4. Ejecutar disrobe en versiones declaradas.
5. Comparar literales con fuente y memoria runtime.
6. Cambiar seed y toolchain para comprobar determinismo y límites.

### Laboratorio D — CFG resolver con corpus controlado

Objetivo: medir GoResolver en lugar de aceptar 0.9 como una cifra mágica.

1. Compilar varias dependencias stdlib y de terceros.
2. Cambiar versión, arquitectura, optimización y trimpath.
3. Obfuscar nombres con Garble.
4. Evaluar precision@1, precision@5, recall y calibración.
5. Analizar colisiones y falsos positivos.

### Laboratorio E — Patcher y symbol writer

Objetivo: comparar modificación de `funcnametab` con adición de símbolos sintéticos.

- Copia original inmutable.
- `unstrip --symbols-as` únicamente sobre un fixture Go 1.18–1.25 (rango declarado por el proyecto; nunca sobre el fixture Go 1.27 del Laboratorio A para evaluar soporte).
- patch de nombre por `nameOff` exacto en una copia experimental.
- reparse por dos herramientas.
- ejecución en sandbox.
- comprobar integridad de regiones no modificadas.

---

## 14. Roadmap priorizado

### P0 — Integridad del análisis

- Retirar el parser/patcher anterior de cualquier uso sobre muestras importantes.
- Implementar container mapping robusto.
- Implementar parser `pclntab` versionado con pruebas.
- Añadir Go 1.27 `textStart` reservado y `.go.type` V5.
- Integrar GoReSym, GoRE y gobin como parsers de contraste.
- Implementar límites, errores tipados y fuzzing.

### P1 — Calidad de recuperación

- `moduledata` y tipos por release.
- itabs y dispatch.
- buildinfo y build ID.
- archivos/líneas/inline trees.
- importador Ghidra con provenance.
- corpus de referencia para GoResolver.

### P2 — Desofuscación y dinámica

- clasificación de Garble.
- extractor de literales por versión.
- Frida hooks específicos por arquitectura.
- eBPF/Delve para symbolization y ejecución.
- análisis de `embed.FS`.

### P3 — Reescritura y producto final

- synthetic symbol tables ELF/PE.
- patch plan + rollback.
- UI de conflictos y confianza.
- export/import Ghidra/IDA/BN/Rizin.
- benchmarks y documentación de compatibilidad.

### P4 — Investigación avanzada

- CFG embeddings y matching multi-feature.
- recuperación de firmas de funciones top-level.
- análisis de stack maps y variables.
- control-flow deobfuscation experimental.
- reconstrucción de semántica con LLM y evidence graph.

---

## 15. Uso de LLM: hipótesis con trazabilidad, no autocompletado de nombres

Un LLM puede acelerar la interpretación de funciones, pero su tendencia a inventar semántica es un riesgo. El modelo debe recibir una función con contexto limitado y verificable:

- pseudocódigo y disassembly;
- llamadas directas e indirectas;
- tipos observados e inferidos;
- strings y referencias;
- caller/callee context;
- comportamiento dinámico observado;
- arquitectura/ABI y Go version;
- candidatos de CFG con score y referencia.

Debe devolver:

- resumen funcional;
- nombre sugerido;
- evidencia concreta;
- hipótesis alternativas;
- contradicciones;
- nivel de confianza explicado;
- preguntas de validación.

No debe devolver solo `name: decryptConfig` sin demostrarlo.

### Política de aceptación de nombres

| Estado | Requisito |
|---|---|
| `observed` | nombre extraído directamente de metadata o símbolo. |
| `matched` | identidad respaldada por referencia exacta/versionada o hash de función fiable. |
| `strong_candidate` | varias evidencias independientes y sin contradicciones relevantes. |
| `candidate` | CFG o LLM con soporte parcial. |
| `unknown` | evidencia insuficiente. |
| `conflict` | fuentes contradictorias. |

Solo `observed` y `matched` deben aplicarse automáticamente. `strong_candidate` puede sugerirse al analista. `candidate` permanece como comentario.

---

## 16. Lista de comprobación operativa

### Antes del análisis

- [ ] Calcular SHA-256 y guardar tamaño/fecha.
- [ ] Guardar original en almacenamiento read-only.
- [ ] Identificar formato, arquitectura y endianness.
- [ ] Detectar packing/overlay/section anomalies.
- [ ] Extraer buildinfo/build ID.
- [ ] Estimar Go version con evidencia múltiple.

### Durante el análisis

- [ ] Ejecutar GoReSym.
- [ ] Ejecutar gobin cuando el rango incluya 1.27 o Wasm.
- [ ] Ejecutar GoRE/Redress si aporta contraste.
- [ ] Ejecutar unstrip solo dentro del rango declarado.
- [ ] Guardar outputs originales sin normalización destructiva.
- [ ] Resolver funciones y tipos con provenance.
- [ ] Marcar unsupported/partial.
- [ ] Importar en Ghidra/IDA/BN/Rizin sobre una copia del proyecto.

### Antes de aceptar una hipótesis

- [ ] ¿Hay metadata directa?
- [ ] ¿El CFG depende de una versión/arquitectura diferente?
- [ ] ¿Hay evidencia de tipo/itab?
- [ ] ¿Los strings apuntan al mismo propósito?
- [ ] ¿El caller/callee context concuerda?
- [ ] ¿Hay contradicciones?
- [ ] ¿La función está inlineada o es wrapper?

### Antes de parchear

- [ ] ¿Es necesario cambiar el binario o basta con la base de datos RE?
- [ ] ¿Hay backup y hash original?
- [ ] ¿Existe patch plan con bytes originales/nuevos?
- [ ] ¿La dirección está resuelta con certeza?
- [ ] ¿Se preservan límites y alineación?
- [ ] ¿Se invalida Authenticode/codesign?
- [ ] ¿Se puede revertir?
- [ ] ¿Se ha reparseado y ejecutado en sandbox?

---

## 17. Referencias primarias y repositorios (URLs en texto plano)

### Go runtime, linker, ABI y formatos

- https://go.dev/src/runtime/symtab.go
- https://github.com/golang/go/blob/master/src/runtime/symtab.go
- https://go.dev/src/debug/gosym/pclntab.go
- https://github.com/golang/go/blob/master/src/debug/gosym/pclntab.go
- https://go.dev/src/cmd/link/internal/ld/pcln.go
- https://go.dev/src/cmd/link/internal/ld/symtab.go
- https://go.dev/src/internal/abi/symtab.go
- https://go.dev/src/cmd/compile/abi-internal
- https://go.dev/src/runtime/string.go
- https://pkg.go.dev/debug/gosym
- https://pkg.go.dev/debug/buildinfo
- https://go.dev/doc/go1.25
- https://go.googlesource.com/website/%2B/ebe2ee449794903b9f2d213bb605bbe6b03bd1eb/_content/doc/go1.27.md
- https://go.googlesource.com/go/%2B/481ab86aafe0cac177df793c9946c5ef2126137c
- https://www.golang.design/history/runtime-compiler/go1.27.html
- https://www.golang.design/history/runtime-compiler/linker.html
- https://www.golang.design/history/runtime-compiler/data-structures.html

### Documentación técnica de `pclntab`

- https://github.com/open-telemetry/opentelemetry-ebpf-profiler/blob/main/doc/gopclntab.md
- https://developers.redhat.com/articles/2024/04/03/how-add-debug-support-go-stripped-binaries
- https://golang.org/s/go12symtab

### Parsers y herramientas Go

- https://github.com/mandiant/GoReSym
- https://github.com/mandiant/GoReSym/releases
- https://github.com/goretk/gore
- https://github.com/goretk/redress
- https://github.com/riven-labs/unstrip
- https://docs.rs/unstrip/latest/unstrip/
- https://docs.rs/unstrip/latest/unstrip/rewrite/
- https://docs.rs/unstrip/latest/unstrip/inline/
- https://github.com/ATRAPSLLC/gobin
- https://docs.rs/gobin/latest/gobin/
- https://docs.rs/crate/gobin/0.4.1
- https://github.com/1-3-7/disrobe
- https://github.com/volexity/GoResolver
- https://github.com/mandiant/gostringungarbler
- https://github.com/burrowers/garble
- https://github.com/burrowers/garble/blob/master/CHANGELOG.md
- https://github.com/burrowers/garble/blob/master/docs/CONTROLFLOW.md
- https://github.com/mooncat-greenpy/Ghidra_GolangAnalyzerExtension
- https://github.com/mooncat-greenpy/Ghidra_GolangAnalyzerExtension/releases
- https://github.com/go-delve/delve

### Descompiladores, análisis y dinámica

- https://github.com/NationalSecurityAgency/ghidra
- https://github.com/NationalSecurityAgency/ghidra/issues/9219
- https://hex-rays.com/ida-pro/
- https://binary.ninja/
- https://rizin.re/
- https://cutter.re/
- https://github.com/rizinorg/rizin
- https://frida.re/
- https://github.com/frida/frida-rust
- https://www.unicorn-engine.org/
- https://www.capstone-engine.org/
- https://qiling.io/
- https://github.com/angr/angr
- https://github.com/cea-sec/miasm
- https://github.com/JonathanSalwan/Triton
- https://github.com/panda-re/panda
- https://github.com/open-telemetry/opentelemetry-ebpf-profiler
- https://github.com/grafana/pyroscope
- https://github.com/parca-dev/parca

### Rust parsing y testing

- https://crates.io/crates/object
- https://docs.rs/object/0.41.0/object/
- https://docs.rs/object/latest/object/
- https://crates.io/crates/goblin
- https://docs.rs/goblin/latest/goblin/
- https://crates.io/crates/gimli
- https://docs.rs/gimli/latest/gimli/
- https://crates.io/crates/addr2line
- https://docs.rs/addr2line/latest/addr2line/
- https://crates.io/crates/iced-x86
- https://docs.rs/iced-x86/latest/iced_x86/
- https://crates.io/crates/capstone
- https://docs.rs/capstone/latest/capstone/
- https://crates.io/crates/pelite
- https://docs.rs/pelite/latest/pelite/
- https://crates.io/crates/pdb
- https://crates.io/crates/memmap2
- https://crates.io/crates/nom
- https://crates.io/crates/binrw
- https://github.com/rust-fuzz/cargo-fuzz
- https://github.com/proptest-rs/proptest
- https://bheisler.github.io/criterion.rs/book/

---

## 18. Conclusión final

La investigación anterior tenía una buena intuición —aprovechar `pclntab`, tipos, interfaces, CFG, desofuscación y automatización—, pero convertía demasiadas hipótesis en capacidades supuestamente disponibles. También proponía modificar `funcnametab` antes de disponer de un parser fiable y de una matriz de pruebas. La auditoría cambia esa prioridad.

La decisión estratégica más sólida es:

1. **Adoptar `gobin` 0.4.1 como candidato de laboratorio para Go 1.27 V5**, no como autoridad hasta validar fixtures.
2. **Mantener GoReSym y GoRE como parsers de contraste y referencia de metadata**, separando la recuperación de funciones de la de tipos.
3. **Usar `unstrip` para Go 1.18–1.25**, aprovechando sus exporters, inline data, xrefs y escritura de símbolos cuando la plataforma esté cubierta.
4. **Usar GolangAnalyzerExtension v1.4.0 en Ghidra para Go 1.27**, verificando su versión y los resultados contra fuentes independientes.
5. **Usar GoResolver solo como generador de candidatos** y medir su precisión con un corpus propio.
6. **Limitar GoStringUngarbler y disrobe a sus rangos reales**, y no confundir literal deobfuscation con restauración de identificadores.
7. **Reescribir desde cero el parser/patcher Rust original**, empezando por el modelo de contenedor y los offsets tipados.
8. **Separar importación de símbolos, análisis semántico y escritura del binario**.
9. **Construir un corpus de fixtures por release/arquitectura/flags y un pipeline de differential testing/fuzzing** antes de llamar «robusta» a la solución.
10. **Usar LLMs como analistas auxiliares con evidencia**, nunca como autoridad de nombres o comportamiento.

El resultado buscado no es un ejecutable que cambie cadenas, sino un sistema que pueda explicar, para cada símbolo/tipo/relación: qué observó, qué parser lo obtuvo, qué versión de Go presupone, qué evidencia lo confirma, qué contradicciones existen y qué pruebas faltan. Esa es la base que permite escalar a binarios modernos, ofuscados y multiplataforma sin acumular errores silenciosos.


---

# PARTE II — Ampliación técnica y especificación de ingeniería (edición 2.0)

> Esta parte añade los cuatro dominios solicitados y amplía la arquitectura para que pueda convertirse en un proyecto de ingeniería reproducible. La evidencia primaria se cita con URLs directas al código Go. Las afirmaciones de layout deben vincularse siempre a una release/tag concreta en el repositorio de fixtures; `master` es una referencia móvil y no una especificación inmutable.

## 19. Modelo multiproducto: varios `moduledata` y módulos cargados

### 19.1 La premisa correcta

Un proceso Go no debe modelarse necesariamente como un único módulo Go. El runtime mantiene una cadena enlazada que comienza en `firstmoduledata`; cada nodo contiene metadatos de un módulo de código Go. El caso relevante no es hipotético: el runtime documenta la incorporación de módulos cargados dinámicamente mediante `-linkshared` y plugins construidos con `-buildmode=plugin`. La inicialización recorre `for md := &firstmoduledata; md != nil; md = md.next`, descarta los marcados `bad` y construye una colección activa.

Fuentes primarias:

- `runtime/symtab.go`, inicialización de módulos: https://go.dev/src/runtime/symtab.go
- `runtime/plugin.go`, recorrido de `firstmoduledata.next`: https://go.dev/src/runtime/plugin.go
- `runtime/runtime2.go`, definición interna de `_func` y estructuras adyacentes: https://go.dev/src/runtime/runtime2.go
- Documentación de plugins: https://pkg.go.dev/plugin

### 19.2 Qué significa para el análisis estático

No se debe deducir que un ELF en disco contiene necesariamente todos los nodos que pueden aparecer en el proceso vivo. Un plugin es otro artefacto cargable, y el estado enlazado de módulos es un estado de proceso después de la carga. Por tanto, hay dos problemas distintos:

1. **Análisis de artefacto:** localizar y parsear el `pclntab`/`moduledata` que pertenece a un ELF/PE/Mach-O concreto.
2. **Análisis de proceso:** enumerar todos los `moduledata` activos en memoria, asociar cada nodo con su imagen cargada, respetar relocations/ASLR y detectar altas/bajas o módulos `bad`.

No se resuelve el segundo problema concatenando los bytes de dos `pclntab` ni suponiendo que las direcciones de un plugin comparten la base del ejecutable anfitrión.

### 19.3 Modelo de datos exigido

El esquema superior debe contener `modules: []`, no `module: {}`. Cada módulo tiene identidad y espacio de direcciones propios:

```json
{
  "modules": [
    {
      "module_index": 0,
      "module_identity": "main-executable",
      "artifact_sha256": "...",
      "load_bias": null,
      "moduledata_va": null,
      "pclntab_file_offset": "0x...",
      "text_base_va": null,
      "text_base_status": "unresolved",
      "plugin_path": null,
      "bad": null,
      "functions": []
    },
    {
      "module_index": 1,
      "module_identity": "plugin:example.com/fixture",
      "artifact_sha256": "...",
      "load_bias": "0x...",
      "moduledata_va": "0x...",
      "pclntab_file_offset": "0x...",
      "text_base_va": "0x...",
      "text_base_status": "runtime-observed",
      "plugin_path": "example.com/fixture",
      "bad": false,
      "functions": []
    }
  ]
}
```

`null` es semánticamente distinto de cero. Cero puede ser un valor real en un campo concreto o un sentinel; `null` significa que el analizador no dispone de evidencia suficiente. `module_index` es un índice local del informe, no un identificador del runtime.

### 19.4 Descubrimiento y validación de la cadena

Un lector de memoria (no implementado en el núcleo offline actual) deberá:

1. identificar `firstmoduledata` mediante símbolos, patrones versionados o análisis de relocations; nunca asumir un desplazamiento fijo universal;
2. leer cada nodo con layout específico de Go release, arquitectura y build mode;
3. validar que el puntero `next` está alineado y apunta a memoria legible del proceso;
4. imponer un límite máximo de nodos y detectar ciclos mediante conjunto de direcciones visitadas;
5. verificar rangos de texto, datos, BSS y tipos, además de coherencia interna de los slices;
6. identificar `bad`, `hasmain`, `pluginpath` y cualquier campo relevante solo si está presente en el layout concreto;
7. comparar intervalos de memoria entre módulos y rechazar solapamientos anómalos;
8. preservar el orden observado, pero no confundirlo con orden de carga universal en todos los build modes;
9. registrar timestamp, PID, arquitectura, ASLR/load bias y método de captura;
10. conservar un volcado bruto de cada nodo para auditoría posterior.

El runtime comprueba solapamientos entre regiones relevantes al cargar plugins y valida hashes de paquetes; la lógica exacta está en `runtime/plugin.go`. Esas comprobaciones son útiles como oráculo, pero no convierten una estructura privada en ABI estable.

### 19.5 Reglas de atribución de símbolos

- Un nombre extraído del módulo plugin pertenece al espacio de direcciones del plugin.
- El mismo nombre cualificado puede aparecer en más de un módulo; la clave primaria es `(module_id, start_va)` y no solo `name`.
- Si una función inline aparece en el `pclntab` del módulo, la atribución debe mantener el `module_id` del módulo propietario.
- Los símbolos importados/resueltos del host no deben confundirse con funciones compiladas dentro del plugin.
- Las referencias cruzadas entre módulos se modelan como edges con origen, destino, relocation/callsite y evidencia, no fusionando ambos grafos.

### 19.6 Laboratorio M1 — plugins y múltiples módulos

**Fixture mínimo:** host que importa `plugin`, plugin con métodos, tipos, funciones genéricas y strings embed. Compilar ambos con la misma versión exacta, `GOOS`, `GOARCH`, tags, cgo y flags. Luego:

- analizar host y plugin en disco por separado;
- ejecutar host, cargar plugin y capturar memoria después de `plugin.Open`;
- enumerar `firstmoduledata.next` en un entorno instrumentado;
- comparar nodos con artefactos, secciones y `moduledata` recuperado;
- repetir con plugin ya cargado, duplicado, y versiones incompatibles para observar errores y `bad`;
- verificar que la herramienta no crea VAs cruzando módulos.

**Criterio de aceptación:** cada nodo del proceso debe mapear a un artefacto o quedar explícitamente sin resolver. No se acepta la salida que mezcle funciones de dos imágenes bajo una sola base.

---

## 20. `pctab` y `pcvalue`: especificación exacta del decodificador

### 20.1 No es una tabla de offsets lineal

Los datos `pcsp`, `pcfile`, `pcln` y las entradas de `pcdata` referencian streams compactos dentro de `moduledata.pctab`. Cada stream codifica tramos de PC con un valor constante, y el valor cambia cuando se consume un par de deltas. Por ello, no es correcto interpretar `pctab[offset:]` como un array de enteros de 32 bits ni escanearlo buscando números que parezcan líneas.

La implementación canónica es `step`/`readvarint` en `runtime/symtab.go`:

- https://go.dev/src/runtime/symtab.go
- https://go.dev/src/runtime/runtime2.go
- https://go.dev/src/cmd/link/internal/ld/pcln.go

### 20.2 Máquina de estados

Estado inicial:

- `p`: cursor dentro de `pctab` en el offset del stream;
- `pc`: PC de entrada de la función;
- `value`: `-1` (valor inicial implícito);
- `first = true`;
- `pcQuantum`: quantum de instrucción de la arquitectura (el runtime usa `sys.PCQuantum`; `pcHeader.minLC` es un dato relacionado que debe verificarse contra el target concreto y no confundirse ciegamente con toda constante de ABI).

Cada iteración:

1. Leer `uvdelta` como unsigned varint de 32 bits.
2. Si `uvdelta == 0` y no es la primera iteración, terminar: es el terminador del stream.
3. Decodificar el delta firmado mediante zig-zag:
   `signed_delta = (uvdelta >> 1) XOR -(uvdelta & 1)`.
4. Actualizar `value = value + signed_delta` con overflow check.
5. Leer `pcdelta` como unsigned varint de 32 bits.
6. Calcular `advance = pcdelta * PCQuantum` con overflow check.
7. Calcular `next_pc = pc + advance`; debe avanzar estrictamente.
8. Emitir el intervalo `[pc, next_pc)` con el valor actual.
9. Actualizar `pc = next_pc`, `first = false` y continuar.

La primera pareja tiene una regla especial: el primer `uvdelta` cero no termina el stream porque el runtime permite que el primer valor inicial siga siendo `-1`. La condición de terminación solo se aplica tras la primera pareja. Ignorar este detalle causa streams vacíos o truncados.

### 20.3 Varint: límites que debe imponer Rust

El `readvarint` del runtime presupone que el stream es válido y opera sobre memoria interna confiable. Un parser forense procesa bytes hostiles y no puede copiar esa función literalmente. Debe imponer:

- máximo cinco bytes para `u32`;
- rechazo de secuencias truncadas;
- rechazo de overflow en el quinto byte;
- rechazo de varints sobredimensionados/no canónicos si se adopta esa política;
- límite máximo de entradas por stream;
- límite máximo de bytes consumidos;
- overflow check para zig-zag, suma del valor, multiplicación del delta y suma del PC;
- terminador obligatorio antes del final del rango permitido;
- monotonicidad de PC y no permitir rangos vacíos;
- límite de PC compatible con el rango de función.

La implementación debe aceptar únicamente el subrango que el runtime/versión acepta. No debe “reparar” streams inválidos de forma silenciosa.

### 20.4 `minLC` frente a `PCQuantum`

No presentar `minLC` como un sinónimo universal e intercambiable de cualquier quantum de PC sin verificar el target. `pcHeader.minLC` es el tamaño mínimo de instrucción codificado en la cabecera. El runtime multiplica `pcdelta` por `sys.PCQuantum`. En las arquitecturas Go soportadas suele coincidir con la granularidad relevante, pero el parser debe resolver el quantum mediante una tabla versionada de arquitectura/GOARCH o una regla derivada de la fuente oficial de la release, y registrar esa decisión. Para el MVP, el campo debe llamarse `pc_quantum` y no `min_lc` para no ocultar esta distinción.

### 20.5 Relación entre `pcvalue` y semántica

| Stream | Qué recupera | Qué NO recupera por sí solo |
|---|---|---|
| `pcsp` | variación de SP/stack frame por PC | prototipo fuente completo |
| `pcfile` | índice de archivo por PC | path si falta `cutab`/`filetab` |
| `pcln` | línea fuente aproximada por PC | línea exacta tras `-trimpath`/ofuscación o inlining ambiguo |
| `PCDATA_StackMapIndex` | índice de mapa de stack por PC | tipos completos de variables |
| `PCDATA_InlTreeIndex` | índice de inline tree por PC | cuerpo fuente original |
| `PCDATA_ArgLiveIndex` | índice de liveness para slots/spills de argumentos | firma fuente completa |

### 20.6 API recomendada

```rust
pub struct PcRange {
    pub start: GoPc,
    pub end: GoPc,
    pub value: i32,
}

pub fn decode_pcvalue(
    bytes: &[u8],
    start_pc: GoPc,
    pc_quantum: NonZeroU8,
    limits: DecodeLimits,
) -> Result<Vec<PcRange>, PcValueError>;
```

No debe devolver una lista parcial como si fuera completa si falta el terminador. Si se desea tolerancia forense, debe existir una variante `decode_partial` cuyo resultado incluya `complete: false`, `consumed_bytes` y causa exacta.

### 20.7 Pruebas obligatorias

- primer `uvdelta = 0` seguido de un `pcdelta` válido;
- terminador cero en segunda posición;
- `uvdelta` de un byte y de cinco bytes;
- delta zig-zag positivo, negativo y extremo;
- `pcdelta = 0` (debe evaluarse conforme al runtime/fixture; no aceptar avance nulo como intervalo);
- `pcdelta * quantum` overflow;
- PC sum overflow;
- stream truncado en `uvdelta` y en `pcdelta`;
- ausencia de terminador;
- límite de número de pares;
- streams reales exportados por toolchains fijadas para amd64, arm64 y 386.

### 20.8 Estado de implementación en este paquete

El workspace `go-retool` incluye un decodificador `pctab` independiente con varint32 acotado, zig-zag, quantum configurable, terminador post-primer-par y pruebas unitarias de ejemplo. No se ha integrado todavía con la lectura de arrays `pcdata` dentro de `_func` ni se ha probado contra streams extraídos de binarios Go reales en este entorno. Por tanto, es una implementación de núcleo verificable en código, no una certificación de compatibilidad completa con todas las releases.

---

## 21. Mapas GC, `FUNCDATA` y recuperación parcial de argumentos

### 21.1 Qué contienen realmente

Las constantes oficiales actuales incluyen:

- `FUNCDATA_ArgsPointerMaps = 0`
- `FUNCDATA_LocalsPointerMaps = 1`
- `FUNCDATA_StackObjects = 2`
- `FUNCDATA_InlTree = 3`
- `FUNCDATA_OpenCodedDeferInfo = 4`
- `FUNCDATA_ArgInfo = 5`
- `FUNCDATA_ArgLiveInfo = 6`
- `FUNCDATA_WrapInfo = 7`

Fuente primaria: https://go.dev/src/internal/abi/symtab.go y https://go.dev/src/runtime/funcdata.h

El registro `_func` contiene `args`, `npcdata` y `nfuncdata`; a continuación siguen arrays variables `pcdata[npcdata]uint32` y `funcdata[nfuncdata]uint32`. El `funcdata` se resuelve con respecto a `moduledata.gofunc` (no como puntero absoluto almacenado directamente en cada `_func`). Los valores sentinel importan: `^uint32(0)` indica ausencia de un descriptor de funcdata. Véase https://go.dev/src/runtime/runtime2.go y https://go.dev/src/runtime/symtab.go.

### 21.2 Qué se puede inferir

Con una estructura `_func` íntegra y el payload `FUNCDATA_ArgsPointerMaps` se puede recuperar información de GC sobre el área de argumentos representada por el mapa: cuántas palabras contiene y qué slots son punteros en el mapa seleccionado. El campo `args` aporta tamaño de argumentos/retornos de acuerdo con la convención interna de esa versión, salvo el sentinel `abi.ArgsSizeUnknown`.

Esto es evidencia útil para un grafo de funciones:

- tamaño del área de argumentos conocida;
- slots pointer/scalar a granularidad de palabra del mapa;
- número de mapas y selector por PC (`PCDATA_StackMapIndex`);
- presencia de objetos de stack;
- liveness de spill slots de argumentos en registros;
- correlación con la convención ABIInternal del target.

No equivale a recuperar nombres, tipos Go, orden semántico de parámetros ni la firma original. Un bit de mapa indica si un slot es puntero para el GC, no si ese slot representa `*MyStruct`, `string`, `map`, `chan`, interfaz u otro tipo concreto. Un scalar puede contener entero, float, enum, offset, bool o fragmento de agregado.

### 21.3 El matiz crítico de ABIInternal

Con register ABI, algunos argumentos/resultados se asignan a registros y no aparecen como una secuencia plana de slots de stack. La información de liveness de argumentos que pasan por registros se refiere a spill slots para traceback: `cmd/compile/internal/liveness/arg.go` explica que los argumentos en registros tienen spill slots y que el formato `FUNCDATA` marca el offset mínimo que requiere seguimiento y una lista de bitmaps; `PCDATA_ArgLiveIndex` selecciona el mapa para un PC.

Fuentes primarias:

- ABI y asignación: https://go.dev/src/cmd/compile/internal/abi/abiutils.go
- seguimiento de liveness: https://go.dev/src/cmd/compile/internal/liveness/arg.go
- IDs PCDATA/FUNCDATA: https://go.dev/src/internal/abi/symtab.go
- runtime stack maps: https://go.dev/src/runtime/stkframe.go
- runtime traceback/ArgInfo: https://go.dev/src/runtime/traceback.go

La reconstrucción debe modelar dos dominios por separado:

1. `stack_argument_area`: bytes/words descritos por `args` y mapas GC;
2. `register_argument_assignment`: ubicación por registro y spill slot según ABIInternal, arquitectura, tipos candidatos y versión del compilador.

No se debe proyectar el mapa de stack directamente sobre el conjunto de registros como si ambos fueran la misma secuencia.

### 21.4 Qué aporta `FUNCDATA_ArgInfo`

`ArgInfo` es consumido por el runtime en el camino de traceback para imprimir argumentos; `ArgLiveInfo` y `PCDATA_ArgLiveIndex` ayudan a determinar si los slots de spill que se imprimen contienen valores vivos. La fuente `runtime/traceback.go` demuestra el consumo del payload, y `cmd/compile/internal/liveness/arg.go` documenta el formato de liveness. No debe afirmarse que `ArgInfo` sea una tabla universal de firmas deserializable sin seguir el formato exacto de la release.

Implementación recomendada:

- parsear primero el prefijo fijo `_func`;
- leer `npcdata` y `nfuncdata` con límites;
- calcular offsets de arrays con checked arithmetic;
- resolver cada `pcdata` como offset dentro de `pctab`;
- resolver cada `funcdata` con la base `gofunc` del módulo;
- distinguir `null` (descriptor ausente/no localizado) de `0` (offset válido, si el formato lo permite);
- parsear stackmap `n`, `nbit` y bytes con límites exactos;
- decodificar bits en orden definido por el runtime de la release;
- enlazar mapas con el PC index correspondiente;
- exportar evidencia de GC separada de las hipótesis de firma.

### 21.5 Modelo de evidencia de firma

No exportar `signature: "func(*T, string) int"` a partir de un mapa GC. Exportar algo como:

```json
{
  "argument_evidence": {
    "args_size_bytes": 32,
    "args_size_status": "known_from__func_args",
    "gc_pointer_map": {
      "map_index": 0,
      "word_size": 8,
      "slots": ["pointer", "scalar", "pointer", "scalar"],
      "source": "FUNCDATA_ArgsPointerMaps"
    },
    "register_liveness": {
      "available": true,
      "source": ["FUNCDATA_ArgLiveInfo", "PCDATA_ArgLiveIndex"],
      "register_assignment": "not_decoded"
    },
    "signature_reconstruction": {
      "status": "partial_constraints_only",
      "confidence": "low",
      "warning": "GC pointer/scalar classification is not a Go source type signature"
    }
  }
}
```

### 21.6 Reglas para el LLM

El prompt debe decir expresamente:

- `pointer` no significa `*T` identificado;
- dos palabras consecutivas pueden formar un valor compuesto (por ejemplo, string/interface/slice según layout) y no deben separarse sin evidencia;
- un puntero puede apuntar a objeto de runtime con layout conocido, pero el mapa por sí solo no revela el tipo;
- las ubicaciones de argumentos en registros deben calcularse con ABIInternal y los tipos candidatos, no inferirse por el orden de bits del mapa;
- si `ArgsSizeUnknown` aparece, no fabricar tamaño;
- si falta `FUNCDATA`, no inferir mapa vacío;
- presentar alternativas y evidencia por parámetro, nunca una firma autoritativa inventada.

---

## 22. `//go:embed`: diferencias de representación y reconstrucción de recursos

### 22.1 Tres representaciones distintas

`//go:embed` no genera siempre una tabla uniforme. La salida depende del tipo de variable:

1. **`string`:** una variable string cuyo descriptor contiene puntero a bytes y longitud.
2. **`[]byte`:** descriptor slice con puntero, longitud y capacidad.
3. **`embed.FS`:** un objeto `embed.FS` que apunta a un slice de `embed.file`; cada registro contiene nombre, datos y hash.

Fuentes primarias:

- API/estructura `embed.FS` y orden de `Files`: https://go.dev/src/embed/embed.go
- emisión de datos por el compilador: https://go.dev/src/cmd/compile/internal/staticdata/embed.go
- documentación de directivas: https://pkg.go.dev/embed

### 22.2 Layout de `embed.FS` y `embed.file`

El código de compilador declara explícitamente que `embed.FS` contiene `files *[]file`. Para el payload de `embed.FS`, `WriteEmbed` crea un símbolo `<var>.files`, emite el header del slice y a continuación un registro por entrada. El propio comentario del compilador muestra el layout lógico de `embed.file`:

```go
type file struct {
    name string
    data string
    hash [16]byte
}
```

En una arquitectura de punteros de 64 bits, un registro ocupa normalmente `2*8 + 2*8 + 16 = 48` bytes; en 32 bits, `2*4 + 2*4 + 16 = 32` bytes, antes de considerar cualquier alineación que el layout exacto del compilador imponga. No hardcodear estos tamaños: calcularlos a partir de la arquitectura y confirmar contra `go/types`/ABI y fixtures.

La tabla no es simplemente `name pointer + data pointer`. Cada string es un par pointer/length y existe un hash de 16 bytes. Las entradas de directorio se marcan con nombre terminado en `/` y `data` nulo/vacío. Las entradas reales tienen string data y hash. El compilador ordena los nombres por directorio y luego elemento, no por comparación lexicográfica plana; la documentación de `embed.FS` explica por qué agrupa directorios para búsquedas binarias.

### 22.3 Localización en `.rodata`, `.noptrdata` y `.data`

No existe una regla universal «todo embed está en `.rodata`»:

- los bytes del recurso suelen estar en símbolos de datos de solo lectura;
- los nombres de archivo se emiten como símbolos string;
- el backing array de `embed.file` y el header del slice son símbolos globales `RODATA|LOCAL` en el código actual del compilador;
- la variable `embed.FS` contiene un puntero a ese backing array;
- los descriptores pointer-rich pueden tener tratamiento de relocations y clasificación distinta según target/linker.

La fuente `staticdata/embed.go` muestra que el backing array se marca `RODATA|LOCAL` y que el descriptor de variable apunta al símbolo `.files`. No asumir que el nombre de la sección de ELF/PE/Mach-O conserva el nombre lógico del símbolo Go ni que la región sea idéntica entre linkers.

### 22.4 Algoritmo de reconstrucción estática

**Fase A — localizar símbolos/variables candidatas**

- usar DWARF si está presente;
- consultar tabla de símbolos y relocation records;
- buscar referencias a tipos `embed.FS` y nombres de símbolos con sufijo `.files` cuando sobrevivan;
- usar metadata de tipos Go como evidencia auxiliar, no como única clave;
- en muestras Garble, no depender de nombre de variable ni package path original.

**Fase B — resolver el slice header**

- identificar pointer al backing array;
- validar len/cap y `len <= cap`;
- limitar recuento por política (por ejemplo, cientos de miles, no `u64::MAX`);
- validar que el array completo está en memoria/file mapping legible;
- calcular `sizeof(file)` a partir del ABI target y layout versionado;
- validar cada campo y la región de hash.

**Fase C — recuperar strings**

- para cada `name`, validar pointer y length, límites y UTF-8 solo como hipótesis (Go strings son bytes, no necesariamente UTF-8);
- preservar bytes originales, y producir una representación textual escapada por separado;
- distinguir directorios por slash final y data vacía;
- para archivos, validar puntero/longitud del payload, límites y hash;
- conservar contenido binario aunque no sea texto.

**Fase D — validar consistencia**

- comprobar orden de nombres según `embedFileLess`;
- verificar que las entradas de directorio tienen `data` vacío;
- comprobar hashes con el algoritmo exacto de la release; no asumir BLAKE2/SHA sin leer la fuente de `embed` correspondiente;
- verificar que rutas cumplen reglas de `io/fs` y no contienen rutas absolutas/traversal inesperado;
- contrastar con ejecución controlada `FS.ReadDir`/`FS.ReadFile` sobre el fixture.

### 22.5 String y []byte: no confundir con `embed.FS`

Para `string` y `[]byte`, `WriteEmbed` obtiene un símbolo de bytes del recurso y escribe el descriptor de la variable. Para string, escribe data pointer + length. Para slice, pointer + length + capacity. No hay tabla de nombres/archivos ni `embed.file` records. Un buscador de estructuras `embed.FS` no detectará necesariamente los recursos embebidos como string/bytes.

### 22.6 Garble y embed

La presencia de recursos no garantiza que sus nombres sean evidentes. Garble puede transformar literales y rutas; se debe probar `-literals` por separado y revisar qué transformaciones aplica a `embed`. No inferir que un nombre de archivo visible demuestra que el contenido no fue transformado, ni que contenido cifrado implica necesariamente `//go:embed`.

### 22.7 Implementación recomendada

Crear un subsistema independiente `embed_recovery` que reciba:

- mapeo de memoria (file offset ↔ RVA ↔ VA);
- arquitectura/pointer size/endian;
- release/layout de Go;
- candidatos a slice header y `embed.file` array;
- evidencia de tipo o símbolo si existe.

Salida por archivo:

```json
{
  "path_bytes_hex": "...",
  "path_display": "assets/index.html",
  "is_directory": false,
  "data_file_offset": "0x...",
  "data_va": null,
  "size": 8192,
  "hash16": "...",
  "hash_status": "not_verified",
  "recovery_method": "embed.FS.file_record",
  "confidence": "high"
}
```

No escribir un extractor basado únicamente en buscar cadenas seguidas de bloques de bytes. Eso genera falsos positivos en recursos ordinarios, strings, tablas de compresión y otros blobs.

---

## 23. Diseño del workspace Rust entregado

### 23.1 Objetivo y límites

El workspace `go-retool/` incluido en el ZIP es un primer núcleo de investigación escrito para compilar en Rust estable, no un producto final con soporte integral de Go 1.27. Su alcance implementado se declara en su README y no se debe ampliar por interpretación.

Incluye:

- tipos nuevos para `FileOffset`, `Rva`, `VirtualAddress`, `GoTextOffset`, `GoFuncOffset` y `GoNameOffset`;
- lector little-endian con límites y checked arithmetic;
- inspección de contenedores ELF/PE/Mach-O mediante el crate `object`, con secciones y rangos file-backed; búsqueda conservadora de candidatos `pclntab` y mapeo de file offset a VA de sección (no a base de texto Go);
- parser conservador para cabeceras de la familia Go 1.18+/Go 1.20 y `functab` de dos `u32`;
- seguimiento de `funcoff` a `_func.nameOff`;
- resolución de nombres en `funcnametab`;
- VA nullable y solo resuelta cuando el operador suministra una base de texto explícita;
- decodificador aislado de `pcvalue` con pruebas unitarias de ejemplo;
- extracción del prefijo fijo `_func` y de los arrays variables `pcdata[]`/`funcdata[]` como offsets crudos;
- walker genérico y cycle-safe para cadenas `moduledata.next` (el lector de nodos sigue dependiendo del layout externo);
- decodificador de bitmap `stackmap` con orden de bits LSB-first;
- decodificador de registros `embed.file` sobre una imagen de memoria ya mapeada y con VA de array suministrada;
- salida JSON con `modules: []` y campos nullable;
- CLI mínima;
- importador JSON de Ghidra;
- fixtures Go con interfaces, generics, closures, goroutines, reflexión, `embed.FS`, string y []byte, más fixture de plugin.

No incluye todavía mapeo de relocations/ASLR, Wasm container parsing, `moduledata` discovery/lectura versionada, full Go 1.27 V5 type graph, funcdata payload resolution, descubrimiento automático de `embed.FS`, patching ni ejecución automática del binario. Esas tareas están descritas en el roadmap.

### 23.2 Tipos de direcciones: reglas formales

El núcleo evita representar todos los valores como `u64`. Reglas:

- `FileOffset` solo indexa bytes del archivo;
- `Rva` representa dirección relativa a image base en formatos que la definen;
- `VirtualAddress` es dirección del espacio virtual del proceso/imagen;
- `GoTextOffset` es un offset dentro del espacio de texto del módulo;
- `GoFuncOffset` es offset al objeto `_func` relativo a la región que el formato exige;
- `GoNameOffset` es offset firmado dentro de `funcnametab`.

Una suma válida debe ser una operación explícita `base.checked_add(offset)`, y una traducción file-to-VA exige un segmento/mapping que pruebe que el offset cae dentro del rango. No permitir `FileOffset + GoTextOffset` ni conversiones implícitas entre VA y RVA.

### 23.3 Contrato del parser actual

CLI:

```sh
go-retool scan <executable> [--out scan.json]
go-retool pclntab <file> --offset 0xFILE_OFFSET [--text-base 0xVA] [--module-index N] [--out report.json]
```

`scan` inspecciona ELF/PE/Mach-O, enumera secciones y busca candidatos modernos de `pclntab`; reporta la dirección de la cabecera dentro de la sección según el dominio del contenedor (por ejemplo, RVA en PE), no la base de texto Go. `pclntab` analiza un offset explícito y entrega las funciones.

- `--offset` es file offset de `pcHeader`, no RVA.
- `--text-base` es un override explícito; el programa no afirma haber descubierto `moduledata.text`.
- sin `--text-base`, `start_va` y `end_va` son `null`; se preservan `entryoff` y sentinel.
- el parser valida magic, padding, `minLC`, pointer size, tamaño de cabecera, `nfunc`, rango de tabla, orden de `entryoff`, `_func` prefix y nombre NUL-terminated.
- el parser no toma un archivo ejecutable como si `pclntab` empezase en offset cero.

### 23.4 Advertencia importante sobre V5

Aceptar una magic no es sinónimo de parsear correctamente todos los datos de una release. Go 1.27 introduce cambios alrededor de metadata de tipos, `moduledata` y `.go.type` que no están implementados en este núcleo. La etiqueta `pclntab_layout` del JSON expresa el layout que el parser pretende leer, no una afirmación de compatibilidad completa con cada versión 1.18–1.27. Antes de etiquetar Go 1.27 como soportado, se requieren fixtures compilados con release oficial fijada y comparación diferencial.

### 23.5 Build/test status honesto

En el entorno de generación no estaban instalados `rustc` ni `cargo`; por tanto, el código Rust (incluido el adaptador `object` para contenedores) no pudo compilarse ni ejecutarse aquí. El ZIP incluye tests fuente, instrucciones exactas y fixture generator, pero no contiene un resultado de `cargo test` exitoso. Sí se construyó y ejecutó el fixture Linux/amd64 y se cargó el plugin con su host, ambos con Go 1.23.2. Un checker estructural independiente en Python validó además cuatro targets: Linux/amd64 1.889/1.889; Linux/arm64 1.887/1.887; Windows/amd64 1.939/1.939; Darwin/arm64 1.946/1.946 nombres resueltos, cero errores estructurales en cada uno. Esto valida el layout de `pclntab` moderno en esos fixtures Go 1.23.2, pero no valida la compilación Rust ni Go 1.27. Esta limitación queda registrada deliberadamente.

### 23.6 Siguientes módulos Rust necesarios

1. El adaptador inicial `container.rs` ya inspecciona ELF/PE/Mach-O y reporta rangos de sección; falta mapeo de relocations/ASLR, segmentos cargables y Wasm.
2. El escaneo de candidatos `pclntab` ya existe para magic little-endian Go 1.18/1.20; falta ampliar formatos históricos y validación específica por release.
3. `moduledata/layout.rs` con snapshots de estructuras por tag/arch/buildmode.
4. `moduledata/chain.rs` para linked list en memoria, límites y ciclo (el walker genérico ya existe; falta lector de nodos versionado).
5. `func.rs` para resolver payloads `funcdata` y enlazar `pcdata`/`funcdata` con sus bases de módulo (los prefijos y offsets variables ya se extraen).
6. `pctab.rs` ya tiene el decodificador inicial; falta integrar los offsets de `pcdata` y validar streams reales por release/GOARCH.
7. `stackmap.rs` ya decodifica `stackmap{n,nbit,bytedata}`; falta integrar `ArgLiveInfo` y los selectores PCDATA.
8. `types/v5.rs` para `.go.type` y nuevos descriptores/itabs.
9. `embed.rs` ya decodifica registros `embed.file` si se proporciona el array mapeado; falta discovery de slice/header, extracción automática de string/[]byte y hash verification.
10. `dwarf.rs` con `gimli`/`addr2line`.
11. `export/ghidra.rs`, `export/ida.rs`, `export/bn.rs`.
12. `patch/plan.rs` y `patch/transaction.rs` separados del parser, deshabilitados por defecto.

---

## 24. Contrato del importador Ghidra

### 24.1 Entrada

El importador consume el JSON versionado, no busca `pclntab` por su cuenta. El parser y Ghidra no deben duplicar heurísticas. El JSON debe conservar:

- `schema_version`;
- `parser.name`, `parser.version`, `pclntab_layout`;
- `modules[]`;
- `functions[]`;
- VAs nullable;
- `entryoff`, `funcoff`, `nameoff`;
- `observed_name`, evidencia y estado.

### 24.2 Reglas de importación

- Solo crear/renombrar en dirección resuelta y mapeada en memoria del programa abierto.
- No crear función si `start_va` es null.
- No sumar `entryoff` a image base dentro del script Ghidra.
- No sobreescribir símbolos de usuario sin opción explícita.
- No convertir barras de package path a separadores de ruta; usar una función de sanitización reversible o conservar el nombre original como comentario.
- No asumir que el `end_va` es una instrucción válida; se usa para validar tamaño/crear body solo si el rango cae en bloque ejecutable y el analizador no ha creado una función contradictoria.
- Informar recuento de importados, renombrados, omitidos y errores.

### 24.3 Idempotencia

Ejecutar dos veces el importador debe producir el mismo estado lógico. La segunda ejecución no debe crear símbolos duplicados ni renombrar nombres ya definidos por el usuario. El comentario de procedencia puede actualizarse de forma determinista.

### 24.4 Módulos externos/plugins

Un JSON puede contener más de un módulo, pero una instancia Ghidra abierta normalmente representa una imagen concreta. El importador debe verificar que cada VA pertenece al programa actual. Si no, omitirla y reportar que se necesita abrir el artefacto/plugin correspondiente. No importar VAs de un proceso vivo como si fueran VAs estáticas del archivo si ASLR/load bias no se ha reconciliado.

---

## 25. Harness de fixtures y laboratorio científico

### 25.1 Fixture source

El directorio `fixtures/` contiene un programa de prueba que ejerce:

- interfaces y method sets;
- estructuras y métodos;
- generics;
- closures;
- goroutines y canales;
- `defer`;
- reflexión;
- `embed.FS` con archivo real y directorio;
- `//go:embed` de string y []byte;
- función `//go:noinline` para reducir una fuente de variación de optimización.

El fixture plugin es un artefacto separado y debe compilarse con el mismo toolchain y configuración del host.

### 25.2 Matriz mínima de compilación

No basta con `go build` una vez. Matriz propuesta:

| Dimensión | Valores |
|---|---|
| Go | 1.18, 1.20, 1.21, 1.23, 1.25, 1.26, 1.27.x; añadir releases intermedias si el parser declara soporte |
| GOARCH | amd64, arm64, 386, arm, riscv64 cuando el toolchain y entorno lo permitan |
| GOOS | linux, windows, darwin; añadir freebsd cuando proceda |
| Link mode | internal, external cuando aplicable |
| Build mode | exe, plugin, c-shared/shared cuando aplicable |
| Debug | DWARF on/off (`-ldflags=-s -w`) |
| Path | normal, `-trimpath` |
| Optimización | default, `-gcflags=all=-N -l` |
| Obfuscación | plain, Garble names, `-literals`, `-tiny` |

No marcar un target como cubierto si la toolchain no lo compila o si el test no se ejecutó. Registrar fallo de build separado de fallo del parser.

### 25.3 Oráculos

Para cada binario fixture, guardar:

- hash SHA-256 del artefacto;
- `go version -m`;
- `go tool nm` cuando sea aplicable;
- `go tool objdump` para funciones seleccionadas;
- `debug/buildinfo` output;
- salida del fixture;
- GoReSym JSON;
- GoRE report;
- gobin report;
- salida del parser propio;
- lista de funciones de DWARF cuando exista;
- manifest de toolchain (commit/tag, `GOOS`, `GOARCH`, flags, env).

Las salidas no se comparan por igualdad textual sin normalización: nombres, offsets, rutas, orden de mapas y diferencias de ABI deben alinearse primero.

### 25.4 Métricas

- precision/recall de funciones físicas;
- exactitud de `entryoff`/`funcoff`;
- tasa de nombres recuperados correctamente;
- errores de VA mapping;
- número de ranges `pctab` correctos por stream;
- cobertura de inline frames;
- exactitud de file/line;
- exactitud de mapa GC por slot;
- tasa de recursos embed recuperados byte a byte;
- falsos positivos del scanner;
- robustez a truncación/fuzzing;
- idempotencia del importador Ghidra;
- divergencia entre parsers independientes.

### 25.5 Baselines de calidad

No declarar «100 % compatible» por haber parseado un solo binario. Para cada release/arch, publicar un informe con número de fixtures, funciones esperadas, funciones observadas, diferencias, casos no aplicables y errores conocidos. Toda métrica debe incluir denominador y criterios de exclusión.

---

## 26. Fuzzing, parsers hostiles y propiedades formales

### 26.1 Ataques contra el parser

Entradas maliciosas pueden incluir:

- magic válida con offsets fuera del archivo;
- `nfunc` desmesurado;
- pointer size imposible;
- header truncado;
- `funcoff` hacia mitad de campo;
- `nameOff` negativo o fuera de rango;
- cadenas sin NUL;
- `entryoff` desordenados;
- sentinel anterior al último inicio;
- varint de seis bytes;
- varint de quinto byte con overflow;
- `pcdelta` que desborda;
- `_func.npcdata` que hace overflow al calcular el array;
- `nfuncdata` inconsistente;
- `moduledata.next` cíclico;
- slice len > cap;
- punteros de embed que apuntan fuera de los segmentos;
- hashes inconsistentes;
- intervalos de texto superpuestos.

### 26.2 Invariantes

- Ninguna lectura ocurre sin comprobar rango.
- Ninguna suma/multiplicación de offset ocurre sin checked arithmetic.
- `entryoff` es monotónico en el rango de tabla que el formato exige.
- `end_pc > start_pc` para funciones y tramos.
- Todo offset de string queda dentro de su tabla y termina en NUL antes del límite.
- `funcoff` apunta a un `_func` íntegro y arrays variables acotados.
- El número de módulos no supera un límite operativo.
- La cadena `next` no repite nodo.
- El parser no crea VAs si no hay mapping/base demostrada.
- La escritura binaria, si se implementa, no se ejecuta en la misma fase de parsing.

### 26.3 Fuzzing diferencial

Usar `cargo-fuzz`/libFuzzer sobre `parse_header`, `parse_functab`, `parse_func`, `decode_pcvalue`, `decode_stackmap`, `decode_embed_fs`. Mantener corpus de seeds válidos por Go release. Los mutadores deben preservar algunas invariantes para alcanzar rutas profundas, y también romperlas deliberadamente.

Comparar parser propio con `debug/gosym`, GoReSym, GoRE y `gobin`, pero no tratar a un solo parser como oráculo perfecto. Ante desacuerdo, rastrear el código fuente de la versión concreta del runtime/linker y compilar un fixture reducido.

---

## 27. Seguridad, integridad y arquitectura de escritura

### 27.1 Separación de fases

La herramienta final debe separar procesos:

- `inspect`: lectura, parsing y extracción;
- `enrich`: análisis semántico y enriquecimiento;
- `export`: JSON/Ghidra/IDA;
- `plan-patch`: genera plan auditable sin escribir;
- `apply-patch`: única fase que puede modificar una copia de trabajo.

El binario original se abre en modo read-only. El parser no importa módulos que escriban. No habilitar una opción global `--write` que cambie silenciosamente el comportamiento.

### 27.2 Patch transaction

Cada patch plan debe incluir:

- SHA-256 de entrada;
- tamaño y formato;
- ranges exactos a modificar;
- bytes originales esperados;
- bytes nuevos;
- precondiciones y postcondiciones;
- estrategia de backup;
- versión del writer;
- verificación de no-overlap;
- firma/hash esperado de salida cuando se conozca.

Aplicar sobre copia temporal, escribir a archivo temporal en el mismo filesystem, fsync, validar formato y parser de salida, ejecutar pruebas opcionales en sandbox, y solo entonces publicar el artefacto. Si falla, conservar original y rollback.

### 27.3 `dry-run` real

`dry-run` nunca debe abrir `MmapMut`, pedir permisos de escritura ni crear el archivo final. Debe producir un diff/plan en memoria o un JSON de plan. Si se necesita mapear, usar mapping inmutable. Los tests deben ejecutar dry-run sobre archivo chmod read-only y comprobar que no se altera ni se intenta abrir en escritura.

### 27.4 Firmas y metadatos

Cualquier cambio en ELF/PE/Mach-O puede invalidar firmas, checksums, Authenticode, code signatures, build IDs o integridad del paquete. La herramienta debe reportar los efectos y no fingir preservar una firma que ya no es válida.

---

## 28. Estrategia de versiones y snapshots de runtime

### 28.1 Por qué no existe un `moduledata` universal

`moduledata` es estructura privada sincronizada entre runtime y linker. Cada actualización puede mover campos, cambiar slices o introducir nuevos rangos. El sistema debe mantener snapshots generados por release/tag y arquitectura, con commit hash, fuente, layout checksum y tests.

### 28.2 Generación de snapshots

Pipeline recomendado:

1. checkout de tag oficial exacto (`go1.x.y`);
2. extracción de `runtime/runtime2.go`, `runtime/symtab.go`, `runtime/plugin.go`, `runtime/type.go`, `internal/abi/symtab.go`, `cmd/link/internal/ld/pcln.go`, `cmd/compile/internal/staticdata/embed.go`;
3. AST parsing de structs/constants con Go parser o extractor propio;
4. cálculo de offsets/size mediante helper compilado con target GOARCH cuando sea posible;
5. revisión manual de fields con `uintptr`, slices, sentinel y build tags;
6. snapshot JSON y tests con fixtures compilados por ese toolchain;
7. checksum del snapshot y aprobación code review.

No extraer layouts de `master` para analizarlos como si fueran release estable. Si se usa master, etiquetarlo como nightly/experimental.

### 28.3 ABI y arquitectura

La asignación de argumentos/register ABI depende de GOARCH. Las estructuras de runtime pueden variar por pointer size y tags. No basta con `ptrSize=8`: amd64 y arm64 comparten tamaño de puntero, pero no convenciones de registros ni `PCQuantum` necesariamente. El snapshot debe tener al menos `(Go release, GOARCH, GOOS, buildmode, linkmode)` como clave cuando los datos lo requieran.

---

## 29. Integración con análisis de tipos Go 1.27 y `.go.type`

### 29.1 Separar recuperación de funciones y recuperación de tipos

El éxito del parser `pclntab` no demuestra que `.go.type` esté bien parseado. La arquitectura debe publicar estados independientes:

- `pclntab_status`;
- `moduledata_status`;
- `type_metadata_status`;
- `itab_status`;
- `dwarf_status`;
- `runtime_module_chain_status`.

### 29.2 Rango y offsets

En el layout moderno, los límites de tipo (`types`, `etypes`) y campos asociados a longitud/itabs no deben reinterpretarse como slices clásicos. Los offsets relativos de tipo/name/text tienen bases y sentinels diferentes. Implementar un `OffsetDomain` para cada uno: `TypeOff`, `NameOff`, `TextOff`, `ModuleTypeOffset`, etc.

### 29.3 Itabs e interfaces

La recuperación de itabs debe distinguir:

- interfaz estática/descriptor de interfaz;
- tipo concreto;
- tabla de métodos de implementación;
- wrapper/thunk y ABI wrapper;
- itab de runtime y metadatos de linker.

No deducir que cada pareja `(interface, concrete)` observada en código produce una única estructura estática retenida. El linker/runtime puede compartir, omitir o generar metadatos bajo condiciones específicas.

### 29.4 `TFlag` y versión

El bit/flag de tipo que indica propiedades de interfaz directa/indirecta cambió en el ciclo de versiones que señalas. No copiar una máscara antigua ni asumir que el nombre de flag equivale a la semántica anterior. Extraer constantes del tag oficial y mantener tests que prueben los casos directos/indirectos con tipos pequeños y grandes en cada release relevante.

### 29.5 Aceptación

La compatibilidad Go 1.27 V5 solo se marca «verificada» cuando:

- se parsean headers y funciones;
- se valida `moduledata` real;
- se enumeran tipos con límites exactos;
- se reconstruyen tipos representativos (struct, ptr, slice, map, chan, func, interface, array, generic instantiations);
- se reconstruyen itabs/method sets y se comparan con reflect/fixture;
- se valida que las direcciones y offsets pertenecen a secciones correctas;
- se contrasta con al menos otro parser independiente y código runtime/linker taggeado.

---

## 30. Herramientas adicionales: matriz de integración y límites

| Herramienta | Uso útil | No delegar en ella |
|---|---|---|
| Go toolchain (`go tool nm`, `objdump`, `buildid`, `version -m`) | baseline y oráculo de build | parser universal de ejecutables externos |
| `debug/gosym` | lectura estándar de pclntab soportado | layouts futuros no implementados |
| GoReSym | recuperación rápida de metadatos Go | afirmar soporte de tipos V5 si su parser no lo implementa |
| GoRE/Redress | contraste y extracción adicional | sustituir verificación runtime exacta |
| gobin | candidato moderno de interés | tratar README como certificación independiente |
| unstrip | reconstrucción de símbolos en su rango | usar fixture 1.27 para evaluar soporte si el rango declarado termina en 1.25 |
| Ghidra GolangAnalyzerExtension | integración de análisis y tipos | asumir que la extensión es equivalente a Ghidra builtin |
| Ghidra MCP/GhidrAssistMCP | automatizar workflows de Ghidra | autorizar acciones sin revisión y reproducibilidad |
| YARA-X | clasificar familias, reglas de detección | parser de metadatos o prueba de identidad |
| RetDec | contraste ocasional de decompilación | pipeline principal para metadata Go moderna |
| LLVM MC | ensamblado/consulta de arquitectura y validación | decoder completo de todas las ISA desde un solo componente |
| yaxpeax | decoders Rust por arquitectura | parser de contenedores o metadata Go |
| rz-frida | integración Rizin/Frida | recuperación estática de `pclntab` |
| Delve | observar runtime Go y módulos | reemplazar análisis offline |
| eBPF profilers | observación de PCs, símbolos y stack | reconstrucción exacta de fuente/argumentos |

Enlaces de los proyectos auxiliares (mantenerlos solo aquí o en el apartado de referencias):

- https://github.com/SentineLabs/AlphaGolang
- https://github.com/bethington/ghidra-mcp
- https://github.com/symgraph/GhidrAssistMCP
- https://github.com/VirusTotal/yara-x
- https://virustotal.github.io/yara-x/
- https://github.com/avast/retdec
- https://llvm.org/docs/CommandGuide/llvm-mc.html
- https://llvm.org/docs/CodeGenerator.html
- https://docs.rs/yaxpeax-arch/latest/yaxpeax_arch/
- https://docs.rs/yaxpeax-x86/latest/yaxpeax_x86/
- https://docs.rs/yaxpeax-arm/latest/yaxpeax_arm/
- https://github.com/rizinorg/rz-frida

---

## 31. Plan de ejecución incremental con criterios de salida

### Fase 0 — Reproducibilidad

- fijar releases y toolchains;
- generar hashes y manifests;
- mantener originales inmutables;
- automatizar `go version -m`, `nm`, `objdump` y baseline.

**Salida:** corpus reproducible y manifests completos.

### Fase 1 — Contenedor y direcciones

- implementar ELF/PE/Mach-O readers;
- mapear file offsets, RVA y VA;
- testear relocations/ASLR y secciones de texto múltiples;
- importar resultados en Ghidra solo con VA demostrada.

**Salida:** todas las direcciones exportadas llevan mapping evidence o `null`.

### Fase 2 — pclntab + pctab

- ampliar parser actual para formatos previos por separado;
- integrar arrays variables de `_func`;
- parsear `pcdata` y `funcdata`;
- validar `pcvalue` contra fixtures reales.

**Salida:** funciones y pc ranges exactos en targets explícitos de la matriz.

### Fase 3 — moduledata multiproducto

- snapshots por release/arch;
- discovery estática y runtime;
- linked list con cycle detection;
- plugins y `-linkshared`.

**Salida:** módulos separados y reconciliados con artefactos.

### Fase 4 — tipos/GC/ABI

- `.go.type` V5;
- itabs y method sets;
- stack maps y ArgInfo;
- ABIInternal y register assignment.

**Salida:** tipos y restricciones de firma con procedencia granular.

### Fase 5 — embed y Garble

- recuperar `string`, `[]byte`, `embed.FS`;
- verificar hash16 y estructura;
- Garble names/literals/tiny en fixtures controlados;
- comparar disrobe/GoStringUngarbler y hooks dinámicos.

**Salida:** recursos recuperados byte a byte o fallo explicado.

### Fase 6 — decompilación y nombres

- CFG fingerprints;
- reference builds;
- BSim/GoResolver;
- candidate ranking y calibración;
- importadores multi-tool.

**Salida:** nombres propuestos con precision/recall medidos y procedencia.

### Fase 7 — patching

- patch plans;
- writer transaccional;
- backups y hashes;
- round-trip parse;
- ejecución sandbox opcional.

**Salida:** parche reproducible, reversible y verificado; nunca parche silencioso.

---

## 32. Registro explícito de correcciones de esta edición

1. Eliminados los bloques de URLs de herramientas auxiliares que se habían pegado al final de 2.9 y 4.8; esos enlaces quedan agrupados en el catálogo de herramientas auxiliares y referencias.
2. Corregido el JSON de la sección 10.3: `modules[]`, `start_va`/`end_va` nullable y `text_section_index` nullable; el offset relativo permanece aunque no exista VA.
3. Corregido Laboratorio E: `unstrip --symbols-as` se prueba sobre Go 1.18–1.25, no sobre fixture Go 1.27.
4. Añadido el modelo de múltiples `moduledata`, `firstmoduledata.next`, plugins y módulos compartidos.
5. Añadida la especificación del stream `pcvalue` y sus reglas de terminación, zig-zag, varints, quantum y límites hostiles.
6. Añadida la separación entre mapa GC de argumentos, tamaño de argumentos, spill slots de ABIInternal y recuperación de firmas.
7. Añadida la descripción de `embed.FS`, `embed.file{name,data,hash[16]byte}`, slice backing array y diferencias con string/[]byte.
8. Añadido un workspace Rust con núcleo parcial, CLI, JSON, tests fuente, fixtures y script Ghidra; incluye extracción de arrays `_func`, walker genérico de módulos, stackmap decoder y decoder de registros embed condicionado a mapping externo.
9. Añadida una declaración de estado de compilación: no se pudo ejecutar Cargo en el entorno de generación porque no estaba instalado Rust.
10. Reforzada la separación entre compatibilidad de magic/layout, compatibilidad `pclntab`, `moduledata`, tipos, itabs y comportamiento real.

---

## 33. Fuentes primarias nuevas de la ampliación

Runtime y módulos:

- https://go.dev/src/runtime/symtab.go
- https://go.dev/src/runtime/plugin.go
- https://go.dev/src/runtime/runtime2.go
- https://go.dev/src/runtime/type.go
- https://go.dev/src/runtime/funcdata.h
- https://go.dev/src/runtime/stkframe.go
- https://go.dev/src/runtime/traceback.go

ABI, compilador y linker:

- https://go.dev/src/internal/abi/symtab.go
- https://go.dev/src/cmd/compile/internal/abi/abiutils.go
- https://go.dev/src/cmd/compile/internal/liveness/arg.go
- https://go.dev/src/cmd/link/internal/ld/pcln.go
- https://go.dev/src/cmd/compile/internal/staticdata/embed.go

Embed y plugins:

- https://go.dev/src/embed/embed.go
- https://pkg.go.dev/embed
- https://pkg.go.dev/plugin

Go standard library debugging:

- https://pkg.go.dev/debug/gosym
- https://pkg.go.dev/debug/buildinfo
- https://pkg.go.dev/debug/elf
- https://pkg.go.dev/debug/pe
- https://pkg.go.dev/debug/macho

Rust ecosystem:

- https://docs.rs/object/latest/object/
- https://docs.rs/goblin/latest/goblin/
- https://docs.rs/gimli/latest/gimli/
- https://docs.rs/serde/latest/serde/
- https://docs.rs/serde_json/latest/serde_json/
- https://github.com/rust-fuzz/cargo-fuzz
- https://github.com/proptest-rs/proptest

---

## 34. Veredicto de la edición 2.0

La versión 2.0 corrige los defectos editoriales identificados y añade especificaciones que conectan directamente con una futura implementación. El repositorio adjunto implementa una fracción deliberadamente pequeña del diseño: tipos de direcciones, adaptador inicial de contenedores ELF/PE/Mach-O, escaneo conservador de candidatos, reader acotado, parser moderno de `functab`, resolución de nombre, arrays variables de `_func`, decodificador `pcvalue`, walker genérico de cadena de módulos, stackmap bitmap decoder, decoder condicionado de registros embed, JSON, CLI e importador Ghidra. No implementa aún relocations/ASLR, lectura versionada de `moduledata`, V5 type metadata, selección de mapas por PCDATA/ArgInfo, descubrimiento automático de recursos embed ni patching.

Esta distinción no es una debilidad editorial; es la condición para que el proyecto sea auditable. La siguiente entrega de ingeniería debe cerrar primero los tests con toolchains Go fijadas, y después incorporar cada módulo con un contrato de datos, fixtures y pruebas negativas. Hasta entonces, la expresión correcta es «prototipo parcial con especificación ampliada», no «retool Go 1.27 completo».
