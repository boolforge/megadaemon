// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f90 (void) {
    if (!x5) goto 0x6df5808 // likely
    (x6, 2) = 3
    if (!x7) goto 0x6df5808 // unlikely
    x5 = [x5 + 0x18] // section..text
    [x0 + 0x20] = (x7, 2)
    x27 = 0xb48f000
    w5 = [x27 + 0x450]
    if (!w5) goto 0x6df5804 // likely
    x5 = [x0 + 0x18] // section..text
    0x4da3500 (x0, x1, x2, x3)
    stp x6, x5, [x25]
    [x0 + 0x18] = x6
    x0 = 0x30     // '0'
    x1 = 0x8a86da0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32f40 (0x30, 0x8a86da0, 0x1, x3)
    x3 = 0x8f613e8
    [x0 + 0x18] = x3
    return x0;
}

