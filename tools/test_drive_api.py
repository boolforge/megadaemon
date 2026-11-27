#!/usr/bin/env python3
"""Tests de tools/drive_api.py (sin red: servidor de Drive falso).

    python3 tools/test_drive_api.py

Cubren el tubo: token (nunca impreso), credenciales 0600, listar, crear
carpeta, subir/bajar (ida y vuelta byte a byte) y subir una carpeta.
"""
import json
import os
import sys
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, HTTPServer

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from drive_api import Drive, DriveError, fingerprint_fp1a, sha16  # noqa: E402

ALMACEN = {"ficheros": {}, "carpetas": {"RAIZ": ["HIJO"]}}
CONTENT = {}


class Manejador(BaseHTTPRequestHandler):
    def log_message(self, *a):  # noqa: D102 - silencio en los tests
        pass

    def _json(self, code, obj):
        cuerpo = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(cuerpo)))
        self.end_headers()
        self.wfile.write(cuerpo)

    def do_POST(self):  # noqa: N802
        if self.path.startswith("/token"):
            self._json(200, {"access_token": "TOKEN_SECRETO_DE_PRUEBA"})
            return
        n = int(self.headers.get("Content-Length", 0))
        cuerpo = self.rfile.read(n)
        if self.path.startswith("/upload"):
            partes = cuerpo.split(b"--megaagentboundary")
            meta = json.loads(partes[1].split(b"\r\n\r\n", 1)[1].split(b"\r\n--")[0])
            datos = partes[2].split(b"\r\n\r\n", 1)[1].rsplit(b"\r\n--", 1)[0]
            if datos.endswith(b"\r\n"):
                datos = datos[:-2]
            fid = "F_" + meta["name"]
            ALMACEN["ficheros"][fid] = {"name": meta["name"], "size": len(datos),
                                        "parents": meta.get("parents", []),
                                        "mimeType": "application/octet-stream"}
            CONTENT[fid] = datos
            self._json(200, {"id": fid, "name": meta["name"], "size": len(datos)})
            return
        if self.path.startswith("/drive/v3/files"):
            meta = json.loads(cuerpo)
            fid = "C_" + meta["name"]
            ALMACEN["carpetas"].setdefault(fid, [])
            self._json(200, {"id": fid, "name": meta["name"]})
            return
        self._json(404, {})

    def do_GET(self):  # noqa: N802
        if self.path.startswith("/drive/v3/files/"):
            fid = self.path.split("/drive/v3/files/", 1)[1].split("?")[0]
            if fid in CONTENT:
                datos = CONTENT[fid]
                self.send_response(200)
                self.send_header("Content-Length", str(len(datos)))
                self.end_headers()
                self.wfile.write(datos)
                return
            self._json(404, {})
            return
        # listar
        files = []
        for fid, f in ALMACEN["ficheros"].items():
            if "RAIZ" in f.get("parents", []):
                files.append({"id": fid, "name": f["name"], "size": str(f["size"]),
                              "mimeType": f["mimeType"], "modifiedTime": "2026-11-27T00:00:00Z"})
        self._json(200, {"files": files})


