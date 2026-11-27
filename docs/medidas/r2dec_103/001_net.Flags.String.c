// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f1 (void) {
    [x6 + 0x40] = q16 // elf_phdr
    x6 = [(sp + 0x1da)]
    x7 = [sp + 0x1e0]
    x27 = sp + 0x16a
    [x27] = x6
    [sp + 0x170] = x7
    goto 0x4ee20bc
    x6 = sp + 0x340
    stp 0, 0, [x6]
    [x6 + 0x10] = (0, 2)
    [x6 + 0x20] = (0, 2)
    [x6 + 0x30] = (0, 2)
    [x6 + 0x40] = (0, 2)
    x6 = sp + 8
    x7 = sp + 0x3f8
    x24 = 4
    (q16, 2) = 3
    [x6] + 0x20 = (q16, 2)
    (q16, 2) = 3
    [x6] + 0x20 = (q16, 2)
    x24 -= 1
    if (x24) goto 0x4ee1f8c // likely
    x25 = [x7 -4]
    [x6 -4] = x25
    x0 = x1
    x1 = [sp + 0x308]
    w2 = (word) [sp + 0x168]
    x3 = [sp + 0x298]
    x4 = [sp + 0x200]
    x5 = [sp + 0x208]
    0x4ee1350 (x0, x1, x2, x3)
    (x16, 2) = 3
    x27 = sp + 0x390
    stp x0, x1, [x27]
    [sp + 0x3a0] = x2
    [sp + 0x3a8] = (half) w3
    [sp + 0x3aa] = (half) w4
    [sp + 0x3ac] = (half) w5
    [sp + 0x3ae] = (half) w6
    [sp + 0x3b0] = (half) w7
    [sp + 0x3b2] = (half) w8
    [sp + 0x3b4] = (byte) w9
    x27 = sp + 0x3b8
    stp x10, x11, [x27]
    [sp + 0x3c8] = (byte) w12
    [sp + 0x3d0] = x13
    [sp + 0x3d8] = (half) w14
    [sp + 0x3da] = (half) w15
    x6 = sp + 0x340
    x7 = sp + 0x390
    (q16, 2) = 3
    stp q16, q17, [x6]
    (q16, 2) = 3  // "hot_cold_t"
    [x6 + 0x20] = (q16, 2)
    q16 = [x7 + 0x40]
    [x6 + 0x40] = q16
    x7 = [sp + 0x110]
    x8 = [(sp + 0x116)]
    x27 = sp + 0x1be
    [x27] = x7
    x27 = sp + 0x1c4
    [x27] = x8
    x7 = sp + 0x390
    (q16, 2) = 3
    stp q16, q17, [x7]
    (q16, 2) = 3  // "hot_cold_t"
    return x0;
}

