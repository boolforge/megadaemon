// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f8 (void) {
    x13 = x4
    (x15, 2) = 3
    x7 = [sp + 0x180]
    x3 = x1
    x4 = x2
    (x1, 2) = 3
    goto 0x5199008
    x5 += 1       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    v = x5 - 0xa  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v >= 0) goto 0x519a00c // unlikely
    w6 = (byte) [x3 + x5]
    if (!w6) goto 0x5199ff4 // unlikely
    goto 0x519a02c
    w5 = (byte) [x3 + 0xa]
    v = w5 - 0xff // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x519a02c // likely
    w5 = (byte) [x3 + 0xb]
    v = w5 - 0xff // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x519a02c // likely
    x5 = x3 + 0xc
    goto 0x519a030
    x5 = 0
    if (!x5) goto 0x5199ee0 // likely
    x0 = 0x10
    x1 = 0x8757890
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32500 (0x10, 0x8757890, 0x1, x3)
    x3 = 0x44682ad
    x4 = 0x36     // '6'
    stp x3, x4, [x0]
    x1 = 0
    x2 = 0
    x3 = 0
    x4 = 0
    x5 = 0
    x6 = 0
    x7 = 0
    x8 = 0
    x9 = 0
    x10 = 0
    x11 = 0
    x12 = 0x8f07508
    x13 = x0
    x0 = 0
    (x29, 2) = 3
    return x0;
}

