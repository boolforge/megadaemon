// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f3 (void) {
    x1 = [sp + 0xc0]
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x5189060 (x0, x1, 0x1, x3)
    if (w0 == 0) goto 0x5199868 // likely
    x5 = [sp + 0xe8]
    x1 = x5 + 1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = [sp + 0xf0]
    v = x2 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x5199308 // likely
    x6 = [sp + 0x188]
    w0 = (byte) [sp + 0x6e]
    goto 0x5199350
    v = x1 - 2    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v > 0) goto 0x5199334 // unlikely
    w0 = (byte) [sp + 0x6e]
    if (w0 != 0) goto 0x5199334 // unlikely
    if (x5) goto 0x5199334 // unlikely
    x6 = sp + 0x1c0
    stp 0, 0, [x6]
    [x6 + 0x10] = (0, 2)
    x2 = 2
    x0 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    goto 0x5199350
    x0 = [sp + 0x188]
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x856f550
    0x4d9e050 (x0, 0x1, 0x2, 0x1)
    x6 = x0
    w0 = (byte) [sp + 0x6e]
    x3 = x1 - 1
    add x3, x6, x3, lsl 4
    x4 = [sp + 0xc0]
    [x3 + 8] = x4
    x27 = 0xb48f000
    w4 = [x27 + 0x450]
    if (w4) goto 0x5199374 // unlikely
    x5 = [sp + 0x170]
    goto 0x5199384
    x4 = [x3]
    0x4da3500 (x0, 0x1, 0x2, x3)
    x5 = [sp + 0x170]
    stp x5, x4, [x25]
    [x3] = x5
    (x3, 2) = 3
    x5 = [sp + 0xf8]
    x8 = x2
    x9 = x1       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x10 = x0
    w11 = (byte) [sp + 0x6d]
    w12 = (byte) [sp + 0x6c]
    (x14, 2) = 3
    x15 = x6
    x7 = [sp + 0x180]
    x6 = [sp + 0x100]
    (x1, 2) = 3
    w0 = (byte) [sp + 0x6f]
    goto 0x5199008
    (x1, 2) = 3
    x0 = 0
    0x4d7d610 (x0, x1, x2, x3)
    [sp + 0x168] = x0
    [sp + 0xb8] = x1
    0x5194f90 (x0, x1, x2, x3)
    if (x0) goto 0x5199a98 // unlikely
    x0 = [sp + 0x168]
    x1 = [sp + 0xb8]
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x5189060 (x0, x1, 0x1, x3)
    if (w0 == 0) goto 0x51999dc // likely
    x5 = [sp + 0xf8]
    x1 = x5 + 1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = [sp + 0x100]
    v = x2 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x5199410 // likely
    x5 = [sp + 0x190]
    return x0;
}

