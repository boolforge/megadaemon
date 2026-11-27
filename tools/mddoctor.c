/* ==========================================================================
 * mddoctor.c — EL MÉDICO DE MEGADAEMON (núcleo forense y recuperación)
 * ==========================================================================
 *
 * QUÉ ES
 *   Un binario diminuto, sin dependencias (solo libc), que:
 *     1. RECOGE el estado real del sistema y del daemon (batería, térmica,
 *        memoria, disco, red, procesos, estado de MegaDaemon, logs).
 *     2. CLASIFICA cada hallazgo (OK / AVISO / CRITICO) con el motivo.
 *     3. PROpone (o aplica) una RECUPERACIÓN segura y concreta.
 *
 * POR QUÉ EN C Y NO EN RUST
 *   - Compila en <1 s con `gcc`, sin deps: en el sandbox (2 CPU, 3 GB sin swap)
 *     el enlazado del crate de Rust de 160 MB MUERE por OOM (SIGKILL medido).
 *     Un diagnóstico tiene que poder compilarse donde no hay RAM.
 *   - ~20 KB estático: se copia al móvil con un `cat` y arranca en cualquier
 *     Android/Termux, con o sin root.
 *   - NO depende de la libreria de MegaDaemon: precisamente tiene que
 *     funcionar cuando el daemon está roto (que es cuando hace falta).
 *
 * CÓMO SE USA
 *   mddoctor scan            # sólo recoge y lista los hechos
 *   mddoctor doctor          # recoge + clasifica + dice qué está mal
 *   mddoctor doctor --json   # lo mismo, en JSON (para otros programas)
 *   mddoctor recover         # aplica la recuperación SEGURA (sin --yes pide
 *                             confirmacion; sin --apply no hace nada)
 *   mddoctor recover --apply --yes
 *
 * CODIGOS DE SALIDA (para scripts y para el CLI de MegaDaemon)
 *   0 = todo OK
 *   1 = hay avisos
 *   2 = hay hallazgos criticos
 *   3 = error de uso
 *
 * COMPILAR
 *   gcc -O2 -Wall -o mddoctor tools/mddoctor.c
 *   # Android/ARM64:  aarch64-linux-android-clang -O2 -o mddoctor-arm64 ...
 *
 * SIN DEPENDENCIAS: ni curl, ni python, ni jq. Todo se lee de /proc y /sys.
 * ========================================================================== */

#define _GNU_SOURCE
#include <stdio.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <dirent.h>
#include <errno.h>
#include <time.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <netdb.h>
#include <sys/select.h>
#include <signal.h>
#include <stdint.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <stdint.h>

#define MAXF 512
#define MAXH 96
static char host[256] = "desconocido";

/* Versión y esquema: los que integran este binario (el tool MCP `doctor`,
 * la Web UI, los scripts) necesitan saber QUÉ CONTRATO ofrece. */
#define MDDOCTOR_VERSION "1.2.0"
#define MDDOCTOR_SCHEMA "megadaemon.doctor/1"

/* Prototipos de las secciones que se definen más abajo del fichero. */
static int status_hex(const char *clave, uint64_t *val);
static void status_txt(const char *clave, char *out, size_t on);
static void capacidades(uint64_t mask, char *out, size_t on);
static void montajes(char *out, size_t on);
static void limites(char *out, size_t on);
static int tcp_puerto(const char *host, int port, int ms);
static void elf_info(const char *path, char *out, size_t on);
static int matar(const char *patron, int espera_ms);
static void url_host(const char *url, char *out, size_t on);
static const char *prefijo(void);
static const char *serviced(void);

/* ---------------------------------------------------------------------- */
/* OPTIMIZACIÓN MIXTA: velocidad donde importa, tamaño donde NO.          */
/* MEDIDO (2026-11-27): el SHA-256 de 64 MB tardaba 1.261 ms con -O2 y   */
/* 1.834 ms con -Os (-45% de rapidez).  El binario entero con -Os pesa   */
/* 43.576 B y con -O2 51.416 B (+18%), pero la RAPIDEZ es una capacidad   */
/* que no se negocia.  Solución: -O2 global + `MD_HOT` (O3) en el camino  */
/* crítico y `MD_COLD` (Os) en las funciones voluminosas, que se ejecutan*/
/* una vez y están limitadas por E/S, no por CPU.  Y `cold` además las  */
/* aparta del camino caliente de la caché de instrucciones.             */
/* ---------------------------------------------------------------------- */
/* `hot`/`cold` los entienden TANTO gcc COMO clang (a diferencia de
 * __attribute__((optimize)), y apartan el codigo frio del camino caliente
 * de la cache de instrucciones.  MEDIDO: añadir `optimize("O3")` al camino
 * critico NO daba ninguna ganancia de velocidad (1.096 ms vs 1.111 ms con
 * 64 MB de SHA-256, dentro del ruido del sandbox) y soplaba el codigo:
 * asi que solo se queda `hot`/`cold`, que es lo que si se nota. */
#if defined(__GNUC__) || defined(__clang__)
#define MD_HOT __attribute__((hot))
#define MD_COLD __attribute__((cold))
#else
#define MD_HOT
#define MD_COLD
#endif

/* ---------------------------------------------------------------------- */
/* Utilidades                                                               */
/* ---------------------------------------------------------------------- */

static void hostname(void) {
    if (gethostname(host, sizeof(host) - 1) != 0) strcpy(host, "?");
}

/* Prefijos de donde vive el daemon en el movil.  Por defecto son los del
 * telefono, pero se pueden mover con el entorno: asi el banco de pruebas
 * (tools/test-mddoctor.c) puede trabajar en un directorio temporal, y un
 * movil con otro layout no necesita recompilar el binario. */
static const char *prefijo(void) {
    const char *p = getenv("MDDOCTOR_PREFIX");
    return (p && *p) ? p : "/data/local/tmp/megadaemon";
}

static const char *serviced(void) {
    const char *p = getenv("MDDOCTOR_SERVICE_D");
    return (p && *p) ? p : "/data/adb/service.d";
}

static int read_file(const char *path, char *buf, size_t n) {
    int fd = open(path, O_RDONLY);
    if (fd < 0) return -1;
    ssize_t r = read(fd, buf, n - 1);
    close(fd);
    if (r < 0) return -1;
    buf[r] = 0;
    return 0;
}

/* Quita el salto de linea y los espacios del final: `read_file` deja el
 * "\n" y comparar "up\n" con "up" NO casa (fallo real: ninguna interfaz
 * de red contaba como arriba). */
static void trim(char *s) {
    if (!s) return;
    size_t n = strlen(s);
    while (n && (s[n - 1] == '\n' || s[n - 1] == '\r' || s[n - 1] == ' ' ||
                 s[n - 1] == '\t'))
        s[--n] = 0;
}

/* Primer entero que aparece en el fichero. -1 si no hay. */
static long first_int(const char *path) {
    char b[256];
    if (read_file(path, b, sizeof(b)) != 0) return -1;
    char *p = b;
    while (*p && (*p < '0' || *p > '9')) p++;
    if (!*p) return -1;
    return strtol(p, NULL, 10);
}

/* Valor de una clave "clave: valor" de /proc/meminfo. -1 si no existe. */
static long meminfo_kb(const char *clave) {
    FILE *f = fopen("/proc/meminfo", "r");
    if (!f) return -1;
    char line[256];
    long v = -1;
    size_t kl = strlen(clave);
    while (fgets(line, sizeof(line), f)) {
        if (strncmp(line, clave, kl) == 0) {
            v = strtol(line + kl, NULL, 10);
            break;
        }
    }
    fclose(f);
    return v;
}

/* ---------------------------------------------------------------------- */
/* Hallazgos: una lista de (clave, valor, veredicto, motivo)                */
/* ---------------------------------------------------------------------- */

#define VER_OK 0
#define VER_AVISO 1
#define VER_CRITICO 2

typedef struct {
    char clave[64];
    char valor[220];
    int  veredicto;
    char motivo[180];
} Hallazgo;

static Hallazgo H[MAXH];
static int NH = 0;
static int peor = VER_OK;

/* Dos versiones a proposito: la mayoria de las llamadas tienen el motivo
 * LITERAL, y `add()` sin variadicos evita el `va_start` en esos puntos de
 * llamada (el binario es pequeno a proposito: va al movil con un `cat`). */
static void add(const char *clave, const char *valor, int ver,
                const char *motivo) {
    if (NH >= MAXH) return;
    Hallazgo *h = &H[NH++];
    snprintf(h->clave, sizeof(h->clave), "%s", clave);
    snprintf(h->valor, sizeof(h->valor), "%s", valor);
    h->veredicto = ver;
    snprintf(h->motivo, sizeof(h->motivo), "%s", motivo);
    if (ver > peor) peor = ver;
}

/* La version con formato, para los pocos motivos que llevan datos. */
static void addf(const char *clave, const char *valor, int ver,
                 const char *motivo_fmt, ...) {
    if (NH >= MAXH) return;
    Hallazgo *h = &H[NH++];
    snprintf(h->clave, sizeof(h->clave), "%s", clave);
    snprintf(h->valor, sizeof(h->valor), "%s", valor);
    h->veredicto = ver;
    va_list ap;
    va_start(ap, motivo_fmt);
    vsnprintf(h->motivo, sizeof(h->motivo), motivo_fmt, ap);
    va_end(ap);
    if (ver > peor) peor = ver;
}

static const char *ver_txt(int v) {
    return v == VER_OK ? "OK" : (v == VER_AVISO ? "AVISO" : "CRITICO");
}

/* ---------------------------------------------------------------------- */
/* Recogida: bateria, termica, memoria, disco, red, procesos, daemon        */
/* ---------------------------------------------------------------------- */

/* Devuelve el % de bateria; si no hay, -1. */
static int bateria_pct(char *detalle, size_t dn) {
    DIR *d = opendir("/sys/class/power_supply");
    if (!d) return -1;
    struct dirent *e;
    int pct = -1;
    char path[512];
    while ((e = readdir(d))) {
        if (e->d_name[0] == '.') continue;
        snprintf(path, sizeof(path), "/sys/class/power_supply/%s/type", e->d_name);
        char t[64];
        if (read_file(path, t, sizeof(t)) != 0) continue;
        if (strncmp(t, "Battery", 7) != 0) continue;
        snprintf(path, sizeof(path), "/sys/class/power_supply/%s/capacity", e->d_name);
        long c = first_int(path);
        if (c < 0) continue;
        pct = (int)c;
        if (detalle && dn) {
            char st[32] = "?";
            snprintf(path, sizeof(path), "/sys/class/power_supply/%s/status", e->d_name);
            read_file(path, st, sizeof(st));
            trim(st);
            snprintf(detalle, dn, "%ld%% %s", c, st);
        }
        break;
    }
    closedir(d);
    return pct;
}

