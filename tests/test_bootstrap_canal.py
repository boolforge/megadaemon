#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Pruebas deterministas (sin red) de tools/bootstrap_canal.py.

Se ejecutan:
    python3 tests/test_bootstrap_canal.py
    python3 tools/bootstrap_canal.py --selftest
"""
import json
import os
import sys
import unittest

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(RAIZ, "tools"))

import bootstrap_canal as bc  # noqa: E402


class TestParseHost(unittest.TestCase):
    def test_con_esquema(self):
        self.assertEqual(bc.parse_host("https://mdtmfnc.duckdns.org/channel.json"),
                         "mdtmfnc.duckdns.org")

    def test_sin_esquema(self):
        self.assertEqual(bc.parse_host("mdtmfnc.duckdns.org"), "mdtmfnc.duckdns.org")

    def test_con_puerto(self):
        self.assertEqual(bc.parse_host("http://192.168.1.124:9090/health"), "192.168.1.124")

    def test_vacio_o_none(self):
        self.assertEqual(bc.parse_host(""), "")
        self.assertEqual(bc.parse_host(None), "")

    def test_mayusculas(self):
        self.assertEqual(bc.parse_host("HTTPS://MDTMFNC.DUCKDNS.ORG/x"), "mdtmfnc.duckdns.org")


class TestParseSha256(unittest.TestCase):
    def test_valido(self):
        h = "a" * 64
        self.assertEqual(bc.parse_sha256_text(f"{h}  megadaemon-core-android-arm64\n"), h)

    def test_solo_hash(self):
        h = "b" * 64
        self.assertEqual(bc.parse_sha256_text(h), h)

    def test_invalido(self):
        self.assertEqual(bc.parse_sha256_text("no es un hash"), "")
        self.assertEqual(bc.parse_sha256_text(""), "")
        self.assertEqual(bc.parse_sha256_text("z" * 64), "")

    def test_corto(self):
        self.assertEqual(bc.parse_sha256_text("abc123"), "")


class TestNormaliseDomain(unittest.TestCase):
    def test_dominio_obsoleto(self):
        st = {"duckdns": "https://mdtmfnc-s21.duckdns.org"}
        out = bc.normalise_domain(st, "mdtmfnc.duckdns.org")
        self.assertEqual(out["duckdns"], "https://mdtmfnc.duckdns.org")
        self.assertTrue(out.get("_dominio_normalizado"))
        self.assertIn("nota", out)

    def test_canonical_obsoleto(self):
        st = {"canonical": "https://mdtmfnc-s21.duckdns.org/channel.json"}
        out = bc.normalise_domain(st, "mdtmfnc.duckdns.org")
        self.assertEqual(out["canonical"], "https://mdtmfnc.duckdns.org/channel.json")
        self.assertTrue(out.get("_dominio_normalizado"))

    def test_respaldo_legitimo_intacto(self):
        st = {"canonical": "https://raw.githubusercontent.com/boolforge/megadaemon/main/state/channel.json",
              "duckdns": "https://mdtmfnc.duckdns.org"}
        out = bc.normalise_domain(st, "mdtmfnc.duckdns.org")
        self.assertEqual(out["canonical"],
                         "https://raw.githubusercontent.com/boolforge/megadaemon/main/state/channel.json")
        self.assertNotIn("_dominio_normalizado", out)

    def test_ya_correcto(self):
        st = {"duckdns": "https://mdtmfnc.duckdns.org"}
        out = bc.normalise_domain(st, "mdtmfnc.duckdns.org")
        self.assertNotIn("_dominio_normalizado", out)

    def test_sin_campos(self):
        out = bc.normalise_domain({"quick": "https://x.trycloudflare.com"}, "mdtmfnc.duckdns.org")
        self.assertNotIn("_dominio_normalizado", out)

    def test_no_dict(self):
        self.assertEqual(bc.normalise_domain(None, "x"), None)

    def test_dominio_alterno_del_usuario(self):
        out = bc.normalise_domain({"duckdns": "https://otro.duckdns.org"}, "otro.duckdns.org")
        self.assertEqual(out["duckdns"], "https://otro.duckdns.org")
        self.assertNotIn("_dominio_normalizado", out)


class TestSha16(unittest.TestCase):
    def test_estable_y_corto(self):
        a = bc.sha16("mdaemon_sk_live_abc")
        self.assertEqual(len(a), 16)
        self.assertEqual(a, bc.sha16("mdaemon_sk_live_abc"))
        self.assertNotEqual(a, bc.sha16("mdaemon_sk_live_abd"))


class TestPickFirstFile(unittest.TestCase):
    def test_elige_el_que_existe(self):
        import tempfile
        d = tempfile.mkdtemp()
        p = os.path.join(d, "t")
        open(p, "w").close()
        self.assertEqual(bc.pick_first_file(["/no/existe", p]), p)
        self.assertEqual(bc.pick_first_file(["/no/existe"]), "")


class TestReport(unittest.TestCase):
    def test_cuenta_fallos(self):
        r = bc.Report()
        r.add("a", bc.OK, "bien")
        r.add("b", bc.WARN, "aviso")
        r.add("c", bc.KO, "mal")
        d = r.to_dict()
        self.assertFalse(d["ok"])
        self.assertEqual(d["fallos"], 1)
        self.assertEqual(d["avisos"], 1)
        self.assertEqual(len(d["checks"]), 3)

    def test_sin_fallos_es_ok(self):
        r = bc.Report()
        r.add("a", bc.OK, "bien")
        r.add("b", bc.SKIP, "n/a")
        self.assertTrue(r.to_dict()["ok"])
        self.assertEqual(r.to_dict()["fallos"], 0)


class TestRedMockeada(unittest.TestCase):
    """El sondeo del canal con la red simulada: 200 con payload, 000 sin red."""

    def setUp(self):
        self._orig = bc.http_get

    def tearDown(self):
        bc.http_get = self._orig

    def test_payload_vivo(self):
        payload = json.dumps({"duckdns": "https://mdtmfnc.duckdns.org",
                              "quick": "https://a.trycloudflare.com",
                              "ts": "2026-01-01T00:00:00Z"})

        def fake(url, timeout=10, headers=None, data=None):
            if url.endswith("/channel.json"):
                return 200, payload
            if "/mcp" in url:
                return 401, "sin credencial"
            return 0, ""

        bc.http_get = fake
        rep = bc.Report()
        bc.check_canal("mdtmfnc.duckdns.org", rep)
        estados = {c["name"]: c["status"] for c in rep.checks}
        self.assertEqual(estados.get("canal"), bc.OK)
        detalles = " ".join(c["detail"] for c in rep.checks)
        self.assertIn("quick", detalles)

    def test_sin_quick(self):
        payload = json.dumps({"duckdns": "https://mdtmfnc.duckdns.org", "ts": "x"})

        def fake(url, timeout=10, headers=None, data=None):
            return (200, payload) if url.endswith("/channel.json") else (0, "")

        bc.http_get = fake
        rep = bc.Report()
        bc.check_canal("mdtmfnc.duckdns.org", rep)
        self.assertTrue(any(c["status"] == bc.KO for c in rep.checks))

    def test_sin_respuesta(self):
        bc.http_get = lambda *a, **k: (0, "")
        rep = bc.Report()
        bc.check_canal("mdtmfnc.duckdns.org", rep)
        self.assertTrue(any(c["status"] == bc.KO for c in rep.checks))


class TestCli(unittest.TestCase):
    def test_help(self):
        with self.assertRaises(SystemExit) as ctx:
            bc.build_parser().parse_args(["--help"])
        self.assertEqual(ctx.exception.code, 0)

    def test_defaults(self):
        a = bc.build_parser().parse_args([])
        self.assertFalse(a.apply)
        self.assertFalse(a.selftest)
        self.assertEqual(a.dominio, bc.DEFAULT_DOMAIN)

    def test_flags(self):
        a = bc.build_parser().parse_args(["--apply", "--json", "--sin-movil"])
        self.assertTrue(a.apply)
        self.assertTrue(a.json)
        self.assertTrue(a.sin_movil)


if __name__ == "__main__":
    unittest.main(verbosity=2)
