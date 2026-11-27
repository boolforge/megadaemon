// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f100 (void) {
    0x4d33a20 (x0, x1, x2, x3)
    w3 = (word) [sp + 0x64]
    [x0] = (half) w3
    x2 = [sp + 0xf0]
    x7 = [sp + 0x188]
    x10 = [sp + 0xd8]
    x13 = 1       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x9 = x0
    x8 = [sp + 0xb8]
    x3 = [sp + 0xa8]
    x0 = [sp + 0x160]
    goto 0x6ec8248
    x10 = x3 + 1  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    w9 = [x9]
    v = x8 - x10  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x6ec8370 // unlikely
    [sp + 0x84] = w9
    x1 = x10      // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = x8
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x88b9c78
    0x4d9e050 (x0, 0x1, x2, 0x1)
    w9 = [sp + 0x84]
    x10 = x1      // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x8 = x2
    [sp + 0x160] = x0
    [sp + 0xa8] = x10
    [sp + 0xb8] = x8
    x3 = x10 - 1
    str w9, [x0, x3, lsl 2]
    x0 = 0x20
    x1 = 0x8b31038
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32c00 (0x20, 0x8b31038, 0x1, x3)
    w3 = [sp + 0x80]
    w2 = [sp + 0x7c]
    stp w3, w2, [x0]
    x4 = [sp + 0xa8] // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x5 = [sp + 0xb8]
    [x0 + 0x10] = (x4, 2)
    x27 = 0xb48f000
    w4 = [x27 + 0x450]
    if (w4) goto 0x6ec83c4 // unlikely
    x4 = [sp + 0x160]
    goto 0x6ec83d0
    0x4da34f0 (0x20, 0x8b31038, x2, x3)
    x4 = [sp + 0x160]
    [x25] = x4
    [x0 + 8] = x4
    goto 0x6ec7f4c
    0x4da37d0 (0x20, 0x8b31038, x2, x3)
    0x4da37d0 (0x20, 0x8b31038, x2, x3)
    0x4d5e250 (0x20, 0x8b31038, x2, x3)
    (x29, 2) = 3
    sp += 0x2f0
    [sp + 8] = (x0, 2)
    [sp + 0x18] = x2
    x3 = x30
    0x4da15a0 (0x20, 0x8b31038, x2, x3)
    (x0, 2) = 3   // (pstr 0x00000020) "@"
    x2 = [sp + 0x18]
    goto 0x6ec78b0
    (x1, 2) = 3
    goto 0x6ec8420
    x1 = x3 + 0x20
    x2 -= 1
    v = x2 - 0    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v <= 0) goto 0x6ec846c // likely
    x3 = x1
    (x4, 2) = 3   // elf_shdr // elf_phdr
    v = x5 - 4    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x6ec8458 // likely
    w4 = [x4]     // elf_phdr
    x5 = 0x6b63616e
    v = w4 - w5   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x6ec8418 // likely
    x1 = [x1 + 0x18]
    if (x1) goto 0x6ec8418 // likely
    goto 0x6ec8464
    x5 = 0x6b63616e
    goto 0x6ec8418
    x0 = 0
    x16 = [x28 + 0x10]
    x17 = sp - 0x40
    v = x17 - x16 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x6ec8670 // likely
    [sp - 0xc0]! = x30
    [sp -8] = x29
    x29 = sp - 8
    [sp + 0xc8] = (half) w0 // (pstr 0x00000020) "@"
    ubfx x5, x0, 0, 0x10
    [sp + 0x54] = w5
    x6 = 0
    x7 = 6
    x8 = section..noptrbss
    x1 = 0
    goto 0x6ec84cc
    x2 = x1 - 1
    strh w10, [x8, x2, lsl 1]
    x7 += 1
    v = x7 - 0x10 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v >= 0) goto 0x6ec85ac // unlikely
    x9 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x10 = x9 << x7
    ubfx x11, x10, 0, 0x10
    v = w11 - w5  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x6ec8534 // unlikely
    x1 += 1       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    v = x6 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x6ec84c0 // unlikely
    [sp + 0x58] = x7
    [sp + 0x52] = (half) w10
    x0 = x8       // section..noptrbss
    x2 = x6
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x856f650
    0x4d9e050 (0xb48bd40, 0x1, x2, 0x1)
    w5 = [sp + 0x54]
    x7 = [sp + 0x58]
    x9 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    w10 = (word) [sp + 0x52]
    x8 = x0       // section..noptrbss
    x6 = x2
    w0 = (word) [sp + 0xc8]
    goto 0x6ec84c0
    ubfx x2, x0, 6, 0xa
    [sp + 0x70] = x2
    x0 = 0x856f750
    x1 = x2
    0x4d9df80 (0x856f750, x1, x2, 0x1)
    [sp + 0x80] = x0
    x0 = 0x38     // '8'
    x1 = 0x8c754a8
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33310 (0x38, 0x8c754a8, 0x1, 0x1)
    x3 = [sp + 0x70]
    return x0;
}

