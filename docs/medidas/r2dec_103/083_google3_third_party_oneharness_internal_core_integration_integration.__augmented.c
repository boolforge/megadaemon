// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f83 (void) {
    stp 0, 0, [x2]
    [x2 + 0x10] = (0, 2)
    x27 = sp + 0x228
    stp x0, x1, [x27]
    x3 = [sp + 0x158]
    x4 = [x3 + 0x78]
    if (x4) goto 0x67ca39c // likely
    x0 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    goto 0x67ca3e8
    [sp + 0x1a0] = (0, 2)
    x1 = [[x3 + 0x78] + 8]
    x0 = x1 + 1
    0x4d98810 (x0, -1, x2, x3)
    x1 = 0x856f750
    [sp + 0x1a0] = (x1, 2)
    x0 = 0x43782b6
    x1 = 0x15
    x2 = sp + 0x1a0
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = x3       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4e34a60 (0x43782b6, 0x15, 0x100781a0, 0x1)
    x27 = sp + 0x238
    stp x0, x1, [x27]
    x2 = sp + 0x228 // (cstr 0x043782b6) "Cursor is on line: %dOther open documents:failed to read uri %s"
    x0 = 2
    x1 = [sp + 0x108]
    x3 = [sp + 0x200]
    x4 = 2
    x5 = sp + 0x228 // (cstr 0x043782b6) "Cursor is on line: %dOther open documents:failed to read uri %s"
    x6 = x0
    x0 = [sp + 0x258]
    x7 = [x0 + 0x48] // elf_phdr
    if (!x7) goto 0x67ca49c // unlikely
    x6 += 1
    v = x4 - x6   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (((unsigned) v) >= 0) goto 0x67ca440 // unlikely
    x0 = x5       // (cstr 0x043782b6) "Cursor is on line: %dOther open documents:failed to read uri %s"
    x1 = x6
    x2 = x4
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = 0x856f550
    0x4d9e050 (0x10078228, 0x3, 0x2, 0x1)
    x6 = x1
    x5 = x0       // (cstr 0x043782b6) "Cursor is on line: %dOther open documents:failed to read uri %s"
    x4 = x2
    x0 = [sp + 0x258]
    x7 = x6 - 1
    add x7, x5, x7, lsl 4
    x8 = 0x15
    [x7 + 8] = x8
    x27 = 0xb48f000
    w8 = [x27 + 0x450]
    return x0;
}

