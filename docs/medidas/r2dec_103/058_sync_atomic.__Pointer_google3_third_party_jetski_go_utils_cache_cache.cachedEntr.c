// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f58 (void) {
    goto 0x66e4950
    if (!x1) goto 0x66e3a1c // likely
    (x5, 2) = 3
    v = x5 - x7   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x66e3a10 // likely
    x6 = 0
    if (v) goto 0x66e3a1c // unlikely
    x5 = [x6]
    goto 0x66e3a20
    x5 = 0
    if (!x5) goto 0x66e3a2c // likely
    x5 = [x5 + 0x28] // elf_shdr
    goto 0x66e3a30
    x5 = 0
    if (x5) goto 0x66e3a40 // unlikely
    x2 = 0
    x3 = 0
    goto 0x66e3ad0
    if (!x1) goto 0x66e3a60 // likely
    (x5, 2) = 3
    return x0;
}

