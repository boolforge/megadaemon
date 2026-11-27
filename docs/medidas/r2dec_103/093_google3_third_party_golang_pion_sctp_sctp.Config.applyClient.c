// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f93 (void) {
    w6 = [x27 + 0x450] // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    if (!w6) goto 0x6df625c // unlikely
    ldr x6, [x0, x4, lsl 3]
    0x4da3500 (x0, x1, x2, x3)
    stp x5, x6, [x25]
    [sp + 0x210] = x0
    [sp + 0x160] = (x2, 2)
    str x5, [x0, x4, lsl 3]
    x4 = [sp + 0x348]
    x5 = [x4 + 0x230]
    if (x5) goto 0x6df6400 // likely
    x1 = sp + 0x5c
    stp 0, 0, [x1]
    [x1 + 0x10] = (0, 2)
    [x1 + 0x20] = (0, 2)
    [x1 + 0x30] = 0
    [x1 0x34] = 0
    x4 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [sp + 0x5c] = (byte) w4
    x4 = [sp + 0x358]
    w4 = (word) [x4 + 0x1a8] // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    [sp + 0x5e] = (half) w4
    x5 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [sp + 0x60] = (byte) w5
    x6 = 2
    [sp + 0x62] = (byte) w6
    [sp + 0x64] = (half) w4
    [sp + 0x66] = (byte) 0
    x6 = 0xb00    // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    sturh w6, [sp, 0x67]
    [sp + 0x6a] = (half) w4
    [sp + 0x6c] = (byte) 0
    x7 = 0xc00    // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    sturh w7, [sp, 0x6d]
    [sp + 0x70] = (half) w4
    [sp + 0x72] = (byte) 0
    x7 = 0xd00    // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    sturh w7, [sp, 0x73]
    [sp + 0x76] = (half) w4
    [sp + 0x78] = (byte) 0
    x7 = 0xe00    // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    sturh w7, [sp, 0x79]
    [sp + 0x7c] = (half) w4
    [sp + 0x7e] = (byte) 0
    sturh w6, [sp, 0x7f]
    [sp + 0x82] = (half) w4
    [sp + 0x84] = (byte) w5
    x6 = 0x1000   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    sturh w6, [sp, 0x85]
    [sp + 0x88] = (half) w4
    [sp + 0x8a] = (byte) w5
    x6 = 0xf00    // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    sturh w6, [sp, 0x8b]
    [sp + 0x8e] = (half) w4
    [sp + 0x90] = (byte) w5
    x6 = 0x1400   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    sturh w6, [sp, 0x91]
    x4 += 1
    [sp + 0x94] = (half) w4
    [sp + 0x96] = (byte) w5
    [sp + 0x97] = (byte) 0
    x0 = [sp + 0x350]
    x2 = 0xa
    x3 = x2
    0x6df8480 (x0, 0x1007805c, 0xa, 0xa)
    x3 = [sp + 0x118]
    x4 = x3 + x1
    v = x2 - x4   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x6df637c // unlikely
    [sp + 0x100] = x1
    x1 = x4
    x4 = 0x856f5d0
    0x4d9e050 (x0, 0x1007805c, 0xa, x3)
    x3 = [sp + 0x118]
    x4 = x1
    x1 = [sp + 0x100]
    [sp + 0x128] = (x4, 2)
    [sp + 0x1c8] = x0
    x0 += x1
    x1 = [sp + 0x1c0]
    x2 = x3
    0x4da39e0 (0x1007805c, x1, x2, x3)
    x3 = [sp + 0x348]
    (x3, 2) = 3
    x3 = [x3 + 0x40] // elf_phdr
    callreg x3    // 0x400000006(0x0, 0x0, 0x0, 0x400000006)
    x3 = [sp + 0x348]
    (x4, 2) = 3   // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x2 = [x3 + 0xb8]
    x3 = [sp + 0x1c8]
    x5 = [sp + 0x130]
    x6 = 0x431493c
    x7 = 0xf
    x8 = x0
    x0 = x4       // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x4 = [sp + 0x128]
    0x6dd9740 (0x200000, 0x66474e551, x2, x3)
    x5 = [sp + 0x348]
    x27 = x5 + 0x230
    stp x1, x2, [x27]
    x27 = 0xb48f000
    w6 = [x27 + 0x450]
    if (!w6) goto 0x6df63f8 // likely
    x6 = [x5 + 0x228]
    0x4da3500 (0x200000, 0x66474e551, x2, x3)
    stp x0, x6, [x25]
    [x5 + 0x228] = x0
    if (x3) goto 0x6df65a4 // unlikely
    x0 = 0x18
    x1 = 0x88474d8
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32830 (0x18, 0x88474d8, 0x1, x3)
    x3 = [sp + 0x348]
    x27 = x3 + 0x228
    (x4, 2) = 3
    x6 = [x3 + 0x238]
    [x0 + 8] = (x5, 2)
    x27 = 0xb48f000
    w5 = [x27 + 0x450]
    if (!w5) goto 0x6df643c // likely
    0x4da34f0 (0x18, 0x88474d8, 0x1, x3)
    [x25] = x4
    [sp + 0x200] = x0
    [x0] = x4
    x0 = 0x30     // '0'
    x1 = 0x8a86da0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32f40 (0x30, 0x8a86da0, 0x1, x3)
    x3 = 0x8f61358
    [x0 + 0x18] = x3
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (w3) goto 0x6df6478 // unlikely
    x3 = [sp + 0x200]
    goto 0x6df6484
    0x4da34f0 (0x30, 0x8a86da0, 0x1, 0x18)
    x3 = [sp + 0x200]
    [x25] = x3
    [sp + 0x220] = x0
    [x0 + 0x20] = x3
    x3 = [[sp + 0x348] + 0x160]
    [sp + 0x158] = x3
    x0 = 0x10
    x1 = 0x8af33b0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32500 (0x10, 0x8af33b0, 0x1, 0x779b960)
    [sp + 0x218] = x0
    x0 = 0x38     // '8'
    x1 = 0x88c89d8
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33310 (0x38, 0x88c89d8, 0x1, 0x779b960)
    x27 = 0x981a000
    w3 = (word) [x27 + 0x154]
    [x0 + 4] = (half) w3
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [x0 + 6] = (half) w3
    x3 = 0x8f61328
    [x0 + 0x28] = x3
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (w3) goto 0x6df64fc // unlikely
    x6 = [sp + 0x218]
    x8 = [sp + 0x220]
    goto 0x6df6514
    x6 = [sp + 0x218]
    x7 = [x6]
    0x4da3510 (0x38, 0x88c89d8, 0x1, x3)
    x8 = [sp + 0x220]
    stp x8, x0, [x25]
    [x25 + 0x10] = x7
    [x0 + 0x30] = x8
    [x6] = x0
    x1 = [sp + 0x168] + 1
    x7 = [sp + 0x158]
    v = x7 - 0    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x7 = (ne)? 1 : 0 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [x6 + 9] = (byte) w7
    x7 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [x6 + 8] = (byte) w7
    [x6 + 0xa] = (byte) w7 // (pstr 0x000004d0) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x2 = [sp + 0x160]
    v = x2 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x6df6554 // likely
    x0 = [sp + 0x210]
    goto 0x6df656c
    x0 = [sp + 0x210]
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x7f13088
    0x4d9e050 (x0, 0x1, x2, 0x1)
    x6 = [sp + 0x218]
    x7 = x1 - 1
    x27 = 0xb48f000
    w8 = [x27 + 0x450]
    if (!w8) goto 0x6df6588 // likely
    ldr x8, [x0, x7, lsl 3]
    0x4da3500 (x0, 0x1, x2, 0x1)
    stp x6, x8, [x25]
    return x0;
}

