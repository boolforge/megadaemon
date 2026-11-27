// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f96 (void) {
    x27 = 0x957e000
    x6 = [x27 + 0x930]
    x7 = [sp + 0x250]
    v = x6 - x7   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x6eae924 // unlikely
    x3 = 0
    goto 0x6eae934
    x27 = 0x957e000
    x8 = [x27 + 0x938]
    v = x8 - x7   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x3 = (ne)? 1 : 0
    x8 = [sp + 0x268]
    v = x6 - x8   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x6eae94c // unlikely
    x0 = 0
    x1 = 0
    goto 0x6eae9cc
    [sp + 0x55] = (byte) w3
    x27 = 0x957e000
    x4 = [x27 + 0x938]
    v = x4 - x8   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x6eae98c // unlikely
    x0 = 0x81ab6d8
    0x4d31c10 (0x81ab6d8, x1, x2, x3)
    x1 = [sp + 0x128]
    rev w2, w1
    [x0] = w2
    w3 = (byte) [sp + 0x55]
    x1 = x0
    x0 = 4
    goto 0x6eae9cc
    x0 = 0x10
    x1 = 0x81a14d8
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33d00 (0x10, 0x81a14d8, 0x1, x3)
    x3 = [sp + 0x120]
    rev x4, x3
    x5 = [sp + 0x128]
    rev x6, x5
    [x0] = x4
    [x0 + 8] = x6
    w3 = (byte) [sp + 0x55]
    x1 = x0
    x0 = 0x10
    x7 = [[sp + 0x260] + 0x18]
    x8 = [sp + 0x4d8]
    (x8, 2) = 3   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x4 = x1
    x5 = x0
    x6 = x5
    x0 = [sp + 0x158]
    x1 = x8
    callreg x7    // section..text(0x0, 0x90, 0x458, 0x0)
    if (!x3) goto 0x6eaec80 // likely
    x0 = 0x30     // '0'
    x1 = 0x81aa5b0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32f40 (0x30, 0x81aa5b0, 0x1, x3)
    [sp + 0x240] = x0
    x0 = [sp + 0x150]
    x1 = [sp + 0x88]
    0x4d98880 (x0, x1, 0x1, x3)
    x3 = 0x856f550
    x4 = [sp + 0x240]
    [x4] = x3
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (!w3) goto 0x6eaea44 // likely
    x2 = [x4 + 8]
    0x4da3500 (x0, x1, 0x30003500400010, x3)
    stp x0, x2, [x25]
    [x4 + 8] = x0
    (x2, 2) = 3
    x27 = sp + 0x378
    stp x2, x3, [x27]
    x2 = [sp + 0x268]
    [sp + 0x388] = x2
    x0 = 0x8eba5d0
    x1 = sp + 0x378
    0x4d2e1f0 (0x8eba5d0, 0x10078378, x2, x3)
    x2 = 0x8eba5d0
    x3 = [sp + 0x240]
    [x3 + 0x10] = x2 // elf_phdr
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x6eaea94 // likely
    x2 = [x3 + 0x18] // elf_phdr
    0x4da3500 (0x8eba5d0, 0x10078378, 0x40, 0x30)
    stp x0, x2, [x25]
    [x3 + 0x18] = x0
    x2 = [sp + 0x4d8]
    (x0, 2) = 3   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    0x4d98880 (0x90, 0x458, x2, 0x30)
    x2 = 0x856f550
    x3 = [sp + 0x240]
    [x3 + 0x20] = x2
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x6eaeacc // likely
    x6 = [x3 + 0x28] // elf_phdr
    0x4da3500 (0x90, 0x458, x2, 0x30)
    stp x0, x6, [x25]
    [x3 + 0x28] = x0
    x6 = [sp + 0x4d8]
    x27 = x6 + 0x2b8
    (x6, 2) = 3   // segment.LOAD5
    x6 = [x6 + 0x60]
    x1 = 0x44620b1
    x2 = 0x34     // '4'
    x4 = 3
    x5 = x4
    callreg x6    // 0x400000003(0xd8d6298, 0x44620b1, 0x34, 0x30)
    w0 = (byte) [sp + 0x6c]
    goto 0x6eadda4
    x0 = 0x10
    x1 = 0x81a54f8
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32500 (0x10, 0x81a54f8, 0x1, 0x30)
    [sp + 0x220] = x0
    x3 = [sp + 0x4d8]
    (x0, 2) = 3   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    0x4d98880 (0x90, 0x458, 0x1, x3)
    x3 = 0x856f550
    x4 = [sp + 0x220]
    [x4] = x3
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (!w3) goto 0x6eaeb4c // likely
    x6 = [x4 + 8] // section..text
    0x4da3500 (0x90, 0x458, 0x1, x3)
    stp x0, x6, [x25]
    [x4 + 8] = x0
    x6 = [sp + 0x4d8]
    x27 = x6 + 0x2b8
    (x6, 2) = 3   // segment.LOAD5
    x6 = [x6 + 0x20]
    x1 = 0x437454b
    x2 = 0x14
    x3 = x4
    x4 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x5 = x4       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    callreg x6    // 0x8100000002(0xd8d6298, 0x437454b, 0x14, 0x10)
    x27 = 0x957e000
    x6 = [x27 + 0x930]
    x7 = [sp + 0x250]
    v = x6 - x7   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x6eaeb98 // unlikely
    x3 = 0
    goto 0x6eaeba8
    x27 = 0x957e000
    x8 = [x27 + 0x938]
    v = x8 - x7   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x3 = (ne)? 1 : 0
    x8 = [sp + 0x268]
    v = x6 - x8   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x6eaebc0 // unlikely
    x0 = 0
    x1 = 0
    goto 0x6eaec40
    [sp + 0x54] = (byte) w3
    x27 = 0x957e000
    x4 = [x27 + 0x938]
    v = x4 - x8   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x6eaec00 // unlikely
    x0 = 0x81ab6d8
    0x4d31c10 (0x81ab6d8, x1, 0x14, x3)
    x1 = [sp + 0x128]
    rev w2, w1
    [x0] = w2
    w3 = (byte) [sp + 0x54]
    x1 = x0
    x0 = 4
    goto 0x6eaec40
    x0 = 0x10
    x1 = 0x81a14d8
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33d00 (0x10, 0x81a14d8, 0x1, x3)
    x3 = [sp + 0x120]
    rev x4, x3
    x5 = [sp + 0x128]
    rev x6, x5
    [x0] = x4
    [x0 + 8] = x6
    w3 = (byte) [sp + 0x54]
    x1 = x0
    x0 = 0x10
    x7 = [sp + 0x4d8]
    x27 = x7 + 0x2d8
    (x8, 2) = 3   // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x8 = [x8 + 0x20] // (pstr 0x042fcc9a) "grx_enc_errorgtx_disparitygtx_enc_errorDiagnosticOffTypeInvaria"
    (x7, 2) = 3   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x4 = x1
    x5 = x0
    x6 = x5
    x0 = x9
    x1 = x7
    callreg x8    // "grx_enc_errorgtx_disparitygtx_enc_errorDiagnosticOffTypeInvaria" // 0x42fcc9a(0x600000001, 0x90, 0x458, 0x0)
    if (x2) goto 0x6eaec94 // likely
    x27 = sp + 0x350
    stp x0, x1, [x27]
    x0 = sp + 0x350
    x1 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    w2 = (byte) [sp + 0x6c]
    x3 = 0
    x4 = 0
    x5 = 0
    goto 0x6eaedc4
    x0 = 0x30     // '0'
    x1 = 0x81aa5b0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32f40 (0x30, 0x81aa5b0, 0x1, x3)
    [sp + 0x240] = x0
    x0 = [sp + 0x150]
    x1 = [sp + 0x88]
    0x4d98880 (x0, x1, 0x1, x3)
    x3 = 0x856f550
    x4 = [sp + 0x240]
    [x4] = x3
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (!w3) goto 0x6eaece0 // likely
    x2 = [x4 + 8]
    0x4da3500 (x0, x1, 0x30003500400010, x3)
    stp x0, x2, [x25]
    [x4 + 8] = x0
    (x2, 2) = 3
    x27 = sp + 0x360
    stp x2, x3, [x27]
    x2 = [sp + 0x268]
    [sp + 0x370] = x2
    x0 = 0x8eba5d0
    x1 = sp + 0x360
    0x4d2e1f0 (0x8eba5d0, 0x10078360, x2, x3)
    x2 = 0x8eba5d0
    x3 = [sp + 0x240]
    [x3 + 0x10] = x2 // elf_phdr
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x6eaed30 // likely
    x2 = [x3 + 0x18] // elf_phdr
    0x4da3500 (0x8eba5d0, 0x10078360, 0x40, 0x30)
    stp x0, x2, [x25]
    [x3 + 0x18] = x0
    x2 = [sp + 0x4d8]
    (x0, 2) = 3   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    0x4d98880 (0x90, 0x458, x2, 0x30)
    x2 = 0x856f550
    x3 = [sp + 0x240]
    [x3 + 0x20] = x2
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x6eaed68 // likely
    x6 = [x3 + 0x28] // elf_phdr
    0x4da3500 (0x90, 0x458, x2, 0x30)
    stp x0, x6, [x25]
    [x3 + 0x28] = x0
    x6 = [sp + 0x4d8]
    x27 = x6 + 0x2b8
    (x6, 2) = 3   // segment.LOAD5
    x6 = [x6 + 0x60]
    x1 = 0x44536f5
    x2 = 0x30     // '0'
    x4 = 3
    x5 = x4
    callreg x6    // 0x400000003(0xd8d6298, 0x44536f5, 0x30, 0x30)
    w0 = (byte) [sp + 0x6c]
    goto 0x6eadda4
    x6 = [sp + 0x238] + 0x10
    x7 = [sp + 0x100] - 1
    x3 = x5
    x4 = x2
    x5 = x1       // "Failed to get TCP connections by ufrag: %s %s %sfailed to get s"
    x1 = x7
    x2 = x0
    x0 = x6
    v = x1 - 0    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v <= 0) goto 0x6eaf008 // likely
    [sp + 0xc0] = (x3, 2)
    [sp + 0x1e8] = x5
    [sp + 0x100] = x1
    [sp + 0x6b] = (byte) w2
    [sp + 0x238] = x0
    (x1, 2) = 3   // section..text
    [sp + 0xd8] = x1
    [sp + 0x1f8] = x0
    x1 = [x1 + 0x20]
    callreg x1    // 0xffffffffffffffff(0x4d0f000, -1, 0x0, 0x3)
    x2 = 0x8f54218
    v = x0 - x2   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x6eaee08 // unlikely
    x1 = 0
    if (v) goto 0x6eaeecc // likely
    x5 = [sp + 0xc0] + 1
    x6 = [x1 + 0x18] // section..text
    x2 = [sp + 0xc8]
    v = x2 - x5   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x6eaee30 // unlikely
    x1 = [sp + 0x1e8] // (pstr 0x044536f5) "Failed to get TCP connections by ufrag: %s %s %sfailed to get s"
    w0 = (byte) [sp + 0x6b]
    goto 0x6eaee84
    [sp + 0xf0] = x6
    v = x5 - 1    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v > 0) goto 0x6eaee5c // likely
    w0 = (byte) [sp + 0x6b]
    if (w0 != 0) goto 0x6eaee5c // unlikely
    x1 = sp + 0x278
    stp 0, 0, [x1]
    [x1 + 0x10] = 0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x0 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    goto 0x6eaee84
    x0 = [sp + 0x1e8] // (pstr 0x044536f5) "Failed to get TCP connections by ufrag: %s %s %sfailed to get s"
    x1 = x5
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x88ba188
    0x4d9e050 (0x44536f5, 0x4, 0x1, 0x1)
    x6 = [sp + 0xf0] // section..text
    x5 = x1
    x1 = x0       // "Failed to get TCP connections by ufrag: %s %s %sfailed to get s"
    w0 = (byte) [sp + 0x6b]
    add x3, x5, x5, lsl 1
    x3 = (x3 << 3) - 0x18
    x4 = [sp + 0xd8]
    [x1 + x3] = x4
    x3 = x1 + x3  // "lexive address %s %s: %vUnable to handle URL in gatherCandidate"
    [x3 + 0x10] = x6 // "s %s: %vUnable to handle URL in gatherCandidatesRelay %vfailed "
    x27 = 0xb48f000
    w4 = [x27 + 0x450]
    if (w4) goto 0x6eaeeb4 // unlikely
    x6 = [sp + 0x1f8] // section..text
    goto 0x6eaeec4
    x4 = [x3 + 8] // "ddress %s %s: %vUnable to handle URL in gatherCandidatesRelay %"
    0x4da3500 (x0, 0x44536f5, 0x1, 0x445373d)
    x6 = [sp + 0x1f8] // section..text
    stp x6, x4, [x25]
    [x3 + 8] = x6 // "ddress %s %s: %vUnable to handle URL in gatherCandidatesRelay %"
    goto 0x6eaed9c
    x0 = [sp + 0xd8]
    if (!x0) goto 0x6eaeef0 // unlikely
    x1 = [x0 + 8]
    x3 = 0x94d2860
    ldar x4, [x3]
    x5 = [x4]
    w6 = [x0 + 0x10]
    goto 0x6eaf030
    [sp + 0x260] = x0
    x0 = 0x30     // '0'
    x1 = 0x81aa5b0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32f40 (0x30, 0x81aa5b0, 0x1, 0x94d2860)
    [sp + 0x240] = x0
    x0 = [sp + 0x150]
    x1 = [sp + 0x88]
    0x4d98880 (x0, x1, 0x1, 0x94d2860)
    return x0;
}