/* Temperatura maxima en mili-grados; -1 si no hay sensor. */
static long temp_max(char *tipo, size_t tn) {
    DIR *d = opendir("/sys/class/thermal");
    if (!d) return -1;
    struct dirent *e;
    long max = -1;
    char path[512];
    while ((e = readdir(d))) {
        if (strncmp(e->d_name, "thermal_zone", 12) != 0) continue;
        snprintf(path, sizeof(path), "/sys/class/thermal/%s/temp", e->d_name);
        long t = first_int(path);
        if (t < 0) continue;
        if (t > max) {
            max = t;
            if (tipo && tn) {
                snprintf(path, sizeof(path), "/sys/class/thermal/%s/type", e->d_name);
                read_file(path, tipo, tn);
                trim(tipo);
            }
        }
    }
    closedir(d);
    return max;
}

/* Espacio libre en MiB de un path; -1 si no se puede. */
static long disco_libre_mb(const char *path) {
    struct statvfs s;
    if (statvfs(path, &s) != 0) return -1;
    return (long)((double)s.f_bavail * (double)s.f_frsize / (1024.0 * 1024.0));
}


/* ---------------------------------------------------------------------- */
/* PERMISOS Y ENTORNO: que se puede hacer aqui y ahora                      */
/* ---------------------------------------------------------------------- */

static int existe(const char *p) {
    struct stat s;
    return stat(p, &s) == 0;
}

/* Se puede escribir en este directorio?  Lo intenta de verdad. */
static int escribible(const char *dir) {
    char t[1200];
    snprintf(t, sizeof(t), "%s/.mddoctor_probe", dir);
    FILE *f = fopen(t, "w");
    if (!f) return 0;
    fclose(f);
    unlink(t);
    return 1;
}

static void permisos(void) {
    char v[200];
    uid_t e = geteuid();
    snprintf(v, sizeof(v), "euid=%d uid=%d", (int)e, (int)getuid());
    add("permisos", v, e == 0 ? VER_OK : VER_AVISO,
        e == 0 ? "root: se puede actuar en el sistema (ajustes, hooks, reinicios)"
               : "sin root: los ajustes de sistema y los hooks NO son posibles");

    char enf[16];
    if (read_file("/sys/fs/selinux/enforce", enf, sizeof(enf)) == 0) {
        trim(enf);
        int perm = (enf[0] == '0');
        add("selinux", enf, perm ? VER_OK : VER_AVISO,
            perm ? "SELinux permisivo: los hooks se pueden poner sin pelea"
                  : "SELinux enforcing: hookear system_server exige el shim de Java (aun no existe)");
    }

    struct statvfs sf;
    if (statvfs("/system", &sf) == 0) {
        int ro = (sf.f_flag & ST_RDONLY) != 0;
        add("sistema-fs", ro ? "/system solo lectura" : "/system escribible",
            ro ? VER_OK : VER_AVISO,
            ro ? "normal en Android: no se puede tocar el sistema"
               : "/system escribible: hay Magisk o una imagen modificada");
    }

    /* Herramientas que el entorno puede usar de verdad */
    struct { const char *n; const char *r; int clave; } h[] = {
        {"su", "/system/bin/su", 1},
        {"magisk", "/sbin/magisk", 0},
        {"busybox", "/data/adb/magisk/busybox", 0},
        {"curl", "/system/bin/curl", 0},
        {"wget", "/system/bin/wget", 0},
        {"adb", "/system/bin/adb", 0},
        {"termux", "/data/data/com.termux/files/usr/bin/bash", 0},
        {NULL, NULL, 0}
    };
    char faltan[256] = "", hay[256] = "";
    for (int i = 0; h[i].n; i++) {
        if (existe(h[i].r)) {
            if (strlen(hay) + 4 < sizeof(hay)) { strcat(hay, h[i].n); strcat(hay, ","); }
        } else if (h[i].clave && strlen(faltan) + 4 < sizeof(faltan)) {
            strcat(faltan, h[i].n); strcat(faltan, ",");
        }
    }
    if (faltan[0])
        add("herramientas", faltan, VER_AVISO, "faltan herramientas clave en el entorno");
    else
        add("herramientas", hay[0] ? hay : "ninguna", VER_OK, "el entorno tiene lo basico");

    const char *sd = getenv("MEGADAEMON_STATE_DIR");
    if (!sd || !*sd) sd = "/data/user/0/com.agy/files/state";
    if (existe(sd)) {
        int w = escribible(sd);
        add("estado-escribible", w ? "si" : "no", w ? VER_OK : VER_CRITICO,
            w ? "el daemon puede escribir su estado"
              : "el estado NO es escribible: el daemon no arranca");
    }

    /* INTROSPECCIÓN REAL DEL ENTORNO (todo sale de /proc, sin getcap ni
     * capsh, que no existen en Android). */
    uint64_t cap = 0;
    if (status_hex("CapEff", &cap) == 0) {
        char caps[220];
        capacidades(cap, caps, sizeof(caps));
        int es_root = (cap & (1ULL << 0)) && (cap & (1ULL << 3)); /* CHOWN+FOWNER */
        add("capacidades", caps, es_root ? VER_OK : VER_AVISO,
            es_root ? "el proceso tiene capacidades casi de root"
                    : "capacidades limitadas: no puede tocar el sistema");
    }
    char seccomp[64], nnp[64];
    status_txt("Seccomp", seccomp, sizeof(seccomp));
    status_txt("NoNewPrivs", nnp, sizeof(nnp));
    snprintf(v, sizeof(v), "seccomp=%s no_new_privs=%s", seccomp, nnp);
    add("aislamiento", v, VER_OK,
        "sandbox del kernel: seccomp filtra las llamadas al sistema");

    char mont[220];
    montajes(mont, sizeof(mont));
    add("montajes", mont, VER_OK, "particiones del sistema y si son escribibles");

    char lim[220];
    limites(lim, sizeof(lim));
    add("limites", lim, VER_OK, "limites del proceso (ficheros, pila, memoria)");
}

/* ---------------------------------------------------------------------- */
/* FORENSE DE DISCO: que se esta comiendo el espacio                        */
/* ---------------------------------------------------------------------- */

#define MAXG 32
typedef struct { char ruta[160]; long bytes; } Grande;
static Grande G[MAXG];
static int NG = 0;

static void escanear(const char *dir, int prof, long *tot, int *nf, long *viejos) {
    if (prof > 3) return;
    DIR *d = opendir(dir);
    if (!d) return;
    struct dirent *e;
    time_t ahora = time(NULL);
    while ((e = readdir(d))) {
        if (!strcmp(e->d_name, ".") || !strcmp(e->d_name, "..")) continue;
        char p[1024];
        snprintf(p, sizeof(p), "%s/%s", dir, e->d_name);
        struct stat st;
        if (lstat(p, &st) != 0) continue;
        if (S_ISDIR(st.st_mode)) { escanear(p, prof + 1, tot, nf, viejos); continue; }
        if (!S_ISREG(st.st_mode)) continue;
        (*nf)++;
        *tot += st.st_size;
        if (ahora - st.st_mtime > 30L * 86400) (*viejos)++;
        if (NG < MAXG) {
            snprintf(G[NG].ruta, sizeof(G[NG].ruta), "%.150s", p);
            G[NG].bytes = st.st_size;
            NG++;
        }
    }
    closedir(d);
}

MD_COLD static void forense(const char *dir) {
    long tot = 0, viejos = 0;
    int nf = 0;
    NG = 0;
    escanear(dir, 0, &tot, &nf, &viejos);
    for (int i = 1; i < NG; i++) {          /* orden por tamano (desc) */
        Grande t = G[i];
        int j = i - 1;
        while (j >= 0 && G[j].bytes < t.bytes) { G[j + 1] = G[j]; j--; }
        G[j + 1] = t;
    }
    char v[200];
    snprintf(v, sizeof(v), "%d ficheros, %ld KiB, %ld de mas de 30 dias", nf, tot / 1024, viejos);
    add("forense-estado", v, nf ? VER_OK : VER_AVISO, "contenido del directorio de estado");
    for (int i = 0; i < NG && i < 3; i++) {
        char k[80], val[220];
        snprintf(k, sizeof(k), "fichero-grande-%d", i + 1);
        snprintf(val, sizeof(val), "%ld KiB  %.180s", G[i].bytes / 1024, G[i].ruta);
        add(k, val, VER_OK, "ocupa espacio");
    }
}

/* ---------------------------------------------------------------------- */
/* AUTOTEST: el propio diagnostico se prueba a si mismo                    */
/* ---------------------------------------------------------------------- */

MD_COLD static int autotest(void) {
    struct { const char *n; const char *r; } p[] = {
        {"meminfo", "/proc/meminfo"},
        {"loadavg", "/proc/loadavg"},
        {"uptime", "/proc/uptime"},
        {"self-status", "/proc/self/status"},
        {"mounts", "/proc/mounts"},
        {"pressure-cpu", "/proc/pressure/cpu"},
        {"power_supply", "/sys/class/power_supply"},
        {"thermal", "/sys/class/thermal"},
        {"class-net", "/sys/class/net"},
        {NULL, NULL}
    };
    int ok = 0, total = 0, fallos = 0;
    printf("== AUTOTEST (las sondas que usa este binario) ==\n");
    for (int i = 0; p[i].n; i++) {
        char buf[64];
        total++;
        if (read_file(p[i].r, buf, sizeof(buf)) == 0) {
            ok++;
            printf("  [ OK  ] %-14s %s\n", p[i].n, p[i].r);
        } else {
            fallos++;
            printf("  [FALLO] %-14s %s\n", p[i].n, p[i].r);
        }
    }
    printf("autotest: %d/%d sondas responden\n", ok, total);
    return fallos;
}

/* ---------------------------------------------------------------------- */
/* Clasificacion                                                            */
/* ---------------------------------------------------------------------- */

