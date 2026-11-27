// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f67 (void) {
    (x1, 2) = 3   // elf_shdr
    goto 0x66e3f68
    x3 = 0
    x1 = 0
    x2 = [x8 + 0x80]
    (x0, 2) = 3
    x6 = x3 + x5
    v = x2 - x6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x66e3fa8 // unlikely
    [sp + 0x148] = x5
    [sp + 0xf8] = x3
    [sp + 0x1b8] = x1
    x1 = x6
    x4 = 0x856f5d0
    0x4d9e050 (0x8, 0x500000001, x2, x3)
    x3 = [sp + 0xf8]
    x5 = [sp + 0x148]
    x6 = x1
    x1 = [sp + 0x1b8]
    [sp + 0x140] = (x6, 2)
    [sp + 0x258] = x0
    x0 += x5
    x2 = x3
    return x0;
}

