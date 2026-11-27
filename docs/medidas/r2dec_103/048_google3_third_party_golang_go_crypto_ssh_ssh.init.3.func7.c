// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f48 (void) {
    x3 = 0x80fc090
    x4 = x0
    x0 = x1
    x1 = 0
    0x590e750 (x0, x1, x2, 0x80fc090)
    x2 = [sp + 0xa8]
    x3 = x2 + x1
    x4 = [sp + 0xb0]
    v = x4 - x3   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x59144fc // unlikely
    x5 = [sp + 0xe0]
    goto 0x5914448 // f47+0x40
    [sp + 0xa0] = x1
    [sp + 0xd8] = x0
    x0 = [sp + 0xe0]
    x2 = x4
    x4 = 0x856f5d0
    x5 = x1
    x1 = x3
    x3 = x5
    0x4d9e050 (x0, x1, x2, x3)
    x5 = x0
    x3 = x1
    x4 = x2
    x0 = [sp + 0xd8]
    (x1, 2) = 3
    goto 0x5914448 // f47+0x40
    x5 = x1 + 1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    v = x0 - x5   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x591458c // unlikely
    v = x5 - 0x20 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v > 0) goto 0x5914564 // unlikely
    if (x1) goto 0x5914564 // unlikely
    x2 = sp + 0xb8
    return x0;
}