MD_COLD static void clasificar(void) {
    char det[128];
    permisos();
    int pct;

    /* --- bateria ------------------------------------------------------- */
    pct = bateria_pct(det, sizeof(det));
    if (pct < 0) {
        add("bateria", "sin sensor", VER_OK, "no aplica (no hay power_supply)");
    } else if (pct <= 15) {
        addf("bateria", det, VER_CRITICO,
             "bateria al %d%%: el daemon puede morir en cualquier momento", pct);
    } else if (pct <= 25) {
        addf("bateria", det, VER_AVISO,
             "bateria al %d%%: conviene modo ahorro y nada de trabajo pesado", pct);
    } else {
        add("bateria", det, VER_OK, "carga suficiente");
    }

    /* --- termica -------------------------------------------------------- */
    char tipo[64] = "?";
    long t = temp_max(tipo, sizeof(tipo));
    if (t < 0) {
        add("termica", "sin sensores", VER_OK, "no aplica");
    } else {
        char v[64];
        snprintf(v, sizeof(v), "%.1f C (%.40s)", t / 1000.0, tipo);
        if (t >= 45000)
            add("termica", v, VER_CRITICO,
                ">=45 C: el SoC va a limitar o parar (el movil se apaga solo)");
        else if (t >= 40000)
            add("termica", v, VER_AVISO, ">=40 C: va-caliente, el trabajo pesado lo acorta");
        else
            add("termica", v, VER_OK, "temperatura normal");
    }

    /* --- memoria -------------------------------------------------------- */
    long tot = meminfo_kb("MemTotal:"), av = meminfo_kb("MemAvailable:");
    long swt = meminfo_kb("SwapTotal:"), swf = meminfo_kb("SwapFree:");
    if (tot > 0 && av > 0) {
        char v[96];
        snprintf(v, sizeof(v), "%ld MiB total, %ld MiB libres", tot / 1024, av / 1024);
        long pctused = 100 - (av * 100 / tot);
        if (pctused >= 92)
            addf("memoria", v, VER_CRITICO,
                 "%ld%% usada: el OOM killer esta a la vuelta de la esquina", pctused);
        else if (pctused >= 80)
            addf("memoria", v, VER_AVISO, "%ld%% usada: sin swap, roza el limite", pctused);
        else
            addf("memoria", v, VER_OK, "%ld%% usada", pctused);
    } else {
        add("memoria", "sin /proc/meminfo", VER_OK, "no aplica");
    }
    if (swt > 0) {
        char v[96];
        snprintf(v, sizeof(v), "%ld MiB swap, %ld MiB libres", swt / 1024, swf / 1024);
        add("swap", v, VER_OK, "hay swap");
    } else {
        add("swap", "0 MiB", VER_AVISO,
            "sin swap: un pico de RAM mata el proceso de golpe (no hay red de seguridad)");
    }

    /* --- presion de CPU (PSI): el indicador fino de "voy a morir" -------- */
    char psi[256];
    if (read_file("/proc/pressure/cpu", psi, sizeof(psi)) == 0) {
        char *some = strstr(psi, "some");
        double a10 = 0;
        if (some) {
            char *p = strstr(some, "avg10=");
            if (p) a10 = atof(p + 6);
        }
        char v[96];
        snprintf(v, sizeof(v), "cpu some avg10=%.1f%%", a10);
        if (a10 >= 40.0)
            add("presion-cpu", v, VER_CRITICO,
                "la CPU esta ahogada: el scheduler va a matar procesos");
        else if (a10 >= 15.0)
            add("presion-cpu", v, VER_AVISO, "CPU presionada");
        else
            add("presion-cpu", v, VER_OK, "sin presion");
    }

    /* --- carga ---------------------------------------------------------- */
    char la[128];
    if (read_file("/proc/loadavg", la, sizeof(la)) == 0) {
        char v[96];
        double l1 = 0, l5 = 0, l15 = 0;
        /* sscanf y noOffsets: "/proc/loadavg" no tiene los numeros en
         * posiciones fijas ("1.34 0.49 0.17 ..."), asi que leerlos con
         * atof(la+7) daria numeros equivocados. */
        if (sscanf(la, "%lf %lf %lf", &l1, &l5, &l15) != 3) { l1 = l5 = l15 = 0; }
        snprintf(v, sizeof(v), "%.2f %.2f %.2f", l1, l5, l15);
        if (l1 >= 4.0)
            add("carga", v, VER_CRITICO, "load1 >= 4: todo va lento y en cola");
        else if (l1 >= 2.0)
            add("carga", v, VER_AVISO, "load1 alto");
        else
            add("carga", v, VER_OK, "carga normal");
    }

    /* --- disco ---------------------------------------------------------- */
    static const char *rutas[] = {"/data", "/storage/emulated/0", "/", NULL};
    for (int i = 0; rutas[i]; i++) {
        long mb = disco_libre_mb(rutas[i]);
        if (mb < 0) continue;
        char v[96];
        snprintf(v, sizeof(v), "%ld MiB libres", mb);
        if (mb < 200)
            addf("disco", v, VER_CRITICO, "%s: casi sin sitio; el daemon no puede escribir", rutas[i]);
        else if (mb < 800)
            addf("disco", v, VER_AVISO, "%s: poco sitio", rutas[i]);
        else
            addf("disco", v, VER_OK, "%s: sitio de sobra", rutas[i]);
    }

    /* --- red: trafico por interfaz y estado ------------------------------ */
    DIR *nd = opendir("/sys/class/net");
    if (nd) {
        struct dirent *e;
        int ifaces = 0, arriba = 0;
        char lista[512] = "";
        while ((e = readdir(nd))) {
            if (e->d_name[0] == '.') continue;
            if (!strcmp(e->d_name, "lo")) continue;
            ifaces++;
            char path[512], st[32];
            snprintf(path, sizeof(path), "/sys/class/net/%s/operstate", e->d_name);
            read_file(path, st, sizeof(st));
            trim(st);
            if (!strcmp(st, "up") || !strcmp(st, "unknown")) {
                arriba++;
                if (strlen(lista) < 400)
                    strncat(lista, e->d_name, sizeof(lista) - strlen(lista) - 2);
            }
        }
        closedir(nd);
        char v[256];
        snprintf(v, sizeof(v), "%d iface, %d arriba [%.200s]", ifaces, arriba, lista);
        if (ifaces == 0)
            add("red", v, VER_CRITICO, "sin interfaces: no hay red (ni movil ni wifi)");
        else if (arriba == 0)
            add("red", v, VER_CRITICO, "ninguna interfaz arriba: el daemon no puede publicar");
        else
            add("red", v, VER_OK, "hay ConnectivityManager y tunel");
    }

    /* --- procesos: zombies y estado D ------------------------------------ */
    int zomb = 0, dstat = 0, total = 0, md = 0;
    DIR *pd = opendir("/proc");
    if (pd) {
        struct dirent *e;
        while ((e = readdir(pd))) {
            if (e->d_name[0] < '0' || e->d_name[0] > '9') continue;
            total++;
            char path[512], buf[512];
            snprintf(path, sizeof(path), "/proc/%s/stat", e->d_name);
            if (read_file(path, buf, sizeof(buf)) == 0) {
                char *p = strrchr(buf, ')');
                if (p && p[1] == ' ' && p[2] == 'Z') zomb++;
                if (p && p[1] == ' ' && p[2] == 'D') dstat++;
            }
            snprintf(path, sizeof(path), "/proc/%s/comm", e->d_name);
            if (read_file(path, buf, sizeof(buf)) == 0 && strstr(buf, "megadaemon"))
                md++;
        }
        closedir(pd);
    }
    char v[96];
    snprintf(v, sizeof(v), "%d procesos, %d zombie, %d en D", total, zomb, dstat);
    if (zomb >= 20)
        addf("procesos", v, VER_AVISO, "%d zombies: alguien no los esta recogiendo", zomb);
    else
        add("procesos", v, VER_OK, "sin acumulacion de zombies");
    snprintf(v, sizeof(v), "%d procesos de MegaDaemon", md);
    if (md == 0)
        add("daemon-procesos", v, VER_AVISO, "no hay ningun proceso de MegaDaemon vivo");
    else
        add("daemon-procesos", v, VER_OK, "el daemon esta vivo");

    /* --- estado de MegaDaemon ------------------------------------------- */
    const char *sd = getenv("MEGADAEMON_STATE_DIR");
    if (!sd || !*sd) sd = "/data/user/0/com.agy/files/state";
    char f[512];
    snprintf(f, sizeof(f), "%s", sd);
    struct stat st;
    if (stat(f, &st) != 0) {
        add("estado-dir", f, VER_AVISO,
            "el directorio de estado no existe: el daemon nunca arranco aqui");
    } else {
        snprintf(f, sizeof(f), "%s/instance.json", sd);
        if (stat(f, &st) == 0)
            add("estado-dir", sd, VER_OK, "instance.json presente");
        else
            add("estado-dir", sd, VER_AVISO, "sin instance.json: el nodo no se ha registrado");
        snprintf(f, sizeof(f), "%s/channel.json", sd);
        if (stat(f, &st) == 0)
            add("canal", "channel.json presente", VER_OK, "el canal de publicacion esta");
        else
            add("canal", "sin channel.json", VER_AVISO,
                "sin canal: el root no sabe donde conectarse");
        forense(sd);
    snprintf(f, sizeof(f), "%s/tunnel.url", sd);
        if (stat(f, &st) == 0) {
            char u[256];
            read_file(f, u, sizeof(u));
            char *nl = strpbrk(u, "\r\n");
            if (nl) *nl = 0;
            add("tunel", u, VER_OK, "URL del tunel publicada");
        } else {
            add("tunel", "sin tunnel.url", VER_AVISO,
                "sin URL de tunel: el root no tiene entrada al movil");
        }
    }
}

/* ---------------------------------------------------------------------- */
/* Recuperacion (solo acciones SEGURAS y explicitas)                       */
/* ---------------------------------------------------------------------- */

static int rm_older(const char *dir, int dias, int *borrados) {
    DIR *d = opendir(dir);
    if (!d) return -1;
    struct dirent *e;
    time_t ahora = time(NULL);
    int n = 0;
    while ((e = readdir(d))) {
        if (!strcmp(e->d_name, ".") || !strcmp(e->d_name, "..")) continue;
        if (e->d_name[0] == '.') continue;
        char path[1200];
        snprintf(path, sizeof(path), "%.1100s/%.90s", dir, e->d_name);
        struct stat st;
        if (stat(path, &st) != 0) continue;
        if (!S_ISREG(st.st_mode)) continue;
        if (ahora - st.st_mtime < (time_t)dias * 86400) continue;
        if (unlink(path) == 0) n++;
    }
    closedir(d);
    *borrados += n;
    return n;
}

