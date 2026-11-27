// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f27 (void) {
    [sp + 0x138] = x0
    [sp + 0x3f] = (byte) 0
    [sp + 0x48] = 0
    x27 = (byte) [x0]
    0x52d3380 (x0, x1, x2, x3)
    if (!x0) goto 0x52e4f80 // likely
    [sp + 0x50] = x0
    x27 = 0x957b000
    x3 = [x27 + 0xaa0]
    x1 = x0
    x2 = 0x8f07588
    x0 = x2
    0x4daa700 (0x8f07588, x1, 0x8f07588, x3)
    if (w0 == 0) goto 0x52e4f6c // likely
    x6 = [sp + 0x138]
    w7 = (byte) [x6 + 0x40] // elf_phdr
    x8 = [x6 + 0x48] // elf_phdr
    x0 = [x6 + 0x38]
    [sp + 0x58] = x0
    [sp + 0x60] = (byte) w7
    [sp + 0x68] = x8
    x27 = 0xb48f000
    w7 = [x27 + 0x450]
    if (!w7) goto 0x52e4ce8 // likely
    x7 = [x6 + 0x38]
    0x4da34f0 (0x30003500400010, x1, 0x8f07588, x3)
    [x25] = x7
    [x6 + 0x38] = 0
    x7 = [x6 + 0x58] // elf_phdr
    x8 = 0x52e7b30
    [sp + 0xf8] = (x8, 2)
    [sp + 0x108] = x0
    x7 = sp + 0xf8
    [sp + 0x120] = x7
    x7 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [sp + 0x3f] = (byte) w7
    w7 = (byte) [x6 + 0x68]
    if (w7 == 0) goto 0x52e4f08 // likely
    w7 = (byte) [sp + 0x60]
    if (w7 == 7) goto 0x52e4f08 // likely
    [sp + 0x70] = x0
    x1 = 0xa
    0x4e060a0 (0x30003500400010, 0xa, 0x8f07588, x3)
    if (!x0) goto 0x52e4d84 // unlikely
    [sp + 0x110] = (0, 2)
    if (!x0) goto 0x52e4d3c // unlikely
    x0 = [x0 + 8]
    [sp + 0x110] = (x0, 2)
    x0 = 0xd
    x1 = 0x43be000
    return x0;
}

