// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f32 (void) {
    x1 = [x5 + 8]
    [sp + 0xc8] = x1
    x6 = 0
    x7 = 0
    x8 = 0
    x9 = 0
    goto 0x55a9194
    x0 = 0
    (x29, 2) = 3
    sp += 0x110
    x27 = 0x948e3c0
    (x1, 2) = 3
    x0 = 0
    (x29, 2) = 3
    sp += 0x110
    x1 = 0
    x2 = 0
    (x29, 2) = 3
    sp += 0x110
    x3 = [sp + 0xf0] + 0x14
    x5 = [sp + 0x90]
    x4 = x5 - 1
    x6 = x0
    x8 = x2
    x9 = x1
    x1 = [sp + 0xc8]
    v = x4 - 0    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v <= 0) goto 0x55a9270 // likely
    [sp + 0x78] = (x9, 2)
    [sp + 0xd8] = x7
    [sp + 0x90] = x4
    [sp + 0x67] = (byte) w6
    [sp + 0xf0] = x3
    q0 = [x3]
    x3 = [x3 0xc] // elf_phdr
    [sp 0x53] = q0
    [sp 0x5f] = x3
    x0 = 0x831e6a0
    x2 = sp + 0x53
    0x4d16880 (0x831e6a0, x1, 0x10078383, 0x40)
    w3 = (byte) [x0]
    if (w3 == 0) goto 0x55a91e8 // likely
    (x1, 2) = 3
    w0 = (byte) [sp + 0x67]
    x7 = [sp + 0xd8]
    goto 0x55a9174
    return x0;
}

