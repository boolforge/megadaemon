// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f29 (void) {
    v = x5 - 4    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    goto 0x53ce7bc
    [sp + 8] = (x0, 2)
    x3 = x30
    0x4da15a0 (x0, x1, x2, x3)
    (x0, 2) = 3
    goto 0x53ce5f0
    x0 = 0
    x1 = 0
    x0 = 0
    x1 = 0
    x16 = [x28 + 0x10]
    v = sp - x16  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x53ce8d0 // likely
    [sp - 0x30]! = x30
    [sp -8] = x29
    x29 = sp - 8
    (x3, 2) = 3
    (x5, 2) = 3
    v = x3 - x5   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x53ce8c0 // unlikely
    v = x4 - x6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x53ce8c0 // unlikely
    (x3, 2) = 3   // section..text
    (x5, 2) = 3   // section..text
    v = x3 - x5   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x53ce8c0 // unlikely
    v = x4 - x6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x53ce8c0 // unlikely
    (x3, 2) = 3   // elf_shdr // elf_phdr
    (x0, 2) = 3   // elf_shdr // elf_phdr
    v = x3 - x0   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x53ce8c0 // unlikely
    0x4d24430 (0x40, 0xace39d8, 0xace39d8, 0x40)
    goto 0x53ce8c4
    x0 = 0
    x29 = [sp -8]
    x30 = [sp] + 0x30
    [sp + 8] = (x0, 2)
    x3 = x30
    0x4da15a0 (x0, 0xace39d8, 0xace39d8, 0x53ce82c)
    (x0, 2) = 3   // elf_shdr
    goto 0x53ce860
    x16 = [x28 + 0x10]
    v = sp - x16  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x53ce9dc // likely
    [sp - 0x40]! = x30
    [sp -8] = x29
    x29 = sp - 8
    return x0;
}

