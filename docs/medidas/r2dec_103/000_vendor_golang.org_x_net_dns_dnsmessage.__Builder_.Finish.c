// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f0 (void) {
    (x0, 2) = 3
    x2 = [sp + 0x18]
    w3 = (byte) [sp + 0x20]
    goto 0x4ec2ce0
    x1 = x0 | x1
    v = x1 - 0    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x0 = (eq)? 1 : 0 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x16 = [x28 + 0x10]
    v = sp - x16  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x4ec3034 // likely
    [sp - 0x30]! = x30
    [sp -8] = x29
    x29 = sp - 8
    (x3, 2) = 3   // elf_phdr // section..text
    (x0, 2) = 3
    x1 = [x4 + 0x48]
    0x4ec30d0 (x0, x1, 0x100b70003, 0x4d0f000)
    x29 = [sp -8]
    x30 = [sp] + 0x30
    x3 = x30
    0x4da1520 (x0, x1, 0x100b70003, x3)
    goto 0x4ec3000
    x16 = [x28 + 0x10]
    v = sp - x16  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x4ec306c // likely
    [sp - 0x20]! = x30
    [sp -8] = x29
    x29 = sp - 8
    x0 = [x26 + 8]
    0x4dac760 (x0, x1, 0x100b70003, x3)
    x29 = [sp -8]
    x30 = [sp] + 0x20
    x3 = x30
    0x4da1520 (x0, x1, 0x100b70003, 0x4ec303c)
    goto 0x4ec3040
    x16 = [x28 + 0x10]
    v = sp - x16  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) <= 0) goto 0x4ec30b8 // likely
    [sp - 0x30]! = x30
    [sp -8] = x29
    x29 = sp - 8
    return x0;
}

