// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f12 (void) {
    0x4d9e050 (x0, x1, x2, x3)
    x3 = [sp + 0xd0]
    x5 = 3
    [x0 + 3] = (byte) w5
    v = x2 - 5    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x526faa8 // unlikely
    x1 = 5
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x856f5d0
    0x4d9e050 (x0, 0x5, x2, 0x1)
    x3 = [sp + 0xd0]
    [x0 + 4] = (byte) w3
    x1 = x3 + 5
    v = x2 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x526fac8 // unlikely
    x4 = 0x856f5d0
    0x4d9e050 (x0, 0x5, x2, x3)
    x3 = [sp + 0xd0]
    [sp + 0xd8] = x1
    [sp + 0x1d0] = x0
    [sp + 0xe0] = x2
    x0 += 5
    x1 = [sp + 0x1a8]
    x2 = x3
    0x4da39e0 (0x5, x1, x2, x3)
    (x1, 2) = 3
    x2 = [sp + 0x1d0]
    goto 0x526fb3c
    x0 = 0x10
    x1 = 0x8757890
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32500 (0x10, 0x8757890, 0x1, x3)
    x3 = 0x42ec3c7
    x4 = 0xd
    stp x3, x4, [x0]
    [sp + 0x178] = (0, 2)
    x3 = 0x8f07508
    [sp + 0x1c0] = (x3, 2)
    0x4d5e250 (0x10, 0x8757890, 0x1, 0x8f07508)
    (x2, 2) = 3
    (x0, 2) = 3
    (x29, 2) = 3
    sp += 0x240
    x1 += 2
    v = x0 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x526fb70 // unlikely
    x3 = 2
    x4 = 0x856f5d0
    x5 = x0
    x0 = x2
    x2 = x5
    0x4d9e050 (0x8f07508, 0x2, x2, 0x2)
    x3 = x0
    x0 = x2
    x2 = x3
    [sp + 0x1d0] = x2
    [sp + 0xd8] = (x1, 2)
    x4 = x1 - 2
    x5 = x2 + x4
    x6 = [sp + 0xc0]
    x7 = x6 >> 8
    [x2 + x4] = (byte) w7
    [x5 + 1] = (byte) w6 // (pstr 0x00008b26) "towardl"
    x4 = [[sp + 0x260] + 0x50]
    x3 = x0
    x0 = [sp + 0x268]
    x5 = x1
    x1 = x2
    x2 = x5
    callreg x4    // elf_phdr(0x0, 0x8f07508, 0x2, 0x0)
    [sp + 0x1c0] = (x1, 2)
    if (x1) goto 0x526fbe8 // likely
    x4 = [sp + 0xe0]
    v = x4 - 4    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x527021c // likely
    x0 = [sp + 0x260]
    if (!x0) goto 0x526fc00 // likely
    x1 = [x0 + 8]
    x6 = 0x949d220
    ldar x7, [x6]
    x8 = [x7]
    w9 = [x0 + 0x10]
    goto 0x52701dc
    0x4d5e250 (x0, x1, 0x2, x3)
    (x2, 2) = 3
    (x0, 2) = 3
    (x29, 2) = 3
    sp += 0x240
    x1 = [sp + 0x268]
    x2 = [sp + 0x1d0]
    x3 = 4
    x5 = x3
    0x4dd5330 (x0, x1, x2, 0x4)
    [sp + 0x1c0] = (x1, 2)
    if (x1) goto 0x526ff5c // unlikely
    x0 = [sp + 0xd8]
    if (!x0) goto 0x52701d8 // likely
    x3 = [sp + 0x1d0]
    w4 = (byte) [x3]
    v = w4 - 5    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x526fec0 // likely
    v = x0 - 1    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x52701d4 // likely
    w4 = (byte) [x3 + 1] // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (x4) goto 0x526fe2c // likely
    v = x0 - 2    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x52701d0 // likely
    w4 = (byte) [x3 + 2]
    if (w4) goto 0x526fde0 // likely
    x0 = 0x30     // '0'
    x1 = 0x89bdc00
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32f40 (0x30, 0x89bdc00, 0x1, x3)
    x3 = [sp + 0xd8]
    v = x3 - 3    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x52701cc // likely
    [sp + 0x230] = x0
    x2 = [sp + 0x1d0]
    w6 = (byte) [x2 + 3]
    v = w6 - 1    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x526fd9c // unlikely
    v = w6 - 3    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x526fcb4 // likely
    x1 = [sp + 0x260]
    if (!x1) goto 0x5270138 // likely
    x6 = [x1 + 8]
    x7 = 0x949d240
    ldar x8, [x7]
    x9 = [x8]
    w10 = [x1 + 0x10]
    goto 0x5270184
    v = w6 - 4    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x526fd00 // likely
    x0 = 0x856f5d0
    x1 = 0x10
    return x0;
}

