// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f39 (void) {
    if (w0 == 0) goto 0x55b0218 // likely
    x4 = [sp + 0x308]
    (x0, 2) = 3
    goto 0x55b0220
    x4 = [sp + 0x308]
    (x0, 2) = 3   // elf_phdr
    x2 = [sp + 0x220]
    x3 = [sp + 0x108]
    0x55b1240 (0x400000006, 0x40, x2, x3)
    x4 = [sp + 0x1e8]
    x1 = x4 + 1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = [sp + 0x1f0]
    v = x2 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x55b0248 // likely
    x3 = [sp + 0x508]
    goto 0x55b0268
    [sp + 0x120] = x0
    x0 = [sp + 0x508]
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x89befb0
    0x4d9e050 (x0, 0x1, x2, 0x1)
    x3 = x0
    x0 = [sp + 0x120]
    return x0;
}

