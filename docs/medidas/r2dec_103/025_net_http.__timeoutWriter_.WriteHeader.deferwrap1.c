// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f25 (void) {
    [x2] = q16
    x25 = [x1 + 0x10]
    [x2 + 0x10] = x25
    x27 = 0xb48f000
    w1 = [x27 + 0x450]
    if (!w1) goto 0x527e574 // likely
    x0 = 0x8e5b130
    x1 = [sp + 0x690]
    x2 = sp + 0x3f0
    0x4d35930 (0x8e5b130, x1, 0x100783f0, x3)
    x7 = [sp + 0x690]
    x8 = sp + 0x3f0
    x0 = x7
    x24 = 3
    (q16, 2) = 3
    [x7] + 0x20 = (q16, 2)
    (q16, 2) = 3
    [x7] + 0x20 = (q16, 2)
    x24 -= 1
    if (x24) goto 0x527e584 // likely
    q16 = [x8]
    [x7] = q16
    x25 = [x8 + 0x10]
    [x7 + 0x10] = x25
    x1 = [sp + 0x268]
    if (!x1) goto 0x527e600 // likely
    x26 = [x1 + 0x58] // elf_phdr
    return x0;
}

