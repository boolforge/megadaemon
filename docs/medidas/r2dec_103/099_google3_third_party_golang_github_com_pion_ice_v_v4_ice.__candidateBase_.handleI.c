// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f99 (void) {
    x1 = [sp + 0xb8] + 1
    x8 = [sp + 0xe0]
    (x9, 2) = 3   // section..text
    [sp + 0x108] = (x9, 2)
    x2 = [sp + 0xc0]
    v = x2 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x6eb3730 // likely
    x0 = [sp + 0x130]
    goto 0x6eb3748
    x0 = [sp + 0x130]
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x88ba218
    0x4d9e050 (x0, 0x1, x2, 0x1)
    x5 = sp + 0xf0
    add x9, x1, x1, lsl 2
    x9 = x0 + ((x9 << 3) - 0x28)
    return x0;
}

