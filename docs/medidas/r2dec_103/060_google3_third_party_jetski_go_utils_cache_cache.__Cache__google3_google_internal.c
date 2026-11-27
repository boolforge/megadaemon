// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f60 (void) {
    v = x4 - x7   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x66e3ae4 // likely
    x5 = 0
    if (v) goto 0x66e3af0 // unlikely
    x4 = [x5]
    goto 0x66e3af4
    x4 = 0
    if (!x4) goto 0x66e3b00 // likely
    (x4, 2) = 3
    goto 0x66e3b08
    x5 = 0
    x4 = 0
    [sp + 0x138] = x3
    [sp + 0x248] = x2
    if (x5) goto 0x66e3b74 // unlikely
    0x52c04d0 (x0, x1, x2, x3)
    q0 = [sp 8]
    [sp 0x84] = q0
    if (x0) goto 0x66e4938 // unlikely
    q0 = [sp 0x84]
    return x0;
}

