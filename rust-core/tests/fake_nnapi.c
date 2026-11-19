/*
 * fake_nnapi.c — Implementación de REFERENCIA de la ABI NNAPI (Android
 * Neural Networks API) para testear el backend Rust sin hardware Exynos.
 *
 * Reproduce el header AOSP `neural_networks.h` (NDK API 27+):
 *   ANeuralNetworksModel_create / addOperand / setOperandValue /
 *   addOperation / finish / free
 *   ANeuralNetworksCompilation_create / setPreference / finish / free
 *   ANeuralNetworksExecution_create / setInput / setOutput / compute / free
 *   ANeuralNetworks_getDeviceCount / getDevice
 *   ANeuralNetworksDevice_getName / getVersion / getType
 *
 * NO es un emulador del driver de Samsung: es una implementación de la
 * MISMA ESPECIFICACIÓN, escrita de forma independiente.  Sirve para validar
 * el camino FFI completo del backend Rust (orden de argumentos, layout del
 * struct OperandType, longitudes en bytes, índices del grafo y aritmética
 * entera), que es exactamente lo que no se puede validar en un sandbox.
 *
 * Compilar:  cc -shared -fPIC -O2 -o libfake_nnapi.so fake_nnapi.c -lm
 */

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <stdio.h>

/* ── Tipos ─────────────────────────────────────────────────────────────── */

typedef struct {
    int32_t  type;
    uint32_t dimensionCount;
    const uint32_t *dimensions;
    float    scale;
    int32_t  zeroPoint;
} ANeuralNetworksOperandType;

#define MAX_OPERANDS 64
#define MAX_OPS      16
#define MAX_OP_IN    16
#define MAX_OP_OUT   4

typedef struct {
    ANeuralNetworksOperandType type;
    uint8_t *constData;
    size_t   constLen;
    int32_t  scalarValue;
    int      hasScalar;
    int      hasConst;
} Operand;

typedef struct {
    Operand  operands[MAX_OPERANDS];
    int      nOperands;
    int      opType[MAX_OPS];
    int      opInCount[MAX_OPS];
    uint32_t opInputs[MAX_OPS][MAX_OP_IN];
    int      opOutCount[MAX_OPS];
    uint32_t opOutputs[MAX_OPS][MAX_OP_OUT];
    int      nOps;
    int      finished;
    int      refcount;   /* la compilación mantiene viva la referencia */
} Model;

typedef struct {
    Model *model;
    int    preference;
} Compilation;

typedef struct {
    Compilation *comp;
    uint8_t  *input;
    size_t    inputLen;
    ANeuralNetworksOperandType inputType;
    uint8_t  *output;      /* buffer del llamante (in-place) */
    uint8_t  *outputOwned;  /* sólo si algún día copiamos internamente */
    size_t    outputLen;
    ANeuralNetworksOperandType outputType;
} Execution;

/* "Dispositivos" simulados */
static char  kAccelName[]    = "Fake Exynos NPU (EDEN)";
static char  kAccelVersion[] = "v1.3";
static char  kCpuName[]      = "Fake ARM CPU Reference";
static char  kCpuVersion[]   = "v1.3";

/* ── Modelo ────────────────────────────────────────────────────────────── */

int ANeuralNetworksModel_create(void **model) {
    if (!model) return 3; /* UNEXPECTED_NULL */
    Model *m = (Model *)calloc(1, sizeof(Model));
    if (!m) return 1;    /* OUT_OF_MEMORY */
    m->refcount = 1;
    *model = m;
    return 0;
}

int ANeuralNetworksModel_addOperand(void *model, const ANeuralNetworksOperandType *type) {
    if (!model || !type) return 3;
    Model *m = (Model *)model;
    if (m->nOperands >= MAX_OPERANDS) return 4; /* BAD_DATA */
    Operand *o = &m->operands[m->nOperands++];
    memset(o, 0, sizeof(Operand));
    o->type = *type;
    /* El NDK COPIA las dimensiones: no se guarda el puntero del llamante. */
    if (type->dimensionCount > 0 && type->dimensions) {
        o->type.dimensions = (const uint32_t *)malloc(type->dimensionCount * sizeof(uint32_t));
        if (!o->type.dimensions) return 1;
        memcpy((void *)o->type.dimensions, type->dimensions,
               type->dimensionCount * sizeof(uint32_t));
    } else {
        o->type.dimensions = NULL;
        o->type.dimensionCount = 0;
    }
    return 0;
}

int ANeuralNetworksModel_setOperandValue(void *model, uint32_t index,
                                         const void *buffer, size_t length) {
    if (!model || !buffer) return 3;
    Model *m = (Model *)model;
    if (index >= (uint32_t)m->nOperands) return 4;
    Operand *o = &m->operands[index];
    free(o->constData);
    o->constData = (uint8_t *)malloc(length ? length : 1);
    if (!o->constData) return 1;
    memcpy(o->constData, buffer, length);
    o->constLen = length;
    o->hasConst = 1;
    return 0;
}

