// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f78 (void) {
    if (w1) goto 0x66e4a64 // unlikely
    x4 = [sp + 0x228]
    goto 0x66e4a74
    x3 = [x0]
    0x4da3500 (x0, x1, x2, 0x10102464c457f)
    x4 = [sp + 0x228]
    stp x4, x3, [x25]
    [x0] = x4
    x0 = 0x48     // 'H'
    x1 = 0x8c419c8
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d33650 (0x48, 0x8c419c8, 0x1, 0x10102464c457f)
    x27 = 0xb48f000
    w3 = [x27 + 0x450]
    if (w3) goto 0x66e4aa0 // unlikely
    x2 = [sp + 0x268]
    goto 0x66e4aac
    0x4da34f0 (0x48, 0x8c419c8, x2, x3)
    x2 = [sp + 0x268]
    return x0;
}

