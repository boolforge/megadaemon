// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f33 (void) {
    x2 = [sp + 0x290]
    ldr x2, [x2, x0, lsl 3]
    [sp + 0x2a8] = x2
    x3 = sp + 0x5a
    return x0;
}

