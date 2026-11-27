// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f46 (void) {
    q0 = [x5 0x18] // section..text // elf_phdr
    x5 = [x5 0x24]
    [sp 0x37] = q0
    [sp 0x43] = x5
    x27 = sp + 0x4b
    (x5, 2) = 3
    x9 = [sp 0x37] // section..text
    x10 = [sp 0x5b]
    v = x5 - x9   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x5 = (eq)? 1 : 0
    v = x8 - x6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x6 = (eq)? 1 : 0 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x5 &= x6
    v = w10 - w7  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x6 = (eq)? 1 : 0 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x5 &= x6
    if (w5 == 0) goto 0x58dcce4 // likely
    x3 = [sp + 0xe8]
    (x3, 2) = 3   // elf_phdr // section..text
    x3 = [x3 + 0x20]
    return x0;
}

