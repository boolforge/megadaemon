// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f57 (void) {
    x1 = x4
    v = x17 - x1  // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v <= 0) goto 0x66e4128 // likely
    [sp + 0x150] = x1
    ldr x1, [x16, x1, lsl 3]
    [sp + 0x1f0] = x1
    x0 = 0x8e59c90
    0x4d31c10 (0x8e59c90, 0x10102464c457f, x2, x3)
    x1 = [sp + 0x1f0]
    if (!x1) goto 0x66e3938 // unlikely
    (x5, 2) = 3
    x7 = 0x8f08ba8
    v = x5 - x7   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x66e392c // unlikely
    x6 = 0
    if (v) goto 0x66e3938 // likely
    x5 = [x6 + 8]
    goto 0x66e393c
    x5 = 0
    [sp + 0x230] = x0
    if (x5) goto 0x66e3d64 // unlikely
    if (!x1) goto 0x66e396c // unlikely
    (x5, 2) = 3
    x7 = 0x8f08b68
    v = x5 - x7   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x66e3960 // unlikely
    x6 = 0
    if (v) goto 0x66e3974 // likely
    x5 = [x6]
    goto 0x66e3978
    x7 = 0x8f08b68
    x5 = 0
    if (!x5) goto 0x66e3ca4 // likely
    if (!x1) goto 0x66e399c // unlikely
    (x5, 2) = 3
    v = x5 - x7   // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (!v) goto 0x66e3990 // unlikely
    x6 = 0
    if (v) goto 0x66e399c // likely
    x5 = [x6]
    goto 0x66e39a0
    x5 = 0
    if (!x5) goto 0x66e39ac // likely
    x5 = [x5 + 0x38]
    return x0;
}

