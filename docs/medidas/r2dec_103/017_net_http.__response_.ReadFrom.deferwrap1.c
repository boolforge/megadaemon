// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f17 (void) {
    (x4, 2) = 3
    v = x5 - 2    // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x527d44c // likely
    w4 = (word) [x4] // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    x5 = 0x3268   // 'h2' // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    v = w4 - w5   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v) goto 0x527d44c // likely
    x27 = (byte) [x0]
    x27 = sp + 0x250
    stp 0, 0, [x27]
    x4 = x0 + 0x38
    ldar x4, [x4]
    if (!x4) goto 0x527d3f0 // unlikely
    x5 = 0xb48bd7a
    v = x4 - x5   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x527d3f0 // unlikely
    x5 = x0 + 0x40 // elf_phdr
    ldar x5, [x5] // elf_phdr
    x27 = sp + 0x250
    stp x4, x5, [x27]
    goto 0x527d3f8
    x27 = sp + 0x250
    stp 0, 0, [x27]
    x27 = sp + 0x250
    (x4, 2) = 3
    x5 = 0x832fd00
    return x0;
}

