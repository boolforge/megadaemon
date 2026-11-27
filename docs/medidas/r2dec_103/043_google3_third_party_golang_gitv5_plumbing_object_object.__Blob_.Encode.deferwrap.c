// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f43 (void) {
    x2 = x4
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x8029050
    0x4d9e050 (x0, x1, x2, 0x1)
    x3 = [sp + 0x210]
    x5 = [sp + 0x188]
    x6 = [sp + 0x260]
    x7 = [sp + 0x2e0]
    x8 = [sp + 0x280]
    x4 = x2
    x9 = x1 - 1
    x27 = 0xb48f000
    w10 = [x27 + 0x450]
    if (!w10) goto 0x55b0860 // likely
    ldr x10, [x0, x9, lsl 3]
    0x4da3500 (x0, x1, x2, x3)
    stp x8, x10, [x25]
    str x8, [x0, x9, lsl 3]
    goto 0x55b07ec
    [sp + 0x240] = x0
    [sp + 0x1e0] = x4
    [sp + 0x140] = x1
    x0 = 0x831e590
    x1 = sp + 0x818
    x2 = sp + 0xe6
    0x4d16d90 (0x831e590, 0x10078818, 0x100780e6, x3)
    x3 = [sp + 0x140]
    x4 = [sp + 0x1e0]
    [x0 + 8] = (x3, 2)
    x27 = 0xb48f000
    return x0;
}

