// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f22 (void) {
    x1 = [sp + 0x3a8]
    goto 0x527e2e8
    [sp + 0x118] = x0
    [sp + 0x280] = x1
    x0 = sp + 0x5e8
    0x4d35870 (0x100785e8, x1, x2, x3)
    x0 = [sp + 0x118]
    x1 = [sp + 0x280]
    x27 = sp + 0x5e8
    (x5, 2) = 3
    if (!x5) goto 0x527e3dc // likely
    [sp + 0x118] = x0
    [sp + 0x280] = x1
    (x2, 2) = 3
    (x5, 2) = 3
    if (!x5) goto 0x527e3a0 // unlikely
    return x0;
}

