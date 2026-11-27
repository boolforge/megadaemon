// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f19 (void) {
    [sp + 0x220] = x2
    x0 = sp + 0x208
    0x4d5dd50 (0x10078208, x1, x2, x3)
    x0 = 0x81e3590
    x1 = 0
    0x4d28110 (0x81e3590, x1, x2, x3)
    [sp + 0x330] = x0
    x0 = 8
    x1 = 0x7f82af8
    x2 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4d322b0 (0x8, 0x7f82af8, 0x1, x3)
    [sp + 0x680] = x0
    x0 = 0x10
    x1 = 0x86a8000
    return x0;
}

