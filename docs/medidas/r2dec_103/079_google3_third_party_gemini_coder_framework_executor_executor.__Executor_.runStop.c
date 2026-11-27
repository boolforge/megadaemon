// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f79 (void) {
    x7 = 0
    [sp + 0x120] = x8
    x2 = [x7 + 0x110] // (pstr 0x016236a0) "ki/codeium_common_pb/codeium_common_go_proto.SearchResultType.S"
    (x7, 2) = 3   // "ogle3/third_party/jetski/codeium_common_pb/codeium_common_go_pr" // segment.GNU_RELRO
    x1 = x9 + x4  // "ogle3/third_party/jetski/codeium_common_pb/codeium_common_go_pr"
    v = x2 - x1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x67b398c // likely
    [sp + 0xd0] = (x9, 2)
    [sp + 0x1c0] = x3
    x0 = x7       // segment.GNU_RELRO
    x3 = x4
    x4 = 0x7e1b238
    0x4d9e050 (0x799b960, 0x1622c30, 0x16236a0, x3)
    x3 = [sp + 0x1c0]
    (x9, 2) = 3   // "ogle3/third_party/jetski/codeium_common_pb/codeium_common_go_pr"
    x7 = x0       // segment.GNU_RELRO
    [sp + 0xd0] = (x1, 2)
    [sp + 0x1c0] = x7
    x5 = x1 - x9
    x6 = x9 & (x9 - x2)
    add x1, x7, x6, lsl 3
    x0 = 0x7e1b238
    x2 = x5
    0x4d99570 (0x7e1b238, 0x12ab1ae0, x2, x3)
    (x5, 2) = 3   // "ki/codeium_common_pb/codeium_common_go_proto.SearchResultType.S" // "ogle3/third_party/jetski/codeium_common_pb/codeium_common_go_pr"
    x7 = [sp + 0x120]
    [x7 + 0x108] = (x5, 2)
    x27 = 0xb48f000
    w5 = [x27 + 0x450]
    if (w5) goto 0x67b39d4 // unlikely
    x5 = [sp + 0x1c0] // segment.GNU_RELRO
    goto 0x67b39e4
    x4 = [x7 + 0x100] // segment.GNU_RELRO
    0x4da3500 (0x7e1b238, 0x12ab1ae0, x2, x3)
    x5 = [sp + 0x1c0] // segment.GNU_RELRO
    stp x5, x4, [x25]
    [x7 + 0x100] = x5
    x0 = [sp + 0x278]
    x3 = [x0 + 0x20] // elf_phdr
    x1 = [sp + 0x280]
    x2 = [sp + 0x288]
    0x67b47a0 (x0, x1, x2, 0x40)
    x0 = [sp + 0x278]
    x5 = [sp + 0x178]
    x11 = 0x8f080c8
    x7 = [x5 + 0x148] // (pstr 0x00000458) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    if (!x7) goto 0x67b3abc // unlikely
    x4 = [x0 + 0x20] // elf_phdr
    if (!x4) goto 0x67b3a24 // unlikely
    x4 = [x4 + 0x30]
    goto 0x67b3a28
    x4 = 0
    if (x4) goto 0x67b3a70 // unlikely
    x0 = 0x30     // '0'
    x1 = 0x8ac84b0
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d32f40 (0x30, 0x8ac84b0, 0x1, 0x40)
    x3 = [sp + 0x278]
    x4 = [x3 + 0x20] // elf_phdr
    x27 = (byte) [x4] // elf_phdr
    x27 = 0xb48f000
    w5 = [x27 + 0x450]
    if (!w5) goto 0x67b3a64 // likely
    x5 = [x4 + 0x30]
    0x4da3500 (0x30, 0x8ac84b0, 0x1, x3)
    stp x0, x5, [x25]
    [x4 + 0x30] = x0
    x0 = x3
    x5 = [sp + 0x178]
    x4 = [x0 + 0x20] // elf_phdr
    if (!x4) goto 0x67b3a80 // unlikely
    x1 = [x4 + 0x30]
    goto 0x67b3a84
    x1 = 0
    x3 = [x5 + 0x148] // (pstr 0x00000458) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x0 = 0x8f254c8
    x2 = x0
    0x4f97120 (0x8f254c8, x1, 0x8f254c8, 0x458)
    x0 = [sp + 0x278]
    x3 = [x0 + 0x20] // elf_phdr
    x1 = [sp + 0x280]
    x2 = [sp + 0x288]
    0x67b47a0 (x0, x1, x2, 0x40)
    x0 = [sp + 0x278]
    x5 = [sp + 0x178]
    x11 = 0x8f080c8
    x7 = [x5 + 0x68]
    if (!x7) goto 0x67b3bf4 // unlikely
    x7 = [x0 + 0x20] // elf_phdr
    if (!x7) goto 0x67b3ae8 // unlikely
    (x8, 2) = 3
    v = x8 - x11  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x67b3adc // unlikely
    x9 = 0
    if (v) goto 0x67b3ae8 // likely
    x8 = [x9]
    goto 0x67b3aec
    x8 = 0
    w9 = (byte) [x5 + 0x94]
    [x8 + 0x70] = (byte) w9
    if (!x7) goto 0x67b3b14 // unlikely
    (x8, 2) = 3
    v = x8 - x11  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x67b3b08 // unlikely
    x9 = 0
    if (v) goto 0x67b3b14 // likely
    x8 = [x9]
    goto 0x67b3b18
    x8 = 0
    if (!x7) goto 0x67b3b38 // unlikely
    (x7, 2) = 3
    v = x7 - x11  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x67b3b2c // unlikely
    x9 = 0
    if (v) goto 0x67b3b38 // likely
    x7 = [x9]
    goto 0x67b3b3c
    x7 = 0
    [sp + 0x118] = x8
    x2 = [x7 + 0x68]
    (x7, 2) = 3   // elf_phdr
    (x1, 2) = 3
    x10 = x9 + x3 // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    v = x2 - x10  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x67b3b84 // unlikely
    [sp + 0xd0] = (x9, 2)
    [sp + 0x1b8] = x1
    x0 = x7       // elf_phdr
    x1 = x10      // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x4 = 0x856f5d0
    0x4d9e050 (0x40, 0x700, 0x380, 0x380)
    (x9, 2) = 3
    x7 = x0       // elf_phdr
    x10 = x1      // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x1 = [sp + 0x1b8]
    [sp + 0xd8] = x10
    [sp + 0x1b8] = x7
    [sp + 0xd0] = x2
    x0 = x7 + x9  // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x2 = x3
    0x4da39e0 (0x3c0, 0x380, 0x380, 0x380)
    (x4, 2) = 3   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x5 = [sp + 0x118]
    [x5 + 0x60] = (x3, 2)
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (w3) goto 0x67b3bbc // unlikely
    x6 = [sp + 0x1b8] // elf_phdr
    goto 0x67b3bcc
    x4 = [x5 + 0x58] // elf_phdr
    0x4da3500 (0x3c0, 0x380, 0x380, x3)
    x6 = [sp + 0x1b8] // elf_phdr
    stp x6, x4, [x25]
    [x5 + 0x58] = x6
    x0 = [sp + 0x278]
    x3 = [x0 + 0x20] // elf_phdr
    x1 = [sp + 0x280]
    x2 = [sp + 0x288]
    0x67b47a0 (x0, x1, x2, 0x40)
    x0 = [sp + 0x278]
    x5 = [sp + 0x178]
    x11 = 0x8f080c8
    x7 = [x5 + 0x98]
    if (!w7) goto 0x67b3c84 // unlikely
    x3 = [x0 + 0x20] // elf_phdr
    if (!x3) goto 0x67b3c20 // unlikely
    (x4, 2) = 3
    return x0;
}

