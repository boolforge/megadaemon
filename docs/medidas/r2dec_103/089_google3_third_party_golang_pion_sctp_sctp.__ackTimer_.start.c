// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f89 (void) {
    if (!w3) goto 0x6df5478 // likely
    x3 = [x1]
    0x4da3500 (x0, x1, x2, 0x10102464c457f)
    stp x2, x3, [x25]
    [x1] = x2
    x0 = [sp + 0x358]
    0x6de3910 (x0, x1, x2, 0x10102464c457f)
    if (x1) goto 0x6df550c // unlikely
    if (!x0) goto 0x6df54c4 // likely
    [sp + 0x1f8] = x0
    x3 = [x0]
    if (!x3) goto 0x6df54b8 // unlikely
    (x1, 2) = 3   // elf_phdr // section..text
    if (!x1) goto 0x6df6610 // unlikely
    x4 = 0x94d2680
    ldar x5, [x4]
    x6 = [x5]
    w7 = [x1 + 0x10]
    goto 0x6df6664
    x1 = 0
    x2 = 0
    goto 0x6df55e4
    x0 = 2
    x1 = 0x89932e0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33a20 (0x2, 0x89932e0, 0x1, 0x40)
    x3 = 0x2802   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    [x0] = (half) w3
    x27 = 0x948d000
    x5 = [x27 + 0x550]
    x1 = 0
    x2 = 0
    x3 = x0
    x4 = 0x8f35c88
    x0 = 0
    (x29, 2) = 3
    sp += 0x330
    [sp + 0x1e8] = x2
    [sp + 0x150] = x1
    x0 = 2
    x1 = 0x89932e0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33a20 (0x2, 0x89932e0, 0x1, 0x2)
    x3 = 0x2802   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    [x0] = (half) w3
    x1 = 0
    x2 = 0
    x3 = x0
    x4 = [sp + 0x150]
    x5 = [sp + 0x1e8]
    x0 = 0
    (x29, 2) = 3
    sp += 0x330
    x0 = 2
    x1 = 0x89932e0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33a20 (0x2, 0x89932e0, 0x1, 0x2)
    x3 = 0x2802   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    [x0] = (half) w3
    x27 = 0x948d000
    x5 = [x27 + 0x468]
    x1 = 0
    x2 = 0
    x3 = x0
    x4 = 0x8f35c88
    x0 = 0
    (x29, 2) = 3
    sp += 0x330
    x0 = 2
    x1 = 0x89932e0
    return x0;
}