static int recuperar(int aplicar) {
    const char *sd = getenv("MEGADAEMON_STATE_DIR");
    if (!sd || !*sd) sd = "/data/user/0/com.agy/files/state";
    char tmp[1024], f[1200];
    int borrados = 0, acciones = 0;

    printf("== RECUPERACION ==\n");
    printf("estado: %s\n", sd);

    /* 1) temporales viejos en el estado (>7 dias): se van solos */
    snprintf(tmp, sizeof(tmp), "%s/tmp", sd);
    int n = rm_older(tmp, 7, &borrados);
    if (n > 0)
        printf("  [hecho] %d temporales de mas de 7 dias borrados de %s\n", n, tmp);
    else
        printf("  [nada]  temporales: %s\n", n < 0 ? "no existe" : "ninguno antiguo");

    /* 2) locks rancios: un lock mas viejo que 1 hora es de un proceso muerto */
    DIR *d = opendir(sd);
    if (d) {
        struct dirent *e;
        time_t ahora = time(NULL);
        while ((e = readdir(d))) {
            size_t l = strlen(e->d_name);
            if (l < 5 || strcmp(e->d_name + l - 5, ".lock")) continue;
            snprintf(f, sizeof(f), "%s/%s", sd, e->d_name);
            struct stat st;
            if (stat(f, &st) == 0 && ahora - st.st_mtime > 3600) {
                if (aplicar) {
                    if (unlink(f) == 0) {
                        printf("  [hecho] lock rancio borrado: %s\n", e->d_name);
                        acciones++;
                    }
                } else {
                    printf("  [plan]  lock rancio (mas de 1 h): %s\n", e->d_name);
                    acciones++;
                }
            }
        }
        closedir(d);
    }
    if (!acciones) printf("  [nada]  locks: ninguno rancio\n");

    /* 3) el tunel: si hay URL pero el daemon no esta vivo, hay que relanzar */
    snprintf(f, sizeof(f), "%s/tunnel.url", sd);
    struct stat st;
    if (stat(f, &st) == 0) {
        printf("  [info]  hay URL de tunel: el root puede entrar al movil\n");
    } else {
        printf("  [plan]  sin tunnel.url: relanzar el stack (cloudflared + daemon)\n");
        acciones++;
    }

    printf("== fin ==\n");
    if (!aplicar)
        printf("nada se ha aplicado (usa --apply para ejecutar, y --yes para no preguntar)\n");
    return acciones;
}

/* ---------------------------------------------------------------------- */
/* Salida                                                                   */
/* ---------------------------------------------------------------------- */

MD_COLD static void imprimir_tabla(void) {
    printf("%-16s %-44s %-8s %s\n", "CLAVE", "VALOR", "VEREDICTO", "MOTIVO");
    printf("----------------------------------------------------------------------------------------------------------------\n");
    for (int i = 0; i < NH; i++)
        printf("%-16s %-44s %-8s %s\n", H[i].clave, H[i].valor, ver_txt(H[i].veredicto),
               H[i].motivo);
    printf("----------------------------------------------------------------------------------------------------------------\n");
    int nok = 0, nwa = 0, ncr = 0;
    for (int i = 0; i < NH; i++) {
        if (H[i].veredicto == VER_OK) nok++;
        else if (H[i].veredicto == VER_AVISO) nwa++;
        else ncr++;
    }
    printf("resumen: %d OK, %d avisos, %d criticos\n", nok, nwa, ncr);
}

/* La huella de memoria del propio binario (VmHWM = pico de RSS).  Se
 * publica en el JSON para que el consumidor vea el coste real. */
static long memoria_kb(void) {
    char b[64];
    status_txt("VmHWM", b, sizeof(b));
    return atol(b);
}

MD_COLD static void imprimir_json(void) {
    printf("{\n  \"schema\": \"%s\",\n  \"version\": \"%s\",\n  \"host\": \"%s\",\n"
           "  \"memoria_kb\": %ld,\n  \"veredicto\": \"%s\",\n  \"hallazgos\": [\n",
           MDDOCTOR_SCHEMA, MDDOCTOR_VERSION, host, memoria_kb(), ver_txt(peor));
    for (int i = 0; i < NH; i++) {
        printf("    {\"clave\": \"%s\", \"valor\": \"%s\", \"veredicto\": \"%s\", \"motivo\": \"%s\"}%s\n",
               H[i].clave, H[i].valor, ver_txt(H[i].veredicto), H[i].motivo,
               i + 1 < NH ? "," : "");
    }
    printf("  ]\n}\n");
}

/* ---------------------------------------------------------------------- */

/* ---------------------------------------------------------------------- */
/* SHA-256: para verificar el binario EN SERVICIO contra el .sha256        */
/* ---------------------------------------------------------------------- */

typedef struct { uint32_t h[8]; uint64_t bits; unsigned char buf[64]; size_t n; } Sha256;

static const uint32_t SHA_K[64] = {
0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2};

static uint32_t ror32(uint32_t x, int n) { return (x >> n) | (x << (32 - n)); }

MD_HOT static void sha256_block(Sha256 *s, const unsigned char *p) {
    uint32_t w[64];
    for (int i = 0; i < 16; i++)
        w[i] = ((uint32_t)p[i*4] << 24) | ((uint32_t)p[i*4+1] << 16) |
               ((uint32_t)p[i*4+2] << 8) | (uint32_t)p[i*4+3];
    for (int i = 16; i < 64; i++) {
        uint32_t s0 = ror32(w[i-15], 7) ^ ror32(w[i-15], 18) ^ (w[i-15] >> 3);
        uint32_t s1 = ror32(w[i-2], 17) ^ ror32(w[i-2], 19) ^ (w[i-2] >> 10);
        w[i] = w[i-16] + s0 + w[i-7] + s1;
    }
    uint32_t a = s->h[0], b = s->h[1], c = s->h[2], d = s->h[3];
    uint32_t e = s->h[4], f = s->h[5], g = s->h[6], h = s->h[7];
    for (int i = 0; i < 64; i++) {
        uint32_t S1 = ror32(e, 6) ^ ror32(e, 11) ^ ror32(e, 25);
        uint32_t ch = (e & f) ^ ((~e) & g);
        uint32_t t1 = h + S1 + ch + SHA_K[i] + w[i];
        uint32_t S0 = ror32(a, 2) ^ ror32(a, 13) ^ ror32(a, 22);
        uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
        uint32_t t2 = S0 + maj;
        h = g; g = f; f = e; e = d + t1; d = c; c = b; b = a; a = t1 + t2;
    }
    s->h[0] += a; s->h[1] += b; s->h[2] += c; s->h[3] += d;
    s->h[4] += e; s->h[5] += f; s->h[6] += g; s->h[7] += h;
}

/* sha256 en hexadecimal de un fichero.  Deja "?" si no se puede leer.
 * Sirve para COMPARAR el binario que corre en el movil con el .sha256 que
 * publica el CI: si no coinciden, hay una build vieja (o manipulada). */
static void sha256_hex(const char *path, char *hex) {
    snprintf(hex, 65, "?");
    int fd = open(path, O_RDONLY);
    if (fd < 0) return;
    Sha256 s = {{0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,
                 0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19},0,{0},0};
    unsigned char buf[8192];
    ssize_t r;
    while ((r = read(fd, buf, sizeof(buf))) > 0) {
        for (ssize_t i = 0; i < r; i++) {
            s.buf[s.n++] = buf[i];
            if (s.n == 64) { sha256_block(&s, s.buf); s.n = 0; }
            s.bits += 8;
        }
    }
    close(fd);
    s.buf[s.n++] = 0x80;
    if (s.n > 56) { while (s.n < 64) s.buf[s.n++] = 0; sha256_block(&s, s.buf); s.n = 0; }
    while (s.n < 56) s.buf[s.n++] = 0;
    for (int i = 0; i < 8; i++) s.buf[56 + i] = (unsigned char)(s.bits >> (56 - 8 * i));
    sha256_block(&s, s.buf);
    static const char *hx = "0123456789abcdef";
    for (int i = 0; i < 8; i++)
        for (int j = 0; j < 4; j++) {
            unsigned char b = (unsigned char)(s.h[i] >> (24 - 8 * j));
            hex[i*8 + j*2]     = hx[b >> 4];
            hex[i*8 + j*2 + 1] = hx[b & 15];
        }
    hex[64] = 0;
}

/* ---------------------------------------------------------------------- */
/* INTROSPECCIÓN DEL ENTORNO: qué permisos tiene REALMENTE este proceso     */
/* ---------------------------------------------------------------------- */
/* MEDIDO: el shell de Android NO tiene `su` fiable, ni `getcap`, ni
 * `capsh`, ni `ping`, ni `curl`.  Todo esto se lee de /proc y /sys, que
 * siempre existen.  Es la respuesta a "¿qué puede hacer este binario aquí". */

static const char *cap_nombre(int bit) {
    /* Tabla como blob CONTIGUO y no como array de punteros: cada puntero
     * costaba una relocalizacion de 24 B en .rela.dyn (el binario tiene que
     * ser pequeno: va al movil con un `cat`). */
    static const char n[][18] = {
        "CHOWN", "DAC_OVERRIDE", "DAC_READ_SEARCH", "FOWNER", "FSETID", "KILL",
        "SETGID", "SETUID", "SETPCAP", "LINUX_IMMUTABLE", "NET_BIND_SERVICE",
        "NET_BROADCAST", "NET_ADMIN", "NET_RAW", "IPC_LOCK", "IPC_OWNER",
        "SYS_MODULE", "SYS_RAWIO", "SYS_CHROOT", "SYS_PTRACE", "SYS_PACCT",
        "SYS_ADMIN", "SYS_BOOT", "SYS_NICE", "SYS_RESOURCE", "SYS_TIME",
        "SYS_TTY_CONFIG", "MKNOD", "LEASE", "AUDIT_WRITE", "AUDIT_CONTROL",
        "SETFCAP"
    };
    if (bit < 0 || bit >= (int)(sizeof(n) / sizeof(n[0]))) return NULL;
    return n[bit];
}

