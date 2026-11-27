// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f59 (void) {
    stp x6, x7, [x27]
    x0 = 0x80fe638
    x1 = x5
    x2 = sp + 0x270
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = x3       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4f32280 (0x80fe638, x1, 0x10078270, 0x1)
    if (x3) goto 0x66e48c4 // likely
    x2 = x1
    x1 = x0
    x0 = 0
    0x4d7d610 (x0, 0x80fe638, x2, 0x1)
    x7 = 0x8f08b68
    x2 = x0
    return x0;
}

