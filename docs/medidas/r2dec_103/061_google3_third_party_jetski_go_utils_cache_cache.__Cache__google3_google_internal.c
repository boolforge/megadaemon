// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f61 (void) {
    (x6, 2) = 3
    v = x6 - x7   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x66e3b88 // likely
    x8 = 0
    if (v) goto 0x66e3b94 // unlikely
    x6 = [x8]
    goto 0x66e3b98
    x6 = 0
    if (!x6) goto 0x66e3ba4 // likely
    (x6, 2) = 3   // elf_phdr // section..text
    goto 0x66e3bac
    x6 = 0
    x7 = 0
    [sp + 0xe0] = x7
    [sp + 0x1a0] = x6
    [sp + 0x1e0] = x4
    [sp + 0x128] = x5
    if (!x1) goto 0x66e3bcc // likely
    (x8, 2) = 3   // elf_shdr
    x10 = [x1 + 0x38]
    goto 0x66e3bd8
    x8 = 0
    x9 = 0
    x10 = 0
    return x0;
}

