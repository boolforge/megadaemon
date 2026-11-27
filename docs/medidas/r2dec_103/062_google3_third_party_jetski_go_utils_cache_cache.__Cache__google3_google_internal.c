// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f62 (void) {
    x3 = [sp + 0x230]
    x4 = [x3 + 0x38]
    x5 = [sp + 0x260]
    x6 = [x5]
    0x4da3560 (x0, x1, x2, x3)
    x7 = [sp + 0x1e0]
    x8 = [sp + 0x1a0]
    stp x7, x8, [x25]
    x9 = [sp + 0x248]
    x10 = [sp + 0x198]
    [x25 + 0x10] = (x9, 2)
    [x25 + 0x20] = (x0, 2)
    [x25 + 0x30] = (x5, 2)
    [x0 + 8] = x7
    x4 = [sp + 0xe0]
    [x0 + 0x18] = (x8, 2)
    return x0;
}

