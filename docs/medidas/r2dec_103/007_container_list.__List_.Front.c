// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f7 (void) {
    w3 = (byte) [sp + 0x6d]
    x0 = [sp + 0x178]
    x5 = x1 - 1
    x27 = 0xb48f000
    w6 = [x27 + 0x450]
    if (!w6) goto 0x5199fbc // likely
    ldr x6, [x4, x5, lsl 3]
    0x4da3500 (x0, x1, x2, x3)
    return x0;
}