/* Decodifica una mascara de capacidades (/proc/self/status). */
MD_HOT static void capacidades(uint64_t mask, char *out, size_t on) {
    out[0] = 0;
    size_t used = 0;
    int ninguna = 1;
    for (int b = 0; b < 64; b++) {
        if (!(mask & (1ULL << b))) continue;
        const char *nm = cap_nombre(b);
        if (!nm) continue;
        ninguna = 0;
        size_t l = strlen(nm);
        if (used + l + 2 >= on) break;
        if (used) out[used++] = ' ';
        memcpy(out + used, nm, l);
        used += l;
        out[used] = 0;
    }
    if (ninguna) snprintf(out, on, "ninguna");
}

/* Valor hexadecimal de una clave de /proc/self/status. */
static int status_hex(const char *clave, uint64_t *val) {
    FILE *f = fopen("/proc/self/status", "r");
    if (!f) return -1;
    char linea[512];
    size_t kl = strlen(clave);
    int r = -1;
    while (fgets(linea, sizeof(linea), f)) {
        if (strncmp(linea, clave, kl) == 0) {
            const char *p = linea + kl;
            while (*p && *p != ':') p++;
            if (*p == ':') {
                p++;
                while (*p == ' ' || *p == '\t') p++;
                *val = strtoull(p, NULL, 16);
                r = 0;
            }
            break;
        }
    }
    fclose(f);
    return r;
}

/* Texto de una clave de /proc/self/status ("Seccomp", "NoNewPrivs"...). */
static void status_txt(const char *clave, char *out, size_t on) {
    snprintf(out, on, "?");
    FILE *f = fopen("/proc/self/status", "r");
    if (!f) return;
    char linea[512];
    size_t kl = strlen(clave);
    while (fgets(linea, sizeof(linea), f)) {
        if (strncmp(linea, clave, kl) == 0) {
            char *p = linea + kl;
            while (*p && *p != ':') p++;
            if (*p == ':') {
                p++;
                while (*p == ' ' || *p == '\t') p++;
                trim(p);
                size_t max = on > 1 ? on - 1 : 0;
                snprintf(out, on, "%.*s", (int)max, p);
            }
            break;
        }
    }
    fclose(f);
}

/* Montajes: cuantas son solo-lectura y donde estan /system, /data y /sdcard. */
static void montajes(char *out, size_t on) {
    out[0] = 0;
    FILE *f = fopen("/proc/mounts", "r");
    if (!f) { snprintf(out, on, "no se pudo leer /proc/mounts"); return; }
    char linea[512], b[2048];
    int ro = 0, total = 0;
    b[0] = 0;
    size_t used = 0;
    while (fgets(linea, sizeof(linea), f)) {
        char *mp = strchr(linea, ' ');
        if (!mp) continue;
        *mp++ = 0;
        char *fs = strchr(mp, ' ');
        if (!fs) continue;
        *fs++ = 0;
        char *opts = strchr(fs, ' ');
        if (!opts) continue;
        opts++;
        total++;
        if (strstr(opts, "ro,") || strstr(opts, ",ro")) ro++;
        if (!strcmp(mp, "/system") || !strcmp(mp, "/data") || !strcmp(mp, "/sdcard")) {
            int r = strstr(opts, "ro") ? 1 : 0;
            size_t l = strlen(mp) + 4;
            if (used + l + 2 < sizeof(b)) {
                int k = snprintf(b + used, sizeof(b) - used, "%s%s ", r ? "ro:" : "rw:", mp);
                used += (size_t)k;
            }
        }
    }
    fclose(f);
    if (b[0]) b[used ? used - 1 : 0] = 0;
    snprintf(out, on, "%d montajes, %d solo-lectura%s%s", total, ro,
             b[0] ? ", " : "", b);
}

/* Limites del proceso (ficheros abiertos, pila, memoria). */
static void limites(char *out, size_t on) {
    out[0] = 0;
    FILE *f = fopen("/proc/self/limits", "r");
    if (!f) { snprintf(out, on, "no se pudo leer /proc/self/limits"); return; }
    char linea[512], b[1024];
    b[0] = 0;
    size_t used = 0;
    while (fgets(linea, sizeof(linea), f)) {
        if (!strncmp(linea, "Max open files", 14) ||
            !strncmp(linea, "Max stack size", 14) ||
            !strncmp(linea, "Max locked memory", 17)) {
            char *p = linea;
            while (*p == ' ' || *p == '\t') p++;
            char *nl = strchr(p, '\n');
            if (nl) *nl = 0;
            size_t l = strlen(p);
            if (used + l + 2 < sizeof(b)) {
                memcpy(b + used, p, l);
                used += l;
                b[used++] = ';';
                b[used++] = ' ';
                b[used] = 0;
            }
        }
    }
    fclose(f);
    snprintf(out, on, "%s", b);
}

/* ---------------------------------------------------------------------- */
/* RED SIN curl: DNS y TCP con sockets (el shell de Android no tiene curl) */
/* ---------------------------------------------------------------------- */

/* 1 = conecta, 0 = resuelve pero no conecta, -1 = no resuelve, -2 = error */
static int tcp_puerto(const char *host, int port, int ms) {
    struct addrinfo hints, *res = NULL;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    char portstr[16];
    snprintf(portstr, sizeof(portstr), "%d", port);
    if (getaddrinfo(host, portstr, &hints, &res) != 0 || !res) return -1;
    int r = 0;
    for (struct addrinfo *ai = res; ai; ai = ai->ai_next) {
        int fd = socket(ai->ai_family, ai->ai_socktype, 0);
        if (fd < 0) continue;
        int fl = fcntl(fd, F_GETFL, 0);
        fcntl(fd, F_SETFL, fl | O_NONBLOCK);
        int c = connect(fd, ai->ai_addr, ai->ai_addrlen);
        if (c == 0) { r = 1; close(fd); break; }
        if (errno == EINPROGRESS) {
            fd_set w;
            FD_ZERO(&w);
            FD_SET(fd, &w);
            struct timeval tv = {ms / 1000, (ms % 1000) * 1000};
            if (select(fd + 1, NULL, &w, NULL, &tv) > 0) {
                int e = 0;
                socklen_t el = sizeof(e);
                getsockopt(fd, SOL_SOCKET, SO_ERROR, &e, &el);
                if (e == 0) { r = 1; close(fd); break; }
            }
        }
        close(fd);
    }
    freeaddrinfo(res);
    return r;
}

/* El binario en servicio: es un ELF valido y de que arquitectura. */
static void elf_info(const char *path, char *out, size_t on) {
    out[0] = 0;
    int fd = open(path, O_RDONLY);
    if (fd < 0) { snprintf(out, on, "no se puede leer"); return; }
    unsigned char h[20];
    ssize_t r = read(fd, h, sizeof(h));
    close(fd);
    if (r < 20 || h[0] != 0x7f || h[1] != 'E' || h[2] != 'L' || h[3] != 'F') {
        snprintf(out, on, "NO es un ELF (fichero roto o no es un binario)");
        return;
    }
    const char *clase = (h[4] == 2) ? "64-bit" : (h[4] == 1) ? "32-bit" : "??";
    unsigned ma = (unsigned)h[18] | ((unsigned)h[19] << 8);
    const char *arch = ma == 0x3e ? "x86-64" : ma == 0xb7 ? "aarch64" :
                       ma == 0x03 ? "x86" : ma == 0x28 ? "arm" : "desconocida";
    snprintf(out, on, "ELF %s %s", clase, arch);
}

/* ---------------------------------------------------------------------- */
/* pkill en C: termina procesos por nombre (no hay pgrep/pkill en Android)  */
/* ---------------------------------------------------------------------- */

/* Termina los procesos cuyo cmdline contiene `patron`.  SIGTERM primero y
 * SIGKILL si hace falta.  Devuelve cuantos ha terminado. */
static int matar(const char *patron, int espera_ms) {
    DIR *d = opendir("/proc");
    if (!d) return -1;
    struct dirent *e;
    int self = (int)getpid();
    int n = 0;
    int pids[64];
    int np = 0;
    while ((e = readdir(d)) && np < 64) {
        if (e->d_name[0] < '0' || e->d_name[0] > '9') continue;
        int pid = atoi(e->d_name);
        if (pid == self) continue;
        char p[256], cmd[600];
        snprintf(p, sizeof(p), "/proc/%.200s/cmdline", e->d_name);
        if (read_file(p, cmd, sizeof(cmd)) != 0) continue;
        if (!strstr(cmd, patron)) continue;
        pids[np++] = pid;
    }
    closedir(d);
    for (int i = 0; i < np; i++) kill(pids[i], SIGTERM);
    if (np && espera_ms > 0) {
        struct timespec ts = {espera_ms / 1000, (espera_ms % 1000) * 1000000L};
        nanosleep(&ts, NULL);
    }
    for (int i = 0; i < np; i++) {
        if (kill(pids[i], 0) == 0) kill(pids[i], SIGKILL);
        else n++;
    }
    return n;
}

/* ---------------------------------------------------------------------- */
/* SALUD DEL MOVIL: estado-movil.sh + verificar-movil.sh en C              */
/* ---------------------------------------------------------------------- */
/* POR QUE EN C Y NO EN SHELL
 *   MEDIDO: el shell de Android NO tiene curl ni wget.  Los scripts de
 *   estado tenian que traer `toybox wget` o resignarse a no mirar /health.
 *   Aqui el chequeo HTTP va con sockets POSIX: sin curl, sin wget, sin
 *   binario externo, y con 30 KB de binario.
 *   MEDIDO: el tunnel.log puede traer `api.trycloudflare.com`, que NO es un
 *   tunel (es la API de Cloudflare).  Aqui se filtra en codigo.
 */

/* GET HTTP en crudo a un host IPv4.  Devuelve el codigo HTTP (0 si fallo) y
 * deja el cuerpo en out.  Sin TLS: solo para el origen local (127.0.0.1). */
