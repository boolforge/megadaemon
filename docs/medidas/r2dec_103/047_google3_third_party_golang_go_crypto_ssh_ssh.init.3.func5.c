// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f47 (void) {
    [sp + 0x1a0] = (x1, 2)
    x2 = sp + 0x120
    stp 0, 0, [x2]
    [x2 + 0x10] = (0, 2)
    [x2 + 0x20] = (0, 2)
    [x2 + 0x30] = (0, 2)
    [x2 + 0x40] = (0, 2)
    [x2 + 0x50] = (0, 2)
    x0 = 0x8910360
    x1 = x5
    0x4d35800 (0x8910360, x1, 0x10078120, x3)
    x0 = 0
    x1 = 0
    x2 = 0
    goto 0x5914474
    [sp + 0xa8] = (x3, 2)
    [sp + 0xe8] = x5
    x3 = x5 + x2
    x2 = x1
    x1 = x0
    x0 = x3
    0x4da39e0 (x0, x1, x2, x3)
    x0 = sp + 0x120
    return x0;
}

