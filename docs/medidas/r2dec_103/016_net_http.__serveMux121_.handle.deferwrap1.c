// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f16 (void) {
    [sp + 0x178] = x2
    x0 = x2
    x1 = x3
    x2 = x5
    0x4d24430 (x0, x1, x2, x3)
    if (w0 != 0) goto 0x527d278 // unlikely
    x2 = [sp + 0x178]
    x3 = [sp + 0x2e8]
    [sp + 0x2a0] = 0
    x27 = sp + 0x320
    stp x2, x3, [x27]
    0x4d5e250 (x0, x1, x2, x3)
    x27 = sp + 0x320
    (x1, 2) = 3
    x0 = [sp + 0x2a0]
    (x29, 2) = 3
    return x0;
}

