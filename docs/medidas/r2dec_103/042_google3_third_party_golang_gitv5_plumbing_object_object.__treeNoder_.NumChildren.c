// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f42 (void) {
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x8029050
    0x4d9e050 (x0, x1, x2, 0x1)
    x3 = x0
    x0 = [sp + 0x2b8]
    x4 = x1 - 1
    x27 = 0xb48f000
    w5 = [x27 + 0x450]
    if (!w5) goto 0x55b075c // likely
    ldr x5, [x3, x4, lsl 3]
    0x4da3500 (x0, x1, x2, x3)
    stp x0, x5, [x25]
    str x0, [x3, x4, lsl 3]
    goto 0x55b0698
    x1 = [sp + 0x138]
    x2 = [sp + 0x148]
    x3 = [sp + 0x238]
    goto 0x55b0698
    x0 = 0x831e590
    x1 = sp + 0x818
    x2 = sp + 0xd2
    0x4d16d90 (0x831e590, 0x10078818, 0x100780d2, x3)
    x3 = [sp + 0x138]
    x4 = [sp + 0x148]
    [x0 + 8] = (x3, 2)
    x27 = 0xb48f000
    return x0;
}

