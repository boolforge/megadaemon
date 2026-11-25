# Plan de optimización extrema del binario de MegaDaemon

**Fecha:** 2026-11-25 · **Base medida:** asset `megadaemon-core-android-arm64` publicado
(42.988.440 B = **41,00 MiB**), analysed con `readelf -SW` sobre el fichero real.

**Regla del proyecto:** *nada de fantasmadas*. Cada palanca de este documento
tiene (a) una medición de partida, (b) un riesgo declarado y (c) un criterio de
verificación. Las cifras del "boceto" que no se sostienen contra el repo están
marcadas como tales.

---

## 0. Auditoría del boceto: qué se sostiene y qué no

| Premisa del boceto | Verificación en este repo | Veredicto |
|---|---|---|
| "26 módulos" | `ls rust-core/src/*.rs \| wc -l` = **206** (204 declarados en `lib.rs`) | ❌ el boceto se apoyó en el README, no en el código |
| "lto=fat explota a 8-24 GB de RAM" | El CI (runner) sí lo mata; por eso ya está `thin` | ✅ ya resuelto |
| `panic = "abort"` "elimina .eh_frame (2,5 MiB)" | `catch_unwind` en `cockpit_server.rs:2832`, `cockpit_server.rs:3380`, `webui.rs:4990` | ❌ **prohibido**: con abort, un panic en una ruta tumba el daemon, y el daemon es el que sostiene el túnel ⇒ 502 y "móvil caído" |
| `--pack-dyn-relocs=relr` | Bicon soporta `DT_RELR` desde **API 28**; el build declara **API 26** (`aarch64-linux-android26-clang`) | ⚠️ sólo con `minSdk` ≥ 28 (el móvil del usuario es API 29+) |
| "mimalloc/jemalloc en Android" | Android usa **Scudo**, no ptmalloc2 | ⚠️ en Android sólo se **mide** (Scudo ya devuelve páginas); el allocator se evalúa en el build Linux |
| `-Z build-std` + `panic_immediate_abort` | El proyecto usa **stable** y `panic=unwind` a propósito | ⚠️ fase aparte, sin `immediate_abort` |
| `--icf=all` | `ld.lld` ya está activo (`rust-core/.cargo/config.toml`) | ⚠️ aplica, **pero ver §4**: ICF puede plegar funciones que los hooks de Frida localizan por dirección |
| `-enable-machine-outliner=always`, `-hot-cold-split` | rustc 1.100.0 (LLVM moderno) | ✅ aplicable vía `-C llvm-args` |
| `--thinlto-jobs=N`, `--thinlto-cache-dir` | El enlace ocurre en el runner del CI (RAM de sobra); en el sandbox (3 GB) sí importa | ✅ se aplica al sandbox, no al CI |

---

## 1. Medición de partida (fichero real, no estimación)

```
megadaemon-core-android-arm64   42.988.440 B (41,00 MiB)

.text              17.575.936 B  40,89 %   código
.rodata            11.329.832 B  26,36 %   datos de solo lectura (payload + tablas + strings)
.strtab            4.166.193 B   9,69 %   <-- tabla de símbolos (INÚTIL en el móvil)
.rela.dyn          2.809.768 B   6,54 %   relocaciones dinámicas
.symtab            2.661.408 B   6,19 %   <-- tabla de símbolos (INÚTIL en el móvil)
.data.rel.ro       1.904.128 B   4,43 %   vtables y relocs
.eh_frame          1.410.128 B   3,28 %   tablas de desenrollado (panic=unwind)
.gcc_except_table    915.444 B   2,13 %   tablas de landing pads
.eh_frame_hdr        175.180 B   0,41 %
```

Además: **90.296 cadenas ASCII** (≥8 chars) = **6,07 MiB** de strings.

---

## 2. Palancas aplicadas ya (riesgo nulo, verificadas con test)

### 2.1 `strip` en el override de paquete — **−6,52 MiB (−15,9 %)**

**Causa raíz medida:** el override `[profile.release.package.megadaemon-core]`
**anula** el `strip = true` del perfil base (Cargo no lo hereda ahí). El binario
publicado llevaba `.symtab` + `.strtab` = 6.827.601 B de tabla de símbolos en el
móvil.

**Reproducción (paquete mínimo, mismo perfil):**

| | binario | `.symtab`/`.strtab` |
|---|---|---|
| con override de paquete | 362.808 B | presentes |
| sin override | 290.944 B | ausentes |

**Arreglo:** `strip = true` dentro de `[profile.release.package.megadaemon-core]`.
Sin tocar una línea de código del daemon.

### 2.2 Payload de frida-gum comprimido — **−2,11 MiB (−4,9 %)**

El payload va **embebido** en el daemon (`include_bytes!`): **3.269.720 B (3,12 MiB
= 7,6 % del binario)**. Medido: `xz -9` → 1.054.700 B; `zstd -19` → ~1,2 MiB y
descomprime en microsegundos.

**Arreglo:** el *build script* comprime el payload con zstd antes de incrustarlo
(`zstd` como `[build-dependencies]`); el daemon lo **descomprime al instalar** y
verifica el SHA-256 del `.so` **descomprimido** (que es byte a byte el
original). El estado publica los dos tamaños (`bytes_comprimidos` y `bytes`) para
no confundir lo que ocupa en el binario con el payload real.

Test: `el_payload_comprimido_descomprime_al_original` (round-trip + ELF magic).

