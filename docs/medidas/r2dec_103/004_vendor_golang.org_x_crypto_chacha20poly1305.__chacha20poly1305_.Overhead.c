// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f4 (void) {
    w0 = (byte) [sp + 0x6c]
    if (w0 != 0) goto 0x519943c // unlikely
    if (x5) goto 0x519943c // unlikely
    x5 = sp + 0x200
    return x0;
}

