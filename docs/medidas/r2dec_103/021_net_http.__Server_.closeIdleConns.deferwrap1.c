// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f21 (void) {
    x26 = [x0 + 0xa0]
    if (!x26) goto 0x527e038 // unlikely
    x5 = [x26]
    x6 = [sp + 0x680]
    x4 = [x6]
    x2 = [sp + 0x6c0]
    x0 = [sp + 0x200]
    x1 = [sp + 0x3b8]
    x3 = [sp + 0x340]
    callreg x5    // 0xffffffffffffffff(0x0, 0x0, 0x0, 0x0)
    x5 = [sp + 0x688]
    [x5] = x0
    x27 = 0xb48f000
    w6 = [x27 + 0x450]
    if (!w6) goto 0x527e030 // likely
    x7 = [x5 + 8]
    0x4da3500 (x0, x1, x2, x3)
    stp x1, x7, [x25]
    [x5 + 8] = x1
    if (x0) goto 0x527e148 // unlikely
    x7 = [[sp + 0x680]]
    x8 = [x7 + 0x10]
    v = x8 - 0xc8 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x527d020 // unlikely
    (x0, 2) = 3
    x2 = 0x4a09688
    return x0;
}

