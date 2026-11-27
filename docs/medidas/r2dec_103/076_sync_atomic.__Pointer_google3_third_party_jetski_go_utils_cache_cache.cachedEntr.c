// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f76 (void) {
    x5 = 0
    if (v) goto 0x66e4990 // likely
    (x2, 2) = 3
    goto 0x66e4998
    x6 = 0x8f09ae8
    x5 = 0
    x2 = 0
    [sp + 0x1a8] = x2
    [sp + 0xe8] = x5
    x0 = 0x8e59c90
    0x4d31c10 (0x8e59c90, x1, x2, x3)
    [sp + 0x228] = x0
    x0 = 8
    x1 = 0x81a1be0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d322b0 (0x8, 0x81a1be0, 0x1, x3)
    [sp + 0x260] = x0
    return x0;
}

