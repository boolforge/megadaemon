// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f86 (void) {
    x4 = 0
    goto 0x687c1c4
    x4 = 0
    if (w4 != 0) goto 0x687c38c // unlikely
    (x0, 2) = 3
    x3 = [sp + 0x1f8]
    x2 = [x3]
    0x6875180 (0x10102464c457f, x1, 0x10102464c457f, x3)
    [sp + 0x64] = (byte) w0
    x3 = [sp + 0x1e0]
    (x0, 2) = 3   // section..text
    x3 = [sp + 0x1f8]
    x2 = [x3]
    0x6875180 (0x100b70003, 0x4d0f000, 0x10102464c457f, x3)
    x3 = [sp + 0x1e0]
    (x4, 2) = 3
    (x1, 2) = 3   // section..text
    v = x2 - x5   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x687c210 // unlikely
    x9 = x3
    w1 = (byte) [sp + 0x64]
    goto 0x687c2a0
    [sp + 0x66] = (byte) w0
    x0 = x4
    0x4d1a430 (0x10102464c457f, 0x7f, x2, x3)
    if (w0 == 0) goto 0x687c294 // unlikely
    x9 = [sp + 0x1e0]
    (x10, 2) = 3
    v = x11 - 4   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x687c244 // likely
    w12 = [x10]
    x13 = 0x64616572
    v = w12 - w13 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x687c27c // unlikely
    v = x11 - 7   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x687c288 // likely
    w11 = [x10]
    x12 = 0x63657865
    v = w11 - w12 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x687c288 // likely
    w11 = (word) [x10 + 4]
    x12 = 0x7475  // 'ut' // "rt"
    v = w11 - w12 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x687c288 // likely
    w10 = (byte) [x10 + 6]
    v = w10 - 0x65 // 'e' // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x687c288 // likely
    x0 = 0
    w1 = (byte) [sp + 0x64]
    goto 0x687c2a0
    w0 = (byte) [sp + 0x66]
    x1 = 0
    goto 0x687c2a0
    x9 = [sp + 0x1e0]
    w0 = (byte) [sp + 0x66]
    w1 = (byte) [sp + 0x64]
    [sp + 0x64] = (byte) w1
    [sp + 0x66] = (byte) w0
    x26 = sp + 0x198
    stp 0, 0, [x26]
    [x26 + 0x10] = (0, 2)
    [x26 + 0x20] = 0
    x10 = 0x68839d0
    [sp + 0x198] = (x10, 2)
    [sp + 0x1a8] = (byte) w1
    [sp + 0x1a9] = (byte) w0
    x10 = [sp + 0x1f8]
    x11 = [sp + 0x98]
    [sp + 0x1b0] = (x10, 2)
    if (w1 == 0) goto 0x687c2e4 // unlikely
    x2 = 0
    goto 0x687c308
    (x0, 2) = 3
    x2 = [x10]
    0x6875460 (0x10102464c457f, x1, 0x10102464c457f, x3)
    w1 = (byte) [sp + 0x64]
    x9 = [sp + 0x1e0]
    x10 = [sp + 0x1f8]
    x26 = sp + 0x198
    x2 = x0
    w0 = (byte) [sp + 0x66]
    [sp + 0x63] = (byte) w2
    if (w0 == 0) goto 0x687c318 // unlikely
    x3 = 0
    goto 0x687c378
    (x0, 2) = 3   // section..text
    x2 = [x10]
    0x6875240 (0x100b70003, 0x4d0f000, 0x10102464c457f, x3)
    if (w0 == 0) goto 0x687c348 // unlikely
    w0 = (byte) [sp + 0x66]
    w1 = (byte) [sp + 0x64]
    w2 = (byte) [sp + 0x63]
    x9 = [sp + 0x1e0]
    x10 = [sp + 0x1f8]
    x26 = sp + 0x198
    x3 = 0
    goto 0x687c378
    x3 = [sp + 0x1e0]
    (x0, 2) = 3   // section..text
    x3 = [sp + 0x1f8]
    x2 = [x3]
    0x68758b0 (0x100b70003, 0x4d0f000, 0x10102464c457f, x3)
    w1 = (byte) [sp + 0x64]
    w2 = (byte) [sp + 0x63]
    x9 = [sp + 0x1e0]
    x10 = [sp + 0x1f8]
    x26 = sp + 0x198
    x3 = x0
    w0 = (byte) [sp + 0x66]
    [sp + 0x65] = (byte) w3
    if (w2 == 0) goto 0x687c618 // unlikely
    if (w0 == 0) goto 0x687c618 // unlikely
    (x4, 2) = 3   // elf_phdr
    goto 0x687d878
    [sp + 0x128] = (0, 2)
    (x0, 2) = 3
    0x4d98880 (-1, -1, 0x7f, 0x100b70003)
    x2 = 0x856f550
    [sp + 0x128] = (x2, 2)
    x0 = 0x44a6e3e
    x1 = 0x54     // 'T'
    x2 = sp + 0x128
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = x3       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4e34a60 (0x44a6e3e, 0x54, 0x10078128, 0x1)
    x2 = sp + 0xa0
    stp 0, 0, [x2]
    [x2 + 0x10] = (0, 2)
    [x2 + 0x20] = (0, 2)
    [x2 + 0x30] = 0
    (x4, 2) = 3
    x2 = 4
    [sp + 0xa0] = (x2, 2)
    [sp + 0xb0] = x1
    x3 = 0
    x6 = 0
    x7 = 0
    x8 = x0       // "executing downloaded file %q is not allowed; downloaded content"
    x0 = x2
    x9 = x1
    x1 = x8       // "executing downloaded file %q is not allowed; downloaded content"
    x2 = x9
    (x29, 2) = 3
    sp += 0x1f0
    x8 = sp + 0xa0
    stp 0, 0, [x8]
    [x8 + 0x10] = (0, 2)
    [x8 + 0x20] = (0, 2)
    [x8 + 0x30] = 0
    (x4, 2) = 3
    x0 = 3
    x1 = 0x444c001
    [sp + 0xa0] = (x0, 2)
    x2 = 0x2e     // '.'
    [sp + 0xb0] = x2
    x3 = 0
    x6 = 0
    x7 = 0
    (x29, 2) = 3
    sp += 0x1f0
    (x8, 2) = 3   // elf_shdr // elf_phdr
    v = x9 - 7    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x687c4dc // likely
    w9 = [x8]     // elf_phdr
    x10 = 0x72636573
    v = w9 - w10  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x687c4dc // likely
    w9 = (word) [x8 + 4]
    x10 = 0x7465  // 'et' // "etenv"
    v = w9 - w10  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x687c4dc // likely
    w8 = (byte) [x8 + 6]
    v = w8 - 0x73 // 's' // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x687c4dc // likely
    x8 = sp + 0xa0
    stp 0, 0, [x8]
    [x8 + 0x10] = (0, 2)
    [x8 + 0x20] = (0, 2)
    [x8 + 0x30] = 0
    (x4, 2) = 3
    x0 = 3
    x1 = 0x4434462
    [sp + 0xa0] = (x0, 2)
    x2 = 0x29     // ')'
    [sp + 0xb0] = x2
    x3 = 0
    x6 = 0
    x7 = 0
    (x29, 2) = 3
    sp += 0x1f0
    x8 = sp + 0xa0
    stp 0, 0, [x8]
    [x8 + 0x10] = (0, 2)
    [x8 + 0x20] = (0, 2)
    [x8 + 0x30] = 0
    (x4, 2) = 3
    x0 = 4
    x1 = 0x44787d6
    [sp + 0xa0] = (x0, 2)
    x2 = 0x3b     // ';'
    [sp + 0xb0] = x2
    x3 = 0
    x6 = 0
    x7 = 0
    (x29, 2) = 3
    sp += 0x1f0
    [sp + 0x90] = x1
    [sp + 0x68] = x0
    x0 = 0x9593ae0
    x1 = 2
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4f73800 (0x9593ae0, 0x2, 0x1, x3)
    if (w0 == 0) goto 0x687c5cc // likely
    x0 = 0x10
    x1 = 0x81a54f8
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32500 (0x10, 0x81a54f8, 0x1, x3)
    x1 = [sp + 0x68]
    if (!x1) goto 0x687c56c // unlikely
    x11 = [x1 + 8]
    goto 0x687c570
    x11 = x1
    [x0] = x11
    x27 = 0xb48f000
    w11 = [x27 + 0x450]
    if (w11) goto 0x687c588 // unlikely
    x11 = [sp + 0x90] // (pstr 0x044787d6) "the URI contains embedded data; confirm this is intentionalsett"
    goto 0x687c594
    0x4da34f0 (0x10, 0x4, 0x1, x3)
    x11 = [sp + 0x90] // (pstr 0x044787d6) "the URI contains embedded data; confirm this is intentionalsett"
    [x25] = x11
    [x0 + 8] = x11
    x1 = [sp + 0x208]
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x3 = 2
    x4 = 0
    x5 = 0
    x6 = 0x445f33e
    x7 = 0x33     // '3'
    x8 = x0
    x9 = x2       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x10 = x2      // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x0 = [sp + 0x200]
    0x4f6f980 (x0, x1, 0x1, 0x2)
    x8 = sp + 0xa0
    stp 0, 0, [x8]
    [x8 + 0x10] = (0, 2)
    return x0;
}

