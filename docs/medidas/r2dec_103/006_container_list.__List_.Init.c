// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f6 (void) {
    [x0 + 0x28] = x5
    [x0 + 0x18] = (x6, 2)
    x5 = [sp + 0xd8]
    x1 = x5 + 1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = [sp + 0xe0]
    v = x2 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x5199f50 // likely
    x4 = [sp + 0x198]
    w3 = (byte) [sp + 0x6d]
    goto 0x5199fa0
    [sp + 0x178] = x0
    v = x1 - 4    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v > 0) goto 0x5199f80 // unlikely
    w3 = (byte) [sp + 0x6d]
    if (w3 != 0) goto 0x5199f80 // unlikely
    if (x5) goto 0x5199f80 // unlikely
    x4 = sp + 0x1e0
    stp 0, 0, [x4]
    [x4 + 0x10] = (0, 2)
    x2 = 4
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    goto 0x5199fa0
    x0 = [sp + 0x198]
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    return x0;
}

