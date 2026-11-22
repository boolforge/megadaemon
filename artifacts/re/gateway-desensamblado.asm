==== GatewayConfig_.IsEnabled
/ fcn.06d92368(int64_t arg1, int64_t arg_28h, int64_t arg_30h, int64_t arg_40h, int64_t arg_70h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg_28h @ stack + 0x28
|           ; arg int64_t arg_30h @ stack + 0x30
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_70h @ stack + 0x70
|       ,=< 0x06d92368      cbz   w3, 0x6d92378
|       |   0x06d9236c      ldp   x4, x5, [x0]
|       |   0x06d92370      bl    0x4da3500
|       |   0x06d92374      stp   x4, x5, [x25]
|       `-> 0x06d92378      adrp  x4, 0x8f93000
|           0x06d9237c      add   x4, x4, 0x8f0
|           0x06d92380      adrp  x5, 0x8f93000
|           0x06d92384      add   x5, x5, 0x7f8
|           0x06d92388      stp   x4, x5, [x0]                         ; arg1
|           0x06d9238c      adrp  x0, 0x8333000
|           0x06d92390      add   x0, x0, 0x110
|           0x06d92394      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92398      adrp  x2, 0x42be000
|           0x06d9239c      add   x2, x2, 0xd0f
|           0x06d923a0      mov   x3, 0xb
|           0x06d923a4      bl    0x4d18e50
|           0x06d923a8      ldrsb x27, [x0]
|           0x06d923ac      adrp  x27, 0xb48f000
|           0x06d923b0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d923b4      cbz   w4, 0x6d923c4
|       |   0x06d923b8      ldp   x4, x5, [x0]
|       |   0x06d923bc      bl    0x4da3500
|       |   0x06d923c0      stp   x4, x5, [x25]
|       `-> 0x06d923c4      adrp  x4, 0x8f93000
|           0x06d923c8      add   x4, x4, 0x8e0
|           0x06d923cc      adrp  x5, 0x8f93000
|           0x06d923d0      add   x5, x5, 0x7e0
|           0x06d923d4      stp   x4, x5, [x0]
|           0x06d923d8      adrp  x27, 0x948c000
|           0x06d923dc      ldr   x4, [x27, 0xd20]                     ; [0xd20:4]=0x58585858
|           0x06d923e0      str   x4, [arg_28h]
|           0x06d923e4      adrp  x0, 0x8333000
|           0x06d923e8      add   x0, x0, 0x110
|           0x06d923ec      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d923f0      adrp  x2, 0x4326000
|           0x06d923f4      add   x2, x2, 0xbfe
|           0x06d923f8      orr   x3, xzr, 0x10
|           0x06d923fc      bl    0x4d18e50
|           0x06d92400      ldrsb x27, [x0]
|           0x06d92404      adrp  x27, 0xb48f000
|           0x06d92408      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9240c      cbnz  w4, 0x6d92418
|       |   0x06d92410      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      ,==< 0x06d92414      b     0x6d9242c
|      |`-> 0x06d92418      ldp   x4, x5, [x0]
|      |    0x06d9241c      bl    0x4da3510
|      |    0x06d92420      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      |    0x06d92424      stp   x6, x4, [x25]
|      |    0x06d92428      str   x5, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92414
|      `--> 0x06d9242c      adrp  x4, 0x8f93000
|           0x06d92430      add   x4, x4, 0x7d8
|           0x06d92434      stp   x6, x4, [x0]
|           0x06d92438      adrp  x0, 0x8333000
|           0x06d9243c      add   x0, x0, 0x110
|           0x06d92440      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92444      adrp  x2, 0x42d6000
|           0x06d92448      add   x2, x2, 0x4dd
|           0x06d9244c      orr   x3, xzr, 0xc
|           0x06d92450      bl    0x4d18e50
|           0x06d92454      ldrsb x27, [x0]
|           0x06d92458      adrp  x27, 0xb48f000
|           0x06d9245c      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92460      cbz   w4, 0x6d92470
|       |   0x06d92464      ldp   x4, x5, [x0]
|       |   0x06d92468      bl    0x4da3500
|       |   0x06d9246c      stp   x4, x5, [x25]
|       `-> 0x06d92470      adrp  x4, 0x8f93000
|           0x06d92474      add   x4, x4, 0x938
|           0x06d92478      adrp  x5, 0x8f93000
|           0x06d9247c      add   x5, x5, 0x830
|           0x06d92480      stp   x4, x5, [x0]
|           0x06d92484      adrp  x0, 0x8333000
|           0x06d92488      add   x0, x0, 0x110
|           0x06d9248c      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92490      adrp  x2, 0x42d6000
|           0x06d92494      add   x2, x2, 0x9a5
|           0x06d92498      orr   x3, xzr, 0xc
|           0x06d9249c      bl    0x4d18e50
|           0x06d924a0      ldrsb x27, [x0]
|           0x06d924a4      adrp  x27, 0xb48f000
|           0x06d924a8      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d924ac      cbz   w4, 0x6d924bc
|       |   0x06d924b0      ldp   x4, x5, [x0]
|       |   0x06d924b4      bl    0x4da3500
|       |   0x06d924b8      stp   x4, x5, [x25]
|       `-> 0x06d924bc      adrp  x4, 0x8f93000
|           0x06d924c0      add   x4, x4, 0x8d8
|           0x06d924c4      adrp  x5, 0x8f93000
|           0x06d924c8      add   x5, x5, 0x7d0
|           0x06d924cc      stp   x4, x5, [x0]
|           0x06d924d0      adrp  x0, 0x8333000
|           0x06d924d4      add   x0, x0, 0x110
|           0x06d924d8      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d924dc      adrp  x2, 0x4279000
|           0x06d924e0      add   x2, x2, 0xd1b
|           0x06d924e4      orr   x3, xzr, 8
|           0x06d924e8      bl    0x4d18e50
|           0x06d924ec      ldrsb x27, [x0]
|           0x06d924f0      adrp  x27, 0xb48f000
|           0x06d924f4      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d924f8      cbz   w4, 0x6d92508
|       |   0x06d924fc      ldp   x4, x5, [x0]
|       |   0x06d92500      bl    0x4da3500
|       |   0x06d92504      stp   x4, x5, [x25]
|       `-> 0x06d92508      adrp  x4, 0x8f93000
|           0x06d9250c      add   x4, x4, 0x918
|           0x06d92510      adrp  x5, 0x8f93000
|           0x06d92514      add   x5, x5, 0x818
|           0x06d92518      stp   x4, x5, [x0]
|           0x06d9251c      adrp  x27, 0x948c000
|           0x06d92520      ldr   x4, [x27, 0xd10]                     ; [0xd10:4]=0x58585858
|           0x06d92524      str   x4, [arg_28h]
|           0x06d92528      adrp  x0, 0x8333000
|           0x06d9252c      add   x0, x0, 0x110
|           0x06d92530      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92534      adrp  x2, 0x4314000
|           0x06d92538      add   x2, x2, 0x3ab
|           0x06d9253c      orr   x3, xzr, 0xf
|           0x06d92540      bl    0x4d18e50
|           0x06d92544      ldrsb x27, [x0]
|           0x06d92548      adrp  x27, 0xb48f000
|           0x06d9254c      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92550      cbnz  w4, 0x6d9255c
|       |   0x06d92554      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      ,==< 0x06d92558      b     0x6d92570
|      |`-> 0x06d9255c      ldp   x4, x5, [x0]
|      |    0x06d92560      bl    0x4da3510
|      |    0x06d92564      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      |    0x06d92568      stp   x6, x4, [x25]
|      |    0x06d9256c      str   x5, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92558
|      `--> 0x06d92570      adrp  x4, 0x8f93000
|           0x06d92574      add   x4, x4, 0x790
|           0x06d92578      stp   x6, x4, [x0]
|           0x06d9257c      adrp  x0, 0x8333000
|           0x06d92580      add   x0, x0, 0x110
|           0x06d92584      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92588      adrp  x2, 0x4301000
|           0x06d9258c      add   x2, x2, 0xb0e
|           0x06d92590      orr   x3, xzr, 0xe
|           0x06d92594      bl    0x4d18e50
|           0x06d92598      ldrsb x27, [x0]
|           0x06d9259c      adrp  x27, 0xb48f000
|           0x06d925a0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d925a4      cbz   w4, 0x6d925b4
|       |   0x06d925a8      ldp   x4, x5, [x0]
|       |   0x06d925ac      bl    0x4da3500
|       |   0x06d925b0      stp   x4, x5, [x25]
|       `-> 0x06d925b4      adrp  x4, 0x8f93000
|           0x06d925b8      add   x4, x4, 0x880
|           0x06d925bc      adrp  x5, 0x8f93000
|           0x06d925c0      add   x5, x5, 0x768
|           0x06d925c4      stp   x4, x5, [x0]
|           0x06d925c8      adrp  x0, 0x8333000
|           0x06d925cc      add   x0, x0, 0x110
|           0x06d925d0      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d925d4      adrp  x2, 0x4376000
|           0x06d925d8      add   x2, x2, 0xfd8
|           0x06d925dc      mov   x3, 0x15
|           0x06d925e0      bl    0x4d18e50
|           0x06d925e4      ldrsb x27, [x0]
|           0x06d925e8      adrp  x27, 0xb48f000
|           0x06d925ec      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d925f0      cbz   w4, 0x6d92600
|       |   0x06d925f4      ldp   x4, x5, [x0]
|       |   0x06d925f8      bl    0x4da3500
|       |   0x06d925fc      stp   x4, x5, [x25]
|       `-> 0x06d92600      adrp  x4, 0x8f93000
|           0x06d92604      add   x4, x4, 0x878
|           0x06d92608      adrp  x5, 0x8f93000
|           0x06d9260c      add   x5, x5, 0x768
|           0x06d92610      stp   x4, x5, [x0]
|           0x06d92614      adrp  x0, 0x8333000
|           0x06d92618      add   x0, x0, 0x110
|           0x06d9261c      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92620      adrp  x2, 0x4327000
|           0x06d92624      add   x2, x2, 0xe
|           0x06d92628      orr   x3, xzr, 0x10
|           0x06d9262c      bl    0x4d18e50
|           0x06d92630      ldrsb x27, [x0]
|           0x06d92634      adrp  x27, 0xb48f000
|           0x06d92638      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9263c      cbnz  w4, 0x6d92648
|       |   0x06d92640      ldr   x2, [arg_70h]                        ; [0x30:4]=0
|      ,==< 0x06d92644      b     0x6d92664
|      |`-> 0x06d92648      adrp  x27, 0x957c000
|      |    0x06d9264c      ldr   x1, [x27, 0xee0]                     ; [0xee0:4]=0x58585858
|      |    0x06d92650      ldp   x2, x3, [x0]
|      |    0x06d92654      bl    0x4da3520
|      |    0x06d92658      stp   x2, x3, [x25]
|      |    0x06d9265c      ldr   x2, [arg_30h]                        ; [0x30:4]=0
|      |    0x06d92660      stp   x2, x1, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92644
|      `--> 0x06d92664      adrp  x1, 0x8f93000
|           0x06d92668      add   x1, x1, 0x8c8
|           0x06d9266c      adrp  x3, 0x8f93000
|           0x06d92670      add   x3, x3, 0x7c0
|           0x06d92674      stp   x1, x3, [x0]
|           0x06d92678      adrp  x27, 0x957c000
|           0x06d9267c      str   x2, [x27, 0xee0]
|           0x06d92680      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d92684      ldr   lr, [sp], 0x40                       ; segment.PHDR
|                                                                      ; [0x40:4]=6 ; "\U00000006"
\           0x06d92688      ret
==== GatewayConfig_.IsRouteWebFetch
/ fcn.06d92368(int64_t arg1, int64_t arg_28h, int64_t arg_30h, int64_t arg_40h, int64_t arg_70h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg_28h @ stack + 0x28
|           ; arg int64_t arg_30h @ stack + 0x30
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_70h @ stack + 0x70
|       ,=< 0x06d92368      cbz   w3, 0x6d92378
|       |   0x06d9236c      ldp   x4, x5, [x0]
|       |   0x06d92370      bl    0x4da3500
|       |   0x06d92374      stp   x4, x5, [x25]
|       `-> 0x06d92378      adrp  x4, 0x8f93000
|           0x06d9237c      add   x4, x4, 0x8f0
|           0x06d92380      adrp  x5, 0x8f93000
|           0x06d92384      add   x5, x5, 0x7f8
|           0x06d92388      stp   x4, x5, [x0]                         ; arg1
|           0x06d9238c      adrp  x0, 0x8333000
|           0x06d92390      add   x0, x0, 0x110
|           0x06d92394      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92398      adrp  x2, 0x42be000
|           0x06d9239c      add   x2, x2, 0xd0f
|           0x06d923a0      mov   x3, 0xb
|           0x06d923a4      bl    0x4d18e50
|           0x06d923a8      ldrsb x27, [x0]
|           0x06d923ac      adrp  x27, 0xb48f000
|           0x06d923b0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d923b4      cbz   w4, 0x6d923c4
|       |   0x06d923b8      ldp   x4, x5, [x0]
|       |   0x06d923bc      bl    0x4da3500
|       |   0x06d923c0      stp   x4, x5, [x25]
|       `-> 0x06d923c4      adrp  x4, 0x8f93000
|           0x06d923c8      add   x4, x4, 0x8e0
|           0x06d923cc      adrp  x5, 0x8f93000
|           0x06d923d0      add   x5, x5, 0x7e0
|           0x06d923d4      stp   x4, x5, [x0]
|           0x06d923d8      adrp  x27, 0x948c000
|           0x06d923dc      ldr   x4, [x27, 0xd20]                     ; [0xd20:4]=0x58585858
|           0x06d923e0      str   x4, [arg_28h]
|           0x06d923e4      adrp  x0, 0x8333000
|           0x06d923e8      add   x0, x0, 0x110
|           0x06d923ec      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d923f0      adrp  x2, 0x4326000
|           0x06d923f4      add   x2, x2, 0xbfe
|           0x06d923f8      orr   x3, xzr, 0x10
|           0x06d923fc      bl    0x4d18e50
|           0x06d92400      ldrsb x27, [x0]
|           0x06d92404      adrp  x27, 0xb48f000
|           0x06d92408      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9240c      cbnz  w4, 0x6d92418
|       |   0x06d92410      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      ,==< 0x06d92414      b     0x6d9242c
|      |`-> 0x06d92418      ldp   x4, x5, [x0]
|      |    0x06d9241c      bl    0x4da3510
|      |    0x06d92420      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      |    0x06d92424      stp   x6, x4, [x25]
|      |    0x06d92428      str   x5, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92414
|      `--> 0x06d9242c      adrp  x4, 0x8f93000
|           0x06d92430      add   x4, x4, 0x7d8
|           0x06d92434      stp   x6, x4, [x0]
|           0x06d92438      adrp  x0, 0x8333000
|           0x06d9243c      add   x0, x0, 0x110
|           0x06d92440      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92444      adrp  x2, 0x42d6000
|           0x06d92448      add   x2, x2, 0x4dd
|           0x06d9244c      orr   x3, xzr, 0xc
|           0x06d92450      bl    0x4d18e50
|           0x06d92454      ldrsb x27, [x0]
|           0x06d92458      adrp  x27, 0xb48f000
|           0x06d9245c      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92460      cbz   w4, 0x6d92470
|       |   0x06d92464      ldp   x4, x5, [x0]
|       |   0x06d92468      bl    0x4da3500
|       |   0x06d9246c      stp   x4, x5, [x25]
|       `-> 0x06d92470      adrp  x4, 0x8f93000
|           0x06d92474      add   x4, x4, 0x938
|           0x06d92478      adrp  x5, 0x8f93000
|           0x06d9247c      add   x5, x5, 0x830
|           0x06d92480      stp   x4, x5, [x0]
|           0x06d92484      adrp  x0, 0x8333000
|           0x06d92488      add   x0, x0, 0x110
|           0x06d9248c      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92490      adrp  x2, 0x42d6000
|           0x06d92494      add   x2, x2, 0x9a5
|           0x06d92498      orr   x3, xzr, 0xc
|           0x06d9249c      bl    0x4d18e50
|           0x06d924a0      ldrsb x27, [x0]
|           0x06d924a4      adrp  x27, 0xb48f000
|           0x06d924a8      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d924ac      cbz   w4, 0x6d924bc
|       |   0x06d924b0      ldp   x4, x5, [x0]
|       |   0x06d924b4      bl    0x4da3500
|       |   0x06d924b8      stp   x4, x5, [x25]
|       `-> 0x06d924bc      adrp  x4, 0x8f93000
|           0x06d924c0      add   x4, x4, 0x8d8
|           0x06d924c4      adrp  x5, 0x8f93000
|           0x06d924c8      add   x5, x5, 0x7d0
|           0x06d924cc      stp   x4, x5, [x0]
|           0x06d924d0      adrp  x0, 0x8333000
|           0x06d924d4      add   x0, x0, 0x110
|           0x06d924d8      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d924dc      adrp  x2, 0x4279000
|           0x06d924e0      add   x2, x2, 0xd1b
|           0x06d924e4      orr   x3, xzr, 8
|           0x06d924e8      bl    0x4d18e50
|           0x06d924ec      ldrsb x27, [x0]
|           0x06d924f0      adrp  x27, 0xb48f000
|           0x06d924f4      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d924f8      cbz   w4, 0x6d92508
|       |   0x06d924fc      ldp   x4, x5, [x0]
|       |   0x06d92500      bl    0x4da3500
|       |   0x06d92504      stp   x4, x5, [x25]
|       `-> 0x06d92508      adrp  x4, 0x8f93000
|           0x06d9250c      add   x4, x4, 0x918
|           0x06d92510      adrp  x5, 0x8f93000
|           0x06d92514      add   x5, x5, 0x818
|           0x06d92518      stp   x4, x5, [x0]
|           0x06d9251c      adrp  x27, 0x948c000
|           0x06d92520      ldr   x4, [x27, 0xd10]                     ; [0xd10:4]=0x58585858
|           0x06d92524      str   x4, [arg_28h]
|           0x06d92528      adrp  x0, 0x8333000
|           0x06d9252c      add   x0, x0, 0x110
|           0x06d92530      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92534      adrp  x2, 0x4314000
|           0x06d92538      add   x2, x2, 0x3ab
|           0x06d9253c      orr   x3, xzr, 0xf
|           0x06d92540      bl    0x4d18e50
|           0x06d92544      ldrsb x27, [x0]
|           0x06d92548      adrp  x27, 0xb48f000
|           0x06d9254c      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92550      cbnz  w4, 0x6d9255c
|       |   0x06d92554      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      ,==< 0x06d92558      b     0x6d92570
|      |`-> 0x06d9255c      ldp   x4, x5, [x0]
|      |    0x06d92560      bl    0x4da3510
|      |    0x06d92564      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      |    0x06d92568      stp   x6, x4, [x25]
|      |    0x06d9256c      str   x5, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92558
|      `--> 0x06d92570      adrp  x4, 0x8f93000
|           0x06d92574      add   x4, x4, 0x790
|           0x06d92578      stp   x6, x4, [x0]
|           0x06d9257c      adrp  x0, 0x8333000
|           0x06d92580      add   x0, x0, 0x110
|           0x06d92584      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92588      adrp  x2, 0x4301000
|           0x06d9258c      add   x2, x2, 0xb0e
|           0x06d92590      orr   x3, xzr, 0xe
|           0x06d92594      bl    0x4d18e50
|           0x06d92598      ldrsb x27, [x0]
|           0x06d9259c      adrp  x27, 0xb48f000
|           0x06d925a0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d925a4      cbz   w4, 0x6d925b4
|       |   0x06d925a8      ldp   x4, x5, [x0]
|       |   0x06d925ac      bl    0x4da3500
|       |   0x06d925b0      stp   x4, x5, [x25]
|       `-> 0x06d925b4      adrp  x4, 0x8f93000
|           0x06d925b8      add   x4, x4, 0x880
|           0x06d925bc      adrp  x5, 0x8f93000
|           0x06d925c0      add   x5, x5, 0x768
|           0x06d925c4      stp   x4, x5, [x0]
|           0x06d925c8      adrp  x0, 0x8333000
|           0x06d925cc      add   x0, x0, 0x110
|           0x06d925d0      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d925d4      adrp  x2, 0x4376000
|           0x06d925d8      add   x2, x2, 0xfd8
|           0x06d925dc      mov   x3, 0x15
|           0x06d925e0      bl    0x4d18e50
|           0x06d925e4      ldrsb x27, [x0]
|           0x06d925e8      adrp  x27, 0xb48f000
|           0x06d925ec      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d925f0      cbz   w4, 0x6d92600
|       |   0x06d925f4      ldp   x4, x5, [x0]
|       |   0x06d925f8      bl    0x4da3500
|       |   0x06d925fc      stp   x4, x5, [x25]
|       `-> 0x06d92600      adrp  x4, 0x8f93000
|           0x06d92604      add   x4, x4, 0x878
|           0x06d92608      adrp  x5, 0x8f93000
|           0x06d9260c      add   x5, x5, 0x768
|           0x06d92610      stp   x4, x5, [x0]
|           0x06d92614      adrp  x0, 0x8333000
|           0x06d92618      add   x0, x0, 0x110
|           0x06d9261c      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92620      adrp  x2, 0x4327000
|           0x06d92624      add   x2, x2, 0xe
|           0x06d92628      orr   x3, xzr, 0x10
|           0x06d9262c      bl    0x4d18e50
|           0x06d92630      ldrsb x27, [x0]
|           0x06d92634      adrp  x27, 0xb48f000
|           0x06d92638      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9263c      cbnz  w4, 0x6d92648
|       |   0x06d92640      ldr   x2, [arg_70h]                        ; [0x30:4]=0
|      ,==< 0x06d92644      b     0x6d92664
|      |`-> 0x06d92648      adrp  x27, 0x957c000
|      |    0x06d9264c      ldr   x1, [x27, 0xee0]                     ; [0xee0:4]=0x58585858
|      |    0x06d92650      ldp   x2, x3, [x0]
|      |    0x06d92654      bl    0x4da3520
|      |    0x06d92658      stp   x2, x3, [x25]
|      |    0x06d9265c      ldr   x2, [arg_30h]                        ; [0x30:4]=0
|      |    0x06d92660      stp   x2, x1, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92644
|      `--> 0x06d92664      adrp  x1, 0x8f93000
|           0x06d92668      add   x1, x1, 0x8c8
|           0x06d9266c      adrp  x3, 0x8f93000
|           0x06d92670      add   x3, x3, 0x7c0
|           0x06d92674      stp   x1, x3, [x0]
|           0x06d92678      adrp  x27, 0x957c000
|           0x06d9267c      str   x2, [x27, 0xee0]
|           0x06d92680      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d92684      ldr   lr, [sp], 0x40                       ; segment.PHDR
|                                                                      ; [0x40:4]=6 ; "\U00000006"
\           0x06d92688      ret
==== AdminSettings_.IsGatewayEnabled
/ fcn.06d92368(int64_t arg1, int64_t arg_28h, int64_t arg_30h, int64_t arg_40h, int64_t arg_70h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg_28h @ stack + 0x28
|           ; arg int64_t arg_30h @ stack + 0x30
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_70h @ stack + 0x70
|       ,=< 0x06d92368      cbz   w3, 0x6d92378
|       |   0x06d9236c      ldp   x4, x5, [x0]
|       |   0x06d92370      bl    0x4da3500
|       |   0x06d92374      stp   x4, x5, [x25]
|       `-> 0x06d92378      adrp  x4, 0x8f93000
|           0x06d9237c      add   x4, x4, 0x8f0
|           0x06d92380      adrp  x5, 0x8f93000
|           0x06d92384      add   x5, x5, 0x7f8
|           0x06d92388      stp   x4, x5, [x0]                         ; arg1
|           0x06d9238c      adrp  x0, 0x8333000
|           0x06d92390      add   x0, x0, 0x110
|           0x06d92394      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92398      adrp  x2, 0x42be000
|           0x06d9239c      add   x2, x2, 0xd0f
|           0x06d923a0      mov   x3, 0xb
|           0x06d923a4      bl    0x4d18e50
|           0x06d923a8      ldrsb x27, [x0]
|           0x06d923ac      adrp  x27, 0xb48f000
|           0x06d923b0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d923b4      cbz   w4, 0x6d923c4
|       |   0x06d923b8      ldp   x4, x5, [x0]
|       |   0x06d923bc      bl    0x4da3500
|       |   0x06d923c0      stp   x4, x5, [x25]
|       `-> 0x06d923c4      adrp  x4, 0x8f93000
|           0x06d923c8      add   x4, x4, 0x8e0
|           0x06d923cc      adrp  x5, 0x8f93000
|           0x06d923d0      add   x5, x5, 0x7e0
|           0x06d923d4      stp   x4, x5, [x0]
|           0x06d923d8      adrp  x27, 0x948c000
|           0x06d923dc      ldr   x4, [x27, 0xd20]                     ; [0xd20:4]=0x58585858
|           0x06d923e0      str   x4, [arg_28h]
|           0x06d923e4      adrp  x0, 0x8333000
|           0x06d923e8      add   x0, x0, 0x110
|           0x06d923ec      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d923f0      adrp  x2, 0x4326000
|           0x06d923f4      add   x2, x2, 0xbfe
|           0x06d923f8      orr   x3, xzr, 0x10
|           0x06d923fc      bl    0x4d18e50
|           0x06d92400      ldrsb x27, [x0]
|           0x06d92404      adrp  x27, 0xb48f000
|           0x06d92408      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9240c      cbnz  w4, 0x6d92418
|       |   0x06d92410      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      ,==< 0x06d92414      b     0x6d9242c
|      |`-> 0x06d92418      ldp   x4, x5, [x0]
|      |    0x06d9241c      bl    0x4da3510
|      |    0x06d92420      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      |    0x06d92424      stp   x6, x4, [x25]
|      |    0x06d92428      str   x5, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92414
|      `--> 0x06d9242c      adrp  x4, 0x8f93000
|           0x06d92430      add   x4, x4, 0x7d8
|           0x06d92434      stp   x6, x4, [x0]
|           0x06d92438      adrp  x0, 0x8333000
|           0x06d9243c      add   x0, x0, 0x110
|           0x06d92440      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92444      adrp  x2, 0x42d6000
|           0x06d92448      add   x2, x2, 0x4dd
|           0x06d9244c      orr   x3, xzr, 0xc
|           0x06d92450      bl    0x4d18e50
|           0x06d92454      ldrsb x27, [x0]
|           0x06d92458      adrp  x27, 0xb48f000
|           0x06d9245c      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92460      cbz   w4, 0x6d92470
|       |   0x06d92464      ldp   x4, x5, [x0]
|       |   0x06d92468      bl    0x4da3500
|       |   0x06d9246c      stp   x4, x5, [x25]
|       `-> 0x06d92470      adrp  x4, 0x8f93000
|           0x06d92474      add   x4, x4, 0x938
|           0x06d92478      adrp  x5, 0x8f93000
|           0x06d9247c      add   x5, x5, 0x830
|           0x06d92480      stp   x4, x5, [x0]
|           0x06d92484      adrp  x0, 0x8333000
|           0x06d92488      add   x0, x0, 0x110
|           0x06d9248c      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92490      adrp  x2, 0x42d6000
|           0x06d92494      add   x2, x2, 0x9a5
|           0x06d92498      orr   x3, xzr, 0xc
|           0x06d9249c      bl    0x4d18e50
|           0x06d924a0      ldrsb x27, [x0]
|           0x06d924a4      adrp  x27, 0xb48f000
|           0x06d924a8      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d924ac      cbz   w4, 0x6d924bc
|       |   0x06d924b0      ldp   x4, x5, [x0]
|       |   0x06d924b4      bl    0x4da3500
|       |   0x06d924b8      stp   x4, x5, [x25]
|       `-> 0x06d924bc      adrp  x4, 0x8f93000
|           0x06d924c0      add   x4, x4, 0x8d8
|           0x06d924c4      adrp  x5, 0x8f93000
|           0x06d924c8      add   x5, x5, 0x7d0
|           0x06d924cc      stp   x4, x5, [x0]
|           0x06d924d0      adrp  x0, 0x8333000
|           0x06d924d4      add   x0, x0, 0x110
|           0x06d924d8      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d924dc      adrp  x2, 0x4279000
|           0x06d924e0      add   x2, x2, 0xd1b
|           0x06d924e4      orr   x3, xzr, 8
|           0x06d924e8      bl    0x4d18e50
|           0x06d924ec      ldrsb x27, [x0]
|           0x06d924f0      adrp  x27, 0xb48f000
|           0x06d924f4      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d924f8      cbz   w4, 0x6d92508
|       |   0x06d924fc      ldp   x4, x5, [x0]
|       |   0x06d92500      bl    0x4da3500
|       |   0x06d92504      stp   x4, x5, [x25]
|       `-> 0x06d92508      adrp  x4, 0x8f93000
|           0x06d9250c      add   x4, x4, 0x918
|           0x06d92510      adrp  x5, 0x8f93000
|           0x06d92514      add   x5, x5, 0x818
|           0x06d92518      stp   x4, x5, [x0]
|           0x06d9251c      adrp  x27, 0x948c000
|           0x06d92520      ldr   x4, [x27, 0xd10]                     ; [0xd10:4]=0x58585858
|           0x06d92524      str   x4, [arg_28h]
|           0x06d92528      adrp  x0, 0x8333000
|           0x06d9252c      add   x0, x0, 0x110
|           0x06d92530      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92534      adrp  x2, 0x4314000
|           0x06d92538      add   x2, x2, 0x3ab
|           0x06d9253c      orr   x3, xzr, 0xf
|           0x06d92540      bl    0x4d18e50
|           0x06d92544      ldrsb x27, [x0]
|           0x06d92548      adrp  x27, 0xb48f000
|           0x06d9254c      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92550      cbnz  w4, 0x6d9255c
|       |   0x06d92554      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      ,==< 0x06d92558      b     0x6d92570
|      |`-> 0x06d9255c      ldp   x4, x5, [x0]
|      |    0x06d92560      bl    0x4da3510
|      |    0x06d92564      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      |    0x06d92568      stp   x6, x4, [x25]
|      |    0x06d9256c      str   x5, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92558
|      `--> 0x06d92570      adrp  x4, 0x8f93000
|           0x06d92574      add   x4, x4, 0x790
|           0x06d92578      stp   x6, x4, [x0]
|           0x06d9257c      adrp  x0, 0x8333000
|           0x06d92580      add   x0, x0, 0x110
|           0x06d92584      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92588      adrp  x2, 0x4301000
|           0x06d9258c      add   x2, x2, 0xb0e
|           0x06d92590      orr   x3, xzr, 0xe
|           0x06d92594      bl    0x4d18e50
|           0x06d92598      ldrsb x27, [x0]
|           0x06d9259c      adrp  x27, 0xb48f000
|           0x06d925a0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d925a4      cbz   w4, 0x6d925b4
|       |   0x06d925a8      ldp   x4, x5, [x0]
|       |   0x06d925ac      bl    0x4da3500
|       |   0x06d925b0      stp   x4, x5, [x25]
|       `-> 0x06d925b4      adrp  x4, 0x8f93000
|           0x06d925b8      add   x4, x4, 0x880
|           0x06d925bc      adrp  x5, 0x8f93000
|           0x06d925c0      add   x5, x5, 0x768
|           0x06d925c4      stp   x4, x5, [x0]
|           0x06d925c8      adrp  x0, 0x8333000
|           0x06d925cc      add   x0, x0, 0x110
|           0x06d925d0      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d925d4      adrp  x2, 0x4376000
|           0x06d925d8      add   x2, x2, 0xfd8
|           0x06d925dc      mov   x3, 0x15
|           0x06d925e0      bl    0x4d18e50
|           0x06d925e4      ldrsb x27, [x0]
|           0x06d925e8      adrp  x27, 0xb48f000
|           0x06d925ec      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d925f0      cbz   w4, 0x6d92600
|       |   0x06d925f4      ldp   x4, x5, [x0]
|       |   0x06d925f8      bl    0x4da3500
|       |   0x06d925fc      stp   x4, x5, [x25]
|       `-> 0x06d92600      adrp  x4, 0x8f93000
|           0x06d92604      add   x4, x4, 0x878
|           0x06d92608      adrp  x5, 0x8f93000
|           0x06d9260c      add   x5, x5, 0x768
|           0x06d92610      stp   x4, x5, [x0]
|           0x06d92614      adrp  x0, 0x8333000
|           0x06d92618      add   x0, x0, 0x110
|           0x06d9261c      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92620      adrp  x2, 0x4327000
|           0x06d92624      add   x2, x2, 0xe
|           0x06d92628      orr   x3, xzr, 0x10
|           0x06d9262c      bl    0x4d18e50
|           0x06d92630      ldrsb x27, [x0]
|           0x06d92634      adrp  x27, 0xb48f000
|           0x06d92638      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9263c      cbnz  w4, 0x6d92648
|       |   0x06d92640      ldr   x2, [arg_70h]                        ; [0x30:4]=0
|      ,==< 0x06d92644      b     0x6d92664
|      |`-> 0x06d92648      adrp  x27, 0x957c000
|      |    0x06d9264c      ldr   x1, [x27, 0xee0]                     ; [0xee0:4]=0x58585858
|      |    0x06d92650      ldp   x2, x3, [x0]
|      |    0x06d92654      bl    0x4da3520
|      |    0x06d92658      stp   x2, x3, [x25]
|      |    0x06d9265c      ldr   x2, [arg_30h]                        ; [0x30:4]=0
|      |    0x06d92660      stp   x2, x1, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92644
|      `--> 0x06d92664      adrp  x1, 0x8f93000
|           0x06d92668      add   x1, x1, 0x8c8
|           0x06d9266c      adrp  x3, 0x8f93000
|           0x06d92670      add   x3, x3, 0x7c0
|           0x06d92674      stp   x1, x3, [x0]
|           0x06d92678      adrp  x27, 0x957c000
|           0x06d9267c      str   x2, [x27, 0xee0]
|           0x06d92680      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d92684      ldr   lr, [sp], 0x40                       ; segment.PHDR
|                                                                      ; [0x40:4]=6 ; "\U00000006"
\           0x06d92688      ret
==== AdminSettings_.AdminFileSetsModels
/ fcn.06d92368(int64_t arg1, int64_t arg_28h, int64_t arg_30h, int64_t arg_40h, int64_t arg_70h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg_28h @ stack + 0x28
|           ; arg int64_t arg_30h @ stack + 0x30
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_70h @ stack + 0x70
|       ,=< 0x06d92368      cbz   w3, 0x6d92378
|       |   0x06d9236c      ldp   x4, x5, [x0]
|       |   0x06d92370      bl    0x4da3500
|       |   0x06d92374      stp   x4, x5, [x25]
|       `-> 0x06d92378      adrp  x4, 0x8f93000
|           0x06d9237c      add   x4, x4, 0x8f0
|           0x06d92380      adrp  x5, 0x8f93000
|           0x06d92384      add   x5, x5, 0x7f8
|           0x06d92388      stp   x4, x5, [x0]                         ; arg1
|           0x06d9238c      adrp  x0, 0x8333000
|           0x06d92390      add   x0, x0, 0x110
|           0x06d92394      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92398      adrp  x2, 0x42be000
|           0x06d9239c      add   x2, x2, 0xd0f
|           0x06d923a0      mov   x3, 0xb
|           0x06d923a4      bl    0x4d18e50
|           0x06d923a8      ldrsb x27, [x0]
|           0x06d923ac      adrp  x27, 0xb48f000
|           0x06d923b0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d923b4      cbz   w4, 0x6d923c4
|       |   0x06d923b8      ldp   x4, x5, [x0]
|       |   0x06d923bc      bl    0x4da3500
|       |   0x06d923c0      stp   x4, x5, [x25]
|       `-> 0x06d923c4      adrp  x4, 0x8f93000
|           0x06d923c8      add   x4, x4, 0x8e0
|           0x06d923cc      adrp  x5, 0x8f93000
|           0x06d923d0      add   x5, x5, 0x7e0
|           0x06d923d4      stp   x4, x5, [x0]
|           0x06d923d8      adrp  x27, 0x948c000
|           0x06d923dc      ldr   x4, [x27, 0xd20]                     ; [0xd20:4]=0x58585858
|           0x06d923e0      str   x4, [arg_28h]
|           0x06d923e4      adrp  x0, 0x8333000
|           0x06d923e8      add   x0, x0, 0x110
|           0x06d923ec      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d923f0      adrp  x2, 0x4326000
|           0x06d923f4      add   x2, x2, 0xbfe
|           0x06d923f8      orr   x3, xzr, 0x10
|           0x06d923fc      bl    0x4d18e50
|           0x06d92400      ldrsb x27, [x0]
|           0x06d92404      adrp  x27, 0xb48f000
|           0x06d92408      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9240c      cbnz  w4, 0x6d92418
|       |   0x06d92410      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      ,==< 0x06d92414      b     0x6d9242c
|      |`-> 0x06d92418      ldp   x4, x5, [x0]
|      |    0x06d9241c      bl    0x4da3510
|      |    0x06d92420      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      |    0x06d92424      stp   x6, x4, [x25]
|      |    0x06d92428      str   x5, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92414
|      `--> 0x06d9242c      adrp  x4, 0x8f93000
|           0x06d92430      add   x4, x4, 0x7d8
|           0x06d92434      stp   x6, x4, [x0]
|           0x06d92438      adrp  x0, 0x8333000
|           0x06d9243c      add   x0, x0, 0x110
|           0x06d92440      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92444      adrp  x2, 0x42d6000
|           0x06d92448      add   x2, x2, 0x4dd
|           0x06d9244c      orr   x3, xzr, 0xc
|           0x06d92450      bl    0x4d18e50
|           0x06d92454      ldrsb x27, [x0]
|           0x06d92458      adrp  x27, 0xb48f000
|           0x06d9245c      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92460      cbz   w4, 0x6d92470
|       |   0x06d92464      ldp   x4, x5, [x0]
|       |   0x06d92468      bl    0x4da3500
|       |   0x06d9246c      stp   x4, x5, [x25]
|       `-> 0x06d92470      adrp  x4, 0x8f93000
|           0x06d92474      add   x4, x4, 0x938
|           0x06d92478      adrp  x5, 0x8f93000
|           0x06d9247c      add   x5, x5, 0x830
|           0x06d92480      stp   x4, x5, [x0]
|           0x06d92484      adrp  x0, 0x8333000
|           0x06d92488      add   x0, x0, 0x110
|           0x06d9248c      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92490      adrp  x2, 0x42d6000
|           0x06d92494      add   x2, x2, 0x9a5
|           0x06d92498      orr   x3, xzr, 0xc
|           0x06d9249c      bl    0x4d18e50
|           0x06d924a0      ldrsb x27, [x0]
|           0x06d924a4      adrp  x27, 0xb48f000
|           0x06d924a8      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d924ac      cbz   w4, 0x6d924bc
|       |   0x06d924b0      ldp   x4, x5, [x0]
|       |   0x06d924b4      bl    0x4da3500
|       |   0x06d924b8      stp   x4, x5, [x25]
|       `-> 0x06d924bc      adrp  x4, 0x8f93000
|           0x06d924c0      add   x4, x4, 0x8d8
|           0x06d924c4      adrp  x5, 0x8f93000
|           0x06d924c8      add   x5, x5, 0x7d0
|           0x06d924cc      stp   x4, x5, [x0]
|           0x06d924d0      adrp  x0, 0x8333000
|           0x06d924d4      add   x0, x0, 0x110
|           0x06d924d8      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d924dc      adrp  x2, 0x4279000
|           0x06d924e0      add   x2, x2, 0xd1b
|           0x06d924e4      orr   x3, xzr, 8
|           0x06d924e8      bl    0x4d18e50
|           0x06d924ec      ldrsb x27, [x0]
|           0x06d924f0      adrp  x27, 0xb48f000
|           0x06d924f4      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d924f8      cbz   w4, 0x6d92508
|       |   0x06d924fc      ldp   x4, x5, [x0]
|       |   0x06d92500      bl    0x4da3500
|       |   0x06d92504      stp   x4, x5, [x25]
|       `-> 0x06d92508      adrp  x4, 0x8f93000
|           0x06d9250c      add   x4, x4, 0x918
|           0x06d92510      adrp  x5, 0x8f93000
|           0x06d92514      add   x5, x5, 0x818
|           0x06d92518      stp   x4, x5, [x0]
|           0x06d9251c      adrp  x27, 0x948c000
|           0x06d92520      ldr   x4, [x27, 0xd10]                     ; [0xd10:4]=0x58585858
|           0x06d92524      str   x4, [arg_28h]
|           0x06d92528      adrp  x0, 0x8333000
|           0x06d9252c      add   x0, x0, 0x110
|           0x06d92530      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92534      adrp  x2, 0x4314000
|           0x06d92538      add   x2, x2, 0x3ab
|           0x06d9253c      orr   x3, xzr, 0xf
|           0x06d92540      bl    0x4d18e50
|           0x06d92544      ldrsb x27, [x0]
|           0x06d92548      adrp  x27, 0xb48f000
|           0x06d9254c      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92550      cbnz  w4, 0x6d9255c
|       |   0x06d92554      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      ,==< 0x06d92558      b     0x6d92570
|      |`-> 0x06d9255c      ldp   x4, x5, [x0]
|      |    0x06d92560      bl    0x4da3510
|      |    0x06d92564      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      |    0x06d92568      stp   x6, x4, [x25]
|      |    0x06d9256c      str   x5, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92558
|      `--> 0x06d92570      adrp  x4, 0x8f93000
|           0x06d92574      add   x4, x4, 0x790
|           0x06d92578      stp   x6, x4, [x0]
|           0x06d9257c      adrp  x0, 0x8333000
|           0x06d92580      add   x0, x0, 0x110
|           0x06d92584      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92588      adrp  x2, 0x4301000
|           0x06d9258c      add   x2, x2, 0xb0e
|           0x06d92590      orr   x3, xzr, 0xe
|           0x06d92594      bl    0x4d18e50
|           0x06d92598      ldrsb x27, [x0]
|           0x06d9259c      adrp  x27, 0xb48f000
|           0x06d925a0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d925a4      cbz   w4, 0x6d925b4
|       |   0x06d925a8      ldp   x4, x5, [x0]
|       |   0x06d925ac      bl    0x4da3500
|       |   0x06d925b0      stp   x4, x5, [x25]
|       `-> 0x06d925b4      adrp  x4, 0x8f93000
|           0x06d925b8      add   x4, x4, 0x880
|           0x06d925bc      adrp  x5, 0x8f93000
|           0x06d925c0      add   x5, x5, 0x768
|           0x06d925c4      stp   x4, x5, [x0]
|           0x06d925c8      adrp  x0, 0x8333000
|           0x06d925cc      add   x0, x0, 0x110
|           0x06d925d0      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d925d4      adrp  x2, 0x4376000
|           0x06d925d8      add   x2, x2, 0xfd8
|           0x06d925dc      mov   x3, 0x15
|           0x06d925e0      bl    0x4d18e50
|           0x06d925e4      ldrsb x27, [x0]
|           0x06d925e8      adrp  x27, 0xb48f000
|           0x06d925ec      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d925f0      cbz   w4, 0x6d92600
|       |   0x06d925f4      ldp   x4, x5, [x0]
|       |   0x06d925f8      bl    0x4da3500
|       |   0x06d925fc      stp   x4, x5, [x25]
|       `-> 0x06d92600      adrp  x4, 0x8f93000
|           0x06d92604      add   x4, x4, 0x878
|           0x06d92608      adrp  x5, 0x8f93000
|           0x06d9260c      add   x5, x5, 0x768
|           0x06d92610      stp   x4, x5, [x0]
|           0x06d92614      adrp  x0, 0x8333000
|           0x06d92618      add   x0, x0, 0x110
|           0x06d9261c      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92620      adrp  x2, 0x4327000
|           0x06d92624      add   x2, x2, 0xe
|           0x06d92628      orr   x3, xzr, 0x10
|           0x06d9262c      bl    0x4d18e50
|           0x06d92630      ldrsb x27, [x0]
|           0x06d92634      adrp  x27, 0xb48f000
|           0x06d92638      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9263c      cbnz  w4, 0x6d92648
|       |   0x06d92640      ldr   x2, [arg_70h]                        ; [0x30:4]=0
|      ,==< 0x06d92644      b     0x6d92664
|      |`-> 0x06d92648      adrp  x27, 0x957c000
|      |    0x06d9264c      ldr   x1, [x27, 0xee0]                     ; [0xee0:4]=0x58585858
|      |    0x06d92650      ldp   x2, x3, [x0]
|      |    0x06d92654      bl    0x4da3520
|      |    0x06d92658      stp   x2, x3, [x25]
|      |    0x06d9265c      ldr   x2, [arg_30h]                        ; [0x30:4]=0
|      |    0x06d92660      stp   x2, x1, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92644
|      `--> 0x06d92664      adrp  x1, 0x8f93000
|           0x06d92668      add   x1, x1, 0x8c8
|           0x06d9266c      adrp  x3, 0x8f93000
|           0x06d92670      add   x3, x3, 0x7c0
|           0x06d92674      stp   x1, x3, [x0]
|           0x06d92678      adrp  x27, 0x957c000
|           0x06d9267c      str   x2, [x27, 0xee0]
|           0x06d92680      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d92684      ldr   lr, [sp], 0x40                       ; segment.PHDR
|                                                                      ; [0x40:4]=6 ; "\U00000006"
\           0x06d92688      ret
==== google3_third_party_jetski_gateway_gateway.defaultAdminSettingsPathForOS
/ fcn.06d92368(int64_t arg1, int64_t arg_28h, int64_t arg_30h, int64_t arg_40h, int64_t arg_70h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg_28h @ stack + 0x28
|           ; arg int64_t arg_30h @ stack + 0x30
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_70h @ stack + 0x70
|       ,=< 0x06d92368      cbz   w3, 0x6d92378
|       |   0x06d9236c      ldp   x4, x5, [x0]
|       |   0x06d92370      bl    0x4da3500
|       |   0x06d92374      stp   x4, x5, [x25]
|       `-> 0x06d92378      adrp  x4, 0x8f93000
|           0x06d9237c      add   x4, x4, 0x8f0
|           0x06d92380      adrp  x5, 0x8f93000
|           0x06d92384      add   x5, x5, 0x7f8
|           0x06d92388      stp   x4, x5, [x0]                         ; arg1
|           0x06d9238c      adrp  x0, 0x8333000
|           0x06d92390      add   x0, x0, 0x110
|           0x06d92394      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92398      adrp  x2, 0x42be000
|           0x06d9239c      add   x2, x2, 0xd0f
|           0x06d923a0      mov   x3, 0xb
|           0x06d923a4      bl    0x4d18e50
|           0x06d923a8      ldrsb x27, [x0]
|           0x06d923ac      adrp  x27, 0xb48f000
|           0x06d923b0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d923b4      cbz   w4, 0x6d923c4
|       |   0x06d923b8      ldp   x4, x5, [x0]
|       |   0x06d923bc      bl    0x4da3500
|       |   0x06d923c0      stp   x4, x5, [x25]
|       `-> 0x06d923c4      adrp  x4, 0x8f93000
|           0x06d923c8      add   x4, x4, 0x8e0
|           0x06d923cc      adrp  x5, 0x8f93000
|           0x06d923d0      add   x5, x5, 0x7e0
|           0x06d923d4      stp   x4, x5, [x0]
|           0x06d923d8      adrp  x27, 0x948c000
|           0x06d923dc      ldr   x4, [x27, 0xd20]                     ; [0xd20:4]=0x58585858
|           0x06d923e0      str   x4, [arg_28h]
|           0x06d923e4      adrp  x0, 0x8333000
|           0x06d923e8      add   x0, x0, 0x110
|           0x06d923ec      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d923f0      adrp  x2, 0x4326000
|           0x06d923f4      add   x2, x2, 0xbfe
|           0x06d923f8      orr   x3, xzr, 0x10
|           0x06d923fc      bl    0x4d18e50
|           0x06d92400      ldrsb x27, [x0]
|           0x06d92404      adrp  x27, 0xb48f000
|           0x06d92408      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9240c      cbnz  w4, 0x6d92418
|       |   0x06d92410      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      ,==< 0x06d92414      b     0x6d9242c
|      |`-> 0x06d92418      ldp   x4, x5, [x0]
|      |    0x06d9241c      bl    0x4da3510
|      |    0x06d92420      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      |    0x06d92424      stp   x6, x4, [x25]
|      |    0x06d92428      str   x5, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92414
|      `--> 0x06d9242c      adrp  x4, 0x8f93000
|           0x06d92430      add   x4, x4, 0x7d8
|           0x06d92434      stp   x6, x4, [x0]
|           0x06d92438      adrp  x0, 0x8333000
|           0x06d9243c      add   x0, x0, 0x110
|           0x06d92440      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92444      adrp  x2, 0x42d6000
|           0x06d92448      add   x2, x2, 0x4dd
|           0x06d9244c      orr   x3, xzr, 0xc
|           0x06d92450      bl    0x4d18e50
|           0x06d92454      ldrsb x27, [x0]
|           0x06d92458      adrp  x27, 0xb48f000
|           0x06d9245c      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92460      cbz   w4, 0x6d92470
|       |   0x06d92464      ldp   x4, x5, [x0]
|       |   0x06d92468      bl    0x4da3500
|       |   0x06d9246c      stp   x4, x5, [x25]
|       `-> 0x06d92470      adrp  x4, 0x8f93000
|           0x06d92474      add   x4, x4, 0x938
|           0x06d92478      adrp  x5, 0x8f93000
|           0x06d9247c      add   x5, x5, 0x830
|           0x06d92480      stp   x4, x5, [x0]
|           0x06d92484      adrp  x0, 0x8333000
|           0x06d92488      add   x0, x0, 0x110
|           0x06d9248c      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92490      adrp  x2, 0x42d6000
|           0x06d92494      add   x2, x2, 0x9a5
|           0x06d92498      orr   x3, xzr, 0xc
|           0x06d9249c      bl    0x4d18e50
|           0x06d924a0      ldrsb x27, [x0]
|           0x06d924a4      adrp  x27, 0xb48f000
|           0x06d924a8      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d924ac      cbz   w4, 0x6d924bc
|       |   0x06d924b0      ldp   x4, x5, [x0]
|       |   0x06d924b4      bl    0x4da3500
|       |   0x06d924b8      stp   x4, x5, [x25]
|       `-> 0x06d924bc      adrp  x4, 0x8f93000
|           0x06d924c0      add   x4, x4, 0x8d8
|           0x06d924c4      adrp  x5, 0x8f93000
|           0x06d924c8      add   x5, x5, 0x7d0
|           0x06d924cc      stp   x4, x5, [x0]
|           0x06d924d0      adrp  x0, 0x8333000
|           0x06d924d4      add   x0, x0, 0x110
|           0x06d924d8      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d924dc      adrp  x2, 0x4279000
|           0x06d924e0      add   x2, x2, 0xd1b
|           0x06d924e4      orr   x3, xzr, 8
|           0x06d924e8      bl    0x4d18e50
|           0x06d924ec      ldrsb x27, [x0]
|           0x06d924f0      adrp  x27, 0xb48f000
|           0x06d924f4      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d924f8      cbz   w4, 0x6d92508
|       |   0x06d924fc      ldp   x4, x5, [x0]
|       |   0x06d92500      bl    0x4da3500
|       |   0x06d92504      stp   x4, x5, [x25]
|       `-> 0x06d92508      adrp  x4, 0x8f93000
|           0x06d9250c      add   x4, x4, 0x918
|           0x06d92510      adrp  x5, 0x8f93000
|           0x06d92514      add   x5, x5, 0x818
|           0x06d92518      stp   x4, x5, [x0]
|           0x06d9251c      adrp  x27, 0x948c000
|           0x06d92520      ldr   x4, [x27, 0xd10]                     ; [0xd10:4]=0x58585858
|           0x06d92524      str   x4, [arg_28h]
|           0x06d92528      adrp  x0, 0x8333000
|           0x06d9252c      add   x0, x0, 0x110
|           0x06d92530      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92534      adrp  x2, 0x4314000
|           0x06d92538      add   x2, x2, 0x3ab
|           0x06d9253c      orr   x3, xzr, 0xf
|           0x06d92540      bl    0x4d18e50
|           0x06d92544      ldrsb x27, [x0]
|           0x06d92548      adrp  x27, 0xb48f000
|           0x06d9254c      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92550      cbnz  w4, 0x6d9255c
|       |   0x06d92554      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      ,==< 0x06d92558      b     0x6d92570
|      |`-> 0x06d9255c      ldp   x4, x5, [x0]
|      |    0x06d92560      bl    0x4da3510
|      |    0x06d92564      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      |    0x06d92568      stp   x6, x4, [x25]
|      |    0x06d9256c      str   x5, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92558
|      `--> 0x06d92570      adrp  x4, 0x8f93000
|           0x06d92574      add   x4, x4, 0x790
|           0x06d92578      stp   x6, x4, [x0]
|           0x06d9257c      adrp  x0, 0x8333000
|           0x06d92580      add   x0, x0, 0x110
|           0x06d92584      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92588      adrp  x2, 0x4301000
|           0x06d9258c      add   x2, x2, 0xb0e
|           0x06d92590      orr   x3, xzr, 0xe
|           0x06d92594      bl    0x4d18e50
|           0x06d92598      ldrsb x27, [x0]
|           0x06d9259c      adrp  x27, 0xb48f000
|           0x06d925a0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d925a4      cbz   w4, 0x6d925b4
|       |   0x06d925a8      ldp   x4, x5, [x0]
|       |   0x06d925ac      bl    0x4da3500
|       |   0x06d925b0      stp   x4, x5, [x25]
|       `-> 0x06d925b4      adrp  x4, 0x8f93000
|           0x06d925b8      add   x4, x4, 0x880
|           0x06d925bc      adrp  x5, 0x8f93000
|           0x06d925c0      add   x5, x5, 0x768
|           0x06d925c4      stp   x4, x5, [x0]
|           0x06d925c8      adrp  x0, 0x8333000
|           0x06d925cc      add   x0, x0, 0x110
|           0x06d925d0      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d925d4      adrp  x2, 0x4376000
|           0x06d925d8      add   x2, x2, 0xfd8
|           0x06d925dc      mov   x3, 0x15
|           0x06d925e0      bl    0x4d18e50
|           0x06d925e4      ldrsb x27, [x0]
|           0x06d925e8      adrp  x27, 0xb48f000
|           0x06d925ec      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d925f0      cbz   w4, 0x6d92600
|       |   0x06d925f4      ldp   x4, x5, [x0]
|       |   0x06d925f8      bl    0x4da3500
|       |   0x06d925fc      stp   x4, x5, [x25]
|       `-> 0x06d92600      adrp  x4, 0x8f93000
|           0x06d92604      add   x4, x4, 0x878
|           0x06d92608      adrp  x5, 0x8f93000
|           0x06d9260c      add   x5, x5, 0x768
|           0x06d92610      stp   x4, x5, [x0]
|           0x06d92614      adrp  x0, 0x8333000
|           0x06d92618      add   x0, x0, 0x110
|           0x06d9261c      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92620      adrp  x2, 0x4327000
|           0x06d92624      add   x2, x2, 0xe
|           0x06d92628      orr   x3, xzr, 0x10
|           0x06d9262c      bl    0x4d18e50
|           0x06d92630      ldrsb x27, [x0]
|           0x06d92634      adrp  x27, 0xb48f000
|           0x06d92638      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9263c      cbnz  w4, 0x6d92648
|       |   0x06d92640      ldr   x2, [arg_70h]                        ; [0x30:4]=0
|      ,==< 0x06d92644      b     0x6d92664
|      |`-> 0x06d92648      adrp  x27, 0x957c000
|      |    0x06d9264c      ldr   x1, [x27, 0xee0]                     ; [0xee0:4]=0x58585858
|      |    0x06d92650      ldp   x2, x3, [x0]
|      |    0x06d92654      bl    0x4da3520
|      |    0x06d92658      stp   x2, x3, [x25]
|      |    0x06d9265c      ldr   x2, [arg_30h]                        ; [0x30:4]=0
|      |    0x06d92660      stp   x2, x1, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92644
|      `--> 0x06d92664      adrp  x1, 0x8f93000
|           0x06d92668      add   x1, x1, 0x8c8
|           0x06d9266c      adrp  x3, 0x8f93000
|           0x06d92670      add   x3, x3, 0x7c0
|           0x06d92674      stp   x1, x3, [x0]
|           0x06d92678      adrp  x27, 0x957c000
|           0x06d9267c      str   x2, [x27, 0xee0]
|           0x06d92680      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d92684      ldr   lr, [sp], 0x40                       ; segment.PHDR
|                                                                      ; [0x40:4]=6 ; "\U00000006"
\           0x06d92688      ret
==== google3_third_party_jetski_gateway_gateway.defaultUserSettingsPath
/ fcn.06d92368(int64_t arg1, int64_t arg_28h, int64_t arg_30h, int64_t arg_40h, int64_t arg_70h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg_28h @ stack + 0x28
|           ; arg int64_t arg_30h @ stack + 0x30
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_70h @ stack + 0x70
|       ,=< 0x06d92368      cbz   w3, 0x6d92378
|       |   0x06d9236c      ldp   x4, x5, [x0]
|       |   0x06d92370      bl    0x4da3500
|       |   0x06d92374      stp   x4, x5, [x25]
|       `-> 0x06d92378      adrp  x4, 0x8f93000
|           0x06d9237c      add   x4, x4, 0x8f0
|           0x06d92380      adrp  x5, 0x8f93000
|           0x06d92384      add   x5, x5, 0x7f8
|           0x06d92388      stp   x4, x5, [x0]                         ; arg1
|           0x06d9238c      adrp  x0, 0x8333000
|           0x06d92390      add   x0, x0, 0x110
|           0x06d92394      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92398      adrp  x2, 0x42be000
|           0x06d9239c      add   x2, x2, 0xd0f
|           0x06d923a0      mov   x3, 0xb
|           0x06d923a4      bl    0x4d18e50
|           0x06d923a8      ldrsb x27, [x0]
|           0x06d923ac      adrp  x27, 0xb48f000
|           0x06d923b0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d923b4      cbz   w4, 0x6d923c4
|       |   0x06d923b8      ldp   x4, x5, [x0]
|       |   0x06d923bc      bl    0x4da3500
|       |   0x06d923c0      stp   x4, x5, [x25]
|       `-> 0x06d923c4      adrp  x4, 0x8f93000
|           0x06d923c8      add   x4, x4, 0x8e0
|           0x06d923cc      adrp  x5, 0x8f93000
|           0x06d923d0      add   x5, x5, 0x7e0
|           0x06d923d4      stp   x4, x5, [x0]
|           0x06d923d8      adrp  x27, 0x948c000
|           0x06d923dc      ldr   x4, [x27, 0xd20]                     ; [0xd20:4]=0x58585858
|           0x06d923e0      str   x4, [arg_28h]
|           0x06d923e4      adrp  x0, 0x8333000
|           0x06d923e8      add   x0, x0, 0x110
|           0x06d923ec      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d923f0      adrp  x2, 0x4326000
|           0x06d923f4      add   x2, x2, 0xbfe
|           0x06d923f8      orr   x3, xzr, 0x10
|           0x06d923fc      bl    0x4d18e50
|           0x06d92400      ldrsb x27, [x0]
|           0x06d92404      adrp  x27, 0xb48f000
|           0x06d92408      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9240c      cbnz  w4, 0x6d92418
|       |   0x06d92410      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      ,==< 0x06d92414      b     0x6d9242c
|      |`-> 0x06d92418      ldp   x4, x5, [x0]
|      |    0x06d9241c      bl    0x4da3510
|      |    0x06d92420      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      |    0x06d92424      stp   x6, x4, [x25]
|      |    0x06d92428      str   x5, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92414
|      `--> 0x06d9242c      adrp  x4, 0x8f93000
|           0x06d92430      add   x4, x4, 0x7d8
|           0x06d92434      stp   x6, x4, [x0]
|           0x06d92438      adrp  x0, 0x8333000
|           0x06d9243c      add   x0, x0, 0x110
|           0x06d92440      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92444      adrp  x2, 0x42d6000
|           0x06d92448      add   x2, x2, 0x4dd
|           0x06d9244c      orr   x3, xzr, 0xc
|           0x06d92450      bl    0x4d18e50
|           0x06d92454      ldrsb x27, [x0]
|           0x06d92458      adrp  x27, 0xb48f000
|           0x06d9245c      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92460      cbz   w4, 0x6d92470
|       |   0x06d92464      ldp   x4, x5, [x0]
|       |   0x06d92468      bl    0x4da3500
|       |   0x06d9246c      stp   x4, x5, [x25]
|       `-> 0x06d92470      adrp  x4, 0x8f93000
|           0x06d92474      add   x4, x4, 0x938
|           0x06d92478      adrp  x5, 0x8f93000
|           0x06d9247c      add   x5, x5, 0x830
|           0x06d92480      stp   x4, x5, [x0]
|           0x06d92484      adrp  x0, 0x8333000
|           0x06d92488      add   x0, x0, 0x110
|           0x06d9248c      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92490      adrp  x2, 0x42d6000
|           0x06d92494      add   x2, x2, 0x9a5
|           0x06d92498      orr   x3, xzr, 0xc
|           0x06d9249c      bl    0x4d18e50
|           0x06d924a0      ldrsb x27, [x0]
|           0x06d924a4      adrp  x27, 0xb48f000
|           0x06d924a8      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d924ac      cbz   w4, 0x6d924bc
|       |   0x06d924b0      ldp   x4, x5, [x0]
|       |   0x06d924b4      bl    0x4da3500
|       |   0x06d924b8      stp   x4, x5, [x25]
|       `-> 0x06d924bc      adrp  x4, 0x8f93000
|           0x06d924c0      add   x4, x4, 0x8d8
|           0x06d924c4      adrp  x5, 0x8f93000
|           0x06d924c8      add   x5, x5, 0x7d0
|           0x06d924cc      stp   x4, x5, [x0]
|           0x06d924d0      adrp  x0, 0x8333000
|           0x06d924d4      add   x0, x0, 0x110
|           0x06d924d8      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d924dc      adrp  x2, 0x4279000
|           0x06d924e0      add   x2, x2, 0xd1b
|           0x06d924e4      orr   x3, xzr, 8
|           0x06d924e8      bl    0x4d18e50
|           0x06d924ec      ldrsb x27, [x0]
|           0x06d924f0      adrp  x27, 0xb48f000
|           0x06d924f4      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d924f8      cbz   w4, 0x6d92508
|       |   0x06d924fc      ldp   x4, x5, [x0]
|       |   0x06d92500      bl    0x4da3500
|       |   0x06d92504      stp   x4, x5, [x25]
|       `-> 0x06d92508      adrp  x4, 0x8f93000
|           0x06d9250c      add   x4, x4, 0x918
|           0x06d92510      adrp  x5, 0x8f93000
|           0x06d92514      add   x5, x5, 0x818
|           0x06d92518      stp   x4, x5, [x0]
|           0x06d9251c      adrp  x27, 0x948c000
|           0x06d92520      ldr   x4, [x27, 0xd10]                     ; [0xd10:4]=0x58585858
|           0x06d92524      str   x4, [arg_28h]
|           0x06d92528      adrp  x0, 0x8333000
|           0x06d9252c      add   x0, x0, 0x110
|           0x06d92530      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92534      adrp  x2, 0x4314000
|           0x06d92538      add   x2, x2, 0x3ab
|           0x06d9253c      orr   x3, xzr, 0xf
|           0x06d92540      bl    0x4d18e50
|           0x06d92544      ldrsb x27, [x0]
|           0x06d92548      adrp  x27, 0xb48f000
|           0x06d9254c      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92550      cbnz  w4, 0x6d9255c
|       |   0x06d92554      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      ,==< 0x06d92558      b     0x6d92570
|      |`-> 0x06d9255c      ldp   x4, x5, [x0]
|      |    0x06d92560      bl    0x4da3510
|      |    0x06d92564      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      |    0x06d92568      stp   x6, x4, [x25]
|      |    0x06d9256c      str   x5, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92558
|      `--> 0x06d92570      adrp  x4, 0x8f93000
|           0x06d92574      add   x4, x4, 0x790
|           0x06d92578      stp   x6, x4, [x0]
|           0x06d9257c      adrp  x0, 0x8333000
|           0x06d92580      add   x0, x0, 0x110
|           0x06d92584      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92588      adrp  x2, 0x4301000
|           0x06d9258c      add   x2, x2, 0xb0e
|           0x06d92590      orr   x3, xzr, 0xe
|           0x06d92594      bl    0x4d18e50
|           0x06d92598      ldrsb x27, [x0]
|           0x06d9259c      adrp  x27, 0xb48f000
|           0x06d925a0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d925a4      cbz   w4, 0x6d925b4
|       |   0x06d925a8      ldp   x4, x5, [x0]
|       |   0x06d925ac      bl    0x4da3500
|       |   0x06d925b0      stp   x4, x5, [x25]
|       `-> 0x06d925b4      adrp  x4, 0x8f93000
|           0x06d925b8      add   x4, x4, 0x880
|           0x06d925bc      adrp  x5, 0x8f93000
|           0x06d925c0      add   x5, x5, 0x768
|           0x06d925c4      stp   x4, x5, [x0]
|           0x06d925c8      adrp  x0, 0x8333000
|           0x06d925cc      add   x0, x0, 0x110
|           0x06d925d0      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d925d4      adrp  x2, 0x4376000
|           0x06d925d8      add   x2, x2, 0xfd8
|           0x06d925dc      mov   x3, 0x15
|           0x06d925e0      bl    0x4d18e50
|           0x06d925e4      ldrsb x27, [x0]
|           0x06d925e8      adrp  x27, 0xb48f000
|           0x06d925ec      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d925f0      cbz   w4, 0x6d92600
|       |   0x06d925f4      ldp   x4, x5, [x0]
|       |   0x06d925f8      bl    0x4da3500
|       |   0x06d925fc      stp   x4, x5, [x25]
|       `-> 0x06d92600      adrp  x4, 0x8f93000
|           0x06d92604      add   x4, x4, 0x878
|           0x06d92608      adrp  x5, 0x8f93000
|           0x06d9260c      add   x5, x5, 0x768
|           0x06d92610      stp   x4, x5, [x0]
|           0x06d92614      adrp  x0, 0x8333000
|           0x06d92618      add   x0, x0, 0x110
|           0x06d9261c      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92620      adrp  x2, 0x4327000
|           0x06d92624      add   x2, x2, 0xe
|           0x06d92628      orr   x3, xzr, 0x10
|           0x06d9262c      bl    0x4d18e50
|           0x06d92630      ldrsb x27, [x0]
|           0x06d92634      adrp  x27, 0xb48f000
|           0x06d92638      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9263c      cbnz  w4, 0x6d92648
|       |   0x06d92640      ldr   x2, [arg_70h]                        ; [0x30:4]=0
|      ,==< 0x06d92644      b     0x6d92664
|      |`-> 0x06d92648      adrp  x27, 0x957c000
|      |    0x06d9264c      ldr   x1, [x27, 0xee0]                     ; [0xee0:4]=0x58585858
|      |    0x06d92650      ldp   x2, x3, [x0]
|      |    0x06d92654      bl    0x4da3520
|      |    0x06d92658      stp   x2, x3, [x25]
|      |    0x06d9265c      ldr   x2, [arg_30h]                        ; [0x30:4]=0
|      |    0x06d92660      stp   x2, x1, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92644
|      `--> 0x06d92664      adrp  x1, 0x8f93000
|           0x06d92668      add   x1, x1, 0x8c8
|           0x06d9266c      adrp  x3, 0x8f93000
|           0x06d92670      add   x3, x3, 0x7c0
|           0x06d92674      stp   x1, x3, [x0]
|           0x06d92678      adrp  x27, 0x957c000
|           0x06d9267c      str   x2, [x27, 0xee0]
|           0x06d92680      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d92684      ldr   lr, [sp], 0x40                       ; segment.PHDR
|                                                                      ; [0x40:4]=6 ; "\U00000006"
\           0x06d92688      ret
==== AdminSettings_.Validate
/ fcn.06d92368(int64_t arg1, int64_t arg_28h, int64_t arg_30h, int64_t arg_40h, int64_t arg_70h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg_28h @ stack + 0x28
|           ; arg int64_t arg_30h @ stack + 0x30
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_70h @ stack + 0x70
|       ,=< 0x06d92368      cbz   w3, 0x6d92378
|       |   0x06d9236c      ldp   x4, x5, [x0]
|       |   0x06d92370      bl    0x4da3500
|       |   0x06d92374      stp   x4, x5, [x25]
|       `-> 0x06d92378      adrp  x4, 0x8f93000
|           0x06d9237c      add   x4, x4, 0x8f0
|           0x06d92380      adrp  x5, 0x8f93000
|           0x06d92384      add   x5, x5, 0x7f8
|           0x06d92388      stp   x4, x5, [x0]                         ; arg1
|           0x06d9238c      adrp  x0, 0x8333000
|           0x06d92390      add   x0, x0, 0x110
|           0x06d92394      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92398      adrp  x2, 0x42be000
|           0x06d9239c      add   x2, x2, 0xd0f
|           0x06d923a0      mov   x3, 0xb
|           0x06d923a4      bl    0x4d18e50
|           0x06d923a8      ldrsb x27, [x0]
|           0x06d923ac      adrp  x27, 0xb48f000
|           0x06d923b0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d923b4      cbz   w4, 0x6d923c4
|       |   0x06d923b8      ldp   x4, x5, [x0]
|       |   0x06d923bc      bl    0x4da3500
|       |   0x06d923c0      stp   x4, x5, [x25]
|       `-> 0x06d923c4      adrp  x4, 0x8f93000
|           0x06d923c8      add   x4, x4, 0x8e0
|           0x06d923cc      adrp  x5, 0x8f93000
|           0x06d923d0      add   x5, x5, 0x7e0
|           0x06d923d4      stp   x4, x5, [x0]
|           0x06d923d8      adrp  x27, 0x948c000
|           0x06d923dc      ldr   x4, [x27, 0xd20]                     ; [0xd20:4]=0x58585858
|           0x06d923e0      str   x4, [arg_28h]
|           0x06d923e4      adrp  x0, 0x8333000
|           0x06d923e8      add   x0, x0, 0x110
|           0x06d923ec      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d923f0      adrp  x2, 0x4326000
|           0x06d923f4      add   x2, x2, 0xbfe
|           0x06d923f8      orr   x3, xzr, 0x10
|           0x06d923fc      bl    0x4d18e50
|           0x06d92400      ldrsb x27, [x0]
|           0x06d92404      adrp  x27, 0xb48f000
|           0x06d92408      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9240c      cbnz  w4, 0x6d92418
|       |   0x06d92410      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      ,==< 0x06d92414      b     0x6d9242c
|      |`-> 0x06d92418      ldp   x4, x5, [x0]
|      |    0x06d9241c      bl    0x4da3510
|      |    0x06d92420      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      |    0x06d92424      stp   x6, x4, [x25]
|      |    0x06d92428      str   x5, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92414
|      `--> 0x06d9242c      adrp  x4, 0x8f93000
|           0x06d92430      add   x4, x4, 0x7d8
|           0x06d92434      stp   x6, x4, [x0]
|           0x06d92438      adrp  x0, 0x8333000
|           0x06d9243c      add   x0, x0, 0x110
|           0x06d92440      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92444      adrp  x2, 0x42d6000
|           0x06d92448      add   x2, x2, 0x4dd
|           0x06d9244c      orr   x3, xzr, 0xc
|           0x06d92450      bl    0x4d18e50
|           0x06d92454      ldrsb x27, [x0]
|           0x06d92458      adrp  x27, 0xb48f000
|           0x06d9245c      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92460      cbz   w4, 0x6d92470
|       |   0x06d92464      ldp   x4, x5, [x0]
|       |   0x06d92468      bl    0x4da3500
|       |   0x06d9246c      stp   x4, x5, [x25]
|       `-> 0x06d92470      adrp  x4, 0x8f93000
|           0x06d92474      add   x4, x4, 0x938
|           0x06d92478      adrp  x5, 0x8f93000
|           0x06d9247c      add   x5, x5, 0x830
|           0x06d92480      stp   x4, x5, [x0]
|           0x06d92484      adrp  x0, 0x8333000
|           0x06d92488      add   x0, x0, 0x110
|           0x06d9248c      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92490      adrp  x2, 0x42d6000
|           0x06d92494      add   x2, x2, 0x9a5
|           0x06d92498      orr   x3, xzr, 0xc
|           0x06d9249c      bl    0x4d18e50
|           0x06d924a0      ldrsb x27, [x0]
|           0x06d924a4      adrp  x27, 0xb48f000
|           0x06d924a8      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d924ac      cbz   w4, 0x6d924bc
|       |   0x06d924b0      ldp   x4, x5, [x0]
|       |   0x06d924b4      bl    0x4da3500
|       |   0x06d924b8      stp   x4, x5, [x25]
|       `-> 0x06d924bc      adrp  x4, 0x8f93000
|           0x06d924c0      add   x4, x4, 0x8d8
|           0x06d924c4      adrp  x5, 0x8f93000
|           0x06d924c8      add   x5, x5, 0x7d0
|           0x06d924cc      stp   x4, x5, [x0]
|           0x06d924d0      adrp  x0, 0x8333000
|           0x06d924d4      add   x0, x0, 0x110
|           0x06d924d8      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d924dc      adrp  x2, 0x4279000
|           0x06d924e0      add   x2, x2, 0xd1b
|           0x06d924e4      orr   x3, xzr, 8
|           0x06d924e8      bl    0x4d18e50
|           0x06d924ec      ldrsb x27, [x0]
|           0x06d924f0      adrp  x27, 0xb48f000
|           0x06d924f4      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d924f8      cbz   w4, 0x6d92508
|       |   0x06d924fc      ldp   x4, x5, [x0]
|       |   0x06d92500      bl    0x4da3500
|       |   0x06d92504      stp   x4, x5, [x25]
|       `-> 0x06d92508      adrp  x4, 0x8f93000
|           0x06d9250c      add   x4, x4, 0x918
|           0x06d92510      adrp  x5, 0x8f93000
|           0x06d92514      add   x5, x5, 0x818
|           0x06d92518      stp   x4, x5, [x0]
|           0x06d9251c      adrp  x27, 0x948c000
|           0x06d92520      ldr   x4, [x27, 0xd10]                     ; [0xd10:4]=0x58585858
|           0x06d92524      str   x4, [arg_28h]
|           0x06d92528      adrp  x0, 0x8333000
|           0x06d9252c      add   x0, x0, 0x110
|           0x06d92530      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92534      adrp  x2, 0x4314000
|           0x06d92538      add   x2, x2, 0x3ab
|           0x06d9253c      orr   x3, xzr, 0xf
|           0x06d92540      bl    0x4d18e50
|           0x06d92544      ldrsb x27, [x0]
|           0x06d92548      adrp  x27, 0xb48f000
|           0x06d9254c      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92550      cbnz  w4, 0x6d9255c
|       |   0x06d92554      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      ,==< 0x06d92558      b     0x6d92570
|      |`-> 0x06d9255c      ldp   x4, x5, [x0]
|      |    0x06d92560      bl    0x4da3510
|      |    0x06d92564      ldr   x6, [arg_28h]                        ; [0x28:4]=0xace39d8
|      |    0x06d92568      stp   x6, x4, [x25]
|      |    0x06d9256c      str   x5, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92558
|      `--> 0x06d92570      adrp  x4, 0x8f93000
|           0x06d92574      add   x4, x4, 0x790
|           0x06d92578      stp   x6, x4, [x0]
|           0x06d9257c      adrp  x0, 0x8333000
|           0x06d92580      add   x0, x0, 0x110
|           0x06d92584      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92588      adrp  x2, 0x4301000
|           0x06d9258c      add   x2, x2, 0xb0e
|           0x06d92590      orr   x3, xzr, 0xe
|           0x06d92594      bl    0x4d18e50
|           0x06d92598      ldrsb x27, [x0]
|           0x06d9259c      adrp  x27, 0xb48f000
|           0x06d925a0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d925a4      cbz   w4, 0x6d925b4
|       |   0x06d925a8      ldp   x4, x5, [x0]
|       |   0x06d925ac      bl    0x4da3500
|       |   0x06d925b0      stp   x4, x5, [x25]
|       `-> 0x06d925b4      adrp  x4, 0x8f93000
|           0x06d925b8      add   x4, x4, 0x880
|           0x06d925bc      adrp  x5, 0x8f93000
|           0x06d925c0      add   x5, x5, 0x768
|           0x06d925c4      stp   x4, x5, [x0]
|           0x06d925c8      adrp  x0, 0x8333000
|           0x06d925cc      add   x0, x0, 0x110
|           0x06d925d0      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d925d4      adrp  x2, 0x4376000
|           0x06d925d8      add   x2, x2, 0xfd8
|           0x06d925dc      mov   x3, 0x15
|           0x06d925e0      bl    0x4d18e50
|           0x06d925e4      ldrsb x27, [x0]
|           0x06d925e8      adrp  x27, 0xb48f000
|           0x06d925ec      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d925f0      cbz   w4, 0x6d92600
|       |   0x06d925f4      ldp   x4, x5, [x0]
|       |   0x06d925f8      bl    0x4da3500
|       |   0x06d925fc      stp   x4, x5, [x25]
|       `-> 0x06d92600      adrp  x4, 0x8f93000
|           0x06d92604      add   x4, x4, 0x878
|           0x06d92608      adrp  x5, 0x8f93000
|           0x06d9260c      add   x5, x5, 0x768
|           0x06d92610      stp   x4, x5, [x0]
|           0x06d92614      adrp  x0, 0x8333000
|           0x06d92618      add   x0, x0, 0x110
|           0x06d9261c      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92620      adrp  x2, 0x4327000
|           0x06d92624      add   x2, x2, 0xe
|           0x06d92628      orr   x3, xzr, 0x10
|           0x06d9262c      bl    0x4d18e50
|           0x06d92630      ldrsb x27, [x0]
|           0x06d92634      adrp  x27, 0xb48f000
|           0x06d92638      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9263c      cbnz  w4, 0x6d92648
|       |   0x06d92640      ldr   x2, [arg_70h]                        ; [0x30:4]=0
|      ,==< 0x06d92644      b     0x6d92664
|      |`-> 0x06d92648      adrp  x27, 0x957c000
|      |    0x06d9264c      ldr   x1, [x27, 0xee0]                     ; [0xee0:4]=0x58585858
|      |    0x06d92650      ldp   x2, x3, [x0]
|      |    0x06d92654      bl    0x4da3520
|      |    0x06d92658      stp   x2, x3, [x25]
|      |    0x06d9265c      ldr   x2, [arg_30h]                        ; [0x30:4]=0
|      |    0x06d92660      stp   x2, x1, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92368 @ 0x6d92644
|      `--> 0x06d92664      adrp  x1, 0x8f93000
|           0x06d92668      add   x1, x1, 0x8c8
|           0x06d9266c      adrp  x3, 0x8f93000
|           0x06d92670      add   x3, x3, 0x7c0
|           0x06d92674      stp   x1, x3, [x0]
|           0x06d92678      adrp  x27, 0x957c000
|           0x06d9267c      str   x2, [x27, 0xee0]
|           0x06d92680      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d92684      ldr   lr, [sp], 0x40                       ; segment.PHDR
|                                                                      ; [0x40:4]=6 ; "\U00000006"
\           0x06d92688      ret
==== google3_third_party_jetski_gateway_gateway.ParseAdminSettings
/ fcn.06d92a88(int64_t arg_30h, int64_t arg_40h, int64_t arg_70h);
|           ; arg int64_t arg_30h @ stack + 0x30
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_70h @ stack + 0x70
|           0x06d92a88      bl    0x4d18e50
|           0x06d92a8c      adrp  x27, 0xb48f000
|           0x06d92a90      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92a94      cbz   w4, 0x6d92aa4
|       |   0x06d92a98      ldr   x4, [x0]
|       |   0x06d92a9c      bl    0x4da34f0
|       |   0x06d92aa0      str   x4, [x25]
|       `-> 0x06d92aa4      adrp  x4, 0x8f93000
|           0x06d92aa8      add   x4, x4, 0x6b0
|           0x06d92aac      str   x4, [x0]
|           0x06d92ab0      adrp  x0, 0x8332000
|           0x06d92ab4      add   x0, x0, 0x3c8
|           0x06d92ab8      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92abc      adrp  x2, 0x4367000
|           0x06d92ac0      add   x2, x2, 0xcd3
|           0x06d92ac4      mov   x3, 0x14
|           0x06d92ac8      bl    0x4d18e50
|           0x06d92acc      adrp  x27, 0xb48f000
|           0x06d92ad0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92ad4      cbz   w4, 0x6d92ae4
|       |   0x06d92ad8      ldr   x4, [x0]
|       |   0x06d92adc      bl    0x4da34f0
|       |   0x06d92ae0      str   x4, [x25]
|       `-> 0x06d92ae4      adrp  x4, 0x8f93000
|           0x06d92ae8      add   x4, x4, 0x6b0
|           0x06d92aec      str   x4, [x0]
|           0x06d92af0      adrp  x0, 0x8332000
|           0x06d92af4      add   x0, x0, 0x3c8
|           0x06d92af8      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92afc      adrp  x2, 0x4358000
|           0x06d92b00      add   x2, x2, 0xcfd
|           0x06d92b04      mov   x3, 0x13
|           0x06d92b08      bl    0x4d18e50
|           0x06d92b0c      adrp  x27, 0xb48f000
|           0x06d92b10      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92b14      cbz   w4, 0x6d92b24
|       |   0x06d92b18      ldr   x4, [x0]
|       |   0x06d92b1c      bl    0x4da34f0
|       |   0x06d92b20      str   x4, [x25]
|       `-> 0x06d92b24      adrp  x4, 0x8f93000
|           0x06d92b28      add   x4, x4, 0x6b0
|           0x06d92b2c      str   x4, [x0]
|           0x06d92b30      adrp  x0, 0x8332000
|           0x06d92b34      add   x0, x0, 0x3c8
|           0x06d92b38      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92b3c      adrp  x2, 0x4388000
|           0x06d92b40      add   x2, x2, 0xfa3
|           0x06d92b44      mov   x3, 0x16
|           0x06d92b48      bl    0x4d18e50
|           0x06d92b4c      adrp  x27, 0xb48f000
|           0x06d92b50      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92b54      cbz   w4, 0x6d92b64
|       |   0x06d92b58      ldr   x4, [x0]
|       |   0x06d92b5c      bl    0x4da34f0
|       |   0x06d92b60      str   x4, [x25]
|       `-> 0x06d92b64      adrp  x4, 0x8f93000
|           0x06d92b68      add   x4, x4, 0x6d0
|           0x06d92b6c      str   x4, [x0]
|           0x06d92b70      adrp  x0, 0x8332000
|           0x06d92b74      add   x0, x0, 0x3c8
|           0x06d92b78      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92b7c      adrp  x2, 0x4348000
|           0x06d92b80      add   x2, x2, 0x682
|           0x06d92b84      mov   x3, 0x12
|           0x06d92b88      bl    0x4d18e50
|           0x06d92b8c      adrp  x27, 0xb48f000
|           0x06d92b90      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92b94      cbz   w4, 0x6d92ba4
|       |   0x06d92b98      ldr   x4, [x0]
|       |   0x06d92b9c      bl    0x4da34f0
|       |   0x06d92ba0      str   x4, [x25]
|       `-> 0x06d92ba4      adrp  x4, 0x8f93000
|           0x06d92ba8      add   x4, x4, 0x6d8
|           0x06d92bac      str   x4, [x0]
|           0x06d92bb0      adrp  x0, 0x8332000
|           0x06d92bb4      add   x0, x0, 0x3c8
|           0x06d92bb8      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92bbc      adrp  x2, 0x43ee000
|           0x06d92bc0      add   x2, x2, 0x9c0
|           0x06d92bc4      orr   x3, xzr, 0x1f
|           0x06d92bc8      bl    0x4d18e50
|           0x06d92bcc      adrp  x27, 0xb48f000
|           0x06d92bd0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92bd4      cbz   w4, 0x6d92be4
|       |   0x06d92bd8      ldr   x4, [x0]
|       |   0x06d92bdc      bl    0x4da34f0
|       |   0x06d92be0      str   x4, [x25]
|       `-> 0x06d92be4      adrp  x4, 0x8f93000
|           0x06d92be8      add   x4, x4, 0x6c8
|           0x06d92bec      str   x4, [x0]
|           0x06d92bf0      adrp  x0, 0x8332000
|           0x06d92bf4      add   x0, x0, 0x3c8
|           0x06d92bf8      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92bfc      adrp  x2, 0x4358000
|           0x06d92c00      add   x2, x2, 0x487
|           0x06d92c04      mov   x3, 0x13
|           0x06d92c08      bl    0x4d18e50
|           0x06d92c0c      adrp  x27, 0xb48f000
|           0x06d92c10      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92c14      cbnz  w4, 0x6d92c20
|       |   0x06d92c18      ldr   x3, [arg_70h]                        ; [0x30:4]=0
|      ,==< 0x06d92c1c      b     0x6d92c3c
|      |`-> 0x06d92c20      adrp  x27, 0x957c000
|      |    0x06d92c24      ldr   x1, [x27, 0xf00]                     ; [0xf00:4]=0x58585858
|      |    0x06d92c28      ldr   x2, [x0]
|      |    0x06d92c2c      bl    0x4da3510
|      |    0x06d92c30      ldr   x3, [arg_30h]                        ; [0x30:4]=0
|      |    0x06d92c34      stp   x2, x3, [x25]
|      |    0x06d92c38      str   x1, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92a88 @ 0x6d92c1c
|      `--> 0x06d92c3c      adrp  x1, 0x8f93000
|           0x06d92c40      add   x1, x1, 0x6e0
|           0x06d92c44      str   x1, [x0]
|           0x06d92c48      adrp  x27, 0x957c000
|           0x06d92c4c      str   x3, [x27, 0xf00]
|           0x06d92c50      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d92c54      ldr   lr, [sp], 0x40                       ; segment.PHDR
|                                                                      ; [0x40:4]=6 ; "\U00000006"
\           0x06d92c58      ret
==== google3_third_party_jetski_gateway_gateway.ParseEnvOverrides
/ fcn.06d92a88(int64_t arg_30h, int64_t arg_40h, int64_t arg_70h);
|           ; arg int64_t arg_30h @ stack + 0x30
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_70h @ stack + 0x70
|           0x06d92a88      bl    0x4d18e50
|           0x06d92a8c      adrp  x27, 0xb48f000
|           0x06d92a90      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92a94      cbz   w4, 0x6d92aa4
|       |   0x06d92a98      ldr   x4, [x0]
|       |   0x06d92a9c      bl    0x4da34f0
|       |   0x06d92aa0      str   x4, [x25]
|       `-> 0x06d92aa4      adrp  x4, 0x8f93000
|           0x06d92aa8      add   x4, x4, 0x6b0
|           0x06d92aac      str   x4, [x0]
|           0x06d92ab0      adrp  x0, 0x8332000
|           0x06d92ab4      add   x0, x0, 0x3c8
|           0x06d92ab8      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92abc      adrp  x2, 0x4367000
|           0x06d92ac0      add   x2, x2, 0xcd3
|           0x06d92ac4      mov   x3, 0x14
|           0x06d92ac8      bl    0x4d18e50
|           0x06d92acc      adrp  x27, 0xb48f000
|           0x06d92ad0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92ad4      cbz   w4, 0x6d92ae4
|       |   0x06d92ad8      ldr   x4, [x0]
|       |   0x06d92adc      bl    0x4da34f0
|       |   0x06d92ae0      str   x4, [x25]
|       `-> 0x06d92ae4      adrp  x4, 0x8f93000
|           0x06d92ae8      add   x4, x4, 0x6b0
|           0x06d92aec      str   x4, [x0]
|           0x06d92af0      adrp  x0, 0x8332000
|           0x06d92af4      add   x0, x0, 0x3c8
|           0x06d92af8      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92afc      adrp  x2, 0x4358000
|           0x06d92b00      add   x2, x2, 0xcfd
|           0x06d92b04      mov   x3, 0x13
|           0x06d92b08      bl    0x4d18e50
|           0x06d92b0c      adrp  x27, 0xb48f000
|           0x06d92b10      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92b14      cbz   w4, 0x6d92b24
|       |   0x06d92b18      ldr   x4, [x0]
|       |   0x06d92b1c      bl    0x4da34f0
|       |   0x06d92b20      str   x4, [x25]
|       `-> 0x06d92b24      adrp  x4, 0x8f93000
|           0x06d92b28      add   x4, x4, 0x6b0
|           0x06d92b2c      str   x4, [x0]
|           0x06d92b30      adrp  x0, 0x8332000
|           0x06d92b34      add   x0, x0, 0x3c8
|           0x06d92b38      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92b3c      adrp  x2, 0x4388000
|           0x06d92b40      add   x2, x2, 0xfa3
|           0x06d92b44      mov   x3, 0x16
|           0x06d92b48      bl    0x4d18e50
|           0x06d92b4c      adrp  x27, 0xb48f000
|           0x06d92b50      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92b54      cbz   w4, 0x6d92b64
|       |   0x06d92b58      ldr   x4, [x0]
|       |   0x06d92b5c      bl    0x4da34f0
|       |   0x06d92b60      str   x4, [x25]
|       `-> 0x06d92b64      adrp  x4, 0x8f93000
|           0x06d92b68      add   x4, x4, 0x6d0
|           0x06d92b6c      str   x4, [x0]
|           0x06d92b70      adrp  x0, 0x8332000
|           0x06d92b74      add   x0, x0, 0x3c8
|           0x06d92b78      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92b7c      adrp  x2, 0x4348000
|           0x06d92b80      add   x2, x2, 0x682
|           0x06d92b84      mov   x3, 0x12
|           0x06d92b88      bl    0x4d18e50
|           0x06d92b8c      adrp  x27, 0xb48f000
|           0x06d92b90      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92b94      cbz   w4, 0x6d92ba4
|       |   0x06d92b98      ldr   x4, [x0]
|       |   0x06d92b9c      bl    0x4da34f0
|       |   0x06d92ba0      str   x4, [x25]
|       `-> 0x06d92ba4      adrp  x4, 0x8f93000
|           0x06d92ba8      add   x4, x4, 0x6d8
|           0x06d92bac      str   x4, [x0]
|           0x06d92bb0      adrp  x0, 0x8332000
|           0x06d92bb4      add   x0, x0, 0x3c8
|           0x06d92bb8      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92bbc      adrp  x2, 0x43ee000
|           0x06d92bc0      add   x2, x2, 0x9c0
|           0x06d92bc4      orr   x3, xzr, 0x1f
|           0x06d92bc8      bl    0x4d18e50
|           0x06d92bcc      adrp  x27, 0xb48f000
|           0x06d92bd0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92bd4      cbz   w4, 0x6d92be4
|       |   0x06d92bd8      ldr   x4, [x0]
|       |   0x06d92bdc      bl    0x4da34f0
|       |   0x06d92be0      str   x4, [x25]
|       `-> 0x06d92be4      adrp  x4, 0x8f93000
|           0x06d92be8      add   x4, x4, 0x6c8
|           0x06d92bec      str   x4, [x0]
|           0x06d92bf0      adrp  x0, 0x8332000
|           0x06d92bf4      add   x0, x0, 0x3c8
|           0x06d92bf8      ldr   x1, [arg_30h]                        ; [0x30:4]=0
|           0x06d92bfc      adrp  x2, 0x4358000
|           0x06d92c00      add   x2, x2, 0x487
|           0x06d92c04      mov   x3, 0x13
|           0x06d92c08      bl    0x4d18e50
|           0x06d92c0c      adrp  x27, 0xb48f000
|           0x06d92c10      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d92c14      cbnz  w4, 0x6d92c20
|       |   0x06d92c18      ldr   x3, [arg_70h]                        ; [0x30:4]=0
|      ,==< 0x06d92c1c      b     0x6d92c3c
|      |`-> 0x06d92c20      adrp  x27, 0x957c000
|      |    0x06d92c24      ldr   x1, [x27, 0xf00]                     ; [0xf00:4]=0x58585858
|      |    0x06d92c28      ldr   x2, [x0]
|      |    0x06d92c2c      bl    0x4da3510
|      |    0x06d92c30      ldr   x3, [arg_30h]                        ; [0x30:4]=0
|      |    0x06d92c34      stp   x2, x3, [x25]
|      |    0x06d92c38      str   x1, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d92a88 @ 0x6d92c1c
|      `--> 0x06d92c3c      adrp  x1, 0x8f93000
|           0x06d92c40      add   x1, x1, 0x6e0
|           0x06d92c44      str   x1, [x0]
|           0x06d92c48      adrp  x27, 0x957c000
|           0x06d92c4c      str   x3, [x27, 0xf00]
|           0x06d92c50      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d92c54      ldr   lr, [sp], 0x40                       ; segment.PHDR
|                                                                      ; [0x40:4]=6 ; "\U00000006"
\           0x06d92c58      ret
==== google3_third_party_jetski_gateway_gateway.MergeSettings
/ fcn.06d92ee8(int64_t arg6, int64_t arg_28h, int64_t arg_90h, int64_t arg_98h);
|       :   ; arg int64_t arg6 @ x5
|       :   ; arg int64_t arg_28h @ stack + 0x28
|       :   ; arg int64_t arg_90h @ stack + 0x90
|       :   ; arg int64_t arg_98h @ stack + 0x98
|       :   0x06d92ee8      ldr   x3, [x5]                             ; arg6
|       :   0x06d92eec      ldr   x4, [arg_98h]                        ; [0x98:4]=0x779b960
|       :   0x06d92ef0      ldr   x0, [x4, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|       :   0x06d92ef4      bl    fcn.063bf840
|       :   0x06d92ef8      add   x0, arg_28h
|       :   0x06d92efc      bl    fcn.04d35870
|       :   0x06d92f00      ldp   x4, x5, [arg_28h]
|       `=< 0x06d92f04      cbnz  x4, 0x6d92ee4
|           0x06d92f08      adrp  x27, 0x957c000
|           0x06d92f0c      ldr   x1, [x27, 0xf00]                     ; [0xf00:4]=0x58585858
|           0x06d92f10      add   x2, arg_28h
|           0x06d92f14      stp   xzr, xzr, [x2]
|           0x06d92f18      stp   xzr, xzr, [x2, 0x10]
|           0x06d92f1c      stp   xzr, xzr, [x2, 0x20]
|           0x06d92f20      stp   xzr, xzr, [x2, 0x30]
|           0x06d92f24      stp   xzr, xzr, [x2, 0x40]
|           0x06d92f28      stp   xzr, xzr, [x2, 0x50]
|           0x06d92f2c      adrp  x0, 0x8332000
|           0x06d92f30      add   x0, x0, 0x3c8
|           0x06d92f34      bl    fcn.04d35800
|       ,=< 0x06d92f38      b     0x6d92f58
|      .--> 0x06d92f3c      ldp   x1, x2, [x4]
|      :|   0x06d92f40      ldr   x3, [x5]
|      :|   0x06d92f44      ldr   x4, [arg_98h]                        ; [0x98:4]=0x779b960
|      :|   0x06d92f48      ldr   x0, [x4, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      :|   0x06d92f4c      bl    fcn.063bf970
|      :|   0x06d92f50      add   x0, arg_28h
|      :|   0x06d92f54      bl    fcn.04d35870
|      :|   ; CODE XREF from fcn.06d92ee8 @ 0x6d92f38
|      :`-> 0x06d92f58      ldp   x4, x5, [arg_28h]
|      `==< 0x06d92f5c      cbnz  x4, 0x6d92f3c
|           0x06d92f60      adrp  x27, 0x957c000
|           0x06d92f64      ldr   x1, [x27, 0xf08]                     ; [0xf08:4]=0x58585858
|           0x06d92f68      add   x2, arg_28h
|           0x06d92f6c      stp   xzr, xzr, [x2]
|           0x06d92f70      stp   xzr, xzr, [x2, 0x10]
|           0x06d92f74      stp   xzr, xzr, [x2, 0x20]
|           0x06d92f78      stp   xzr, xzr, [x2, 0x30]
|           0x06d92f7c      stp   xzr, xzr, [x2, 0x40]
|           0x06d92f80      stp   xzr, xzr, [x2, 0x50]
|           0x06d92f84      adrp  x0, 0x8332000
|           0x06d92f88      add   x0, x0, 0x5e8
|           0x06d92f8c      bl    fcn.04d35800
|       ,=< 0x06d92f90      b     0x6d92fb0
|      .--> 0x06d92f94      ldp   x1, x2, [x4]
|      :|   0x06d92f98      ldr   x3, [x5]
|      :|   0x06d92f9c      ldr   x4, [arg_98h]                        ; [0x98:4]=0x779b960
|      :|   0x06d92fa0      ldr   x0, [x4, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      :|   0x06d92fa4      bl    fcn.063bfaa0
|      :|   0x06d92fa8      add   x0, arg_28h
|      :|   0x06d92fac      bl    fcn.04d35870
|      :|   ; CODE XREF from fcn.06d92ee8 @ 0x6d92f90
|      :`-> 0x06d92fb0      ldp   x4, x5, [arg_28h]
|      `==< 0x06d92fb4      cbnz  x4, 0x6d92f94
|           0x06d92fb8      adrp  x27, 0x957c000
|           0x06d92fbc      ldr   x1, [x27, 0xf10]                     ; [0xf10:4]=0x58585858
|           0x06d92fc0      add   x2, arg_28h
|           0x06d92fc4      stp   xzr, xzr, [x2]
|           0x06d92fc8      stp   xzr, xzr, [x2, 0x10]
|           0x06d92fcc      stp   xzr, xzr, [x2, 0x20]
|           0x06d92fd0      stp   xzr, xzr, [x2, 0x30]
|           0x06d92fd4      stp   xzr, xzr, [x2, 0x40]
|           0x06d92fd8      stp   xzr, xzr, [x2, 0x50]
|           0x06d92fdc      adrp  x0, 0x8332000
|           0x06d92fe0      add   x0, x0, 0x560
|           0x06d92fe4      bl    fcn.04d35800
|       ,=< 0x06d92fe8      b     0x6d93008
|      .--> 0x06d92fec      ldp   x1, x2, [x4]
|      :|   0x06d92ff0      ldr   x3, [x5]
|      :|   0x06d92ff4      ldr   x4, [arg_98h]                        ; [0x98:4]=0x779b960
|      :|   0x06d92ff8      ldr   x0, [x4, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      :|   0x06d92ffc      bl    fcn.063bfbd0
|      :|   0x06d93000      add   x0, arg_28h
|      :|   0x06d93004      bl    fcn.04d35870
|      :|   ; CODE XREF from fcn.06d92ee8 @ 0x6d92fe8
|      :`-> 0x06d93008      ldp   x4, x5, [arg_28h]
|      `==< 0x06d9300c      cbnz  x4, 0x6d92fec
|           0x06d93010      adrp  x27, 0x957c000
|           0x06d93014      ldr   x1, [x27, 0xf18]                     ; [0xf18:4]=0x58585858
|           0x06d93018      add   x2, arg_28h
|           0x06d9301c      stp   xzr, xzr, [x2]
|           0x06d93020      stp   xzr, xzr, [x2, 0x10]
|           0x06d93024      stp   xzr, xzr, [x2, 0x20]
|           0x06d93028      stp   xzr, xzr, [x2, 0x30]
|           0x06d9302c      stp   xzr, xzr, [x2, 0x40]
|           0x06d93030      stp   xzr, xzr, [x2, 0x50]
|           0x06d93034      adrp  x0, 0x8332000
|           0x06d93038      add   x0, x0, 0x450
|           0x06d9303c      bl    fcn.04d35800
|       ,=< 0x06d93040      b     0x6d93060
|      .--> 0x06d93044      ldp   x1, x2, [x4]
|      :|   0x06d93048      ldr   x3, [x5]
|      :|   0x06d9304c      ldr   x4, [arg_98h]                        ; [0x98:4]=0x779b960
|      :|   0x06d93050      ldr   x0, [x4, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      :|   0x06d93054      bl    fcn.063bfd00
|      :|   0x06d93058      add   x0, arg_28h
|      :|   0x06d9305c      bl    fcn.04d35870
|      :|   ; CODE XREF from fcn.06d92ee8 @ 0x6d93040
|      :`-> 0x06d93060      ldp   x4, x5, [arg_28h]
|      `==< 0x06d93064      cbnz  x4, 0x6d93044
|           0x06d93068      mov   x0, xzr
|           0x06d9306c      mov   x1, xzr
|           0x06d93070      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d93074      ldr   lr, [sp], 0x90                       ; [0x90:4]=0
|           0x06d93078      ret
==== google3_third_party_jetski_gateway_gateway.mergeInto
/ fcn.06d92ee8(int64_t arg6, int64_t arg_28h, int64_t arg_90h, int64_t arg_98h);
|       :   ; arg int64_t arg6 @ x5
|       :   ; arg int64_t arg_28h @ stack + 0x28
|       :   ; arg int64_t arg_90h @ stack + 0x90
|       :   ; arg int64_t arg_98h @ stack + 0x98
|       :   0x06d92ee8      ldr   x3, [x5]                             ; arg6
|       :   0x06d92eec      ldr   x4, [arg_98h]                        ; [0x98:4]=0x779b960
|       :   0x06d92ef0      ldr   x0, [x4, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|       :   0x06d92ef4      bl    fcn.063bf840
|       :   0x06d92ef8      add   x0, arg_28h
|       :   0x06d92efc      bl    fcn.04d35870
|       :   0x06d92f00      ldp   x4, x5, [arg_28h]
|       `=< 0x06d92f04      cbnz  x4, 0x6d92ee4
|           0x06d92f08      adrp  x27, 0x957c000
|           0x06d92f0c      ldr   x1, [x27, 0xf00]                     ; [0xf00:4]=0x58585858
|           0x06d92f10      add   x2, arg_28h
|           0x06d92f14      stp   xzr, xzr, [x2]
|           0x06d92f18      stp   xzr, xzr, [x2, 0x10]
|           0x06d92f1c      stp   xzr, xzr, [x2, 0x20]
|           0x06d92f20      stp   xzr, xzr, [x2, 0x30]
|           0x06d92f24      stp   xzr, xzr, [x2, 0x40]
|           0x06d92f28      stp   xzr, xzr, [x2, 0x50]
|           0x06d92f2c      adrp  x0, 0x8332000
|           0x06d92f30      add   x0, x0, 0x3c8
|           0x06d92f34      bl    fcn.04d35800
|       ,=< 0x06d92f38      b     0x6d92f58
|      .--> 0x06d92f3c      ldp   x1, x2, [x4]
|      :|   0x06d92f40      ldr   x3, [x5]
|      :|   0x06d92f44      ldr   x4, [arg_98h]                        ; [0x98:4]=0x779b960
|      :|   0x06d92f48      ldr   x0, [x4, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      :|   0x06d92f4c      bl    fcn.063bf970
|      :|   0x06d92f50      add   x0, arg_28h
|      :|   0x06d92f54      bl    fcn.04d35870
|      :|   ; CODE XREF from fcn.06d92ee8 @ 0x6d92f38
|      :`-> 0x06d92f58      ldp   x4, x5, [arg_28h]
|      `==< 0x06d92f5c      cbnz  x4, 0x6d92f3c
|           0x06d92f60      adrp  x27, 0x957c000
|           0x06d92f64      ldr   x1, [x27, 0xf08]                     ; [0xf08:4]=0x58585858
|           0x06d92f68      add   x2, arg_28h
|           0x06d92f6c      stp   xzr, xzr, [x2]
|           0x06d92f70      stp   xzr, xzr, [x2, 0x10]
|           0x06d92f74      stp   xzr, xzr, [x2, 0x20]
|           0x06d92f78      stp   xzr, xzr, [x2, 0x30]
|           0x06d92f7c      stp   xzr, xzr, [x2, 0x40]
|           0x06d92f80      stp   xzr, xzr, [x2, 0x50]
|           0x06d92f84      adrp  x0, 0x8332000
|           0x06d92f88      add   x0, x0, 0x5e8
|           0x06d92f8c      bl    fcn.04d35800
|       ,=< 0x06d92f90      b     0x6d92fb0
|      .--> 0x06d92f94      ldp   x1, x2, [x4]
|      :|   0x06d92f98      ldr   x3, [x5]
|      :|   0x06d92f9c      ldr   x4, [arg_98h]                        ; [0x98:4]=0x779b960
|      :|   0x06d92fa0      ldr   x0, [x4, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      :|   0x06d92fa4      bl    fcn.063bfaa0
|      :|   0x06d92fa8      add   x0, arg_28h
|      :|   0x06d92fac      bl    fcn.04d35870
|      :|   ; CODE XREF from fcn.06d92ee8 @ 0x6d92f90
|      :`-> 0x06d92fb0      ldp   x4, x5, [arg_28h]
|      `==< 0x06d92fb4      cbnz  x4, 0x6d92f94
|           0x06d92fb8      adrp  x27, 0x957c000
|           0x06d92fbc      ldr   x1, [x27, 0xf10]                     ; [0xf10:4]=0x58585858
|           0x06d92fc0      add   x2, arg_28h
|           0x06d92fc4      stp   xzr, xzr, [x2]
|           0x06d92fc8      stp   xzr, xzr, [x2, 0x10]
|           0x06d92fcc      stp   xzr, xzr, [x2, 0x20]
|           0x06d92fd0      stp   xzr, xzr, [x2, 0x30]
|           0x06d92fd4      stp   xzr, xzr, [x2, 0x40]
|           0x06d92fd8      stp   xzr, xzr, [x2, 0x50]
|           0x06d92fdc      adrp  x0, 0x8332000
|           0x06d92fe0      add   x0, x0, 0x560
|           0x06d92fe4      bl    fcn.04d35800
|       ,=< 0x06d92fe8      b     0x6d93008
|      .--> 0x06d92fec      ldp   x1, x2, [x4]
|      :|   0x06d92ff0      ldr   x3, [x5]
|      :|   0x06d92ff4      ldr   x4, [arg_98h]                        ; [0x98:4]=0x779b960
|      :|   0x06d92ff8      ldr   x0, [x4, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      :|   0x06d92ffc      bl    fcn.063bfbd0
|      :|   0x06d93000      add   x0, arg_28h
|      :|   0x06d93004      bl    fcn.04d35870
|      :|   ; CODE XREF from fcn.06d92ee8 @ 0x6d92fe8
|      :`-> 0x06d93008      ldp   x4, x5, [arg_28h]
|      `==< 0x06d9300c      cbnz  x4, 0x6d92fec
|           0x06d93010      adrp  x27, 0x957c000
|           0x06d93014      ldr   x1, [x27, 0xf18]                     ; [0xf18:4]=0x58585858
|           0x06d93018      add   x2, arg_28h
|           0x06d9301c      stp   xzr, xzr, [x2]
|           0x06d93020      stp   xzr, xzr, [x2, 0x10]
|           0x06d93024      stp   xzr, xzr, [x2, 0x20]
|           0x06d93028      stp   xzr, xzr, [x2, 0x30]
|           0x06d9302c      stp   xzr, xzr, [x2, 0x40]
|           0x06d93030      stp   xzr, xzr, [x2, 0x50]
|           0x06d93034      adrp  x0, 0x8332000
|           0x06d93038      add   x0, x0, 0x450
|           0x06d9303c      bl    fcn.04d35800
|       ,=< 0x06d93040      b     0x6d93060
|      .--> 0x06d93044      ldp   x1, x2, [x4]
|      :|   0x06d93048      ldr   x3, [x5]
|      :|   0x06d9304c      ldr   x4, [arg_98h]                        ; [0x98:4]=0x779b960
|      :|   0x06d93050      ldr   x0, [x4, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      :|   0x06d93054      bl    fcn.063bfd00
|      :|   0x06d93058      add   x0, arg_28h
|      :|   0x06d9305c      bl    fcn.04d35870
|      :|   ; CODE XREF from fcn.06d92ee8 @ 0x6d93040
|      :`-> 0x06d93060      ldp   x4, x5, [arg_28h]
|      `==< 0x06d93064      cbnz  x4, 0x6d93044
|           0x06d93068      mov   x0, xzr
|           0x06d9306c      mov   x1, xzr
|           0x06d93070      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d93074      ldr   lr, [sp], 0x90                       ; [0x90:4]=0
|           0x06d93078      ret
==== google3_third_party_jetski_gateway_gateway.mergeGateway
/ fcn.06d93178(int64_t arg3, int64_t arg_28h, int64_t arg_90h, int64_t arg_b0h);
|           ; arg int64_t arg3 @ x2
|           ; arg int64_t arg_28h @ stack + 0x28
|           ; arg int64_t arg_90h @ stack + 0x90
|           ; arg int64_t arg_b0h @ stack + 0xb0
|           0x06d93178      stp   xzr, xzr, [x2, 0x50]                 ; arg3
|           0x06d9317c      adrp  x0, 0x8312000
|           0x06d93180      add   x0, x0, 0xe48
|           0x06d93184      bl    fcn.04d35800
|       ,=< 0x06d93188      b     0x6d931a8
|      .--> 0x06d9318c      ldr   x1, [x3]
|      :|   0x06d93190      ldr   x2, [x4]
|      :|   0x06d93194      ldr   x3, [arg_b0h]                        ; [0xb0:4]=0x6474e551
|      :|   0x06d93198      ldr   x0, [x3, 0x10]                       ; [0x10:4]=0xb70003
|      :|   0x06d9319c      bl    fcn.063c2090
|      :|   0x06d931a0      add   x0, arg_28h
|      :|   0x06d931a4      bl    fcn.04d35870
|      :|   ; CODE XREF from fcn.06d93178 @ 0x6d93188
|      :`-> 0x06d931a8      ldp   x3, x4, [arg_28h]
|      `==< 0x06d931ac      cbnz  x3, 0x6d9318c
|           0x06d931b0      ldr   x0, [arg_b0h]                        ; [0xb0:4]=0x6474e551
|           0x06d931b4      bl    fcn.06d92e90
|           0x06d931b8      mov   x0, xzr
|           0x06d931bc      mov   x1, xzr
|           0x06d931c0      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d931c4      ldr   lr, [sp], 0x90                       ; [0x90:4]=0
\           0x06d931c8      ret
==== google3_third_party_jetski_gateway_gateway.mergeModels
/ fcn.06d934f8(int64_t arg6, int64_t arg7, int64_t arg8, int64_t arg_8h, int64_t arg_118h, int64_t arg_120h, int64_t arg_128h, int64_t arg_130h, int64_t arg_138h, int64_t arg_140h, int64_t arg_148h, int64_t arg_150h, int64_t arg_158h, int64_t arg_160h, int64_t arg_168h, int64_t arg_180h, int64_t arg_188h, int64_t arg_190h, int64_t arg_198h, int64_t arg_1a0h, int64_t arg_1a8h, int64_t arg_1b0h, int64_t arg_1b8h, int64_t arg_258h, int64_t arg_278h, int64_t arg_280h, int64_t arg_2d8h, int64_t arg_2e0h, int64_t arg_2e8h, int64_t arg_2f0h, int64_t arg_2f8h, int64_t arg_300h, int64_t arg_308h, int64_t arg_310h, int64_t arg_318h, int64_t arg_320h, int64_t arg_328h, int64_t arg_330h, int64_t arg_338h, int64_t arg_368h, int64_t arg_378h, int64_t arg_380h, int64_t arg_390h, int64_t arg_3e8h, int64_t arg_3f8h, int64_t arg_408h, int64_t arg_450h, int64_t arg_4c0h, int64_t arg_4c8h, int64_t arg_4d0h, int64_t arg_4e8h, int64_t arg_4f0h, int64_t arg_4f8h, int64_t arg_500h, int64_t arg_508h, int64_t arg_518h, int64_t arg_520h, int64_t arg_9f8h);
|           ; arg int64_t arg6 @ x5
|           ; arg int64_t arg7 @ x6
|           ; arg int64_t arg8 @ x7
|           ; arg int64_t arg_8h @ stack + 0x8
|           ; arg int64_t arg_118h @ stack + 0x118
|           ; arg int64_t arg_120h @ stack + 0x120
|           ; arg int64_t arg_128h @ stack + 0x128
|           ; arg int64_t arg_130h @ stack + 0x130
|           ; arg int64_t arg_138h @ stack + 0x138
|           ; arg int64_t arg_140h @ stack + 0x140
|           ; arg int64_t arg_148h @ stack + 0x148
|           ; arg int64_t arg_150h @ stack + 0x150
|           ; arg int64_t arg_158h @ stack + 0x158
|           ; arg int64_t arg_160h @ stack + 0x160
|           ; arg int64_t arg_168h @ stack + 0x168
|           ; arg int64_t arg_180h @ stack + 0x180
|           ; arg int64_t arg_188h @ stack + 0x188
|           ; arg int64_t arg_190h @ stack + 0x190
|           ; arg int64_t arg_198h @ stack + 0x198
|           ; arg int64_t arg_1a0h @ stack + 0x1a0
|           ; arg int64_t arg_1a8h @ stack + 0x1a8
|           ; arg int64_t arg_1b0h @ stack + 0x1b0
|           ; arg int64_t arg_1b8h @ stack + 0x1b8
|           ; arg int64_t arg_258h @ stack + 0x258
|           ; arg int64_t arg_278h @ stack + 0x278
|           ; arg int64_t arg_280h @ stack + 0x280
|           ; arg int64_t arg_2d8h @ stack + 0x2d8
|           ; arg int64_t arg_2e0h @ stack + 0x2e0
|           ; arg int64_t arg_2e8h @ stack + 0x2e8
|           ; arg int64_t arg_2f0h @ stack + 0x2f0
|           ; arg int64_t arg_2f8h @ stack + 0x2f8
|           ; arg int64_t arg_300h @ stack + 0x300
|           ; arg int64_t arg_308h @ stack + 0x308
|           ; arg int64_t arg_310h @ stack + 0x310
|           ; arg int64_t arg_318h @ stack + 0x318
|           ; arg int64_t arg_320h @ stack + 0x320
|           ; arg int64_t arg_328h @ stack + 0x328
|           ; arg int64_t arg_330h @ stack + 0x330
|           ; arg int64_t arg_338h @ stack + 0x338
|           ; arg int64_t arg_368h @ stack + 0x368
|           ; arg int64_t arg_378h @ stack + 0x378
|           ; arg int64_t arg_380h @ stack + 0x380
|           ; arg int64_t arg_390h @ stack + 0x390
|           ; arg int64_t arg_3e8h @ stack + 0x3e8
|           ; arg int64_t arg_3f8h @ stack + 0x3f8
|           ; arg int64_t arg_408h @ stack + 0x408
|           ; arg int64_t arg_450h @ stack + 0x450
|           ; arg int64_t arg_4c0h @ stack + 0x4c0
|           ; arg int64_t arg_4c8h @ stack + 0x4c8
|           ; arg int64_t arg_4d0h @ stack + 0x4d0
|           ; arg int64_t arg_4e8h @ stack + 0x4e8
|           ; arg int64_t arg_4f0h @ stack + 0x4f0
|           ; arg int64_t arg_4f8h @ stack + 0x4f8
|           ; arg int64_t arg_500h @ stack + 0x500
|           ; arg int64_t arg_508h @ stack + 0x508
|           ; arg int64_t arg_518h @ stack + 0x518
|           ; arg int64_t arg_520h @ stack + 0x520
|           ; arg int64_t arg_9f8h @ stack + 0x9f8
|           0x06d934f8      ldrsw x9, [x10]
|       ,=< 0x06d934fc      b     0x6d9350c
..
|      ||   ; CODE XREF from fcn.06d934f8 @ 0x6d934fc
|      ``-> 0x06d9350c      str   x5, [arg_130h]                       ; arg6
|           0x06d93510      str   x7, [arg_2d8h]                       ; arg8
|           0x06d93514      str   x6, [arg_138h]                       ; arg7
|           0x06d93518      str   x8, [arg_2e0h]
|           0x06d9351c      ldr   x10, [arg_4e8h]                      ; [0x4e8:4]=0x58585858
|           0x06d93520      ldr   x11, [x10]
|           0x06d93524      ldp   x11, x12, [x11, 0x108]
|       ,=< 0x06d93528      cbz   x11, 0x6d93564
|       |   0x06d9352c      ldr   x4, [x11, 0x18]                      ; [0x18:4]=0x4d0f000 section..text
|       |   0x06d93530      mov   x0, x12
|       |   0x06d93534      ldr   x1, [arg_4f0h]                       ; [0x4f0:4]=0x58585858
|       |   0x06d93538      ldr   x2, [arg_4f8h]                       ; [0x4f8:4]=0x58585858
|       |   0x06d9353c      mov   x3, x9
|       |   0x06d93540      blr   x4
|      ,==< 0x06d93544      cbz   x0, 0x6d93d74
|      ||   0x06d93548      ldr   x5, [x0, 8]                          ; [0x8:4]=0
|      ||   0x06d9354c      adrp  x6, 0x94a8000
|      ||   0x06d93550      add   x6, x6, 0x480
|      ||   0x06d93554      ldar  x7, [x6]
|      ||   0x06d93558      ldr   x8, [x7]                             ; arg8
|      ||   0x06d9355c      ldr   w9, [x0, 0x10]                       ; [0x10:4]=0xb70003
|     ,===< 0x06d93560      b     0x6d93d84
|     ||`-> 0x06d93564      mov   x9, xzr
|     ||    0x06d93568      mov   x11, xzr
|     ||,=< 0x06d9356c      b     0x6d93598
..
|     |||   ; CODE XREFS from fcn.06d934f8 @ 0x6d9356c, 0x6d93d80
|    .--`-> 0x06d93598      str   x11, [arg_190h]
|    :||    0x06d9359c      str   x9, [arg_118h]
|    :||    0x06d935a0      ldr   x12, [x10]
|    :||    0x06d935a4      ldrsb x27, [x12]
|    :||    0x06d935a8      add   x13, x12, 0xb0
|    :||    0x06d935ac      ldr   x12, [x12, 0xb8]                     ; [0xb8:4]=0
|    :||    0x06d935b0      add   x14, arg_280h
|    :||    0x06d935b4      ldp   q16, q17, [x13]
|    :||    0x06d935b8      stp   q16, q17, [x14]
|    :||    0x06d935bc      ldp   q16, q17, [x13, 0x20]
|    :||    0x06d935c0      stp   q16, q17, [x14, 0x20]
|    :||    0x06d935c4      ldr   q16, [x13, 0x40]                     ; segment.PHDR
|    :||                                                               ; [0x40:4]=6 ; "\U00000006"
|    :||    0x06d935c8      str   q16, [x14, 0x40]
|    :||    0x06d935cc      ldr   x25, [x13, 0x50]                     ; [0x50:4]=64
|    :||    0x06d935d0      str   x25, [x14, 0x50]
|    :||,=< 0x06d935d4      cbnz  x12, 0x6d93608
|    :|||   0x06d935d8      orr   x0, xzr, 8
|    :|||   0x06d935dc      adrp  x1, 0x81a4000
|    :|||   0x06d935e0      add   x1, x1, 0x580
|    :|||   0x06d935e4      orr   x2, xzr, 1
|    :|||   0x06d935e8      bl    0x4d322b0
|    :|||   0x06d935ec      adrp  x3, 0x8f91000
|    :|||   0x06d935f0      add   x3, x3, 0x3a8
|    :|||   0x06d935f4      str   x3, [x0]
|    :|||   0x06d935f8      orr   x3, xzr, 1
|    :|||   0x06d935fc      add   x27, arg_280h
|    :|||   0x06d93600      stp   x0, x3, [x27]
|    :|||   0x06d93604      str   x3, [arg_280h + 0x10]
|    :||`-> 0x06d93608      add   x27, arg_280h
|    :||    0x06d9360c      ldp   x12, x13, [x27]
|    :||,=< 0x06d93610      cbnz  x13, 0x6d93624
|    :|||   0x06d93614      mov   x14, xzr
|    :|||   0x06d93618      mov   x15, xzr
|    :|||   0x06d9361c      mov   x16, xzr
|   ,=====< 0x06d93620      b     0x6d9365c
|   |:||`-> 0x06d93624      str   x13, [arg_158h]
|   |:||    0x06d93628      str   x12, [arg_328h]
|   |:||    0x06d9362c      mov   x0, xzr
|   |:||    0x06d93630      mov   x1, x13
|   |:||    0x06d93634      mov   x2, xzr
|   |:||    0x06d93638      mov   x3, x1
|   |:||    0x06d9363c      adrp  x4, 0x86f9000
|   |:||    0x06d93640      add   x4, x4, 0x788
|   |:||    0x06d93644      bl    0x4d9e050
|   |:||    0x06d93648      ldr   x12, [arg_328h]                      ; [0x328:4]=0xd8f5070 segment.INTERP
|   |:||    0x06d9364c      ldr   x13, [arg_158h]                      ; [0x158:4]=0x6474e552
|   |:||    0x06d93650      mov   x14, x0
|   |:||    0x06d93654      mov   x15, x1
|   |:||    0x06d93658      mov   x16, x2
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d93620
|   `-----> 0x06d9365c      stp   x15, x16, [arg_150h]
|    :||    0x06d93660      str   x14, [arg_328h]
|    :||    0x06d93664      adrp  x0, 0x86f9000
|    :||    0x06d93668      add   x0, x0, 0x788
|    :||    0x06d9366c      mov   x1, x14
|    :||    0x06d93670      mov   x2, x15
|    :||    0x06d93674      mov   x3, x12
|    :||    0x06d93678      mov   x4, x13
|    :||    0x06d9367c      bl    0x4d99570
|    :||    0x06d93680      orr   x0, xzr, 0x20
|    :||    0x06d93684      adrp  x1, 0x81a4000
|    :||    0x06d93688      add   x1, x1, 0xa48
|    :||    0x06d9368c      orr   x2, xzr, 1
|    :||    0x06d93690      bl    0x4d32c00
|    :||    0x06d93694      ldr   x5, [arg_160h]                       ; [0x160:4]=0x779b960
|    :||    0x06d93698      str   x5, [x0]
|    :||    0x06d9369c      ldr   x5, [arg_168h]                       ; [0x168:4]=0x799b960 segment.GNU_RELRO
|    :||    0x06d936a0      str   x5, [x0, 0x18]
|    :||    0x06d936a4      adrp  x27, 0xb48f000
|    :||    0x06d936a8      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d936ac      cbnz  w5, 0x6d936bc
|    :|||   0x06d936b0      ldr   x3, [arg_338h]                       ; [0x338:4]=27
|    :|||   0x06d936b4      ldr   x4, [arg_330h]                       ; [0x330:4]=0xd8f5070 segment.INTERP
|   ,=====< 0x06d936b8      b     0x6d936cc
|   |:||`-> 0x06d936bc      bl    0x4da3500
|   |:||    0x06d936c0      ldr   x3, [arg_338h]                       ; [0x338:4]=27
|   |:||    0x06d936c4      ldr   x4, [arg_330h]                       ; [0x330:4]=0xd8f5070 segment.INTERP
|   |:||    0x06d936c8      stp   x3, x4, [x25]
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d936b8
|   `-----> 0x06d936cc      str   x0, [arg_320h]
|    :||    0x06d936d0      stp   x3, x4, [x0, 8]
|    :||    0x06d936d4      orr   x0, xzr, 0x20
|    :||    0x06d936d8      adrp  x1, 0x87f7000
|    :||    0x06d936dc      add   x1, x1, 0xe18
|    :||    0x06d936e0      orr   x2, xzr, 1
|    :||    0x06d936e4      bl    0x4d32c00
|    :||    0x06d936e8      adrp  x3, 0x673f000
|    :||    0x06d936ec      add   x3, x3, 0x490
|    :||    0x06d936f0      str   x3, [x0]
|    :||    0x06d936f4      orr   x3, xzr, 1
|    :||    0x06d936f8      stp   x3, x3, [x0, 0x10]
|    :||    0x06d936fc      adrp  x27, 0xb48f000
|    :||    0x06d93700      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d93704      cbnz  w4, 0x6d93710
|    :|||   0x06d93708      ldr   x5, [arg_320h]                       ; [0x320:4]=0xacf5070
|   ,=====< 0x06d9370c      b     0x6d9371c
|   |:||`-> 0x06d93710      bl    0x4da34f0
|   |:||    0x06d93714      ldr   x5, [arg_320h]                       ; [0x320:4]=0xacf5070
|   |:||    0x06d93718      str   x5, [x25]
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d9370c
|   `-----> 0x06d9371c      str   x5, [x0, 8]
|    :||    0x06d93720      ldr   x5, [arg_150h]                       ; [0x150:4]=32
|    :||    0x06d93724      add   x1, x5, 1
|    :||    0x06d93728      ldr   x2, [arg_158h]                       ; [0x158:4]=0x6474e552
|    :||    0x06d9372c      cmp   x2, x1
|    :||,=< 0x06d93730      b.lo  0x6d9373c
|    :|||   0x06d93734      ldr   x3, [arg_328h]                       ; [0x328:4]=0xd8f5070 segment.INTERP
|   ,=====< 0x06d93738      b     0x6d93758
|   |:||`-> 0x06d9373c      str   x0, [arg_318h]
|   |:||    0x06d93740      ldr   x0, [arg_328h]                       ; [0x328:4]=0xd8f5070 segment.INTERP
|   |:||    0x06d93744      adrp  x4, 0x86f9000
|   |:||    0x06d93748      add   x4, x4, 0x788
|   |:||    0x06d9374c      bl    0x4d9e050
|   |:||    0x06d93750      mov   x3, x0
|   |:||    0x06d93754      ldr   x0, [arg_318h]                       ; [0x318:4]=3
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d93738
|   `-----> 0x06d93758      sub   x4, x1, 1
|    :||    0x06d9375c      adrp  x27, 0xb48f000
|    :||    0x06d93760      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d93764      cbz   w5, 0x6d93774
|    :|||   0x06d93768      ldr   x5, [x3, x4, lsl 3]
|    :|||   0x06d9376c      bl    0x4da3500
|    :|||   0x06d93770      stp   x0, x5, [x25]
|    :||`-> 0x06d93774      str   x0, [x3, x4, lsl 3]
|    :||    0x06d93778      add   x27, arg_280h
|    :||    0x06d9377c      stp   x3, x1, [x27]
|    :||    0x06d93780      str   x2, [arg_280h + 0x10]
|    :||    0x06d93784      add   x3, arg_390h
|    :||    0x06d93788      stp   xzr, xzr, [x3]
|    :||    0x06d9378c      stp   xzr, xzr, [x3, 0x10]
|    :||    0x06d93790      stp   xzr, xzr, [x3, 0x20]
|    :||    0x06d93794      stp   xzr, xzr, [x3, 0x30]
|    :||    0x06d93798      stp   xzr, xzr, [x3, 0x40]
|    :||    0x06d9379c      stp   xzr, xzr, [x3, 0x50]
|    :||    0x06d937a0      stp   xzr, xzr, [x3, 0x60]
|    :||    0x06d937a4      stp   xzr, xzr, [x3, 0x70]
|    :||    0x06d937a8      stp   xzr, xzr, [x3, 0x80]
|    :||    0x06d937ac      stp   xzr, xzr, [x3, 0x90]
|    :||    0x06d937b0      stp   xzr, xzr, [x3, 0xa0]
|    :||    0x06d937b4      str   xzr, [x3, 0xb0]
|    :||    0x06d937b8      add   x4, arg_390h
|    :||    0x06d937bc      add   x5, arg_280h
|    :||    0x06d937c0      ldp   q16, q17, [x5]
|    :||    0x06d937c4      stp   q16, q17, [x4]
|    :||    0x06d937c8      ldp   q16, q17, [x5, 0x20]
|    :||    0x06d937cc      stp   q16, q17, [x4, 0x20]
|    :||    0x06d937d0      ldr   q16, [x5, 0x40]                      ; segment.PHDR
|    :||                                                               ; [0x40:4]=6 ; "\U00000006"
|    :||    0x06d937d4      str   q16, [x4, 0x40]
|    :||    0x06d937d8      ldr   x25, [x5, 0x50]                      ; [0x50:4]=64
|    :||    0x06d937dc      str   x25, [x4, 0x50]
|    :||    0x06d937e0      ldr   x4, [arg_1a0h]                       ; [0x1a0:4]=0x799bde0 segment.DYNAMIC
|    :||    0x06d937e4      ldr   x5, [arg_138h]                       ; [0x138:4]=0x799b960 segment.GNU_RELRO
|    :||    0x06d937e8      add   x27, arg_3f8h
|    :||    0x06d937ec      stp   x4, x5, [x27]
|    :||    0x06d937f0      ldr   x4, [arg_2e0h]                       ; [0x2e0:4]=1
|    :||    0x06d937f4      ldr   x5, [arg_2d8h]                       ; [0x2d8:4]=0x200000
|    :||    0x06d937f8      add   x27, arg_408h
|    :||    0x06d937fc      stp   x4, x5, [x27]
|    :||    0x06d93800      ldr   x4, [arg_130h]                       ; [0x130:4]=0x799b960 segment.GNU_RELRO
|    :||    0x06d93804      str   x4, [arg_408h + 0x10]
|    :||    0x06d93808      ldr   x4, [arg_118h]                       ; [0x118:4]=0x200000
|    :||    0x06d9380c      ldr   x5, [arg_190h]                       ; [0x190:4]=2
|    :||    0x06d93810      add   x27, arg_3e8h
|    :||    0x06d93814      stp   x4, x5, [x27]
|    :||    0x06d93818      ldr   x4, [arg_518h]                       ; [0x518:4]=0x58585858
|    :||    0x06d9381c      str   x4, [arg_408h + 0x18]
|    :||    0x06d93820      ldr   x0, [arg_520h]                       ; [0x520:4]=0x58585858
|    :||    0x06d93824      ldr   x4, [arg_4e8h]                       ; [0x4e8:4]=0x58585858
|    :||    0x06d93828      ldp   x1, x2, [x4, 8]
|    :||    0x06d9382c      add   x4, arg_8h
|    :||    0x06d93830      ldp   q16, q17, [x3]
|    :||    0x06d93834      stp   q16, q17, [x4]
|    :||    0x06d93838      ldp   q16, q17, [x3, 0x20]
|    :||    0x06d9383c      stp   q16, q17, [x4, 0x20]
|    :||    0x06d93840      ldp   q16, q17, [x3, 0x40]
|    :||    0x06d93844      stp   q16, q17, [x4, 0x40]
|    :||    0x06d93848      ldp   q16, q17, [x3, 0x60]
|    :||    0x06d9384c      stp   q16, q17, [x4, 0x60]
|    :||    0x06d93850      ldp   q16, q17, [x3, 0x80]
|    :||    0x06d93854      stp   q16, q17, [x4, 0x80]
|    :||    0x06d93858      ldr   q16, [x3, 0xa0]                      ; [0xa0:4]=0x779b960
|    :||    0x06d9385c      str   q16, [x4, 0xa0]
|    :||    0x06d93860      ldr   x25, [x3, 0xb0]                      ; [0xb0:4]=0x6474e551
|    :||    0x06d93864      str   x25, [x4, 0xb0]
|    :||    0x06d93868      bl    0x672a030
|    :||,=< 0x06d9386c      cbnz  x1, 0x6d93b4c
|    :|||   0x06d93870      str   x0, [arg_278h]
|    :|||   0x06d93874      ldr   x0, [arg_4f0h]                       ; [0x4f0:4]=0x58585858
|    :|||   0x06d93878      ldr   x1, [arg_4f8h]                       ; [0x4f8:4]=0x58585858
|    :|||   0x06d9387c      bl    0x6711700
|    :|||   0x06d93880      str   x0, [arg_128h]
|    :|||   0x06d93884      str   x1, [arg_1a8h]
|    :|||   0x06d93888      orr   x0, xzr, 8
|    :|||   0x06d9388c      adrp  x1, 0x800b000
|    :|||   0x06d93890      add   x1, x1, 0xf98
|    :|||   0x06d93894      orr   x2, xzr, 1
|    :|||   0x06d93898      bl    0x4d322b0
|    :|||   0x06d9389c      str   x0, [arg_4c8h]
|    :|||   0x06d938a0      orr   x0, xzr, 0x18
|    :|||   0x06d938a4      adrp  x1, 0x81b3000
|    :|||   0x06d938a8      add   x1, x1, 0xd48
|    :|||   0x06d938ac      orr   x2, xzr, 1
|    :|||   0x06d938b0      bl    0x4d32830
|    :|||   0x06d938b4      str   x0, [arg_4c0h]
|    :|||   0x06d938b8      adrp  x0, 0x8c17000
|    :|||   0x06d938bc      add   x0, x0, 0x978
|    :|||   0x06d938c0      bl    0x4d31c10
|    :|||   0x06d938c4      nop
|    :|||   0x06d938c8      adr   x2, 0x6d944d0
|    :|||   0x06d938cc      str   x2, [x0]
|    :|||   0x06d938d0      adrp  x27, 0xb48f000
|    :|||   0x06d938d4      ldr   w2, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d938d8      cbnz  w2, 0x6d938e4
|   |:|||   0x06d938dc      ldr   x6, [arg_4c8h]                       ; [0x4c8:4]=0x58585858
|  ,======< 0x06d938e0      b     0x6d938f0
|  |`-----> 0x06d938e4      bl    0x4da34f0
|  | :|||   0x06d938e8      ldr   x6, [arg_4c8h]                       ; [0x4c8:4]=0x58585858
|  | :|||   0x06d938ec      str   x6, [x25]
|  | :|||   ; CODE XREF from fcn.06d934f8 @ 0x6d938e0
|  `------> 0x06d938f0      str   x6, [x0, 8]
|    :|||   0x06d938f4      ldr   x1, [arg_128h]                       ; [0x128:4]=0x779b960
|    :|||   0x06d938f8      str   x1, [x0, 0x10]
|    :|||   0x06d938fc      adrp  x27, 0xb48f000
|    :|||   0x06d93900      ldr   w7, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d93904      cbz   w7, 0x6d93944
|   |:|||   0x06d93908      str   x0, [arg_310h]
|   |:|||   0x06d9390c      ldr   x3, [x0, 0x60]                       ; [0x60:4]=0x380
|   |:|||   0x06d93910      bl    0x4da3520
|   |:|||   0x06d93914      ldr   x4, [arg_1a8h]                       ; [0x1a8:4]=0x799bde0 segment.DYNAMIC
|   |:|||   0x06d93918      ldr   x5, [arg_4d0h]                       ; [0x4d0:4]=0x58585858
|   |:|||   0x06d9391c      stp   x4, x5, [x25]
|   |:|||   0x06d93920      ldr   x4, [arg_4c0h]                       ; [0x4c0:4]=0x58585858
|   |:|||   0x06d93924      stp   x4, x3, [x25, 0x10]
|   |:|||   0x06d93928      add   x1, x0, 0x28
|   |:|||   0x06d9392c      adrp  x0, 0x8b70000
|   |:|||   0x06d93930      add   x0, x0, 0x778
|   |:|||   0x06d93934      add   x2, arg_508h
|   |:|||   0x06d93938      bl    0x4d35930
|   |:|||   0x06d9393c      ldr   x0, [arg_310h]                       ; [0x310:4]=0x10000
|   |:|||   0x06d93940      ldr   x1, [arg_128h]                       ; [0x128:4]=0x779b960
|   `-----> 0x06d93944      ldr   x2, [arg_1a8h]                       ; [0x1a8:4]=0x799bde0 segment.DYNAMIC
|    :|||   0x06d93948      ldr   x6, [arg_4d0h]                       ; [0x4d0:4]=0x58585858
|    :|||   0x06d9394c      stp   x2, x6, [x0, 0x18]
|    :|||   0x06d93950      add   x27, arg_508h
|    :|||   0x06d93954      ldp   q0, q1, [x27]
|    :|||   0x06d93958      add   x27, arg_520h
|    :|||   0x06d9395c      ldp   q2, q3, [x27]
|    :|||   0x06d93960      add   x27, x0, 0x28
|    :|||   0x06d93964      stp   q0, q1, [x27]
|    :|||   0x06d93968      stp   q2, q3, [x0, 0x40]
|    :|||   0x06d9396c      ldr   x6, [arg_4c0h]                       ; [0x4c0:4]=0x58585858
|    :|||   0x06d93970      str   x6, [x0, 0x60]
|    :|||   0x06d93974      adrp  x6, 0x6743000
|    :|||   0x06d93978      add   x6, x6, 0x100
|    :|||   0x06d9397c      add   x27, arg_368h
|    :|||   0x06d93980      stp   x6, x0, [x27]
|    :|||   0x06d93984      add   x6, arg_390h
|    :|||   0x06d93988      orr   x7, xzr, 3
|   .-----> 0x06d9398c      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d93990      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d93994      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d93998      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d9399c      sub   x7, x7, 1
|   `=====< 0x06d939a0      cbnz  x7, 0x6d9398c
|    :|||   0x06d939a4      adrp  x6, 0x8f92000
|    :|||   0x06d939a8      add   x6, x6, 0xc28
|    :|||   0x06d939ac      add   x7, arg_368h
|    :|||   0x06d939b0      add   x27, arg_450h
|    :|||   0x06d939b4      stp   x6, x7, [x27]
|    :|||   0x06d939b8      ldr   x0, [arg_278h]                       ; [0x278:4]=0x9019dd0
|    :|||   0x06d939bc      add   x3, arg_450h
|    :|||   0x06d939c0      orr   x4, xzr, 2
|    :|||   0x06d939c4      mov   x5, x4
|    :|||   0x06d939c8      bl    0x672b7f0
|    :|||   0x06d939cc      add   x6, arg_390h
|    :|||   0x06d939d0      add   x7, arg_8h
|    :|||   0x06d939d4      orr   x24, xzr, 3
|   .-----> 0x06d939d8      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d939dc      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d939e0      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d939e4      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d939e8      sub   x24, x24, 1
|   `=====< 0x06d939ec      cbnz  x24, 0x6d939d8
|    :|||   0x06d939f0      add   x6, arg_1b8h
|    :|||   0x06d939f4      add   x7, arg_390h
|    :|||   0x06d939f8      orr   x24, xzr, 3
|   .-----> 0x06d939fc      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d93a00      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d93a04      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d93a08      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d93a0c      sub   x24, x24, 1
|   `=====< 0x06d93a10      cbnz  x24, 0x6d939fc
|   ,=====< 0x06d93a14      cbz   x0, 0x6d93b20
|   |:|||   0x06d93a18      str   x1, [arg_198h]
|   |:|||   0x06d93a1c      str   x0, [arg_120h]
|   |:|||   0x06d93a20      adrp  x27, 0x9491000
|   |:|||   0x06d93a24      add   x27, x27, 0x210
|   |:|||   0x06d93a28      ldp   x2, x3, [x27]
|   |:|||   0x06d93a2c      bl    0x5116df0
|  ,======< 0x06d93a30      tbz   w0, 0, 0x6d93b38
|  ||:|||   0x06d93a34      ldr   x3, [arg_9f8h]                       ; [0x518:4]=0x58585858
| ,=======< 0x06d93a38      cbz   x3, 0x6d93a44
| |||:|||   0x06d93a3c      ldr   x3, [x3, 8]                          ; [0x8:4]=0
| ========< 0x06d93a40      b     0x6d93a48
| `-------> 0x06d93a44      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a40
| ,=======< 0x06d93a48      cbz   x3, 0x6d93a70
| |||:|||   0x06d93a4c      ldp   x3, x4, [x3, 8]
| |||:|||   0x06d93a50      adrp  x5, 0x8f0d000
| |||:|||   0x06d93a54      add   x5, x5, 0x9c8
| |||:|||   0x06d93a58      cmp   x3, x5
| ========< 0x06d93a5c      b.eq  0x6d93a64
| |||:|||   0x06d93a60      mov   x4, xzr
| --------> 0x06d93a64      b.ne  0x6d93a70
| |||:|||   0x06d93a68      ldr   x3, [x4]
| ========< 0x06d93a6c      b     0x6d93a74
| `-------> 0x06d93a70      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a6c
| ,=======< 0x06d93a74      cbz   x3, 0x6d93a80
| |||:|||   0x06d93a78      ldr   x3, [x3, 0xe0]                       ; [0xe0:4]=0
| ========< 0x06d93a7c      b     0x6d93a84
| `-------> 0x06d93a80      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a7c
| ,=======< 0x06d93a84      cbz   x3, 0x6d93a90
| |||:|||   0x06d93a88      ldrsw x3, [x3, 8]                          ; [0x8:4]=0
| ========< 0x06d93a8c      b     0x6d93a94
| `-------> 0x06d93a90      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a8c
| --------> 0x06d93a94      cmp   w3, 1
| ,=======< 0x06d93a98      b.ne  0x6d93b38
| |||:|||   0x06d93a9c      orr   x0, xzr, 0x10
| |||:|||   0x06d93aa0      adrp  x1, 0x81a5000
| |||:|||   0x06d93aa4      add   x1, x1, 0x4f8
| |||:|||   0x06d93aa8      orr   x2, xzr, 1
| |||:|||   0x06d93aac      bl    0x4d32500
| |||:|||   0x06d93ab0      ldr   x1, [arg_120h]                       ; [0x120:4]=7
| ========< 0x06d93ab4      cbz   x1, 0x6d93ac0
| |||:|||   0x06d93ab8      ldr   x11, [x1, 8]                         ; [0x8:4]=0
| ========< 0x06d93abc      b     0x6d93ac4
| --------> 0x06d93ac0      mov   x11, x1
| |||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93abc
| --------> 0x06d93ac4      str   x11, [x0]
| |||:|||   0x06d93ac8      adrp  x27, 0xb48f000
| |||:|||   0x06d93acc      ldr   w11, [x27, 0x450]                    ; [0x450:4]=0x58585858
| ========< 0x06d93ad0      cbnz  w11, 0x6d93adc
| |||:|||   0x06d93ad4      ldr   x11, [arg_198h]                      ; [0x198:4]=0x779bde0
| ========< 0x06d93ad8      b     0x6d93ae8
| --------> 0x06d93adc      bl    0x4da34f0
| |||:|||   0x06d93ae0      ldr   x11, [arg_198h]                      ; [0x198:4]=0x779bde0
| |||:|||   0x06d93ae4      str   x11, [x25]
| |||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93ad8
| --------> 0x06d93ae8      str   x11, [x0, 8]
| |||:|||   0x06d93aec      ldr   x1, [arg_4f8h]                       ; [0x4f8:4]=0x58585858
| |||:|||   0x06d93af0      orr   x2, xzr, 1
| |||:|||   0x06d93af4      orr   x3, xzr, 1
| |||:|||   0x06d93af8      mov   x4, xzr
| |||:|||   0x06d93afc      mov   x5, xzr
| |||:|||   0x06d93b00      adrp  x6, 0x4479000
| |||:|||   0x06d93b04      add   x6, x6, 0x8ad
| |||:|||   0x06d93b08      orr   x7, xzr, 0x3c
| |||:|||   0x06d93b0c      mov   x8, x0
| |||:|||   0x06d93b10      mov   x9, x2
| |||:|||   0x06d93b14      mov   x10, x2
| |||:|||   0x06d93b18      ldr   x0, [arg_4f0h]                       ; [0x4f0:4]=0x58585858
| |||:|||   0x06d93b1c      bl    0x4f6f980
| ||`-----> 0x06d93b20      ldr   x4, [arg_4c0h]                       ; [0x4c0:4]=0x58585858
| || :|||   0x06d93b24      ldp   x4, x5, [x4]
| || :|||   0x06d93b28      str   x5, [arg_168h]
| || :|||   0x06d93b2c      str   x4, [arg_308h]
| || :|||   0x06d93b30      mov   x6, xzr
| ||,=====< 0x06d93b34      b     0x6d93b70
| ``------> 0x06d93b38      ldr   x0, [arg_120h]                       ; [0x120:4]=7
|   |:|||   0x06d93b3c      ldr   x1, [arg_198h]                       ; [0x198:4]=0x779bde0
|   |:|||   0x06d93b40      ldp   fp, lr, [sp, -8]
|   |:|||   0x06d93b44      add   sp, sp, 0x4e0
|   |:|||   0x06d93b48      ret
|   |:||`-> 0x06d93b4c      mov   x0, x1
|   |:||    0x06d93b50      mov   x1, x2
|   |:||    0x06d93b54      ldp   fp, lr, [sp, -8]
|   |:||    0x06d93b58      add   sp, sp, 0x4e0
|   |:||    0x06d93b5c      ret
|   |:||.-> 0x06d93b60      ldr   x7, [arg_158h]                       ; [0x158:4]=0x6474e552
|   |:||:   0x06d93b64      add   x6, x7, 1
|   |:||:   0x06d93b68      ldr   x4, [arg_308h]                       ; [0x308:4]=0x50d0
|   |:||:   0x06d93b6c      ldr   x5, [arg_168h]                       ; [0x168:4]=0x799b960 segment.GNU_RELRO
|   |:||:   ; CODE XREF from fcn.06d934f8 @ 0x6d93b34
|   `-----> 0x06d93b70      cmp   x6, x5
|   ,=====< 0x06d93b74      b.ge  0x6d93ca0
|   |:||:   0x06d93b78      str   x6, [arg_158h]
|   |:||:   0x06d93b7c      ldr   x7, [x4, x6, lsl 3]
|   |:||:   0x06d93b80      str   x7, [arg_188h]
|   |:||:   0x06d93b84      ldp   x8, x9, [x7, 0x28]
|   |:||:   0x06d93b88      str   x9, [arg_150h]
|   |:||:   0x06d93b8c      str   x8, [arg_300h]
|   |:||:   0x06d93b90      mov   x0, xzr
|  ,======< 0x06d93b94      b     0x6d93ba8
|  ||:||:   ; CODE XREF from fcn.06d934f8 @ 0x6d93c9c
| .-------> 0x06d93b98      ldr   x4, [arg_148h]                       ; [0x148:4]=0x458
| :||:||:   0x06d93b9c      add   x0, x4, 1
| :||:||:   0x06d93ba0      ldr   x8, [arg_300h]                       ; [0x300:4]=0x50d0
| :||:||:   0x06d93ba4      ldr   x9, [arg_150h]                       ; [0x150:4]=32
| :||:||:   ; CODE XREF from fcn.06d934f8 @ 0x6d93b94
| :`------> 0x06d93ba8      cmp   x0, x9
| : |:||`=< 0x06d93bac      b.ge  0x6d93b60
| : |:||    0x06d93bb0      str   x0, [arg_148h]
| : |:||    0x06d93bb4      ldr   x4, [x8, x0, lsl 3]
| : |:||    0x06d93bb8      str   x4, [arg_180h]
| : |:||    0x06d93bbc      adrp  x27, 0x948b000
| : |:||    0x06d93bc0      ldr   x26, [x27, 0x830]                    ; [0x830:4]=0x58585858
| : |:||    0x06d93bc4      ldr   x5, [x26]
| : |:||    0x06d93bc8      ldp   x0, x1, [x4, 0x58]
| : |:||    0x06d93bcc      ldp   x2, x3, [x4, 8]
| : |:||    0x06d93bd0      blr   x5
| : |:||    0x06d93bd4      ldr   x4, [arg_258h]                       ; [0x258:4]=0xeb030
| : |:||    0x06d93bd8      orr   x5, xzr, 7
| : |:||    0x06d93bdc      str   w5, [arg_378h]
| : |:||    0x06d93be0      add   x27, arg_380h
| : |:||    0x06d93be4      stp   x0, x1, [x27]
| : |:||    0x06d93be8      adrp  x0, 0x8de9000
| : |:||    0x06d93bec      add   x0, x0, 0xc48
| : |:||    0x06d93bf0      mov   x1, x4
| : |:||    0x06d93bf4      add   x2, arg_378h
| : |:||    0x06d93bf8      bl    0x4d16af0
| ========< 0x06d93bfc      tbz   w1, 0, 0x6d93b98
| : |:||    0x06d93c00      ldr   x5, [arg_188h]                       ; [0x188:4]=1
| : |:||    0x06d93c04      ldr   x6, [x5, 8]                          ; [0x8:4]=0
| : |:||    0x06d93c08      ldr   x2, [x6, 0x70]                       ; [0x70:4]=8
| : |:||    0x06d93c0c      ldp   x0, x7, [x6, 0x60]
| : |:||    0x06d93c10      add   x1, x7, 1
| : |:||    0x06d93c14      ldr   x7, [arg_180h]                       ; [0x180:4]=0x16236a0 "ki/codeium_common_pb/codeium_common_go_proto.SearchResultType.String"
| : |:||    0x06d93c18      ldp   x7, x8, [x7, 8]
| : |:||    0x06d93c1c      cmp   x2, x1
| : |:||,=< 0x06d93c20      b.hs  0x6d93c70
| : |:|||   0x06d93c24      str   x8, [arg_140h]
| : |:|||   0x06d93c28      str   x7, [arg_338h]
| : |:|||   0x06d93c2c      str   x6, [arg_2e8h]
| : |:|||   0x06d93c30      orr   x3, xzr, 1
| : |:|||   0x06d93c34      adrp  x4, 0x856f000
| : |:|||   0x06d93c38      add   x4, x4, 0x550
| : |:|||   0x06d93c3c      bl    0x4d9e050
| : |:|||   0x06d93c40      ldr   x5, [arg_2e8h]                       ; [0x2e8:4]=0xacf0000
| : |:|||   0x06d93c44      str   x2, [x5, 0x70]
| : |:|||   0x06d93c48      adrp  x27, 0xb48f000
| : |:|||   0x06d93c4c      ldr   w6, [x27, 0x450]                     ; [0x450:4]=0x58585858
| :,======< 0x06d93c50      cbz   w6, 0x6d93c60
| :||:|||   0x06d93c54      ldr   x4, [x5, 0x60]                       ; [0x60:4]=0x380
| :||:|||   0x06d93c58      bl    0x4da3500
| :||:|||   0x06d93c5c      stp   x0, x4, [x25]
| :`------> 0x06d93c60      str   x0, [x5, 0x60]
| : |:|||   0x06d93c64      mov   x6, x5
| : |:|||   0x06d93c68      ldr   x7, [arg_338h]                       ; [0x338:4]=27
| : |:|||   0x06d93c6c      ldr   x8, [arg_140h]                       ; [0x140:4]=144
| : |:||`-> 0x06d93c70      str   x1, [x6, 0x68]
| : |:||    0x06d93c74      sub   x4, x1, 1
| : |:||    0x06d93c78      add   x4, x0, x4, lsl 4
| : |:||    0x06d93c7c      str   x8, [x4, 8]
| : |:||    0x06d93c80      adrp  x27, 0xb48f000
| : |:||    0x06d93c84      ldr   w6, [x27, 0x450]                     ; [0x450:4]=0x58585858
| : |:||,=< 0x06d93c88      cbz   w6, 0x6d93c98
| : |:|||   0x06d93c8c      ldr   x6, [x4]
| : |:|||   0x06d93c90      bl    0x4da3500
| : |:|||   0x06d93c94      stp   x7, x6, [x25]
| : |:||`-> 0x06d93c98      str   x7, [x4]
| `=======< 0x06d93c9c      b     0x6d93b98
|   `-----> 0x06d93ca0      adrp  x27, 0x948b000
|    :||    0x06d93ca4      ldr   x0, [x27, 0xa18]                     ; [0xa18:4]=0x58585858
|    :||    0x06d93ca8      nop
|    :||    0x06d93cac      adrp  x1, 0x79e9000
|    :||    0x06d93cb0      add   x1, x1, 0xc80
|    :||    0x06d93cb4      ldr   x2, [arg_500h]                       ; [0x500:4]=0x58585858
|    :||    0x06d93cb8      ldr   x3, [arg_4d0h]                       ; [0x4d0:4]=0x58585858
|    :||    0x06d93cbc      bl    0x6786750
|    :||    0x06d93cc0      adrp  x0, 0x89d0000
|    :||    0x06d93cc4      add   x0, x0, 0xb30
|    :||    0x06d93cc8      bl    0x4d31c10
|    :||    0x06d93ccc      ldr   x4, [arg_4c8h]                       ; [0x4c8:4]=0x58585858
|    :||    0x06d93cd0      ldr   x4, [x4]
|    :||    0x06d93cd4      add   x1, x0, 0x10
|    :||    0x06d93cd8      adrp  x27, 0xb48f000
|    :||    0x06d93cdc      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d93ce0      cbz   w5, 0x6d93d18
|    :|||   0x06d93ce4      str   x0, [arg_1b0h]
|    :|||   0x06d93ce8      str   x4, [arg_2f8h]
|    :|||   0x06d93cec      str   x1, [arg_2f0h]
|    :|||   0x06d93cf0      bl    0x4da3500
|    :|||   0x06d93cf4      ldr   x3, [arg_278h]                       ; [0x278:4]=0x9019dd0
|    :|||   0x06d93cf8      stp   x4, x3, [x25]
|    :|||   0x06d93cfc      adrp  x0, 0x8dec000
|    :|||   0x06d93d00      add   x0, x0, 0x128
|    :|||   0x06d93d04      add   x2, arg_1b8h
|    :|||   0x06d93d08      bl    0x4d35930
|    :|||   0x06d93d0c      ldr   x0, [arg_1b0h]                       ; [0x1b0:4]=0x220
|    :|||   0x06d93d10      ldr   x1, [arg_2f0h]                       ; [0x2f0:4]=0xd8f0000 segment.LOAD6
|    :|||   0x06d93d14      ldr   x4, [arg_2f8h]                       ; [0x2f8:4]=0xd8f0000 segment.LOAD6
|    :||`-> 0x06d93d18      ldr   x5, [arg_278h]                       ; [0x278:4]=0x9019dd0
|    :||    0x06d93d1c      stp   x4, x5, [x0]
|    :||    0x06d93d20      add   x4, arg_1b8h
|    :||    0x06d93d24      orr   x24, xzr, 3
|    :||.-> 0x06d93d28      ldp   q16, q17, [x4], 0x20
|    :||:   0x06d93d2c      stp   q16, q17, [x1], 0x20
|    :||:   0x06d93d30      ldp   q16, q17, [x4], 0x20
|    :||:   0x06d93d34      stp   q16, q17, [x1], 0x20
|    :||:   0x06d93d38      sub   x24, x24, 1
|    :||`=< 0x06d93d3c      cbnz  x24, 0x6d93d28
|    :||    0x06d93d40      adrp  x27, 0x948b000
|    :||    0x06d93d44      ldr   x4, [x27, 0xa20]                     ; [0xa20:4]=0x58585858
|    :||    0x06d93d48      adrp  x1, 0x79e9000
|    :||    0x06d93d4c      add   x1, x1, 0xc20
|    :||    0x06d93d50      ldr   x2, [arg_500h]                       ; [0x500:4]=0x58585858
|    :||    0x06d93d54      mov   x3, x0
|    :||    0x06d93d58      mov   x0, x4
|    :||    0x06d93d5c      bl    0x6786750
|    :||    0x06d93d60      mov   x0, xzr
|    :||    0x06d93d64      mov   x1, xzr
|    :||    0x06d93d68      ldp   fp, lr, [sp, -8]
|    :||    0x06d93d6c      add   sp, sp, 0x4e0
|    :||    0x06d93d70      ret
|    :||    ; CODE XREFS from fcn.06d934f8 @ 0x6d93dbc, 0x6d93dc4
|   .--`.-> 0x06d93d74      ldr   x10, [arg_4e8h]                      ; [0x4e8:4]=0x58585858
|   ::| :   0x06d93d78      mov   x9, x0
|   ::| :   0x06d93d7c      mov   x11, x1
|   :`====< 0x06d93d80      b     0x6d93598
|   : | :   ; CODE XREF from fcn.06d934f8 @ 0x6d93560
|   : `.--> 0x06d93d84      and   x10, x9, x8
|   :  ::   0x06d93d88      lsl   x10, x10, 4
|   :  ::   0x06d93d8c      add   x10, x10, 8
|   :  ::   0x06d93d90      ldr   x11, [x7, x10]                       ; arg8
|   :  ::   0x06d93d94      add   x10, x7, x10                         ; arg8
|   :  ::   0x06d93d98      cmp   x11, x5
|   : ,===< 0x06d93d9c      b.eq  0x6d93dc0
|   : |::   0x06d93da0      add   x9, x9, 1
|   : |`==< 0x06d93da4      cbnz  x11, 0x6d93d84
|   : | :   0x06d93da8      str   x1, [arg_190h]
|   : | :   0x06d93dac      mov   x0, x6
|   : | :   0x06d93db0      mov   x1, x5
|   : | :   0x06d93db4      bl    0x4d2e4c0
|   : | :   0x06d93db8      ldr   x1, [arg_190h]                       ; [0x190:4]=2
|   `=====< 0x06d93dbc      b     0x6d93d74
|     `---> 0x06d93dc0      ldr   x0, [x10, 8]                         ; [0x8:4]=0
\       `=< 0x06d93dc4      b     0x6d93d74
==== google3_third_party_jetski_gateway_gateway.LoadDefaultEffectiveSettings
/ fcn.06d934f8(int64_t arg6, int64_t arg7, int64_t arg8, int64_t arg_8h, int64_t arg_118h, int64_t arg_120h, int64_t arg_128h, int64_t arg_130h, int64_t arg_138h, int64_t arg_140h, int64_t arg_148h, int64_t arg_150h, int64_t arg_158h, int64_t arg_160h, int64_t arg_168h, int64_t arg_180h, int64_t arg_188h, int64_t arg_190h, int64_t arg_198h, int64_t arg_1a0h, int64_t arg_1a8h, int64_t arg_1b0h, int64_t arg_1b8h, int64_t arg_258h, int64_t arg_278h, int64_t arg_280h, int64_t arg_2d8h, int64_t arg_2e0h, int64_t arg_2e8h, int64_t arg_2f0h, int64_t arg_2f8h, int64_t arg_300h, int64_t arg_308h, int64_t arg_310h, int64_t arg_318h, int64_t arg_320h, int64_t arg_328h, int64_t arg_330h, int64_t arg_338h, int64_t arg_368h, int64_t arg_378h, int64_t arg_380h, int64_t arg_390h, int64_t arg_3e8h, int64_t arg_3f8h, int64_t arg_408h, int64_t arg_450h, int64_t arg_4c0h, int64_t arg_4c8h, int64_t arg_4d0h, int64_t arg_4e8h, int64_t arg_4f0h, int64_t arg_4f8h, int64_t arg_500h, int64_t arg_508h, int64_t arg_518h, int64_t arg_520h, int64_t arg_9f8h);
|           ; arg int64_t arg6 @ x5
|           ; arg int64_t arg7 @ x6
|           ; arg int64_t arg8 @ x7
|           ; arg int64_t arg_8h @ stack + 0x8
|           ; arg int64_t arg_118h @ stack + 0x118
|           ; arg int64_t arg_120h @ stack + 0x120
|           ; arg int64_t arg_128h @ stack + 0x128
|           ; arg int64_t arg_130h @ stack + 0x130
|           ; arg int64_t arg_138h @ stack + 0x138
|           ; arg int64_t arg_140h @ stack + 0x140
|           ; arg int64_t arg_148h @ stack + 0x148
|           ; arg int64_t arg_150h @ stack + 0x150
|           ; arg int64_t arg_158h @ stack + 0x158
|           ; arg int64_t arg_160h @ stack + 0x160
|           ; arg int64_t arg_168h @ stack + 0x168
|           ; arg int64_t arg_180h @ stack + 0x180
|           ; arg int64_t arg_188h @ stack + 0x188
|           ; arg int64_t arg_190h @ stack + 0x190
|           ; arg int64_t arg_198h @ stack + 0x198
|           ; arg int64_t arg_1a0h @ stack + 0x1a0
|           ; arg int64_t arg_1a8h @ stack + 0x1a8
|           ; arg int64_t arg_1b0h @ stack + 0x1b0
|           ; arg int64_t arg_1b8h @ stack + 0x1b8
|           ; arg int64_t arg_258h @ stack + 0x258
|           ; arg int64_t arg_278h @ stack + 0x278
|           ; arg int64_t arg_280h @ stack + 0x280
|           ; arg int64_t arg_2d8h @ stack + 0x2d8
|           ; arg int64_t arg_2e0h @ stack + 0x2e0
|           ; arg int64_t arg_2e8h @ stack + 0x2e8
|           ; arg int64_t arg_2f0h @ stack + 0x2f0
|           ; arg int64_t arg_2f8h @ stack + 0x2f8
|           ; arg int64_t arg_300h @ stack + 0x300
|           ; arg int64_t arg_308h @ stack + 0x308
|           ; arg int64_t arg_310h @ stack + 0x310
|           ; arg int64_t arg_318h @ stack + 0x318
|           ; arg int64_t arg_320h @ stack + 0x320
|           ; arg int64_t arg_328h @ stack + 0x328
|           ; arg int64_t arg_330h @ stack + 0x330
|           ; arg int64_t arg_338h @ stack + 0x338
|           ; arg int64_t arg_368h @ stack + 0x368
|           ; arg int64_t arg_378h @ stack + 0x378
|           ; arg int64_t arg_380h @ stack + 0x380
|           ; arg int64_t arg_390h @ stack + 0x390
|           ; arg int64_t arg_3e8h @ stack + 0x3e8
|           ; arg int64_t arg_3f8h @ stack + 0x3f8
|           ; arg int64_t arg_408h @ stack + 0x408
|           ; arg int64_t arg_450h @ stack + 0x450
|           ; arg int64_t arg_4c0h @ stack + 0x4c0
|           ; arg int64_t arg_4c8h @ stack + 0x4c8
|           ; arg int64_t arg_4d0h @ stack + 0x4d0
|           ; arg int64_t arg_4e8h @ stack + 0x4e8
|           ; arg int64_t arg_4f0h @ stack + 0x4f0
|           ; arg int64_t arg_4f8h @ stack + 0x4f8
|           ; arg int64_t arg_500h @ stack + 0x500
|           ; arg int64_t arg_508h @ stack + 0x508
|           ; arg int64_t arg_518h @ stack + 0x518
|           ; arg int64_t arg_520h @ stack + 0x520
|           ; arg int64_t arg_9f8h @ stack + 0x9f8
|           0x06d934f8      ldrsw x9, [x10]
|       ,=< 0x06d934fc      b     0x6d9350c
..
|      ||   ; CODE XREF from fcn.06d934f8 @ 0x6d934fc
|      ``-> 0x06d9350c      str   x5, [arg_130h]                       ; arg6
|           0x06d93510      str   x7, [arg_2d8h]                       ; arg8
|           0x06d93514      str   x6, [arg_138h]                       ; arg7
|           0x06d93518      str   x8, [arg_2e0h]
|           0x06d9351c      ldr   x10, [arg_4e8h]                      ; [0x4e8:4]=0x58585858
|           0x06d93520      ldr   x11, [x10]
|           0x06d93524      ldp   x11, x12, [x11, 0x108]
|       ,=< 0x06d93528      cbz   x11, 0x6d93564
|       |   0x06d9352c      ldr   x4, [x11, 0x18]                      ; [0x18:4]=0x4d0f000 section..text
|       |   0x06d93530      mov   x0, x12
|       |   0x06d93534      ldr   x1, [arg_4f0h]                       ; [0x4f0:4]=0x58585858
|       |   0x06d93538      ldr   x2, [arg_4f8h]                       ; [0x4f8:4]=0x58585858
|       |   0x06d9353c      mov   x3, x9
|       |   0x06d93540      blr   x4
|      ,==< 0x06d93544      cbz   x0, 0x6d93d74
|      ||   0x06d93548      ldr   x5, [x0, 8]                          ; [0x8:4]=0
|      ||   0x06d9354c      adrp  x6, 0x94a8000
|      ||   0x06d93550      add   x6, x6, 0x480
|      ||   0x06d93554      ldar  x7, [x6]
|      ||   0x06d93558      ldr   x8, [x7]                             ; arg8
|      ||   0x06d9355c      ldr   w9, [x0, 0x10]                       ; [0x10:4]=0xb70003
|     ,===< 0x06d93560      b     0x6d93d84
|     ||`-> 0x06d93564      mov   x9, xzr
|     ||    0x06d93568      mov   x11, xzr
|     ||,=< 0x06d9356c      b     0x6d93598
..
|     |||   ; CODE XREFS from fcn.06d934f8 @ 0x6d9356c, 0x6d93d80
|    .--`-> 0x06d93598      str   x11, [arg_190h]
|    :||    0x06d9359c      str   x9, [arg_118h]
|    :||    0x06d935a0      ldr   x12, [x10]
|    :||    0x06d935a4      ldrsb x27, [x12]
|    :||    0x06d935a8      add   x13, x12, 0xb0
|    :||    0x06d935ac      ldr   x12, [x12, 0xb8]                     ; [0xb8:4]=0
|    :||    0x06d935b0      add   x14, arg_280h
|    :||    0x06d935b4      ldp   q16, q17, [x13]
|    :||    0x06d935b8      stp   q16, q17, [x14]
|    :||    0x06d935bc      ldp   q16, q17, [x13, 0x20]
|    :||    0x06d935c0      stp   q16, q17, [x14, 0x20]
|    :||    0x06d935c4      ldr   q16, [x13, 0x40]                     ; segment.PHDR
|    :||                                                               ; [0x40:4]=6 ; "\U00000006"
|    :||    0x06d935c8      str   q16, [x14, 0x40]
|    :||    0x06d935cc      ldr   x25, [x13, 0x50]                     ; [0x50:4]=64
|    :||    0x06d935d0      str   x25, [x14, 0x50]
|    :||,=< 0x06d935d4      cbnz  x12, 0x6d93608
|    :|||   0x06d935d8      orr   x0, xzr, 8
|    :|||   0x06d935dc      adrp  x1, 0x81a4000
|    :|||   0x06d935e0      add   x1, x1, 0x580
|    :|||   0x06d935e4      orr   x2, xzr, 1
|    :|||   0x06d935e8      bl    0x4d322b0
|    :|||   0x06d935ec      adrp  x3, 0x8f91000
|    :|||   0x06d935f0      add   x3, x3, 0x3a8
|    :|||   0x06d935f4      str   x3, [x0]
|    :|||   0x06d935f8      orr   x3, xzr, 1
|    :|||   0x06d935fc      add   x27, arg_280h
|    :|||   0x06d93600      stp   x0, x3, [x27]
|    :|||   0x06d93604      str   x3, [arg_280h + 0x10]
|    :||`-> 0x06d93608      add   x27, arg_280h
|    :||    0x06d9360c      ldp   x12, x13, [x27]
|    :||,=< 0x06d93610      cbnz  x13, 0x6d93624
|    :|||   0x06d93614      mov   x14, xzr
|    :|||   0x06d93618      mov   x15, xzr
|    :|||   0x06d9361c      mov   x16, xzr
|   ,=====< 0x06d93620      b     0x6d9365c
|   |:||`-> 0x06d93624      str   x13, [arg_158h]
|   |:||    0x06d93628      str   x12, [arg_328h]
|   |:||    0x06d9362c      mov   x0, xzr
|   |:||    0x06d93630      mov   x1, x13
|   |:||    0x06d93634      mov   x2, xzr
|   |:||    0x06d93638      mov   x3, x1
|   |:||    0x06d9363c      adrp  x4, 0x86f9000
|   |:||    0x06d93640      add   x4, x4, 0x788
|   |:||    0x06d93644      bl    0x4d9e050
|   |:||    0x06d93648      ldr   x12, [arg_328h]                      ; [0x328:4]=0xd8f5070 segment.INTERP
|   |:||    0x06d9364c      ldr   x13, [arg_158h]                      ; [0x158:4]=0x6474e552
|   |:||    0x06d93650      mov   x14, x0
|   |:||    0x06d93654      mov   x15, x1
|   |:||    0x06d93658      mov   x16, x2
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d93620
|   `-----> 0x06d9365c      stp   x15, x16, [arg_150h]
|    :||    0x06d93660      str   x14, [arg_328h]
|    :||    0x06d93664      adrp  x0, 0x86f9000
|    :||    0x06d93668      add   x0, x0, 0x788
|    :||    0x06d9366c      mov   x1, x14
|    :||    0x06d93670      mov   x2, x15
|    :||    0x06d93674      mov   x3, x12
|    :||    0x06d93678      mov   x4, x13
|    :||    0x06d9367c      bl    0x4d99570
|    :||    0x06d93680      orr   x0, xzr, 0x20
|    :||    0x06d93684      adrp  x1, 0x81a4000
|    :||    0x06d93688      add   x1, x1, 0xa48
|    :||    0x06d9368c      orr   x2, xzr, 1
|    :||    0x06d93690      bl    0x4d32c00
|    :||    0x06d93694      ldr   x5, [arg_160h]                       ; [0x160:4]=0x779b960
|    :||    0x06d93698      str   x5, [x0]
|    :||    0x06d9369c      ldr   x5, [arg_168h]                       ; [0x168:4]=0x799b960 segment.GNU_RELRO
|    :||    0x06d936a0      str   x5, [x0, 0x18]
|    :||    0x06d936a4      adrp  x27, 0xb48f000
|    :||    0x06d936a8      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d936ac      cbnz  w5, 0x6d936bc
|    :|||   0x06d936b0      ldr   x3, [arg_338h]                       ; [0x338:4]=27
|    :|||   0x06d936b4      ldr   x4, [arg_330h]                       ; [0x330:4]=0xd8f5070 segment.INTERP
|   ,=====< 0x06d936b8      b     0x6d936cc
|   |:||`-> 0x06d936bc      bl    0x4da3500
|   |:||    0x06d936c0      ldr   x3, [arg_338h]                       ; [0x338:4]=27
|   |:||    0x06d936c4      ldr   x4, [arg_330h]                       ; [0x330:4]=0xd8f5070 segment.INTERP
|   |:||    0x06d936c8      stp   x3, x4, [x25]
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d936b8
|   `-----> 0x06d936cc      str   x0, [arg_320h]
|    :||    0x06d936d0      stp   x3, x4, [x0, 8]
|    :||    0x06d936d4      orr   x0, xzr, 0x20
|    :||    0x06d936d8      adrp  x1, 0x87f7000
|    :||    0x06d936dc      add   x1, x1, 0xe18
|    :||    0x06d936e0      orr   x2, xzr, 1
|    :||    0x06d936e4      bl    0x4d32c00
|    :||    0x06d936e8      adrp  x3, 0x673f000
|    :||    0x06d936ec      add   x3, x3, 0x490
|    :||    0x06d936f0      str   x3, [x0]
|    :||    0x06d936f4      orr   x3, xzr, 1
|    :||    0x06d936f8      stp   x3, x3, [x0, 0x10]
|    :||    0x06d936fc      adrp  x27, 0xb48f000
|    :||    0x06d93700      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d93704      cbnz  w4, 0x6d93710
|    :|||   0x06d93708      ldr   x5, [arg_320h]                       ; [0x320:4]=0xacf5070
|   ,=====< 0x06d9370c      b     0x6d9371c
|   |:||`-> 0x06d93710      bl    0x4da34f0
|   |:||    0x06d93714      ldr   x5, [arg_320h]                       ; [0x320:4]=0xacf5070
|   |:||    0x06d93718      str   x5, [x25]
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d9370c
|   `-----> 0x06d9371c      str   x5, [x0, 8]
|    :||    0x06d93720      ldr   x5, [arg_150h]                       ; [0x150:4]=32
|    :||    0x06d93724      add   x1, x5, 1
|    :||    0x06d93728      ldr   x2, [arg_158h]                       ; [0x158:4]=0x6474e552
|    :||    0x06d9372c      cmp   x2, x1
|    :||,=< 0x06d93730      b.lo  0x6d9373c
|    :|||   0x06d93734      ldr   x3, [arg_328h]                       ; [0x328:4]=0xd8f5070 segment.INTERP
|   ,=====< 0x06d93738      b     0x6d93758
|   |:||`-> 0x06d9373c      str   x0, [arg_318h]
|   |:||    0x06d93740      ldr   x0, [arg_328h]                       ; [0x328:4]=0xd8f5070 segment.INTERP
|   |:||    0x06d93744      adrp  x4, 0x86f9000
|   |:||    0x06d93748      add   x4, x4, 0x788
|   |:||    0x06d9374c      bl    0x4d9e050
|   |:||    0x06d93750      mov   x3, x0
|   |:||    0x06d93754      ldr   x0, [arg_318h]                       ; [0x318:4]=3
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d93738
|   `-----> 0x06d93758      sub   x4, x1, 1
|    :||    0x06d9375c      adrp  x27, 0xb48f000
|    :||    0x06d93760      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d93764      cbz   w5, 0x6d93774
|    :|||   0x06d93768      ldr   x5, [x3, x4, lsl 3]
|    :|||   0x06d9376c      bl    0x4da3500
|    :|||   0x06d93770      stp   x0, x5, [x25]
|    :||`-> 0x06d93774      str   x0, [x3, x4, lsl 3]
|    :||    0x06d93778      add   x27, arg_280h
|    :||    0x06d9377c      stp   x3, x1, [x27]
|    :||    0x06d93780      str   x2, [arg_280h + 0x10]
|    :||    0x06d93784      add   x3, arg_390h
|    :||    0x06d93788      stp   xzr, xzr, [x3]
|    :||    0x06d9378c      stp   xzr, xzr, [x3, 0x10]
|    :||    0x06d93790      stp   xzr, xzr, [x3, 0x20]
|    :||    0x06d93794      stp   xzr, xzr, [x3, 0x30]
|    :||    0x06d93798      stp   xzr, xzr, [x3, 0x40]
|    :||    0x06d9379c      stp   xzr, xzr, [x3, 0x50]
|    :||    0x06d937a0      stp   xzr, xzr, [x3, 0x60]
|    :||    0x06d937a4      stp   xzr, xzr, [x3, 0x70]
|    :||    0x06d937a8      stp   xzr, xzr, [x3, 0x80]
|    :||    0x06d937ac      stp   xzr, xzr, [x3, 0x90]
|    :||    0x06d937b0      stp   xzr, xzr, [x3, 0xa0]
|    :||    0x06d937b4      str   xzr, [x3, 0xb0]
|    :||    0x06d937b8      add   x4, arg_390h
|    :||    0x06d937bc      add   x5, arg_280h
|    :||    0x06d937c0      ldp   q16, q17, [x5]
|    :||    0x06d937c4      stp   q16, q17, [x4]
|    :||    0x06d937c8      ldp   q16, q17, [x5, 0x20]
|    :||    0x06d937cc      stp   q16, q17, [x4, 0x20]
|    :||    0x06d937d0      ldr   q16, [x5, 0x40]                      ; segment.PHDR
|    :||                                                               ; [0x40:4]=6 ; "\U00000006"
|    :||    0x06d937d4      str   q16, [x4, 0x40]
|    :||    0x06d937d8      ldr   x25, [x5, 0x50]                      ; [0x50:4]=64
|    :||    0x06d937dc      str   x25, [x4, 0x50]
|    :||    0x06d937e0      ldr   x4, [arg_1a0h]                       ; [0x1a0:4]=0x799bde0 segment.DYNAMIC
|    :||    0x06d937e4      ldr   x5, [arg_138h]                       ; [0x138:4]=0x799b960 segment.GNU_RELRO
|    :||    0x06d937e8      add   x27, arg_3f8h
|    :||    0x06d937ec      stp   x4, x5, [x27]
|    :||    0x06d937f0      ldr   x4, [arg_2e0h]                       ; [0x2e0:4]=1
|    :||    0x06d937f4      ldr   x5, [arg_2d8h]                       ; [0x2d8:4]=0x200000
|    :||    0x06d937f8      add   x27, arg_408h
|    :||    0x06d937fc      stp   x4, x5, [x27]
|    :||    0x06d93800      ldr   x4, [arg_130h]                       ; [0x130:4]=0x799b960 segment.GNU_RELRO
|    :||    0x06d93804      str   x4, [arg_408h + 0x10]
|    :||    0x06d93808      ldr   x4, [arg_118h]                       ; [0x118:4]=0x200000
|    :||    0x06d9380c      ldr   x5, [arg_190h]                       ; [0x190:4]=2
|    :||    0x06d93810      add   x27, arg_3e8h
|    :||    0x06d93814      stp   x4, x5, [x27]
|    :||    0x06d93818      ldr   x4, [arg_518h]                       ; [0x518:4]=0x58585858
|    :||    0x06d9381c      str   x4, [arg_408h + 0x18]
|    :||    0x06d93820      ldr   x0, [arg_520h]                       ; [0x520:4]=0x58585858
|    :||    0x06d93824      ldr   x4, [arg_4e8h]                       ; [0x4e8:4]=0x58585858
|    :||    0x06d93828      ldp   x1, x2, [x4, 8]
|    :||    0x06d9382c      add   x4, arg_8h
|    :||    0x06d93830      ldp   q16, q17, [x3]
|    :||    0x06d93834      stp   q16, q17, [x4]
|    :||    0x06d93838      ldp   q16, q17, [x3, 0x20]
|    :||    0x06d9383c      stp   q16, q17, [x4, 0x20]
|    :||    0x06d93840      ldp   q16, q17, [x3, 0x40]
|    :||    0x06d93844      stp   q16, q17, [x4, 0x40]
|    :||    0x06d93848      ldp   q16, q17, [x3, 0x60]
|    :||    0x06d9384c      stp   q16, q17, [x4, 0x60]
|    :||    0x06d93850      ldp   q16, q17, [x3, 0x80]
|    :||    0x06d93854      stp   q16, q17, [x4, 0x80]
|    :||    0x06d93858      ldr   q16, [x3, 0xa0]                      ; [0xa0:4]=0x779b960
|    :||    0x06d9385c      str   q16, [x4, 0xa0]
|    :||    0x06d93860      ldr   x25, [x3, 0xb0]                      ; [0xb0:4]=0x6474e551
|    :||    0x06d93864      str   x25, [x4, 0xb0]
|    :||    0x06d93868      bl    0x672a030
|    :||,=< 0x06d9386c      cbnz  x1, 0x6d93b4c
|    :|||   0x06d93870      str   x0, [arg_278h]
|    :|||   0x06d93874      ldr   x0, [arg_4f0h]                       ; [0x4f0:4]=0x58585858
|    :|||   0x06d93878      ldr   x1, [arg_4f8h]                       ; [0x4f8:4]=0x58585858
|    :|||   0x06d9387c      bl    0x6711700
|    :|||   0x06d93880      str   x0, [arg_128h]
|    :|||   0x06d93884      str   x1, [arg_1a8h]
|    :|||   0x06d93888      orr   x0, xzr, 8
|    :|||   0x06d9388c      adrp  x1, 0x800b000
|    :|||   0x06d93890      add   x1, x1, 0xf98
|    :|||   0x06d93894      orr   x2, xzr, 1
|    :|||   0x06d93898      bl    0x4d322b0
|    :|||   0x06d9389c      str   x0, [arg_4c8h]
|    :|||   0x06d938a0      orr   x0, xzr, 0x18
|    :|||   0x06d938a4      adrp  x1, 0x81b3000
|    :|||   0x06d938a8      add   x1, x1, 0xd48
|    :|||   0x06d938ac      orr   x2, xzr, 1
|    :|||   0x06d938b0      bl    0x4d32830
|    :|||   0x06d938b4      str   x0, [arg_4c0h]
|    :|||   0x06d938b8      adrp  x0, 0x8c17000
|    :|||   0x06d938bc      add   x0, x0, 0x978
|    :|||   0x06d938c0      bl    0x4d31c10
|    :|||   0x06d938c4      nop
|    :|||   0x06d938c8      adr   x2, 0x6d944d0
|    :|||   0x06d938cc      str   x2, [x0]
|    :|||   0x06d938d0      adrp  x27, 0xb48f000
|    :|||   0x06d938d4      ldr   w2, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d938d8      cbnz  w2, 0x6d938e4
|   |:|||   0x06d938dc      ldr   x6, [arg_4c8h]                       ; [0x4c8:4]=0x58585858
|  ,======< 0x06d938e0      b     0x6d938f0
|  |`-----> 0x06d938e4      bl    0x4da34f0
|  | :|||   0x06d938e8      ldr   x6, [arg_4c8h]                       ; [0x4c8:4]=0x58585858
|  | :|||   0x06d938ec      str   x6, [x25]
|  | :|||   ; CODE XREF from fcn.06d934f8 @ 0x6d938e0
|  `------> 0x06d938f0      str   x6, [x0, 8]
|    :|||   0x06d938f4      ldr   x1, [arg_128h]                       ; [0x128:4]=0x779b960
|    :|||   0x06d938f8      str   x1, [x0, 0x10]
|    :|||   0x06d938fc      adrp  x27, 0xb48f000
|    :|||   0x06d93900      ldr   w7, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d93904      cbz   w7, 0x6d93944
|   |:|||   0x06d93908      str   x0, [arg_310h]
|   |:|||   0x06d9390c      ldr   x3, [x0, 0x60]                       ; [0x60:4]=0x380
|   |:|||   0x06d93910      bl    0x4da3520
|   |:|||   0x06d93914      ldr   x4, [arg_1a8h]                       ; [0x1a8:4]=0x799bde0 segment.DYNAMIC
|   |:|||   0x06d93918      ldr   x5, [arg_4d0h]                       ; [0x4d0:4]=0x58585858
|   |:|||   0x06d9391c      stp   x4, x5, [x25]
|   |:|||   0x06d93920      ldr   x4, [arg_4c0h]                       ; [0x4c0:4]=0x58585858
|   |:|||   0x06d93924      stp   x4, x3, [x25, 0x10]
|   |:|||   0x06d93928      add   x1, x0, 0x28
|   |:|||   0x06d9392c      adrp  x0, 0x8b70000
|   |:|||   0x06d93930      add   x0, x0, 0x778
|   |:|||   0x06d93934      add   x2, arg_508h
|   |:|||   0x06d93938      bl    0x4d35930
|   |:|||   0x06d9393c      ldr   x0, [arg_310h]                       ; [0x310:4]=0x10000
|   |:|||   0x06d93940      ldr   x1, [arg_128h]                       ; [0x128:4]=0x779b960
|   `-----> 0x06d93944      ldr   x2, [arg_1a8h]                       ; [0x1a8:4]=0x799bde0 segment.DYNAMIC
|    :|||   0x06d93948      ldr   x6, [arg_4d0h]                       ; [0x4d0:4]=0x58585858
|    :|||   0x06d9394c      stp   x2, x6, [x0, 0x18]
|    :|||   0x06d93950      add   x27, arg_508h
|    :|||   0x06d93954      ldp   q0, q1, [x27]
|    :|||   0x06d93958      add   x27, arg_520h
|    :|||   0x06d9395c      ldp   q2, q3, [x27]
|    :|||   0x06d93960      add   x27, x0, 0x28
|    :|||   0x06d93964      stp   q0, q1, [x27]
|    :|||   0x06d93968      stp   q2, q3, [x0, 0x40]
|    :|||   0x06d9396c      ldr   x6, [arg_4c0h]                       ; [0x4c0:4]=0x58585858
|    :|||   0x06d93970      str   x6, [x0, 0x60]
|    :|||   0x06d93974      adrp  x6, 0x6743000
|    :|||   0x06d93978      add   x6, x6, 0x100
|    :|||   0x06d9397c      add   x27, arg_368h
|    :|||   0x06d93980      stp   x6, x0, [x27]
|    :|||   0x06d93984      add   x6, arg_390h
|    :|||   0x06d93988      orr   x7, xzr, 3
|   .-----> 0x06d9398c      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d93990      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d93994      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d93998      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d9399c      sub   x7, x7, 1
|   `=====< 0x06d939a0      cbnz  x7, 0x6d9398c
|    :|||   0x06d939a4      adrp  x6, 0x8f92000
|    :|||   0x06d939a8      add   x6, x6, 0xc28
|    :|||   0x06d939ac      add   x7, arg_368h
|    :|||   0x06d939b0      add   x27, arg_450h
|    :|||   0x06d939b4      stp   x6, x7, [x27]
|    :|||   0x06d939b8      ldr   x0, [arg_278h]                       ; [0x278:4]=0x9019dd0
|    :|||   0x06d939bc      add   x3, arg_450h
|    :|||   0x06d939c0      orr   x4, xzr, 2
|    :|||   0x06d939c4      mov   x5, x4
|    :|||   0x06d939c8      bl    0x672b7f0
|    :|||   0x06d939cc      add   x6, arg_390h
|    :|||   0x06d939d0      add   x7, arg_8h
|    :|||   0x06d939d4      orr   x24, xzr, 3
|   .-----> 0x06d939d8      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d939dc      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d939e0      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d939e4      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d939e8      sub   x24, x24, 1
|   `=====< 0x06d939ec      cbnz  x24, 0x6d939d8
|    :|||   0x06d939f0      add   x6, arg_1b8h
|    :|||   0x06d939f4      add   x7, arg_390h
|    :|||   0x06d939f8      orr   x24, xzr, 3
|   .-----> 0x06d939fc      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d93a00      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d93a04      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d93a08      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d93a0c      sub   x24, x24, 1
|   `=====< 0x06d93a10      cbnz  x24, 0x6d939fc
|   ,=====< 0x06d93a14      cbz   x0, 0x6d93b20
|   |:|||   0x06d93a18      str   x1, [arg_198h]
|   |:|||   0x06d93a1c      str   x0, [arg_120h]
|   |:|||   0x06d93a20      adrp  x27, 0x9491000
|   |:|||   0x06d93a24      add   x27, x27, 0x210
|   |:|||   0x06d93a28      ldp   x2, x3, [x27]
|   |:|||   0x06d93a2c      bl    0x5116df0
|  ,======< 0x06d93a30      tbz   w0, 0, 0x6d93b38
|  ||:|||   0x06d93a34      ldr   x3, [arg_9f8h]                       ; [0x518:4]=0x58585858
| ,=======< 0x06d93a38      cbz   x3, 0x6d93a44
| |||:|||   0x06d93a3c      ldr   x3, [x3, 8]                          ; [0x8:4]=0
| ========< 0x06d93a40      b     0x6d93a48
| `-------> 0x06d93a44      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a40
| ,=======< 0x06d93a48      cbz   x3, 0x6d93a70
| |||:|||   0x06d93a4c      ldp   x3, x4, [x3, 8]
| |||:|||   0x06d93a50      adrp  x5, 0x8f0d000
| |||:|||   0x06d93a54      add   x5, x5, 0x9c8
| |||:|||   0x06d93a58      cmp   x3, x5
| ========< 0x06d93a5c      b.eq  0x6d93a64
| |||:|||   0x06d93a60      mov   x4, xzr
| --------> 0x06d93a64      b.ne  0x6d93a70
| |||:|||   0x06d93a68      ldr   x3, [x4]
| ========< 0x06d93a6c      b     0x6d93a74
| `-------> 0x06d93a70      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a6c
| ,=======< 0x06d93a74      cbz   x3, 0x6d93a80
| |||:|||   0x06d93a78      ldr   x3, [x3, 0xe0]                       ; [0xe0:4]=0
| ========< 0x06d93a7c      b     0x6d93a84
| `-------> 0x06d93a80      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a7c
| ,=======< 0x06d93a84      cbz   x3, 0x6d93a90
| |||:|||   0x06d93a88      ldrsw x3, [x3, 8]                          ; [0x8:4]=0
| ========< 0x06d93a8c      b     0x6d93a94
| `-------> 0x06d93a90      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a8c
| --------> 0x06d93a94      cmp   w3, 1
| ,=======< 0x06d93a98      b.ne  0x6d93b38
| |||:|||   0x06d93a9c      orr   x0, xzr, 0x10
| |||:|||   0x06d93aa0      adrp  x1, 0x81a5000
| |||:|||   0x06d93aa4      add   x1, x1, 0x4f8
| |||:|||   0x06d93aa8      orr   x2, xzr, 1
| |||:|||   0x06d93aac      bl    0x4d32500
| |||:|||   0x06d93ab0      ldr   x1, [arg_120h]                       ; [0x120:4]=7
| ========< 0x06d93ab4      cbz   x1, 0x6d93ac0
| |||:|||   0x06d93ab8      ldr   x11, [x1, 8]                         ; [0x8:4]=0
| ========< 0x06d93abc      b     0x6d93ac4
| --------> 0x06d93ac0      mov   x11, x1
| |||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93abc
| --------> 0x06d93ac4      str   x11, [x0]
| |||:|||   0x06d93ac8      adrp  x27, 0xb48f000
| |||:|||   0x06d93acc      ldr   w11, [x27, 0x450]                    ; [0x450:4]=0x58585858
| ========< 0x06d93ad0      cbnz  w11, 0x6d93adc
| |||:|||   0x06d93ad4      ldr   x11, [arg_198h]                      ; [0x198:4]=0x779bde0
| ========< 0x06d93ad8      b     0x6d93ae8
| --------> 0x06d93adc      bl    0x4da34f0
| |||:|||   0x06d93ae0      ldr   x11, [arg_198h]                      ; [0x198:4]=0x779bde0
| |||:|||   0x06d93ae4      str   x11, [x25]
| |||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93ad8
| --------> 0x06d93ae8      str   x11, [x0, 8]
| |||:|||   0x06d93aec      ldr   x1, [arg_4f8h]                       ; [0x4f8:4]=0x58585858
| |||:|||   0x06d93af0      orr   x2, xzr, 1
| |||:|||   0x06d93af4      orr   x3, xzr, 1
| |||:|||   0x06d93af8      mov   x4, xzr
| |||:|||   0x06d93afc      mov   x5, xzr
| |||:|||   0x06d93b00      adrp  x6, 0x4479000
| |||:|||   0x06d93b04      add   x6, x6, 0x8ad
| |||:|||   0x06d93b08      orr   x7, xzr, 0x3c
| |||:|||   0x06d93b0c      mov   x8, x0
| |||:|||   0x06d93b10      mov   x9, x2
| |||:|||   0x06d93b14      mov   x10, x2
| |||:|||   0x06d93b18      ldr   x0, [arg_4f0h]                       ; [0x4f0:4]=0x58585858
| |||:|||   0x06d93b1c      bl    0x4f6f980
| ||`-----> 0x06d93b20      ldr   x4, [arg_4c0h]                       ; [0x4c0:4]=0x58585858
| || :|||   0x06d93b24      ldp   x4, x5, [x4]
| || :|||   0x06d93b28      str   x5, [arg_168h]
| || :|||   0x06d93b2c      str   x4, [arg_308h]
| || :|||   0x06d93b30      mov   x6, xzr
| ||,=====< 0x06d93b34      b     0x6d93b70
| ``------> 0x06d93b38      ldr   x0, [arg_120h]                       ; [0x120:4]=7
|   |:|||   0x06d93b3c      ldr   x1, [arg_198h]                       ; [0x198:4]=0x779bde0
|   |:|||   0x06d93b40      ldp   fp, lr, [sp, -8]
|   |:|||   0x06d93b44      add   sp, sp, 0x4e0
|   |:|||   0x06d93b48      ret
|   |:||`-> 0x06d93b4c      mov   x0, x1
|   |:||    0x06d93b50      mov   x1, x2
|   |:||    0x06d93b54      ldp   fp, lr, [sp, -8]
|   |:||    0x06d93b58      add   sp, sp, 0x4e0
|   |:||    0x06d93b5c      ret
|   |:||.-> 0x06d93b60      ldr   x7, [arg_158h]                       ; [0x158:4]=0x6474e552
|   |:||:   0x06d93b64      add   x6, x7, 1
|   |:||:   0x06d93b68      ldr   x4, [arg_308h]                       ; [0x308:4]=0x50d0
|   |:||:   0x06d93b6c      ldr   x5, [arg_168h]                       ; [0x168:4]=0x799b960 segment.GNU_RELRO
|   |:||:   ; CODE XREF from fcn.06d934f8 @ 0x6d93b34
|   `-----> 0x06d93b70      cmp   x6, x5
|   ,=====< 0x06d93b74      b.ge  0x6d93ca0
|   |:||:   0x06d93b78      str   x6, [arg_158h]
|   |:||:   0x06d93b7c      ldr   x7, [x4, x6, lsl 3]
|   |:||:   0x06d93b80      str   x7, [arg_188h]
|   |:||:   0x06d93b84      ldp   x8, x9, [x7, 0x28]
|   |:||:   0x06d93b88      str   x9, [arg_150h]
|   |:||:   0x06d93b8c      str   x8, [arg_300h]
|   |:||:   0x06d93b90      mov   x0, xzr
|  ,======< 0x06d93b94      b     0x6d93ba8
|  ||:||:   ; CODE XREF from fcn.06d934f8 @ 0x6d93c9c
| .-------> 0x06d93b98      ldr   x4, [arg_148h]                       ; [0x148:4]=0x458
| :||:||:   0x06d93b9c      add   x0, x4, 1
| :||:||:   0x06d93ba0      ldr   x8, [arg_300h]                       ; [0x300:4]=0x50d0
| :||:||:   0x06d93ba4      ldr   x9, [arg_150h]                       ; [0x150:4]=32
| :||:||:   ; CODE XREF from fcn.06d934f8 @ 0x6d93b94
| :`------> 0x06d93ba8      cmp   x0, x9
| : |:||`=< 0x06d93bac      b.ge  0x6d93b60
| : |:||    0x06d93bb0      str   x0, [arg_148h]
| : |:||    0x06d93bb4      ldr   x4, [x8, x0, lsl 3]
| : |:||    0x06d93bb8      str   x4, [arg_180h]
| : |:||    0x06d93bbc      adrp  x27, 0x948b000
| : |:||    0x06d93bc0      ldr   x26, [x27, 0x830]                    ; [0x830:4]=0x58585858
| : |:||    0x06d93bc4      ldr   x5, [x26]
| : |:||    0x06d93bc8      ldp   x0, x1, [x4, 0x58]
| : |:||    0x06d93bcc      ldp   x2, x3, [x4, 8]
| : |:||    0x06d93bd0      blr   x5
| : |:||    0x06d93bd4      ldr   x4, [arg_258h]                       ; [0x258:4]=0xeb030
| : |:||    0x06d93bd8      orr   x5, xzr, 7
| : |:||    0x06d93bdc      str   w5, [arg_378h]
| : |:||    0x06d93be0      add   x27, arg_380h
| : |:||    0x06d93be4      stp   x0, x1, [x27]
| : |:||    0x06d93be8      adrp  x0, 0x8de9000
| : |:||    0x06d93bec      add   x0, x0, 0xc48
| : |:||    0x06d93bf0      mov   x1, x4
| : |:||    0x06d93bf4      add   x2, arg_378h
| : |:||    0x06d93bf8      bl    0x4d16af0
| ========< 0x06d93bfc      tbz   w1, 0, 0x6d93b98
| : |:||    0x06d93c00      ldr   x5, [arg_188h]                       ; [0x188:4]=1
| : |:||    0x06d93c04      ldr   x6, [x5, 8]                          ; [0x8:4]=0
| : |:||    0x06d93c08      ldr   x2, [x6, 0x70]                       ; [0x70:4]=8
| : |:||    0x06d93c0c      ldp   x0, x7, [x6, 0x60]
| : |:||    0x06d93c10      add   x1, x7, 1
| : |:||    0x06d93c14      ldr   x7, [arg_180h]                       ; [0x180:4]=0x16236a0 "ki/codeium_common_pb/codeium_common_go_proto.SearchResultType.String"
| : |:||    0x06d93c18      ldp   x7, x8, [x7, 8]
| : |:||    0x06d93c1c      cmp   x2, x1
| : |:||,=< 0x06d93c20      b.hs  0x6d93c70
| : |:|||   0x06d93c24      str   x8, [arg_140h]
| : |:|||   0x06d93c28      str   x7, [arg_338h]
| : |:|||   0x06d93c2c      str   x6, [arg_2e8h]
| : |:|||   0x06d93c30      orr   x3, xzr, 1
| : |:|||   0x06d93c34      adrp  x4, 0x856f000
| : |:|||   0x06d93c38      add   x4, x4, 0x550
| : |:|||   0x06d93c3c      bl    0x4d9e050
| : |:|||   0x06d93c40      ldr   x5, [arg_2e8h]                       ; [0x2e8:4]=0xacf0000
| : |:|||   0x06d93c44      str   x2, [x5, 0x70]
| : |:|||   0x06d93c48      adrp  x27, 0xb48f000
| : |:|||   0x06d93c4c      ldr   w6, [x27, 0x450]                     ; [0x450:4]=0x58585858
| :,======< 0x06d93c50      cbz   w6, 0x6d93c60
| :||:|||   0x06d93c54      ldr   x4, [x5, 0x60]                       ; [0x60:4]=0x380
| :||:|||   0x06d93c58      bl    0x4da3500
| :||:|||   0x06d93c5c      stp   x0, x4, [x25]
| :`------> 0x06d93c60      str   x0, [x5, 0x60]
| : |:|||   0x06d93c64      mov   x6, x5
| : |:|||   0x06d93c68      ldr   x7, [arg_338h]                       ; [0x338:4]=27
| : |:|||   0x06d93c6c      ldr   x8, [arg_140h]                       ; [0x140:4]=144
| : |:||`-> 0x06d93c70      str   x1, [x6, 0x68]
| : |:||    0x06d93c74      sub   x4, x1, 1
| : |:||    0x06d93c78      add   x4, x0, x4, lsl 4
| : |:||    0x06d93c7c      str   x8, [x4, 8]
| : |:||    0x06d93c80      adrp  x27, 0xb48f000
| : |:||    0x06d93c84      ldr   w6, [x27, 0x450]                     ; [0x450:4]=0x58585858
| : |:||,=< 0x06d93c88      cbz   w6, 0x6d93c98
| : |:|||   0x06d93c8c      ldr   x6, [x4]
| : |:|||   0x06d93c90      bl    0x4da3500
| : |:|||   0x06d93c94      stp   x7, x6, [x25]
| : |:||`-> 0x06d93c98      str   x7, [x4]
| `=======< 0x06d93c9c      b     0x6d93b98
|   `-----> 0x06d93ca0      adrp  x27, 0x948b000
|    :||    0x06d93ca4      ldr   x0, [x27, 0xa18]                     ; [0xa18:4]=0x58585858
|    :||    0x06d93ca8      nop
|    :||    0x06d93cac      adrp  x1, 0x79e9000
|    :||    0x06d93cb0      add   x1, x1, 0xc80
|    :||    0x06d93cb4      ldr   x2, [arg_500h]                       ; [0x500:4]=0x58585858
|    :||    0x06d93cb8      ldr   x3, [arg_4d0h]                       ; [0x4d0:4]=0x58585858
|    :||    0x06d93cbc      bl    0x6786750
|    :||    0x06d93cc0      adrp  x0, 0x89d0000
|    :||    0x06d93cc4      add   x0, x0, 0xb30
|    :||    0x06d93cc8      bl    0x4d31c10
|    :||    0x06d93ccc      ldr   x4, [arg_4c8h]                       ; [0x4c8:4]=0x58585858
|    :||    0x06d93cd0      ldr   x4, [x4]
|    :||    0x06d93cd4      add   x1, x0, 0x10
|    :||    0x06d93cd8      adrp  x27, 0xb48f000
|    :||    0x06d93cdc      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d93ce0      cbz   w5, 0x6d93d18
|    :|||   0x06d93ce4      str   x0, [arg_1b0h]
|    :|||   0x06d93ce8      str   x4, [arg_2f8h]
|    :|||   0x06d93cec      str   x1, [arg_2f0h]
|    :|||   0x06d93cf0      bl    0x4da3500
|    :|||   0x06d93cf4      ldr   x3, [arg_278h]                       ; [0x278:4]=0x9019dd0
|    :|||   0x06d93cf8      stp   x4, x3, [x25]
|    :|||   0x06d93cfc      adrp  x0, 0x8dec000
|    :|||   0x06d93d00      add   x0, x0, 0x128
|    :|||   0x06d93d04      add   x2, arg_1b8h
|    :|||   0x06d93d08      bl    0x4d35930
|    :|||   0x06d93d0c      ldr   x0, [arg_1b0h]                       ; [0x1b0:4]=0x220
|    :|||   0x06d93d10      ldr   x1, [arg_2f0h]                       ; [0x2f0:4]=0xd8f0000 segment.LOAD6
|    :|||   0x06d93d14      ldr   x4, [arg_2f8h]                       ; [0x2f8:4]=0xd8f0000 segment.LOAD6
|    :||`-> 0x06d93d18      ldr   x5, [arg_278h]                       ; [0x278:4]=0x9019dd0
|    :||    0x06d93d1c      stp   x4, x5, [x0]
|    :||    0x06d93d20      add   x4, arg_1b8h
|    :||    0x06d93d24      orr   x24, xzr, 3
|    :||.-> 0x06d93d28      ldp   q16, q17, [x4], 0x20
|    :||:   0x06d93d2c      stp   q16, q17, [x1], 0x20
|    :||:   0x06d93d30      ldp   q16, q17, [x4], 0x20
|    :||:   0x06d93d34      stp   q16, q17, [x1], 0x20
|    :||:   0x06d93d38      sub   x24, x24, 1
|    :||`=< 0x06d93d3c      cbnz  x24, 0x6d93d28
|    :||    0x06d93d40      adrp  x27, 0x948b000
|    :||    0x06d93d44      ldr   x4, [x27, 0xa20]                     ; [0xa20:4]=0x58585858
|    :||    0x06d93d48      adrp  x1, 0x79e9000
|    :||    0x06d93d4c      add   x1, x1, 0xc20
|    :||    0x06d93d50      ldr   x2, [arg_500h]                       ; [0x500:4]=0x58585858
|    :||    0x06d93d54      mov   x3, x0
|    :||    0x06d93d58      mov   x0, x4
|    :||    0x06d93d5c      bl    0x6786750
|    :||    0x06d93d60      mov   x0, xzr
|    :||    0x06d93d64      mov   x1, xzr
|    :||    0x06d93d68      ldp   fp, lr, [sp, -8]
|    :||    0x06d93d6c      add   sp, sp, 0x4e0
|    :||    0x06d93d70      ret
|    :||    ; CODE XREFS from fcn.06d934f8 @ 0x6d93dbc, 0x6d93dc4
|   .--`.-> 0x06d93d74      ldr   x10, [arg_4e8h]                      ; [0x4e8:4]=0x58585858
|   ::| :   0x06d93d78      mov   x9, x0
|   ::| :   0x06d93d7c      mov   x11, x1
|   :`====< 0x06d93d80      b     0x6d93598
|   : | :   ; CODE XREF from fcn.06d934f8 @ 0x6d93560
|   : `.--> 0x06d93d84      and   x10, x9, x8
|   :  ::   0x06d93d88      lsl   x10, x10, 4
|   :  ::   0x06d93d8c      add   x10, x10, 8
|   :  ::   0x06d93d90      ldr   x11, [x7, x10]                       ; arg8
|   :  ::   0x06d93d94      add   x10, x7, x10                         ; arg8
|   :  ::   0x06d93d98      cmp   x11, x5
|   : ,===< 0x06d93d9c      b.eq  0x6d93dc0
|   : |::   0x06d93da0      add   x9, x9, 1
|   : |`==< 0x06d93da4      cbnz  x11, 0x6d93d84
|   : | :   0x06d93da8      str   x1, [arg_190h]
|   : | :   0x06d93dac      mov   x0, x6
|   : | :   0x06d93db0      mov   x1, x5
|   : | :   0x06d93db4      bl    0x4d2e4c0
|   : | :   0x06d93db8      ldr   x1, [arg_190h]                       ; [0x190:4]=2
|   `=====< 0x06d93dbc      b     0x6d93d74
|     `---> 0x06d93dc0      ldr   x0, [x10, 8]                         ; [0x8:4]=0
\       `=< 0x06d93dc4      b     0x6d93d74
==== google3_third_party_jetski_gateway_gateway.LoadEffectiveSettings
/ fcn.06d934f8(int64_t arg6, int64_t arg7, int64_t arg8, int64_t arg_8h, int64_t arg_118h, int64_t arg_120h, int64_t arg_128h, int64_t arg_130h, int64_t arg_138h, int64_t arg_140h, int64_t arg_148h, int64_t arg_150h, int64_t arg_158h, int64_t arg_160h, int64_t arg_168h, int64_t arg_180h, int64_t arg_188h, int64_t arg_190h, int64_t arg_198h, int64_t arg_1a0h, int64_t arg_1a8h, int64_t arg_1b0h, int64_t arg_1b8h, int64_t arg_258h, int64_t arg_278h, int64_t arg_280h, int64_t arg_2d8h, int64_t arg_2e0h, int64_t arg_2e8h, int64_t arg_2f0h, int64_t arg_2f8h, int64_t arg_300h, int64_t arg_308h, int64_t arg_310h, int64_t arg_318h, int64_t arg_320h, int64_t arg_328h, int64_t arg_330h, int64_t arg_338h, int64_t arg_368h, int64_t arg_378h, int64_t arg_380h, int64_t arg_390h, int64_t arg_3e8h, int64_t arg_3f8h, int64_t arg_408h, int64_t arg_450h, int64_t arg_4c0h, int64_t arg_4c8h, int64_t arg_4d0h, int64_t arg_4e8h, int64_t arg_4f0h, int64_t arg_4f8h, int64_t arg_500h, int64_t arg_508h, int64_t arg_518h, int64_t arg_520h, int64_t arg_9f8h);
|           ; arg int64_t arg6 @ x5
|           ; arg int64_t arg7 @ x6
|           ; arg int64_t arg8 @ x7
|           ; arg int64_t arg_8h @ stack + 0x8
|           ; arg int64_t arg_118h @ stack + 0x118
|           ; arg int64_t arg_120h @ stack + 0x120
|           ; arg int64_t arg_128h @ stack + 0x128
|           ; arg int64_t arg_130h @ stack + 0x130
|           ; arg int64_t arg_138h @ stack + 0x138
|           ; arg int64_t arg_140h @ stack + 0x140
|           ; arg int64_t arg_148h @ stack + 0x148
|           ; arg int64_t arg_150h @ stack + 0x150
|           ; arg int64_t arg_158h @ stack + 0x158
|           ; arg int64_t arg_160h @ stack + 0x160
|           ; arg int64_t arg_168h @ stack + 0x168
|           ; arg int64_t arg_180h @ stack + 0x180
|           ; arg int64_t arg_188h @ stack + 0x188
|           ; arg int64_t arg_190h @ stack + 0x190
|           ; arg int64_t arg_198h @ stack + 0x198
|           ; arg int64_t arg_1a0h @ stack + 0x1a0
|           ; arg int64_t arg_1a8h @ stack + 0x1a8
|           ; arg int64_t arg_1b0h @ stack + 0x1b0
|           ; arg int64_t arg_1b8h @ stack + 0x1b8
|           ; arg int64_t arg_258h @ stack + 0x258
|           ; arg int64_t arg_278h @ stack + 0x278
|           ; arg int64_t arg_280h @ stack + 0x280
|           ; arg int64_t arg_2d8h @ stack + 0x2d8
|           ; arg int64_t arg_2e0h @ stack + 0x2e0
|           ; arg int64_t arg_2e8h @ stack + 0x2e8
|           ; arg int64_t arg_2f0h @ stack + 0x2f0
|           ; arg int64_t arg_2f8h @ stack + 0x2f8
|           ; arg int64_t arg_300h @ stack + 0x300
|           ; arg int64_t arg_308h @ stack + 0x308
|           ; arg int64_t arg_310h @ stack + 0x310
|           ; arg int64_t arg_318h @ stack + 0x318
|           ; arg int64_t arg_320h @ stack + 0x320
|           ; arg int64_t arg_328h @ stack + 0x328
|           ; arg int64_t arg_330h @ stack + 0x330
|           ; arg int64_t arg_338h @ stack + 0x338
|           ; arg int64_t arg_368h @ stack + 0x368
|           ; arg int64_t arg_378h @ stack + 0x378
|           ; arg int64_t arg_380h @ stack + 0x380
|           ; arg int64_t arg_390h @ stack + 0x390
|           ; arg int64_t arg_3e8h @ stack + 0x3e8
|           ; arg int64_t arg_3f8h @ stack + 0x3f8
|           ; arg int64_t arg_408h @ stack + 0x408
|           ; arg int64_t arg_450h @ stack + 0x450
|           ; arg int64_t arg_4c0h @ stack + 0x4c0
|           ; arg int64_t arg_4c8h @ stack + 0x4c8
|           ; arg int64_t arg_4d0h @ stack + 0x4d0
|           ; arg int64_t arg_4e8h @ stack + 0x4e8
|           ; arg int64_t arg_4f0h @ stack + 0x4f0
|           ; arg int64_t arg_4f8h @ stack + 0x4f8
|           ; arg int64_t arg_500h @ stack + 0x500
|           ; arg int64_t arg_508h @ stack + 0x508
|           ; arg int64_t arg_518h @ stack + 0x518
|           ; arg int64_t arg_520h @ stack + 0x520
|           ; arg int64_t arg_9f8h @ stack + 0x9f8
|           0x06d934f8      ldrsw x9, [x10]
|       ,=< 0x06d934fc      b     0x6d9350c
..
|      ||   ; CODE XREF from fcn.06d934f8 @ 0x6d934fc
|      ``-> 0x06d9350c      str   x5, [arg_130h]                       ; arg6
|           0x06d93510      str   x7, [arg_2d8h]                       ; arg8
|           0x06d93514      str   x6, [arg_138h]                       ; arg7
|           0x06d93518      str   x8, [arg_2e0h]
|           0x06d9351c      ldr   x10, [arg_4e8h]                      ; [0x4e8:4]=0x58585858
|           0x06d93520      ldr   x11, [x10]
|           0x06d93524      ldp   x11, x12, [x11, 0x108]
|       ,=< 0x06d93528      cbz   x11, 0x6d93564
|       |   0x06d9352c      ldr   x4, [x11, 0x18]                      ; [0x18:4]=0x4d0f000 section..text
|       |   0x06d93530      mov   x0, x12
|       |   0x06d93534      ldr   x1, [arg_4f0h]                       ; [0x4f0:4]=0x58585858
|       |   0x06d93538      ldr   x2, [arg_4f8h]                       ; [0x4f8:4]=0x58585858
|       |   0x06d9353c      mov   x3, x9
|       |   0x06d93540      blr   x4
|      ,==< 0x06d93544      cbz   x0, 0x6d93d74
|      ||   0x06d93548      ldr   x5, [x0, 8]                          ; [0x8:4]=0
|      ||   0x06d9354c      adrp  x6, 0x94a8000
|      ||   0x06d93550      add   x6, x6, 0x480
|      ||   0x06d93554      ldar  x7, [x6]
|      ||   0x06d93558      ldr   x8, [x7]                             ; arg8
|      ||   0x06d9355c      ldr   w9, [x0, 0x10]                       ; [0x10:4]=0xb70003
|     ,===< 0x06d93560      b     0x6d93d84
|     ||`-> 0x06d93564      mov   x9, xzr
|     ||    0x06d93568      mov   x11, xzr
|     ||,=< 0x06d9356c      b     0x6d93598
..
|     |||   ; CODE XREFS from fcn.06d934f8 @ 0x6d9356c, 0x6d93d80
|    .--`-> 0x06d93598      str   x11, [arg_190h]
|    :||    0x06d9359c      str   x9, [arg_118h]
|    :||    0x06d935a0      ldr   x12, [x10]
|    :||    0x06d935a4      ldrsb x27, [x12]
|    :||    0x06d935a8      add   x13, x12, 0xb0
|    :||    0x06d935ac      ldr   x12, [x12, 0xb8]                     ; [0xb8:4]=0
|    :||    0x06d935b0      add   x14, arg_280h
|    :||    0x06d935b4      ldp   q16, q17, [x13]
|    :||    0x06d935b8      stp   q16, q17, [x14]
|    :||    0x06d935bc      ldp   q16, q17, [x13, 0x20]
|    :||    0x06d935c0      stp   q16, q17, [x14, 0x20]
|    :||    0x06d935c4      ldr   q16, [x13, 0x40]                     ; segment.PHDR
|    :||                                                               ; [0x40:4]=6 ; "\U00000006"
|    :||    0x06d935c8      str   q16, [x14, 0x40]
|    :||    0x06d935cc      ldr   x25, [x13, 0x50]                     ; [0x50:4]=64
|    :||    0x06d935d0      str   x25, [x14, 0x50]
|    :||,=< 0x06d935d4      cbnz  x12, 0x6d93608
|    :|||   0x06d935d8      orr   x0, xzr, 8
|    :|||   0x06d935dc      adrp  x1, 0x81a4000
|    :|||   0x06d935e0      add   x1, x1, 0x580
|    :|||   0x06d935e4      orr   x2, xzr, 1
|    :|||   0x06d935e8      bl    0x4d322b0
|    :|||   0x06d935ec      adrp  x3, 0x8f91000
|    :|||   0x06d935f0      add   x3, x3, 0x3a8
|    :|||   0x06d935f4      str   x3, [x0]
|    :|||   0x06d935f8      orr   x3, xzr, 1
|    :|||   0x06d935fc      add   x27, arg_280h
|    :|||   0x06d93600      stp   x0, x3, [x27]
|    :|||   0x06d93604      str   x3, [arg_280h + 0x10]
|    :||`-> 0x06d93608      add   x27, arg_280h
|    :||    0x06d9360c      ldp   x12, x13, [x27]
|    :||,=< 0x06d93610      cbnz  x13, 0x6d93624
|    :|||   0x06d93614      mov   x14, xzr
|    :|||   0x06d93618      mov   x15, xzr
|    :|||   0x06d9361c      mov   x16, xzr
|   ,=====< 0x06d93620      b     0x6d9365c
|   |:||`-> 0x06d93624      str   x13, [arg_158h]
|   |:||    0x06d93628      str   x12, [arg_328h]
|   |:||    0x06d9362c      mov   x0, xzr
|   |:||    0x06d93630      mov   x1, x13
|   |:||    0x06d93634      mov   x2, xzr
|   |:||    0x06d93638      mov   x3, x1
|   |:||    0x06d9363c      adrp  x4, 0x86f9000
|   |:||    0x06d93640      add   x4, x4, 0x788
|   |:||    0x06d93644      bl    0x4d9e050
|   |:||    0x06d93648      ldr   x12, [arg_328h]                      ; [0x328:4]=0xd8f5070 segment.INTERP
|   |:||    0x06d9364c      ldr   x13, [arg_158h]                      ; [0x158:4]=0x6474e552
|   |:||    0x06d93650      mov   x14, x0
|   |:||    0x06d93654      mov   x15, x1
|   |:||    0x06d93658      mov   x16, x2
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d93620
|   `-----> 0x06d9365c      stp   x15, x16, [arg_150h]
|    :||    0x06d93660      str   x14, [arg_328h]
|    :||    0x06d93664      adrp  x0, 0x86f9000
|    :||    0x06d93668      add   x0, x0, 0x788
|    :||    0x06d9366c      mov   x1, x14
|    :||    0x06d93670      mov   x2, x15
|    :||    0x06d93674      mov   x3, x12
|    :||    0x06d93678      mov   x4, x13
|    :||    0x06d9367c      bl    0x4d99570
|    :||    0x06d93680      orr   x0, xzr, 0x20
|    :||    0x06d93684      adrp  x1, 0x81a4000
|    :||    0x06d93688      add   x1, x1, 0xa48
|    :||    0x06d9368c      orr   x2, xzr, 1
|    :||    0x06d93690      bl    0x4d32c00
|    :||    0x06d93694      ldr   x5, [arg_160h]                       ; [0x160:4]=0x779b960
|    :||    0x06d93698      str   x5, [x0]
|    :||    0x06d9369c      ldr   x5, [arg_168h]                       ; [0x168:4]=0x799b960 segment.GNU_RELRO
|    :||    0x06d936a0      str   x5, [x0, 0x18]
|    :||    0x06d936a4      adrp  x27, 0xb48f000
|    :||    0x06d936a8      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d936ac      cbnz  w5, 0x6d936bc
|    :|||   0x06d936b0      ldr   x3, [arg_338h]                       ; [0x338:4]=27
|    :|||   0x06d936b4      ldr   x4, [arg_330h]                       ; [0x330:4]=0xd8f5070 segment.INTERP
|   ,=====< 0x06d936b8      b     0x6d936cc
|   |:||`-> 0x06d936bc      bl    0x4da3500
|   |:||    0x06d936c0      ldr   x3, [arg_338h]                       ; [0x338:4]=27
|   |:||    0x06d936c4      ldr   x4, [arg_330h]                       ; [0x330:4]=0xd8f5070 segment.INTERP
|   |:||    0x06d936c8      stp   x3, x4, [x25]
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d936b8
|   `-----> 0x06d936cc      str   x0, [arg_320h]
|    :||    0x06d936d0      stp   x3, x4, [x0, 8]
|    :||    0x06d936d4      orr   x0, xzr, 0x20
|    :||    0x06d936d8      adrp  x1, 0x87f7000
|    :||    0x06d936dc      add   x1, x1, 0xe18
|    :||    0x06d936e0      orr   x2, xzr, 1
|    :||    0x06d936e4      bl    0x4d32c00
|    :||    0x06d936e8      adrp  x3, 0x673f000
|    :||    0x06d936ec      add   x3, x3, 0x490
|    :||    0x06d936f0      str   x3, [x0]
|    :||    0x06d936f4      orr   x3, xzr, 1
|    :||    0x06d936f8      stp   x3, x3, [x0, 0x10]
|    :||    0x06d936fc      adrp  x27, 0xb48f000
|    :||    0x06d93700      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d93704      cbnz  w4, 0x6d93710
|    :|||   0x06d93708      ldr   x5, [arg_320h]                       ; [0x320:4]=0xacf5070
|   ,=====< 0x06d9370c      b     0x6d9371c
|   |:||`-> 0x06d93710      bl    0x4da34f0
|   |:||    0x06d93714      ldr   x5, [arg_320h]                       ; [0x320:4]=0xacf5070
|   |:||    0x06d93718      str   x5, [x25]
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d9370c
|   `-----> 0x06d9371c      str   x5, [x0, 8]
|    :||    0x06d93720      ldr   x5, [arg_150h]                       ; [0x150:4]=32
|    :||    0x06d93724      add   x1, x5, 1
|    :||    0x06d93728      ldr   x2, [arg_158h]                       ; [0x158:4]=0x6474e552
|    :||    0x06d9372c      cmp   x2, x1
|    :||,=< 0x06d93730      b.lo  0x6d9373c
|    :|||   0x06d93734      ldr   x3, [arg_328h]                       ; [0x328:4]=0xd8f5070 segment.INTERP
|   ,=====< 0x06d93738      b     0x6d93758
|   |:||`-> 0x06d9373c      str   x0, [arg_318h]
|   |:||    0x06d93740      ldr   x0, [arg_328h]                       ; [0x328:4]=0xd8f5070 segment.INTERP
|   |:||    0x06d93744      adrp  x4, 0x86f9000
|   |:||    0x06d93748      add   x4, x4, 0x788
|   |:||    0x06d9374c      bl    0x4d9e050
|   |:||    0x06d93750      mov   x3, x0
|   |:||    0x06d93754      ldr   x0, [arg_318h]                       ; [0x318:4]=3
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d93738
|   `-----> 0x06d93758      sub   x4, x1, 1
|    :||    0x06d9375c      adrp  x27, 0xb48f000
|    :||    0x06d93760      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d93764      cbz   w5, 0x6d93774
|    :|||   0x06d93768      ldr   x5, [x3, x4, lsl 3]
|    :|||   0x06d9376c      bl    0x4da3500
|    :|||   0x06d93770      stp   x0, x5, [x25]
|    :||`-> 0x06d93774      str   x0, [x3, x4, lsl 3]
|    :||    0x06d93778      add   x27, arg_280h
|    :||    0x06d9377c      stp   x3, x1, [x27]
|    :||    0x06d93780      str   x2, [arg_280h + 0x10]
|    :||    0x06d93784      add   x3, arg_390h
|    :||    0x06d93788      stp   xzr, xzr, [x3]
|    :||    0x06d9378c      stp   xzr, xzr, [x3, 0x10]
|    :||    0x06d93790      stp   xzr, xzr, [x3, 0x20]
|    :||    0x06d93794      stp   xzr, xzr, [x3, 0x30]
|    :||    0x06d93798      stp   xzr, xzr, [x3, 0x40]
|    :||    0x06d9379c      stp   xzr, xzr, [x3, 0x50]
|    :||    0x06d937a0      stp   xzr, xzr, [x3, 0x60]
|    :||    0x06d937a4      stp   xzr, xzr, [x3, 0x70]
|    :||    0x06d937a8      stp   xzr, xzr, [x3, 0x80]
|    :||    0x06d937ac      stp   xzr, xzr, [x3, 0x90]
|    :||    0x06d937b0      stp   xzr, xzr, [x3, 0xa0]
|    :||    0x06d937b4      str   xzr, [x3, 0xb0]
|    :||    0x06d937b8      add   x4, arg_390h
|    :||    0x06d937bc      add   x5, arg_280h
|    :||    0x06d937c0      ldp   q16, q17, [x5]
|    :||    0x06d937c4      stp   q16, q17, [x4]
|    :||    0x06d937c8      ldp   q16, q17, [x5, 0x20]
|    :||    0x06d937cc      stp   q16, q17, [x4, 0x20]
|    :||    0x06d937d0      ldr   q16, [x5, 0x40]                      ; segment.PHDR
|    :||                                                               ; [0x40:4]=6 ; "\U00000006"
|    :||    0x06d937d4      str   q16, [x4, 0x40]
|    :||    0x06d937d8      ldr   x25, [x5, 0x50]                      ; [0x50:4]=64
|    :||    0x06d937dc      str   x25, [x4, 0x50]
|    :||    0x06d937e0      ldr   x4, [arg_1a0h]                       ; [0x1a0:4]=0x799bde0 segment.DYNAMIC
|    :||    0x06d937e4      ldr   x5, [arg_138h]                       ; [0x138:4]=0x799b960 segment.GNU_RELRO
|    :||    0x06d937e8      add   x27, arg_3f8h
|    :||    0x06d937ec      stp   x4, x5, [x27]
|    :||    0x06d937f0      ldr   x4, [arg_2e0h]                       ; [0x2e0:4]=1
|    :||    0x06d937f4      ldr   x5, [arg_2d8h]                       ; [0x2d8:4]=0x200000
|    :||    0x06d937f8      add   x27, arg_408h
|    :||    0x06d937fc      stp   x4, x5, [x27]
|    :||    0x06d93800      ldr   x4, [arg_130h]                       ; [0x130:4]=0x799b960 segment.GNU_RELRO
|    :||    0x06d93804      str   x4, [arg_408h + 0x10]
|    :||    0x06d93808      ldr   x4, [arg_118h]                       ; [0x118:4]=0x200000
|    :||    0x06d9380c      ldr   x5, [arg_190h]                       ; [0x190:4]=2
|    :||    0x06d93810      add   x27, arg_3e8h
|    :||    0x06d93814      stp   x4, x5, [x27]
|    :||    0x06d93818      ldr   x4, [arg_518h]                       ; [0x518:4]=0x58585858
|    :||    0x06d9381c      str   x4, [arg_408h + 0x18]
|    :||    0x06d93820      ldr   x0, [arg_520h]                       ; [0x520:4]=0x58585858
|    :||    0x06d93824      ldr   x4, [arg_4e8h]                       ; [0x4e8:4]=0x58585858
|    :||    0x06d93828      ldp   x1, x2, [x4, 8]
|    :||    0x06d9382c      add   x4, arg_8h
|    :||    0x06d93830      ldp   q16, q17, [x3]
|    :||    0x06d93834      stp   q16, q17, [x4]
|    :||    0x06d93838      ldp   q16, q17, [x3, 0x20]
|    :||    0x06d9383c      stp   q16, q17, [x4, 0x20]
|    :||    0x06d93840      ldp   q16, q17, [x3, 0x40]
|    :||    0x06d93844      stp   q16, q17, [x4, 0x40]
|    :||    0x06d93848      ldp   q16, q17, [x3, 0x60]
|    :||    0x06d9384c      stp   q16, q17, [x4, 0x60]
|    :||    0x06d93850      ldp   q16, q17, [x3, 0x80]
|    :||    0x06d93854      stp   q16, q17, [x4, 0x80]
|    :||    0x06d93858      ldr   q16, [x3, 0xa0]                      ; [0xa0:4]=0x779b960
|    :||    0x06d9385c      str   q16, [x4, 0xa0]
|    :||    0x06d93860      ldr   x25, [x3, 0xb0]                      ; [0xb0:4]=0x6474e551
|    :||    0x06d93864      str   x25, [x4, 0xb0]
|    :||    0x06d93868      bl    0x672a030
|    :||,=< 0x06d9386c      cbnz  x1, 0x6d93b4c
|    :|||   0x06d93870      str   x0, [arg_278h]
|    :|||   0x06d93874      ldr   x0, [arg_4f0h]                       ; [0x4f0:4]=0x58585858
|    :|||   0x06d93878      ldr   x1, [arg_4f8h]                       ; [0x4f8:4]=0x58585858
|    :|||   0x06d9387c      bl    0x6711700
|    :|||   0x06d93880      str   x0, [arg_128h]
|    :|||   0x06d93884      str   x1, [arg_1a8h]
|    :|||   0x06d93888      orr   x0, xzr, 8
|    :|||   0x06d9388c      adrp  x1, 0x800b000
|    :|||   0x06d93890      add   x1, x1, 0xf98
|    :|||   0x06d93894      orr   x2, xzr, 1
|    :|||   0x06d93898      bl    0x4d322b0
|    :|||   0x06d9389c      str   x0, [arg_4c8h]
|    :|||   0x06d938a0      orr   x0, xzr, 0x18
|    :|||   0x06d938a4      adrp  x1, 0x81b3000
|    :|||   0x06d938a8      add   x1, x1, 0xd48
|    :|||   0x06d938ac      orr   x2, xzr, 1
|    :|||   0x06d938b0      bl    0x4d32830
|    :|||   0x06d938b4      str   x0, [arg_4c0h]
|    :|||   0x06d938b8      adrp  x0, 0x8c17000
|    :|||   0x06d938bc      add   x0, x0, 0x978
|    :|||   0x06d938c0      bl    0x4d31c10
|    :|||   0x06d938c4      nop
|    :|||   0x06d938c8      adr   x2, 0x6d944d0
|    :|||   0x06d938cc      str   x2, [x0]
|    :|||   0x06d938d0      adrp  x27, 0xb48f000
|    :|||   0x06d938d4      ldr   w2, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d938d8      cbnz  w2, 0x6d938e4
|   |:|||   0x06d938dc      ldr   x6, [arg_4c8h]                       ; [0x4c8:4]=0x58585858
|  ,======< 0x06d938e0      b     0x6d938f0
|  |`-----> 0x06d938e4      bl    0x4da34f0
|  | :|||   0x06d938e8      ldr   x6, [arg_4c8h]                       ; [0x4c8:4]=0x58585858
|  | :|||   0x06d938ec      str   x6, [x25]
|  | :|||   ; CODE XREF from fcn.06d934f8 @ 0x6d938e0
|  `------> 0x06d938f0      str   x6, [x0, 8]
|    :|||   0x06d938f4      ldr   x1, [arg_128h]                       ; [0x128:4]=0x779b960
|    :|||   0x06d938f8      str   x1, [x0, 0x10]
|    :|||   0x06d938fc      adrp  x27, 0xb48f000
|    :|||   0x06d93900      ldr   w7, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d93904      cbz   w7, 0x6d93944
|   |:|||   0x06d93908      str   x0, [arg_310h]
|   |:|||   0x06d9390c      ldr   x3, [x0, 0x60]                       ; [0x60:4]=0x380
|   |:|||   0x06d93910      bl    0x4da3520
|   |:|||   0x06d93914      ldr   x4, [arg_1a8h]                       ; [0x1a8:4]=0x799bde0 segment.DYNAMIC
|   |:|||   0x06d93918      ldr   x5, [arg_4d0h]                       ; [0x4d0:4]=0x58585858
|   |:|||   0x06d9391c      stp   x4, x5, [x25]
|   |:|||   0x06d93920      ldr   x4, [arg_4c0h]                       ; [0x4c0:4]=0x58585858
|   |:|||   0x06d93924      stp   x4, x3, [x25, 0x10]
|   |:|||   0x06d93928      add   x1, x0, 0x28
|   |:|||   0x06d9392c      adrp  x0, 0x8b70000
|   |:|||   0x06d93930      add   x0, x0, 0x778
|   |:|||   0x06d93934      add   x2, arg_508h
|   |:|||   0x06d93938      bl    0x4d35930
|   |:|||   0x06d9393c      ldr   x0, [arg_310h]                       ; [0x310:4]=0x10000
|   |:|||   0x06d93940      ldr   x1, [arg_128h]                       ; [0x128:4]=0x779b960
|   `-----> 0x06d93944      ldr   x2, [arg_1a8h]                       ; [0x1a8:4]=0x799bde0 segment.DYNAMIC
|    :|||   0x06d93948      ldr   x6, [arg_4d0h]                       ; [0x4d0:4]=0x58585858
|    :|||   0x06d9394c      stp   x2, x6, [x0, 0x18]
|    :|||   0x06d93950      add   x27, arg_508h
|    :|||   0x06d93954      ldp   q0, q1, [x27]
|    :|||   0x06d93958      add   x27, arg_520h
|    :|||   0x06d9395c      ldp   q2, q3, [x27]
|    :|||   0x06d93960      add   x27, x0, 0x28
|    :|||   0x06d93964      stp   q0, q1, [x27]
|    :|||   0x06d93968      stp   q2, q3, [x0, 0x40]
|    :|||   0x06d9396c      ldr   x6, [arg_4c0h]                       ; [0x4c0:4]=0x58585858
|    :|||   0x06d93970      str   x6, [x0, 0x60]
|    :|||   0x06d93974      adrp  x6, 0x6743000
|    :|||   0x06d93978      add   x6, x6, 0x100
|    :|||   0x06d9397c      add   x27, arg_368h
|    :|||   0x06d93980      stp   x6, x0, [x27]
|    :|||   0x06d93984      add   x6, arg_390h
|    :|||   0x06d93988      orr   x7, xzr, 3
|   .-----> 0x06d9398c      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d93990      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d93994      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d93998      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d9399c      sub   x7, x7, 1
|   `=====< 0x06d939a0      cbnz  x7, 0x6d9398c
|    :|||   0x06d939a4      adrp  x6, 0x8f92000
|    :|||   0x06d939a8      add   x6, x6, 0xc28
|    :|||   0x06d939ac      add   x7, arg_368h
|    :|||   0x06d939b0      add   x27, arg_450h
|    :|||   0x06d939b4      stp   x6, x7, [x27]
|    :|||   0x06d939b8      ldr   x0, [arg_278h]                       ; [0x278:4]=0x9019dd0
|    :|||   0x06d939bc      add   x3, arg_450h
|    :|||   0x06d939c0      orr   x4, xzr, 2
|    :|||   0x06d939c4      mov   x5, x4
|    :|||   0x06d939c8      bl    0x672b7f0
|    :|||   0x06d939cc      add   x6, arg_390h
|    :|||   0x06d939d0      add   x7, arg_8h
|    :|||   0x06d939d4      orr   x24, xzr, 3
|   .-----> 0x06d939d8      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d939dc      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d939e0      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d939e4      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d939e8      sub   x24, x24, 1
|   `=====< 0x06d939ec      cbnz  x24, 0x6d939d8
|    :|||   0x06d939f0      add   x6, arg_1b8h
|    :|||   0x06d939f4      add   x7, arg_390h
|    :|||   0x06d939f8      orr   x24, xzr, 3
|   .-----> 0x06d939fc      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d93a00      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d93a04      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d93a08      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d93a0c      sub   x24, x24, 1
|   `=====< 0x06d93a10      cbnz  x24, 0x6d939fc
|   ,=====< 0x06d93a14      cbz   x0, 0x6d93b20
|   |:|||   0x06d93a18      str   x1, [arg_198h]
|   |:|||   0x06d93a1c      str   x0, [arg_120h]
|   |:|||   0x06d93a20      adrp  x27, 0x9491000
|   |:|||   0x06d93a24      add   x27, x27, 0x210
|   |:|||   0x06d93a28      ldp   x2, x3, [x27]
|   |:|||   0x06d93a2c      bl    0x5116df0
|  ,======< 0x06d93a30      tbz   w0, 0, 0x6d93b38
|  ||:|||   0x06d93a34      ldr   x3, [arg_9f8h]                       ; [0x518:4]=0x58585858
| ,=======< 0x06d93a38      cbz   x3, 0x6d93a44
| |||:|||   0x06d93a3c      ldr   x3, [x3, 8]                          ; [0x8:4]=0
| ========< 0x06d93a40      b     0x6d93a48
| `-------> 0x06d93a44      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a40
| ,=======< 0x06d93a48      cbz   x3, 0x6d93a70
| |||:|||   0x06d93a4c      ldp   x3, x4, [x3, 8]
| |||:|||   0x06d93a50      adrp  x5, 0x8f0d000
| |||:|||   0x06d93a54      add   x5, x5, 0x9c8
| |||:|||   0x06d93a58      cmp   x3, x5
| ========< 0x06d93a5c      b.eq  0x6d93a64
| |||:|||   0x06d93a60      mov   x4, xzr
| --------> 0x06d93a64      b.ne  0x6d93a70
| |||:|||   0x06d93a68      ldr   x3, [x4]
| ========< 0x06d93a6c      b     0x6d93a74
| `-------> 0x06d93a70      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a6c
| ,=======< 0x06d93a74      cbz   x3, 0x6d93a80
| |||:|||   0x06d93a78      ldr   x3, [x3, 0xe0]                       ; [0xe0:4]=0
| ========< 0x06d93a7c      b     0x6d93a84
| `-------> 0x06d93a80      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a7c
| ,=======< 0x06d93a84      cbz   x3, 0x6d93a90
| |||:|||   0x06d93a88      ldrsw x3, [x3, 8]                          ; [0x8:4]=0
| ========< 0x06d93a8c      b     0x6d93a94
| `-------> 0x06d93a90      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a8c
| --------> 0x06d93a94      cmp   w3, 1
| ,=======< 0x06d93a98      b.ne  0x6d93b38
| |||:|||   0x06d93a9c      orr   x0, xzr, 0x10
| |||:|||   0x06d93aa0      adrp  x1, 0x81a5000
| |||:|||   0x06d93aa4      add   x1, x1, 0x4f8
| |||:|||   0x06d93aa8      orr   x2, xzr, 1
| |||:|||   0x06d93aac      bl    0x4d32500
| |||:|||   0x06d93ab0      ldr   x1, [arg_120h]                       ; [0x120:4]=7
| ========< 0x06d93ab4      cbz   x1, 0x6d93ac0
| |||:|||   0x06d93ab8      ldr   x11, [x1, 8]                         ; [0x8:4]=0
| ========< 0x06d93abc      b     0x6d93ac4
| --------> 0x06d93ac0      mov   x11, x1
| |||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93abc
| --------> 0x06d93ac4      str   x11, [x0]
| |||:|||   0x06d93ac8      adrp  x27, 0xb48f000
| |||:|||   0x06d93acc      ldr   w11, [x27, 0x450]                    ; [0x450:4]=0x58585858
| ========< 0x06d93ad0      cbnz  w11, 0x6d93adc
| |||:|||   0x06d93ad4      ldr   x11, [arg_198h]                      ; [0x198:4]=0x779bde0
| ========< 0x06d93ad8      b     0x6d93ae8
| --------> 0x06d93adc      bl    0x4da34f0
| |||:|||   0x06d93ae0      ldr   x11, [arg_198h]                      ; [0x198:4]=0x779bde0
| |||:|||   0x06d93ae4      str   x11, [x25]
| |||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93ad8
| --------> 0x06d93ae8      str   x11, [x0, 8]
| |||:|||   0x06d93aec      ldr   x1, [arg_4f8h]                       ; [0x4f8:4]=0x58585858
| |||:|||   0x06d93af0      orr   x2, xzr, 1
| |||:|||   0x06d93af4      orr   x3, xzr, 1
| |||:|||   0x06d93af8      mov   x4, xzr
| |||:|||   0x06d93afc      mov   x5, xzr
| |||:|||   0x06d93b00      adrp  x6, 0x4479000
| |||:|||   0x06d93b04      add   x6, x6, 0x8ad
| |||:|||   0x06d93b08      orr   x7, xzr, 0x3c
| |||:|||   0x06d93b0c      mov   x8, x0
| |||:|||   0x06d93b10      mov   x9, x2
| |||:|||   0x06d93b14      mov   x10, x2
| |||:|||   0x06d93b18      ldr   x0, [arg_4f0h]                       ; [0x4f0:4]=0x58585858
| |||:|||   0x06d93b1c      bl    0x4f6f980
| ||`-----> 0x06d93b20      ldr   x4, [arg_4c0h]                       ; [0x4c0:4]=0x58585858
| || :|||   0x06d93b24      ldp   x4, x5, [x4]
| || :|||   0x06d93b28      str   x5, [arg_168h]
| || :|||   0x06d93b2c      str   x4, [arg_308h]
| || :|||   0x06d93b30      mov   x6, xzr
| ||,=====< 0x06d93b34      b     0x6d93b70
| ``------> 0x06d93b38      ldr   x0, [arg_120h]                       ; [0x120:4]=7
|   |:|||   0x06d93b3c      ldr   x1, [arg_198h]                       ; [0x198:4]=0x779bde0
|   |:|||   0x06d93b40      ldp   fp, lr, [sp, -8]
|   |:|||   0x06d93b44      add   sp, sp, 0x4e0
|   |:|||   0x06d93b48      ret
|   |:||`-> 0x06d93b4c      mov   x0, x1
|   |:||    0x06d93b50      mov   x1, x2
|   |:||    0x06d93b54      ldp   fp, lr, [sp, -8]
|   |:||    0x06d93b58      add   sp, sp, 0x4e0
|   |:||    0x06d93b5c      ret
|   |:||.-> 0x06d93b60      ldr   x7, [arg_158h]                       ; [0x158:4]=0x6474e552
|   |:||:   0x06d93b64      add   x6, x7, 1
|   |:||:   0x06d93b68      ldr   x4, [arg_308h]                       ; [0x308:4]=0x50d0
|   |:||:   0x06d93b6c      ldr   x5, [arg_168h]                       ; [0x168:4]=0x799b960 segment.GNU_RELRO
|   |:||:   ; CODE XREF from fcn.06d934f8 @ 0x6d93b34
|   `-----> 0x06d93b70      cmp   x6, x5
|   ,=====< 0x06d93b74      b.ge  0x6d93ca0
|   |:||:   0x06d93b78      str   x6, [arg_158h]
|   |:||:   0x06d93b7c      ldr   x7, [x4, x6, lsl 3]
|   |:||:   0x06d93b80      str   x7, [arg_188h]
|   |:||:   0x06d93b84      ldp   x8, x9, [x7, 0x28]
|   |:||:   0x06d93b88      str   x9, [arg_150h]
|   |:||:   0x06d93b8c      str   x8, [arg_300h]
|   |:||:   0x06d93b90      mov   x0, xzr
|  ,======< 0x06d93b94      b     0x6d93ba8
|  ||:||:   ; CODE XREF from fcn.06d934f8 @ 0x6d93c9c
| .-------> 0x06d93b98      ldr   x4, [arg_148h]                       ; [0x148:4]=0x458
| :||:||:   0x06d93b9c      add   x0, x4, 1
| :||:||:   0x06d93ba0      ldr   x8, [arg_300h]                       ; [0x300:4]=0x50d0
| :||:||:   0x06d93ba4      ldr   x9, [arg_150h]                       ; [0x150:4]=32
| :||:||:   ; CODE XREF from fcn.06d934f8 @ 0x6d93b94
| :`------> 0x06d93ba8      cmp   x0, x9
| : |:||`=< 0x06d93bac      b.ge  0x6d93b60
| : |:||    0x06d93bb0      str   x0, [arg_148h]
| : |:||    0x06d93bb4      ldr   x4, [x8, x0, lsl 3]
| : |:||    0x06d93bb8      str   x4, [arg_180h]
| : |:||    0x06d93bbc      adrp  x27, 0x948b000
| : |:||    0x06d93bc0      ldr   x26, [x27, 0x830]                    ; [0x830:4]=0x58585858
| : |:||    0x06d93bc4      ldr   x5, [x26]
| : |:||    0x06d93bc8      ldp   x0, x1, [x4, 0x58]
| : |:||    0x06d93bcc      ldp   x2, x3, [x4, 8]
| : |:||    0x06d93bd0      blr   x5
| : |:||    0x06d93bd4      ldr   x4, [arg_258h]                       ; [0x258:4]=0xeb030
| : |:||    0x06d93bd8      orr   x5, xzr, 7
| : |:||    0x06d93bdc      str   w5, [arg_378h]
| : |:||    0x06d93be0      add   x27, arg_380h
| : |:||    0x06d93be4      stp   x0, x1, [x27]
| : |:||    0x06d93be8      adrp  x0, 0x8de9000
| : |:||    0x06d93bec      add   x0, x0, 0xc48
| : |:||    0x06d93bf0      mov   x1, x4
| : |:||    0x06d93bf4      add   x2, arg_378h
| : |:||    0x06d93bf8      bl    0x4d16af0
| ========< 0x06d93bfc      tbz   w1, 0, 0x6d93b98
| : |:||    0x06d93c00      ldr   x5, [arg_188h]                       ; [0x188:4]=1
| : |:||    0x06d93c04      ldr   x6, [x5, 8]                          ; [0x8:4]=0
| : |:||    0x06d93c08      ldr   x2, [x6, 0x70]                       ; [0x70:4]=8
| : |:||    0x06d93c0c      ldp   x0, x7, [x6, 0x60]
| : |:||    0x06d93c10      add   x1, x7, 1
| : |:||    0x06d93c14      ldr   x7, [arg_180h]                       ; [0x180:4]=0x16236a0 "ki/codeium_common_pb/codeium_common_go_proto.SearchResultType.String"
| : |:||    0x06d93c18      ldp   x7, x8, [x7, 8]
| : |:||    0x06d93c1c      cmp   x2, x1
| : |:||,=< 0x06d93c20      b.hs  0x6d93c70
| : |:|||   0x06d93c24      str   x8, [arg_140h]
| : |:|||   0x06d93c28      str   x7, [arg_338h]
| : |:|||   0x06d93c2c      str   x6, [arg_2e8h]
| : |:|||   0x06d93c30      orr   x3, xzr, 1
| : |:|||   0x06d93c34      adrp  x4, 0x856f000
| : |:|||   0x06d93c38      add   x4, x4, 0x550
| : |:|||   0x06d93c3c      bl    0x4d9e050
| : |:|||   0x06d93c40      ldr   x5, [arg_2e8h]                       ; [0x2e8:4]=0xacf0000
| : |:|||   0x06d93c44      str   x2, [x5, 0x70]
| : |:|||   0x06d93c48      adrp  x27, 0xb48f000
| : |:|||   0x06d93c4c      ldr   w6, [x27, 0x450]                     ; [0x450:4]=0x58585858
| :,======< 0x06d93c50      cbz   w6, 0x6d93c60
| :||:|||   0x06d93c54      ldr   x4, [x5, 0x60]                       ; [0x60:4]=0x380
| :||:|||   0x06d93c58      bl    0x4da3500
| :||:|||   0x06d93c5c      stp   x0, x4, [x25]
| :`------> 0x06d93c60      str   x0, [x5, 0x60]
| : |:|||   0x06d93c64      mov   x6, x5
| : |:|||   0x06d93c68      ldr   x7, [arg_338h]                       ; [0x338:4]=27
| : |:|||   0x06d93c6c      ldr   x8, [arg_140h]                       ; [0x140:4]=144
| : |:||`-> 0x06d93c70      str   x1, [x6, 0x68]
| : |:||    0x06d93c74      sub   x4, x1, 1
| : |:||    0x06d93c78      add   x4, x0, x4, lsl 4
| : |:||    0x06d93c7c      str   x8, [x4, 8]
| : |:||    0x06d93c80      adrp  x27, 0xb48f000
| : |:||    0x06d93c84      ldr   w6, [x27, 0x450]                     ; [0x450:4]=0x58585858
| : |:||,=< 0x06d93c88      cbz   w6, 0x6d93c98
| : |:|||   0x06d93c8c      ldr   x6, [x4]
| : |:|||   0x06d93c90      bl    0x4da3500
| : |:|||   0x06d93c94      stp   x7, x6, [x25]
| : |:||`-> 0x06d93c98      str   x7, [x4]
| `=======< 0x06d93c9c      b     0x6d93b98
|   `-----> 0x06d93ca0      adrp  x27, 0x948b000
|    :||    0x06d93ca4      ldr   x0, [x27, 0xa18]                     ; [0xa18:4]=0x58585858
|    :||    0x06d93ca8      nop
|    :||    0x06d93cac      adrp  x1, 0x79e9000
|    :||    0x06d93cb0      add   x1, x1, 0xc80
|    :||    0x06d93cb4      ldr   x2, [arg_500h]                       ; [0x500:4]=0x58585858
|    :||    0x06d93cb8      ldr   x3, [arg_4d0h]                       ; [0x4d0:4]=0x58585858
|    :||    0x06d93cbc      bl    0x6786750
|    :||    0x06d93cc0      adrp  x0, 0x89d0000
|    :||    0x06d93cc4      add   x0, x0, 0xb30
|    :||    0x06d93cc8      bl    0x4d31c10
|    :||    0x06d93ccc      ldr   x4, [arg_4c8h]                       ; [0x4c8:4]=0x58585858
|    :||    0x06d93cd0      ldr   x4, [x4]
|    :||    0x06d93cd4      add   x1, x0, 0x10
|    :||    0x06d93cd8      adrp  x27, 0xb48f000
|    :||    0x06d93cdc      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d93ce0      cbz   w5, 0x6d93d18
|    :|||   0x06d93ce4      str   x0, [arg_1b0h]
|    :|||   0x06d93ce8      str   x4, [arg_2f8h]
|    :|||   0x06d93cec      str   x1, [arg_2f0h]
|    :|||   0x06d93cf0      bl    0x4da3500
|    :|||   0x06d93cf4      ldr   x3, [arg_278h]                       ; [0x278:4]=0x9019dd0
|    :|||   0x06d93cf8      stp   x4, x3, [x25]
|    :|||   0x06d93cfc      adrp  x0, 0x8dec000
|    :|||   0x06d93d00      add   x0, x0, 0x128
|    :|||   0x06d93d04      add   x2, arg_1b8h
|    :|||   0x06d93d08      bl    0x4d35930
|    :|||   0x06d93d0c      ldr   x0, [arg_1b0h]                       ; [0x1b0:4]=0x220
|    :|||   0x06d93d10      ldr   x1, [arg_2f0h]                       ; [0x2f0:4]=0xd8f0000 segment.LOAD6
|    :|||   0x06d93d14      ldr   x4, [arg_2f8h]                       ; [0x2f8:4]=0xd8f0000 segment.LOAD6
|    :||`-> 0x06d93d18      ldr   x5, [arg_278h]                       ; [0x278:4]=0x9019dd0
|    :||    0x06d93d1c      stp   x4, x5, [x0]
|    :||    0x06d93d20      add   x4, arg_1b8h
|    :||    0x06d93d24      orr   x24, xzr, 3
|    :||.-> 0x06d93d28      ldp   q16, q17, [x4], 0x20
|    :||:   0x06d93d2c      stp   q16, q17, [x1], 0x20
|    :||:   0x06d93d30      ldp   q16, q17, [x4], 0x20
|    :||:   0x06d93d34      stp   q16, q17, [x1], 0x20
|    :||:   0x06d93d38      sub   x24, x24, 1
|    :||`=< 0x06d93d3c      cbnz  x24, 0x6d93d28
|    :||    0x06d93d40      adrp  x27, 0x948b000
|    :||    0x06d93d44      ldr   x4, [x27, 0xa20]                     ; [0xa20:4]=0x58585858
|    :||    0x06d93d48      adrp  x1, 0x79e9000
|    :||    0x06d93d4c      add   x1, x1, 0xc20
|    :||    0x06d93d50      ldr   x2, [arg_500h]                       ; [0x500:4]=0x58585858
|    :||    0x06d93d54      mov   x3, x0
|    :||    0x06d93d58      mov   x0, x4
|    :||    0x06d93d5c      bl    0x6786750
|    :||    0x06d93d60      mov   x0, xzr
|    :||    0x06d93d64      mov   x1, xzr
|    :||    0x06d93d68      ldp   fp, lr, [sp, -8]
|    :||    0x06d93d6c      add   sp, sp, 0x4e0
|    :||    0x06d93d70      ret
|    :||    ; CODE XREFS from fcn.06d934f8 @ 0x6d93dbc, 0x6d93dc4
|   .--`.-> 0x06d93d74      ldr   x10, [arg_4e8h]                      ; [0x4e8:4]=0x58585858
|   ::| :   0x06d93d78      mov   x9, x0
|   ::| :   0x06d93d7c      mov   x11, x1
|   :`====< 0x06d93d80      b     0x6d93598
|   : | :   ; CODE XREF from fcn.06d934f8 @ 0x6d93560
|   : `.--> 0x06d93d84      and   x10, x9, x8
|   :  ::   0x06d93d88      lsl   x10, x10, 4
|   :  ::   0x06d93d8c      add   x10, x10, 8
|   :  ::   0x06d93d90      ldr   x11, [x7, x10]                       ; arg8
|   :  ::   0x06d93d94      add   x10, x7, x10                         ; arg8
|   :  ::   0x06d93d98      cmp   x11, x5
|   : ,===< 0x06d93d9c      b.eq  0x6d93dc0
|   : |::   0x06d93da0      add   x9, x9, 1
|   : |`==< 0x06d93da4      cbnz  x11, 0x6d93d84
|   : | :   0x06d93da8      str   x1, [arg_190h]
|   : | :   0x06d93dac      mov   x0, x6
|   : | :   0x06d93db0      mov   x1, x5
|   : | :   0x06d93db4      bl    0x4d2e4c0
|   : | :   0x06d93db8      ldr   x1, [arg_190h]                       ; [0x190:4]=2
|   `=====< 0x06d93dbc      b     0x6d93d74
|     `---> 0x06d93dc0      ldr   x0, [x10, 8]                         ; [0x8:4]=0
\       `=< 0x06d93dc4      b     0x6d93d74
==== google3_third_party_jetski_gateway_gateway.loadSettingsFile
/ fcn.06d934f8(int64_t arg6, int64_t arg7, int64_t arg8, int64_t arg_8h, int64_t arg_118h, int64_t arg_120h, int64_t arg_128h, int64_t arg_130h, int64_t arg_138h, int64_t arg_140h, int64_t arg_148h, int64_t arg_150h, int64_t arg_158h, int64_t arg_160h, int64_t arg_168h, int64_t arg_180h, int64_t arg_188h, int64_t arg_190h, int64_t arg_198h, int64_t arg_1a0h, int64_t arg_1a8h, int64_t arg_1b0h, int64_t arg_1b8h, int64_t arg_258h, int64_t arg_278h, int64_t arg_280h, int64_t arg_2d8h, int64_t arg_2e0h, int64_t arg_2e8h, int64_t arg_2f0h, int64_t arg_2f8h, int64_t arg_300h, int64_t arg_308h, int64_t arg_310h, int64_t arg_318h, int64_t arg_320h, int64_t arg_328h, int64_t arg_330h, int64_t arg_338h, int64_t arg_368h, int64_t arg_378h, int64_t arg_380h, int64_t arg_390h, int64_t arg_3e8h, int64_t arg_3f8h, int64_t arg_408h, int64_t arg_450h, int64_t arg_4c0h, int64_t arg_4c8h, int64_t arg_4d0h, int64_t arg_4e8h, int64_t arg_4f0h, int64_t arg_4f8h, int64_t arg_500h, int64_t arg_508h, int64_t arg_518h, int64_t arg_520h, int64_t arg_9f8h);
|           ; arg int64_t arg6 @ x5
|           ; arg int64_t arg7 @ x6
|           ; arg int64_t arg8 @ x7
|           ; arg int64_t arg_8h @ stack + 0x8
|           ; arg int64_t arg_118h @ stack + 0x118
|           ; arg int64_t arg_120h @ stack + 0x120
|           ; arg int64_t arg_128h @ stack + 0x128
|           ; arg int64_t arg_130h @ stack + 0x130
|           ; arg int64_t arg_138h @ stack + 0x138
|           ; arg int64_t arg_140h @ stack + 0x140
|           ; arg int64_t arg_148h @ stack + 0x148
|           ; arg int64_t arg_150h @ stack + 0x150
|           ; arg int64_t arg_158h @ stack + 0x158
|           ; arg int64_t arg_160h @ stack + 0x160
|           ; arg int64_t arg_168h @ stack + 0x168
|           ; arg int64_t arg_180h @ stack + 0x180
|           ; arg int64_t arg_188h @ stack + 0x188
|           ; arg int64_t arg_190h @ stack + 0x190
|           ; arg int64_t arg_198h @ stack + 0x198
|           ; arg int64_t arg_1a0h @ stack + 0x1a0
|           ; arg int64_t arg_1a8h @ stack + 0x1a8
|           ; arg int64_t arg_1b0h @ stack + 0x1b0
|           ; arg int64_t arg_1b8h @ stack + 0x1b8
|           ; arg int64_t arg_258h @ stack + 0x258
|           ; arg int64_t arg_278h @ stack + 0x278
|           ; arg int64_t arg_280h @ stack + 0x280
|           ; arg int64_t arg_2d8h @ stack + 0x2d8
|           ; arg int64_t arg_2e0h @ stack + 0x2e0
|           ; arg int64_t arg_2e8h @ stack + 0x2e8
|           ; arg int64_t arg_2f0h @ stack + 0x2f0
|           ; arg int64_t arg_2f8h @ stack + 0x2f8
|           ; arg int64_t arg_300h @ stack + 0x300
|           ; arg int64_t arg_308h @ stack + 0x308
|           ; arg int64_t arg_310h @ stack + 0x310
|           ; arg int64_t arg_318h @ stack + 0x318
|           ; arg int64_t arg_320h @ stack + 0x320
|           ; arg int64_t arg_328h @ stack + 0x328
|           ; arg int64_t arg_330h @ stack + 0x330
|           ; arg int64_t arg_338h @ stack + 0x338
|           ; arg int64_t arg_368h @ stack + 0x368
|           ; arg int64_t arg_378h @ stack + 0x378
|           ; arg int64_t arg_380h @ stack + 0x380
|           ; arg int64_t arg_390h @ stack + 0x390
|           ; arg int64_t arg_3e8h @ stack + 0x3e8
|           ; arg int64_t arg_3f8h @ stack + 0x3f8
|           ; arg int64_t arg_408h @ stack + 0x408
|           ; arg int64_t arg_450h @ stack + 0x450
|           ; arg int64_t arg_4c0h @ stack + 0x4c0
|           ; arg int64_t arg_4c8h @ stack + 0x4c8
|           ; arg int64_t arg_4d0h @ stack + 0x4d0
|           ; arg int64_t arg_4e8h @ stack + 0x4e8
|           ; arg int64_t arg_4f0h @ stack + 0x4f0
|           ; arg int64_t arg_4f8h @ stack + 0x4f8
|           ; arg int64_t arg_500h @ stack + 0x500
|           ; arg int64_t arg_508h @ stack + 0x508
|           ; arg int64_t arg_518h @ stack + 0x518
|           ; arg int64_t arg_520h @ stack + 0x520
|           ; arg int64_t arg_9f8h @ stack + 0x9f8
|           0x06d934f8      ldrsw x9, [x10]
|       ,=< 0x06d934fc      b     0x6d9350c
..
|      ||   ; CODE XREF from fcn.06d934f8 @ 0x6d934fc
|      ``-> 0x06d9350c      str   x5, [arg_130h]                       ; arg6
|           0x06d93510      str   x7, [arg_2d8h]                       ; arg8
|           0x06d93514      str   x6, [arg_138h]                       ; arg7
|           0x06d93518      str   x8, [arg_2e0h]
|           0x06d9351c      ldr   x10, [arg_4e8h]                      ; [0x4e8:4]=0x58585858
|           0x06d93520      ldr   x11, [x10]
|           0x06d93524      ldp   x11, x12, [x11, 0x108]
|       ,=< 0x06d93528      cbz   x11, 0x6d93564
|       |   0x06d9352c      ldr   x4, [x11, 0x18]                      ; [0x18:4]=0x4d0f000 section..text
|       |   0x06d93530      mov   x0, x12
|       |   0x06d93534      ldr   x1, [arg_4f0h]                       ; [0x4f0:4]=0x58585858
|       |   0x06d93538      ldr   x2, [arg_4f8h]                       ; [0x4f8:4]=0x58585858
|       |   0x06d9353c      mov   x3, x9
|       |   0x06d93540      blr   x4
|      ,==< 0x06d93544      cbz   x0, 0x6d93d74
|      ||   0x06d93548      ldr   x5, [x0, 8]                          ; [0x8:4]=0
|      ||   0x06d9354c      adrp  x6, 0x94a8000
|      ||   0x06d93550      add   x6, x6, 0x480
|      ||   0x06d93554      ldar  x7, [x6]
|      ||   0x06d93558      ldr   x8, [x7]                             ; arg8
|      ||   0x06d9355c      ldr   w9, [x0, 0x10]                       ; [0x10:4]=0xb70003
|     ,===< 0x06d93560      b     0x6d93d84
|     ||`-> 0x06d93564      mov   x9, xzr
|     ||    0x06d93568      mov   x11, xzr
|     ||,=< 0x06d9356c      b     0x6d93598
..
|     |||   ; CODE XREFS from fcn.06d934f8 @ 0x6d9356c, 0x6d93d80
|    .--`-> 0x06d93598      str   x11, [arg_190h]
|    :||    0x06d9359c      str   x9, [arg_118h]
|    :||    0x06d935a0      ldr   x12, [x10]
|    :||    0x06d935a4      ldrsb x27, [x12]
|    :||    0x06d935a8      add   x13, x12, 0xb0
|    :||    0x06d935ac      ldr   x12, [x12, 0xb8]                     ; [0xb8:4]=0
|    :||    0x06d935b0      add   x14, arg_280h
|    :||    0x06d935b4      ldp   q16, q17, [x13]
|    :||    0x06d935b8      stp   q16, q17, [x14]
|    :||    0x06d935bc      ldp   q16, q17, [x13, 0x20]
|    :||    0x06d935c0      stp   q16, q17, [x14, 0x20]
|    :||    0x06d935c4      ldr   q16, [x13, 0x40]                     ; segment.PHDR
|    :||                                                               ; [0x40:4]=6 ; "\U00000006"
|    :||    0x06d935c8      str   q16, [x14, 0x40]
|    :||    0x06d935cc      ldr   x25, [x13, 0x50]                     ; [0x50:4]=64
|    :||    0x06d935d0      str   x25, [x14, 0x50]
|    :||,=< 0x06d935d4      cbnz  x12, 0x6d93608
|    :|||   0x06d935d8      orr   x0, xzr, 8
|    :|||   0x06d935dc      adrp  x1, 0x81a4000
|    :|||   0x06d935e0      add   x1, x1, 0x580
|    :|||   0x06d935e4      orr   x2, xzr, 1
|    :|||   0x06d935e8      bl    0x4d322b0
|    :|||   0x06d935ec      adrp  x3, 0x8f91000
|    :|||   0x06d935f0      add   x3, x3, 0x3a8
|    :|||   0x06d935f4      str   x3, [x0]
|    :|||   0x06d935f8      orr   x3, xzr, 1
|    :|||   0x06d935fc      add   x27, arg_280h
|    :|||   0x06d93600      stp   x0, x3, [x27]
|    :|||   0x06d93604      str   x3, [arg_280h + 0x10]
|    :||`-> 0x06d93608      add   x27, arg_280h
|    :||    0x06d9360c      ldp   x12, x13, [x27]
|    :||,=< 0x06d93610      cbnz  x13, 0x6d93624
|    :|||   0x06d93614      mov   x14, xzr
|    :|||   0x06d93618      mov   x15, xzr
|    :|||   0x06d9361c      mov   x16, xzr
|   ,=====< 0x06d93620      b     0x6d9365c
|   |:||`-> 0x06d93624      str   x13, [arg_158h]
|   |:||    0x06d93628      str   x12, [arg_328h]
|   |:||    0x06d9362c      mov   x0, xzr
|   |:||    0x06d93630      mov   x1, x13
|   |:||    0x06d93634      mov   x2, xzr
|   |:||    0x06d93638      mov   x3, x1
|   |:||    0x06d9363c      adrp  x4, 0x86f9000
|   |:||    0x06d93640      add   x4, x4, 0x788
|   |:||    0x06d93644      bl    0x4d9e050
|   |:||    0x06d93648      ldr   x12, [arg_328h]                      ; [0x328:4]=0xd8f5070 segment.INTERP
|   |:||    0x06d9364c      ldr   x13, [arg_158h]                      ; [0x158:4]=0x6474e552
|   |:||    0x06d93650      mov   x14, x0
|   |:||    0x06d93654      mov   x15, x1
|   |:||    0x06d93658      mov   x16, x2
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d93620
|   `-----> 0x06d9365c      stp   x15, x16, [arg_150h]
|    :||    0x06d93660      str   x14, [arg_328h]
|    :||    0x06d93664      adrp  x0, 0x86f9000
|    :||    0x06d93668      add   x0, x0, 0x788
|    :||    0x06d9366c      mov   x1, x14
|    :||    0x06d93670      mov   x2, x15
|    :||    0x06d93674      mov   x3, x12
|    :||    0x06d93678      mov   x4, x13
|    :||    0x06d9367c      bl    0x4d99570
|    :||    0x06d93680      orr   x0, xzr, 0x20
|    :||    0x06d93684      adrp  x1, 0x81a4000
|    :||    0x06d93688      add   x1, x1, 0xa48
|    :||    0x06d9368c      orr   x2, xzr, 1
|    :||    0x06d93690      bl    0x4d32c00
|    :||    0x06d93694      ldr   x5, [arg_160h]                       ; [0x160:4]=0x779b960
|    :||    0x06d93698      str   x5, [x0]
|    :||    0x06d9369c      ldr   x5, [arg_168h]                       ; [0x168:4]=0x799b960 segment.GNU_RELRO
|    :||    0x06d936a0      str   x5, [x0, 0x18]
|    :||    0x06d936a4      adrp  x27, 0xb48f000
|    :||    0x06d936a8      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d936ac      cbnz  w5, 0x6d936bc
|    :|||   0x06d936b0      ldr   x3, [arg_338h]                       ; [0x338:4]=27
|    :|||   0x06d936b4      ldr   x4, [arg_330h]                       ; [0x330:4]=0xd8f5070 segment.INTERP
|   ,=====< 0x06d936b8      b     0x6d936cc
|   |:||`-> 0x06d936bc      bl    0x4da3500
|   |:||    0x06d936c0      ldr   x3, [arg_338h]                       ; [0x338:4]=27
|   |:||    0x06d936c4      ldr   x4, [arg_330h]                       ; [0x330:4]=0xd8f5070 segment.INTERP
|   |:||    0x06d936c8      stp   x3, x4, [x25]
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d936b8
|   `-----> 0x06d936cc      str   x0, [arg_320h]
|    :||    0x06d936d0      stp   x3, x4, [x0, 8]
|    :||    0x06d936d4      orr   x0, xzr, 0x20
|    :||    0x06d936d8      adrp  x1, 0x87f7000
|    :||    0x06d936dc      add   x1, x1, 0xe18
|    :||    0x06d936e0      orr   x2, xzr, 1
|    :||    0x06d936e4      bl    0x4d32c00
|    :||    0x06d936e8      adrp  x3, 0x673f000
|    :||    0x06d936ec      add   x3, x3, 0x490
|    :||    0x06d936f0      str   x3, [x0]
|    :||    0x06d936f4      orr   x3, xzr, 1
|    :||    0x06d936f8      stp   x3, x3, [x0, 0x10]
|    :||    0x06d936fc      adrp  x27, 0xb48f000
|    :||    0x06d93700      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d93704      cbnz  w4, 0x6d93710
|    :|||   0x06d93708      ldr   x5, [arg_320h]                       ; [0x320:4]=0xacf5070
|   ,=====< 0x06d9370c      b     0x6d9371c
|   |:||`-> 0x06d93710      bl    0x4da34f0
|   |:||    0x06d93714      ldr   x5, [arg_320h]                       ; [0x320:4]=0xacf5070
|   |:||    0x06d93718      str   x5, [x25]
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d9370c
|   `-----> 0x06d9371c      str   x5, [x0, 8]
|    :||    0x06d93720      ldr   x5, [arg_150h]                       ; [0x150:4]=32
|    :||    0x06d93724      add   x1, x5, 1
|    :||    0x06d93728      ldr   x2, [arg_158h]                       ; [0x158:4]=0x6474e552
|    :||    0x06d9372c      cmp   x2, x1
|    :||,=< 0x06d93730      b.lo  0x6d9373c
|    :|||   0x06d93734      ldr   x3, [arg_328h]                       ; [0x328:4]=0xd8f5070 segment.INTERP
|   ,=====< 0x06d93738      b     0x6d93758
|   |:||`-> 0x06d9373c      str   x0, [arg_318h]
|   |:||    0x06d93740      ldr   x0, [arg_328h]                       ; [0x328:4]=0xd8f5070 segment.INTERP
|   |:||    0x06d93744      adrp  x4, 0x86f9000
|   |:||    0x06d93748      add   x4, x4, 0x788
|   |:||    0x06d9374c      bl    0x4d9e050
|   |:||    0x06d93750      mov   x3, x0
|   |:||    0x06d93754      ldr   x0, [arg_318h]                       ; [0x318:4]=3
|   |:||    ; CODE XREF from fcn.06d934f8 @ 0x6d93738
|   `-----> 0x06d93758      sub   x4, x1, 1
|    :||    0x06d9375c      adrp  x27, 0xb48f000
|    :||    0x06d93760      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d93764      cbz   w5, 0x6d93774
|    :|||   0x06d93768      ldr   x5, [x3, x4, lsl 3]
|    :|||   0x06d9376c      bl    0x4da3500
|    :|||   0x06d93770      stp   x0, x5, [x25]
|    :||`-> 0x06d93774      str   x0, [x3, x4, lsl 3]
|    :||    0x06d93778      add   x27, arg_280h
|    :||    0x06d9377c      stp   x3, x1, [x27]
|    :||    0x06d93780      str   x2, [arg_280h + 0x10]
|    :||    0x06d93784      add   x3, arg_390h
|    :||    0x06d93788      stp   xzr, xzr, [x3]
|    :||    0x06d9378c      stp   xzr, xzr, [x3, 0x10]
|    :||    0x06d93790      stp   xzr, xzr, [x3, 0x20]
|    :||    0x06d93794      stp   xzr, xzr, [x3, 0x30]
|    :||    0x06d93798      stp   xzr, xzr, [x3, 0x40]
|    :||    0x06d9379c      stp   xzr, xzr, [x3, 0x50]
|    :||    0x06d937a0      stp   xzr, xzr, [x3, 0x60]
|    :||    0x06d937a4      stp   xzr, xzr, [x3, 0x70]
|    :||    0x06d937a8      stp   xzr, xzr, [x3, 0x80]
|    :||    0x06d937ac      stp   xzr, xzr, [x3, 0x90]
|    :||    0x06d937b0      stp   xzr, xzr, [x3, 0xa0]
|    :||    0x06d937b4      str   xzr, [x3, 0xb0]
|    :||    0x06d937b8      add   x4, arg_390h
|    :||    0x06d937bc      add   x5, arg_280h
|    :||    0x06d937c0      ldp   q16, q17, [x5]
|    :||    0x06d937c4      stp   q16, q17, [x4]
|    :||    0x06d937c8      ldp   q16, q17, [x5, 0x20]
|    :||    0x06d937cc      stp   q16, q17, [x4, 0x20]
|    :||    0x06d937d0      ldr   q16, [x5, 0x40]                      ; segment.PHDR
|    :||                                                               ; [0x40:4]=6 ; "\U00000006"
|    :||    0x06d937d4      str   q16, [x4, 0x40]
|    :||    0x06d937d8      ldr   x25, [x5, 0x50]                      ; [0x50:4]=64
|    :||    0x06d937dc      str   x25, [x4, 0x50]
|    :||    0x06d937e0      ldr   x4, [arg_1a0h]                       ; [0x1a0:4]=0x799bde0 segment.DYNAMIC
|    :||    0x06d937e4      ldr   x5, [arg_138h]                       ; [0x138:4]=0x799b960 segment.GNU_RELRO
|    :||    0x06d937e8      add   x27, arg_3f8h
|    :||    0x06d937ec      stp   x4, x5, [x27]
|    :||    0x06d937f0      ldr   x4, [arg_2e0h]                       ; [0x2e0:4]=1
|    :||    0x06d937f4      ldr   x5, [arg_2d8h]                       ; [0x2d8:4]=0x200000
|    :||    0x06d937f8      add   x27, arg_408h
|    :||    0x06d937fc      stp   x4, x5, [x27]
|    :||    0x06d93800      ldr   x4, [arg_130h]                       ; [0x130:4]=0x799b960 segment.GNU_RELRO
|    :||    0x06d93804      str   x4, [arg_408h + 0x10]
|    :||    0x06d93808      ldr   x4, [arg_118h]                       ; [0x118:4]=0x200000
|    :||    0x06d9380c      ldr   x5, [arg_190h]                       ; [0x190:4]=2
|    :||    0x06d93810      add   x27, arg_3e8h
|    :||    0x06d93814      stp   x4, x5, [x27]
|    :||    0x06d93818      ldr   x4, [arg_518h]                       ; [0x518:4]=0x58585858
|    :||    0x06d9381c      str   x4, [arg_408h + 0x18]
|    :||    0x06d93820      ldr   x0, [arg_520h]                       ; [0x520:4]=0x58585858
|    :||    0x06d93824      ldr   x4, [arg_4e8h]                       ; [0x4e8:4]=0x58585858
|    :||    0x06d93828      ldp   x1, x2, [x4, 8]
|    :||    0x06d9382c      add   x4, arg_8h
|    :||    0x06d93830      ldp   q16, q17, [x3]
|    :||    0x06d93834      stp   q16, q17, [x4]
|    :||    0x06d93838      ldp   q16, q17, [x3, 0x20]
|    :||    0x06d9383c      stp   q16, q17, [x4, 0x20]
|    :||    0x06d93840      ldp   q16, q17, [x3, 0x40]
|    :||    0x06d93844      stp   q16, q17, [x4, 0x40]
|    :||    0x06d93848      ldp   q16, q17, [x3, 0x60]
|    :||    0x06d9384c      stp   q16, q17, [x4, 0x60]
|    :||    0x06d93850      ldp   q16, q17, [x3, 0x80]
|    :||    0x06d93854      stp   q16, q17, [x4, 0x80]
|    :||    0x06d93858      ldr   q16, [x3, 0xa0]                      ; [0xa0:4]=0x779b960
|    :||    0x06d9385c      str   q16, [x4, 0xa0]
|    :||    0x06d93860      ldr   x25, [x3, 0xb0]                      ; [0xb0:4]=0x6474e551
|    :||    0x06d93864      str   x25, [x4, 0xb0]
|    :||    0x06d93868      bl    0x672a030
|    :||,=< 0x06d9386c      cbnz  x1, 0x6d93b4c
|    :|||   0x06d93870      str   x0, [arg_278h]
|    :|||   0x06d93874      ldr   x0, [arg_4f0h]                       ; [0x4f0:4]=0x58585858
|    :|||   0x06d93878      ldr   x1, [arg_4f8h]                       ; [0x4f8:4]=0x58585858
|    :|||   0x06d9387c      bl    0x6711700
|    :|||   0x06d93880      str   x0, [arg_128h]
|    :|||   0x06d93884      str   x1, [arg_1a8h]
|    :|||   0x06d93888      orr   x0, xzr, 8
|    :|||   0x06d9388c      adrp  x1, 0x800b000
|    :|||   0x06d93890      add   x1, x1, 0xf98
|    :|||   0x06d93894      orr   x2, xzr, 1
|    :|||   0x06d93898      bl    0x4d322b0
|    :|||   0x06d9389c      str   x0, [arg_4c8h]
|    :|||   0x06d938a0      orr   x0, xzr, 0x18
|    :|||   0x06d938a4      adrp  x1, 0x81b3000
|    :|||   0x06d938a8      add   x1, x1, 0xd48
|    :|||   0x06d938ac      orr   x2, xzr, 1
|    :|||   0x06d938b0      bl    0x4d32830
|    :|||   0x06d938b4      str   x0, [arg_4c0h]
|    :|||   0x06d938b8      adrp  x0, 0x8c17000
|    :|||   0x06d938bc      add   x0, x0, 0x978
|    :|||   0x06d938c0      bl    0x4d31c10
|    :|||   0x06d938c4      nop
|    :|||   0x06d938c8      adr   x2, 0x6d944d0
|    :|||   0x06d938cc      str   x2, [x0]
|    :|||   0x06d938d0      adrp  x27, 0xb48f000
|    :|||   0x06d938d4      ldr   w2, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d938d8      cbnz  w2, 0x6d938e4
|   |:|||   0x06d938dc      ldr   x6, [arg_4c8h]                       ; [0x4c8:4]=0x58585858
|  ,======< 0x06d938e0      b     0x6d938f0
|  |`-----> 0x06d938e4      bl    0x4da34f0
|  | :|||   0x06d938e8      ldr   x6, [arg_4c8h]                       ; [0x4c8:4]=0x58585858
|  | :|||   0x06d938ec      str   x6, [x25]
|  | :|||   ; CODE XREF from fcn.06d934f8 @ 0x6d938e0
|  `------> 0x06d938f0      str   x6, [x0, 8]
|    :|||   0x06d938f4      ldr   x1, [arg_128h]                       ; [0x128:4]=0x779b960
|    :|||   0x06d938f8      str   x1, [x0, 0x10]
|    :|||   0x06d938fc      adrp  x27, 0xb48f000
|    :|||   0x06d93900      ldr   w7, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d93904      cbz   w7, 0x6d93944
|   |:|||   0x06d93908      str   x0, [arg_310h]
|   |:|||   0x06d9390c      ldr   x3, [x0, 0x60]                       ; [0x60:4]=0x380
|   |:|||   0x06d93910      bl    0x4da3520
|   |:|||   0x06d93914      ldr   x4, [arg_1a8h]                       ; [0x1a8:4]=0x799bde0 segment.DYNAMIC
|   |:|||   0x06d93918      ldr   x5, [arg_4d0h]                       ; [0x4d0:4]=0x58585858
|   |:|||   0x06d9391c      stp   x4, x5, [x25]
|   |:|||   0x06d93920      ldr   x4, [arg_4c0h]                       ; [0x4c0:4]=0x58585858
|   |:|||   0x06d93924      stp   x4, x3, [x25, 0x10]
|   |:|||   0x06d93928      add   x1, x0, 0x28
|   |:|||   0x06d9392c      adrp  x0, 0x8b70000
|   |:|||   0x06d93930      add   x0, x0, 0x778
|   |:|||   0x06d93934      add   x2, arg_508h
|   |:|||   0x06d93938      bl    0x4d35930
|   |:|||   0x06d9393c      ldr   x0, [arg_310h]                       ; [0x310:4]=0x10000
|   |:|||   0x06d93940      ldr   x1, [arg_128h]                       ; [0x128:4]=0x779b960
|   `-----> 0x06d93944      ldr   x2, [arg_1a8h]                       ; [0x1a8:4]=0x799bde0 segment.DYNAMIC
|    :|||   0x06d93948      ldr   x6, [arg_4d0h]                       ; [0x4d0:4]=0x58585858
|    :|||   0x06d9394c      stp   x2, x6, [x0, 0x18]
|    :|||   0x06d93950      add   x27, arg_508h
|    :|||   0x06d93954      ldp   q0, q1, [x27]
|    :|||   0x06d93958      add   x27, arg_520h
|    :|||   0x06d9395c      ldp   q2, q3, [x27]
|    :|||   0x06d93960      add   x27, x0, 0x28
|    :|||   0x06d93964      stp   q0, q1, [x27]
|    :|||   0x06d93968      stp   q2, q3, [x0, 0x40]
|    :|||   0x06d9396c      ldr   x6, [arg_4c0h]                       ; [0x4c0:4]=0x58585858
|    :|||   0x06d93970      str   x6, [x0, 0x60]
|    :|||   0x06d93974      adrp  x6, 0x6743000
|    :|||   0x06d93978      add   x6, x6, 0x100
|    :|||   0x06d9397c      add   x27, arg_368h
|    :|||   0x06d93980      stp   x6, x0, [x27]
|    :|||   0x06d93984      add   x6, arg_390h
|    :|||   0x06d93988      orr   x7, xzr, 3
|   .-----> 0x06d9398c      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d93990      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d93994      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d93998      stp   xzr, xzr, [x6], 0x10
|   ::|||   0x06d9399c      sub   x7, x7, 1
|   `=====< 0x06d939a0      cbnz  x7, 0x6d9398c
|    :|||   0x06d939a4      adrp  x6, 0x8f92000
|    :|||   0x06d939a8      add   x6, x6, 0xc28
|    :|||   0x06d939ac      add   x7, arg_368h
|    :|||   0x06d939b0      add   x27, arg_450h
|    :|||   0x06d939b4      stp   x6, x7, [x27]
|    :|||   0x06d939b8      ldr   x0, [arg_278h]                       ; [0x278:4]=0x9019dd0
|    :|||   0x06d939bc      add   x3, arg_450h
|    :|||   0x06d939c0      orr   x4, xzr, 2
|    :|||   0x06d939c4      mov   x5, x4
|    :|||   0x06d939c8      bl    0x672b7f0
|    :|||   0x06d939cc      add   x6, arg_390h
|    :|||   0x06d939d0      add   x7, arg_8h
|    :|||   0x06d939d4      orr   x24, xzr, 3
|   .-----> 0x06d939d8      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d939dc      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d939e0      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d939e4      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d939e8      sub   x24, x24, 1
|   `=====< 0x06d939ec      cbnz  x24, 0x6d939d8
|    :|||   0x06d939f0      add   x6, arg_1b8h
|    :|||   0x06d939f4      add   x7, arg_390h
|    :|||   0x06d939f8      orr   x24, xzr, 3
|   .-----> 0x06d939fc      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d93a00      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d93a04      ldp   q16, q17, [x7], 0x20
|   ::|||   0x06d93a08      stp   q16, q17, [x6], 0x20
|   ::|||   0x06d93a0c      sub   x24, x24, 1
|   `=====< 0x06d93a10      cbnz  x24, 0x6d939fc
|   ,=====< 0x06d93a14      cbz   x0, 0x6d93b20
|   |:|||   0x06d93a18      str   x1, [arg_198h]
|   |:|||   0x06d93a1c      str   x0, [arg_120h]
|   |:|||   0x06d93a20      adrp  x27, 0x9491000
|   |:|||   0x06d93a24      add   x27, x27, 0x210
|   |:|||   0x06d93a28      ldp   x2, x3, [x27]
|   |:|||   0x06d93a2c      bl    0x5116df0
|  ,======< 0x06d93a30      tbz   w0, 0, 0x6d93b38
|  ||:|||   0x06d93a34      ldr   x3, [arg_9f8h]                       ; [0x518:4]=0x58585858
| ,=======< 0x06d93a38      cbz   x3, 0x6d93a44
| |||:|||   0x06d93a3c      ldr   x3, [x3, 8]                          ; [0x8:4]=0
| ========< 0x06d93a40      b     0x6d93a48
| `-------> 0x06d93a44      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a40
| ,=======< 0x06d93a48      cbz   x3, 0x6d93a70
| |||:|||   0x06d93a4c      ldp   x3, x4, [x3, 8]
| |||:|||   0x06d93a50      adrp  x5, 0x8f0d000
| |||:|||   0x06d93a54      add   x5, x5, 0x9c8
| |||:|||   0x06d93a58      cmp   x3, x5
| ========< 0x06d93a5c      b.eq  0x6d93a64
| |||:|||   0x06d93a60      mov   x4, xzr
| --------> 0x06d93a64      b.ne  0x6d93a70
| |||:|||   0x06d93a68      ldr   x3, [x4]
| ========< 0x06d93a6c      b     0x6d93a74
| `-------> 0x06d93a70      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a6c
| ,=======< 0x06d93a74      cbz   x3, 0x6d93a80
| |||:|||   0x06d93a78      ldr   x3, [x3, 0xe0]                       ; [0xe0:4]=0
| ========< 0x06d93a7c      b     0x6d93a84
| `-------> 0x06d93a80      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a7c
| ,=======< 0x06d93a84      cbz   x3, 0x6d93a90
| |||:|||   0x06d93a88      ldrsw x3, [x3, 8]                          ; [0x8:4]=0
| ========< 0x06d93a8c      b     0x6d93a94
| `-------> 0x06d93a90      mov   x3, xzr
|  ||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93a8c
| --------> 0x06d93a94      cmp   w3, 1
| ,=======< 0x06d93a98      b.ne  0x6d93b38
| |||:|||   0x06d93a9c      orr   x0, xzr, 0x10
| |||:|||   0x06d93aa0      adrp  x1, 0x81a5000
| |||:|||   0x06d93aa4      add   x1, x1, 0x4f8
| |||:|||   0x06d93aa8      orr   x2, xzr, 1
| |||:|||   0x06d93aac      bl    0x4d32500
| |||:|||   0x06d93ab0      ldr   x1, [arg_120h]                       ; [0x120:4]=7
| ========< 0x06d93ab4      cbz   x1, 0x6d93ac0
| |||:|||   0x06d93ab8      ldr   x11, [x1, 8]                         ; [0x8:4]=0
| ========< 0x06d93abc      b     0x6d93ac4
| --------> 0x06d93ac0      mov   x11, x1
| |||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93abc
| --------> 0x06d93ac4      str   x11, [x0]
| |||:|||   0x06d93ac8      adrp  x27, 0xb48f000
| |||:|||   0x06d93acc      ldr   w11, [x27, 0x450]                    ; [0x450:4]=0x58585858
| ========< 0x06d93ad0      cbnz  w11, 0x6d93adc
| |||:|||   0x06d93ad4      ldr   x11, [arg_198h]                      ; [0x198:4]=0x779bde0
| ========< 0x06d93ad8      b     0x6d93ae8
| --------> 0x06d93adc      bl    0x4da34f0
| |||:|||   0x06d93ae0      ldr   x11, [arg_198h]                      ; [0x198:4]=0x779bde0
| |||:|||   0x06d93ae4      str   x11, [x25]
| |||:|||   ; CODE XREF from fcn.06d934f8 @ 0x6d93ad8
| --------> 0x06d93ae8      str   x11, [x0, 8]
| |||:|||   0x06d93aec      ldr   x1, [arg_4f8h]                       ; [0x4f8:4]=0x58585858
| |||:|||   0x06d93af0      orr   x2, xzr, 1
| |||:|||   0x06d93af4      orr   x3, xzr, 1
| |||:|||   0x06d93af8      mov   x4, xzr
| |||:|||   0x06d93afc      mov   x5, xzr
| |||:|||   0x06d93b00      adrp  x6, 0x4479000
| |||:|||   0x06d93b04      add   x6, x6, 0x8ad
| |||:|||   0x06d93b08      orr   x7, xzr, 0x3c
| |||:|||   0x06d93b0c      mov   x8, x0
| |||:|||   0x06d93b10      mov   x9, x2
| |||:|||   0x06d93b14      mov   x10, x2
| |||:|||   0x06d93b18      ldr   x0, [arg_4f0h]                       ; [0x4f0:4]=0x58585858
| |||:|||   0x06d93b1c      bl    0x4f6f980
| ||`-----> 0x06d93b20      ldr   x4, [arg_4c0h]                       ; [0x4c0:4]=0x58585858
| || :|||   0x06d93b24      ldp   x4, x5, [x4]
| || :|||   0x06d93b28      str   x5, [arg_168h]
| || :|||   0x06d93b2c      str   x4, [arg_308h]
| || :|||   0x06d93b30      mov   x6, xzr
| ||,=====< 0x06d93b34      b     0x6d93b70
| ``------> 0x06d93b38      ldr   x0, [arg_120h]                       ; [0x120:4]=7
|   |:|||   0x06d93b3c      ldr   x1, [arg_198h]                       ; [0x198:4]=0x779bde0
|   |:|||   0x06d93b40      ldp   fp, lr, [sp, -8]
|   |:|||   0x06d93b44      add   sp, sp, 0x4e0
|   |:|||   0x06d93b48      ret
|   |:||`-> 0x06d93b4c      mov   x0, x1
|   |:||    0x06d93b50      mov   x1, x2
|   |:||    0x06d93b54      ldp   fp, lr, [sp, -8]
|   |:||    0x06d93b58      add   sp, sp, 0x4e0
|   |:||    0x06d93b5c      ret
|   |:||.-> 0x06d93b60      ldr   x7, [arg_158h]                       ; [0x158:4]=0x6474e552
|   |:||:   0x06d93b64      add   x6, x7, 1
|   |:||:   0x06d93b68      ldr   x4, [arg_308h]                       ; [0x308:4]=0x50d0
|   |:||:   0x06d93b6c      ldr   x5, [arg_168h]                       ; [0x168:4]=0x799b960 segment.GNU_RELRO
|   |:||:   ; CODE XREF from fcn.06d934f8 @ 0x6d93b34
|   `-----> 0x06d93b70      cmp   x6, x5
|   ,=====< 0x06d93b74      b.ge  0x6d93ca0
|   |:||:   0x06d93b78      str   x6, [arg_158h]
|   |:||:   0x06d93b7c      ldr   x7, [x4, x6, lsl 3]
|   |:||:   0x06d93b80      str   x7, [arg_188h]
|   |:||:   0x06d93b84      ldp   x8, x9, [x7, 0x28]
|   |:||:   0x06d93b88      str   x9, [arg_150h]
|   |:||:   0x06d93b8c      str   x8, [arg_300h]
|   |:||:   0x06d93b90      mov   x0, xzr
|  ,======< 0x06d93b94      b     0x6d93ba8
|  ||:||:   ; CODE XREF from fcn.06d934f8 @ 0x6d93c9c
| .-------> 0x06d93b98      ldr   x4, [arg_148h]                       ; [0x148:4]=0x458
| :||:||:   0x06d93b9c      add   x0, x4, 1
| :||:||:   0x06d93ba0      ldr   x8, [arg_300h]                       ; [0x300:4]=0x50d0
| :||:||:   0x06d93ba4      ldr   x9, [arg_150h]                       ; [0x150:4]=32
| :||:||:   ; CODE XREF from fcn.06d934f8 @ 0x6d93b94
| :`------> 0x06d93ba8      cmp   x0, x9
| : |:||`=< 0x06d93bac      b.ge  0x6d93b60
| : |:||    0x06d93bb0      str   x0, [arg_148h]
| : |:||    0x06d93bb4      ldr   x4, [x8, x0, lsl 3]
| : |:||    0x06d93bb8      str   x4, [arg_180h]
| : |:||    0x06d93bbc      adrp  x27, 0x948b000
| : |:||    0x06d93bc0      ldr   x26, [x27, 0x830]                    ; [0x830:4]=0x58585858
| : |:||    0x06d93bc4      ldr   x5, [x26]
| : |:||    0x06d93bc8      ldp   x0, x1, [x4, 0x58]
| : |:||    0x06d93bcc      ldp   x2, x3, [x4, 8]
| : |:||    0x06d93bd0      blr   x5
| : |:||    0x06d93bd4      ldr   x4, [arg_258h]                       ; [0x258:4]=0xeb030
| : |:||    0x06d93bd8      orr   x5, xzr, 7
| : |:||    0x06d93bdc      str   w5, [arg_378h]
| : |:||    0x06d93be0      add   x27, arg_380h
| : |:||    0x06d93be4      stp   x0, x1, [x27]
| : |:||    0x06d93be8      adrp  x0, 0x8de9000
| : |:||    0x06d93bec      add   x0, x0, 0xc48
| : |:||    0x06d93bf0      mov   x1, x4
| : |:||    0x06d93bf4      add   x2, arg_378h
| : |:||    0x06d93bf8      bl    0x4d16af0
| ========< 0x06d93bfc      tbz   w1, 0, 0x6d93b98
| : |:||    0x06d93c00      ldr   x5, [arg_188h]                       ; [0x188:4]=1
| : |:||    0x06d93c04      ldr   x6, [x5, 8]                          ; [0x8:4]=0
| : |:||    0x06d93c08      ldr   x2, [x6, 0x70]                       ; [0x70:4]=8
| : |:||    0x06d93c0c      ldp   x0, x7, [x6, 0x60]
| : |:||    0x06d93c10      add   x1, x7, 1
| : |:||    0x06d93c14      ldr   x7, [arg_180h]                       ; [0x180:4]=0x16236a0 "ki/codeium_common_pb/codeium_common_go_proto.SearchResultType.String"
| : |:||    0x06d93c18      ldp   x7, x8, [x7, 8]
| : |:||    0x06d93c1c      cmp   x2, x1
| : |:||,=< 0x06d93c20      b.hs  0x6d93c70
| : |:|||   0x06d93c24      str   x8, [arg_140h]
| : |:|||   0x06d93c28      str   x7, [arg_338h]
| : |:|||   0x06d93c2c      str   x6, [arg_2e8h]
| : |:|||   0x06d93c30      orr   x3, xzr, 1
| : |:|||   0x06d93c34      adrp  x4, 0x856f000
| : |:|||   0x06d93c38      add   x4, x4, 0x550
| : |:|||   0x06d93c3c      bl    0x4d9e050
| : |:|||   0x06d93c40      ldr   x5, [arg_2e8h]                       ; [0x2e8:4]=0xacf0000
| : |:|||   0x06d93c44      str   x2, [x5, 0x70]
| : |:|||   0x06d93c48      adrp  x27, 0xb48f000
| : |:|||   0x06d93c4c      ldr   w6, [x27, 0x450]                     ; [0x450:4]=0x58585858
| :,======< 0x06d93c50      cbz   w6, 0x6d93c60
| :||:|||   0x06d93c54      ldr   x4, [x5, 0x60]                       ; [0x60:4]=0x380
| :||:|||   0x06d93c58      bl    0x4da3500
| :||:|||   0x06d93c5c      stp   x0, x4, [x25]
| :`------> 0x06d93c60      str   x0, [x5, 0x60]
| : |:|||   0x06d93c64      mov   x6, x5
| : |:|||   0x06d93c68      ldr   x7, [arg_338h]                       ; [0x338:4]=27
| : |:|||   0x06d93c6c      ldr   x8, [arg_140h]                       ; [0x140:4]=144
| : |:||`-> 0x06d93c70      str   x1, [x6, 0x68]
| : |:||    0x06d93c74      sub   x4, x1, 1
| : |:||    0x06d93c78      add   x4, x0, x4, lsl 4
| : |:||    0x06d93c7c      str   x8, [x4, 8]
| : |:||    0x06d93c80      adrp  x27, 0xb48f000
| : |:||    0x06d93c84      ldr   w6, [x27, 0x450]                     ; [0x450:4]=0x58585858
| : |:||,=< 0x06d93c88      cbz   w6, 0x6d93c98
| : |:|||   0x06d93c8c      ldr   x6, [x4]
| : |:|||   0x06d93c90      bl    0x4da3500
| : |:|||   0x06d93c94      stp   x7, x6, [x25]
| : |:||`-> 0x06d93c98      str   x7, [x4]
| `=======< 0x06d93c9c      b     0x6d93b98
|   `-----> 0x06d93ca0      adrp  x27, 0x948b000
|    :||    0x06d93ca4      ldr   x0, [x27, 0xa18]                     ; [0xa18:4]=0x58585858
|    :||    0x06d93ca8      nop
|    :||    0x06d93cac      adrp  x1, 0x79e9000
|    :||    0x06d93cb0      add   x1, x1, 0xc80
|    :||    0x06d93cb4      ldr   x2, [arg_500h]                       ; [0x500:4]=0x58585858
|    :||    0x06d93cb8      ldr   x3, [arg_4d0h]                       ; [0x4d0:4]=0x58585858
|    :||    0x06d93cbc      bl    0x6786750
|    :||    0x06d93cc0      adrp  x0, 0x89d0000
|    :||    0x06d93cc4      add   x0, x0, 0xb30
|    :||    0x06d93cc8      bl    0x4d31c10
|    :||    0x06d93ccc      ldr   x4, [arg_4c8h]                       ; [0x4c8:4]=0x58585858
|    :||    0x06d93cd0      ldr   x4, [x4]
|    :||    0x06d93cd4      add   x1, x0, 0x10
|    :||    0x06d93cd8      adrp  x27, 0xb48f000
|    :||    0x06d93cdc      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    :||,=< 0x06d93ce0      cbz   w5, 0x6d93d18
|    :|||   0x06d93ce4      str   x0, [arg_1b0h]
|    :|||   0x06d93ce8      str   x4, [arg_2f8h]
|    :|||   0x06d93cec      str   x1, [arg_2f0h]
|    :|||   0x06d93cf0      bl    0x4da3500
|    :|||   0x06d93cf4      ldr   x3, [arg_278h]                       ; [0x278:4]=0x9019dd0
|    :|||   0x06d93cf8      stp   x4, x3, [x25]
|    :|||   0x06d93cfc      adrp  x0, 0x8dec000
|    :|||   0x06d93d00      add   x0, x0, 0x128
|    :|||   0x06d93d04      add   x2, arg_1b8h
|    :|||   0x06d93d08      bl    0x4d35930
|    :|||   0x06d93d0c      ldr   x0, [arg_1b0h]                       ; [0x1b0:4]=0x220
|    :|||   0x06d93d10      ldr   x1, [arg_2f0h]                       ; [0x2f0:4]=0xd8f0000 segment.LOAD6
|    :|||   0x06d93d14      ldr   x4, [arg_2f8h]                       ; [0x2f8:4]=0xd8f0000 segment.LOAD6
|    :||`-> 0x06d93d18      ldr   x5, [arg_278h]                       ; [0x278:4]=0x9019dd0
|    :||    0x06d93d1c      stp   x4, x5, [x0]
|    :||    0x06d93d20      add   x4, arg_1b8h
|    :||    0x06d93d24      orr   x24, xzr, 3
|    :||.-> 0x06d93d28      ldp   q16, q17, [x4], 0x20
|    :||:   0x06d93d2c      stp   q16, q17, [x1], 0x20
|    :||:   0x06d93d30      ldp   q16, q17, [x4], 0x20
|    :||:   0x06d93d34      stp   q16, q17, [x1], 0x20
|    :||:   0x06d93d38      sub   x24, x24, 1
|    :||`=< 0x06d93d3c      cbnz  x24, 0x6d93d28
|    :||    0x06d93d40      adrp  x27, 0x948b000
|    :||    0x06d93d44      ldr   x4, [x27, 0xa20]                     ; [0xa20:4]=0x58585858
|    :||    0x06d93d48      adrp  x1, 0x79e9000
|    :||    0x06d93d4c      add   x1, x1, 0xc20
|    :||    0x06d93d50      ldr   x2, [arg_500h]                       ; [0x500:4]=0x58585858
|    :||    0x06d93d54      mov   x3, x0
|    :||    0x06d93d58      mov   x0, x4
|    :||    0x06d93d5c      bl    0x6786750
|    :||    0x06d93d60      mov   x0, xzr
|    :||    0x06d93d64      mov   x1, xzr
|    :||    0x06d93d68      ldp   fp, lr, [sp, -8]
|    :||    0x06d93d6c      add   sp, sp, 0x4e0
|    :||    0x06d93d70      ret
|    :||    ; CODE XREFS from fcn.06d934f8 @ 0x6d93dbc, 0x6d93dc4
|   .--`.-> 0x06d93d74      ldr   x10, [arg_4e8h]                      ; [0x4e8:4]=0x58585858
|   ::| :   0x06d93d78      mov   x9, x0
|   ::| :   0x06d93d7c      mov   x11, x1
|   :`====< 0x06d93d80      b     0x6d93598
|   : | :   ; CODE XREF from fcn.06d934f8 @ 0x6d93560
|   : `.--> 0x06d93d84      and   x10, x9, x8
|   :  ::   0x06d93d88      lsl   x10, x10, 4
|   :  ::   0x06d93d8c      add   x10, x10, 8
|   :  ::   0x06d93d90      ldr   x11, [x7, x10]                       ; arg8
|   :  ::   0x06d93d94      add   x10, x7, x10                         ; arg8
|   :  ::   0x06d93d98      cmp   x11, x5
|   : ,===< 0x06d93d9c      b.eq  0x6d93dc0
|   : |::   0x06d93da0      add   x9, x9, 1
|   : |`==< 0x06d93da4      cbnz  x11, 0x6d93d84
|   : | :   0x06d93da8      str   x1, [arg_190h]
|   : | :   0x06d93dac      mov   x0, x6
|   : | :   0x06d93db0      mov   x1, x5
|   : | :   0x06d93db4      bl    0x4d2e4c0
|   : | :   0x06d93db8      ldr   x1, [arg_190h]                       ; [0x190:4]=2
|   `=====< 0x06d93dbc      b     0x6d93d74
|     `---> 0x06d93dc0      ldr   x0, [x10, 8]                         ; [0x8:4]=0
\       `=< 0x06d93dc4      b     0x6d93d74
==== google3_third_party_jetski_gateway_gateway.NewAuthProviderWithOptions
/ fcn.06d93dc8(int64_t arg_feh, int64_t arg_ffh, int64_t arg_100h, int64_t arg_108h, int64_t arg_168h, int64_t arg_170h, int64_t arg_178h, int64_t arg_338h, int64_t arg_348h, int64_t arg_460h, int64_t arg_4d0h, int64_t arg_4e8h, int64_t arg_508h, int64_t arg_518h, int64_t arg_9f8h);
|       :   ; arg int64_t arg_feh @ stack + 0xfe
|       :   ; arg int64_t arg_ffh @ stack + 0xff
|       :   ; arg int64_t arg_100h @ stack + 0x100
|       :   ; arg int64_t arg_108h @ stack + 0x108
|       :   ; arg int64_t arg_168h @ stack + 0x168
|       :   ; arg int64_t arg_170h @ stack + 0x170
|       :   ; arg int64_t arg_178h @ stack + 0x178
|       :   ; arg int64_t arg_338h @ stack + 0x338
|       :   ; arg int64_t arg_348h @ stack + 0x348
|       :   ; arg int64_t arg_460h @ stack + 0x460
|       :   ; arg int64_t arg_4d0h @ stack + 0x4d0
|       :   ; arg int64_t arg_4e8h @ stack + 0x4e8
|       :   ; arg int64_t arg_508h @ stack + 0x508
|       :   ; arg int64_t arg_518h @ stack + 0x518
|       :   ; arg int64_t arg_9f8h @ stack + 0x9f8
|       :   0x06d93dc8      adrp  x1, 0x7ee6000
|       :   0x06d93dcc      add   x1, x1, 0x338
|       :   0x06d93dd0      adrp  x2, 0x86af000
|       :   0x06d93dd4      add   x2, x2, 0x668
|       :   0x06d93dd8      bl    0x4d2e0c0
|       :   ; CODE XREF from fcn.06d93dc8 @ 0x6d93ea4
|     ..--> 0x06d93ddc      str   x5, [x2]
|     :::   0x06d93de0      add   x0, arg_460h
|     :::   0x06d93de4      bl    fcn.04d35870
|     :::   0x06d93de8      ldrb  w0, [arg_feh]                        ; [0xfe:4]=0xb9600000
|     :::   0x06d93dec      ldp   x2, x1, [arg_108h]
|     :::   0x06d93df0      ldr   x3, [arg_178h]                       ; [0x178:4]=0x1622c30 "ogle3/third_party/jetski/codeium_common_pb/codeium_common_go_proto.FilterReason.Descriptor"
|     :::   0x06d93df4      ldr   x5, [arg_460h]                       ; [0x460:4]=0x58585858
|    ,====< 0x06d93df8      cbz   x5, 0x6d93ea8
|    |:::   0x06d93dfc      ldp   x5, x6, [x5]
|    |:::   0x06d93e00      add   x7, x2, 1
|    |:::   0x06d93e04      cmp   x1, x7
|   ,=====< 0x06d93e08      b.hs  0x6d93e74
|   ||:::   0x06d93e0c      str   x5, [arg_338h]
|   ||:::   0x06d93e10      str   x6, [arg_168h]
|   ||:::   0x06d93e14      strb  w0, [arg_ffh]
|   ||:::   0x06d93e18      cmp   x7, 2
|  ,======< 0x06d93e1c      b.gt  0x6d93e40
| ,=======< 0x06d93e20      tbnz  w0, 0, 0x6d93e40
| ========< 0x06d93e24      cbnz  x2, 0x6d93e40
| ||||:::   0x06d93e28      add   x3, arg_348h
| ||||:::   0x06d93e2c      stp   xzr, xzr, [x3]
| ||||:::   0x06d93e30      stp   xzr, xzr, [x3, 0x10]
| ||||:::   0x06d93e34      orr   x1, xzr, 2
| ||||:::   0x06d93e38      orr   x0, xzr, 1
| ========< 0x06d93e3c      b     0x6d93e74
| ``------> 0x06d93e40      mov   x0, x3
|   ||:::   0x06d93e44      mov   x2, x1
|   ||:::   0x06d93e48      orr   x3, xzr, 1
|   ||:::   0x06d93e4c      adrp  x4, 0x856f000
|   ||:::   0x06d93e50      add   x4, x4, 0x550
|   ||:::   0x06d93e54      mov   x1, x7
|   ||:::   0x06d93e58      bl    0x4d9e050
|   ||:::   0x06d93e5c      ldr   x5, [arg_338h]                       ; [0x338:4]=27
|   ||:::   0x06d93e60      ldr   x6, [arg_168h]                       ; [0x168:4]=0x799b960 segment.GNU_RELRO
|   ||:::   0x06d93e64      mov   x7, x1
|   ||:::   0x06d93e68      mov   x3, x0
|   ||:::   0x06d93e6c      mov   x1, x2
|   ||:::   0x06d93e70      ldrb  w0, [arg_ffh]                        ; [0xff:4]=0x99b96000
|   ||:::   ; CODE XREF from fcn.06d93dc8 @ 0x6d93e3c
| --`-----> 0x06d93e74      str   x3, [arg_178h]
|    |:::   0x06d93e78      stp   x7, x1, [arg_108h]
|    |:::   0x06d93e7c      strb  w0, [arg_feh]
|    |:::   0x06d93e80      sub   x2, x7, 1
|    |:::   0x06d93e84      add   x2, x3, x2, lsl 4
|    |:::   0x06d93e88      str   x6, [x2, 8]
|    |:::   0x06d93e8c      adrp  x27, 0xb48f000
|    |:::   0x06d93e90      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    |`===< 0x06d93e94      cbz   w4, 0x6d93ddc
|    | ::   0x06d93e98      ldr   x4, [x2]
|    | ::   0x06d93e9c      bl    0x4da3500
|    | ::   0x06d93ea0      stp   x5, x4, [x25]
|    | `==< 0x06d93ea4      b     0x6d93ddc
|    `----> 0x06d93ea8      ldr   x4, [arg_4d0h]                       ; [0x4d0:4]=0x58585858
|       :   0x06d93eac      ldr   x0, [x4, 0x60]                       ; [0x60:4]=0x380
|       :   0x06d93eb0      mov   x4, x1
|       :   0x06d93eb4      mov   x1, x3
|       :   0x06d93eb8      mov   x3, x4
|       :   0x06d93ebc      bl    0x628f290
|       :   0x06d93ec0      ldr   x1, [arg_4e8h]                       ; [0x4e8:4]=0x58585858
|       `=< 0x06d93ec4      b     0x6d93390
==== google3_third_party_jetski_gateway_gateway.NewAuthProviderFromSettings
/ fcn.06d94188(int64_t arg3, int64_t arg4, int64_t arg5, int64_t arg6, int64_t arg7, int64_t arg_50h);
|           ; arg int64_t arg3 @ x2
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg5 @ x4
|           ; arg int64_t arg6 @ x5
|           ; arg int64_t arg7 @ x6
|           ; arg int64_t arg_50h @ stack + 0x50
|       ,=< 0x06d94188      b     0x6d94190
..
|       |   ; CODE XREF from fcn.06d94188 @ 0x6d94188
|      ,`-> 0x06d94190      cbz   x7, 0x6d941c0
|      |    0x06d94194      mov   x8, x2
|      |    0x06d94198      mov   x2, x4                               ; arg5
|      |    0x06d9419c      mov   x9, x3
|      |    0x06d941a0      mov   x3, x5                               ; arg6
|      |    0x06d941a4      mov   x4, x6                               ; arg7
|      |    0x06d941a8      mov   x5, x8
|      |    0x06d941ac      mov   x6, x9
|      |    0x06d941b0      bl    fcn.06a092a0
|      |    0x06d941b4      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |    0x06d941b8      ldr   lr, [sp], 0x50                       ; [0x50:4]=64
|      |    0x06d941bc      ret
|      `--> 0x06d941c0      mov   x0, x2                               ; arg3
|           0x06d941c4      mov   x1, x3                               ; arg4
|           0x06d941c8      mov   x2, xzr
|           0x06d941cc      mov   x3, xzr
|           0x06d941d0      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d941d4      ldr   lr, [sp], 0x50                       ; [0x50:4]=64
\           0x06d941d8      ret
==== AuthProvider_.AdminSettings
/ fcn.06d94288(int64_t arg_70h);
|           ; arg int64_t arg_70h @ stack + 0x70
|           0x06d94288      mov   x11, xzr
|           0x06d9428c      bl    fcn.06704710
|           0x06d94290      mov   x2, xzr
|           0x06d94294      mov   x3, xzr
|           0x06d94298      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d9429c      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
\           0x06d942a0      ret
==== AuthProvider_.setLastDiscoveredCatalog
/ fcn.06d94388(int64_t arg1, int64_t arg_b8h, int64_t arg_f0h, int64_t arg_f8h, int64_t arg_108h, int64_t arg_118h, int64_t arg_128h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg_b8h @ stack + 0xb8
|           ; arg int64_t arg_f0h @ stack + 0xf0
|           ; arg int64_t arg_f8h @ stack + 0xf8
|           ; arg int64_t arg_108h @ stack + 0x108
|           ; arg int64_t arg_118h @ stack + 0x118
|           ; arg int64_t arg_128h @ stack + 0x128
|           0x06d94388      str   x8, [x25]
|           0x06d9438c      adrp  x8, 0x8f93000
|           0x06d94390      add   x8, x8, 0x850
|           0x06d94394      str   x8, [x0]                             ; arg1
|           0x06d94398      ldp   x0, x1, [arg_f8h]
|           0x06d9439c      ldp   x2, x3, [arg_108h]
|           0x06d943a0      ldp   x4, x5, [arg_118h]
|           0x06d943a4      ldr   x6, [arg_128h]                       ; [0x128:4]=0x779b960
|           0x06d943a8      add   x7, arg_b8h
|           0x06d943ac      bl    fcn.0697cd40
|           0x06d943b0      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d943b4      ldr   lr, [sp], 0xf0                       ; [0xf0:4]=0x779b960
\           0x06d943b8      ret
==== AuthProvider_.getLastDiscoveredCatalog
==== google3_third_party_jetski_gateway_gateway.splitHeaderEntries
/ fcn.06d94548(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg_58h, int64_t arg_70h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg2 @ x1
|           ; arg int64_t arg3 @ x2
|           ; arg int64_t arg_58h @ stack + 0x58
|           ; arg int64_t arg_70h @ stack + 0x70
|           0x06d94548      stp   x0, x2, [x25]                        ; arg3
|           0x06d9454c      str   x0, [x1]                             ; arg2
|           0x06d94550      bl    fcn.06297e40
|           0x06d94554      ldr   x3, [arg_58h]                        ; [0x58:4]=64
|           0x06d94558      stp   x1, x2, [x3, 8]
|           0x06d9455c      adrp  x27, 0xb48f000
|           0x06d94560      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d94564      cbz   w4, 0x6d94574
|       |   0x06d94568      ldr   x4, [x3]
|       |   0x06d9456c      bl    fcn.04da3500
|       |   0x06d94570      stp   x0, x4, [x25]
|       `-> 0x06d94574      str   x0, [x3]
|           0x06d94578      bl    fcn.0629d230
|           0x06d9457c      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d94580      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
\           0x06d94584      ret
==== google3_third_party_jetski_gateway_gateway.ParseCustomHeaders
/ fcn.06d94998(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg4, int64_t arg5, int64_t arg_8h, int64_t arg_10h, int64_t arg_18h, int64_t arg_20h, int64_t arg_28h, int64_t arg_50h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; arg int64_t arg2 @ x1
|       :   ; arg int64_t arg3 @ x2
|       :   ; arg int64_t arg4 @ x3
|       :   ; arg int64_t arg5 @ x4
|       :   ; var int64_t var_58h @ stack - 0x58
|       :   ; var int64_t var_50h @ stack - 0x50
|       :   ; var int64_t var_48h @ stack - 0x48
|       :   ; var int64_t var_40h @ stack - 0x40
|       :   ; var int64_t var_38h @ stack - 0x38
|       :   ; var int64_t var_30h @ stack - 0x30
|       :   ; var int64_t var_28h @ stack - 0x28
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_10h @ stack + 0x10
|       :   ; arg int64_t arg_18h @ stack + 0x18
|       :   ; arg int64_t arg_20h @ stack + 0x20
|       :   ; arg int64_t arg_28h @ stack + 0x28
|       :   ; arg int64_t arg_50h @ stack + 0x50
|       :   0x06d94998      cmp   x17, x16
|      ,==< 0x06d9499c      b.ls  0x6d94c14
|      |:   0x06d949a0      str   lr, [sp, -0xa0]!
|      |:   0x06d949a4      stur  fp, [sp, -8]
|      |:   0x06d949a8      sub   fp, sp, 8
|      |:   0x06d949ac      stp   x1, x2, [arg_10h]
|      |:   0x06d949b0      str   x0, [arg_8h]
|      |:   0x06d949b4      stp   x3, x4, [arg_20h]
|      |:   0x06d949b8      adrp  x27, 0x9583000
|      |:   0x06d949bc      add   x27, x27, 0x520
|      |:   0x06d949c0      ldp   x1, x0, [x27]
|      |:   0x06d949c4      ldr   x1, [x1, 0x90]                       ; [0x90:4]=0
|      |:   0x06d949c8      blr   x1
|      |:   0x06d949cc      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949d0      mov   x0, x1
|      |:   0x06d949d4      adrp  x1, 0x42a6000
|      |:   0x06d949d8      add   x1, x1, 0xe15
|      |:   0x06d949dc      mov   x3, x2
|      |:   0x06d949e0      mov   x2, 0xa
|      |:   0x06d949e4      blr   x3
|      |:   0x06d949e8      ldr   x2, [x0, 0x30]                       ; [0x30:4]=0
|      |:   0x06d949ec      mov   x0, x1
|      |:   0x06d949f0      blr   x2
|      |:   0x06d949f4      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949f8      mov   x0, x1
|      |:   0x06d949fc      adrp  x1, 0x4237000
|      |:   0x06d94a00      add   x1, x1, 0x7c0
|      |:   0x06d94a04      mov   x3, x2
|      |:   0x06d94a08      orr   x2, xzr, 3
|      |:   0x06d94a0c      blr   x3
|      |:   0x06d94a10      str   x1, [var_58h]
|     ,===< 0x06d94a14      cbz   x0, 0x6d94a1c
|     ||:   0x06d94a18      ldr   x0, [x0, 8]                          ; [0x8:4]=0
|     `---> 0x06d94a1c      str   x0, [var_30h]
|      |:   0x06d94a20      orr   x0, xzr, 0x10
|      |:   0x06d94a24      adrp  x1, 0x8744000
|      |:   0x06d94a28      add   x1, x1, 0xba0
|      |:   0x06d94a2c      orr   x2, xzr, 1
|      |:   0x06d94a30      bl    0x4d32500
|      |:   0x06d94a34      ldr   x3, [var_30h]                        ; [0x70:4]=8
|      |:   0x06d94a38      str   x3, [x0]
|      |:   0x06d94a3c      adrp  x27, 0xb48f000
|      |:   0x06d94a40      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a44      cbnz  w3, 0x6d94a50
|     ||:   0x06d94a48      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    ,====< 0x06d94a4c      b     0x6d94a5c
|    |`---> 0x06d94a50      bl    0x4da34f0
|    | |:   0x06d94a54      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    | |:   0x06d94a58      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a4c
|    `----> 0x06d94a5c      str   x0, [var_38h]
|      |:   0x06d94a60      str   x3, [x0, 8]
|      |:   0x06d94a64      nop
|      |:   0x06d94a68      orr   x0, xzr, 0x18
|      |:   0x06d94a6c      adrp  x1, 0x8744000
|      |:   0x06d94a70      add   x1, x1, 0xc18
|      |:   0x06d94a74      orr   x2, xzr, 1
|      |:   0x06d94a78      bl    0x4d32830
|      |:   0x06d94a7c      ldp   x3, x4, [arg_20h]
|      |:   0x06d94a80      stp   x3, x4, [x0, 8]
|      |:   0x06d94a84      adrp  x27, 0xb48f000
|      |:   0x06d94a88      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a8c      cbnz  w3, 0x6d94a98
|     ||:   0x06d94a90      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    ,====< 0x06d94a94      b     0x6d94aa4
|    |`---> 0x06d94a98      bl    0x4da34f0
|    | |:   0x06d94a9c      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    | |:   0x06d94aa0      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a94
|    `----> 0x06d94aa4      str   x0, [var_40h]
|      |:   0x06d94aa8      str   x3, [x0]
|      |:   0x06d94aac      ldr   x3, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ab0      ldrsb x27, [x3]
|      |:   0x06d94ab4      orr   x0, xzr, 0x18
|      |:   0x06d94ab8      adrp  x1, 0x87fc000
|      |:   0x06d94abc      add   x1, x1, 0x498
|      |:   0x06d94ac0      orr   x2, xzr, 1
|      |:   0x06d94ac4      bl    0x4d32830
|      |:   0x06d94ac8      nop
|      |:   0x06d94acc      adr   x3, 0x6d95f00
|      |:   0x06d94ad0      ldr   x4, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ad4      stp   x3, x4, [x0]
|      |:   0x06d94ad8      adrp  x27, 0xb48f000
|      |:   0x06d94adc      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94ae0      cbnz  w3, 0x6d94aec
|     ||:   0x06d94ae4      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    ,====< 0x06d94ae8      b     0x6d94af8
|    |`---> 0x06d94aec      bl    0x4da34f0
|    | |:   0x06d94af0      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    | |:   0x06d94af4      str   x7, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94ae8
|    `----> 0x06d94af8      str   x7, [x0, 0x10]
|      |:   0x06d94afc      add   x4, var_28h
|      |:   0x06d94b00      stp   xzr, xzr, [x4]
|      |:   0x06d94b04      stp   xzr, xzr, [x4, 0x10]
|      |:   0x06d94b08      adrp  x7, 0x94a8000
|      |:   0x06d94b0c      add   x7, x7, 0x4c0
|      |:   0x06d94b10      ldar  x8, [x7]
|      |:   0x06d94b14      ldr   x9, [x8]
|      |:   0x06d94b18      adrp  x27, 0x8f54000
|      |:   0x06d94b1c      ldr   w10, [x27, 0x930]                    ; [0x930:4]=0x58585858
|     .---> 0x06d94b20      and   x11, x10, x9
|     :|:   0x06d94b24      lsl   x11, x11, 4
|     :|:   0x06d94b28      add   x11, x11, 8
|     :|:   0x06d94b2c      ldr   x12, [x8, x11]
|     :|:   0x06d94b30      add   x11, x8, x11
|     :|:   0x06d94b34      adrp  x1, 0x7eb5000
|     :|:   0x06d94b38      add   x1, x1, 0xe38
|     :|:   0x06d94b3c      cmp   x12, x1
|    ,====< 0x06d94b40      b.eq  0x6d94b68
|    |:|:   0x06d94b44      add   x10, x10, 1
|    |`===< 0x06d94b48      cbnz  x12, 0x6d94b20
|    | |:   0x06d94b4c      str   x0, [var_48h]
|    | |:   0x06d94b50      mov   x0, x7
|    | |:   0x06d94b54      bl    0x4d2e4c0
|    | |:   0x06d94b58      add   x4, var_28h
|    | |:   0x06d94b5c      mov   x7, x0
|    | |:   0x06d94b60      ldr   x0, [var_48h]                        ; [0x58:4]=64
|    |,===< 0x06d94b64      b     0x6d94b6c
|    `----> 0x06d94b68      ldr   x7, [x11, 8]                         ; [0x8:4]=0
|     ||:   ; CODE XREF from fcn.06d94998 @ 0x6d94b64
|     `---> 0x06d94b6c      ldr   x8, [var_38h]                        ; [0x68:4]=0x380
|      |:   0x06d94b70      stp   x7, x8, [var_28h]
|      |:   0x06d94b74      adrp  x7, 0x8f0a000
|      |:   0x06d94b78      add   x7, x7, 0x408
|      |:   0x06d94b7c      ldr   x8, [var_40h]                        ; [0x60:4]=0x380
|      |:   0x06d94b80      stp   x7, x8, [var_28h + 0x10]
|      |:   0x06d94b84      adrp  x1, 0x43be000
|      |:   0x06d94b88      add   x1, x1, 0xf1e
|      |:   0x06d94b8c      mov   x2, 0x1a
|      |:   0x06d94b90      mov   x3, x0
|      |:   0x06d94b94      orr   x5, xzr, 2
|      |:   0x06d94b98      mov   x6, x5
|      |:   0x06d94b9c      adrp  x0, 0x79ca000
|      |:   0x06d94ba0      add   x0, x0, 0x800
|      |:   0x06d94ba4      bl    0x6d95270
|      |:   0x06d94ba8      str   x0, [var_50h]
|      |:   0x06d94bac      orr   x0, xzr, 0x10
|      |:   0x06d94bb0      adrp  x1, 0x87fc000
|      |:   0x06d94bb4      add   x1, x1, 0x518
|      |:   0x06d94bb8      orr   x2, xzr, 1
|      |:   0x06d94bbc      bl    0x4d32500
|      |:   0x06d94bc0      nop
|      |:   0x06d94bc4      adr   x7, 0x6d94c40
|      |:   0x06d94bc8      str   x7, [x0]
|      |:   0x06d94bcc      adrp  x27, 0xb48f000
|      |:   0x06d94bd0      ldr   w7, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94bd4      cbnz  w7, 0x6d94be0
|     ||:   0x06d94bd8      ldr   x4, [arg_50h]                        ; [0x50:4]=64
|    ,====< 0x06d94bdc      b     0x6d94bec
|    |`---> 0x06d94be0      bl    0x4da34f0
|    | |:   0x06d94be4      ldr   x4, [var_50h]                        ; [0x50:4]=64
|    | |:   0x06d94be8      str   x4, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94bdc
|    `----> 0x06d94bec      str   x4, [x0, 8]
|      |:   0x06d94bf0      mov   x1, 0x17
|      |:   0x06d94bf4      adrp  x2, 0x8f07000
|      |:   0x06d94bf8      add   x2, x2, 0x728
|      |:   0x06d94bfc      mov   x3, x0
|      |:   0x06d94c00      adrp  x0, 0x439b000
|      |:   0x06d94c04      add   x0, x0, 0x4fe
|      |:   0x06d94c08      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |:   0x06d94c0c      ldr   lr, [sp], 0xa0                       ; [0xa0:4]=0x779b960
|      |:   0x06d94c10      ret
|      `--> 0x06d94c14      stp   x0, x1, [arg_8h]                     ; arg2
|       :   0x06d94c18      stp   x2, x3, [arg_18h]                    ; arg4
|       :   0x06d94c1c      str   x4, [arg_28h]                        ; arg5
|       :   0x06d94c20      mov   x3, lr
|       :   0x06d94c24      bl    fcn.04da15a0
|       :   0x06d94c28      ldp   x0, x1, [arg_8h]
|       :   0x06d94c2c      ldp   x2, x3, [arg_18h]
|       :   0x06d94c30      ldr   x4, [arg_28h]                        ; [0x28:4]=0xace39d8
|       `=< 0x06d94c34      b     0x6d94990
==== AuthProvider_.GetTokenSource
/ fcn.06d94998(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg4, int64_t arg5, int64_t arg_8h, int64_t arg_10h, int64_t arg_18h, int64_t arg_20h, int64_t arg_28h, int64_t arg_50h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; arg int64_t arg2 @ x1
|       :   ; arg int64_t arg3 @ x2
|       :   ; arg int64_t arg4 @ x3
|       :   ; arg int64_t arg5 @ x4
|       :   ; var int64_t var_58h @ stack - 0x58
|       :   ; var int64_t var_50h @ stack - 0x50
|       :   ; var int64_t var_48h @ stack - 0x48
|       :   ; var int64_t var_40h @ stack - 0x40
|       :   ; var int64_t var_38h @ stack - 0x38
|       :   ; var int64_t var_30h @ stack - 0x30
|       :   ; var int64_t var_28h @ stack - 0x28
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_10h @ stack + 0x10
|       :   ; arg int64_t arg_18h @ stack + 0x18
|       :   ; arg int64_t arg_20h @ stack + 0x20
|       :   ; arg int64_t arg_28h @ stack + 0x28
|       :   ; arg int64_t arg_50h @ stack + 0x50
|       :   0x06d94998      cmp   x17, x16
|      ,==< 0x06d9499c      b.ls  0x6d94c14
|      |:   0x06d949a0      str   lr, [sp, -0xa0]!
|      |:   0x06d949a4      stur  fp, [sp, -8]
|      |:   0x06d949a8      sub   fp, sp, 8
|      |:   0x06d949ac      stp   x1, x2, [arg_10h]
|      |:   0x06d949b0      str   x0, [arg_8h]
|      |:   0x06d949b4      stp   x3, x4, [arg_20h]
|      |:   0x06d949b8      adrp  x27, 0x9583000
|      |:   0x06d949bc      add   x27, x27, 0x520
|      |:   0x06d949c0      ldp   x1, x0, [x27]
|      |:   0x06d949c4      ldr   x1, [x1, 0x90]                       ; [0x90:4]=0
|      |:   0x06d949c8      blr   x1
|      |:   0x06d949cc      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949d0      mov   x0, x1
|      |:   0x06d949d4      adrp  x1, 0x42a6000
|      |:   0x06d949d8      add   x1, x1, 0xe15
|      |:   0x06d949dc      mov   x3, x2
|      |:   0x06d949e0      mov   x2, 0xa
|      |:   0x06d949e4      blr   x3
|      |:   0x06d949e8      ldr   x2, [x0, 0x30]                       ; [0x30:4]=0
|      |:   0x06d949ec      mov   x0, x1
|      |:   0x06d949f0      blr   x2
|      |:   0x06d949f4      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949f8      mov   x0, x1
|      |:   0x06d949fc      adrp  x1, 0x4237000
|      |:   0x06d94a00      add   x1, x1, 0x7c0
|      |:   0x06d94a04      mov   x3, x2
|      |:   0x06d94a08      orr   x2, xzr, 3
|      |:   0x06d94a0c      blr   x3
|      |:   0x06d94a10      str   x1, [var_58h]
|     ,===< 0x06d94a14      cbz   x0, 0x6d94a1c
|     ||:   0x06d94a18      ldr   x0, [x0, 8]                          ; [0x8:4]=0
|     `---> 0x06d94a1c      str   x0, [var_30h]
|      |:   0x06d94a20      orr   x0, xzr, 0x10
|      |:   0x06d94a24      adrp  x1, 0x8744000
|      |:   0x06d94a28      add   x1, x1, 0xba0
|      |:   0x06d94a2c      orr   x2, xzr, 1
|      |:   0x06d94a30      bl    0x4d32500
|      |:   0x06d94a34      ldr   x3, [var_30h]                        ; [0x70:4]=8
|      |:   0x06d94a38      str   x3, [x0]
|      |:   0x06d94a3c      adrp  x27, 0xb48f000
|      |:   0x06d94a40      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a44      cbnz  w3, 0x6d94a50
|     ||:   0x06d94a48      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    ,====< 0x06d94a4c      b     0x6d94a5c
|    |`---> 0x06d94a50      bl    0x4da34f0
|    | |:   0x06d94a54      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    | |:   0x06d94a58      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a4c
|    `----> 0x06d94a5c      str   x0, [var_38h]
|      |:   0x06d94a60      str   x3, [x0, 8]
|      |:   0x06d94a64      nop
|      |:   0x06d94a68      orr   x0, xzr, 0x18
|      |:   0x06d94a6c      adrp  x1, 0x8744000
|      |:   0x06d94a70      add   x1, x1, 0xc18
|      |:   0x06d94a74      orr   x2, xzr, 1
|      |:   0x06d94a78      bl    0x4d32830
|      |:   0x06d94a7c      ldp   x3, x4, [arg_20h]
|      |:   0x06d94a80      stp   x3, x4, [x0, 8]
|      |:   0x06d94a84      adrp  x27, 0xb48f000
|      |:   0x06d94a88      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a8c      cbnz  w3, 0x6d94a98
|     ||:   0x06d94a90      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    ,====< 0x06d94a94      b     0x6d94aa4
|    |`---> 0x06d94a98      bl    0x4da34f0
|    | |:   0x06d94a9c      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    | |:   0x06d94aa0      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a94
|    `----> 0x06d94aa4      str   x0, [var_40h]
|      |:   0x06d94aa8      str   x3, [x0]
|      |:   0x06d94aac      ldr   x3, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ab0      ldrsb x27, [x3]
|      |:   0x06d94ab4      orr   x0, xzr, 0x18
|      |:   0x06d94ab8      adrp  x1, 0x87fc000
|      |:   0x06d94abc      add   x1, x1, 0x498
|      |:   0x06d94ac0      orr   x2, xzr, 1
|      |:   0x06d94ac4      bl    0x4d32830
|      |:   0x06d94ac8      nop
|      |:   0x06d94acc      adr   x3, 0x6d95f00
|      |:   0x06d94ad0      ldr   x4, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ad4      stp   x3, x4, [x0]
|      |:   0x06d94ad8      adrp  x27, 0xb48f000
|      |:   0x06d94adc      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94ae0      cbnz  w3, 0x6d94aec
|     ||:   0x06d94ae4      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    ,====< 0x06d94ae8      b     0x6d94af8
|    |`---> 0x06d94aec      bl    0x4da34f0
|    | |:   0x06d94af0      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    | |:   0x06d94af4      str   x7, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94ae8
|    `----> 0x06d94af8      str   x7, [x0, 0x10]
|      |:   0x06d94afc      add   x4, var_28h
|      |:   0x06d94b00      stp   xzr, xzr, [x4]
|      |:   0x06d94b04      stp   xzr, xzr, [x4, 0x10]
|      |:   0x06d94b08      adrp  x7, 0x94a8000
|      |:   0x06d94b0c      add   x7, x7, 0x4c0
|      |:   0x06d94b10      ldar  x8, [x7]
|      |:   0x06d94b14      ldr   x9, [x8]
|      |:   0x06d94b18      adrp  x27, 0x8f54000
|      |:   0x06d94b1c      ldr   w10, [x27, 0x930]                    ; [0x930:4]=0x58585858
|     .---> 0x06d94b20      and   x11, x10, x9
|     :|:   0x06d94b24      lsl   x11, x11, 4
|     :|:   0x06d94b28      add   x11, x11, 8
|     :|:   0x06d94b2c      ldr   x12, [x8, x11]
|     :|:   0x06d94b30      add   x11, x8, x11
|     :|:   0x06d94b34      adrp  x1, 0x7eb5000
|     :|:   0x06d94b38      add   x1, x1, 0xe38
|     :|:   0x06d94b3c      cmp   x12, x1
|    ,====< 0x06d94b40      b.eq  0x6d94b68
|    |:|:   0x06d94b44      add   x10, x10, 1
|    |`===< 0x06d94b48      cbnz  x12, 0x6d94b20
|    | |:   0x06d94b4c      str   x0, [var_48h]
|    | |:   0x06d94b50      mov   x0, x7
|    | |:   0x06d94b54      bl    0x4d2e4c0
|    | |:   0x06d94b58      add   x4, var_28h
|    | |:   0x06d94b5c      mov   x7, x0
|    | |:   0x06d94b60      ldr   x0, [var_48h]                        ; [0x58:4]=64
|    |,===< 0x06d94b64      b     0x6d94b6c
|    `----> 0x06d94b68      ldr   x7, [x11, 8]                         ; [0x8:4]=0
|     ||:   ; CODE XREF from fcn.06d94998 @ 0x6d94b64
|     `---> 0x06d94b6c      ldr   x8, [var_38h]                        ; [0x68:4]=0x380
|      |:   0x06d94b70      stp   x7, x8, [var_28h]
|      |:   0x06d94b74      adrp  x7, 0x8f0a000
|      |:   0x06d94b78      add   x7, x7, 0x408
|      |:   0x06d94b7c      ldr   x8, [var_40h]                        ; [0x60:4]=0x380
|      |:   0x06d94b80      stp   x7, x8, [var_28h + 0x10]
|      |:   0x06d94b84      adrp  x1, 0x43be000
|      |:   0x06d94b88      add   x1, x1, 0xf1e
|      |:   0x06d94b8c      mov   x2, 0x1a
|      |:   0x06d94b90      mov   x3, x0
|      |:   0x06d94b94      orr   x5, xzr, 2
|      |:   0x06d94b98      mov   x6, x5
|      |:   0x06d94b9c      adrp  x0, 0x79ca000
|      |:   0x06d94ba0      add   x0, x0, 0x800
|      |:   0x06d94ba4      bl    0x6d95270
|      |:   0x06d94ba8      str   x0, [var_50h]
|      |:   0x06d94bac      orr   x0, xzr, 0x10
|      |:   0x06d94bb0      adrp  x1, 0x87fc000
|      |:   0x06d94bb4      add   x1, x1, 0x518
|      |:   0x06d94bb8      orr   x2, xzr, 1
|      |:   0x06d94bbc      bl    0x4d32500
|      |:   0x06d94bc0      nop
|      |:   0x06d94bc4      adr   x7, 0x6d94c40
|      |:   0x06d94bc8      str   x7, [x0]
|      |:   0x06d94bcc      adrp  x27, 0xb48f000
|      |:   0x06d94bd0      ldr   w7, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94bd4      cbnz  w7, 0x6d94be0
|     ||:   0x06d94bd8      ldr   x4, [arg_50h]                        ; [0x50:4]=64
|    ,====< 0x06d94bdc      b     0x6d94bec
|    |`---> 0x06d94be0      bl    0x4da34f0
|    | |:   0x06d94be4      ldr   x4, [var_50h]                        ; [0x50:4]=64
|    | |:   0x06d94be8      str   x4, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94bdc
|    `----> 0x06d94bec      str   x4, [x0, 8]
|      |:   0x06d94bf0      mov   x1, 0x17
|      |:   0x06d94bf4      adrp  x2, 0x8f07000
|      |:   0x06d94bf8      add   x2, x2, 0x728
|      |:   0x06d94bfc      mov   x3, x0
|      |:   0x06d94c00      adrp  x0, 0x439b000
|      |:   0x06d94c04      add   x0, x0, 0x4fe
|      |:   0x06d94c08      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |:   0x06d94c0c      ldr   lr, [sp], 0xa0                       ; [0xa0:4]=0x779b960
|      |:   0x06d94c10      ret
|      `--> 0x06d94c14      stp   x0, x1, [arg_8h]                     ; arg2
|       :   0x06d94c18      stp   x2, x3, [arg_18h]                    ; arg4
|       :   0x06d94c1c      str   x4, [arg_28h]                        ; arg5
|       :   0x06d94c20      mov   x3, lr
|       :   0x06d94c24      bl    fcn.04da15a0
|       :   0x06d94c28      ldp   x0, x1, [arg_8h]
|       :   0x06d94c2c      ldp   x2, x3, [arg_18h]
|       :   0x06d94c30      ldr   x4, [arg_28h]                        ; [0x28:4]=0xace39d8
|       `=< 0x06d94c34      b     0x6d94990
==== AuthProvider_.GetCAICProject
/ fcn.06d94998(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg4, int64_t arg5, int64_t arg_8h, int64_t arg_10h, int64_t arg_18h, int64_t arg_20h, int64_t arg_28h, int64_t arg_50h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; arg int64_t arg2 @ x1
|       :   ; arg int64_t arg3 @ x2
|       :   ; arg int64_t arg4 @ x3
|       :   ; arg int64_t arg5 @ x4
|       :   ; var int64_t var_58h @ stack - 0x58
|       :   ; var int64_t var_50h @ stack - 0x50
|       :   ; var int64_t var_48h @ stack - 0x48
|       :   ; var int64_t var_40h @ stack - 0x40
|       :   ; var int64_t var_38h @ stack - 0x38
|       :   ; var int64_t var_30h @ stack - 0x30
|       :   ; var int64_t var_28h @ stack - 0x28
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_10h @ stack + 0x10
|       :   ; arg int64_t arg_18h @ stack + 0x18
|       :   ; arg int64_t arg_20h @ stack + 0x20
|       :   ; arg int64_t arg_28h @ stack + 0x28
|       :   ; arg int64_t arg_50h @ stack + 0x50
|       :   0x06d94998      cmp   x17, x16
|      ,==< 0x06d9499c      b.ls  0x6d94c14
|      |:   0x06d949a0      str   lr, [sp, -0xa0]!
|      |:   0x06d949a4      stur  fp, [sp, -8]
|      |:   0x06d949a8      sub   fp, sp, 8
|      |:   0x06d949ac      stp   x1, x2, [arg_10h]
|      |:   0x06d949b0      str   x0, [arg_8h]
|      |:   0x06d949b4      stp   x3, x4, [arg_20h]
|      |:   0x06d949b8      adrp  x27, 0x9583000
|      |:   0x06d949bc      add   x27, x27, 0x520
|      |:   0x06d949c0      ldp   x1, x0, [x27]
|      |:   0x06d949c4      ldr   x1, [x1, 0x90]                       ; [0x90:4]=0
|      |:   0x06d949c8      blr   x1
|      |:   0x06d949cc      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949d0      mov   x0, x1
|      |:   0x06d949d4      adrp  x1, 0x42a6000
|      |:   0x06d949d8      add   x1, x1, 0xe15
|      |:   0x06d949dc      mov   x3, x2
|      |:   0x06d949e0      mov   x2, 0xa
|      |:   0x06d949e4      blr   x3
|      |:   0x06d949e8      ldr   x2, [x0, 0x30]                       ; [0x30:4]=0
|      |:   0x06d949ec      mov   x0, x1
|      |:   0x06d949f0      blr   x2
|      |:   0x06d949f4      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949f8      mov   x0, x1
|      |:   0x06d949fc      adrp  x1, 0x4237000
|      |:   0x06d94a00      add   x1, x1, 0x7c0
|      |:   0x06d94a04      mov   x3, x2
|      |:   0x06d94a08      orr   x2, xzr, 3
|      |:   0x06d94a0c      blr   x3
|      |:   0x06d94a10      str   x1, [var_58h]
|     ,===< 0x06d94a14      cbz   x0, 0x6d94a1c
|     ||:   0x06d94a18      ldr   x0, [x0, 8]                          ; [0x8:4]=0
|     `---> 0x06d94a1c      str   x0, [var_30h]
|      |:   0x06d94a20      orr   x0, xzr, 0x10
|      |:   0x06d94a24      adrp  x1, 0x8744000
|      |:   0x06d94a28      add   x1, x1, 0xba0
|      |:   0x06d94a2c      orr   x2, xzr, 1
|      |:   0x06d94a30      bl    0x4d32500
|      |:   0x06d94a34      ldr   x3, [var_30h]                        ; [0x70:4]=8
|      |:   0x06d94a38      str   x3, [x0]
|      |:   0x06d94a3c      adrp  x27, 0xb48f000
|      |:   0x06d94a40      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a44      cbnz  w3, 0x6d94a50
|     ||:   0x06d94a48      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    ,====< 0x06d94a4c      b     0x6d94a5c
|    |`---> 0x06d94a50      bl    0x4da34f0
|    | |:   0x06d94a54      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    | |:   0x06d94a58      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a4c
|    `----> 0x06d94a5c      str   x0, [var_38h]
|      |:   0x06d94a60      str   x3, [x0, 8]
|      |:   0x06d94a64      nop
|      |:   0x06d94a68      orr   x0, xzr, 0x18
|      |:   0x06d94a6c      adrp  x1, 0x8744000
|      |:   0x06d94a70      add   x1, x1, 0xc18
|      |:   0x06d94a74      orr   x2, xzr, 1
|      |:   0x06d94a78      bl    0x4d32830
|      |:   0x06d94a7c      ldp   x3, x4, [arg_20h]
|      |:   0x06d94a80      stp   x3, x4, [x0, 8]
|      |:   0x06d94a84      adrp  x27, 0xb48f000
|      |:   0x06d94a88      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a8c      cbnz  w3, 0x6d94a98
|     ||:   0x06d94a90      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    ,====< 0x06d94a94      b     0x6d94aa4
|    |`---> 0x06d94a98      bl    0x4da34f0
|    | |:   0x06d94a9c      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    | |:   0x06d94aa0      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a94
|    `----> 0x06d94aa4      str   x0, [var_40h]
|      |:   0x06d94aa8      str   x3, [x0]
|      |:   0x06d94aac      ldr   x3, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ab0      ldrsb x27, [x3]
|      |:   0x06d94ab4      orr   x0, xzr, 0x18
|      |:   0x06d94ab8      adrp  x1, 0x87fc000
|      |:   0x06d94abc      add   x1, x1, 0x498
|      |:   0x06d94ac0      orr   x2, xzr, 1
|      |:   0x06d94ac4      bl    0x4d32830
|      |:   0x06d94ac8      nop
|      |:   0x06d94acc      adr   x3, 0x6d95f00
|      |:   0x06d94ad0      ldr   x4, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ad4      stp   x3, x4, [x0]
|      |:   0x06d94ad8      adrp  x27, 0xb48f000
|      |:   0x06d94adc      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94ae0      cbnz  w3, 0x6d94aec
|     ||:   0x06d94ae4      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    ,====< 0x06d94ae8      b     0x6d94af8
|    |`---> 0x06d94aec      bl    0x4da34f0
|    | |:   0x06d94af0      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    | |:   0x06d94af4      str   x7, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94ae8
|    `----> 0x06d94af8      str   x7, [x0, 0x10]
|      |:   0x06d94afc      add   x4, var_28h
|      |:   0x06d94b00      stp   xzr, xzr, [x4]
|      |:   0x06d94b04      stp   xzr, xzr, [x4, 0x10]
|      |:   0x06d94b08      adrp  x7, 0x94a8000
|      |:   0x06d94b0c      add   x7, x7, 0x4c0
|      |:   0x06d94b10      ldar  x8, [x7]
|      |:   0x06d94b14      ldr   x9, [x8]
|      |:   0x06d94b18      adrp  x27, 0x8f54000
|      |:   0x06d94b1c      ldr   w10, [x27, 0x930]                    ; [0x930:4]=0x58585858
|     .---> 0x06d94b20      and   x11, x10, x9
|     :|:   0x06d94b24      lsl   x11, x11, 4
|     :|:   0x06d94b28      add   x11, x11, 8
|     :|:   0x06d94b2c      ldr   x12, [x8, x11]
|     :|:   0x06d94b30      add   x11, x8, x11
|     :|:   0x06d94b34      adrp  x1, 0x7eb5000
|     :|:   0x06d94b38      add   x1, x1, 0xe38
|     :|:   0x06d94b3c      cmp   x12, x1
|    ,====< 0x06d94b40      b.eq  0x6d94b68
|    |:|:   0x06d94b44      add   x10, x10, 1
|    |`===< 0x06d94b48      cbnz  x12, 0x6d94b20
|    | |:   0x06d94b4c      str   x0, [var_48h]
|    | |:   0x06d94b50      mov   x0, x7
|    | |:   0x06d94b54      bl    0x4d2e4c0
|    | |:   0x06d94b58      add   x4, var_28h
|    | |:   0x06d94b5c      mov   x7, x0
|    | |:   0x06d94b60      ldr   x0, [var_48h]                        ; [0x58:4]=64
|    |,===< 0x06d94b64      b     0x6d94b6c
|    `----> 0x06d94b68      ldr   x7, [x11, 8]                         ; [0x8:4]=0
|     ||:   ; CODE XREF from fcn.06d94998 @ 0x6d94b64
|     `---> 0x06d94b6c      ldr   x8, [var_38h]                        ; [0x68:4]=0x380
|      |:   0x06d94b70      stp   x7, x8, [var_28h]
|      |:   0x06d94b74      adrp  x7, 0x8f0a000
|      |:   0x06d94b78      add   x7, x7, 0x408
|      |:   0x06d94b7c      ldr   x8, [var_40h]                        ; [0x60:4]=0x380
|      |:   0x06d94b80      stp   x7, x8, [var_28h + 0x10]
|      |:   0x06d94b84      adrp  x1, 0x43be000
|      |:   0x06d94b88      add   x1, x1, 0xf1e
|      |:   0x06d94b8c      mov   x2, 0x1a
|      |:   0x06d94b90      mov   x3, x0
|      |:   0x06d94b94      orr   x5, xzr, 2
|      |:   0x06d94b98      mov   x6, x5
|      |:   0x06d94b9c      adrp  x0, 0x79ca000
|      |:   0x06d94ba0      add   x0, x0, 0x800
|      |:   0x06d94ba4      bl    0x6d95270
|      |:   0x06d94ba8      str   x0, [var_50h]
|      |:   0x06d94bac      orr   x0, xzr, 0x10
|      |:   0x06d94bb0      adrp  x1, 0x87fc000
|      |:   0x06d94bb4      add   x1, x1, 0x518
|      |:   0x06d94bb8      orr   x2, xzr, 1
|      |:   0x06d94bbc      bl    0x4d32500
|      |:   0x06d94bc0      nop
|      |:   0x06d94bc4      adr   x7, 0x6d94c40
|      |:   0x06d94bc8      str   x7, [x0]
|      |:   0x06d94bcc      adrp  x27, 0xb48f000
|      |:   0x06d94bd0      ldr   w7, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94bd4      cbnz  w7, 0x6d94be0
|     ||:   0x06d94bd8      ldr   x4, [arg_50h]                        ; [0x50:4]=64
|    ,====< 0x06d94bdc      b     0x6d94bec
|    |`---> 0x06d94be0      bl    0x4da34f0
|    | |:   0x06d94be4      ldr   x4, [var_50h]                        ; [0x50:4]=64
|    | |:   0x06d94be8      str   x4, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94bdc
|    `----> 0x06d94bec      str   x4, [x0, 8]
|      |:   0x06d94bf0      mov   x1, 0x17
|      |:   0x06d94bf4      adrp  x2, 0x8f07000
|      |:   0x06d94bf8      add   x2, x2, 0x728
|      |:   0x06d94bfc      mov   x3, x0
|      |:   0x06d94c00      adrp  x0, 0x439b000
|      |:   0x06d94c04      add   x0, x0, 0x4fe
|      |:   0x06d94c08      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |:   0x06d94c0c      ldr   lr, [sp], 0xa0                       ; [0xa0:4]=0x779b960
|      |:   0x06d94c10      ret
|      `--> 0x06d94c14      stp   x0, x1, [arg_8h]                     ; arg2
|       :   0x06d94c18      stp   x2, x3, [arg_18h]                    ; arg4
|       :   0x06d94c1c      str   x4, [arg_28h]                        ; arg5
|       :   0x06d94c20      mov   x3, lr
|       :   0x06d94c24      bl    fcn.04da15a0
|       :   0x06d94c28      ldp   x0, x1, [arg_8h]
|       :   0x06d94c2c      ldp   x2, x3, [arg_18h]
|       :   0x06d94c30      ldr   x4, [arg_28h]                        ; [0x28:4]=0xace39d8
|       `=< 0x06d94c34      b     0x6d94990
==== AuthProvider_.GetGCPLocation
/ fcn.06d94998(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg4, int64_t arg5, int64_t arg_8h, int64_t arg_10h, int64_t arg_18h, int64_t arg_20h, int64_t arg_28h, int64_t arg_50h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; arg int64_t arg2 @ x1
|       :   ; arg int64_t arg3 @ x2
|       :   ; arg int64_t arg4 @ x3
|       :   ; arg int64_t arg5 @ x4
|       :   ; var int64_t var_58h @ stack - 0x58
|       :   ; var int64_t var_50h @ stack - 0x50
|       :   ; var int64_t var_48h @ stack - 0x48
|       :   ; var int64_t var_40h @ stack - 0x40
|       :   ; var int64_t var_38h @ stack - 0x38
|       :   ; var int64_t var_30h @ stack - 0x30
|       :   ; var int64_t var_28h @ stack - 0x28
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_10h @ stack + 0x10
|       :   ; arg int64_t arg_18h @ stack + 0x18
|       :   ; arg int64_t arg_20h @ stack + 0x20
|       :   ; arg int64_t arg_28h @ stack + 0x28
|       :   ; arg int64_t arg_50h @ stack + 0x50
|       :   0x06d94998      cmp   x17, x16
|      ,==< 0x06d9499c      b.ls  0x6d94c14
|      |:   0x06d949a0      str   lr, [sp, -0xa0]!
|      |:   0x06d949a4      stur  fp, [sp, -8]
|      |:   0x06d949a8      sub   fp, sp, 8
|      |:   0x06d949ac      stp   x1, x2, [arg_10h]
|      |:   0x06d949b0      str   x0, [arg_8h]
|      |:   0x06d949b4      stp   x3, x4, [arg_20h]
|      |:   0x06d949b8      adrp  x27, 0x9583000
|      |:   0x06d949bc      add   x27, x27, 0x520
|      |:   0x06d949c0      ldp   x1, x0, [x27]
|      |:   0x06d949c4      ldr   x1, [x1, 0x90]                       ; [0x90:4]=0
|      |:   0x06d949c8      blr   x1
|      |:   0x06d949cc      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949d0      mov   x0, x1
|      |:   0x06d949d4      adrp  x1, 0x42a6000
|      |:   0x06d949d8      add   x1, x1, 0xe15
|      |:   0x06d949dc      mov   x3, x2
|      |:   0x06d949e0      mov   x2, 0xa
|      |:   0x06d949e4      blr   x3
|      |:   0x06d949e8      ldr   x2, [x0, 0x30]                       ; [0x30:4]=0
|      |:   0x06d949ec      mov   x0, x1
|      |:   0x06d949f0      blr   x2
|      |:   0x06d949f4      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949f8      mov   x0, x1
|      |:   0x06d949fc      adrp  x1, 0x4237000
|      |:   0x06d94a00      add   x1, x1, 0x7c0
|      |:   0x06d94a04      mov   x3, x2
|      |:   0x06d94a08      orr   x2, xzr, 3
|      |:   0x06d94a0c      blr   x3
|      |:   0x06d94a10      str   x1, [var_58h]
|     ,===< 0x06d94a14      cbz   x0, 0x6d94a1c
|     ||:   0x06d94a18      ldr   x0, [x0, 8]                          ; [0x8:4]=0
|     `---> 0x06d94a1c      str   x0, [var_30h]
|      |:   0x06d94a20      orr   x0, xzr, 0x10
|      |:   0x06d94a24      adrp  x1, 0x8744000
|      |:   0x06d94a28      add   x1, x1, 0xba0
|      |:   0x06d94a2c      orr   x2, xzr, 1
|      |:   0x06d94a30      bl    0x4d32500
|      |:   0x06d94a34      ldr   x3, [var_30h]                        ; [0x70:4]=8
|      |:   0x06d94a38      str   x3, [x0]
|      |:   0x06d94a3c      adrp  x27, 0xb48f000
|      |:   0x06d94a40      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a44      cbnz  w3, 0x6d94a50
|     ||:   0x06d94a48      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    ,====< 0x06d94a4c      b     0x6d94a5c
|    |`---> 0x06d94a50      bl    0x4da34f0
|    | |:   0x06d94a54      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    | |:   0x06d94a58      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a4c
|    `----> 0x06d94a5c      str   x0, [var_38h]
|      |:   0x06d94a60      str   x3, [x0, 8]
|      |:   0x06d94a64      nop
|      |:   0x06d94a68      orr   x0, xzr, 0x18
|      |:   0x06d94a6c      adrp  x1, 0x8744000
|      |:   0x06d94a70      add   x1, x1, 0xc18
|      |:   0x06d94a74      orr   x2, xzr, 1
|      |:   0x06d94a78      bl    0x4d32830
|      |:   0x06d94a7c      ldp   x3, x4, [arg_20h]
|      |:   0x06d94a80      stp   x3, x4, [x0, 8]
|      |:   0x06d94a84      adrp  x27, 0xb48f000
|      |:   0x06d94a88      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a8c      cbnz  w3, 0x6d94a98
|     ||:   0x06d94a90      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    ,====< 0x06d94a94      b     0x6d94aa4
|    |`---> 0x06d94a98      bl    0x4da34f0
|    | |:   0x06d94a9c      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    | |:   0x06d94aa0      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a94
|    `----> 0x06d94aa4      str   x0, [var_40h]
|      |:   0x06d94aa8      str   x3, [x0]
|      |:   0x06d94aac      ldr   x3, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ab0      ldrsb x27, [x3]
|      |:   0x06d94ab4      orr   x0, xzr, 0x18
|      |:   0x06d94ab8      adrp  x1, 0x87fc000
|      |:   0x06d94abc      add   x1, x1, 0x498
|      |:   0x06d94ac0      orr   x2, xzr, 1
|      |:   0x06d94ac4      bl    0x4d32830
|      |:   0x06d94ac8      nop
|      |:   0x06d94acc      adr   x3, 0x6d95f00
|      |:   0x06d94ad0      ldr   x4, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ad4      stp   x3, x4, [x0]
|      |:   0x06d94ad8      adrp  x27, 0xb48f000
|      |:   0x06d94adc      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94ae0      cbnz  w3, 0x6d94aec
|     ||:   0x06d94ae4      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    ,====< 0x06d94ae8      b     0x6d94af8
|    |`---> 0x06d94aec      bl    0x4da34f0
|    | |:   0x06d94af0      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    | |:   0x06d94af4      str   x7, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94ae8
|    `----> 0x06d94af8      str   x7, [x0, 0x10]
|      |:   0x06d94afc      add   x4, var_28h
|      |:   0x06d94b00      stp   xzr, xzr, [x4]
|      |:   0x06d94b04      stp   xzr, xzr, [x4, 0x10]
|      |:   0x06d94b08      adrp  x7, 0x94a8000
|      |:   0x06d94b0c      add   x7, x7, 0x4c0
|      |:   0x06d94b10      ldar  x8, [x7]
|      |:   0x06d94b14      ldr   x9, [x8]
|      |:   0x06d94b18      adrp  x27, 0x8f54000
|      |:   0x06d94b1c      ldr   w10, [x27, 0x930]                    ; [0x930:4]=0x58585858
|     .---> 0x06d94b20      and   x11, x10, x9
|     :|:   0x06d94b24      lsl   x11, x11, 4
|     :|:   0x06d94b28      add   x11, x11, 8
|     :|:   0x06d94b2c      ldr   x12, [x8, x11]
|     :|:   0x06d94b30      add   x11, x8, x11
|     :|:   0x06d94b34      adrp  x1, 0x7eb5000
|     :|:   0x06d94b38      add   x1, x1, 0xe38
|     :|:   0x06d94b3c      cmp   x12, x1
|    ,====< 0x06d94b40      b.eq  0x6d94b68
|    |:|:   0x06d94b44      add   x10, x10, 1
|    |`===< 0x06d94b48      cbnz  x12, 0x6d94b20
|    | |:   0x06d94b4c      str   x0, [var_48h]
|    | |:   0x06d94b50      mov   x0, x7
|    | |:   0x06d94b54      bl    0x4d2e4c0
|    | |:   0x06d94b58      add   x4, var_28h
|    | |:   0x06d94b5c      mov   x7, x0
|    | |:   0x06d94b60      ldr   x0, [var_48h]                        ; [0x58:4]=64
|    |,===< 0x06d94b64      b     0x6d94b6c
|    `----> 0x06d94b68      ldr   x7, [x11, 8]                         ; [0x8:4]=0
|     ||:   ; CODE XREF from fcn.06d94998 @ 0x6d94b64
|     `---> 0x06d94b6c      ldr   x8, [var_38h]                        ; [0x68:4]=0x380
|      |:   0x06d94b70      stp   x7, x8, [var_28h]
|      |:   0x06d94b74      adrp  x7, 0x8f0a000
|      |:   0x06d94b78      add   x7, x7, 0x408
|      |:   0x06d94b7c      ldr   x8, [var_40h]                        ; [0x60:4]=0x380
|      |:   0x06d94b80      stp   x7, x8, [var_28h + 0x10]
|      |:   0x06d94b84      adrp  x1, 0x43be000
|      |:   0x06d94b88      add   x1, x1, 0xf1e
|      |:   0x06d94b8c      mov   x2, 0x1a
|      |:   0x06d94b90      mov   x3, x0
|      |:   0x06d94b94      orr   x5, xzr, 2
|      |:   0x06d94b98      mov   x6, x5
|      |:   0x06d94b9c      adrp  x0, 0x79ca000
|      |:   0x06d94ba0      add   x0, x0, 0x800
|      |:   0x06d94ba4      bl    0x6d95270
|      |:   0x06d94ba8      str   x0, [var_50h]
|      |:   0x06d94bac      orr   x0, xzr, 0x10
|      |:   0x06d94bb0      adrp  x1, 0x87fc000
|      |:   0x06d94bb4      add   x1, x1, 0x518
|      |:   0x06d94bb8      orr   x2, xzr, 1
|      |:   0x06d94bbc      bl    0x4d32500
|      |:   0x06d94bc0      nop
|      |:   0x06d94bc4      adr   x7, 0x6d94c40
|      |:   0x06d94bc8      str   x7, [x0]
|      |:   0x06d94bcc      adrp  x27, 0xb48f000
|      |:   0x06d94bd0      ldr   w7, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94bd4      cbnz  w7, 0x6d94be0
|     ||:   0x06d94bd8      ldr   x4, [arg_50h]                        ; [0x50:4]=64
|    ,====< 0x06d94bdc      b     0x6d94bec
|    |`---> 0x06d94be0      bl    0x4da34f0
|    | |:   0x06d94be4      ldr   x4, [var_50h]                        ; [0x50:4]=64
|    | |:   0x06d94be8      str   x4, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94bdc
|    `----> 0x06d94bec      str   x4, [x0, 8]
|      |:   0x06d94bf0      mov   x1, 0x17
|      |:   0x06d94bf4      adrp  x2, 0x8f07000
|      |:   0x06d94bf8      add   x2, x2, 0x728
|      |:   0x06d94bfc      mov   x3, x0
|      |:   0x06d94c00      adrp  x0, 0x439b000
|      |:   0x06d94c04      add   x0, x0, 0x4fe
|      |:   0x06d94c08      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |:   0x06d94c0c      ldr   lr, [sp], 0xa0                       ; [0xa0:4]=0x779b960
|      |:   0x06d94c10      ret
|      `--> 0x06d94c14      stp   x0, x1, [arg_8h]                     ; arg2
|       :   0x06d94c18      stp   x2, x3, [arg_18h]                    ; arg4
|       :   0x06d94c1c      str   x4, [arg_28h]                        ; arg5
|       :   0x06d94c20      mov   x3, lr
|       :   0x06d94c24      bl    fcn.04da15a0
|       :   0x06d94c28      ldp   x0, x1, [arg_8h]
|       :   0x06d94c2c      ldp   x2, x3, [arg_18h]
|       :   0x06d94c30      ldr   x4, [arg_28h]                        ; [0x28:4]=0xace39d8
|       `=< 0x06d94c34      b     0x6d94990
==== AuthProvider_.IsBusinessPayGo
/ fcn.06d94998(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg4, int64_t arg5, int64_t arg_8h, int64_t arg_10h, int64_t arg_18h, int64_t arg_20h, int64_t arg_28h, int64_t arg_50h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; arg int64_t arg2 @ x1
|       :   ; arg int64_t arg3 @ x2
|       :   ; arg int64_t arg4 @ x3
|       :   ; arg int64_t arg5 @ x4
|       :   ; var int64_t var_58h @ stack - 0x58
|       :   ; var int64_t var_50h @ stack - 0x50
|       :   ; var int64_t var_48h @ stack - 0x48
|       :   ; var int64_t var_40h @ stack - 0x40
|       :   ; var int64_t var_38h @ stack - 0x38
|       :   ; var int64_t var_30h @ stack - 0x30
|       :   ; var int64_t var_28h @ stack - 0x28
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_10h @ stack + 0x10
|       :   ; arg int64_t arg_18h @ stack + 0x18
|       :   ; arg int64_t arg_20h @ stack + 0x20
|       :   ; arg int64_t arg_28h @ stack + 0x28
|       :   ; arg int64_t arg_50h @ stack + 0x50
|       :   0x06d94998      cmp   x17, x16
|      ,==< 0x06d9499c      b.ls  0x6d94c14
|      |:   0x06d949a0      str   lr, [sp, -0xa0]!
|      |:   0x06d949a4      stur  fp, [sp, -8]
|      |:   0x06d949a8      sub   fp, sp, 8
|      |:   0x06d949ac      stp   x1, x2, [arg_10h]
|      |:   0x06d949b0      str   x0, [arg_8h]
|      |:   0x06d949b4      stp   x3, x4, [arg_20h]
|      |:   0x06d949b8      adrp  x27, 0x9583000
|      |:   0x06d949bc      add   x27, x27, 0x520
|      |:   0x06d949c0      ldp   x1, x0, [x27]
|      |:   0x06d949c4      ldr   x1, [x1, 0x90]                       ; [0x90:4]=0
|      |:   0x06d949c8      blr   x1
|      |:   0x06d949cc      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949d0      mov   x0, x1
|      |:   0x06d949d4      adrp  x1, 0x42a6000
|      |:   0x06d949d8      add   x1, x1, 0xe15
|      |:   0x06d949dc      mov   x3, x2
|      |:   0x06d949e0      mov   x2, 0xa
|      |:   0x06d949e4      blr   x3
|      |:   0x06d949e8      ldr   x2, [x0, 0x30]                       ; [0x30:4]=0
|      |:   0x06d949ec      mov   x0, x1
|      |:   0x06d949f0      blr   x2
|      |:   0x06d949f4      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949f8      mov   x0, x1
|      |:   0x06d949fc      adrp  x1, 0x4237000
|      |:   0x06d94a00      add   x1, x1, 0x7c0
|      |:   0x06d94a04      mov   x3, x2
|      |:   0x06d94a08      orr   x2, xzr, 3
|      |:   0x06d94a0c      blr   x3
|      |:   0x06d94a10      str   x1, [var_58h]
|     ,===< 0x06d94a14      cbz   x0, 0x6d94a1c
|     ||:   0x06d94a18      ldr   x0, [x0, 8]                          ; [0x8:4]=0
|     `---> 0x06d94a1c      str   x0, [var_30h]
|      |:   0x06d94a20      orr   x0, xzr, 0x10
|      |:   0x06d94a24      adrp  x1, 0x8744000
|      |:   0x06d94a28      add   x1, x1, 0xba0
|      |:   0x06d94a2c      orr   x2, xzr, 1
|      |:   0x06d94a30      bl    0x4d32500
|      |:   0x06d94a34      ldr   x3, [var_30h]                        ; [0x70:4]=8
|      |:   0x06d94a38      str   x3, [x0]
|      |:   0x06d94a3c      adrp  x27, 0xb48f000
|      |:   0x06d94a40      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a44      cbnz  w3, 0x6d94a50
|     ||:   0x06d94a48      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    ,====< 0x06d94a4c      b     0x6d94a5c
|    |`---> 0x06d94a50      bl    0x4da34f0
|    | |:   0x06d94a54      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    | |:   0x06d94a58      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a4c
|    `----> 0x06d94a5c      str   x0, [var_38h]
|      |:   0x06d94a60      str   x3, [x0, 8]
|      |:   0x06d94a64      nop
|      |:   0x06d94a68      orr   x0, xzr, 0x18
|      |:   0x06d94a6c      adrp  x1, 0x8744000
|      |:   0x06d94a70      add   x1, x1, 0xc18
|      |:   0x06d94a74      orr   x2, xzr, 1
|      |:   0x06d94a78      bl    0x4d32830
|      |:   0x06d94a7c      ldp   x3, x4, [arg_20h]
|      |:   0x06d94a80      stp   x3, x4, [x0, 8]
|      |:   0x06d94a84      adrp  x27, 0xb48f000
|      |:   0x06d94a88      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a8c      cbnz  w3, 0x6d94a98
|     ||:   0x06d94a90      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    ,====< 0x06d94a94      b     0x6d94aa4
|    |`---> 0x06d94a98      bl    0x4da34f0
|    | |:   0x06d94a9c      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    | |:   0x06d94aa0      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a94
|    `----> 0x06d94aa4      str   x0, [var_40h]
|      |:   0x06d94aa8      str   x3, [x0]
|      |:   0x06d94aac      ldr   x3, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ab0      ldrsb x27, [x3]
|      |:   0x06d94ab4      orr   x0, xzr, 0x18
|      |:   0x06d94ab8      adrp  x1, 0x87fc000
|      |:   0x06d94abc      add   x1, x1, 0x498
|      |:   0x06d94ac0      orr   x2, xzr, 1
|      |:   0x06d94ac4      bl    0x4d32830
|      |:   0x06d94ac8      nop
|      |:   0x06d94acc      adr   x3, 0x6d95f00
|      |:   0x06d94ad0      ldr   x4, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ad4      stp   x3, x4, [x0]
|      |:   0x06d94ad8      adrp  x27, 0xb48f000
|      |:   0x06d94adc      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94ae0      cbnz  w3, 0x6d94aec
|     ||:   0x06d94ae4      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    ,====< 0x06d94ae8      b     0x6d94af8
|    |`---> 0x06d94aec      bl    0x4da34f0
|    | |:   0x06d94af0      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    | |:   0x06d94af4      str   x7, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94ae8
|    `----> 0x06d94af8      str   x7, [x0, 0x10]
|      |:   0x06d94afc      add   x4, var_28h
|      |:   0x06d94b00      stp   xzr, xzr, [x4]
|      |:   0x06d94b04      stp   xzr, xzr, [x4, 0x10]
|      |:   0x06d94b08      adrp  x7, 0x94a8000
|      |:   0x06d94b0c      add   x7, x7, 0x4c0
|      |:   0x06d94b10      ldar  x8, [x7]
|      |:   0x06d94b14      ldr   x9, [x8]
|      |:   0x06d94b18      adrp  x27, 0x8f54000
|      |:   0x06d94b1c      ldr   w10, [x27, 0x930]                    ; [0x930:4]=0x58585858
|     .---> 0x06d94b20      and   x11, x10, x9
|     :|:   0x06d94b24      lsl   x11, x11, 4
|     :|:   0x06d94b28      add   x11, x11, 8
|     :|:   0x06d94b2c      ldr   x12, [x8, x11]
|     :|:   0x06d94b30      add   x11, x8, x11
|     :|:   0x06d94b34      adrp  x1, 0x7eb5000
|     :|:   0x06d94b38      add   x1, x1, 0xe38
|     :|:   0x06d94b3c      cmp   x12, x1
|    ,====< 0x06d94b40      b.eq  0x6d94b68
|    |:|:   0x06d94b44      add   x10, x10, 1
|    |`===< 0x06d94b48      cbnz  x12, 0x6d94b20
|    | |:   0x06d94b4c      str   x0, [var_48h]
|    | |:   0x06d94b50      mov   x0, x7
|    | |:   0x06d94b54      bl    0x4d2e4c0
|    | |:   0x06d94b58      add   x4, var_28h
|    | |:   0x06d94b5c      mov   x7, x0
|    | |:   0x06d94b60      ldr   x0, [var_48h]                        ; [0x58:4]=64
|    |,===< 0x06d94b64      b     0x6d94b6c
|    `----> 0x06d94b68      ldr   x7, [x11, 8]                         ; [0x8:4]=0
|     ||:   ; CODE XREF from fcn.06d94998 @ 0x6d94b64
|     `---> 0x06d94b6c      ldr   x8, [var_38h]                        ; [0x68:4]=0x380
|      |:   0x06d94b70      stp   x7, x8, [var_28h]
|      |:   0x06d94b74      adrp  x7, 0x8f0a000
|      |:   0x06d94b78      add   x7, x7, 0x408
|      |:   0x06d94b7c      ldr   x8, [var_40h]                        ; [0x60:4]=0x380
|      |:   0x06d94b80      stp   x7, x8, [var_28h + 0x10]
|      |:   0x06d94b84      adrp  x1, 0x43be000
|      |:   0x06d94b88      add   x1, x1, 0xf1e
|      |:   0x06d94b8c      mov   x2, 0x1a
|      |:   0x06d94b90      mov   x3, x0
|      |:   0x06d94b94      orr   x5, xzr, 2
|      |:   0x06d94b98      mov   x6, x5
|      |:   0x06d94b9c      adrp  x0, 0x79ca000
|      |:   0x06d94ba0      add   x0, x0, 0x800
|      |:   0x06d94ba4      bl    0x6d95270
|      |:   0x06d94ba8      str   x0, [var_50h]
|      |:   0x06d94bac      orr   x0, xzr, 0x10
|      |:   0x06d94bb0      adrp  x1, 0x87fc000
|      |:   0x06d94bb4      add   x1, x1, 0x518
|      |:   0x06d94bb8      orr   x2, xzr, 1
|      |:   0x06d94bbc      bl    0x4d32500
|      |:   0x06d94bc0      nop
|      |:   0x06d94bc4      adr   x7, 0x6d94c40
|      |:   0x06d94bc8      str   x7, [x0]
|      |:   0x06d94bcc      adrp  x27, 0xb48f000
|      |:   0x06d94bd0      ldr   w7, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94bd4      cbnz  w7, 0x6d94be0
|     ||:   0x06d94bd8      ldr   x4, [arg_50h]                        ; [0x50:4]=64
|    ,====< 0x06d94bdc      b     0x6d94bec
|    |`---> 0x06d94be0      bl    0x4da34f0
|    | |:   0x06d94be4      ldr   x4, [var_50h]                        ; [0x50:4]=64
|    | |:   0x06d94be8      str   x4, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94bdc
|    `----> 0x06d94bec      str   x4, [x0, 8]
|      |:   0x06d94bf0      mov   x1, 0x17
|      |:   0x06d94bf4      adrp  x2, 0x8f07000
|      |:   0x06d94bf8      add   x2, x2, 0x728
|      |:   0x06d94bfc      mov   x3, x0
|      |:   0x06d94c00      adrp  x0, 0x439b000
|      |:   0x06d94c04      add   x0, x0, 0x4fe
|      |:   0x06d94c08      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |:   0x06d94c0c      ldr   lr, [sp], 0xa0                       ; [0xa0:4]=0x779b960
|      |:   0x06d94c10      ret
|      `--> 0x06d94c14      stp   x0, x1, [arg_8h]                     ; arg2
|       :   0x06d94c18      stp   x2, x3, [arg_18h]                    ; arg4
|       :   0x06d94c1c      str   x4, [arg_28h]                        ; arg5
|       :   0x06d94c20      mov   x3, lr
|       :   0x06d94c24      bl    fcn.04da15a0
|       :   0x06d94c28      ldp   x0, x1, [arg_8h]
|       :   0x06d94c2c      ldp   x2, x3, [arg_18h]
|       :   0x06d94c30      ldr   x4, [arg_28h]                        ; [0x28:4]=0xace39d8
|       `=< 0x06d94c34      b     0x6d94990
==== AuthProvider_.IsBusinessEnterprise
/ fcn.06d94998(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg4, int64_t arg5, int64_t arg_8h, int64_t arg_10h, int64_t arg_18h, int64_t arg_20h, int64_t arg_28h, int64_t arg_50h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; arg int64_t arg2 @ x1
|       :   ; arg int64_t arg3 @ x2
|       :   ; arg int64_t arg4 @ x3
|       :   ; arg int64_t arg5 @ x4
|       :   ; var int64_t var_58h @ stack - 0x58
|       :   ; var int64_t var_50h @ stack - 0x50
|       :   ; var int64_t var_48h @ stack - 0x48
|       :   ; var int64_t var_40h @ stack - 0x40
|       :   ; var int64_t var_38h @ stack - 0x38
|       :   ; var int64_t var_30h @ stack - 0x30
|       :   ; var int64_t var_28h @ stack - 0x28
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_10h @ stack + 0x10
|       :   ; arg int64_t arg_18h @ stack + 0x18
|       :   ; arg int64_t arg_20h @ stack + 0x20
|       :   ; arg int64_t arg_28h @ stack + 0x28
|       :   ; arg int64_t arg_50h @ stack + 0x50
|       :   0x06d94998      cmp   x17, x16
|      ,==< 0x06d9499c      b.ls  0x6d94c14
|      |:   0x06d949a0      str   lr, [sp, -0xa0]!
|      |:   0x06d949a4      stur  fp, [sp, -8]
|      |:   0x06d949a8      sub   fp, sp, 8
|      |:   0x06d949ac      stp   x1, x2, [arg_10h]
|      |:   0x06d949b0      str   x0, [arg_8h]
|      |:   0x06d949b4      stp   x3, x4, [arg_20h]
|      |:   0x06d949b8      adrp  x27, 0x9583000
|      |:   0x06d949bc      add   x27, x27, 0x520
|      |:   0x06d949c0      ldp   x1, x0, [x27]
|      |:   0x06d949c4      ldr   x1, [x1, 0x90]                       ; [0x90:4]=0
|      |:   0x06d949c8      blr   x1
|      |:   0x06d949cc      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949d0      mov   x0, x1
|      |:   0x06d949d4      adrp  x1, 0x42a6000
|      |:   0x06d949d8      add   x1, x1, 0xe15
|      |:   0x06d949dc      mov   x3, x2
|      |:   0x06d949e0      mov   x2, 0xa
|      |:   0x06d949e4      blr   x3
|      |:   0x06d949e8      ldr   x2, [x0, 0x30]                       ; [0x30:4]=0
|      |:   0x06d949ec      mov   x0, x1
|      |:   0x06d949f0      blr   x2
|      |:   0x06d949f4      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949f8      mov   x0, x1
|      |:   0x06d949fc      adrp  x1, 0x4237000
|      |:   0x06d94a00      add   x1, x1, 0x7c0
|      |:   0x06d94a04      mov   x3, x2
|      |:   0x06d94a08      orr   x2, xzr, 3
|      |:   0x06d94a0c      blr   x3
|      |:   0x06d94a10      str   x1, [var_58h]
|     ,===< 0x06d94a14      cbz   x0, 0x6d94a1c
|     ||:   0x06d94a18      ldr   x0, [x0, 8]                          ; [0x8:4]=0
|     `---> 0x06d94a1c      str   x0, [var_30h]
|      |:   0x06d94a20      orr   x0, xzr, 0x10
|      |:   0x06d94a24      adrp  x1, 0x8744000
|      |:   0x06d94a28      add   x1, x1, 0xba0
|      |:   0x06d94a2c      orr   x2, xzr, 1
|      |:   0x06d94a30      bl    0x4d32500
|      |:   0x06d94a34      ldr   x3, [var_30h]                        ; [0x70:4]=8
|      |:   0x06d94a38      str   x3, [x0]
|      |:   0x06d94a3c      adrp  x27, 0xb48f000
|      |:   0x06d94a40      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a44      cbnz  w3, 0x6d94a50
|     ||:   0x06d94a48      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    ,====< 0x06d94a4c      b     0x6d94a5c
|    |`---> 0x06d94a50      bl    0x4da34f0
|    | |:   0x06d94a54      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    | |:   0x06d94a58      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a4c
|    `----> 0x06d94a5c      str   x0, [var_38h]
|      |:   0x06d94a60      str   x3, [x0, 8]
|      |:   0x06d94a64      nop
|      |:   0x06d94a68      orr   x0, xzr, 0x18
|      |:   0x06d94a6c      adrp  x1, 0x8744000
|      |:   0x06d94a70      add   x1, x1, 0xc18
|      |:   0x06d94a74      orr   x2, xzr, 1
|      |:   0x06d94a78      bl    0x4d32830
|      |:   0x06d94a7c      ldp   x3, x4, [arg_20h]
|      |:   0x06d94a80      stp   x3, x4, [x0, 8]
|      |:   0x06d94a84      adrp  x27, 0xb48f000
|      |:   0x06d94a88      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a8c      cbnz  w3, 0x6d94a98
|     ||:   0x06d94a90      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    ,====< 0x06d94a94      b     0x6d94aa4
|    |`---> 0x06d94a98      bl    0x4da34f0
|    | |:   0x06d94a9c      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    | |:   0x06d94aa0      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a94
|    `----> 0x06d94aa4      str   x0, [var_40h]
|      |:   0x06d94aa8      str   x3, [x0]
|      |:   0x06d94aac      ldr   x3, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ab0      ldrsb x27, [x3]
|      |:   0x06d94ab4      orr   x0, xzr, 0x18
|      |:   0x06d94ab8      adrp  x1, 0x87fc000
|      |:   0x06d94abc      add   x1, x1, 0x498
|      |:   0x06d94ac0      orr   x2, xzr, 1
|      |:   0x06d94ac4      bl    0x4d32830
|      |:   0x06d94ac8      nop
|      |:   0x06d94acc      adr   x3, 0x6d95f00
|      |:   0x06d94ad0      ldr   x4, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ad4      stp   x3, x4, [x0]
|      |:   0x06d94ad8      adrp  x27, 0xb48f000
|      |:   0x06d94adc      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94ae0      cbnz  w3, 0x6d94aec
|     ||:   0x06d94ae4      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    ,====< 0x06d94ae8      b     0x6d94af8
|    |`---> 0x06d94aec      bl    0x4da34f0
|    | |:   0x06d94af0      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    | |:   0x06d94af4      str   x7, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94ae8
|    `----> 0x06d94af8      str   x7, [x0, 0x10]
|      |:   0x06d94afc      add   x4, var_28h
|      |:   0x06d94b00      stp   xzr, xzr, [x4]
|      |:   0x06d94b04      stp   xzr, xzr, [x4, 0x10]
|      |:   0x06d94b08      adrp  x7, 0x94a8000
|      |:   0x06d94b0c      add   x7, x7, 0x4c0
|      |:   0x06d94b10      ldar  x8, [x7]
|      |:   0x06d94b14      ldr   x9, [x8]
|      |:   0x06d94b18      adrp  x27, 0x8f54000
|      |:   0x06d94b1c      ldr   w10, [x27, 0x930]                    ; [0x930:4]=0x58585858
|     .---> 0x06d94b20      and   x11, x10, x9
|     :|:   0x06d94b24      lsl   x11, x11, 4
|     :|:   0x06d94b28      add   x11, x11, 8
|     :|:   0x06d94b2c      ldr   x12, [x8, x11]
|     :|:   0x06d94b30      add   x11, x8, x11
|     :|:   0x06d94b34      adrp  x1, 0x7eb5000
|     :|:   0x06d94b38      add   x1, x1, 0xe38
|     :|:   0x06d94b3c      cmp   x12, x1
|    ,====< 0x06d94b40      b.eq  0x6d94b68
|    |:|:   0x06d94b44      add   x10, x10, 1
|    |`===< 0x06d94b48      cbnz  x12, 0x6d94b20
|    | |:   0x06d94b4c      str   x0, [var_48h]
|    | |:   0x06d94b50      mov   x0, x7
|    | |:   0x06d94b54      bl    0x4d2e4c0
|    | |:   0x06d94b58      add   x4, var_28h
|    | |:   0x06d94b5c      mov   x7, x0
|    | |:   0x06d94b60      ldr   x0, [var_48h]                        ; [0x58:4]=64
|    |,===< 0x06d94b64      b     0x6d94b6c
|    `----> 0x06d94b68      ldr   x7, [x11, 8]                         ; [0x8:4]=0
|     ||:   ; CODE XREF from fcn.06d94998 @ 0x6d94b64
|     `---> 0x06d94b6c      ldr   x8, [var_38h]                        ; [0x68:4]=0x380
|      |:   0x06d94b70      stp   x7, x8, [var_28h]
|      |:   0x06d94b74      adrp  x7, 0x8f0a000
|      |:   0x06d94b78      add   x7, x7, 0x408
|      |:   0x06d94b7c      ldr   x8, [var_40h]                        ; [0x60:4]=0x380
|      |:   0x06d94b80      stp   x7, x8, [var_28h + 0x10]
|      |:   0x06d94b84      adrp  x1, 0x43be000
|      |:   0x06d94b88      add   x1, x1, 0xf1e
|      |:   0x06d94b8c      mov   x2, 0x1a
|      |:   0x06d94b90      mov   x3, x0
|      |:   0x06d94b94      orr   x5, xzr, 2
|      |:   0x06d94b98      mov   x6, x5
|      |:   0x06d94b9c      adrp  x0, 0x79ca000
|      |:   0x06d94ba0      add   x0, x0, 0x800
|      |:   0x06d94ba4      bl    0x6d95270
|      |:   0x06d94ba8      str   x0, [var_50h]
|      |:   0x06d94bac      orr   x0, xzr, 0x10
|      |:   0x06d94bb0      adrp  x1, 0x87fc000
|      |:   0x06d94bb4      add   x1, x1, 0x518
|      |:   0x06d94bb8      orr   x2, xzr, 1
|      |:   0x06d94bbc      bl    0x4d32500
|      |:   0x06d94bc0      nop
|      |:   0x06d94bc4      adr   x7, 0x6d94c40
|      |:   0x06d94bc8      str   x7, [x0]
|      |:   0x06d94bcc      adrp  x27, 0xb48f000
|      |:   0x06d94bd0      ldr   w7, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94bd4      cbnz  w7, 0x6d94be0
|     ||:   0x06d94bd8      ldr   x4, [arg_50h]                        ; [0x50:4]=64
|    ,====< 0x06d94bdc      b     0x6d94bec
|    |`---> 0x06d94be0      bl    0x4da34f0
|    | |:   0x06d94be4      ldr   x4, [var_50h]                        ; [0x50:4]=64
|    | |:   0x06d94be8      str   x4, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94bdc
|    `----> 0x06d94bec      str   x4, [x0, 8]
|      |:   0x06d94bf0      mov   x1, 0x17
|      |:   0x06d94bf4      adrp  x2, 0x8f07000
|      |:   0x06d94bf8      add   x2, x2, 0x728
|      |:   0x06d94bfc      mov   x3, x0
|      |:   0x06d94c00      adrp  x0, 0x439b000
|      |:   0x06d94c04      add   x0, x0, 0x4fe
|      |:   0x06d94c08      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |:   0x06d94c0c      ldr   lr, [sp], 0xa0                       ; [0xa0:4]=0x779b960
|      |:   0x06d94c10      ret
|      `--> 0x06d94c14      stp   x0, x1, [arg_8h]                     ; arg2
|       :   0x06d94c18      stp   x2, x3, [arg_18h]                    ; arg4
|       :   0x06d94c1c      str   x4, [arg_28h]                        ; arg5
|       :   0x06d94c20      mov   x3, lr
|       :   0x06d94c24      bl    fcn.04da15a0
|       :   0x06d94c28      ldp   x0, x1, [arg_8h]
|       :   0x06d94c2c      ldp   x2, x3, [arg_18h]
|       :   0x06d94c30      ldr   x4, [arg_28h]                        ; [0x28:4]=0xace39d8
|       `=< 0x06d94c34      b     0x6d94990
==== AuthProvider_.ResolveAuthMode
/ fcn.06d94998(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg4, int64_t arg5, int64_t arg_8h, int64_t arg_10h, int64_t arg_18h, int64_t arg_20h, int64_t arg_28h, int64_t arg_50h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; arg int64_t arg2 @ x1
|       :   ; arg int64_t arg3 @ x2
|       :   ; arg int64_t arg4 @ x3
|       :   ; arg int64_t arg5 @ x4
|       :   ; var int64_t var_58h @ stack - 0x58
|       :   ; var int64_t var_50h @ stack - 0x50
|       :   ; var int64_t var_48h @ stack - 0x48
|       :   ; var int64_t var_40h @ stack - 0x40
|       :   ; var int64_t var_38h @ stack - 0x38
|       :   ; var int64_t var_30h @ stack - 0x30
|       :   ; var int64_t var_28h @ stack - 0x28
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_10h @ stack + 0x10
|       :   ; arg int64_t arg_18h @ stack + 0x18
|       :   ; arg int64_t arg_20h @ stack + 0x20
|       :   ; arg int64_t arg_28h @ stack + 0x28
|       :   ; arg int64_t arg_50h @ stack + 0x50
|       :   0x06d94998      cmp   x17, x16
|      ,==< 0x06d9499c      b.ls  0x6d94c14
|      |:   0x06d949a0      str   lr, [sp, -0xa0]!
|      |:   0x06d949a4      stur  fp, [sp, -8]
|      |:   0x06d949a8      sub   fp, sp, 8
|      |:   0x06d949ac      stp   x1, x2, [arg_10h]
|      |:   0x06d949b0      str   x0, [arg_8h]
|      |:   0x06d949b4      stp   x3, x4, [arg_20h]
|      |:   0x06d949b8      adrp  x27, 0x9583000
|      |:   0x06d949bc      add   x27, x27, 0x520
|      |:   0x06d949c0      ldp   x1, x0, [x27]
|      |:   0x06d949c4      ldr   x1, [x1, 0x90]                       ; [0x90:4]=0
|      |:   0x06d949c8      blr   x1
|      |:   0x06d949cc      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949d0      mov   x0, x1
|      |:   0x06d949d4      adrp  x1, 0x42a6000
|      |:   0x06d949d8      add   x1, x1, 0xe15
|      |:   0x06d949dc      mov   x3, x2
|      |:   0x06d949e0      mov   x2, 0xa
|      |:   0x06d949e4      blr   x3
|      |:   0x06d949e8      ldr   x2, [x0, 0x30]                       ; [0x30:4]=0
|      |:   0x06d949ec      mov   x0, x1
|      |:   0x06d949f0      blr   x2
|      |:   0x06d949f4      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949f8      mov   x0, x1
|      |:   0x06d949fc      adrp  x1, 0x4237000
|      |:   0x06d94a00      add   x1, x1, 0x7c0
|      |:   0x06d94a04      mov   x3, x2
|      |:   0x06d94a08      orr   x2, xzr, 3
|      |:   0x06d94a0c      blr   x3
|      |:   0x06d94a10      str   x1, [var_58h]
|     ,===< 0x06d94a14      cbz   x0, 0x6d94a1c
|     ||:   0x06d94a18      ldr   x0, [x0, 8]                          ; [0x8:4]=0
|     `---> 0x06d94a1c      str   x0, [var_30h]
|      |:   0x06d94a20      orr   x0, xzr, 0x10
|      |:   0x06d94a24      adrp  x1, 0x8744000
|      |:   0x06d94a28      add   x1, x1, 0xba0
|      |:   0x06d94a2c      orr   x2, xzr, 1
|      |:   0x06d94a30      bl    0x4d32500
|      |:   0x06d94a34      ldr   x3, [var_30h]                        ; [0x70:4]=8
|      |:   0x06d94a38      str   x3, [x0]
|      |:   0x06d94a3c      adrp  x27, 0xb48f000
|      |:   0x06d94a40      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a44      cbnz  w3, 0x6d94a50
|     ||:   0x06d94a48      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    ,====< 0x06d94a4c      b     0x6d94a5c
|    |`---> 0x06d94a50      bl    0x4da34f0
|    | |:   0x06d94a54      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    | |:   0x06d94a58      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a4c
|    `----> 0x06d94a5c      str   x0, [var_38h]
|      |:   0x06d94a60      str   x3, [x0, 8]
|      |:   0x06d94a64      nop
|      |:   0x06d94a68      orr   x0, xzr, 0x18
|      |:   0x06d94a6c      adrp  x1, 0x8744000
|      |:   0x06d94a70      add   x1, x1, 0xc18
|      |:   0x06d94a74      orr   x2, xzr, 1
|      |:   0x06d94a78      bl    0x4d32830
|      |:   0x06d94a7c      ldp   x3, x4, [arg_20h]
|      |:   0x06d94a80      stp   x3, x4, [x0, 8]
|      |:   0x06d94a84      adrp  x27, 0xb48f000
|      |:   0x06d94a88      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a8c      cbnz  w3, 0x6d94a98
|     ||:   0x06d94a90      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    ,====< 0x06d94a94      b     0x6d94aa4
|    |`---> 0x06d94a98      bl    0x4da34f0
|    | |:   0x06d94a9c      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    | |:   0x06d94aa0      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a94
|    `----> 0x06d94aa4      str   x0, [var_40h]
|      |:   0x06d94aa8      str   x3, [x0]
|      |:   0x06d94aac      ldr   x3, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ab0      ldrsb x27, [x3]
|      |:   0x06d94ab4      orr   x0, xzr, 0x18
|      |:   0x06d94ab8      adrp  x1, 0x87fc000
|      |:   0x06d94abc      add   x1, x1, 0x498
|      |:   0x06d94ac0      orr   x2, xzr, 1
|      |:   0x06d94ac4      bl    0x4d32830
|      |:   0x06d94ac8      nop
|      |:   0x06d94acc      adr   x3, 0x6d95f00
|      |:   0x06d94ad0      ldr   x4, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ad4      stp   x3, x4, [x0]
|      |:   0x06d94ad8      adrp  x27, 0xb48f000
|      |:   0x06d94adc      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94ae0      cbnz  w3, 0x6d94aec
|     ||:   0x06d94ae4      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    ,====< 0x06d94ae8      b     0x6d94af8
|    |`---> 0x06d94aec      bl    0x4da34f0
|    | |:   0x06d94af0      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    | |:   0x06d94af4      str   x7, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94ae8
|    `----> 0x06d94af8      str   x7, [x0, 0x10]
|      |:   0x06d94afc      add   x4, var_28h
|      |:   0x06d94b00      stp   xzr, xzr, [x4]
|      |:   0x06d94b04      stp   xzr, xzr, [x4, 0x10]
|      |:   0x06d94b08      adrp  x7, 0x94a8000
|      |:   0x06d94b0c      add   x7, x7, 0x4c0
|      |:   0x06d94b10      ldar  x8, [x7]
|      |:   0x06d94b14      ldr   x9, [x8]
|      |:   0x06d94b18      adrp  x27, 0x8f54000
|      |:   0x06d94b1c      ldr   w10, [x27, 0x930]                    ; [0x930:4]=0x58585858
|     .---> 0x06d94b20      and   x11, x10, x9
|     :|:   0x06d94b24      lsl   x11, x11, 4
|     :|:   0x06d94b28      add   x11, x11, 8
|     :|:   0x06d94b2c      ldr   x12, [x8, x11]
|     :|:   0x06d94b30      add   x11, x8, x11
|     :|:   0x06d94b34      adrp  x1, 0x7eb5000
|     :|:   0x06d94b38      add   x1, x1, 0xe38
|     :|:   0x06d94b3c      cmp   x12, x1
|    ,====< 0x06d94b40      b.eq  0x6d94b68
|    |:|:   0x06d94b44      add   x10, x10, 1
|    |`===< 0x06d94b48      cbnz  x12, 0x6d94b20
|    | |:   0x06d94b4c      str   x0, [var_48h]
|    | |:   0x06d94b50      mov   x0, x7
|    | |:   0x06d94b54      bl    0x4d2e4c0
|    | |:   0x06d94b58      add   x4, var_28h
|    | |:   0x06d94b5c      mov   x7, x0
|    | |:   0x06d94b60      ldr   x0, [var_48h]                        ; [0x58:4]=64
|    |,===< 0x06d94b64      b     0x6d94b6c
|    `----> 0x06d94b68      ldr   x7, [x11, 8]                         ; [0x8:4]=0
|     ||:   ; CODE XREF from fcn.06d94998 @ 0x6d94b64
|     `---> 0x06d94b6c      ldr   x8, [var_38h]                        ; [0x68:4]=0x380
|      |:   0x06d94b70      stp   x7, x8, [var_28h]
|      |:   0x06d94b74      adrp  x7, 0x8f0a000
|      |:   0x06d94b78      add   x7, x7, 0x408
|      |:   0x06d94b7c      ldr   x8, [var_40h]                        ; [0x60:4]=0x380
|      |:   0x06d94b80      stp   x7, x8, [var_28h + 0x10]
|      |:   0x06d94b84      adrp  x1, 0x43be000
|      |:   0x06d94b88      add   x1, x1, 0xf1e
|      |:   0x06d94b8c      mov   x2, 0x1a
|      |:   0x06d94b90      mov   x3, x0
|      |:   0x06d94b94      orr   x5, xzr, 2
|      |:   0x06d94b98      mov   x6, x5
|      |:   0x06d94b9c      adrp  x0, 0x79ca000
|      |:   0x06d94ba0      add   x0, x0, 0x800
|      |:   0x06d94ba4      bl    0x6d95270
|      |:   0x06d94ba8      str   x0, [var_50h]
|      |:   0x06d94bac      orr   x0, xzr, 0x10
|      |:   0x06d94bb0      adrp  x1, 0x87fc000
|      |:   0x06d94bb4      add   x1, x1, 0x518
|      |:   0x06d94bb8      orr   x2, xzr, 1
|      |:   0x06d94bbc      bl    0x4d32500
|      |:   0x06d94bc0      nop
|      |:   0x06d94bc4      adr   x7, 0x6d94c40
|      |:   0x06d94bc8      str   x7, [x0]
|      |:   0x06d94bcc      adrp  x27, 0xb48f000
|      |:   0x06d94bd0      ldr   w7, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94bd4      cbnz  w7, 0x6d94be0
|     ||:   0x06d94bd8      ldr   x4, [arg_50h]                        ; [0x50:4]=64
|    ,====< 0x06d94bdc      b     0x6d94bec
|    |`---> 0x06d94be0      bl    0x4da34f0
|    | |:   0x06d94be4      ldr   x4, [var_50h]                        ; [0x50:4]=64
|    | |:   0x06d94be8      str   x4, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94bdc
|    `----> 0x06d94bec      str   x4, [x0, 8]
|      |:   0x06d94bf0      mov   x1, 0x17
|      |:   0x06d94bf4      adrp  x2, 0x8f07000
|      |:   0x06d94bf8      add   x2, x2, 0x728
|      |:   0x06d94bfc      mov   x3, x0
|      |:   0x06d94c00      adrp  x0, 0x439b000
|      |:   0x06d94c04      add   x0, x0, 0x4fe
|      |:   0x06d94c08      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |:   0x06d94c0c      ldr   lr, [sp], 0xa0                       ; [0xa0:4]=0x779b960
|      |:   0x06d94c10      ret
|      `--> 0x06d94c14      stp   x0, x1, [arg_8h]                     ; arg2
|       :   0x06d94c18      stp   x2, x3, [arg_18h]                    ; arg4
|       :   0x06d94c1c      str   x4, [arg_28h]                        ; arg5
|       :   0x06d94c20      mov   x3, lr
|       :   0x06d94c24      bl    fcn.04da15a0
|       :   0x06d94c28      ldp   x0, x1, [arg_8h]
|       :   0x06d94c2c      ldp   x2, x3, [arg_18h]
|       :   0x06d94c30      ldr   x4, [arg_28h]                        ; [0x28:4]=0xace39d8
|       `=< 0x06d94c34      b     0x6d94990
==== AuthProvider_.GetHTTPClient
/ fcn.06d94998(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg4, int64_t arg5, int64_t arg_8h, int64_t arg_10h, int64_t arg_18h, int64_t arg_20h, int64_t arg_28h, int64_t arg_50h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; arg int64_t arg2 @ x1
|       :   ; arg int64_t arg3 @ x2
|       :   ; arg int64_t arg4 @ x3
|       :   ; arg int64_t arg5 @ x4
|       :   ; var int64_t var_58h @ stack - 0x58
|       :   ; var int64_t var_50h @ stack - 0x50
|       :   ; var int64_t var_48h @ stack - 0x48
|       :   ; var int64_t var_40h @ stack - 0x40
|       :   ; var int64_t var_38h @ stack - 0x38
|       :   ; var int64_t var_30h @ stack - 0x30
|       :   ; var int64_t var_28h @ stack - 0x28
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_10h @ stack + 0x10
|       :   ; arg int64_t arg_18h @ stack + 0x18
|       :   ; arg int64_t arg_20h @ stack + 0x20
|       :   ; arg int64_t arg_28h @ stack + 0x28
|       :   ; arg int64_t arg_50h @ stack + 0x50
|       :   0x06d94998      cmp   x17, x16
|      ,==< 0x06d9499c      b.ls  0x6d94c14
|      |:   0x06d949a0      str   lr, [sp, -0xa0]!
|      |:   0x06d949a4      stur  fp, [sp, -8]
|      |:   0x06d949a8      sub   fp, sp, 8
|      |:   0x06d949ac      stp   x1, x2, [arg_10h]
|      |:   0x06d949b0      str   x0, [arg_8h]
|      |:   0x06d949b4      stp   x3, x4, [arg_20h]
|      |:   0x06d949b8      adrp  x27, 0x9583000
|      |:   0x06d949bc      add   x27, x27, 0x520
|      |:   0x06d949c0      ldp   x1, x0, [x27]
|      |:   0x06d949c4      ldr   x1, [x1, 0x90]                       ; [0x90:4]=0
|      |:   0x06d949c8      blr   x1
|      |:   0x06d949cc      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949d0      mov   x0, x1
|      |:   0x06d949d4      adrp  x1, 0x42a6000
|      |:   0x06d949d8      add   x1, x1, 0xe15
|      |:   0x06d949dc      mov   x3, x2
|      |:   0x06d949e0      mov   x2, 0xa
|      |:   0x06d949e4      blr   x3
|      |:   0x06d949e8      ldr   x2, [x0, 0x30]                       ; [0x30:4]=0
|      |:   0x06d949ec      mov   x0, x1
|      |:   0x06d949f0      blr   x2
|      |:   0x06d949f4      ldr   x2, [x0, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |:   0x06d949f8      mov   x0, x1
|      |:   0x06d949fc      adrp  x1, 0x4237000
|      |:   0x06d94a00      add   x1, x1, 0x7c0
|      |:   0x06d94a04      mov   x3, x2
|      |:   0x06d94a08      orr   x2, xzr, 3
|      |:   0x06d94a0c      blr   x3
|      |:   0x06d94a10      str   x1, [var_58h]
|     ,===< 0x06d94a14      cbz   x0, 0x6d94a1c
|     ||:   0x06d94a18      ldr   x0, [x0, 8]                          ; [0x8:4]=0
|     `---> 0x06d94a1c      str   x0, [var_30h]
|      |:   0x06d94a20      orr   x0, xzr, 0x10
|      |:   0x06d94a24      adrp  x1, 0x8744000
|      |:   0x06d94a28      add   x1, x1, 0xba0
|      |:   0x06d94a2c      orr   x2, xzr, 1
|      |:   0x06d94a30      bl    0x4d32500
|      |:   0x06d94a34      ldr   x3, [var_30h]                        ; [0x70:4]=8
|      |:   0x06d94a38      str   x3, [x0]
|      |:   0x06d94a3c      adrp  x27, 0xb48f000
|      |:   0x06d94a40      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a44      cbnz  w3, 0x6d94a50
|     ||:   0x06d94a48      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    ,====< 0x06d94a4c      b     0x6d94a5c
|    |`---> 0x06d94a50      bl    0x4da34f0
|    | |:   0x06d94a54      ldr   x3, [var_58h]                        ; [0x48:4]=64
|    | |:   0x06d94a58      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a4c
|    `----> 0x06d94a5c      str   x0, [var_38h]
|      |:   0x06d94a60      str   x3, [x0, 8]
|      |:   0x06d94a64      nop
|      |:   0x06d94a68      orr   x0, xzr, 0x18
|      |:   0x06d94a6c      adrp  x1, 0x8744000
|      |:   0x06d94a70      add   x1, x1, 0xc18
|      |:   0x06d94a74      orr   x2, xzr, 1
|      |:   0x06d94a78      bl    0x4d32830
|      |:   0x06d94a7c      ldp   x3, x4, [arg_20h]
|      |:   0x06d94a80      stp   x3, x4, [x0, 8]
|      |:   0x06d94a84      adrp  x27, 0xb48f000
|      |:   0x06d94a88      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94a8c      cbnz  w3, 0x6d94a98
|     ||:   0x06d94a90      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    ,====< 0x06d94a94      b     0x6d94aa4
|    |`---> 0x06d94a98      bl    0x4da34f0
|    | |:   0x06d94a9c      ldr   x3, [arg_18h]                        ; [0xb8:4]=0
|    | |:   0x06d94aa0      str   x3, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94a94
|    `----> 0x06d94aa4      str   x0, [var_40h]
|      |:   0x06d94aa8      str   x3, [x0]
|      |:   0x06d94aac      ldr   x3, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ab0      ldrsb x27, [x3]
|      |:   0x06d94ab4      orr   x0, xzr, 0x18
|      |:   0x06d94ab8      adrp  x1, 0x87fc000
|      |:   0x06d94abc      add   x1, x1, 0x498
|      |:   0x06d94ac0      orr   x2, xzr, 1
|      |:   0x06d94ac4      bl    0x4d32830
|      |:   0x06d94ac8      nop
|      |:   0x06d94acc      adr   x3, 0x6d95f00
|      |:   0x06d94ad0      ldr   x4, [arg_8h]                         ; [0xa8:4]=0x200000
|      |:   0x06d94ad4      stp   x3, x4, [x0]
|      |:   0x06d94ad8      adrp  x27, 0xb48f000
|      |:   0x06d94adc      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94ae0      cbnz  w3, 0x6d94aec
|     ||:   0x06d94ae4      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    ,====< 0x06d94ae8      b     0x6d94af8
|    |`---> 0x06d94aec      bl    0x4da34f0
|    | |:   0x06d94af0      ldr   x7, [arg_10h]                        ; [0xb0:4]=0x6474e551
|    | |:   0x06d94af4      str   x7, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94ae8
|    `----> 0x06d94af8      str   x7, [x0, 0x10]
|      |:   0x06d94afc      add   x4, var_28h
|      |:   0x06d94b00      stp   xzr, xzr, [x4]
|      |:   0x06d94b04      stp   xzr, xzr, [x4, 0x10]
|      |:   0x06d94b08      adrp  x7, 0x94a8000
|      |:   0x06d94b0c      add   x7, x7, 0x4c0
|      |:   0x06d94b10      ldar  x8, [x7]
|      |:   0x06d94b14      ldr   x9, [x8]
|      |:   0x06d94b18      adrp  x27, 0x8f54000
|      |:   0x06d94b1c      ldr   w10, [x27, 0x930]                    ; [0x930:4]=0x58585858
|     .---> 0x06d94b20      and   x11, x10, x9
|     :|:   0x06d94b24      lsl   x11, x11, 4
|     :|:   0x06d94b28      add   x11, x11, 8
|     :|:   0x06d94b2c      ldr   x12, [x8, x11]
|     :|:   0x06d94b30      add   x11, x8, x11
|     :|:   0x06d94b34      adrp  x1, 0x7eb5000
|     :|:   0x06d94b38      add   x1, x1, 0xe38
|     :|:   0x06d94b3c      cmp   x12, x1
|    ,====< 0x06d94b40      b.eq  0x6d94b68
|    |:|:   0x06d94b44      add   x10, x10, 1
|    |`===< 0x06d94b48      cbnz  x12, 0x6d94b20
|    | |:   0x06d94b4c      str   x0, [var_48h]
|    | |:   0x06d94b50      mov   x0, x7
|    | |:   0x06d94b54      bl    0x4d2e4c0
|    | |:   0x06d94b58      add   x4, var_28h
|    | |:   0x06d94b5c      mov   x7, x0
|    | |:   0x06d94b60      ldr   x0, [var_48h]                        ; [0x58:4]=64
|    |,===< 0x06d94b64      b     0x6d94b6c
|    `----> 0x06d94b68      ldr   x7, [x11, 8]                         ; [0x8:4]=0
|     ||:   ; CODE XREF from fcn.06d94998 @ 0x6d94b64
|     `---> 0x06d94b6c      ldr   x8, [var_38h]                        ; [0x68:4]=0x380
|      |:   0x06d94b70      stp   x7, x8, [var_28h]
|      |:   0x06d94b74      adrp  x7, 0x8f0a000
|      |:   0x06d94b78      add   x7, x7, 0x408
|      |:   0x06d94b7c      ldr   x8, [var_40h]                        ; [0x60:4]=0x380
|      |:   0x06d94b80      stp   x7, x8, [var_28h + 0x10]
|      |:   0x06d94b84      adrp  x1, 0x43be000
|      |:   0x06d94b88      add   x1, x1, 0xf1e
|      |:   0x06d94b8c      mov   x2, 0x1a
|      |:   0x06d94b90      mov   x3, x0
|      |:   0x06d94b94      orr   x5, xzr, 2
|      |:   0x06d94b98      mov   x6, x5
|      |:   0x06d94b9c      adrp  x0, 0x79ca000
|      |:   0x06d94ba0      add   x0, x0, 0x800
|      |:   0x06d94ba4      bl    0x6d95270
|      |:   0x06d94ba8      str   x0, [var_50h]
|      |:   0x06d94bac      orr   x0, xzr, 0x10
|      |:   0x06d94bb0      adrp  x1, 0x87fc000
|      |:   0x06d94bb4      add   x1, x1, 0x518
|      |:   0x06d94bb8      orr   x2, xzr, 1
|      |:   0x06d94bbc      bl    0x4d32500
|      |:   0x06d94bc0      nop
|      |:   0x06d94bc4      adr   x7, 0x6d94c40
|      |:   0x06d94bc8      str   x7, [x0]
|      |:   0x06d94bcc      adrp  x27, 0xb48f000
|      |:   0x06d94bd0      ldr   w7, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d94bd4      cbnz  w7, 0x6d94be0
|     ||:   0x06d94bd8      ldr   x4, [arg_50h]                        ; [0x50:4]=64
|    ,====< 0x06d94bdc      b     0x6d94bec
|    |`---> 0x06d94be0      bl    0x4da34f0
|    | |:   0x06d94be4      ldr   x4, [var_50h]                        ; [0x50:4]=64
|    | |:   0x06d94be8      str   x4, [x25]
|    | |:   ; CODE XREF from fcn.06d94998 @ 0x6d94bdc
|    `----> 0x06d94bec      str   x4, [x0, 8]
|      |:   0x06d94bf0      mov   x1, 0x17
|      |:   0x06d94bf4      adrp  x2, 0x8f07000
|      |:   0x06d94bf8      add   x2, x2, 0x728
|      |:   0x06d94bfc      mov   x3, x0
|      |:   0x06d94c00      adrp  x0, 0x439b000
|      |:   0x06d94c04      add   x0, x0, 0x4fe
|      |:   0x06d94c08      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |:   0x06d94c0c      ldr   lr, [sp], 0xa0                       ; [0xa0:4]=0x779b960
|      |:   0x06d94c10      ret
|      `--> 0x06d94c14      stp   x0, x1, [arg_8h]                     ; arg2
|       :   0x06d94c18      stp   x2, x3, [arg_18h]                    ; arg4
|       :   0x06d94c1c      str   x4, [arg_28h]                        ; arg5
|       :   0x06d94c20      mov   x3, lr
|       :   0x06d94c24      bl    fcn.04da15a0
|       :   0x06d94c28      ldp   x0, x1, [arg_8h]
|       :   0x06d94c2c      ldp   x2, x3, [arg_18h]
|       :   0x06d94c30      ldr   x4, [arg_28h]                        ; [0x28:4]=0xace39d8
|       `=< 0x06d94c34      b     0x6d94990
==== AuthProvider_.GetCACertPool
/ fcn.06d94d58(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg4, int64_t arg7, int64_t arg_38h, int64_t arg_40h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_60h, int64_t arg_68h, int64_t arg_70h, int64_t arg_78h, int64_t arg_88h, int64_t arg_90h, int64_t arg_98h, int64_t arg_a0h, int64_t arg_a8h, int64_t arg_b0h, int64_t arg_b8h, int64_t arg_c0h, int64_t arg_c8h, int64_t arg_138h, int64_t arg_140h, int64_t arg_148h, int64_t arg_150h, int64_t arg_1b0h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg2 @ x1
|           ; arg int64_t arg3 @ x2
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg7 @ x6
|           ; arg int64_t arg_38h @ stack + 0x38
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_48h @ stack + 0x48
|           ; arg int64_t arg_50h @ stack + 0x50
|           ; arg int64_t arg_58h @ stack + 0x58
|           ; arg int64_t arg_60h @ stack + 0x60
|           ; arg int64_t arg_68h @ stack + 0x68
|           ; arg int64_t arg_70h @ stack + 0x70
|           ; arg int64_t arg_78h @ stack + 0x78
|           ; arg int64_t arg_88h @ stack + 0x88
|           ; arg int64_t arg_90h @ stack + 0x90
|           ; arg int64_t arg_98h @ stack + 0x98
|           ; arg int64_t arg_a0h @ stack + 0xa0
|           ; arg int64_t arg_a8h @ stack + 0xa8
|           ; arg int64_t arg_b0h @ stack + 0xb0
|           ; arg int64_t arg_b8h @ stack + 0xb8
|           ; arg int64_t arg_c0h @ stack + 0xc0
|           ; arg int64_t arg_c8h @ stack + 0xc8
|           ; arg int64_t arg_138h @ stack + 0x138
|           ; arg int64_t arg_140h @ stack + 0x140
|           ; arg int64_t arg_148h @ stack + 0x148
|           ; arg int64_t arg_150h @ stack + 0x150
|           ; arg int64_t arg_1b0h @ stack + 0x1b0
|           0x06d94d58      mov   sp, x20
|           0x06d94d5c      sub   fp, sp, 8
|           0x06d94d60      stp   x0, x1, [arg_138h]                   ; arg2
|           0x06d94d64      stp   x2, x3, [arg_148h]                   ; arg4
|           0x06d94d68      ldr   x4, [x26, 0x18]                      ; [0x18:4]=0x4d0f000 section..text
|           0x06d94d6c      str   x4, [arg_b0h]
|           0x06d94d70      ldp   x5, x6, [x26, 8]
|           0x06d94d74      str   x6, [arg_78h + 0x8]                  ; arg7
|           0x06d94d78      ldr   x0, [x4]
|           0x06d94d7c      ldr   x4, [x5, 0x60]                       ; [0x60:4]=0x380
|           0x06d94d80      mov   x1, x2                               ; arg3
|           0x06d94d84      mov   x2, x3                               ; arg4
|           0x06d94d88      mov   x3, x4
|           0x06d94d8c      bl    0x6d954f0
|       ,=< 0x06d94d90      cbnz  x1, 0x6d94e28
|       |   0x06d94d94      ldr   x26, [arg_1b0h]                      ; [0x80:4]=0
|       |   0x06d94d98      ldr   x4, [x26]
|       |   0x06d94d9c      ldr   x5, [arg_b0h]                        ; [0xb0:4]=0x6474e551
|       |   0x06d94da0      ldr   x2, [x5, 0x38]                       ; [0x38:4]=0x400010
|       |   0x06d94da4      ldr   x1, [arg_140h]                       ; [0x140:4]=144
|       |   0x06d94da8      mov   x3, x0
|       |   0x06d94dac      ldr   x0, [arg_138h]                       ; [0x138:4]=0x799b960 segment.GNU_RELRO
|       |   0x06d94db0      blr   x4
|      ,==< 0x06d94db4      cbnz  x2, 0x6d94e14
|      ||   0x06d94db8      str   x1, [arg_88h]
|      ||   0x06d94dbc      str   x0, [arg_48h]
|      ||   0x06d94dc0      ldr   x1, [arg_148h]                       ; [0x148:4]=0x458
|      ||   0x06d94dc4      ldr   x1, [x1, 0x30]                       ; [0x30:4]=0
|      ||   0x06d94dc8      ldr   x0, [arg_150h]                       ; [0x150:4]=32
|      ||   0x06d94dcc      blr   x1
|      ||   0x06d94dd0      str   x0, [arg_a8h]
|      ||   0x06d94dd4      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|      ||   0x06d94dd8      ldr   x1, [x1, 0x20]                       ; [0x20:4]=64 ; "@"
|      ||   0x06d94ddc      ldr   x0, [arg_88h]                        ; [0x88:4]=0
|      ||   0x06d94de0      blr   x1
|      ||   0x06d94de4      add   x2, arg_c8h
|      ||   0x06d94de8      stp   xzr, xzr, [x2]
|      ||   0x06d94dec      stp   xzr, xzr, [x2, 0x10]
|      ||   0x06d94df0      stp   xzr, xzr, [x2, 0x20]
|      ||   0x06d94df4      stp   xzr, xzr, [x2, 0x30]
|      ||   0x06d94df8      stp   xzr, xzr, [x2, 0x40]
|      ||   0x06d94dfc      stp   xzr, xzr, [x2, 0x50]
|      ||   0x06d94e00      mov   x1, x0
|      ||   0x06d94e04      adrp  x0, 0x8dbe000
|      ||   0x06d94e08      add   x0, x0, 0xfe0
|      ||   0x06d94e0c      bl    fcn.04d35800
|     ,===< 0x06d94e10      b     0x6d94e44
|     |`--> 0x06d94e14      mov   x0, x2
|     | |   0x06d94e18      mov   x1, x3
|     | |   0x06d94e1c      ldp   fp, lr, [sp, -8]
|     | |   0x06d94e20      add   sp, sp, 0x130
|     | |   0x06d94e24      ret
|     | `-> 0x06d94e28      mov   x0, x1
|     |     0x06d94e2c      mov   x1, x2
|     |     0x06d94e30      ldp   fp, lr, [sp, -8]
|     |     0x06d94e34      add   sp, sp, 0x130
|     |     0x06d94e38      ret
|     |     ; CODE XREF from fcn.06d94d58 @ 0x6d94f34
|    .-..-> 0x06d94e3c      add   x0, arg_c8h
|    :|::   0x06d94e40      bl    fcn.04d35870
|    :|::   ; CODE XREF from fcn.06d94d58 @ 0x6d94e10
|    :`---> 0x06d94e44      ldp   x4, x5, [arg_c8h]
|    :,===< 0x06d94e48      cbz   x4, 0x6d94f38
|    :|::   0x06d94e4c      ldp   x2, x3, [x4]
|    :|::   0x06d94e50      ldp   x4, x5, [x5]
|    `====< 0x06d94e54      cbz   x5, 0x6d94e3c
|     |::   0x06d94e58      str   x5, [arg_40h]
|     |::   0x06d94e5c      str   x4, [arg_78h]
|     |::   0x06d94e60      str   x3, [arg_58h]
|     |::   0x06d94e64      str   x2, [arg_98h]
|     |::   0x06d94e68      adrp  x27, 0x957b000
|     |::   0x06d94e6c      ldr   x1, [x27, 0xaa8]                     ; [0xaa8:4]=0x58585858
|     |::   0x06d94e70      adrp  x0, 0x8333000
|     |::   0x06d94e74      add   x0, x0, 0x770
|     |::   0x06d94e78      bl    0x4d18bb0
|     |`==< 0x06d94e7c      tbnz  w1, 0, 0x6d94e3c
|     | :   0x06d94e80      adrp  x0, 0x8dbe000
|     | :   0x06d94e84      add   x0, x0, 0xfe0
|     | :   0x06d94e88      ldr   x1, [arg_a8h]                        ; [0xa8:4]=0x200000
|     | :   0x06d94e8c      ldr   x2, [arg_98h]                        ; [0x98:4]=0x779b960
|     | :   0x06d94e90      ldr   x3, [arg_58h]                        ; [0x58:4]=64
|     | :   0x06d94e94      bl    0x4d18e50
|     | :   0x06d94e98      str   x0, [arg_c0h]
|     | :   0x06d94e9c      ldr   x2, [x0, 0x10]                       ; [0x10:4]=0xb70003
|     | :   0x06d94ea0      ldp   x4, x5, [x0]
|     | :   0x06d94ea4      ldr   x3, [arg_40h]                        ; segment.PHDR
|     | :                                                              ; [0x40:4]=6 ; "\U00000006"
|     | :   0x06d94ea8      add   x1, x5, x3
|     | :   0x06d94eac      cmp   x2, x1
|     |,==< 0x06d94eb0      b.hs  0x6d94ed4
|     ||:   0x06d94eb4      str   x5, [arg_68h]
|     ||:   0x06d94eb8      mov   x0, x4
|     ||:   0x06d94ebc      adrp  x4, 0x856f000
|     ||:   0x06d94ec0      add   x4, x4, 0x550
|     ||:   0x06d94ec4      bl    0x4d9e050
|     ||:   0x06d94ec8      ldr   x3, [arg_40h]                        ; segment.PHDR
|     ||:                                                              ; [0x40:4]=6 ; "\U00000006"
|     ||:   0x06d94ecc      ldr   x5, [arg_68h]                        ; [0x68:4]=0x380
|     ||:   0x06d94ed0      mov   x4, x0
|     |`--> 0x06d94ed4      stp   x1, x2, [arg_60h]
|     | :   0x06d94ed8      str   x4, [arg_b8h]
|     | :   0x06d94edc      sub   x6, x5, x2
|     | :   0x06d94ee0      sub   x2, x1, x5
|     | :   0x06d94ee4      and   x5, x5, x6, asr 63
|     | :   0x06d94ee8      add   x1, x4, x5, lsl 4
|     | :   0x06d94eec      adrp  x0, 0x856f000
|     | :   0x06d94ef0      add   x0, x0, 0x550
|     | :   0x06d94ef4      mov   x4, x3
|     | :   0x06d94ef8      ldr   x3, [arg_78h]                        ; [0x78:4]=1
|     | :   0x06d94efc      bl    0x4d99570
|     | :   0x06d94f00      ldp   x5, x6, [arg_60h]
|     | :   0x06d94f04      ldr   x7, [arg_c0h]                        ; [0xc0:4]=0
|     | :   0x06d94f08      stp   x5, x6, [x7, 8]
|     | :   0x06d94f0c      adrp  x27, 0xb48f000
|     | :   0x06d94f10      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     |,==< 0x06d94f14      cbnz  w5, 0x6d94f20
|     ||:   0x06d94f18      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    ,====< 0x06d94f1c      b     0x6d94f30
|    ||`--> 0x06d94f20      ldr   x1, [x7]
|    || :   0x06d94f24      bl    fcn.04da3500
|    || :   0x06d94f28      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    || :   0x06d94f2c      stp   x2, x1, [x25]
|    || :   ; CODE XREF from fcn.06d94d58 @ 0x6d94f1c
|    `----> 0x06d94f30      str   x2, [x7]
|     | `=< 0x06d94f34      b     0x6d94e3c
|     `---> 0x06d94f38      ldr   x1, [arg_148h]                       ; [0x148:4]=0x458
|           0x06d94f3c      ldr   x1, [x1, 0x38]                       ; [0x38:4]=0x400010
|           0x06d94f40      ldr   x0, [arg_150h]                       ; [0x150:4]=32
|           0x06d94f44      blr   x1
|           0x06d94f48      str   x0, [arg_a0h]
|           0x06d94f4c      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|           0x06d94f50      ldr   x1, [x1, 0x28]                       ; [0x28:4]=0xace39d8
|           0x06d94f54      ldr   x0, [arg_88h]                        ; [0x88:4]=0
|           0x06d94f58      blr   x1
|           0x06d94f5c      add   x2, arg_c8h
|           0x06d94f60      stp   xzr, xzr, [x2]
|           0x06d94f64      stp   xzr, xzr, [x2, 0x10]
|           0x06d94f68      stp   xzr, xzr, [x2, 0x20]
|           0x06d94f6c      stp   xzr, xzr, [x2, 0x30]
|           0x06d94f70      stp   xzr, xzr, [x2, 0x40]
|           0x06d94f74      stp   xzr, xzr, [x2, 0x50]
|           0x06d94f78      mov   x1, x0
|           0x06d94f7c      adrp  x0, 0x8dbe000
|           0x06d94f80      add   x0, x0, 0xfe0
|           0x06d94f84      bl    fcn.04d35800
|       ,=< 0x06d94f88      b     0x6d94f94
|       |   ; CODE XREF from fcn.06d94d58 @ 0x6d95088
|    ...--> 0x06d94f8c      add   x0, arg_c8h
|    :::|   0x06d94f90      bl    fcn.04d35870
|    :::|   ; CODE XREF from fcn.06d94d58 @ 0x6d94f88
|    :::`-> 0x06d94f94      ldp   x4, x5, [arg_c8h]
|    :::,=< 0x06d94f98      cbz   x4, 0x6d9508c
|    :::|   0x06d94f9c      ldp   x2, x3, [x4]
|    :::|   0x06d94fa0      ldp   x4, x5, [x5]
|    `====< 0x06d94fa4      cbz   x5, 0x6d94f8c
|     ::|   0x06d94fa8      str   x2, [arg_90h]
|     ::|   0x06d94fac      str   x3, [arg_50h]
|     ::|   0x06d94fb0      str   x4, [arg_70h]
|     ::|   0x06d94fb4      str   x5, [arg_38h]
|     ::|   0x06d94fb8      adrp  x27, 0x957b000
|     ::|   0x06d94fbc      ldr   x1, [x27, 0xaa8]                     ; [0xaa8:4]=0x58585858
|     ::|   0x06d94fc0      adrp  x0, 0x8333000
|     ::|   0x06d94fc4      add   x0, x0, 0x770
|     ::|   0x06d94fc8      bl    0x4d18bb0
|     `===< 0x06d94fcc      tbnz  w1, 0, 0x6d94f8c
|      :|   0x06d94fd0      adrp  x0, 0x8dbe000
|      :|   0x06d94fd4      add   x0, x0, 0xfe0
|      :|   0x06d94fd8      ldr   x1, [arg_a0h]                        ; [0xa0:4]=0x779b960
|      :|   0x06d94fdc      ldr   x2, [arg_90h]                        ; [0x90:4]=0
|      :|   0x06d94fe0      ldr   x3, [arg_50h]                        ; [0x50:4]=64
|      :|   0x06d94fe4      bl    0x4d18e50
|      :|   0x06d94fe8      str   x0, [arg_c0h]
|      :|   0x06d94fec      ldr   x2, [x0, 0x10]                       ; [0x10:4]=0xb70003
|      :|   0x06d94ff0      ldp   x4, x5, [x0]
|      :|   0x06d94ff4      ldr   x3, [arg_38h]                        ; [0x38:4]=0x400010
|      :|   0x06d94ff8      add   x1, x3, x5
|      :|   0x06d94ffc      cmp   x2, x1
|     ,===< 0x06d95000      b.hs  0x6d95024
|     |:|   0x06d95004      str   x5, [arg_68h]
|     |:|   0x06d95008      mov   x0, x4
|     |:|   0x06d9500c      adrp  x4, 0x856f000
|     |:|   0x06d95010      add   x4, x4, 0x550
|     |:|   0x06d95014      bl    0x4d9e050
|     |:|   0x06d95018      ldr   x3, [arg_38h]                        ; [0x38:4]=0x400010
|     |:|   0x06d9501c      ldr   x5, [arg_68h]                        ; [0x68:4]=0x380
|     |:|   0x06d95020      mov   x4, x0
|     `---> 0x06d95024      str   x1, [arg_68h]
|      :|   0x06d95028      str   x4, [arg_b8h]
|      :|   0x06d9502c      str   x2, [arg_60h]
|      :|   0x06d95030      sub   x6, x5, x2
|      :|   0x06d95034      sub   x2, x1, x5
|      :|   0x06d95038      and   x5, x5, x6, asr 63
|      :|   0x06d9503c      add   x1, x4, x5, lsl 4
|      :|   0x06d95040      adrp  x0, 0x856f000
|      :|   0x06d95044      add   x0, x0, 0x550
|      :|   0x06d95048      mov   x4, x3
|      :|   0x06d9504c      ldr   x3, [arg_70h]                        ; [0x70:4]=8
|      :|   0x06d95050      bl    0x4d99570
|      :|   0x06d95054      ldp   x6, x5, [arg_60h]
|      :|   0x06d95058      ldr   x7, [arg_c0h]                        ; [0xc0:4]=0
|      :|   0x06d9505c      stp   x5, x6, [x7, 8]
|      :|   0x06d95060      adrp  x27, 0xb48f000
|      :|   0x06d95064      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d95068      cbnz  w5, 0x6d95074
|     |:|   0x06d9506c      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    ,====< 0x06d95070      b     0x6d95084
|    |`---> 0x06d95074      ldr   x1, [x7]
|    | :|   0x06d95078      bl    fcn.04da3500
|    | :|   0x06d9507c      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    | :|   0x06d95080      stp   x2, x1, [x25]
|    | :|   ; CODE XREF from fcn.06d94d58 @ 0x6d95070
|    `----> 0x06d95084      str   x2, [x7]
|      `==< 0x06d95088      b     0x6d94f8c
|       `-> 0x06d9508c      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|           0x06d95090      ldr   x1, [x1, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|           0x06d95094      ldr   x0, [arg_88h]                        ; [0x88:4]=0
|           0x06d95098      blr   x1
|           0x06d9509c      ldr   x2, [arg_148h]                       ; [0x148:4]=0x458
|           0x06d950a0      ldr   x2, [x2, 0x40]                       ; segment.PHDR
|                                                                      ; [0x40:4]=6 ; "\U00000006"
|           0x06d950a4      mov   x3, x0
|           0x06d950a8      ldr   x0, [arg_150h]                       ; [0x150:4]=32
|           0x06d950ac      mov   x4, x1
|           0x06d950b0      mov   x1, x3
|           0x06d950b4      mov   x5, x2
|           0x06d950b8      mov   x2, x4
|           0x06d950bc      blr   x5
|           0x06d950c0      ldp   fp, lr, [sp, -8]
|           0x06d950c4      add   sp, sp, 0x130
\           0x06d950c8      ret
==== AuthProvider_.GetCACertPath
/ fcn.06d94d58(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg4, int64_t arg7, int64_t arg_38h, int64_t arg_40h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_60h, int64_t arg_68h, int64_t arg_70h, int64_t arg_78h, int64_t arg_88h, int64_t arg_90h, int64_t arg_98h, int64_t arg_a0h, int64_t arg_a8h, int64_t arg_b0h, int64_t arg_b8h, int64_t arg_c0h, int64_t arg_c8h, int64_t arg_138h, int64_t arg_140h, int64_t arg_148h, int64_t arg_150h, int64_t arg_1b0h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg2 @ x1
|           ; arg int64_t arg3 @ x2
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg7 @ x6
|           ; arg int64_t arg_38h @ stack + 0x38
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_48h @ stack + 0x48
|           ; arg int64_t arg_50h @ stack + 0x50
|           ; arg int64_t arg_58h @ stack + 0x58
|           ; arg int64_t arg_60h @ stack + 0x60
|           ; arg int64_t arg_68h @ stack + 0x68
|           ; arg int64_t arg_70h @ stack + 0x70
|           ; arg int64_t arg_78h @ stack + 0x78
|           ; arg int64_t arg_88h @ stack + 0x88
|           ; arg int64_t arg_90h @ stack + 0x90
|           ; arg int64_t arg_98h @ stack + 0x98
|           ; arg int64_t arg_a0h @ stack + 0xa0
|           ; arg int64_t arg_a8h @ stack + 0xa8
|           ; arg int64_t arg_b0h @ stack + 0xb0
|           ; arg int64_t arg_b8h @ stack + 0xb8
|           ; arg int64_t arg_c0h @ stack + 0xc0
|           ; arg int64_t arg_c8h @ stack + 0xc8
|           ; arg int64_t arg_138h @ stack + 0x138
|           ; arg int64_t arg_140h @ stack + 0x140
|           ; arg int64_t arg_148h @ stack + 0x148
|           ; arg int64_t arg_150h @ stack + 0x150
|           ; arg int64_t arg_1b0h @ stack + 0x1b0
|           0x06d94d58      mov   sp, x20
|           0x06d94d5c      sub   fp, sp, 8
|           0x06d94d60      stp   x0, x1, [arg_138h]                   ; arg2
|           0x06d94d64      stp   x2, x3, [arg_148h]                   ; arg4
|           0x06d94d68      ldr   x4, [x26, 0x18]                      ; [0x18:4]=0x4d0f000 section..text
|           0x06d94d6c      str   x4, [arg_b0h]
|           0x06d94d70      ldp   x5, x6, [x26, 8]
|           0x06d94d74      str   x6, [arg_78h + 0x8]                  ; arg7
|           0x06d94d78      ldr   x0, [x4]
|           0x06d94d7c      ldr   x4, [x5, 0x60]                       ; [0x60:4]=0x380
|           0x06d94d80      mov   x1, x2                               ; arg3
|           0x06d94d84      mov   x2, x3                               ; arg4
|           0x06d94d88      mov   x3, x4
|           0x06d94d8c      bl    0x6d954f0
|       ,=< 0x06d94d90      cbnz  x1, 0x6d94e28
|       |   0x06d94d94      ldr   x26, [arg_1b0h]                      ; [0x80:4]=0
|       |   0x06d94d98      ldr   x4, [x26]
|       |   0x06d94d9c      ldr   x5, [arg_b0h]                        ; [0xb0:4]=0x6474e551
|       |   0x06d94da0      ldr   x2, [x5, 0x38]                       ; [0x38:4]=0x400010
|       |   0x06d94da4      ldr   x1, [arg_140h]                       ; [0x140:4]=144
|       |   0x06d94da8      mov   x3, x0
|       |   0x06d94dac      ldr   x0, [arg_138h]                       ; [0x138:4]=0x799b960 segment.GNU_RELRO
|       |   0x06d94db0      blr   x4
|      ,==< 0x06d94db4      cbnz  x2, 0x6d94e14
|      ||   0x06d94db8      str   x1, [arg_88h]
|      ||   0x06d94dbc      str   x0, [arg_48h]
|      ||   0x06d94dc0      ldr   x1, [arg_148h]                       ; [0x148:4]=0x458
|      ||   0x06d94dc4      ldr   x1, [x1, 0x30]                       ; [0x30:4]=0
|      ||   0x06d94dc8      ldr   x0, [arg_150h]                       ; [0x150:4]=32
|      ||   0x06d94dcc      blr   x1
|      ||   0x06d94dd0      str   x0, [arg_a8h]
|      ||   0x06d94dd4      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|      ||   0x06d94dd8      ldr   x1, [x1, 0x20]                       ; [0x20:4]=64 ; "@"
|      ||   0x06d94ddc      ldr   x0, [arg_88h]                        ; [0x88:4]=0
|      ||   0x06d94de0      blr   x1
|      ||   0x06d94de4      add   x2, arg_c8h
|      ||   0x06d94de8      stp   xzr, xzr, [x2]
|      ||   0x06d94dec      stp   xzr, xzr, [x2, 0x10]
|      ||   0x06d94df0      stp   xzr, xzr, [x2, 0x20]
|      ||   0x06d94df4      stp   xzr, xzr, [x2, 0x30]
|      ||   0x06d94df8      stp   xzr, xzr, [x2, 0x40]
|      ||   0x06d94dfc      stp   xzr, xzr, [x2, 0x50]
|      ||   0x06d94e00      mov   x1, x0
|      ||   0x06d94e04      adrp  x0, 0x8dbe000
|      ||   0x06d94e08      add   x0, x0, 0xfe0
|      ||   0x06d94e0c      bl    fcn.04d35800
|     ,===< 0x06d94e10      b     0x6d94e44
|     |`--> 0x06d94e14      mov   x0, x2
|     | |   0x06d94e18      mov   x1, x3
|     | |   0x06d94e1c      ldp   fp, lr, [sp, -8]
|     | |   0x06d94e20      add   sp, sp, 0x130
|     | |   0x06d94e24      ret
|     | `-> 0x06d94e28      mov   x0, x1
|     |     0x06d94e2c      mov   x1, x2
|     |     0x06d94e30      ldp   fp, lr, [sp, -8]
|     |     0x06d94e34      add   sp, sp, 0x130
|     |     0x06d94e38      ret
|     |     ; CODE XREF from fcn.06d94d58 @ 0x6d94f34
|    .-..-> 0x06d94e3c      add   x0, arg_c8h
|    :|::   0x06d94e40      bl    fcn.04d35870
|    :|::   ; CODE XREF from fcn.06d94d58 @ 0x6d94e10
|    :`---> 0x06d94e44      ldp   x4, x5, [arg_c8h]
|    :,===< 0x06d94e48      cbz   x4, 0x6d94f38
|    :|::   0x06d94e4c      ldp   x2, x3, [x4]
|    :|::   0x06d94e50      ldp   x4, x5, [x5]
|    `====< 0x06d94e54      cbz   x5, 0x6d94e3c
|     |::   0x06d94e58      str   x5, [arg_40h]
|     |::   0x06d94e5c      str   x4, [arg_78h]
|     |::   0x06d94e60      str   x3, [arg_58h]
|     |::   0x06d94e64      str   x2, [arg_98h]
|     |::   0x06d94e68      adrp  x27, 0x957b000
|     |::   0x06d94e6c      ldr   x1, [x27, 0xaa8]                     ; [0xaa8:4]=0x58585858
|     |::   0x06d94e70      adrp  x0, 0x8333000
|     |::   0x06d94e74      add   x0, x0, 0x770
|     |::   0x06d94e78      bl    0x4d18bb0
|     |`==< 0x06d94e7c      tbnz  w1, 0, 0x6d94e3c
|     | :   0x06d94e80      adrp  x0, 0x8dbe000
|     | :   0x06d94e84      add   x0, x0, 0xfe0
|     | :   0x06d94e88      ldr   x1, [arg_a8h]                        ; [0xa8:4]=0x200000
|     | :   0x06d94e8c      ldr   x2, [arg_98h]                        ; [0x98:4]=0x779b960
|     | :   0x06d94e90      ldr   x3, [arg_58h]                        ; [0x58:4]=64
|     | :   0x06d94e94      bl    0x4d18e50
|     | :   0x06d94e98      str   x0, [arg_c0h]
|     | :   0x06d94e9c      ldr   x2, [x0, 0x10]                       ; [0x10:4]=0xb70003
|     | :   0x06d94ea0      ldp   x4, x5, [x0]
|     | :   0x06d94ea4      ldr   x3, [arg_40h]                        ; segment.PHDR
|     | :                                                              ; [0x40:4]=6 ; "\U00000006"
|     | :   0x06d94ea8      add   x1, x5, x3
|     | :   0x06d94eac      cmp   x2, x1
|     |,==< 0x06d94eb0      b.hs  0x6d94ed4
|     ||:   0x06d94eb4      str   x5, [arg_68h]
|     ||:   0x06d94eb8      mov   x0, x4
|     ||:   0x06d94ebc      adrp  x4, 0x856f000
|     ||:   0x06d94ec0      add   x4, x4, 0x550
|     ||:   0x06d94ec4      bl    0x4d9e050
|     ||:   0x06d94ec8      ldr   x3, [arg_40h]                        ; segment.PHDR
|     ||:                                                              ; [0x40:4]=6 ; "\U00000006"
|     ||:   0x06d94ecc      ldr   x5, [arg_68h]                        ; [0x68:4]=0x380
|     ||:   0x06d94ed0      mov   x4, x0
|     |`--> 0x06d94ed4      stp   x1, x2, [arg_60h]
|     | :   0x06d94ed8      str   x4, [arg_b8h]
|     | :   0x06d94edc      sub   x6, x5, x2
|     | :   0x06d94ee0      sub   x2, x1, x5
|     | :   0x06d94ee4      and   x5, x5, x6, asr 63
|     | :   0x06d94ee8      add   x1, x4, x5, lsl 4
|     | :   0x06d94eec      adrp  x0, 0x856f000
|     | :   0x06d94ef0      add   x0, x0, 0x550
|     | :   0x06d94ef4      mov   x4, x3
|     | :   0x06d94ef8      ldr   x3, [arg_78h]                        ; [0x78:4]=1
|     | :   0x06d94efc      bl    0x4d99570
|     | :   0x06d94f00      ldp   x5, x6, [arg_60h]
|     | :   0x06d94f04      ldr   x7, [arg_c0h]                        ; [0xc0:4]=0
|     | :   0x06d94f08      stp   x5, x6, [x7, 8]
|     | :   0x06d94f0c      adrp  x27, 0xb48f000
|     | :   0x06d94f10      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     |,==< 0x06d94f14      cbnz  w5, 0x6d94f20
|     ||:   0x06d94f18      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    ,====< 0x06d94f1c      b     0x6d94f30
|    ||`--> 0x06d94f20      ldr   x1, [x7]
|    || :   0x06d94f24      bl    fcn.04da3500
|    || :   0x06d94f28      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    || :   0x06d94f2c      stp   x2, x1, [x25]
|    || :   ; CODE XREF from fcn.06d94d58 @ 0x6d94f1c
|    `----> 0x06d94f30      str   x2, [x7]
|     | `=< 0x06d94f34      b     0x6d94e3c
|     `---> 0x06d94f38      ldr   x1, [arg_148h]                       ; [0x148:4]=0x458
|           0x06d94f3c      ldr   x1, [x1, 0x38]                       ; [0x38:4]=0x400010
|           0x06d94f40      ldr   x0, [arg_150h]                       ; [0x150:4]=32
|           0x06d94f44      blr   x1
|           0x06d94f48      str   x0, [arg_a0h]
|           0x06d94f4c      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|           0x06d94f50      ldr   x1, [x1, 0x28]                       ; [0x28:4]=0xace39d8
|           0x06d94f54      ldr   x0, [arg_88h]                        ; [0x88:4]=0
|           0x06d94f58      blr   x1
|           0x06d94f5c      add   x2, arg_c8h
|           0x06d94f60      stp   xzr, xzr, [x2]
|           0x06d94f64      stp   xzr, xzr, [x2, 0x10]
|           0x06d94f68      stp   xzr, xzr, [x2, 0x20]
|           0x06d94f6c      stp   xzr, xzr, [x2, 0x30]
|           0x06d94f70      stp   xzr, xzr, [x2, 0x40]
|           0x06d94f74      stp   xzr, xzr, [x2, 0x50]
|           0x06d94f78      mov   x1, x0
|           0x06d94f7c      adrp  x0, 0x8dbe000
|           0x06d94f80      add   x0, x0, 0xfe0
|           0x06d94f84      bl    fcn.04d35800
|       ,=< 0x06d94f88      b     0x6d94f94
|       |   ; CODE XREF from fcn.06d94d58 @ 0x6d95088
|    ...--> 0x06d94f8c      add   x0, arg_c8h
|    :::|   0x06d94f90      bl    fcn.04d35870
|    :::|   ; CODE XREF from fcn.06d94d58 @ 0x6d94f88
|    :::`-> 0x06d94f94      ldp   x4, x5, [arg_c8h]
|    :::,=< 0x06d94f98      cbz   x4, 0x6d9508c
|    :::|   0x06d94f9c      ldp   x2, x3, [x4]
|    :::|   0x06d94fa0      ldp   x4, x5, [x5]
|    `====< 0x06d94fa4      cbz   x5, 0x6d94f8c
|     ::|   0x06d94fa8      str   x2, [arg_90h]
|     ::|   0x06d94fac      str   x3, [arg_50h]
|     ::|   0x06d94fb0      str   x4, [arg_70h]
|     ::|   0x06d94fb4      str   x5, [arg_38h]
|     ::|   0x06d94fb8      adrp  x27, 0x957b000
|     ::|   0x06d94fbc      ldr   x1, [x27, 0xaa8]                     ; [0xaa8:4]=0x58585858
|     ::|   0x06d94fc0      adrp  x0, 0x8333000
|     ::|   0x06d94fc4      add   x0, x0, 0x770
|     ::|   0x06d94fc8      bl    0x4d18bb0
|     `===< 0x06d94fcc      tbnz  w1, 0, 0x6d94f8c
|      :|   0x06d94fd0      adrp  x0, 0x8dbe000
|      :|   0x06d94fd4      add   x0, x0, 0xfe0
|      :|   0x06d94fd8      ldr   x1, [arg_a0h]                        ; [0xa0:4]=0x779b960
|      :|   0x06d94fdc      ldr   x2, [arg_90h]                        ; [0x90:4]=0
|      :|   0x06d94fe0      ldr   x3, [arg_50h]                        ; [0x50:4]=64
|      :|   0x06d94fe4      bl    0x4d18e50
|      :|   0x06d94fe8      str   x0, [arg_c0h]
|      :|   0x06d94fec      ldr   x2, [x0, 0x10]                       ; [0x10:4]=0xb70003
|      :|   0x06d94ff0      ldp   x4, x5, [x0]
|      :|   0x06d94ff4      ldr   x3, [arg_38h]                        ; [0x38:4]=0x400010
|      :|   0x06d94ff8      add   x1, x3, x5
|      :|   0x06d94ffc      cmp   x2, x1
|     ,===< 0x06d95000      b.hs  0x6d95024
|     |:|   0x06d95004      str   x5, [arg_68h]
|     |:|   0x06d95008      mov   x0, x4
|     |:|   0x06d9500c      adrp  x4, 0x856f000
|     |:|   0x06d95010      add   x4, x4, 0x550
|     |:|   0x06d95014      bl    0x4d9e050
|     |:|   0x06d95018      ldr   x3, [arg_38h]                        ; [0x38:4]=0x400010
|     |:|   0x06d9501c      ldr   x5, [arg_68h]                        ; [0x68:4]=0x380
|     |:|   0x06d95020      mov   x4, x0
|     `---> 0x06d95024      str   x1, [arg_68h]
|      :|   0x06d95028      str   x4, [arg_b8h]
|      :|   0x06d9502c      str   x2, [arg_60h]
|      :|   0x06d95030      sub   x6, x5, x2
|      :|   0x06d95034      sub   x2, x1, x5
|      :|   0x06d95038      and   x5, x5, x6, asr 63
|      :|   0x06d9503c      add   x1, x4, x5, lsl 4
|      :|   0x06d95040      adrp  x0, 0x856f000
|      :|   0x06d95044      add   x0, x0, 0x550
|      :|   0x06d95048      mov   x4, x3
|      :|   0x06d9504c      ldr   x3, [arg_70h]                        ; [0x70:4]=8
|      :|   0x06d95050      bl    0x4d99570
|      :|   0x06d95054      ldp   x6, x5, [arg_60h]
|      :|   0x06d95058      ldr   x7, [arg_c0h]                        ; [0xc0:4]=0
|      :|   0x06d9505c      stp   x5, x6, [x7, 8]
|      :|   0x06d95060      adrp  x27, 0xb48f000
|      :|   0x06d95064      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d95068      cbnz  w5, 0x6d95074
|     |:|   0x06d9506c      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    ,====< 0x06d95070      b     0x6d95084
|    |`---> 0x06d95074      ldr   x1, [x7]
|    | :|   0x06d95078      bl    fcn.04da3500
|    | :|   0x06d9507c      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    | :|   0x06d95080      stp   x2, x1, [x25]
|    | :|   ; CODE XREF from fcn.06d94d58 @ 0x6d95070
|    `----> 0x06d95084      str   x2, [x7]
|      `==< 0x06d95088      b     0x6d94f8c
|       `-> 0x06d9508c      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|           0x06d95090      ldr   x1, [x1, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|           0x06d95094      ldr   x0, [arg_88h]                        ; [0x88:4]=0
|           0x06d95098      blr   x1
|           0x06d9509c      ldr   x2, [arg_148h]                       ; [0x148:4]=0x458
|           0x06d950a0      ldr   x2, [x2, 0x40]                       ; segment.PHDR
|                                                                      ; [0x40:4]=6 ; "\U00000006"
|           0x06d950a4      mov   x3, x0
|           0x06d950a8      ldr   x0, [arg_150h]                       ; [0x150:4]=32
|           0x06d950ac      mov   x4, x1
|           0x06d950b0      mov   x1, x3
|           0x06d950b4      mov   x5, x2
|           0x06d950b8      mov   x2, x4
|           0x06d950bc      blr   x5
|           0x06d950c0      ldp   fp, lr, [sp, -8]
|           0x06d950c4      add   sp, sp, 0x130
\           0x06d950c8      ret
==== AuthProvider_.GetProxyURL
/ fcn.06d94d58(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg4, int64_t arg7, int64_t arg_38h, int64_t arg_40h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_60h, int64_t arg_68h, int64_t arg_70h, int64_t arg_78h, int64_t arg_88h, int64_t arg_90h, int64_t arg_98h, int64_t arg_a0h, int64_t arg_a8h, int64_t arg_b0h, int64_t arg_b8h, int64_t arg_c0h, int64_t arg_c8h, int64_t arg_138h, int64_t arg_140h, int64_t arg_148h, int64_t arg_150h, int64_t arg_1b0h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg2 @ x1
|           ; arg int64_t arg3 @ x2
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg7 @ x6
|           ; arg int64_t arg_38h @ stack + 0x38
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_48h @ stack + 0x48
|           ; arg int64_t arg_50h @ stack + 0x50
|           ; arg int64_t arg_58h @ stack + 0x58
|           ; arg int64_t arg_60h @ stack + 0x60
|           ; arg int64_t arg_68h @ stack + 0x68
|           ; arg int64_t arg_70h @ stack + 0x70
|           ; arg int64_t arg_78h @ stack + 0x78
|           ; arg int64_t arg_88h @ stack + 0x88
|           ; arg int64_t arg_90h @ stack + 0x90
|           ; arg int64_t arg_98h @ stack + 0x98
|           ; arg int64_t arg_a0h @ stack + 0xa0
|           ; arg int64_t arg_a8h @ stack + 0xa8
|           ; arg int64_t arg_b0h @ stack + 0xb0
|           ; arg int64_t arg_b8h @ stack + 0xb8
|           ; arg int64_t arg_c0h @ stack + 0xc0
|           ; arg int64_t arg_c8h @ stack + 0xc8
|           ; arg int64_t arg_138h @ stack + 0x138
|           ; arg int64_t arg_140h @ stack + 0x140
|           ; arg int64_t arg_148h @ stack + 0x148
|           ; arg int64_t arg_150h @ stack + 0x150
|           ; arg int64_t arg_1b0h @ stack + 0x1b0
|           0x06d94d58      mov   sp, x20
|           0x06d94d5c      sub   fp, sp, 8
|           0x06d94d60      stp   x0, x1, [arg_138h]                   ; arg2
|           0x06d94d64      stp   x2, x3, [arg_148h]                   ; arg4
|           0x06d94d68      ldr   x4, [x26, 0x18]                      ; [0x18:4]=0x4d0f000 section..text
|           0x06d94d6c      str   x4, [arg_b0h]
|           0x06d94d70      ldp   x5, x6, [x26, 8]
|           0x06d94d74      str   x6, [arg_78h + 0x8]                  ; arg7
|           0x06d94d78      ldr   x0, [x4]
|           0x06d94d7c      ldr   x4, [x5, 0x60]                       ; [0x60:4]=0x380
|           0x06d94d80      mov   x1, x2                               ; arg3
|           0x06d94d84      mov   x2, x3                               ; arg4
|           0x06d94d88      mov   x3, x4
|           0x06d94d8c      bl    0x6d954f0
|       ,=< 0x06d94d90      cbnz  x1, 0x6d94e28
|       |   0x06d94d94      ldr   x26, [arg_1b0h]                      ; [0x80:4]=0
|       |   0x06d94d98      ldr   x4, [x26]
|       |   0x06d94d9c      ldr   x5, [arg_b0h]                        ; [0xb0:4]=0x6474e551
|       |   0x06d94da0      ldr   x2, [x5, 0x38]                       ; [0x38:4]=0x400010
|       |   0x06d94da4      ldr   x1, [arg_140h]                       ; [0x140:4]=144
|       |   0x06d94da8      mov   x3, x0
|       |   0x06d94dac      ldr   x0, [arg_138h]                       ; [0x138:4]=0x799b960 segment.GNU_RELRO
|       |   0x06d94db0      blr   x4
|      ,==< 0x06d94db4      cbnz  x2, 0x6d94e14
|      ||   0x06d94db8      str   x1, [arg_88h]
|      ||   0x06d94dbc      str   x0, [arg_48h]
|      ||   0x06d94dc0      ldr   x1, [arg_148h]                       ; [0x148:4]=0x458
|      ||   0x06d94dc4      ldr   x1, [x1, 0x30]                       ; [0x30:4]=0
|      ||   0x06d94dc8      ldr   x0, [arg_150h]                       ; [0x150:4]=32
|      ||   0x06d94dcc      blr   x1
|      ||   0x06d94dd0      str   x0, [arg_a8h]
|      ||   0x06d94dd4      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|      ||   0x06d94dd8      ldr   x1, [x1, 0x20]                       ; [0x20:4]=64 ; "@"
|      ||   0x06d94ddc      ldr   x0, [arg_88h]                        ; [0x88:4]=0
|      ||   0x06d94de0      blr   x1
|      ||   0x06d94de4      add   x2, arg_c8h
|      ||   0x06d94de8      stp   xzr, xzr, [x2]
|      ||   0x06d94dec      stp   xzr, xzr, [x2, 0x10]
|      ||   0x06d94df0      stp   xzr, xzr, [x2, 0x20]
|      ||   0x06d94df4      stp   xzr, xzr, [x2, 0x30]
|      ||   0x06d94df8      stp   xzr, xzr, [x2, 0x40]
|      ||   0x06d94dfc      stp   xzr, xzr, [x2, 0x50]
|      ||   0x06d94e00      mov   x1, x0
|      ||   0x06d94e04      adrp  x0, 0x8dbe000
|      ||   0x06d94e08      add   x0, x0, 0xfe0
|      ||   0x06d94e0c      bl    fcn.04d35800
|     ,===< 0x06d94e10      b     0x6d94e44
|     |`--> 0x06d94e14      mov   x0, x2
|     | |   0x06d94e18      mov   x1, x3
|     | |   0x06d94e1c      ldp   fp, lr, [sp, -8]
|     | |   0x06d94e20      add   sp, sp, 0x130
|     | |   0x06d94e24      ret
|     | `-> 0x06d94e28      mov   x0, x1
|     |     0x06d94e2c      mov   x1, x2
|     |     0x06d94e30      ldp   fp, lr, [sp, -8]
|     |     0x06d94e34      add   sp, sp, 0x130
|     |     0x06d94e38      ret
|     |     ; CODE XREF from fcn.06d94d58 @ 0x6d94f34
|    .-..-> 0x06d94e3c      add   x0, arg_c8h
|    :|::   0x06d94e40      bl    fcn.04d35870
|    :|::   ; CODE XREF from fcn.06d94d58 @ 0x6d94e10
|    :`---> 0x06d94e44      ldp   x4, x5, [arg_c8h]
|    :,===< 0x06d94e48      cbz   x4, 0x6d94f38
|    :|::   0x06d94e4c      ldp   x2, x3, [x4]
|    :|::   0x06d94e50      ldp   x4, x5, [x5]
|    `====< 0x06d94e54      cbz   x5, 0x6d94e3c
|     |::   0x06d94e58      str   x5, [arg_40h]
|     |::   0x06d94e5c      str   x4, [arg_78h]
|     |::   0x06d94e60      str   x3, [arg_58h]
|     |::   0x06d94e64      str   x2, [arg_98h]
|     |::   0x06d94e68      adrp  x27, 0x957b000
|     |::   0x06d94e6c      ldr   x1, [x27, 0xaa8]                     ; [0xaa8:4]=0x58585858
|     |::   0x06d94e70      adrp  x0, 0x8333000
|     |::   0x06d94e74      add   x0, x0, 0x770
|     |::   0x06d94e78      bl    0x4d18bb0
|     |`==< 0x06d94e7c      tbnz  w1, 0, 0x6d94e3c
|     | :   0x06d94e80      adrp  x0, 0x8dbe000
|     | :   0x06d94e84      add   x0, x0, 0xfe0
|     | :   0x06d94e88      ldr   x1, [arg_a8h]                        ; [0xa8:4]=0x200000
|     | :   0x06d94e8c      ldr   x2, [arg_98h]                        ; [0x98:4]=0x779b960
|     | :   0x06d94e90      ldr   x3, [arg_58h]                        ; [0x58:4]=64
|     | :   0x06d94e94      bl    0x4d18e50
|     | :   0x06d94e98      str   x0, [arg_c0h]
|     | :   0x06d94e9c      ldr   x2, [x0, 0x10]                       ; [0x10:4]=0xb70003
|     | :   0x06d94ea0      ldp   x4, x5, [x0]
|     | :   0x06d94ea4      ldr   x3, [arg_40h]                        ; segment.PHDR
|     | :                                                              ; [0x40:4]=6 ; "\U00000006"
|     | :   0x06d94ea8      add   x1, x5, x3
|     | :   0x06d94eac      cmp   x2, x1
|     |,==< 0x06d94eb0      b.hs  0x6d94ed4
|     ||:   0x06d94eb4      str   x5, [arg_68h]
|     ||:   0x06d94eb8      mov   x0, x4
|     ||:   0x06d94ebc      adrp  x4, 0x856f000
|     ||:   0x06d94ec0      add   x4, x4, 0x550
|     ||:   0x06d94ec4      bl    0x4d9e050
|     ||:   0x06d94ec8      ldr   x3, [arg_40h]                        ; segment.PHDR
|     ||:                                                              ; [0x40:4]=6 ; "\U00000006"
|     ||:   0x06d94ecc      ldr   x5, [arg_68h]                        ; [0x68:4]=0x380
|     ||:   0x06d94ed0      mov   x4, x0
|     |`--> 0x06d94ed4      stp   x1, x2, [arg_60h]
|     | :   0x06d94ed8      str   x4, [arg_b8h]
|     | :   0x06d94edc      sub   x6, x5, x2
|     | :   0x06d94ee0      sub   x2, x1, x5
|     | :   0x06d94ee4      and   x5, x5, x6, asr 63
|     | :   0x06d94ee8      add   x1, x4, x5, lsl 4
|     | :   0x06d94eec      adrp  x0, 0x856f000
|     | :   0x06d94ef0      add   x0, x0, 0x550
|     | :   0x06d94ef4      mov   x4, x3
|     | :   0x06d94ef8      ldr   x3, [arg_78h]                        ; [0x78:4]=1
|     | :   0x06d94efc      bl    0x4d99570
|     | :   0x06d94f00      ldp   x5, x6, [arg_60h]
|     | :   0x06d94f04      ldr   x7, [arg_c0h]                        ; [0xc0:4]=0
|     | :   0x06d94f08      stp   x5, x6, [x7, 8]
|     | :   0x06d94f0c      adrp  x27, 0xb48f000
|     | :   0x06d94f10      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     |,==< 0x06d94f14      cbnz  w5, 0x6d94f20
|     ||:   0x06d94f18      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    ,====< 0x06d94f1c      b     0x6d94f30
|    ||`--> 0x06d94f20      ldr   x1, [x7]
|    || :   0x06d94f24      bl    fcn.04da3500
|    || :   0x06d94f28      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    || :   0x06d94f2c      stp   x2, x1, [x25]
|    || :   ; CODE XREF from fcn.06d94d58 @ 0x6d94f1c
|    `----> 0x06d94f30      str   x2, [x7]
|     | `=< 0x06d94f34      b     0x6d94e3c
|     `---> 0x06d94f38      ldr   x1, [arg_148h]                       ; [0x148:4]=0x458
|           0x06d94f3c      ldr   x1, [x1, 0x38]                       ; [0x38:4]=0x400010
|           0x06d94f40      ldr   x0, [arg_150h]                       ; [0x150:4]=32
|           0x06d94f44      blr   x1
|           0x06d94f48      str   x0, [arg_a0h]
|           0x06d94f4c      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|           0x06d94f50      ldr   x1, [x1, 0x28]                       ; [0x28:4]=0xace39d8
|           0x06d94f54      ldr   x0, [arg_88h]                        ; [0x88:4]=0
|           0x06d94f58      blr   x1
|           0x06d94f5c      add   x2, arg_c8h
|           0x06d94f60      stp   xzr, xzr, [x2]
|           0x06d94f64      stp   xzr, xzr, [x2, 0x10]
|           0x06d94f68      stp   xzr, xzr, [x2, 0x20]
|           0x06d94f6c      stp   xzr, xzr, [x2, 0x30]
|           0x06d94f70      stp   xzr, xzr, [x2, 0x40]
|           0x06d94f74      stp   xzr, xzr, [x2, 0x50]
|           0x06d94f78      mov   x1, x0
|           0x06d94f7c      adrp  x0, 0x8dbe000
|           0x06d94f80      add   x0, x0, 0xfe0
|           0x06d94f84      bl    fcn.04d35800
|       ,=< 0x06d94f88      b     0x6d94f94
|       |   ; CODE XREF from fcn.06d94d58 @ 0x6d95088
|    ...--> 0x06d94f8c      add   x0, arg_c8h
|    :::|   0x06d94f90      bl    fcn.04d35870
|    :::|   ; CODE XREF from fcn.06d94d58 @ 0x6d94f88
|    :::`-> 0x06d94f94      ldp   x4, x5, [arg_c8h]
|    :::,=< 0x06d94f98      cbz   x4, 0x6d9508c
|    :::|   0x06d94f9c      ldp   x2, x3, [x4]
|    :::|   0x06d94fa0      ldp   x4, x5, [x5]
|    `====< 0x06d94fa4      cbz   x5, 0x6d94f8c
|     ::|   0x06d94fa8      str   x2, [arg_90h]
|     ::|   0x06d94fac      str   x3, [arg_50h]
|     ::|   0x06d94fb0      str   x4, [arg_70h]
|     ::|   0x06d94fb4      str   x5, [arg_38h]
|     ::|   0x06d94fb8      adrp  x27, 0x957b000
|     ::|   0x06d94fbc      ldr   x1, [x27, 0xaa8]                     ; [0xaa8:4]=0x58585858
|     ::|   0x06d94fc0      adrp  x0, 0x8333000
|     ::|   0x06d94fc4      add   x0, x0, 0x770
|     ::|   0x06d94fc8      bl    0x4d18bb0
|     `===< 0x06d94fcc      tbnz  w1, 0, 0x6d94f8c
|      :|   0x06d94fd0      adrp  x0, 0x8dbe000
|      :|   0x06d94fd4      add   x0, x0, 0xfe0
|      :|   0x06d94fd8      ldr   x1, [arg_a0h]                        ; [0xa0:4]=0x779b960
|      :|   0x06d94fdc      ldr   x2, [arg_90h]                        ; [0x90:4]=0
|      :|   0x06d94fe0      ldr   x3, [arg_50h]                        ; [0x50:4]=64
|      :|   0x06d94fe4      bl    0x4d18e50
|      :|   0x06d94fe8      str   x0, [arg_c0h]
|      :|   0x06d94fec      ldr   x2, [x0, 0x10]                       ; [0x10:4]=0xb70003
|      :|   0x06d94ff0      ldp   x4, x5, [x0]
|      :|   0x06d94ff4      ldr   x3, [arg_38h]                        ; [0x38:4]=0x400010
|      :|   0x06d94ff8      add   x1, x3, x5
|      :|   0x06d94ffc      cmp   x2, x1
|     ,===< 0x06d95000      b.hs  0x6d95024
|     |:|   0x06d95004      str   x5, [arg_68h]
|     |:|   0x06d95008      mov   x0, x4
|     |:|   0x06d9500c      adrp  x4, 0x856f000
|     |:|   0x06d95010      add   x4, x4, 0x550
|     |:|   0x06d95014      bl    0x4d9e050
|     |:|   0x06d95018      ldr   x3, [arg_38h]                        ; [0x38:4]=0x400010
|     |:|   0x06d9501c      ldr   x5, [arg_68h]                        ; [0x68:4]=0x380
|     |:|   0x06d95020      mov   x4, x0
|     `---> 0x06d95024      str   x1, [arg_68h]
|      :|   0x06d95028      str   x4, [arg_b8h]
|      :|   0x06d9502c      str   x2, [arg_60h]
|      :|   0x06d95030      sub   x6, x5, x2
|      :|   0x06d95034      sub   x2, x1, x5
|      :|   0x06d95038      and   x5, x5, x6, asr 63
|      :|   0x06d9503c      add   x1, x4, x5, lsl 4
|      :|   0x06d95040      adrp  x0, 0x856f000
|      :|   0x06d95044      add   x0, x0, 0x550
|      :|   0x06d95048      mov   x4, x3
|      :|   0x06d9504c      ldr   x3, [arg_70h]                        ; [0x70:4]=8
|      :|   0x06d95050      bl    0x4d99570
|      :|   0x06d95054      ldp   x6, x5, [arg_60h]
|      :|   0x06d95058      ldr   x7, [arg_c0h]                        ; [0xc0:4]=0
|      :|   0x06d9505c      stp   x5, x6, [x7, 8]
|      :|   0x06d95060      adrp  x27, 0xb48f000
|      :|   0x06d95064      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d95068      cbnz  w5, 0x6d95074
|     |:|   0x06d9506c      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    ,====< 0x06d95070      b     0x6d95084
|    |`---> 0x06d95074      ldr   x1, [x7]
|    | :|   0x06d95078      bl    fcn.04da3500
|    | :|   0x06d9507c      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    | :|   0x06d95080      stp   x2, x1, [x25]
|    | :|   ; CODE XREF from fcn.06d94d58 @ 0x6d95070
|    `----> 0x06d95084      str   x2, [x7]
|      `==< 0x06d95088      b     0x6d94f8c
|       `-> 0x06d9508c      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|           0x06d95090      ldr   x1, [x1, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|           0x06d95094      ldr   x0, [arg_88h]                        ; [0x88:4]=0
|           0x06d95098      blr   x1
|           0x06d9509c      ldr   x2, [arg_148h]                       ; [0x148:4]=0x458
|           0x06d950a0      ldr   x2, [x2, 0x40]                       ; segment.PHDR
|                                                                      ; [0x40:4]=6 ; "\U00000006"
|           0x06d950a4      mov   x3, x0
|           0x06d950a8      ldr   x0, [arg_150h]                       ; [0x150:4]=32
|           0x06d950ac      mov   x4, x1
|           0x06d950b0      mov   x1, x3
|           0x06d950b4      mov   x5, x2
|           0x06d950b8      mov   x2, x4
|           0x06d950bc      blr   x5
|           0x06d950c0      ldp   fp, lr, [sp, -8]
|           0x06d950c4      add   sp, sp, 0x130
\           0x06d950c8      ret
==== AuthProvider_.GetProxyURLString
/ fcn.06d94d58(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg4, int64_t arg7, int64_t arg_38h, int64_t arg_40h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_60h, int64_t arg_68h, int64_t arg_70h, int64_t arg_78h, int64_t arg_88h, int64_t arg_90h, int64_t arg_98h, int64_t arg_a0h, int64_t arg_a8h, int64_t arg_b0h, int64_t arg_b8h, int64_t arg_c0h, int64_t arg_c8h, int64_t arg_138h, int64_t arg_140h, int64_t arg_148h, int64_t arg_150h, int64_t arg_1b0h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg2 @ x1
|           ; arg int64_t arg3 @ x2
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg7 @ x6
|           ; arg int64_t arg_38h @ stack + 0x38
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_48h @ stack + 0x48
|           ; arg int64_t arg_50h @ stack + 0x50
|           ; arg int64_t arg_58h @ stack + 0x58
|           ; arg int64_t arg_60h @ stack + 0x60
|           ; arg int64_t arg_68h @ stack + 0x68
|           ; arg int64_t arg_70h @ stack + 0x70
|           ; arg int64_t arg_78h @ stack + 0x78
|           ; arg int64_t arg_88h @ stack + 0x88
|           ; arg int64_t arg_90h @ stack + 0x90
|           ; arg int64_t arg_98h @ stack + 0x98
|           ; arg int64_t arg_a0h @ stack + 0xa0
|           ; arg int64_t arg_a8h @ stack + 0xa8
|           ; arg int64_t arg_b0h @ stack + 0xb0
|           ; arg int64_t arg_b8h @ stack + 0xb8
|           ; arg int64_t arg_c0h @ stack + 0xc0
|           ; arg int64_t arg_c8h @ stack + 0xc8
|           ; arg int64_t arg_138h @ stack + 0x138
|           ; arg int64_t arg_140h @ stack + 0x140
|           ; arg int64_t arg_148h @ stack + 0x148
|           ; arg int64_t arg_150h @ stack + 0x150
|           ; arg int64_t arg_1b0h @ stack + 0x1b0
|           0x06d94d58      mov   sp, x20
|           0x06d94d5c      sub   fp, sp, 8
|           0x06d94d60      stp   x0, x1, [arg_138h]                   ; arg2
|           0x06d94d64      stp   x2, x3, [arg_148h]                   ; arg4
|           0x06d94d68      ldr   x4, [x26, 0x18]                      ; [0x18:4]=0x4d0f000 section..text
|           0x06d94d6c      str   x4, [arg_b0h]
|           0x06d94d70      ldp   x5, x6, [x26, 8]
|           0x06d94d74      str   x6, [arg_78h + 0x8]                  ; arg7
|           0x06d94d78      ldr   x0, [x4]
|           0x06d94d7c      ldr   x4, [x5, 0x60]                       ; [0x60:4]=0x380
|           0x06d94d80      mov   x1, x2                               ; arg3
|           0x06d94d84      mov   x2, x3                               ; arg4
|           0x06d94d88      mov   x3, x4
|           0x06d94d8c      bl    0x6d954f0
|       ,=< 0x06d94d90      cbnz  x1, 0x6d94e28
|       |   0x06d94d94      ldr   x26, [arg_1b0h]                      ; [0x80:4]=0
|       |   0x06d94d98      ldr   x4, [x26]
|       |   0x06d94d9c      ldr   x5, [arg_b0h]                        ; [0xb0:4]=0x6474e551
|       |   0x06d94da0      ldr   x2, [x5, 0x38]                       ; [0x38:4]=0x400010
|       |   0x06d94da4      ldr   x1, [arg_140h]                       ; [0x140:4]=144
|       |   0x06d94da8      mov   x3, x0
|       |   0x06d94dac      ldr   x0, [arg_138h]                       ; [0x138:4]=0x799b960 segment.GNU_RELRO
|       |   0x06d94db0      blr   x4
|      ,==< 0x06d94db4      cbnz  x2, 0x6d94e14
|      ||   0x06d94db8      str   x1, [arg_88h]
|      ||   0x06d94dbc      str   x0, [arg_48h]
|      ||   0x06d94dc0      ldr   x1, [arg_148h]                       ; [0x148:4]=0x458
|      ||   0x06d94dc4      ldr   x1, [x1, 0x30]                       ; [0x30:4]=0
|      ||   0x06d94dc8      ldr   x0, [arg_150h]                       ; [0x150:4]=32
|      ||   0x06d94dcc      blr   x1
|      ||   0x06d94dd0      str   x0, [arg_a8h]
|      ||   0x06d94dd4      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|      ||   0x06d94dd8      ldr   x1, [x1, 0x20]                       ; [0x20:4]=64 ; "@"
|      ||   0x06d94ddc      ldr   x0, [arg_88h]                        ; [0x88:4]=0
|      ||   0x06d94de0      blr   x1
|      ||   0x06d94de4      add   x2, arg_c8h
|      ||   0x06d94de8      stp   xzr, xzr, [x2]
|      ||   0x06d94dec      stp   xzr, xzr, [x2, 0x10]
|      ||   0x06d94df0      stp   xzr, xzr, [x2, 0x20]
|      ||   0x06d94df4      stp   xzr, xzr, [x2, 0x30]
|      ||   0x06d94df8      stp   xzr, xzr, [x2, 0x40]
|      ||   0x06d94dfc      stp   xzr, xzr, [x2, 0x50]
|      ||   0x06d94e00      mov   x1, x0
|      ||   0x06d94e04      adrp  x0, 0x8dbe000
|      ||   0x06d94e08      add   x0, x0, 0xfe0
|      ||   0x06d94e0c      bl    fcn.04d35800
|     ,===< 0x06d94e10      b     0x6d94e44
|     |`--> 0x06d94e14      mov   x0, x2
|     | |   0x06d94e18      mov   x1, x3
|     | |   0x06d94e1c      ldp   fp, lr, [sp, -8]
|     | |   0x06d94e20      add   sp, sp, 0x130
|     | |   0x06d94e24      ret
|     | `-> 0x06d94e28      mov   x0, x1
|     |     0x06d94e2c      mov   x1, x2
|     |     0x06d94e30      ldp   fp, lr, [sp, -8]
|     |     0x06d94e34      add   sp, sp, 0x130
|     |     0x06d94e38      ret
|     |     ; CODE XREF from fcn.06d94d58 @ 0x6d94f34
|    .-..-> 0x06d94e3c      add   x0, arg_c8h
|    :|::   0x06d94e40      bl    fcn.04d35870
|    :|::   ; CODE XREF from fcn.06d94d58 @ 0x6d94e10
|    :`---> 0x06d94e44      ldp   x4, x5, [arg_c8h]
|    :,===< 0x06d94e48      cbz   x4, 0x6d94f38
|    :|::   0x06d94e4c      ldp   x2, x3, [x4]
|    :|::   0x06d94e50      ldp   x4, x5, [x5]
|    `====< 0x06d94e54      cbz   x5, 0x6d94e3c
|     |::   0x06d94e58      str   x5, [arg_40h]
|     |::   0x06d94e5c      str   x4, [arg_78h]
|     |::   0x06d94e60      str   x3, [arg_58h]
|     |::   0x06d94e64      str   x2, [arg_98h]
|     |::   0x06d94e68      adrp  x27, 0x957b000
|     |::   0x06d94e6c      ldr   x1, [x27, 0xaa8]                     ; [0xaa8:4]=0x58585858
|     |::   0x06d94e70      adrp  x0, 0x8333000
|     |::   0x06d94e74      add   x0, x0, 0x770
|     |::   0x06d94e78      bl    0x4d18bb0
|     |`==< 0x06d94e7c      tbnz  w1, 0, 0x6d94e3c
|     | :   0x06d94e80      adrp  x0, 0x8dbe000
|     | :   0x06d94e84      add   x0, x0, 0xfe0
|     | :   0x06d94e88      ldr   x1, [arg_a8h]                        ; [0xa8:4]=0x200000
|     | :   0x06d94e8c      ldr   x2, [arg_98h]                        ; [0x98:4]=0x779b960
|     | :   0x06d94e90      ldr   x3, [arg_58h]                        ; [0x58:4]=64
|     | :   0x06d94e94      bl    0x4d18e50
|     | :   0x06d94e98      str   x0, [arg_c0h]
|     | :   0x06d94e9c      ldr   x2, [x0, 0x10]                       ; [0x10:4]=0xb70003
|     | :   0x06d94ea0      ldp   x4, x5, [x0]
|     | :   0x06d94ea4      ldr   x3, [arg_40h]                        ; segment.PHDR
|     | :                                                              ; [0x40:4]=6 ; "\U00000006"
|     | :   0x06d94ea8      add   x1, x5, x3
|     | :   0x06d94eac      cmp   x2, x1
|     |,==< 0x06d94eb0      b.hs  0x6d94ed4
|     ||:   0x06d94eb4      str   x5, [arg_68h]
|     ||:   0x06d94eb8      mov   x0, x4
|     ||:   0x06d94ebc      adrp  x4, 0x856f000
|     ||:   0x06d94ec0      add   x4, x4, 0x550
|     ||:   0x06d94ec4      bl    0x4d9e050
|     ||:   0x06d94ec8      ldr   x3, [arg_40h]                        ; segment.PHDR
|     ||:                                                              ; [0x40:4]=6 ; "\U00000006"
|     ||:   0x06d94ecc      ldr   x5, [arg_68h]                        ; [0x68:4]=0x380
|     ||:   0x06d94ed0      mov   x4, x0
|     |`--> 0x06d94ed4      stp   x1, x2, [arg_60h]
|     | :   0x06d94ed8      str   x4, [arg_b8h]
|     | :   0x06d94edc      sub   x6, x5, x2
|     | :   0x06d94ee0      sub   x2, x1, x5
|     | :   0x06d94ee4      and   x5, x5, x6, asr 63
|     | :   0x06d94ee8      add   x1, x4, x5, lsl 4
|     | :   0x06d94eec      adrp  x0, 0x856f000
|     | :   0x06d94ef0      add   x0, x0, 0x550
|     | :   0x06d94ef4      mov   x4, x3
|     | :   0x06d94ef8      ldr   x3, [arg_78h]                        ; [0x78:4]=1
|     | :   0x06d94efc      bl    0x4d99570
|     | :   0x06d94f00      ldp   x5, x6, [arg_60h]
|     | :   0x06d94f04      ldr   x7, [arg_c0h]                        ; [0xc0:4]=0
|     | :   0x06d94f08      stp   x5, x6, [x7, 8]
|     | :   0x06d94f0c      adrp  x27, 0xb48f000
|     | :   0x06d94f10      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     |,==< 0x06d94f14      cbnz  w5, 0x6d94f20
|     ||:   0x06d94f18      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    ,====< 0x06d94f1c      b     0x6d94f30
|    ||`--> 0x06d94f20      ldr   x1, [x7]
|    || :   0x06d94f24      bl    fcn.04da3500
|    || :   0x06d94f28      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    || :   0x06d94f2c      stp   x2, x1, [x25]
|    || :   ; CODE XREF from fcn.06d94d58 @ 0x6d94f1c
|    `----> 0x06d94f30      str   x2, [x7]
|     | `=< 0x06d94f34      b     0x6d94e3c
|     `---> 0x06d94f38      ldr   x1, [arg_148h]                       ; [0x148:4]=0x458
|           0x06d94f3c      ldr   x1, [x1, 0x38]                       ; [0x38:4]=0x400010
|           0x06d94f40      ldr   x0, [arg_150h]                       ; [0x150:4]=32
|           0x06d94f44      blr   x1
|           0x06d94f48      str   x0, [arg_a0h]
|           0x06d94f4c      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|           0x06d94f50      ldr   x1, [x1, 0x28]                       ; [0x28:4]=0xace39d8
|           0x06d94f54      ldr   x0, [arg_88h]                        ; [0x88:4]=0
|           0x06d94f58      blr   x1
|           0x06d94f5c      add   x2, arg_c8h
|           0x06d94f60      stp   xzr, xzr, [x2]
|           0x06d94f64      stp   xzr, xzr, [x2, 0x10]
|           0x06d94f68      stp   xzr, xzr, [x2, 0x20]
|           0x06d94f6c      stp   xzr, xzr, [x2, 0x30]
|           0x06d94f70      stp   xzr, xzr, [x2, 0x40]
|           0x06d94f74      stp   xzr, xzr, [x2, 0x50]
|           0x06d94f78      mov   x1, x0
|           0x06d94f7c      adrp  x0, 0x8dbe000
|           0x06d94f80      add   x0, x0, 0xfe0
|           0x06d94f84      bl    fcn.04d35800
|       ,=< 0x06d94f88      b     0x6d94f94
|       |   ; CODE XREF from fcn.06d94d58 @ 0x6d95088
|    ...--> 0x06d94f8c      add   x0, arg_c8h
|    :::|   0x06d94f90      bl    fcn.04d35870
|    :::|   ; CODE XREF from fcn.06d94d58 @ 0x6d94f88
|    :::`-> 0x06d94f94      ldp   x4, x5, [arg_c8h]
|    :::,=< 0x06d94f98      cbz   x4, 0x6d9508c
|    :::|   0x06d94f9c      ldp   x2, x3, [x4]
|    :::|   0x06d94fa0      ldp   x4, x5, [x5]
|    `====< 0x06d94fa4      cbz   x5, 0x6d94f8c
|     ::|   0x06d94fa8      str   x2, [arg_90h]
|     ::|   0x06d94fac      str   x3, [arg_50h]
|     ::|   0x06d94fb0      str   x4, [arg_70h]
|     ::|   0x06d94fb4      str   x5, [arg_38h]
|     ::|   0x06d94fb8      adrp  x27, 0x957b000
|     ::|   0x06d94fbc      ldr   x1, [x27, 0xaa8]                     ; [0xaa8:4]=0x58585858
|     ::|   0x06d94fc0      adrp  x0, 0x8333000
|     ::|   0x06d94fc4      add   x0, x0, 0x770
|     ::|   0x06d94fc8      bl    0x4d18bb0
|     `===< 0x06d94fcc      tbnz  w1, 0, 0x6d94f8c
|      :|   0x06d94fd0      adrp  x0, 0x8dbe000
|      :|   0x06d94fd4      add   x0, x0, 0xfe0
|      :|   0x06d94fd8      ldr   x1, [arg_a0h]                        ; [0xa0:4]=0x779b960
|      :|   0x06d94fdc      ldr   x2, [arg_90h]                        ; [0x90:4]=0
|      :|   0x06d94fe0      ldr   x3, [arg_50h]                        ; [0x50:4]=64
|      :|   0x06d94fe4      bl    0x4d18e50
|      :|   0x06d94fe8      str   x0, [arg_c0h]
|      :|   0x06d94fec      ldr   x2, [x0, 0x10]                       ; [0x10:4]=0xb70003
|      :|   0x06d94ff0      ldp   x4, x5, [x0]
|      :|   0x06d94ff4      ldr   x3, [arg_38h]                        ; [0x38:4]=0x400010
|      :|   0x06d94ff8      add   x1, x3, x5
|      :|   0x06d94ffc      cmp   x2, x1
|     ,===< 0x06d95000      b.hs  0x6d95024
|     |:|   0x06d95004      str   x5, [arg_68h]
|     |:|   0x06d95008      mov   x0, x4
|     |:|   0x06d9500c      adrp  x4, 0x856f000
|     |:|   0x06d95010      add   x4, x4, 0x550
|     |:|   0x06d95014      bl    0x4d9e050
|     |:|   0x06d95018      ldr   x3, [arg_38h]                        ; [0x38:4]=0x400010
|     |:|   0x06d9501c      ldr   x5, [arg_68h]                        ; [0x68:4]=0x380
|     |:|   0x06d95020      mov   x4, x0
|     `---> 0x06d95024      str   x1, [arg_68h]
|      :|   0x06d95028      str   x4, [arg_b8h]
|      :|   0x06d9502c      str   x2, [arg_60h]
|      :|   0x06d95030      sub   x6, x5, x2
|      :|   0x06d95034      sub   x2, x1, x5
|      :|   0x06d95038      and   x5, x5, x6, asr 63
|      :|   0x06d9503c      add   x1, x4, x5, lsl 4
|      :|   0x06d95040      adrp  x0, 0x856f000
|      :|   0x06d95044      add   x0, x0, 0x550
|      :|   0x06d95048      mov   x4, x3
|      :|   0x06d9504c      ldr   x3, [arg_70h]                        ; [0x70:4]=8
|      :|   0x06d95050      bl    0x4d99570
|      :|   0x06d95054      ldp   x6, x5, [arg_60h]
|      :|   0x06d95058      ldr   x7, [arg_c0h]                        ; [0xc0:4]=0
|      :|   0x06d9505c      stp   x5, x6, [x7, 8]
|      :|   0x06d95060      adrp  x27, 0xb48f000
|      :|   0x06d95064      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d95068      cbnz  w5, 0x6d95074
|     |:|   0x06d9506c      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    ,====< 0x06d95070      b     0x6d95084
|    |`---> 0x06d95074      ldr   x1, [x7]
|    | :|   0x06d95078      bl    fcn.04da3500
|    | :|   0x06d9507c      ldr   x2, [arg_b8h]                        ; [0xb8:4]=0
|    | :|   0x06d95080      stp   x2, x1, [x25]
|    | :|   ; CODE XREF from fcn.06d94d58 @ 0x6d95070
|    `----> 0x06d95084      str   x2, [x7]
|      `==< 0x06d95088      b     0x6d94f8c
|       `-> 0x06d9508c      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|           0x06d95090      ldr   x1, [x1, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|           0x06d95094      ldr   x0, [arg_88h]                        ; [0x88:4]=0
|           0x06d95098      blr   x1
|           0x06d9509c      ldr   x2, [arg_148h]                       ; [0x148:4]=0x458
|           0x06d950a0      ldr   x2, [x2, 0x40]                       ; segment.PHDR
|                                                                      ; [0x40:4]=6 ; "\U00000006"
|           0x06d950a4      mov   x3, x0
|           0x06d950a8      ldr   x0, [arg_150h]                       ; [0x150:4]=32
|           0x06d950ac      mov   x4, x1
|           0x06d950b0      mov   x1, x3
|           0x06d950b4      mov   x5, x2
|           0x06d950b8      mov   x2, x4
|           0x06d950bc      blr   x5
|           0x06d950c0      ldp   fp, lr, [sp, -8]
|           0x06d950c4      add   sp, sp, 0x130
\           0x06d950c8      ret
==== AuthProvider_.SetHTTPClient
/ fcn.06d95118(int64_t arg1, int64_t arg2, int64_t arg4, int64_t arg_8h, int64_t arg_18h, int64_t arg_38h, int64_t arg_40h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_70h, int64_t arg_78h, int64_t arg_80h, int64_t arg_88h, int64_t arg_90h, int64_t arg_c0h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; arg int64_t arg2 @ x1
|       :   ; arg int64_t arg4 @ x3
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_18h @ stack + 0x18
|       :   ; arg int64_t arg_38h @ stack + 0x38
|       :   ; arg int64_t arg_40h @ stack + 0x40
|       :   ; arg int64_t arg_48h @ stack + 0x48
|       :   ; arg int64_t arg_50h @ stack + 0x50
|       :   ; arg int64_t arg_58h @ stack + 0x58
|       :   ; arg int64_t arg_70h @ stack + 0x70
|       :   ; arg int64_t arg_78h @ stack + 0x78
|       :   ; arg int64_t arg_80h @ stack + 0x80
|       :   ; arg int64_t arg_88h @ stack + 0x88
|       :   ; arg int64_t arg_90h @ stack + 0x90
|       :   ; arg int64_t arg_c0h @ stack + 0xc0
|       :   0x06d95118      str   x3, [arg_50h]                        ; arg4
|       :   0x06d9511c      ldp   x2, x3, [x26, 8]
|       :   0x06d95120      stp   x2, x3, [arg_40h]                    ; arg4
|       :   0x06d95124      ldr   x2, [x0, 0x28]                       ; [0x28:4]=0xace39d8 ; arg1
|       :   0x06d95128      mov   x0, x1                               ; arg2
|       :   0x06d9512c      blr   x2
|      ,==< 0x06d95130      cbnz  x0, 0x6d951f8
|      |:   0x06d95134      ldr   x6, [arg_c0h]                        ; [0x50:4]=64
|      |:   0x06d95138      ldr   x7, [x6, 0x28]                       ; [0x28:4]=0xace39d8
|      |:   0x06d9513c      ldr   x8, [arg_88h]                        ; [0x88:4]=0
|      |:   0x06d95140      cmp   x8, x7
|     ,===< 0x06d95144      b.ne  0x6d95150
|     ||:   0x06d95148      ldr   x0, [arg_90h]                        ; [0x90:4]=0
|    ,====< 0x06d9514c      b     0x6d95154
|    |`---> 0x06d95150      mov   x0, xzr
|    | |:   ; CODE XREF from fcn.06d95118 @ 0x6d9514c
|    `,===< 0x06d95154      b.eq  0x6d951b4
|     ||:   0x06d95158      stp   xzr, xzr, [arg_58h]
|    ,====< 0x06d9515c      cbz   x8, 0x6d95168
|    |||:   0x06d95160      ldr   x2, [x8, 8]                          ; [0x8:4]=0
|   ,=====< 0x06d95164      b     0x6d9516c
|   |`----> 0x06d95168      mov   x2, x8
|   | ||:   ; CODE XREF from fcn.06d95118 @ 0x6d95164
|   `-----> 0x06d9516c      ldr   x6, [arg_90h]                        ; [0x90:4]=0
|     ||:   0x06d95170      stp   x2, x6, [arg_58h]
|     ||:   0x06d95174      mov   x0, 0xd
|     ||:   0x06d95178      adrp  x1, 0x4405000
|     ||:   0x06d9517c      add   x1, x1, 0xbd7
|     ||:   0x06d95180      mov   x2, 0x22                             ; '"'
|     ||:   0x06d95184      add   x3, arg_58h
|     ||:   0x06d95188      orr   x4, xzr, 1
|     ||:   0x06d9518c      mov   x5, x4
|     ||:   0x06d95190      bl    0x52d4240
|     ||:   0x06d95194      mov   x1, xzr
|     ||:   0x06d95198      adrp  x2, 0x8f07000
|     ||:   0x06d9519c      add   x2, x2, 0x588
|     ||:   0x06d951a0      mov   x3, x0
|     ||:   0x06d951a4      mov   x0, xzr
|     ||:   0x06d951a8      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|     ||:   0x06d951ac      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|     ||:   0x06d951b0      ret
|     `---> 0x06d951b4      ldr   x26, [arg_40h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d951b8      ldr   x3, [x26]
|      |:   0x06d951bc      ldr   x1, [arg_80h]                        ; [0x80:4]=0
|      |:   0x06d951c0      mov   x2, x0
|      |:   0x06d951c4      ldr   x0, [arg_78h]                        ; [0x78:4]=1
|      |:   0x06d951c8      blr   x3
|     ,===< 0x06d951cc      cbnz  x0, 0x6d951d4
|    ,====< 0x06d951d0      cbz   x1, 0x6d95214
|    |`---> 0x06d951d4      ldr   x4, [arg_50h]                        ; [0x50:4]=64
|    | |:   0x06d951d8      ldr   x4, [x4, 0x30]                       ; [0x30:4]=0
|    | |:   0x06d951dc      mov   x3, x2
|    | |:   0x06d951e0      mov   x2, x1
|    | |:   0x06d951e4      mov   x1, x0
|    | |:   0x06d951e8      mov   x0, x4
|    | |:   0x06d951ec      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    | |:   0x06d951f0      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|    | |:   0x06d951f4      ret
|    | `--> 0x06d951f8      mov   x2, x0
|    |  :   0x06d951fc      mov   x3, x1                               ; arg2
|    |  :   0x06d95200      mov   x0, xzr
|    |  :   0x06d95204      mov   x1, xzr
|    |  :   0x06d95208      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    |  :   0x06d9520c      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|    |  :   0x06d95210      ret
|    `----> 0x06d95214      mov   x0, xzr
|       :   0x06d95218      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|       :   0x06d9521c      ldr   x2, [arg_38h]                        ; [0x38:4]=0x400010
|       :   0x06d95220      adrp  x3, 0x4443000
|       :   0x06d95224      add   x3, x3, 0xc20
|       :   0x06d95228      mov   x4, 0x2d                             ; '-'
|       :   0x06d9522c      bl    0x4d7d0e0
|       :   0x06d95230      bl    0x4d98880
|       :   0x06d95234      mov   x1, x0
|       :   0x06d95238      adrp  x0, 0x856f000
|       :   0x06d9523c      add   x0, x0, 0x550
|       :   0x06d95240      bl    0x4d9b050
|       :   0x06d95244      nop
|       :   0x06d95248      stp   x0, x1, [arg_8h]
|       :   0x06d9524c      stp   x2, x3, [arg_18h]
|       :   0x06d95250      mov   x3, lr
|       :   0x06d95254      bl    fcn.04da1520
|       :   0x06d95258      ldp   x0, x1, [arg_8h]
|       :   0x06d9525c      ldp   x2, x3, [arg_18h]
|       `=< 0x06d95260      b     0x6d950f0
==== AuthProvider_.GetUserAgentName
/ fcn.06d95118(int64_t arg1, int64_t arg2, int64_t arg4, int64_t arg_8h, int64_t arg_18h, int64_t arg_38h, int64_t arg_40h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_70h, int64_t arg_78h, int64_t arg_80h, int64_t arg_88h, int64_t arg_90h, int64_t arg_c0h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; arg int64_t arg2 @ x1
|       :   ; arg int64_t arg4 @ x3
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_18h @ stack + 0x18
|       :   ; arg int64_t arg_38h @ stack + 0x38
|       :   ; arg int64_t arg_40h @ stack + 0x40
|       :   ; arg int64_t arg_48h @ stack + 0x48
|       :   ; arg int64_t arg_50h @ stack + 0x50
|       :   ; arg int64_t arg_58h @ stack + 0x58
|       :   ; arg int64_t arg_70h @ stack + 0x70
|       :   ; arg int64_t arg_78h @ stack + 0x78
|       :   ; arg int64_t arg_80h @ stack + 0x80
|       :   ; arg int64_t arg_88h @ stack + 0x88
|       :   ; arg int64_t arg_90h @ stack + 0x90
|       :   ; arg int64_t arg_c0h @ stack + 0xc0
|       :   0x06d95118      str   x3, [arg_50h]                        ; arg4
|       :   0x06d9511c      ldp   x2, x3, [x26, 8]
|       :   0x06d95120      stp   x2, x3, [arg_40h]                    ; arg4
|       :   0x06d95124      ldr   x2, [x0, 0x28]                       ; [0x28:4]=0xace39d8 ; arg1
|       :   0x06d95128      mov   x0, x1                               ; arg2
|       :   0x06d9512c      blr   x2
|      ,==< 0x06d95130      cbnz  x0, 0x6d951f8
|      |:   0x06d95134      ldr   x6, [arg_c0h]                        ; [0x50:4]=64
|      |:   0x06d95138      ldr   x7, [x6, 0x28]                       ; [0x28:4]=0xace39d8
|      |:   0x06d9513c      ldr   x8, [arg_88h]                        ; [0x88:4]=0
|      |:   0x06d95140      cmp   x8, x7
|     ,===< 0x06d95144      b.ne  0x6d95150
|     ||:   0x06d95148      ldr   x0, [arg_90h]                        ; [0x90:4]=0
|    ,====< 0x06d9514c      b     0x6d95154
|    |`---> 0x06d95150      mov   x0, xzr
|    | |:   ; CODE XREF from fcn.06d95118 @ 0x6d9514c
|    `,===< 0x06d95154      b.eq  0x6d951b4
|     ||:   0x06d95158      stp   xzr, xzr, [arg_58h]
|    ,====< 0x06d9515c      cbz   x8, 0x6d95168
|    |||:   0x06d95160      ldr   x2, [x8, 8]                          ; [0x8:4]=0
|   ,=====< 0x06d95164      b     0x6d9516c
|   |`----> 0x06d95168      mov   x2, x8
|   | ||:   ; CODE XREF from fcn.06d95118 @ 0x6d95164
|   `-----> 0x06d9516c      ldr   x6, [arg_90h]                        ; [0x90:4]=0
|     ||:   0x06d95170      stp   x2, x6, [arg_58h]
|     ||:   0x06d95174      mov   x0, 0xd
|     ||:   0x06d95178      adrp  x1, 0x4405000
|     ||:   0x06d9517c      add   x1, x1, 0xbd7
|     ||:   0x06d95180      mov   x2, 0x22                             ; '"'
|     ||:   0x06d95184      add   x3, arg_58h
|     ||:   0x06d95188      orr   x4, xzr, 1
|     ||:   0x06d9518c      mov   x5, x4
|     ||:   0x06d95190      bl    0x52d4240
|     ||:   0x06d95194      mov   x1, xzr
|     ||:   0x06d95198      adrp  x2, 0x8f07000
|     ||:   0x06d9519c      add   x2, x2, 0x588
|     ||:   0x06d951a0      mov   x3, x0
|     ||:   0x06d951a4      mov   x0, xzr
|     ||:   0x06d951a8      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|     ||:   0x06d951ac      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|     ||:   0x06d951b0      ret
|     `---> 0x06d951b4      ldr   x26, [arg_40h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d951b8      ldr   x3, [x26]
|      |:   0x06d951bc      ldr   x1, [arg_80h]                        ; [0x80:4]=0
|      |:   0x06d951c0      mov   x2, x0
|      |:   0x06d951c4      ldr   x0, [arg_78h]                        ; [0x78:4]=1
|      |:   0x06d951c8      blr   x3
|     ,===< 0x06d951cc      cbnz  x0, 0x6d951d4
|    ,====< 0x06d951d0      cbz   x1, 0x6d95214
|    |`---> 0x06d951d4      ldr   x4, [arg_50h]                        ; [0x50:4]=64
|    | |:   0x06d951d8      ldr   x4, [x4, 0x30]                       ; [0x30:4]=0
|    | |:   0x06d951dc      mov   x3, x2
|    | |:   0x06d951e0      mov   x2, x1
|    | |:   0x06d951e4      mov   x1, x0
|    | |:   0x06d951e8      mov   x0, x4
|    | |:   0x06d951ec      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    | |:   0x06d951f0      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|    | |:   0x06d951f4      ret
|    | `--> 0x06d951f8      mov   x2, x0
|    |  :   0x06d951fc      mov   x3, x1                               ; arg2
|    |  :   0x06d95200      mov   x0, xzr
|    |  :   0x06d95204      mov   x1, xzr
|    |  :   0x06d95208      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    |  :   0x06d9520c      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|    |  :   0x06d95210      ret
|    `----> 0x06d95214      mov   x0, xzr
|       :   0x06d95218      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|       :   0x06d9521c      ldr   x2, [arg_38h]                        ; [0x38:4]=0x400010
|       :   0x06d95220      adrp  x3, 0x4443000
|       :   0x06d95224      add   x3, x3, 0xc20
|       :   0x06d95228      mov   x4, 0x2d                             ; '-'
|       :   0x06d9522c      bl    0x4d7d0e0
|       :   0x06d95230      bl    0x4d98880
|       :   0x06d95234      mov   x1, x0
|       :   0x06d95238      adrp  x0, 0x856f000
|       :   0x06d9523c      add   x0, x0, 0x550
|       :   0x06d95240      bl    0x4d9b050
|       :   0x06d95244      nop
|       :   0x06d95248      stp   x0, x1, [arg_8h]
|       :   0x06d9524c      stp   x2, x3, [arg_18h]
|       :   0x06d95250      mov   x3, lr
|       :   0x06d95254      bl    fcn.04da1520
|       :   0x06d95258      ldp   x0, x1, [arg_8h]
|       :   0x06d9525c      ldp   x2, x3, [arg_18h]
|       `=< 0x06d95260      b     0x6d950f0
==== AuthProvider_.GetAuthMethod
/ fcn.06d95118(int64_t arg1, int64_t arg2, int64_t arg4, int64_t arg_8h, int64_t arg_18h, int64_t arg_38h, int64_t arg_40h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_70h, int64_t arg_78h, int64_t arg_80h, int64_t arg_88h, int64_t arg_90h, int64_t arg_c0h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; arg int64_t arg2 @ x1
|       :   ; arg int64_t arg4 @ x3
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_18h @ stack + 0x18
|       :   ; arg int64_t arg_38h @ stack + 0x38
|       :   ; arg int64_t arg_40h @ stack + 0x40
|       :   ; arg int64_t arg_48h @ stack + 0x48
|       :   ; arg int64_t arg_50h @ stack + 0x50
|       :   ; arg int64_t arg_58h @ stack + 0x58
|       :   ; arg int64_t arg_70h @ stack + 0x70
|       :   ; arg int64_t arg_78h @ stack + 0x78
|       :   ; arg int64_t arg_80h @ stack + 0x80
|       :   ; arg int64_t arg_88h @ stack + 0x88
|       :   ; arg int64_t arg_90h @ stack + 0x90
|       :   ; arg int64_t arg_c0h @ stack + 0xc0
|       :   0x06d95118      str   x3, [arg_50h]                        ; arg4
|       :   0x06d9511c      ldp   x2, x3, [x26, 8]
|       :   0x06d95120      stp   x2, x3, [arg_40h]                    ; arg4
|       :   0x06d95124      ldr   x2, [x0, 0x28]                       ; [0x28:4]=0xace39d8 ; arg1
|       :   0x06d95128      mov   x0, x1                               ; arg2
|       :   0x06d9512c      blr   x2
|      ,==< 0x06d95130      cbnz  x0, 0x6d951f8
|      |:   0x06d95134      ldr   x6, [arg_c0h]                        ; [0x50:4]=64
|      |:   0x06d95138      ldr   x7, [x6, 0x28]                       ; [0x28:4]=0xace39d8
|      |:   0x06d9513c      ldr   x8, [arg_88h]                        ; [0x88:4]=0
|      |:   0x06d95140      cmp   x8, x7
|     ,===< 0x06d95144      b.ne  0x6d95150
|     ||:   0x06d95148      ldr   x0, [arg_90h]                        ; [0x90:4]=0
|    ,====< 0x06d9514c      b     0x6d95154
|    |`---> 0x06d95150      mov   x0, xzr
|    | |:   ; CODE XREF from fcn.06d95118 @ 0x6d9514c
|    `,===< 0x06d95154      b.eq  0x6d951b4
|     ||:   0x06d95158      stp   xzr, xzr, [arg_58h]
|    ,====< 0x06d9515c      cbz   x8, 0x6d95168
|    |||:   0x06d95160      ldr   x2, [x8, 8]                          ; [0x8:4]=0
|   ,=====< 0x06d95164      b     0x6d9516c
|   |`----> 0x06d95168      mov   x2, x8
|   | ||:   ; CODE XREF from fcn.06d95118 @ 0x6d95164
|   `-----> 0x06d9516c      ldr   x6, [arg_90h]                        ; [0x90:4]=0
|     ||:   0x06d95170      stp   x2, x6, [arg_58h]
|     ||:   0x06d95174      mov   x0, 0xd
|     ||:   0x06d95178      adrp  x1, 0x4405000
|     ||:   0x06d9517c      add   x1, x1, 0xbd7
|     ||:   0x06d95180      mov   x2, 0x22                             ; '"'
|     ||:   0x06d95184      add   x3, arg_58h
|     ||:   0x06d95188      orr   x4, xzr, 1
|     ||:   0x06d9518c      mov   x5, x4
|     ||:   0x06d95190      bl    0x52d4240
|     ||:   0x06d95194      mov   x1, xzr
|     ||:   0x06d95198      adrp  x2, 0x8f07000
|     ||:   0x06d9519c      add   x2, x2, 0x588
|     ||:   0x06d951a0      mov   x3, x0
|     ||:   0x06d951a4      mov   x0, xzr
|     ||:   0x06d951a8      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|     ||:   0x06d951ac      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|     ||:   0x06d951b0      ret
|     `---> 0x06d951b4      ldr   x26, [arg_40h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d951b8      ldr   x3, [x26]
|      |:   0x06d951bc      ldr   x1, [arg_80h]                        ; [0x80:4]=0
|      |:   0x06d951c0      mov   x2, x0
|      |:   0x06d951c4      ldr   x0, [arg_78h]                        ; [0x78:4]=1
|      |:   0x06d951c8      blr   x3
|     ,===< 0x06d951cc      cbnz  x0, 0x6d951d4
|    ,====< 0x06d951d0      cbz   x1, 0x6d95214
|    |`---> 0x06d951d4      ldr   x4, [arg_50h]                        ; [0x50:4]=64
|    | |:   0x06d951d8      ldr   x4, [x4, 0x30]                       ; [0x30:4]=0
|    | |:   0x06d951dc      mov   x3, x2
|    | |:   0x06d951e0      mov   x2, x1
|    | |:   0x06d951e4      mov   x1, x0
|    | |:   0x06d951e8      mov   x0, x4
|    | |:   0x06d951ec      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    | |:   0x06d951f0      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|    | |:   0x06d951f4      ret
|    | `--> 0x06d951f8      mov   x2, x0
|    |  :   0x06d951fc      mov   x3, x1                               ; arg2
|    |  :   0x06d95200      mov   x0, xzr
|    |  :   0x06d95204      mov   x1, xzr
|    |  :   0x06d95208      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    |  :   0x06d9520c      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|    |  :   0x06d95210      ret
|    `----> 0x06d95214      mov   x0, xzr
|       :   0x06d95218      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|       :   0x06d9521c      ldr   x2, [arg_38h]                        ; [0x38:4]=0x400010
|       :   0x06d95220      adrp  x3, 0x4443000
|       :   0x06d95224      add   x3, x3, 0xc20
|       :   0x06d95228      mov   x4, 0x2d                             ; '-'
|       :   0x06d9522c      bl    0x4d7d0e0
|       :   0x06d95230      bl    0x4d98880
|       :   0x06d95234      mov   x1, x0
|       :   0x06d95238      adrp  x0, 0x856f000
|       :   0x06d9523c      add   x0, x0, 0x550
|       :   0x06d95240      bl    0x4d9b050
|       :   0x06d95244      nop
|       :   0x06d95248      stp   x0, x1, [arg_8h]
|       :   0x06d9524c      stp   x2, x3, [arg_18h]
|       :   0x06d95250      mov   x3, lr
|       :   0x06d95254      bl    fcn.04da1520
|       :   0x06d95258      ldp   x0, x1, [arg_8h]
|       :   0x06d9525c      ldp   x2, x3, [arg_18h]
|       `=< 0x06d95260      b     0x6d950f0
==== AuthProvider_.GetWIFProvider
/ fcn.06d95118(int64_t arg1, int64_t arg2, int64_t arg4, int64_t arg_8h, int64_t arg_18h, int64_t arg_38h, int64_t arg_40h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_70h, int64_t arg_78h, int64_t arg_80h, int64_t arg_88h, int64_t arg_90h, int64_t arg_c0h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; arg int64_t arg2 @ x1
|       :   ; arg int64_t arg4 @ x3
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_18h @ stack + 0x18
|       :   ; arg int64_t arg_38h @ stack + 0x38
|       :   ; arg int64_t arg_40h @ stack + 0x40
|       :   ; arg int64_t arg_48h @ stack + 0x48
|       :   ; arg int64_t arg_50h @ stack + 0x50
|       :   ; arg int64_t arg_58h @ stack + 0x58
|       :   ; arg int64_t arg_70h @ stack + 0x70
|       :   ; arg int64_t arg_78h @ stack + 0x78
|       :   ; arg int64_t arg_80h @ stack + 0x80
|       :   ; arg int64_t arg_88h @ stack + 0x88
|       :   ; arg int64_t arg_90h @ stack + 0x90
|       :   ; arg int64_t arg_c0h @ stack + 0xc0
|       :   0x06d95118      str   x3, [arg_50h]                        ; arg4
|       :   0x06d9511c      ldp   x2, x3, [x26, 8]
|       :   0x06d95120      stp   x2, x3, [arg_40h]                    ; arg4
|       :   0x06d95124      ldr   x2, [x0, 0x28]                       ; [0x28:4]=0xace39d8 ; arg1
|       :   0x06d95128      mov   x0, x1                               ; arg2
|       :   0x06d9512c      blr   x2
|      ,==< 0x06d95130      cbnz  x0, 0x6d951f8
|      |:   0x06d95134      ldr   x6, [arg_c0h]                        ; [0x50:4]=64
|      |:   0x06d95138      ldr   x7, [x6, 0x28]                       ; [0x28:4]=0xace39d8
|      |:   0x06d9513c      ldr   x8, [arg_88h]                        ; [0x88:4]=0
|      |:   0x06d95140      cmp   x8, x7
|     ,===< 0x06d95144      b.ne  0x6d95150
|     ||:   0x06d95148      ldr   x0, [arg_90h]                        ; [0x90:4]=0
|    ,====< 0x06d9514c      b     0x6d95154
|    |`---> 0x06d95150      mov   x0, xzr
|    | |:   ; CODE XREF from fcn.06d95118 @ 0x6d9514c
|    `,===< 0x06d95154      b.eq  0x6d951b4
|     ||:   0x06d95158      stp   xzr, xzr, [arg_58h]
|    ,====< 0x06d9515c      cbz   x8, 0x6d95168
|    |||:   0x06d95160      ldr   x2, [x8, 8]                          ; [0x8:4]=0
|   ,=====< 0x06d95164      b     0x6d9516c
|   |`----> 0x06d95168      mov   x2, x8
|   | ||:   ; CODE XREF from fcn.06d95118 @ 0x6d95164
|   `-----> 0x06d9516c      ldr   x6, [arg_90h]                        ; [0x90:4]=0
|     ||:   0x06d95170      stp   x2, x6, [arg_58h]
|     ||:   0x06d95174      mov   x0, 0xd
|     ||:   0x06d95178      adrp  x1, 0x4405000
|     ||:   0x06d9517c      add   x1, x1, 0xbd7
|     ||:   0x06d95180      mov   x2, 0x22                             ; '"'
|     ||:   0x06d95184      add   x3, arg_58h
|     ||:   0x06d95188      orr   x4, xzr, 1
|     ||:   0x06d9518c      mov   x5, x4
|     ||:   0x06d95190      bl    0x52d4240
|     ||:   0x06d95194      mov   x1, xzr
|     ||:   0x06d95198      adrp  x2, 0x8f07000
|     ||:   0x06d9519c      add   x2, x2, 0x588
|     ||:   0x06d951a0      mov   x3, x0
|     ||:   0x06d951a4      mov   x0, xzr
|     ||:   0x06d951a8      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|     ||:   0x06d951ac      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|     ||:   0x06d951b0      ret
|     `---> 0x06d951b4      ldr   x26, [arg_40h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d951b8      ldr   x3, [x26]
|      |:   0x06d951bc      ldr   x1, [arg_80h]                        ; [0x80:4]=0
|      |:   0x06d951c0      mov   x2, x0
|      |:   0x06d951c4      ldr   x0, [arg_78h]                        ; [0x78:4]=1
|      |:   0x06d951c8      blr   x3
|     ,===< 0x06d951cc      cbnz  x0, 0x6d951d4
|    ,====< 0x06d951d0      cbz   x1, 0x6d95214
|    |`---> 0x06d951d4      ldr   x4, [arg_50h]                        ; [0x50:4]=64
|    | |:   0x06d951d8      ldr   x4, [x4, 0x30]                       ; [0x30:4]=0
|    | |:   0x06d951dc      mov   x3, x2
|    | |:   0x06d951e0      mov   x2, x1
|    | |:   0x06d951e4      mov   x1, x0
|    | |:   0x06d951e8      mov   x0, x4
|    | |:   0x06d951ec      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    | |:   0x06d951f0      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|    | |:   0x06d951f4      ret
|    | `--> 0x06d951f8      mov   x2, x0
|    |  :   0x06d951fc      mov   x3, x1                               ; arg2
|    |  :   0x06d95200      mov   x0, xzr
|    |  :   0x06d95204      mov   x1, xzr
|    |  :   0x06d95208      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    |  :   0x06d9520c      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|    |  :   0x06d95210      ret
|    `----> 0x06d95214      mov   x0, xzr
|       :   0x06d95218      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|       :   0x06d9521c      ldr   x2, [arg_38h]                        ; [0x38:4]=0x400010
|       :   0x06d95220      adrp  x3, 0x4443000
|       :   0x06d95224      add   x3, x3, 0xc20
|       :   0x06d95228      mov   x4, 0x2d                             ; '-'
|       :   0x06d9522c      bl    0x4d7d0e0
|       :   0x06d95230      bl    0x4d98880
|       :   0x06d95234      mov   x1, x0
|       :   0x06d95238      adrp  x0, 0x856f000
|       :   0x06d9523c      add   x0, x0, 0x550
|       :   0x06d95240      bl    0x4d9b050
|       :   0x06d95244      nop
|       :   0x06d95248      stp   x0, x1, [arg_8h]
|       :   0x06d9524c      stp   x2, x3, [arg_18h]
|       :   0x06d95250      mov   x3, lr
|       :   0x06d95254      bl    fcn.04da1520
|       :   0x06d95258      ldp   x0, x1, [arg_8h]
|       :   0x06d9525c      ldp   x2, x3, [arg_18h]
|       `=< 0x06d95260      b     0x6d950f0
==== AuthProvider_.GetUserTier
/ fcn.06d95118(int64_t arg1, int64_t arg2, int64_t arg4, int64_t arg_8h, int64_t arg_18h, int64_t arg_38h, int64_t arg_40h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_70h, int64_t arg_78h, int64_t arg_80h, int64_t arg_88h, int64_t arg_90h, int64_t arg_c0h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; arg int64_t arg2 @ x1
|       :   ; arg int64_t arg4 @ x3
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_18h @ stack + 0x18
|       :   ; arg int64_t arg_38h @ stack + 0x38
|       :   ; arg int64_t arg_40h @ stack + 0x40
|       :   ; arg int64_t arg_48h @ stack + 0x48
|       :   ; arg int64_t arg_50h @ stack + 0x50
|       :   ; arg int64_t arg_58h @ stack + 0x58
|       :   ; arg int64_t arg_70h @ stack + 0x70
|       :   ; arg int64_t arg_78h @ stack + 0x78
|       :   ; arg int64_t arg_80h @ stack + 0x80
|       :   ; arg int64_t arg_88h @ stack + 0x88
|       :   ; arg int64_t arg_90h @ stack + 0x90
|       :   ; arg int64_t arg_c0h @ stack + 0xc0
|       :   0x06d95118      str   x3, [arg_50h]                        ; arg4
|       :   0x06d9511c      ldp   x2, x3, [x26, 8]
|       :   0x06d95120      stp   x2, x3, [arg_40h]                    ; arg4
|       :   0x06d95124      ldr   x2, [x0, 0x28]                       ; [0x28:4]=0xace39d8 ; arg1
|       :   0x06d95128      mov   x0, x1                               ; arg2
|       :   0x06d9512c      blr   x2
|      ,==< 0x06d95130      cbnz  x0, 0x6d951f8
|      |:   0x06d95134      ldr   x6, [arg_c0h]                        ; [0x50:4]=64
|      |:   0x06d95138      ldr   x7, [x6, 0x28]                       ; [0x28:4]=0xace39d8
|      |:   0x06d9513c      ldr   x8, [arg_88h]                        ; [0x88:4]=0
|      |:   0x06d95140      cmp   x8, x7
|     ,===< 0x06d95144      b.ne  0x6d95150
|     ||:   0x06d95148      ldr   x0, [arg_90h]                        ; [0x90:4]=0
|    ,====< 0x06d9514c      b     0x6d95154
|    |`---> 0x06d95150      mov   x0, xzr
|    | |:   ; CODE XREF from fcn.06d95118 @ 0x6d9514c
|    `,===< 0x06d95154      b.eq  0x6d951b4
|     ||:   0x06d95158      stp   xzr, xzr, [arg_58h]
|    ,====< 0x06d9515c      cbz   x8, 0x6d95168
|    |||:   0x06d95160      ldr   x2, [x8, 8]                          ; [0x8:4]=0
|   ,=====< 0x06d95164      b     0x6d9516c
|   |`----> 0x06d95168      mov   x2, x8
|   | ||:   ; CODE XREF from fcn.06d95118 @ 0x6d95164
|   `-----> 0x06d9516c      ldr   x6, [arg_90h]                        ; [0x90:4]=0
|     ||:   0x06d95170      stp   x2, x6, [arg_58h]
|     ||:   0x06d95174      mov   x0, 0xd
|     ||:   0x06d95178      adrp  x1, 0x4405000
|     ||:   0x06d9517c      add   x1, x1, 0xbd7
|     ||:   0x06d95180      mov   x2, 0x22                             ; '"'
|     ||:   0x06d95184      add   x3, arg_58h
|     ||:   0x06d95188      orr   x4, xzr, 1
|     ||:   0x06d9518c      mov   x5, x4
|     ||:   0x06d95190      bl    0x52d4240
|     ||:   0x06d95194      mov   x1, xzr
|     ||:   0x06d95198      adrp  x2, 0x8f07000
|     ||:   0x06d9519c      add   x2, x2, 0x588
|     ||:   0x06d951a0      mov   x3, x0
|     ||:   0x06d951a4      mov   x0, xzr
|     ||:   0x06d951a8      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|     ||:   0x06d951ac      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|     ||:   0x06d951b0      ret
|     `---> 0x06d951b4      ldr   x26, [arg_40h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d951b8      ldr   x3, [x26]
|      |:   0x06d951bc      ldr   x1, [arg_80h]                        ; [0x80:4]=0
|      |:   0x06d951c0      mov   x2, x0
|      |:   0x06d951c4      ldr   x0, [arg_78h]                        ; [0x78:4]=1
|      |:   0x06d951c8      blr   x3
|     ,===< 0x06d951cc      cbnz  x0, 0x6d951d4
|    ,====< 0x06d951d0      cbz   x1, 0x6d95214
|    |`---> 0x06d951d4      ldr   x4, [arg_50h]                        ; [0x50:4]=64
|    | |:   0x06d951d8      ldr   x4, [x4, 0x30]                       ; [0x30:4]=0
|    | |:   0x06d951dc      mov   x3, x2
|    | |:   0x06d951e0      mov   x2, x1
|    | |:   0x06d951e4      mov   x1, x0
|    | |:   0x06d951e8      mov   x0, x4
|    | |:   0x06d951ec      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    | |:   0x06d951f0      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|    | |:   0x06d951f4      ret
|    | `--> 0x06d951f8      mov   x2, x0
|    |  :   0x06d951fc      mov   x3, x1                               ; arg2
|    |  :   0x06d95200      mov   x0, xzr
|    |  :   0x06d95204      mov   x1, xzr
|    |  :   0x06d95208      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    |  :   0x06d9520c      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|    |  :   0x06d95210      ret
|    `----> 0x06d95214      mov   x0, xzr
|       :   0x06d95218      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|       :   0x06d9521c      ldr   x2, [arg_38h]                        ; [0x38:4]=0x400010
|       :   0x06d95220      adrp  x3, 0x4443000
|       :   0x06d95224      add   x3, x3, 0xc20
|       :   0x06d95228      mov   x4, 0x2d                             ; '-'
|       :   0x06d9522c      bl    0x4d7d0e0
|       :   0x06d95230      bl    0x4d98880
|       :   0x06d95234      mov   x1, x0
|       :   0x06d95238      adrp  x0, 0x856f000
|       :   0x06d9523c      add   x0, x0, 0x550
|       :   0x06d95240      bl    0x4d9b050
|       :   0x06d95244      nop
|       :   0x06d95248      stp   x0, x1, [arg_8h]
|       :   0x06d9524c      stp   x2, x3, [arg_18h]
|       :   0x06d95250      mov   x3, lr
|       :   0x06d95254      bl    fcn.04da1520
|       :   0x06d95258      ldp   x0, x1, [arg_8h]
|       :   0x06d9525c      ldp   x2, x3, [arg_18h]
|       `=< 0x06d95260      b     0x6d950f0
==== AuthProvider_.SetUserTier
/ fcn.06d95118(int64_t arg1, int64_t arg2, int64_t arg4, int64_t arg_8h, int64_t arg_18h, int64_t arg_38h, int64_t arg_40h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_70h, int64_t arg_78h, int64_t arg_80h, int64_t arg_88h, int64_t arg_90h, int64_t arg_c0h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; arg int64_t arg2 @ x1
|       :   ; arg int64_t arg4 @ x3
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_18h @ stack + 0x18
|       :   ; arg int64_t arg_38h @ stack + 0x38
|       :   ; arg int64_t arg_40h @ stack + 0x40
|       :   ; arg int64_t arg_48h @ stack + 0x48
|       :   ; arg int64_t arg_50h @ stack + 0x50
|       :   ; arg int64_t arg_58h @ stack + 0x58
|       :   ; arg int64_t arg_70h @ stack + 0x70
|       :   ; arg int64_t arg_78h @ stack + 0x78
|       :   ; arg int64_t arg_80h @ stack + 0x80
|       :   ; arg int64_t arg_88h @ stack + 0x88
|       :   ; arg int64_t arg_90h @ stack + 0x90
|       :   ; arg int64_t arg_c0h @ stack + 0xc0
|       :   0x06d95118      str   x3, [arg_50h]                        ; arg4
|       :   0x06d9511c      ldp   x2, x3, [x26, 8]
|       :   0x06d95120      stp   x2, x3, [arg_40h]                    ; arg4
|       :   0x06d95124      ldr   x2, [x0, 0x28]                       ; [0x28:4]=0xace39d8 ; arg1
|       :   0x06d95128      mov   x0, x1                               ; arg2
|       :   0x06d9512c      blr   x2
|      ,==< 0x06d95130      cbnz  x0, 0x6d951f8
|      |:   0x06d95134      ldr   x6, [arg_c0h]                        ; [0x50:4]=64
|      |:   0x06d95138      ldr   x7, [x6, 0x28]                       ; [0x28:4]=0xace39d8
|      |:   0x06d9513c      ldr   x8, [arg_88h]                        ; [0x88:4]=0
|      |:   0x06d95140      cmp   x8, x7
|     ,===< 0x06d95144      b.ne  0x6d95150
|     ||:   0x06d95148      ldr   x0, [arg_90h]                        ; [0x90:4]=0
|    ,====< 0x06d9514c      b     0x6d95154
|    |`---> 0x06d95150      mov   x0, xzr
|    | |:   ; CODE XREF from fcn.06d95118 @ 0x6d9514c
|    `,===< 0x06d95154      b.eq  0x6d951b4
|     ||:   0x06d95158      stp   xzr, xzr, [arg_58h]
|    ,====< 0x06d9515c      cbz   x8, 0x6d95168
|    |||:   0x06d95160      ldr   x2, [x8, 8]                          ; [0x8:4]=0
|   ,=====< 0x06d95164      b     0x6d9516c
|   |`----> 0x06d95168      mov   x2, x8
|   | ||:   ; CODE XREF from fcn.06d95118 @ 0x6d95164
|   `-----> 0x06d9516c      ldr   x6, [arg_90h]                        ; [0x90:4]=0
|     ||:   0x06d95170      stp   x2, x6, [arg_58h]
|     ||:   0x06d95174      mov   x0, 0xd
|     ||:   0x06d95178      adrp  x1, 0x4405000
|     ||:   0x06d9517c      add   x1, x1, 0xbd7
|     ||:   0x06d95180      mov   x2, 0x22                             ; '"'
|     ||:   0x06d95184      add   x3, arg_58h
|     ||:   0x06d95188      orr   x4, xzr, 1
|     ||:   0x06d9518c      mov   x5, x4
|     ||:   0x06d95190      bl    0x52d4240
|     ||:   0x06d95194      mov   x1, xzr
|     ||:   0x06d95198      adrp  x2, 0x8f07000
|     ||:   0x06d9519c      add   x2, x2, 0x588
|     ||:   0x06d951a0      mov   x3, x0
|     ||:   0x06d951a4      mov   x0, xzr
|     ||:   0x06d951a8      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|     ||:   0x06d951ac      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|     ||:   0x06d951b0      ret
|     `---> 0x06d951b4      ldr   x26, [arg_40h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d951b8      ldr   x3, [x26]
|      |:   0x06d951bc      ldr   x1, [arg_80h]                        ; [0x80:4]=0
|      |:   0x06d951c0      mov   x2, x0
|      |:   0x06d951c4      ldr   x0, [arg_78h]                        ; [0x78:4]=1
|      |:   0x06d951c8      blr   x3
|     ,===< 0x06d951cc      cbnz  x0, 0x6d951d4
|    ,====< 0x06d951d0      cbz   x1, 0x6d95214
|    |`---> 0x06d951d4      ldr   x4, [arg_50h]                        ; [0x50:4]=64
|    | |:   0x06d951d8      ldr   x4, [x4, 0x30]                       ; [0x30:4]=0
|    | |:   0x06d951dc      mov   x3, x2
|    | |:   0x06d951e0      mov   x2, x1
|    | |:   0x06d951e4      mov   x1, x0
|    | |:   0x06d951e8      mov   x0, x4
|    | |:   0x06d951ec      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    | |:   0x06d951f0      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|    | |:   0x06d951f4      ret
|    | `--> 0x06d951f8      mov   x2, x0
|    |  :   0x06d951fc      mov   x3, x1                               ; arg2
|    |  :   0x06d95200      mov   x0, xzr
|    |  :   0x06d95204      mov   x1, xzr
|    |  :   0x06d95208      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    |  :   0x06d9520c      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|    |  :   0x06d95210      ret
|    `----> 0x06d95214      mov   x0, xzr
|       :   0x06d95218      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|       :   0x06d9521c      ldr   x2, [arg_38h]                        ; [0x38:4]=0x400010
|       :   0x06d95220      adrp  x3, 0x4443000
|       :   0x06d95224      add   x3, x3, 0xc20
|       :   0x06d95228      mov   x4, 0x2d                             ; '-'
|       :   0x06d9522c      bl    0x4d7d0e0
|       :   0x06d95230      bl    0x4d98880
|       :   0x06d95234      mov   x1, x0
|       :   0x06d95238      adrp  x0, 0x856f000
|       :   0x06d9523c      add   x0, x0, 0x550
|       :   0x06d95240      bl    0x4d9b050
|       :   0x06d95244      nop
|       :   0x06d95248      stp   x0, x1, [arg_8h]
|       :   0x06d9524c      stp   x2, x3, [arg_18h]
|       :   0x06d95250      mov   x3, lr
|       :   0x06d95254      bl    fcn.04da1520
|       :   0x06d95258      ldp   x0, x1, [arg_8h]
|       :   0x06d9525c      ldp   x2, x3, [arg_18h]
|       `=< 0x06d95260      b     0x6d950f0
==== AuthProvider_.GetEndpointURL
/ fcn.06d95118(int64_t arg1, int64_t arg2, int64_t arg4, int64_t arg_8h, int64_t arg_18h, int64_t arg_38h, int64_t arg_40h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_70h, int64_t arg_78h, int64_t arg_80h, int64_t arg_88h, int64_t arg_90h, int64_t arg_c0h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; arg int64_t arg2 @ x1
|       :   ; arg int64_t arg4 @ x3
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_18h @ stack + 0x18
|       :   ; arg int64_t arg_38h @ stack + 0x38
|       :   ; arg int64_t arg_40h @ stack + 0x40
|       :   ; arg int64_t arg_48h @ stack + 0x48
|       :   ; arg int64_t arg_50h @ stack + 0x50
|       :   ; arg int64_t arg_58h @ stack + 0x58
|       :   ; arg int64_t arg_70h @ stack + 0x70
|       :   ; arg int64_t arg_78h @ stack + 0x78
|       :   ; arg int64_t arg_80h @ stack + 0x80
|       :   ; arg int64_t arg_88h @ stack + 0x88
|       :   ; arg int64_t arg_90h @ stack + 0x90
|       :   ; arg int64_t arg_c0h @ stack + 0xc0
|       :   0x06d95118      str   x3, [arg_50h]                        ; arg4
|       :   0x06d9511c      ldp   x2, x3, [x26, 8]
|       :   0x06d95120      stp   x2, x3, [arg_40h]                    ; arg4
|       :   0x06d95124      ldr   x2, [x0, 0x28]                       ; [0x28:4]=0xace39d8 ; arg1
|       :   0x06d95128      mov   x0, x1                               ; arg2
|       :   0x06d9512c      blr   x2
|      ,==< 0x06d95130      cbnz  x0, 0x6d951f8
|      |:   0x06d95134      ldr   x6, [arg_c0h]                        ; [0x50:4]=64
|      |:   0x06d95138      ldr   x7, [x6, 0x28]                       ; [0x28:4]=0xace39d8
|      |:   0x06d9513c      ldr   x8, [arg_88h]                        ; [0x88:4]=0
|      |:   0x06d95140      cmp   x8, x7
|     ,===< 0x06d95144      b.ne  0x6d95150
|     ||:   0x06d95148      ldr   x0, [arg_90h]                        ; [0x90:4]=0
|    ,====< 0x06d9514c      b     0x6d95154
|    |`---> 0x06d95150      mov   x0, xzr
|    | |:   ; CODE XREF from fcn.06d95118 @ 0x6d9514c
|    `,===< 0x06d95154      b.eq  0x6d951b4
|     ||:   0x06d95158      stp   xzr, xzr, [arg_58h]
|    ,====< 0x06d9515c      cbz   x8, 0x6d95168
|    |||:   0x06d95160      ldr   x2, [x8, 8]                          ; [0x8:4]=0
|   ,=====< 0x06d95164      b     0x6d9516c
|   |`----> 0x06d95168      mov   x2, x8
|   | ||:   ; CODE XREF from fcn.06d95118 @ 0x6d95164
|   `-----> 0x06d9516c      ldr   x6, [arg_90h]                        ; [0x90:4]=0
|     ||:   0x06d95170      stp   x2, x6, [arg_58h]
|     ||:   0x06d95174      mov   x0, 0xd
|     ||:   0x06d95178      adrp  x1, 0x4405000
|     ||:   0x06d9517c      add   x1, x1, 0xbd7
|     ||:   0x06d95180      mov   x2, 0x22                             ; '"'
|     ||:   0x06d95184      add   x3, arg_58h
|     ||:   0x06d95188      orr   x4, xzr, 1
|     ||:   0x06d9518c      mov   x5, x4
|     ||:   0x06d95190      bl    0x52d4240
|     ||:   0x06d95194      mov   x1, xzr
|     ||:   0x06d95198      adrp  x2, 0x8f07000
|     ||:   0x06d9519c      add   x2, x2, 0x588
|     ||:   0x06d951a0      mov   x3, x0
|     ||:   0x06d951a4      mov   x0, xzr
|     ||:   0x06d951a8      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|     ||:   0x06d951ac      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|     ||:   0x06d951b0      ret
|     `---> 0x06d951b4      ldr   x26, [arg_40h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d951b8      ldr   x3, [x26]
|      |:   0x06d951bc      ldr   x1, [arg_80h]                        ; [0x80:4]=0
|      |:   0x06d951c0      mov   x2, x0
|      |:   0x06d951c4      ldr   x0, [arg_78h]                        ; [0x78:4]=1
|      |:   0x06d951c8      blr   x3
|     ,===< 0x06d951cc      cbnz  x0, 0x6d951d4
|    ,====< 0x06d951d0      cbz   x1, 0x6d95214
|    |`---> 0x06d951d4      ldr   x4, [arg_50h]                        ; [0x50:4]=64
|    | |:   0x06d951d8      ldr   x4, [x4, 0x30]                       ; [0x30:4]=0
|    | |:   0x06d951dc      mov   x3, x2
|    | |:   0x06d951e0      mov   x2, x1
|    | |:   0x06d951e4      mov   x1, x0
|    | |:   0x06d951e8      mov   x0, x4
|    | |:   0x06d951ec      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    | |:   0x06d951f0      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|    | |:   0x06d951f4      ret
|    | `--> 0x06d951f8      mov   x2, x0
|    |  :   0x06d951fc      mov   x3, x1                               ; arg2
|    |  :   0x06d95200      mov   x0, xzr
|    |  :   0x06d95204      mov   x1, xzr
|    |  :   0x06d95208      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    |  :   0x06d9520c      ldr   lr, [sp], 0x70                       ; [0x70:4]=8
|    |  :   0x06d95210      ret
|    `----> 0x06d95214      mov   x0, xzr
|       :   0x06d95218      ldr   x1, [arg_48h]                        ; [0x48:4]=64
|       :   0x06d9521c      ldr   x2, [arg_38h]                        ; [0x38:4]=0x400010
|       :   0x06d95220      adrp  x3, 0x4443000
|       :   0x06d95224      add   x3, x3, 0xc20
|       :   0x06d95228      mov   x4, 0x2d                             ; '-'
|       :   0x06d9522c      bl    0x4d7d0e0
|       :   0x06d95230      bl    0x4d98880
|       :   0x06d95234      mov   x1, x0
|       :   0x06d95238      adrp  x0, 0x856f000
|       :   0x06d9523c      add   x0, x0, 0x550
|       :   0x06d95240      bl    0x4d9b050
|       :   0x06d95244      nop
|       :   0x06d95248      stp   x0, x1, [arg_8h]
|       :   0x06d9524c      stp   x2, x3, [arg_18h]
|       :   0x06d95250      mov   x3, lr
|       :   0x06d95254      bl    fcn.04da1520
|       :   0x06d95258      ldp   x0, x1, [arg_8h]
|       :   0x06d9525c      ldp   x2, x3, [arg_18h]
|       `=< 0x06d95260      b     0x6d950f0
==== AuthProvider_.SetEndpointURL
/ fcn.06d95368(int64_t arg_38h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_70h, int64_t arg_90h, int64_t arg_98h, int64_t arg_a0h, int64_t arg_a8h, int64_t arg_b0h, int64_t arg_b8h, int64_t arg_c0h, int64_t arg_c8h, int64_t arg_e0h, int64_t arg_e8h, int64_t arg_180h);
|           ; arg int64_t arg_38h @ stack + 0x38
|           ; arg int64_t arg_48h @ stack + 0x48
|           ; arg int64_t arg_50h @ stack + 0x50
|           ; arg int64_t arg_58h @ stack + 0x58
|           ; arg int64_t arg_70h @ stack + 0x70
|           ; arg int64_t arg_90h @ stack + 0x90
|           ; arg int64_t arg_98h @ stack + 0x98
|           ; arg int64_t arg_a0h @ stack + 0xa0
|           ; arg int64_t arg_a8h @ stack + 0xa8
|           ; arg int64_t arg_b0h @ stack + 0xb0
|           ; arg int64_t arg_b8h @ stack + 0xb8
|           ; arg int64_t arg_c0h @ stack + 0xc0
|           ; arg int64_t arg_c8h @ stack + 0xc8
|           ; arg int64_t arg_e0h @ stack + 0xe0
|           ; arg int64_t arg_e8h @ stack + 0xe8
|           ; arg int64_t arg_180h @ stack + 0x180
|       ,=< 0x06d95368      cbnz  w3, 0x6d9537c
|       |   0x06d9536c      ldr   x1, [arg_180h]                       ; [0xa0:4]=0x779b960
|       |   0x06d95370      ldr   x2, [arg_90h]                        ; [0x90:4]=0
|       |   0x06d95374      ldr   x3, [arg_e8h]                        ; [0xe8:4]=1
|      ,==< 0x06d95378      b     0x6d95394
|      |`-> 0x06d9537c      bl    0x4da3510
|      |    0x06d95380      ldr   x1, [arg_a0h]                        ; [0xa0:4]=0x779b960
|      |    0x06d95384      ldr   x2, [arg_90h]                        ; [0x90:4]=0
|      |    0x06d95388      stp   x1, x2, [x25]
|      |    0x06d9538c      ldr   x3, [arg_e8h]                        ; [0xe8:4]=1
|      |    0x06d95390      str   x3, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d95368 @ 0x6d95378
|      `--> 0x06d95394      str   x0, [arg_c8h]
|           0x06d95398      stp   x1, x2, [x0, 8]
|           0x06d9539c      str   x3, [x0, 0x18]
|           0x06d953a0      mov   x0, x1
|           0x06d953a4      bl    0x52d6250
|           0x06d953a8      str   x0, [arg_98h]
|           0x06d953ac      stp   x1, x2, [arg_38h]
|           0x06d953b0      add   x3, arg_58h
|           0x06d953b4      stp   xzr, xzr, [x3]
|           0x06d953b8      stp   xzr, xzr, [x3, 0x10]
|           0x06d953bc      stp   xzr, xzr, [x3, 0x20]
|           0x06d953c0      str   xzr, [x3, 0x30]
|           0x06d953c4      ldr   x3, [arg_a0h]                        ; [0xa0:4]=0x779b960
|           0x06d953c8      ldp   x4, x5, [x3, 0x40]
|           0x06d953cc      stp   x4, x5, [arg_70h]
|           0x06d953d0      ldp   x4, x5, [x3, 0x50]
|           0x06d953d4      stp   x4, x5, [arg_58h + 0x8]
|           0x06d953d8      ldrb  w4, [x3, 0x90]                       ; [0x90:4]=0
|           0x06d953dc      strb  w4, [arg_58h]
|           0x06d953e0      ldr   x3, [x3, 0x70]                       ; [0x70:4]=8
|           0x06d953e4      str   x3, [arg_70h + 0x18]
|           0x06d953e8      bl    0x52d7d00
|           0x06d953ec      str   x0, [arg_c0h]
|           0x06d953f0      ldr   x0, [arg_98h]                        ; [0x98:4]=0x779b960
|           0x06d953f4      ldp   x1, x2, [arg_38h]
|           0x06d953f8      bl    0x52d8130
|           0x06d953fc      str   x0, [arg_b8h]
|           0x06d95400      str   x1, [arg_50h]
|           0x06d95404      ldr   x0, [arg_98h]                        ; [0x98:4]=0x779b960
|           0x06d95408      ldp   x1, x2, [arg_38h]
|           0x06d9540c      bl    0x52d7e80
|           0x06d95410      str   x0, [arg_b0h]
|           0x06d95414      str   x1, [arg_48h]
|           0x06d95418      adrp  x0, 0x8b68000
|           0x06d9541c      add   x0, x0, 0x228
|           0x06d95420      bl    0x4d31c10
|           0x06d95424      adrp  x27, 0xb48f000
|           0x06d95428      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9542c      cbz   w1, 0x6d9547c
|       |   0x06d95430      str   x0, [arg_a8h]
|       |   0x06d95434      ldp   x3, x4, [x0, 0x38]
|       |   0x06d95438      ldr   x5, [x0, 0x48]                       ; [0x48:4]=64
|       |   0x06d9543c      ldr   x6, [x0, 0x58]                       ; [0x58:4]=64
|       |   0x06d95440      bl    0x4da3560
|       |   0x06d95444      ldr   x7, [arg_c8h]                        ; [0xc8:4]=0
|       |   0x06d95448      stp   x7, x3, [x25]
|       |   0x06d9544c      ldr   x3, [arg_c0h]                        ; [0xc0:4]=0
|       |   0x06d95450      stp   x3, x4, [x25, 0x10]
|       |   0x06d95454      ldr   x3, [arg_b8h]                        ; [0xb8:4]=0
|       |   0x06d95458      stp   x3, x5, [x25, 0x20]
|       |   0x06d9545c      ldr   x3, [arg_b0h]                        ; [0xb0:4]=0x6474e551
|       |   0x06d95460      stp   x3, x6, [x25, 0x30]
|       |   0x06d95464      mov   x1, x0
|       |   0x06d95468      add   x2, arg_58h
|       |   0x06d9546c      adrp  x0, 0x8b68000
|       |   0x06d95470      add   x0, x0, 0x150
|       |   0x06d95474      bl    0x4d35930
|       |   0x06d95478      ldr   x0, [arg_a8h]                        ; [0xa8:4]=0x200000
|       `-> 0x06d9547c      add   x27, arg_58h
|           0x06d95480      ldp   q0, q1, [x27]
|           0x06d95484      ldp   q2, q3, [arg_70h]
|           0x06d95488      stp   q0, q1, [x0]
|           0x06d9548c      add   x27, x0, 0x18
|           0x06d95490      stp   q2, q3, [x27]
|           0x06d95494      ldp   x2, x1, [arg_c0h]
|           0x06d95498      stp   x1, x2, [x0, 0x38]
|           0x06d9549c      ldr   x1, [arg_b8h]                        ; [0xb8:4]=0
|           0x06d954a0      ldr   x2, [arg_50h]                        ; [0x50:4]=64
|           0x06d954a4      stp   x1, x2, [x0, 0x48]
|           0x06d954a8      ldr   x1, [arg_b0h]                        ; [0xb0:4]=0x6474e551
|           0x06d954ac      ldr   x2, [arg_48h]                        ; [0x48:4]=64
|           0x06d954b0      stp   x1, x2, [x0, 0x58]
|           0x06d954b4      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d954b8      ldr   lr, [sp], 0xe0                       ; [0xe0:4]=0
\           0x06d954bc      ret
==== AuthProvider_.GetWireProtocol
/ fcn.06d95368(int64_t arg_38h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_70h, int64_t arg_90h, int64_t arg_98h, int64_t arg_a0h, int64_t arg_a8h, int64_t arg_b0h, int64_t arg_b8h, int64_t arg_c0h, int64_t arg_c8h, int64_t arg_e0h, int64_t arg_e8h, int64_t arg_180h);
|           ; arg int64_t arg_38h @ stack + 0x38
|           ; arg int64_t arg_48h @ stack + 0x48
|           ; arg int64_t arg_50h @ stack + 0x50
|           ; arg int64_t arg_58h @ stack + 0x58
|           ; arg int64_t arg_70h @ stack + 0x70
|           ; arg int64_t arg_90h @ stack + 0x90
|           ; arg int64_t arg_98h @ stack + 0x98
|           ; arg int64_t arg_a0h @ stack + 0xa0
|           ; arg int64_t arg_a8h @ stack + 0xa8
|           ; arg int64_t arg_b0h @ stack + 0xb0
|           ; arg int64_t arg_b8h @ stack + 0xb8
|           ; arg int64_t arg_c0h @ stack + 0xc0
|           ; arg int64_t arg_c8h @ stack + 0xc8
|           ; arg int64_t arg_e0h @ stack + 0xe0
|           ; arg int64_t arg_e8h @ stack + 0xe8
|           ; arg int64_t arg_180h @ stack + 0x180
|       ,=< 0x06d95368      cbnz  w3, 0x6d9537c
|       |   0x06d9536c      ldr   x1, [arg_180h]                       ; [0xa0:4]=0x779b960
|       |   0x06d95370      ldr   x2, [arg_90h]                        ; [0x90:4]=0
|       |   0x06d95374      ldr   x3, [arg_e8h]                        ; [0xe8:4]=1
|      ,==< 0x06d95378      b     0x6d95394
|      |`-> 0x06d9537c      bl    0x4da3510
|      |    0x06d95380      ldr   x1, [arg_a0h]                        ; [0xa0:4]=0x779b960
|      |    0x06d95384      ldr   x2, [arg_90h]                        ; [0x90:4]=0
|      |    0x06d95388      stp   x1, x2, [x25]
|      |    0x06d9538c      ldr   x3, [arg_e8h]                        ; [0xe8:4]=1
|      |    0x06d95390      str   x3, [x25, 0x10]
|      |    ; CODE XREF from fcn.06d95368 @ 0x6d95378
|      `--> 0x06d95394      str   x0, [arg_c8h]
|           0x06d95398      stp   x1, x2, [x0, 8]
|           0x06d9539c      str   x3, [x0, 0x18]
|           0x06d953a0      mov   x0, x1
|           0x06d953a4      bl    0x52d6250
|           0x06d953a8      str   x0, [arg_98h]
|           0x06d953ac      stp   x1, x2, [arg_38h]
|           0x06d953b0      add   x3, arg_58h
|           0x06d953b4      stp   xzr, xzr, [x3]
|           0x06d953b8      stp   xzr, xzr, [x3, 0x10]
|           0x06d953bc      stp   xzr, xzr, [x3, 0x20]
|           0x06d953c0      str   xzr, [x3, 0x30]
|           0x06d953c4      ldr   x3, [arg_a0h]                        ; [0xa0:4]=0x779b960
|           0x06d953c8      ldp   x4, x5, [x3, 0x40]
|           0x06d953cc      stp   x4, x5, [arg_70h]
|           0x06d953d0      ldp   x4, x5, [x3, 0x50]
|           0x06d953d4      stp   x4, x5, [arg_58h + 0x8]
|           0x06d953d8      ldrb  w4, [x3, 0x90]                       ; [0x90:4]=0
|           0x06d953dc      strb  w4, [arg_58h]
|           0x06d953e0      ldr   x3, [x3, 0x70]                       ; [0x70:4]=8
|           0x06d953e4      str   x3, [arg_70h + 0x18]
|           0x06d953e8      bl    0x52d7d00
|           0x06d953ec      str   x0, [arg_c0h]
|           0x06d953f0      ldr   x0, [arg_98h]                        ; [0x98:4]=0x779b960
|           0x06d953f4      ldp   x1, x2, [arg_38h]
|           0x06d953f8      bl    0x52d8130
|           0x06d953fc      str   x0, [arg_b8h]
|           0x06d95400      str   x1, [arg_50h]
|           0x06d95404      ldr   x0, [arg_98h]                        ; [0x98:4]=0x779b960
|           0x06d95408      ldp   x1, x2, [arg_38h]
|           0x06d9540c      bl    0x52d7e80
|           0x06d95410      str   x0, [arg_b0h]
|           0x06d95414      str   x1, [arg_48h]
|           0x06d95418      adrp  x0, 0x8b68000
|           0x06d9541c      add   x0, x0, 0x228
|           0x06d95420      bl    0x4d31c10
|           0x06d95424      adrp  x27, 0xb48f000
|           0x06d95428      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d9542c      cbz   w1, 0x6d9547c
|       |   0x06d95430      str   x0, [arg_a8h]
|       |   0x06d95434      ldp   x3, x4, [x0, 0x38]
|       |   0x06d95438      ldr   x5, [x0, 0x48]                       ; [0x48:4]=64
|       |   0x06d9543c      ldr   x6, [x0, 0x58]                       ; [0x58:4]=64
|       |   0x06d95440      bl    0x4da3560
|       |   0x06d95444      ldr   x7, [arg_c8h]                        ; [0xc8:4]=0
|       |   0x06d95448      stp   x7, x3, [x25]
|       |   0x06d9544c      ldr   x3, [arg_c0h]                        ; [0xc0:4]=0
|       |   0x06d95450      stp   x3, x4, [x25, 0x10]
|       |   0x06d95454      ldr   x3, [arg_b8h]                        ; [0xb8:4]=0
|       |   0x06d95458      stp   x3, x5, [x25, 0x20]
|       |   0x06d9545c      ldr   x3, [arg_b0h]                        ; [0xb0:4]=0x6474e551
|       |   0x06d95460      stp   x3, x6, [x25, 0x30]
|       |   0x06d95464      mov   x1, x0
|       |   0x06d95468      add   x2, arg_58h
|       |   0x06d9546c      adrp  x0, 0x8b68000
|       |   0x06d95470      add   x0, x0, 0x150
|       |   0x06d95474      bl    0x4d35930
|       |   0x06d95478      ldr   x0, [arg_a8h]                        ; [0xa8:4]=0x200000
|       `-> 0x06d9547c      add   x27, arg_58h
|           0x06d95480      ldp   q0, q1, [x27]
|           0x06d95484      ldp   q2, q3, [arg_70h]
|           0x06d95488      stp   q0, q1, [x0]
|           0x06d9548c      add   x27, x0, 0x18
|           0x06d95490      stp   q2, q3, [x27]
|           0x06d95494      ldp   x2, x1, [arg_c0h]
|           0x06d95498      stp   x1, x2, [x0, 0x38]
|           0x06d9549c      ldr   x1, [arg_b8h]                        ; [0xb8:4]=0
|           0x06d954a0      ldr   x2, [arg_50h]                        ; [0x50:4]=64
|           0x06d954a4      stp   x1, x2, [x0, 0x48]
|           0x06d954a8      ldr   x1, [arg_b0h]                        ; [0xb0:4]=0x6474e551
|           0x06d954ac      ldr   x2, [arg_48h]                        ; [0x48:4]=64
|           0x06d954b0      stp   x1, x2, [x0, 0x58]
|           0x06d954b4      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d954b8      ldr   lr, [sp], 0xe0                       ; [0xe0:4]=0
\           0x06d954bc      ret
==== AuthProvider_.SetWireProtocol
/ fcn.06d95528(int64_t arg1, int64_t arg2, int64_t arg4, int64_t arg7, int64_t arg8, int64_t arg_38h, int64_t arg_40h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_60h, int64_t arg_68h, int64_t arg_70h, int64_t arg_88h, int64_t arg_90h, int64_t arg_a8h, int64_t arg_c8h, int64_t arg_d0h, int64_t arg_e8h, int64_t arg_118h, int64_t arg_120h, int64_t arg_128h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg2 @ x1
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg7 @ x6
|           ; arg int64_t arg8 @ x7
|           ; arg int64_t arg_38h @ stack + 0x38
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_48h @ stack + 0x48
|           ; arg int64_t arg_50h @ stack + 0x50
|           ; arg int64_t arg_58h @ stack + 0x58
|           ; arg int64_t arg_60h @ stack + 0x60
|           ; arg int64_t arg_68h @ stack + 0x68
|           ; arg int64_t arg_70h @ stack + 0x70
|           ; arg int64_t arg_88h @ stack + 0x88
|           ; arg int64_t arg_90h @ stack + 0x90
|           ; arg int64_t arg_a8h @ stack + 0xa8
|           ; arg int64_t arg_c8h @ stack + 0xc8
|           ; arg int64_t arg_d0h @ stack + 0xd0
|           ; arg int64_t arg_e8h @ stack + 0xe8
|           ; arg int64_t arg_118h @ stack + 0x118
|           ; arg int64_t arg_120h @ stack + 0x120
|           ; arg int64_t arg_128h @ stack + 0x128
|           0x06d95528      ldr   x9, [x8]
|           0x06d9552c      ldr   w10, [x1, 0x10]                      ; [0x10:4]=0xb70003 ; arg2
|       ,=< 0x06d95530      b     0x6d95750
|       |   ; CODE XREFS from fcn.06d95528 @ 0x6d95798, 0x6d957a4
|     ..--> 0x06d95534      ldr   x0, [x0]                             ; arg1
|     ::|   0x06d95538      adrp  x4, 0x4264000
|     ::|   0x06d9553c      add   x4, x4, 0xa93
|     ::|   0x06d95540      orr   x5, xzr, 7
|     ::|   0x06d95544      bl    0x6d957d0
|    ,====< 0x06d95548      cbnz  x1, 0x6d9557c
|    |::|   0x06d9554c      str   x0, [arg_40h]
|    |::|   0x06d95550      ldr   x1, [arg_120h]                       ; [0x120:4]=7
|   ,=====< 0x06d95554      cbz   x1, 0x6d95574
|   ||::|   0x06d95558      ldr   x2, [x1, 8]                          ; [0x8:4]=0
|   ||::|   0x06d9555c      adrp  x3, 0x94a8000
|   ||::|   0x06d95560      add   x3, x3, 0x500
|   ||::|   0x06d95564      ldar  x4, [x3]
|   ||::|   0x06d95568      ldr   x5, [x4]
|   ||::|   0x06d9556c      ldr   w6, [x1, 0x10]                       ; [0x10:4]=0xb70003
|  ,======< 0x06d95570      b     0x6d95704
|  |`-----> 0x06d95574      mov   x2, x1
|  |,=====< 0x06d95578      b     0x6d9558c
|  ||`----> 0x06d9557c      mov   x0, xzr
|  || ::|   0x06d95580      ldp   fp, lr, [sp, -8]
|  || ::|   0x06d95584      add   sp, sp, 0x110
|  || ::|   0x06d95588      ret
|  || ::|   ; CODE XREFS from fcn.06d95528 @ 0x6d95578, 0x6d9573c, 0x6d9574c
| .-`.----> 0x06d9558c      cbnz  x1, 0x6d955a0
| :| :::|   0x06d95590      adrp  x1, 0x423d000
| :| :::|   0x06d95594      add   x1, x1, 0xa79
| :| :::|   0x06d95598      orr   x3, xzr, 4
| :|,=====< 0x06d9559c      b     0x6d955b8
| --------> 0x06d955a0      ldr   x1, [x1, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
| :||:::|   0x06d955a4      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :||:::|   0x06d955a8      blr   x1
| :||:::|   0x06d955ac      ldr   x2, [arg_120h]                       ; [0x120:4]=7
| :||:::|   0x06d955b0      mov   x3, x1
| :||:::|   0x06d955b4      mov   x1, x0
| :||:::|   ; CODE XREF from fcn.06d95528 @ 0x6d9559c
| :|`-----> 0x06d955b8      str   x1, [arg_48h]
| :| :::|   0x06d955bc      str   x3, [arg_38h]
| :| :::|   0x06d955c0      ldr   x1, [x2, 0x48]                       ; [0x48:4]=64
| :| :::|   0x06d955c4      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :| :::|   0x06d955c8      blr   x1
| :| :::|   0x06d955cc      strb  w0, [arg_70h]
| :| :::|   0x06d955d0      stp   x1, x2, [arg_70h + 0x8]
| :| :::|   0x06d955d4      stp   x3, x4, [arg_88h]
| :| :::|   0x06d955d8      strb  w5, [arg_90h + 0x8]
| :| :::|   0x06d955dc      str   x6, [arg_90h + 0x10]
| :| :::|   0x06d955e0      ldp   q0, q1, [arg_70h]
| :| :::|   0x06d955e4      add   x27, arg_88h
| :| :::|   0x06d955e8      ldp   q2, q3, [x27]
| :| :::|   0x06d955ec      stp   q0, q1, [arg_d0h]
| :| :::|   0x06d955f0      add   x27, arg_e8h
| :| :::|   0x06d955f4      stp   q2, q3, [x27]
| :| :::|   0x06d955f8      ldr   x1, [arg_120h]                       ; [0x120:4]=7
| :| :::|   0x06d955fc      ldr   x1, [x1, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
| :| :::|   0x06d95600      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :| :::|   0x06d95604      blr   x1
| :| :::|   0x06d95608      stp   x0, x1, [arg_70h]
| :| :::|   0x06d9560c      stp   x2, x3, [arg_70h + 0x10]
| :| :::|   0x06d95610      str   x4, [arg_90h]
| :| :::|   0x06d95614      ldp   q0, q1, [arg_70h]
| :| :::|   0x06d95618      ldr   x1, [arg_90h]                        ; [0x90:4]=0
| :| :::|   0x06d9561c      add   x27, arg_a8h
| :| :::|   0x06d95620      stp   q0, q1, [x27]
| :| :::|   0x06d95624      str   x1, [arg_c8h]
| :| :::|   0x06d95628      ldr   x1, [arg_120h]                       ; [0x120:4]=7
| :| :::|   0x06d9562c      ldr   x1, [x1, 0x28]                       ; [0x28:4]=0xace39d8
| :| :::|   0x06d95630      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :| :::|   0x06d95634      blr   x1
| :| :::|   0x06d95638      str   x0, [arg_68h]
| :| :::|   0x06d9563c      adrp  x0, 0x8be2000
| :| :::|   0x06d95640      add   x0, x0, 0x5f8
| :| :::|   0x06d95644      bl    0x4d31c10
| :| :::|   0x06d95648      adrp  x27, 0xb48f000
| :| :::|   0x06d9564c      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
| :|,=====< 0x06d95650      cbz   w1, 0x6d956ac
| :||:::|   0x06d95654      str   x0, [arg_60h]
| :||:::|   0x06d95658      add   x1, x0, 8
| :||:::|   0x06d9565c      add   x3, x0, 0x40
| :||:::|   0x06d95660      str   x3, [arg_58h]
| :||:::|   0x06d95664      ldp   x3, x4, [x0, 0x68]
| :||:::|   0x06d95668      bl    0x4da3530
| :||:::|   0x06d9566c      ldr   x5, [arg_40h]                        ; segment.PHDR
| :||:::|                                                              ; [0x40:4]=6 ; "\U00000006"
| :||:::|   0x06d95670      ldr   x6, [arg_68h]                        ; [0x68:4]=0x380
| :||:::|   0x06d95674      stp   x5, x6, [x25]
| :||:::|   0x06d95678      ldr   x5, [arg_48h]                        ; [0x48:4]=64
| :||:::|   0x06d9567c      stp   x3, x5, [x25, 0x10]
| :||:::|   0x06d95680      str   x4, [x25, 0x20]
| :||:::|   0x06d95684      adrp  x0, 0x8b68000
| :||:::|   0x06d95688      add   x0, x0, 0x150
| :||:::|   0x06d9568c      add   x2, arg_d0h
| :||:::|   0x06d95690      bl    0x4d35930
| :||:::|   0x06d95694      adrp  x0, 0x89be000
| :||:::|   0x06d95698      add   x0, x0, 0x530
| :||:::|   0x06d9569c      ldr   x1, [arg_58h]                        ; [0x58:4]=64
| :||:::|   0x06d956a0      add   x2, arg_a8h
| :||:::|   0x06d956a4      bl    0x4d35930
| :||:::|   0x06d956a8      ldr   x0, [arg_60h]                        ; [0x60:4]=0x380
| :|`-----> 0x06d956ac      ldr   x3, [arg_40h]                        ; segment.PHDR
| :| :::|                                                              ; [0x40:4]=6 ; "\U00000006"
| :| :::|   0x06d956b0      str   x3, [x0]
| :| :::|   0x06d956b4      ldp   q0, q1, [arg_d0h]
| :| :::|   0x06d956b8      add   x27, arg_e8h
| :| :::|   0x06d956bc      ldp   q2, q3, [x27]
| :| :::|   0x06d956c0      add   x27, x0, 8
| :| :::|   0x06d956c4      stp   q0, q1, [x27]
| :| :::|   0x06d956c8      stp   q2, q3, [x0, 0x20]
| :| :::|   0x06d956cc      add   x27, arg_a8h
| :| :::|   0x06d956d0      ldp   q0, q1, [x27]
| :| :::|   0x06d956d4      ldr   x3, [arg_c8h]                        ; [0xc8:4]=0
| :| :::|   0x06d956d8      stp   q0, q1, [x0, 0x40]
| :| :::|   0x06d956dc      ldr   x4, [arg_68h]                        ; [0x68:4]=0x380
| :| :::|   0x06d956e0      stp   x3, x4, [x0, 0x60]
| :| :::|   0x06d956e4      ldr   x3, [arg_48h]                        ; [0x48:4]=64
| :| :::|   0x06d956e8      ldr   x4, [arg_38h]                        ; [0x38:4]=0x400010
| :| :::|   0x06d956ec      stp   x3, x4, [x0, 0x70]
| :| :::|   0x06d956f0      mov   x1, xzr
| :| :::|   0x06d956f4      mov   x2, xzr
| :| :::|   0x06d956f8      ldp   fp, lr, [sp, -8]
| :| :::|   0x06d956fc      add   sp, sp, 0x110
| :| :::|   0x06d95700      ret
| :| :::|   ; CODE XREF from fcn.06d95528 @ 0x6d95570
| :`.-----> 0x06d95704      and   x7, x6, x5
| : ::::|   0x06d95708      lsl   x7, x7, 4
| : ::::|   0x06d9570c      add   x7, x7, 8
| : ::::|   0x06d95710      ldr   x8, [x4, x7]
| : ::::|   0x06d95714      add   x7, x4, x7
| : ::::|   0x06d95718      cmp   x8, x2
| :,======< 0x06d9571c      b.eq  0x6d95740
| :|::::|   0x06d95720      add   x6, x6, 1
| :|`=====< 0x06d95724      cbnz  x8, 0x6d95704
| :| :::|   0x06d95728      mov   x0, x3
| :| :::|   0x06d9572c      mov   x1, x2
| :| :::|   0x06d95730      bl    0x4d2e4c0
| :| :::|   0x06d95734      ldr   x2, [arg_120h]                       ; [0x120:4]=7
| :| :::|   0x06d95738      mov   x1, x0
| `=======< 0x06d9573c      b     0x6d9558c
|  `------> 0x06d95740      ldr   x3, [x7, 8]                          ; [0x8:4]=0
|    :::|   0x06d95744      mov   x2, x1
|    :::|   0x06d95748      mov   x1, x3
|    `====< 0x06d9574c      b     0x6d9558c
|     ::|   ; CODE XREF from fcn.06d95528 @ 0x6d95530
|    .--`-> 0x06d95750      and   x11, x10, x9
|    :::    0x06d95754      lsl   x11, x11, 4
|    :::    0x06d95758      add   x11, x11, 8
|    :::    0x06d9575c      ldr   x12, [x8, x11]
|    :::    0x06d95760      add   x11, x8, x11
|    :::    0x06d95764      cmp   x12, x6                              ; arg7
|    :::,=< 0x06d95768      b.eq  0x6d9579c
|    :::|   0x06d9576c      add   x10, x10, 1
|    `====< 0x06d95770      cbnz  x12, 0x6d95750
|     ::|   0x06d95774      str   x0, [arg_118h]
|     ::|   0x06d95778      str   x3, [arg_50h]                        ; arg4
|     ::|   0x06d9577c      mov   x0, x7                               ; arg8
|     ::|   0x06d95780      mov   x1, x6                               ; arg7
|     ::|   0x06d95784      bl    0x4d2e4c0
|     ::|   0x06d95788      ldr   x2, [arg_128h]                       ; [0x128:4]=0x779b960
|     ::|   0x06d9578c      ldr   x3, [arg_50h]                        ; [0x50:4]=64
|     ::|   0x06d95790      mov   x1, x0
|     ::|   0x06d95794      ldr   x0, [arg_118h]                       ; [0x118:4]=0x200000
|     `===< 0x06d95798      b     0x6d95534
|      :`-> 0x06d9579c      ldr   x6, [x11, 8]                         ; [0x8:4]=0
|      :    0x06d957a0      mov   x1, x6
\      `==< 0x06d957a4      b     0x6d95534
==== AuthProvider_.IsEval
/ fcn.06d95528(int64_t arg1, int64_t arg2, int64_t arg4, int64_t arg7, int64_t arg8, int64_t arg_38h, int64_t arg_40h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_60h, int64_t arg_68h, int64_t arg_70h, int64_t arg_88h, int64_t arg_90h, int64_t arg_a8h, int64_t arg_c8h, int64_t arg_d0h, int64_t arg_e8h, int64_t arg_118h, int64_t arg_120h, int64_t arg_128h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg2 @ x1
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg7 @ x6
|           ; arg int64_t arg8 @ x7
|           ; arg int64_t arg_38h @ stack + 0x38
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_48h @ stack + 0x48
|           ; arg int64_t arg_50h @ stack + 0x50
|           ; arg int64_t arg_58h @ stack + 0x58
|           ; arg int64_t arg_60h @ stack + 0x60
|           ; arg int64_t arg_68h @ stack + 0x68
|           ; arg int64_t arg_70h @ stack + 0x70
|           ; arg int64_t arg_88h @ stack + 0x88
|           ; arg int64_t arg_90h @ stack + 0x90
|           ; arg int64_t arg_a8h @ stack + 0xa8
|           ; arg int64_t arg_c8h @ stack + 0xc8
|           ; arg int64_t arg_d0h @ stack + 0xd0
|           ; arg int64_t arg_e8h @ stack + 0xe8
|           ; arg int64_t arg_118h @ stack + 0x118
|           ; arg int64_t arg_120h @ stack + 0x120
|           ; arg int64_t arg_128h @ stack + 0x128
|           0x06d95528      ldr   x9, [x8]
|           0x06d9552c      ldr   w10, [x1, 0x10]                      ; [0x10:4]=0xb70003 ; arg2
|       ,=< 0x06d95530      b     0x6d95750
|       |   ; CODE XREFS from fcn.06d95528 @ 0x6d95798, 0x6d957a4
|     ..--> 0x06d95534      ldr   x0, [x0]                             ; arg1
|     ::|   0x06d95538      adrp  x4, 0x4264000
|     ::|   0x06d9553c      add   x4, x4, 0xa93
|     ::|   0x06d95540      orr   x5, xzr, 7
|     ::|   0x06d95544      bl    0x6d957d0
|    ,====< 0x06d95548      cbnz  x1, 0x6d9557c
|    |::|   0x06d9554c      str   x0, [arg_40h]
|    |::|   0x06d95550      ldr   x1, [arg_120h]                       ; [0x120:4]=7
|   ,=====< 0x06d95554      cbz   x1, 0x6d95574
|   ||::|   0x06d95558      ldr   x2, [x1, 8]                          ; [0x8:4]=0
|   ||::|   0x06d9555c      adrp  x3, 0x94a8000
|   ||::|   0x06d95560      add   x3, x3, 0x500
|   ||::|   0x06d95564      ldar  x4, [x3]
|   ||::|   0x06d95568      ldr   x5, [x4]
|   ||::|   0x06d9556c      ldr   w6, [x1, 0x10]                       ; [0x10:4]=0xb70003
|  ,======< 0x06d95570      b     0x6d95704
|  |`-----> 0x06d95574      mov   x2, x1
|  |,=====< 0x06d95578      b     0x6d9558c
|  ||`----> 0x06d9557c      mov   x0, xzr
|  || ::|   0x06d95580      ldp   fp, lr, [sp, -8]
|  || ::|   0x06d95584      add   sp, sp, 0x110
|  || ::|   0x06d95588      ret
|  || ::|   ; CODE XREFS from fcn.06d95528 @ 0x6d95578, 0x6d9573c, 0x6d9574c
| .-`.----> 0x06d9558c      cbnz  x1, 0x6d955a0
| :| :::|   0x06d95590      adrp  x1, 0x423d000
| :| :::|   0x06d95594      add   x1, x1, 0xa79
| :| :::|   0x06d95598      orr   x3, xzr, 4
| :|,=====< 0x06d9559c      b     0x6d955b8
| --------> 0x06d955a0      ldr   x1, [x1, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
| :||:::|   0x06d955a4      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :||:::|   0x06d955a8      blr   x1
| :||:::|   0x06d955ac      ldr   x2, [arg_120h]                       ; [0x120:4]=7
| :||:::|   0x06d955b0      mov   x3, x1
| :||:::|   0x06d955b4      mov   x1, x0
| :||:::|   ; CODE XREF from fcn.06d95528 @ 0x6d9559c
| :|`-----> 0x06d955b8      str   x1, [arg_48h]
| :| :::|   0x06d955bc      str   x3, [arg_38h]
| :| :::|   0x06d955c0      ldr   x1, [x2, 0x48]                       ; [0x48:4]=64
| :| :::|   0x06d955c4      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :| :::|   0x06d955c8      blr   x1
| :| :::|   0x06d955cc      strb  w0, [arg_70h]
| :| :::|   0x06d955d0      stp   x1, x2, [arg_70h + 0x8]
| :| :::|   0x06d955d4      stp   x3, x4, [arg_88h]
| :| :::|   0x06d955d8      strb  w5, [arg_90h + 0x8]
| :| :::|   0x06d955dc      str   x6, [arg_90h + 0x10]
| :| :::|   0x06d955e0      ldp   q0, q1, [arg_70h]
| :| :::|   0x06d955e4      add   x27, arg_88h
| :| :::|   0x06d955e8      ldp   q2, q3, [x27]
| :| :::|   0x06d955ec      stp   q0, q1, [arg_d0h]
| :| :::|   0x06d955f0      add   x27, arg_e8h
| :| :::|   0x06d955f4      stp   q2, q3, [x27]
| :| :::|   0x06d955f8      ldr   x1, [arg_120h]                       ; [0x120:4]=7
| :| :::|   0x06d955fc      ldr   x1, [x1, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
| :| :::|   0x06d95600      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :| :::|   0x06d95604      blr   x1
| :| :::|   0x06d95608      stp   x0, x1, [arg_70h]
| :| :::|   0x06d9560c      stp   x2, x3, [arg_70h + 0x10]
| :| :::|   0x06d95610      str   x4, [arg_90h]
| :| :::|   0x06d95614      ldp   q0, q1, [arg_70h]
| :| :::|   0x06d95618      ldr   x1, [arg_90h]                        ; [0x90:4]=0
| :| :::|   0x06d9561c      add   x27, arg_a8h
| :| :::|   0x06d95620      stp   q0, q1, [x27]
| :| :::|   0x06d95624      str   x1, [arg_c8h]
| :| :::|   0x06d95628      ldr   x1, [arg_120h]                       ; [0x120:4]=7
| :| :::|   0x06d9562c      ldr   x1, [x1, 0x28]                       ; [0x28:4]=0xace39d8
| :| :::|   0x06d95630      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :| :::|   0x06d95634      blr   x1
| :| :::|   0x06d95638      str   x0, [arg_68h]
| :| :::|   0x06d9563c      adrp  x0, 0x8be2000
| :| :::|   0x06d95640      add   x0, x0, 0x5f8
| :| :::|   0x06d95644      bl    0x4d31c10
| :| :::|   0x06d95648      adrp  x27, 0xb48f000
| :| :::|   0x06d9564c      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
| :|,=====< 0x06d95650      cbz   w1, 0x6d956ac
| :||:::|   0x06d95654      str   x0, [arg_60h]
| :||:::|   0x06d95658      add   x1, x0, 8
| :||:::|   0x06d9565c      add   x3, x0, 0x40
| :||:::|   0x06d95660      str   x3, [arg_58h]
| :||:::|   0x06d95664      ldp   x3, x4, [x0, 0x68]
| :||:::|   0x06d95668      bl    0x4da3530
| :||:::|   0x06d9566c      ldr   x5, [arg_40h]                        ; segment.PHDR
| :||:::|                                                              ; [0x40:4]=6 ; "\U00000006"
| :||:::|   0x06d95670      ldr   x6, [arg_68h]                        ; [0x68:4]=0x380
| :||:::|   0x06d95674      stp   x5, x6, [x25]
| :||:::|   0x06d95678      ldr   x5, [arg_48h]                        ; [0x48:4]=64
| :||:::|   0x06d9567c      stp   x3, x5, [x25, 0x10]
| :||:::|   0x06d95680      str   x4, [x25, 0x20]
| :||:::|   0x06d95684      adrp  x0, 0x8b68000
| :||:::|   0x06d95688      add   x0, x0, 0x150
| :||:::|   0x06d9568c      add   x2, arg_d0h
| :||:::|   0x06d95690      bl    0x4d35930
| :||:::|   0x06d95694      adrp  x0, 0x89be000
| :||:::|   0x06d95698      add   x0, x0, 0x530
| :||:::|   0x06d9569c      ldr   x1, [arg_58h]                        ; [0x58:4]=64
| :||:::|   0x06d956a0      add   x2, arg_a8h
| :||:::|   0x06d956a4      bl    0x4d35930
| :||:::|   0x06d956a8      ldr   x0, [arg_60h]                        ; [0x60:4]=0x380
| :|`-----> 0x06d956ac      ldr   x3, [arg_40h]                        ; segment.PHDR
| :| :::|                                                              ; [0x40:4]=6 ; "\U00000006"
| :| :::|   0x06d956b0      str   x3, [x0]
| :| :::|   0x06d956b4      ldp   q0, q1, [arg_d0h]
| :| :::|   0x06d956b8      add   x27, arg_e8h
| :| :::|   0x06d956bc      ldp   q2, q3, [x27]
| :| :::|   0x06d956c0      add   x27, x0, 8
| :| :::|   0x06d956c4      stp   q0, q1, [x27]
| :| :::|   0x06d956c8      stp   q2, q3, [x0, 0x20]
| :| :::|   0x06d956cc      add   x27, arg_a8h
| :| :::|   0x06d956d0      ldp   q0, q1, [x27]
| :| :::|   0x06d956d4      ldr   x3, [arg_c8h]                        ; [0xc8:4]=0
| :| :::|   0x06d956d8      stp   q0, q1, [x0, 0x40]
| :| :::|   0x06d956dc      ldr   x4, [arg_68h]                        ; [0x68:4]=0x380
| :| :::|   0x06d956e0      stp   x3, x4, [x0, 0x60]
| :| :::|   0x06d956e4      ldr   x3, [arg_48h]                        ; [0x48:4]=64
| :| :::|   0x06d956e8      ldr   x4, [arg_38h]                        ; [0x38:4]=0x400010
| :| :::|   0x06d956ec      stp   x3, x4, [x0, 0x70]
| :| :::|   0x06d956f0      mov   x1, xzr
| :| :::|   0x06d956f4      mov   x2, xzr
| :| :::|   0x06d956f8      ldp   fp, lr, [sp, -8]
| :| :::|   0x06d956fc      add   sp, sp, 0x110
| :| :::|   0x06d95700      ret
| :| :::|   ; CODE XREF from fcn.06d95528 @ 0x6d95570
| :`.-----> 0x06d95704      and   x7, x6, x5
| : ::::|   0x06d95708      lsl   x7, x7, 4
| : ::::|   0x06d9570c      add   x7, x7, 8
| : ::::|   0x06d95710      ldr   x8, [x4, x7]
| : ::::|   0x06d95714      add   x7, x4, x7
| : ::::|   0x06d95718      cmp   x8, x2
| :,======< 0x06d9571c      b.eq  0x6d95740
| :|::::|   0x06d95720      add   x6, x6, 1
| :|`=====< 0x06d95724      cbnz  x8, 0x6d95704
| :| :::|   0x06d95728      mov   x0, x3
| :| :::|   0x06d9572c      mov   x1, x2
| :| :::|   0x06d95730      bl    0x4d2e4c0
| :| :::|   0x06d95734      ldr   x2, [arg_120h]                       ; [0x120:4]=7
| :| :::|   0x06d95738      mov   x1, x0
| `=======< 0x06d9573c      b     0x6d9558c
|  `------> 0x06d95740      ldr   x3, [x7, 8]                          ; [0x8:4]=0
|    :::|   0x06d95744      mov   x2, x1
|    :::|   0x06d95748      mov   x1, x3
|    `====< 0x06d9574c      b     0x6d9558c
|     ::|   ; CODE XREF from fcn.06d95528 @ 0x6d95530
|    .--`-> 0x06d95750      and   x11, x10, x9
|    :::    0x06d95754      lsl   x11, x11, 4
|    :::    0x06d95758      add   x11, x11, 8
|    :::    0x06d9575c      ldr   x12, [x8, x11]
|    :::    0x06d95760      add   x11, x8, x11
|    :::    0x06d95764      cmp   x12, x6                              ; arg7
|    :::,=< 0x06d95768      b.eq  0x6d9579c
|    :::|   0x06d9576c      add   x10, x10, 1
|    `====< 0x06d95770      cbnz  x12, 0x6d95750
|     ::|   0x06d95774      str   x0, [arg_118h]
|     ::|   0x06d95778      str   x3, [arg_50h]                        ; arg4
|     ::|   0x06d9577c      mov   x0, x7                               ; arg8
|     ::|   0x06d95780      mov   x1, x6                               ; arg7
|     ::|   0x06d95784      bl    0x4d2e4c0
|     ::|   0x06d95788      ldr   x2, [arg_128h]                       ; [0x128:4]=0x779b960
|     ::|   0x06d9578c      ldr   x3, [arg_50h]                        ; [0x50:4]=64
|     ::|   0x06d95790      mov   x1, x0
|     ::|   0x06d95794      ldr   x0, [arg_118h]                       ; [0x118:4]=0x200000
|     `===< 0x06d95798      b     0x6d95534
|      :`-> 0x06d9579c      ldr   x6, [x11, 8]                         ; [0x8:4]=0
|      :    0x06d957a0      mov   x1, x6
\      `==< 0x06d957a4      b     0x6d95534
==== AuthProvider_.ShouldUseCAICProjectAsOverride
/ fcn.06d95528(int64_t arg1, int64_t arg2, int64_t arg4, int64_t arg7, int64_t arg8, int64_t arg_38h, int64_t arg_40h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_60h, int64_t arg_68h, int64_t arg_70h, int64_t arg_88h, int64_t arg_90h, int64_t arg_a8h, int64_t arg_c8h, int64_t arg_d0h, int64_t arg_e8h, int64_t arg_118h, int64_t arg_120h, int64_t arg_128h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg2 @ x1
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg7 @ x6
|           ; arg int64_t arg8 @ x7
|           ; arg int64_t arg_38h @ stack + 0x38
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_48h @ stack + 0x48
|           ; arg int64_t arg_50h @ stack + 0x50
|           ; arg int64_t arg_58h @ stack + 0x58
|           ; arg int64_t arg_60h @ stack + 0x60
|           ; arg int64_t arg_68h @ stack + 0x68
|           ; arg int64_t arg_70h @ stack + 0x70
|           ; arg int64_t arg_88h @ stack + 0x88
|           ; arg int64_t arg_90h @ stack + 0x90
|           ; arg int64_t arg_a8h @ stack + 0xa8
|           ; arg int64_t arg_c8h @ stack + 0xc8
|           ; arg int64_t arg_d0h @ stack + 0xd0
|           ; arg int64_t arg_e8h @ stack + 0xe8
|           ; arg int64_t arg_118h @ stack + 0x118
|           ; arg int64_t arg_120h @ stack + 0x120
|           ; arg int64_t arg_128h @ stack + 0x128
|           0x06d95528      ldr   x9, [x8]
|           0x06d9552c      ldr   w10, [x1, 0x10]                      ; [0x10:4]=0xb70003 ; arg2
|       ,=< 0x06d95530      b     0x6d95750
|       |   ; CODE XREFS from fcn.06d95528 @ 0x6d95798, 0x6d957a4
|     ..--> 0x06d95534      ldr   x0, [x0]                             ; arg1
|     ::|   0x06d95538      adrp  x4, 0x4264000
|     ::|   0x06d9553c      add   x4, x4, 0xa93
|     ::|   0x06d95540      orr   x5, xzr, 7
|     ::|   0x06d95544      bl    0x6d957d0
|    ,====< 0x06d95548      cbnz  x1, 0x6d9557c
|    |::|   0x06d9554c      str   x0, [arg_40h]
|    |::|   0x06d95550      ldr   x1, [arg_120h]                       ; [0x120:4]=7
|   ,=====< 0x06d95554      cbz   x1, 0x6d95574
|   ||::|   0x06d95558      ldr   x2, [x1, 8]                          ; [0x8:4]=0
|   ||::|   0x06d9555c      adrp  x3, 0x94a8000
|   ||::|   0x06d95560      add   x3, x3, 0x500
|   ||::|   0x06d95564      ldar  x4, [x3]
|   ||::|   0x06d95568      ldr   x5, [x4]
|   ||::|   0x06d9556c      ldr   w6, [x1, 0x10]                       ; [0x10:4]=0xb70003
|  ,======< 0x06d95570      b     0x6d95704
|  |`-----> 0x06d95574      mov   x2, x1
|  |,=====< 0x06d95578      b     0x6d9558c
|  ||`----> 0x06d9557c      mov   x0, xzr
|  || ::|   0x06d95580      ldp   fp, lr, [sp, -8]
|  || ::|   0x06d95584      add   sp, sp, 0x110
|  || ::|   0x06d95588      ret
|  || ::|   ; CODE XREFS from fcn.06d95528 @ 0x6d95578, 0x6d9573c, 0x6d9574c
| .-`.----> 0x06d9558c      cbnz  x1, 0x6d955a0
| :| :::|   0x06d95590      adrp  x1, 0x423d000
| :| :::|   0x06d95594      add   x1, x1, 0xa79
| :| :::|   0x06d95598      orr   x3, xzr, 4
| :|,=====< 0x06d9559c      b     0x6d955b8
| --------> 0x06d955a0      ldr   x1, [x1, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
| :||:::|   0x06d955a4      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :||:::|   0x06d955a8      blr   x1
| :||:::|   0x06d955ac      ldr   x2, [arg_120h]                       ; [0x120:4]=7
| :||:::|   0x06d955b0      mov   x3, x1
| :||:::|   0x06d955b4      mov   x1, x0
| :||:::|   ; CODE XREF from fcn.06d95528 @ 0x6d9559c
| :|`-----> 0x06d955b8      str   x1, [arg_48h]
| :| :::|   0x06d955bc      str   x3, [arg_38h]
| :| :::|   0x06d955c0      ldr   x1, [x2, 0x48]                       ; [0x48:4]=64
| :| :::|   0x06d955c4      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :| :::|   0x06d955c8      blr   x1
| :| :::|   0x06d955cc      strb  w0, [arg_70h]
| :| :::|   0x06d955d0      stp   x1, x2, [arg_70h + 0x8]
| :| :::|   0x06d955d4      stp   x3, x4, [arg_88h]
| :| :::|   0x06d955d8      strb  w5, [arg_90h + 0x8]
| :| :::|   0x06d955dc      str   x6, [arg_90h + 0x10]
| :| :::|   0x06d955e0      ldp   q0, q1, [arg_70h]
| :| :::|   0x06d955e4      add   x27, arg_88h
| :| :::|   0x06d955e8      ldp   q2, q3, [x27]
| :| :::|   0x06d955ec      stp   q0, q1, [arg_d0h]
| :| :::|   0x06d955f0      add   x27, arg_e8h
| :| :::|   0x06d955f4      stp   q2, q3, [x27]
| :| :::|   0x06d955f8      ldr   x1, [arg_120h]                       ; [0x120:4]=7
| :| :::|   0x06d955fc      ldr   x1, [x1, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
| :| :::|   0x06d95600      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :| :::|   0x06d95604      blr   x1
| :| :::|   0x06d95608      stp   x0, x1, [arg_70h]
| :| :::|   0x06d9560c      stp   x2, x3, [arg_70h + 0x10]
| :| :::|   0x06d95610      str   x4, [arg_90h]
| :| :::|   0x06d95614      ldp   q0, q1, [arg_70h]
| :| :::|   0x06d95618      ldr   x1, [arg_90h]                        ; [0x90:4]=0
| :| :::|   0x06d9561c      add   x27, arg_a8h
| :| :::|   0x06d95620      stp   q0, q1, [x27]
| :| :::|   0x06d95624      str   x1, [arg_c8h]
| :| :::|   0x06d95628      ldr   x1, [arg_120h]                       ; [0x120:4]=7
| :| :::|   0x06d9562c      ldr   x1, [x1, 0x28]                       ; [0x28:4]=0xace39d8
| :| :::|   0x06d95630      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :| :::|   0x06d95634      blr   x1
| :| :::|   0x06d95638      str   x0, [arg_68h]
| :| :::|   0x06d9563c      adrp  x0, 0x8be2000
| :| :::|   0x06d95640      add   x0, x0, 0x5f8
| :| :::|   0x06d95644      bl    0x4d31c10
| :| :::|   0x06d95648      adrp  x27, 0xb48f000
| :| :::|   0x06d9564c      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
| :|,=====< 0x06d95650      cbz   w1, 0x6d956ac
| :||:::|   0x06d95654      str   x0, [arg_60h]
| :||:::|   0x06d95658      add   x1, x0, 8
| :||:::|   0x06d9565c      add   x3, x0, 0x40
| :||:::|   0x06d95660      str   x3, [arg_58h]
| :||:::|   0x06d95664      ldp   x3, x4, [x0, 0x68]
| :||:::|   0x06d95668      bl    0x4da3530
| :||:::|   0x06d9566c      ldr   x5, [arg_40h]                        ; segment.PHDR
| :||:::|                                                              ; [0x40:4]=6 ; "\U00000006"
| :||:::|   0x06d95670      ldr   x6, [arg_68h]                        ; [0x68:4]=0x380
| :||:::|   0x06d95674      stp   x5, x6, [x25]
| :||:::|   0x06d95678      ldr   x5, [arg_48h]                        ; [0x48:4]=64
| :||:::|   0x06d9567c      stp   x3, x5, [x25, 0x10]
| :||:::|   0x06d95680      str   x4, [x25, 0x20]
| :||:::|   0x06d95684      adrp  x0, 0x8b68000
| :||:::|   0x06d95688      add   x0, x0, 0x150
| :||:::|   0x06d9568c      add   x2, arg_d0h
| :||:::|   0x06d95690      bl    0x4d35930
| :||:::|   0x06d95694      adrp  x0, 0x89be000
| :||:::|   0x06d95698      add   x0, x0, 0x530
| :||:::|   0x06d9569c      ldr   x1, [arg_58h]                        ; [0x58:4]=64
| :||:::|   0x06d956a0      add   x2, arg_a8h
| :||:::|   0x06d956a4      bl    0x4d35930
| :||:::|   0x06d956a8      ldr   x0, [arg_60h]                        ; [0x60:4]=0x380
| :|`-----> 0x06d956ac      ldr   x3, [arg_40h]                        ; segment.PHDR
| :| :::|                                                              ; [0x40:4]=6 ; "\U00000006"
| :| :::|   0x06d956b0      str   x3, [x0]
| :| :::|   0x06d956b4      ldp   q0, q1, [arg_d0h]
| :| :::|   0x06d956b8      add   x27, arg_e8h
| :| :::|   0x06d956bc      ldp   q2, q3, [x27]
| :| :::|   0x06d956c0      add   x27, x0, 8
| :| :::|   0x06d956c4      stp   q0, q1, [x27]
| :| :::|   0x06d956c8      stp   q2, q3, [x0, 0x20]
| :| :::|   0x06d956cc      add   x27, arg_a8h
| :| :::|   0x06d956d0      ldp   q0, q1, [x27]
| :| :::|   0x06d956d4      ldr   x3, [arg_c8h]                        ; [0xc8:4]=0
| :| :::|   0x06d956d8      stp   q0, q1, [x0, 0x40]
| :| :::|   0x06d956dc      ldr   x4, [arg_68h]                        ; [0x68:4]=0x380
| :| :::|   0x06d956e0      stp   x3, x4, [x0, 0x60]
| :| :::|   0x06d956e4      ldr   x3, [arg_48h]                        ; [0x48:4]=64
| :| :::|   0x06d956e8      ldr   x4, [arg_38h]                        ; [0x38:4]=0x400010
| :| :::|   0x06d956ec      stp   x3, x4, [x0, 0x70]
| :| :::|   0x06d956f0      mov   x1, xzr
| :| :::|   0x06d956f4      mov   x2, xzr
| :| :::|   0x06d956f8      ldp   fp, lr, [sp, -8]
| :| :::|   0x06d956fc      add   sp, sp, 0x110
| :| :::|   0x06d95700      ret
| :| :::|   ; CODE XREF from fcn.06d95528 @ 0x6d95570
| :`.-----> 0x06d95704      and   x7, x6, x5
| : ::::|   0x06d95708      lsl   x7, x7, 4
| : ::::|   0x06d9570c      add   x7, x7, 8
| : ::::|   0x06d95710      ldr   x8, [x4, x7]
| : ::::|   0x06d95714      add   x7, x4, x7
| : ::::|   0x06d95718      cmp   x8, x2
| :,======< 0x06d9571c      b.eq  0x6d95740
| :|::::|   0x06d95720      add   x6, x6, 1
| :|`=====< 0x06d95724      cbnz  x8, 0x6d95704
| :| :::|   0x06d95728      mov   x0, x3
| :| :::|   0x06d9572c      mov   x1, x2
| :| :::|   0x06d95730      bl    0x4d2e4c0
| :| :::|   0x06d95734      ldr   x2, [arg_120h]                       ; [0x120:4]=7
| :| :::|   0x06d95738      mov   x1, x0
| `=======< 0x06d9573c      b     0x6d9558c
|  `------> 0x06d95740      ldr   x3, [x7, 8]                          ; [0x8:4]=0
|    :::|   0x06d95744      mov   x2, x1
|    :::|   0x06d95748      mov   x1, x3
|    `====< 0x06d9574c      b     0x6d9558c
|     ::|   ; CODE XREF from fcn.06d95528 @ 0x6d95530
|    .--`-> 0x06d95750      and   x11, x10, x9
|    :::    0x06d95754      lsl   x11, x11, 4
|    :::    0x06d95758      add   x11, x11, 8
|    :::    0x06d9575c      ldr   x12, [x8, x11]
|    :::    0x06d95760      add   x11, x8, x11
|    :::    0x06d95764      cmp   x12, x6                              ; arg7
|    :::,=< 0x06d95768      b.eq  0x6d9579c
|    :::|   0x06d9576c      add   x10, x10, 1
|    `====< 0x06d95770      cbnz  x12, 0x6d95750
|     ::|   0x06d95774      str   x0, [arg_118h]
|     ::|   0x06d95778      str   x3, [arg_50h]                        ; arg4
|     ::|   0x06d9577c      mov   x0, x7                               ; arg8
|     ::|   0x06d95780      mov   x1, x6                               ; arg7
|     ::|   0x06d95784      bl    0x4d2e4c0
|     ::|   0x06d95788      ldr   x2, [arg_128h]                       ; [0x128:4]=0x779b960
|     ::|   0x06d9578c      ldr   x3, [arg_50h]                        ; [0x50:4]=64
|     ::|   0x06d95790      mov   x1, x0
|     ::|   0x06d95794      ldr   x0, [arg_118h]                       ; [0x118:4]=0x200000
|     `===< 0x06d95798      b     0x6d95534
|      :`-> 0x06d9579c      ldr   x6, [x11, 8]                         ; [0x8:4]=0
|      :    0x06d957a0      mov   x1, x6
\      `==< 0x06d957a4      b     0x6d95534
==== AuthProvider_.SetHTTPHeaders
/ fcn.06d95528(int64_t arg1, int64_t arg2, int64_t arg4, int64_t arg7, int64_t arg8, int64_t arg_38h, int64_t arg_40h, int64_t arg_48h, int64_t arg_50h, int64_t arg_58h, int64_t arg_60h, int64_t arg_68h, int64_t arg_70h, int64_t arg_88h, int64_t arg_90h, int64_t arg_a8h, int64_t arg_c8h, int64_t arg_d0h, int64_t arg_e8h, int64_t arg_118h, int64_t arg_120h, int64_t arg_128h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg2 @ x1
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg7 @ x6
|           ; arg int64_t arg8 @ x7
|           ; arg int64_t arg_38h @ stack + 0x38
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_48h @ stack + 0x48
|           ; arg int64_t arg_50h @ stack + 0x50
|           ; arg int64_t arg_58h @ stack + 0x58
|           ; arg int64_t arg_60h @ stack + 0x60
|           ; arg int64_t arg_68h @ stack + 0x68
|           ; arg int64_t arg_70h @ stack + 0x70
|           ; arg int64_t arg_88h @ stack + 0x88
|           ; arg int64_t arg_90h @ stack + 0x90
|           ; arg int64_t arg_a8h @ stack + 0xa8
|           ; arg int64_t arg_c8h @ stack + 0xc8
|           ; arg int64_t arg_d0h @ stack + 0xd0
|           ; arg int64_t arg_e8h @ stack + 0xe8
|           ; arg int64_t arg_118h @ stack + 0x118
|           ; arg int64_t arg_120h @ stack + 0x120
|           ; arg int64_t arg_128h @ stack + 0x128
|           0x06d95528      ldr   x9, [x8]
|           0x06d9552c      ldr   w10, [x1, 0x10]                      ; [0x10:4]=0xb70003 ; arg2
|       ,=< 0x06d95530      b     0x6d95750
|       |   ; CODE XREFS from fcn.06d95528 @ 0x6d95798, 0x6d957a4
|     ..--> 0x06d95534      ldr   x0, [x0]                             ; arg1
|     ::|   0x06d95538      adrp  x4, 0x4264000
|     ::|   0x06d9553c      add   x4, x4, 0xa93
|     ::|   0x06d95540      orr   x5, xzr, 7
|     ::|   0x06d95544      bl    0x6d957d0
|    ,====< 0x06d95548      cbnz  x1, 0x6d9557c
|    |::|   0x06d9554c      str   x0, [arg_40h]
|    |::|   0x06d95550      ldr   x1, [arg_120h]                       ; [0x120:4]=7
|   ,=====< 0x06d95554      cbz   x1, 0x6d95574
|   ||::|   0x06d95558      ldr   x2, [x1, 8]                          ; [0x8:4]=0
|   ||::|   0x06d9555c      adrp  x3, 0x94a8000
|   ||::|   0x06d95560      add   x3, x3, 0x500
|   ||::|   0x06d95564      ldar  x4, [x3]
|   ||::|   0x06d95568      ldr   x5, [x4]
|   ||::|   0x06d9556c      ldr   w6, [x1, 0x10]                       ; [0x10:4]=0xb70003
|  ,======< 0x06d95570      b     0x6d95704
|  |`-----> 0x06d95574      mov   x2, x1
|  |,=====< 0x06d95578      b     0x6d9558c
|  ||`----> 0x06d9557c      mov   x0, xzr
|  || ::|   0x06d95580      ldp   fp, lr, [sp, -8]
|  || ::|   0x06d95584      add   sp, sp, 0x110
|  || ::|   0x06d95588      ret
|  || ::|   ; CODE XREFS from fcn.06d95528 @ 0x6d95578, 0x6d9573c, 0x6d9574c
| .-`.----> 0x06d9558c      cbnz  x1, 0x6d955a0
| :| :::|   0x06d95590      adrp  x1, 0x423d000
| :| :::|   0x06d95594      add   x1, x1, 0xa79
| :| :::|   0x06d95598      orr   x3, xzr, 4
| :|,=====< 0x06d9559c      b     0x6d955b8
| --------> 0x06d955a0      ldr   x1, [x1, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
| :||:::|   0x06d955a4      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :||:::|   0x06d955a8      blr   x1
| :||:::|   0x06d955ac      ldr   x2, [arg_120h]                       ; [0x120:4]=7
| :||:::|   0x06d955b0      mov   x3, x1
| :||:::|   0x06d955b4      mov   x1, x0
| :||:::|   ; CODE XREF from fcn.06d95528 @ 0x6d9559c
| :|`-----> 0x06d955b8      str   x1, [arg_48h]
| :| :::|   0x06d955bc      str   x3, [arg_38h]
| :| :::|   0x06d955c0      ldr   x1, [x2, 0x48]                       ; [0x48:4]=64
| :| :::|   0x06d955c4      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :| :::|   0x06d955c8      blr   x1
| :| :::|   0x06d955cc      strb  w0, [arg_70h]
| :| :::|   0x06d955d0      stp   x1, x2, [arg_70h + 0x8]
| :| :::|   0x06d955d4      stp   x3, x4, [arg_88h]
| :| :::|   0x06d955d8      strb  w5, [arg_90h + 0x8]
| :| :::|   0x06d955dc      str   x6, [arg_90h + 0x10]
| :| :::|   0x06d955e0      ldp   q0, q1, [arg_70h]
| :| :::|   0x06d955e4      add   x27, arg_88h
| :| :::|   0x06d955e8      ldp   q2, q3, [x27]
| :| :::|   0x06d955ec      stp   q0, q1, [arg_d0h]
| :| :::|   0x06d955f0      add   x27, arg_e8h
| :| :::|   0x06d955f4      stp   q2, q3, [x27]
| :| :::|   0x06d955f8      ldr   x1, [arg_120h]                       ; [0x120:4]=7
| :| :::|   0x06d955fc      ldr   x1, [x1, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
| :| :::|   0x06d95600      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :| :::|   0x06d95604      blr   x1
| :| :::|   0x06d95608      stp   x0, x1, [arg_70h]
| :| :::|   0x06d9560c      stp   x2, x3, [arg_70h + 0x10]
| :| :::|   0x06d95610      str   x4, [arg_90h]
| :| :::|   0x06d95614      ldp   q0, q1, [arg_70h]
| :| :::|   0x06d95618      ldr   x1, [arg_90h]                        ; [0x90:4]=0
| :| :::|   0x06d9561c      add   x27, arg_a8h
| :| :::|   0x06d95620      stp   q0, q1, [x27]
| :| :::|   0x06d95624      str   x1, [arg_c8h]
| :| :::|   0x06d95628      ldr   x1, [arg_120h]                       ; [0x120:4]=7
| :| :::|   0x06d9562c      ldr   x1, [x1, 0x28]                       ; [0x28:4]=0xace39d8
| :| :::|   0x06d95630      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
| :| :::|   0x06d95634      blr   x1
| :| :::|   0x06d95638      str   x0, [arg_68h]
| :| :::|   0x06d9563c      adrp  x0, 0x8be2000
| :| :::|   0x06d95640      add   x0, x0, 0x5f8
| :| :::|   0x06d95644      bl    0x4d31c10
| :| :::|   0x06d95648      adrp  x27, 0xb48f000
| :| :::|   0x06d9564c      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
| :|,=====< 0x06d95650      cbz   w1, 0x6d956ac
| :||:::|   0x06d95654      str   x0, [arg_60h]
| :||:::|   0x06d95658      add   x1, x0, 8
| :||:::|   0x06d9565c      add   x3, x0, 0x40
| :||:::|   0x06d95660      str   x3, [arg_58h]
| :||:::|   0x06d95664      ldp   x3, x4, [x0, 0x68]
| :||:::|   0x06d95668      bl    0x4da3530
| :||:::|   0x06d9566c      ldr   x5, [arg_40h]                        ; segment.PHDR
| :||:::|                                                              ; [0x40:4]=6 ; "\U00000006"
| :||:::|   0x06d95670      ldr   x6, [arg_68h]                        ; [0x68:4]=0x380
| :||:::|   0x06d95674      stp   x5, x6, [x25]
| :||:::|   0x06d95678      ldr   x5, [arg_48h]                        ; [0x48:4]=64
| :||:::|   0x06d9567c      stp   x3, x5, [x25, 0x10]
| :||:::|   0x06d95680      str   x4, [x25, 0x20]
| :||:::|   0x06d95684      adrp  x0, 0x8b68000
| :||:::|   0x06d95688      add   x0, x0, 0x150
| :||:::|   0x06d9568c      add   x2, arg_d0h
| :||:::|   0x06d95690      bl    0x4d35930
| :||:::|   0x06d95694      adrp  x0, 0x89be000
| :||:::|   0x06d95698      add   x0, x0, 0x530
| :||:::|   0x06d9569c      ldr   x1, [arg_58h]                        ; [0x58:4]=64
| :||:::|   0x06d956a0      add   x2, arg_a8h
| :||:::|   0x06d956a4      bl    0x4d35930
| :||:::|   0x06d956a8      ldr   x0, [arg_60h]                        ; [0x60:4]=0x380
| :|`-----> 0x06d956ac      ldr   x3, [arg_40h]                        ; segment.PHDR
| :| :::|                                                              ; [0x40:4]=6 ; "\U00000006"
| :| :::|   0x06d956b0      str   x3, [x0]
| :| :::|   0x06d956b4      ldp   q0, q1, [arg_d0h]
| :| :::|   0x06d956b8      add   x27, arg_e8h
| :| :::|   0x06d956bc      ldp   q2, q3, [x27]
| :| :::|   0x06d956c0      add   x27, x0, 8
| :| :::|   0x06d956c4      stp   q0, q1, [x27]
| :| :::|   0x06d956c8      stp   q2, q3, [x0, 0x20]
| :| :::|   0x06d956cc      add   x27, arg_a8h
| :| :::|   0x06d956d0      ldp   q0, q1, [x27]
| :| :::|   0x06d956d4      ldr   x3, [arg_c8h]                        ; [0xc8:4]=0
| :| :::|   0x06d956d8      stp   q0, q1, [x0, 0x40]
| :| :::|   0x06d956dc      ldr   x4, [arg_68h]                        ; [0x68:4]=0x380
| :| :::|   0x06d956e0      stp   x3, x4, [x0, 0x60]
| :| :::|   0x06d956e4      ldr   x3, [arg_48h]                        ; [0x48:4]=64
| :| :::|   0x06d956e8      ldr   x4, [arg_38h]                        ; [0x38:4]=0x400010
| :| :::|   0x06d956ec      stp   x3, x4, [x0, 0x70]
| :| :::|   0x06d956f0      mov   x1, xzr
| :| :::|   0x06d956f4      mov   x2, xzr
| :| :::|   0x06d956f8      ldp   fp, lr, [sp, -8]
| :| :::|   0x06d956fc      add   sp, sp, 0x110
| :| :::|   0x06d95700      ret
| :| :::|   ; CODE XREF from fcn.06d95528 @ 0x6d95570
| :`.-----> 0x06d95704      and   x7, x6, x5
| : ::::|   0x06d95708      lsl   x7, x7, 4
| : ::::|   0x06d9570c      add   x7, x7, 8
| : ::::|   0x06d95710      ldr   x8, [x4, x7]
| : ::::|   0x06d95714      add   x7, x4, x7
| : ::::|   0x06d95718      cmp   x8, x2
| :,======< 0x06d9571c      b.eq  0x6d95740
| :|::::|   0x06d95720      add   x6, x6, 1
| :|`=====< 0x06d95724      cbnz  x8, 0x6d95704
| :| :::|   0x06d95728      mov   x0, x3
| :| :::|   0x06d9572c      mov   x1, x2
| :| :::|   0x06d95730      bl    0x4d2e4c0
| :| :::|   0x06d95734      ldr   x2, [arg_120h]                       ; [0x120:4]=7
| :| :::|   0x06d95738      mov   x1, x0
| `=======< 0x06d9573c      b     0x6d9558c
|  `------> 0x06d95740      ldr   x3, [x7, 8]                          ; [0x8:4]=0
|    :::|   0x06d95744      mov   x2, x1
|    :::|   0x06d95748      mov   x1, x3
|    `====< 0x06d9574c      b     0x6d9558c
|     ::|   ; CODE XREF from fcn.06d95528 @ 0x6d95530
|    .--`-> 0x06d95750      and   x11, x10, x9
|    :::    0x06d95754      lsl   x11, x11, 4
|    :::    0x06d95758      add   x11, x11, 8
|    :::    0x06d9575c      ldr   x12, [x8, x11]
|    :::    0x06d95760      add   x11, x8, x11
|    :::    0x06d95764      cmp   x12, x6                              ; arg7
|    :::,=< 0x06d95768      b.eq  0x6d9579c
|    :::|   0x06d9576c      add   x10, x10, 1
|    `====< 0x06d95770      cbnz  x12, 0x6d95750
|     ::|   0x06d95774      str   x0, [arg_118h]
|     ::|   0x06d95778      str   x3, [arg_50h]                        ; arg4
|     ::|   0x06d9577c      mov   x0, x7                               ; arg8
|     ::|   0x06d95780      mov   x1, x6                               ; arg7
|     ::|   0x06d95784      bl    0x4d2e4c0
|     ::|   0x06d95788      ldr   x2, [arg_128h]                       ; [0x128:4]=0x779b960
|     ::|   0x06d9578c      ldr   x3, [arg_50h]                        ; [0x50:4]=64
|     ::|   0x06d95790      mov   x1, x0
|     ::|   0x06d95794      ldr   x0, [arg_118h]                       ; [0x118:4]=0x200000
|     `===< 0x06d95798      b     0x6d95534
|      :`-> 0x06d9579c      ldr   x6, [x11, 8]                         ; [0x8:4]=0
|      :    0x06d957a0      mov   x1, x6
\      `==< 0x06d957a4      b     0x6d95534
==== AuthProvider_.GetEntitlement
/ fcn.06d959f8(int64_t arg_58h, int64_t arg_78h, int64_t arg_80h, int64_t arg_d0h, int64_t arg_f8h, int64_t arg_100h, int64_t arg_118h, int64_t arg_128h, int64_t arg_138h, int64_t arg_170h, int64_t arg_188h, int64_t arg_230h);
|           ; arg int64_t arg_58h @ stack + 0x58
|           ; arg int64_t arg_78h @ stack + 0x78
|           ; arg int64_t arg_80h @ stack + 0x80
|           ; arg int64_t arg_d0h @ stack + 0xd0
|           ; arg int64_t arg_f8h @ stack + 0xf8
|           ; arg int64_t arg_100h @ stack + 0x100
|           ; arg int64_t arg_118h @ stack + 0x118
|           ; arg int64_t arg_128h @ stack + 0x128
|           ; arg int64_t arg_138h @ stack + 0x138
|           ; arg int64_t arg_170h @ stack + 0x170
|           ; arg int64_t arg_188h @ stack + 0x188
|           ; arg int64_t arg_230h @ stack + 0x230
|           0x06d959f8      mov   x0, xzr
|           0x06d959fc      mov   x1, xzr
|       ,=< 0x06d95a00      b     0x6d95a2c
..
|       |   ; CODE XREF from fcn.06d959f8 @ 0x6d95a00
|      ,`-> 0x06d95a2c      cbnz  x0, 0x6d95b6c
|      |    0x06d95a30      ldr   x3, [arg_230h]                       ; [0x120:4]=7
|      |    0x06d95a34      ldr   x3, [x3, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|      |    0x06d95a38      ldr   x4, [arg_118h]                       ; [0x118:4]=0x200000
|      |    0x06d95a3c      ldr   x1, [x4, 0x28]                       ; [0x28:4]=0xace39d8
|      |    0x06d95a40      ldr   x0, [arg_128h]                       ; [0x128:4]=0x779b960
|      |    0x06d95a44      ldr   x2, [arg_f8h]                        ; [0xf8:4]=0x799b960 segment.GNU_RELRO
|      |    0x06d95a48      blr   x3
|      |    0x06d95a4c      str   x0, [arg_58h + 0x8]
|      |    0x06d95a50      str   x1, [arg_80h]
|      |    0x06d95a54      adrp  x27, 0x948e000
|      |    0x06d95a58      add   x27, x27, 0x3c0
|      |    0x06d95a5c      ldp   x2, x3, [x27]
|      |    0x06d95a60      bl    0x4daa700
|      |,=< 0x06d95a64      tbnz  w0, 0, 0x6d95b54
|      ||   0x06d95a68      ldr   x0, [arg_170h]                       ; [0x60:4]=0x380
|     ,===< 0x06d95a6c      cbz   x0, 0x6d95a7c
|     |||   0x06d95a70      mov   x1, x0
|     |||   0x06d95a74      ldr   x0, [arg_80h]                        ; [0x80:4]=0
|    ,====< 0x06d95a78      b     0x6d95b40
|    |`---> 0x06d95a7c      stp   xzr, xzr, [arg_d0h]
|    | ||   0x06d95a80      ldp   x0, x1, [arg_138h]
|    | ||   0x06d95a84      bl    0x4d98880
|    | ||   0x06d95a88      adrp  x2, 0x856f000
|    | ||   0x06d95a8c      add   x2, x2, 0x550
|    | ||   0x06d95a90      stp   x2, x0, [arg_d0h]
|    | ||   0x06d95a94      adrp  x0, 0x43e6000
|    | ||   0x06d95a98      add   x0, x0, 0x1bf
|    | ||   0x06d95a9c      orr   x1, xzr, 0x1e
|    | ||   0x06d95aa0      add   x2, arg_d0h
|    | ||   0x06d95aa4      orr   x3, xzr, 1
|    | ||   0x06d95aa8      mov   x4, x3
|    | ||   0x06d95aac      bl    0x4e321f0
|    |,===< 0x06d95ab0      cbnz  x0, 0x6d95ae8
|    ||||   0x06d95ab4      nop
|    ||||   0x06d95ab8      orr   x0, xzr, 0x10
|    ||||   0x06d95abc      adrp  x1, 0x8757000
|    ||||   0x06d95ac0      add   x1, x1, 0x890
|    ||||   0x06d95ac4      orr   x2, xzr, 1
|    ||||   0x06d95ac8      bl    0x4d32500
|    ||||   0x06d95acc      adrp  x3, 0x43e6000
|    ||||   0x06d95ad0      add   x3, x3, 0x1bf
|    ||||   0x06d95ad4      orr   x4, xzr, 0x1e
|    ||||   0x06d95ad8      stp   x3, x4, [x0]
|    ||||   0x06d95adc      mov   x1, x0
|    ||||   0x06d95ae0      adrp  x0, 0x8f07000
|    ||||   0x06d95ae4      add   x0, x0, 0x508
|    |`---> 0x06d95ae8      str   x0, [arg_58h]
|    | ||   0x06d95aec      str   x1, [arg_78h]
|    | ||   0x06d95af0      orr   x0, xzr, 0x40
|    | ||   0x06d95af4      adrp  x1, 0x8b68000
|    | ||   0x06d95af8      add   x1, x1, 0x78
|    | ||   0x06d95afc      orr   x2, xzr, 1
|    | ||   0x06d95b00      bl    0x4d33310
|    | ||   0x06d95b04      orr   x3, xzr, 0xc
|    | ||   0x06d95b08      str   w3, [x0]
|    | ||   0x06d95b0c      ldr   x3, [arg_58h]                        ; [0x58:4]=64
|    | ||   0x06d95b10      str   x3, [x0, 8]
|    | ||   0x06d95b14      adrp  x27, 0xb48f000
|    | ||   0x06d95b18      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    |,===< 0x06d95b1c      cbnz  w3, 0x6d95b28
|    ||||   0x06d95b20      ldr   x3, [arg_188h]                       ; [0x78:4]=1
|   ,=====< 0x06d95b24      b     0x6d95b34
|   ||`---> 0x06d95b28      bl    0x4da34f0
|   || ||   0x06d95b2c      ldr   x3, [arg_78h]                        ; [0x78:4]=1
|   || ||   0x06d95b30      str   x3, [x25]
|   || ||   ; CODE XREF from fcn.06d959f8 @ 0x6d95b24
|   `-----> 0x06d95b34      str   x3, [x0, 0x10]
|    | ||   0x06d95b38      adrp  x1, 0x8f07000
|    | ||   0x06d95b3c      add   x1, x1, 0x588
|    | ||   ; CODE XREF from fcn.06d959f8 @ 0x6d95a78
|    `----> 0x06d95b40      mov   x2, x0
|      ||   0x06d95b44      mov   x0, xzr
|      ||   0x06d95b48      ldp   fp, lr, [sp, -8]
|      ||   0x06d95b4c      add   sp, sp, 0x110
|      ||   0x06d95b50      ret
|      |`-> 0x06d95b54      ldr   x0, [arg_100h]                       ; [0x100:4]=0x799b960 segment.GNU_RELRO
|      |    0x06d95b58      mov   x1, xzr
|      |    0x06d95b5c      mov   x2, xzr
|      |    0x06d95b60      ldp   fp, lr, [sp, -8]
|      |    0x06d95b64      add   sp, sp, 0x110
|      |    0x06d95b68      ret
|      `--> 0x06d95b6c      mov   x2, x1
|           0x06d95b70      mov   x1, x0
|           0x06d95b74      mov   x0, xzr
|           0x06d95b78      ldp   fp, lr, [sp, -8]
|           0x06d95b7c      add   sp, sp, 0x110
\           0x06d95b80      ret
==== google3_third_party_jetski_gateway_gateway.LoadCACertPool
/ fcn.06d95a08(int64_t arg1, int64_t arg2, int64_t arg_98h, int64_t arg_c0h, int64_t arg_c8h, int64_t arg_f8h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg2 @ x1
|           ; arg int64_t arg_98h @ stack + 0x98
|           ; arg int64_t arg_c0h @ stack + 0xc0
|           ; arg int64_t arg_c8h @ stack + 0xc8
|           ; arg int64_t arg_f8h @ stack + 0xf8
|           0x06d95a08      ldr   x7, [x9, 0x20]                       ; [0x20:4]=64 ; "@"
|           0x06d95a0c      ldrb  w9, [arg_98h]                        ; [0x98:4]=0x779b960
|           0x06d95a10      ldr   x10, [x0]                            ; arg1
|           0x06d95a14      ldrb  w5, [arg_c0h]                        ; [0xc0:4]=0
|           0x06d95a18      ldr   x6, [arg_c8h]                        ; [0xc8:4]=0
|           0x06d95a1c      ldr   x8, [arg_f8h]                        ; [0xf8:4]=0x799b960 segment.GNU_RELRO
|           0x06d95a20      mov   x26, x0                              ; arg1
|           0x06d95a24      mov   x0, x9
\           0x06d95a28      blr   x10
==== google3_third_party_jetski_gateway_gateway.LoadProxyURL
/ fcn.06d95c68(int64_t arg_10h, int64_t arg_18h, int64_t arg_28h);
|           ; arg int64_t arg_10h @ stack + 0x10
|           ; arg int64_t arg_18h @ stack + 0x18
|           ; arg int64_t arg_28h @ stack + 0x28
|           0x06d95c68      adrp  x27, 0xb48f000
|           0x06d95c6c      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d95c70      cbnz  w1, 0x6d95c7c
|       |   0x06d95c74      ldr   x1, [arg_28h]                        ; [0x18:4]=0x4d0f000 section..text
|      ,==< 0x06d95c78      b     0x6d95c8c
|      |`-> 0x06d95c7c      ldr   x1, [arg_18h]                        ; [0x18:4]=0x4d0f000 section..text
|      |    0x06d95c80      ldr   x2, [x1, 8]                          ; [0x8:4]=0
|      |    0x06d95c84      bl    fcn.04da3500
|      |    0x06d95c88      stp   x0, x2, [x25]
|      |    ; CODE XREF from fcn.06d95c68 @ 0x6d95c78
|      `--> 0x06d95c8c      str   x0, [x1, 8]
|           0x06d95c90      mov   x0, x1
|           0x06d95c94      ldr   x0, [x0, 8]                          ; [0x8:4]=0
|           0x06d95c98      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d95c9c      ldr   lr, [sp], 0x10                       ; [0x10:4]=0xb70003
\           0x06d95ca0      ret
==== google3_third_party_jetski_gateway_gateway.NewHTTPTransportWithOptions
/ fcn.06d95ec8(int64_t arg_8h);
|       :   ; var int64_t var_78h @ stack - 0x78
|       :   ; var int64_t var_70h @ stack - 0x70
|       :   ; var int64_t var_60h @ stack - 0x60
|       :   ; var int64_t var_50h @ stack - 0x50
|       :   ; var int64_t var_48h @ stack - 0x48
|       :   ; var int64_t var_40h @ stack - 0x40
|       :   ; var int64_t var_28h @ stack - 0x28
|       :   ; var int64_t var_8h @ stack - 0x8
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   0x06d95ec8      bl    fcn.04da15a0
|       :   0x06d95ecc      ldr   x0, [arg_8h]                         ; [0x8:4]=0
|       `=< 0x06d95ed0      b     0x6d95e40
==== google3_third_party_jetski_gateway_gateway.ConfigureDefaultTransport
/ fcn.06d96128(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg4, int64_t arg5, int64_t arg_38h, int64_t arg_48h, int64_t arg_60h, int64_t arg_68h, int64_t arg_70h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg2 @ x1
|           ; arg int64_t arg3 @ x2
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg5 @ x4
|           ; arg int64_t arg_38h @ stack + 0x38
|           ; arg int64_t arg_48h @ stack + 0x48
|           ; arg int64_t arg_60h @ stack + 0x60
|           ; arg int64_t arg_68h @ stack + 0x68
|           ; arg int64_t arg_70h @ stack + 0x70
|           0x06d96128      str   x0, [arg_68h]                        ; arg1
|           0x06d9612c      strb  w1, [arg_70h]                        ; arg2
|           0x06d96130      str   x2, [arg_70h + 0x8]                  ; arg3
|           0x06d96134      str   d0, [arg_70h + 0x10]
|           0x06d96138      stp   x3, x4, [arg_70h + 0x18]             ; arg5
|           0x06d9613c      ldr   x0, [arg_68h]                        ; [0x68:4]=0x380
|           0x06d96140      cmp   x0, 1
|       ,=< 0x06d96144      b.gt  0x6d961d4
|      ,==< 0x06d96148      cbz   x0, 0x6d96198
|     ,===< 0x06d9614c      b.ne  0x6d961ec
|     |||   0x06d96150      ldrb  w5, [arg_70h]                        ; [0x70:4]=8
|     |||   0x06d96154      adrp  x6, 0x4a82000
|     |||   0x06d96158      add   x6, x6, 0xb20
|     |||   0x06d9615c      add   x1, x6, x5, lsl 3
|     |||   0x06d96160      adrp  x5, 0x8f08000
|     |||   0x06d96164      add   x5, x5, 0x88
|     |||   0x06d96168      adrp  x6, 0x94f7000
|     |||   0x06d9616c      add   x6, x6, 0xfe0
|     |||   0x06d96170      stp   x5, x6, [arg_48h]
|     |||   0x06d96174      adrp  x0, 0x856f000
|     |||   0x06d96178      add   x0, x0, 0x950
|     |||   0x06d9617c      add   x2, arg_48h
|     |||   0x06d96180      orr   x3, xzr, 1
|     |||   0x06d96184      mov   x4, x3
|     |||   0x06d96188      bl    fcn.04f32280
|     |||   0x06d9618c      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|     |||   0x06d96190      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|     |||   0x06d96194      ret
|     |`--> 0x06d96198      orr   x0, xzr, 4
|     | |   0x06d9619c      adrp  x1, 0x81ab000
|     | |   0x06d961a0      add   x1, x1, 0x6d8
|     | |   0x06d961a4      orr   x2, xzr, 1
|     | |   0x06d961a8      bl    fcn.04d33a20
|     | |   0x06d961ac      mov   x3, 0x756e                           ; 'nu'
|     | |   0x06d961b0      movk  x3, 0x6c6c, lsl 16
|     | |   0x06d961b4      str   w3, [x0]
|     | |   0x06d961b8      orr   x1, xzr, 4
|     | |   0x06d961bc      mov   x2, x1
|     | |   0x06d961c0      mov   x3, xzr
|     | |   0x06d961c4      mov   x4, xzr
|     | |   0x06d961c8      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|     | |   0x06d961cc      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|     | |   0x06d961d0      ret
|     | `-> 0x06d961d4      cmp   x0, 2
|     | ,=< 0x06d961d8      b.eq  0x6d96300
|     | |   0x06d961dc      cmp   x0, 3
|     |,==< 0x06d961e0      b.eq  0x6d962bc
|     |||   0x06d961e4      cmp   x0, 4
|    ,====< 0x06d961e8      b.eq  0x6d96274
|    |`---> 0x06d961ec      stp   xzr, xzr, [arg_38h]
|    | ||   0x06d961f0      bl    fcn.04d98810
|    | ||   0x06d961f4      adrp  x1, 0x8571000
|    | ||   0x06d961f8      add   x1, x1, 0xd0
|    | ||   0x06d961fc      stp   x1, x0, [arg_38h]
|    | ||   0x06d96200      adrp  x0, 0x4358000
|    | ||   0x06d96204      add   x0, x0, 0xd10
|    | ||   0x06d96208      mov   x1, 0x13
|    | ||   0x06d9620c      add   x2, arg_38h
|    | ||   0x06d96210      orr   x3, xzr, 1
|    | ||   0x06d96214      mov   x4, x3
|    | ||   0x06d96218      bl    fcn.04e321f0
|    |,===< 0x06d9621c      cbnz  x0, 0x6d96254
|    ||||   0x06d96220      nop
|    ||||   0x06d96224      orr   x0, xzr, 0x10
|    ||||   0x06d96228      adrp  x1, 0x8757000
|    ||||   0x06d9622c      add   x1, x1, 0x890
|    ||||   0x06d96230      orr   x2, xzr, 1
|    ||||   0x06d96234      bl    fcn.04d32500
|    ||||   0x06d96238      adrp  x3, 0x4358000
|    ||||   0x06d9623c      add   x3, x3, 0xd10
|    ||||   0x06d96240      mov   x4, 0x13
|    ||||   0x06d96244      stp   x3, x4, [x0]
|    ||||   0x06d96248      mov   x1, x0
|    ||||   0x06d9624c      adrp  x0, 0x8f07000
|    ||||   0x06d96250      add   x0, x0, 0x508
|    |`---> 0x06d96254      mov   x2, xzr
|    | ||   0x06d96258      mov   x3, x0
|    | ||   0x06d9625c      mov   x4, x1
|    | ||   0x06d96260      mov   x0, xzr
|    | ||   0x06d96264      mov   x1, xzr
|    | ||   0x06d96268      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    | ||   0x06d9626c      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|    | ||   0x06d96270      ret
|    `----> 0x06d96274      mov   x0, x3                               ; arg4
|      ||   0x06d96278      mov   x1, x4                               ; arg5
|      ||   0x06d9627c      bl    fcn.04d98880
|      ||   0x06d96280      adrp  x2, 0x8f08000
|      ||   0x06d96284      add   x2, x2, 0x88
|      ||   0x06d96288      adrp  x3, 0x94f7000
|      ||   0x06d9628c      add   x3, x3, 0xfe0
|      ||   0x06d96290      stp   x2, x3, [arg_48h]
|      ||   0x06d96294      mov   x1, x0
|      ||   0x06d96298      add   x2, arg_48h
|      ||   0x06d9629c      orr   x3, xzr, 1
|      ||   0x06d962a0      mov   x4, x3
|      ||   0x06d962a4      adrp  x0, 0x856f000
|      ||   0x06d962a8      add   x0, x0, 0x550
|      ||   0x06d962ac      bl    fcn.04f32280
|      ||   0x06d962b0      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      ||   0x06d962b4      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|      ||   0x06d962b8      ret
|      `--> 0x06d962bc      fmov  x0, d0
|       |   0x06d962c0      bl    fcn.04d98810
|       |   0x06d962c4      adrp  x1, 0x8f08000
|       |   0x06d962c8      add   x1, x1, 0x88
|       |   0x06d962cc      adrp  x2, 0x94f7000
|       |   0x06d962d0      add   x2, x2, 0xfe0
|       |   0x06d962d4      stp   x1, x2, [arg_48h]
|       |   0x06d962d8      mov   x1, x0
|       |   0x06d962dc      add   x2, arg_48h
|       |   0x06d962e0      orr   x3, xzr, 1
|       |   0x06d962e4      mov   x4, x3
|       |   0x06d962e8      adrp  x0, 0x833c000
|       |   0x06d962ec      add   x0, x0, 0xfc8
|       |   0x06d962f0      bl    fcn.04f32280
|       |   0x06d962f4      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|       |   0x06d962f8      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|       |   0x06d962fc      ret
|       `-> 0x06d96300      mov   x0, x2                               ; arg3
|           0x06d96304      mov   x1, 0xa
|           0x06d96308      bl    fcn.04d1efa0
|           0x06d9630c      mov   x2, x1
|           0x06d96310      mov   x1, x0
|           0x06d96314      mov   x0, xzr
|           0x06d96318      bl    fcn.04d7d7c0
|           0x06d9631c      mov   x3, xzr
|           0x06d96320      mov   x4, xzr
|           0x06d96324      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d96328      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
\           0x06d9632c      ret
==== google3_third_party_jetski_gateway_gateway.redactURL
/ fcn.06d96128(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg4, int64_t arg5, int64_t arg_38h, int64_t arg_48h, int64_t arg_60h, int64_t arg_68h, int64_t arg_70h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg2 @ x1
|           ; arg int64_t arg3 @ x2
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg5 @ x4
|           ; arg int64_t arg_38h @ stack + 0x38
|           ; arg int64_t arg_48h @ stack + 0x48
|           ; arg int64_t arg_60h @ stack + 0x60
|           ; arg int64_t arg_68h @ stack + 0x68
|           ; arg int64_t arg_70h @ stack + 0x70
|           0x06d96128      str   x0, [arg_68h]                        ; arg1
|           0x06d9612c      strb  w1, [arg_70h]                        ; arg2
|           0x06d96130      str   x2, [arg_70h + 0x8]                  ; arg3
|           0x06d96134      str   d0, [arg_70h + 0x10]
|           0x06d96138      stp   x3, x4, [arg_70h + 0x18]             ; arg5
|           0x06d9613c      ldr   x0, [arg_68h]                        ; [0x68:4]=0x380
|           0x06d96140      cmp   x0, 1
|       ,=< 0x06d96144      b.gt  0x6d961d4
|      ,==< 0x06d96148      cbz   x0, 0x6d96198
|     ,===< 0x06d9614c      b.ne  0x6d961ec
|     |||   0x06d96150      ldrb  w5, [arg_70h]                        ; [0x70:4]=8
|     |||   0x06d96154      adrp  x6, 0x4a82000
|     |||   0x06d96158      add   x6, x6, 0xb20
|     |||   0x06d9615c      add   x1, x6, x5, lsl 3
|     |||   0x06d96160      adrp  x5, 0x8f08000
|     |||   0x06d96164      add   x5, x5, 0x88
|     |||   0x06d96168      adrp  x6, 0x94f7000
|     |||   0x06d9616c      add   x6, x6, 0xfe0
|     |||   0x06d96170      stp   x5, x6, [arg_48h]
|     |||   0x06d96174      adrp  x0, 0x856f000
|     |||   0x06d96178      add   x0, x0, 0x950
|     |||   0x06d9617c      add   x2, arg_48h
|     |||   0x06d96180      orr   x3, xzr, 1
|     |||   0x06d96184      mov   x4, x3
|     |||   0x06d96188      bl    fcn.04f32280
|     |||   0x06d9618c      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|     |||   0x06d96190      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|     |||   0x06d96194      ret
|     |`--> 0x06d96198      orr   x0, xzr, 4
|     | |   0x06d9619c      adrp  x1, 0x81ab000
|     | |   0x06d961a0      add   x1, x1, 0x6d8
|     | |   0x06d961a4      orr   x2, xzr, 1
|     | |   0x06d961a8      bl    fcn.04d33a20
|     | |   0x06d961ac      mov   x3, 0x756e                           ; 'nu'
|     | |   0x06d961b0      movk  x3, 0x6c6c, lsl 16
|     | |   0x06d961b4      str   w3, [x0]
|     | |   0x06d961b8      orr   x1, xzr, 4
|     | |   0x06d961bc      mov   x2, x1
|     | |   0x06d961c0      mov   x3, xzr
|     | |   0x06d961c4      mov   x4, xzr
|     | |   0x06d961c8      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|     | |   0x06d961cc      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|     | |   0x06d961d0      ret
|     | `-> 0x06d961d4      cmp   x0, 2
|     | ,=< 0x06d961d8      b.eq  0x6d96300
|     | |   0x06d961dc      cmp   x0, 3
|     |,==< 0x06d961e0      b.eq  0x6d962bc
|     |||   0x06d961e4      cmp   x0, 4
|    ,====< 0x06d961e8      b.eq  0x6d96274
|    |`---> 0x06d961ec      stp   xzr, xzr, [arg_38h]
|    | ||   0x06d961f0      bl    fcn.04d98810
|    | ||   0x06d961f4      adrp  x1, 0x8571000
|    | ||   0x06d961f8      add   x1, x1, 0xd0
|    | ||   0x06d961fc      stp   x1, x0, [arg_38h]
|    | ||   0x06d96200      adrp  x0, 0x4358000
|    | ||   0x06d96204      add   x0, x0, 0xd10
|    | ||   0x06d96208      mov   x1, 0x13
|    | ||   0x06d9620c      add   x2, arg_38h
|    | ||   0x06d96210      orr   x3, xzr, 1
|    | ||   0x06d96214      mov   x4, x3
|    | ||   0x06d96218      bl    fcn.04e321f0
|    |,===< 0x06d9621c      cbnz  x0, 0x6d96254
|    ||||   0x06d96220      nop
|    ||||   0x06d96224      orr   x0, xzr, 0x10
|    ||||   0x06d96228      adrp  x1, 0x8757000
|    ||||   0x06d9622c      add   x1, x1, 0x890
|    ||||   0x06d96230      orr   x2, xzr, 1
|    ||||   0x06d96234      bl    fcn.04d32500
|    ||||   0x06d96238      adrp  x3, 0x4358000
|    ||||   0x06d9623c      add   x3, x3, 0xd10
|    ||||   0x06d96240      mov   x4, 0x13
|    ||||   0x06d96244      stp   x3, x4, [x0]
|    ||||   0x06d96248      mov   x1, x0
|    ||||   0x06d9624c      adrp  x0, 0x8f07000
|    ||||   0x06d96250      add   x0, x0, 0x508
|    |`---> 0x06d96254      mov   x2, xzr
|    | ||   0x06d96258      mov   x3, x0
|    | ||   0x06d9625c      mov   x4, x1
|    | ||   0x06d96260      mov   x0, xzr
|    | ||   0x06d96264      mov   x1, xzr
|    | ||   0x06d96268      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    | ||   0x06d9626c      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|    | ||   0x06d96270      ret
|    `----> 0x06d96274      mov   x0, x3                               ; arg4
|      ||   0x06d96278      mov   x1, x4                               ; arg5
|      ||   0x06d9627c      bl    fcn.04d98880
|      ||   0x06d96280      adrp  x2, 0x8f08000
|      ||   0x06d96284      add   x2, x2, 0x88
|      ||   0x06d96288      adrp  x3, 0x94f7000
|      ||   0x06d9628c      add   x3, x3, 0xfe0
|      ||   0x06d96290      stp   x2, x3, [arg_48h]
|      ||   0x06d96294      mov   x1, x0
|      ||   0x06d96298      add   x2, arg_48h
|      ||   0x06d9629c      orr   x3, xzr, 1
|      ||   0x06d962a0      mov   x4, x3
|      ||   0x06d962a4      adrp  x0, 0x856f000
|      ||   0x06d962a8      add   x0, x0, 0x550
|      ||   0x06d962ac      bl    fcn.04f32280
|      ||   0x06d962b0      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      ||   0x06d962b4      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|      ||   0x06d962b8      ret
|      `--> 0x06d962bc      fmov  x0, d0
|       |   0x06d962c0      bl    fcn.04d98810
|       |   0x06d962c4      adrp  x1, 0x8f08000
|       |   0x06d962c8      add   x1, x1, 0x88
|       |   0x06d962cc      adrp  x2, 0x94f7000
|       |   0x06d962d0      add   x2, x2, 0xfe0
|       |   0x06d962d4      stp   x1, x2, [arg_48h]
|       |   0x06d962d8      mov   x1, x0
|       |   0x06d962dc      add   x2, arg_48h
|       |   0x06d962e0      orr   x3, xzr, 1
|       |   0x06d962e4      mov   x4, x3
|       |   0x06d962e8      adrp  x0, 0x833c000
|       |   0x06d962ec      add   x0, x0, 0xfc8
|       |   0x06d962f0      bl    fcn.04f32280
|       |   0x06d962f4      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|       |   0x06d962f8      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|       |   0x06d962fc      ret
|       `-> 0x06d96300      mov   x0, x2                               ; arg3
|           0x06d96304      mov   x1, 0xa
|           0x06d96308      bl    fcn.04d1efa0
|           0x06d9630c      mov   x2, x1
|           0x06d96310      mov   x1, x0
|           0x06d96314      mov   x0, xzr
|           0x06d96318      bl    fcn.04d7d7c0
|           0x06d9631c      mov   x3, xzr
|           0x06d96320      mov   x4, xzr
|           0x06d96324      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d96328      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
\           0x06d9632c      ret
==== AuthProvider_.GetModelCatalog
/ fcn.06d96128(int64_t arg1, int64_t arg2, int64_t arg3, int64_t arg4, int64_t arg5, int64_t arg_38h, int64_t arg_48h, int64_t arg_60h, int64_t arg_68h, int64_t arg_70h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg2 @ x1
|           ; arg int64_t arg3 @ x2
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg5 @ x4
|           ; arg int64_t arg_38h @ stack + 0x38
|           ; arg int64_t arg_48h @ stack + 0x48
|           ; arg int64_t arg_60h @ stack + 0x60
|           ; arg int64_t arg_68h @ stack + 0x68
|           ; arg int64_t arg_70h @ stack + 0x70
|           0x06d96128      str   x0, [arg_68h]                        ; arg1
|           0x06d9612c      strb  w1, [arg_70h]                        ; arg2
|           0x06d96130      str   x2, [arg_70h + 0x8]                  ; arg3
|           0x06d96134      str   d0, [arg_70h + 0x10]
|           0x06d96138      stp   x3, x4, [arg_70h + 0x18]             ; arg5
|           0x06d9613c      ldr   x0, [arg_68h]                        ; [0x68:4]=0x380
|           0x06d96140      cmp   x0, 1
|       ,=< 0x06d96144      b.gt  0x6d961d4
|      ,==< 0x06d96148      cbz   x0, 0x6d96198
|     ,===< 0x06d9614c      b.ne  0x6d961ec
|     |||   0x06d96150      ldrb  w5, [arg_70h]                        ; [0x70:4]=8
|     |||   0x06d96154      adrp  x6, 0x4a82000
|     |||   0x06d96158      add   x6, x6, 0xb20
|     |||   0x06d9615c      add   x1, x6, x5, lsl 3
|     |||   0x06d96160      adrp  x5, 0x8f08000
|     |||   0x06d96164      add   x5, x5, 0x88
|     |||   0x06d96168      adrp  x6, 0x94f7000
|     |||   0x06d9616c      add   x6, x6, 0xfe0
|     |||   0x06d96170      stp   x5, x6, [arg_48h]
|     |||   0x06d96174      adrp  x0, 0x856f000
|     |||   0x06d96178      add   x0, x0, 0x950
|     |||   0x06d9617c      add   x2, arg_48h
|     |||   0x06d96180      orr   x3, xzr, 1
|     |||   0x06d96184      mov   x4, x3
|     |||   0x06d96188      bl    fcn.04f32280
|     |||   0x06d9618c      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|     |||   0x06d96190      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|     |||   0x06d96194      ret
|     |`--> 0x06d96198      orr   x0, xzr, 4
|     | |   0x06d9619c      adrp  x1, 0x81ab000
|     | |   0x06d961a0      add   x1, x1, 0x6d8
|     | |   0x06d961a4      orr   x2, xzr, 1
|     | |   0x06d961a8      bl    fcn.04d33a20
|     | |   0x06d961ac      mov   x3, 0x756e                           ; 'nu'
|     | |   0x06d961b0      movk  x3, 0x6c6c, lsl 16
|     | |   0x06d961b4      str   w3, [x0]
|     | |   0x06d961b8      orr   x1, xzr, 4
|     | |   0x06d961bc      mov   x2, x1
|     | |   0x06d961c0      mov   x3, xzr
|     | |   0x06d961c4      mov   x4, xzr
|     | |   0x06d961c8      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|     | |   0x06d961cc      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|     | |   0x06d961d0      ret
|     | `-> 0x06d961d4      cmp   x0, 2
|     | ,=< 0x06d961d8      b.eq  0x6d96300
|     | |   0x06d961dc      cmp   x0, 3
|     |,==< 0x06d961e0      b.eq  0x6d962bc
|     |||   0x06d961e4      cmp   x0, 4
|    ,====< 0x06d961e8      b.eq  0x6d96274
|    |`---> 0x06d961ec      stp   xzr, xzr, [arg_38h]
|    | ||   0x06d961f0      bl    fcn.04d98810
|    | ||   0x06d961f4      adrp  x1, 0x8571000
|    | ||   0x06d961f8      add   x1, x1, 0xd0
|    | ||   0x06d961fc      stp   x1, x0, [arg_38h]
|    | ||   0x06d96200      adrp  x0, 0x4358000
|    | ||   0x06d96204      add   x0, x0, 0xd10
|    | ||   0x06d96208      mov   x1, 0x13
|    | ||   0x06d9620c      add   x2, arg_38h
|    | ||   0x06d96210      orr   x3, xzr, 1
|    | ||   0x06d96214      mov   x4, x3
|    | ||   0x06d96218      bl    fcn.04e321f0
|    |,===< 0x06d9621c      cbnz  x0, 0x6d96254
|    ||||   0x06d96220      nop
|    ||||   0x06d96224      orr   x0, xzr, 0x10
|    ||||   0x06d96228      adrp  x1, 0x8757000
|    ||||   0x06d9622c      add   x1, x1, 0x890
|    ||||   0x06d96230      orr   x2, xzr, 1
|    ||||   0x06d96234      bl    fcn.04d32500
|    ||||   0x06d96238      adrp  x3, 0x4358000
|    ||||   0x06d9623c      add   x3, x3, 0xd10
|    ||||   0x06d96240      mov   x4, 0x13
|    ||||   0x06d96244      stp   x3, x4, [x0]
|    ||||   0x06d96248      mov   x1, x0
|    ||||   0x06d9624c      adrp  x0, 0x8f07000
|    ||||   0x06d96250      add   x0, x0, 0x508
|    |`---> 0x06d96254      mov   x2, xzr
|    | ||   0x06d96258      mov   x3, x0
|    | ||   0x06d9625c      mov   x4, x1
|    | ||   0x06d96260      mov   x0, xzr
|    | ||   0x06d96264      mov   x1, xzr
|    | ||   0x06d96268      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|    | ||   0x06d9626c      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|    | ||   0x06d96270      ret
|    `----> 0x06d96274      mov   x0, x3                               ; arg4
|      ||   0x06d96278      mov   x1, x4                               ; arg5
|      ||   0x06d9627c      bl    fcn.04d98880
|      ||   0x06d96280      adrp  x2, 0x8f08000
|      ||   0x06d96284      add   x2, x2, 0x88
|      ||   0x06d96288      adrp  x3, 0x94f7000
|      ||   0x06d9628c      add   x3, x3, 0xfe0
|      ||   0x06d96290      stp   x2, x3, [arg_48h]
|      ||   0x06d96294      mov   x1, x0
|      ||   0x06d96298      add   x2, arg_48h
|      ||   0x06d9629c      orr   x3, xzr, 1
|      ||   0x06d962a0      mov   x4, x3
|      ||   0x06d962a4      adrp  x0, 0x856f000
|      ||   0x06d962a8      add   x0, x0, 0x550
|      ||   0x06d962ac      bl    fcn.04f32280
|      ||   0x06d962b0      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      ||   0x06d962b4      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|      ||   0x06d962b8      ret
|      `--> 0x06d962bc      fmov  x0, d0
|       |   0x06d962c0      bl    fcn.04d98810
|       |   0x06d962c4      adrp  x1, 0x8f08000
|       |   0x06d962c8      add   x1, x1, 0x88
|       |   0x06d962cc      adrp  x2, 0x94f7000
|       |   0x06d962d0      add   x2, x2, 0xfe0
|       |   0x06d962d4      stp   x1, x2, [arg_48h]
|       |   0x06d962d8      mov   x1, x0
|       |   0x06d962dc      add   x2, arg_48h
|       |   0x06d962e0      orr   x3, xzr, 1
|       |   0x06d962e4      mov   x4, x3
|       |   0x06d962e8      adrp  x0, 0x833c000
|       |   0x06d962ec      add   x0, x0, 0xfc8
|       |   0x06d962f0      bl    fcn.04f32280
|       |   0x06d962f4      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|       |   0x06d962f8      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|       |   0x06d962fc      ret
|       `-> 0x06d96300      mov   x0, x2                               ; arg3
|           0x06d96304      mov   x1, 0xa
|           0x06d96308      bl    fcn.04d1efa0
|           0x06d9630c      mov   x2, x1
|           0x06d96310      mov   x1, x0
|           0x06d96314      mov   x0, xzr
|           0x06d96318      bl    fcn.04d7d7c0
|           0x06d9631c      mov   x3, xzr
|           0x06d96320      mov   x4, xzr
|           0x06d96324      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d96328      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
\           0x06d9632c      ret
==== AuthProvider_.AdminFileSetsModels
/ fcn.06d969a8(int64_t arg3, int64_t arg_68h, int64_t arg_78h, int64_t arg_80h, int64_t arg_88h, int64_t arg_98h, int64_t arg_a0h, int64_t arg_a8h, int64_t arg_b0h, int64_t arg_c8h, int64_t arg_d8h, int64_t arg_e0h, int64_t arg_e8h, int64_t arg_f0h, int64_t arg_f8h, int64_t arg_100h, int64_t arg_108h, int64_t arg_1c8h, int64_t arg_298h);
|      ::   ; arg int64_t arg3 @ x2
|      ::   ; arg int64_t arg_68h @ stack + 0x68
|      ::   ; arg int64_t arg_78h @ stack + 0x78
|      ::   ; arg int64_t arg_80h @ stack + 0x80
|      ::   ; arg int64_t arg_88h @ stack + 0x88
|      ::   ; arg int64_t arg_98h @ stack + 0x98
|      ::   ; arg int64_t arg_a0h @ stack + 0xa0
|      ::   ; arg int64_t arg_a8h @ stack + 0xa8
|      ::   ; arg int64_t arg_b0h @ stack + 0xb0
|      ::   ; arg int64_t arg_c8h @ stack + 0xc8
|      ::   ; arg int64_t arg_d8h @ stack + 0xd8
|      ::   ; arg int64_t arg_e0h @ stack + 0xe0
|      ::   ; arg int64_t arg_e8h @ stack + 0xe8
|      ::   ; arg int64_t arg_f0h @ stack + 0xf0
|      ::   ; arg int64_t arg_f8h @ stack + 0xf8
|      ::   ; arg int64_t arg_100h @ stack + 0x100
|      ::   ; arg int64_t arg_108h @ stack + 0x108
|      ::   ; arg int64_t arg_1c8h @ stack + 0x1c8
|      ::   ; arg int64_t arg_298h @ stack + 0x298
|     ,===< 0x06d969a8      b.ne  0x6d96aac
|     |::   0x06d969ac      orr   x0, xzr, 8
|     |::   0x06d969b0      adrp  x1, 0x875e000
|     |::   0x06d969b4      add   x1, x1, 0xf28
|     |::   0x06d969b8      orr   x2, xzr, 1
|     |::   0x06d969bc      bl    fcn.04d33a20
|     |::   0x06d969c0      str   x0, [arg_100h]
|     |::   0x06d969c4      ldr   x3, [arg_a0h]                        ; [0xa0:4]=0x779b960
|     |::   0x06d969c8      str   x3, [x0]
|     |::   0x06d969cc      mov   x0, 0x50                             ; 'P'
|     |::   0x06d969d0      adrp  x1, 0x8c55000
|     |::   0x06d969d4      add   x1, x1, 0xa68
|     |::   0x06d969d8      orr   x2, xzr, 1
|     |::   0x06d969dc      bl    0x4d33650
|     |::   0x06d969e0      ldr   x3, [arg_68h]                        ; [0x68:4]=0x380
|     |::   0x06d969e4      str   x3, [x0, 0x10]
|     |::   0x06d969e8      adrp  x27, 0xb48f000
|     |::   0x06d969ec      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d969f0      cbnz  w3, 0x6d96a00
|    ||::   0x06d969f4      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
|    ||::   0x06d969f8      ldr   x6, [arg_100h]                       ; [0x100:4]=0x799b960 segment.GNU_RELRO
|   ,=====< 0x06d969fc      b     0x6d96a10
|   |`----> 0x06d96a00      bl    fcn.04da3500
|   | |::   0x06d96a04      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
|   | |::   0x06d96a08      ldr   x6, [arg_100h]                       ; [0x100:4]=0x799b960 segment.GNU_RELRO
|   | |::   0x06d96a0c      stp   x5, x6, [x25]
|   | |::   ; CODE XREF from fcn.06d969a8 @ 0x6d969fc
|   `-----> 0x06d96a10      str   x5, [x0, 8]
|     |::   0x06d96a14      adrp  x5, 0x8f08000
|     |::   0x06d96a18      add   x5, x5, 0xcc8
|     |::   0x06d96a1c      stp   x5, x6, [x0, 0x18]
|    ,====< 0x06d96a20      b     0x6d96bb8
|    ||::   0x06d96a24      orr   x0, xzr, 1
|    ||::   0x06d96a28      adrp  x1, 0x875f000
|    ||::   0x06d96a2c      add   x1, x1, 0x18
|    ||::   0x06d96a30      orr   x2, xzr, 1
|    ||::   0x06d96a34      bl    fcn.04d33a20
|    ||::   0x06d96a38      str   x0, [arg_f8h]
|    ||::   0x06d96a3c      ldrb  w3, [arg_98h]                        ; [0x98:4]=0x779b960
|    ||::   0x06d96a40      strb  w3, [x0]
|    ||::   0x06d96a44      mov   x0, 0x50                             ; 'P'
|    ||::   0x06d96a48      adrp  x1, 0x8c55000
|    ||::   0x06d96a4c      add   x1, x1, 0xa68
|    ||::   0x06d96a50      orr   x2, xzr, 1
|    ||::   0x06d96a54      bl    0x4d33650
|    ||::   0x06d96a58      ldr   x3, [arg_68h]                        ; [0x68:4]=0x380
|    ||::   0x06d96a5c      str   x3, [x0, 0x10]
|    ||::   0x06d96a60      adrp  x27, 0xb48f000
|    ||::   0x06d96a64      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d96a68      cbnz  w3, 0x6d96a78
|   |||::   0x06d96a6c      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
|   |||::   0x06d96a70      ldr   x6, [arg_f8h]                        ; [0xf8:4]=0x799b960 segment.GNU_RELRO
|  ,======< 0x06d96a74      b     0x6d96a88
|  |`-----> 0x06d96a78      bl    fcn.04da3500
|  | ||::   0x06d96a7c      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
|  | ||::   0x06d96a80      ldr   x6, [arg_f8h]                        ; [0xf8:4]=0x799b960 segment.GNU_RELRO
|  | ||::   0x06d96a84      stp   x5, x6, [x25]
|  | ||::   ; CODE XREF from fcn.06d969a8 @ 0x6d96a74
|  `------> 0x06d96a88      str   x5, [x0, 8]
|    ||::   0x06d96a8c      adrp  x5, 0x8f08000
|    ||::   0x06d96a90      add   x5, x5, 0xca8
|    ||::   0x06d96a94      stp   x5, x6, [x0, 0x18]
|   ,=====< 0x06d96a98      b     0x6d96bb8
|   |||::   0x06d96a9c      cmp   x4, 3
|  ,======< 0x06d96aa0      b.eq  0x6d96b44
|  ||||::   0x06d96aa4      cmp   x4, 4
| ,=======< 0x06d96aa8      b.eq  0x6d96ab4
| ||||`---> 0x06d96aac      mov   x0, xzr
| ||||,===< 0x06d96ab0      b     0x6d96bb8
| `-------> 0x06d96ab4      orr   x0, xzr, 0x10
|  ||||::   0x06d96ab8      adrp  x1, 0x875e000
|  ||||::   0x06d96abc      add   x1, x1, 0xeb0
|  ||||::   0x06d96ac0      orr   x2, xzr, 1
|  ||||::   0x06d96ac4      bl    fcn.04d32500
|  ||||::   0x06d96ac8      ldp   x3, x4, [arg_b0h]
|  ||||::   0x06d96acc      str   x4, [x0, 8]
|  ||||::   0x06d96ad0      adrp  x27, 0xb48f000
|  ||||::   0x06d96ad4      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
| ,=======< 0x06d96ad8      cbz   w4, 0x6d96ae4
| |||||::   0x06d96adc      bl    fcn.04da34f0
| |||||::   0x06d96ae0      str   x3, [x25]
| `-------> 0x06d96ae4      str   x0, [arg_f0h]
|  ||||::   0x06d96ae8      str   x3, [x0]
|  ||||::   0x06d96aec      mov   x0, 0x50                             ; 'P'
|  ||||::   0x06d96af0      adrp  x1, 0x8c55000
|  ||||::   0x06d96af4      add   x1, x1, 0xa68
|  ||||::   0x06d96af8      orr   x2, xzr, 1
|  ||||::   0x06d96afc      bl    0x4d33650
|  ||||::   0x06d96b00      ldr   x3, [arg_68h]                        ; [0x68:4]=0x380
|  ||||::   0x06d96b04      str   x3, [x0, 0x10]
|  ||||::   0x06d96b08      adrp  x27, 0xb48f000
|  ||||::   0x06d96b0c      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
| ,=======< 0x06d96b10      cbnz  w3, 0x6d96b20
| |||||::   0x06d96b14      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
| |||||::   0x06d96b18      ldr   x6, [arg_f0h]                        ; [0xf0:4]=0x779b960
| ========< 0x06d96b1c      b     0x6d96b30
| `-------> 0x06d96b20      bl    fcn.04da3500
|  ||||::   0x06d96b24      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
|  ||||::   0x06d96b28      ldr   x6, [arg_f0h]                        ; [0xf0:4]=0x779b960
|  ||||::   0x06d96b2c      stp   x5, x6, [x25]
|  ||||::   ; CODE XREF from fcn.06d969a8 @ 0x6d96b1c
| --------> 0x06d96b30      str   x5, [x0, 8]
|  ||||::   0x06d96b34      adrp  x5, 0x8f08000
|  ||||::   0x06d96b38      add   x5, x5, 0xd08
|  ||||::   0x06d96b3c      stp   x5, x6, [x0, 0x18]
| ,=======< 0x06d96b40      b     0x6d96bb8
| |`------> 0x06d96b44      orr   x0, xzr, 8
| | |||::   0x06d96b48      adrp  x1, 0x875e000
| | |||::   0x06d96b4c      add   x1, x1, 0xfa0
| | |||::   0x06d96b50      orr   x2, xzr, 1
| | |||::   0x06d96b54      bl    fcn.04d33a20
| | |||::   0x06d96b58      str   x0, [arg_e8h]
| | |||::   0x06d96b5c      ldr   d0, [arg_a8h]                        ; [0xa8:4]=0x200000
| | |||::   0x06d96b60      str   d0, [x0]
| | |||::   0x06d96b64      mov   x0, 0x50                             ; 'P'
| | |||::   0x06d96b68      adrp  x1, 0x8c55000
| | |||::   0x06d96b6c      add   x1, x1, 0xa68
| | |||::   0x06d96b70      orr   x2, xzr, 1
| | |||::   0x06d96b74      bl    0x4d33650
| | |||::   0x06d96b78      ldr   x3, [arg_68h]                        ; [0x68:4]=0x380
| | |||::   0x06d96b7c      str   x3, [x0, 0x10]
| | |||::   0x06d96b80      adrp  x27, 0xb48f000
| | |||::   0x06d96b84      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
| |,======< 0x06d96b88      cbnz  w3, 0x6d96b98
| |||||::   0x06d96b8c      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
| |||||::   0x06d96b90      ldr   x6, [arg_e8h]                        ; [0xe8:4]=1
| ========< 0x06d96b94      b     0x6d96ba8
| |`------> 0x06d96b98      bl    fcn.04da3500
| | |||::   0x06d96b9c      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
| | |||::   0x06d96ba0      ldr   x6, [arg_e8h]                        ; [0xe8:4]=1
| | |||::   0x06d96ba4      stp   x5, x6, [x25]
| | |||::   ; CODE XREF from fcn.06d969a8 @ 0x6d96b94
| --------> 0x06d96ba8      str   x5, [x0, 8]
| | |||::   0x06d96bac      adrp  x5, 0x8f08000
| | |||::   0x06d96bb0      add   x5, x5, 0xce8
| | |||::   0x06d96bb4      stp   x5, x6, [x0, 0x18]
| | |||::   ; CODE XREFS from fcn.06d969a8 @ 0x6d96a20, 0x6d96a98, 0x6d96ab0, 0x6d96b40
| `,```---> 0x06d96bb8      cbz   x0, 0x6d96c1c
|  |   ::   0x06d96bbc      ldr   x5, [arg_78h]                        ; [0x78:4]=1
|  |   ::   0x06d96bc0      add   x1, x5, 1
|  |   ::   0x06d96bc4      ldr   x2, [arg_80h]                        ; [0x80:4]=0
|  |   ::   0x06d96bc8      cmp   x2, x1
|  |  ,===< 0x06d96bcc      b.lo  0x6d96bd8
|  |  |::   0x06d96bd0      ldr   x3, [arg_d8h]                        ; [0xd8:4]=0
|  | ,====< 0x06d96bd4      b     0x6d96bf8
|  | |`---> 0x06d96bd8      str   x0, [arg_e0h]
|  | | ::   0x06d96bdc      ldr   x0, [arg_d8h]                        ; [0xd8:4]=0
|  | | ::   0x06d96be0      orr   x3, xzr, 1
|  | | ::   0x06d96be4      adrp  x4, 0x8141000
|  | | ::   0x06d96be8      add   x4, x4, 0x730
|  | | ::   0x06d96bec      bl    fcn.04d9e050
|  | | ::   0x06d96bf0      mov   x3, x0
|  | | ::   0x06d96bf4      ldr   x0, [arg_e0h]                        ; [0xe0:4]=0
|  | | ::   ; CODE XREF from fcn.06d969a8 @ 0x6d96bd4
|  | `----> 0x06d96bf8      sub   x4, x1, 1
|  |   ::   0x06d96bfc      adrp  x27, 0xb48f000
|  |   ::   0x06d96c00      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|  |  ,===< 0x06d96c04      cbz   w5, 0x6d96c14
|  |  |::   0x06d96c08      ldr   x5, [x3, x4, lsl 3]
|  |  |::   0x06d96c0c      bl    fcn.04da3500
|  |  |::   0x06d96c10      stp   x0, x5, [x25]
|  |  `---> 0x06d96c14      str   x0, [x3, x4, lsl 3]
|  |   `==< 0x06d96c18      b     0x6d9692c
|  `------> 0x06d96c1c      ldp   x1, x2, [arg_78h]
|       :   0x06d96c20      ldr   x3, [arg_d8h]                        ; [0xd8:4]=0
|       `=< 0x06d96c24      b     0x6d9692c
|           0x06d96c28      adrp  x0, 0x8cd3000
|           0x06d96c2c      add   x0, x0, 0x38
|           0x06d96c30      bl    0x4d31c10
|           0x06d96c34      ldp   x1, x2, [arg_78h]
|           0x06d96c38      stp   x1, x2, [x0, 0x28]
|           0x06d96c3c      adrp  x27, 0xb48f000
|           0x06d96c40      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d96c44      cbnz  w1, 0x6d96c50
|       |   0x06d96c48      ldr   x1, [arg_298h]                       ; [0xd8:4]=0
|      ,==< 0x06d96c4c      b     0x6d96c5c
|      |`-> 0x06d96c50      bl    fcn.04da34f0
|      |    0x06d96c54      ldr   x1, [arg_d8h]                        ; [0xd8:4]=0
|      |    0x06d96c58      str   x1, [x25]
|      |    ; CODE XREF from fcn.06d969a8 @ 0x6d96c4c
|      `--> 0x06d96c5c      str   x1, [x0, 0x20]
|           0x06d96c60      ldp   fp, lr, [sp, -8]
|           0x06d96c64      add   sp, sp, 0x1c0
|           0x06d96c68      ret
==== google3_third_party_jetski_gateway_gateway.isImageModel
/ fcn.06d969a8(int64_t arg3, int64_t arg_68h, int64_t arg_78h, int64_t arg_80h, int64_t arg_88h, int64_t arg_98h, int64_t arg_a0h, int64_t arg_a8h, int64_t arg_b0h, int64_t arg_c8h, int64_t arg_d8h, int64_t arg_e0h, int64_t arg_e8h, int64_t arg_f0h, int64_t arg_f8h, int64_t arg_100h, int64_t arg_108h, int64_t arg_1c8h, int64_t arg_298h);
|      ::   ; arg int64_t arg3 @ x2
|      ::   ; arg int64_t arg_68h @ stack + 0x68
|      ::   ; arg int64_t arg_78h @ stack + 0x78
|      ::   ; arg int64_t arg_80h @ stack + 0x80
|      ::   ; arg int64_t arg_88h @ stack + 0x88
|      ::   ; arg int64_t arg_98h @ stack + 0x98
|      ::   ; arg int64_t arg_a0h @ stack + 0xa0
|      ::   ; arg int64_t arg_a8h @ stack + 0xa8
|      ::   ; arg int64_t arg_b0h @ stack + 0xb0
|      ::   ; arg int64_t arg_c8h @ stack + 0xc8
|      ::   ; arg int64_t arg_d8h @ stack + 0xd8
|      ::   ; arg int64_t arg_e0h @ stack + 0xe0
|      ::   ; arg int64_t arg_e8h @ stack + 0xe8
|      ::   ; arg int64_t arg_f0h @ stack + 0xf0
|      ::   ; arg int64_t arg_f8h @ stack + 0xf8
|      ::   ; arg int64_t arg_100h @ stack + 0x100
|      ::   ; arg int64_t arg_108h @ stack + 0x108
|      ::   ; arg int64_t arg_1c8h @ stack + 0x1c8
|      ::   ; arg int64_t arg_298h @ stack + 0x298
|     ,===< 0x06d969a8      b.ne  0x6d96aac
|     |::   0x06d969ac      orr   x0, xzr, 8
|     |::   0x06d969b0      adrp  x1, 0x875e000
|     |::   0x06d969b4      add   x1, x1, 0xf28
|     |::   0x06d969b8      orr   x2, xzr, 1
|     |::   0x06d969bc      bl    fcn.04d33a20
|     |::   0x06d969c0      str   x0, [arg_100h]
|     |::   0x06d969c4      ldr   x3, [arg_a0h]                        ; [0xa0:4]=0x779b960
|     |::   0x06d969c8      str   x3, [x0]
|     |::   0x06d969cc      mov   x0, 0x50                             ; 'P'
|     |::   0x06d969d0      adrp  x1, 0x8c55000
|     |::   0x06d969d4      add   x1, x1, 0xa68
|     |::   0x06d969d8      orr   x2, xzr, 1
|     |::   0x06d969dc      bl    0x4d33650
|     |::   0x06d969e0      ldr   x3, [arg_68h]                        ; [0x68:4]=0x380
|     |::   0x06d969e4      str   x3, [x0, 0x10]
|     |::   0x06d969e8      adrp  x27, 0xb48f000
|     |::   0x06d969ec      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d969f0      cbnz  w3, 0x6d96a00
|    ||::   0x06d969f4      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
|    ||::   0x06d969f8      ldr   x6, [arg_100h]                       ; [0x100:4]=0x799b960 segment.GNU_RELRO
|   ,=====< 0x06d969fc      b     0x6d96a10
|   |`----> 0x06d96a00      bl    fcn.04da3500
|   | |::   0x06d96a04      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
|   | |::   0x06d96a08      ldr   x6, [arg_100h]                       ; [0x100:4]=0x799b960 segment.GNU_RELRO
|   | |::   0x06d96a0c      stp   x5, x6, [x25]
|   | |::   ; CODE XREF from fcn.06d969a8 @ 0x6d969fc
|   `-----> 0x06d96a10      str   x5, [x0, 8]
|     |::   0x06d96a14      adrp  x5, 0x8f08000
|     |::   0x06d96a18      add   x5, x5, 0xcc8
|     |::   0x06d96a1c      stp   x5, x6, [x0, 0x18]
|    ,====< 0x06d96a20      b     0x6d96bb8
|    ||::   0x06d96a24      orr   x0, xzr, 1
|    ||::   0x06d96a28      adrp  x1, 0x875f000
|    ||::   0x06d96a2c      add   x1, x1, 0x18
|    ||::   0x06d96a30      orr   x2, xzr, 1
|    ||::   0x06d96a34      bl    fcn.04d33a20
|    ||::   0x06d96a38      str   x0, [arg_f8h]
|    ||::   0x06d96a3c      ldrb  w3, [arg_98h]                        ; [0x98:4]=0x779b960
|    ||::   0x06d96a40      strb  w3, [x0]
|    ||::   0x06d96a44      mov   x0, 0x50                             ; 'P'
|    ||::   0x06d96a48      adrp  x1, 0x8c55000
|    ||::   0x06d96a4c      add   x1, x1, 0xa68
|    ||::   0x06d96a50      orr   x2, xzr, 1
|    ||::   0x06d96a54      bl    0x4d33650
|    ||::   0x06d96a58      ldr   x3, [arg_68h]                        ; [0x68:4]=0x380
|    ||::   0x06d96a5c      str   x3, [x0, 0x10]
|    ||::   0x06d96a60      adrp  x27, 0xb48f000
|    ||::   0x06d96a64      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d96a68      cbnz  w3, 0x6d96a78
|   |||::   0x06d96a6c      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
|   |||::   0x06d96a70      ldr   x6, [arg_f8h]                        ; [0xf8:4]=0x799b960 segment.GNU_RELRO
|  ,======< 0x06d96a74      b     0x6d96a88
|  |`-----> 0x06d96a78      bl    fcn.04da3500
|  | ||::   0x06d96a7c      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
|  | ||::   0x06d96a80      ldr   x6, [arg_f8h]                        ; [0xf8:4]=0x799b960 segment.GNU_RELRO
|  | ||::   0x06d96a84      stp   x5, x6, [x25]
|  | ||::   ; CODE XREF from fcn.06d969a8 @ 0x6d96a74
|  `------> 0x06d96a88      str   x5, [x0, 8]
|    ||::   0x06d96a8c      adrp  x5, 0x8f08000
|    ||::   0x06d96a90      add   x5, x5, 0xca8
|    ||::   0x06d96a94      stp   x5, x6, [x0, 0x18]
|   ,=====< 0x06d96a98      b     0x6d96bb8
|   |||::   0x06d96a9c      cmp   x4, 3
|  ,======< 0x06d96aa0      b.eq  0x6d96b44
|  ||||::   0x06d96aa4      cmp   x4, 4
| ,=======< 0x06d96aa8      b.eq  0x6d96ab4
| ||||`---> 0x06d96aac      mov   x0, xzr
| ||||,===< 0x06d96ab0      b     0x6d96bb8
| `-------> 0x06d96ab4      orr   x0, xzr, 0x10
|  ||||::   0x06d96ab8      adrp  x1, 0x875e000
|  ||||::   0x06d96abc      add   x1, x1, 0xeb0
|  ||||::   0x06d96ac0      orr   x2, xzr, 1
|  ||||::   0x06d96ac4      bl    fcn.04d32500
|  ||||::   0x06d96ac8      ldp   x3, x4, [arg_b0h]
|  ||||::   0x06d96acc      str   x4, [x0, 8]
|  ||||::   0x06d96ad0      adrp  x27, 0xb48f000
|  ||||::   0x06d96ad4      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
| ,=======< 0x06d96ad8      cbz   w4, 0x6d96ae4
| |||||::   0x06d96adc      bl    fcn.04da34f0
| |||||::   0x06d96ae0      str   x3, [x25]
| `-------> 0x06d96ae4      str   x0, [arg_f0h]
|  ||||::   0x06d96ae8      str   x3, [x0]
|  ||||::   0x06d96aec      mov   x0, 0x50                             ; 'P'
|  ||||::   0x06d96af0      adrp  x1, 0x8c55000
|  ||||::   0x06d96af4      add   x1, x1, 0xa68
|  ||||::   0x06d96af8      orr   x2, xzr, 1
|  ||||::   0x06d96afc      bl    0x4d33650
|  ||||::   0x06d96b00      ldr   x3, [arg_68h]                        ; [0x68:4]=0x380
|  ||||::   0x06d96b04      str   x3, [x0, 0x10]
|  ||||::   0x06d96b08      adrp  x27, 0xb48f000
|  ||||::   0x06d96b0c      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
| ,=======< 0x06d96b10      cbnz  w3, 0x6d96b20
| |||||::   0x06d96b14      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
| |||||::   0x06d96b18      ldr   x6, [arg_f0h]                        ; [0xf0:4]=0x779b960
| ========< 0x06d96b1c      b     0x6d96b30
| `-------> 0x06d96b20      bl    fcn.04da3500
|  ||||::   0x06d96b24      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
|  ||||::   0x06d96b28      ldr   x6, [arg_f0h]                        ; [0xf0:4]=0x779b960
|  ||||::   0x06d96b2c      stp   x5, x6, [x25]
|  ||||::   ; CODE XREF from fcn.06d969a8 @ 0x6d96b1c
| --------> 0x06d96b30      str   x5, [x0, 8]
|  ||||::   0x06d96b34      adrp  x5, 0x8f08000
|  ||||::   0x06d96b38      add   x5, x5, 0xd08
|  ||||::   0x06d96b3c      stp   x5, x6, [x0, 0x18]
| ,=======< 0x06d96b40      b     0x6d96bb8
| |`------> 0x06d96b44      orr   x0, xzr, 8
| | |||::   0x06d96b48      adrp  x1, 0x875e000
| | |||::   0x06d96b4c      add   x1, x1, 0xfa0
| | |||::   0x06d96b50      orr   x2, xzr, 1
| | |||::   0x06d96b54      bl    fcn.04d33a20
| | |||::   0x06d96b58      str   x0, [arg_e8h]
| | |||::   0x06d96b5c      ldr   d0, [arg_a8h]                        ; [0xa8:4]=0x200000
| | |||::   0x06d96b60      str   d0, [x0]
| | |||::   0x06d96b64      mov   x0, 0x50                             ; 'P'
| | |||::   0x06d96b68      adrp  x1, 0x8c55000
| | |||::   0x06d96b6c      add   x1, x1, 0xa68
| | |||::   0x06d96b70      orr   x2, xzr, 1
| | |||::   0x06d96b74      bl    0x4d33650
| | |||::   0x06d96b78      ldr   x3, [arg_68h]                        ; [0x68:4]=0x380
| | |||::   0x06d96b7c      str   x3, [x0, 0x10]
| | |||::   0x06d96b80      adrp  x27, 0xb48f000
| | |||::   0x06d96b84      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
| |,======< 0x06d96b88      cbnz  w3, 0x6d96b98
| |||||::   0x06d96b8c      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
| |||||::   0x06d96b90      ldr   x6, [arg_e8h]                        ; [0xe8:4]=1
| ========< 0x06d96b94      b     0x6d96ba8
| |`------> 0x06d96b98      bl    fcn.04da3500
| | |||::   0x06d96b9c      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
| | |||::   0x06d96ba0      ldr   x6, [arg_e8h]                        ; [0xe8:4]=1
| | |||::   0x06d96ba4      stp   x5, x6, [x25]
| | |||::   ; CODE XREF from fcn.06d969a8 @ 0x6d96b94
| --------> 0x06d96ba8      str   x5, [x0, 8]
| | |||::   0x06d96bac      adrp  x5, 0x8f08000
| | |||::   0x06d96bb0      add   x5, x5, 0xce8
| | |||::   0x06d96bb4      stp   x5, x6, [x0, 0x18]
| | |||::   ; CODE XREFS from fcn.06d969a8 @ 0x6d96a20, 0x6d96a98, 0x6d96ab0, 0x6d96b40
| `,```---> 0x06d96bb8      cbz   x0, 0x6d96c1c
|  |   ::   0x06d96bbc      ldr   x5, [arg_78h]                        ; [0x78:4]=1
|  |   ::   0x06d96bc0      add   x1, x5, 1
|  |   ::   0x06d96bc4      ldr   x2, [arg_80h]                        ; [0x80:4]=0
|  |   ::   0x06d96bc8      cmp   x2, x1
|  |  ,===< 0x06d96bcc      b.lo  0x6d96bd8
|  |  |::   0x06d96bd0      ldr   x3, [arg_d8h]                        ; [0xd8:4]=0
|  | ,====< 0x06d96bd4      b     0x6d96bf8
|  | |`---> 0x06d96bd8      str   x0, [arg_e0h]
|  | | ::   0x06d96bdc      ldr   x0, [arg_d8h]                        ; [0xd8:4]=0
|  | | ::   0x06d96be0      orr   x3, xzr, 1
|  | | ::   0x06d96be4      adrp  x4, 0x8141000
|  | | ::   0x06d96be8      add   x4, x4, 0x730
|  | | ::   0x06d96bec      bl    fcn.04d9e050
|  | | ::   0x06d96bf0      mov   x3, x0
|  | | ::   0x06d96bf4      ldr   x0, [arg_e0h]                        ; [0xe0:4]=0
|  | | ::   ; CODE XREF from fcn.06d969a8 @ 0x6d96bd4
|  | `----> 0x06d96bf8      sub   x4, x1, 1
|  |   ::   0x06d96bfc      adrp  x27, 0xb48f000
|  |   ::   0x06d96c00      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|  |  ,===< 0x06d96c04      cbz   w5, 0x6d96c14
|  |  |::   0x06d96c08      ldr   x5, [x3, x4, lsl 3]
|  |  |::   0x06d96c0c      bl    fcn.04da3500
|  |  |::   0x06d96c10      stp   x0, x5, [x25]
|  |  `---> 0x06d96c14      str   x0, [x3, x4, lsl 3]
|  |   `==< 0x06d96c18      b     0x6d9692c
|  `------> 0x06d96c1c      ldp   x1, x2, [arg_78h]
|       :   0x06d96c20      ldr   x3, [arg_d8h]                        ; [0xd8:4]=0
|       `=< 0x06d96c24      b     0x6d9692c
|           0x06d96c28      adrp  x0, 0x8cd3000
|           0x06d96c2c      add   x0, x0, 0x38
|           0x06d96c30      bl    0x4d31c10
|           0x06d96c34      ldp   x1, x2, [arg_78h]
|           0x06d96c38      stp   x1, x2, [x0, 0x28]
|           0x06d96c3c      adrp  x27, 0xb48f000
|           0x06d96c40      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d96c44      cbnz  w1, 0x6d96c50
|       |   0x06d96c48      ldr   x1, [arg_298h]                       ; [0xd8:4]=0
|      ,==< 0x06d96c4c      b     0x6d96c5c
|      |`-> 0x06d96c50      bl    fcn.04da34f0
|      |    0x06d96c54      ldr   x1, [arg_d8h]                        ; [0xd8:4]=0
|      |    0x06d96c58      str   x1, [x25]
|      |    ; CODE XREF from fcn.06d969a8 @ 0x6d96c4c
|      `--> 0x06d96c5c      str   x1, [x0, 0x20]
|           0x06d96c60      ldp   fp, lr, [sp, -8]
|           0x06d96c64      add   sp, sp, 0x1c0
|           0x06d96c68      ret
==== google3_third_party_jetski_gateway_gateway.findKnownModelInfo
/ fcn.06d969a8(int64_t arg3, int64_t arg_68h, int64_t arg_78h, int64_t arg_80h, int64_t arg_88h, int64_t arg_98h, int64_t arg_a0h, int64_t arg_a8h, int64_t arg_b0h, int64_t arg_c8h, int64_t arg_d8h, int64_t arg_e0h, int64_t arg_e8h, int64_t arg_f0h, int64_t arg_f8h, int64_t arg_100h, int64_t arg_108h, int64_t arg_1c8h, int64_t arg_298h);
|      ::   ; arg int64_t arg3 @ x2
|      ::   ; arg int64_t arg_68h @ stack + 0x68
|      ::   ; arg int64_t arg_78h @ stack + 0x78
|      ::   ; arg int64_t arg_80h @ stack + 0x80
|      ::   ; arg int64_t arg_88h @ stack + 0x88
|      ::   ; arg int64_t arg_98h @ stack + 0x98
|      ::   ; arg int64_t arg_a0h @ stack + 0xa0
|      ::   ; arg int64_t arg_a8h @ stack + 0xa8
|      ::   ; arg int64_t arg_b0h @ stack + 0xb0
|      ::   ; arg int64_t arg_c8h @ stack + 0xc8
|      ::   ; arg int64_t arg_d8h @ stack + 0xd8
|      ::   ; arg int64_t arg_e0h @ stack + 0xe0
|      ::   ; arg int64_t arg_e8h @ stack + 0xe8
|      ::   ; arg int64_t arg_f0h @ stack + 0xf0
|      ::   ; arg int64_t arg_f8h @ stack + 0xf8
|      ::   ; arg int64_t arg_100h @ stack + 0x100
|      ::   ; arg int64_t arg_108h @ stack + 0x108
|      ::   ; arg int64_t arg_1c8h @ stack + 0x1c8
|      ::   ; arg int64_t arg_298h @ stack + 0x298
|     ,===< 0x06d969a8      b.ne  0x6d96aac
|     |::   0x06d969ac      orr   x0, xzr, 8
|     |::   0x06d969b0      adrp  x1, 0x875e000
|     |::   0x06d969b4      add   x1, x1, 0xf28
|     |::   0x06d969b8      orr   x2, xzr, 1
|     |::   0x06d969bc      bl    fcn.04d33a20
|     |::   0x06d969c0      str   x0, [arg_100h]
|     |::   0x06d969c4      ldr   x3, [arg_a0h]                        ; [0xa0:4]=0x779b960
|     |::   0x06d969c8      str   x3, [x0]
|     |::   0x06d969cc      mov   x0, 0x50                             ; 'P'
|     |::   0x06d969d0      adrp  x1, 0x8c55000
|     |::   0x06d969d4      add   x1, x1, 0xa68
|     |::   0x06d969d8      orr   x2, xzr, 1
|     |::   0x06d969dc      bl    0x4d33650
|     |::   0x06d969e0      ldr   x3, [arg_68h]                        ; [0x68:4]=0x380
|     |::   0x06d969e4      str   x3, [x0, 0x10]
|     |::   0x06d969e8      adrp  x27, 0xb48f000
|     |::   0x06d969ec      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d969f0      cbnz  w3, 0x6d96a00
|    ||::   0x06d969f4      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
|    ||::   0x06d969f8      ldr   x6, [arg_100h]                       ; [0x100:4]=0x799b960 segment.GNU_RELRO
|   ,=====< 0x06d969fc      b     0x6d96a10
|   |`----> 0x06d96a00      bl    fcn.04da3500
|   | |::   0x06d96a04      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
|   | |::   0x06d96a08      ldr   x6, [arg_100h]                       ; [0x100:4]=0x799b960 segment.GNU_RELRO
|   | |::   0x06d96a0c      stp   x5, x6, [x25]
|   | |::   ; CODE XREF from fcn.06d969a8 @ 0x6d969fc
|   `-----> 0x06d96a10      str   x5, [x0, 8]
|     |::   0x06d96a14      adrp  x5, 0x8f08000
|     |::   0x06d96a18      add   x5, x5, 0xcc8
|     |::   0x06d96a1c      stp   x5, x6, [x0, 0x18]
|    ,====< 0x06d96a20      b     0x6d96bb8
|    ||::   0x06d96a24      orr   x0, xzr, 1
|    ||::   0x06d96a28      adrp  x1, 0x875f000
|    ||::   0x06d96a2c      add   x1, x1, 0x18
|    ||::   0x06d96a30      orr   x2, xzr, 1
|    ||::   0x06d96a34      bl    fcn.04d33a20
|    ||::   0x06d96a38      str   x0, [arg_f8h]
|    ||::   0x06d96a3c      ldrb  w3, [arg_98h]                        ; [0x98:4]=0x779b960
|    ||::   0x06d96a40      strb  w3, [x0]
|    ||::   0x06d96a44      mov   x0, 0x50                             ; 'P'
|    ||::   0x06d96a48      adrp  x1, 0x8c55000
|    ||::   0x06d96a4c      add   x1, x1, 0xa68
|    ||::   0x06d96a50      orr   x2, xzr, 1
|    ||::   0x06d96a54      bl    0x4d33650
|    ||::   0x06d96a58      ldr   x3, [arg_68h]                        ; [0x68:4]=0x380
|    ||::   0x06d96a5c      str   x3, [x0, 0x10]
|    ||::   0x06d96a60      adrp  x27, 0xb48f000
|    ||::   0x06d96a64      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d96a68      cbnz  w3, 0x6d96a78
|   |||::   0x06d96a6c      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
|   |||::   0x06d96a70      ldr   x6, [arg_f8h]                        ; [0xf8:4]=0x799b960 segment.GNU_RELRO
|  ,======< 0x06d96a74      b     0x6d96a88
|  |`-----> 0x06d96a78      bl    fcn.04da3500
|  | ||::   0x06d96a7c      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
|  | ||::   0x06d96a80      ldr   x6, [arg_f8h]                        ; [0xf8:4]=0x799b960 segment.GNU_RELRO
|  | ||::   0x06d96a84      stp   x5, x6, [x25]
|  | ||::   ; CODE XREF from fcn.06d969a8 @ 0x6d96a74
|  `------> 0x06d96a88      str   x5, [x0, 8]
|    ||::   0x06d96a8c      adrp  x5, 0x8f08000
|    ||::   0x06d96a90      add   x5, x5, 0xca8
|    ||::   0x06d96a94      stp   x5, x6, [x0, 0x18]
|   ,=====< 0x06d96a98      b     0x6d96bb8
|   |||::   0x06d96a9c      cmp   x4, 3
|  ,======< 0x06d96aa0      b.eq  0x6d96b44
|  ||||::   0x06d96aa4      cmp   x4, 4
| ,=======< 0x06d96aa8      b.eq  0x6d96ab4
| ||||`---> 0x06d96aac      mov   x0, xzr
| ||||,===< 0x06d96ab0      b     0x6d96bb8
| `-------> 0x06d96ab4      orr   x0, xzr, 0x10
|  ||||::   0x06d96ab8      adrp  x1, 0x875e000
|  ||||::   0x06d96abc      add   x1, x1, 0xeb0
|  ||||::   0x06d96ac0      orr   x2, xzr, 1
|  ||||::   0x06d96ac4      bl    fcn.04d32500
|  ||||::   0x06d96ac8      ldp   x3, x4, [arg_b0h]
|  ||||::   0x06d96acc      str   x4, [x0, 8]
|  ||||::   0x06d96ad0      adrp  x27, 0xb48f000
|  ||||::   0x06d96ad4      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
| ,=======< 0x06d96ad8      cbz   w4, 0x6d96ae4
| |||||::   0x06d96adc      bl    fcn.04da34f0
| |||||::   0x06d96ae0      str   x3, [x25]
| `-------> 0x06d96ae4      str   x0, [arg_f0h]
|  ||||::   0x06d96ae8      str   x3, [x0]
|  ||||::   0x06d96aec      mov   x0, 0x50                             ; 'P'
|  ||||::   0x06d96af0      adrp  x1, 0x8c55000
|  ||||::   0x06d96af4      add   x1, x1, 0xa68
|  ||||::   0x06d96af8      orr   x2, xzr, 1
|  ||||::   0x06d96afc      bl    0x4d33650
|  ||||::   0x06d96b00      ldr   x3, [arg_68h]                        ; [0x68:4]=0x380
|  ||||::   0x06d96b04      str   x3, [x0, 0x10]
|  ||||::   0x06d96b08      adrp  x27, 0xb48f000
|  ||||::   0x06d96b0c      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
| ,=======< 0x06d96b10      cbnz  w3, 0x6d96b20
| |||||::   0x06d96b14      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
| |||||::   0x06d96b18      ldr   x6, [arg_f0h]                        ; [0xf0:4]=0x779b960
| ========< 0x06d96b1c      b     0x6d96b30
| `-------> 0x06d96b20      bl    fcn.04da3500
|  ||||::   0x06d96b24      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
|  ||||::   0x06d96b28      ldr   x6, [arg_f0h]                        ; [0xf0:4]=0x779b960
|  ||||::   0x06d96b2c      stp   x5, x6, [x25]
|  ||||::   ; CODE XREF from fcn.06d969a8 @ 0x6d96b1c
| --------> 0x06d96b30      str   x5, [x0, 8]
|  ||||::   0x06d96b34      adrp  x5, 0x8f08000
|  ||||::   0x06d96b38      add   x5, x5, 0xd08
|  ||||::   0x06d96b3c      stp   x5, x6, [x0, 0x18]
| ,=======< 0x06d96b40      b     0x6d96bb8
| |`------> 0x06d96b44      orr   x0, xzr, 8
| | |||::   0x06d96b48      adrp  x1, 0x875e000
| | |||::   0x06d96b4c      add   x1, x1, 0xfa0
| | |||::   0x06d96b50      orr   x2, xzr, 1
| | |||::   0x06d96b54      bl    fcn.04d33a20
| | |||::   0x06d96b58      str   x0, [arg_e8h]
| | |||::   0x06d96b5c      ldr   d0, [arg_a8h]                        ; [0xa8:4]=0x200000
| | |||::   0x06d96b60      str   d0, [x0]
| | |||::   0x06d96b64      mov   x0, 0x50                             ; 'P'
| | |||::   0x06d96b68      adrp  x1, 0x8c55000
| | |||::   0x06d96b6c      add   x1, x1, 0xa68
| | |||::   0x06d96b70      orr   x2, xzr, 1
| | |||::   0x06d96b74      bl    0x4d33650
| | |||::   0x06d96b78      ldr   x3, [arg_68h]                        ; [0x68:4]=0x380
| | |||::   0x06d96b7c      str   x3, [x0, 0x10]
| | |||::   0x06d96b80      adrp  x27, 0xb48f000
| | |||::   0x06d96b84      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
| |,======< 0x06d96b88      cbnz  w3, 0x6d96b98
| |||||::   0x06d96b8c      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
| |||||::   0x06d96b90      ldr   x6, [arg_e8h]                        ; [0xe8:4]=1
| ========< 0x06d96b94      b     0x6d96ba8
| |`------> 0x06d96b98      bl    fcn.04da3500
| | |||::   0x06d96b9c      ldr   x5, [arg_c8h]                        ; [0xc8:4]=0
| | |||::   0x06d96ba0      ldr   x6, [arg_e8h]                        ; [0xe8:4]=1
| | |||::   0x06d96ba4      stp   x5, x6, [x25]
| | |||::   ; CODE XREF from fcn.06d969a8 @ 0x6d96b94
| --------> 0x06d96ba8      str   x5, [x0, 8]
| | |||::   0x06d96bac      adrp  x5, 0x8f08000
| | |||::   0x06d96bb0      add   x5, x5, 0xce8
| | |||::   0x06d96bb4      stp   x5, x6, [x0, 0x18]
| | |||::   ; CODE XREFS from fcn.06d969a8 @ 0x6d96a20, 0x6d96a98, 0x6d96ab0, 0x6d96b40
| `,```---> 0x06d96bb8      cbz   x0, 0x6d96c1c
|  |   ::   0x06d96bbc      ldr   x5, [arg_78h]                        ; [0x78:4]=1
|  |   ::   0x06d96bc0      add   x1, x5, 1
|  |   ::   0x06d96bc4      ldr   x2, [arg_80h]                        ; [0x80:4]=0
|  |   ::   0x06d96bc8      cmp   x2, x1
|  |  ,===< 0x06d96bcc      b.lo  0x6d96bd8
|  |  |::   0x06d96bd0      ldr   x3, [arg_d8h]                        ; [0xd8:4]=0
|  | ,====< 0x06d96bd4      b     0x6d96bf8
|  | |`---> 0x06d96bd8      str   x0, [arg_e0h]
|  | | ::   0x06d96bdc      ldr   x0, [arg_d8h]                        ; [0xd8:4]=0
|  | | ::   0x06d96be0      orr   x3, xzr, 1
|  | | ::   0x06d96be4      adrp  x4, 0x8141000
|  | | ::   0x06d96be8      add   x4, x4, 0x730
|  | | ::   0x06d96bec      bl    fcn.04d9e050
|  | | ::   0x06d96bf0      mov   x3, x0
|  | | ::   0x06d96bf4      ldr   x0, [arg_e0h]                        ; [0xe0:4]=0
|  | | ::   ; CODE XREF from fcn.06d969a8 @ 0x6d96bd4
|  | `----> 0x06d96bf8      sub   x4, x1, 1
|  |   ::   0x06d96bfc      adrp  x27, 0xb48f000
|  |   ::   0x06d96c00      ldr   w5, [x27, 0x450]                     ; [0x450:4]=0x58585858
|  |  ,===< 0x06d96c04      cbz   w5, 0x6d96c14
|  |  |::   0x06d96c08      ldr   x5, [x3, x4, lsl 3]
|  |  |::   0x06d96c0c      bl    fcn.04da3500
|  |  |::   0x06d96c10      stp   x0, x5, [x25]
|  |  `---> 0x06d96c14      str   x0, [x3, x4, lsl 3]
|  |   `==< 0x06d96c18      b     0x6d9692c
|  `------> 0x06d96c1c      ldp   x1, x2, [arg_78h]
|       :   0x06d96c20      ldr   x3, [arg_d8h]                        ; [0xd8:4]=0
|       `=< 0x06d96c24      b     0x6d9692c
|           0x06d96c28      adrp  x0, 0x8cd3000
|           0x06d96c2c      add   x0, x0, 0x38
|           0x06d96c30      bl    0x4d31c10
|           0x06d96c34      ldp   x1, x2, [arg_78h]
|           0x06d96c38      stp   x1, x2, [x0, 0x28]
|           0x06d96c3c      adrp  x27, 0xb48f000
|           0x06d96c40      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|       ,=< 0x06d96c44      cbnz  w1, 0x6d96c50
|       |   0x06d96c48      ldr   x1, [arg_298h]                       ; [0xd8:4]=0
|      ,==< 0x06d96c4c      b     0x6d96c5c
|      |`-> 0x06d96c50      bl    fcn.04da34f0
|      |    0x06d96c54      ldr   x1, [arg_d8h]                        ; [0xd8:4]=0
|      |    0x06d96c58      str   x1, [x25]
|      |    ; CODE XREF from fcn.06d969a8 @ 0x6d96c4c
|      `--> 0x06d96c5c      str   x1, [x0, 0x20]
|           0x06d96c60      ldp   fp, lr, [sp, -8]
|           0x06d96c64      add   sp, sp, 0x1c0
|           0x06d96c68      ret
==== google3_third_party_jetski_gateway_gateway.isSupportedMimeType
/ fcn.06d96cc8();
\           0x06d96cc8      ret
==== google3_third_party_jetski_gateway_gateway.modelEntryToModelDetails
/ fcn.06d96df8(int64_t arg_48h, int64_t arg_88h, int64_t arg_90h, int64_t arg_98h, int64_t arg_b8h, int64_t arg_c8h, int64_t arg_e8h, int64_t arg_f8h, int64_t arg_118h, int64_t arg_128h, int64_t arg_148h, int64_t arg_158h, int64_t arg_178h, int64_t arg_188h, int64_t arg_190h, int64_t arg_198h, int64_t arg_1a0h, int64_t arg_1a8h, int64_t arg_1b0h, int64_t arg_1b8h, int64_t arg_1c0h, int64_t arg_3f8h, int64_t arg_498h);
|           ; arg int64_t arg_48h @ stack + 0x48
|           ; arg int64_t arg_88h @ stack + 0x88
|           ; arg int64_t arg_90h @ stack + 0x90
|           ; arg int64_t arg_98h @ stack + 0x98
|           ; arg int64_t arg_b8h @ stack + 0xb8
|           ; arg int64_t arg_c8h @ stack + 0xc8
|           ; arg int64_t arg_e8h @ stack + 0xe8
|           ; arg int64_t arg_f8h @ stack + 0xf8
|           ; arg int64_t arg_118h @ stack + 0x118
|           ; arg int64_t arg_128h @ stack + 0x128
|           ; arg int64_t arg_148h @ stack + 0x148
|           ; arg int64_t arg_158h @ stack + 0x158
|           ; arg int64_t arg_178h @ stack + 0x178
|           ; arg int64_t arg_188h @ stack + 0x188
|           ; arg int64_t arg_190h @ stack + 0x190
|           ; arg int64_t arg_198h @ stack + 0x198
|           ; arg int64_t arg_1a0h @ stack + 0x1a0
|           ; arg int64_t arg_1a8h @ stack + 0x1a8
|           ; arg int64_t arg_1b0h @ stack + 0x1b0
|           ; arg int64_t arg_1b8h @ stack + 0x1b8
|           ; arg int64_t arg_1c0h @ stack + 0x1c0
|           ; arg int64_t arg_3f8h @ stack + 0x3f8
|           ; arg int64_t arg_498h @ stack + 0x498
|           0x06d96df8      add   x4, arg_1c0h
|           0x06d96dfc      str   x4, [arg_1c0h + 0x218]
|           0x06d96e00      adrp  x0, 0x8333000
|           0x06d96e04      add   x0, x0, 0x198
|           0x06d96e08      bl    0x4d98fd0
|           0x06d96e0c      str   x0, [arg_1a0h]
|           0x06d96e10      add   x2, arg_3f8h
|           0x06d96e14      stp   xzr, xzr, [x2]
|           0x06d96e18      stp   xzr, xzr, [x2, 0x10]
|           0x06d96e1c      stp   xzr, xzr, [x2, 0x20]
|           0x06d96e20      stp   xzr, xzr, [x2, 0x30]
|           0x06d96e24      stp   xzr, xzr, [x2, 0x40]
|           0x06d96e28      stp   xzr, xzr, [x2, 0x50]
|           0x06d96e2c      adrp  x0, 0x8333000
|           0x06d96e30      add   x0, x0, 0x198
|           0x06d96e34      ldr   x1, [arg_190h]                       ; [0x190:4]=2
|           0x06d96e38      bl    fcn.04d35800
|       ,=< 0x06d96e3c      b     0x6d96e78
..
|       |   ; CODE XREF from fcn.06d96df8 @ 0x6d96ee0
|     ..--> 0x06d96e58      add   x27, arg_158h
|     ::|   0x06d96e5c      ldp   q0, q1, [x27]
|     ::|   0x06d96e60      add   x27, arg_178h
|     ::|   0x06d96e64      ldr   q2, [x27]
|     ::|   0x06d96e68      stp   q0, q1, [x0]
|     ::|   0x06d96e6c      str   q2, [x0, 0x20]
|     ::|   0x06d96e70      add   x0, arg_3f8h
|     ::|   0x06d96e74      bl    fcn.04d35870
|     ::|   ; CODE XREF from fcn.06d96df8 @ 0x6d96e3c
|     ::`-> 0x06d96e78      add   x27, arg_3f8h
|     ::    0x06d96e7c      ldp   x7, x8, [x27]
|     ::,=< 0x06d96e80      cbz   x7, 0x6d96ee4
|     ::|   0x06d96e84      ldrsb x27, [x8]
|     ::|   0x06d96e88      ldp   x2, x3, [x7]
|     ::|   0x06d96e8c      ldp   q0, q1, [x8]
|     ::|   0x06d96e90      ldr   q2, [x8, 0x20]                       ; [0x20:4]=64 ; "@"
|     ::|   0x06d96e94      add   x27, arg_158h
|     ::|   0x06d96e98      stp   q0, q1, [x27]
|     ::|   0x06d96e9c      add   x27, arg_178h
|     ::|   0x06d96ea0      str   q2, [x27]
|     ::|   0x06d96ea4      adrp  x0, 0x8333000
|     ::|   0x06d96ea8      add   x0, x0, 0x198
|     ::|   0x06d96eac      ldr   x1, [arg_1a0h]                       ; [0x1a0:4]=0x799bde0 segment.DYNAMIC
|     ::|   0x06d96eb0      bl    0x4d18e50
|     ::|   0x06d96eb4      adrp  x27, 0xb48f000
|     ::|   0x06d96eb8      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     `===< 0x06d96ebc      cbz   w4, 0x6d96e58
|      :|   0x06d96ec0      str   x0, [arg_1b8h]
|      :|   0x06d96ec4      ldrsb x27, [x0]
|      :|   0x06d96ec8      mov   x1, x0
|      :|   0x06d96ecc      add   x2, arg_158h
|      :|   0x06d96ed0      adrp  x0, 0x8c96000
|      :|   0x06d96ed4      add   x0, x0, 0xa98
|      :|   0x06d96ed8      bl    0x4d35930
|      :|   0x06d96edc      ldr   x0, [arg_1b8h]                       ; [0x1b8:4]=0x220
|      `==< 0x06d96ee0      b     0x6d96e58
|       `-> 0x06d96ee4      ldr   x2, [arg_88h]                        ; [0x88:4]=0
|           0x06d96ee8      cmp   x2, 3
|       ,=< 0x06d96eec      b.eq  0x6d96ef8
|       |   0x06d96ef0      ldr   x1, [arg_188h]                       ; [0x188:4]=1
|      ,==< 0x06d96ef4      b     0x6d96f18
|      |`-> 0x06d96ef8      ldr   x1, [arg_188h]                       ; [0x188:4]=1
|      |    0x06d96efc      ldrh  w7, [x1]
|      |    0x06d96f00      mov   x8, 0x5548                           ; 'HU'
|      |    0x06d96f04      cmp   w7, w8
|      |,=< 0x06d96f08      b.ne  0x6d96f18
|      ||   0x06d96f0c      ldrb  w7, [x1, 2]                          ; [0x2:4]=0x102464c
|      ||   0x06d96f10      cmp   w7, 0x42                             ; 'B'
|     ,===< 0x06d96f14      b.eq  0x6d96f68
|     |||   ; CODE XREF from fcn.06d96df8 @ 0x6d96ef4
|     |``-> 0x06d96f18      ldr   x4, [arg_1a8h]                       ; [0x1a8:4]=0x799bde0 segment.DYNAMIC
|     |     0x06d96f1c      ldr   x4, [x4, 8]                          ; [0x8:4]=0
|     |     0x06d96f20      adrp  x0, 0x8330000
|     |     0x06d96f24      add   x0, x0, 0xd78
|     |     0x06d96f28      mov   x3, x2
|     |     0x06d96f2c      mov   x2, x1
|     |     0x06d96f30      mov   x1, x4
|     |     0x06d96f34      bl    0x4d18950
|     |     0x06d96f38      ldr   x1, [x0]
|     |     0x06d96f3c      add   x2, arg_3f8h
|     |     0x06d96f40      stp   xzr, xzr, [x2]
|     |     0x06d96f44      stp   xzr, xzr, [x2, 0x10]
|     |     0x06d96f48      stp   xzr, xzr, [x2, 0x20]
|     |     0x06d96f4c      stp   xzr, xzr, [x2, 0x30]
|     |     0x06d96f50      stp   xzr, xzr, [x2, 0x40]
|     |     0x06d96f54      stp   xzr, xzr, [x2, 0x50]
|     |     0x06d96f58      adrp  x0, 0x8333000
|     |     0x06d96f5c      add   x0, x0, 0x198
|     |     0x06d96f60      bl    fcn.04d35800
|     | ,=< 0x06d96f64      b     0x6d97350
|     | |   ; CODE XREF from fcn.06d96df8 @ 0x6d973f8
|     `.--> 0x06d96f68      ldr   x6, [arg_90h]                        ; [0x90:4]=0
|      :|   0x06d96f6c      cmp   x6, 5
|     ,===< 0x06d96f70      b.eq  0x6d96f7c
|     |:|   0x06d96f74      ldr   x5, [arg_198h]                       ; [0x198:4]=0x779bde0
|    ,====< 0x06d96f78      b     0x6d96fa0
|    |`---> 0x06d96f7c      ldr   x5, [arg_198h]                       ; [0x198:4]=0x779bde0
|    | :|   0x06d96f80      ldr   w7, [x5]
|    | :|   0x06d96f84      mov   x8, 0x696c                           ; 'li'
|    | :|   0x06d96f88      movk  x8, 0x756e, lsl 16
|    | :|   0x06d96f8c      cmp   w7, w8
|    |,===< 0x06d96f90      b.ne  0x6d96fa0
|    ||:|   0x06d96f94      ldrb  w7, [x5, 4]                          ; [0x4:4]=0x10102
|    ||:|   0x06d96f98      cmp   w7, 0x78                             ; 'x'
|   ,=====< 0x06d96f9c      b.eq  0x6d96ff4
|   |||:|   ; CODE XREF from fcn.06d96df8 @ 0x6d96f78
|   |``---> 0x06d96fa0      ldr   x4, [arg_1a8h]                       ; [0x1a8:4]=0x799bde0 segment.DYNAMIC
|   |  :|   0x06d96fa4      ldr   x1, [x4, 0x10]                       ; [0x10:4]=0xb70003
|   |  :|   0x06d96fa8      adrp  x0, 0x8330000
|   |  :|   0x06d96fac      add   x0, x0, 0xd78
|   |  :|   0x06d96fb0      mov   x2, x5
|   |  :|   0x06d96fb4      mov   x3, x6
|   |  :|   0x06d96fb8      bl    0x4d18950
|   |  :|   0x06d96fbc      ldr   x1, [x0]
|   |  :|   0x06d96fc0      add   x2, arg_3f8h
|   |  :|   0x06d96fc4      stp   xzr, xzr, [x2]
|   |  :|   0x06d96fc8      stp   xzr, xzr, [x2, 0x10]
|   |  :|   0x06d96fcc      stp   xzr, xzr, [x2, 0x20]
|   |  :|   0x06d96fd0      stp   xzr, xzr, [x2, 0x30]
|   |  :|   0x06d96fd4      stp   xzr, xzr, [x2, 0x40]
|   |  :|   0x06d96fd8      stp   xzr, xzr, [x2, 0x50]
|   |  :|   0x06d96fdc      adrp  x0, 0x8333000
|   |  :|   0x06d96fe0      add   x0, x0, 0x198
|   |  :|   0x06d96fe4      bl    fcn.04d35800
|   |  :|   0x06d96fe8      ldr   x2, [arg_88h]                        ; [0x88:4]=0
|   |  :|   0x06d96fec      cmp   x2, 3
|   | ,===< 0x06d96ff0      b     0x6d97294
|   `-----> 0x06d96ff4      cmp   x2, 3
|     |:|   ; CODE XREF from fcn.06d96df8 @ 0x6d97344
|   ,.----> 0x06d96ff8      b.eq  0x6d97004
|   |:|:|   0x06d96ffc      cmp   x6, 5
|  ,======< 0x06d97000      b     0x6d9702c
|  |`-----> 0x06d97004      ldrh  w7, [x1]
|  | :|:|   0x06d97008      mov   x8, 0x5548                           ; 'HU'
|  | :|:|   0x06d9700c      cmp   w7, w8
|  |,=====< 0x06d97010      b.eq  0x6d9701c
|  ||:|:|   0x06d97014      cmp   x6, 5
| ,=======< 0x06d97018      b     0x6d9702c
| ||`-----> 0x06d9701c      ldrb  w7, [x1, 2]                          ; [0x2:4]=0x102464c
| || :|:|   0x06d97020      cmp   w7, 0x42                             ; 'B'
| ||,=====< 0x06d97024      b.eq  0x6d970b8
| |||:|:|   0x06d97028      cmp   x6, 5
| |||:|:|   ; CODE XREFS from fcn.06d96df8 @ 0x6d97000, 0x6d97018
| ``------> 0x06d9702c      b.ne  0x6d97050
|   |:|:|   0x06d97030      ldr   w7, [x5]
|   |:|:|   0x06d97034      mov   x8, 0x696c                           ; 'li'
|   |:|:|   0x06d97038      movk  x8, 0x756e, lsl 16
|   |:|:|   0x06d9703c      cmp   w7, w8
|  ,======< 0x06d97040      b.ne  0x6d97050
|  ||:|:|   0x06d97044      ldrb  w7, [x5, 4]                          ; [0x4:4]=0x10102
|  ||:|:|   0x06d97048      cmp   w7, 0x78                             ; 'x'
| ,=======< 0x06d9704c      b.eq  0x6d970b8
| -`------> 0x06d97050      ldr   x7, [arg_1a8h]                       ; [0x1a8:4]=0x799bde0 segment.DYNAMIC
| | |:|:|   0x06d97054      ldr   x7, [x7, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
| | |:|:|   0x06d97058      str   x7, [arg_1b0h]
| | |:|:|   0x06d9705c      add   x0, arg_48h
| | |:|:|   0x06d97060      adrp  x3, 0x4a09000
| | |:|:|   0x06d97064      add   x3, x3, 0x698
| | |:|:|   0x06d97068      orr   x4, xzr, 1
| | |:|:|   0x06d9706c      bl    0x4d7d150
| | |:|:|   0x06d97070      mov   x2, x0
| | |:|:|   0x06d97074      mov   x3, x1
| | |:|:|   0x06d97078      adrp  x0, 0x8330000
| | |:|:|   0x06d9707c      add   x0, x0, 0xd78
| | |:|:|   0x06d97080      ldr   x1, [arg_1b0h]                       ; [0x1b0:4]=0x220
| | |:|:|   0x06d97084      bl    0x4d18950
| | |:|:|   0x06d97088      ldr   x1, [x0]
| | |:|:|   0x06d9708c      add   x2, arg_3f8h
| | |:|:|   0x06d97090      stp   xzr, xzr, [x2]
| | |:|:|   0x06d97094      stp   xzr, xzr, [x2, 0x10]
| | |:|:|   0x06d97098      stp   xzr, xzr, [x2, 0x20]
| | |:|:|   0x06d9709c      stp   xzr, xzr, [x2, 0x30]
| | |:|:|   0x06d970a0      stp   xzr, xzr, [x2, 0x40]
| | |:|:|   0x06d970a4      stp   xzr, xzr, [x2, 0x50]
| | |:|:|   0x06d970a8      adrp  x0, 0x8333000
| | |:|:|   0x06d970ac      add   x0, x0, 0x198
| | |:|:|   0x06d970b0      bl    fcn.04d35800
| |,======< 0x06d970b4      b     0x6d971ec
| `-`-----> 0x06d970b8      ldrsw x0, [arg_498h]                       ; [0x498:4]=0x58585858
|  |,=====< 0x06d970bc      cbz   w0, 0x6d97130
|  ||:|:|   0x06d970c0      bl    0x5464120
|  ||:|:|   0x06d970c4      ldr   x2, [arg_1a8h]                       ; [0x1a8:4]=0x799bde0 segment.DYNAMIC
|  ||:|:|   0x06d970c8      ldr   x2, [x2, 0x28]                       ; [0x28:4]=0xace39d8
|  ||:|:|   0x06d970cc      mov   x3, x1
|  ||:|:|   0x06d970d0      mov   x1, x2
|  ||:|:|   0x06d970d4      mov   x2, x0
|  ||:|:|   0x06d970d8      adrp  x0, 0x8333000
|  ||:|:|   0x06d970dc      add   x0, x0, 0x4c8
|  ||:|:|   0x06d970e0      bl    0x4d18bb0
|  ||:|:|   0x06d970e4      ldp   x2, x3, [x0]
| ,=======< 0x06d970e8      tbz   w1, 0, 0x6d97130
| |||:|:|   0x06d970ec      ldr   x4, [arg_1a8h]                       ; [0x1a8:4]=0x799bde0 segment.DYNAMIC
| |||:|:|   0x06d970f0      ldr   x1, [x4, 0x20]                       ; [0x20:4]=64 ; "@"
| |||:|:|   0x06d970f4      adrp  x0, 0x8330000
| |||:|:|   0x06d970f8      add   x0, x0, 0xd78
| |||:|:|   0x06d970fc      bl    0x4d18950
| |||:|:|   0x06d97100      ldr   x1, [x0]
| |||:|:|   0x06d97104      add   x2, arg_3f8h
| |||:|:|   0x06d97108      stp   xzr, xzr, [x2]
| |||:|:|   0x06d9710c      stp   xzr, xzr, [x2, 0x10]
| |||:|:|   0x06d97110      stp   xzr, xzr, [x2, 0x20]
| |||:|:|   0x06d97114      stp   xzr, xzr, [x2, 0x30]
| |||:|:|   0x06d97118      stp   xzr, xzr, [x2, 0x40]
| |||:|:|   0x06d9711c      stp   xzr, xzr, [x2, 0x50]
| |||:|:|   0x06d97120      adrp  x0, 0x8333000
| |||:|:|   0x06d97124      add   x0, x0, 0x198
| |||:|:|   0x06d97128      bl    fcn.04d35800
| ========< 0x06d9712c      b     0x6d9714c
| `-`-----> 0x06d97130      ldr   x0, [arg_1a0h]                       ; [0x1a0:4]=0x799bde0 segment.DYNAMIC
|  | :|:|   0x06d97134      bl    0x6d96780
|  | :|:|   0x06d97138      ldp   fp, lr, [sp, -8]
|  | :|:|   0x06d9713c      add   sp, sp, 0x460
|  | :|:|   0x06d97140      ret
|  | :|:|   ; CODE XREFS from fcn.06d96df8 @ 0x6d9718c, 0x6d971e0
| .-.-----> 0x06d97144      add   x0, arg_3f8h
| :|::|:|   0x06d97148      bl    fcn.04d35870
| :|::|:|   ; CODE XREF from fcn.06d96df8 @ 0x6d9712c
| --------> 0x06d9714c      add   x27, arg_3f8h
| :|::|:|   0x06d97150      ldp   x4, x5, [x27]
| ========< 0x06d97154      cbz   x4, 0x6d97130
| :|::|:|   0x06d97158      ldrsb x27, [x5]
| :|::|:|   0x06d9715c      ldp   x2, x3, [x4]
| :|::|:|   0x06d97160      ldp   q0, q1, [x5]
| :|::|:|   0x06d97164      ldr   q2, [x5, 0x20]                       ; [0x20:4]=64 ; "@"
| :|::|:|   0x06d97168      ldr   x4, [x5]
| :|::|:|   0x06d9716c      add   x27, arg_98h
| :|::|:|   0x06d97170      stp   q0, q1, [x27]
| :|::|:|   0x06d97174      stur  q2, [arg_b8h]
| ========< 0x06d97178      cbnz  x4, 0x6d97190
| :|::|:|   0x06d9717c      adrp  x0, 0x8333000
| :|::|:|   0x06d97180      add   x0, x0, 0x198
| :|::|:|   0x06d97184      ldr   x1, [arg_1a0h]                       ; [0x1a0:4]=0x799bde0 segment.DYNAMIC
| :|::|:|   0x06d97188      bl    0x4d19300
| `=======< 0x06d9718c      b     0x6d97144
| --------> 0x06d97190      adrp  x0, 0x8333000
|  |::|:|   0x06d97194      add   x0, x0, 0x198
|  |::|:|   0x06d97198      ldr   x1, [arg_1a0h]                       ; [0x1a0:4]=0x799bde0 segment.DYNAMIC
|  |::|:|   0x06d9719c      bl    0x4d18e50
|  |::|:|   0x06d971a0      adrp  x27, 0xb48f000
|  |::|:|   0x06d971a4      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
| ,=======< 0x06d971a8      cbz   w4, 0x6d971cc
| ||::|:|   0x06d971ac      str   x0, [arg_1b8h]
| ||::|:|   0x06d971b0      ldrsb x27, [x0]
| ||::|:|   0x06d971b4      mov   x1, x0
| ||::|:|   0x06d971b8      add   x2, arg_98h
| ||::|:|   0x06d971bc      adrp  x0, 0x8c96000
| ||::|:|   0x06d971c0      add   x0, x0, 0xa98
| ||::|:|   0x06d971c4      bl    0x4d35930
| ||::|:|   0x06d971c8      ldr   x0, [arg_1b8h]                       ; [0x1b8:4]=0x220
| `-------> 0x06d971cc      add   x27, arg_98h
|  |::|:|   0x06d971d0      ldp   q0, q1, [x27]
|  |::|:|   0x06d971d4      ldur  q2, [arg_b8h]                        ; [0xb8:4]=0
|  |::|:|   0x06d971d8      stp   q0, q1, [x0]
|  |::|:|   0x06d971dc      str   q2, [x0, 0x20]
|  |`=====< 0x06d971e0      b     0x6d97144
|  | :|:|   ; CODE XREFS from fcn.06d96df8 @ 0x6d9722c, 0x6d97280
| .-.-----> 0x06d971e4      add   x0, arg_3f8h
| :|::|:|   0x06d971e8      bl    fcn.04d35870
| :|::|:|   ; CODE XREF from fcn.06d96df8 @ 0x6d970b4
| :`------> 0x06d971ec      add   x27, arg_3f8h
| : ::|:|   0x06d971f0      ldp   x4, x5, [x27]
| ========< 0x06d971f4      cbz   x4, 0x6d970b8
| : ::|:|   0x06d971f8      ldrsb x27, [x5]
| : ::|:|   0x06d971fc      ldp   x2, x3, [x4]
| : ::|:|   0x06d97200      ldp   q0, q1, [x5]
| : ::|:|   0x06d97204      ldr   q2, [x5, 0x20]                       ; [0x20:4]=64 ; "@"
| : ::|:|   0x06d97208      ldr   x4, [x5]
| : ::|:|   0x06d9720c      add   x27, arg_c8h
| : ::|:|   0x06d97210      stp   q0, q1, [x27]
| : ::|:|   0x06d97214      stur  q2, [arg_e8h]
| :,======< 0x06d97218      cbnz  x4, 0x6d97230
| :|::|:|   0x06d9721c      adrp  x0, 0x8333000
| :|::|:|   0x06d97220      add   x0, x0, 0x198
| :|::|:|   0x06d97224      ldr   x1, [arg_1a0h]                       ; [0x1a0:4]=0x799bde0 segment.DYNAMIC
| :|::|:|   0x06d97228      bl    0x4d19300
| `=======< 0x06d9722c      b     0x6d971e4
|  `------> 0x06d97230      adrp  x0, 0x8333000
|   ::|:|   0x06d97234      add   x0, x0, 0x198
|   ::|:|   0x06d97238      ldr   x1, [arg_1a0h]                       ; [0x1a0:4]=0x799bde0 segment.DYNAMIC
|   ::|:|   0x06d9723c      bl    0x4d18e50
|   ::|:|   0x06d97240      adrp  x27, 0xb48f000
|   ::|:|   0x06d97244      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|  ,======< 0x06d97248      cbz   w4, 0x6d9726c
|  |::|:|   0x06d9724c      str   x0, [arg_1b8h]
|  |::|:|   0x06d97250      ldrsb x27, [x0]
|  |::|:|   0x06d97254      mov   x1, x0
|  |::|:|   0x06d97258      add   x2, arg_c8h
|  |::|:|   0x06d9725c      adrp  x0, 0x8c96000
|  |::|:|   0x06d97260      add   x0, x0, 0xa98
|  |::|:|   0x06d97264      bl    0x4d35930
|  |::|:|   0x06d97268      ldr   x0, [arg_1b8h]                       ; [0x1b8:4]=0x220
|  `------> 0x06d9726c      add   x27, arg_c8h
|   ::|:|   0x06d97270      ldp   q0, q1, [x27]
|   ::|:|   0x06d97274      ldur  q2, [arg_e8h]                        ; [0xe8:4]=1
|   ::|:|   0x06d97278      stp   q0, q1, [x0]
|   ::|:|   0x06d9727c      str   q2, [x0, 0x20]
|   `=====< 0x06d97280      b     0x6d971e4
|    :|:|   ; CODE XREFS from fcn.06d96df8 @ 0x6d972d8, 0x6d97330
|  ..-----> 0x06d97284      add   x0, arg_3f8h
|  :::|:|   0x06d97288      bl    fcn.04d35870
|  :::|:|   0x06d9728c      ldr   x2, [arg_88h]                        ; [0x88:4]=0
|  :::|:|   0x06d97290      cmp   x2, 3
|  :::|:|   ; CODE XREF from fcn.06d96df8 @ 0x6d96ff0
|  :::`---> 0x06d97294      add   x27, arg_3f8h
|  ::: :|   0x06d97298      ldp   x7, x8, [x27]
|  :::,===< 0x06d9729c      cbz   x7, 0x6d97334
|  :::|:|   0x06d972a0      ldrsb x27, [x8]
|  :::|:|   0x06d972a4      ldp   x2, x3, [x7]
|  :::|:|   0x06d972a8      ldp   q0, q1, [x8]
|  :::|:|   0x06d972ac      ldr   q2, [x8, 0x20]                       ; [0x20:4]=64 ; "@"
|  :::|:|   0x06d972b0      ldr   x4, [x8]
|  :::|:|   0x06d972b4      add   x27, arg_f8h
|  :::|:|   0x06d972b8      stp   q0, q1, [x27]
|  :::|:|   0x06d972bc      add   x27, arg_118h
|  :::|:|   0x06d972c0      str   q2, [x27]
| ,=======< 0x06d972c4      cbnz  x4, 0x6d972dc
| |:::|:|   0x06d972c8      adrp  x0, 0x8333000
| |:::|:|   0x06d972cc      add   x0, x0, 0x198
| |:::|:|   0x06d972d0      ldr   x1, [arg_1a0h]                       ; [0x1a0:4]=0x799bde0 segment.DYNAMIC
| |:::|:|   0x06d972d4      bl    0x4d19300
| |`======< 0x06d972d8      b     0x6d97284
| `-------> 0x06d972dc      adrp  x0, 0x8333000
|   ::|:|   0x06d972e0      add   x0, x0, 0x198
|   ::|:|   0x06d972e4      ldr   x1, [arg_1a0h]                       ; [0x1a0:4]=0x799bde0 segment.DYNAMIC
|   ::|:|   0x06d972e8      bl    0x4d18e50
|   ::|:|   0x06d972ec      adrp  x27, 0xb48f000
|   ::|:|   0x06d972f0      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|  ,======< 0x06d972f4      cbz   w4, 0x6d97318
|  |::|:|   0x06d972f8      str   x0, [arg_1b8h]
|  |::|:|   0x06d972fc      ldrsb x27, [x0]
|  |::|:|   0x06d97300      mov   x1, x0
|  |::|:|   0x06d97304      add   x2, arg_f8h
|  |::|:|   0x06d97308      adrp  x0, 0x8c96000
|  |::|:|   0x06d9730c      add   x0, x0, 0xa98
|  |::|:|   0x06d97310      bl    0x4d35930
|  |::|:|   0x06d97314      ldr   x0, [arg_1b8h]                       ; [0x1b8:4]=0x220
|  `------> 0x06d97318      add   x27, arg_f8h
|   ::|:|   0x06d9731c      ldp   q0, q1, [x27]
|   ::|:|   0x06d97320      add   x27, arg_118h
|   ::|:|   0x06d97324      ldr   q2, [x27]
|   ::|:|   0x06d97328      stp   q0, q1, [x0]
|   ::|:|   0x06d9732c      str   q2, [x0, 0x20]
|   `=====< 0x06d97330      b     0x6d97284
|    :`---> 0x06d97334      ldr   x1, [arg_188h]                       ; [0x188:4]=1
|    : :|   0x06d97338      ldr   x2, [arg_88h]                        ; [0x88:4]=0
|    : :|   0x06d9733c      ldr   x5, [arg_198h]                       ; [0x198:4]=0x779bde0
|    : :|   0x06d97340      ldr   x6, [arg_90h]                        ; [0x90:4]=0
|    `====< 0x06d97344      b     0x6d96ff8
|      :|   ; CODE XREFS from fcn.06d96df8 @ 0x6d97394, 0x6d973ec
|    ..---> 0x06d97348      add   x0, arg_3f8h
|    :::|   0x06d9734c      bl    fcn.04d35870
|    :::|   ; CODE XREF from fcn.06d96df8 @ 0x6d96f64
|    :::`-> 0x06d97350      add   x27, arg_3f8h
|    :::    0x06d97354      ldp   x7, x8, [x27]
|    :::,=< 0x06d97358      cbz   x7, 0x6d973f0
|    :::|   0x06d9735c      ldrsb x27, [x8]
|    :::|   0x06d97360      ldp   x2, x3, [x7]
|    :::|   0x06d97364      ldp   q0, q1, [x8]
|    :::|   0x06d97368      ldr   q2, [x8, 0x20]                       ; [0x20:4]=64 ; "@"
|    :::|   0x06d9736c      ldr   x4, [x8]
|    :::|   0x06d97370      add   x27, arg_128h
|    :::|   0x06d97374      stp   q0, q1, [x27]
|    :::|   0x06d97378      add   x27, arg_148h
|    :::|   0x06d9737c      str   q2, [x27]
|   ,=====< 0x06d97380      cbnz  x4, 0x6d97398
|   |:::|   0x06d97384      adrp  x0, 0x8333000
|   |:::|   0x06d97388      add   x0, x0, 0x198
|   |:::|   0x06d9738c      ldr   x1, [arg_1a0h]                       ; [0x1a0:4]=0x799bde0 segment.DYNAMIC
|   |:::|   0x06d97390      bl    0x4d19300
|   |`====< 0x06d97394      b     0x6d97348
|   `-----> 0x06d97398      adrp  x0, 0x8333000
|     ::|   0x06d9739c      add   x0, x0, 0x198
|     ::|   0x06d973a0      ldr   x1, [arg_1a0h]                       ; [0x1a0:4]=0x799bde0 segment.DYNAMIC
|     ::|   0x06d973a4      bl    0x4d18e50
|     ::|   0x06d973a8      adrp  x27, 0xb48f000
|     ::|   0x06d973ac      ldr   w4, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d973b0      cbz   w4, 0x6d973d4
|    |::|   0x06d973b4      str   x0, [arg_1b8h]
|    |::|   0x06d973b8      ldrsb x27, [x0]
|    |::|   0x06d973bc      mov   x1, x0
|    |::|   0x06d973c0      add   x2, arg_128h
|    |::|   0x06d973c4      adrp  x0, 0x8c96000
|    |::|   0x06d973c8      add   x0, x0, 0xa98
|    |::|   0x06d973cc      bl    0x4d35930
|    |::|   0x06d973d0      ldr   x0, [arg_1b8h]                       ; [0x1b8:4]=0x220
|    `----> 0x06d973d4      add   x27, arg_128h
|     ::|   0x06d973d8      ldp   q0, q1, [x27]
|     ::|   0x06d973dc      add   x27, arg_148h
|     ::|   0x06d973e0      ldr   q2, [x27]
|     ::|   0x06d973e4      stp   q0, q1, [x0]
|     ::|   0x06d973e8      str   q2, [x0, 0x20]
|     `===< 0x06d973ec      b     0x6d97348
|      :`-> 0x06d973f0      ldr   x1, [arg_188h]                       ; [0x188:4]=1
|      :    0x06d973f4      ldr   x2, [arg_88h]                        ; [0x88:4]=0
\      `==< 0x06d973f8      b     0x6d96f68
==== google3_third_party_jetski_gateway_gateway.buildResponseFromAdminModelList
/ fcn.06d978e8(int64_t arg_30h);
|           ; arg int64_t arg_30h @ stack + 0x30
|           0x06d978e8      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d978ec      ldr   lr, [sp], 0lr                        ; [0x30:4]=0
\           0x06d978f0      ret
==== google3_third_party_jetski_gateway_gateway.effectivePort
/ fcn.06d98a28(int64_t arg3, int64_t arg_8h, int64_t arg_80h, int64_t arg_88h, int64_t arg_90h, int64_t arg_98h, int64_t arg_a0h, int64_t arg_a8h, int64_t arg_b0h, int64_t arg_b8h, int64_t arg_c0h, int64_t arg_c8h, int64_t arg_d0h, int64_t arg_d8h, int64_t arg_e0h, int64_t arg_e8h, int64_t arg_f0h, int64_t arg_f8h, int64_t arg_100h, int64_t arg_108h, int64_t arg_110h, int64_t arg_118h, int64_t arg_120h, int64_t arg_128h, int64_t arg_130h, int64_t arg_138h, int64_t arg_140h, int64_t arg_148h, int64_t arg_158h, int64_t arg_168h, int64_t arg_188h, int64_t arg_198h, int64_t arg_1b0h, int64_t arg_1b8h, int64_t arg_1c0h, int64_t arg_1c8h, int64_t arg_1d0h, int64_t arg_1d8h, int64_t arg_208h, int64_t arg_210h, int64_t arg_340h);
|    ::::   ; arg int64_t arg3 @ x2
|    ::::   ; arg int64_t arg_8h @ stack + 0x8
|    ::::   ; arg int64_t arg_80h @ stack + 0x80
|    ::::   ; arg int64_t arg_88h @ stack + 0x88
|    ::::   ; arg int64_t arg_90h @ stack + 0x90
|    ::::   ; arg int64_t arg_98h @ stack + 0x98
|    ::::   ; arg int64_t arg_a0h @ stack + 0xa0
|    ::::   ; arg int64_t arg_a8h @ stack + 0xa8
|    ::::   ; arg int64_t arg_b0h @ stack + 0xb0
|    ::::   ; arg int64_t arg_b8h @ stack + 0xb8
|    ::::   ; arg int64_t arg_c0h @ stack + 0xc0
|    ::::   ; arg int64_t arg_c8h @ stack + 0xc8
|    ::::   ; arg int64_t arg_d0h @ stack + 0xd0
|    ::::   ; arg int64_t arg_d8h @ stack + 0xd8
|    ::::   ; arg int64_t arg_e0h @ stack + 0xe0
|    ::::   ; arg int64_t arg_e8h @ stack + 0xe8
|    ::::   ; arg int64_t arg_f0h @ stack + 0xf0
|    ::::   ; arg int64_t arg_f8h @ stack + 0xf8
|    ::::   ; arg int64_t arg_100h @ stack + 0x100
|    ::::   ; arg int64_t arg_108h @ stack + 0x108
|    ::::   ; arg int64_t arg_110h @ stack + 0x110
|    ::::   ; arg int64_t arg_118h @ stack + 0x118
|    ::::   ; arg int64_t arg_120h @ stack + 0x120
|    ::::   ; arg int64_t arg_128h @ stack + 0x128
|    ::::   ; arg int64_t arg_130h @ stack + 0x130
|    ::::   ; arg int64_t arg_138h @ stack + 0x138
|    ::::   ; arg int64_t arg_140h @ stack + 0x140
|    ::::   ; arg int64_t arg_148h @ stack + 0x148
|    ::::   ; arg int64_t arg_158h @ stack + 0x158
|    ::::   ; arg int64_t arg_168h @ stack + 0x168
|    ::::   ; arg int64_t arg_188h @ stack + 0x188
|    ::::   ; arg int64_t arg_198h @ stack + 0x198
|    ::::   ; arg int64_t arg_1b0h @ stack + 0x1b0
|    ::::   ; arg int64_t arg_1b8h @ stack + 0x1b8
|    ::::   ; arg int64_t arg_1c0h @ stack + 0x1c0
|    ::::   ; arg int64_t arg_1c8h @ stack + 0x1c8
|    ::::   ; arg int64_t arg_1d0h @ stack + 0x1d0
|    ::::   ; arg int64_t arg_1d8h @ stack + 0x1d8
|    ::::   ; arg int64_t arg_208h @ stack + 0x208
|    ::::   ; arg int64_t arg_210h @ stack + 0x210
|    ::::   ; arg int64_t arg_340h @ stack + 0x340
|    ::::   0x06d98a28      mov   x10, x2                              ; arg3
|    ::::   0x06d98a2c      bl    0x4f6f980
|    `====< 0x06d98a30      b     0x6d98378
|     :::   0x06d98a34      str   x4, [arg_138h]
|     :::   0x06d98a38      str   x3, [arg_c8h]
|     :::   0x06d98a3c      orr   x0, xzr, 0x20
|     :::   0x06d98a40      adrp  x1, 0x81a8000
|     :::   0x06d98a44      add   x1, x1, 0xcf0
|     :::   0x06d98a48      orr   x2, xzr, 1
|     :::   0x06d98a4c      bl    fcn.04d32c00
|     :::   0x06d98a50      str   x0, [arg_1c0h]
|     :::   0x06d98a54      ldr   x0, [arg_f0h]                        ; [0xf0:4]=0x779b960
|     :::   0x06d98a58      ldr   x1, [arg_80h]                        ; [0x80:4]=0
|     :::   0x06d98a5c      bl    fcn.04d98880
|     :::   0x06d98a60      adrp  x3, 0x856f000
|     :::   0x06d98a64      add   x3, x3, 0x550
|     :::   0x06d98a68      ldr   x8, [arg_1c0h]                       ; [0x1c0:4]=8
|     :::   0x06d98a6c      str   x3, [x8]
|     :::   0x06d98a70      adrp  x27, 0xb48f000
|     :::   0x06d98a74      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d98a78      cbz   w3, 0x6d98a88
|    |:::   0x06d98a7c      ldr   x11, [x8, 8]                         ; [0x8:4]=0
|    |:::   0x06d98a80      bl    fcn.04da3500
|    |:::   0x06d98a84      stp   x0, x11, [x25]
|    `----> 0x06d98a88      str   x0, [x8, 8]
|     :::   0x06d98a8c      ldr   x0, [arg_c8h]                        ; [0xc8:4]=0
|    ,====< 0x06d98a90      cbz   x0, 0x6d98a9c
|    |:::   0x06d98a94      ldr   x3, [x0, 8]                          ; [0x8:4]=0
|   ,=====< 0x06d98a98      b     0x6d98aa0
|   |`----> 0x06d98a9c      mov   x3, x0
|   | :::   ; CODE XREF from fcn.06d98a28 @ 0x6d98a98
|   `-----> 0x06d98aa0      str   x3, [x8, 0x10]
|     :::   0x06d98aa4      adrp  x27, 0xb48f000
|     :::   0x06d98aa8      ldr   w11, [x27, 0x450]                    ; [0x450:4]=0x58585858
|    ,====< 0x06d98aac      cbnz  w11, 0x6d98ab8
|    |:::   0x06d98ab0      ldr   x12, [arg_138h]                      ; [0x138:4]=0x799b960 segment.GNU_RELRO
|   ,=====< 0x06d98ab4      b     0x6d98ac8
|   |`----> 0x06d98ab8      ldr   x11, [x8, 0x18]                      ; [0x18:4]=0x4d0f000 section..text
|   | :::   0x06d98abc      bl    fcn.04da3500
|   | :::   0x06d98ac0      ldr   x12, [arg_138h]                      ; [0x138:4]=0x799b960 segment.GNU_RELRO
|   | :::   0x06d98ac4      stp   x12, x11, [x25]
|   | :::   ; CODE XREF from fcn.06d98a28 @ 0x6d98ab4
|   `-----> 0x06d98ac8      str   x12, [x8, 0x18]
|     :::   0x06d98acc      nop
|     :::   0x06d98ad0      mov   x0, xzr
|     :::   0x06d98ad4      mov   x1, xzr
|     :::   0x06d98ad8      orr   x2, xzr, 2
|     :::   0x06d98adc      orr   x3, xzr, 2
|     :::   0x06d98ae0      mov   x4, xzr
|     :::   0x06d98ae4      mov   x5, xzr
|     :::   0x06d98ae8      adrp  x6, 0x43ee000
|     :::   0x06d98aec      add   x6, x6, 0x184
|     :::   0x06d98af0      orr   x7, xzr, 0x1f
|     :::   0x06d98af4      mov   x9, x2
|     :::   0x06d98af8      mov   x10, x2
|     :::   0x06d98afc      bl    0x4f6f980
|     `===< 0x06d98b00      b     0x6d98378
|      ::   0x06d98b04      str   x2, [arg_130h]
|      ::   0x06d98b08      str   x1, [arg_c0h]
|      ::   0x06d98b0c      orr   x0, xzr, 0x20
|      ::   0x06d98b10      adrp  x1, 0x81a8000
|      ::   0x06d98b14      add   x1, x1, 0xcf0
|      ::   0x06d98b18      orr   x2, xzr, 1
|      ::   0x06d98b1c      bl    fcn.04d32c00
|      ::   0x06d98b20      str   x0, [arg_1c0h]
|      ::   0x06d98b24      ldr   x0, [arg_90h]                        ; [0x90:4]=0
|      ::   0x06d98b28      bl    fcn.04d98810
|      ::   0x06d98b2c      adrp  x3, 0x856f000
|      ::   0x06d98b30      add   x3, x3, 0x790
|      ::   0x06d98b34      ldr   x8, [arg_1c0h]                       ; [0x1c0:4]=8
|      ::   0x06d98b38      str   x3, [x8]
|      ::   0x06d98b3c      adrp  x27, 0xb48f000
|      ::   0x06d98b40      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d98b44      cbz   w3, 0x6d98b54
|     |::   0x06d98b48      ldr   x11, [x8, 8]                         ; [0x8:4]=0
|     |::   0x06d98b4c      bl    fcn.04da3500
|     |::   0x06d98b50      stp   x0, x11, [x25]
|     `---> 0x06d98b54      str   x0, [x8, 8]
|      ::   0x06d98b58      ldr   x0, [arg_c0h]                        ; [0xc0:4]=0
|     ,===< 0x06d98b5c      cbz   x0, 0x6d98b68
|     |::   0x06d98b60      ldr   x1, [x0, 8]                          ; [0x8:4]=0
|    ,====< 0x06d98b64      b     0x6d98b6c
|    |`---> 0x06d98b68      mov   x1, x0
|    | ::   ; CODE XREF from fcn.06d98a28 @ 0x6d98b64
|    `----> 0x06d98b6c      str   x1, [x8, 0x10]
|      ::   0x06d98b70      adrp  x27, 0xb48f000
|      ::   0x06d98b74      ldr   w11, [x27, 0x450]                    ; [0x450:4]=0x58585858
|     ,===< 0x06d98b78      cbnz  w11, 0x6d98b84
|     |::   0x06d98b7c      ldr   x12, [arg_130h]                      ; [0x130:4]=0x799b960 segment.GNU_RELRO
|    ,====< 0x06d98b80      b     0x6d98b94
|    |`---> 0x06d98b84      ldr   x11, [x8, 0x18]                      ; [0x18:4]=0x4d0f000 section..text
|    | ::   0x06d98b88      bl    fcn.04da3500
|    | ::   0x06d98b8c      ldr   x12, [arg_130h]                      ; [0x130:4]=0x799b960 segment.GNU_RELRO
|    | ::   0x06d98b90      stp   x12, x11, [x25]
|    | ::   ; CODE XREF from fcn.06d98a28 @ 0x6d98b80
|    `----> 0x06d98b94      str   x12, [x8, 0x18]
|      ::   0x06d98b98      nop
|      ::   0x06d98b9c      mov   x0, xzr
|      ::   0x06d98ba0      mov   x1, xzr
|      ::   0x06d98ba4      orr   x2, xzr, 2
|      ::   0x06d98ba8      orr   x3, xzr, 2
|      ::   0x06d98bac      mov   x4, xzr
|      ::   0x06d98bb0      mov   x5, xzr
|      ::   0x06d98bb4      adrp  x6, 0x443e000
|      ::   0x06d98bb8      add   x6, x6, 0xd07
|      ::   0x06d98bbc      mov   x7, 0x2c                             ; ','
|      ::   0x06d98bc0      mov   x9, x2
|      ::   0x06d98bc4      mov   x10, x2
|      ::   0x06d98bc8      bl    0x4f6f980
|      `==< 0x06d98bcc      b     0x6d98378
|       :   0x06d98bd0      ldp   fp, lr, [sp, -8]
|       :   0x06d98bd4      add   sp, sp, 0x200
|       :   0x06d98bd8      ret
|       :   0x06d98bdc      bl    fcn.04da37d0
|       :   0x06d98be0      nop
|       :   0x06d98be4      stp   x0, x1, [arg_8h]
|       :   0x06d98be8      mov   x3, lr
|       :   0x06d98bec      bl    fcn.04da15a0
|       :   0x06d98bf0      ldp   x0, x1, [arg_8h]
|       `=< 0x06d98bf4      b     0x6d981d0
==== google3_third_party_jetski_gateway_gateway.shouldAttachGatewayHeaders
/ fcn.06d98a28(int64_t arg3, int64_t arg_8h, int64_t arg_80h, int64_t arg_88h, int64_t arg_90h, int64_t arg_98h, int64_t arg_a0h, int64_t arg_a8h, int64_t arg_b0h, int64_t arg_b8h, int64_t arg_c0h, int64_t arg_c8h, int64_t arg_d0h, int64_t arg_d8h, int64_t arg_e0h, int64_t arg_e8h, int64_t arg_f0h, int64_t arg_f8h, int64_t arg_100h, int64_t arg_108h, int64_t arg_110h, int64_t arg_118h, int64_t arg_120h, int64_t arg_128h, int64_t arg_130h, int64_t arg_138h, int64_t arg_140h, int64_t arg_148h, int64_t arg_158h, int64_t arg_168h, int64_t arg_188h, int64_t arg_198h, int64_t arg_1b0h, int64_t arg_1b8h, int64_t arg_1c0h, int64_t arg_1c8h, int64_t arg_1d0h, int64_t arg_1d8h, int64_t arg_208h, int64_t arg_210h, int64_t arg_340h);
|    ::::   ; arg int64_t arg3 @ x2
|    ::::   ; arg int64_t arg_8h @ stack + 0x8
|    ::::   ; arg int64_t arg_80h @ stack + 0x80
|    ::::   ; arg int64_t arg_88h @ stack + 0x88
|    ::::   ; arg int64_t arg_90h @ stack + 0x90
|    ::::   ; arg int64_t arg_98h @ stack + 0x98
|    ::::   ; arg int64_t arg_a0h @ stack + 0xa0
|    ::::   ; arg int64_t arg_a8h @ stack + 0xa8
|    ::::   ; arg int64_t arg_b0h @ stack + 0xb0
|    ::::   ; arg int64_t arg_b8h @ stack + 0xb8
|    ::::   ; arg int64_t arg_c0h @ stack + 0xc0
|    ::::   ; arg int64_t arg_c8h @ stack + 0xc8
|    ::::   ; arg int64_t arg_d0h @ stack + 0xd0
|    ::::   ; arg int64_t arg_d8h @ stack + 0xd8
|    ::::   ; arg int64_t arg_e0h @ stack + 0xe0
|    ::::   ; arg int64_t arg_e8h @ stack + 0xe8
|    ::::   ; arg int64_t arg_f0h @ stack + 0xf0
|    ::::   ; arg int64_t arg_f8h @ stack + 0xf8
|    ::::   ; arg int64_t arg_100h @ stack + 0x100
|    ::::   ; arg int64_t arg_108h @ stack + 0x108
|    ::::   ; arg int64_t arg_110h @ stack + 0x110
|    ::::   ; arg int64_t arg_118h @ stack + 0x118
|    ::::   ; arg int64_t arg_120h @ stack + 0x120
|    ::::   ; arg int64_t arg_128h @ stack + 0x128
|    ::::   ; arg int64_t arg_130h @ stack + 0x130
|    ::::   ; arg int64_t arg_138h @ stack + 0x138
|    ::::   ; arg int64_t arg_140h @ stack + 0x140
|    ::::   ; arg int64_t arg_148h @ stack + 0x148
|    ::::   ; arg int64_t arg_158h @ stack + 0x158
|    ::::   ; arg int64_t arg_168h @ stack + 0x168
|    ::::   ; arg int64_t arg_188h @ stack + 0x188
|    ::::   ; arg int64_t arg_198h @ stack + 0x198
|    ::::   ; arg int64_t arg_1b0h @ stack + 0x1b0
|    ::::   ; arg int64_t arg_1b8h @ stack + 0x1b8
|    ::::   ; arg int64_t arg_1c0h @ stack + 0x1c0
|    ::::   ; arg int64_t arg_1c8h @ stack + 0x1c8
|    ::::   ; arg int64_t arg_1d0h @ stack + 0x1d0
|    ::::   ; arg int64_t arg_1d8h @ stack + 0x1d8
|    ::::   ; arg int64_t arg_208h @ stack + 0x208
|    ::::   ; arg int64_t arg_210h @ stack + 0x210
|    ::::   ; arg int64_t arg_340h @ stack + 0x340
|    ::::   0x06d98a28      mov   x10, x2                              ; arg3
|    ::::   0x06d98a2c      bl    0x4f6f980
|    `====< 0x06d98a30      b     0x6d98378
|     :::   0x06d98a34      str   x4, [arg_138h]
|     :::   0x06d98a38      str   x3, [arg_c8h]
|     :::   0x06d98a3c      orr   x0, xzr, 0x20
|     :::   0x06d98a40      adrp  x1, 0x81a8000
|     :::   0x06d98a44      add   x1, x1, 0xcf0
|     :::   0x06d98a48      orr   x2, xzr, 1
|     :::   0x06d98a4c      bl    fcn.04d32c00
|     :::   0x06d98a50      str   x0, [arg_1c0h]
|     :::   0x06d98a54      ldr   x0, [arg_f0h]                        ; [0xf0:4]=0x779b960
|     :::   0x06d98a58      ldr   x1, [arg_80h]                        ; [0x80:4]=0
|     :::   0x06d98a5c      bl    fcn.04d98880
|     :::   0x06d98a60      adrp  x3, 0x856f000
|     :::   0x06d98a64      add   x3, x3, 0x550
|     :::   0x06d98a68      ldr   x8, [arg_1c0h]                       ; [0x1c0:4]=8
|     :::   0x06d98a6c      str   x3, [x8]
|     :::   0x06d98a70      adrp  x27, 0xb48f000
|     :::   0x06d98a74      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d98a78      cbz   w3, 0x6d98a88
|    |:::   0x06d98a7c      ldr   x11, [x8, 8]                         ; [0x8:4]=0
|    |:::   0x06d98a80      bl    fcn.04da3500
|    |:::   0x06d98a84      stp   x0, x11, [x25]
|    `----> 0x06d98a88      str   x0, [x8, 8]
|     :::   0x06d98a8c      ldr   x0, [arg_c8h]                        ; [0xc8:4]=0
|    ,====< 0x06d98a90      cbz   x0, 0x6d98a9c
|    |:::   0x06d98a94      ldr   x3, [x0, 8]                          ; [0x8:4]=0
|   ,=====< 0x06d98a98      b     0x6d98aa0
|   |`----> 0x06d98a9c      mov   x3, x0
|   | :::   ; CODE XREF from fcn.06d98a28 @ 0x6d98a98
|   `-----> 0x06d98aa0      str   x3, [x8, 0x10]
|     :::   0x06d98aa4      adrp  x27, 0xb48f000
|     :::   0x06d98aa8      ldr   w11, [x27, 0x450]                    ; [0x450:4]=0x58585858
|    ,====< 0x06d98aac      cbnz  w11, 0x6d98ab8
|    |:::   0x06d98ab0      ldr   x12, [arg_138h]                      ; [0x138:4]=0x799b960 segment.GNU_RELRO
|   ,=====< 0x06d98ab4      b     0x6d98ac8
|   |`----> 0x06d98ab8      ldr   x11, [x8, 0x18]                      ; [0x18:4]=0x4d0f000 section..text
|   | :::   0x06d98abc      bl    fcn.04da3500
|   | :::   0x06d98ac0      ldr   x12, [arg_138h]                      ; [0x138:4]=0x799b960 segment.GNU_RELRO
|   | :::   0x06d98ac4      stp   x12, x11, [x25]
|   | :::   ; CODE XREF from fcn.06d98a28 @ 0x6d98ab4
|   `-----> 0x06d98ac8      str   x12, [x8, 0x18]
|     :::   0x06d98acc      nop
|     :::   0x06d98ad0      mov   x0, xzr
|     :::   0x06d98ad4      mov   x1, xzr
|     :::   0x06d98ad8      orr   x2, xzr, 2
|     :::   0x06d98adc      orr   x3, xzr, 2
|     :::   0x06d98ae0      mov   x4, xzr
|     :::   0x06d98ae4      mov   x5, xzr
|     :::   0x06d98ae8      adrp  x6, 0x43ee000
|     :::   0x06d98aec      add   x6, x6, 0x184
|     :::   0x06d98af0      orr   x7, xzr, 0x1f
|     :::   0x06d98af4      mov   x9, x2
|     :::   0x06d98af8      mov   x10, x2
|     :::   0x06d98afc      bl    0x4f6f980
|     `===< 0x06d98b00      b     0x6d98378
|      ::   0x06d98b04      str   x2, [arg_130h]
|      ::   0x06d98b08      str   x1, [arg_c0h]
|      ::   0x06d98b0c      orr   x0, xzr, 0x20
|      ::   0x06d98b10      adrp  x1, 0x81a8000
|      ::   0x06d98b14      add   x1, x1, 0xcf0
|      ::   0x06d98b18      orr   x2, xzr, 1
|      ::   0x06d98b1c      bl    fcn.04d32c00
|      ::   0x06d98b20      str   x0, [arg_1c0h]
|      ::   0x06d98b24      ldr   x0, [arg_90h]                        ; [0x90:4]=0
|      ::   0x06d98b28      bl    fcn.04d98810
|      ::   0x06d98b2c      adrp  x3, 0x856f000
|      ::   0x06d98b30      add   x3, x3, 0x790
|      ::   0x06d98b34      ldr   x8, [arg_1c0h]                       ; [0x1c0:4]=8
|      ::   0x06d98b38      str   x3, [x8]
|      ::   0x06d98b3c      adrp  x27, 0xb48f000
|      ::   0x06d98b40      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|     ,===< 0x06d98b44      cbz   w3, 0x6d98b54
|     |::   0x06d98b48      ldr   x11, [x8, 8]                         ; [0x8:4]=0
|     |::   0x06d98b4c      bl    fcn.04da3500
|     |::   0x06d98b50      stp   x0, x11, [x25]
|     `---> 0x06d98b54      str   x0, [x8, 8]
|      ::   0x06d98b58      ldr   x0, [arg_c0h]                        ; [0xc0:4]=0
|     ,===< 0x06d98b5c      cbz   x0, 0x6d98b68
|     |::   0x06d98b60      ldr   x1, [x0, 8]                          ; [0x8:4]=0
|    ,====< 0x06d98b64      b     0x6d98b6c
|    |`---> 0x06d98b68      mov   x1, x0
|    | ::   ; CODE XREF from fcn.06d98a28 @ 0x6d98b64
|    `----> 0x06d98b6c      str   x1, [x8, 0x10]
|      ::   0x06d98b70      adrp  x27, 0xb48f000
|      ::   0x06d98b74      ldr   w11, [x27, 0x450]                    ; [0x450:4]=0x58585858
|     ,===< 0x06d98b78      cbnz  w11, 0x6d98b84
|     |::   0x06d98b7c      ldr   x12, [arg_130h]                      ; [0x130:4]=0x799b960 segment.GNU_RELRO
|    ,====< 0x06d98b80      b     0x6d98b94
|    |`---> 0x06d98b84      ldr   x11, [x8, 0x18]                      ; [0x18:4]=0x4d0f000 section..text
|    | ::   0x06d98b88      bl    fcn.04da3500
|    | ::   0x06d98b8c      ldr   x12, [arg_130h]                      ; [0x130:4]=0x799b960 segment.GNU_RELRO
|    | ::   0x06d98b90      stp   x12, x11, [x25]
|    | ::   ; CODE XREF from fcn.06d98a28 @ 0x6d98b80
|    `----> 0x06d98b94      str   x12, [x8, 0x18]
|      ::   0x06d98b98      nop
|      ::   0x06d98b9c      mov   x0, xzr
|      ::   0x06d98ba0      mov   x1, xzr
|      ::   0x06d98ba4      orr   x2, xzr, 2
|      ::   0x06d98ba8      orr   x3, xzr, 2
|      ::   0x06d98bac      mov   x4, xzr
|      ::   0x06d98bb0      mov   x5, xzr
|      ::   0x06d98bb4      adrp  x6, 0x443e000
|      ::   0x06d98bb8      add   x6, x6, 0xd07
|      ::   0x06d98bbc      mov   x7, 0x2c                             ; ','
|      ::   0x06d98bc0      mov   x9, x2
|      ::   0x06d98bc4      mov   x10, x2
|      ::   0x06d98bc8      bl    0x4f6f980
|      `==< 0x06d98bcc      b     0x6d98378
|       :   0x06d98bd0      ldp   fp, lr, [sp, -8]
|       :   0x06d98bd4      add   sp, sp, 0x200
|       :   0x06d98bd8      ret
|       :   0x06d98bdc      bl    fcn.04da37d0
|       :   0x06d98be0      nop
|       :   0x06d98be4      stp   x0, x1, [arg_8h]
|       :   0x06d98be8      mov   x3, lr
|       :   0x06d98bec      bl    fcn.04da15a0
|       :   0x06d98bf0      ldp   x0, x1, [arg_8h]
|       `=< 0x06d98bf4      b     0x6d981d0
==== AuthProvider_.fetchRemoteModelCatalog
/ fcn.06d98c68(int64_t arg4, int64_t arg_28h, int64_t arg_40h);
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg_28h @ stack + 0x28
|           ; arg int64_t arg_40h @ stack + 0x40
|           0x06d98c68      add   x3, x3, 0xb38                        ; arg4
|           0x06d98c6c      bl    fcn.05116df0
|       ,=< 0x06d98c70      tbnz  w0, 0, 0x6d98c8c
|       |   0x06d98c74      adrp  x27, 0x948d000
|       |   0x06d98c78      add   x27, x27, 0xde0
|       |   0x06d98c7c      ldp   x2, x3, [x27]
|       |   0x06d98c80      ldp   x0, x1, [arg_28h]
|       |   0x06d98c84      bl    fcn.05116df0
|      ,==< 0x06d98c88      tbz   w0, 0, 0x6d98ca4
|      |`-> 0x06d98c8c      mov   x0, xzr
|      |    0x06d98c90      mov   x1, xzr
|      |    0x06d98c94      mov   x2, xzr
|      |    0x06d98c98      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |    0x06d98c9c      ldr   lr, [sp], 0x40                       ; segment.PHDR
|      |                                                               ; [0x40:4]=6 ; "\U00000006"
|      |    0x06d98ca0      ret
|      `--> 0x06d98ca4      mov   x0, xzr
|           0x06d98ca8      ldp   x1, x2, [arg_28h]
|           0x06d98cac      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d98cb0      ldr   lr, [sp], 0x40                       ; segment.PHDR
|                                                                      ; [0x40:4]=6 ; "\U00000006"
\           0x06d98cb4      ret
==== AuthProvider_.fetchGatewayDiscoveredModels
/ fcn.06d994a8(int64_t arg1, int64_t arg2, int64_t arg4, int64_t arg_40h, int64_t arg_48h, int64_t arg_70h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg2 @ x1
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg_40h @ stack + 0x40
|           ; arg int64_t arg_48h @ stack + 0x48
|           ; arg int64_t arg_70h @ stack + 0x70
|       ,=< 0x06d994a8      b     0x6d9951c
..
|   ||  |   ; CODE XREF from fcn.06d994a8 @ 0x6d994a8
|   ``-,`-> 0x06d9951c      cbz   x3, 0x6d99574
|      |    0x06d99520      str   x0, [arg_48h]                        ; arg1
|      |    0x06d99524      str   x3, [sp, 0x30]                       ; arg4
|      |    0x06d99528      mov   x0, x1                               ; arg2
|      |    0x06d9952c      bl    fcn.06d99f50
|      |    0x06d99530      mov   x2, x1
|      |    0x06d99534      mov   x1, x0
|      |    0x06d99538      ldr   x0, [arg_48h]                        ; [0x48:4]=64
|      |    0x06d9953c      bl    fcn.06d99710
|      |,=< 0x06d99540      tbz   w0, 0, 0x6d9955c
|      ||   0x06d99544      ldr   x0, [arg_70h]                        ; [0x30:4]=0
|      ||   0x06d99548      mov   x1, xzr
|      ||   0x06d9954c      mov   x2, xzr
|      ||   0x06d99550      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      ||   0x06d99554      ldr   lr, [sp], 0x40                       ; segment.PHDR
|      ||                                                              ; [0x40:4]=6 ; "\U00000006"
|      ||   0x06d99558      ret
|      |`-> 0x06d9955c      mov   x0, xzr
|      |    0x06d99560      mov   x1, xzr
|      |    0x06d99564      mov   x2, xzr
|      |    0x06d99568      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |    0x06d9956c      ldr   lr, [sp], 0x40                       ; segment.PHDR
|      |                                                               ; [0x40:4]=6 ; "\U00000006"
|      |    0x06d99570      ret
|      `--> 0x06d99574      mov   x0, xzr
|           0x06d99578      mov   x1, xzr
|           0x06d9957c      mov   x2, xzr
|           0x06d99580      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d99584      ldr   lr, [sp], 0x40                       ; segment.PHDR
|                                                                      ; [0x40:4]=6 ; "\U00000006"
\           0x06d99588      ret
==== google3_third_party_jetski_gateway_gateway.buildDefaultGatewayCatalog
/ fcn.06d9aad8(int64_t arg2);
|           ; arg int64_t arg2 @ x1
|           0x06d9aad8      add   x1, x1, 0x890                        ; arg2
|           0x06d9aadc      orr   x2, xzr, 1
|           0x06d9aae0      bl    fcn.04d32500
|           0x06d9aae4      adrp  x3, 0x4481000
|           0x06d9aae8      add   x3, x3, 0xd12
|           0x06d9aaec      orr   x4, xzr, 0x3f
|           0x06d9aaf0      stp   x3, x4, [x0]
|           0x06d9aaf4      mov   x1, x0
|           0x06d9aaf8      adrp  x0, 0x8f07000
|           0x06d9aafc      add   x0, x0, 0x508
|           0x06d9ab00      ldp   fp, lr, [sp, -8]
|           0x06d9ab04      add   sp, sp, 0x100
\           0x06d9ab08      ret
==== AuthProvider_.AdminSettings.deferwrap1
/ fcn.06d9aba8(int64_t arg3, int64_t arg4, int64_t arg_70h);
|           ; arg int64_t arg3 @ x2
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg_70h @ stack + 0x70
|           0x06d9aba8      stp   x2, x3, [arg_70h + 0x10]             ; arg4
|           0x06d9abac      adrp  x3, 0x79a4000
|           0x06d9abb0      add   x3, x3, 0x230
|           0x06d9abb4      stp   x2, x3, [arg_70h + 0x20]             ; arg3
|           0x06d9abb8      adrp  x0, 0x446d000
|           0x06d9abbc      add   x0, x0, 0x284
|           0x06d9abc0      orr   x1, xzr, 0x38
|           0x06d9abc4      add   x2, arg_70h
|           0x06d9abc8      orr   x3, xzr, 3
|           0x06d9abcc      mov   x4, x3
|           0x06d9abd0      bl    fcn.04e321f0
|       ,=< 0x06d9abd4      cbnz  x0, 0x6d9ac0c
|       |   0x06d9abd8      nop
|       |   0x06d9abdc      orr   x0, xzr, 0x10
|       |   0x06d9abe0      adrp  x1, 0x8757000
|       |   0x06d9abe4      add   x1, x1, 0x890
|       |   0x06d9abe8      orr   x2, xzr, 1
|       |   0x06d9abec      bl    fcn.04d32500
|       |   0x06d9abf0      adrp  x3, 0x446d000
|       |   0x06d9abf4      add   x3, x3, 0x284
|       |   0x06d9abf8      orr   x4, xzr, 0x38
|       |   0x06d9abfc      stp   x3, x4, [x0]
|       |   0x06d9ac00      mov   x1, x0
|       |   0x06d9ac04      adrp  x0, 0x8f07000
|       |   0x06d9ac08      add   x0, x0, 0x508
|       `-> 0x06d9ac0c      ldp   fp, lr, [sp, -8]
|           0x06d9ac10      add   sp, sp, 0x100
\           0x06d9ac14      ret
==== AuthProvider_.setLastDiscoveredCatalog.deferwrap1
/ fcn.06d9ac28(int64_t arg_a0h);
|           ; arg int64_t arg_a0h @ stack + 0xa0
|           0x06d9ac28      bl    fcn.04d98880
|           0x06d9ac2c      adrp  x2, 0x856f000
|           0x06d9ac30      add   x2, x2, 0x550
|           0x06d9ac34      stp   x2, x0, [arg_a0h]
|           0x06d9ac38      adrp  x0, 0x44a9000
|           0x06d9ac3c      add   x0, x0, 0x8c9
|           0x06d9ac40      mov   x1, 0x59                             ; 'Y'
|           0x06d9ac44      add   x2, arg_a0h
|           0x06d9ac48      orr   x3, xzr, 1
|           0x06d9ac4c      mov   x4, x3
|           0x06d9ac50      bl    fcn.04e321f0
|       ,=< 0x06d9ac54      cbnz  x0, 0x6d9ac8c
|       |   0x06d9ac58      nop
|       |   0x06d9ac5c      orr   x0, xzr, 0x10
|       |   0x06d9ac60      adrp  x1, 0x8757000
|       |   0x06d9ac64      add   x1, x1, 0x890
|       |   0x06d9ac68      orr   x2, xzr, 1
|       |   0x06d9ac6c      bl    fcn.04d32500
|       |   0x06d9ac70      adrp  x3, 0x44a9000
|       |   0x06d9ac74      add   x3, x3, 0x8c9
|       |   0x06d9ac78      mov   x4, 0x59                             ; 'Y'
|       |   0x06d9ac7c      stp   x3, x4, [x0]
|       |   0x06d9ac80      mov   x1, x0
|       |   0x06d9ac84      adrp  x0, 0x8f07000
|       |   0x06d9ac88      add   x0, x0, 0x508
|       `-> 0x06d9ac8c      ldp   fp, lr, [sp, -8]
|           0x06d9ac90      add   sp, sp, 0x100
\           0x06d9ac94      ret
==== AuthProvider_.getLastDiscoveredCatalog.deferwrap1
/ fcn.06d9ac28(int64_t arg_a0h);
|           ; arg int64_t arg_a0h @ stack + 0xa0
|           0x06d9ac28      bl    fcn.04d98880
|           0x06d9ac2c      adrp  x2, 0x856f000
|           0x06d9ac30      add   x2, x2, 0x550
|           0x06d9ac34      stp   x2, x0, [arg_a0h]
|           0x06d9ac38      adrp  x0, 0x44a9000
|           0x06d9ac3c      add   x0, x0, 0x8c9
|           0x06d9ac40      mov   x1, 0x59                             ; 'Y'
|           0x06d9ac44      add   x2, arg_a0h
|           0x06d9ac48      orr   x3, xzr, 1
|           0x06d9ac4c      mov   x4, x3
|           0x06d9ac50      bl    fcn.04e321f0
|       ,=< 0x06d9ac54      cbnz  x0, 0x6d9ac8c
|       |   0x06d9ac58      nop
|       |   0x06d9ac5c      orr   x0, xzr, 0x10
|       |   0x06d9ac60      adrp  x1, 0x8757000
|       |   0x06d9ac64      add   x1, x1, 0x890
|       |   0x06d9ac68      orr   x2, xzr, 1
|       |   0x06d9ac6c      bl    fcn.04d32500
|       |   0x06d9ac70      adrp  x3, 0x44a9000
|       |   0x06d9ac74      add   x3, x3, 0x8c9
|       |   0x06d9ac78      mov   x4, 0x59                             ; 'Y'
|       |   0x06d9ac7c      stp   x3, x4, [x0]
|       |   0x06d9ac80      mov   x1, x0
|       |   0x06d9ac84      adrp  x0, 0x8f07000
|       |   0x06d9ac88      add   x0, x0, 0x508
|       `-> 0x06d9ac8c      ldp   fp, lr, [sp, -8]
|           0x06d9ac90      add   sp, sp, 0x100
\           0x06d9ac94      ret
==== AuthProvider_.GetHTTPClient.deferwrap1
/ fcn.06d9ace8(int64_t arg1, int64_t arg_d0h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg_d0h @ stack + 0xd0
|           0x06d9ace8      add   x0, x0, 0x203                        ; arg1
|           0x06d9acec      mov   x1, 0x1d
|           0x06d9acf0      add   x2, arg_d0h
|           0x06d9acf4      orr   x3, xzr, 2
|           0x06d9acf8      mov   x4, x3
|           0x06d9acfc      bl    fcn.04e321f0
|       ,=< 0x06d9ad00      cbnz  x0, 0x6d9ad38
|       |   0x06d9ad04      nop
|       |   0x06d9ad08      orr   x0, xzr, 0x10
|       |   0x06d9ad0c      adrp  x1, 0x8757000
|       |   0x06d9ad10      add   x1, x1, 0x890
|       |   0x06d9ad14      orr   x2, xzr, 1
|       |   0x06d9ad18      bl    fcn.04d32500
|       |   0x06d9ad1c      adrp  x3, 0x43de000
|       |   0x06d9ad20      add   x3, x3, 0x203
|       |   0x06d9ad24      mov   x4, 0x1d
|       |   0x06d9ad28      stp   x3, x4, [x0]
|       |   0x06d9ad2c      mov   x1, x0
|       |   0x06d9ad30      adrp  x0, 0x8f07000
|       |   0x06d9ad34      add   x0, x0, 0x508
|       `-> 0x06d9ad38      ldp   fp, lr, [sp, -8]
|           0x06d9ad3c      add   sp, sp, 0x100
\           0x06d9ad40      ret
==== AuthProvider_.GetCACertPool.deferwrap1
/ fcn.06d9ad68(int64_t arg1);
|           ; arg int64_t arg1 @ x0
|           0x06d9ad68      mov   x1, x0                               ; arg1
|           0x06d9ad6c      adrp  x0, 0x8f07000
|           0x06d9ad70      add   x0, x0, 0x508
|           0x06d9ad74      ldp   fp, lr, [sp, -8]
|           0x06d9ad78      add   sp, sp, 0x100
\           0x06d9ad7c      ret
==== AuthProvider_.GetCACertPath.deferwrap1
/ fcn.06d9ade8(int64_t arg3, int64_t arg_38h, int64_t arg_48h, int64_t arg_60h, int64_t arg_f0h);
|       :   ; arg int64_t arg3 @ x2
|       :   ; arg int64_t arg_38h @ stack + 0x38
|       :   ; arg int64_t arg_48h @ stack + 0x48
|       :   ; arg int64_t arg_60h @ stack + 0x60
|       :   ; arg int64_t arg_f0h @ stack + 0xf0
|       :   0x06d9ade8      ldp   x0, x1, [x2]                         ; arg3
|       :   0x06d9adec      bl    fcn.04e00e20
|       `=< 0x06d9adf0      cbnz  x1, 0x6d9adc4
|           0x06d9adf4      stp   xzr, xzr, [arg_60h]
|           0x06d9adf8      ldr   x0, [arg_38h]                        ; [0x38:4]=0x400010
|           0x06d9adfc      bl    fcn.04d98810
|           0x06d9ae00      adrp  x1, 0x856f000
|           0x06d9ae04      add   x1, x1, 0x790
|           0x06d9ae08      stp   x1, x0, [arg_60h]
|           0x06d9ae0c      adrp  x0, 0x4413000
|           0x06d9ae10      add   x0, x0, 0xba5
|           0x06d9ae14      mov   x1, 0x24                             ; '$'
|           0x06d9ae18      add   x2, arg_60h
|           0x06d9ae1c      orr   x3, xzr, 1
|           0x06d9ae20      mov   x4, x3
|           0x06d9ae24      bl    fcn.04e321f0
|       ,=< 0x06d9ae28      cbnz  x0, 0x6d9ae60
|       |   0x06d9ae2c      nop
|       |   0x06d9ae30      orr   x0, xzr, 0x10
|       |   0x06d9ae34      adrp  x1, 0x8757000
|       |   0x06d9ae38      add   x1, x1, 0x890
|       |   0x06d9ae3c      orr   x2, xzr, 1
|       |   0x06d9ae40      bl    fcn.04d32500
|       |   0x06d9ae44      adrp  x3, 0x4413000
|       |   0x06d9ae48      add   x3, x3, 0xba5
|       |   0x06d9ae4c      mov   x4, 0x24                             ; '$'
|       |   0x06d9ae50      stp   x3, x4, [x0]
|       |   0x06d9ae54      mov   x1, x0
|       |   0x06d9ae58      adrp  x0, 0x8f07000
|       |   0x06d9ae5c      add   x0, x0, 0x508
|       `-> 0x06d9ae60      ldp   fp, lr, [sp, -8]
|           0x06d9ae64      add   sp, sp, 0x100
|           0x06d9ae68      ret
==== AuthProvider_.GetProxyURL.deferwrap1
/ fcn.06d9ade8(int64_t arg3, int64_t arg_38h, int64_t arg_48h, int64_t arg_60h, int64_t arg_f0h);
|       :   ; arg int64_t arg3 @ x2
|       :   ; arg int64_t arg_38h @ stack + 0x38
|       :   ; arg int64_t arg_48h @ stack + 0x48
|       :   ; arg int64_t arg_60h @ stack + 0x60
|       :   ; arg int64_t arg_f0h @ stack + 0xf0
|       :   0x06d9ade8      ldp   x0, x1, [x2]                         ; arg3
|       :   0x06d9adec      bl    fcn.04e00e20
|       `=< 0x06d9adf0      cbnz  x1, 0x6d9adc4
|           0x06d9adf4      stp   xzr, xzr, [arg_60h]
|           0x06d9adf8      ldr   x0, [arg_38h]                        ; [0x38:4]=0x400010
|           0x06d9adfc      bl    fcn.04d98810
|           0x06d9ae00      adrp  x1, 0x856f000
|           0x06d9ae04      add   x1, x1, 0x790
|           0x06d9ae08      stp   x1, x0, [arg_60h]
|           0x06d9ae0c      adrp  x0, 0x4413000
|           0x06d9ae10      add   x0, x0, 0xba5
|           0x06d9ae14      mov   x1, 0x24                             ; '$'
|           0x06d9ae18      add   x2, arg_60h
|           0x06d9ae1c      orr   x3, xzr, 1
|           0x06d9ae20      mov   x4, x3
|           0x06d9ae24      bl    fcn.04e321f0
|       ,=< 0x06d9ae28      cbnz  x0, 0x6d9ae60
|       |   0x06d9ae2c      nop
|       |   0x06d9ae30      orr   x0, xzr, 0x10
|       |   0x06d9ae34      adrp  x1, 0x8757000
|       |   0x06d9ae38      add   x1, x1, 0x890
|       |   0x06d9ae3c      orr   x2, xzr, 1
|       |   0x06d9ae40      bl    fcn.04d32500
|       |   0x06d9ae44      adrp  x3, 0x4413000
|       |   0x06d9ae48      add   x3, x3, 0xba5
|       |   0x06d9ae4c      mov   x4, 0x24                             ; '$'
|       |   0x06d9ae50      stp   x3, x4, [x0]
|       |   0x06d9ae54      mov   x1, x0
|       |   0x06d9ae58      adrp  x0, 0x8f07000
|       |   0x06d9ae5c      add   x0, x0, 0x508
|       `-> 0x06d9ae60      ldp   fp, lr, [sp, -8]
|           0x06d9ae64      add   sp, sp, 0x100
|           0x06d9ae68      ret
==== AuthProvider_.GetProxyURLString.deferwrap1
/ fcn.06d9aee8(int64_t arg1, int64_t arg4, int64_t arg5, int64_t arg_50h, int64_t arg_58h, int64_t arg_60h, int64_t arg_68h, int64_t arg_70h, int64_t arg_80h, int64_t arg_90h, int64_t arg_a0h, int64_t arg_a8h, int64_t arg_b0h, int64_t arg_b8h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg5 @ x4
|           ; arg int64_t arg_50h @ stack + 0x50
|           ; arg int64_t arg_58h @ stack + 0x58
|           ; arg int64_t arg_60h @ stack + 0x60
|           ; arg int64_t arg_68h @ stack + 0x68
|           ; arg int64_t arg_70h @ stack + 0x70
|           ; arg int64_t arg_80h @ stack + 0x80
|           ; arg int64_t arg_90h @ stack + 0x90
|           ; arg int64_t arg_a0h @ stack + 0xa0
|           ; arg int64_t arg_a8h @ stack + 0xa8
|           ; arg int64_t arg_b0h @ stack + 0xb0
|           ; arg int64_t arg_b8h @ stack + 0xb8
|           0x06d9aee8      add   x4, x4, 0xfe0                        ; arg5
|           0x06d9aeec      stp   x3, x4, [arg_80h]                    ; arg5
|           0x06d9aef0      ldr   x3, [arg_60h]                        ; [0x60:4]=0x380
|           0x06d9aef4      cmp   x3, 0
|           0x06d9aef8      cset  x3, ne
|           0x06d9aefc      eor   x3, x3, 1
|           0x06d9af00      tst   w3, 1                                ; arg1
|           0x06d9af04      ldr   x3, [arg_58h]                        ; [0x58:4]=64
|           0x06d9af08      ldr   x4, [arg_b8h]                        ; [0xb8:4]=0
|           0x06d9af0c      csel  x2, x3, x4, ne
|           0x06d9af10      ldr   x3, [arg_50h]                        ; [0x50:4]=64
|           0x06d9af14      ldr   x4, [arg_b0h]                        ; [0xb0:4]=0x6474e551
|           0x06d9af18      csel  x1, x3, x4, ne
|           0x06d9af1c      ldr   x3, [arg_68h]                        ; [0x68:4]=0x380
|           0x06d9af20      ldr   x4, [arg_a8h]                        ; [0xa8:4]=0x200000
|           0x06d9af24      csel  x3, x3, x4, ne
|           0x06d9af28      mov   x4, x0                               ; arg1
|           0x06d9af2c      add   x5, arg_80h
|           0x06d9af30      orr   x6, xzr, 1
|           0x06d9af34      mov   x7, x6
|           0x06d9af38      mov   x0, x3
|           0x06d9af3c      adrp  x3, 0x7f44000
|           0x06d9af40      add   x3, x3, 0xd88
|           0x06d9af44      bl    0x4f32ac0
|       ,=< 0x06d9af48      cbz   x0, 0x6d9afc8
|       |   0x06d9af4c      stp   xzr, xzr, [arg_70h]
|      ,==< 0x06d9af50      cbz   x0, 0x6d9af58
|      ||   0x06d9af54      ldr   x0, [x0, 8]                          ; [0x8:4]=0
|      `--> 0x06d9af58      stp   x0, x1, [arg_70h]
|       |   0x06d9af5c      adrp  x0, 0x4425000
|       |   0x06d9af60      add   x0, x0, 0xa97
|       |   0x06d9af64      mov   x1, 0x27                             ; '''
|       |   0x06d9af68      add   x2, arg_70h
|       |   0x06d9af6c      orr   x3, xzr, 1
|       |   0x06d9af70      mov   x4, x3
|       |   0x06d9af74      bl    fcn.04e321f0
|      ,==< 0x06d9af78      cbnz  x0, 0x6d9afb0
|      ||   0x06d9af7c      nop
|      ||   0x06d9af80      orr   x0, xzr, 0x10
|      ||   0x06d9af84      adrp  x1, 0x8757000
|      ||   0x06d9af88      add   x1, x1, 0x890
|      ||   0x06d9af8c      orr   x2, xzr, 1
|      ||   0x06d9af90      bl    fcn.04d32500
|      ||   0x06d9af94      adrp  x3, 0x4425000
|      ||   0x06d9af98      add   x3, x3, 0xa97
|      ||   0x06d9af9c      mov   x4, 0x27                             ; '''
|      ||   0x06d9afa0      stp   x3, x4, [x0]
|      ||   0x06d9afa4      mov   x1, x0
|      ||   0x06d9afa8      adrp  x0, 0x8f07000
|      ||   0x06d9afac      add   x0, x0, 0x508
|      `--> 0x06d9afb0      mov   x2, x1
|       |   0x06d9afb4      mov   x1, x0
|       |   0x06d9afb8      mov   x0, xzr
|       |   0x06d9afbc      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|       |   0x06d9afc0      ldr   lr, [sp], 0xa0                       ; [0xa0:4]=0x779b960
|       |   0x06d9afc4      ret
|       `-> 0x06d9afc8      ldr   x0, [arg_90h]                        ; [0x90:4]=0
|           0x06d9afcc      mov   x1, xzr
|           0x06d9afd0      mov   x2, xzr
|           0x06d9afd4      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d9afd8      ldr   lr, [sp], 0xa0                       ; [0xa0:4]=0x779b960
\           0x06d9afdc      ret
==== AuthProvider_.SetHTTPClient.deferwrap1
/ fcn.06d9aee8(int64_t arg1, int64_t arg4, int64_t arg5, int64_t arg_50h, int64_t arg_58h, int64_t arg_60h, int64_t arg_68h, int64_t arg_70h, int64_t arg_80h, int64_t arg_90h, int64_t arg_a0h, int64_t arg_a8h, int64_t arg_b0h, int64_t arg_b8h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg5 @ x4
|           ; arg int64_t arg_50h @ stack + 0x50
|           ; arg int64_t arg_58h @ stack + 0x58
|           ; arg int64_t arg_60h @ stack + 0x60
|           ; arg int64_t arg_68h @ stack + 0x68
|           ; arg int64_t arg_70h @ stack + 0x70
|           ; arg int64_t arg_80h @ stack + 0x80
|           ; arg int64_t arg_90h @ stack + 0x90
|           ; arg int64_t arg_a0h @ stack + 0xa0
|           ; arg int64_t arg_a8h @ stack + 0xa8
|           ; arg int64_t arg_b0h @ stack + 0xb0
|           ; arg int64_t arg_b8h @ stack + 0xb8
|           0x06d9aee8      add   x4, x4, 0xfe0                        ; arg5
|           0x06d9aeec      stp   x3, x4, [arg_80h]                    ; arg5
|           0x06d9aef0      ldr   x3, [arg_60h]                        ; [0x60:4]=0x380
|           0x06d9aef4      cmp   x3, 0
|           0x06d9aef8      cset  x3, ne
|           0x06d9aefc      eor   x3, x3, 1
|           0x06d9af00      tst   w3, 1                                ; arg1
|           0x06d9af04      ldr   x3, [arg_58h]                        ; [0x58:4]=64
|           0x06d9af08      ldr   x4, [arg_b8h]                        ; [0xb8:4]=0
|           0x06d9af0c      csel  x2, x3, x4, ne
|           0x06d9af10      ldr   x3, [arg_50h]                        ; [0x50:4]=64
|           0x06d9af14      ldr   x4, [arg_b0h]                        ; [0xb0:4]=0x6474e551
|           0x06d9af18      csel  x1, x3, x4, ne
|           0x06d9af1c      ldr   x3, [arg_68h]                        ; [0x68:4]=0x380
|           0x06d9af20      ldr   x4, [arg_a8h]                        ; [0xa8:4]=0x200000
|           0x06d9af24      csel  x3, x3, x4, ne
|           0x06d9af28      mov   x4, x0                               ; arg1
|           0x06d9af2c      add   x5, arg_80h
|           0x06d9af30      orr   x6, xzr, 1
|           0x06d9af34      mov   x7, x6
|           0x06d9af38      mov   x0, x3
|           0x06d9af3c      adrp  x3, 0x7f44000
|           0x06d9af40      add   x3, x3, 0xd88
|           0x06d9af44      bl    0x4f32ac0
|       ,=< 0x06d9af48      cbz   x0, 0x6d9afc8
|       |   0x06d9af4c      stp   xzr, xzr, [arg_70h]
|      ,==< 0x06d9af50      cbz   x0, 0x6d9af58
|      ||   0x06d9af54      ldr   x0, [x0, 8]                          ; [0x8:4]=0
|      `--> 0x06d9af58      stp   x0, x1, [arg_70h]
|       |   0x06d9af5c      adrp  x0, 0x4425000
|       |   0x06d9af60      add   x0, x0, 0xa97
|       |   0x06d9af64      mov   x1, 0x27                             ; '''
|       |   0x06d9af68      add   x2, arg_70h
|       |   0x06d9af6c      orr   x3, xzr, 1
|       |   0x06d9af70      mov   x4, x3
|       |   0x06d9af74      bl    fcn.04e321f0
|      ,==< 0x06d9af78      cbnz  x0, 0x6d9afb0
|      ||   0x06d9af7c      nop
|      ||   0x06d9af80      orr   x0, xzr, 0x10
|      ||   0x06d9af84      adrp  x1, 0x8757000
|      ||   0x06d9af88      add   x1, x1, 0x890
|      ||   0x06d9af8c      orr   x2, xzr, 1
|      ||   0x06d9af90      bl    fcn.04d32500
|      ||   0x06d9af94      adrp  x3, 0x4425000
|      ||   0x06d9af98      add   x3, x3, 0xa97
|      ||   0x06d9af9c      mov   x4, 0x27                             ; '''
|      ||   0x06d9afa0      stp   x3, x4, [x0]
|      ||   0x06d9afa4      mov   x1, x0
|      ||   0x06d9afa8      adrp  x0, 0x8f07000
|      ||   0x06d9afac      add   x0, x0, 0x508
|      `--> 0x06d9afb0      mov   x2, x1
|       |   0x06d9afb4      mov   x1, x0
|       |   0x06d9afb8      mov   x0, xzr
|       |   0x06d9afbc      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|       |   0x06d9afc0      ldr   lr, [sp], 0xa0                       ; [0xa0:4]=0x779b960
|       |   0x06d9afc4      ret
|       `-> 0x06d9afc8      ldr   x0, [arg_90h]                        ; [0x90:4]=0
|           0x06d9afcc      mov   x1, xzr
|           0x06d9afd0      mov   x2, xzr
|           0x06d9afd4      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d9afd8      ldr   lr, [sp], 0xa0                       ; [0xa0:4]=0x779b960
\           0x06d9afdc      ret
==== AuthProvider_.GetEndpointURL.deferwrap1
/ fcn.06d9aee8(int64_t arg1, int64_t arg4, int64_t arg5, int64_t arg_50h, int64_t arg_58h, int64_t arg_60h, int64_t arg_68h, int64_t arg_70h, int64_t arg_80h, int64_t arg_90h, int64_t arg_a0h, int64_t arg_a8h, int64_t arg_b0h, int64_t arg_b8h);
|           ; arg int64_t arg1 @ x0
|           ; arg int64_t arg4 @ x3
|           ; arg int64_t arg5 @ x4
|           ; arg int64_t arg_50h @ stack + 0x50
|           ; arg int64_t arg_58h @ stack + 0x58
|           ; arg int64_t arg_60h @ stack + 0x60
|           ; arg int64_t arg_68h @ stack + 0x68
|           ; arg int64_t arg_70h @ stack + 0x70
|           ; arg int64_t arg_80h @ stack + 0x80
|           ; arg int64_t arg_90h @ stack + 0x90
|           ; arg int64_t arg_a0h @ stack + 0xa0
|           ; arg int64_t arg_a8h @ stack + 0xa8
|           ; arg int64_t arg_b0h @ stack + 0xb0
|           ; arg int64_t arg_b8h @ stack + 0xb8
|           0x06d9aee8      add   x4, x4, 0xfe0                        ; arg5
|           0x06d9aeec      stp   x3, x4, [arg_80h]                    ; arg5
|           0x06d9aef0      ldr   x3, [arg_60h]                        ; [0x60:4]=0x380
|           0x06d9aef4      cmp   x3, 0
|           0x06d9aef8      cset  x3, ne
|           0x06d9aefc      eor   x3, x3, 1
|           0x06d9af00      tst   w3, 1                                ; arg1
|           0x06d9af04      ldr   x3, [arg_58h]                        ; [0x58:4]=64
|           0x06d9af08      ldr   x4, [arg_b8h]                        ; [0xb8:4]=0
|           0x06d9af0c      csel  x2, x3, x4, ne
|           0x06d9af10      ldr   x3, [arg_50h]                        ; [0x50:4]=64
|           0x06d9af14      ldr   x4, [arg_b0h]                        ; [0xb0:4]=0x6474e551
|           0x06d9af18      csel  x1, x3, x4, ne
|           0x06d9af1c      ldr   x3, [arg_68h]                        ; [0x68:4]=0x380
|           0x06d9af20      ldr   x4, [arg_a8h]                        ; [0xa8:4]=0x200000
|           0x06d9af24      csel  x3, x3, x4, ne
|           0x06d9af28      mov   x4, x0                               ; arg1
|           0x06d9af2c      add   x5, arg_80h
|           0x06d9af30      orr   x6, xzr, 1
|           0x06d9af34      mov   x7, x6
|           0x06d9af38      mov   x0, x3
|           0x06d9af3c      adrp  x3, 0x7f44000
|           0x06d9af40      add   x3, x3, 0xd88
|           0x06d9af44      bl    0x4f32ac0
|       ,=< 0x06d9af48      cbz   x0, 0x6d9afc8
|       |   0x06d9af4c      stp   xzr, xzr, [arg_70h]
|      ,==< 0x06d9af50      cbz   x0, 0x6d9af58
|      ||   0x06d9af54      ldr   x0, [x0, 8]                          ; [0x8:4]=0
|      `--> 0x06d9af58      stp   x0, x1, [arg_70h]
|       |   0x06d9af5c      adrp  x0, 0x4425000
|       |   0x06d9af60      add   x0, x0, 0xa97
|       |   0x06d9af64      mov   x1, 0x27                             ; '''
|       |   0x06d9af68      add   x2, arg_70h
|       |   0x06d9af6c      orr   x3, xzr, 1
|       |   0x06d9af70      mov   x4, x3
|       |   0x06d9af74      bl    fcn.04e321f0
|      ,==< 0x06d9af78      cbnz  x0, 0x6d9afb0
|      ||   0x06d9af7c      nop
|      ||   0x06d9af80      orr   x0, xzr, 0x10
|      ||   0x06d9af84      adrp  x1, 0x8757000
|      ||   0x06d9af88      add   x1, x1, 0x890
|      ||   0x06d9af8c      orr   x2, xzr, 1
|      ||   0x06d9af90      bl    fcn.04d32500
|      ||   0x06d9af94      adrp  x3, 0x4425000
|      ||   0x06d9af98      add   x3, x3, 0xa97
|      ||   0x06d9af9c      mov   x4, 0x27                             ; '''
|      ||   0x06d9afa0      stp   x3, x4, [x0]
|      ||   0x06d9afa4      mov   x1, x0
|      ||   0x06d9afa8      adrp  x0, 0x8f07000
|      ||   0x06d9afac      add   x0, x0, 0x508
|      `--> 0x06d9afb0      mov   x2, x1
|       |   0x06d9afb4      mov   x1, x0
|       |   0x06d9afb8      mov   x0, xzr
|       |   0x06d9afbc      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|       |   0x06d9afc0      ldr   lr, [sp], 0xa0                       ; [0xa0:4]=0x779b960
|       |   0x06d9afc4      ret
|       `-> 0x06d9afc8      ldr   x0, [arg_90h]                        ; [0x90:4]=0
|           0x06d9afcc      mov   x1, xzr
|           0x06d9afd0      mov   x2, xzr
|           0x06d9afd4      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|           0x06d9afd8      ldr   lr, [sp], 0xa0                       ; [0xa0:4]=0x779b960
\           0x06d9afdc      ret
==== AuthProvider_.SetEndpointURL.deferwrap1
/ fcn.06d9b028(int64_t arg1, int64_t arg_8h, int64_t arg_48h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; var int64_t var_29h @ stack - 0x29
|       :   ; var int64_t var_28h @ stack - 0x28
|       :   ; var int64_t var_20h @ stack - 0x20
|       :   ; var int64_t var_18h @ stack - 0x18
|       :   ; var int64_t var_10h @ stack - 0x10
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_48h @ stack + 0x48
|      ,==< 0x06d9b028      b.ls  0x6d9b2cc
|      |:   0x06d9b02c      str   lr, [sp, -0x60]!
|      |:   0x06d9b030      stur  fp, [sp, -8]
|      |:   0x06d9b034      sub   fp, sp, 8
|     ,===< 0x06d9b038      cbnz  x0, 0x6d9b044
|     ||:   0x06d9b03c      adrp  x0, 0x8f91000
|     ||:   0x06d9b040      add   x0, x0, 0x498
|     `---> 0x06d9b044      str   x0, [var_20h]
|      |:   0x06d9b048      mov   x0, 0x28                             ; '('
|      |:   0x06d9b04c      adrp  x1, 0x8ac1000
|      |:   0x06d9b050      add   x1, x1, 0xd30
|      |:   0x06d9b054      orr   x2, xzr, 1
|      |:   0x06d9b058      bl    fcn.04d32f40
|      |:   0x06d9b05c      str   x0, [var_18h]
|      |:   0x06d9b060      mov   x0, 0x48                             ; 'H'
|      |:   0x06d9b064      adrp  x1, 0x8c53000
|      |:   0x06d9b068      add   x1, x1, 0x128
|      |:   0x06d9b06c      orr   x2, xzr, 1
|      |:   0x06d9b070      bl    0x4d33650
|      |:   0x06d9b074      str   x0, [var_10h]
|      |:   0x06d9b078      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b07c      ldr   x3, [x26]
|      |:   0x06d9b080      adrp  x0, 0x4358000
|      |:   0x06d9b084      add   x0, x0, 0xd36
|      |:   0x06d9b088      mov   x1, 0x13
|      |:   0x06d9b08c      blr   x3
|      |:   0x06d9b090      str   x1, [var_28h]
|     ,===< 0x06d9b094      cbz   x1, 0x6d9b0f8
|     ||:   0x06d9b098      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b09c      str   x1, [x2, 0x10]
|     ||:   0x06d9b0a0      adrp  x27, 0xb48f000
|     ||:   0x06d9b0a4      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b0a8      cbz   w3, 0x6d9b0b8
|    |||:   0x06d9b0ac      ldr   x3, [x2, 8]                          ; [0x8:4]=0
|    |||:   0x06d9b0b0      bl    fcn.04da3500
|    |||:   0x06d9b0b4      stp   x0, x3, [x25]
|    `----> 0x06d9b0b8      str   x0, [x2, 8]
|     ||:   0x06d9b0bc      adrp  x0, 0x856f000
|     ||:   0x06d9b0c0      add   x0, x0, 0x950
|     ||:   0x06d9b0c4      bl    fcn.04d31c10
|     ||:   0x06d9b0c8      orr   x1, xzr, 1
|     ||:   0x06d9b0cc      strb  w1, [x0]
|     ||:   0x06d9b0d0      adrp  x27, 0xb48f000
|     ||:   0x06d9b0d4      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b0d8      cbnz  w1, 0x6d9b0e4
|    |||:   0x06d9b0dc      ldr   x2, [var_10h]                        ; [0x50:4]=64
|   ,=====< 0x06d9b0e0      b     0x6d9b0f4
|   |`----> 0x06d9b0e4      ldr   x2, [var_10h]                        ; [0x50:4]=64
|   | ||:   0x06d9b0e8      ldr   x3, [x2]
|   | ||:   0x06d9b0ec      bl    fcn.04da3500
|   | ||:   0x06d9b0f0      stp   x0, x3, [x25]
|   | ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b0e0
|   `-----> 0x06d9b0f4      str   x0, [x2]
|     `---> 0x06d9b0f8      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b0fc      ldr   x2, [x26]
|      |:   0x06d9b100      adrp  x0, 0x43de000
|      |:   0x06d9b104      add   x0, x0, 0x220
|      |:   0x06d9b108      mov   x1, 0x1d
|      |:   0x06d9b10c      blr   x2
|     ,===< 0x06d9b110      cbz   x1, 0x6d9b138
|     ||:   0x06d9b114      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b118      str   x1, [x2, 0x20]
|     ||:   0x06d9b11c      adrp  x27, 0xb48f000
|     ||:   0x06d9b120      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b124      cbz   w3, 0x6d9b134
|    |||:   0x06d9b128      ldr   x3, [x2, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|    |||:   0x06d9b12c      bl    fcn.04da3500
|    |||:   0x06d9b130      stp   x0, x3, [x25]
|    `----> 0x06d9b134      str   x0, [x2, 0x18]
|     `---> 0x06d9b138      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b13c      ldr   x2, [x26]
|      |:   0x06d9b140      ldr   x3, [var_28h]                        ; [0x38:4]=0x400010
|      |:   0x06d9b144      cmp   x3, 0
|      |:   0x06d9b148      cset  x3, ne
|      |:   0x06d9b14c      cmp   x1, 0
|      |:   0x06d9b150      cset  x4, ne
|      |:   0x06d9b154      orr   x3, x4, x3
|      |:   0x06d9b158      strb  w3, [var_29h]
|      |:   0x06d9b15c      adrp  x0, 0x439b000
|      |:   0x06d9b160      add   x0, x0, 0x515
|      |:   0x06d9b164      mov   x1, 0x17
|      |:   0x06d9b168      blr   x2
|     ,===< 0x06d9b16c      cbz   x1, 0x6d9b194
|     ||:   0x06d9b170      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b174      str   x1, [x2, 0x30]
|     ||:   0x06d9b178      adrp  x27, 0xb48f000
|     ||:   0x06d9b17c      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b180      cbz   w3, 0x6d9b190
|    |||:   0x06d9b184      ldr   x3, [x2, 0x28]                       ; [0x28:4]=0xace39d8
|    |||:   0x06d9b188      bl    fcn.04da3500
|    |||:   0x06d9b18c      stp   x0, x3, [x25]
|    `----> 0x06d9b190      str   x0, [x2, 0x28]
|     `---> 0x06d9b194      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b198      ldr   x2, [x26]
|      |:   0x06d9b19c      cmp   x1, 0
|      |:   0x06d9b1a0      cset  x3, ne
|      |:   0x06d9b1a4      ldrb  w4, [var_29h]                        ; [0x37:4]=0x40001000
|      |:   0x06d9b1a8      orr   x3, x4, x3
|      |:   0x06d9b1ac      strb  w3, [var_29h]
|      |:   0x06d9b1b0      adrp  x0, 0x439b000
|      |:   0x06d9b1b4      add   x0, x0, 0x52c
|      |:   0x06d9b1b8      mov   x1, 0x17
|      |:   0x06d9b1bc      blr   x2
|     ,===< 0x06d9b1c0      cbnz  x1, 0x6d9b1d0
|     ||:   0x06d9b1c4      ldr   x3, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b1c8      ldrb  w2, [var_29h]                        ; [0x37:4]=0x40001000
|    ,====< 0x06d9b1cc      b     0x6d9b22c
|    |`---> 0x06d9b1d0      bl    0x6d9cd90
|    |,===< 0x06d9b1d4      cbz   x0, 0x6d9b1e0
|    |||:   0x06d9b1d8      ldr   x2, [x0]
|   ,=====< 0x06d9b1dc      b     0x6d9b1e4
|   ||`---> 0x06d9b1e0      mov   x2, xzr
|   || |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1dc
|   `-----> 0x06d9b1e4      cmp   x2, 0
|    |,===< 0x06d9b1e8      b.le  0x6d9b21c
|    |||:   0x06d9b1ec      adrp  x27, 0xb48f000
|    |||:   0x06d9b1f0      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d9b1f4      cbnz  w3, 0x6d9b200
|   ||||:   0x06d9b1f8      ldr   x3, [var_10h]                        ; [0x50:4]=64
|  ,======< 0x06d9b1fc      b     0x6d9b214
|  |`-----> 0x06d9b200      ldr   x3, [var_10h]                        ; [0x50:4]=64
|  | |||:   0x06d9b204      ldr   x4, [x3, 0x38]                       ; [0x38:4]=0x400010
|  | |||:   0x06d9b208      bl    fcn.04da3500
|  | |||:   0x06d9b20c      stp   x0, x4, [x25]
|  | |||:   0x06d9b210      cmp   x2, 0
|  | |||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1fc
|  `------> 0x06d9b214      str   x0, [x3, 0x38]
|   ,=====< 0x06d9b218      b     0x6d9b220
|   ||`---> 0x06d9b21c      ldr   x3, [var_10h]                        ; [0x50:4]=64
|   || |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b218
|   `-----> 0x06d9b220      cset  x2, gt
|    | |:   0x06d9b224      ldrb  w4, [var_29h]                        ; [0x37:4]=0x40001000
|    | |:   0x06d9b228      orr   x2, x2, x4
|    | |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1cc
|    `,===< 0x06d9b22c      tbz   w2, 0, 0x6d9b258
|     ||:   0x06d9b230      adrp  x27, 0xb48f000
|     ||:   0x06d9b234      ldr   w2, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b238      cbnz  w2, 0x6d9b244
|    |||:   0x06d9b23c      ldr   x2, [var_18h]                        ; [0x48:4]=64
|   ,=====< 0x06d9b240      b     0x6d9b254
|   |`----> 0x06d9b244      ldr   x2, [var_18h]                        ; [0x48:4]=64
|   | ||:   0x06d9b248      ldr   x4, [x2, 0x10]                       ; [0x10:4]=0xb70003
|   | ||:   0x06d9b24c      bl    fcn.04da3500
|   | ||:   0x06d9b250      stp   x3, x4, [x25]
|   | ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b240
|   `-----> 0x06d9b254      str   x3, [x2, 0x10]
|     `---> 0x06d9b258      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b25c      ldr   x2, [x26]
|      |:   0x06d9b260      adrp  x0, 0x439b000
|      |:   0x06d9b264      add   x0, x0, 0x543
|      |:   0x06d9b268      mov   x1, 0x17
|      |:   0x06d9b26c      blr   x2
|     ,===< 0x06d9b270      cbnz  x1, 0x6d9b28c
|     ||:   0x06d9b274      ldr   x26, [var_20h]                       ; segment.PHDR
|     ||:                                                              ; [0x40:4]=6 ; "\U00000006"
|     ||:   0x06d9b278      ldr   x2, [x26]
|     ||:   0x06d9b27c      adrp  x0, 0x42ec000
|     ||:   0x06d9b280      add   x0, x0, 0xb4b
|     ||:   0x06d9b284      mov   x1, 0xd
|     ||:   0x06d9b288      blr   x2
|    ,`---> 0x06d9b28c      cbz   x1, 0x6d9b2b8
|    | |:   0x06d9b290      ldr   x2, [arg_48h]                        ; [0x48:4]=64
|    | |:   0x06d9b294      str   x1, [x2, 8]
|    | |:   0x06d9b298      adrp  x27, 0xb48f000
|    | |:   0x06d9b29c      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    |,===< 0x06d9b2a0      cbz   w1, 0x6d9b2b0
|    |||:   0x06d9b2a4      ldr   x1, [x2]
|    |||:   0x06d9b2a8      bl    fcn.04da3500
|    |||:   0x06d9b2ac      stp   x0, x1, [x25]
|    |`---> 0x06d9b2b0      str   x0, [x2]
|    |,===< 0x06d9b2b4      b     0x6d9b2bc
|    `----> 0x06d9b2b8      ldr   x2, [var_18h]                        ; [0x48:4]=64
|     ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b2b4
|     `---> 0x06d9b2bc      mov   x0, x2
|      |:   0x06d9b2c0      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |:   0x06d9b2c4      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|      |:   0x06d9b2c8      ret
|      `--> 0x06d9b2cc      str   x0, [arg_8h]                         ; arg1
|       :   0x06d9b2d0      mov   x3, lr
|       :   0x06d9b2d4      bl    fcn.04da15a0
|       :   0x06d9b2d8      ldr   x0, [arg_8h]                         ; [0x8:4]=0
|       `=< 0x06d9b2dc      b     0x6d9b020
==== AuthProvider_.GetWireProtocol.deferwrap1
/ fcn.06d9b028(int64_t arg1, int64_t arg_8h, int64_t arg_48h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; var int64_t var_29h @ stack - 0x29
|       :   ; var int64_t var_28h @ stack - 0x28
|       :   ; var int64_t var_20h @ stack - 0x20
|       :   ; var int64_t var_18h @ stack - 0x18
|       :   ; var int64_t var_10h @ stack - 0x10
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_48h @ stack + 0x48
|      ,==< 0x06d9b028      b.ls  0x6d9b2cc
|      |:   0x06d9b02c      str   lr, [sp, -0x60]!
|      |:   0x06d9b030      stur  fp, [sp, -8]
|      |:   0x06d9b034      sub   fp, sp, 8
|     ,===< 0x06d9b038      cbnz  x0, 0x6d9b044
|     ||:   0x06d9b03c      adrp  x0, 0x8f91000
|     ||:   0x06d9b040      add   x0, x0, 0x498
|     `---> 0x06d9b044      str   x0, [var_20h]
|      |:   0x06d9b048      mov   x0, 0x28                             ; '('
|      |:   0x06d9b04c      adrp  x1, 0x8ac1000
|      |:   0x06d9b050      add   x1, x1, 0xd30
|      |:   0x06d9b054      orr   x2, xzr, 1
|      |:   0x06d9b058      bl    fcn.04d32f40
|      |:   0x06d9b05c      str   x0, [var_18h]
|      |:   0x06d9b060      mov   x0, 0x48                             ; 'H'
|      |:   0x06d9b064      adrp  x1, 0x8c53000
|      |:   0x06d9b068      add   x1, x1, 0x128
|      |:   0x06d9b06c      orr   x2, xzr, 1
|      |:   0x06d9b070      bl    0x4d33650
|      |:   0x06d9b074      str   x0, [var_10h]
|      |:   0x06d9b078      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b07c      ldr   x3, [x26]
|      |:   0x06d9b080      adrp  x0, 0x4358000
|      |:   0x06d9b084      add   x0, x0, 0xd36
|      |:   0x06d9b088      mov   x1, 0x13
|      |:   0x06d9b08c      blr   x3
|      |:   0x06d9b090      str   x1, [var_28h]
|     ,===< 0x06d9b094      cbz   x1, 0x6d9b0f8
|     ||:   0x06d9b098      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b09c      str   x1, [x2, 0x10]
|     ||:   0x06d9b0a0      adrp  x27, 0xb48f000
|     ||:   0x06d9b0a4      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b0a8      cbz   w3, 0x6d9b0b8
|    |||:   0x06d9b0ac      ldr   x3, [x2, 8]                          ; [0x8:4]=0
|    |||:   0x06d9b0b0      bl    fcn.04da3500
|    |||:   0x06d9b0b4      stp   x0, x3, [x25]
|    `----> 0x06d9b0b8      str   x0, [x2, 8]
|     ||:   0x06d9b0bc      adrp  x0, 0x856f000
|     ||:   0x06d9b0c0      add   x0, x0, 0x950
|     ||:   0x06d9b0c4      bl    fcn.04d31c10
|     ||:   0x06d9b0c8      orr   x1, xzr, 1
|     ||:   0x06d9b0cc      strb  w1, [x0]
|     ||:   0x06d9b0d0      adrp  x27, 0xb48f000
|     ||:   0x06d9b0d4      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b0d8      cbnz  w1, 0x6d9b0e4
|    |||:   0x06d9b0dc      ldr   x2, [var_10h]                        ; [0x50:4]=64
|   ,=====< 0x06d9b0e0      b     0x6d9b0f4
|   |`----> 0x06d9b0e4      ldr   x2, [var_10h]                        ; [0x50:4]=64
|   | ||:   0x06d9b0e8      ldr   x3, [x2]
|   | ||:   0x06d9b0ec      bl    fcn.04da3500
|   | ||:   0x06d9b0f0      stp   x0, x3, [x25]
|   | ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b0e0
|   `-----> 0x06d9b0f4      str   x0, [x2]
|     `---> 0x06d9b0f8      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b0fc      ldr   x2, [x26]
|      |:   0x06d9b100      adrp  x0, 0x43de000
|      |:   0x06d9b104      add   x0, x0, 0x220
|      |:   0x06d9b108      mov   x1, 0x1d
|      |:   0x06d9b10c      blr   x2
|     ,===< 0x06d9b110      cbz   x1, 0x6d9b138
|     ||:   0x06d9b114      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b118      str   x1, [x2, 0x20]
|     ||:   0x06d9b11c      adrp  x27, 0xb48f000
|     ||:   0x06d9b120      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b124      cbz   w3, 0x6d9b134
|    |||:   0x06d9b128      ldr   x3, [x2, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|    |||:   0x06d9b12c      bl    fcn.04da3500
|    |||:   0x06d9b130      stp   x0, x3, [x25]
|    `----> 0x06d9b134      str   x0, [x2, 0x18]
|     `---> 0x06d9b138      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b13c      ldr   x2, [x26]
|      |:   0x06d9b140      ldr   x3, [var_28h]                        ; [0x38:4]=0x400010
|      |:   0x06d9b144      cmp   x3, 0
|      |:   0x06d9b148      cset  x3, ne
|      |:   0x06d9b14c      cmp   x1, 0
|      |:   0x06d9b150      cset  x4, ne
|      |:   0x06d9b154      orr   x3, x4, x3
|      |:   0x06d9b158      strb  w3, [var_29h]
|      |:   0x06d9b15c      adrp  x0, 0x439b000
|      |:   0x06d9b160      add   x0, x0, 0x515
|      |:   0x06d9b164      mov   x1, 0x17
|      |:   0x06d9b168      blr   x2
|     ,===< 0x06d9b16c      cbz   x1, 0x6d9b194
|     ||:   0x06d9b170      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b174      str   x1, [x2, 0x30]
|     ||:   0x06d9b178      adrp  x27, 0xb48f000
|     ||:   0x06d9b17c      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b180      cbz   w3, 0x6d9b190
|    |||:   0x06d9b184      ldr   x3, [x2, 0x28]                       ; [0x28:4]=0xace39d8
|    |||:   0x06d9b188      bl    fcn.04da3500
|    |||:   0x06d9b18c      stp   x0, x3, [x25]
|    `----> 0x06d9b190      str   x0, [x2, 0x28]
|     `---> 0x06d9b194      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b198      ldr   x2, [x26]
|      |:   0x06d9b19c      cmp   x1, 0
|      |:   0x06d9b1a0      cset  x3, ne
|      |:   0x06d9b1a4      ldrb  w4, [var_29h]                        ; [0x37:4]=0x40001000
|      |:   0x06d9b1a8      orr   x3, x4, x3
|      |:   0x06d9b1ac      strb  w3, [var_29h]
|      |:   0x06d9b1b0      adrp  x0, 0x439b000
|      |:   0x06d9b1b4      add   x0, x0, 0x52c
|      |:   0x06d9b1b8      mov   x1, 0x17
|      |:   0x06d9b1bc      blr   x2
|     ,===< 0x06d9b1c0      cbnz  x1, 0x6d9b1d0
|     ||:   0x06d9b1c4      ldr   x3, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b1c8      ldrb  w2, [var_29h]                        ; [0x37:4]=0x40001000
|    ,====< 0x06d9b1cc      b     0x6d9b22c
|    |`---> 0x06d9b1d0      bl    0x6d9cd90
|    |,===< 0x06d9b1d4      cbz   x0, 0x6d9b1e0
|    |||:   0x06d9b1d8      ldr   x2, [x0]
|   ,=====< 0x06d9b1dc      b     0x6d9b1e4
|   ||`---> 0x06d9b1e0      mov   x2, xzr
|   || |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1dc
|   `-----> 0x06d9b1e4      cmp   x2, 0
|    |,===< 0x06d9b1e8      b.le  0x6d9b21c
|    |||:   0x06d9b1ec      adrp  x27, 0xb48f000
|    |||:   0x06d9b1f0      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d9b1f4      cbnz  w3, 0x6d9b200
|   ||||:   0x06d9b1f8      ldr   x3, [var_10h]                        ; [0x50:4]=64
|  ,======< 0x06d9b1fc      b     0x6d9b214
|  |`-----> 0x06d9b200      ldr   x3, [var_10h]                        ; [0x50:4]=64
|  | |||:   0x06d9b204      ldr   x4, [x3, 0x38]                       ; [0x38:4]=0x400010
|  | |||:   0x06d9b208      bl    fcn.04da3500
|  | |||:   0x06d9b20c      stp   x0, x4, [x25]
|  | |||:   0x06d9b210      cmp   x2, 0
|  | |||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1fc
|  `------> 0x06d9b214      str   x0, [x3, 0x38]
|   ,=====< 0x06d9b218      b     0x6d9b220
|   ||`---> 0x06d9b21c      ldr   x3, [var_10h]                        ; [0x50:4]=64
|   || |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b218
|   `-----> 0x06d9b220      cset  x2, gt
|    | |:   0x06d9b224      ldrb  w4, [var_29h]                        ; [0x37:4]=0x40001000
|    | |:   0x06d9b228      orr   x2, x2, x4
|    | |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1cc
|    `,===< 0x06d9b22c      tbz   w2, 0, 0x6d9b258
|     ||:   0x06d9b230      adrp  x27, 0xb48f000
|     ||:   0x06d9b234      ldr   w2, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b238      cbnz  w2, 0x6d9b244
|    |||:   0x06d9b23c      ldr   x2, [var_18h]                        ; [0x48:4]=64
|   ,=====< 0x06d9b240      b     0x6d9b254
|   |`----> 0x06d9b244      ldr   x2, [var_18h]                        ; [0x48:4]=64
|   | ||:   0x06d9b248      ldr   x4, [x2, 0x10]                       ; [0x10:4]=0xb70003
|   | ||:   0x06d9b24c      bl    fcn.04da3500
|   | ||:   0x06d9b250      stp   x3, x4, [x25]
|   | ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b240
|   `-----> 0x06d9b254      str   x3, [x2, 0x10]
|     `---> 0x06d9b258      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b25c      ldr   x2, [x26]
|      |:   0x06d9b260      adrp  x0, 0x439b000
|      |:   0x06d9b264      add   x0, x0, 0x543
|      |:   0x06d9b268      mov   x1, 0x17
|      |:   0x06d9b26c      blr   x2
|     ,===< 0x06d9b270      cbnz  x1, 0x6d9b28c
|     ||:   0x06d9b274      ldr   x26, [var_20h]                       ; segment.PHDR
|     ||:                                                              ; [0x40:4]=6 ; "\U00000006"
|     ||:   0x06d9b278      ldr   x2, [x26]
|     ||:   0x06d9b27c      adrp  x0, 0x42ec000
|     ||:   0x06d9b280      add   x0, x0, 0xb4b
|     ||:   0x06d9b284      mov   x1, 0xd
|     ||:   0x06d9b288      blr   x2
|    ,`---> 0x06d9b28c      cbz   x1, 0x6d9b2b8
|    | |:   0x06d9b290      ldr   x2, [arg_48h]                        ; [0x48:4]=64
|    | |:   0x06d9b294      str   x1, [x2, 8]
|    | |:   0x06d9b298      adrp  x27, 0xb48f000
|    | |:   0x06d9b29c      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    |,===< 0x06d9b2a0      cbz   w1, 0x6d9b2b0
|    |||:   0x06d9b2a4      ldr   x1, [x2]
|    |||:   0x06d9b2a8      bl    fcn.04da3500
|    |||:   0x06d9b2ac      stp   x0, x1, [x25]
|    |`---> 0x06d9b2b0      str   x0, [x2]
|    |,===< 0x06d9b2b4      b     0x6d9b2bc
|    `----> 0x06d9b2b8      ldr   x2, [var_18h]                        ; [0x48:4]=64
|     ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b2b4
|     `---> 0x06d9b2bc      mov   x0, x2
|      |:   0x06d9b2c0      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |:   0x06d9b2c4      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|      |:   0x06d9b2c8      ret
|      `--> 0x06d9b2cc      str   x0, [arg_8h]                         ; arg1
|       :   0x06d9b2d0      mov   x3, lr
|       :   0x06d9b2d4      bl    fcn.04da15a0
|       :   0x06d9b2d8      ldr   x0, [arg_8h]                         ; [0x8:4]=0
|       `=< 0x06d9b2dc      b     0x6d9b020
==== AuthProvider_.SetWireProtocol.deferwrap1
/ fcn.06d9b028(int64_t arg1, int64_t arg_8h, int64_t arg_48h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; var int64_t var_29h @ stack - 0x29
|       :   ; var int64_t var_28h @ stack - 0x28
|       :   ; var int64_t var_20h @ stack - 0x20
|       :   ; var int64_t var_18h @ stack - 0x18
|       :   ; var int64_t var_10h @ stack - 0x10
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_48h @ stack + 0x48
|      ,==< 0x06d9b028      b.ls  0x6d9b2cc
|      |:   0x06d9b02c      str   lr, [sp, -0x60]!
|      |:   0x06d9b030      stur  fp, [sp, -8]
|      |:   0x06d9b034      sub   fp, sp, 8
|     ,===< 0x06d9b038      cbnz  x0, 0x6d9b044
|     ||:   0x06d9b03c      adrp  x0, 0x8f91000
|     ||:   0x06d9b040      add   x0, x0, 0x498
|     `---> 0x06d9b044      str   x0, [var_20h]
|      |:   0x06d9b048      mov   x0, 0x28                             ; '('
|      |:   0x06d9b04c      adrp  x1, 0x8ac1000
|      |:   0x06d9b050      add   x1, x1, 0xd30
|      |:   0x06d9b054      orr   x2, xzr, 1
|      |:   0x06d9b058      bl    fcn.04d32f40
|      |:   0x06d9b05c      str   x0, [var_18h]
|      |:   0x06d9b060      mov   x0, 0x48                             ; 'H'
|      |:   0x06d9b064      adrp  x1, 0x8c53000
|      |:   0x06d9b068      add   x1, x1, 0x128
|      |:   0x06d9b06c      orr   x2, xzr, 1
|      |:   0x06d9b070      bl    0x4d33650
|      |:   0x06d9b074      str   x0, [var_10h]
|      |:   0x06d9b078      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b07c      ldr   x3, [x26]
|      |:   0x06d9b080      adrp  x0, 0x4358000
|      |:   0x06d9b084      add   x0, x0, 0xd36
|      |:   0x06d9b088      mov   x1, 0x13
|      |:   0x06d9b08c      blr   x3
|      |:   0x06d9b090      str   x1, [var_28h]
|     ,===< 0x06d9b094      cbz   x1, 0x6d9b0f8
|     ||:   0x06d9b098      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b09c      str   x1, [x2, 0x10]
|     ||:   0x06d9b0a0      adrp  x27, 0xb48f000
|     ||:   0x06d9b0a4      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b0a8      cbz   w3, 0x6d9b0b8
|    |||:   0x06d9b0ac      ldr   x3, [x2, 8]                          ; [0x8:4]=0
|    |||:   0x06d9b0b0      bl    fcn.04da3500
|    |||:   0x06d9b0b4      stp   x0, x3, [x25]
|    `----> 0x06d9b0b8      str   x0, [x2, 8]
|     ||:   0x06d9b0bc      adrp  x0, 0x856f000
|     ||:   0x06d9b0c0      add   x0, x0, 0x950
|     ||:   0x06d9b0c4      bl    fcn.04d31c10
|     ||:   0x06d9b0c8      orr   x1, xzr, 1
|     ||:   0x06d9b0cc      strb  w1, [x0]
|     ||:   0x06d9b0d0      adrp  x27, 0xb48f000
|     ||:   0x06d9b0d4      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b0d8      cbnz  w1, 0x6d9b0e4
|    |||:   0x06d9b0dc      ldr   x2, [var_10h]                        ; [0x50:4]=64
|   ,=====< 0x06d9b0e0      b     0x6d9b0f4
|   |`----> 0x06d9b0e4      ldr   x2, [var_10h]                        ; [0x50:4]=64
|   | ||:   0x06d9b0e8      ldr   x3, [x2]
|   | ||:   0x06d9b0ec      bl    fcn.04da3500
|   | ||:   0x06d9b0f0      stp   x0, x3, [x25]
|   | ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b0e0
|   `-----> 0x06d9b0f4      str   x0, [x2]
|     `---> 0x06d9b0f8      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b0fc      ldr   x2, [x26]
|      |:   0x06d9b100      adrp  x0, 0x43de000
|      |:   0x06d9b104      add   x0, x0, 0x220
|      |:   0x06d9b108      mov   x1, 0x1d
|      |:   0x06d9b10c      blr   x2
|     ,===< 0x06d9b110      cbz   x1, 0x6d9b138
|     ||:   0x06d9b114      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b118      str   x1, [x2, 0x20]
|     ||:   0x06d9b11c      adrp  x27, 0xb48f000
|     ||:   0x06d9b120      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b124      cbz   w3, 0x6d9b134
|    |||:   0x06d9b128      ldr   x3, [x2, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|    |||:   0x06d9b12c      bl    fcn.04da3500
|    |||:   0x06d9b130      stp   x0, x3, [x25]
|    `----> 0x06d9b134      str   x0, [x2, 0x18]
|     `---> 0x06d9b138      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b13c      ldr   x2, [x26]
|      |:   0x06d9b140      ldr   x3, [var_28h]                        ; [0x38:4]=0x400010
|      |:   0x06d9b144      cmp   x3, 0
|      |:   0x06d9b148      cset  x3, ne
|      |:   0x06d9b14c      cmp   x1, 0
|      |:   0x06d9b150      cset  x4, ne
|      |:   0x06d9b154      orr   x3, x4, x3
|      |:   0x06d9b158      strb  w3, [var_29h]
|      |:   0x06d9b15c      adrp  x0, 0x439b000
|      |:   0x06d9b160      add   x0, x0, 0x515
|      |:   0x06d9b164      mov   x1, 0x17
|      |:   0x06d9b168      blr   x2
|     ,===< 0x06d9b16c      cbz   x1, 0x6d9b194
|     ||:   0x06d9b170      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b174      str   x1, [x2, 0x30]
|     ||:   0x06d9b178      adrp  x27, 0xb48f000
|     ||:   0x06d9b17c      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b180      cbz   w3, 0x6d9b190
|    |||:   0x06d9b184      ldr   x3, [x2, 0x28]                       ; [0x28:4]=0xace39d8
|    |||:   0x06d9b188      bl    fcn.04da3500
|    |||:   0x06d9b18c      stp   x0, x3, [x25]
|    `----> 0x06d9b190      str   x0, [x2, 0x28]
|     `---> 0x06d9b194      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b198      ldr   x2, [x26]
|      |:   0x06d9b19c      cmp   x1, 0
|      |:   0x06d9b1a0      cset  x3, ne
|      |:   0x06d9b1a4      ldrb  w4, [var_29h]                        ; [0x37:4]=0x40001000
|      |:   0x06d9b1a8      orr   x3, x4, x3
|      |:   0x06d9b1ac      strb  w3, [var_29h]
|      |:   0x06d9b1b0      adrp  x0, 0x439b000
|      |:   0x06d9b1b4      add   x0, x0, 0x52c
|      |:   0x06d9b1b8      mov   x1, 0x17
|      |:   0x06d9b1bc      blr   x2
|     ,===< 0x06d9b1c0      cbnz  x1, 0x6d9b1d0
|     ||:   0x06d9b1c4      ldr   x3, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b1c8      ldrb  w2, [var_29h]                        ; [0x37:4]=0x40001000
|    ,====< 0x06d9b1cc      b     0x6d9b22c
|    |`---> 0x06d9b1d0      bl    0x6d9cd90
|    |,===< 0x06d9b1d4      cbz   x0, 0x6d9b1e0
|    |||:   0x06d9b1d8      ldr   x2, [x0]
|   ,=====< 0x06d9b1dc      b     0x6d9b1e4
|   ||`---> 0x06d9b1e0      mov   x2, xzr
|   || |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1dc
|   `-----> 0x06d9b1e4      cmp   x2, 0
|    |,===< 0x06d9b1e8      b.le  0x6d9b21c
|    |||:   0x06d9b1ec      adrp  x27, 0xb48f000
|    |||:   0x06d9b1f0      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d9b1f4      cbnz  w3, 0x6d9b200
|   ||||:   0x06d9b1f8      ldr   x3, [var_10h]                        ; [0x50:4]=64
|  ,======< 0x06d9b1fc      b     0x6d9b214
|  |`-----> 0x06d9b200      ldr   x3, [var_10h]                        ; [0x50:4]=64
|  | |||:   0x06d9b204      ldr   x4, [x3, 0x38]                       ; [0x38:4]=0x400010
|  | |||:   0x06d9b208      bl    fcn.04da3500
|  | |||:   0x06d9b20c      stp   x0, x4, [x25]
|  | |||:   0x06d9b210      cmp   x2, 0
|  | |||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1fc
|  `------> 0x06d9b214      str   x0, [x3, 0x38]
|   ,=====< 0x06d9b218      b     0x6d9b220
|   ||`---> 0x06d9b21c      ldr   x3, [var_10h]                        ; [0x50:4]=64
|   || |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b218
|   `-----> 0x06d9b220      cset  x2, gt
|    | |:   0x06d9b224      ldrb  w4, [var_29h]                        ; [0x37:4]=0x40001000
|    | |:   0x06d9b228      orr   x2, x2, x4
|    | |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1cc
|    `,===< 0x06d9b22c      tbz   w2, 0, 0x6d9b258
|     ||:   0x06d9b230      adrp  x27, 0xb48f000
|     ||:   0x06d9b234      ldr   w2, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b238      cbnz  w2, 0x6d9b244
|    |||:   0x06d9b23c      ldr   x2, [var_18h]                        ; [0x48:4]=64
|   ,=====< 0x06d9b240      b     0x6d9b254
|   |`----> 0x06d9b244      ldr   x2, [var_18h]                        ; [0x48:4]=64
|   | ||:   0x06d9b248      ldr   x4, [x2, 0x10]                       ; [0x10:4]=0xb70003
|   | ||:   0x06d9b24c      bl    fcn.04da3500
|   | ||:   0x06d9b250      stp   x3, x4, [x25]
|   | ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b240
|   `-----> 0x06d9b254      str   x3, [x2, 0x10]
|     `---> 0x06d9b258      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b25c      ldr   x2, [x26]
|      |:   0x06d9b260      adrp  x0, 0x439b000
|      |:   0x06d9b264      add   x0, x0, 0x543
|      |:   0x06d9b268      mov   x1, 0x17
|      |:   0x06d9b26c      blr   x2
|     ,===< 0x06d9b270      cbnz  x1, 0x6d9b28c
|     ||:   0x06d9b274      ldr   x26, [var_20h]                       ; segment.PHDR
|     ||:                                                              ; [0x40:4]=6 ; "\U00000006"
|     ||:   0x06d9b278      ldr   x2, [x26]
|     ||:   0x06d9b27c      adrp  x0, 0x42ec000
|     ||:   0x06d9b280      add   x0, x0, 0xb4b
|     ||:   0x06d9b284      mov   x1, 0xd
|     ||:   0x06d9b288      blr   x2
|    ,`---> 0x06d9b28c      cbz   x1, 0x6d9b2b8
|    | |:   0x06d9b290      ldr   x2, [arg_48h]                        ; [0x48:4]=64
|    | |:   0x06d9b294      str   x1, [x2, 8]
|    | |:   0x06d9b298      adrp  x27, 0xb48f000
|    | |:   0x06d9b29c      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    |,===< 0x06d9b2a0      cbz   w1, 0x6d9b2b0
|    |||:   0x06d9b2a4      ldr   x1, [x2]
|    |||:   0x06d9b2a8      bl    fcn.04da3500
|    |||:   0x06d9b2ac      stp   x0, x1, [x25]
|    |`---> 0x06d9b2b0      str   x0, [x2]
|    |,===< 0x06d9b2b4      b     0x6d9b2bc
|    `----> 0x06d9b2b8      ldr   x2, [var_18h]                        ; [0x48:4]=64
|     ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b2b4
|     `---> 0x06d9b2bc      mov   x0, x2
|      |:   0x06d9b2c0      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |:   0x06d9b2c4      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|      |:   0x06d9b2c8      ret
|      `--> 0x06d9b2cc      str   x0, [arg_8h]                         ; arg1
|       :   0x06d9b2d0      mov   x3, lr
|       :   0x06d9b2d4      bl    fcn.04da15a0
|       :   0x06d9b2d8      ldr   x0, [arg_8h]                         ; [0x8:4]=0
|       `=< 0x06d9b2dc      b     0x6d9b020
==== AuthProvider_.SetHTTPHeaders.deferwrap1
/ fcn.06d9b028(int64_t arg1, int64_t arg_8h, int64_t arg_48h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; var int64_t var_29h @ stack - 0x29
|       :   ; var int64_t var_28h @ stack - 0x28
|       :   ; var int64_t var_20h @ stack - 0x20
|       :   ; var int64_t var_18h @ stack - 0x18
|       :   ; var int64_t var_10h @ stack - 0x10
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_48h @ stack + 0x48
|      ,==< 0x06d9b028      b.ls  0x6d9b2cc
|      |:   0x06d9b02c      str   lr, [sp, -0x60]!
|      |:   0x06d9b030      stur  fp, [sp, -8]
|      |:   0x06d9b034      sub   fp, sp, 8
|     ,===< 0x06d9b038      cbnz  x0, 0x6d9b044
|     ||:   0x06d9b03c      adrp  x0, 0x8f91000
|     ||:   0x06d9b040      add   x0, x0, 0x498
|     `---> 0x06d9b044      str   x0, [var_20h]
|      |:   0x06d9b048      mov   x0, 0x28                             ; '('
|      |:   0x06d9b04c      adrp  x1, 0x8ac1000
|      |:   0x06d9b050      add   x1, x1, 0xd30
|      |:   0x06d9b054      orr   x2, xzr, 1
|      |:   0x06d9b058      bl    fcn.04d32f40
|      |:   0x06d9b05c      str   x0, [var_18h]
|      |:   0x06d9b060      mov   x0, 0x48                             ; 'H'
|      |:   0x06d9b064      adrp  x1, 0x8c53000
|      |:   0x06d9b068      add   x1, x1, 0x128
|      |:   0x06d9b06c      orr   x2, xzr, 1
|      |:   0x06d9b070      bl    0x4d33650
|      |:   0x06d9b074      str   x0, [var_10h]
|      |:   0x06d9b078      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b07c      ldr   x3, [x26]
|      |:   0x06d9b080      adrp  x0, 0x4358000
|      |:   0x06d9b084      add   x0, x0, 0xd36
|      |:   0x06d9b088      mov   x1, 0x13
|      |:   0x06d9b08c      blr   x3
|      |:   0x06d9b090      str   x1, [var_28h]
|     ,===< 0x06d9b094      cbz   x1, 0x6d9b0f8
|     ||:   0x06d9b098      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b09c      str   x1, [x2, 0x10]
|     ||:   0x06d9b0a0      adrp  x27, 0xb48f000
|     ||:   0x06d9b0a4      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b0a8      cbz   w3, 0x6d9b0b8
|    |||:   0x06d9b0ac      ldr   x3, [x2, 8]                          ; [0x8:4]=0
|    |||:   0x06d9b0b0      bl    fcn.04da3500
|    |||:   0x06d9b0b4      stp   x0, x3, [x25]
|    `----> 0x06d9b0b8      str   x0, [x2, 8]
|     ||:   0x06d9b0bc      adrp  x0, 0x856f000
|     ||:   0x06d9b0c0      add   x0, x0, 0x950
|     ||:   0x06d9b0c4      bl    fcn.04d31c10
|     ||:   0x06d9b0c8      orr   x1, xzr, 1
|     ||:   0x06d9b0cc      strb  w1, [x0]
|     ||:   0x06d9b0d0      adrp  x27, 0xb48f000
|     ||:   0x06d9b0d4      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b0d8      cbnz  w1, 0x6d9b0e4
|    |||:   0x06d9b0dc      ldr   x2, [var_10h]                        ; [0x50:4]=64
|   ,=====< 0x06d9b0e0      b     0x6d9b0f4
|   |`----> 0x06d9b0e4      ldr   x2, [var_10h]                        ; [0x50:4]=64
|   | ||:   0x06d9b0e8      ldr   x3, [x2]
|   | ||:   0x06d9b0ec      bl    fcn.04da3500
|   | ||:   0x06d9b0f0      stp   x0, x3, [x25]
|   | ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b0e0
|   `-----> 0x06d9b0f4      str   x0, [x2]
|     `---> 0x06d9b0f8      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b0fc      ldr   x2, [x26]
|      |:   0x06d9b100      adrp  x0, 0x43de000
|      |:   0x06d9b104      add   x0, x0, 0x220
|      |:   0x06d9b108      mov   x1, 0x1d
|      |:   0x06d9b10c      blr   x2
|     ,===< 0x06d9b110      cbz   x1, 0x6d9b138
|     ||:   0x06d9b114      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b118      str   x1, [x2, 0x20]
|     ||:   0x06d9b11c      adrp  x27, 0xb48f000
|     ||:   0x06d9b120      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b124      cbz   w3, 0x6d9b134
|    |||:   0x06d9b128      ldr   x3, [x2, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|    |||:   0x06d9b12c      bl    fcn.04da3500
|    |||:   0x06d9b130      stp   x0, x3, [x25]
|    `----> 0x06d9b134      str   x0, [x2, 0x18]
|     `---> 0x06d9b138      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b13c      ldr   x2, [x26]
|      |:   0x06d9b140      ldr   x3, [var_28h]                        ; [0x38:4]=0x400010
|      |:   0x06d9b144      cmp   x3, 0
|      |:   0x06d9b148      cset  x3, ne
|      |:   0x06d9b14c      cmp   x1, 0
|      |:   0x06d9b150      cset  x4, ne
|      |:   0x06d9b154      orr   x3, x4, x3
|      |:   0x06d9b158      strb  w3, [var_29h]
|      |:   0x06d9b15c      adrp  x0, 0x439b000
|      |:   0x06d9b160      add   x0, x0, 0x515
|      |:   0x06d9b164      mov   x1, 0x17
|      |:   0x06d9b168      blr   x2
|     ,===< 0x06d9b16c      cbz   x1, 0x6d9b194
|     ||:   0x06d9b170      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b174      str   x1, [x2, 0x30]
|     ||:   0x06d9b178      adrp  x27, 0xb48f000
|     ||:   0x06d9b17c      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b180      cbz   w3, 0x6d9b190
|    |||:   0x06d9b184      ldr   x3, [x2, 0x28]                       ; [0x28:4]=0xace39d8
|    |||:   0x06d9b188      bl    fcn.04da3500
|    |||:   0x06d9b18c      stp   x0, x3, [x25]
|    `----> 0x06d9b190      str   x0, [x2, 0x28]
|     `---> 0x06d9b194      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b198      ldr   x2, [x26]
|      |:   0x06d9b19c      cmp   x1, 0
|      |:   0x06d9b1a0      cset  x3, ne
|      |:   0x06d9b1a4      ldrb  w4, [var_29h]                        ; [0x37:4]=0x40001000
|      |:   0x06d9b1a8      orr   x3, x4, x3
|      |:   0x06d9b1ac      strb  w3, [var_29h]
|      |:   0x06d9b1b0      adrp  x0, 0x439b000
|      |:   0x06d9b1b4      add   x0, x0, 0x52c
|      |:   0x06d9b1b8      mov   x1, 0x17
|      |:   0x06d9b1bc      blr   x2
|     ,===< 0x06d9b1c0      cbnz  x1, 0x6d9b1d0
|     ||:   0x06d9b1c4      ldr   x3, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b1c8      ldrb  w2, [var_29h]                        ; [0x37:4]=0x40001000
|    ,====< 0x06d9b1cc      b     0x6d9b22c
|    |`---> 0x06d9b1d0      bl    0x6d9cd90
|    |,===< 0x06d9b1d4      cbz   x0, 0x6d9b1e0
|    |||:   0x06d9b1d8      ldr   x2, [x0]
|   ,=====< 0x06d9b1dc      b     0x6d9b1e4
|   ||`---> 0x06d9b1e0      mov   x2, xzr
|   || |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1dc
|   `-----> 0x06d9b1e4      cmp   x2, 0
|    |,===< 0x06d9b1e8      b.le  0x6d9b21c
|    |||:   0x06d9b1ec      adrp  x27, 0xb48f000
|    |||:   0x06d9b1f0      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d9b1f4      cbnz  w3, 0x6d9b200
|   ||||:   0x06d9b1f8      ldr   x3, [var_10h]                        ; [0x50:4]=64
|  ,======< 0x06d9b1fc      b     0x6d9b214
|  |`-----> 0x06d9b200      ldr   x3, [var_10h]                        ; [0x50:4]=64
|  | |||:   0x06d9b204      ldr   x4, [x3, 0x38]                       ; [0x38:4]=0x400010
|  | |||:   0x06d9b208      bl    fcn.04da3500
|  | |||:   0x06d9b20c      stp   x0, x4, [x25]
|  | |||:   0x06d9b210      cmp   x2, 0
|  | |||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1fc
|  `------> 0x06d9b214      str   x0, [x3, 0x38]
|   ,=====< 0x06d9b218      b     0x6d9b220
|   ||`---> 0x06d9b21c      ldr   x3, [var_10h]                        ; [0x50:4]=64
|   || |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b218
|   `-----> 0x06d9b220      cset  x2, gt
|    | |:   0x06d9b224      ldrb  w4, [var_29h]                        ; [0x37:4]=0x40001000
|    | |:   0x06d9b228      orr   x2, x2, x4
|    | |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1cc
|    `,===< 0x06d9b22c      tbz   w2, 0, 0x6d9b258
|     ||:   0x06d9b230      adrp  x27, 0xb48f000
|     ||:   0x06d9b234      ldr   w2, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b238      cbnz  w2, 0x6d9b244
|    |||:   0x06d9b23c      ldr   x2, [var_18h]                        ; [0x48:4]=64
|   ,=====< 0x06d9b240      b     0x6d9b254
|   |`----> 0x06d9b244      ldr   x2, [var_18h]                        ; [0x48:4]=64
|   | ||:   0x06d9b248      ldr   x4, [x2, 0x10]                       ; [0x10:4]=0xb70003
|   | ||:   0x06d9b24c      bl    fcn.04da3500
|   | ||:   0x06d9b250      stp   x3, x4, [x25]
|   | ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b240
|   `-----> 0x06d9b254      str   x3, [x2, 0x10]
|     `---> 0x06d9b258      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b25c      ldr   x2, [x26]
|      |:   0x06d9b260      adrp  x0, 0x439b000
|      |:   0x06d9b264      add   x0, x0, 0x543
|      |:   0x06d9b268      mov   x1, 0x17
|      |:   0x06d9b26c      blr   x2
|     ,===< 0x06d9b270      cbnz  x1, 0x6d9b28c
|     ||:   0x06d9b274      ldr   x26, [var_20h]                       ; segment.PHDR
|     ||:                                                              ; [0x40:4]=6 ; "\U00000006"
|     ||:   0x06d9b278      ldr   x2, [x26]
|     ||:   0x06d9b27c      adrp  x0, 0x42ec000
|     ||:   0x06d9b280      add   x0, x0, 0xb4b
|     ||:   0x06d9b284      mov   x1, 0xd
|     ||:   0x06d9b288      blr   x2
|    ,`---> 0x06d9b28c      cbz   x1, 0x6d9b2b8
|    | |:   0x06d9b290      ldr   x2, [arg_48h]                        ; [0x48:4]=64
|    | |:   0x06d9b294      str   x1, [x2, 8]
|    | |:   0x06d9b298      adrp  x27, 0xb48f000
|    | |:   0x06d9b29c      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    |,===< 0x06d9b2a0      cbz   w1, 0x6d9b2b0
|    |||:   0x06d9b2a4      ldr   x1, [x2]
|    |||:   0x06d9b2a8      bl    fcn.04da3500
|    |||:   0x06d9b2ac      stp   x0, x1, [x25]
|    |`---> 0x06d9b2b0      str   x0, [x2]
|    |,===< 0x06d9b2b4      b     0x6d9b2bc
|    `----> 0x06d9b2b8      ldr   x2, [var_18h]                        ; [0x48:4]=64
|     ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b2b4
|     `---> 0x06d9b2bc      mov   x0, x2
|      |:   0x06d9b2c0      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |:   0x06d9b2c4      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|      |:   0x06d9b2c8      ret
|      `--> 0x06d9b2cc      str   x0, [arg_8h]                         ; arg1
|       :   0x06d9b2d0      mov   x3, lr
|       :   0x06d9b2d4      bl    fcn.04da15a0
|       :   0x06d9b2d8      ldr   x0, [arg_8h]                         ; [0x8:4]=0
|       `=< 0x06d9b2dc      b     0x6d9b020
==== AuthProvider_.fetchRemoteModelCatalog.deferwrap1
/ fcn.06d9b028(int64_t arg1, int64_t arg_8h, int64_t arg_48h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; var int64_t var_29h @ stack - 0x29
|       :   ; var int64_t var_28h @ stack - 0x28
|       :   ; var int64_t var_20h @ stack - 0x20
|       :   ; var int64_t var_18h @ stack - 0x18
|       :   ; var int64_t var_10h @ stack - 0x10
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_48h @ stack + 0x48
|      ,==< 0x06d9b028      b.ls  0x6d9b2cc
|      |:   0x06d9b02c      str   lr, [sp, -0x60]!
|      |:   0x06d9b030      stur  fp, [sp, -8]
|      |:   0x06d9b034      sub   fp, sp, 8
|     ,===< 0x06d9b038      cbnz  x0, 0x6d9b044
|     ||:   0x06d9b03c      adrp  x0, 0x8f91000
|     ||:   0x06d9b040      add   x0, x0, 0x498
|     `---> 0x06d9b044      str   x0, [var_20h]
|      |:   0x06d9b048      mov   x0, 0x28                             ; '('
|      |:   0x06d9b04c      adrp  x1, 0x8ac1000
|      |:   0x06d9b050      add   x1, x1, 0xd30
|      |:   0x06d9b054      orr   x2, xzr, 1
|      |:   0x06d9b058      bl    fcn.04d32f40
|      |:   0x06d9b05c      str   x0, [var_18h]
|      |:   0x06d9b060      mov   x0, 0x48                             ; 'H'
|      |:   0x06d9b064      adrp  x1, 0x8c53000
|      |:   0x06d9b068      add   x1, x1, 0x128
|      |:   0x06d9b06c      orr   x2, xzr, 1
|      |:   0x06d9b070      bl    0x4d33650
|      |:   0x06d9b074      str   x0, [var_10h]
|      |:   0x06d9b078      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b07c      ldr   x3, [x26]
|      |:   0x06d9b080      adrp  x0, 0x4358000
|      |:   0x06d9b084      add   x0, x0, 0xd36
|      |:   0x06d9b088      mov   x1, 0x13
|      |:   0x06d9b08c      blr   x3
|      |:   0x06d9b090      str   x1, [var_28h]
|     ,===< 0x06d9b094      cbz   x1, 0x6d9b0f8
|     ||:   0x06d9b098      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b09c      str   x1, [x2, 0x10]
|     ||:   0x06d9b0a0      adrp  x27, 0xb48f000
|     ||:   0x06d9b0a4      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b0a8      cbz   w3, 0x6d9b0b8
|    |||:   0x06d9b0ac      ldr   x3, [x2, 8]                          ; [0x8:4]=0
|    |||:   0x06d9b0b0      bl    fcn.04da3500
|    |||:   0x06d9b0b4      stp   x0, x3, [x25]
|    `----> 0x06d9b0b8      str   x0, [x2, 8]
|     ||:   0x06d9b0bc      adrp  x0, 0x856f000
|     ||:   0x06d9b0c0      add   x0, x0, 0x950
|     ||:   0x06d9b0c4      bl    fcn.04d31c10
|     ||:   0x06d9b0c8      orr   x1, xzr, 1
|     ||:   0x06d9b0cc      strb  w1, [x0]
|     ||:   0x06d9b0d0      adrp  x27, 0xb48f000
|     ||:   0x06d9b0d4      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b0d8      cbnz  w1, 0x6d9b0e4
|    |||:   0x06d9b0dc      ldr   x2, [var_10h]                        ; [0x50:4]=64
|   ,=====< 0x06d9b0e0      b     0x6d9b0f4
|   |`----> 0x06d9b0e4      ldr   x2, [var_10h]                        ; [0x50:4]=64
|   | ||:   0x06d9b0e8      ldr   x3, [x2]
|   | ||:   0x06d9b0ec      bl    fcn.04da3500
|   | ||:   0x06d9b0f0      stp   x0, x3, [x25]
|   | ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b0e0
|   `-----> 0x06d9b0f4      str   x0, [x2]
|     `---> 0x06d9b0f8      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b0fc      ldr   x2, [x26]
|      |:   0x06d9b100      adrp  x0, 0x43de000
|      |:   0x06d9b104      add   x0, x0, 0x220
|      |:   0x06d9b108      mov   x1, 0x1d
|      |:   0x06d9b10c      blr   x2
|     ,===< 0x06d9b110      cbz   x1, 0x6d9b138
|     ||:   0x06d9b114      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b118      str   x1, [x2, 0x20]
|     ||:   0x06d9b11c      adrp  x27, 0xb48f000
|     ||:   0x06d9b120      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b124      cbz   w3, 0x6d9b134
|    |||:   0x06d9b128      ldr   x3, [x2, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|    |||:   0x06d9b12c      bl    fcn.04da3500
|    |||:   0x06d9b130      stp   x0, x3, [x25]
|    `----> 0x06d9b134      str   x0, [x2, 0x18]
|     `---> 0x06d9b138      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b13c      ldr   x2, [x26]
|      |:   0x06d9b140      ldr   x3, [var_28h]                        ; [0x38:4]=0x400010
|      |:   0x06d9b144      cmp   x3, 0
|      |:   0x06d9b148      cset  x3, ne
|      |:   0x06d9b14c      cmp   x1, 0
|      |:   0x06d9b150      cset  x4, ne
|      |:   0x06d9b154      orr   x3, x4, x3
|      |:   0x06d9b158      strb  w3, [var_29h]
|      |:   0x06d9b15c      adrp  x0, 0x439b000
|      |:   0x06d9b160      add   x0, x0, 0x515
|      |:   0x06d9b164      mov   x1, 0x17
|      |:   0x06d9b168      blr   x2
|     ,===< 0x06d9b16c      cbz   x1, 0x6d9b194
|     ||:   0x06d9b170      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b174      str   x1, [x2, 0x30]
|     ||:   0x06d9b178      adrp  x27, 0xb48f000
|     ||:   0x06d9b17c      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b180      cbz   w3, 0x6d9b190
|    |||:   0x06d9b184      ldr   x3, [x2, 0x28]                       ; [0x28:4]=0xace39d8
|    |||:   0x06d9b188      bl    fcn.04da3500
|    |||:   0x06d9b18c      stp   x0, x3, [x25]
|    `----> 0x06d9b190      str   x0, [x2, 0x28]
|     `---> 0x06d9b194      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b198      ldr   x2, [x26]
|      |:   0x06d9b19c      cmp   x1, 0
|      |:   0x06d9b1a0      cset  x3, ne
|      |:   0x06d9b1a4      ldrb  w4, [var_29h]                        ; [0x37:4]=0x40001000
|      |:   0x06d9b1a8      orr   x3, x4, x3
|      |:   0x06d9b1ac      strb  w3, [var_29h]
|      |:   0x06d9b1b0      adrp  x0, 0x439b000
|      |:   0x06d9b1b4      add   x0, x0, 0x52c
|      |:   0x06d9b1b8      mov   x1, 0x17
|      |:   0x06d9b1bc      blr   x2
|     ,===< 0x06d9b1c0      cbnz  x1, 0x6d9b1d0
|     ||:   0x06d9b1c4      ldr   x3, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b1c8      ldrb  w2, [var_29h]                        ; [0x37:4]=0x40001000
|    ,====< 0x06d9b1cc      b     0x6d9b22c
|    |`---> 0x06d9b1d0      bl    0x6d9cd90
|    |,===< 0x06d9b1d4      cbz   x0, 0x6d9b1e0
|    |||:   0x06d9b1d8      ldr   x2, [x0]
|   ,=====< 0x06d9b1dc      b     0x6d9b1e4
|   ||`---> 0x06d9b1e0      mov   x2, xzr
|   || |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1dc
|   `-----> 0x06d9b1e4      cmp   x2, 0
|    |,===< 0x06d9b1e8      b.le  0x6d9b21c
|    |||:   0x06d9b1ec      adrp  x27, 0xb48f000
|    |||:   0x06d9b1f0      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d9b1f4      cbnz  w3, 0x6d9b200
|   ||||:   0x06d9b1f8      ldr   x3, [var_10h]                        ; [0x50:4]=64
|  ,======< 0x06d9b1fc      b     0x6d9b214
|  |`-----> 0x06d9b200      ldr   x3, [var_10h]                        ; [0x50:4]=64
|  | |||:   0x06d9b204      ldr   x4, [x3, 0x38]                       ; [0x38:4]=0x400010
|  | |||:   0x06d9b208      bl    fcn.04da3500
|  | |||:   0x06d9b20c      stp   x0, x4, [x25]
|  | |||:   0x06d9b210      cmp   x2, 0
|  | |||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1fc
|  `------> 0x06d9b214      str   x0, [x3, 0x38]
|   ,=====< 0x06d9b218      b     0x6d9b220
|   ||`---> 0x06d9b21c      ldr   x3, [var_10h]                        ; [0x50:4]=64
|   || |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b218
|   `-----> 0x06d9b220      cset  x2, gt
|    | |:   0x06d9b224      ldrb  w4, [var_29h]                        ; [0x37:4]=0x40001000
|    | |:   0x06d9b228      orr   x2, x2, x4
|    | |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1cc
|    `,===< 0x06d9b22c      tbz   w2, 0, 0x6d9b258
|     ||:   0x06d9b230      adrp  x27, 0xb48f000
|     ||:   0x06d9b234      ldr   w2, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b238      cbnz  w2, 0x6d9b244
|    |||:   0x06d9b23c      ldr   x2, [var_18h]                        ; [0x48:4]=64
|   ,=====< 0x06d9b240      b     0x6d9b254
|   |`----> 0x06d9b244      ldr   x2, [var_18h]                        ; [0x48:4]=64
|   | ||:   0x06d9b248      ldr   x4, [x2, 0x10]                       ; [0x10:4]=0xb70003
|   | ||:   0x06d9b24c      bl    fcn.04da3500
|   | ||:   0x06d9b250      stp   x3, x4, [x25]
|   | ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b240
|   `-----> 0x06d9b254      str   x3, [x2, 0x10]
|     `---> 0x06d9b258      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b25c      ldr   x2, [x26]
|      |:   0x06d9b260      adrp  x0, 0x439b000
|      |:   0x06d9b264      add   x0, x0, 0x543
|      |:   0x06d9b268      mov   x1, 0x17
|      |:   0x06d9b26c      blr   x2
|     ,===< 0x06d9b270      cbnz  x1, 0x6d9b28c
|     ||:   0x06d9b274      ldr   x26, [var_20h]                       ; segment.PHDR
|     ||:                                                              ; [0x40:4]=6 ; "\U00000006"
|     ||:   0x06d9b278      ldr   x2, [x26]
|     ||:   0x06d9b27c      adrp  x0, 0x42ec000
|     ||:   0x06d9b280      add   x0, x0, 0xb4b
|     ||:   0x06d9b284      mov   x1, 0xd
|     ||:   0x06d9b288      blr   x2
|    ,`---> 0x06d9b28c      cbz   x1, 0x6d9b2b8
|    | |:   0x06d9b290      ldr   x2, [arg_48h]                        ; [0x48:4]=64
|    | |:   0x06d9b294      str   x1, [x2, 8]
|    | |:   0x06d9b298      adrp  x27, 0xb48f000
|    | |:   0x06d9b29c      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    |,===< 0x06d9b2a0      cbz   w1, 0x6d9b2b0
|    |||:   0x06d9b2a4      ldr   x1, [x2]
|    |||:   0x06d9b2a8      bl    fcn.04da3500
|    |||:   0x06d9b2ac      stp   x0, x1, [x25]
|    |`---> 0x06d9b2b0      str   x0, [x2]
|    |,===< 0x06d9b2b4      b     0x6d9b2bc
|    `----> 0x06d9b2b8      ldr   x2, [var_18h]                        ; [0x48:4]=64
|     ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b2b4
|     `---> 0x06d9b2bc      mov   x0, x2
|      |:   0x06d9b2c0      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |:   0x06d9b2c4      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|      |:   0x06d9b2c8      ret
|      `--> 0x06d9b2cc      str   x0, [arg_8h]                         ; arg1
|       :   0x06d9b2d0      mov   x3, lr
|       :   0x06d9b2d4      bl    fcn.04da15a0
|       :   0x06d9b2d8      ldr   x0, [arg_8h]                         ; [0x8:4]=0
|       `=< 0x06d9b2dc      b     0x6d9b020
==== google3_third_party_jetski_gateway_gateway.NewHTTPTransportWithOptions.func1
/ fcn.06d9b028(int64_t arg1, int64_t arg_8h, int64_t arg_48h);
|       :   ; arg int64_t arg1 @ x0
|       :   ; var int64_t var_29h @ stack - 0x29
|       :   ; var int64_t var_28h @ stack - 0x28
|       :   ; var int64_t var_20h @ stack - 0x20
|       :   ; var int64_t var_18h @ stack - 0x18
|       :   ; var int64_t var_10h @ stack - 0x10
|       :   ; arg int64_t arg_8h @ stack + 0x8
|       :   ; arg int64_t arg_48h @ stack + 0x48
|      ,==< 0x06d9b028      b.ls  0x6d9b2cc
|      |:   0x06d9b02c      str   lr, [sp, -0x60]!
|      |:   0x06d9b030      stur  fp, [sp, -8]
|      |:   0x06d9b034      sub   fp, sp, 8
|     ,===< 0x06d9b038      cbnz  x0, 0x6d9b044
|     ||:   0x06d9b03c      adrp  x0, 0x8f91000
|     ||:   0x06d9b040      add   x0, x0, 0x498
|     `---> 0x06d9b044      str   x0, [var_20h]
|      |:   0x06d9b048      mov   x0, 0x28                             ; '('
|      |:   0x06d9b04c      adrp  x1, 0x8ac1000
|      |:   0x06d9b050      add   x1, x1, 0xd30
|      |:   0x06d9b054      orr   x2, xzr, 1
|      |:   0x06d9b058      bl    fcn.04d32f40
|      |:   0x06d9b05c      str   x0, [var_18h]
|      |:   0x06d9b060      mov   x0, 0x48                             ; 'H'
|      |:   0x06d9b064      adrp  x1, 0x8c53000
|      |:   0x06d9b068      add   x1, x1, 0x128
|      |:   0x06d9b06c      orr   x2, xzr, 1
|      |:   0x06d9b070      bl    0x4d33650
|      |:   0x06d9b074      str   x0, [var_10h]
|      |:   0x06d9b078      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b07c      ldr   x3, [x26]
|      |:   0x06d9b080      adrp  x0, 0x4358000
|      |:   0x06d9b084      add   x0, x0, 0xd36
|      |:   0x06d9b088      mov   x1, 0x13
|      |:   0x06d9b08c      blr   x3
|      |:   0x06d9b090      str   x1, [var_28h]
|     ,===< 0x06d9b094      cbz   x1, 0x6d9b0f8
|     ||:   0x06d9b098      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b09c      str   x1, [x2, 0x10]
|     ||:   0x06d9b0a0      adrp  x27, 0xb48f000
|     ||:   0x06d9b0a4      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b0a8      cbz   w3, 0x6d9b0b8
|    |||:   0x06d9b0ac      ldr   x3, [x2, 8]                          ; [0x8:4]=0
|    |||:   0x06d9b0b0      bl    fcn.04da3500
|    |||:   0x06d9b0b4      stp   x0, x3, [x25]
|    `----> 0x06d9b0b8      str   x0, [x2, 8]
|     ||:   0x06d9b0bc      adrp  x0, 0x856f000
|     ||:   0x06d9b0c0      add   x0, x0, 0x950
|     ||:   0x06d9b0c4      bl    fcn.04d31c10
|     ||:   0x06d9b0c8      orr   x1, xzr, 1
|     ||:   0x06d9b0cc      strb  w1, [x0]
|     ||:   0x06d9b0d0      adrp  x27, 0xb48f000
|     ||:   0x06d9b0d4      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b0d8      cbnz  w1, 0x6d9b0e4
|    |||:   0x06d9b0dc      ldr   x2, [var_10h]                        ; [0x50:4]=64
|   ,=====< 0x06d9b0e0      b     0x6d9b0f4
|   |`----> 0x06d9b0e4      ldr   x2, [var_10h]                        ; [0x50:4]=64
|   | ||:   0x06d9b0e8      ldr   x3, [x2]
|   | ||:   0x06d9b0ec      bl    fcn.04da3500
|   | ||:   0x06d9b0f0      stp   x0, x3, [x25]
|   | ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b0e0
|   `-----> 0x06d9b0f4      str   x0, [x2]
|     `---> 0x06d9b0f8      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b0fc      ldr   x2, [x26]
|      |:   0x06d9b100      adrp  x0, 0x43de000
|      |:   0x06d9b104      add   x0, x0, 0x220
|      |:   0x06d9b108      mov   x1, 0x1d
|      |:   0x06d9b10c      blr   x2
|     ,===< 0x06d9b110      cbz   x1, 0x6d9b138
|     ||:   0x06d9b114      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b118      str   x1, [x2, 0x20]
|     ||:   0x06d9b11c      adrp  x27, 0xb48f000
|     ||:   0x06d9b120      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b124      cbz   w3, 0x6d9b134
|    |||:   0x06d9b128      ldr   x3, [x2, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
|    |||:   0x06d9b12c      bl    fcn.04da3500
|    |||:   0x06d9b130      stp   x0, x3, [x25]
|    `----> 0x06d9b134      str   x0, [x2, 0x18]
|     `---> 0x06d9b138      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b13c      ldr   x2, [x26]
|      |:   0x06d9b140      ldr   x3, [var_28h]                        ; [0x38:4]=0x400010
|      |:   0x06d9b144      cmp   x3, 0
|      |:   0x06d9b148      cset  x3, ne
|      |:   0x06d9b14c      cmp   x1, 0
|      |:   0x06d9b150      cset  x4, ne
|      |:   0x06d9b154      orr   x3, x4, x3
|      |:   0x06d9b158      strb  w3, [var_29h]
|      |:   0x06d9b15c      adrp  x0, 0x439b000
|      |:   0x06d9b160      add   x0, x0, 0x515
|      |:   0x06d9b164      mov   x1, 0x17
|      |:   0x06d9b168      blr   x2
|     ,===< 0x06d9b16c      cbz   x1, 0x6d9b194
|     ||:   0x06d9b170      ldr   x2, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b174      str   x1, [x2, 0x30]
|     ||:   0x06d9b178      adrp  x27, 0xb48f000
|     ||:   0x06d9b17c      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b180      cbz   w3, 0x6d9b190
|    |||:   0x06d9b184      ldr   x3, [x2, 0x28]                       ; [0x28:4]=0xace39d8
|    |||:   0x06d9b188      bl    fcn.04da3500
|    |||:   0x06d9b18c      stp   x0, x3, [x25]
|    `----> 0x06d9b190      str   x0, [x2, 0x28]
|     `---> 0x06d9b194      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b198      ldr   x2, [x26]
|      |:   0x06d9b19c      cmp   x1, 0
|      |:   0x06d9b1a0      cset  x3, ne
|      |:   0x06d9b1a4      ldrb  w4, [var_29h]                        ; [0x37:4]=0x40001000
|      |:   0x06d9b1a8      orr   x3, x4, x3
|      |:   0x06d9b1ac      strb  w3, [var_29h]
|      |:   0x06d9b1b0      adrp  x0, 0x439b000
|      |:   0x06d9b1b4      add   x0, x0, 0x52c
|      |:   0x06d9b1b8      mov   x1, 0x17
|      |:   0x06d9b1bc      blr   x2
|     ,===< 0x06d9b1c0      cbnz  x1, 0x6d9b1d0
|     ||:   0x06d9b1c4      ldr   x3, [var_10h]                        ; [0x50:4]=64
|     ||:   0x06d9b1c8      ldrb  w2, [var_29h]                        ; [0x37:4]=0x40001000
|    ,====< 0x06d9b1cc      b     0x6d9b22c
|    |`---> 0x06d9b1d0      bl    0x6d9cd90
|    |,===< 0x06d9b1d4      cbz   x0, 0x6d9b1e0
|    |||:   0x06d9b1d8      ldr   x2, [x0]
|   ,=====< 0x06d9b1dc      b     0x6d9b1e4
|   ||`---> 0x06d9b1e0      mov   x2, xzr
|   || |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1dc
|   `-----> 0x06d9b1e4      cmp   x2, 0
|    |,===< 0x06d9b1e8      b.le  0x6d9b21c
|    |||:   0x06d9b1ec      adrp  x27, 0xb48f000
|    |||:   0x06d9b1f0      ldr   w3, [x27, 0x450]                     ; [0x450:4]=0x58585858
|   ,=====< 0x06d9b1f4      cbnz  w3, 0x6d9b200
|   ||||:   0x06d9b1f8      ldr   x3, [var_10h]                        ; [0x50:4]=64
|  ,======< 0x06d9b1fc      b     0x6d9b214
|  |`-----> 0x06d9b200      ldr   x3, [var_10h]                        ; [0x50:4]=64
|  | |||:   0x06d9b204      ldr   x4, [x3, 0x38]                       ; [0x38:4]=0x400010
|  | |||:   0x06d9b208      bl    fcn.04da3500
|  | |||:   0x06d9b20c      stp   x0, x4, [x25]
|  | |||:   0x06d9b210      cmp   x2, 0
|  | |||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1fc
|  `------> 0x06d9b214      str   x0, [x3, 0x38]
|   ,=====< 0x06d9b218      b     0x6d9b220
|   ||`---> 0x06d9b21c      ldr   x3, [var_10h]                        ; [0x50:4]=64
|   || |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b218
|   `-----> 0x06d9b220      cset  x2, gt
|    | |:   0x06d9b224      ldrb  w4, [var_29h]                        ; [0x37:4]=0x40001000
|    | |:   0x06d9b228      orr   x2, x2, x4
|    | |:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b1cc
|    `,===< 0x06d9b22c      tbz   w2, 0, 0x6d9b258
|     ||:   0x06d9b230      adrp  x27, 0xb48f000
|     ||:   0x06d9b234      ldr   w2, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    ,====< 0x06d9b238      cbnz  w2, 0x6d9b244
|    |||:   0x06d9b23c      ldr   x2, [var_18h]                        ; [0x48:4]=64
|   ,=====< 0x06d9b240      b     0x6d9b254
|   |`----> 0x06d9b244      ldr   x2, [var_18h]                        ; [0x48:4]=64
|   | ||:   0x06d9b248      ldr   x4, [x2, 0x10]                       ; [0x10:4]=0xb70003
|   | ||:   0x06d9b24c      bl    fcn.04da3500
|   | ||:   0x06d9b250      stp   x3, x4, [x25]
|   | ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b240
|   `-----> 0x06d9b254      str   x3, [x2, 0x10]
|     `---> 0x06d9b258      ldr   x26, [var_20h]                       ; segment.PHDR
|      |:                                                              ; [0x40:4]=6 ; "\U00000006"
|      |:   0x06d9b25c      ldr   x2, [x26]
|      |:   0x06d9b260      adrp  x0, 0x439b000
|      |:   0x06d9b264      add   x0, x0, 0x543
|      |:   0x06d9b268      mov   x1, 0x17
|      |:   0x06d9b26c      blr   x2
|     ,===< 0x06d9b270      cbnz  x1, 0x6d9b28c
|     ||:   0x06d9b274      ldr   x26, [var_20h]                       ; segment.PHDR
|     ||:                                                              ; [0x40:4]=6 ; "\U00000006"
|     ||:   0x06d9b278      ldr   x2, [x26]
|     ||:   0x06d9b27c      adrp  x0, 0x42ec000
|     ||:   0x06d9b280      add   x0, x0, 0xb4b
|     ||:   0x06d9b284      mov   x1, 0xd
|     ||:   0x06d9b288      blr   x2
|    ,`---> 0x06d9b28c      cbz   x1, 0x6d9b2b8
|    | |:   0x06d9b290      ldr   x2, [arg_48h]                        ; [0x48:4]=64
|    | |:   0x06d9b294      str   x1, [x2, 8]
|    | |:   0x06d9b298      adrp  x27, 0xb48f000
|    | |:   0x06d9b29c      ldr   w1, [x27, 0x450]                     ; [0x450:4]=0x58585858
|    |,===< 0x06d9b2a0      cbz   w1, 0x6d9b2b0
|    |||:   0x06d9b2a4      ldr   x1, [x2]
|    |||:   0x06d9b2a8      bl    fcn.04da3500
|    |||:   0x06d9b2ac      stp   x0, x1, [x25]
|    |`---> 0x06d9b2b0      str   x0, [x2]
|    |,===< 0x06d9b2b4      b     0x6d9b2bc
|    `----> 0x06d9b2b8      ldr   x2, [var_18h]                        ; [0x48:4]=64
|     ||:   ; CODE XREF from fcn.06d9b028 @ 0x6d9b2b4
|     `---> 0x06d9b2bc      mov   x0, x2
|      |:   0x06d9b2c0      ldur  fp, [sp, -8]                         ; [0x8:4]=0
|      |:   0x06d9b2c4      ldr   lr, [sp], 0x60                       ; [0x60:4]=0x380
|      |:   0x06d9b2c8      ret
|      `--> 0x06d9b2cc      str   x0, [arg_8h]                         ; arg1
|       :   0x06d9b2d0      mov   x3, lr
|       :   0x06d9b2d4      bl    fcn.04da15a0
|       :   0x06d9b2d8      ldr   x0, [arg_8h]                         ; [0x8:4]=0
|       `=< 0x06d9b2dc      b     0x6d9b020
