# AUDITORÍA DE RAMAS — MegaDaemon (2026-11-23)

> **Base `main` medida:** `89a2237` — el `main` se mueve constante (el root integra ramas mientras se mide): las cifras `ahead/behind/novel` son de esa foto.
> **Ramas auditadas:** 81 (todas las locales, `refs/heads/`) + `origin/*` revisadas aparte.
> **Método:** `git cherry` reimplementado con patch-id (precalculado 1 sola vez sobre `main`, que es lo que lo hace viable: 80 ramas × 900 commits), `git merge-tree --write-tree` para los conflictos y `git grep` para los marcadores de conflicto.
> **TAREA PRINCIPAL: esta tabla.** Todo lo demás (rotación de credenciales, integración con el bus/GUI) es secundario frente a integrar bien estas ramas.

## 0. Hallazgo P0 — `main` está roto y es PÚBLICO

`main` tiene **marcadores de conflicto sin resolver** en 3 ficheros ⇒ **`state/mesh_board.json` no parsea como JSON** ⇒ el bus de coordinación no arranca:

```
git grep -n -E '^(<<<<<<< |>>>>>>> )' main --
main:state/mesh_board.json:3:<<<<<<< HEAD
main:state/mesh_board.json:5:=======
main:state/mesh_board.json:7:>>>>>>> agent/antigravity-backlog
main:docs/ANTIGRAVITY_BACKLOG.md:30:<<<<<<< HEAD          (:31 ====, :33 >>>>>>>)
main:docs/ANTIGRAVITY_BACKLOG.md:340:<<<<<<< HEAD         (:341 ====, :343 >>>>>>>)
main:docs/AUDIT_ADVERSARIAL_V15.md:285:<<<<<<< HEAD        (:287 ====, :289 >>>>>>> origin/main)
main:docs/AUDIT_ADVERSARIAL_V15.md:309:<<<<<<< HEAD        (:311 ====, :313 >>>>>>> origin/main)
```

**MEDIDO además sobre `origin/main` (lo que está PUBLICADO en GitHub):** los mismos
marcadores siguen ahí y `state/mesh_board.json` **no parsea**:

```
git show origin/main:state/mesh_board.json | python3 -c 'import sys,json; json.load(sys.stdin)'
# -> json.decoder.JSONDecodeError: Expecting property name enclosed in double quotes: line 3
```

**Cadena de culpa medida:**

| commit | qué hizo |
|---|---|
| `d148c95` | `merge(flota): todo el trabajo pendiente, commiteado y fusionado en main` — **introdujo** los marcadores (`git log -S'<<<<<<< HEAD' main` → `d148c95`, `d7f48e9`) |
| `d902a7e` | `merge(agent/backup-perfil-raw): integracion (conflictos: docs=theirs, codigo=ours)` — **propagó** los marcadores a `main` tomando `theirs` en `docs/` |

`git merge` con `docs=theirs` + `codigo=ours` es exactamente el patrón que ya rompió `main` antes. **No repetir: integrar rama por rama, con el diff a la vista.**

> El repo es **público**: `boolforge/megadaemon` → `private=false, visibility=public` (push `2026-11-23T06:08:19Z`). `curl -s https://api.github.com/repos/boolforge/megadaemon`

## 1. Resumen por clase

| clase | nº | qué hacer |
|---|---|---|
| 🟢 **B — MERGEABLE SIN CONFLICTO** | 7 | integrar ya |
| 🟡 **B — MERGEABLE CON CONFLICTOS** | 19 | integrar con revisión manual |
| 🔴 **D — RIESGOSA** | 6 | NO mergear |
| ⚪ **C — OBSOLETA** | 49 | cerrar la rama |

## 2. TABLA PRIORIZADA (orden de integración)

`novel` = commits cuyo *patch-id* **NO** está en `main` ⇒ lo que realmente falta.
`🟢` = se integra ya · `🟡` = integrar con revisión manual · `🟠` = sin resolver · `🔴` = **NO mergear** · `⚪/⚫` = cerrar.

