// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f45 (void) {
    [x0 + 0xa0] = (0, 2)
    x1 = -0x7f7f7f7f7f7f7f80
    [sp + 0xe0] = x1
    [sp + 0x288] = x0
    0x4d9c1c0 (x0, 0x8080808080808080, x2, x3)
    [sp + 0x280] = x0
    x0 = 0x58d2440
    x2 = [sp + 0x240]
    x27 = sp + 0x260
    stp x0, x2, [x27]
    x1 = sp + 0x2a8
    [sp + 0x270] = x1
    x0 = [sp + 0x358]
    x3 = [sp + 0x360]
    goto 0x58d189c
    x0 = [sp + 0x258] + 0x14
    x8 = [sp + 0xc0]
    x3 = x8 - 1
    x2 = [sp + 0x240]
    v = x3 - 0    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v <= 0) goto 0x58d1954 // likely
    [sp + 0xc0] = x3
    [sp + 0x258] = x0
    q0 = [x0]
    x8 = [x0 0xc] // elf_phdr
    [sp 0x92] = q0
    [sp 0x9e] = x8
    q0 = [sp 0x92]
    x8 = [sp 0x9e] // elf_phdr
    [sp 8] = q0
    [sp 0x14] = x8
    x0 = [sp + 0x348]
    x1 = [sp + 0x350]
    x3 = sp + 0x278
    x4 = [sp + 0x370]
    x5 = [sp + 0x378]
    x6 = [sp + 0x380]
    x7 = sp + 0x260
    0x58d1ad0 (x0, x1, x2, 0x10078278)
    if (!x0) goto 0x58d1888 // likely
    w2 = (byte) [sp + 0x388]
    if (w2 == 0) goto 0x58d1934 // likely
    return x0;
}

