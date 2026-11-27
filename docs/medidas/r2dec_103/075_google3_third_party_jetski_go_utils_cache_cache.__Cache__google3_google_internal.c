// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f75 (void) {
    x0 += 0x90
    0x4d177a0 (0x90, x1, x2, x3)
    x27 = 0xb48f000
    w1 = [x27 + 0x450]
    if (w1) goto 0x66e44a4 // unlikely
    x4 = [sp + 0x218]
    goto 0x66e44b4
    x3 = [x0]
    0x4da3500 (0x90, x1, x2, x3)
    x4 = [sp + 0x218]
    stp x4, x3, [x25]
    [x0] = x4
    x0 = 0x48     // 'H'
    x1 = 0x8c419c8
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33650 (0x48, 0x8c419c8, 0x1, x3)
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (w3) goto 0x66e44e0 // unlikely
    x2 = [sp + 0x268]
    goto 0x66e44ec
    0x4da34f0 (0x48, 0x8c419c8, x2, x3)
    x2 = [sp + 0x268]
    [x25] = x2
    [x0 + 8] = x2
    [sp + 0x280] = x0
    x2 = [sp + 0x1f8]
    x0 = [x2]
    x1 = sp + 0x280 // (pstr 0x00000048) "@"
    0x4d282f0 (0x10102464c457f, 0x10078280, x2, x3)
    x4 = [sp + 0x1f8]
    x9 = [sp + 0x1d8]
    x12 = [sp + 0x1c8]
    x13 = [sp + 0x108]
    x14 = [sp + 0x158]
    goto 0x66e3878
    if (!x9) goto 0x66e4528 // likely
    x1 = [x9 + 0x10]
    goto 0x66e452c
    x1 = 0
    if (x1) goto 0x66e4694 // unlikely
    if (!x9) goto 0x66e453c // likely
    x1 = [x9 + 0x48] // elf_phdr
    goto 0x66e4540
    x1 = 0
    if (!x1) goto 0x66e454c // likely
    x1 = [x1 + 8]
    return x0;
}

