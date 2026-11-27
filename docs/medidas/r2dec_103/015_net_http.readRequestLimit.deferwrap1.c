// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f15 (void) {
    w8 = [x7]
    x9 = 0x70747468
    v = w8 - w9   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x527d0a0 // likely
    w7 = (byte) [x7 + 4]
    v = w7 - 0x73 // 's' // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x527d0a0 // likely
    x0 = [sp + 0x2a0]
    x27 = sp + 0x6d8
    (x3, 2) = 3
    x1 = [sp + 0x200]
    x2 = [sp + 0x3b8]
    x5 = [sp + 0x268]
    0x527bcf0 (x0, x1, x2, x3)
    if (!x0) goto 0x527d0a0 // likely
    [sp + 0x2a0] = 0
    x27 = sp + 0x320
    stp x0, x1, [x27]
    0x4d5e250 (x0, x1, x2, x3)
    x27 = sp + 0x320
    (x1, 2) = 3
    x0 = [sp + 0x2a0]
    (x29, 2) = 3
    sp += 0x6a0
    x7 = [sp + 0x2a0]
    x8 = [x7 + 0x60]
    return x0;
}

