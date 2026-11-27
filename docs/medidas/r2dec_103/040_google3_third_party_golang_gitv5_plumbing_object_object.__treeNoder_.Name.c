// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f40 (void) {
    x3 = sp + 0x5b0
    stp 0, 0, [x3]
    [x3 + 0x10] = (0, 2)
    [x3 + 0x20] = (0, 2)
    return x0;
}