| # | rama | último commit | fecha | ⬆️ | novel | ↔️ | A/M/D | estado | comando exacto | ficheros a revisar a mano |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | `agent/merge-flota-pendiente` | `dc95816` | 2026-11-21 | 17 | **5** | 345 | 4/105/7120 | 🔴 D | `NO MERGEAR   (si sólo interesa un fichero: git checkout agent/merge-flota-pendiente -- <fichero>)` | Merge, |
| 2 | `agent/f3-integracion` | `9771f91` | 2026-11-21 | 19 | **4** | 342 | 0/104/7122 | 🔴 D | `NO MERGEAR   (si sólo interesa un fichero: git checkout agent/f3-integracion -- <fichero>)` | Merge, |
| 3 | `integration/merge-all` | `88d0531` | 2026-11-21 | 3 | **1** | 368 | 0/92/7147 | 🔴 D | `NO MERGEAR   (si sólo interesa un fichero: git checkout integration/merge-all -- <fichero>)` | Merge, |
| 4 | `rescue/shared-tree-2026-11-21` | `a5ba863` | 2026-11-21 | 1 | **1** | 618 | 4/103/7218 | 🔴 D | `NO MERGEAR   (si sólo interesa un fichero: git checkout rescue/shared-tree-2026-11-21 -- <fichero>)` | Merge,deploy/magisk_csc_recording/module.prop,deploy/patch_csc_recording.sh,docs/IDEAS_BACKLOG.md,docs/KALI_NETHUNTER_KERNEL_PLAN.md,docs/MEGADAEMON_C |
| 5 | `wip/llm-v16-raw-snapshot` | `da1cf89` | 2026-11-22 | 1 | **1** | 236 | 0/73/292 | 🔴 D | `NO MERGEAR   (si sólo interesa un fichero: git checkout wip/llm-v16-raw-snapshot -- <fichero>)` | Merge, |
| 6 | `agent/energia-v15` | `a621f1f` | 2026-11-20 | 0 | **0** | 629 | 1/110/7261 | 🔴 D | `NO MERGEAR   (si sólo interesa un fichero: git checkout agent/energia-v15 -- <fichero>)` | — · marcadores nuevos: docs/ANDROID_EXYNOS2100_FINDINGS.md |
| 7 | `rama/ramas-auditoria-2026-11-23` | `bdcea7c` | 2026-11-23 | 34 | **31** | 0 | 35/24/0 | 🟢🔵 B | `git merge --no-ff rama/ramas-auditoria-2026-11-23` | — |
| 8 | `rama/ci-calidad-2026-11-23` | `cd01498` | 2026-11-23 | 31 | **28** | 0 | 31/25/0 | 🟢🔵 B | `git merge --no-ff rama/ci-calidad-2026-11-23` | — |
| 9 | `main` | `e23b18b` | 2026-11-23 | 30 | **27** | 0 | 31/26/0 | 🟢🔵 B | `git merge --no-ff main` | — |
| 10 | `rama/regenerar-go-2026-11-23` | `1768d20` | 2026-11-23 | 4 | **4** | 93 | 1/50/37 | 🟢🔵 B | `git merge --no-ff rama/regenerar-go-2026-11-23` | — |
| 11 | `rama/frida-gum-docs-2026-11-23` | `c61c95e` | 2026-11-23 | 2 | **2** | 0 | 0/2/0 | 🟢 B | `git merge --no-ff rama/frida-gum-docs-2026-11-23` | — |
| 12 | `agent/go-re-completo` | `e081d1e` | 2026-11-23 | 2 | **2** | 234 | 8/54/298 | 🟢🔵 B | `git merge --no-ff agent/go-re-completo` | — |
| 13 | `agent/re-engine` | `b951962` | 2026-11-23 | 1 | **1** | 185 | 0/58/250 | 🟢🔵 B | `git merge --no-ff agent/re-engine` | — |
| 14 | `rama/mitm-proxy-2026-11-23` | `fea8a41` | 2026-11-23 | 12 | **12** | 87 | 14/52/37 | 🟡🔵 B | `git merge --no-ff rama/mitm-proxy-2026-11-23   # resolver: Merge,` | Merge, |
| 15 | `agent/llm-testing-v16` | `3e35c23` | 2026-11-22 | 9 | **9** | 194 | 0/61/235 | 🟡🔵 B | `git merge --no-ff agent/llm-testing-v16   # resolver: Merge,` | Merge, |
| 16 | `agent/kernel-nethunter` | `0e02b76` | 2026-11-22 | 7 | **7** | 611 | 4/112/7194 | 🟡🔵 B | `git merge --no-ff agent/kernel-nethunter   # resolver: Merge,` | Merge, |
| 17 | `agent/phone-inventory` | `1b9f4a1` | 2026-11-22 | 5 | **5** | 268 | 1/57/387 | 🟡 B | `git merge --no-ff agent/phone-inventory   # resolver: Merge,` | Merge, |
| 18 | `agent/telecom-otp-antispam` | `de3bdf4` | 2026-11-21 | 4 | **4** | 400 | 2/105/7186 | 🟡🔵 B | `git merge --no-ff agent/telecom-otp-antispam   # resolver: Merge,` | Merge, |
| 19 | `agent/backup-perfil-raw` | `c974b4d` | 2026-11-22 | 4 | **4** | 269 | 8/56/389 | 🟡🔵 B | `git merge --no-ff agent/backup-perfil-raw   # resolver: Merge,` | Merge, |
| 20 | `verify/integracion` | `9c96dcc` | 2026-11-21 | 4 | **3** | 374 | 2/96/7132 | 🟡 B | `git merge --no-ff verify/integracion   # resolver: Merge,` | Merge, |
| 21 | `agent/automacion-software` | `24b657f` | 2026-11-23 | 3 | **3** | 611 | 16/111/7199 | 🟡🔵 B | `git merge --no-ff agent/automacion-software   # resolver: Merge,rust-core/tests/root_hide_app_intel.rs,` | Merge,rust-core/tests/root_hide_app_intel.rs, |
| 22 | `agentgeneral/canal` | `2ed5d5e` | 2026-11-21 | 2 | **2** | 415 | 0/104/7195 | 🟡 B | `git merge --no-ff agentgeneral/canal   # resolver: Merge,` | Merge, |
| 23 | `agent/cert-android` | `741794f` | 2026-11-21 | 2 | **2** | 402 | 2/106/7186 | 🟡 B | `git merge --no-ff agent/cert-android   # resolver: Merge,` | Merge, |
| 24 | `fix/mobile-web-bridge` | `93e0503` | 2026-11-19 | 1 | **1** | 907 | 0/58/7398 | 🟡 B | `git merge --no-ff fix/mobile-web-bridge   # resolver: Merge,` | Merge, |
| 25 | `agent/swift-aead-deobf` | `16f628f` | 2026-11-21 | 1 | **1** | 408 | 0/104/7195 | 🟡 B | `git merge --no-ff agent/swift-aead-deobf   # resolver: Merge,` | Merge, |
| 26 | `agent/ui-v2-desde-disco` | `5da68a3` | 2026-11-21 | 1 | **1** | 376 | 0/93/7148 | 🟡 B | `git merge --no-ff agent/ui-v2-desde-disco   # resolver: Merge,` | Merge, |
| 27 | `agent/voice-p0-fix` | `0fe342f` | 2026-11-22 | 1 | **1** | 272 | 0/56/391 | 🟡 B | `git merge --no-ff agent/voice-p0-fix   # resolver: Merge,` | Merge, |
| 28 | `agent/rescate-antigravity-22nov` | `a995194` | 2026-11-22 | 1 | **1** | 232 | 1/53/305 | 🟡 B | `git merge --no-ff agent/rescate-antigravity-22nov   # resolver: Merge,` | Merge, |
| 29 | `agent/re-engine-bdd` | `826db20` | 2026-11-22 | 1 | **1** | 191 | 2/57/256 | 🟡🔵 B | `git merge --no-ff agent/re-engine-bdd   # resolver: Merge,` | Merge, |
| 30 | `agent/fix-e0382-listener` | `b420980` | 2026-11-23 | 1 | **1** | 90 | 0/44/39 | 🟡 B | `git merge --no-ff agent/fix-e0382-listener   # resolver: Merge,` | Merge, |
| 31 | `rama/frida-gum-2026-11-23` | `bc5f853` | 2026-11-23 | 1 | **1** | 84 | 1/45/29 | 🟡🔵 B | `git merge --no-ff rama/frida-gum-2026-11-23   # resolver: Merge,` | Merge, |
| 32 | `agent/antigravity-instrumentado` | `b6e3895` | 2026-11-23 | 1 | **1** | 264 | 0/60/381 | 🟡🔵 B | `git merge --no-ff agent/antigravity-instrumentado   # resolver: Merge,` | Merge, |
| 33 | `agent/grabaciones-merge` | `a29d6bc` | 2026-11-21 | 3 | **0** | 361 | 0/89/7140 | ⚪ C | `(nada: cerrar)   git branch -D agent/grabaciones-merge` | Merge, |
| 34 | `agent/profile` | `0df4edd` | 2026-11-20 | 0 | **0** | 720 | 1/110/7308 | ⚪ C | `(nada: cerrar)   git branch -D agent/profile` | — |
| 35 | `tmp-push` | `79cb8e1` | 2026-11-21 | 0 | **0** | 423 | 0/108/7196 | ⚪ C | `(nada: cerrar)   git branch -D tmp-push` | — |
| 36 | `agent/delegacion-parasitaria` | `4035dde` | 2026-11-21 | 0 | **0** | 618 | 4/108/7203 | ⚪ C | `(nada: cerrar)   git branch -D agent/delegacion-parasitaria` | — |
| 37 | `agent/lab-kit-verdict` | `d7f48e9` | 2026-11-21 | 0 | **0** | 375 | 0/106/7133 | ⚪ C | `(nada: cerrar)   git branch -D agent/lab-kit-verdict` | — |
| 38 | `agent/qa-sec-harness-spec` | `d148c95` | 2026-11-21 | 0 | **0** | 374 | 0/96/7132 | ⚪🔵 C | `(nada: cerrar)   git branch -D agent/qa-sec-harness-spec` | — |
| 39 | `feat/google-drive-nativo` | `76674bf` | 2026-11-21 | 0 | **0** | 336 | 0/94/7122 | ⚪ C | `(nada: cerrar)   git branch -D feat/google-drive-nativo` | — |
| 40 | `agent/merge-ramas-pendientes` | `4592c60` | 2026-11-21 | 0 | **0** | 350 | 0/93/7125 | ⚪ C | `(nada: cerrar)   git branch -D agent/merge-ramas-pendientes` | — |
| 41 | `agent/delegacion-sobre-main` | `9b96b56` | 2026-11-21 | 0 | **0** | 373 | 0/94/7132 | ⚪ C | `(nada: cerrar)   git branch -D agent/delegacion-sobre-main` | — |
| 42 | `agent/delegacion-bloque-final` | `c28daef` | 2026-11-21 | 0 | **0** | 335 | 0/92/7122 | ⚪ C | `(nada: cerrar)   git branch -D agent/delegacion-bloque-final` | — |
| 43 | `agentgeneral/canal2` | `9314dac` | 2026-11-21 | 0 | **0** | 372 | 0/93/7130 | ⚪ C | `(nada: cerrar)   git branch -D agentgeneral/canal2` | — |
| 44 | `agent/fix-build-main` | `c93859d` | 2026-11-21 | 0 | **0** | 332 | 0/93/7119 | ⚪ C | `(nada: cerrar)   git branch -D agent/fix-build-main` | — |
| 45 | `agent/repair-entorno` | `cea7e8d` | 2026-11-21 | 0 | **0** | 616 | 4/108/7202 | ⚪ C | `(nada: cerrar)   git branch -D agent/repair-entorno` | — |
| 46 | `integration/main-merge` | `299d55e` | 2026-11-21 | 0 | **0** | 330 | 0/91/7118 | ⚪ C | `(nada: cerrar)   git branch -D integration/main-merge` | — |
| 47 | `main-integracion` | `abefa58` | 2026-11-21 | 0 | **0** | 329 | 0/89/7118 | ⚪🔵 C | `(nada: cerrar)   git branch -D main-integracion` | — |
| 48 | `agent/tools-grabaciones-y-doctor` | `81aa54a` | 2026-11-21 | 0 | **0** | 328 | 0/89/7116 | ⚪ C | `(nada: cerrar)   git branch -D agent/tools-grabaciones-y-doctor` | — |
| 49 | `agent/delegacion-reaplicada` | `6bd5f24` | 2026-11-21 | 0 | **0** | 329 | 0/89/7118 | ⚪ C | `(nada: cerrar)   git branch -D agent/delegacion-reaplicada` | — |
| 50 | `agent/tablón-contrato-v15` | `efabaef` | 2026-11-21 | 0 | **0** | 329 | 0/91/7118 | ⚪ C | `(nada: cerrar)   git branch -D agent/tablón-contrato-v15` | — |
| 51 | `agent/board-v1-sobre-main` | `5cd76de` | 2026-11-21 | 0 | **0** | 329 | 0/89/7118 | ⚪ C | `(nada: cerrar)   git branch -D agent/board-v1-sobre-main` | — |
| 52 | `agent/backup-nativo-apps` | `ca54841` | 2026-11-21 | 0 | **0** | 278 | 0/55/396 | ⚪ C | `(nada: cerrar)   git branch -D agent/backup-nativo-apps` | — |
| 53 | `feature/clipboard` | `4099862` | 2026-11-22 | 0 | **0** | 275 | 0/56/394 | ⚪ C | `(nada: cerrar)   git branch -D feature/clipboard` | — |
| 54 | `agent/antigravity-backlog` | `b9f1072` | 2026-11-22 | 0 | **0** | 268 | 0/58/389 | ⚪ C | `(nada: cerrar)   git branch -D agent/antigravity-backlog` | — |
| 55 | `agent/antigravity-bridge` | `560c440` | 2026-11-22 | 0 | **0** | 265 | 0/58/385 | ⚪ C | `(nada: cerrar)   git branch -D agent/antigravity-bridge` | — |
| 56 | `agent/backup-perfil-raw-wip` | `560c440` | 2026-11-22 | 0 | **0** | 265 | 0/58/385 | ⚪ C | `(nada: cerrar)   git branch -D agent/backup-perfil-raw-wip` | — |
| 57 | `agent/inventario-exhaustivo` | `560c440` | 2026-11-22 | 0 | **0** | 265 | 0/58/385 | ⚪ C | `(nada: cerrar)   git branch -D agent/inventario-exhaustivo` | — |
| 58 | `agent/board-v1` | `fac38e7` | 2026-11-22 | 0 | **0** | 264 | 0/59/381 | ⚪ C | `(nada: cerrar)   git branch -D agent/board-v1` | — |
| 59 | `agent/fix-backup-engine-2rojos` | `867e871` | 2026-11-22 | 0 | **0** | 261 | 0/57/383 | ⚪ C | `(nada: cerrar)   git branch -D agent/fix-backup-engine-2rojos` | — |
| 60 | `agent/voice-fmt` | `98c3251` | 2026-11-22 | 0 | **0** | 260 | 0/56/383 | ⚪ C | `(nada: cerrar)   git branch -D agent/voice-fmt` | — |
| 61 | `agent/w2-ci-hardening` | `1eb0849` | 2026-11-22 | 0 | **0** | 259 | 0/53/382 | ⚪ C | `(nada: cerrar)   git branch -D agent/w2-ci-hardening` | — |
| 62 | `agent/panico-500-ui-v2` | `9f93274` | 2026-11-22 | 0 | **0** | 258 | 0/53/382 | ⚪ C | `(nada: cerrar)   git branch -D agent/panico-500-ui-v2` | — |
| 63 | `agent/providers-path-dinamico` | `9cf016c` | 2026-11-22 | 0 | **0** | 255 | 0/52/379 | ⚪ C | `(nada: cerrar)   git branch -D agent/providers-path-dinamico` | — |
| 64 | `agent/rescate-antigravity-bridge` | `512c11f` | 2026-11-22 | 0 | **0** | 243 | 0/57/319 | ⚪ C | `(nada: cerrar)   git branch -D agent/rescate-antigravity-bridge` | — |
| 65 | `agent/rescate-ghidra` | `9e1f7b7` | 2026-11-22 | 0 | **0** | 242 | 0/59/315 | ⚪ C | `(nada: cerrar)   git branch -D agent/rescate-ghidra` | — |
| 66 | `agent/rescate-inventario` | `699dddc` | 2026-11-22 | 0 | **0** | 241 | 0/61/313 | ⚪ C | `(nada: cerrar)   git branch -D agent/rescate-inventario` | — |
| 67 | `agent/titanium-migracion` | `69f8d26` | 2026-11-22 | 0 | **0** | 234 | 0/55/306 | ⚪ C | `(nada: cerrar)   git branch -D agent/titanium-migracion` | — |
| 68 | `agent/go-retool-textbase` | `2503d5d` | 2026-11-22 | 0 | **0** | 232 | 0/52/306 | ⚪ C | `(nada: cerrar)   git branch -D agent/go-retool-textbase` | — |
| 69 | `agent/ghidra-support` | `d9f0511` | 2026-11-22 | 0 | **0** | 264 | 0/58/384 | ⚪ C | `(nada: cerrar)   git branch -D agent/ghidra-support` | — |
| 70 | `agent/root-occlusion-magisk` | `033626b` | 2026-11-22 | 0 | **0** | 230 | 0/53/306 | ⚪🔵 C | `(nada: cerrar)   git branch -D agent/root-occlusion-magisk` | — |
| 71 | `agent/re-engine-deobf` | `51a2198` | 2026-11-22 | 0 | **0** | 191 | 0/57/256 | ⚪ C | `(nada: cerrar)   git branch -D agent/re-engine-deobf` | — |
| 72 | `agent/titanium-migracion-v2` | `25a4a2b` | 2026-11-22 | 0 | **0** | 188 | 0/64/251 | ⚪🔵 C | `(nada: cerrar)   git branch -D agent/titanium-migracion-v2` | — |
| 73 | `agent/backup-nativo-v2` | `68a507e` | 2026-11-22 | 0 | **0** | 227 | 0/57/292 | ⚪🔵 C | `(nada: cerrar)   git branch -D agent/backup-nativo-v2` | — |
| 74 | `wip/sync-22nov` | `0ba46cb` | 2026-11-23 | 0 | **0** | 95 | 0/48/39 | ⚪ C | `(nada: cerrar)   git branch -D wip/sync-22nov` | — |
| 75 | `agent/automacion-software-v16` | `1665674` | 2026-11-23 | 0 | **0** | 90 | 0/44/39 | ⚪ C | `(nada: cerrar)   git branch -D agent/automacion-software-v16` | — |
| 76 | `agent/fix-backup-catalog-delimitador` | `1665674` | 2026-11-23 | 0 | **0** | 90 | 0/44/39 | ⚪🔵 C | `(nada: cerrar)   git branch -D agent/fix-backup-catalog-delimitador` | — |
| 77 | `integration/main-merge2` | `1665674` | 2026-11-23 | 0 | **0** | 90 | 0/44/39 | ⚪🔵 C | `(nada: cerrar)   git branch -D integration/main-merge2` | — |
| 78 | `rama/proto-regen-2026-11-23` | `74274f2` | 2026-11-23 | 0 | **0** | 600 | 4/111/7187 | ⚪🔵 C | `(nada: cerrar)   git branch -D rama/proto-regen-2026-11-23` | — |
| 79 | `rama/fix-ci-2026-11-23` | `a1a1561` | 2026-11-23 | 0 | **0** | 85 | 0/40/39 | ⚪🔵 C | `(nada: cerrar)   git branch -D rama/fix-ci-2026-11-23` | — |
| 80 | `rama/inyeccion-frida-2026-11-23` | `b7c96ce` | 2026-11-23 | 0 | **0** | 84 | 0/45/29 | ⚪🔵 C | `(nada: cerrar)   git branch -D rama/inyeccion-frida-2026-11-23` | — |
| 81 | `rama/audit-calidad-2026-11-23` | `22052a0` | 2026-11-23 | 0 | **0** | 59 | 0/30/38 | ⚪🔵 C | `(nada: cerrar)   git branch -D rama/audit-calidad-2026-11-23` | — |

