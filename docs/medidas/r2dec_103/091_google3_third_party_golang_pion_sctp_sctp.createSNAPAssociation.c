// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f91 (void) {
    x7 = [sp + 0x220]
    stp x7, x0, [x25]
    [x25 + 0x10] = x6
    [x0 + 0x30] = x7
    [x5] = x0
    x6 = [sp + 0x168]
    x1 = x6 + 1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = [sp + 0x160]
    v = x2 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x6df58e8 // likely
    x0 = [sp + 0x210]
    goto 0x6df5900
    x0 = [sp + 0x210]
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x7f13088
    0x4d9e050 (x0, 0x1, x2, 0x1)
    x5 = [sp + 0x218]
    [sp + 0x160] = (x2, 2)
    x4 = x1 - 1
    x27 = 0xb48f000
    w6 = [x27 + 0x450]
    if (!w6) goto 0x6df5920 // likely
    ldr x6, [x0, x4, lsl 3]
    0x4da3500 (x0, 0x1, x2, 0x1)
    stp x5, x6, [x25]
    [sp + 0x210] = x0
    str x5, [x0, x4, lsl 3]
    [sp + 0x54] = 0
    x4 = 0xc
    [sp + 0x52] = (byte) w4
    x4 = [sp + 0x358]
    w4 = (word) [x4 + 0x1a8] // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    [sp + 0x54] = (half) w4
    [sp + 0x56] = (byte) 0
    [sp + 0x57] = (byte) 0
    x0 = [sp + 0x350]
    x1 = sp + 0x52
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x3 = x2       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x6df8480 (x0, 0x10078052, 0x1, 0x1)
    x4 = sp + 0x2c0
    stp 0, 0, [x4]
    [x4 + 0x10] = (0, 2)
    [x4 + 0x20] = (0, 2)
    [x4 + 0x30] = (0, 2)
    [x4 + 0x40] = (0, 2)
    [x4 + 0x50] = (0, 2)
    if (x1) goto 0x6df59dc // likely
    x4 = sp + 0x260
    stp 0, 0, [x4]
    [x4 + 0x10] = (0, 2)
    [x4 + 0x20] = (0, 2)
    [x4 + 0x30] = (0, 2)
    [x4 + 0x40] = (0, 2)
    [x4 + 0x50] = (0, 2)
    x0 = [sp + 0x338]
    x1 = [sp + 0x340]
    x2 = [sp + 0x348]
    x3 = [sp + 0x358]
    0x6df0390 (x0, x1, x2, x3)
    if (x1) goto 0x6df59b8 // unlikely
    x3 = sp + 0x2c0
    goto 0x6df5a4c
    x3 = x0
    x4 = x1
    x5 = x2
    x0 = 0
    x1 = 0
    x2 = 0
    (x29, 2) = 3
    sp += 0x330
    return x0;
}

