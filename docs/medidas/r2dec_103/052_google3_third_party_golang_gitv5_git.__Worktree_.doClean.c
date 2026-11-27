// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f52 (void) {
    [x0] = x3
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (w3) goto 0x594fc74 // unlikely
    x1 = [sp + 0x138]
    x2 = [sp + 0x148]
    goto 0x594fc84
    0x4da3500 (x0, x1, x2, x3)
    x1 = [sp + 0x138]
    x2 = [sp + 0x148]
    stp x1, x2, [x25]
    [x0 + 8] = (x1, 2)
    x1 = [sp + 0xd8]
    x2 = [sp + 0xe8]
    [x0 + 0x18] = (x1, 2)
    x1 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [x0 + 0x28] = (byte) w1
    x1 = [sp + 0x160]
    (x1, 2) = 3   // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX" // "ki/codeium_common_pb/codeium_common_go_proto.SearchResultType.S"
    x3 = 0x8f5bb68
    [sp + 0x1f8] = (x3, 2)
    x27 = sp + 0x208
    stp x1, x2, [x27]
    x0 = sp + 0x1f8 // (pstr 0x08f5bb68) "p\u0556"
    0x55a7390 (0x100781f8, 0x16236a0, 0x200000, 0x8f5bb68)
    if (x1) goto 0x594fe1c // likely
    x3 = [sp + 0x100]
    v = x3 - 1    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x594fcf0 // likely
    x1 = [sp + 0x108]
    x5 = [sp + 0x248]
    x8 = 0x877af48
    w4 = (byte) [sp + 0xaf]
    x3 = x0       // (pstr 0x08f5bb68) "p\u0556"
    x0 = [sp + 0x190]
    goto 0x594fab8
    x0 = sp + 0x1f8 // (pstr 0x08f5bb68) "p\u0556"
    0x55a7390 (0x100781f8, x1, 0x200000, 0x100781f8)
    if (x1) goto 0x594fe08 // unlikely
    x1 = [sp + 0x108]
    x5 = [sp + 0x248]
    x8 = 0x877af48
    w4 = (byte) [sp + 0xaf]
    x3 = x0       // (pstr 0x08f5bb68) "p\u0556"
    x0 = [sp + 0x190]
    goto 0x594fab8
    x8 = 0x88b0648
    goto 0x594fde8
    x9 = 0x3cf8c08
    v = w9 + w8
    if (v) goto 0x594fde0 // likely
    x8 = 0x88b0648
    v = x6 - x8   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x594fde8 // likely
    w0 = (byte) [x7 + 8]
    [sp + 0xae] = (byte) w0
    x0 = [x7]
    [sp + 0x180] = x0
    0x4d98f80 (0x10102464c457f, x1, 0x200000, 0x100781f8)
    x1 = sp + 0x1a8
    stp 0, 0, [x1]
    [x1 + 0x10] = (0, 2)
    [x1 + 0x20] = (0, 2)
    x2 = section..noptrbss
    [sp + 0x1b0] = (x0, 2)
    x0 = [sp + 0x160]
    [sp + 0x1d0] = x0
    [sp + 0x168] = 0
    x0 = 0x5960690
    [sp + 0x218] = x0
    w0 = (byte) [sp + 0xae]
    [sp + 0x220] = (byte) w0
    x0 = [sp + 0x180]
    x2 = sp + 0x168
    x27 = sp + 0x228
    stp x0, x2, [x27]
    x0 = x1
    x1 = sp + 0x218
    0x55a9430 (0x100781a8, 0x10078218, 0x10078168, 0x100781f8)
    if (x0) goto 0x594febc // likely
    x3 = [sp + 0x168]
    if (!x3) goto 0x594fe30 // likely
    x0 = [sp + 0x190]
    x1 = [sp + 0x108]
    x5 = [sp + 0x248]
    x8 = 0x877af48
    w4 = (byte) [sp + 0xaf]
    goto 0x594fab8
    x8 = 0x877af48
    goto 0x594fab8
    x8 = 0x877af48
    goto 0x594fab8
    x0 = 0xb48f500
    (x29, 2) = 3
    sp += 0x240
    x0 = 0xb48f500
    (x29, 2) = 3
    sp += 0x240
    x2 = [sp + 0x180]
    (x0, 2) = 3
    [sp + 0x198] = (0, 2)
    0x4d98880 (0x10102464c457f, x1, x2, x3)
    x2 = 0x856f550
    [sp + 0x198] = (x2, 2)
    x0 = 0x4407337
    x1 = 0x22     // '\"'
    x2 = sp + 0x198
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = x3       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4e321f0 (0x4407337, 0x22, 0x10078618, 0x1)
    return x0;
}

