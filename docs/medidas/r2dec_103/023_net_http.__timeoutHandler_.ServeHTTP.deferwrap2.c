// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f23 (void) {
    x2 = [sp + 0x280]
    stp x2, x1, [x25]
    v = x4 - x6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [x0] = x2
    if (((unsigned) v) > 0) goto 0x527e498 // unlikely
    x3 = x4 & (x4 - x6)
    x0 = x6 - x4
    add x1, x2, x3, lsl 4
    goto 0x527e2d0 // f22+0x8
    x0 = 0x8dbefe0
    x1 = [sp + 0x3a0]
    0x4d18e50 (0x8dbefe0, x1, x2, x3)
    [x0 + 8] = (0, 2)
    x27 = 0xb48f000
    w4 = [x27 + 0x450]
    if (!w4) goto 0x527e3cc // likely
    x1 = [x0]
    0x4da34f0 (0x8dbefe0, 0x8, x2, x3)
    [x25] = x1
    [x0] = 0
    x0 = [sp + 0x118]
    x1 = [sp + 0x280]
    goto 0x527e2d0 // f22+0x8
    x2 = [sp + 0x3a0]
    [sp + 0x2c8] = x2
    return x0;
}

