// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f69 (void) {
    0x4d177a0 (x0, x1, x2, x3)
    x27 = 0xb48f000
    w1 = [x27 + 0x450]
    if (w1) goto 0x66e40c0 // unlikely
    x4 = [sp + 0x230]
    goto 0x66e40d0
    x3 = [x0]
    0x4da3500 (x0, x1, x2, 0x10102464c457f)
    x4 = [sp + 0x230]
    stp x4, x3, [x25]
    [x0] = x4
    x0 = 0x48     // 'H'
    x1 = 0x8c419c8
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33650 (0x48, 0x8c419c8, 0x1, 0x10102464c457f)
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (w3) goto 0x66e40fc // unlikely
    x2 = [sp + 0x268]
    goto 0x66e4108
    0x4da34f0 (0x48, 0x8c419c8, x2, x3)
    x2 = [sp + 0x268]
    [x25] = x2
    [x0 + 8] = x2
    [sp + 0x280] = x0
    x2 = [sp + 0x1f8]
    x0 = [x2]
    x1 = sp + 0x280 // (pstr 0x00000048) "@"
    0x4d282f0 (0x10102464c457f, 0x10078280, x2, x3)
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    goto 0x66e38d8
    x0 = [sp + 0x240]
    if (!x0) goto 0x66e4138 // likely
    x3 = [x0 + 0x58] // elf_phdr
    goto 0x66e413c
    x3 = 0
    if (!x3) goto 0x66e43dc // likely
    x0 = 0x40     // elf_phdr
    x1 = 0x8aa22f0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33310 (0x40, 0x8aa22f0, 0x1, x3)
    x1 = [sp + 0x240]
    if (!x1) goto 0x66e4164 // likely
    x2 = [x1 + 0x58] // elf_phdr
    goto 0x66e4168
    x2 = 0
    return x0;
}

