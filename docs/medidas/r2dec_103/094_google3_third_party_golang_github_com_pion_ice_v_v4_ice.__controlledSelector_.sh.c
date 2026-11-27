// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f94 (void) {
    [x0] = x4
    [x0 + 8] = x6
    x6 = [sp + 0x268]
    x1 = 0x10
    x2 = x0
    [sp + 0x78] = x1
    [sp + 0x140] = x2
    x27 = 0x957e000
    x5 = [x27 + 0x930]
    v = x5 - x6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x6eadef4 // unlikely
    x5 = 0
    x7 = 0
    goto 0x6eadefc
    (x5, 2) = 3
    [sp + 0x70] = x7
    [sp + 0x138] = x5
    x0 = 0x30     // '0'
    x1 = 0x89cd9f8
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32f40 (0x30, 0x89cd9f8, 0x1, x3)
    x3 = [sp + 0x78]
    [x0 + 8] = (x3, 2)
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (w3) goto 0x6eadf34 // unlikely
    (x10, 2) = 3
    goto 0x6eadf40
    0x4da3500 (0x30, 0x89cd9f8, 0x1, x3)
    (x10, 2) = 3
    stp x9, x10, [x25]
    [x0] = x9
    [x0 + 0x18] = (0, 2)
    x9 = [sp + 0x70]
    [x0 + 0x28] = x9
    x9 = [sp + 0x4d8]
    x27 = x9 + 0x2c8
    (x10, 2) = 3
    x27 = x9 + 0x2b8
    (x2, 2) = 3   // segment.LOAD5
    w4 = (word) [x9 + 0xfe]
    w5 = (word) [x9 + 0xfc]
    x6 = [sp + 0x150]
    x7 = [sp + 0x88]
    x8 = x0
    x0 = x10
    0x6eb3800 (0xd41c, 0xd41c, 0xd8d6298, 0xd8d6298)
    if (!x2) goto 0x6eae054 // unlikely
    x0 = 0x20
    x1 = 0x81a8cf0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32c00 (0x20, 0x81a8cf0, 0x1, 0xd8d6298)
    [sp + 0x248] = x0
    x0 = [sp + 0x150]
    x1 = [sp + 0x88]
    0x4d98880 (x0, x1, 0x1, 0xd8d6298)
    x3 = 0x856f550
    x4 = [sp + 0x248] // (pstr 0x00000020) "@"
    [x4] = x3
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (!w3) goto 0x6eadfd0 // likely
    x2 = [x4 + 8] // elf_shdr
    0x4da3500 (x0, x1, 0xace39d8, x3)
    stp x0, x2, [x25]
    [x4 + 8] = x0
    (x2, 2) = 3
    x27 = sp + 0x320
    stp x2, x3, [x27]
    x2 = [sp + 0x268]
    [sp + 0x330] = x2
    x0 = 0x8eba5d0
    x1 = sp + 0x320
    0x4d2e1f0 (0x8eba5d0, 0x10078320, x2, x3)
    x2 = 0x8eba5d0
    x3 = [sp + 0x248] // (pstr 0x00000020) "@"
    [x3 + 0x10] = x2
    x27 = 0xb48f000
    w2 = [x27 + 0x450]
    if (!w2) goto 0x6eae020 // likely
    x6 = [x3 + 0x18]
    0x4da3500 (0x8eba5d0, 0x10078320, x2, 0x20)
    stp x0, x6, [x25]
    [x3 + 0x18] = x0
    x6 = [sp + 0x4d8]
    return x0;
}

