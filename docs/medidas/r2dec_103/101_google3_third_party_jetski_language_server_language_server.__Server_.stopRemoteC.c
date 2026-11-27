// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f101 (void) {
    [x27] = q0
    x0 = sp + 0x20f
    stp 0, 0, [x0]
    [x0 + 0x10] = (0, 2)
    [x0 0x1c] = 0
    x27 = sp + 0x1df
    q0 = [x27]
    [sp 8] = q0
    x1 = 0x24     // '$'
    x2 = x1
    0x52bffc0 (0x1007820f, 0x24, 0x24, x3)
    x0 = 0
    x1 = sp + 0x20f
    x2 = 0x24     // '$'
    0x4d7d610 (x0, 0x1007820f, 0x24, x3)
    x4 = [sp + 0x2f8]
    x8 = [sp + 0x700]
    x6 = x1
    x5 = x0
    x7 = [sp + 0x800]
    (x9, 2) = 3
    x11 = 0x8f5b0e8
    v = x9 - x11  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x714e9ec // likely
    x9 = [x10]
    (x9, 2) = 3
    x11 = 0x8f54218
    v = x9 - x11  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x714e9d4 // likely
    [sp + 0x2f8] = x4
    [sp + 0x700] = x8
    [sp + 0x2f0] = x6
    [sp + 0x6e0] = x5
    x0 = [x10 + 0x18]
    x27 = sp + 0xa68
    stp 0, 0, [x27]
    0x4d98810 (0x4d0f00000, 0x1007820f, 0x24, x3)
    x1 = 0x856f790
    x27 = sp + 0xa68
    stp x1, x0, [x27]
    x0 = 0x42d5d81
    x1 = 0xc
    x2 = sp + 0xa68
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = x3       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4e34a60 (0x42d5d81, 0xc, 0x10078a68, 0x1)
    [sp + 0xa28] = x0
    [sp + 0x450] = x1
    x2 = [sp + 0x730]
    if (x2) goto 0x714ba28 // unlikely
    x3 = 0
    x4 = 0
    x5 = 0
    x6 = 0
    x7 = 0
    goto 0x714ba8c
    x0 = 0x10
    x1 = 0x8742878
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    return x0;
}