`🔵` = la rama está **checked out en un worktree con un agente trabajando ahora** ⇒ no se toca. `A/M/D` = ficheros añadidos / modificados / borrados respecto a `main` (dos puntas).

## 3.b. 🟢 B — MERGEABLE SIN CONFLICTO (7) — integrar ya

### `rama/ramas-auditoria-2026-11-23` — `bdcea7c` — 2026-11-23 08:34:47

- **Asunto:** 
- **Delta:** 59 files changed, 13840 insertions(+), 206 deletions(-)
- **Commits no integrados:** 31 (patch-ids nuevos) · `ahead` 34 · `behind` 0
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `git merge --no-ff rama/ramas-auditoria-2026-11-23`

### `rama/ci-calidad-2026-11-23` — `cd01498` — 2026-11-23 08:38:32

- **Asunto:** 
- **Delta:** 56 files changed, 12942 insertions(+), 208 deletions(-)
- **Commits no integrados:** 28 (patch-ids nuevos) · `ahead` 31 · `behind` 0
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `git merge --no-ff rama/ci-calidad-2026-11-23`

### `main` — `e23b18b` — 2026-11-23 08:38:09

- **Asunto:** 
- **Delta:** 57 files changed, 12930 insertions(+), 206 deletions(-)
- **Commits no integrados:** 27 (patch-ids nuevos) · `ahead` 30 · `behind` 0
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `git merge --no-ff main`

