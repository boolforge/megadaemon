// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f13 (void) {
    (x7, 2) = 3
    goto 0x527c968
    x27 = sp + 0x6c8
    (x7, 2) = 3
    v = x8 - 5    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x527cc04 // likely
    w8 = [x7]
    x9 = 0x70747468
    v = w8 - w9   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x527cc04 // likely
    w7 = (byte) [x7 + 4]
    v = w7 - 0x73 // 's' // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x527cc04 // likely
    if (x0) goto 0x527c9a0 // unlikely
    x27 = sp + 0x6d8
    (x3, 2) = 3
    goto 0x527c9ac
    0x52820b0 (x0, x1, x2, x3)
    x4 = x1
    x3 = x0
    x0 = [sp + 0x2a0]
    x1 = [sp + 0x200]
    x2 = [sp + 0x3b8]
    x5 = [sp + 0x268]
    0x527bcf0 (x0, x1, x2, x3)
    if (!x0) goto 0x527cc04 // likely
    x3 = [sp + 0x6c0]
    return x0;
}