MD_HOT static int http_get(const char *host, int port, const char *path,
                    char *out, size_t n) {
    if (!n) return 0;
    out[0] = 0;
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) return 0;
    struct timeval tv = {8, 0};
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv));
    setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &tv, sizeof(tv));
    struct sockaddr_in a;
    memset(&a, 0, sizeof(a));
    a.sin_family = AF_INET;
    a.sin_port = htons((uint16_t)port);
    if (inet_pton(AF_INET, host, &a.sin_addr) != 1) { close(fd); return 0; }
    if (connect(fd, (struct sockaddr *)&a, sizeof(a)) != 0) { close(fd); return 0; }
    char req[512];
    int rl = snprintf(req, sizeof(req),
                      "GET %s HTTP/1.0\r\nHost: %s\r\nUser-Agent: mddoctor\r\n"
                      "Connection: close\r\n\r\n", path, host);
    if (rl <= 0 || write(fd, req, (size_t)rl) != rl) { close(fd); return 0; }
    size_t got = 0;
    ssize_t r;
    while (got + 1 < n && (r = read(fd, out + got, n - 1 - got)) > 0) got += (size_t)r;
    close(fd);
    out[got] = 0;
    if (strncmp(out, "HTTP/", 5) != 0) return 0;
    const char *sp = strchr(out, ' ');
    if (!sp) return 0;
    return atoi(sp + 1);
}

/* Extrae "campo":"valor" de un JSON sin parsearlo entero.  Heuristico a
 * proposito: el cuerpo de /health lo produce el daemon y aqui solo se
 * MUESTRA, no se decide nada con el. */
static void json_campo(const char *json, const char *campo, char *out, size_t on) {
    snprintf(out, on, "?");
    char patron[64];
    snprintf(patron, sizeof(patron), "\"%s\"", campo);
    const char *p = strstr(json, patron);
    if (!p) return;
    p += strlen(patron);
    while (*p && *p != ':') p++;
    if (!*p) return;
    p++;
    while (*p == ' ' || *p == ':') p++;
    if (*p != '"') {
        size_t k = 0;
        while (*p && *p != ',' && *p != '}' && *p != ' ' && k < on - 1) out[k++] = *p++;
        out[k] = 0;
        return;
    }
    p++;
    size_t k = 0;
    while (*p && *p != '"' && k < on - 1) out[k++] = *p++;
    out[k] = 0;
}

/* La URL REAL del tunel: la ultima de tunnel.log que NO sea api.*, y si no
 * hay, el fichero tunnel.url.  Es lo que pedia estado-movil.sh. */
static void leer_tunel(char *out, size_t on) {
    out[0] = 0;
    char p1[256], p2[256];
    snprintf(p1, sizeof(p1), "%s", prefijo());
    snprintf(p2, sizeof(p2), "/data/adb/megadaemon");
    const char *prefs[2];
    prefs[0] = p1;
    prefs[1] = p2;
    char mejor[512] = "";
    for (unsigned i = 0; i < sizeof(prefs) / sizeof(prefs[0]); i++) {
        char log[512];
        snprintf(log, sizeof(log), "%s/tunnel.log", prefs[i]);
        FILE *f = fopen(log, "r");
        if (!f) continue;
        char linea[1024];
        while (fgets(linea, sizeof(linea), f)) {
            char *p = linea;
            while ((p = strstr(p, "https://")) != NULL) {
                p += 8;
                char h[256];
                size_t k = 0;
                while (*p && *p != '/' && *p != '"' && *p != ' ' && *p != '\n' &&
                       k < sizeof(h) - 1)
                    h[k++] = *p++;
                h[k] = 0;
                /* api.trycloudflare.com es la API de Cloudflare, NO el tunel */
                if (strstr(h, ".trycloudflare.com") && strncmp(h, "api.", 4) != 0)
                    snprintf(mejor, sizeof(mejor), "https://%s", h);
            }
        }
        fclose(f);
    }
    if (mejor[0]) { snprintf(out, on, "%.400s", mejor); return; }
    for (unsigned i = 0; i < sizeof(prefs) / sizeof(prefs[0]); i++) {
        char u[512], b[512];
        snprintf(u, sizeof(u), "%s/tunnel.url", prefs[i]);
        if (read_file(u, b, sizeof(b)) == 0) {
            trim(b);
            if (b[0]) { snprintf(out, on, "%.400s", b); return; }
        }
    }
}

/* Procesos de MegaDaemon leyendo /proc (sin pgrep: pgrep no existe en
 * todos los Android y hay que depender de busybox si lo hay). */
static void procesos_megadaemon(char *out, size_t on) {
    out[0] = 0;
    DIR *d = opendir("/proc");
    if (!d) { snprintf(out, on, "no se pudo leer /proc"); return; }
    struct dirent *e;
    size_t used = 0;
    int n = 0;
    while ((e = readdir(d))) {
        if (e->d_name[0] < '0' || e->d_name[0] > '9') continue;
        char p[256], cmd[600];
        snprintf(p, sizeof(p), "/proc/%.200s/cmdline", e->d_name);
        if (read_file(p, cmd, sizeof(cmd)) != 0) continue;
        if (!strstr(cmd, "megadaemon") && !strstr(cmd, "cloudflared")) continue;
        char item[640];
        snprintf(item, sizeof(item), "%.100s %.400s", e->d_name, cmd);
        size_t l = strlen(item);
        if (used + l + 2 >= on) break;
        memcpy(out + used, item, l);
        used += l;
        out[used++] = '\n';
        out[used] = 0;
        if (++n >= 8) break;
    }
    closedir(d);
    if (!n) snprintf(out, on, "ninguno");
}

/* Ultimas n lineas de un fichero.  out queda con lo que haya. */
static void tail_file(const char *path, int n, char *out, size_t on) {
    /* SIN malloc: un buffer estatico y una ventana de lineas.  En un movil
     * con la RAM justa, la pila no se toca y el binario no depende del
     * heap (ademas el binario es mas pequeno: sin malloc/free/strdup). */
    static char ventana[16][256];
    out[0] = 0;
    if (n < 1) return;
    if (n > 16) n = 16;
    FILE *f = fopen(path, "r");
    if (!f) return;
    char linea[256];
    int i = 0, c = 0;
    while (fgets(linea, sizeof(linea), f)) {
        size_t l = strlen(linea);
        if (l >= sizeof(ventana[0])) l = sizeof(ventana[0]) - 1;
        memcpy(ventana[i], linea, l);
        ventana[i][l] = 0;
        i = (i + 1) % n;
        c++;
    }
    fclose(f);
    int total = c >= n ? n : c;
    int ini = c >= n ? i : 0;
    size_t used = 0;
    for (int k = 0; k < total; k++) {
        const char *ln = ventana[(ini + k) % n];
        size_t len = strlen(ln);
        if (used + len + 2 >= on) break;
        memcpy(out + used, ln, len);
        used += len;
        if (len && out[used - 1] != '\n') out[used++] = '\n';
    }
    out[used] = 0;
}

/* Recoge la salud del movil.  `espera` son segundos de espera por /health
 * (el bucle de espera de verificar-movil.sh, contra el ORIGEN local: la
 * parte que funciona sin TLS). */
/* Extrae el host de una URL ("https://host/path" -> "host"). */
static void url_host(const char *url, char *out, size_t on) {
    out[0] = 0;
    const char *p = strstr(url, "://");
    p = p ? p + 3 : url;
    size_t k = 0;
    while (*p && *p != '/' && *p != ':' && *p != '?' && k < on - 1) out[k++] = *p++;
    out[k] = 0;
}

/* Recoge la salud del movil.  `espera` son segundos de espera por /health
 * (el bucle de espera de verificar-movil.sh, contra el ORIGEN local: la
 * parte que funciona sin TLS). */
MD_COLD static void salud(int espera) {
    char v[220];

    char url[512] = "";
    leer_tunel(url, sizeof(url));
    if (url[0]) add("tunel", url, VER_OK, "la URL publica del movil");
    else add("tunel", "NO ENCONTRADO", VER_AVISO,
             "ni tunnel.log ni tunnel.url: el tunel no esta levantado");

    /* ¿El tunel esta vivo?  DNS + TCP, sin curl (que no existe en Android).
     * Esto dice SI el corte esta en el DNS, en el tunel o mas abajo. */
    if (url[0]) {
        char host[256];
        url_host(url, host, sizeof(host));
        int r = tcp_puerto(host, 443, 3000);
        if (r == 1)
            add("tunel-tcp", host, VER_OK, "el tunel acepta conexiones en el 443");
        else if (r == 0)
            add("tunel-tcp", host, VER_CRITICO,
                "el host resuelve pero el 443 NO acepta: el tunel esta caido");
        else
            add("tunel-tcp", host, VER_CRITICO,
                "el host del tunel NO resuelve: el corte esta en el DNS");
    }

    char body[4096];
    int code = 0;
    time_t limite = time(NULL) + (espera > 0 ? espera : 0);
    do {
        code = http_get("127.0.0.1", 9090, "/health", body, sizeof(body));
        if (code == 200 || espera <= 0 || time(NULL) >= limite) break;
        sleep(3);
    } while (1);

    if (code == 200) {
        char build[64];
        json_campo(body, "short", build, sizeof(build));
        snprintf(v, sizeof(v), "HTTP 200 (build %s)", build);
        add("daemon-health", v, VER_OK, "el daemon responde en 127.0.0.1:9090");
    } else if (code > 0) {
        snprintf(v, sizeof(v), "HTTP %d", code);
        add("daemon-health", v, VER_CRITICO, "el daemon contesta pero con error");
    } else {
        add("daemon-health", "sin respuesta", VER_CRITICO,
            "el daemon NO responde en 127.0.0.1:9090: muerto o el puerto esta ocupado");
    }

    char procs[1024];
    procesos_megadaemon(procs, sizeof(procs));
    snprintf(v, sizeof(v), "%.200s", procs);
    add("procesos-mv", v, strstr(procs, "megadaemon-core") ? VER_OK : VER_AVISO,
        strstr(procs, "megadaemon-core") ? "el daemon esta en ejecucion"
                                         : "no hay ningun megadaemon-core vivo");

    char svc_sup[512], svc_kep[512];
    snprintf(svc_sup, sizeof(svc_sup), "%s/99-megadaemon.sh", serviced());
    snprintf(svc_kep, sizeof(svc_kep), "%s/97-megadaemon-keeper.sh", serviced());
    int hay_sup = existe(svc_sup);
    int hay_kep = existe(svc_kep);
    snprintf(v, sizeof(v), "supervisor=%s keeper=%s",
             hay_sup ? "si" : "no", hay_kep ? "si" : "no");
    add("autoarranque", v, hay_sup ? VER_OK : VER_CRITICO,
        hay_sup ? "el supervisor esta instalado: el daemon arranca solo"
                : "NO hay supervisor: el daemon no se levanta tras un reinicio");

    char bin1[512], bin2[512];
    snprintf(bin1, sizeof(bin1), "%s/megadaemon-core", prefijo());
    snprintf(bin2, sizeof(bin2), "/data/adb/megadaemon/megadaemon-core");
    const char *bins[2];
    bins[0] = bin1;
    bins[1] = bin2;
    const char *binpath = NULL;
    for (unsigned i = 0; i < sizeof(bins) / sizeof(bins[0]); i++)
        if (existe(bins[i])) { binpath = bins[i]; break; }
    if (binpath) {
        char hex[65];
        sha256_hex(binpath, hex);
        snprintf(v, sizeof(v), "%.64s", hex);
        add("sha-servicio", v, VER_OK, "sha256 del binario en servicio");
        char elf[64];
        elf_info(binpath, elf, sizeof(elf));
        add("elf-servicio", elf, strncmp(elf, "ELF", 3) == 0 ? VER_OK : VER_CRITICO,
            "cabecera ELF del binario en servicio (arquitectura y validez)");
    } else {
        add("sha-servicio", "sin binario", VER_AVISO,
            "no se encuentra megadaemon-core en el movil");
    }

    char ult[1024];
    char autolog[512];
    snprintf(autolog, sizeof(autolog), "%s/state/auto_update.log", prefijo());
    tail_file(autolog, 1, ult, sizeof(ult));
    trim(ult);
    if (ult[0]) {
        snprintf(v, sizeof(v), "%.200s", ult);
        add("auto-update", v, VER_OK, "ultima linea del log del auto-actualizador");
    }
}

