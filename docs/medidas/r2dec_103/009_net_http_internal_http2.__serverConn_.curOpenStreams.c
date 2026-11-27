// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f9 (void) {
    ldr x26, [x10, x2, lsl 3]
    if (x26) goto 0x521a6f8 // likely
    x26 = 0x8f94fc0
    x10 = [x26]
    w1 = (byte) [sp + 0xd0]
    (w4, 2) = 3
    w3 = (byte) [sp + 0xd2]
    x11 = [sp + 0xc8]
    x0 = [x11 + 0xf0]
    x6 = [x11 + 0x30]
    x7 = [sp + 0x80]
    (x8, 2) = 3
    callreg x10   // pstate(0x779b960, 0x0, 0x0, 0x0)
    if (!x2) goto 0x521a7e0 // likely
    x4 = 0x8f26bc8
    v = x2 - x4   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x521a7cc // likely
    (x4, 2) = 3
    [sp + 0x78] = x5
    [sp + 0xa0] = x4
    w3 = [x3]
    [sp + 0x48] = w3
    x0 = 0x10
    x1 = 0x8757890
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32500 (0x10, 0x8757890, 0x1, 0x464c457f)
    x3 = [sp + 0x78]
    [x0 + 8] = x3
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (w3) goto 0x521a77c // unlikely
    x1 = [sp + 0xc8]
    x3 = [sp + 0xa0]
    goto 0x521a794
    x1 = [sp + 0xc8]
    x2 = [x1 + 0x28] // elf_shdr
    0x4da3510 (0x10, x1, 0xace39d8, x3)
    return x0;
}