int ANeuralNetworksModel_addOperation(void *model, int32_t type,
                                       uint32_t inputCount, const uint32_t *inputs,
                                       uint32_t outputCount, const uint32_t *outputs) {
    if (!model || (inputCount && !inputs) || (outputCount && !outputs)) return 3;
    Model *m = (Model *)model;
    if (m->nOps >= MAX_OPS) return 4;
    if (inputCount > MAX_OP_IN || outputCount > MAX_OP_OUT) return 4;
    int k = m->nOps++;
    m->opType[k] = type;
    m->opInCount[k] = (int)inputCount;
    m->opOutCount[k] = (int)outputCount;
    for (uint32_t i = 0; i < inputCount; i++) m->opInputs[k][i] = inputs[i];
    for (uint32_t i = 0; i < outputCount; i++) m->opOutputs[k][i] = outputs[i];
    return 0;
}

int ANeuralNetworksModel_finish(void *model) {
    if (!model) return 3;
    ((Model *)model)->finished = 1;
    return 0;
}

void ANeuralNetworksModel_free(void *model) {
    if (!model) return;
    Model *m = (Model *)model;
    /* Igual que el NDK: el modelo es reference-counted y la compilación lo
     * mantiene vivo aunque el llamante libere su puntero. */
    if (--m->refcount > 0) return;
    for (int i = 0; i < m->nOperands; i++) {
        free(m->operands[i].constData);
        free((void *)m->operands[i].type.dimensions);
    }
    free(m);
}

/* ── Compilación ───────────────────────────────────────────────────────── */

int ANeuralNetworksCompilation_create(void *model, void **compilation) {
    if (!model || !compilation) return 3;
    Compilation *c = (Compilation *)calloc(1, sizeof(Compilation));
    if (!c) return 1;
    c->model = (Model *)model;
    c->model->refcount++;   /* la compilación referencia al modelo */
    *compilation = c;
    return 0;
}

int ANeuralNetworksCompilation_createForDevices(void *model, const void **devices,
                                                size_t numDevices, void **compilation) {
    (void)devices; (void)numDevices;
    return ANeuralNetworksCompilation_create(model, compilation);
}

int ANeuralNetworksCompilation_setPreference(void *compilation, int32_t preference) {
    if (!compilation) return 3;
    ((Compilation *)compilation)->preference = preference;
    return 0;
}

int ANeuralNetworksCompilation_finish(void *compilation) {
    if (!compilation) return 3;
    return 0;
}

void ANeuralNetworksCompilation_free(void *compilation) {
    if (!compilation) return;
    Compilation *c = (Compilation *)compilation;
    if (c->model) ANeuralNetworksModel_free(c->model);
    free(c);
}

/* ── Ejecución ─────────────────────────────────────────────────────────── */

int ANeuralNetworksExecution_create(void *compilation, void **execution) {
    if (!compilation || !execution) return 3;
    Execution *e = (Execution *)calloc(1, sizeof(Execution));
    if (!e) return 1;
    e->comp = (Compilation *)compilation;
    *execution = e;
    return 0;
}

int ANeuralNetworksExecution_setInput(void *execution, uint32_t index,
                                       const ANeuralNetworksOperandType *type,
                                       const void *buffer, size_t length) {
    if (!execution || !type || !buffer) return 3;
    Execution *e = (Execution *)execution;
    if (index != 0) return 4;
    free(e->input);
    e->input = (uint8_t *)malloc(length ? length : 1);
    if (!e->input) return 1;
    memcpy(e->input, buffer, length);
    e->inputLen = length;
    e->inputType = *type;
    return 0;
}

int ANeuralNetworksExecution_setOutput(void *execution, uint32_t index,
                                        const ANeuralNetworksOperandType *type,
                                        void *buffer, size_t length) {
    if (!execution || !type || !buffer) return 3;
    Execution *e = (Execution *)execution;
    if (index != 0) return 4;
    /* El NDK escribe el resultado EN EL BUFFER DEL LLAMANTE (in-place):
     * no se copia ni se libera. */
    free(e->outputOwned);
    e->outputOwned = NULL;
    e->output = (uint8_t *)buffer;
    e->outputLen = length;
    e->outputType = *type;
    return 0;
}

void ANeuralNetworksExecution_free(void *execution) {
    if (!execution) return;
    Execution *e = (Execution *)execution;
    free(e->input);
    free(e->outputOwned);
    free(e);
}

/* Lee el valor de un operando escalar desde sus bytes estáticos (LE int32),
 * igual que haría el driver real al consumir setOperandValue. */
