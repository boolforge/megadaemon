// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f97 (void) {
    0x4da3500 (x0, x1, x2, x3)
    stp x0, x2, [x25]
    [x3 + 0x18] = x0
    x2 = [sp + 0x4d8]
    (x0, 2) = 3   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    0x4d98880 (0x90, 0x458, x2, x3)
    x2 = 0x856f550
    x6 = [sp + 0x240]
    [x6 + 0x20] = x2
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x6eaefc8 // likely
    x9 = [x6 + 0x28] // elf_shdr
    0x4da3500 (0x90, 0x458, x2, x3)
    stp x0, x9, [x25]
    [x6 + 0x28] = x0
    x9 = [sp + 0x4d8]
    x27 = x9 + 0x2b8
    (x2, 2) = 3   // segment.LOAD5
    x0 = [sp + 0x260]
    x1 = [sp + 0x1f8]
    x4 = 0x4468c97
    x5 = 0x36     // '6'
    x7 = 3
    x8 = x7
    0x6eac5a0 (x0, x1, 0xd8d6298, 0xd8d6298)
    (x5, 2) = 3
    w0 = (byte) [sp + 0x6b]
    x1 = [sp + 0x1e8]
    goto 0x6eaed9c // f96+0x494
    if (x3) goto 0x6eaf014 // likely
    x0 = x2
    goto 0x6eadda4
    x4 = [sp + 0x88]
    x1 = x5
    x0 = x2
    x5 = x3       // segment.LOAD5
    x3 = [sp + 0x150]
    x2 = 2
    goto 0x6eae1b8 // f95+0xb0
    x7 = ((x6 & x5) << 4) + 8
    x8 = [x4 + x7]
    x7 = x4 + x7
    v = x8 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x6eaf060 // likely
    x6 += 1       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (x8) goto 0x6eaf030 // unlikely
    x0 = x3
    0x4d2e4c0 (x0, x1, 0x2, x3)
    goto 0x6eaeef0 // f96+0x5e8
    x0 = [x7 + 8]
    goto 0x6eaeef0 // f96+0x5e8
    x11 = ((x6 & x10) << 4) + 8
    x12 = [x9 + x11]
    x11 = x9 + x11
    v = x12 - x1  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x6eaf09c // likely
    x6 += 1
    if (x12) goto 0x6eaf068 // unlikely
    x0 = x8
    0x4d2e4c0 (x0, x1, 0x2, x3)
    x6 = x0
    goto 0x6eae884
    x6 = [x11 + 8]
    goto 0x6eae884
    x5 += 1
    v = x5 - x4   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v >= 0) goto 0x6eaf0c4 // likely
    ldr x9, [x3, x5, lsl 3]
    w10 = (byte) [x9 + 0x98]
    if (w10 != 0) goto 0x6eaf0c4 // unlikely
    w9 = (byte) [x9 + 0xc0]
    if (w9 == 0) goto 0x6eaf0a4 // likely
    x9 = (lt)? 1 : 0
    x0 = [sp + 0x4d8]
    (x1, 2) = 3
    x7 = [sp + 0x260]
    x8 = [sp + 0x118]
    x10 = [sp + 0x268]
    x5 = x9
    w9 = (byte) [sp + 0x6e]
    goto 0x6eadca0
    [sp + 8] = (x0, 2)
    [sp + 0x18] = (x2, 2)
    [sp + 0x28] = (x4, 2)
    x3 = x30
    return x0;
}

