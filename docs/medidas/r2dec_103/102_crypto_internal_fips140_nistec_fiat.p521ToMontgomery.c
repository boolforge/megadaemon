// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f102 (void) {
    aese v6.16b, v30.16b
    aesmc v6.16b, v6.16b
    aese v7.16b, v30.16b
    aesmc v7.16b, v7.16b
    if (w10 == 3) goto 0x7185fd4 // likely
    ld1 (v12.16b, v13.16b), [x13], 32 // section..text
    aese v0.16b, v12.16b
    aesmc v0.16b, v0.16b
    aese v1.16b, v12.16b
    aesmc v1.16b, v1.16b
    aese v2.16b, v12.16b
    aesmc v2.16b, v2.16b
    aese v3.16b, v12.16b
    aesmc v3.16b, v3.16b
    aese v4.16b, v12.16b
    aesmc v4.16b, v4.16b
    aese v5.16b, v12.16b
    aesmc v5.16b, v5.16b
    aese v6.16b, v12.16b
    aesmc v6.16b, v6.16b
    aese v7.16b, v12.16b
    aesmc v7.16b, v7.16b
    aese v0.16b, v13.16b
    aesmc v0.16b, v0.16b
    aese v1.16b, v13.16b
    aesmc v1.16b, v1.16b
    aese v2.16b, v13.16b
    aesmc v2.16b, v2.16b
    aese v3.16b, v13.16b
    aesmc v3.16b, v3.16b
    aese v4.16b, v13.16b
    aesmc v4.16b, v4.16b
    aese v5.16b, v13.16b
    aesmc v5.16b, v5.16b
    aese v6.16b, v13.16b
    aesmc v6.16b, v6.16b
    aese v7.16b, v13.16b
    aesmc v7.16b, v7.16b
    x13 = x11
    aese v0.16b, v27.16b
    aesmc v0.16b, v0.16b
    aese v1.16b, v27.16b
    aesmc v1.16b, v1.16b
    aese v2.16b, v27.16b
    aesmc v2.16b, v2.16b
    aese v3.16b, v27.16b
    aesmc v3.16b, v3.16b
    aese v4.16b, v27.16b
    aesmc v4.16b, v4.16b
    aese v5.16b, v27.16b
    aesmc v5.16b, v5.16b
    aese v6.16b, v27.16b
    aesmc v6.16b, v6.16b
    aese v7.16b, v27.16b
    aesmc v7.16b, v7.16b
    aese v0.16b, v28.16b
    aese v1.16b, v28.16b
    aese v2.16b, v28.16b
    aese v3.16b, v28.16b
    aese v4.16b, v28.16b
    aese v5.16b, v28.16b
    aese v6.16b, v28.16b
    aese v7.16b, v28.16b
    v0.16b ^= v31.16b
    v1.16b ^= v31.16b
    v2.16b ^= v31.16b
    v3.16b ^= v31.16b
    v4.16b ^= v31.16b
    v5.16b ^= v31.16b
    v6.16b ^= v31.16b
    v7.16b ^= v31.16b
    ld1 (v12.16b, v13.16b), [x3], 32 // section..text
    v0.16b = v12.16b ^ v0.16b // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    v1.16b = v13.16b ^ v1.16b // section..text
    st1 (v0.16b, v1.16b), [x1], 32
    ld1 (v12.16b, v13.16b), [x3], 32 // elf_phdr // elf_shdr // elf_phdr
    v2.16b = v12.16b ^ v2.16b // elf_phdr // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX" // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX" // elf_shdr
    v3.16b = v13.16b ^ v3.16b // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    st1 (v2.16b, v3.16b), [x1], 32
    ld1 (v12.16b, v13.16b), [x3], 32 // elf_phdr
    v4.16b = v12.16b ^ v4.16b // elf_phdr
    v5.16b = v13.16b ^ v5.16b // elf_phdr
    st1 (v4.16b, v5.16b), [x1], 32
    ld1 (v12.16b, v13.16b), [x3], 32
    v6.16b = v12.16b ^ v6.16b
    v7.16b = v13.16b ^ v7.16b // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    st1 (v6.16b, v7.16b), [x1], 32
    ld1 (v12.16b, v13.16b), [x0], 32 // section..text
    rev64 v0.16b, v0.16b
    v0.16b ^= v8.16b
    ext v11.16b, v0.16b, v0.16b, 8
    v11.16b ^= v0.16b // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX" // "_party/golang/github_com/openai/openai_go/v/v3/responses/respon"
    pmull v9.1q, v12.1d, v0.1d
    pmull2 v8.1q, v12.2d, v0.2d
    pmull v10.1q, v13.1d, v11.1d
    ld1 (v12.16b, v13.16b), [x0], 32 // elf_phdr // elf_shdr // elf_phdr
    rev64 v1.16b, v1.16b
    ext v11.16b, v1.16b, v1.16b, 8
    v11.16b ^= v1.16b
    pmull v14.1q, v12.1d, v1.1d
    v9.16b ^= v14.16b
    pmull2 v14.1q, v12.2d, v1.2d
    v8.16b ^= v14.16b
    pmull v14.1q, v13.1d, v11.1d
    v10.16b ^= v14.16b
    ld1 (v12.16b, v13.16b), [x0], 32 // elf_phdr
    rev64 v2.16b, v2.16b
    ext v11.16b, v2.16b, v2.16b, 8
    v11.16b ^= v2.16b
    pmull v14.1q, v12.1d, v2.1d
    v9.16b ^= v14.16b
    pmull2 v14.1q, v12.2d, v2.2d
    v8.16b ^= v14.16b
    pmull v14.1q, v13.1d, v11.1d
    v10.16b ^= v14.16b
    ld1 (v12.16b, v13.16b), [x0], 32
    rev64 v3.16b, v3.16b
    ext v11.16b, v3.16b, v3.16b, 8
    v11.16b ^= v3.16b
    pmull v14.1q, v12.1d, v3.1d
    v9.16b ^= v14.16b
    pmull2 v14.1q, v12.2d, v3.2d
    v8.16b ^= v14.16b
    pmull v14.1q, v13.1d, v11.1d
    v10.16b ^= v14.16b
    ld1 (v12.16b, v13.16b), [x0], 32
    rev64 v4.16b, v4.16b
    ext v11.16b, v4.16b, v4.16b, 8
    v11.16b ^= v4.16b
    pmull v14.1q, v12.1d, v4.1d
    v9.16b ^= v14.16b
    pmull2 v14.1q, v12.2d, v4.2d
    v8.16b ^= v14.16b
    pmull v14.1q, v13.1d, v11.1d
    v10.16b ^= v14.16b
    ld1 (v12.16b, v13.16b), [x0], 32 // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    rev64 v5.16b, v5.16b
    ext v11.16b, v5.16b, v5.16b, 8
    v11.16b ^= v5.16b
    pmull v14.1q, v12.1d, v5.1d
    v9.16b ^= v14.16b
    pmull2 v14.1q, v12.2d, v5.2d
    v8.16b ^= v14.16b
    pmull v14.1q, v13.1d, v11.1d
    v10.16b ^= v14.16b
    ld1 (v12.16b, v13.16b), [x0], 32
    rev64 v6.16b, v6.16b
    ext v11.16b, v6.16b, v6.16b, 8
    v11.16b ^= v6.16b
    pmull v14.1q, v12.1d, v6.1d
    v9.16b ^= v14.16b
    pmull2 v14.1q, v12.2d, v6.2d
    v8.16b ^= v14.16b
    pmull v14.1q, v13.1d, v11.1d
    v10.16b ^= v14.16b
    ld1 (v12.16b, v13.16b), [x0], 32 // segment.GNU_RELRO
    rev64 v7.16b, v7.16b
    ext v11.16b, v7.16b, v7.16b, 8
    v11.16b ^= v7.16b
    pmull v14.1q, v12.1d, v7.1d
    v9.16b ^= v14.16b
    pmull2 v14.1q, v12.2d, v7.2d
    v8.16b ^= v14.16b
    pmull v14.1q, v13.1d, v11.1d
    v10.16b ^= v14.16b
    x0 = x14
    v10.16b ^= v8.16b
    v10.16b ^= v9.16b
    ext v11.16b, v10.16b, v16.16b, 8
    ext v10.16b, v16.16b, v10.16b, 8
    v8.16b ^= v10.16b
    v9.16b ^= v11.16b // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX" // (pstr 0x043d2b99) "slice result of non-ptr typespan is not for a user arenainvalid" // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    pmull v11.1q, v8.1d, v15.1d
    ext v8.16b, v8.16b, v8.16b, 8
    v8.16b ^= v11.16b // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX" // (pstr 0x043d2b99) "slice result of non-ptr typespan is not for a user arenainvalid" // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    pmull v11.1q, v8.1d, v15.1d
    v9.16b ^= v11.16b
    ext v9.16b, v9.16b, v9.16b, 8
    v8.16b ^= v9.16b
    v = x6 - 0x80 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v >= 0) goto 0x7185c7c // unlikely
    if (!x6) goto 0x7186460 // likely
    x0 += 0xe0
    ld1 (v12.16b, v13.16b), [x0], 32 // segment.GNU_RELRO
    x4 += 0x80
    ld1 (v27.16b, v28.16b, v29.16b), [x4], 48 // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    v31.16b = v29.16b // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    if (w10 == 4) goto 0x718623c // likely
    ld1 (v1.16b, v2.16b), [x4], 32
    v31.16b = v2.16b
    if (w10 == 3) goto 0x718623c // likely
    ld1 (v3.16b, v4.16b), [x4], 32
    v31.16b = v4.16b
    v = x6 - 0x10 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v < 0) goto 0x7186328 // likely
    x6 -= 0x10
    ld1 (v11.16b), [x3], 16
    v11.16b ^= v31.16b // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    rev32 v0.16b, v18.16b
    v18.4s = v17.4s + v18.4s
    aese v0.16b, v19.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v20.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v21.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v22.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v23.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v24.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v25.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v26.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v27.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v28.16b
    if (w10 == 4) goto 0x71862cc // likely
    aesmc v0.16b, v0.16b
    aese v0.16b, v29.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v1.16b
    if (w10 == 3) goto 0x71862cc // likely
    aesmc v0.16b, v0.16b
    aese v0.16b, v2.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v3.16b
    v0.16b ^= v11.16b // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    st1 (v0.16b), [x1], 16
    rev64 v0.16b, v0.16b
    v0.16b ^= v8.16b // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX" // (pstr 0x043d2b99) "slice result of non-ptr typespan is not for a user arenainvalid"
    ext v11.16b, v0.16b, v0.16b, 8
    v11.16b ^= v0.16b // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX" // (pstr 0x043d2b99) "slice result of non-ptr typespan is not for a user arenainvalid"
    pmull v9.1q, v12.1d, v0.1d
    pmull2 v8.1q, v12.2d, v0.2d
    pmull v10.1q, v13.1d, v11.1d
    v10.16b ^= v8.16b // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX" // (pstr 0x043d2b99) "slice result of non-ptr typespan is not for a user arenainvalid" // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    v10.16b ^= v9.16b
    ext v11.16b, v10.16b, v16.16b, 8
    ext v10.16b, v16.16b, v10.16b, 8
    v8.16b ^= v10.16b
    v9.16b ^= v11.16b // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX" // (pstr 0x043d2b99) "slice result of non-ptr typespan is not for a user arenainvalid" // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec" // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    pmull v11.1q, v8.1d, v15.1d
    ext v8.16b, v8.16b, v8.16b, 8
    v8.16b ^= v11.16b // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX" // (pstr 0x043d2b99) "slice result of non-ptr typespan is not for a user arenainvalid" // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec" // "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    pmull v11.1q, v8.1d, v15.1d
    v9.16b ^= v11.16b
    ext v9.16b, v9.16b, v9.16b, 8
    v8.16b ^= v9.16b
    goto 0x718623c
    if (!x6) goto 0x7186460 // unlikely
    v11.16b ^= v11.16b
    v14.16b ^= v14.16b
    x12 = -1
    x3 += x6
    if (w6 == 3) goto 0x718634c // likely
    x11 = [x3 - 8]!
    v11.d[0] = x11
    v14.d[0] = x12
    if (w6 == 2) goto 0x7186364 // likely
    x11 = [x3 - 4]!
    ext v11.16b, v16.16b, v11.16b, 0xc
    ext v14.16b, v16.16b, v14.16b, 0xc
    v11.s[0] = w11
    v14.s[0] = w12
    if (w6 == 1) goto 0x718637c // likely
    ldrsh x11, [x3, -2]!
    ext v11.16b, v16.16b, v11.16b, 0xe
    ext v14.16b, v16.16b, v14.16b, 0xe
    v11.h[0] = w11
    v14.h[0] = w12
    if (w6 == 0) goto 0x7186394 // likely
    x11 = (byte) [x3 - 1]!
    ext v11.16b, v16.16b, v11.16b, 0xf
    ext v14.16b, v16.16b, v14.16b, 0xf
    v11.b[0] = w11
    v14.b[0] = w12
    v11.16b ^= v31.16b // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    rev32 v0.16b, v18.16b
    aese v0.16b, v19.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v20.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v21.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v22.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v23.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v24.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v25.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v26.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v27.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v28.16b
    if (w10 == 4) goto 0x7186410 // likely
    aesmc v0.16b, v0.16b
    aese v0.16b, v29.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v1.16b
    if (w10 == 3) goto 0x7186410 // likely
    aesmc v0.16b, v0.16b
    aese v0.16b, v2.16b
    aesmc v0.16b, v0.16b
    aese v0.16b, v3.16b
    v0.16b ^= v11.16b // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    v0.16b &= v14.16b
    v11.16b = v0.16b
    if (w6 == 3) goto 0x718642c // likely
    x11 = v11.d[0]
    [x1] + 8 = x11
    ext v11.16b, v11.16b, v16.16b, 8
    if (w6 == 2) goto 0x718643c // likely
    w11 = v11.s[0]
    [x1] + 4 = w11
    ext v11.16b, v11.16b, v16.16b, 4
    if (w6 == 1) goto 0x718644c // likely
    umov w11, v11.h[0]
    [x1] + 2 = (half) w11
    ext v11.16b, v11.16b, v16.16b, 2
    if (w6 == 0) goto 0x7186458 // likely
    umov w11, v11.b[0]
    [x1] + 1 = (byte) w11
    x6 = 0
    goto 0x71862d4
    st1 (v8.16b), [x5]
    x0 = [sp + 8]
    x1 = [sp + 0x10]
    x3 = [sp + 0x28]
    x6 = [sp + 0x30]
    x2 = [sp + 0x40]
    x5 = [sp + 0x48]
    x4 = [sp + 0x50]
    x10 = [sp + 0x58]
    x12 = 0xc200000000000000
    x11 = 1       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    v15.d[0] = x12
    v15.d[1] = x11 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    v16.16b ^= v16.16b
    x14 = x0
    ld1 (v8.16b), [x5]
    v9.16b ^= v9.16b
    v10.16b ^= v10.16b
    ld1 (v18.16b), [x2]
    v17.16b ^= v17.16b
    x11 = 1       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    v17.s[3] = w11
    rev32 v18.16b, v18.16b
    v18.4s = v17.4s + v18.4s
    x11 = x4
    ld1 (v19.16b, v20.16b, v21.16b, v22.16b), [x11], 64 // section..text // elf_phdr // elf_shdr // elf_phdr
    ld1 (v23.16b, v24.16b, v25.16b, v26.16b), [x11], 64 // elf_phdr
    ld1 (v27.16b, v28.16b, v29.16b), [x11], 48 // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    v31.16b = v29.16b // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    v = x6 - 0x80 // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    if (v < 0) goto 0x7186c78 // likely
    x27 = 0xb48b000
    w12 = (byte) [x27 + 0xd9d]
    if (w10 == 4) goto 0x7186524 // likely
    v29.16b = v27.16b
    v30.16b = v28.16b
    v27.16b = v31.16b // (pstr 0x00000403) "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
    ld1 (v28.16b), [x11], 16
    ld1 (v31.16b), [x11], 16
    if (w10 == 3) goto 0x7186524 // likely
    v27.16b = v31.16b
    ld1 (v28.16b), [x11], 16
    ld1 (v31.16b), [x11], 16
    x11 = x4 + 0xa0
    x13 = x11
    x6 -= 0x80
    v0.16b = v18.16b
    v1.4s = v17.4s + v0.4s
    rev32 v0.16b, v0.16b
    v2.4s = v17.4s + v1.4s
    rev32 v1.16b, v1.16b
    v3.4s = v17.4s + v2.4s
    rev32 v2.16b, v2.16b
    v4.4s = v17.4s + v3.4s
    rev32 v3.16b, v3.16b
    v5.4s = v17.4s + v4.4s
    rev32 v4.16b, v4.16b
    v6.4s = v17.4s + v5.4s
    rev32 v5.16b, v5.16b
    v7.4s = v17.4s + v6.4s
    rev32 v6.16b, v6.16b
    v18.4s = v17.4s + v7.4s
    rev32 v7.16b, v7.16b
    aese v0.16b, v19.16b
    aesmc v0.16b, v0.16b
    aese v1.16b, v19.16b
    aesmc v1.16b, v1.16b
    aese v2.16b, v19.16b
    aesmc v2.16b, v2.16b
    aese v3.16b, v19.16b
    aesmc v3.16b, v3.16b
    aese v4.16b, v19.16b
    aesmc v4.16b, v4.16b
    aese v5.16b, v19.16b
    aesmc v5.16b, v5.16b
    aese v6.16b, v19.16b
    aesmc v6.16b, v6.16b
    aese v7.16b, v19.16b
    aesmc v7.16b, v7.16b
    aese v0.16b, v20.16b
    aesmc v0.16b, v0.16b
    aese v1.16b, v20.16b
    aesmc v1.16b, v1.16b
    aese v2.16b, v20.16b
    aesmc v2.16b, v2.16b
    aese v3.16b, v20.16b
    aesmc v3.16b, v3.16b
    aese v4.16b, v20.16b
    aesmc v4.16b, v4.16b
    aese v5.16b, v20.16b
    aesmc v5.16b, v5.16b
    aese v6.16b, v20.16b
    aesmc v6.16b, v6.16b
    aese v7.16b, v20.16b
    aesmc v7.16b, v7.16b
    aese v0.16b, v21.16b
    aesmc v0.16b, v0.16b
    aese v1.16b, v21.16b
    return x0;
}

