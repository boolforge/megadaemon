// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f38 (void) {
    if (v >= 0) goto 0x55b02b8 // likely
    [sp + 0x190] = x1
    [sp + 0x248] = x0
    [sp + 0x160] = (x2, 2)
    x2 = [sp + 0x258]
    ldr x2, [x2, x1, lsl 3]
    [sp + 0x278] = x2
    x27 = (byte) [x2]
    x0 = x2 + 0x40
    x1 = 0x9593d60
    0x55ba080 (0x10102464c45bf, 0x9593d60, 0x10102464c457f, x3)
    if (w0 == 0) goto 0x55b0198 // unlikely
    x3 = [sp + 0x278]
    (x3, 2) = 3
    goto 0x55b01a0
    x5 = [sp + 0x278]
    (x3, 2) = 3
    [sp + 0x220] = x3
    [sp + 0x108] = x4
    x0 = 0
    x1 = [sp + 0x168]
    x2 = [sp + 0x248]
    x5 = [sp + 0x160]
    goto 0x55b01cc
    x0 = x5 + 1   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x5 = x1
    x1 = x2
    x2 = x3
    x3 = [sp + 0x1c8]
    v = x0 - x3   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v >= 0) goto 0x55b0298 // likely
    [sp + 0x1d8] = x0
    [sp + 0x1f0] = x1
    [sp + 0x508] = x2
    [sp + 0x1e8] = x5
    x2 = [sp + 0x2f8]
    ldr x2, [x2, x0, lsl 3]
    [sp + 0x308] = x2
    x27 = (byte) [x2]
    return x0;
}

