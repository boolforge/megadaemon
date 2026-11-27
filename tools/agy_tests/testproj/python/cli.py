#!/usr/bin/env python3
"""cli.py — consume la lógica de testproj. Interfaz pública NO se cambia."""
import json
import subprocess
import sys


def rust_values(values):
    """Pide a Rust los valores normalizados vía un programa auxiliar."""
    prog = (
        'fn main(){let v:Vec<f64>=std::env::args().skip(1)'
        '.filter_map(|a|a.parse().ok()).collect();'
        'let o=testproj::normalize(&v);'
        'println!("{}",o.iter().map(|x|format!("{:.6}",x)).collect::<Vec<_>>().join(","));}'
    )
    src = f"{prog}\nfn main(){{}}"
    # se compila un binario auxiliar con rustc enlazando la lib
    with open("/tmp/_tp_bin.rs", "w") as fh:
        fh.write(prog)
    out = subprocess.run(
        ["rustc", "-O", "--edition", "2021", "/tmp/_tp_bin.rs",
         "-L", "target/debug", "-l", "testproj", "-o", "/tmp/_tp_bin"],
        capture_output=True, text=True)
    if out.returncode != 0:
        raise SystemExit("no se pudo compilar el auxiliar: " + out.stderr[-400:])
    run = subprocess.run(["/tmp/_tp_bin"] + [str(v) for v in values],
                         capture_output=True, text=True)
    if run.returncode != 0:
        raise SystemExit("fallo el auxiliar: " + run.stderr[-200:])
    return [float(x) for x in run.stdout.strip().split(",") if x]


def main(argv):
    if len(argv) < 2:
        print("uso: cli.py normalize <n1> <n2> ...", file=sys.stderr)
        return 2
    cmd = argv[1]
    if cmd == "normalize":
        vals = [float(a) for a in argv[2:]]
        print(json.dumps(rust_values(vals)))
        return 0
    print("comando desconocido: " + cmd, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
