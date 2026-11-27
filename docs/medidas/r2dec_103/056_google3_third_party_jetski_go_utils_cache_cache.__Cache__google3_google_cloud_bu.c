// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f56 (void) {
    goto 0x66e3804
    x13 = 0
    x12 = 0
    [sp + 0x2b8] = x0
    [sp + 0x290] = x7
    [sp + 0x298] = x8
    [sp + 0x2a0] = x3
    [sp + 0x1f8] = x4
    [sp + 0x288] = x5
    [sp + 0x200] = x6
    [sp + 0x1d8] = x9
    [sp + 0x108] = x13
    [sp + 0x1c8] = x12
    [sp + 0x110] = x11
    [sp + 0x1d0] = x10
    x14 = 0
    goto 0x66e387c
    [x8] = x1
    x27 = 0xb48f000
    w1 = [x27 + 0x450]
    if (!w1) goto 0x66e3858 // likely
    x1 = [x8 + 8]
    0x4da3500 (x0, x1, x2, x3)
    stp x2, x1, [x25]
    return x0;
}