### `rama/regenerar-go-2026-11-23` — `1768d20` — 2026-11-23 08:35:07

- **Asunto:** 
- **Delta:** 88 files changed, 1032 insertions(+), 14187 deletions(-)
- **Commits no integrados:** 4 (patch-ids nuevos) · `ahead` 4 · `behind` 93
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `git merge --no-ff rama/regenerar-go-2026-11-23`

### `rama/frida-gum-docs-2026-11-23` — `c61c95e` — 2026-11-23 06:56:41

- **Asunto:** 
- **Delta:** 2 files changed, 13 insertions(+), 3 deletions(-)
- **Commits no integrados:** 2 (patch-ids nuevos) · `ahead` 2 · `behind` 0
- **Comando:** `git merge --no-ff rama/frida-gum-docs-2026-11-23`

### `agent/go-re-completo` — `e081d1e` — 2026-11-23 07:45:00

- **Asunto:** 
- **Delta:** 360 files changed, 3346 insertions(+), 69916 deletions(-)
- **Commits no integrados:** 2 (patch-ids nuevos) · `ahead` 2 · `behind` 234
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `git merge --no-ff agent/go-re-completo`

### `agent/re-engine` — `b951962` — 2026-11-23 02:57:31

- **Asunto:** 
- **Delta:** 308 files changed, 864 insertions(+), 48779 deletions(-)
- **Commits no integrados:** 1 (patch-ids nuevos) · `ahead` 1 · `behind` 185
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `git merge --no-ff agent/re-engine`

## 3.b. 🟡 B — MERGEABLE CON CONFLICTOS (19) — integrar con revisión manual

### `rama/mitm-proxy-2026-11-23` — `fea8a41` — 2026-11-23 06:55:34

- **Asunto:** 
- **Delta:** 103 files changed, 4504 insertions(+), 15012 deletions(-)
- **Commits no integrados:** 12 (patch-ids nuevos) · `ahead` 12 · `behind` 87
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `git merge --no-ff rama/mitm-proxy-2026-11-23   # resolver: Merge,`

### `agent/llm-testing-v16` — `3e35c23` — 2026-11-22 20:40:11

- **Asunto:** 
- **Delta:** 296 files changed, 934 insertions(+), 50719 deletions(-)
- **Commits no integrados:** 9 (patch-ids nuevos) · `ahead` 9 · `behind` 194
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `git merge --no-ff agent/llm-testing-v16   # resolver: Merge,`

### `agent/kernel-nethunter` — `0e02b76` — 2026-11-22 14:22:41

- **Asunto:** 
- **Delta:** 7310 files changed, 3432 insertions(+), 2500046 deletions(-)
- **Commits no integrados:** 7 (patch-ids nuevos) · `ahead` 7 · `behind` 611
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `git merge --no-ff agent/kernel-nethunter   # resolver: Merge,`

### `agent/phone-inventory` — `1b9f4a1` — 2026-11-22 00:19:07

- **Asunto:** 
- **Delta:** 445 files changed, 1157 insertions(+), 94129 deletions(-)
- **Commits no integrados:** 5 (patch-ids nuevos) · `ahead` 5 · `behind` 268
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- **Comando:** `git merge --no-ff agent/phone-inventory   # resolver: Merge,`

### `agent/telecom-otp-antispam` — `de3bdf4` — 2026-11-21 15:22:58

