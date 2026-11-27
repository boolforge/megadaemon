// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f14 (void) {
    x27 = sp + 0x6c8
    (x7, 2) = 3
    v = x8 - 4    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x527cf00 // likely
    w9 = [x7]
    x10 = 0x70747468
    v = w9 - w10  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x527cf00 // likely
    x1 = [sp + 0x2a0]
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [x1 + 0xa0] = (byte) w2
    x0 = sp + 0x6c0
    0x5278fe0 (0x100786c0, x1, 0x1, x3)
    if (!x1) goto 0x527d020 // likely
    [sp + 0x148] = x1
    [sp + 0x2b0] = x0
    x0 = 0x18
    x1 = 0x87f4218
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32830 (0x18, 0x87f4218, 0x1, x3)
    x3 = 0x5287d70
    [x0] = x3
    x3 = [sp + 0x148]
    [x0 + 0x10] = x3
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (w3) goto 0x527cec8 // unlikely
    x7 = [sp + 0x2b0]
    goto 0x527ced4
    0x4da34f0 (0x18, 0x87f4218, 0x1, x3)
    x7 = [sp + 0x2b0]
    [x25] = x7
    [x0 + 8] = x7
    x7 = [sp + 0x2a0]
    x27 = (byte) [x7]
    x27 = 0xb48f000
    w8 = [x27 + 0x450]
    if (!w8) goto 0x527cef8 // likely
    x8 = [x7 + 0x120]
    0x4da3500 (0x18, 0x87f4218, 0x1, x3)
    stp x0, x8, [x25]
    [x7 + 0x120] = x0
    goto 0x527d020
    v = x8 - 5    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x527d020 // likely
    w8 = [x7]
    x9 = 0x70747468
    v = w8 - w9   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x527d020 // likely
    w7 = (byte) [x7 + 4]
    v = w7 - 0x73 // 's' // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x527d020 // likely
    x5 = [sp + 0x2a0]
    (x5, 2) = 3   // elf_phdr
    [sp + 0x1b8] = x5
    [sp + 0x350] = x6
    x7 = [sp + 0x6a8]
    (x8, 2) = 3   // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    if (!x26) goto 0x527cf64 // unlikely
    x5 = [x26]
    x27 = sp + 0x6d8
    (x3, 2) = 3
    x0 = [sp + 0x200]
    x1 = [sp + 0x3b8]
    callreg x5    // 0xffffffffffffffff(0x0, 0x0, 0x1, 0x0)
    if (x1) goto 0x527cfd8 // unlikely
    x8 = x0
    if (x8) goto 0x527cf70 // unlikely
    0x4d98f80 (x0, x1, 0x1, x3)
    x8 = x0
    [sp + 0x2c8] = x8
    x0 = sp + 0x6c0
    0x5278fe0 (0x100786c0, x1, 0x1, x3)
    if (!x1) goto 0x527cfd0 // likely
    [sp + 0x140] = x1
    [sp + 0x2a8] = x0
    x2 = [sp + 0x2c8]
    if (x2) goto 0x527cf98 // unlikely
    x2 = 0
    goto 0x527e3e0
    x3 = sp + 0x5e8
    stp 0, 0, [x3]
    [x3 + 0x10] = (0, 2)
    [x3 + 0x20] = (0, 2)
    return x0;
}

