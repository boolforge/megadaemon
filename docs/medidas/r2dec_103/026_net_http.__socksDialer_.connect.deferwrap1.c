// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f26 (void) {
    x7 = x0
    x0 = [sp + 0x338]
    goto 0x527e4e8
    x3 = [x9 + 8]
    x7 = x3
    goto 0x527e4e8
    x11 = ((x7 & x10) << 4) + 8
    x12 = [x9 + x11]
    x11 = x9 + x11
    v = x12 - x1  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x527e788 // likely
    x7 += 1       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (x12) goto 0x527e750 // unlikely
    x0 = x8
    0x4d2e4c0 (x0, x1, x2, x3)
    x7 = x0
    x0 = [sp + 0x338]
    goto 0x527e4a0 // f24+0x18
    return x0;
}