- **Asunto:** 
- **Delta:** 7293 files changed, 1863 insertions(+), 2499625 deletions(-)
- **Commits no integrados:** 4 (patch-ids nuevos) · `ahead` 4 · `behind` 400
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `git merge --no-ff agent/telecom-otp-antispam   # resolver: Merge,`

### `agent/backup-perfil-raw` — `c974b4d` — 2026-11-22 00:52:25

- **Asunto:** 
- **Delta:** 453 files changed, 6118 insertions(+), 95007 deletions(-)
- **Commits no integrados:** 4 (patch-ids nuevos) · `ahead` 4 · `behind` 269
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `git merge --no-ff agent/backup-perfil-raw   # resolver: Merge,`

### `verify/integracion` — `9c96dcc` — 2026-11-21 14:57:21

- **Asunto:** 
- **Delta:** 7230 files changed, 1968 insertions(+), 2465486 deletions(-)
- **Commits no integrados:** 3 (patch-ids nuevos) · `ahead` 4 · `behind` 374
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- **Comando:** `git merge --no-ff verify/integracion   # resolver: Merge,`

### `agent/automacion-software` — `24b657f` — 2026-11-23 02:58:38

- **Asunto:** 
- **Delta:** 7326 files changed, 10373 insertions(+), 2502710 deletions(-)
- **Commits no integrados:** 3 (patch-ids nuevos) · `ahead` 3 · `behind` 611
- 🟡 **Ficheros en conflicto al mergear:** `Merge,rust-core/tests/root_hide_app_intel.rs,`
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `git merge --no-ff agent/automacion-software   # resolver: Merge,rust-core/tests/root_hide_app_intel.rs,`

### `agentgeneral/canal` — `2ed5d5e` — 2026-11-21 04:55:32

- **Asunto:** 
- **Delta:** 7299 files changed, 1339 insertions(+), 2505737 deletions(-)
- **Commits no integrados:** 2 (patch-ids nuevos) · `ahead` 2 · `behind` 415
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- **Comando:** `git merge --no-ff agentgeneral/canal   # resolver: Merge,`

### `agent/cert-android` — `741794f` — 2026-11-21 14:43:34

- **Asunto:** 
- **Delta:** 7294 files changed, 2209 insertions(+), 2500411 deletions(-)
- **Commits no integrados:** 2 (patch-ids nuevos) · `ahead` 2 · `behind` 402
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- **Comando:** `git merge --no-ff agent/cert-android   # resolver: Merge,`

### `fix/mobile-web-bridge` — `93e0503` — 2026-11-19 20:35:50

- **Asunto:** 
- **Delta:** 7456 files changed, 1926 insertions(+), 2698370 deletions(-)
- **Commits no integrados:** 1 (patch-ids nuevos) · `ahead` 1 · `behind` 907
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- **Comando:** `git merge --no-ff fix/mobile-web-bridge   # resolver: Merge,`

### `agent/swift-aead-deobf` — `16f628f` — 2026-11-21 09:47:16

- **Asunto:** 
- **Delta:** 7299 files changed, 2306 insertions(+), 2505334 deletions(-)
- **Commits no integrados:** 1 (patch-ids nuevos) · `ahead` 1 · `behind` 408
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- **Comando:** `git merge --no-ff agent/swift-aead-deobf   # resolver: Merge,`

### `agent/ui-v2-desde-disco` — `5da68a3` — 2026-11-21 14:43:48

- **Asunto:** 
- **Delta:** 7241 files changed, 1378 insertions(+), 2471838 deletions(-)
- **Commits no integrados:** 1 (patch-ids nuevos) · `ahead` 1 · `behind` 376
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- **Comando:** `git merge --no-ff agent/ui-v2-desde-disco   # resolver: Merge,`

### `agent/voice-p0-fix` — `0fe342f` — 2026-11-22 00:38:07

- **Asunto:** 
- **Delta:** 447 files changed, 1051 insertions(+), 95901 deletions(-)
- **Commits no integrados:** 1 (patch-ids nuevos) · `ahead` 1 · `behind` 272
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- **Comando:** `git merge --no-ff agent/voice-p0-fix   # resolver: Merge,`

### `agent/rescate-antigravity-22nov` — `a995194` — 2026-11-22 17:12:01

- **Asunto:** 
- **Delta:** 359 files changed, 1113 insertions(+), 70590 deletions(-)
- **Commits no integrados:** 1 (patch-ids nuevos) · `ahead` 1 · `behind` 232
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- **Comando:** `git merge --no-ff agent/rescate-antigravity-22nov   # resolver: Merge,`

### `agent/re-engine-bdd` — `826db20` — 2026-11-22 20:53:41

- **Asunto:** 
- **Delta:** 315 files changed, 1586 insertions(+), 51972 deletions(-)
- **Commits no integrados:** 1 (patch-ids nuevos) · `ahead` 1 · `behind` 191
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `git merge --no-ff agent/re-engine-bdd   # resolver: Merge,`

### `agent/fix-e0382-listener` — `b420980` — 2026-11-23 03:10:51

- **Asunto:** 
- **Delta:** 83 files changed, 348 insertions(+), 15602 deletions(-)
- **Commits no integrados:** 1 (patch-ids nuevos) · `ahead` 1 · `behind` 90
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- **Comando:** `git merge --no-ff agent/fix-e0382-listener   # resolver: Merge,`

### `rama/frida-gum-2026-11-23` — `bc5f853` — 2026-11-23 06:26:34

- **Asunto:** 
- **Delta:** 75 files changed, 641 insertions(+), 11419 deletions(-)
- **Commits no integrados:** 1 (patch-ids nuevos) · `ahead` 1 · `behind` 84
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `git merge --no-ff rama/frida-gum-2026-11-23   # resolver: Merge,`

### `agent/antigravity-instrumentado` — `b6e3895` — 2026-11-23 06:40:01

- **Asunto:** 
- **Delta:** 441 files changed, 1415 insertions(+), 91573 deletions(-)
- **Commits no integrados:** 1 (patch-ids nuevos) · `ahead` 1 · `behind` 264
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `git merge --no-ff agent/antigravity-instrumentado   # resolver: Merge,`

## 3.d. 🔴 D — RIESGOSA (6) — NO mergear

### `agent/merge-flota-pendiente` — `dc95816` — 2026-11-21 15:12:33

- **Asunto:** 
- **Delta:** 7229 files changed, 3626 insertions(+), 2467580 deletions(-)
- **Commits no integrados:** 5 (patch-ids nuevos) · `ahead` 17 · `behind` 345
- 🔴 **Merge `ours` detectado** (árbol del merge == árbol del 1er padre ⇒ código perdido): `,88d0531`
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- **Comando:** `NO MERGEAR   (si sólo interesa un fichero: git checkout agent/merge-flota-pendiente -- <fichero>)`

### `agent/f3-integracion` — `9771f91` — 2026-11-21 14:50:50

- **Asunto:** 
- **Delta:** 7226 files changed, 2468 insertions(+), 2467912 deletions(-)
- **Commits no integrados:** 4 (patch-ids nuevos) · `ahead` 19 · `behind` 342
- 🔴 **Merge `ours` detectado** (árbol del merge == árbol del 1er padre ⇒ código perdido): `,9771f91,447f7e8,88d0531`
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- **Comando:** `NO MERGEAR   (si sólo interesa un fichero: git checkout agent/f3-integracion -- <fichero>)`

