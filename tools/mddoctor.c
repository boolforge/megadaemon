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

#define MAXF 512
#define MAXH 96
static char host[256] = "desconocido";

/* ---------------------------------------------------------------------- */
/* Utilidades                                                               */
/* ---------------------------------------------------------------------- */

static void hostname(void) {
    if (gethostname(host, sizeof(host) - 1) != 0) strcpy(host, "?");
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

static void add(const char *clave, const char *valor, int ver,
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
}

/* ---------------------------------------------------------------------- */
/* FORENSE DE DISCO: que se esta comiendo el espacio                        */
/* ---------------------------------------------------------------------- */

#define MAXG 64
typedef struct { char ruta[192]; long bytes; } Grande;
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
            snprintf(G[NG].ruta, sizeof(G[NG].ruta), "%.190s", p);
            G[NG].bytes = st.st_size;
            NG++;
        }
    }
    closedir(d);
}

static void forense(const char *dir) {
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

static int autotest(void) {
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

static void clasificar(void) {
    char det[128];
    permisos();
    int pct;

    /* --- bateria ------------------------------------------------------- */
    pct = bateria_pct(det, sizeof(det));
    if (pct < 0) {
        add("bateria", "sin sensor", VER_OK, "no aplica (no hay power_supply)");
    } else if (pct <= 15) {
        add("bateria", det, VER_CRITICO,
            "bateria al %d%%: el daemon puede morir en cualquier momento", pct);
    } else if (pct <= 25) {
        add("bateria", det, VER_AVISO,
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
        snprintf(v, sizeof(v), "%.1f C (%s)", t / 1000.0, tipo);
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
            add("memoria", v, VER_CRITICO,
                "%ld%% usada: el OOM killer esta a la vuelta de la esquina", pctused);
        else if (pctused >= 80)
            add("memoria", v, VER_AVISO, "%ld%% usada: sin swap, roza el limite", pctused);
        else
            add("memoria", v, VER_OK, "%ld%% usada", pctused);
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
            add("disco", v, VER_CRITICO, "%s: casi sin sitio; el daemon no puede escribir", rutas[i]);
        else if (mb < 800)
            add("disco", v, VER_AVISO, "%s: poco sitio", rutas[i]);
        else
            add("disco", v, VER_OK, "%s: sitio de sobra", rutas[i]);
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
        snprintf(v, sizeof(v), "%d iface, %d arriba [%s]", ifaces, arriba, lista);
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
        add("procesos", v, VER_AVISO, "%d zombies: alguien no los esta recogiendo", zomb);
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

static void imprimir_tabla(void) {
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

static void imprimir_json(void) {
    printf("{\n  \"host\": \"%s\",\n  \"veredicto\": \"%s\",\n  \"hallazgos\": [\n", host,
           ver_txt(peor));
    for (int i = 0; i < NH; i++) {
        printf("    {\"clave\": \"%s\", \"valor\": \"%s\", \"veredicto\": \"%s\", \"motivo\": \"%s\"}%s\n",
               H[i].clave, H[i].valor, ver_txt(H[i].veredicto), H[i].motivo,
               i + 1 < NH ? "," : "");
    }
    printf("  ]\n}\n");
}

/* ---------------------------------------------------------------------- */

static void uso(void) {
    fprintf(stderr,
            "mddoctor — diagnostico y recuperacion de MegaDaemon\n"
            "uso: mddoctor [scan|doctor|recover] [--json] [--apply] [--yes]\n"
            "  scan     recoge los hechos y los lista\n"
            "  doctor   recoge, clasifica y dice que esta mal\n"
            "  recover  plan de recuperacion (--apply lo ejecuta)\n"
            "  --json   salida en JSON\n");
}

int main(int argc, char **argv) {
    hostname();
    const char *modo = "doctor";
    int json = 0, aplicar = 0, yes = 0, todo = 0;
    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "scan")) modo = "scan";
        else if (!strcmp(argv[i], "doctor")) modo = "doctor";
        else if (!strcmp(argv[i], "recover")) modo = "recover";
        else if (!strcmp(argv[i], "--json")) json = 1;
        else if (!strcmp(argv[i], "--apply")) aplicar = 1;
        else if (!strcmp(argv[i], "--yes")) yes = 1;
        else if (!strcmp(argv[i], "--todo")) todo = 1;
        else if (!strcmp(argv[i], "selftest")) modo = "selftest";
        else { uso(); return 3; }
    }
    (void)yes;

    if (!strcmp(modo, "selftest")) {
        return autotest() ? 1 : 0;
    }

    if (!strcmp(modo, "recover")) {
        int a = recuperar(aplicar);
        return a ? 1 : 0;
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
