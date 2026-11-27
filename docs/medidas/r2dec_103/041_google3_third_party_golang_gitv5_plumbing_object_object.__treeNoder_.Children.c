// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f41 (void) {
    [sp + 0x610] = x3
    0x4d9c1c0 (x0, x1, x2, x3)
    [sp + 0x608] = x0
    x3 = [sp + 0x210] - 1
    x0 = [sp + 0x518]
    x4 = [sp + 0x180]
    x5 = [sp + 0x258]
    x6 = [sp + 0x1c8]
    x7 = [sp + 0x2f8]
    goto 0x55b0388
    x10 = [sp + 0x170]
    x3 = x10 - 1
    x0 = x4
    x4 = x8
    x5 = x9
    if (x3 != 0x3f) goto 0x55b0580 // likely
    add x8, x3, x3, lsl 1
    add x9, x0, x8, lsl 3
    x10 = [x9 + 8]
    v = x10 - x4  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x55b07d4 // likely
    ldr x11, [x0, x8, lsl 3]
    ldr x10, [x5, x10, lsl 3]
    v = x11 - x6  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x55b07d0 // likely
    [sp + 0x170] = x3
    ldr x2, [x7, x11, lsl 3]
    if (!x2) goto 0x55b0570 // unlikely
    if (!x10) goto 0x55b0570 // unlikely
    [sp + 0x510] = x9
    [sp + 0x270] = x10
    [sp + 0x300] = x2
    [sp + 0x210] = x8
    x0 = 0x8311ff0
    x1 = sp + 0x680
    0x4d184e0 (0x8311ff0, 0x10078680, -1, -1)
    x27 = (byte) [x0]
    x0 = 0x8311ff0
    x1 = sp + 0x600
    x2 = [sp + 0x270]
    0x4d184e0 (0x8311ff0, 0x10078600, -1, -1)
    x27 = (byte) [x0]
    x0 = 0x8883888
    0x4d31c10 (0x8883888, 0x10078600, -1, -1)
    [sp + 0x548] = x0
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (!w3) goto 0x55b044c // likely
    x1 = x0
    x2 = [sp + 0x270]
    x0 = 0x89bec68
    0x4d35930 (0x89bec68, 0x8883888, -1, x3)
    x1 = [sp + 0x548] + 0x40
    x3 = [sp + 0x300]
    x2 = x3 + 0x40 // (pstr 0x00000600) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x0 = 0x89bec68
    0x4d35930 (0x89bec68, 0x88838c8, 0x3f, -1)
    return x0;
}