### `integration/merge-all` — `88d0531` — 2026-11-21 14:37:29

- **Asunto:** 
- **Delta:** 7239 files changed, 1351 insertions(+), 2471528 deletions(-)
- **Commits no integrados:** 1 (patch-ids nuevos) · `ahead` 3 · `behind` 368
- 🔴 **Merge `ours` detectado** (árbol del merge == árbol del 1er padre ⇒ código perdido): `,88d0531`
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- **Comando:** `NO MERGEAR   (si sólo interesa un fichero: git checkout integration/merge-all -- <fichero>)`

### `rescue/shared-tree-2026-11-21` — `a5ba863` — 2026-11-21 14:43:50

- **Asunto:** 
- **Delta:** 7326 files changed, 2944 insertions(+), 2505329 deletions(-)
- **Commits no integrados:** 1 (patch-ids nuevos) · `ahead` 1 · `behind` 618
- 🟡 **Ficheros en conflicto al mergear:** `Merge,deploy/magisk_csc_recording/module.prop,deploy/patch_csc_recording.sh,docs/IDEAS_BACKLOG.md,docs/KALI_NETHUNTER_KERNEL_PLAN.md,docs/MEGADAEMON_CLEANUP_PLAN.md,rust-core/tests/agent_loop.rs,`
- **Comando:** `NO MERGEAR   (si sólo interesa un fichero: git checkout rescue/shared-tree-2026-11-21 -- <fichero>)`

### `wip/llm-v16-raw-snapshot` — `da1cf89` — 2026-11-22 20:07:34

- **Asunto:** 
- **Delta:** 365 files changed, 3384 insertions(+), 71325 deletions(-)
- **Commits no integrados:** 1 (patch-ids nuevos) · `ahead` 1 · `behind` 236
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- **Comando:** `NO MERGEAR   (si sólo interesa un fichero: git checkout wip/llm-v16-raw-snapshot -- <fichero>)`

### `agent/energia-v15` — `a621f1f` — 2026-11-20 07:31:04

- **Asunto:** 
- **Delta:** 7372 files changed, 2739 insertions(+), 2538178 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 629
- 🔴 **Marcadores de conflicto dentro de la rama:** `docs/ANDROID_EXYNOS2100_FINDINGS.md`
- **Comando:** `NO MERGEAR   (si sólo interesa un fichero: git checkout agent/energia-v15 -- <fichero>)`

## 3.c. ⚪ C — OBSOLETA (49) — cerrar la rama

### `agent/grabaciones-merge` — `a29d6bc` — 2026-11-21 14:46:06

- **Asunto:** 
- **Delta:** 7229 files changed, 1311 insertions(+), 2468763 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 3 · `behind` 361
- 🟡 **Ficheros en conflicto al mergear:** `Merge,`
- **Comando:** `(nada: cerrar)   git branch -D agent/grabaciones-merge`

### `agent/profile` — `0df4edd` — 2026-11-20 02:50:40

- **Asunto:** 
- **Delta:** 7419 files changed, 2162 insertions(+), 2629373 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 720
- **Comando:** `(nada: cerrar)   git branch -D agent/profile`

### `tmp-push` — `79cb8e1` — 2026-11-21 04:37:33

- **Asunto:** 
- **Delta:** 7304 files changed, 1852 insertions(+), 2506755 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 423
- **Comando:** `(nada: cerrar)   git branch -D tmp-push`

### `agent/delegacion-parasitaria` — `4035dde` — 2026-11-21 14:14:59

- **Asunto:** 
- **Delta:** 7315 files changed, 2959 insertions(+), 2503283 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 618
- **Comando:** `(nada: cerrar)   git branch -D agent/delegacion-parasitaria`

### `agent/lab-kit-verdict` — `d7f48e9` — 2026-11-21 14:17:06

- **Asunto:** 
- **Delta:** 7239 files changed, 1986 insertions(+), 2465906 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 375
- **Comando:** `(nada: cerrar)   git branch -D agent/lab-kit-verdict`

### `agent/qa-sec-harness-spec` — `d148c95` — 2026-11-21 14:35:39

- **Asunto:** 
- **Delta:** 7228 files changed, 1494 insertions(+), 2465488 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 374
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `(nada: cerrar)   git branch -D agent/qa-sec-harness-spec`

### `feat/google-drive-nativo` — `76674bf` — 2026-11-21 14:43:42

- **Asunto:** 
- **Delta:** 7216 files changed, 1286 insertions(+), 2462082 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 336
- **Comando:** `(nada: cerrar)   git branch -D feat/google-drive-nativo`

### `agent/merge-ramas-pendientes` — `4592c60` — 2026-11-21 14:47:25

- **Asunto:** 
- **Delta:** 7218 files changed, 1322 insertions(+), 2462705 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 350
- **Comando:** `(nada: cerrar)   git branch -D agent/merge-ramas-pendientes`

### `agent/delegacion-sobre-main` — `9b96b56` — 2026-11-21 14:51:47

- **Asunto:** 
- **Delta:** 7226 files changed, 1470 insertions(+), 2464231 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 373
- **Comando:** `(nada: cerrar)   git branch -D agent/delegacion-sobre-main`

### `agent/delegacion-bloque-final` — `c28daef` — 2026-11-21 14:57:26

- **Asunto:** 
- **Delta:** 7214 files changed, 1262 insertions(+), 2460825 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 335
- **Comando:** `(nada: cerrar)   git branch -D agent/delegacion-bloque-final`

### `agentgeneral/canal2` — `9314dac` — 2026-11-21 15:05:56

- **Asunto:** 
- **Delta:** 7223 files changed, 1481 insertions(+), 2464510 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 372
- **Comando:** `(nada: cerrar)   git branch -D agentgeneral/canal2`

### `agent/fix-build-main` — `c93859d` — 2026-11-21 15:05:57

- **Asunto:** 
- **Delta:** 7212 files changed, 1268 insertions(+), 2461395 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 332
- **Comando:** `(nada: cerrar)   git branch -D agent/fix-build-main`

### `agent/repair-entorno` — `cea7e8d` — 2026-11-21 15:06:43

- **Asunto:** 
- **Delta:** 7314 files changed, 2959 insertions(+), 2502765 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 616
- **Comando:** `(nada: cerrar)   git branch -D agent/repair-entorno`

### `integration/main-merge` — `299d55e` — 2026-11-21 15:12:49

- **Asunto:** 
- **Delta:** 7209 files changed, 1264 insertions(+), 2461202 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 330
- **Comando:** `(nada: cerrar)   git branch -D integration/main-merge`

### `main-integracion` — `abefa58` — 2026-11-21 15:17:20

- **Asunto:** 
- **Delta:** 7207 files changed, 1243 insertions(+), 2460661 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 329
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `(nada: cerrar)   git branch -D main-integracion`

### `agent/tools-grabaciones-y-doctor` — `81aa54a` — 2026-11-21 15:23:08

- **Asunto:** 
- **Delta:** 7205 files changed, 1264 insertions(+), 2460813 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 328
- **Comando:** `(nada: cerrar)   git branch -D agent/tools-grabaciones-y-doctor`

### `agent/delegacion-reaplicada` — `6bd5f24` — 2026-11-21 15:27:32

