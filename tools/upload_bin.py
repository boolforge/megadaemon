#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Sube un binario al dispositivo por trozos (fs_write base64) y verifica SHA-256."""
import base64
import hashlib
import json
import sys
import urllib.request

ENDPOINT = "https://lou-larry-telecharger-changed.trycloudflare.com/mcp"
TOKEN = "JxPRvM7GVyL3QILIu4vZnJ39_tnC9GqIhsjszZY5Enw"


def rpc(tool, args, _id=1, timeout=900):
    body = json.dumps({"jsonrpc": "2.0", "id": _id, "method": "tools/call",
                       "params": {"name": tool, "arguments": args}}).encode()
    req = urllib.request.Request(ENDPOINT, data=body)
    req.add_header("Authorization", "Bearer " + TOKEN)
    req.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read().decode())


def main():
    local, remote = sys.argv[1], sys.argv[2]
    chunk = 400 * 1024
    data = open(local, "rb").read()
    total = len(data)
    digest = hashlib.sha256(data).hexdigest()
    print("subiendo %d bytes (sha256 %s)" % (total, digest))
    parts = []
    for i, off in enumerate(range(0, total, chunk)):
        p = "/data/local/tmp/.mdpart_%02d" % i
        parts.append(p)
        b64 = base64.b64encode(data[off:off + chunk]).decode()
        res = rpc("fs_write", {"path": p, "content": b64, "base64": True, "chmod": "644"}, i + 1)
        txt = res["result"]["content"][0]["text"]
        print("  parte %d/%d: %s" % (i + 1, (total + chunk - 1) // chunk, txt[:60]))
    cmd = "cat %s > %s && chmod 755 %s && rm -f %s && sha256sum %s && ls -la %s" % (
        " ".join(parts), remote, remote, " ".join(parts), remote, remote)
    res = rpc("root_exec", {"command": cmd, "timeout": 180000, "max_chars": 4096}, 99)
    out = res["result"]["content"][0]["text"]
    print(out)
    ok = digest in out
    print("SHA256 MATCH" if ok else "SHA256 MISMATCH")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
