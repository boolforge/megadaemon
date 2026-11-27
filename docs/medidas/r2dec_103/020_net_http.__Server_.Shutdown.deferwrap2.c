// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f20 (void) {
    [x25 + 0x10] = (x3, 2)
    x5 = [sp + 0x6a8]
    x6 = [sp + 0x680]
    [x25 + 0x20] = (x5, 2)
    [x0 + 8] = (x1, 2)
    x1 = [sp + 0x1b8]
    [x0 + 0x18] = (x3, 2)
    [x0 + 0x28] = (x4, 2)
    [x0 + 0x38] = x6
    0x4d6bbe0 (x0, x1, x2, x3)
    x1 = [[sp + 0x1b0] + 0x20]
    x0 = [sp + 0x348]
    callreg x1    // elf_phdr(0x0, 0x40, 0x0, 0x0)
    x1 = sp + 0x558
    stp 0, 0, [x1]
    return x0;
}

