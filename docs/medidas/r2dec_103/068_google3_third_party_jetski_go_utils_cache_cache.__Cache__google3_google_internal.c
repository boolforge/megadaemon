// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f68 (void) {
    goto 0x66e402c
    x4 = [x5 + 0x98]
    0x4da3500 (x0, x1, x2, x3)
    x6 = [sp + 0x1d0]
    stp x6, x4, [x25]
    [x5 + 0x98] = x6
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [x3] = (byte) w2
    goto 0x66e4040
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x1 = [sp + 0x1d8]
    if (!x1) goto 0x66e4050 // likely
    x4 = [x1 + 0x28] // elf_shdr
    goto 0x66e4054
    x4 = 0
    if (!x4) goto 0x66e408c // likely
    return x0;
}

