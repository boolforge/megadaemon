# Esquema protobuf del backend de Antigravity (extraído del `.so`)

Estas herramientas hacen lo que faltaba para que el shim no **invente** respuestas:
recuperar el **esquema real** de la API interna de Google desde el propio
`liblanguage_server.so` del language server de Antigravity.

## El problema que resolvió (MEDIDO 2026-11-26, móvil)

El LS parsea la respuesta de `POST /v1internal:fetchAvailableModels` con
**protojson**, no con JSON a pelo.  El shim le servía ahí el catálogo de
MegaDaemon, que es una **lista** (`{"models":[...]}`), pero en el proto ese
campo es un **MAP**:

```proto
message FetchAvailableModelsResponse {
  map FetchAvailableModelsResponse.ModelsEntry models = 1;   // MAP, no lista
  optional string default_agent_model_id = 2;
  repeated ModelSort agent_model_sorts = 3;
  repeated string command_model_ids = 4;
  ...
}
```

⇒ el LS petaba **exactamente** en la llave, cada 60 s:

```
cache.go:135] Cache(availableModels): Singleflight refresh failed:
proto: syntax error (line 1:11): unexpected token [
errorreport.go:224] proto: syntax error (line 1:11): unexpected token [
```

y la app volvía a pedir el login de Google en bucle.

## Herramientas

| fichero | qué hace |
|---|---|
| `extraer_esquema.py` | Localiza el `FileDescriptorProto` de un `.proto` **embebido** en el ELF y lo decodea (escribe texto o JSON). Detecta los `map` (campo `repeated` cuyo tipo es una entrada de mapa). |
| `validar_protojson.py` | Valida un JSON contra ese esquema con las reglas de **protojson**: un `map` tiene que ser un objeto, un `repeated` una lista, y un campo desconocido es un error. |
| `model_configs.json` | El esquema ya extraído de `model_configs.proto` (para validar sin el `.so`). |

## Uso

```sh
# extraer el esquema del binario del LS
python3 tools/proto-schema/extraer_esquema.py \
    /ruta/a/liblanguage_server.so \
    google/internal/cloud/code/v1internal/model_configs.proto \
    --json --salida /tmp/model_configs.json

# validar una respuesta antes de mandarla
python3 tools/proto-schema/validar_protojson.py \
    --esquema tools/proto-schema/model_configs.json \
    FetchAvailableModelsResponse /tmp/catalogo.json
```

Salida del validador con el catálogo que **rompía** el LS:

```
FALLA (1 problema/s):
  - FetchAvailableModelsResponse.models: es un MAP ⇒ debe ser un objeto JSON, hay list
```

y con la forma correcta (`"models": { "<id>": {…} }`): `OK`.

## Procedencia (para reproducirlo)

El `.so` es el `liblanguage_server.so` que la app de Antigravity instala en
`/data/user/0/com.agy/files/.gemini/antigravity-app/` (181 MB, Go, arm64).
Los descriptores protobuf van embebidos como `FileDescriptorProto` serializado
en `.rodata`: el nombre del fichero aparece como campo 1 (`0x0A`) del
descriptor, pero **también** como dependencia (campo 3, `0x1A`) de otros
descriptores ⇒ hay que quedarse con el que casa con el `name` del descriptor
(por eso `buscar_blob()` comprueba el nombre y no se queda con la primera
aparición).

Todo lo que este directorio afirma está medido sobre el binario; lo que no se
pudo verificar queda marcado como NO VERIFICADO en `REPORTE.md` del
directorio hermano.
