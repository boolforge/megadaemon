// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f44 (void) {
    x5 = [sp + 0x198]
    stp x5, x4, [x25]
    [x3 + 8] = x5
    goto 0x58c9394
    x1 = [x3 + 0x18] // section..text
    callreg x1    // section..text(0x0, 0x4d0f000, 0x0, 0x0)
    if (x3) goto 0x58c9318 // unlikely
    [sp + 0x180] = x1
    [sp + 0xc0] = x0
    x3 = [x2]
    x4 = [sp + 0x190]
    x0 = (byte) [x4]
    x1 = [x4 + 0x10]
    x26 = x2
    callreg x3    // 0x10102464c457f(0x7f, 0x100b70003, 0x0, 0x10102464c457f)
    if (x0) goto 0x58c9300 // likely
    x3 = [sp + 0xc0]
    x27 = (byte) [x3]
    x0 = 0x18
    x1 = 0x8808398
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32830 (0x18, 0x8808398, 0x1, x3)
    x3 = 0x58cfaf0
    x4 = [sp + 0xc0]
    stp x3, x4, [x0]
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (w3) goto 0x58c92c8 // unlikely
    x1 = [sp + 0x180] // section..text
    goto 0x58c92d4
    0x4da34f0 (0x18, 0x4d0f000, 0x1, x3)
    x1 = [sp + 0x180] // section..text
    [x25] = x1
    [x0 + 0x10] = x1
    0x4d5d8f0 (0x18, 0x4d0f000, 0x1, x3)
    x0 = [sp + 0xc0]
    if (!x0) goto 0x58c9330 // likely
    x1 = [x0 + 8]
    x7 = 0x94ccf90
    ldar x8, [x7]
    x9 = [x8]
    w10 = [x0 + 0x10]
    goto 0x58c93b0
    [sp + 0x158] = (x0, 2)
    0x4d5e250 (x0, x1, 0x1, x3)
    (x0, 2) = 3
    (x29, 2) = 3
    sp += 0x250
    return x0;
}

