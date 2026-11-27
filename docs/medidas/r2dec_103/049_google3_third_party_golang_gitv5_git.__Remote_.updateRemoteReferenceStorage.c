// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f49 (void) {
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32830 (x0, x1, 0x1, x3)
    x3 = 0x8f08ae8
    [x0] = x3
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (w3) goto 0x593d060 // unlikely
    x1 = [sp + 0x58]
    goto 0x593d06c
    0x4da34f0 (x0, x1, 0x1, x3)
    x1 = [sp + 0x58]
    [x25] = x1
    [sp + 0x60] = x0
    [x0 + 8] = x1
    x1 = 0x20000  // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    [x0 + 0x10] = x1
    x0 = 0x8ccd890
    0x4d31c10 (0x8ccd890, 0x20000, 0x1, x3)
    [sp + 0x48] = x0
    x0 = 0x856f5d0
    x1 = 0x1000   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x2 = x1       // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    0x4d9df80 (0x856f5d0, 0x1000, 0x1000, x3)
    x2 = sp + 0x70
    stp 0, 0, [x2]
    [x2 + 0x10] = (0, 2)
    [x2 + 0x20] = (0, 2)
    [x2 + 0x30] = (0, 2)
    [x2 + 0x40] = (0, 2)
    [x2 + 0x50] = 0
    x1 = 0x1000   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    [sp + 0x70] = (x0, 2)
    x3 = 0x8f08488
    [sp + 0x80] = (x1, 2)
    x1 = [sp + 0x60]
    [sp + 0x90] = x1
    x1 = -1
    [sp + 0xb8] = (x1, 2)
    x27 = 0xb48f000
    w1 = [x27 + 0x450]
    if (!w1) goto 0x593d104 // likely
    x0 = 0x8ccd890
    x1 = [sp + 0x48] // (pstr 0x08ccd890) "X" // (pstr 0x00000058) "@"
    0x4d35930 (0x8ccd890, 0x8ccd890, 0x10078070, 0x8f08488)
    x2 = sp + 0x70
    x3 = [sp + 0x48] // (pstr 0x08ccd890) "X" // (pstr 0x00000058) "@"
    (q16, 2) = 3  // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    stp q16, q17, [x3]
    (q16, 2) = 3
    [x3 + 0x20] = (q16, 2)
    q16 = [x2 + 0x40]
    [x3 + 0x40] = q16
    x25 = [x2 + 0x50]
    [x3 + 0x50] = x25
    x0 = 8
    x1 = 0x877b128
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d322b0 (0x8, 0x877b128, 0x1, 0x8ccd890)
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (w3) goto 0x593d150 // unlikely
    x3 = [sp + 0x48]
    goto 0x593d15c
    0x4da34f0 (0x8, 0x877b128, 0x1, 0x8ccd890)
    x3 = [sp + 0x48]
    [x25] = x3
    [sp + 0x50] = x0
    [x0] = x3
    x0 = 0x28     // '('
    x1 = 0x89ed3f0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32f40 (0x28, 0x89ed3f0, 0x1, 0x8ccd890)
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (w3) goto 0x593d18c // unlikely
    x1 = [sp + 0x50]
    goto 0x593d198
    0x4da34f0 (0x28, 0x8, 0x1, x3)
    x1 = [sp + 0x50]
    [x25] = x1
    [x0] = x1
    x29 = [sp -8]
    x30 = [sp] + 0xd0
    [sp + 8] = (x0, 2)
    x3 = x30
    0x4da15a0 (0x28, 0x8, 0x1, x3)
    (x0, 2) = 3
    goto 0x593cfb0
    x16 = [x28 + 0x10]
    v = sp - x16  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x593d244 // likely
    [sp - 0x20]! = x30
    [sp -8] = x29
    x29 = sp - 8
    w5 = (byte) [x0 + 0x20] // elf_phdr
    if (w5 != 0) goto 0x593d220 // unlikely
    [sp + 0x28] = x0
    x0 = [x0]     // elf_shdr
    0x593f1f0 (0xace39d8, 0x8, 0x1, x3)
    x5 = [sp + 0x28]
    [x5 + 8] = x0
    [x5 + 0x18] = x2 // elf_phdr
    x27 = 0xb48f000
    w6 = [x27 + 0x450]
    if (!w6) goto 0x593d210 // likely
    x6 = [x5 + 0x10]
    0x4da3500 (0xace39d8, 0x8, 0x1, x3)
    stp x1, x6, [x25]
    [x5 + 0x10] = x1
    x29 = [sp -8]
    x30 = [sp] + 0x20
    [x0 + 0x20] = (byte) 0 // (pstr 0x01c00101) "ents/environments.(*Status).ToPointer"
    (x5, 2) = 3
    x2 = [x0 + 0x18]
    x0 = x5
    x3 = 0
    x4 = 0
    x29 = [sp -8]
    x30 = [sp] + 0x20
    [sp + 8] = x0
    x3 = x30
    0x4da15a0 (0x3d01040101023c02, 0x10f026e3d023e05, 0xbf3d01c004020009, x3)
    x0 = [sp + 8]
    goto 0x593d1c0
    x16 = [x28 + 0x10]
    v = sp - x16  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x593d428 // likely
    [sp - 0x80]! = x30
    [sp -8] = x29
    x29 = sp - 8
    [sp + 0x88] = x0
    [sp + 0x58] = (0, 2)
    [sp + 0x68] = 0
    goto 0x593d28c
    [x2 + 8] = x1
    x0 = [sp + 0x88]
    0x593d1c0 (0x3d01040101023c02, 0x10f026e3d023e05, 0xbf3d01c004020009, x3)
    if (x3) goto 0x593d408 // unlikely
    v = x0 - 3    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v > 0) goto 0x593d2b4 // likely
    if (!x0) goto 0x593d380 // unlikely
    if (v) goto 0x593d2cc // likely
    x0 = [sp + 0x88]
    0x593d970 (0x3d01040101023c02, 0x10f026e3d023e05, 0xbf3d01c004020009, x3)
    goto 0x593d304
    v = x0 - 4    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x593d2fc // unlikely
    v = x0 - 6    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x593d2f0 // unlikely
    v = x0 - 0x11 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x593d2e4 // unlikely
    x1 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x0 = [sp + 0x88]
    [x0 + 0x20] = (byte) w1
    0x593eac0 (0x3d01040101023c02, 0x1, 0xbf3d01c004020009, x3)
    goto 0x593d304
    x0 = [sp + 0x88]
    0x593ddf0 (0x3d01040101023c02, 0x1, 0xbf3d01c004020009, x3)
    goto 0x593d304
    x0 = [sp + 0x88]
    0x593e590 (0x3d01040101023c02, 0x1, 0xbf3d01c004020009, x3)
    goto 0x593d304
    x0 = [sp + 0x88]
    0x593deb0 (0x3d01040101023c02, 0x1, 0xbf3d01c004020009, x3)
    if (x2) goto 0x593d3dc // likely
    (x5, 2) = 3
    x6 += 1       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = [sp + 0x68]
    v = x2 - x6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x593d354 // unlikely
    [sp + 0x70] = x1
    [sp + 0x50] = x0
    x0 = x5
    x1 = x6       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x86712e8
    return x0;
}

