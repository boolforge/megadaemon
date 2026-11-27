// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f36 (void) {
    x5 = [sp + 0x2d0]
    goto 0x55af9e0
    ldr x4, [x8, x3, lsl 3]
    0x4da3500 (x0, x1, x2, x3)
    x5 = [sp + 0x2d0]
    stp x5, x4, [x25]
    str x5, [x8, x3, lsl 3]
    goto 0x55af5e4
    x0 = 0x8883888
    0x4d31c10 (0x8883888, x1, x2, x3)
    [sp + 0x548] = x0
    x1 = [sp + 0x260]
    x2 = [x1]
    x27 = (byte) [x2]
    x27 = 0xb48f000
    w1 = [x27 + 0x450]
    if (!w1) goto 0x55afa44 // likely
    [sp + 0x530] = x2
    x1 = x0
    return x0;
}

