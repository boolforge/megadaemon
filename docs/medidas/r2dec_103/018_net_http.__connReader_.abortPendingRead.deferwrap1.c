// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f18 (void) {
    x2 = sp + 0x500
    stp 0, 0, [x2]
    [x2 + 0x10] = (0, 2)
    [x2 + 0x20] = (0, 2)
    [x2 + 0x30] = (0, 2)
    [x2 + 0x40] = (0, 2)
    [x2 + 0x50] = 0
    x1 = [sp + 0x1d0]
    x27 = sp + 0x500
    stp x0, x1, [x27]
    x3 = 0x8f0a128
    x27 = sp + 0x510
    stp x1, x3, [x27]
    x1 = [sp + 0x3e8]
    [sp + 0x520] = x1
    x1 = -1
    x27 = sp + 0x548
    stp x1, x1, [x27]
    x27 = 0xb48f000
    w1 = [x27 + 0x450]
    if (!w1) goto 0x527d5b4 // likely
    x0 = 0x8ccd890
    x1 = [sp + 0x248]
    0x4d35930 (0x8ccd890, x1, 0x10078500, 0x8f0a128)
    x2 = sp + 0x500
    x3 = [sp + 0x248]
    (q16, 2) = 3
    stp q16, q17, [x3]
    (q16, 2) = 3
    [x3 + 0x20] = (q16, 2)
    return x0;
}