class TestDriveApi(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.srv = HTTPServer(("127.0.0.1", 0), Manejador)
        cls.url = f"http://127.0.0.1:{cls.srv.server_port}"
        cls.hilo = threading.Thread(target=cls.srv.serve_forever, daemon=True)
        cls.hilo.start()
        cls.tmp = tempfile.mkdtemp()
        cls.creds = os.path.join(cls.tmp, "creds.json")
        with open(cls.creds, "w", encoding="utf-8") as fh:
            json.dump({"client_id": "c", "client_secret": "s", "refresh_token": "r",
                       "type": "authorized_user"}, fh)
        os.chmod(cls.creds, 0o600)

    @classmethod
    def tearDownClass(cls):
        cls.srv.shutdown()

    def drive(self):
        return Drive(creds=self.creds, api_base=f"{self.url}/drive/v3",
                     upload_base=f"{self.url}/upload/drive/v3",
                     token_url=f"{self.url}/token")

    def test_el_token_no_se_imprime_nunca(self):
        d = self.drive()
        d.token()
        self.assertNotIn("TOKEN_SECRETO", repr(d))
        self.assertNotIn("TOKEN_SECRETO", repr(d))
        self.assertEqual(len(d.huella_token()), 16)
        self.assertEqual(d.huella_token(), sha16(b"TOKEN_SECRETO_DE_PRUEBA"))

    def test_el_cli_no_imprime_el_token(self):
        import subprocess
        d = self.drive()
        d.subir(self._fichero("hola2.txt", b"hola"), "RAIZ")
        r = subprocess.run(
            [sys.executable, os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                          "drive_tube.py"),
             "--creds", self.creds, "--api-base", f"{self.url}/drive/v3",
             "--upload-base", f"{self.url}/upload/drive/v3",
             "--token-url", f"{self.url}/token", "listar"],
            capture_output=True, text=True, timeout=120)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("hola2.txt", r.stdout)
        self.assertNotIn("TOKEN_SECRETO", r.stdout + r.stderr)

    def test_credenciales_deben_ser_0600(self):
        p = os.path.join(self.tmp, "abiertas.json")
        with open(p, "w", encoding="utf-8") as fh:
            json.dump({"client_id": "c", "client_secret": "s", "refresh_token": "r"}, fh)
        os.chmod(p, 0o644)
        with self.assertRaises(DriveError) as ctx:
            Drive(creds=p).token()
        self.assertIn("0600", str(ctx.exception))

    def test_listar(self):
        d = self.drive()
        d.subir(self._fichero("hola.txt", b"hola"), "RAIZ")
        nombres = [f["name"] for f in d.listar("RAIZ")]
        self.assertIn("hola.txt", nombres)

    def test_crear_carpeta_es_idempotente(self):
        d = self.drive()
        a = d.crear_carpeta("cosas", "RAIZ")
        b = d.crear_carpeta("cosas", "RAIZ")
        self.assertEqual(a, b)

    def test_subir_y_bajar_ida_y_vuelta(self):
        d = self.drive()
        datos = b"contenido\x00binario\xff" * 1000
        info = d.subir(self._fichero("datos.bin", datos), "RAIZ")
        destino = os.path.join(self.tmp, "bajado.bin")
        n = d.descargar(info["id"], destino)
        self.assertEqual(n, len(datos))
        with open(destino, "rb") as fh:
            self.assertEqual(fh.read(), datos)

    def test_subir_carpeta_devuelve_sha256(self):
        d = self.drive()
        carpeta = os.path.join(self.tmp, "codigo")
        os.makedirs(os.path.join(carpeta, "sub"), exist_ok=True)
        with open(os.path.join(carpeta, "sub", "a.rs"), "w", encoding="utf-8") as fh:
            fh.write("fn main() {}\n")
        info = d.subir_carpeta(carpeta, "RAIZ", "codigo")
        self.assertEqual(len(info["sha256"]), 64)
        self.assertIn(".tar.gz", info["name"])

    def test_huella_fp1a_conocida(self):
        # MEDIDO: vector SINTETICO (no es ninguna clave real).  La huella
        # esperada se calculó con una implementacion FNV-1a 64 independiente
        # en C (offset basis 0xCBF29CE484222325, prime 0x100000001B3) y
        # coincide byte a byte con la de esta funcion de Python:
        #   echo -n "mdaemon_sk_test_0123456789abcdef0123456789abcdef" | fnv1a64
        #   -> ea37d383e9cbf9b9
        self.assertEqual(
            fingerprint_fp1a("mdaemon_sk_test_0123456789abcdef0123456789abcdef"),
            "ea37d383e9cbf9b9")

    def _fichero(self, nombre, datos):
        p = os.path.join(self.tmp, nombre)
        with open(p, "wb") as fh:
            fh.write(datos)
        return p


if __name__ == "__main__":
    unittest.main(verbosity=2)
