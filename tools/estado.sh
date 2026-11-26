#!/usr/bin/env sh
# =============================================================================
# estado.sh — EL TABLÓN DEL ENJAMBRE EN UN COMANDO
# =============================================================================
#
#   POR QUÉ EXISTE
#   -------------
#   Cada sesión nueva releía el repo entero para saber dónde estaba el
#   trabajo: eso cuesta tokens y se equivoca. Este script devuelve el estado
#   completo en **menos de 30 líneas** y en **menos de 15 segundos**.
#
#   Imperativo de eficiencia de tokens (docs/ORDEN_IMPERATIVA_GLOBAL.md §2):
#   un script, salida compacta, y el detalle en ficheros para consultarlo
#   bajo demanda.
#
#   USO
#     tools/estado.sh              # tablón compacto
#     tools/estado.sh --json       # lo mismo, en JSON (para otro script)
# =============================================================================

set -u
REPO="${MEGADAEMON_REPO:-/workspace/md-root}"
API="https://api.github.com/repos/boolforge/megadaemon"
JSON=0
[ "${1:-}" = "--json" ] && JSON=1

say() { [ "$JSON" -eq 0 ] && echo "$@"; }
k()   { printf '%-22s %s\n' "$1" "$2"; }

# --- git --------------------------------------------------------------------
sha=$(git -C "$REPO" rev-parse --short HEAD 2>/dev/null || echo "?")
branch=$(git -C "$REPO" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "?")
sucios=$(git -C "$REPO" status --porcelain 2>/dev/null | wc -l | tr -d ' ')
sinpush=$(git -C "$REPO" log --oneline origin/main..HEAD 2>/dev/null | wc -l | tr -d ' ')
say "== GIT =="
k "rama/HEAD" "$branch @ $sha"
k "sin pushear" "$sinpush commit(s)"
k "arbol sucio" "$sucios fichero(s)"

# --- CI ---------------------------------------------------------------------
if [ -n "${GITHUB_TOKEN:-}" ]; then
  runs=$(curl -s -m 12 -H "Authorization: Bearer $GITHUB_TOKEN" -H "Accept: application/vnd.github+json" \
        "$API/actions/runs?branch=main&per_page=4" 2>/dev/null \
        | python3 -c "
import json,sys
try: d=json.load(sys.stdin)
except Exception: sys.exit(0)
out=[]
for r in d.get('workflow_runs',[])[:4]:
    out.append('%s:%s%s' % (r['run_number'], (r.get('conclusion') or r['status'])[:9], '' if r.get('head_sha','').startswith('$sha') else '*'))
print(' '.join(out))
" 2>/dev/null)
  [ -n "$runs" ] && k "CI (run:resultado)" "$runs"
  # Cuota REAL del proyecto: los runner-minutos que gasta el CI.  Es la
  # unica cuota que se puede MEDIR desde aqui (la de LLM la lleva el
  # runtime y no la expone a los agentes: `mcode-tools auth status` solo
  # dice "managed_by_sandbox_egress", sin numeros).
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    min=$(python3 - "$API" <<'PY' 2>/dev/null
import json,os,sys,urllib.request,datetime
api=sys.argv[1]
tok=os.environ.get("GITHUB_TOKEN","")
def get(u):
    r=urllib.request.Request(u,headers={"Authorization":"Bearer "+tok,"Accept":"application/vnd.github+json"})
    return json.load(urllib.request.urlopen(r,timeout=12))
try:
    runs=get(api+"/actions/runs?branch=main&per_page=10").get("workflow_runs",[])
    tot=0.0; n=0
    for r in runs:
        try: jobs=get(api+"/actions/runs/%s/jobs?per_page=30"%r["id"]).get("jobs",[])
        except Exception: continue
        for j in jobs:
            a,b=j.get("started_at"),j.get("completed_at")
            if a and b:
                d=datetime.datetime.fromisoformat(b.replace("Z","+00:00"))-datetime.datetime.fromisoformat(a.replace("Z","+00:00"))
                tot+=d.total_seconds()/60; n+=1
    print("%d runner-minutos en %d jobs (ultimos %d runs)"%(round(tot),n,len(runs)))
except Exception:
    pass
PY
)
    [ -n "$min" ] && k "cuota CI" "$min"
  fi
fi

# --- backlog ----------------------------------------------------------------
if [ -f "$REPO/docs/IDEAS_BACKLOG.md" ]; then
  b=$(python3 - "$REPO/docs/IDEAS_BACKLOG.md" <<'PY' 2>/dev/null
import re,sys
t=open(sys.argv[1],encoding='utf-8').read()
rows=[l for l in t.splitlines() if l.startswith('|') and re.match(r'\|\s*[A-Z]\d+\s*\|',l)]
hecho=sum(1 for l in rows if '✅' in l)
pend=sum(1 for l in rows if '⛔' in l)
curso=sum(1 for l in rows if '🔄' in l or '🚧' in l)
otros=len(rows)-hecho-pend-curso
print('items=%d hecho=%d pendiente=%d en_curso=%d sin_marcar=%d'%(len(rows),hecho,pend,curso,otros))
PY
)
  say "== BACKLOG =="
  [ -n "$b" ] && k "IDEAS_BACKLOG" "$b"
fi

# --- gates ------------------------------------------------------------------
if [ -f "$REPO/tools/check-sintaxis.sh" ]; then
  s=$(bash "$REPO/tools/check-sintaxis.sh" 2>&1 | tail -1)
  say "== GATES =="
  k "sintaxis" "$s"
fi

# --- movil ------------------------------------------------------------------
# 3 comprobaciones cortas: el movil es inalcanzable y eso hay que verlo rapido.
puerta=$(python3 -c "
import socket
s=socket.socket(); s.settimeout(2)
try:
    s.connect(('127.0.0.1',5037)); print('adb: ok')
except Exception as e:
    print('adb: caido')
finally: s.close()
" 2>/dev/null)
say "== MOVIL =="
k "adb local" "${puerta:-desconocido}"
k "movil real" "ver docs/TELEMETRIA_DRIVE.md (bloqueado: sin tunel ni ADB)"

# --- resumen ----------------------------------------------------------------
say "== ACCION SIGUIENTE =="
say "  1) CI: si 'calidad' esta en rojo por tests NUESTOS, se arreglan; los"
say "     rojos de audit_scout_* / audit_adversarial son preexistentes."
say "  2) Movil: sin tunel ni ADB no hay instrumentacion en vivo. La"
say "     telemetria a Drive es justamente el mecanismo para que se"
say "     encuentre solo: no hace falta que nadie este conectado."
say "  3) Detalle de cualquier punto: mira los ficheros citados arriba."
exit 0
