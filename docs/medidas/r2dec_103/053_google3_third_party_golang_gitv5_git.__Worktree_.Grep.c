// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f53 (void) {
    [sp + 0x1d8] = (x3, 2)
    [sp + 0x1e8] = (x1, 2)
    x0 = sp + 0x1d8
    0x55a7390 (0x100781d8, x1, x2, x3)
    if (!x1) goto 0x5950220 // likely
    x0 = 0xb48f500
    (x29, 2) = 3
    sp += 0x240
    x0 = [sp + 0x190]
    x1 = [sp + 0x108]
    x5 = [sp + 0x248]
    x8 = 0x877af48
    w4 = (byte) [sp + 0xaf]
    return x0;
}

