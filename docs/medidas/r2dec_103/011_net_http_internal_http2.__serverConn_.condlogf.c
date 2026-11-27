// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f11 (void) {
    [sp + 0x10] = (byte) w1
    [sp + 0x11] = (byte) w2
    [sp + 0x12] = (byte) w3
    [sp + 0x14] = (w4, 2)
    x3 = x30
    0x4da15a0 (x0, x1, x2, x3)
    x0 = [sp + 8]
    w1 = (byte) [sp + 0x10]
    w2 = (byte) [sp + 0x11]
    w3 = (byte) [sp + 0x12]
    (w4, 2) = 3
    goto 0x521a630
    x16 = [x28 + 0x10]
    v = sp - x16  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x521ab04 // likely
    [sp - 0x40]! = x30
    [sp -8] = x29
    x29 = sp - 8
    [sp + 0x48] = x0
    0x521a220 (x0, x1, x2, x3)
    x0 = [sp + 0x48]
    0x521a390 (x0, x1, x2, x3)
    [sp + 0x2c] = (byte) w0
    [sp + 0x2d] = (byte) w1
    [sp + 0x2e] = (byte) w2
    [sp + 0x30] = (w3, 2)
    (w4, 2) = 3
    if (!x5) goto 0x521aadc // likely
    x0 = 0
    x1 = 0
    x2 = x5
    x3 = x6
    x29 = [sp -8]
    x30 = [sp] + 0x40
    w6 = (byte) [sp + 0x2c]
    w2 = (byte) [sp + 0x2d]
    w3 = (byte) [sp + 0x2e]
    x0 = [sp + 0x48]
    x5 = x1
    x1 = x6
    0x521a630 (x0, x1, x2, x3)
    x29 = [sp -8]
    x30 = [sp] + 0x40
    [sp + 8] = x0
    x3 = x30
    0x4da15a0 (x0, x1, x2, x3)
    x0 = [sp + 8]
    goto 0x521aa80
    x16 = [x28 + 0x10]
    x17 = sp - 0x60
    v = x17 - x16 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x521ae70 // likely
    [sp - 0xe0]! = x30
    [sp -8] = x29
    x29 = sp - 8
    [sp + 0xf0] = (byte) w1
    [sp + 0xf1] = (byte) w2
    [sp + 0xf2] = (byte) w3
    [sp + 0xf4] = (w4, 2)
    w4 = (byte) [x0 + 0x3c]
    [x0 + 0x3c] = (byte) w2
    w2 = (byte) [x0 + 0xa9] // (pstr 0x00002000) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    if (w2 != 0) goto 0x521ae5c // unlikely
    [sp + 0xe8] = x0
    w2 = [x0 + 0x38] // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    if (!w2) goto 0x521ad64 // unlikely
    w6 = (byte) [sp + 0xf1]
    v = w6 - 9    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x521ac50 // likely
    v = w5 - w2   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x521ad70 // unlikely
    x1 = sp + 0x78
    stp 0, 0, [x1]
    [x1 + 0x10] = (0, 2)
    w0 = [sp + 0xf8]
    0x4d2e350 (x0, 0x10077fd8, 0x400010, x3)
    x1 = 0x856f6d0
    [sp + 0x78] = (x1, 2)
    x1 = [sp + 0xe8]
    w0 = [x1 + 0x38] // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    0x4d2e350 (0x400010, x1, 0x400010, x3)
    x1 = 0x856f6d0
    [sp + 0x88] = (x1, 2)
    x0 = 0x4459923
    x1 = 0x32     // '2'
    x2 = sp + 0x78
    x3 = 2
    x4 = x3
    0x4e34a60 (0x4459923, 0x32, 0x10077fd8, 0x2)
    [sp + 0x40] = x1
    [sp + 0x58] = x0
    x0 = 0x10
    x1 = 0x8757890
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32500 (0x10, 0x8757890, 0x1, 0x2)
    x1 = [sp + 0x40]
    [x0 + 8] = x1
    x27 = 0xb48f000
    w1 = [x27 + 0x450]
    if (w1) goto 0x521ac0c // unlikely
    x2 = [sp + 0xe8]
    x4 = [sp + 0x58] // (pstr 0x04459923) "got CONTINUATION for stream %d; expected stream %d/exa.index_pb"
    goto 0x521ac24
    x2 = [sp + 0xe8]
    x3 = [x2 + 0x28] // elf_shdr
    0x4da3510 (0x10, x1, x2, 0xace39d8)
    x4 = [sp + 0x58] // (pstr 0x04459923) "got CONTINUATION for stream %d; expected stream %d/exa.index_pb"
    stp x4, x0, [x25]
    [x25 + 0x10] = x3
    [x0] = x4
    x3 = 0x8f07508
    [x2 + 0x20] = (x3, 2)
    x0 = 0x8f26000
    return x0;
}

