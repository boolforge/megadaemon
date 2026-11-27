// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f66 (void) {
    x7 = 0x8f08ba8
    v = x5 - x7   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x66e3ecc // unlikely
    x6 = 0
    if (v) goto 0x66e3ee0 // likely
    (x5, 2) = 3
    goto 0x66e3ee8
    x7 = 0x8f08ba8
    x6 = 0
    x5 = 0
    x8 = [sp + 0x230]
    [x8 + 0x10] = x6
    x27 = 0xb48f000
    w6 = [x27 + 0x450]
    if (!w6) goto 0x66e3f08 // likely
    x6 = [x8 + 8]
    0x4da3500 (x0, x1, x2, x3)
    stp x5, x6, [x25]
    return x0;
}

