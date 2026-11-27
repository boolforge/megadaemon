// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f72 (void) {
    x2 = [x5 + 0x18] // section..text
    x1 = x7 + 1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    v = x2 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x66e42ec // likely
    [sp + 0x250] = x0
    x0 = x6
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x7e6a0f0
    0x4d9e050 (x0, 0x1, 0x4d0f000, 0x1)
    x5 = [sp + 0x238]
    [x5 + 0x18] = x2
    x27 = 0xb48f000
    w6 = [x27 + 0x450]
    if (!w6) goto 0x66e42e0 // likely
    x2 = [x5 + 8]
    0x4da3500 (x0, 0x1, x2, 0x1)
    stp x0, x2, [x25]
    [x5 + 8] = x0
    x6 = x0
    return x0;
}

