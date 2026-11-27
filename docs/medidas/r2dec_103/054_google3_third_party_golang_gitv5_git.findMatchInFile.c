// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f54 (void) {
    0x4da39e0 (x0, x1, x2, x3)
    x0 = 0x14
    x1 = 0x81a5cd8
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33f50 (0x14, 0x81a5cd8, 0x1, x3)
    q0 = [sp 0x7b]
    x3 = [sp 0x87]
    [x0] = q0
    [x0 0xc] = x3
    x1 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = x1       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    (x29, 2) = 3
    sp += 0x140
    x0 = 0
    x1 = 0
    x2 = 0
    (x29, 2) = 3
    sp += 0x140
    [sp + 0x148] = x0
    [sp + 0x150] = (x1, 2)
    x2 &= 0xfffffffffffffffe
    [sp + 0xe0] = x2
    x0 = x1
    x1 = x2
    0x4f31070 (x0, x1, x2, x3)
    if (x3) goto 0x5950434 // unlikely
    x5 = [sp + 0x148]
    (x5, 2) = 3
    if (!x5) goto 0x5950464 // unlikely
    x7 = [x5 + 8]
    x8 = 0x94b6540
    ldar x9, [x8]
    x10 = [x9]
    w5 = [x5 + 0x10]
    goto 0x59506b4
    x0 = 0
    x1 = 0
    x2 = 0
    (x29, 2) = 3
    sp += 0x140
    x0 = 0
    x1 = 0
    x2 = 0
    (x29, 2) = 3
    sp += 0x140
    x3 = x1
    x4 = x2
    x1 = x6
    x2 = x0
    x0 = x5
    0x5951340 (-1, x1, x2, x3)
    x5 = [sp + 0x158]
    x6 = [sp + 0xe0]
    v = x5 - x6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x59504a0 // likely
    x2 = 0
    x3 = 0
    x4 = 0
    x6 = 0
    goto 0x59504d0
    (x29, 2) = 3
    sp += 0x140
    x5 = [sp + 0x128] + 0x14
    x7 = [sp + 0x110] - 1
    x4 = x2
    x6 = x1
    x1 = x7
    x2 = x0
    x0 = x5
    v = x1 - 0    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v <= 0) goto 0x5950658 // likely
    [sp + 0x110] = x1
    [sp + 0x130] = x3
    [sp + 0x100] = (x6, 2)
    [sp + 0xcf] = (byte) w2
    [sp + 0x128] = x0
    q0 = [x0]
    x3 = [x0 0xc] // elf_phdr
    [sp 0x67] = q0
    [sp 0x73] = x3
    q0 = [sp 0x67]
    x3 = [sp 0x73] // elf_phdr
    [sp 0x53] = q0
    [sp 0x5f] = x3
    x0 = 0x856f5d0
    x1 = 0x28     // '('
    x2 = x1
    0x4d9df80 (0x856f5d0, 0x28, 0x28, 0x40)
    x3 = 0
    x1 = 0
    goto 0x5950540
    [x0 + x6] = (byte) w5
    x3 += 1       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x1 += 2
    v = x3 - 0x14 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v >= 0) goto 0x5950584 // unlikely
    x4 = sp + 0x53
    w5 = (byte) [x4 + x3]
    ubfx x6, x5, 4, 4
    x7 = 0x4325fce
    w6 = (byte) [x7 + x6]
    v = x1 - 0x28 // '(' // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x59506ac // unlikely
    [x0 + x1] = (byte) w6
    x6 = x1 + 1
    x5 &= 0xf
    w5 = (byte) [x7 + x5]
    v = x6 - 0x28 // '(' // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x5950534 // likely
    goto 0x59506a0
    x1 = x0
    x2 = 0x28     // '('
    x0 = sp + 0x8f
    0x4d7d610 (0x100786cf, 0x856f5d0, 0x28, 0x1)
    x2 = [sp + 0x158]
    v = x1 - x2   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v >= 0) goto 0x59505b4 // likely
    (x1, 2) = 3
    w0 = (byte) [sp + 0xcf]
    x3 = [sp + 0x130]
    goto 0x59504ac
    x1 = [sp + 0x150]
    0x4d1a430 (0xff, x1, x2, x3)
    if (w0 == 0) goto 0x5950648 // unlikely
    x5 = [sp + 0x100]
    x1 = x5 + 1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = [sp + 0x108]
    v = x2 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x59505e0 // likely
    x3 = [sp + 0x130]
    w0 = (byte) [sp + 0xcf]
    goto 0x5950624
    v = x1 - 1    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v > 0) goto 0x5950608 // unlikely
    w0 = (byte) [sp + 0xcf]
    if (w0 != 0) goto 0x5950608 // likely
    x3 = sp + 0xe8
    stp 0, 0, [x3]
    [x3 + 0x10] = 0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x0 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    goto 0x5950624
    x0 = [sp + 0x130]
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x8763230
    0x4d9e050 (x0, 0x1, 0x1, 0x1)
    x3 = x0
    w0 = (byte) [sp + 0xcf]
    q0 = [sp 0x67]
    add x5, x1, x1, lsl 2
    x7 = [sp 0x73] // elf_phdr
    x5 = x3 + ((x5 << 2) - 0x14)
    [x5] = q0
    [x5 0xc] = x7
    goto 0x59504ac
    (x1, 2) = 3
    w0 = (byte) [sp + 0xcf]
    x3 = [sp + 0x130]
    goto 0x59504ac
    x5 = sp + 0x140
    x7 = sp
    x5 -= x7
    x0 = x3
    return x0;
}

