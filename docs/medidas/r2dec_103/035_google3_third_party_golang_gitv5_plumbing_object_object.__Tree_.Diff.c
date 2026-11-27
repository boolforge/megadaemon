// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f35 (void) {
    x2 = [sp + 0x2d0]
    w3 = [x2 + 0x68]
    [sp + 0x104] = w3
    x2 = [sp + 0x2e8]
    x27 = (byte) [x2]
    x0 = x2 + 0x40 // elf_phdr
    x1 = 0x9593d60
    0x55ba080 (0x40, 0x9593d60, x2, 0x380)
    if (w0 == 0) goto 0x55af93c // likely
    x5 = [sp + 0x2e8]
    w5 = [x5 + 0x28] // elf_shdr
    goto 0x55af944
    x6 = [sp + 0x2e8]
    w5 = [x6 + 0x68]
    w6 = [sp + 0x104]
    v = w6 - w5   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x55af9e8 // likely
    x5 = [sp + 0x1b8]
    x1 = x5 + 1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = [sp + 0x1c0]
    v = x2 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x55af970 // likely
    x8 = [sp + 0x2f0]
    return x0;
}

