// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f50 (void) {
    x0 = [sp + 0x190]
    0x59420d0 (x0, x1, x2, x3)
    if (!x0) goto 0x59424b4 // likely
    (x29, 2) = 3
    sp += 0x170
    0x4da37d0 (x0, x1, x2, x3)
    x8 = ((x7 & x6) << 4) + 8
    x9 = [x5 + x8]
    x8 = x5 + x8
    v = x9 - x2   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x59425a0 // likely
    x7 += 1       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (x9) goto 0x5942564 // unlikely
    [sp + 0x158] = x1
    x0 = x4
    x1 = x2
    0x4d2e4c0 (x0, x1, x2, x3)
    x1 = [sp + 0x158]
    goto 0x5942194
    x0 = [x8 + 8]
    goto 0x5942194
    [sp + 0x20] = x0
    x3 = x30
    0x4da15a0 (0x100b70003, x1, x2, 0x5942598)
    x0 = [sp + 0x20]
    goto 0x59420d0
    x16 = [x28 + 0x10]
    v = sp - x16  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x5942654 // likely
    [sp - 0x10]! = x30
    [sp -8] = x29
    x29 = sp - 8
    x2 = [x0 + 8]
    if (x2) goto 0x594260c // likely
    x2 = 6
    [x0 + 8] = x2
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x5942600 // likely
    x2 = [x0]
    0x4da34f0 (0x100b70003, x1, -1, 0x5942598)
    [x25] = x2
    x2 = 0x4254594
    [x0] = x2
    x2 = [x0 + 0x28]
    if (x2) goto 0x5942640 // likely
    x2 = 4
    [x0 + 0x28] = x2
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x5942634 // likely
    x2 = [x0 + 0x20]
    0x4da34f0 (0x100b70003, x1, -1, 0x5942598)
    [x25] = x2
    x2 = 0x423dabd
    [x0 + 0x20] = x2
    x0 = 0
    x1 = 0
    x29 = [sp -8]
    x30 = [sp] + 0x10
    [sp + 8] = x0
    x3 = x30
    0x4da15a0 (x0, x1, 0x423dabd, 0x59425b4)
    x0 = [sp + 8]
    goto 0x59425c0
    x16 = [x28 + 0x10]
    v = sp - x16  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x594271c // likely
    [sp - 0x30]! = x30
    [sp -8] = x29
    x29 = sp - 8
    x2 = [x0 + 8]
    if (x2) goto 0x59426bc // unlikely
    x2 = 6
    [x0 + 8] = x2
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x59426b0 // likely
    x2 = [x0]
    0x4da34f0 (x0, x1, 0x10102464c457f, 0x59425b4)
    [x25] = x2
    x2 = 0x4254594
    [x0] = x2
    x2 = [x0 + 0x60]
    if (x2) goto 0x59426cc // likely
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [x0 + 0x60] = x2
    (x2, 2) = 3   // elf_shdr // elf_phdr
    goto 0x59426e4
    x2 = [sp + 0x20] + 0x10
    x4 = [sp + 0x18]
    x3 = x4 - 1
    v = x3 - 0    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    return x0;
}

