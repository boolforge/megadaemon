#!/usr/bin/env python3
"""agentes_fuentes.py — descarga y clasifica EN MASA el código fuente de los
agentes del benchmark (docs/BENCHMARK_AGENTICO_59_PROYECTOS.md).

El "tubo": cada repo se clona (shallow), se clasifica, se sube a Drive
(privado) y se borra la copia local ⇒ el workspace no se llena (5 GB de
cuota).  Lo que queda aquí es el INVENTARIO (qué hay, de qué tipo, con qué
ficheros de contrato).

    python3 tools/agentes_fuentes.py --dest /workspace/md-re-productos/artifacts/agentes-fuente
    python3 tools/agentes_fuentes.py --sin-drive --repos a/b,c/d --dest /tmp/x

Método repetible (es el que usa el benchmark): API de GitHub + clones
shallow + clasificación por tipo de fichero y ficheros de contrato
(rules/skills/prompts/mcp/AGENTS.md).
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from drive_api import RAIZ_BACKUP, Drive  # noqa: E402

DOC = "docs/BENCHMARK_AGENTICO_59_PROYECTOS.md"
CARPETA_DRIVE = "agentes-fuente-codigo"
CONTRATO = re.compile(
    r"(^|/)(SKILL\.md|AGENTS?\.md|CLAUDE\.md|GEMINI\.md|CURSOR\.md"
    r"|copilot-instructions\.md|rules?\.(md|json|toml|yaml|yml)"
    r"|prompts?\.(md|json)|mcp[^/]*\.(json|md|toml)|settings\.(json|toml)"
    r"|Cargo\.toml|package\.json|pyproject\.toml)$", re.I)
EXT = {".rs": "Rust", ".py": "Python", ".ts": "TypeScript", ".tsx": "TypeScript",
       ".js": "JavaScript", ".go": "Go", ".java": "Java", ".kt": "Kotlin",
       ".c": "C", ".h": "C/C++", ".cpp": "C/C++", ".swift": "Swift",
       ".md": "Markdown", ".json": "JSON", ".toml": "Config", ".yaml": "Config",
       ".yml": "Config", ".sh": "Shell", ".lua": "Lua", ".rb": "Ruby"}


def api(url: str, token: str = ""):
    cab = {"Accept": "application/vnd.github+json",
           "User-Agent": "megadaemon-agentes-fuente",
           "X-GitHub-Api-Version": "2022-11-28"}
    if token:
        cab["Authorization"] = f"Bearer {token}"
    try:
        req = urllib.request.Request(url, headers=cab)
        with urllib.request.urlopen(req, timeout=45) as r:
            return json.load(r)
    except Exception:  # noqa: BLE001
        return None


def du(p: str) -> int:
    try:
        return int(subprocess.run(["du", "-sb", p], capture_output=True, text=True,
                                  timeout=180).stdout.split()[0])
    except Exception:  # noqa: BLE001
        return 0


def clasificar(dest: str):
    tipos, contrato, lineas, n = {}, [], {}, 0
    for raiz, dirs, files in os.walk(dest):
        dirs[:] = [d for d in dirs if d != ".git"]
        for f in files:
            p = os.path.join(raiz, f)
            rel = os.path.relpath(p, dest)
            n += 1
            lang = EXT.get(os.path.splitext(f)[1].lower())
            if lang:
                tipos[lang] = tipos.get(lang, 0) + 1
                try:
                    with open(p, "rb") as fh:
                        lineas[lang] = lineas.get(lang, 0) + sum(1 for _ in fh)
                except Exception:  # noqa: BLE001
                    pass
            try:
                if CONTRATO.search(rel) and os.path.getsize(p) < 400_000:
                    contrato.append(rel)
            except OSError:
                pass
    return n, tipos, sorted(contrato), lineas


def top(t: dict, k: int = 4) -> str:
    return ", ".join(f"{a}:{b}" for a, b in sorted(t.items(), key=lambda x: -x[1])[:k])


def repos_del_doc(doc: str):
    if not os.path.exists(doc):
        return []
    out = []
    for m in re.finditer(r"^\|\s*\*\*([A-Za-z0-9_.\-]+/[A-Za-z0-9_.\-]+)\*\*\s*\|",
                         open(doc, encoding="utf-8").read(), re.M):
        if m.group(1) not in out:
            out.append(m.group(1))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dest", default="/workspace/md-re-productos/artifacts/agentes-fuente")
    ap.add_argument("--doc", default=DOC)
    ap.add_argument("--repos", default="", help="lista explícita repo1,repo2")
    ap.add_argument("--sin-drive", action="store_true", help="no sube a Drive")
    ap.add_argument("--drive-parent", default=RAIZ_BACKUP)
    ap.add_argument("--creds", default="/workspace/md-secrets/gdrive_credentials.json")
    ap.add_argument("--max-kb", type=int, default=1_500_000)
    ap.add_argument("--max-total-mb", type=int, default=3000)
    args = ap.parse_args()

    token = os.environ.get("GITHUB_TOKEN", "")
    rs = ([r.strip() for r in args.repos.split(",") if r.strip()]
          or repos_del_doc(os.path.join(os.path.dirname(os.path.dirname(
              os.path.abspath(__file__))), args.doc)))
    os.makedirs(args.dest, exist_ok=True)
    drive = None
    if not args.sin_drive:
        try:
            drive = Drive(creds=args.creds)
            print(f"drive: listo (token sha16 {drive.huella_token()})", flush=True)
        except Exception as e:  # noqa: BLE001
            print(f"drive: NO disponible ({e}); sigo sin subir", flush=True)
            drive = None

    filas, gastado = [], 0
    for i, repo in enumerate(rs, 1):
        dest = os.path.join(args.dest, repo.replace("/", "__"))
        meta = api(f"https://api.github.com/repos/{repo}", token)
        if not meta:
            filas.append(f"{repo}\tERROR_API\t0\t0\t")
            print(f"[{i}/{len(rs)}] {repo}: ERROR de API", flush=True)
            continue
        kb = meta.get("size") or 0
        if kb > args.max_kb:
            filas.append(f"{repo}\tOMITIDO_{kb}KB\t0\t0\t")
            print(f"[{i}/{len(rs)}] {repo}: OMITIDO ({kb} KB)", flush=True)
            continue
        if not os.path.isdir(os.path.join(dest, ".git")):
            shutil.rmtree(dest, ignore_errors=True)
            r = subprocess.run(["git", "clone", "--depth", "1", "--single-branch", "--quiet",
                                f"https://github.com/{repo}.git", dest],
                               capture_output=True, text=True, timeout=1200)
            if r.returncode != 0:
                filas.append(f"{repo}\tCLONE_ERROR\t0\t0\t")
                print(f"[{i}/{len(rs)}] {repo}: CLONE ERROR", flush=True)
                continue
        sz = du(dest)
        nfiles, tipos, contrato, lineas = clasificar(dest)
        estado = f"OK {sz // 1024} KB {nfiles} ficheros"
        if drive is not None:
            try:
                carpeta = drive.crear_carpeta(CARPETA_DRIVE, args.drive_parent)
                info = drive.subir_carpeta(dest, carpeta, repo.replace("/", "__"))
                estado = (f"TUBO drive/{CARPETA_DRIVE}/{repo.replace('/', '__')}.tar.gz "
                          f"{info.get('bytes', 0)} B sha256={info.get('sha256', '')[:16]} "
                          f"id={info.get('id')}")
                shutil.rmtree(dest, ignore_errors=True)
                gastado -= sz
            except Exception as e:  # noqa: BLE001
                estado += f" | TUBO_ERROR {e}"
        filas.append(f"{repo}\t{estado}\t{len(contrato)}\t{top(tipos)}\t"
                     f"{'|'.join(contrato[:12])}")
        print(f"[{i}/{len(rs)}] {repo}: {estado} | contrato={len(contrato)} | "
              f"{top(tipos, 3)}", flush=True)
        if gastado > args.max_total_mb * 1024 * 1024:
            print("tope local alcanzado: paro", flush=True)
            break
    inv = os.path.join(args.dest, "INVENTARIO_AGENTES.tsv")
    with open(inv, "w", encoding="utf-8") as fh:
        fh.write("repo\testado\tficheros_contrato\ttipos_top\tcontratos\n")
        fh.write("\n".join(filas) + "\n")
    print(f"FIN: {len(filas)} repos -> {inv}", flush=True)
    if drive is not None:
        try:
            carpeta = drive.crear_carpeta(CARPETA_DRIVE, args.drive_parent)
            drive.subir(inv, carpeta, "INVENTARIO_AGENTES.tsv")
            print("inventario subido a Drive", flush=True)
        except Exception as e:  # noqa: BLE001
            print(f"inventario NO subido: {e}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
