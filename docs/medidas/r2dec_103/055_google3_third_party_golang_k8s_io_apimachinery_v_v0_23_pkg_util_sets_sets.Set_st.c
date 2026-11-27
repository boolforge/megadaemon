// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f55 (void) {
    x0 = x2
    0x4f31070 (x0, x1, x2, x3)
    x2 = sp + 0x76
    stp 0, 0, [x2]
    [x2 0xc] = 0
    v = x1 - 0x14 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x3 = 0x14
    x3 = (lt)? x1 : x3
    v = x0 - x2   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x597a384 // unlikely
    x1 = x0
    x0 = x2
    x2 = x3
    0x4da39e0 (0x10078076, x1, x2, x3)
    q0 = [sp 0x76]
    x2 = [sp 0x82]
    [sp 0x62] = q0
    [sp 0x6e] = x2
    x2 = [sp + 0xc8]
    (x0, 2) = 3
    if (!x0) goto 0x597a440 // unlikely
    x2 = [x0 + 8]
    x3 = 0x94a70b0
    ldar x4, [x3]
    x5 = [x4]
    w6 = [x0 + 0x10]
    goto 0x597a608
    [sp + 0xe0] = x2
    [sp + 0xb8] = x1
    x0 = 2
    0x52b6f20 (0x2, x1, -1, 0x94a70b0)
    [sp + 0xf0] = x0
    return x0;
}