/* Texto extra del modo salud (solo modo texto): las colas de log. */
MD_COLD static void salud_tails(void) {
    char b[2048];
    char autolog[512], daemonlog[512];
    snprintf(autolog, sizeof(autolog), "%s/state/auto_update.log", prefijo());
    snprintf(daemonlog, sizeof(daemonlog), "/sdcard/megadaemon.log");
    printf("\n--- ultimo estado del auto-actualizador ---\n");
    tail_file(autolog, 6, b, sizeof(b));
    printf("%s", b);
    printf("--- ultimas lineas del log del daemon ---\n");
    tail_file(daemonlog, 12, b, sizeof(b));
    printf("%s", b);
    printf("--- cuerpo de /health (primeros 600 bytes) ---\n");
    char body[4096];
    if (http_get("127.0.0.1", 9090, "/health", body, sizeof(body)))
        printf("%.600s\n", body);
}

/* ---------------------------------------------------------------------- */
/* Modo `entorno`: la introspección del entorno en detalle.                  */
/* Es la respuesta a "¿qué puede hacer este binario AQUÍ?".                */
/* ---------------------------------------------------------------------- */
MD_COLD static void entorno(int json) {
    char caps[1024];
    uint64_t cap = 0;
    int hay_cap = status_hex("CapEff", &cap) == 0;
    if (hay_cap) capacidades(cap, caps, sizeof(caps));

    char seccomp[64], nnp[64], umask[64];
    status_txt("Seccomp", seccomp, sizeof(seccomp));
    status_txt("NoNewPrivs", nnp, sizeof(nnp));
    status_txt("Umask", umask, sizeof(umask));

    char mont[1024], lim[1024];
    montajes(mont, sizeof(mont));
    limites(lim, sizeof(lim));

    char enf[64];
    int hay_selinux = read_file("/sys/fs/selinux/enforce", enf, sizeof(enf)) == 0;
    if (hay_selinux) trim(enf);

    static const char linkers[][24] = {"/system/bin/linker64", "/system/bin/linker",
                                       "/system/xbin/linker64", {0}};
    int hay_linker = 0;
    for (int i = 0; linkers[i][0]; i++)
        if (existe(linkers[i])) hay_linker = 1;

    static const char herr[][10] = {"su", "magisk", "busybox", "curl", "wget",
                                    "pgrep", "pkill", "adb", {0}};
    char lista[256];
    lista[0] = 0;
    size_t used = 0;
    for (int i = 0; herr[i][0]; i++) {
        char ruta[256];
        int la_hay = 0;
        static const char dirs[][20] = {"/system/bin", "/system/xbin",
                                       "/data/adb/magisk", {0}};
        for (int d = 0; dirs[d][0] && !la_hay; d++) {
            snprintf(ruta, sizeof(ruta), "%s/%s", dirs[d], herr[i]);
            la_hay = existe(ruta);
        }
        if (!la_hay) continue;
        size_t l = strlen(herr[i]);
        if (used + l + 2 >= sizeof(lista)) break;
        if (used) lista[used++] = ' ';
        memcpy(lista + used, herr[i], l);
        used += l;
        lista[used] = 0;
    }
    if (!lista[0]) snprintf(lista, sizeof(lista), "ninguna de las habituales");

    /* Los mismos datos, como hallazgos: así `entorno --json` se integra. */
    char v[220];
    snprintf(v, sizeof(v), "euid=%d uid=%d egid=%d", (int)geteuid(),
             (int)getuid(), (int)getegid());
    add("identidad", v, VER_OK, "con quien se ejecuta este binario");
    if (hay_cap) {
        int es_root = (cap & (1ULL << 0)) && (cap & (1ULL << 3));
        add("capacidades", caps, es_root ? VER_OK : VER_AVISO,
            es_root ? "capacidades casi de root" : "capacidades limitadas");
    }
    snprintf(v, sizeof(v), "seccomp=%s no_new_privs=%s umask=%s", seccomp, nnp, umask);
    add("aislamiento", v, VER_OK, "lo que el kernel permite a este proceso");
    add("montajes", mont, VER_OK, "particiones y si son escribibles");
    add("limites", lim, VER_OK, "limites del proceso");
    add("selinux", hay_selinux ? enf : "no hay SELinux", VER_OK,
        "enforcement de SELinux");
    add("linker-android", hay_linker ? "presente" : "AUSENTE",
        hay_linker ? VER_OK : VER_CRITICO,
        hay_linker ? "los binarios ARM64 arrancan"
                   : "sin linker64 los binarios ARM64 no arrancan");
    add("herramientas", lista, VER_OK,
        "herramientas del sistema (este binario no las necesita)");

    if (json) return;

    printf("== ENTORNO: permisos reales de este proceso ==\n");
    printf("  euid=%d uid=%d egid=%d\n", (int)geteuid(), (int)getuid(),
           (int)getegid());
    if (hay_cap) printf("  capacidades efectivas: %s\n", caps);
    printf("  seccomp: %s\n", seccomp);
    printf("  no_new_privs: %s\n", nnp);
    printf("  umask: %s\n", umask);
    printf("  montajes: %s\n", mont);
    printf("  limites: %s\n", lim);
    if (hay_selinux)
        printf("  selinux: %s (%s)\n", enf, enf[0] == '0' ? "permisivo" : "enforcing");
    else
        printf("  selinux: no hay /sys/fs/selinux/enforce (no hay SELinux)\n");
    printf("  linker de Android: %s\n",
           hay_linker ? "presente" : "AUSENTE (los binarios ARM64 no arrancaran)");
    printf("  herramientas del sistema: %s\n", lista);
}

/* ---------------------------------------------------------------------- */
/* Modo `repair`: la recuperación que se puede hacer SIN el daemon.         */
/* Con --apply ejecuta; sin él, sólo dice qué haría.                      */
/* ---------------------------------------------------------------------- */

/* Las rutas llegan del ENTORNO (MEGADAEMON_STATE_DIR), y este modo las
 * mete en `system()`.  Una ruta con comillas o `;` sería una inyección de
 * comandos, así que se rechaza: el binario no ejecuta lo que le metan. */
static int ruta_segura(const char *p) {
    if (!p || !*p) return 0;
    for (const char *q = p; *q; q++)
        if (*q == '\'' || *q == '"' || *q == ';' || *q == '|' || *q == '&' ||
            *q == '$' || *q == '`' || *q == '\n')
            return 0;
    return 1;
}

MD_COLD static int reparar(int aplicar) {
    char sd[512], bin[512];
    const char *est = getenv("MEGADAEMON_STATE_DIR");
    snprintf(sd, sizeof(sd), "%s", (est && *est) ? est : "/data/user/0/com.agy/files/state");
    snprintf(bin, sizeof(bin), "%s/megadaemon-core", prefijo());

    int problemas = 0;

    if (!ruta_segura(sd)) {
        printf("  [FALLO] la ruta del estado contiene caracteres peligrosos: no se toca\n");
        return 1;
    }

    /* 1) el directorio de estado */
    if (!existe(sd)) {
        if (aplicar) {
            /* mkdir/chmod DIRECTOS: sin shell (el movil no siempre tiene uno
             * util) y sin inyectar la ruta en una linea de comandos. */
            mkdir(sd, 0700);
            chmod(sd, 0700);
            if (existe(sd) && escribible(sd))
                printf("  [hecho] creado el directorio de estado %s\n", sd);
            else {
                printf("  [FALLO] no se pudo crear %s\n", sd);
                problemas++;
            }
        } else {
            printf("  [plan]   crear el directorio de estado %s (mkdir -p + chmod 700)\n", sd);
            problemas++;
        }
    } else if (!escribible(sd)) {
        if (aplicar) {
            chmod(sd, 0700);
            if (escribible(sd))
                printf("  [hecho] arreglado el permiso de escritura de %s\n", sd);
            else {
                printf("  [FALLO] %s sigue sin escritura\n", sd);
                problemas++;
            }
        } else {
            printf("  [plan]   %s no es escribible (chmod 700)\n", sd);
            problemas++;
        }
    } else {
        printf("  [ok]     el directorio de estado esta bien (%s)\n", sd);
    }

    /* 2) el binario en servicio */
    if (existe(bin)) {
        if (access(bin, X_OK) != 0) {
            if (aplicar) {
                chmod(bin, 0755);
                if (access(bin, X_OK) == 0)
                    printf("  [hecho] permiso de ejecucion puesto en %s\n", bin);
                else {
                    printf("  [FALLO] no se pudo chmod +x %s\n", bin);
                    problemas++;
                }
            } else {
                printf("  [plan]   %s no es ejecutable (chmod 755)\n", bin);
                problemas++;
            }
        } else {
            char elf[64];
            elf_info(bin, elf, sizeof(elf));
            printf("  [ok]     el binario en servicio esta bien (%s)\n", elf);
        }
    } else {
        printf("  [FALLO] no hay binario en %s\n", bin);
        problemas++;
    }

    /* 3) procesos zombis del daemon (pkill en C: no hay pkill en Android) */
    char procs[1024];
    procesos_megadaemon(procs, sizeof(procs));
    int hay = strcmp(procs, "ninguno") != 0;
    if (hay && aplicar) {
        int n = matar("megadaemon-core", 500);
        printf("  [hecho] %d proceso(s) de megadaemon-core terminado(s)\n", n);
    } else if (hay) {
        printf("  [plan]   terminar los procesos de megadaemon-core (%s)\n", procs);
    }

    /* 4) relanzar el supervisor (si esta instalado) */
    char sup[512];
    snprintf(sup, sizeof(sup), "%s/99-megadaemon.sh", serviced());
    if (existe(sup)) {
        if (aplicar) {
            char cmd[600];
            snprintf(cmd, sizeof(cmd), "'%.240s' >/dev/null 2>&1 &", sup);
            int rc = system(cmd);
            printf("  [%s] supervisor relanzado (%s)\n", rc == 0 ? "hecho" : "FALLO", sup);
            if (rc != 0) problemas++;
        } else {
            printf("  [plan]   relanzar el supervisor %s\n", sup);
        }
    } else {
        printf("  [aviso]  no hay supervisor en %s (el daemon no arrancara solo)\n", sup);
        problemas++;
    }

    if (!aplicar && problemas)
        printf("\n(sin --apply esto es SOLO un plan: nada se ha tocado)\n");
    return problemas ? 1 : 0;
}

