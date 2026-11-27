// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f74 (void) {
    x2 = 0x4a148bc
    x1 = 0xf
    [sp + 0x9c] = w1
    if (!x0) goto 0x66e441c // likely
    x3 = [x0 + 0x50] // elf_phdr
    if (!x3) goto 0x66e441c // unlikely
    (x3, 2) = 3   // elf_phdr
    goto 0x66e4424
    x4 = 0
    x3 = 0
    [sp + 0x168] = x3
    [sp + 0xa8] = x4
    x0 = 0x8e59c90
    0x4d31c10 (0x8e59c90, 0xf, 0x4a148bc, x3)
    return x0;
}

