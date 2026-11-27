// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f73 (void) {
    [x25] = x1
    [x0 + 0x90] = x1
    if (!x0) goto 0x66e43d8 // likely
    [sp + 0x220] = x0
    0x4d98f80 (x0, x1, x2, x3)
    [sp + 0x268] = x0
    x1 = x0
    x2 = 0
    x0 = 0x8319090
    0x4d177a0 (0x8319090, x1, x2, x3)
    x27 = 0xb48f000
    w1 = [x27 + 0x450]
    if (w1) goto 0x66e4378 // unlikely
    x4 = [sp + 0x220]
    goto 0x66e4388
    x3 = [x0]
    0x4da3500 (0x8319090, x1, x2, 0x8)
    x4 = [sp + 0x220]
    stp x4, x3, [x25]
    [x0] = x4
    x0 = 0x48     // 'H'
    x1 = 0x8c419c8
    return x0;
}

