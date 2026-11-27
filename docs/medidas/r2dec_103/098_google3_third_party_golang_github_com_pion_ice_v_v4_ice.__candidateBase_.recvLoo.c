// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f98 (void) {
    x27 = 0xb48f000
    w8 = [x27 + 0x450]
    if (!w8) goto 0x6eb3580 // likely
    ldr x8, [x0, x5, lsl 3]
    0x4da3500 (x0, x1, x2, x3)
    stp x12, x8, [x25]
    str x12, [x0, x5, lsl 3]
    goto 0x6eb3590
    (x1, 2) = 3
    x0 = [sp + 0xe8]
    x4 = [sp + 0x78]
    x10 = [sp + 0xd8]
    w11 = (byte) [sp + 0x180]
    x26 = [sp + 0x158]
    x5 = x1
    x1 = [sp + 0xc8]
    return x0;
}

