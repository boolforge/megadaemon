/* test-mddoctor.c — banco de pruebas del núcleo forense en C.
 *
 *   POR QUÉ EXISTE
 *   -------------
 *   El binario `mddoctor` se publica en el móvil y se usa CUANDO el daemon
 *   está roto: si su SHA-256 miente, el verificador de "la build en servicio
 *   es la nueva" miente con él, y eso es peor que no verificar nada.  Por eso
 *   el SHA-256 se contrasta contra `sha256sum`, el cliente HTTP contra un
 *   servidor real y el parser del túnel contra un log con trampas.
 *
 *   Los static de mddoctor son privados, asi que este fichero lo INCLUYE
 *   (con `main` renombrado) para probarlos sin duplicar la logica.
 *
 *   USO:  tools/test-mddoctor.sh   (prepara los fixtures y llama a esto)
 *   Los fixtures se buscan en $MDDOCTOR_TEST_FIX (por defecto /tmp).
 */

#define main mddoctor_main
#include "mddoctor.c"
#undef main

#include <stdio.h>

static int fallos = 0;
static int pruebas = 0;

static void ok(int cond, const char *que) {
    pruebas++;
    if (cond) printf("  [ OK  ] %s\n", que);
    else { printf("  [FALLO] %s\n", que); fallos++; }
}

static const char *fixdir(void) {
    const char *d = getenv("MDDOCTOR_TEST_FIX");
    return (d && *d) ? d : "/tmp";
}

static const char *fx(const char *name) {
    static char buf[512];
    snprintf(buf, sizeof(buf), "%s/%s", fixdir(), name);
    return buf;
}

/* El puerto del servidor de prueba lo decide el script (por defecto 18099). */
static int puerto(void) {
    const char *p = getenv("MDDOCTOR_TEST_PORT");
    return (p && *p) ? atoi(p) : 18099;
}

int main(void) {
    char hex[65], ref[65], linea[256];

    /* Las rutas del movil se apuntan a un directorio temporal: el binario
     * tiene que poder probarse sin /data. */
    char pref[512], svc[512];
    snprintf(pref, sizeof(pref), "%s/prefix", fixdir());
    snprintf(svc, sizeof(svc), "%s/service", fixdir());
    setenv("MDDOCTOR_PREFIX", pref, 1);
    setenv("MDDOCTOR_SERVICE_D", svc, 1);

    printf("== SHA-256 contra sha256sum ==\n");
    const char *nombres[] = {"vacio", "abc", "grande"};
    for (int i = 0; i < 3; i++) {
        const char *f = fx(nombres[i]);
        char cmd[600];
        snprintf(cmd, sizeof(cmd), "sha256sum '%s' | cut -c1-64", f);
        FILE *p = popen(cmd, "r");
        ref[0] = 0;
        if (p && fgets(linea, sizeof(linea), p)) {
            trim(linea);
            snprintf(ref, sizeof(ref), "%.64s", linea);
        }
        if (p) pclose(p);
        sha256_hex(f, hex);
        char msg[640];
        snprintf(msg, sizeof(msg), "sha256 de %s", nombres[i]);
        ok(strcmp(hex, ref) == 0, msg);
        if (strcmp(hex, ref) != 0)
            printf("         mio=%s\n         ref=%s\n", hex, ref);
    }

    printf("== vector conocido (cadena vacia) ==\n");
    sha256_hex(fx("vacio"), hex);
    ok(strncmp(hex, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", 64) == 0,
       "sha256 de la cadena vacia (vector conocido de la RFC)");

    printf("== cliente HTTP contra un servidor real ==\n");
    char cuerpo[4096];
    int code = http_get("127.0.0.1", puerto(), "/health", cuerpo, sizeof(cuerpo));
    ok(code == 200, "codigo HTTP 200");
    ok(strstr(cuerpo, "abc1234") != NULL && strstr(cuerpo, "deadbeef") != NULL,
       "el cuerpo llega integro");
    ok(strstr(cuerpo, "HTTP/1.0 200 OK") != NULL, "la cabecera llega");

    printf("== extraccion de campo JSON ==\n");
    char v[64];
    json_campo(cuerpo, "short", v, sizeof(v));
    ok(strcmp(v, "abc1234") == 0, "json_campo lee el valor");
    json_campo(cuerpo, "inexistente", v, sizeof(v));
    ok(strcmp(v, "?") == 0, "json_campo devuelve ? si no existe");

    printf("== puerto sin servidor ==\n");
    ok(http_get("127.0.0.1", 1, "/health", cuerpo, sizeof(cuerpo)) == 0,
       "un puerto sin servidor da 0 (no un codigo inventado)");

    printf("== la URL real del tunel (api.trycloudflare.com NO es un tunel) ==\n");
    char url[512];
    leer_tunel(url, sizeof(url));
    ok(strcmp(url, "https://segundo-tunel.trycloudflare.com") == 0,
       "toma el ULTIMO host bueno del log (como el tail -1 del script)");
    ok(strstr(url, "api.") == NULL, "descarta api.trycloudflare.com");

    printf("== cola de lineas ==\n");
    char cola[1024];
    tail_file(fx("lineas"), 3, cola, sizeof(cola));
    ok(strstr(cola, "linea18") != NULL, "la cola incluye la linea 18");
    ok(strstr(cola, "linea20") != NULL, "la cola incluye la linea 20");
    ok(strstr(cola, "linea17") == NULL, "la cola NO incluye la linea 17");

    printf("== procesos (sin pgrep) ==\n");
    char procs[1024];
    procesos_megadaemon(procs, sizeof(procs));
    ok(strlen(procs) > 0, "devuelve algo (o 'ninguno')");

    printf("\n%d pruebas, %d fallos\n", pruebas, fallos);
    return fallos ? 1 : 0;
}
