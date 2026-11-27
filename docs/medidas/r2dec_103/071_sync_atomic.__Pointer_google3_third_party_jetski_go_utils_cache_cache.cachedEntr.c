// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f71 (void) {
    x1 = [sp + 0xc0]
    [x0 + 0x18] = x1
    x27 = 0xb48f000
    w1 = [x27 + 0x450]
    if (w1) goto 0x66e4268 // unlikely
    (x6, 2) = 3
    x7 = [sp + 0x170]
    goto 0x66e427c
    0x4da3510 (x0, x1, x2, x3)
    (x6, 2) = 3
    stp x5, x6, [x25]
    x7 = [sp + 0x170]
    [x25 + 0x10] = x7
    [x0 + 0x10] = x5
    x5 = [sp + 0xb8]
    [x0 + 0x20] = (x6, 2)
    return x0;
}

