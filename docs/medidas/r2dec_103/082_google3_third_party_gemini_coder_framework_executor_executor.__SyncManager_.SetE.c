// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f82 (void) {
    x11 = [sp + 0x1a0]
    if (!x11) goto 0x67b3c9c // likely
    (x1, 2) = 3   // elf_shdr
    (x3, 2) = 3   // elf_shdr
    x0 = 0
    0x4d7d0e0 (x0, 0xace39d8, 0x38004000000000, 0xace39d8)
    x5 = [sp + 0x1a0]
    [x5 + 0x30] = x1
    x27 = 0xb48f000
    w6 = [x27 + 0x450]
    if (!w6) goto 0x67b3d20 // likely
    x6 = [x5 + 0x28] // elf_shdr
    0x4da3500 (x0, 0xace39d8, 0x38004000000000, 0xace39d8)
    stp x0, x6, [x25]
    [x5 + 0x28] = x0
    x3 = [sp + 0x1a0]
    x0 = [sp + 0x278]
    x1 = [sp + 0x280]
    x2 = [sp + 0x288]
    x4 = [sp + 0x1a8]
    x5 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x67b5210 (x0, x1, x2, x3)
    [sp + 0x180] = (x0, 2)
    if (x0) goto 0x67b4670 // unlikely
    x0 = [sp + 0x278]
    (x7, 2) = 3
    (x9, 2) = 3
    goto 0x67b3c9c
    x3 = [sp + 0x1a0]
    x1 = [sp + 0x280]
    x2 = [sp + 0x288]
    x4 = x7
    return x0;
}

