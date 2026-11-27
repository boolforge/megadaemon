// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f2 (void) {
    x15 = x4
    x4 = x13
    x13 = x17
    [sp + 0x198] = x4
    [sp + 0xf8] = (x9, 2)
    [sp + 0x190] = x5
    [sp + 0xe8] = (x11, 2)
    [sp + 0x188] = x6
    [sp + 0xd8] = (x16, 2)
    x14 = [sp + 0x2b0]
    if (!x14) goto 0x5199be8 // likely
    [sp + 0x6f] = (byte) w0
    [sp + 0x6e] = (byte) w1
    [sp + 0x6d] = (byte) w2
    [sp + 0xc8] = (x13, 2)
    [sp + 0x180] = x7
    [sp + 0x6c] = (byte) w3
    [sp + 0x138] = (0, 2)
    [sp + 0x148] = 0
    [sp + 0x108] = (0, 2)
    [sp + 0x118] = 0
    [sp + 0x5b] = (byte) 0
    x0 = sp + 0x2a8
    x1 = sp + 0x138
    x2 = 0x30     // '0'
    0x51613f0 (0x100782a8, 0x10078138, 0x30, x3)
    if (w0 != 0) goto 0x51990ac // unlikely
    x4 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    goto 0x51990c4
    x0 = sp + 0x138
    x1 = sp + 0x108
    x2 = sp + 0x5b
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x51619c0 (0x10078138, 0x10078108, 0x1007805b, 0x1)
    x4 = x0 ^ 1
    if (w4 != 0) goto 0x5199b50 // likely
    w3 = (byte) [sp + 0x5b]
    v = w3 - 0x82 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x51993c0 // unlikely
    v = w3 - 0x87 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x519911c // likely
    (x3, 2) = 3
    x4 = [sp + 0x118]
    v = x0 - 8    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x51990fc // likely
    x0 = x3 + 4
    x2 = x4 - 4
    x1 = 4
    goto 0x5199110
    v = x0 - 0x20 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x51994c8 // likely
    x0 = x3 + 0x10
    x2 = x4 - 0x10
    x1 = 0x10
    x5 = 0
    x6 = 0
    goto 0x5199d7c
    v = w3 - 0x81 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x5199284 // unlikely
    v = w3 - 0x86 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x5199164 // unlikely
    x14 = 1       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x17 = [sp + 0x290]
    [x17] = (byte) w14
    (x3, 2) = 3
    (x5, 2) = 3
    (x1, 2) = 3
    (x9, 2) = 3
    w0 = (byte) [sp + 0x6f]
    w10 = (byte) [sp + 0x6e]
    w11 = (byte) [sp + 0x6d]
    w12 = (byte) [sp + 0x6c]
    (x14, 2) = 3
    (x7, 2) = 3
    goto 0x5199008
    (x1, 2) = 3
    x0 = 0
    0x4d7d610 (x0, x1, x2, x3)
    [sp + 0x160] = x0
    [sp + 0xb0] = x1
    0x5194f90 (x0, x1, x2, x3)
    if (x0) goto 0x51996f4 // unlikely
    x0 = [sp + 0x160]
    x1 = [sp + 0xb0]
    0x4eeeac0 (x0, x1, x2, x3)
    q0 = [sp 8]
    [sp + 0x70] = q0
    q0 = [sp + 0x70]
    [sp 0x5c] = q0
    v = w0 & 1    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = sp + 0x5c
    x2 = (ne)? x2 : 0
    if (x2) goto 0x5199638 // unlikely
    x0 = [sp + 0x160]
    x1 = [sp + 0xb0]
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x5189060 (x0, x1, 0x1, x3)
    if (w0 == 0) goto 0x519957c // likely
    x5 = [sp + 0xc8]
    x1 = x5 + 1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = [sp + 0xd0]
    v = x2 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x51991e0 // likely
    x7 = [sp + 0x180]
    w0 = (byte) [sp + 0x6f]
    goto 0x5199228
    v = x1 - 2    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v > 0) goto 0x519920c // unlikely
    w0 = (byte) [sp + 0x6f]
    if (w0 != 0) goto 0x519920c // unlikely
    if (x5) goto 0x519920c // unlikely
    x7 = sp + 0x1a0
    stp 0, 0, [x7]
    [x7 + 0x10] = (0, 2)
    x2 = 2
    x0 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    goto 0x5199228
    x0 = [sp + 0x180]
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x856f550
    0x4d9e050 (x0, 0x1, 0x2, 0x1)
    x7 = x0
    w0 = (byte) [sp + 0x6f]
    x3 = x1 - 1
    add x3, x7, x3, lsl 4
    x4 = [sp + 0xb0]
    [x3 + 8] = x4
    x27 = 0xb48f000
    w4 = [x27 + 0x450]
    if (w4) goto 0x519924c // unlikely
    x5 = [sp + 0x160]
    goto 0x519925c
    x4 = [x3]
    0x4da3500 (x0, 0x1, 0x2, x3)
    x5 = [sp + 0x160]
    stp x5, x4, [x25]
    [x3] = x5
    (x3, 2) = 3
    (x5, 2) = 3
    return x0;
}

