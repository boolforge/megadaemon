// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f37 (void) {
    x6 = x0
    x0 = [sp + 0x548]
    [x5 + 0x38] = x1
    x3 = x1 - 1
    x27 = 0xb48f000
    w4 = [x27 + 0x450]
    if (!w4) goto 0x55afae0 // likely
    ldr x4, [x6, x3, lsl 3]
    0x4da3500 (x0, x1, x2, -1)
    stp x0, x4, [x25]
    str x0, [x6, x3, lsl 3]
    x0 = 0x831e590
    x1 = sp + 0x818
    x2 = sp + 0xe6
    0x4d99020 (0x831e590, 0x10078818, 0x100780e6, -1)
    (x1, 2) = 3
    w0 = (byte) [sp + 0xfb]
    x8 = [sp + 0x2f0]
    goto 0x55af5e4
    x3 = (sp + 0x850) - x7
    x1 = [sp + 0x230]
    x4 = x1 - x7
    v = x4 - x3   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) < 0) goto 0x55afb2c // likely
    x0 = [sp + 0x128]
    goto 0x55afb7c
    x0 = 0x81bd748
    x2 = [sp + 0x128]
    0x4d79b60 (0x81bd748, x1, x2, 0x10078850)
    x5 = [sp + 0x1c0]
    x7 = sp
    x8 = [sp + 0x2f0]
    x9 = [sp + 0x1b8]
    x2 = x1
    x1 = x0
    x0 = x2
    goto 0x55afb7c
    x1 = [sp + 0x528] + 0x18
    x3 = [sp + 0x218]
    return x0;
}

