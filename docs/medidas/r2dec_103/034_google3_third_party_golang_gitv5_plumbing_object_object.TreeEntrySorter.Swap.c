// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f34 (void) {
    0x4d35800 (x0, x1, x2, x3)
    x0 = 0
    x1 = 0
    x2 = 0
    x3 = 0
    x4 = 0
    x5 = 0
    x6 = 0
    x7 = 0
    goto 0x55af408
    [sp + 0x218] = x5
    [sp + 0x540] = x2
    [sp + 0x210] = x7
    [sp + 0x128] = x6
    [sp + 0x208] = x9
    [sp + 0x230] = x3
    [sp + 0xfb] = (byte) w0
    [sp + 0xfa] = (byte) w1
    x0 = sp + 0x4a8
    0x4d35870 (0x100784a8, x1, x2, x3)
    w0 = (byte) [sp + 0xfb]
    w1 = (byte) [sp + 0xfa]
    x2 = [sp + 0x540]
    x3 = [sp + 0x230]
    x4 = [sp + 0x208]
    x5 = [sp + 0x218]
    x6 = [sp + 0x128]
    x7 = [sp + 0x210]
    [sp + 0x128] = x6
    [sp + 0x230] = x3
    x27 = sp + 0x4a8
    (x8, 2) = 3
    if (!x8) goto 0x55af58c // likely
    [sp + 0xfb] = (byte) w0
    [sp + 0xfa] = (byte) w1
    (x8, 2) = 3
    x9 = [x9 + 0x10]
    v = x10 - 1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x55af4d4 // likely
    x9 = x4 + 1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x8 = [x8]
    v = x5 - x9   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x55af4b0 // unlikely
    [sp + 0x548] = x8
    [sp + 0x130] = x7
    v = x9 - 4    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v > 0) goto 0x55af474 // unlikely
    if (w1 != 0) goto 0x55af474 // unlikely
    if (x4) goto 0x55af474 // unlikely
    x2 = sp + 0x590
    stp 0, 0, [x2]
    [x2 + 0x10] = (0, 2)
    x5 = 4
    x1 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    goto 0x55af4b0
    x0 = x2
    return x0;
}

