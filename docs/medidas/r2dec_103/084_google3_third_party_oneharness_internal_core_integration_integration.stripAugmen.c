// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f84 (void) {
    0x4d98880 (x0, x1, x2, x3)
    x2 = 0x856f550
    [sp + 0x1a0] = (x2, 2)
    x0 = 0x423dd09
    x1 = 4
    x2 = sp + 0x1a0
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = x3       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4e34a60 (0x423dd09, 0x4, 0x100781a0, 0x1)
    [sp + 0x148] = x0
    [sp + 0xc8] = x1
    x2 = [sp + 0x150]
    x3 = [x2 + 0x38]
    if (x3) goto 0x67ca6ac // likely
    w3 = (byte) [sp + 0x77]
    x4 = 0
    x5 = 0
    x6 = 0
    goto 0x67ca764
    [sp + 0x1a0] = (0, 2)
    (x0, 2) = 3
    0x4d98880 (0x38004000000000, 0x30003500400010, x2, x3)
    x2 = 0x856f550
    [sp + 0x1a0] = (x2, 2)
    x0 = 0x4248750
    x1 = 5
    x2 = sp + 0x1a0
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = x3       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4e34a60 (0x4248750, 0x5, 0x100781a0, 0x1)
    w2 = (byte) [sp + 0x77]
    if (w2 != 0) goto 0x67ca6fc // unlikely
    x5 = sp + 0x208
    stp 0, 0, [x5]
    [x5 + 0x10] = (0, 2)
    x2 = 2
    goto 0x67ca72c
    [sp + 0xe8] = x1
    [sp + 0x1e8] = x0
    x0 = 0
    return x0;
}

