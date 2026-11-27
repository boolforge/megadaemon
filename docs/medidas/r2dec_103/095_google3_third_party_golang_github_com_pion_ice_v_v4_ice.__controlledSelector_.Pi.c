// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f95 (void) {
    x0 = 0x8eba5d0
    x1 = sp + 0x308
    0x4d2e1f0 (0x8eba5d0, 0x10078308, x2, x3)
    x2 = 0x8eba5d0
    x3 = [sp + 0x240]
    [x3 + 0x10] = x2
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x6eae140 // likely
    x2 = [x3 + 0x18] // section..text
    0x4da3500 (0x8eba5d0, 0x10078308, 0x4d0f000, x3)
    stp x0, x2, [x25]
    [x3 + 0x18] = x0
    x2 = [sp + 0x4d8]
    (x0, 2) = 3   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    0x4d98880 (0x90, 0x458, x2, x3)
    x2 = 0x856f550
    x3 = [sp + 0x240]
    [x3 + 0x20] = x2
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x6eae178 // likely
    x6 = [x3 + 0x28] // elf_shdr
    0x4da3500 (0x90, 0x458, x2, x3)
    stp x0, x6, [x25]
    [x3 + 0x28] = x0
    x6 = [sp + 0x4d8]
    x27 = x6 + 0x2b8
    (x6, 2) = 3   // segment.LOAD5
    x6 = [x6 + 0x60]
    x1 = 0x4488ab8
    x2 = 0x41     // 'A'
    x4 = 3
    x5 = x4
    callreg x6    // 0x400000003(0xd8d6298, 0x4488ab8, 0x41, 0x0)
    w0 = (byte) [sp + 0x6c]
    goto 0x6eadda4
    x1 = 0
    x2 = 0
    x5 = 0
    [sp + 0x6e] = (byte) w0
    [sp + 0x80] = x2
    x6 = sp + 0x4d0
    x7 = sp
    x6 -= x7      // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x8 = x1 - x7
    v = x8 - x6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x6eae220 // likely
    x0 = 0x88ba188
    x2 = x5
    0x4d79b60 (0x88ba188, x1, x2, x3)
    x2 = [sp + 0x80]
    x3 = [sp + 0x150]
    x4 = [sp + 0x88]
    x5 = x1
    x1 = x0
    goto 0x6eae220
    x1 = [sp + 0x238] + 0x18
    x6 = [sp + 0x100]
    x5 = x6 - 1
    x2 = [sp + 0x80]
    x3 = [sp + 0x150]
    x4 = [sp + 0x88]
    v = x5 - 0    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v <= 0) goto 0x6eae490 // likely
    [sp + 0x100] = x5
    [sp + 0x238] = x1
    (x5, 2) = 3   // elf_phdr // section..text
    [sp + 0xf8] = x5
    [sp + 0x260] = x6
    x1 = [x1 + 0x10] // elf_shdr
    [sp + 0xf0] = x1
    x5 = sp + 0x2a8
    x6 = 0x4a15008
    (q16, 2) = 3
    stp q16, q17, [x5]
    (q16, 2) = 3  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [x5 + 0x20] = (q16, 2)
    (q16, 2) = 3
    [x5 + 0x40] = (q16, 2)
    x27 = sp + 0x2b8
    stp x3, x4, [x27]
    x3 = [sp + 0x210]
    x4 = [sp + 0xe8]
    x27 = sp + 0x2c8
    stp x3, x4, [x27]
    [sp + 0x2d8] = x1
    [sp + 0x2f8] = x2
    w1 = (byte) [sp + 0x6a]
    [sp + 0x300] = (byte) w1
    x0 = sp + 0x168
    (q16, 2) = 3
    stp q16, q17, [x0]
    (q16, 2) = 3  // elf_shdr // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [x0 + 0x20] = (q16, 2)
    (q16, 2) = 3
    [x0 + 0x40] = (q16, 2)
    0x6ea8ed0 (0x10078168, x1, x2, x3)
    [sp + 0x208] = x0
    if (!x1) goto 0x6eae2c0 // likely
    x7 = [sp + 0x4d8]
    goto 0x6eae2ec
    x7 = [sp + 0x4d8]
    w8 = (byte) [x7 + 0x90]
    v = w8 - 3    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x6eae2ec // likely
    (x1, 2) = 3
    x3 = [sp + 0x268]
    0x6ea9120 (0x10078168, x1, x2, x3)
    x7 = [sp + 0x4d8]
    x2 = x1
    x1 = x0
    x0 = [sp + 0x208]
    if (!x1) goto 0x6eae31c // unlikely
    [sp + 0xa8] = x1
    [sp + 0x1d0] = x2
    x0 = [sp + 0xf8] // section..text
    if (!x0) goto 0x6eae4d8 // unlikely
    x3 = [x0 + 8]
    x4 = 0x94d28a0
    ldar x5, [x4]
    x6 = [x5]
    w8 = [x0 + 0x10]
    goto 0x6eae620
    x1 = [sp + 0x4e0]
    x2 = [sp + 0x4e8]
    x3 = 0x8f8b978
    x4 = x0       // section..text
    x5 = [sp + 0xf8] // section..text
    x6 = [sp + 0x260] // elf_phdr
    x0 = x7
    0x6e9c430 (x0, x1, x2, 0x8f8b978)
    if (!x0) goto 0x6eae204 // likely
    [sp + 0x1c8] = x1
    [sp + 0xa0] = x0
    x0 = [sp + 0x208]
    x27 = (byte) [x0]
    0x6ea4b20 (0x10078168, x1, x2, 0x8f8b978)
    if (!x0) goto 0x6eae3e4 // unlikely
    [sp + 0x200] = x1
    [sp + 0xe0] = x0
    x0 = 0x10
    x1 = 0x81a54f8
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32500 (0x10, 0x81a54f8, 0x1, 0x8f8b978)
    x1 = [sp + 0xe0]
    if (!x1) goto 0x6eae388 // unlikely
    x6 = [x1 + 8]
    goto 0x6eae38c
    x6 = x1
    [x0] = x6
    x27 = 0xb48f000
    w6 = [x27 + 0x450]
    if (w6) goto 0x6eae3a4 // unlikely
    x6 = [sp + 0x200]
    goto 0x6eae3b0
    0x4da34f0 (0x10, 0x10078168, 0x1, 0x8f8b978)
    x6 = [sp + 0x200]
    [x25] = x6
    [x0 + 8] = x6
    x6 = [sp + 0x4d8]
    return x0;
}

