// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f77 (void) {
    0x4da3530 (x0, x1, x2, x3)
    x7 = [sp + 0x1a8]
    stp x7, x0, [x25]
    [x25 + 0x10] = (x6, 2)
    [x25 + 0x20] = x4
    [x0 + 0x28] = x7
    [x5] = x0
    x4 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [x3 + 0x48] = x4
    [x3 + 0x38] = (x5, 2)
    if (!x3) goto 0x66e4940 // likely
    0x4d98f80 (x0, x1, x2, x3)
    [sp + 0x268] = x0
    x1 = x0
    x2 = 0
    x0 = 0x8319000
    return x0;
}

