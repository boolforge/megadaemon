// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f63 (void) {
    x1 = 0
    x2 = [x0 + 0x80]
    (x5, 2) = 3
    x7 = x3 + x6
    v = x2 - x7   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x66e3d14 // unlikely
    [sp + 0xd0] = x3
    [sp + 0x190] = x1
    [sp + 0x148] = x6
    x0 = x5
    x1 = x7
    x4 = 0x856f5d0
    0x4d9e050 (0x8, 0x500000001, x2, x3)
    x3 = [sp + 0xd0]
    x6 = [sp + 0x148]
    x5 = x0
    x7 = x1
    x1 = [sp + 0x190]
    [sp + 0x140] = (x7, 2)
    [sp + 0x258] = x5
    x0 = x5 + x6
    x2 = x3
    0x4da39e0 (0x500000009, x1, x2, x3)
    (x3, 2) = 3
    x5 = [sp + 0x230]
    [x5 + 0x78] = (x3, 2)
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (w3) goto 0x66e3d48 // unlikely
    x4 = [sp + 0x258]
    goto 0x66e3d58
    x3 = [x5 + 0x70]
    0x4da3500 (0x500000009, x1, x2, 0x8)
    x4 = [sp + 0x258]
    stp x4, x3, [x25]
    [x5 + 0x70] = x4
    x0 = x5
    goto 0x66e4928
    if (!x1) goto 0x66e3d8c // likely
    (x4, 2) = 3   // elf_phdr
    x6 = 0x8f09ac8
    v = x4 - x6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x66e3d80 // unlikely
    x5 = 0
    if (v) goto 0x66e3d8c // likely
    x4 = [x5]
    return x0;
}