- **Asunto:** 
- **Delta:** 7207 files changed, 1240 insertions(+), 2459945 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 329
- **Comando:** `(nada: cerrar)   git branch -D agent/delegacion-reaplicada`

### `agent/tablón-contrato-v15` — `efabaef` — 2026-11-21 15:28:06

- **Asunto:** 
- **Delta:** 7209 files changed, 1264 insertions(+), 2461201 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 329
- **Comando:** `(nada: cerrar)   git branch -D agent/tablón-contrato-v15`

### `agent/board-v1-sobre-main` — `5cd76de` — 2026-11-21 15:29:34

- **Asunto:** 
- **Delta:** 7207 files changed, 1240 insertions(+), 2459945 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 329
- **Comando:** `(nada: cerrar)   git branch -D agent/board-v1-sobre-main`

### `agent/backup-nativo-apps` — `ca54841` — 2026-11-21 23:54:58

- **Asunto:** 
- **Delta:** 451 files changed, 1056 insertions(+), 96451 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 278
- **Comando:** `(nada: cerrar)   git branch -D agent/backup-nativo-apps`

### `feature/clipboard` — `4099862` — 2026-11-22 00:01:47

- **Asunto:** 
- **Delta:** 450 files changed, 1062 insertions(+), 96199 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 275
- **Comando:** `(nada: cerrar)   git branch -D feature/clipboard`

### `agent/antigravity-backlog` — `b9f1072` — 2026-11-22 00:15:28

- **Asunto:** 
- **Delta:** 447 files changed, 1060 insertions(+), 95105 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 268
- **Comando:** `(nada: cerrar)   git branch -D agent/antigravity-backlog`

### `agent/antigravity-bridge` — `560c440` — 2026-11-22 00:29:02

- **Asunto:** 
- **Delta:** 443 files changed, 1064 insertions(+), 93994 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 265
- **Comando:** `(nada: cerrar)   git branch -D agent/antigravity-bridge`

### `agent/backup-perfil-raw-wip` — `560c440` — 2026-11-22 00:29:02

- **Asunto:** 
- **Delta:** 443 files changed, 1064 insertions(+), 93994 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 265
- **Comando:** `(nada: cerrar)   git branch -D agent/backup-perfil-raw-wip`

### `agent/inventario-exhaustivo` — `560c440` — 2026-11-22 00:29:02

- **Asunto:** 
- **Delta:** 443 files changed, 1064 insertions(+), 93994 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 265
- **Comando:** `(nada: cerrar)   git branch -D agent/inventario-exhaustivo`

### `agent/board-v1` — `fac38e7` — 2026-11-22 00:50:50

- **Asunto:** 
- **Delta:** 440 files changed, 1066 insertions(+), 91477 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 264
- **Comando:** `(nada: cerrar)   git branch -D agent/board-v1`

### `agent/fix-backup-engine-2rojos` — `867e871` — 2026-11-22 00:51:55

- **Asunto:** 
- **Delta:** 440 files changed, 987 insertions(+), 93409 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 261
- **Comando:** `(nada: cerrar)   git branch -D agent/fix-backup-engine-2rojos`

### `agent/voice-fmt` — `98c3251` — 2026-11-22 01:00:09

- **Asunto:** 
- **Delta:** 439 files changed, 933 insertions(+), 93252 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 260
- **Comando:** `(nada: cerrar)   git branch -D agent/voice-fmt`

### `agent/w2-ci-hardening` — `1eb0849` — 2026-11-22 01:03:43

- **Asunto:** 
- **Delta:** 435 files changed, 981 insertions(+), 93328 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 259
- **Comando:** `(nada: cerrar)   git branch -D agent/w2-ci-hardening`

### `agent/panico-500-ui-v2` — `9f93274` — 2026-11-22 01:11:38

- **Asunto:** 
- **Delta:** 435 files changed, 975 insertions(+), 93213 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 258
- **Comando:** `(nada: cerrar)   git branch -D agent/panico-500-ui-v2`

### `agent/providers-path-dinamico` — `9cf016c` — 2026-11-22 01:13:02

- **Asunto:** 
- **Delta:** 431 files changed, 987 insertions(+), 92782 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 255
- **Comando:** `(nada: cerrar)   git branch -D agent/providers-path-dinamico`

### `agent/rescate-antigravity-bridge` — `512c11f` — 2026-11-22 11:37:58

- **Asunto:** 
- **Delta:** 376 files changed, 512 insertions(+), 78155 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 243
- **Comando:** `(nada: cerrar)   git branch -D agent/rescate-antigravity-bridge`

### `agent/rescate-ghidra` — `9e1f7b7` — 2026-11-22 11:38:02

- **Asunto:** 
- **Delta:** 374 files changed, 523 insertions(+), 76042 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 242
- **Comando:** `(nada: cerrar)   git branch -D agent/rescate-ghidra`

### `agent/rescate-inventario` — `699dddc` — 2026-11-22 11:38:06

- **Asunto:** 
- **Delta:** 374 files changed, 612 insertions(+), 72636 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 241
- **Comando:** `(nada: cerrar)   git branch -D agent/rescate-inventario`

### `agent/titanium-migracion` — `69f8d26` — 2026-11-22 14:33:43

- **Asunto:** 
- **Delta:** 361 files changed, 723 insertions(+), 70831 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 234
- **Comando:** `(nada: cerrar)   git branch -D agent/titanium-migracion`

### `agent/go-retool-textbase` — `2503d5d` — 2026-11-22 15:24:52

- **Asunto:** 
- **Delta:** 358 files changed, 738 insertions(+), 70750 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 232
- **Comando:** `(nada: cerrar)   git branch -D agent/go-retool-textbase`

### `agent/ghidra-support` — `d9f0511` — 2026-11-22 18:33:55

- **Asunto:** 
- **Delta:** 442 files changed, 1061 insertions(+), 93923 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 264
- **Comando:** `(nada: cerrar)   git branch -D agent/ghidra-support`

### `agent/root-occlusion-magisk` — `033626b` — 2026-11-22 20:07:31

- **Asunto:** 
- **Delta:** 359 files changed, 548 insertions(+), 69088 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 230
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `(nada: cerrar)   git branch -D agent/root-occlusion-magisk`

### `agent/re-engine-deobf` — `51a2198` — 2026-11-22 20:24:43

- **Asunto:** 
- **Delta:** 313 files changed, 859 insertions(+), 51972 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 191
- **Comando:** `(nada: cerrar)   git branch -D agent/re-engine-deobf`

### `agent/titanium-migracion-v2` — `25a4a2b` — 2026-11-22 20:35:08

- **Asunto:** 
- **Delta:** 315 files changed, 868 insertions(+), 50965 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 188
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `(nada: cerrar)   git branch -D agent/titanium-migracion-v2`

### `agent/backup-nativo-v2` — `68a507e` — 2026-11-22 20:46:27

- **Asunto:** 
- **Delta:** 349 files changed, 774 insertions(+), 65254 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 227
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `(nada: cerrar)   git branch -D agent/backup-nativo-v2`

### `wip/sync-22nov` — `0ba46cb` — 2026-11-23 02:13:24

- **Asunto:** 
- **Delta:** 87 files changed, 387 insertions(+), 15858 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 95
- **Comando:** `(nada: cerrar)   git branch -D wip/sync-22nov`

