#!/usr/bin/env python3
"""Regenera el codigo Go de los `.proto` reconstruidos.

Dos motores, en este orden:

  1. `protoc` + `protoc-gen-go` (el oficial).  El binario `protoc-gen-go`
     se descarga del release de GitHub de `protocolbuffers/protobuf-go`
     (github.com SI es accesible desde el sandbox).  Se le pasan los
     mappings `M<fichero>=<ruta_de_importacion>` para que cada `.proto`
    lands en el mismo paquete Go que tenia en el binario original (esa
     ruta se saca de `go.funcs.tsv`, ver `mapa_go.py`).

  2. Si NO se puede usar `protoc-gen-go` (sin red, o `--sin-plugin`), se
     usa el generador propio de este repositorio (`generador_go_propio.py`),
     que escribe structs con las etiquetas `protobuf:"..."`, los enums con
     su mapa de valores y el `RawDesc` con los bytes del descriptor en
     hexadecimal.

MEDIDAS que se escriben al final: nº de ficheros .go, líneas, bytes y el
resultado de la validación sintáctica (`validar_go.py`).

USO:
    python3 generar_go.py --protos protos/ --out go/ \
        --go-funcs /workspace/agy-re/tools-out/go.funcs.tsv
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mapa_go import construir_mapa, leer_mapa  # noqa: E402

RELEASES = {
    "linux-amd64": "protoc-gen-go.v{ver}.linux.amd64.tar.gz",
    "linux-arm64": "protoc-gen-go.v{ver}.linux.arm64.tar.gz",
    "darwin-amd64": "protoc-gen-go.v{ver}.darwin.amd64.tar.gz",
    "darwin-arm64": "protoc-gen-go.v{ver}.darwin.arm64.tar.gz",
}


def plataforma():
    import platform
    s = platform.system().lower()
    m = platform.machine().lower()
    if s not in ("linux", "darwin"):
        return None
    if m in ("x86_64", "amd64"):
        return f"{s}-amd64"
    if m in ("aarch64", "arm64"):
        return f"{s}-arm64"
    return None


def ultima_version_go():
    url = "https://api.github.com/repos/protocolbuffers/protobuf-go/releases/latest"
    with urllib.request.urlopen(url, timeout=60) as fh:
        datos = json.load(fh)
    return datos["tag_name"].lstrip("v")


def descargar_plugin(destino):
    """Descarga protoc-gen-go de GitHub.  Devuelve la ruta o None."""
    plat = plataforma()
    if plat is None:
        return None
    ver = ultima_version_go()
    nombre = RELEASES[plat].format(ver=ver)
    url = f"https://github.com/protocolbuffers/protobuf-go/releases/download/v{ver}/{nombre}"
    tmp = os.path.join(destino, nombre)
    os.makedirs(destino, exist_ok=True)
    with urllib.request.urlopen(url, timeout=180) as fh, open(tmp, "wb") as out:
        shutil.copyfileobj(fh, out)
    with tarfile.open(tmp) as tf:
        tf.extractall(destino)
    os.remove(tmp)
    binario = os.path.join(destino, "protoc-gen-go")
    os.chmod(binario, 0o755)
    return binario


# El shim NO declara `google.protobuf.XxxOptions` (eso chocaria con el
# descriptor.proto INTERNO de protoc y haria irresolubles las opciones
# `java_package` y demas).  En su lugar se cambia el `extendee` al mensaje
# real del descriptor de google.protobuf, que protoc lleva siempre dentro.
IMPORT_DESCRIPTOR_BUILTIN = 'import "google/protobuf/descriptor.proto";'


def preparar_arbol_go(protos_dir, dest):
    """Copia el arbol de .proto y adapts las extensiones proto3.

    MEDIDO: hay 107 extensiones declaradas en ficheros proto3 cuyo
    `extendee` es `proto2.XxxOptions` (el descriptor proto2 interno de
    Google).  `protoc` las acepta, pero `protoc-gen-go` (el publico) exige
    que en proto3 solo se extiendan mensajes de opciones de
    `google.protobuf`.

    Aqui se reescriben esos `extendee` a `google.protobuf.XxxOptions` y se
    cambia el import del descriptor por el `google/protobuf/descriptor.proto`
    QUE TRAE PROTOC CONSTRUIDO (no lo declaramos nosotros, asi que no
    chocamos con el).

    NO se toca el arbol de `.proto` fiel: esta es una copia derivada, solo
    para el paso de generacion de Go.

    Devuelve (ruta_del_arbol, ficheros_reescritos).
    """
    if os.path.exists(dest):
        shutil.rmtree(dest)
    shutil.copytree(protos_dir, dest)
    reescritos = 0
    for dirpath, _d, filenames in os.walk(dest):
        for fn in filenames:
            if not fn.endswith(".proto"):
                continue
            ruta = os.path.join(dirpath, fn)
            with open(ruta, "r", encoding="utf-8") as fh:
                txt = fh.read()
            # `message_set_wire_format` (proto1) no lo soporta protoc-gen-go
            txt2 = re.sub(r"^\s*option message_set_wire_format = true;\n", "",
                          txt, flags=re.M)
            if 'syntax = "proto3"' in txt and "extend proto2." in txt2:
                txt2 = re.sub(r"extend proto2\.(\w+Options)",
                              r"extend google.protobuf.\1", txt2)
                if IMPORT_DESCRIPTOR_BUILTIN not in txt2:
                    txt2 = txt2.replace(
                        'import "net/proto2/proto/descriptor.proto";',
                        'import "net/proto2/proto/descriptor.proto";\n'
                        + IMPORT_DESCRIPTOR_BUILTIN)
            if txt2 == txt:
                continue
            if IMPORT_DESCRIPTOR_BUILTIN not in txt2:
                txt2 = txt2.replace(
                    'import "net/proto2/proto/descriptor.proto";',
                    'import "net/proto2/proto/descriptor.proto";\n'
                    + IMPORT_DESCRIPTOR_BUILTIN)
            with open(ruta, "w", encoding="utf-8") as fh:
                fh.write(txt2)
            reescritos += 1
    return dest, reescritos


def listar_protos(raiz):
    out = []
    for dirpath, _d, filenames in os.walk(raiz):
        for fn in filenames:
            if fn.endswith(".proto"):
                out.append(os.path.relpath(os.path.join(dirpath, fn), raiz))
    out.sort()
    return out


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--protos", required=True)
    ap.add_argument("--out", required=True, help="directorio de salida del .go")
    ap.add_argument("--go-funcs", default=None,
                    help="go.funcs.tsv para saber el paquete Go de cada .proto")
    ap.add_argument("--plugin", default=None, help="ruta a protoc-gen-go")
    ap.add_argument("--plugin-dir", default=None,
                    help="directorio donde se deja el protoc-gen-go descargado")
    ap.add_argument("--sin-plugin", action="store_true",
                    help="usar el generador propio en vez de protoc-gen-go")
    ap.add_argument("--stats", default=None)
    ap.add_argument("--descriptor-set", default=None,
                    help="descriptor_set.pb (para el generador propio)")
    args = ap.parse_args(argv)

    protos = listar_protos(args.protos)
    os.makedirs(args.out, exist_ok=True)

    informe = {"ficheros_proto": len(protos), "motor": None}

    # --- mapa .proto -> ruta de importacion Go -----------------------------
    mapa = construir_mapa(protos, args.go_funcs)
    paquetes_medidos = leer_mapa(args.go_funcs) if args.go_funcs else {}
    medidos = sum(1 for p in protos if
                  ("blaze-out/arm-opt/bin/" + p[:-len(".proto")] + ".pb.go")
                  in paquetes_medidos)
    informe["paquetes_go_medidos"] = len(set(mapa.values()))
    informe["ficheros_con_paquete_go_medido"] = medidos

    plugin = None
    if not args.sin_plugin:
        plugin = args.plugin
        if plugin is None or not os.path.exists(plugin):
            destino = args.plugin_dir or os.path.join(args.out, "_plugin")
            try:
                plugin = descargar_plugin(destino)
            except Exception as exc:  # noqa: BLE001
                print(f"[aviso] no se pudo descargar protoc-gen-go: {exc}",
                      file=sys.stderr)
                plugin = None
        if plugin:
            informe["protoc_gen_go"] = plugin
            informe["motor"] = "protoc-gen-go"

    if plugin:
        arbol, reescritos = preparar_arbol_go(args.protos, os.path.join(args.out, "_arbol_go"))
        informe["ficheros_con_shim"] = reescritos
        protos_go = sorted(listar_protos(arbol))
        cmd = [sys.executable, "-m", "grpc_tools.protoc",
               "--plugin=protoc-gen-go=" + plugin,
               "--go_out=" + args.out,
               "--go_opt=paths=import"]
        for p in protos_go:
            cmd.append("--go_opt=M%s=%s" % (p, mapa.get(p, "google3/" + p[:-len(".proto")])))
        cmd += ["-I", arbol] + protos_go
        print("[go] protoc-gen-go sobre %d ficheros..." % len(protos))
        r = subprocess.run(cmd, capture_output=True, text=True)
        if r.returncode != 0:
            print(r.stdout[-4000:])
            print(r.stderr[-4000:], file=sys.stderr)
            print("[go] ERROR de protoc-gen-go", file=sys.stderr)
            informe["error_protoc_gen_go"] = (r.stderr or "")[-4000:]
            informe["motor"] = "fallo-protoc-gen-go"
        else:
            informe["aviso_protoc_gen_go"] = (r.stderr or "")[-2000:]
    else:
        informe["motor"] = "generador-propio"

    # --- generador propio (respaldo / auditoria) --------------------------
    if informe["motor"] != "protoc-gen-go":
        from generador_go_propio import generar_todo
        generar_todo(args.protos, mapa, args.out, args.descriptor_set)

    # --- medidas del .go generado -----------------------------------------
    n_go = 0
    lineas = 0
    bytes_go = 0
    for dirpath, _d, filenames in os.walk(args.out):
        for fn in filenames:
            if fn.endswith(".go"):
                n_go += 1
                with open(os.path.join(dirpath, fn), "rb") as fh:
                    datos = fh.read()
                bytes_go += len(datos)
                lineas += datos.count(b"\n")
    informe.update({"ficheros_go": n_go, "lineas_go": lineas, "bytes_go": bytes_go})

    if args.stats:
        with open(args.stats, "w", encoding="utf-8") as fh:
            json.dump(informe, fh, indent=2, ensure_ascii=False)
    print(json.dumps(informe, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