static int32_t op_scalar(const Operand *o) {
    if (o->hasConst && o->constLen >= 4) {
        int32_t v;
        memcpy(&v, o->constData, 4);
        return v;
    }
    return o->scalarValue;
}

/* Evaluador del grafo (misma aritmética que la spec de NNAPI). */
static int evaluate(Execution *e) {
    Model *m = e->comp->model;
    if (!m || !m->finished) return 6; /* BAD_STATE */
    if (!e->input || !e->output) return 4;

    /* Buffers intermedios indexados por operando. */
    int8_t *bufs[MAX_OPERANDS];
    for (int i = 0; i < m->nOperands; i++) bufs[i] = NULL;

    for (int k = 0; k < m->nOps; k++) {
        int type = m->opType[k];
        if (type == 3) { /* CONV_2D */
            uint32_t inIdx = m->opInputs[k][0];
            uint32_t fIdx  = m->opInputs[k][1];
            uint32_t bIdx  = m->opInputs[k][2];
            int padL = (int)op_scalar(&m->operands[m->opInputs[k][3]]);
            int padR = (int)op_scalar(&m->operands[m->opInputs[k][4]]);
            int padT = (int)op_scalar(&m->operands[m->opInputs[k][5]]);
            int padB = (int)op_scalar(&m->operands[m->opInputs[k][6]]);
            int sw   = (int)op_scalar(&m->operands[m->opInputs[k][7]]);
            int sh   = (int)op_scalar(&m->operands[m->opInputs[k][8]]);
            (void)padR; (void)padB;
            uint32_t outIdx = m->opOutputs[k][0];

            /* La entrada dinámica llega por ANeuralNetworksExecution_setInput. */
            const uint8_t *img = bufs[inIdx] ? bufs[inIdx] : e->input;
            const uint8_t *flt = m->operands[fIdx].constData;
            const int32_t *bias = (const int32_t *)m->operands[bIdx].constData;
            if (!img || !flt) return 4;

            uint32_t ih = m->operands[inIdx].type.dimensions[1];
            uint32_t iw = m->operands[inIdx].type.dimensions[2];
            uint32_t oh = m->operands[outIdx].type.dimensions[1];
            uint32_t ow = m->operands[outIdx].type.dimensions[2];
            uint32_t oc = m->operands[outIdx].type.dimensions[3];

            float inScale  = m->operands[inIdx].type.scale;
            float filtScale = m->operands[fIdx].type.scale;
            float outScale = m->operands[outIdx].type.scale;
            float eff = inScale * filtScale;

            int8_t *out = (int8_t *)calloc((size_t)oh * ow * oc, 1);
            if (!out) return 1;
            for (uint32_t y = 0; y < oh; y++) {
                for (uint32_t x = 0; x < ow; x++) {
                    for (uint32_t c = 0; c < oc; c++) {
                        int32_t acc = bias ? bias[c] : 0;
                        for (int dy = 0; dy < 3; dy++) {
                            for (int dx = 0; dx < 3; dx++) {
                                long iy = (long)(y * sh + dy) - padT;
                                long ix = (long)(x * sw + dx) - padL;
                                int32_t qx = 0;
                                if (iy >= 0 && ix >= 0 && iy < (long)ih && ix < (long)iw)
                                    qx = (int32_t)img[(size_t)iy * iw + ix];
                                int32_t fw = (int32_t)(int8_t)flt[(size_t)c * 9 + dy * 3 + dx];
                                acc += qx * fw;
                            }
                        }
                        float real = (float)acc * eff;
                        int32_t q = (int32_t)roundf(real / outScale);
                        (void)0;
                        if (q > 127) q = 127;
                        if (q < -128) q = -128;
                        out[((size_t)y * ow + x) * oc + c] = (int8_t)q;
                    }
                }
            }
            if (getenv("FAKE_NNAPI_DEBUG")) {
                int32_t mx = 0; int nz = 0;
                for (size_t i = 0; i < (size_t)oh * ow * oc; i++) {
                    if (out[i] != 0) nz++;
                    if (out[i] > mx) mx = out[i];
                }
                fprintf(stderr, "[fake] conv c=%u dims=%ux%u max_q=%d nonzero=%d\n",
                        oc, oh, ow, mx, nz);
            }
            bufs[outIdx] = out;
        } else if (type == 19) { /* RELU */
            uint32_t inIdx = m->opInputs[k][0], outIdx = m->opOutputs[k][0];
            uint32_t ih = m->operands[inIdx].type.dimensions[1];
            uint32_t iw = m->operands[inIdx].type.dimensions[2];
            uint32_t ch = m->operands[inIdx].type.dimensions[3];
            size_t n = (size_t)ih * iw * ch;
            int8_t *out = (int8_t *)malloc(n ? n : 1);
            if (!out) return 1;
            for (size_t i = 0; i < n; i++) {
                int8_t v = bufs[inIdx][i];
                out[i] = v > 0 ? v : 0;
            }
            if (getenv("FAKE_NNAPI_DEBUG")) {
                int32_t mx = 0; int nz = 0;
                for (size_t i = 0; i < n; i++) { if (out[i] != 0) nz++; if (out[i] > mx) mx = out[i]; }
                fprintf(stderr, "[fake] relu n=%zu max=%d nz=%d\n", n, mx, nz);
            }
            bufs[outIdx] = out;
        } else if (type == 17) { /* MAX_POOL_2D */
            uint32_t inIdx = m->opInputs[k][0], outIdx = m->opOutputs[k][0];
            int padL = (int)op_scalar(&m->operands[m->opInputs[k][1]]);
            int padT = (int)op_scalar(&m->operands[m->opInputs[k][3]]);
            int sw   = (int)op_scalar(&m->operands[m->opInputs[k][5]]);
            int sh   = (int)op_scalar(&m->operands[m->opInputs[k][6]]);
            int fw   = (int)op_scalar(&m->operands[m->opInputs[k][7]]);
            int fh   = (int)op_scalar(&m->operands[m->opInputs[k][8]]);
            uint32_t sh_ = m->operands[inIdx].type.dimensions[1];
            uint32_t sw_ = m->operands[inIdx].type.dimensions[2];
            uint32_t ch  = m->operands[inIdx].type.dimensions[3];
            uint32_t oh = m->operands[outIdx].type.dimensions[1];
            uint32_t ow = m->operands[outIdx].type.dimensions[2];
            int8_t *out = (int8_t *)calloc((size_t)oh * ow * ch, 1);
            if (!out) return 1;
            for (uint32_t y = 0; y < oh; y++) {
                for (uint32_t x = 0; x < ow; x++) {
                    for (uint32_t c = 0; c < ch; c++) {
                        int8_t best = -128;
                        for (int dy = 0; dy < fh; dy++) {
                            for (int dx = 0; dx < fw; dx++) {
                                long iy = (long)(y * sh + dy) - padT;
                                long ix = (long)(x * sw + dx) - padL;
                                if (iy < 0 || ix < 0 || iy >= (long)sh_ || ix >= (long)sw_) continue;
                                int8_t v = bufs[inIdx][((size_t)iy * sw_ + ix) * ch + c];
                                if (v > best) best = v;
                            }
                        }
                        if (best < 0) best = 0;
                        out[((size_t)y * ow + x) * ch + c] = best;
                    }
                }
            }
            if (getenv("FAKE_NNAPI_DEBUG")) {
                int32_t mx = 0; int nz = 0;
                for (size_t i = 0; i < (size_t)oh * ow * ch; i++) { if (out[i] != 0) nz++; if (out[i] > mx) mx = out[i]; }
                fprintf(stderr, "[fake] pool dims=%ux%u max=%d nz=%d\n", oh, ow, mx, nz);
            }
            bufs[outIdx] = out;
        } else {
            return 5; /* OP_FAILED: operación no soportada */
        }
    }

    uint32_t outIdx = m->nOps > 0 ? m->opOutputs[m->nOps - 1][0] : 0;
    if (!bufs[outIdx]) return 5;
    size_t n = e->outputLen;
    if (n > 0) memcpy(e->output, bufs[outIdx], n);

    for (int i = 0; i < m->nOperands; i++) free(bufs[i]);
    return 0;
}

