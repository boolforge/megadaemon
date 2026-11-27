// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f85 (void) {
    x2 = [sp + 0x80]
    x1 = (ne)? x2 : x1
    x2 = [sp + 0x110]
    x0 = (ne)? x2 : x0
    0x4d98880 (x0, x1, x2, x3)
    x2 = 0x856f550
    [sp + 0x1b0] = (x2, 2)
    x2 = [sp + 0x130]
    x0 = [x2 + 0x68]
    0x4d2e350 (0x380, x1, x2, x3)
    x2 = 0x8884338
    [sp + 0x1c0] = (x2, 2)
    x0 = 0x4290ef2
    x1 = 9
    x2 = sp + 0x1b0
    x3 = 2
    x4 = x3
    0x4e34a60 (0x4290ef2, 0x9, 0x100781b0, 0x2)
    (x3, 2) = 3
    v = x2 - x3   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x67cacc0 // unlikely
    x4 = [sp + 0x1d0]
    goto 0x67cacf0
    [sp + 0xd0] = x1
    [sp + 0x1e8] = x0
    x0 = [sp + 0x1d0]
    x1 = x3
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x856f550
    0x4d9e050 (x0, x1, x2, 0x1)
    x3 = x1
    x4 = x0
    x0 = [sp + 0x1e8] // (pstr 0x04290ef2) "- %s (%s)committerinsteadOfremainingno change_defer_fkForwarded"
    x1 = [sp + 0xd0]
    x5 = x3 - 1
    add x5, x4, x5, lsl 4
    [x5 + 8] = x1
    x27 = 0xb48f000
    w6 = [x27 + 0x450]
    if (!w6) goto 0x67cabec // likely
    x6 = [x5]
    0x4da3500 (0x4290ef2, 0x9, x2, x3)
    stp x0, x6, [x25]
    goto 0x67cabec
    x0 = [sp + 0x258]
    x1 = [sp + 0x108]
    x3 = [sp + 0x200]
    goto 0x67ca49c
    [sp + 8] = (x0, 2)
    [sp + 0x18] = (x2, 2)
    x3 = x30
    0x4da15a0 (x0, x1, x2, 0x67cad10)
    (x0, 2) = 3
    (x2, 2) = 3
    goto 0x67ca2b0
    x16 = [x28 + 0x10]
    v = sp - x16  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x67cadac // likely
    [sp - 0x60]! = x30
    [sp -8] = x29
    x29 = sp - 8
    [sp + 0x68] = x0
    x6 = 0x67c09f0
    x7 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    [sp + 0x38] = (x6, 2)
    x6 = 0x8f9a6c8
    x7 = sp + 0x38
    [sp + 0x48] = (x6, 2)
    x3 = sp + 0x48
    x4 = 2
    x5 = x4
    0x67c02a0 (x0, x1, x2, 0x10077fe8)
    0x67c00c0 (x0, x1, x2, 0x10077fe8)
    x29 = [sp -8]
    x30 = [sp] + 0x60
    [sp + 8] = (x0, 2)
    [sp + 0x18] = x2
    x3 = x30
    0x4da15a0 (x0, x1, x2, 0x67cad38)
    (x0, 2) = 3
    x2 = [sp + 0x18]
    goto 0x67cad50
    x16 = [x28 + 0x10]
    x17 = sp - 0x20
    v = x17 - x16 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x67caf00 // likely
    [sp - 0xa0]! = x30
    [sp -8] = x29
    x29 = sp - 8
    [sp + 0xa8] = x0
    (x0, 2) = 3
    [sp + 0x38] = (x1, 2)
    0x54ca100 (x0, 0x100b70003, x2, 0x67cad38)
    x3 = [sp + 0xa8]
    (w4, 2) = 3   // section..text
    v = x2 - 0    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = [sp + 0x38]
    x1 = (ne)? x2 : x1
    x2 = [sp + 0x40]
    x0 = (ne)? x2 : x0
    v = w4 - w5   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x67cae80 // unlikely
    x2 = sp + 0x78
    stp 0, 0, [x2]
    [x2 + 0x10] = (0, 2)
    0x4d98880 (x0, 0x100b70003, 0x10077fd8, x3)
    x2 = 0x856f550
    [sp + 0x78] = (x2, 2)
    x2 = [sp + 0xa8]
    w2 = [x2 + 0x18] // section..text
    x0 = x2 + 1
    0x4d2e350 (0x4d0f001, 0x100b70003, 0x4d0f000, x3)
    x2 = 0x856f6d0
    [sp + 0x88] = (x2, 2)
    x0 = 0x4254510
    x1 = 6
    x2 = sp + 0x78
    x3 = 2
    x4 = x3
    0x4e34a60 (0x4254510, 0x6, 0x10077fd8, 0x2)
    x29 = [sp -8]
    x30 = [sp] + 0xa0
    x2 = sp + 0x48
    stp 0, 0, [x2]
    [x2 + 0x10] = (0, 2)
    [x2 + 0x20] = (0, 2)
    0x4d98880 (0x4254510, 0x6, 0x10078048, 0x2)
    x2 = 0x856f550
    [sp + 0x48] = (x2, 2)
    x2 = [sp + 0xa8]
    w2 = [x2 + 0x18] // section..text
    x0 = x2 + 1
    0x4d2e350 (0x4d0f001, 0x6, 0x4d0f000, 0x2)
    x2 = 0x856f6d0
    [sp + 0x58] = (x2, 2)
    x2 = [sp + 0xa8]
    w2 = [x2 + 0x1c]
    x0 = x2 + 1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d2e350 (0x1, 0x6, x2, 0x2)
    x2 = 0x856f6d0
    [sp + 0x68] = (x2, 2)
    x0 = 0x42a6bbd
    x1 = 0xa
    x2 = sp + 0x48
    x3 = 3
    x4 = x3
    0x4e34a60 (0x42a6bbd, 0xa, 0x10078048, 0x3)
    x29 = [sp -8]
    x30 = [sp] + 0xa0
    [sp + 8] = x0
    x3 = x30
    0x4da15a0 (0x42a6bbd, 0xa, 0x10078048, x3)
    x0 = [sp + 8] // (pstr 0x042a6bbd) "%s:L%d-L%ddockerfilejavascripttypescriptRewriteUrienable-arkena"
    goto 0x67cadd0
    x16 = [x28 + 0x10]
    x17 = sp - 0xa0
    v = x17 - x16 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x67cb20c // likely
    x20 = sp - 0x120
    [x20 - 8] = (x29, 2)
    sp = x20
    x29 = sp - 8
    [sp + 0x128] = x0 // (pstr 0x042a6bbd) "%s:L%d-L%ddockerfilejavascripttypescriptRewriteUrienable-arkena"
    if (!x0) goto 0x67caf50 // unlikely
    x1 = [x0 + 0x28] // "RewriteUrienable-arkenable-ptystep_indexsearch_webwrite_fileLC_"
    goto 0x67caf54
    x1 = 0
    x2 = sp + 0xb8
    stp 0, 0, [x2]
    [x2 + 0x10] = (0, 2)
    [x2 + 0x20] = (0, 2)
    [x2 + 0x30] = (0, 2)
    [x2 + 0x40] = (0, 2)
    [x2 + 0x50] = (0, 2)
    x0 = 0x83334c8
    0x4d35800 (0x83334c8, x1, 0x10078038, x3)
    x0 = 0
    x1 = 0
    x2 = 0
    x3 = 0
    goto 0x67cafa8
    [x5] = x0
    x0 = sp + 0xb8
    0x4d35870 (0x10078038, x1, x2, x3)
    w0 = (byte) [sp + 0x5f]
    (x1, 2) = 3
    x3 = [sp + 0x90]
    [sp + 0x70] = (x2, 2)
    [sp + 0x90] = x3
    (x4, 2) = 3
    if (!x4) goto 0x67cb09c // likely
    [sp + 0x5f] = (byte) w0
    (x0, 2) = 3
    0x54c9170 (0x10102464c457f, x1, x2, x3)
    if (x2) goto 0x67cb07c // unlikely
    x5 = [sp + 0x70]
    x6 = x5 + 1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x2 = [sp + 0x78]
    v = x2 - x6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x67cafe8 // unlikely
    x3 = [sp + 0x90]
    w4 = (byte) [sp + 0x5f]
    goto 0x67cb048
    [sp + 0x78] = x1
    [sp + 0x88] = x0
    v = x6 - 2    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v > 0) goto 0x67cb01c // unlikely
    w3 = (byte) [sp + 0x5f]
    if (w3 != 0) goto 0x67cb01c // unlikely
    if (x5) goto 0x67cb01c // unlikely
    x3 = sp + 0x98
    stp 0, 0, [x3]
    [x3 + 0x10] = (0, 2)
    x2 = 2
    x4 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    goto 0x67cb048
    x0 = [sp + 0x90]
    x1 = x6       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x8c965c0
    0x4d9e050 (x0, 0x1, 0x2, 0x1)
    x6 = x1       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x3 = x0
    w4 = (byte) [sp + 0x5f]
    x0 = [sp + 0x88]
    x1 = [sp + 0x78]
    [sp + 0x90] = x3
    [sp + 0x70] = (x2, 2)
    [sp + 0x5f] = (byte) w4
    x5 = x6 - 1
    add x5, x3, x5, lsl 4
    [x5 + 8] = x1
    x27 = 0xb48f000
    w1 = [x27 + 0x450]
    if (!w1) goto 0x67caf90 // likely
    x1 = [x5]
    0x4da3500 (0x10102464c457f, 0x10102464c457f, 0x2, x3)
    stp x0, x1, [x25]
    goto 0x67caf90
    x0 = 0
    x1 = 0
    x4 = x3
    x3 = x2
    x2 = 0
    (x29, 2) = 3
    sp += 0x120
    x0 = [sp + 0x128]
    if (!x0) goto 0x67cb0ac // unlikely
    x4 = [x0 + 0x30] // (pstr 0x00000020) "@"
    goto 0x67cb0b0
    x4 = 0
    x2 = sp + 0xb8
    stp 0, 0, [x2]
    [x2 + 0x10] = (0, 2)
    [x2 + 0x20] = (0, 2)
    [x2 + 0x30] = (0, 2)
    [x2 + 0x40] = (0, 2)
    [x2 + 0x50] = (0, 2)
    x0 = 0x83334c8
    x1 = x4
    0x4d35800 (0x83334c8, x1, 0x10078158, 0x2)
    x0 = [sp + 0x90]
    (x2, 2) = 3
    goto 0x67cb0fc
    [x4] = x0
    x0 = sp + 0xb8
    0x4d35870 (0x10078158, x1, x2, 0x2)
    x0 = [sp + 0x90]
    (x1, 2) = 3
    (x5, 2) = 3
    if (!x5) goto 0x67cb1b4 // likely
    [sp + 0x60] = (x2, 2)
    [sp + 0x80] = x0
    (x0, 2) = 3
    0x54c9170 (0x10102464c457f, x1, x2, 0x2)
    if (x2) goto 0x67cb194 // unlikely
    x5 = [sp + 0x60] + 1
    x2 = [sp + 0x68]
    v = x2 - x5   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x67cb134 // likely
    x3 = [sp + 0x80]
    goto 0x67cb164
    [sp + 0x78] = x1
    [sp + 0x88] = x0
    x0 = [sp + 0x80]
    x1 = x5       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x8c965c0
    0x4d9e050 (x0, 0x1, x2, 0x1)
    x5 = x1       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x3 = x0
    x0 = [sp + 0x88]
    x1 = [sp + 0x78]
    [sp + 0x90] = x3
    [sp + 0x70] = (x2, 2)
    x4 = x5 - 1
    add x4, x3, x4, lsl 4
    [x4 + 8] = x1
    x27 = 0xb48f000
    w1 = [x27 + 0x450]
    if (!w1) goto 0x67cb0e8 // likely
    x1 = [x4]
    0x4da3500 (0x10102464c457f, 0x10102464c457f, x2, x3)
    stp x0, x1, [x25]
    goto 0x67cb0e8
    x0 = 0
    x1 = 0
    x4 = x3
    x3 = x2
    x2 = 0
    (x29, 2) = 3
    sp += 0x120
    x5 = sp + 0x120
    x6 = sp
    x5 -= x6
    x7 = x0
    x6 = x7 - x6
    v = x6 - x5   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x67cb1ec // likely
    x1 = x0
    x0 = 0x8c965c0
    0x4d79b60 (0x8c965c0, x1, x2, x3)
    x3 = x2
    x2 = x1
    x1 = x3
    x3 = 0
    x4 = 0
    x5 = x1
    x1 = x2
    x2 = x5
    (x29, 2) = 3
    sp += 0x120
    [sp + 8] = x0
    x3 = x30
    0x4da15a0 (0x8c965c0, x1, x2, 0x8884338)
    x0 = [sp + 8]
    goto 0x67caf20
    x16 = [x28 + 0x10]
    v = sp - x16  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x67cb2d4 // likely
    [sp - 0x20]! = x30
    [sp -8] = x29
    x29 = sp - 8
    [sp + 0x28] = x0
    0x67caf20 (0x8c965c0, x1, x2, 0x8884338)
    if (x3) goto 0x67cb254 // likely
    if (!x1) goto 0x67cb254 // likely
    (x2, 2) = 3
    x4 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    goto 0x67cb27c
    x0 = 0x8f54b28
    x1 = [sp + 0x28]
    0x67cb2f0 (0x8f54b28, 0x8c965c0, 0x10, 0x8)
    x29 = [sp -8]
    x30 = [sp] + 0x20
    x4 += 1
    x3 = x6
    x2 = x5
    v = x1 - x4   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v <= 0) goto 0x67cb2c0 // unlikely
    add x5, x0, x4, lsl 4
    (x5, 2) = 3
    v = x3 - x6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v <= 0) goto 0x67cb29c // likely
    x7 = -1
    goto 0x67cb2ac
    if (v >= 0) goto 0x67cb2a8 // unlikely
    x7 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    goto 0x67cb2ac
    x7 = 0
    v = x7 - 0    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v > 0) goto 0x67cb270 // unlikely
    x6 = x3
    x5 = x2
    goto 0x67cb270
    x0 = x2
    x1 = x3
    x29 = [sp -8]
    x30 = [sp] + 0x20
    [sp + 8] = x0
    x3 = x30
    0x4da15a0 (x0, 0xffffffffeff87e40, x2, x3)
    x0 = [sp + 8]
    goto 0x67cb220
    x16 = [x28 + 0x10]
    v = sp - x16  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x67cb398 // likely
    [sp - 0x20]! = x30
    [sp -8] = x29
    x29 = sp - 8
    return x0;
}

