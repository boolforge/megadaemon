// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f64 (void) {
    x6 = 0
    if (v) goto 0x66e3e4c // likely
    w5 = (byte) [x6]
    if (w5 == 0) goto 0x66e3e4c // unlikely
    x0 = [sp + 0x1f0]
    if (!x0) goto 0x66e3e14 // likely
    (x5, 2) = 3
    x7 = 0x8f08ba8
    v = x5 - x7   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x66e3e08 // unlikely
    x6 = 0
    if (v) goto 0x66e3e1c // likely
    (x5, 2) = 3
    goto 0x66e3e24
    x7 = 0x8f08ba8
    x6 = 0
    x5 = 0
    x8 = [sp + 0x230]
    return x0;
}

