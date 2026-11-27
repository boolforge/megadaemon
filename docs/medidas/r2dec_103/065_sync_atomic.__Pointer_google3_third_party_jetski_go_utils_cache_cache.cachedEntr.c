// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f65 (void) {
    v = x5 - x7   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x66e3e74 // likely
    x6 = 0
    if (v) goto 0x66e3e80 // unlikely
    (x5, 2) = 3
    goto 0x66e3e88
    x6 = 0
    x5 = 0
    x8 = [sp + 0x230]
    [x8 + 0x58] = x6
    x27 = 0xb48f000
    w6 = [x27 + 0x450]
    if (!w6) goto 0x66e3ea8 // likely
    x6 = [x8 + 0x50] // elf_phdr
    0x4da3500 (x0, x1, x2, x3)
    stp x5, x6, [x25]
    return x0;
}