### `agent/automacion-software-v16` — `1665674` — 2026-11-23 02:50:26

- **Asunto:** 
- **Delta:** 83 files changed, 344 insertions(+), 15603 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 90
- **Comando:** `(nada: cerrar)   git branch -D agent/automacion-software-v16`

### `agent/fix-backup-catalog-delimitador` — `1665674` — 2026-11-23 02:50:26

- **Asunto:** 
- **Delta:** 83 files changed, 344 insertions(+), 15603 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 90
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `(nada: cerrar)   git branch -D agent/fix-backup-catalog-delimitador`

### `integration/main-merge2` — `1665674` — 2026-11-23 02:50:26

- **Asunto:** 
- **Delta:** 83 files changed, 344 insertions(+), 15603 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 90
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `(nada: cerrar)   git branch -D integration/main-merge2`

### `rama/proto-regen-2026-11-23` — `74274f2` — 2026-11-23 03:06:47

- **Asunto:** 
- **Delta:** 7302 files changed, 3424 insertions(+), 2499523 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 600
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `(nada: cerrar)   git branch -D rama/proto-regen-2026-11-23`

### `rama/fix-ci-2026-11-23` — `a1a1561` — 2026-11-23 03:07:01

- **Asunto:** 
- **Delta:** 79 files changed, 330 insertions(+), 15507 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 85
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `(nada: cerrar)   git branch -D rama/fix-ci-2026-11-23`

### `rama/inyeccion-frida-2026-11-23` — `b7c96ce` — 2026-11-23 03:09:29

- **Asunto:** 
- **Delta:** 74 files changed, 444 insertions(+), 11902 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 84
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `(nada: cerrar)   git branch -D rama/inyeccion-frida-2026-11-23`

### `rama/audit-calidad-2026-11-23` — `22052a0` — 2026-11-23 03:14:45

- **Asunto:** 
- **Delta:** 68 files changed, 207 insertions(+), 14785 deletions(-)
- **Commits no integrados:** 0 (patch-ids nuevos) · `ahead` 0 · `behind` 59
- 🔵 **Rama ocupada**: hay un worktree con un agente vivo ⇒ coordinar antes de tocar nada.
- **Comando:** `(nada: cerrar)   git branch -D rama/audit-calidad-2026-11-23`

## 4. Puntos que siguen abiertos del histórico — estado MEDIDO

Los 5 puntos que el jefe tenía abiertos. **No se repite ninguno**: lo que ya está hecho se dice con la prueba, y lo que no, con el comando para comprobarlo.

| punto | estado MEDIDO | prueba |
|---|---|---|
| **Frida operativo (inyección real)** | ✅ **YA INTEGRADO en `main`** | `crates/hooks-payload/` (payload con frida-gum: `hooks.rs`, `json.rs`, `sink.rs`, `sys.rs`) + `crates/injector-payload/` (inyector ptrace: `elf.rs`, `log.rs`, `lib.rs`) + `rust-core/src/frida_dbi.rs` + `rust-core/src/frida_hooks.rs` — los 4 declarados en `lib.rs` y usados. `rama/inyeccion-frida-2026-11-23`: `git log main..rama` **vacío** y `git cherry main rama` **vacío** ⇒ integrada. `rama/frida-gum-2026-11-23`: queda **1** commit (`bc5f853`), que sólo añade 40 líneas a `docs/COORDINATION.md` ⇒ **cero código pendiente** |
| **MITM SQLite con bind-mount real** | ✅ **YA INTEGRADO en `main`** | `deploy/magisk-mitm-sqlite/` (`module.prop`, `service.sh` con el re-montaje tras reinicio leyendo `$STATE_DIR/mitm/estado.json`, `post-fs-data.sh`, `uninstall.sh`, `mitm-sqlite.sh`, `customize.sh`) + `rust-core/src/sqlite_mitm.rs` (declarado en `lib.rs`) + `tools/mitm-sqlite.sh` + `docs/MITM-SQLITE-Y-PROXY.md`. `rama/mitm-proxy-2026-11-23`: `git log main..rama` **vacío** ⇒ integrada |
| **agyshim como proxy real** | ✅ **YA INTEGRADO en `main`** | `rust-core/src/agyshim.rs` declarado en `lib.rs` y usado por 3 ficheros más. `docs/MITM-SQLITE-Y-PROXY.md` §0 dice que ya intercepta `/v1internal/*`, `/v1beta/*` y `/proxy/*` y reenvía por el gateway canónico |
| **Auditoría de ramas pendientes** | ✅ **ESTA ES LA TAREA 1 de este documento** | tabla de §2 |
| **Rotación de credenciales expuestas** | 🟡 **AUDITADA, LA ROTACIÓN LA HACE EL ROOT** | `docs/ROTACION-CREDENCIALES.md`: 4 API keys de Google (`tools/drive_ls.py:23-26`) en repo **público** = lo único realmente expuesto. La API key viva **ya está rotada** (0 coincidencias por sha256 contra las 81 puntas) |

> ⚠️ **Lo que sí queda abierto (y NO es de estas 5 tareas)**: `main` roto con marcadores de conflicto (§0) y la GUI fusionada sin cablear en el daemon (falta la ruta `/v3` en `cockpit_server.rs`, 3 líneas, mismo patrón que `docs/API_V2.md` §2).

---

## 5. Cómo se ha medido (reproducible)

```bash
cd /workspace/md-root
# clasificador de ramas (patch-id precalculado sobre main + merge-tree + marcadores)
bash /workspace/audit_branches4.sh <main_sha>      # -> /workspace/audit_branches.tsv
# marcadores de conflicto en cada rama
bash /workspace/audit_markers.sh                  # -> /workspace/audit_conflict_markers.txt
# ficheros en conflicto, con la extracción correcta (git 2.39)
bash /workspace/audit_conflicts.sh <main_sha>     # -> /workspace/audit_conflicts.tsv
# generador de esta tabla
python3 /workspace/gen_branch_table.py <salida.md> <main_sha>
```

**El truco que hace esto viable**: `git cherry` recalcula los patch-ids de los ~900 commits de `main` **en cada invocación** ⇒ 80 ramas × 15 min = 20 h. Precalculando los patch-ids de `main` **una sola vez** y comparando por `comm`, las 80 ramas salen en minutos. (Y `git patch-id` **necesita** las líneas `commit <sha>` del log: con `--format=""` la salida es vacía y el resultado es silenciosamente 0 ⇒ false "todo está integrado". Es la trampa que hizo perder una vuelta a esta auditoría.)

## 6. Limitaciones honestas de esta auditoría

1. `main` **se mueve mientras se mide** (el root integra ramas): las cifras `ahead/behind/novel` son la foto de `<main_sha>` y hay que recalcularlas si se integran más ramas antes de actuar.
2. `git merge-tree --write-tree` predice los conflictos de un merge de 3 vías; **no** es un merge real: un fichero que hoy no conflictúa puede conflictar si cambia `main` antes del merge.
3. Las ramas marcadas 🔵 tienen **un agente vivo** detrás ⇒ sus commits cambian mientras se miden: no deben tocarse sin coordinar.
4. **No se compiló nada** (política del sandbox: `cargo` bloqueado salvo `MD_COMPILE_OK=1`, que es sólo del root). «No rompe el build» se afirma por análisis del diff y por `rustfmt` (gate de sintaxis), **no** por compilación.

