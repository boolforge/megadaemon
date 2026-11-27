// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f88 (void) {
    0x4d98880 (x0, x1, x2, x3)
    x1 = 0x856f550
    x3 = [sp + 0xe0]
    [x3] = x1
    x27 = 0xb48f000
    w1 = [x27 + 0x450]
    if (!w1) goto 0x6defc14 // likely
    x6 = [x3 + 8]
    0x4da3500 (x0, x1, x2, x3)
    stp x0, x6, [x25]
    [x3 + 8] = x0
    x6 = [sp + 0x148]
    (x6, 2) = 3   // "ki/codeium_common_pb/codeium_common_go_proto.SearchResultType.S" // "ogle3/third_party/jetski/codeium_common_pb/codeium_common_go_pr"
    x6 = [x6 + 0x50] // "Descriptor"
    x1 = 0x43fc73c
    x2 = 0x20
    x4 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x5 = x4       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    callreg x6    // 0x7470697263736544(0x16236a0, 0x43fc73c, 0x20, 0x0)
    x6 = [sp + 0xb8]
    (x1, 2) = 3   // elf_phdr
    if (!x7) goto 0x6defc8c // unlikely
    x8 = [sp + 0x138]
    (x0, 2) = 3   // segment.GNU_RELRO
    v = x7 - x2   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x6defc90 // likely
    0x4d1a430 (0x799b960, 0x400000006, 0x799b960, x3)
    if (w0 != 0) goto 0x6defc6c // unlikely
    x6 = [sp + 0xb8]
    x8 = [sp + 0x138]
    goto 0x6defc90
    (x0, 2) = 3
    (x2, 2) = 3
    (x4, 2) = 3
    x6 = [sp + 0x148]
    0x6deffc0 (x0, x1, x2, x3)
    (x29, 2) = 3
    sp += 0x110
    x8 = [sp + 0x138]
    x7 = [x8 + 0x138] // segment.GNU_RELRO
    if (!x7) goto 0x6defd3c // unlikely
    x0 = 0x10
    x1 = 0x81a54f8
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32500 (0x10, 0x81a54f8, 0x1, x3)
    [sp + 0xe0] = x0
    x3 = [sp + 0x138]
    (x0, 2) = 3   // segment.GNU_RELRO
    x2 = [x3 + 0x140]
    0x4d98920 (0x799b960, 0x799b960, 0x90, x3)
    x3 = 0x81df050
    x4 = [sp + 0xe0]
    [x4] = x3
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (!w3) goto 0x6defce8 // likely
    x6 = [x4 + 8] // section..text
    0x4da3500 (0x799b960, 0x799b960, 0x90, x3)
    stp x0, x6, [x25]
    [x4 + 8] = x0
    x6 = [sp + 0x148]
    (x6, 2) = 3   // "ki/codeium_common_pb/codeium_common_go_proto.SearchResultType.S" // "ogle3/third_party/jetski/codeium_common_pb/codeium_common_go_pr"
    x6 = [x6 + 0x50] // "Descriptor"
    x1 = 0x440ab65
    x2 = 0x22     // '\"'
    x3 = x4
    x4 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x5 = x4       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    callreg x6    // 0x7470697263736544(0x16236a0, 0x440ab65, 0x22, 0x10)
    x6 = [sp + 0x148]
    (x6, 2) = 3   // "ki/codeium_common_pb/codeium_common_go_proto.SearchResultType.S" // "ogle3/third_party/jetski/codeium_common_pb/codeium_common_go_pr"
    x6 = [x6 + 0x18] // "/codeium_common_pb/codeium_common_go_proto.FilterReason.Descrip"
    x7 = [sp + 0x138]
    (x1, 2) = 3   // segment.GNU_RELRO
    x3 = [x7 + 0x140]
    callreg x6    // 0x6d756965646f632f(0x16236a0, 0x799b960, 0x799b960, 0x90)
    if (x0) goto 0x6defdc8 // likely
    x6 = [sp + 0xb8]
    x8 = [sp + 0x138]
    x7 = [sp + 0x148]
    x9 = [x7 + 0x108] // (pstr 0x01622c30) "ogle3/third_party/jetski/codeium_common_pb/codeium_common_go_pr"
    if (!x9) goto 0x6defd74 // unlikely
    (x9, 2) = 3   // elf_phdr
    x11 = [x6 + 0x50] // elf_phdr
    [x8 + 0x138] = (x10, 2)
    x27 = 0xb48f000
    w10 = [x27 + 0x450]
    if (!w10) goto 0x6defd6c // likely
    x10 = [x8 + 0x130] // segment.GNU_RELRO
    0x4da3500 (0x16236a0, 0x799b960, 0x799b960, 0x90)
    stp x9, x10, [x25]
    [x8 + 0x130] = x9
    goto 0x6defd9c
    [x8 + 0x138] = (0, 2)
    x27 = 0xb48f000
    w9 = [x27 + 0x450]
    if (!w9) goto 0x6defd90 // likely
    x9 = [x8 + 0x130] // segment.GNU_RELRO
    0x4da34f0 (0x16236a0, 0x799b960, 0x799b960, 0x90)
    [x25] = x9
    x9 = section..noptrbss
    [x8 + 0x130] = x9
    [x8 + 0xb0] = (0, 2)
    x27 = 0xb48f000
    w9 = [x27 + 0x450]
    if (!w9) goto 0x6defdb8 // likely
    x9 = [x8 + 0xa8] // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    0x4da34f0 (0x16236a0, 0x799b960, 0x799b960, 0x90)
    [x25] = x9
    x9 = section..noptrbss
    [x8 + 0xa8] = x9
    goto 0x6def578
    [sp + 0xd0] = x1
    [sp + 0x88] = x0
    x0 = 2
    x1 = 0x89932e0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33a20 (0x2, 0x89932e0, 0x1, 0x90)
    x3 = 0x5002   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    [x0] = (half) w3
    x1 = x0
    x2 = [sp + 0x88] // (pstr 0x016236a0) "ki/codeium_common_pb/codeium_common_go_proto.SearchResultType.S"
    x3 = [sp + 0xd0] // segment.GNU_RELRO
    x0 = 0
    (x29, 2) = 3
    sp += 0x110
    x0 = 2
    x1 = 0x89932e0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33a20 (0x2, 0x89932e0, 0x1, 0x799b960)
    x3 = 0x4702   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    [x0] = (half) w3
    x27 = 0x948d000
    x3 = [x27 + 0x498]
    x1 = x0
    x2 = 0x8f35c88
    x0 = 0
    (x29, 2) = 3
    sp += 0x110
    x0 = 2
    x1 = 0x89932e0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33a20 (0x2, 0x89932e0, 0x1, x3)
    x3 = 0x4702   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    [x0] = (half) w3
    x27 = 0x948d000
    x3 = [x27 + 0x458]
    x1 = x0
    x2 = 0x8f35c88
    x0 = 0
    (x29, 2) = 3
    sp += 0x110
    0x4da37d0 (x0, 0x2, 0x8f35c88, x3)
    x14 += 1      // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    v = x14 - x11 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v >= 0) goto 0x6defed0 // unlikely
    ldrh w0, [x8, x14, lsl 1] // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x15 = 0
    goto 0x6defeac
    x15 += 1      // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    v = x15 - x13 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v >= 0) goto 0x6defec4 // likely
    ldrh w16, [x12, x15, lsl 1] // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    v = w0 - w16  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x6defea8 // unlikely
    goto 0x6defec8
    x15 = -1
    if (x15 != 0x3f) goto 0x6defe8c // likely
    goto 0x6deff10
    x0 = 2
    x1 = 0x89932e0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33a20 (0x2, 0x89932e0, 0x1, x3)
    x3 = 0x2f02   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    [x0] = (half) w3
    x27 = 0x948d000
    x3 = [x27 + 0x470]
    x1 = x0
    x2 = 0x8f35c88
    x0 = 0
    (x29, 2) = 3
    sp += 0x110
    [sp + 0xb0] = x9
    [sp + 0x98] = x5
    [sp + 0xd8] = x3
    x1 = x7 + 0xd8
    [sp + 0xe8] = x1
    0x4d2e2d0 (x0, 0xd8, 0x8f35c88, x3)
    x1 = 0x8579c50
    x2 = x0
    x0 = [sp + 0xe8]
    0x4dab5e0 (0xd8, 0x8579c50, x2, x3)
    x1 = [sp + 0xb0] // section..noptrbss
    (x2, 2) = 3
    x1 = [x1 + 0x28]
    x4 = [sp + 0x138]
    [x4 + 0xf0] = (x3, 2)
    x27 = 0xb48f000
    w1 = [x27 + 0x450]
    if (!w1) goto 0x6deff6c // likely
    x3 = [x4 + 0xe8]
    0x4da3500 (0xd8, x1, x2, 0x600000001)
    stp x2, x3, [x25]
    [x4 + 0xe8] = x2
    x3 = [sp + 0xd8]
    x4 = [sp + 0xb8]
    x5 = [sp + 0x98] // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x6 = [sp + 0x148]
    x7 = [sp + 0x138]
    x10 = 0x8f61568
    goto 0x6def804
    [sp + 8] = (x0, 2)
    [sp + 0x18] = (x2, 2)
    [sp + 0x28] = (x4, 2)
    [sp + 0x38] = x6
    x3 = x30
    0x4da15a0 (0xd8, x1, x2, 0x6deff68)
    (x0, 2) = 3
    (x2, 2) = 3
    (x4, 2) = 3   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x6 = [sp + 0x38]
    goto 0x6def280
    x16 = [x28 + 0x10]
    x17 = sp - 0x60
    v = x17 - x16 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x6df0360 // likely
    [sp - 0xe0]! = x30
    [sp -8] = x29
    x29 = sp - 8
    [sp + 0x108] = (x4, 2)
    [sp + 0x118] = x6
    [sp + 0xf8] = (x2, 2)
    [sp + 0xe8] = (x0, 2)
    x0 = x4
    0x6dfaef0 (x0, x1, x2, x3)
    if (x0) goto 0x6df0320 // unlikely
    x3 = [[sp + 0xf8] + 0x18]
    x0 = [sp + 0x100]
    (x1, 2) = 3
    callreg x3    // section..text(0x0, 0xd8, 0x0, 0x4d0f000)
    if (x0) goto 0x6df02e0 // unlikely
    [sp + 0x84] = 0
    x7 = 0x14
    [sp + 0x82] = (byte) w7
    x7 = [sp + 0x118]
    w7 = (word) [x7 + 0x1a8] // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x7 += 1
    [sp + 0x84] = (half) w7
    [sp + 0x86] = (byte) 0
    [sp + 0x87] = (byte) 0
    x7 = [sp + 0x108]
    x8 = [x7 + 0x1c0]
    x1 = x8 + 1
    (x2, 2) = 3
    x0 = [sp + 0x110] // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = sp + 0x82
    x5 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x6 = x5       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x6df7fe0 (0x1, 0x9, x2, x3)
    if (w2 == 0) goto 0x6df02c4 // likely
    x0 = 0x83180a0
    x2 = 0x4a12468
    0x4d16880 (0x83180a0, 0x9, 0x4a12468, x3)
    (x3, 2) = 3
    x5 = 0x8f61358
    v = x3 - x5   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x6df028c // likely
    [sp + 0xd0] = x4
    return x0;
}