int ANeuralNetworksExecution_compute(void *execution) {
    if (!execution) return 3;
    return evaluate((Execution *)execution);
}

/* ── Dispositivos ──────────────────────────────────────────────────────── */

int ANeuralNetworks_getDeviceCount(uint32_t *numDevices) {
    if (!numDevices) return 3;
    *numDevices = 2;
    return 0;
}

int ANeuralNetworks_getDevice(uint32_t devIndex, void **device) {
    if (!device) return 3;
    if (devIndex > 1) return 4;
    *device = (void *)(uintptr_t)(devIndex + 1);
    return 0;
}

int ANeuralNetworksDevice_getName(void *device, const char **name) {
    if (!device || !name) return 3;
    uintptr_t idx = (uintptr_t)device - 1;
    *name = (idx == 0) ? kAccelName : kCpuName;
    return 0;
}

int ANeuralNetworksDevice_getVersion(void *device, const char **version) {
    if (!device || !version) return 3;
    uintptr_t idx = (uintptr_t)device - 1;
    *version = (idx == 0) ? kAccelVersion : kCpuVersion;
    return 0;
}

int ANeuralNetworksDevice_getType(void *device, int32_t *type) {
    if (!device || !type) return 3;
    uintptr_t idx = (uintptr_t)device - 1;
    *type = (idx == 0) ? 4 /* ACCELERATOR */ : 2 /* CPU */;
    return 0;
}
