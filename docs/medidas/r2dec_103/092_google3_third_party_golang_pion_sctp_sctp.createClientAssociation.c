// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f92 (void) {
    x4 += 0x5d0   // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    0x4d9e050 (x0, x1, x2, x3)
    x3 = [sp + 0x118]
    x4 = x1
    x1 = [sp + 0x158]
    [sp + 0x1a8] = x0
    [sp + 0x108] = (x4, 2)
    x0 += x1
    x1 = [sp + 0x1c0]
    x2 = x3
    0x4da39e0 (x0, x1, x2, x3)
    x3 = [sp + 0x348]
    (x0, 2) = 3   // segment.LOAD2
    x2 = [sp + 0xd8]
    if (!x2) goto 0x6df5dbc // likely
    x5 = [x2 + 8]
    goto 0x6df5dc0
    x5 = x2
    x2 = [x3 + 0x1e8]
    x3 = x5
    x4 = [sp + 0x180]
    x5 = 0
    0x6dcb920 (0x91be590, 0x91be590, 0xcd080, x3)
    if (x2) goto 0x6df6128 // likely
    return x0;
}