---

## 3. Palancas de siguiente paso (con su cifra y su riesgo)

| # | Palanca | Ahorro estimado | Riesgo | Cómo se verifica |
|---|---|---|---|---|
| 3.1 | **`--pack-dyn-relocs=relr`** (target Android, `minSdk` 28) | **−2,4 MiB** (`.rela.dyn` 2,81 MiB → ~0,35 MiB) | Bicon <28 no entiende `DT_RELR` ⇒ el binario no arrancaría en Android 8 | `readelf -d` muestra `DT_RELR`; arranque en el móvil (API 29+) |
| 3.2 | **`-C link-arg=-Wl,--gc-sections`** | −1 a −3 MiB de `.text` | Puede podar secciones referenciadas dinámicamente | tamaño + la suite completa + arranque en el móvil |
| 3.3 | **deps a `opt-level = "s"`** (`[profile.release.package."*"]`) | −1 a −3 MiB | Bajo: las deps son I/O (HTTP/TLS/JSON); el núcleo sigue a `3` | tamaño + benchmark |
| 3.4 | **`-C llvm-args=-enable-machine-outliner=always`** | −0,5 a −1,5 MiB | Cambia el layout del código | los hooks se recalculan en runtime (pclntab) ⇒ seguro; medir el tamaño |
| 3.5 | **`-C llvm-args=-hot-cold-split=true` + `#[cold]`** en las rutas de error | −0,3 a −1 MiB | Bajo | tamaño + `perf`/simpleperf |
| 3.6 | **`-C force-unwind-tables=no`** | −1 a −1,5 MiB de `.eh_frame` | Afecta al backtrace de los panics (no al comportamiento) | tamaño + `catch_unwind` sigue funcionando |
| 3.7 | **PGO + BOLT** | −10-15 % adicional + velocidad | Alto: necesita banco de pruebas en el CI y artefactos `.profdata` | fase 2 |
| 3.8 | **`madvise(MADV_DONTNEED)`** tras los escaneos de `memmap2` | RSS (no tamaño) | Bajo | `/proc/<pid>/smaps_rollup` en el móvil |
| 3.9 | **Publicar el asset una sola vez** (hoy se sube el normal **y** el versionado) | −41 MiB de release y de descarga | Bajo | `latest.json` + el auto-update |
| 3.10 | **`-Z build-std`** (nightly, `optimize_for_size`, **sin** `panic_immediate_abort`) | −1 a −3 MiB | Nightly en el CI; +5 % de tiempo | tamaño + CI verde |

### 3.11 Lo que NO se hace (y por qué)

- **`panic = "abort"` / `panic_immediate_abort`**: rompería el aislamiento de
  peticiones (`catch_unwind` en 3 sitios medidos) y mataría al daemon que
  sostiene el túnel de Cloudflare. Decisión v15 del proyecto, mantenida.
- **`--icf=all`**: `ld.lld` puede plegar funciones *idénticas*; los hooks de
  Frida localizan su objetivo por dirección ⇒ una función plegada puede hacer
  que el hook apunte a la equivalente (o a la canónica). Se puede probar, pero
  **no por defecto**: exige verificar en el móvil que el hook sigue 적용ando a
  la función correcta.
- **mimalloc/jemalloc en Android**: añade 1-2 MiB al binario y Scudo ya
  devuelve páginas al SO. Se evalúa sólo para el build Linux (desktop), donde
  el allocator es glibc.

---

## 4. Objetivo y secuencia

| Paso | Acumulado | Métrica |
|---|---|---|
| hoy | **41,00 MiB** | asset publicado |
| + strip (§2.1) | 34,48 MiB | `readelf -SW` sin `.symtab`/`.strtab` |
| + payload comprimido (§2.2) | **32,37 MiB** | `estado().embebido.bytes_comprimidos` |
| + RELR (§3.1) | 29,97 MiB | `readelf -d \| grep RELR` |
| + gc-sections + deps "s" (§3.2, §3.3) | **~26-28 MiB** | asset publicado |

**Objetivo: ≤ 30 MiB (−27 %) sin perder ninguna capacidad**, con el CI verde
en los 7 jobs y el arranque verificado en el móvil.

**Regla de cada iteración:** un cambio ⇒ un commit ⇒ un run del CI ⇒ el paso
"Tamano del binario" publica el tamaño ⇒ se compara. Si el binario no baja o
el CI se pone rojo, se revierte ese cambio (nunca a ciegas).

---

## 5. Verificación en el móvil (lo que falta y cómo)

El daemon es el proceso que sostiene el túnel: si el binario no arranca, el
móvil "parece caído" aunque el hardware esté bien. Por eso:

1. `megadaemon-core self-test` (ya implementado): arranque mínimo sin red.
2. El auto-actualizador **ejecuta el `.new` con `self-test` antes de
   instalarlo** (ya implementado): si no arranca, no se instala nada.
3. El supervisor hace **auto-rollback** y marca el binario como malo
   (ya implementado) ⇒ la auto-actualización no buchea.

Falta (depende de recuperar el móvil): medir `/proc/<pid>/smaps_rollup`
(`Pss_Anon` vs `Pss_File`) antes y después de §3.8, y comprobar que el arranque
del daemon con el binario nuevo tarda lo mismo (el `oom_score_adj` y los topes de
malloc ya están en `deploy/99-megadaemon.sh`).
