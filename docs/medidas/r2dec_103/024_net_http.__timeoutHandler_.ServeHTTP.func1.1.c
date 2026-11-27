// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f24 (void) {
    stp x2, x1, [x25]
    [x0] = x2
    x0 = [sp + 0x2c8]
    goto 0x527dde0
    0x4da37d0 (x0, x1, x2, x3)
    0x4da37d0 (x0, x1, x2, x3)
    if (!x7) goto 0x527cc04 // likely
    [sp + 0x398] = x7
    x1 = [sp + 0x268]
    if (!x1) goto 0x527e4c8 // likely
    x26 = [x1 + 0x50] // elf_phdr
    if (!x26) goto 0x527e4c8 // unlikely
    x0 = [x26]    // elf_phdr
    callreg x0    // 0x400000006(0x400000006, 0x0, 0x0, 0x0)
    x0 = [sp + 0x338]
    x7 = [sp + 0x398]
    if (!x7) goto 0x527e4e8 // likely
    x3 = [x7 + 8]
    x4 = 0x949d4c0
    return x0;
}

