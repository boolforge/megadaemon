// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f87 (void) {
    x2 += 0x6e2   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x3 = 3
    0x4d18950 (x0, x1, 0x6e2, 0x3)
    (x4, 2) = 3
    x6 = 0x856f550
    v = x4 - x6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x688e1d0 // likely
    (x0, 2) = 3
    goto 0x688e1d8
    x1 = 0
    x0 = 0
    [sp + 0x98] = x1
    [sp + 0x1f8] = x0
    x3 = [sp + 0x5c0]
    v = x3 - 0xb  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x688e22c // likely
    x2 = [sp + 0x5b8]
    x5 = [x2]
    x6 = 0x6d6d6f635f6e7572
    v = x5 - x6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x688e230 // likely
    w5 = (word) [x2 + 8]
    x6 = 0x6e61   // 'an' // "_ZNSt9exceptionD2Ev"
    v = w5 - w6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x688e230 // likely
    w5 = (byte) [x2 + 0xa]
    v = w5 - 0x64 // 'd' // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x688e2a8 // unlikely
    goto 0x688e230
    x2 = [sp + 0x5b8]
    v = x3 - 0x14 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x688e28c // likely
    (x5, 2) = 3
    x7 = [x2 + 0x10]
    x8 = 0x5f65747563657865
    v = x5 - x8   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x5 = (eq)? 1 : 0
    x8 = 0x6d6f635f68736162
    v = x6 - x8   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x6 = (eq)? 1 : 0
    x5 &= x6
    x6 = 0x646e616d
    v = w7 - w6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x6 = (eq)? 1 : 0
    x5 &= x6
    if (w5 != 0) goto 0x688e2a8 // unlikely
    v = x3 - 4    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x688e2dc // likely
    w5 = [x2]
    x6 = 0x68736162
    v = w5 - w6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x688e2d4 // likely
    x0 = [sp + 0x368]
    x1 = [sp + 0x118]
    0x4e00e20 (x0, x1, x2, x3)
    v = x1 - 0    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = (eq)? 1 : 0 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x0 = [sp + 0x1f8]
    x1 = [sp + 0x98]
    x3 = [sp + 0x5c0]
    x4 = x2       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = [sp + 0x5b8]
    goto 0x688e2e0
    x4 = 0
    goto 0x688e2e0
    x4 = 0
    if (w4 != 0) goto 0x688e5f8 // unlikely
    x4 = [sp + 0x118]
    if (x4) goto 0x688e2f4 // unlikely
    x5 = 0
    goto 0x688e324
    x5 = [[[sp + 0x598] + 0x88]]
    x0 = [sp + 0x368]
    x1 = x4
    x4 = x5
    0x6892d70 (x0, x1, x2, x3)
    x1 = [sp + 0x98]
    x2 = [sp + 0x5b8]
    x3 = [sp + 0x5c0]
    x5 = x0
    x0 = [sp + 0x1f8]
    if (w5 == 0) goto 0x688e330 // likely
    x5 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    goto 0x688e344
    if (x1) goto 0x688e33c // unlikely
    x5 = 0
    goto 0x688e344
    0x6892b70 (x0, x1, x2, x3)
    x5 = x0
    if (w5 != 0) goto 0x688e3f4 // unlikely
    x1 = [sp + 0x598]
    x5 = x1 + 0xc0
    x27 = 0xb48b000
    w6 = (byte) [x27 + 0xd50]
    if (w6 == 0) goto 0x688e378 // likely
    x6 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    w27 = 0
    casal w27, w6, [x5]
    v = w27 - 0   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x7 = (eq)? 1 : 0 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    goto 0x688e394
    x6 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    w27 = [x5]
    v = w27 - 0   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x688e390 // unlikely
    [x5] = w6
    if (x27) goto 0x688e37c // unlikely
    x7 = (eq)? 1 : 0 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [sp + 0x3d0] = x5
    x0 = x1 + 0xc0
    [sp + 0x2c8] = x0
    if (w7 != 0) goto 0x688e3ac // likely
    0x4dabb30 (0xc0, x1, x2, x3)
    x1 = [sp + 0x598]
    x0 = x1 + 0xc8
    [sp + 0x3c8] = x0
    0x6895a70 (0xc8, x1, x2, x3)
    [sp + 0x208] = x0
    x1 = [sp + 0x598]
    x2 = [x1 + 0xf8] // segment.GNU_RELRO
    [sp + 0xf8] = x2
    x0 = 0x89fbe38
    x1 = x2       // segment.GNU_RELRO
    0x4d9df80 (0x89fbe38, 0x799b960, 0x799b960, x3)
    [sp + 0x3d8] = x0
    x1 = [sp + 0x598]
    (x2, 2) = 3   // segment.GNU_RELRO
    [sp + 0x138] = x3
    x4 = 0
    x5 = [sp + 0xf8] // segment.GNU_RELRO
    goto 0x688e8c4
    x0 = 0x9593ae0
    x1 = 2
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4f73800 (0x9593ae0, 0x2, 0x1, 0x799b960)
    [sp + 0x8a] = (byte) w0
    x0 = 0x40     // elf_phdr
    x1 = 0x81ab210
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33310 (0x40, 0x81ab210, 0x1, 0x799b960)
    [sp + 0x3c0] = x0
    x0 = [sp + 0x5b0]
    0x4d98810 (x0, 0x81ab210, 0x1, 0x799b960)
    x3 = 0x856f790
    x4 = [sp + 0x3c0] // elf_phdr
    [x4] = x3     // elf_phdr
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (!w3) goto 0x688e458 // likely
    x2 = [x4 + 8] // elf_phdr
    0x4da3500 (x0, 0x81ab210, 0x40, x3)
    stp x0, x2, [x25]
    [x4 + 8] = x0
    x0 = [sp + 0x5b8]
    x1 = [sp + 0x5c0]
    0x4d98880 (x0, x1, 0x40, x3)
    x2 = 0x856f550
    x3 = [sp + 0x3c0] // elf_phdr
    [x3 + 0x10] = x2
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x688e490 // likely
    x2 = [x3 + 0x18] // elf_phdr
    0x4da3500 (x0, x1, 0x40, 0x40)
    stp x0, x2, [x25]
    [x3 + 0x18] = x0
    x0 = [sp + 0x368]
    x1 = [sp + 0x118]
    0x4d98880 (x0, x1, 0x40, 0x40)
    x2 = 0x856f550
    x3 = [sp + 0x3c0] // elf_phdr
    [x3 + 0x20] = x2
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x688e4c8 // likely
    x2 = [x3 + 0x28]
    0x4da3500 (x0, x1, 0x380, 0x40)
    stp x0, x2, [x25]
    [x3 + 0x28] = x0
    x0 = [sp + 0x1f8]
    x1 = [sp + 0x98]
    0x4d98880 (x0, x1, 0x380, 0x40)
    x2 = 0x856f550
    x8 = [sp + 0x3c0] // elf_phdr
    [x8 + 0x30] = x2
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x688e500 // likely
    x11 = [x8 + 0x38]
    0x4da3500 (x0, x1, x2, 0x40)
    stp x0, x11, [x25]
    [x8 + 0x38] = x0
    w0 = (byte) [sp + 0x8a]
    if (w0 == 0) goto 0x688e53c // likely
    x0 = [sp + 0x5a0]
    x1 = [sp + 0x5a8]
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x3 = 0
    x4 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x5 = 0
    x6 = 0x449f6d2
    x7 = 0x4d     // 'M'
    x9 = 4
    x10 = x9
    0x4f6f980 (x0, x1, 0x1, x3)
    x0 = 0x30     // '0'
    x1 = 0x89fc030
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32f40 (0x30, 0x89fc030, 0x1, x3)
    [sp + 0x1e0] = x0
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x4428d79
    stp x3, x4, [x0]
    x3 = 0x27     // '\''
    [x0 + 0x10] = x3 // elf_phdr
    (x1, 2) = 3
    x2 = [sp + 0x3f0]
    0x4dc58a0 (x0, x1, x2, 0x27)
    x3 = 0x431bde82d7b634db
    smulh x3, x0, x3
    x3 = x3 >> 0x12
    sub x7, x3, x0, asr 63
    x0 = [sp + 0x598]
    x1 = [sp + 0x5a0]
    x2 = [sp + 0x5a8]
    x3 = [sp + 0x5b0]
    x4 = [sp + 0x5b8]
    x5 = [sp + 0x5c0]
    x6 = [sp + 0x1e0]
    x8 = 0
    x9 = 0
    x10 = 0
    x11 = 0
    x12 = 0
    x13 = 0
    x14 = 0
    x15 = 0
    0x6892250 (x0, x1, x2, x3)
    x3 = [sp + 0x1e0]
    [sp + 0x1c8] = x3
    [sp + 0x1b0] = (0, 2)
    x0 = [sp + 0x1c8]
    x1 = 0
    x2 = 0
    (x29, 2) = 3
    sp += 0x590
    x0 = 0x9593ae0
    x1 = 2
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4f73800 (0x9593ae0, 0x2, 0x1, 0x30)
    [sp + 0x8a] = (byte) w0
    x0 = 0x20
    x1 = 0x81a8cf0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32c00 (0x20, 0x81a8cf0, 0x1, 0x30)
    [sp + 0x3b8] = x0
    x0 = [sp + 0x5b0]
    0x4d98810 (x0, 0x81a8cf0, 0x1, 0x30)
    x3 = 0x856f790
    x4 = [sp + 0x3b8] // (pstr 0x00000020) "@"
    [x4] = x3
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (!w3) goto 0x688e65c // likely
    x2 = [x4 + 8] // elf_shdr
    0x4da3500 (x0, 0x81a8cf0, 0xace39d8, x3)
    stp x0, x2, [x25]
    [x4 + 8] = x0
    x0 = [sp + 0x5b8]
    x1 = [sp + 0x5c0]
    0x4d98880 (x0, x1, 0xace39d8, x3)
    x2 = 0x856f550
    x8 = [sp + 0x3b8] // (pstr 0x00000020) "@"
    [x8 + 0x10] = x2
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x688e694 // likely
    x11 = [x8 + 0x18]
    0x4da3500 (x0, x1, x2, x3)
    stp x0, x11, [x25]
    [x8 + 0x18] = x0
    w0 = (byte) [sp + 0x8a]
    if (w0 == 0) goto 0x688e6d0 // likely
    x0 = [sp + 0x5a0]
    x1 = [sp + 0x5a8]
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x3 = 0
    x4 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x5 = 0
    x6 = 0x44889b4
    x7 = 0x41     // 'A'
    x9 = 2
    x10 = x9
    0x4f6f980 (x0, x1, 0x1, x3)
    x0 = 0x30     // '0'
    x1 = 0x89fc030
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32f40 (0x30, 0x89fc030, 0x1, x3)
    [sp + 0x1e8] = x0
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x43d1712
    stp x3, x4, [x0]
    x3 = 0x1b
    [x0 + 0x10] = x3 // elf_phdr
    (x1, 2) = 3
    x2 = [sp + 0x3f0]
    0x4dc58a0 (x0, x1, x2, 0x1b)
    x3 = 0x431bde82d7b634db
    smulh x3, x0, x3
    x3 = x3 >> 0x12
    sub x7, x3, x0, asr 63
    x0 = [sp + 0x598]
    x1 = [sp + 0x5a0]
    x2 = [sp + 0x5a8]
    x3 = [sp + 0x5b0]
    x4 = [sp + 0x5b8]
    x5 = [sp + 0x5c0]
    x6 = [sp + 0x1e8]
    x8 = 0
    x9 = 0
    x10 = 0
    x11 = 0
    x12 = 0
    x13 = 0
    x14 = 0
    x15 = 0
    0x6892250 (x0, x1, x2, x3)
    x3 = [sp + 0x1e8]
    [sp + 0x1c8] = x3
    [sp + 0x1b0] = (0, 2)
    x0 = [sp + 0x1c8]
    x1 = 0
    x2 = 0
    (x29, 2) = 3
    sp += 0x590
    x0 = 0x9593ae0
    x1 = 2
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4f73800 (0x9593ae0, 0x2, 0x1, 0x30)
    [sp + 0x8a] = (byte) w0
    x0 = 0x20
    x1 = 0x81a8cf0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32c00 (0x20, 0x81a8cf0, 0x1, 0x30)
    [sp + 0x3b8] = x0
    x0 = [sp + 0x5b0]
    0x4d98810 (x0, 0x81a8cf0, 0x1, 0x30)
    x3 = 0x856f790
    x4 = [sp + 0x3b8] // (pstr 0x00000020) "@"
    [x4] = x3
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (!w3) goto 0x688e7f0 // likely
    x2 = [x4 + 8] // elf_shdr
    0x4da3500 (x0, 0x81a8cf0, 0xace39d8, x3)
    stp x0, x2, [x25]
    [x4 + 8] = x0
    x0 = [sp + 0x5b8]
    x1 = [sp + 0x5c0]
    0x4d98880 (x0, x1, 0xace39d8, x3)
    x2 = 0x856f550
    x8 = [sp + 0x3b8] // (pstr 0x00000020) "@"
    [x8 + 0x10] = x2
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x688e828 // likely
    x11 = [x8 + 0x18]
    0x4da3500 (x0, x1, x2, x3)
    stp x0, x11, [x25]
    [x8 + 0x18] = x0
    w0 = (byte) [sp + 0x8a]
    if (w0 == 0) goto 0x688e864 // likely
    x0 = [sp + 0x5a0]
    x1 = [sp + 0x5a8]
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x3 = 0
    x4 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x5 = 0
    x6 = 0x448fc79
    x7 = 0x44     // 'D'
    x9 = 2
    x10 = x9
    0x4f6f980 (x0, x1, 0x1, x3)
    x0 = 0x30     // '0'
    x1 = 0x89fc030
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32f40 (0x30, 0x89fc030, 0x1, x3)
    x3 = 0x43b0ed2
    x4 = 0x18
    [x0 + 8] = (x3, 2)
    [sp + 0x1c8] = x0
    [sp + 0x1b0] = (0, 2)
    x0 = [sp + 0x1c8]
    x1 = 0
    x2 = 0
    (x29, 2) = 3
    sp += 0x590
    x27 = sp + 0x498
    (q0, 2) = 3
    x6 = [sp + 0x4b8]
    stp q0, q1, [x7]
    [x7 + 0x20] = x6
    x2 += 0x28
    x4 += 1
    v = x4 - x3   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v >= 0) goto 0x688e96c // unlikely
    (q0, 2) = 3   // elf_shdr
    x6 = [x2 + 0x20] // elf_phdr
    [sp + 0x2e0] = (q0, 2)
    [sp + 0x300] = x6
    x6 = sp + 0x498
    stp 0, 0, [x6]
    [x6 + 0x10] = (0, 2)
    [x6 + 0x20] = 0
    x27 = sp + 0x2e0
    (x7, 2) = 3   // elf_shdr
    x27 = sp + 0x498
    stp x7, x8, [x27]
    x7 = [sp + 0x2f0]
    [sp + 0x4a8] = x7
    x27 = sp + 0x2f8
    (x7, 2) = 3   // elf_phdr
    x27 = sp + 0x4b0
    stp x7, x8, [x27]
    v = x4 - x5   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x688ff48 // likely
    add x7, x4, x4, lsl 2
    add x7, x0, x7, lsl 3 // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x27 = 0xb48f000
    w8 = [x27 + 0x450]
    if (!w8) goto 0x688e8a8 // likely
    [sp + 0xd8] = x4
    [sp + 0x3b0] = x2
    [sp + 0x3a8] = x7
    x0 = 0x89fbe38
    x1 = x7       // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x2 = x6
    0x4d35930 (0x89fbe38, 0x418, 0x10079548, 0x43b0ed2)
    x0 = [sp + 0x3d8]
    x2 = [sp + 0x3b0]
    return x0;
}