/* ---------------------------------------------------------------------- */

/* ---------------------------------------------------------------------- */
/* Modo `reconectar`: devolver el control cuando el movil se ha ido.       */
/* Es la accion mas valiosa cuando el daemon esta muerto: relanza lo que   */
/* falte (tunel, daemon), espera a que /health vuelva y DICE la URL nueva, */
/* que es lo que el root necesita para reconectar.                        */
/* ---------------------------------------------------------------------- */
static int reconectar(int aplicar, int espera) {
    char procs[2048];
    procesos_megadaemon(procs, sizeof(procs));
    int hay_daemon = strstr(procs, "megadaemon-core") != NULL;
    int hay_tunel = strstr(procs, "cloudflared") != NULL;

    printf("== RECONEXION ==\n");
    printf("  daemon: %s | tunel: %s\n",
           hay_daemon ? "vivo" : "MUERTO",
           hay_tunel ? "vivo" : "caido");

    char cloud[512], sup[512];
    snprintf(cloud, sizeof(cloud), "%s/98-cloudflared.sh", serviced());
    snprintf(sup, sizeof(sup), "%s/99-megadaemon.sh", serviced());
    if (!ruta_segura(sup) || !ruta_segura(cloud)) {
        printf("  [FALLO] rutas de servicio peligrosas: no se ejecuta nada\n");
        return 1;
    }

    if (!aplicar) {
        if (!hay_tunel && existe(cloud))
            printf("  [plan]   relanzar el tunel: %s\n", cloud);
        if (!hay_daemon && existe(sup))
            printf("  [plan]   relanzar el daemon: %s\n", sup);
        if (hay_tunel && hay_daemon)
            printf("  [plan]   no hace falta nada: daemon y tunel estan vivos\n");
        printf("\n(sin --apply esto es SOLO un plan: nada se ha tocado)\n");
        return 0;
    }

    /* 1) el tunel */
    if (!hay_tunel) {
        if (existe(cloud)) {
            char cmd[600];
            snprintf(cmd, sizeof(cmd), "sh '%.240s' >/dev/null 2>&1 &", cloud);
            int rc = system(cmd);
            printf("  [%s] tunel relanzado (%s)\n", rc == 0 ? "hecho" : "FALLO", cloud);
        } else {
            printf("  [FALLO] no hay %s: el tunel no se puede relanzar\n", cloud);
        }
        /* El tunel tarda en levantar: se le da un respiro antes de medir. */
        struct timespec ts = {5, 0};
        nanosleep(&ts, NULL);
    }

    /* 2) el daemon */
    if (!hay_daemon) {
        if (existe(sup)) {
            char cmd[600];
            snprintf(cmd, sizeof(cmd), "sh '%.240s' >/dev/null 2>&1 &", sup);
            int rc = system(cmd);
            printf("  [%s] daemon relanzado (%s)\n", rc == 0 ? "hecho" : "FALLO", sup);
        } else {
            printf("  [FALLO] no hay %s: el daemon no se puede relanzar\n", sup);
        }
    }

    /* 3) esperar a que /health vuelva */
    char body[4096];
    int code = 0;
    int intentos = espera > 0 ? espera / 3 + 1 : 1;
    for (int i = 0; i < intentos; i++) {
        code = http_get("127.0.0.1", 9090, "/health", body, sizeof(body));
        if (code == 200) break;
        struct timespec ts = {3, 0};
        nanosleep(&ts, NULL);
    }
    if (code == 200) {
        char build[64];
        json_campo(body, "short", build, sizeof(build));
        printf("  [hecho] el daemon responde (build %s)\n", build);
    } else {
        printf("  [aviso] /health no responde todavia (codigo %d)\n", code);
    }

    /* 4) la URL NUEVA: el tunel cambia de hostname en cada reconexion */
    char url[512] = "";
    leer_tunel(url, sizeof(url));
    if (url[0]) {
        printf("  tunel ahora: %s\n", url);
        char host[256];
        url_host(url, host, sizeof(host));
        int r = tcp_puerto(host, 443, 3000);
        if (r == 1) printf("  [ok] el 443 del tunel acepta conexiones\n");
        else if (r == 0) printf("  [aviso] el host resuelve pero el 443 no acepta\n");
        else printf("  [aviso] el host del tunel no resuelve\n");
    } else {
        printf("  [aviso] no hay URL de tunel todavia\n");
    }
    return code == 200 ? 0 : 1;
}

MD_COLD static void uso(void) {
    fprintf(stderr,
            "mddoctor — diagnostico, salud y recuperacion de MegaDaemon (v" MDDOCTOR_VERSION ")\n"
            "uso: mddoctor [doctor|salud|entorno|repair|reconectar|scan|recover|selftest|todo]\n"
            "                      [--json] [--apply] [--espera N]\n"
            "  doctor    recoge los hechos y dice que esta mal\n"
            "  salud     estado del movil: tunel real, DNS y TCP del tunel, /health\n"
            "            por sockets (sin curl ni wget), procesos, SHA-256 y ELF del\n"
            "            binario en servicio (--espera N reintenta /health)\n"
            "  entorno   los permisos REALES de este proceso: capacidades, seccomp,\n"
            "            montajes, limites, SELinux, linker de Android\n"
            "  repair    repara lo que se puede SIN el daemon (--apply lo ejecuta)\n"
            "  reconectar relanza tunel y/o daemon y dice la URL nueva (--apply lo hace)\n"
            "  scan      recoge los hechos y los lista\n"
            "  recover   plan de recuperacion (--apply lo ejecuta)\n"
            "  todo      doctor + salud en un solo informe (para integrar)\n"
            "  selftest  prueba las sondas que usa este binario\n"
            "  --json    salida en JSON (schema " MDDOCTOR_SCHEMA ")\n"
            "  --version version y contrato\n"
            "\n"
            "codigos de salida: 0 = todo OK, 1 = hay avisos, 2 = hay criticos, 3 = uso incorrecto\n");
}

int main(int argc, char **argv) {
    hostname();
    const char *modo = "doctor";
    int json = 0, aplicar = 0, yes = 0, todo = 0, espera = 0;
    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "scan")) modo = "scan";
        else if (!strcmp(argv[i], "doctor")) modo = "doctor";
        else if (!strcmp(argv[i], "recover")) modo = "recover";
        else if (!strcmp(argv[i], "salud")) modo = "salud";
        else if (!strcmp(argv[i], "entorno")) modo = "entorno";
        else if (!strcmp(argv[i], "repair")) modo = "repair";
        else if (!strcmp(argv[i], "reconectar")) modo = "reconectar";
        else if (!strcmp(argv[i], "todo")) modo = "todo";
        else if (!strcmp(argv[i], "--json")) json = 1;
        else if (!strcmp(argv[i], "--apply")) aplicar = 1;
        else if (!strcmp(argv[i], "--yes")) yes = 1;
        else if (!strcmp(argv[i], "--todo")) todo = 1;
        else if (!strcmp(argv[i], "--espera") && i + 1 < argc) espera = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--version")) {
            printf("mddoctor %s (schema %s)\n", MDDOCTOR_VERSION, MDDOCTOR_SCHEMA);
            return 0;
        }
        else if (!strcmp(argv[i], "selftest")) modo = "selftest";
        else { uso(); return 3; }
    }
    (void)yes;

    if (!strcmp(modo, "selftest")) {
        return autotest() ? 1 : 0;
    }

    if (!strcmp(modo, "entorno")) {
        entorno(json);
        if (json) imprimir_json();
        else imprimir_tabla();
        return peor;
    }

    if (!strcmp(modo, "repair")) {
        return reparar(aplicar);
    }

    if (!strcmp(modo, "reconectar")) {
        return reconectar(aplicar, espera);
    }

    if (!strcmp(modo, "recover")) {
        int a = recuperar(aplicar);
        return a ? 1 : 0;
    }

    if (!strcmp(modo, "salud")) {
        salud(espera);
        if (json) imprimir_json();
        else { imprimir_tabla(); salud_tails(); }
        return peor;
    }

    /* `todo`: el informe completo en una sola llamada (lo que integra el
     * tool MCP `doctor` y la Web UI). */
    if (!strcmp(modo, "todo")) {
        clasificar();
        salud(espera);
        if (json) imprimir_json();
        else { imprimir_tabla(); salud_tails(); }
        return peor;
    }

    clasificar();
    if (!strcmp(modo, "scan")) {
        for (int i = 0; i < NH; i++)
            printf("%-16s %s\n", H[i].clave, H[i].valor);
    } else if (json) {
        imprimir_json();
    } else {
        imprimir_tabla();
    }
    if (todo) {
        printf("\n");
        autotest();
    }
    return peor;
}
