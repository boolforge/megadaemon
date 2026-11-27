// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f70 (void) {
    goto 0x66e41c0
    x5 = 0
    if (!x1) goto 0x66e41cc // likely
    x6 = [x1 + 0xc]
    goto 0x66e41d0
    x6 = 0
    if (!x1) goto 0x66e41dc // likely
    (x7, 2) = 3   // section..text
    goto 0x66e41e4
    x7 = 0
    x8 = 0
    if (!x1) goto 0x66e41f0 // likely
    (x9, 2) = 3   // elf_shdr // elf_phdr
    goto 0x66e41f8
    x9 = 0
    x10 = 0
    [sp + 0x180] = x7
    [sp + 0xc0] = x8
    [sp + 0x150] = x4
    [sp + 0x178] = x9
    return x0;
}

