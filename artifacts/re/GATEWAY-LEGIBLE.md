# gateway de Antigravity: codigo legible (strings + llamadas resueltas)


## GatewayConfig_.IsEnabled

```asm
0x06d92368  cbz   w3, 0x6d92378
0x06d9236c  ldp   x4, x5, [x0]
0x06d92370  bl    0x4da3500
0x06d92374  stp   x4, x5, [x25]
0x06d92378  adrp  x4, 0x8f93000
0x06d9237c  add   x4, x4, 0x8f0
0x06d92380  adrp  x5, 0x8f93000
0x06d92384  add   x5, x5, 0x7f8
0x06d92388  stp   x4, x5, [x0]                          ; arg1
0x06d9238c  adrp  x0, 0x8333000
0x06d92390  add   x0, x0, 0x110
0x06d92394  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92398  adrp  x2, 0x42be000
0x06d9239c  add   x2, x2, 0xd0f                      ; "manage_taskgrep_searchLC_MESSAGESLC_MONETARYPROGRAMDATASYSTEMDRIVEUSERPROFILEexe"
0x06d923a0  mov   x3, 0xb
0x06d923a4  bl    0x4d18e50
0x06d923a8  ldrsb x27, [x0]
0x06d923ac  adrp  x27, 0xb48f000
0x06d923b0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d923b4  cbz   w4, 0x6d923c4
0x06d923b8  ldp   x4, x5, [x0]
0x06d923bc  bl    0x4da3500
0x06d923c0  stp   x4, x5, [x25]
0x06d923c4  adrp  x4, 0x8f93000
0x06d923c8  add   x4, x4, 0x8e0
0x06d923cc  adrp  x5, 0x8f93000
0x06d923d0  add   x5, x5, 0x7e0
0x06d923d4  stp   x4, x5, [x0]
0x06d923d8  adrp  x27, 0x948c000
0x06d923dc  ldr   x4, [x27, 0xd20]                      ; [0xd20:4]=0x58585858
0x06d923e0  str   x4, [arg_28h]
0x06d923e4  adrp  x0, 0x8333000
0x06d923e8  add   x0, x0, 0x110
0x06d923ec  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d923f0  adrp  x2, 0x4326000
0x06d923f4  add   x2, x2, 0xbfe                      ; "manage_subagentsrewrite_scheduleDomain name or *non_fast_forwardunknown revision"
0x06d923f8  orr   x3, xzr, 0x10
0x06d923fc  bl    0x4d18e50
0x06d92400  ldrsb x27, [x0]
0x06d92404  adrp  x27, 0xb48f000
0x06d92408  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9240c  cbnz  w4, 0x6d92418
0x06d92410  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92418  ldp   x4, x5, [x0]
0x06d9241c  bl    0x4da3510
0x06d92420  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92424  stp   x6, x4, [x25]
0x06d92428  str   x5, [x25, 0x10]
0x06d92430  add   x4, x4, 0x7d8
0x06d92434  stp   x6, x4, [x0]
0x06d92438  adrp  x0, 0x8333000
0x06d9243c  add   x0, x0, 0x110
0x06d92440  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92444  adrp  x2, 0x42d6000
0x06d92448  add   x2, x2, 0x4dd                      ; "send_messagerestoring %swebp_encoderwebm_encoderSEVERITY_LOWstore %v: %vmarketpl"
0x06d9244c  orr   x3, xzr, 0xc
0x06d92450  bl    0x4d18e50
0x06d92454  ldrsb x27, [x0]
0x06d92458  adrp  x27, 0xb48f000
0x06d9245c  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92460  cbz   w4, 0x6d92470
0x06d92464  ldp   x4, x5, [x0]
0x06d92468  bl    0x4da3500
0x06d9246c  stp   x4, x5, [x25]
0x06d92470  adrp  x4, 0x8f93000
0x06d92474  add   x4, x4, 0x938
0x06d92478  adrp  x5, 0x8f93000
0x06d9247c  add   x5, x5, 0x830
0x06d92480  stp   x4, x5, [x0]
0x06d92484  adrp  x0, 0x8333000
0x06d92488  add   x0, x0, 0x110
0x06d9248c  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92490  adrp  x2, 0x42d6000
0x06d92494  add   x2, x2, 0x9a5                      ; "manage_inboxki_insertionread_file(*)AbsolutePathnpm test -- project-%02dProject "
0x06d92498  orr   x3, xzr, 0xc
0x06d9249c  bl    0x4d18e50
0x06d924a0  ldrsb x27, [x0]
0x06d924a4  adrp  x27, 0xb48f000
0x06d924a8  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d924ac  cbz   w4, 0x6d924bc
0x06d924b0  ldp   x4, x5, [x0]
0x06d924b4  bl    0x4da3500
0x06d924b8  stp   x4, x5, [x25]
0x06d924bc  adrp  x4, 0x8f93000
0x06d924c0  add   x4, x4, 0x8d8
0x06d924c4  adrp  x5, 0x8f93000
0x06d924c8  add   x5, x5, 0x7d0
0x06d924cc  stp   x4, x5, [x0]
0x06d924d0  adrp  x0, 0x8333000
0x06d924d4  add   x0, x0, 0x110
0x06d924d8  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d924dc  adrp  x2, 0x4279000
0x06d924e0  add   x2, x2, 0xd1b                      ; "scheduleLANGUAGELC_CTYPELC_PAPERNO_COLORHOMEPATHUSERNAMEread_urldescribeIsDaemon"
0x06d924e4  orr   x3, xzr, 8
0x06d924e8  bl    0x4d18e50
0x06d924ec  ldrsb x27, [x0]
0x06d924f0  adrp  x27, 0xb48f000
0x06d924f4  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d924f8  cbz   w4, 0x6d92508
0x06d924fc  ldp   x4, x5, [x0]
0x06d92500  bl    0x4da3500
0x06d92504  stp   x4, x5, [x25]
0x06d92508  adrp  x4, 0x8f93000
0x06d9250c  add   x4, x4, 0x918
0x06d92510  adrp  x5, 0x8f93000
0x06d92514  add   x5, x5, 0x818
0x06d92518  stp   x4, x5, [x0]
0x06d9251c  adrp  x27, 0x948c000
0x06d92520  ldr   x4, [x27, 0xd10]                      ; [0xd10:4]=0x58585858
0x06d92524  str   x4, [arg_28h]
0x06d92528  adrp  x0, 0x8333000
0x06d9252c  add   x0, x0, 0x110
0x06d92530  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92534  adrp  x2, 0x4314000
0x06d92538  add   x2, x2, 0x3ab                      ; "define_subagentDurationSecondsmodel not foundremote_rejectedTab.BuildPromptPROBE"
0x06d9253c  orr   x3, xzr, 0xf
0x06d92540  bl    0x4d18e50
0x06d92544  ldrsb x27, [x0]
0x06d92548  adrp  x27, 0xb48f000
0x06d9254c  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92550  cbnz  w4, 0x6d9255c
0x06d92554  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d9255c  ldp   x4, x5, [x0]
0x06d92560  bl    0x4da3510
0x06d92564  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92568  stp   x6, x4, [x25]
0x06d9256c  str   x5, [x25, 0x10]
0x06d92574  add   x4, x4, 0x790
0x06d92578  stp   x6, x4, [x0]
0x06d9257c  adrp  x0, 0x8333000
0x06d92580  add   x0, x0, 0x110
0x06d92584  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92588  adrp  x2, 0x4301000
0x06d9258c  add   x2, x2, 0xb0e                      ; "ask_permissionunsandboxed(*)write_file(%s)browser-visionexecute_url(*)C:\Program"
0x06d92590  orr   x3, xzr, 0xe
0x06d92594  bl    0x4d18e50
0x06d92598  ldrsb x27, [x0]
0x06d9259c  adrp  x27, 0xb48f000
0x06d925a0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d925a4  cbz   w4, 0x6d925b4
0x06d925a8  ldp   x4, x5, [x0]
0x06d925ac  bl    0x4da3500
0x06d925b0  stp   x4, x5, [x25]
0x06d925b4  adrp  x4, 0x8f93000
0x06d925b8  add   x4, x4, 0x880
0x06d925bc  adrp  x5, 0x8f93000
0x06d925c0  add   x5, x5, 0x768
0x06d925c4  stp   x4, x5, [x0]
0x06d925c8  adrp  x0, 0x8333000
0x06d925cc  add   x0, x0, 0x110
0x06d925d0  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d925d4  adrp  x2, 0x4376000
0x06d925d8  add   x2, x2, 0xfd8                      ; "ask_custom_permissioncheckpoint_validation/models?pageSize=1000MarkdownDoc30Sect"
0x06d925dc  mov   x3, 0x15
0x06d925e0  bl    0x4d18e50
0x06d925e4  ldrsb x27, [x0]
0x06d925e8  adrp  x27, 0xb48f000
0x06d925ec  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d925f0  cbz   w4, 0x6d92600
0x06d925f4  ldp   x4, x5, [x0]
0x06d925f8  bl    0x4da3500
0x06d925fc  stp   x4, x5, [x25]
0x06d92600  adrp  x4, 0x8f93000
0x06d92604  add   x4, x4, 0x878
0x06d92608  adrp  x5, 0x8f93000
0x06d9260c  add   x5, x5, 0x768
0x06d92610  stp   x4, x5, [x0]
0x06d92614  adrp  x0, 0x8333000
0x06d92618  add   x0, x0, 0x110
0x06d9261c  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92620  adrp  x2, 0x4327000
0x06d92624  add   x2, x2, 0xe                        ; "list_permissionsmessage_deliveryconversation_logteamwork_previewDeepInvestigator"
0x06d92628  orr   x3, xzr, 0x10
0x06d9262c  bl    0x4d18e50
0x06d92630  ldrsb x27, [x0]
0x06d92634  adrp  x27, 0xb48f000
0x06d92638  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9263c  cbnz  w4, 0x6d92648
0x06d92640  ldr   x2, [arg_70h]                         ; [0x30:4]=0
0x06d92648  adrp  x27, 0x957c000
0x06d9264c  ldr   x1, [x27, 0xee0]                      ; [0xee0:4]=0x58585858
0x06d92650  ldp   x2, x3, [x0]
0x06d92654  bl    0x4da3520
0x06d92658  stp   x2, x3, [x25]
0x06d9265c  ldr   x2, [arg_30h]                         ; [0x30:4]=0
0x06d92660  stp   x2, x1, [x25, 0x10]
0x06d92668  add   x1, x1, 0x8c8
0x06d9266c  adrp  x3, 0x8f93000
0x06d92670  add   x3, x3, 0x7c0
0x06d92674  stp   x1, x3, [x0]
0x06d92678  adrp  x27, 0x957c000
0x06d9267c  str   x2, [x27, 0xee0]
0x06d92680  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d92684  ldr   lr, [sp], 0x40                        ; segment.PHDR

```
## GatewayConfig_.IsRouteWebFetch

```asm
0x06d92368  cbz   w3, 0x6d92378
0x06d9236c  ldp   x4, x5, [x0]
0x06d92370  bl    0x4da3500
0x06d92374  stp   x4, x5, [x25]
0x06d92378  adrp  x4, 0x8f93000
0x06d9237c  add   x4, x4, 0x8f0
0x06d92380  adrp  x5, 0x8f93000
0x06d92384  add   x5, x5, 0x7f8
0x06d92388  stp   x4, x5, [x0]                          ; arg1
0x06d9238c  adrp  x0, 0x8333000
0x06d92390  add   x0, x0, 0x110
0x06d92394  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92398  adrp  x2, 0x42be000
0x06d9239c  add   x2, x2, 0xd0f                      ; "manage_taskgrep_searchLC_MESSAGESLC_MONETARYPROGRAMDATASYSTEMDRIVEUSERPROFILEexe"
0x06d923a0  mov   x3, 0xb
0x06d923a4  bl    0x4d18e50
0x06d923a8  ldrsb x27, [x0]
0x06d923ac  adrp  x27, 0xb48f000
0x06d923b0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d923b4  cbz   w4, 0x6d923c4
0x06d923b8  ldp   x4, x5, [x0]
0x06d923bc  bl    0x4da3500
0x06d923c0  stp   x4, x5, [x25]
0x06d923c4  adrp  x4, 0x8f93000
0x06d923c8  add   x4, x4, 0x8e0
0x06d923cc  adrp  x5, 0x8f93000
0x06d923d0  add   x5, x5, 0x7e0
0x06d923d4  stp   x4, x5, [x0]
0x06d923d8  adrp  x27, 0x948c000
0x06d923dc  ldr   x4, [x27, 0xd20]                      ; [0xd20:4]=0x58585858
0x06d923e0  str   x4, [arg_28h]
0x06d923e4  adrp  x0, 0x8333000
0x06d923e8  add   x0, x0, 0x110
0x06d923ec  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d923f0  adrp  x2, 0x4326000
0x06d923f4  add   x2, x2, 0xbfe                      ; "manage_subagentsrewrite_scheduleDomain name or *non_fast_forwardunknown revision"
0x06d923f8  orr   x3, xzr, 0x10
0x06d923fc  bl    0x4d18e50
0x06d92400  ldrsb x27, [x0]
0x06d92404  adrp  x27, 0xb48f000
0x06d92408  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9240c  cbnz  w4, 0x6d92418
0x06d92410  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92418  ldp   x4, x5, [x0]
0x06d9241c  bl    0x4da3510
0x06d92420  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92424  stp   x6, x4, [x25]
0x06d92428  str   x5, [x25, 0x10]
0x06d92430  add   x4, x4, 0x7d8
0x06d92434  stp   x6, x4, [x0]
0x06d92438  adrp  x0, 0x8333000
0x06d9243c  add   x0, x0, 0x110
0x06d92440  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92444  adrp  x2, 0x42d6000
0x06d92448  add   x2, x2, 0x4dd                      ; "send_messagerestoring %swebp_encoderwebm_encoderSEVERITY_LOWstore %v: %vmarketpl"
0x06d9244c  orr   x3, xzr, 0xc
0x06d92450  bl    0x4d18e50
0x06d92454  ldrsb x27, [x0]
0x06d92458  adrp  x27, 0xb48f000
0x06d9245c  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92460  cbz   w4, 0x6d92470
0x06d92464  ldp   x4, x5, [x0]
0x06d92468  bl    0x4da3500
0x06d9246c  stp   x4, x5, [x25]
0x06d92470  adrp  x4, 0x8f93000
0x06d92474  add   x4, x4, 0x938
0x06d92478  adrp  x5, 0x8f93000
0x06d9247c  add   x5, x5, 0x830
0x06d92480  stp   x4, x5, [x0]
0x06d92484  adrp  x0, 0x8333000
0x06d92488  add   x0, x0, 0x110
0x06d9248c  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92490  adrp  x2, 0x42d6000
0x06d92494  add   x2, x2, 0x9a5                      ; "manage_inboxki_insertionread_file(*)AbsolutePathnpm test -- project-%02dProject "
0x06d92498  orr   x3, xzr, 0xc
0x06d9249c  bl    0x4d18e50
0x06d924a0  ldrsb x27, [x0]
0x06d924a4  adrp  x27, 0xb48f000
0x06d924a8  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d924ac  cbz   w4, 0x6d924bc
0x06d924b0  ldp   x4, x5, [x0]
0x06d924b4  bl    0x4da3500
0x06d924b8  stp   x4, x5, [x25]
0x06d924bc  adrp  x4, 0x8f93000
0x06d924c0  add   x4, x4, 0x8d8
0x06d924c4  adrp  x5, 0x8f93000
0x06d924c8  add   x5, x5, 0x7d0
0x06d924cc  stp   x4, x5, [x0]
0x06d924d0  adrp  x0, 0x8333000
0x06d924d4  add   x0, x0, 0x110
0x06d924d8  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d924dc  adrp  x2, 0x4279000
0x06d924e0  add   x2, x2, 0xd1b                      ; "scheduleLANGUAGELC_CTYPELC_PAPERNO_COLORHOMEPATHUSERNAMEread_urldescribeIsDaemon"
0x06d924e4  orr   x3, xzr, 8
0x06d924e8  bl    0x4d18e50
0x06d924ec  ldrsb x27, [x0]
0x06d924f0  adrp  x27, 0xb48f000
0x06d924f4  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d924f8  cbz   w4, 0x6d92508
0x06d924fc  ldp   x4, x5, [x0]
0x06d92500  bl    0x4da3500
0x06d92504  stp   x4, x5, [x25]
0x06d92508  adrp  x4, 0x8f93000
0x06d9250c  add   x4, x4, 0x918
0x06d92510  adrp  x5, 0x8f93000
0x06d92514  add   x5, x5, 0x818
0x06d92518  stp   x4, x5, [x0]
0x06d9251c  adrp  x27, 0x948c000
0x06d92520  ldr   x4, [x27, 0xd10]                      ; [0xd10:4]=0x58585858
0x06d92524  str   x4, [arg_28h]
0x06d92528  adrp  x0, 0x8333000
0x06d9252c  add   x0, x0, 0x110
0x06d92530  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92534  adrp  x2, 0x4314000
0x06d92538  add   x2, x2, 0x3ab                      ; "define_subagentDurationSecondsmodel not foundremote_rejectedTab.BuildPromptPROBE"
0x06d9253c  orr   x3, xzr, 0xf
0x06d92540  bl    0x4d18e50
0x06d92544  ldrsb x27, [x0]
0x06d92548  adrp  x27, 0xb48f000
0x06d9254c  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92550  cbnz  w4, 0x6d9255c
0x06d92554  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d9255c  ldp   x4, x5, [x0]
0x06d92560  bl    0x4da3510
0x06d92564  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92568  stp   x6, x4, [x25]
0x06d9256c  str   x5, [x25, 0x10]
0x06d92574  add   x4, x4, 0x790
0x06d92578  stp   x6, x4, [x0]
0x06d9257c  adrp  x0, 0x8333000
0x06d92580  add   x0, x0, 0x110
0x06d92584  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92588  adrp  x2, 0x4301000
0x06d9258c  add   x2, x2, 0xb0e                      ; "ask_permissionunsandboxed(*)write_file(%s)browser-visionexecute_url(*)C:\Program"
0x06d92590  orr   x3, xzr, 0xe
0x06d92594  bl    0x4d18e50
0x06d92598  ldrsb x27, [x0]
0x06d9259c  adrp  x27, 0xb48f000
0x06d925a0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d925a4  cbz   w4, 0x6d925b4
0x06d925a8  ldp   x4, x5, [x0]
0x06d925ac  bl    0x4da3500
0x06d925b0  stp   x4, x5, [x25]
0x06d925b4  adrp  x4, 0x8f93000
0x06d925b8  add   x4, x4, 0x880
0x06d925bc  adrp  x5, 0x8f93000
0x06d925c0  add   x5, x5, 0x768
0x06d925c4  stp   x4, x5, [x0]
0x06d925c8  adrp  x0, 0x8333000
0x06d925cc  add   x0, x0, 0x110
0x06d925d0  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d925d4  adrp  x2, 0x4376000
0x06d925d8  add   x2, x2, 0xfd8                      ; "ask_custom_permissioncheckpoint_validation/models?pageSize=1000MarkdownDoc30Sect"
0x06d925dc  mov   x3, 0x15
0x06d925e0  bl    0x4d18e50
0x06d925e4  ldrsb x27, [x0]
0x06d925e8  adrp  x27, 0xb48f000
0x06d925ec  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d925f0  cbz   w4, 0x6d92600
0x06d925f4  ldp   x4, x5, [x0]
0x06d925f8  bl    0x4da3500
0x06d925fc  stp   x4, x5, [x25]
0x06d92600  adrp  x4, 0x8f93000
0x06d92604  add   x4, x4, 0x878
0x06d92608  adrp  x5, 0x8f93000
0x06d9260c  add   x5, x5, 0x768
0x06d92610  stp   x4, x5, [x0]
0x06d92614  adrp  x0, 0x8333000
0x06d92618  add   x0, x0, 0x110
0x06d9261c  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92620  adrp  x2, 0x4327000
0x06d92624  add   x2, x2, 0xe                        ; "list_permissionsmessage_deliveryconversation_logteamwork_previewDeepInvestigator"
0x06d92628  orr   x3, xzr, 0x10
0x06d9262c  bl    0x4d18e50
0x06d92630  ldrsb x27, [x0]
0x06d92634  adrp  x27, 0xb48f000
0x06d92638  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9263c  cbnz  w4, 0x6d92648
0x06d92640  ldr   x2, [arg_70h]                         ; [0x30:4]=0
0x06d92648  adrp  x27, 0x957c000
0x06d9264c  ldr   x1, [x27, 0xee0]                      ; [0xee0:4]=0x58585858
0x06d92650  ldp   x2, x3, [x0]
0x06d92654  bl    0x4da3520
0x06d92658  stp   x2, x3, [x25]
0x06d9265c  ldr   x2, [arg_30h]                         ; [0x30:4]=0
0x06d92660  stp   x2, x1, [x25, 0x10]
0x06d92668  add   x1, x1, 0x8c8
0x06d9266c  adrp  x3, 0x8f93000
0x06d92670  add   x3, x3, 0x7c0
0x06d92674  stp   x1, x3, [x0]
0x06d92678  adrp  x27, 0x957c000
0x06d9267c  str   x2, [x27, 0xee0]
0x06d92680  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d92684  ldr   lr, [sp], 0x40                        ; segment.PHDR

```
## AdminSettings_.IsGatewayEnabled

```asm
0x06d92368  cbz   w3, 0x6d92378
0x06d9236c  ldp   x4, x5, [x0]
0x06d92370  bl    0x4da3500
0x06d92374  stp   x4, x5, [x25]
0x06d92378  adrp  x4, 0x8f93000
0x06d9237c  add   x4, x4, 0x8f0
0x06d92380  adrp  x5, 0x8f93000
0x06d92384  add   x5, x5, 0x7f8
0x06d92388  stp   x4, x5, [x0]                          ; arg1
0x06d9238c  adrp  x0, 0x8333000
0x06d92390  add   x0, x0, 0x110
0x06d92394  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92398  adrp  x2, 0x42be000
0x06d9239c  add   x2, x2, 0xd0f                      ; "manage_taskgrep_searchLC_MESSAGESLC_MONETARYPROGRAMDATASYSTEMDRIVEUSERPROFILEexe"
0x06d923a0  mov   x3, 0xb
0x06d923a4  bl    0x4d18e50
0x06d923a8  ldrsb x27, [x0]
0x06d923ac  adrp  x27, 0xb48f000
0x06d923b0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d923b4  cbz   w4, 0x6d923c4
0x06d923b8  ldp   x4, x5, [x0]
0x06d923bc  bl    0x4da3500
0x06d923c0  stp   x4, x5, [x25]
0x06d923c4  adrp  x4, 0x8f93000
0x06d923c8  add   x4, x4, 0x8e0
0x06d923cc  adrp  x5, 0x8f93000
0x06d923d0  add   x5, x5, 0x7e0
0x06d923d4  stp   x4, x5, [x0]
0x06d923d8  adrp  x27, 0x948c000
0x06d923dc  ldr   x4, [x27, 0xd20]                      ; [0xd20:4]=0x58585858
0x06d923e0  str   x4, [arg_28h]
0x06d923e4  adrp  x0, 0x8333000
0x06d923e8  add   x0, x0, 0x110
0x06d923ec  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d923f0  adrp  x2, 0x4326000
0x06d923f4  add   x2, x2, 0xbfe                      ; "manage_subagentsrewrite_scheduleDomain name or *non_fast_forwardunknown revision"
0x06d923f8  orr   x3, xzr, 0x10
0x06d923fc  bl    0x4d18e50
0x06d92400  ldrsb x27, [x0]
0x06d92404  adrp  x27, 0xb48f000
0x06d92408  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9240c  cbnz  w4, 0x6d92418
0x06d92410  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92418  ldp   x4, x5, [x0]
0x06d9241c  bl    0x4da3510
0x06d92420  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92424  stp   x6, x4, [x25]
0x06d92428  str   x5, [x25, 0x10]
0x06d92430  add   x4, x4, 0x7d8
0x06d92434  stp   x6, x4, [x0]
0x06d92438  adrp  x0, 0x8333000
0x06d9243c  add   x0, x0, 0x110
0x06d92440  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92444  adrp  x2, 0x42d6000
0x06d92448  add   x2, x2, 0x4dd                      ; "send_messagerestoring %swebp_encoderwebm_encoderSEVERITY_LOWstore %v: %vmarketpl"
0x06d9244c  orr   x3, xzr, 0xc
0x06d92450  bl    0x4d18e50
0x06d92454  ldrsb x27, [x0]
0x06d92458  adrp  x27, 0xb48f000
0x06d9245c  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92460  cbz   w4, 0x6d92470
0x06d92464  ldp   x4, x5, [x0]
0x06d92468  bl    0x4da3500
0x06d9246c  stp   x4, x5, [x25]
0x06d92470  adrp  x4, 0x8f93000
0x06d92474  add   x4, x4, 0x938
0x06d92478  adrp  x5, 0x8f93000
0x06d9247c  add   x5, x5, 0x830
0x06d92480  stp   x4, x5, [x0]
0x06d92484  adrp  x0, 0x8333000
0x06d92488  add   x0, x0, 0x110
0x06d9248c  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92490  adrp  x2, 0x42d6000
0x06d92494  add   x2, x2, 0x9a5                      ; "manage_inboxki_insertionread_file(*)AbsolutePathnpm test -- project-%02dProject "
0x06d92498  orr   x3, xzr, 0xc
0x06d9249c  bl    0x4d18e50
0x06d924a0  ldrsb x27, [x0]
0x06d924a4  adrp  x27, 0xb48f000
0x06d924a8  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d924ac  cbz   w4, 0x6d924bc
0x06d924b0  ldp   x4, x5, [x0]
0x06d924b4  bl    0x4da3500
0x06d924b8  stp   x4, x5, [x25]
0x06d924bc  adrp  x4, 0x8f93000
0x06d924c0  add   x4, x4, 0x8d8
0x06d924c4  adrp  x5, 0x8f93000
0x06d924c8  add   x5, x5, 0x7d0
0x06d924cc  stp   x4, x5, [x0]
0x06d924d0  adrp  x0, 0x8333000
0x06d924d4  add   x0, x0, 0x110
0x06d924d8  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d924dc  adrp  x2, 0x4279000
0x06d924e0  add   x2, x2, 0xd1b                      ; "scheduleLANGUAGELC_CTYPELC_PAPERNO_COLORHOMEPATHUSERNAMEread_urldescribeIsDaemon"
0x06d924e4  orr   x3, xzr, 8
0x06d924e8  bl    0x4d18e50
0x06d924ec  ldrsb x27, [x0]
0x06d924f0  adrp  x27, 0xb48f000
0x06d924f4  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d924f8  cbz   w4, 0x6d92508
0x06d924fc  ldp   x4, x5, [x0]
0x06d92500  bl    0x4da3500
0x06d92504  stp   x4, x5, [x25]
0x06d92508  adrp  x4, 0x8f93000
0x06d9250c  add   x4, x4, 0x918
0x06d92510  adrp  x5, 0x8f93000
0x06d92514  add   x5, x5, 0x818
0x06d92518  stp   x4, x5, [x0]
0x06d9251c  adrp  x27, 0x948c000
0x06d92520  ldr   x4, [x27, 0xd10]                      ; [0xd10:4]=0x58585858
0x06d92524  str   x4, [arg_28h]
0x06d92528  adrp  x0, 0x8333000
0x06d9252c  add   x0, x0, 0x110
0x06d92530  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92534  adrp  x2, 0x4314000
0x06d92538  add   x2, x2, 0x3ab                      ; "define_subagentDurationSecondsmodel not foundremote_rejectedTab.BuildPromptPROBE"
0x06d9253c  orr   x3, xzr, 0xf
0x06d92540  bl    0x4d18e50
0x06d92544  ldrsb x27, [x0]
0x06d92548  adrp  x27, 0xb48f000
0x06d9254c  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92550  cbnz  w4, 0x6d9255c
0x06d92554  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d9255c  ldp   x4, x5, [x0]
0x06d92560  bl    0x4da3510
0x06d92564  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92568  stp   x6, x4, [x25]
0x06d9256c  str   x5, [x25, 0x10]
0x06d92574  add   x4, x4, 0x790
0x06d92578  stp   x6, x4, [x0]
0x06d9257c  adrp  x0, 0x8333000
0x06d92580  add   x0, x0, 0x110
0x06d92584  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92588  adrp  x2, 0x4301000
0x06d9258c  add   x2, x2, 0xb0e                      ; "ask_permissionunsandboxed(*)write_file(%s)browser-visionexecute_url(*)C:\Program"
0x06d92590  orr   x3, xzr, 0xe
0x06d92594  bl    0x4d18e50
0x06d92598  ldrsb x27, [x0]
0x06d9259c  adrp  x27, 0xb48f000
0x06d925a0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d925a4  cbz   w4, 0x6d925b4
0x06d925a8  ldp   x4, x5, [x0]
0x06d925ac  bl    0x4da3500
0x06d925b0  stp   x4, x5, [x25]
0x06d925b4  adrp  x4, 0x8f93000
0x06d925b8  add   x4, x4, 0x880
0x06d925bc  adrp  x5, 0x8f93000
0x06d925c0  add   x5, x5, 0x768
0x06d925c4  stp   x4, x5, [x0]
0x06d925c8  adrp  x0, 0x8333000
0x06d925cc  add   x0, x0, 0x110
0x06d925d0  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d925d4  adrp  x2, 0x4376000
0x06d925d8  add   x2, x2, 0xfd8                      ; "ask_custom_permissioncheckpoint_validation/models?pageSize=1000MarkdownDoc30Sect"
0x06d925dc  mov   x3, 0x15
0x06d925e0  bl    0x4d18e50
0x06d925e4  ldrsb x27, [x0]
0x06d925e8  adrp  x27, 0xb48f000
0x06d925ec  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d925f0  cbz   w4, 0x6d92600
0x06d925f4  ldp   x4, x5, [x0]
0x06d925f8  bl    0x4da3500
0x06d925fc  stp   x4, x5, [x25]
0x06d92600  adrp  x4, 0x8f93000
0x06d92604  add   x4, x4, 0x878
0x06d92608  adrp  x5, 0x8f93000
0x06d9260c  add   x5, x5, 0x768
0x06d92610  stp   x4, x5, [x0]
0x06d92614  adrp  x0, 0x8333000
0x06d92618  add   x0, x0, 0x110
0x06d9261c  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92620  adrp  x2, 0x4327000
0x06d92624  add   x2, x2, 0xe                        ; "list_permissionsmessage_deliveryconversation_logteamwork_previewDeepInvestigator"
0x06d92628  orr   x3, xzr, 0x10
0x06d9262c  bl    0x4d18e50
0x06d92630  ldrsb x27, [x0]
0x06d92634  adrp  x27, 0xb48f000
0x06d92638  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9263c  cbnz  w4, 0x6d92648
0x06d92640  ldr   x2, [arg_70h]                         ; [0x30:4]=0
0x06d92648  adrp  x27, 0x957c000
0x06d9264c  ldr   x1, [x27, 0xee0]                      ; [0xee0:4]=0x58585858
0x06d92650  ldp   x2, x3, [x0]
0x06d92654  bl    0x4da3520
0x06d92658  stp   x2, x3, [x25]
0x06d9265c  ldr   x2, [arg_30h]                         ; [0x30:4]=0
0x06d92660  stp   x2, x1, [x25, 0x10]
0x06d92668  add   x1, x1, 0x8c8
0x06d9266c  adrp  x3, 0x8f93000
0x06d92670  add   x3, x3, 0x7c0
0x06d92674  stp   x1, x3, [x0]
0x06d92678  adrp  x27, 0x957c000
0x06d9267c  str   x2, [x27, 0xee0]
0x06d92680  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d92684  ldr   lr, [sp], 0x40                        ; segment.PHDR

```
## AdminSettings_.AdminFileSetsModels

```asm
0x06d92368  cbz   w3, 0x6d92378
0x06d9236c  ldp   x4, x5, [x0]
0x06d92370  bl    0x4da3500
0x06d92374  stp   x4, x5, [x25]
0x06d92378  adrp  x4, 0x8f93000
0x06d9237c  add   x4, x4, 0x8f0
0x06d92380  adrp  x5, 0x8f93000
0x06d92384  add   x5, x5, 0x7f8
0x06d92388  stp   x4, x5, [x0]                          ; arg1
0x06d9238c  adrp  x0, 0x8333000
0x06d92390  add   x0, x0, 0x110
0x06d92394  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92398  adrp  x2, 0x42be000
0x06d9239c  add   x2, x2, 0xd0f                      ; "manage_taskgrep_searchLC_MESSAGESLC_MONETARYPROGRAMDATASYSTEMDRIVEUSERPROFILEexe"
0x06d923a0  mov   x3, 0xb
0x06d923a4  bl    0x4d18e50
0x06d923a8  ldrsb x27, [x0]
0x06d923ac  adrp  x27, 0xb48f000
0x06d923b0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d923b4  cbz   w4, 0x6d923c4
0x06d923b8  ldp   x4, x5, [x0]
0x06d923bc  bl    0x4da3500
0x06d923c0  stp   x4, x5, [x25]
0x06d923c4  adrp  x4, 0x8f93000
0x06d923c8  add   x4, x4, 0x8e0
0x06d923cc  adrp  x5, 0x8f93000
0x06d923d0  add   x5, x5, 0x7e0
0x06d923d4  stp   x4, x5, [x0]
0x06d923d8  adrp  x27, 0x948c000
0x06d923dc  ldr   x4, [x27, 0xd20]                      ; [0xd20:4]=0x58585858
0x06d923e0  str   x4, [arg_28h]
0x06d923e4  adrp  x0, 0x8333000
0x06d923e8  add   x0, x0, 0x110
0x06d923ec  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d923f0  adrp  x2, 0x4326000
0x06d923f4  add   x2, x2, 0xbfe                      ; "manage_subagentsrewrite_scheduleDomain name or *non_fast_forwardunknown revision"
0x06d923f8  orr   x3, xzr, 0x10
0x06d923fc  bl    0x4d18e50
0x06d92400  ldrsb x27, [x0]
0x06d92404  adrp  x27, 0xb48f000
0x06d92408  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9240c  cbnz  w4, 0x6d92418
0x06d92410  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92418  ldp   x4, x5, [x0]
0x06d9241c  bl    0x4da3510
0x06d92420  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92424  stp   x6, x4, [x25]
0x06d92428  str   x5, [x25, 0x10]
0x06d92430  add   x4, x4, 0x7d8
0x06d92434  stp   x6, x4, [x0]
0x06d92438  adrp  x0, 0x8333000
0x06d9243c  add   x0, x0, 0x110
0x06d92440  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92444  adrp  x2, 0x42d6000
0x06d92448  add   x2, x2, 0x4dd                      ; "send_messagerestoring %swebp_encoderwebm_encoderSEVERITY_LOWstore %v: %vmarketpl"
0x06d9244c  orr   x3, xzr, 0xc
0x06d92450  bl    0x4d18e50
0x06d92454  ldrsb x27, [x0]
0x06d92458  adrp  x27, 0xb48f000
0x06d9245c  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92460  cbz   w4, 0x6d92470
0x06d92464  ldp   x4, x5, [x0]
0x06d92468  bl    0x4da3500
0x06d9246c  stp   x4, x5, [x25]
0x06d92470  adrp  x4, 0x8f93000
0x06d92474  add   x4, x4, 0x938
0x06d92478  adrp  x5, 0x8f93000
0x06d9247c  add   x5, x5, 0x830
0x06d92480  stp   x4, x5, [x0]
0x06d92484  adrp  x0, 0x8333000
0x06d92488  add   x0, x0, 0x110
0x06d9248c  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92490  adrp  x2, 0x42d6000
0x06d92494  add   x2, x2, 0x9a5                      ; "manage_inboxki_insertionread_file(*)AbsolutePathnpm test -- project-%02dProject "
0x06d92498  orr   x3, xzr, 0xc
0x06d9249c  bl    0x4d18e50
0x06d924a0  ldrsb x27, [x0]
0x06d924a4  adrp  x27, 0xb48f000
0x06d924a8  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d924ac  cbz   w4, 0x6d924bc
0x06d924b0  ldp   x4, x5, [x0]
0x06d924b4  bl    0x4da3500
0x06d924b8  stp   x4, x5, [x25]
0x06d924bc  adrp  x4, 0x8f93000
0x06d924c0  add   x4, x4, 0x8d8
0x06d924c4  adrp  x5, 0x8f93000
0x06d924c8  add   x5, x5, 0x7d0
0x06d924cc  stp   x4, x5, [x0]
0x06d924d0  adrp  x0, 0x8333000
0x06d924d4  add   x0, x0, 0x110
0x06d924d8  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d924dc  adrp  x2, 0x4279000
0x06d924e0  add   x2, x2, 0xd1b                      ; "scheduleLANGUAGELC_CTYPELC_PAPERNO_COLORHOMEPATHUSERNAMEread_urldescribeIsDaemon"
0x06d924e4  orr   x3, xzr, 8
0x06d924e8  bl    0x4d18e50
0x06d924ec  ldrsb x27, [x0]
0x06d924f0  adrp  x27, 0xb48f000
0x06d924f4  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d924f8  cbz   w4, 0x6d92508
0x06d924fc  ldp   x4, x5, [x0]
0x06d92500  bl    0x4da3500
0x06d92504  stp   x4, x5, [x25]
0x06d92508  adrp  x4, 0x8f93000
0x06d9250c  add   x4, x4, 0x918
0x06d92510  adrp  x5, 0x8f93000
0x06d92514  add   x5, x5, 0x818
0x06d92518  stp   x4, x5, [x0]
0x06d9251c  adrp  x27, 0x948c000
0x06d92520  ldr   x4, [x27, 0xd10]                      ; [0xd10:4]=0x58585858
0x06d92524  str   x4, [arg_28h]
0x06d92528  adrp  x0, 0x8333000
0x06d9252c  add   x0, x0, 0x110
0x06d92530  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92534  adrp  x2, 0x4314000
0x06d92538  add   x2, x2, 0x3ab                      ; "define_subagentDurationSecondsmodel not foundremote_rejectedTab.BuildPromptPROBE"
0x06d9253c  orr   x3, xzr, 0xf
0x06d92540  bl    0x4d18e50
0x06d92544  ldrsb x27, [x0]
0x06d92548  adrp  x27, 0xb48f000
0x06d9254c  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92550  cbnz  w4, 0x6d9255c
0x06d92554  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d9255c  ldp   x4, x5, [x0]
0x06d92560  bl    0x4da3510
0x06d92564  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92568  stp   x6, x4, [x25]
0x06d9256c  str   x5, [x25, 0x10]
0x06d92574  add   x4, x4, 0x790
0x06d92578  stp   x6, x4, [x0]
0x06d9257c  adrp  x0, 0x8333000
0x06d92580  add   x0, x0, 0x110
0x06d92584  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92588  adrp  x2, 0x4301000
0x06d9258c  add   x2, x2, 0xb0e                      ; "ask_permissionunsandboxed(*)write_file(%s)browser-visionexecute_url(*)C:\Program"
0x06d92590  orr   x3, xzr, 0xe
0x06d92594  bl    0x4d18e50
0x06d92598  ldrsb x27, [x0]
0x06d9259c  adrp  x27, 0xb48f000
0x06d925a0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d925a4  cbz   w4, 0x6d925b4
0x06d925a8  ldp   x4, x5, [x0]
0x06d925ac  bl    0x4da3500
0x06d925b0  stp   x4, x5, [x25]
0x06d925b4  adrp  x4, 0x8f93000
0x06d925b8  add   x4, x4, 0x880
0x06d925bc  adrp  x5, 0x8f93000
0x06d925c0  add   x5, x5, 0x768
0x06d925c4  stp   x4, x5, [x0]
0x06d925c8  adrp  x0, 0x8333000
0x06d925cc  add   x0, x0, 0x110
0x06d925d0  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d925d4  adrp  x2, 0x4376000
0x06d925d8  add   x2, x2, 0xfd8                      ; "ask_custom_permissioncheckpoint_validation/models?pageSize=1000MarkdownDoc30Sect"
0x06d925dc  mov   x3, 0x15
0x06d925e0  bl    0x4d18e50
0x06d925e4  ldrsb x27, [x0]
0x06d925e8  adrp  x27, 0xb48f000
0x06d925ec  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d925f0  cbz   w4, 0x6d92600
0x06d925f4  ldp   x4, x5, [x0]
0x06d925f8  bl    0x4da3500
0x06d925fc  stp   x4, x5, [x25]
0x06d92600  adrp  x4, 0x8f93000
0x06d92604  add   x4, x4, 0x878
0x06d92608  adrp  x5, 0x8f93000
0x06d9260c  add   x5, x5, 0x768
0x06d92610  stp   x4, x5, [x0]
0x06d92614  adrp  x0, 0x8333000
0x06d92618  add   x0, x0, 0x110
0x06d9261c  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92620  adrp  x2, 0x4327000
0x06d92624  add   x2, x2, 0xe                        ; "list_permissionsmessage_deliveryconversation_logteamwork_previewDeepInvestigator"
0x06d92628  orr   x3, xzr, 0x10
0x06d9262c  bl    0x4d18e50
0x06d92630  ldrsb x27, [x0]
0x06d92634  adrp  x27, 0xb48f000
0x06d92638  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9263c  cbnz  w4, 0x6d92648
0x06d92640  ldr   x2, [arg_70h]                         ; [0x30:4]=0
0x06d92648  adrp  x27, 0x957c000
0x06d9264c  ldr   x1, [x27, 0xee0]                      ; [0xee0:4]=0x58585858
0x06d92650  ldp   x2, x3, [x0]
0x06d92654  bl    0x4da3520
0x06d92658  stp   x2, x3, [x25]
0x06d9265c  ldr   x2, [arg_30h]                         ; [0x30:4]=0
0x06d92660  stp   x2, x1, [x25, 0x10]
0x06d92668  add   x1, x1, 0x8c8
0x06d9266c  adrp  x3, 0x8f93000
0x06d92670  add   x3, x3, 0x7c0
0x06d92674  stp   x1, x3, [x0]
0x06d92678  adrp  x27, 0x957c000
0x06d9267c  str   x2, [x27, 0xee0]
0x06d92680  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d92684  ldr   lr, [sp], 0x40                        ; segment.PHDR

```
## google3_third_party_jetski_gateway_gateway.defaultAdminSettingsPathForOS

```asm
0x06d92368  cbz   w3, 0x6d92378
0x06d9236c  ldp   x4, x5, [x0]
0x06d92370  bl    0x4da3500
0x06d92374  stp   x4, x5, [x25]
0x06d92378  adrp  x4, 0x8f93000
0x06d9237c  add   x4, x4, 0x8f0
0x06d92380  adrp  x5, 0x8f93000
0x06d92384  add   x5, x5, 0x7f8
0x06d92388  stp   x4, x5, [x0]                          ; arg1
0x06d9238c  adrp  x0, 0x8333000
0x06d92390  add   x0, x0, 0x110
0x06d92394  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92398  adrp  x2, 0x42be000
0x06d9239c  add   x2, x2, 0xd0f                      ; "manage_taskgrep_searchLC_MESSAGESLC_MONETARYPROGRAMDATASYSTEMDRIVEUSERPROFILEexe"
0x06d923a0  mov   x3, 0xb
0x06d923a4  bl    0x4d18e50
0x06d923a8  ldrsb x27, [x0]
0x06d923ac  adrp  x27, 0xb48f000
0x06d923b0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d923b4  cbz   w4, 0x6d923c4
0x06d923b8  ldp   x4, x5, [x0]
0x06d923bc  bl    0x4da3500
0x06d923c0  stp   x4, x5, [x25]
0x06d923c4  adrp  x4, 0x8f93000
0x06d923c8  add   x4, x4, 0x8e0
0x06d923cc  adrp  x5, 0x8f93000
0x06d923d0  add   x5, x5, 0x7e0
0x06d923d4  stp   x4, x5, [x0]
0x06d923d8  adrp  x27, 0x948c000
0x06d923dc  ldr   x4, [x27, 0xd20]                      ; [0xd20:4]=0x58585858
0x06d923e0  str   x4, [arg_28h]
0x06d923e4  adrp  x0, 0x8333000
0x06d923e8  add   x0, x0, 0x110
0x06d923ec  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d923f0  adrp  x2, 0x4326000
0x06d923f4  add   x2, x2, 0xbfe                      ; "manage_subagentsrewrite_scheduleDomain name or *non_fast_forwardunknown revision"
0x06d923f8  orr   x3, xzr, 0x10
0x06d923fc  bl    0x4d18e50
0x06d92400  ldrsb x27, [x0]
0x06d92404  adrp  x27, 0xb48f000
0x06d92408  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9240c  cbnz  w4, 0x6d92418
0x06d92410  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92418  ldp   x4, x5, [x0]
0x06d9241c  bl    0x4da3510
0x06d92420  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92424  stp   x6, x4, [x25]
0x06d92428  str   x5, [x25, 0x10]
0x06d92430  add   x4, x4, 0x7d8
0x06d92434  stp   x6, x4, [x0]
0x06d92438  adrp  x0, 0x8333000
0x06d9243c  add   x0, x0, 0x110
0x06d92440  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92444  adrp  x2, 0x42d6000
0x06d92448  add   x2, x2, 0x4dd                      ; "send_messagerestoring %swebp_encoderwebm_encoderSEVERITY_LOWstore %v: %vmarketpl"
0x06d9244c  orr   x3, xzr, 0xc
0x06d92450  bl    0x4d18e50
0x06d92454  ldrsb x27, [x0]
0x06d92458  adrp  x27, 0xb48f000
0x06d9245c  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92460  cbz   w4, 0x6d92470
0x06d92464  ldp   x4, x5, [x0]
0x06d92468  bl    0x4da3500
0x06d9246c  stp   x4, x5, [x25]
0x06d92470  adrp  x4, 0x8f93000
0x06d92474  add   x4, x4, 0x938
0x06d92478  adrp  x5, 0x8f93000
0x06d9247c  add   x5, x5, 0x830
0x06d92480  stp   x4, x5, [x0]
0x06d92484  adrp  x0, 0x8333000
0x06d92488  add   x0, x0, 0x110
0x06d9248c  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92490  adrp  x2, 0x42d6000
0x06d92494  add   x2, x2, 0x9a5                      ; "manage_inboxki_insertionread_file(*)AbsolutePathnpm test -- project-%02dProject "
0x06d92498  orr   x3, xzr, 0xc
0x06d9249c  bl    0x4d18e50
0x06d924a0  ldrsb x27, [x0]
0x06d924a4  adrp  x27, 0xb48f000
0x06d924a8  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d924ac  cbz   w4, 0x6d924bc
0x06d924b0  ldp   x4, x5, [x0]
0x06d924b4  bl    0x4da3500
0x06d924b8  stp   x4, x5, [x25]
0x06d924bc  adrp  x4, 0x8f93000
0x06d924c0  add   x4, x4, 0x8d8
0x06d924c4  adrp  x5, 0x8f93000
0x06d924c8  add   x5, x5, 0x7d0
0x06d924cc  stp   x4, x5, [x0]
0x06d924d0  adrp  x0, 0x8333000
0x06d924d4  add   x0, x0, 0x110
0x06d924d8  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d924dc  adrp  x2, 0x4279000
0x06d924e0  add   x2, x2, 0xd1b                      ; "scheduleLANGUAGELC_CTYPELC_PAPERNO_COLORHOMEPATHUSERNAMEread_urldescribeIsDaemon"
0x06d924e4  orr   x3, xzr, 8
0x06d924e8  bl    0x4d18e50
0x06d924ec  ldrsb x27, [x0]
0x06d924f0  adrp  x27, 0xb48f000
0x06d924f4  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d924f8  cbz   w4, 0x6d92508
0x06d924fc  ldp   x4, x5, [x0]
0x06d92500  bl    0x4da3500
0x06d92504  stp   x4, x5, [x25]
0x06d92508  adrp  x4, 0x8f93000
0x06d9250c  add   x4, x4, 0x918
0x06d92510  adrp  x5, 0x8f93000
0x06d92514  add   x5, x5, 0x818
0x06d92518  stp   x4, x5, [x0]
0x06d9251c  adrp  x27, 0x948c000
0x06d92520  ldr   x4, [x27, 0xd10]                      ; [0xd10:4]=0x58585858
0x06d92524  str   x4, [arg_28h]
0x06d92528  adrp  x0, 0x8333000
0x06d9252c  add   x0, x0, 0x110
0x06d92530  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92534  adrp  x2, 0x4314000
0x06d92538  add   x2, x2, 0x3ab                      ; "define_subagentDurationSecondsmodel not foundremote_rejectedTab.BuildPromptPROBE"
0x06d9253c  orr   x3, xzr, 0xf
0x06d92540  bl    0x4d18e50
0x06d92544  ldrsb x27, [x0]
0x06d92548  adrp  x27, 0xb48f000
0x06d9254c  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92550  cbnz  w4, 0x6d9255c
0x06d92554  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d9255c  ldp   x4, x5, [x0]
0x06d92560  bl    0x4da3510
0x06d92564  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92568  stp   x6, x4, [x25]
0x06d9256c  str   x5, [x25, 0x10]
0x06d92574  add   x4, x4, 0x790
0x06d92578  stp   x6, x4, [x0]
0x06d9257c  adrp  x0, 0x8333000
0x06d92580  add   x0, x0, 0x110
0x06d92584  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92588  adrp  x2, 0x4301000
0x06d9258c  add   x2, x2, 0xb0e                      ; "ask_permissionunsandboxed(*)write_file(%s)browser-visionexecute_url(*)C:\Program"
0x06d92590  orr   x3, xzr, 0xe
0x06d92594  bl    0x4d18e50
0x06d92598  ldrsb x27, [x0]
0x06d9259c  adrp  x27, 0xb48f000
0x06d925a0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d925a4  cbz   w4, 0x6d925b4
0x06d925a8  ldp   x4, x5, [x0]
0x06d925ac  bl    0x4da3500
0x06d925b0  stp   x4, x5, [x25]
0x06d925b4  adrp  x4, 0x8f93000
0x06d925b8  add   x4, x4, 0x880
0x06d925bc  adrp  x5, 0x8f93000
0x06d925c0  add   x5, x5, 0x768
0x06d925c4  stp   x4, x5, [x0]
0x06d925c8  adrp  x0, 0x8333000
0x06d925cc  add   x0, x0, 0x110
0x06d925d0  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d925d4  adrp  x2, 0x4376000
0x06d925d8  add   x2, x2, 0xfd8                      ; "ask_custom_permissioncheckpoint_validation/models?pageSize=1000MarkdownDoc30Sect"
0x06d925dc  mov   x3, 0x15
0x06d925e0  bl    0x4d18e50
0x06d925e4  ldrsb x27, [x0]
0x06d925e8  adrp  x27, 0xb48f000
0x06d925ec  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d925f0  cbz   w4, 0x6d92600
0x06d925f4  ldp   x4, x5, [x0]
0x06d925f8  bl    0x4da3500
0x06d925fc  stp   x4, x5, [x25]
0x06d92600  adrp  x4, 0x8f93000
0x06d92604  add   x4, x4, 0x878
0x06d92608  adrp  x5, 0x8f93000
0x06d9260c  add   x5, x5, 0x768
0x06d92610  stp   x4, x5, [x0]
0x06d92614  adrp  x0, 0x8333000
0x06d92618  add   x0, x0, 0x110
0x06d9261c  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92620  adrp  x2, 0x4327000
0x06d92624  add   x2, x2, 0xe                        ; "list_permissionsmessage_deliveryconversation_logteamwork_previewDeepInvestigator"
0x06d92628  orr   x3, xzr, 0x10
0x06d9262c  bl    0x4d18e50
0x06d92630  ldrsb x27, [x0]
0x06d92634  adrp  x27, 0xb48f000
0x06d92638  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9263c  cbnz  w4, 0x6d92648
0x06d92640  ldr   x2, [arg_70h]                         ; [0x30:4]=0
0x06d92648  adrp  x27, 0x957c000
0x06d9264c  ldr   x1, [x27, 0xee0]                      ; [0xee0:4]=0x58585858
0x06d92650  ldp   x2, x3, [x0]
0x06d92654  bl    0x4da3520
0x06d92658  stp   x2, x3, [x25]
0x06d9265c  ldr   x2, [arg_30h]                         ; [0x30:4]=0
0x06d92660  stp   x2, x1, [x25, 0x10]
0x06d92668  add   x1, x1, 0x8c8
0x06d9266c  adrp  x3, 0x8f93000
0x06d92670  add   x3, x3, 0x7c0
0x06d92674  stp   x1, x3, [x0]
0x06d92678  adrp  x27, 0x957c000
0x06d9267c  str   x2, [x27, 0xee0]
0x06d92680  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d92684  ldr   lr, [sp], 0x40                        ; segment.PHDR

```
## google3_third_party_jetski_gateway_gateway.defaultUserSettingsPath

```asm
0x06d92368  cbz   w3, 0x6d92378
0x06d9236c  ldp   x4, x5, [x0]
0x06d92370  bl    0x4da3500
0x06d92374  stp   x4, x5, [x25]
0x06d92378  adrp  x4, 0x8f93000
0x06d9237c  add   x4, x4, 0x8f0
0x06d92380  adrp  x5, 0x8f93000
0x06d92384  add   x5, x5, 0x7f8
0x06d92388  stp   x4, x5, [x0]                          ; arg1
0x06d9238c  adrp  x0, 0x8333000
0x06d92390  add   x0, x0, 0x110
0x06d92394  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92398  adrp  x2, 0x42be000
0x06d9239c  add   x2, x2, 0xd0f                      ; "manage_taskgrep_searchLC_MESSAGESLC_MONETARYPROGRAMDATASYSTEMDRIVEUSERPROFILEexe"
0x06d923a0  mov   x3, 0xb
0x06d923a4  bl    0x4d18e50
0x06d923a8  ldrsb x27, [x0]
0x06d923ac  adrp  x27, 0xb48f000
0x06d923b0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d923b4  cbz   w4, 0x6d923c4
0x06d923b8  ldp   x4, x5, [x0]
0x06d923bc  bl    0x4da3500
0x06d923c0  stp   x4, x5, [x25]
0x06d923c4  adrp  x4, 0x8f93000
0x06d923c8  add   x4, x4, 0x8e0
0x06d923cc  adrp  x5, 0x8f93000
0x06d923d0  add   x5, x5, 0x7e0
0x06d923d4  stp   x4, x5, [x0]
0x06d923d8  adrp  x27, 0x948c000
0x06d923dc  ldr   x4, [x27, 0xd20]                      ; [0xd20:4]=0x58585858
0x06d923e0  str   x4, [arg_28h]
0x06d923e4  adrp  x0, 0x8333000
0x06d923e8  add   x0, x0, 0x110
0x06d923ec  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d923f0  adrp  x2, 0x4326000
0x06d923f4  add   x2, x2, 0xbfe                      ; "manage_subagentsrewrite_scheduleDomain name or *non_fast_forwardunknown revision"
0x06d923f8  orr   x3, xzr, 0x10
0x06d923fc  bl    0x4d18e50
0x06d92400  ldrsb x27, [x0]
0x06d92404  adrp  x27, 0xb48f000
0x06d92408  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9240c  cbnz  w4, 0x6d92418
0x06d92410  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92418  ldp   x4, x5, [x0]
0x06d9241c  bl    0x4da3510
0x06d92420  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92424  stp   x6, x4, [x25]
0x06d92428  str   x5, [x25, 0x10]
0x06d92430  add   x4, x4, 0x7d8
0x06d92434  stp   x6, x4, [x0]
0x06d92438  adrp  x0, 0x8333000
0x06d9243c  add   x0, x0, 0x110
0x06d92440  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92444  adrp  x2, 0x42d6000
0x06d92448  add   x2, x2, 0x4dd                      ; "send_messagerestoring %swebp_encoderwebm_encoderSEVERITY_LOWstore %v: %vmarketpl"
0x06d9244c  orr   x3, xzr, 0xc
0x06d92450  bl    0x4d18e50
0x06d92454  ldrsb x27, [x0]
0x06d92458  adrp  x27, 0xb48f000
0x06d9245c  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92460  cbz   w4, 0x6d92470
0x06d92464  ldp   x4, x5, [x0]
0x06d92468  bl    0x4da3500
0x06d9246c  stp   x4, x5, [x25]
0x06d92470  adrp  x4, 0x8f93000
0x06d92474  add   x4, x4, 0x938
0x06d92478  adrp  x5, 0x8f93000
0x06d9247c  add   x5, x5, 0x830
0x06d92480  stp   x4, x5, [x0]
0x06d92484  adrp  x0, 0x8333000
0x06d92488  add   x0, x0, 0x110
0x06d9248c  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92490  adrp  x2, 0x42d6000
0x06d92494  add   x2, x2, 0x9a5                      ; "manage_inboxki_insertionread_file(*)AbsolutePathnpm test -- project-%02dProject "
0x06d92498  orr   x3, xzr, 0xc
0x06d9249c  bl    0x4d18e50
0x06d924a0  ldrsb x27, [x0]
0x06d924a4  adrp  x27, 0xb48f000
0x06d924a8  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d924ac  cbz   w4, 0x6d924bc
0x06d924b0  ldp   x4, x5, [x0]
0x06d924b4  bl    0x4da3500
0x06d924b8  stp   x4, x5, [x25]
0x06d924bc  adrp  x4, 0x8f93000
0x06d924c0  add   x4, x4, 0x8d8
0x06d924c4  adrp  x5, 0x8f93000
0x06d924c8  add   x5, x5, 0x7d0
0x06d924cc  stp   x4, x5, [x0]
0x06d924d0  adrp  x0, 0x8333000
0x06d924d4  add   x0, x0, 0x110
0x06d924d8  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d924dc  adrp  x2, 0x4279000
0x06d924e0  add   x2, x2, 0xd1b                      ; "scheduleLANGUAGELC_CTYPELC_PAPERNO_COLORHOMEPATHUSERNAMEread_urldescribeIsDaemon"
0x06d924e4  orr   x3, xzr, 8
0x06d924e8  bl    0x4d18e50
0x06d924ec  ldrsb x27, [x0]
0x06d924f0  adrp  x27, 0xb48f000
0x06d924f4  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d924f8  cbz   w4, 0x6d92508
0x06d924fc  ldp   x4, x5, [x0]
0x06d92500  bl    0x4da3500
0x06d92504  stp   x4, x5, [x25]
0x06d92508  adrp  x4, 0x8f93000
0x06d9250c  add   x4, x4, 0x918
0x06d92510  adrp  x5, 0x8f93000
0x06d92514  add   x5, x5, 0x818
0x06d92518  stp   x4, x5, [x0]
0x06d9251c  adrp  x27, 0x948c000
0x06d92520  ldr   x4, [x27, 0xd10]                      ; [0xd10:4]=0x58585858
0x06d92524  str   x4, [arg_28h]
0x06d92528  adrp  x0, 0x8333000
0x06d9252c  add   x0, x0, 0x110
0x06d92530  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92534  adrp  x2, 0x4314000
0x06d92538  add   x2, x2, 0x3ab                      ; "define_subagentDurationSecondsmodel not foundremote_rejectedTab.BuildPromptPROBE"
0x06d9253c  orr   x3, xzr, 0xf
0x06d92540  bl    0x4d18e50
0x06d92544  ldrsb x27, [x0]
0x06d92548  adrp  x27, 0xb48f000
0x06d9254c  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92550  cbnz  w4, 0x6d9255c
0x06d92554  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d9255c  ldp   x4, x5, [x0]
0x06d92560  bl    0x4da3510
0x06d92564  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92568  stp   x6, x4, [x25]
0x06d9256c  str   x5, [x25, 0x10]
0x06d92574  add   x4, x4, 0x790
0x06d92578  stp   x6, x4, [x0]
0x06d9257c  adrp  x0, 0x8333000
0x06d92580  add   x0, x0, 0x110
0x06d92584  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92588  adrp  x2, 0x4301000
0x06d9258c  add   x2, x2, 0xb0e                      ; "ask_permissionunsandboxed(*)write_file(%s)browser-visionexecute_url(*)C:\Program"
0x06d92590  orr   x3, xzr, 0xe
0x06d92594  bl    0x4d18e50
0x06d92598  ldrsb x27, [x0]
0x06d9259c  adrp  x27, 0xb48f000
0x06d925a0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d925a4  cbz   w4, 0x6d925b4
0x06d925a8  ldp   x4, x5, [x0]
0x06d925ac  bl    0x4da3500
0x06d925b0  stp   x4, x5, [x25]
0x06d925b4  adrp  x4, 0x8f93000
0x06d925b8  add   x4, x4, 0x880
0x06d925bc  adrp  x5, 0x8f93000
0x06d925c0  add   x5, x5, 0x768
0x06d925c4  stp   x4, x5, [x0]
0x06d925c8  adrp  x0, 0x8333000
0x06d925cc  add   x0, x0, 0x110
0x06d925d0  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d925d4  adrp  x2, 0x4376000
0x06d925d8  add   x2, x2, 0xfd8                      ; "ask_custom_permissioncheckpoint_validation/models?pageSize=1000MarkdownDoc30Sect"
0x06d925dc  mov   x3, 0x15
0x06d925e0  bl    0x4d18e50
0x06d925e4  ldrsb x27, [x0]
0x06d925e8  adrp  x27, 0xb48f000
0x06d925ec  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d925f0  cbz   w4, 0x6d92600
0x06d925f4  ldp   x4, x5, [x0]
0x06d925f8  bl    0x4da3500
0x06d925fc  stp   x4, x5, [x25]
0x06d92600  adrp  x4, 0x8f93000
0x06d92604  add   x4, x4, 0x878
0x06d92608  adrp  x5, 0x8f93000
0x06d9260c  add   x5, x5, 0x768
0x06d92610  stp   x4, x5, [x0]
0x06d92614  adrp  x0, 0x8333000
0x06d92618  add   x0, x0, 0x110
0x06d9261c  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92620  adrp  x2, 0x4327000
0x06d92624  add   x2, x2, 0xe                        ; "list_permissionsmessage_deliveryconversation_logteamwork_previewDeepInvestigator"
0x06d92628  orr   x3, xzr, 0x10
0x06d9262c  bl    0x4d18e50
0x06d92630  ldrsb x27, [x0]
0x06d92634  adrp  x27, 0xb48f000
0x06d92638  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9263c  cbnz  w4, 0x6d92648
0x06d92640  ldr   x2, [arg_70h]                         ; [0x30:4]=0
0x06d92648  adrp  x27, 0x957c000
0x06d9264c  ldr   x1, [x27, 0xee0]                      ; [0xee0:4]=0x58585858
0x06d92650  ldp   x2, x3, [x0]
0x06d92654  bl    0x4da3520
0x06d92658  stp   x2, x3, [x25]
0x06d9265c  ldr   x2, [arg_30h]                         ; [0x30:4]=0
0x06d92660  stp   x2, x1, [x25, 0x10]
0x06d92668  add   x1, x1, 0x8c8
0x06d9266c  adrp  x3, 0x8f93000
0x06d92670  add   x3, x3, 0x7c0
0x06d92674  stp   x1, x3, [x0]
0x06d92678  adrp  x27, 0x957c000
0x06d9267c  str   x2, [x27, 0xee0]
0x06d92680  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d92684  ldr   lr, [sp], 0x40                        ; segment.PHDR

```
## AdminSettings_.Validate

```asm
0x06d92368  cbz   w3, 0x6d92378
0x06d9236c  ldp   x4, x5, [x0]
0x06d92370  bl    0x4da3500
0x06d92374  stp   x4, x5, [x25]
0x06d92378  adrp  x4, 0x8f93000
0x06d9237c  add   x4, x4, 0x8f0
0x06d92380  adrp  x5, 0x8f93000
0x06d92384  add   x5, x5, 0x7f8
0x06d92388  stp   x4, x5, [x0]                          ; arg1
0x06d9238c  adrp  x0, 0x8333000
0x06d92390  add   x0, x0, 0x110
0x06d92394  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92398  adrp  x2, 0x42be000
0x06d9239c  add   x2, x2, 0xd0f                      ; "manage_taskgrep_searchLC_MESSAGESLC_MONETARYPROGRAMDATASYSTEMDRIVEUSERPROFILEexe"
0x06d923a0  mov   x3, 0xb
0x06d923a4  bl    0x4d18e50
0x06d923a8  ldrsb x27, [x0]
0x06d923ac  adrp  x27, 0xb48f000
0x06d923b0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d923b4  cbz   w4, 0x6d923c4
0x06d923b8  ldp   x4, x5, [x0]
0x06d923bc  bl    0x4da3500
0x06d923c0  stp   x4, x5, [x25]
0x06d923c4  adrp  x4, 0x8f93000
0x06d923c8  add   x4, x4, 0x8e0
0x06d923cc  adrp  x5, 0x8f93000
0x06d923d0  add   x5, x5, 0x7e0
0x06d923d4  stp   x4, x5, [x0]
0x06d923d8  adrp  x27, 0x948c000
0x06d923dc  ldr   x4, [x27, 0xd20]                      ; [0xd20:4]=0x58585858
0x06d923e0  str   x4, [arg_28h]
0x06d923e4  adrp  x0, 0x8333000
0x06d923e8  add   x0, x0, 0x110
0x06d923ec  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d923f0  adrp  x2, 0x4326000
0x06d923f4  add   x2, x2, 0xbfe                      ; "manage_subagentsrewrite_scheduleDomain name or *non_fast_forwardunknown revision"
0x06d923f8  orr   x3, xzr, 0x10
0x06d923fc  bl    0x4d18e50
0x06d92400  ldrsb x27, [x0]
0x06d92404  adrp  x27, 0xb48f000
0x06d92408  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9240c  cbnz  w4, 0x6d92418
0x06d92410  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92418  ldp   x4, x5, [x0]
0x06d9241c  bl    0x4da3510
0x06d92420  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92424  stp   x6, x4, [x25]
0x06d92428  str   x5, [x25, 0x10]
0x06d92430  add   x4, x4, 0x7d8
0x06d92434  stp   x6, x4, [x0]
0x06d92438  adrp  x0, 0x8333000
0x06d9243c  add   x0, x0, 0x110
0x06d92440  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92444  adrp  x2, 0x42d6000
0x06d92448  add   x2, x2, 0x4dd                      ; "send_messagerestoring %swebp_encoderwebm_encoderSEVERITY_LOWstore %v: %vmarketpl"
0x06d9244c  orr   x3, xzr, 0xc
0x06d92450  bl    0x4d18e50
0x06d92454  ldrsb x27, [x0]
0x06d92458  adrp  x27, 0xb48f000
0x06d9245c  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92460  cbz   w4, 0x6d92470
0x06d92464  ldp   x4, x5, [x0]
0x06d92468  bl    0x4da3500
0x06d9246c  stp   x4, x5, [x25]
0x06d92470  adrp  x4, 0x8f93000
0x06d92474  add   x4, x4, 0x938
0x06d92478  adrp  x5, 0x8f93000
0x06d9247c  add   x5, x5, 0x830
0x06d92480  stp   x4, x5, [x0]
0x06d92484  adrp  x0, 0x8333000
0x06d92488  add   x0, x0, 0x110
0x06d9248c  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92490  adrp  x2, 0x42d6000
0x06d92494  add   x2, x2, 0x9a5                      ; "manage_inboxki_insertionread_file(*)AbsolutePathnpm test -- project-%02dProject "
0x06d92498  orr   x3, xzr, 0xc
0x06d9249c  bl    0x4d18e50
0x06d924a0  ldrsb x27, [x0]
0x06d924a4  adrp  x27, 0xb48f000
0x06d924a8  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d924ac  cbz   w4, 0x6d924bc
0x06d924b0  ldp   x4, x5, [x0]
0x06d924b4  bl    0x4da3500
0x06d924b8  stp   x4, x5, [x25]
0x06d924bc  adrp  x4, 0x8f93000
0x06d924c0  add   x4, x4, 0x8d8
0x06d924c4  adrp  x5, 0x8f93000
0x06d924c8  add   x5, x5, 0x7d0
0x06d924cc  stp   x4, x5, [x0]
0x06d924d0  adrp  x0, 0x8333000
0x06d924d4  add   x0, x0, 0x110
0x06d924d8  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d924dc  adrp  x2, 0x4279000
0x06d924e0  add   x2, x2, 0xd1b                      ; "scheduleLANGUAGELC_CTYPELC_PAPERNO_COLORHOMEPATHUSERNAMEread_urldescribeIsDaemon"
0x06d924e4  orr   x3, xzr, 8
0x06d924e8  bl    0x4d18e50
0x06d924ec  ldrsb x27, [x0]
0x06d924f0  adrp  x27, 0xb48f000
0x06d924f4  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d924f8  cbz   w4, 0x6d92508
0x06d924fc  ldp   x4, x5, [x0]
0x06d92500  bl    0x4da3500
0x06d92504  stp   x4, x5, [x25]
0x06d92508  adrp  x4, 0x8f93000
0x06d9250c  add   x4, x4, 0x918
0x06d92510  adrp  x5, 0x8f93000
0x06d92514  add   x5, x5, 0x818
0x06d92518  stp   x4, x5, [x0]
0x06d9251c  adrp  x27, 0x948c000
0x06d92520  ldr   x4, [x27, 0xd10]                      ; [0xd10:4]=0x58585858
0x06d92524  str   x4, [arg_28h]
0x06d92528  adrp  x0, 0x8333000
0x06d9252c  add   x0, x0, 0x110
0x06d92530  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92534  adrp  x2, 0x4314000
0x06d92538  add   x2, x2, 0x3ab                      ; "define_subagentDurationSecondsmodel not foundremote_rejectedTab.BuildPromptPROBE"
0x06d9253c  orr   x3, xzr, 0xf
0x06d92540  bl    0x4d18e50
0x06d92544  ldrsb x27, [x0]
0x06d92548  adrp  x27, 0xb48f000
0x06d9254c  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92550  cbnz  w4, 0x6d9255c
0x06d92554  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d9255c  ldp   x4, x5, [x0]
0x06d92560  bl    0x4da3510
0x06d92564  ldr   x6, [arg_28h]                         ; [0x28:4]=0xace39d8
0x06d92568  stp   x6, x4, [x25]
0x06d9256c  str   x5, [x25, 0x10]
0x06d92574  add   x4, x4, 0x790
0x06d92578  stp   x6, x4, [x0]
0x06d9257c  adrp  x0, 0x8333000
0x06d92580  add   x0, x0, 0x110
0x06d92584  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92588  adrp  x2, 0x4301000
0x06d9258c  add   x2, x2, 0xb0e                      ; "ask_permissionunsandboxed(*)write_file(%s)browser-visionexecute_url(*)C:\Program"
0x06d92590  orr   x3, xzr, 0xe
0x06d92594  bl    0x4d18e50
0x06d92598  ldrsb x27, [x0]
0x06d9259c  adrp  x27, 0xb48f000
0x06d925a0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d925a4  cbz   w4, 0x6d925b4
0x06d925a8  ldp   x4, x5, [x0]
0x06d925ac  bl    0x4da3500
0x06d925b0  stp   x4, x5, [x25]
0x06d925b4  adrp  x4, 0x8f93000
0x06d925b8  add   x4, x4, 0x880
0x06d925bc  adrp  x5, 0x8f93000
0x06d925c0  add   x5, x5, 0x768
0x06d925c4  stp   x4, x5, [x0]
0x06d925c8  adrp  x0, 0x8333000
0x06d925cc  add   x0, x0, 0x110
0x06d925d0  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d925d4  adrp  x2, 0x4376000
0x06d925d8  add   x2, x2, 0xfd8                      ; "ask_custom_permissioncheckpoint_validation/models?pageSize=1000MarkdownDoc30Sect"
0x06d925dc  mov   x3, 0x15
0x06d925e0  bl    0x4d18e50
0x06d925e4  ldrsb x27, [x0]
0x06d925e8  adrp  x27, 0xb48f000
0x06d925ec  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d925f0  cbz   w4, 0x6d92600
0x06d925f4  ldp   x4, x5, [x0]
0x06d925f8  bl    0x4da3500
0x06d925fc  stp   x4, x5, [x25]
0x06d92600  adrp  x4, 0x8f93000
0x06d92604  add   x4, x4, 0x878
0x06d92608  adrp  x5, 0x8f93000
0x06d9260c  add   x5, x5, 0x768
0x06d92610  stp   x4, x5, [x0]
0x06d92614  adrp  x0, 0x8333000
0x06d92618  add   x0, x0, 0x110
0x06d9261c  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92620  adrp  x2, 0x4327000
0x06d92624  add   x2, x2, 0xe                        ; "list_permissionsmessage_deliveryconversation_logteamwork_previewDeepInvestigator"
0x06d92628  orr   x3, xzr, 0x10
0x06d9262c  bl    0x4d18e50
0x06d92630  ldrsb x27, [x0]
0x06d92634  adrp  x27, 0xb48f000
0x06d92638  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9263c  cbnz  w4, 0x6d92648
0x06d92640  ldr   x2, [arg_70h]                         ; [0x30:4]=0
0x06d92648  adrp  x27, 0x957c000
0x06d9264c  ldr   x1, [x27, 0xee0]                      ; [0xee0:4]=0x58585858
0x06d92650  ldp   x2, x3, [x0]
0x06d92654  bl    0x4da3520
0x06d92658  stp   x2, x3, [x25]
0x06d9265c  ldr   x2, [arg_30h]                         ; [0x30:4]=0
0x06d92660  stp   x2, x1, [x25, 0x10]
0x06d92668  add   x1, x1, 0x8c8
0x06d9266c  adrp  x3, 0x8f93000
0x06d92670  add   x3, x3, 0x7c0
0x06d92674  stp   x1, x3, [x0]
0x06d92678  adrp  x27, 0x957c000
0x06d9267c  str   x2, [x27, 0xee0]
0x06d92680  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d92684  ldr   lr, [sp], 0x40                        ; segment.PHDR

```
## google3_third_party_jetski_gateway_gateway.ParseAdminSettings

```asm
0x06d92a88  bl    0x4d18e50
0x06d92a8c  adrp  x27, 0xb48f000
0x06d92a90  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92a94  cbz   w4, 0x6d92aa4
0x06d92a98  ldr   x4, [x0]
0x06d92a9c  bl    0x4da34f0
0x06d92aa0  str   x4, [x25]
0x06d92aa4  adrp  x4, 0x8f93000
0x06d92aa8  add   x4, x4, 0x6b0
0x06d92aac  str   x4, [x0]
0x06d92ab0  adrp  x0, 0x8332000
0x06d92ab4  add   x0, x0, 0x3c8
0x06d92ab8  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92abc  adrp  x2, 0x4367000
0x06d92ac0  add   x2, x2, 0xcd3                      ; "knowledge_generationa must come before bCodeArtifact800LinesReviewDiff120Modules"
0x06d92ac4  mov   x3, 0x14
0x06d92ac8  bl    0x4d18e50
0x06d92acc  adrp  x27, 0xb48f000
0x06d92ad0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92ad4  cbz   w4, 0x6d92ae4
0x06d92ad8  ldr   x4, [x0]
0x06d92adc  bl    0x4da34f0
0x06d92ae0  str   x4, [x25]
0x06d92ae4  adrp  x4, 0x8f93000
0x06d92ae8  add   x4, x4, 0x6b0
0x06d92aec  str   x4, [x0]
0x06d92af0  adrp  x0, 0x8332000
0x06d92af4  add   x0, x0, 0x3c8
0x06d92af8  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92afc  adrp  x2, 0x4358000
0x06d92b00  add   x2, x2, 0xcfd                      ; "knowledge_timestampunknown flagKind %dworkspace not foundAGY_LLM_GATEWAY_URLPyth"
0x06d92b04  mov   x3, 0x13
0x06d92b08  bl    0x4d18e50
0x06d92b0c  adrp  x27, 0xb48f000
0x06d92b10  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92b14  cbz   w4, 0x6d92b24
0x06d92b18  ldr   x4, [x0]
0x06d92b1c  bl    0x4da34f0
0x06d92b20  str   x4, [x25]
0x06d92b24  adrp  x4, 0x8f93000
0x06d92b28  add   x4, x4, 0x6b0
0x06d92b2c  str   x4, [x0]
0x06d92b30  adrp  x0, 0x8332000
0x06d92b34  add   x0, x0, 0x3c8
0x06d92b38  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92b3c  adrp  x2, 0x4388000
0x06d92b40  add   x2, x2, 0xfa3
0x06d92b44  mov   x3, 0x16
0x06d92b48  bl    0x4d18e50
0x06d92b4c  adrp  x27, 0xb48f000
0x06d92b50  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92b54  cbz   w4, 0x6d92b64
0x06d92b58  ldr   x4, [x0]
0x06d92b5c  bl    0x4da34f0
0x06d92b60  str   x4, [x25]
0x06d92b64  adrp  x4, 0x8f93000
0x06d92b68  add   x4, x4, 0x6d0
0x06d92b6c  str   x4, [x0]
0x06d92b70  adrp  x0, 0x8332000
0x06d92b74  add   x0, x0, 0x3c8
0x06d92b78  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92b7c  adrp  x2, 0x4348000
0x06d92b80  add   x2, x2, 0x682                      ; "no_tool_call_checkMigrating %s to %splugin-kill-switchgoogle_credentialsAGY_BROW"
0x06d92b84  mov   x3, 0x12
0x06d92b88  bl    0x4d18e50
0x06d92b8c  adrp  x27, 0xb48f000
0x06d92b90  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92b94  cbz   w4, 0x6d92ba4
0x06d92b98  ldr   x4, [x0]
0x06d92b9c  bl    0x4da34f0
0x06d92ba0  str   x4, [x25]
0x06d92ba4  adrp  x4, 0x8f93000
0x06d92ba8  add   x4, x4, 0x6d8
0x06d92bac  str   x4, [x0]
0x06d92bb0  adrp  x0, 0x8332000
0x06d92bb4  add   x0, x0, 0x3c8
0x06d92bb8  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92bbc  adrp  x2, 0x43ee000
0x06d92bc0  add   x2, x2, 0x9c0
0x06d92bc4  orr   x3, xzr, 0x1f
0x06d92bc8  bl    0x4d18e50
0x06d92bcc  adrp  x27, 0xb48f000
0x06d92bd0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92bd4  cbz   w4, 0x6d92be4
0x06d92bd8  ldr   x4, [x0]
0x06d92bdc  bl    0x4da34f0
0x06d92be0  str   x4, [x25]
0x06d92be4  adrp  x4, 0x8f93000
0x06d92be8  add   x4, x4, 0x6c8
0x06d92bec  str   x4, [x0]
0x06d92bf0  adrp  x0, 0x8332000
0x06d92bf4  add   x0, x0, 0x3c8
0x06d92bf8  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92bfc  adrp  x2, 0x4358000
0x06d92c00  add   x2, x2, 0x487
0x06d92c04  mov   x3, 0x13
0x06d92c08  bl    0x4d18e50
0x06d92c0c  adrp  x27, 0xb48f000
0x06d92c10  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92c14  cbnz  w4, 0x6d92c20
0x06d92c18  ldr   x3, [arg_70h]                         ; [0x30:4]=0
0x06d92c20  adrp  x27, 0x957c000
0x06d92c24  ldr   x1, [x27, 0xf00]                      ; [0xf00:4]=0x58585858
0x06d92c28  ldr   x2, [x0]
0x06d92c2c  bl    0x4da3510
0x06d92c30  ldr   x3, [arg_30h]                         ; [0x30:4]=0
0x06d92c34  stp   x2, x3, [x25]
0x06d92c38  str   x1, [x25, 0x10]
0x06d92c40  add   x1, x1, 0x6e0
0x06d92c44  str   x1, [x0]
0x06d92c48  adrp  x27, 0x957c000
0x06d92c4c  str   x3, [x27, 0xf00]
0x06d92c50  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d92c54  ldr   lr, [sp], 0x40                        ; segment.PHDR

```
## google3_third_party_jetski_gateway_gateway.ParseEnvOverrides

```asm
0x06d92a88  bl    0x4d18e50
0x06d92a8c  adrp  x27, 0xb48f000
0x06d92a90  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92a94  cbz   w4, 0x6d92aa4
0x06d92a98  ldr   x4, [x0]
0x06d92a9c  bl    0x4da34f0
0x06d92aa0  str   x4, [x25]
0x06d92aa4  adrp  x4, 0x8f93000
0x06d92aa8  add   x4, x4, 0x6b0
0x06d92aac  str   x4, [x0]
0x06d92ab0  adrp  x0, 0x8332000
0x06d92ab4  add   x0, x0, 0x3c8
0x06d92ab8  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92abc  adrp  x2, 0x4367000
0x06d92ac0  add   x2, x2, 0xcd3                      ; "knowledge_generationa must come before bCodeArtifact800LinesReviewDiff120Modules"
0x06d92ac4  mov   x3, 0x14
0x06d92ac8  bl    0x4d18e50
0x06d92acc  adrp  x27, 0xb48f000
0x06d92ad0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92ad4  cbz   w4, 0x6d92ae4
0x06d92ad8  ldr   x4, [x0]
0x06d92adc  bl    0x4da34f0
0x06d92ae0  str   x4, [x25]
0x06d92ae4  adrp  x4, 0x8f93000
0x06d92ae8  add   x4, x4, 0x6b0
0x06d92aec  str   x4, [x0]
0x06d92af0  adrp  x0, 0x8332000
0x06d92af4  add   x0, x0, 0x3c8
0x06d92af8  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92afc  adrp  x2, 0x4358000
0x06d92b00  add   x2, x2, 0xcfd                      ; "knowledge_timestampunknown flagKind %dworkspace not foundAGY_LLM_GATEWAY_URLPyth"
0x06d92b04  mov   x3, 0x13
0x06d92b08  bl    0x4d18e50
0x06d92b0c  adrp  x27, 0xb48f000
0x06d92b10  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92b14  cbz   w4, 0x6d92b24
0x06d92b18  ldr   x4, [x0]
0x06d92b1c  bl    0x4da34f0
0x06d92b20  str   x4, [x25]
0x06d92b24  adrp  x4, 0x8f93000
0x06d92b28  add   x4, x4, 0x6b0
0x06d92b2c  str   x4, [x0]
0x06d92b30  adrp  x0, 0x8332000
0x06d92b34  add   x0, x0, 0x3c8
0x06d92b38  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92b3c  adrp  x2, 0x4388000
0x06d92b40  add   x2, x2, 0xfa3
0x06d92b44  mov   x3, 0x16
0x06d92b48  bl    0x4d18e50
0x06d92b4c  adrp  x27, 0xb48f000
0x06d92b50  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92b54  cbz   w4, 0x6d92b64
0x06d92b58  ldr   x4, [x0]
0x06d92b5c  bl    0x4da34f0
0x06d92b60  str   x4, [x25]
0x06d92b64  adrp  x4, 0x8f93000
0x06d92b68  add   x4, x4, 0x6d0
0x06d92b6c  str   x4, [x0]
0x06d92b70  adrp  x0, 0x8332000
0x06d92b74  add   x0, x0, 0x3c8
0x06d92b78  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92b7c  adrp  x2, 0x4348000
0x06d92b80  add   x2, x2, 0x682                      ; "no_tool_call_checkMigrating %s to %splugin-kill-switchgoogle_credentialsAGY_BROW"
0x06d92b84  mov   x3, 0x12
0x06d92b88  bl    0x4d18e50
0x06d92b8c  adrp  x27, 0xb48f000
0x06d92b90  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92b94  cbz   w4, 0x6d92ba4
0x06d92b98  ldr   x4, [x0]
0x06d92b9c  bl    0x4da34f0
0x06d92ba0  str   x4, [x25]
0x06d92ba4  adrp  x4, 0x8f93000
0x06d92ba8  add   x4, x4, 0x6d8
0x06d92bac  str   x4, [x0]
0x06d92bb0  adrp  x0, 0x8332000
0x06d92bb4  add   x0, x0, 0x3c8
0x06d92bb8  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92bbc  adrp  x2, 0x43ee000
0x06d92bc0  add   x2, x2, 0x9c0
0x06d92bc4  orr   x3, xzr, 0x1f
0x06d92bc8  bl    0x4d18e50
0x06d92bcc  adrp  x27, 0xb48f000
0x06d92bd0  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92bd4  cbz   w4, 0x6d92be4
0x06d92bd8  ldr   x4, [x0]
0x06d92bdc  bl    0x4da34f0
0x06d92be0  str   x4, [x25]
0x06d92be4  adrp  x4, 0x8f93000
0x06d92be8  add   x4, x4, 0x6c8
0x06d92bec  str   x4, [x0]
0x06d92bf0  adrp  x0, 0x8332000
0x06d92bf4  add   x0, x0, 0x3c8
0x06d92bf8  ldr   x1, [arg_30h]                         ; [0x30:4]=0
0x06d92bfc  adrp  x2, 0x4358000
0x06d92c00  add   x2, x2, 0x487
0x06d92c04  mov   x3, 0x13
0x06d92c08  bl    0x4d18e50
0x06d92c0c  adrp  x27, 0xb48f000
0x06d92c10  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d92c14  cbnz  w4, 0x6d92c20
0x06d92c18  ldr   x3, [arg_70h]                         ; [0x30:4]=0
0x06d92c20  adrp  x27, 0x957c000
0x06d92c24  ldr   x1, [x27, 0xf00]                      ; [0xf00:4]=0x58585858
0x06d92c28  ldr   x2, [x0]
0x06d92c2c  bl    0x4da3510
0x06d92c30  ldr   x3, [arg_30h]                         ; [0x30:4]=0
0x06d92c34  stp   x2, x3, [x25]
0x06d92c38  str   x1, [x25, 0x10]
0x06d92c40  add   x1, x1, 0x6e0
0x06d92c44  str   x1, [x0]
0x06d92c48  adrp  x27, 0x957c000
0x06d92c4c  str   x3, [x27, 0xf00]
0x06d92c50  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d92c54  ldr   lr, [sp], 0x40                        ; segment.PHDR

```
## google3_third_party_jetski_gateway_gateway.MergeSettings

```asm
0x06d92f08  adrp  x27, 0x957c000
0x06d92f0c  ldr   x1, [x27, 0xf00]                      ; [0xf00:4]=0x58585858
0x06d92f10  add   x2, arg_28h
0x06d92f14  stp   xzr, xzr, [x2]
0x06d92f18  stp   xzr, xzr, [x2, 0x10]
0x06d92f1c  stp   xzr, xzr, [x2, 0x20]
0x06d92f20  stp   xzr, xzr, [x2, 0x30]
0x06d92f24  stp   xzr, xzr, [x2, 0x40]
0x06d92f28  stp   xzr, xzr, [x2, 0x50]
0x06d92f2c  adrp  x0, 0x8332000
0x06d92f30  add   x0, x0, 0x3c8
0x06d92f34  bl    fcn.04d35800
0x06d92f38  b     0x6d92f58
0x06d92f60  adrp  x27, 0x957c000
0x06d92f64  ldr   x1, [x27, 0xf08]                      ; [0xf08:4]=0x58585858
0x06d92f68  add   x2, arg_28h
0x06d92f6c  stp   xzr, xzr, [x2]
0x06d92f70  stp   xzr, xzr, [x2, 0x10]
0x06d92f74  stp   xzr, xzr, [x2, 0x20]
0x06d92f78  stp   xzr, xzr, [x2, 0x30]
0x06d92f7c  stp   xzr, xzr, [x2, 0x40]
0x06d92f80  stp   xzr, xzr, [x2, 0x50]
0x06d92f84  adrp  x0, 0x8332000
0x06d92f88  add   x0, x0, 0x5e8
0x06d92f8c  bl    fcn.04d35800
0x06d92f90  b     0x6d92fb0
0x06d92fb8  adrp  x27, 0x957c000
0x06d92fbc  ldr   x1, [x27, 0xf10]                      ; [0xf10:4]=0x58585858
0x06d92fc0  add   x2, arg_28h
0x06d92fc4  stp   xzr, xzr, [x2]
0x06d92fc8  stp   xzr, xzr, [x2, 0x10]
0x06d92fcc  stp   xzr, xzr, [x2, 0x20]
0x06d92fd0  stp   xzr, xzr, [x2, 0x30]
0x06d92fd4  stp   xzr, xzr, [x2, 0x40]
0x06d92fd8  stp   xzr, xzr, [x2, 0x50]
0x06d92fdc  adrp  x0, 0x8332000
0x06d92fe0  add   x0, x0, 0x560
0x06d92fe4  bl    fcn.04d35800
0x06d92fe8  b     0x6d93008
0x06d93010  adrp  x27, 0x957c000
0x06d93014  ldr   x1, [x27, 0xf18]                      ; [0xf18:4]=0x58585858
0x06d93018  add   x2, arg_28h
0x06d9301c  stp   xzr, xzr, [x2]
0x06d93020  stp   xzr, xzr, [x2, 0x10]
0x06d93024  stp   xzr, xzr, [x2, 0x20]
0x06d93028  stp   xzr, xzr, [x2, 0x30]
0x06d9302c  stp   xzr, xzr, [x2, 0x40]
0x06d93030  stp   xzr, xzr, [x2, 0x50]
0x06d93034  adrp  x0, 0x8332000
0x06d93038  add   x0, x0, 0x450
0x06d9303c  bl    fcn.04d35800
0x06d93040  b     0x6d93060
0x06d93068  mov   x0, xzr
0x06d9306c  mov   x1, xzr
0x06d93070  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d93074  ldr   lr, [sp], 0x90                        ; [0x90:4]=0
0x06d93078  ret

```
## google3_third_party_jetski_gateway_gateway.mergeInto

```asm
0x06d92f08  adrp  x27, 0x957c000
0x06d92f0c  ldr   x1, [x27, 0xf00]                      ; [0xf00:4]=0x58585858
0x06d92f10  add   x2, arg_28h
0x06d92f14  stp   xzr, xzr, [x2]
0x06d92f18  stp   xzr, xzr, [x2, 0x10]
0x06d92f1c  stp   xzr, xzr, [x2, 0x20]
0x06d92f20  stp   xzr, xzr, [x2, 0x30]
0x06d92f24  stp   xzr, xzr, [x2, 0x40]
0x06d92f28  stp   xzr, xzr, [x2, 0x50]
0x06d92f2c  adrp  x0, 0x8332000
0x06d92f30  add   x0, x0, 0x3c8
0x06d92f34  bl    fcn.04d35800
0x06d92f38  b     0x6d92f58
0x06d92f60  adrp  x27, 0x957c000
0x06d92f64  ldr   x1, [x27, 0xf08]                      ; [0xf08:4]=0x58585858
0x06d92f68  add   x2, arg_28h
0x06d92f6c  stp   xzr, xzr, [x2]
0x06d92f70  stp   xzr, xzr, [x2, 0x10]
0x06d92f74  stp   xzr, xzr, [x2, 0x20]
0x06d92f78  stp   xzr, xzr, [x2, 0x30]
0x06d92f7c  stp   xzr, xzr, [x2, 0x40]
0x06d92f80  stp   xzr, xzr, [x2, 0x50]
0x06d92f84  adrp  x0, 0x8332000
0x06d92f88  add   x0, x0, 0x5e8
0x06d92f8c  bl    fcn.04d35800
0x06d92f90  b     0x6d92fb0
0x06d92fb8  adrp  x27, 0x957c000
0x06d92fbc  ldr   x1, [x27, 0xf10]                      ; [0xf10:4]=0x58585858
0x06d92fc0  add   x2, arg_28h
0x06d92fc4  stp   xzr, xzr, [x2]
0x06d92fc8  stp   xzr, xzr, [x2, 0x10]
0x06d92fcc  stp   xzr, xzr, [x2, 0x20]
0x06d92fd0  stp   xzr, xzr, [x2, 0x30]
0x06d92fd4  stp   xzr, xzr, [x2, 0x40]
0x06d92fd8  stp   xzr, xzr, [x2, 0x50]
0x06d92fdc  adrp  x0, 0x8332000
0x06d92fe0  add   x0, x0, 0x560
0x06d92fe4  bl    fcn.04d35800
0x06d92fe8  b     0x6d93008
0x06d93010  adrp  x27, 0x957c000
0x06d93014  ldr   x1, [x27, 0xf18]                      ; [0xf18:4]=0x58585858
0x06d93018  add   x2, arg_28h
0x06d9301c  stp   xzr, xzr, [x2]
0x06d93020  stp   xzr, xzr, [x2, 0x10]
0x06d93024  stp   xzr, xzr, [x2, 0x20]
0x06d93028  stp   xzr, xzr, [x2, 0x30]
0x06d9302c  stp   xzr, xzr, [x2, 0x40]
0x06d93030  stp   xzr, xzr, [x2, 0x50]
0x06d93034  adrp  x0, 0x8332000
0x06d93038  add   x0, x0, 0x450
0x06d9303c  bl    fcn.04d35800
0x06d93040  b     0x6d93060
0x06d93068  mov   x0, xzr
0x06d9306c  mov   x1, xzr
0x06d93070  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d93074  ldr   lr, [sp], 0x90                        ; [0x90:4]=0
0x06d93078  ret

```
## google3_third_party_jetski_gateway_gateway.mergeGateway

```asm
0x06d93178  stp   xzr, xzr, [x2, 0x50]                  ; arg3
0x06d9317c  adrp  x0, 0x8312000
0x06d93180  add   x0, x0, 0xe48
0x06d93184  bl    fcn.04d35800
0x06d93188  b     0x6d931a8
0x06d931b0  ldr   x0, [arg_b0h]                         ; [0xb0:4]=0x6474e551
0x06d931b4  bl    fcn.06d92e90
0x06d931b8  mov   x0, xzr
0x06d931bc  mov   x1, xzr
0x06d931c0  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d931c4  ldr   lr, [sp], 0x90                        ; [0x90:4]=0

```
## google3_third_party_jetski_gateway_gateway.mergeModels

```asm
0x06d934f8  ldrsw x9, [x10]
0x06d934fc  b     0x6d9350c
0x06d9350c  str   x5, [arg_130h]                        ; arg6
0x06d93510  str   x7, [arg_2d8h]                        ; arg8
0x06d93514  str   x6, [arg_138h]                        ; arg7
0x06d93518  str   x8, [arg_2e0h]
0x06d9351c  ldr   x10, [arg_4e8h]                       ; [0x4e8:4]=0x58585858
0x06d93520  ldr   x11, [x10]
0x06d93524  ldp   x11, x12, [x11, 0x108]
0x06d93528  cbz   x11, 0x6d93564
0x06d9352c  ldr   x4, [x11, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
0x06d93530  mov   x0, x12
0x06d93534  ldr   x1, [arg_4f0h]                        ; [0x4f0:4]=0x58585858
0x06d93538  ldr   x2, [arg_4f8h]                        ; [0x4f8:4]=0x58585858
0x06d9353c  mov   x3, x9
0x06d93540  blr   x4
0x06d93548  ldr   x5, [x0, 8]                           ; [0x8:4]=0
0x06d9354c  adrp  x6, 0x94a8000
0x06d93550  add   x6, x6, 0x480
0x06d93554  ldar  x7, [x6]
0x06d93558  ldr   x8, [x7]                              ; arg8
0x06d9355c  ldr   w9, [x0, 0x10]                        ; [0x10:4]=0xb70003
0x06d93564  mov   x9, xzr
0x06d93568  mov   x11, xzr
0x06d9356c  b     0x6d93598

```
## google3_third_party_jetski_gateway_gateway.LoadDefaultEffectiveSettings

```asm
0x06d934f8  ldrsw x9, [x10]
0x06d934fc  b     0x6d9350c
0x06d9350c  str   x5, [arg_130h]                        ; arg6
0x06d93510  str   x7, [arg_2d8h]                        ; arg8
0x06d93514  str   x6, [arg_138h]                        ; arg7
0x06d93518  str   x8, [arg_2e0h]
0x06d9351c  ldr   x10, [arg_4e8h]                       ; [0x4e8:4]=0x58585858
0x06d93520  ldr   x11, [x10]
0x06d93524  ldp   x11, x12, [x11, 0x108]
0x06d93528  cbz   x11, 0x6d93564
0x06d9352c  ldr   x4, [x11, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
0x06d93530  mov   x0, x12
0x06d93534  ldr   x1, [arg_4f0h]                        ; [0x4f0:4]=0x58585858
0x06d93538  ldr   x2, [arg_4f8h]                        ; [0x4f8:4]=0x58585858
0x06d9353c  mov   x3, x9
0x06d93540  blr   x4
0x06d93548  ldr   x5, [x0, 8]                           ; [0x8:4]=0
0x06d9354c  adrp  x6, 0x94a8000
0x06d93550  add   x6, x6, 0x480
0x06d93554  ldar  x7, [x6]
0x06d93558  ldr   x8, [x7]                              ; arg8
0x06d9355c  ldr   w9, [x0, 0x10]                        ; [0x10:4]=0xb70003
0x06d93564  mov   x9, xzr
0x06d93568  mov   x11, xzr
0x06d9356c  b     0x6d93598

```
## google3_third_party_jetski_gateway_gateway.LoadEffectiveSettings

```asm
0x06d934f8  ldrsw x9, [x10]
0x06d934fc  b     0x6d9350c
0x06d9350c  str   x5, [arg_130h]                        ; arg6
0x06d93510  str   x7, [arg_2d8h]                        ; arg8
0x06d93514  str   x6, [arg_138h]                        ; arg7
0x06d93518  str   x8, [arg_2e0h]
0x06d9351c  ldr   x10, [arg_4e8h]                       ; [0x4e8:4]=0x58585858
0x06d93520  ldr   x11, [x10]
0x06d93524  ldp   x11, x12, [x11, 0x108]
0x06d93528  cbz   x11, 0x6d93564
0x06d9352c  ldr   x4, [x11, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
0x06d93530  mov   x0, x12
0x06d93534  ldr   x1, [arg_4f0h]                        ; [0x4f0:4]=0x58585858
0x06d93538  ldr   x2, [arg_4f8h]                        ; [0x4f8:4]=0x58585858
0x06d9353c  mov   x3, x9
0x06d93540  blr   x4
0x06d93548  ldr   x5, [x0, 8]                           ; [0x8:4]=0
0x06d9354c  adrp  x6, 0x94a8000
0x06d93550  add   x6, x6, 0x480
0x06d93554  ldar  x7, [x6]
0x06d93558  ldr   x8, [x7]                              ; arg8
0x06d9355c  ldr   w9, [x0, 0x10]                        ; [0x10:4]=0xb70003
0x06d93564  mov   x9, xzr
0x06d93568  mov   x11, xzr
0x06d9356c  b     0x6d93598

```
## google3_third_party_jetski_gateway_gateway.loadSettingsFile

```asm
0x06d934f8  ldrsw x9, [x10]
0x06d934fc  b     0x6d9350c
0x06d9350c  str   x5, [arg_130h]                        ; arg6
0x06d93510  str   x7, [arg_2d8h]                        ; arg8
0x06d93514  str   x6, [arg_138h]                        ; arg7
0x06d93518  str   x8, [arg_2e0h]
0x06d9351c  ldr   x10, [arg_4e8h]                       ; [0x4e8:4]=0x58585858
0x06d93520  ldr   x11, [x10]
0x06d93524  ldp   x11, x12, [x11, 0x108]
0x06d93528  cbz   x11, 0x6d93564
0x06d9352c  ldr   x4, [x11, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
0x06d93530  mov   x0, x12
0x06d93534  ldr   x1, [arg_4f0h]                        ; [0x4f0:4]=0x58585858
0x06d93538  ldr   x2, [arg_4f8h]                        ; [0x4f8:4]=0x58585858
0x06d9353c  mov   x3, x9
0x06d93540  blr   x4
0x06d93548  ldr   x5, [x0, 8]                           ; [0x8:4]=0
0x06d9354c  adrp  x6, 0x94a8000
0x06d93550  add   x6, x6, 0x480
0x06d93554  ldar  x7, [x6]
0x06d93558  ldr   x8, [x7]                              ; arg8
0x06d9355c  ldr   w9, [x0, 0x10]                        ; [0x10:4]=0xb70003
0x06d93564  mov   x9, xzr
0x06d93568  mov   x11, xzr
0x06d9356c  b     0x6d93598

```
## google3_third_party_jetski_gateway_gateway.NewAuthProviderWithOptions

```asm

```
## google3_third_party_jetski_gateway_gateway.NewAuthProviderFromSettings

```asm
0x06d94188  b     0x6d94190
0x06d94194  mov   x8, x2
0x06d94198  mov   x2, x4                                ; arg5
0x06d9419c  mov   x9, x3
0x06d941a0  mov   x3, x5                                ; arg6
0x06d941a4  mov   x4, x6                                ; arg7
0x06d941a8  mov   x5, x8
0x06d941ac  mov   x6, x9
0x06d941b0  bl    fcn.06a092a0
0x06d941b4  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d941b8  ldr   lr, [sp], 0x50                        ; [0x50:4]=64
0x06d941bc  ret
0x06d941c4  mov   x1, x3                                ; arg4
0x06d941c8  mov   x2, xzr
0x06d941cc  mov   x3, xzr
0x06d941d0  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d941d4  ldr   lr, [sp], 0x50                        ; [0x50:4]=64

```
## AuthProvider_.AdminSettings

```asm
0x06d94288  mov   x11, xzr
0x06d9428c  bl    fcn.06704710
0x06d94290  mov   x2, xzr
0x06d94294  mov   x3, xzr
0x06d94298  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d9429c  ldr   lr, [sp], 0x70                        ; [0x70:4]=8

```
## AuthProvider_.setLastDiscoveredCatalog

```asm
0x06d94388  str   x8, [x25]
0x06d9438c  adrp  x8, 0x8f93000
0x06d94390  add   x8, x8, 0x850
0x06d94394  str   x8, [x0]                              ; arg1
0x06d94398  ldp   x0, x1, [arg_f8h]
0x06d9439c  ldp   x2, x3, [arg_108h]
0x06d943a0  ldp   x4, x5, [arg_118h]
0x06d943a4  ldr   x6, [arg_128h]                        ; [0x128:4]=0x779b960
0x06d943a8  add   x7, arg_b8h
0x06d943ac  bl    fcn.0697cd40
0x06d943b0  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d943b4  ldr   lr, [sp], 0xf0                        ; [0xf0:4]=0x779b960

```
## AuthProvider_.getLastDiscoveredCatalog

```asm

```
## google3_third_party_jetski_gateway_gateway.splitHeaderEntries

```asm
0x06d94548  stp   x0, x2, [x25]                         ; arg3
0x06d9454c  str   x0, [x1]                              ; arg2
0x06d94550  bl    fcn.06297e40
0x06d94554  ldr   x3, [arg_58h]                         ; [0x58:4]=64
0x06d94558  stp   x1, x2, [x3, 8]
0x06d9455c  adrp  x27, 0xb48f000
0x06d94560  ldr   w4, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d94564  cbz   w4, 0x6d94574
0x06d94568  ldr   x4, [x3]
0x06d9456c  bl    fcn.04da3500
0x06d94570  stp   x0, x4, [x25]
0x06d94574  str   x0, [x3]
0x06d94578  bl    fcn.0629d230
0x06d9457c  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d94580  ldr   lr, [sp], 0x70                        ; [0x70:4]=8

```
## google3_third_party_jetski_gateway_gateway.ParseCustomHeaders

```asm

```
## AuthProvider_.GetTokenSource

```asm

```
## AuthProvider_.GetCAICProject

```asm

```
## AuthProvider_.GetGCPLocation

```asm

```
## AuthProvider_.IsBusinessPayGo

```asm

```
## AuthProvider_.IsBusinessEnterprise

```asm

```
## AuthProvider_.ResolveAuthMode

```asm

```
## AuthProvider_.GetHTTPClient

```asm

```
## AuthProvider_.GetCACertPool

```asm
0x06d94d58  mov   sp, x20
0x06d94d5c  sub   fp, sp, 8
0x06d94d60  stp   x0, x1, [arg_138h]                    ; arg2
0x06d94d64  stp   x2, x3, [arg_148h]                    ; arg4
0x06d94d68  ldr   x4, [x26, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
0x06d94d6c  str   x4, [arg_b0h]
0x06d94d70  ldp   x5, x6, [x26, 8]
0x06d94d74  str   x6, [arg_78h + 0x8]                   ; arg7
0x06d94d78  ldr   x0, [x4]
0x06d94d7c  ldr   x4, [x5, 0x60]                        ; [0x60:4]=0x380
0x06d94d80  mov   x1, x2                                ; arg3
0x06d94d84  mov   x2, x3                                ; arg4
0x06d94d88  mov   x3, x4
0x06d94d8c  bl    0x6d954f0
0x06d94d90  cbnz  x1, 0x6d94e28
0x06d94d94  ldr   x26, [arg_1b0h]                       ; [0x80:4]=0
0x06d94d98  ldr   x4, [x26]
0x06d94d9c  ldr   x5, [arg_b0h]                         ; [0xb0:4]=0x6474e551
0x06d94da0  ldr   x2, [x5, 0x38]                        ; [0x38:4]=0x400010
0x06d94da4  ldr   x1, [arg_140h]                        ; [0x140:4]=144
0x06d94da8  mov   x3, x0
0x06d94dac  ldr   x0, [arg_138h]                        ; [0x138:4]=0x799b960 segment.GNU_RELRO
0x06d94db0  blr   x4
0x06d94db8  str   x1, [arg_88h]
0x06d94dbc  str   x0, [arg_48h]
0x06d94dc0  ldr   x1, [arg_148h]                        ; [0x148:4]=0x458
0x06d94dc4  ldr   x1, [x1, 0x30]                        ; [0x30:4]=0
0x06d94dc8  ldr   x0, [arg_150h]                        ; [0x150:4]=32
0x06d94dcc  blr   x1
0x06d94dd0  str   x0, [arg_a8h]
0x06d94dd4  ldr   x1, [arg_48h]                         ; [0x48:4]=64
0x06d94dd8  ldr   x1, [x1, 0x20]                        ; [0x20:4]=64  ; "@"
0x06d94ddc  ldr   x0, [arg_88h]                         ; [0x88:4]=0
0x06d94de0  blr   x1
0x06d94de4  add   x2, arg_c8h
0x06d94de8  stp   xzr, xzr, [x2]
0x06d94dec  stp   xzr, xzr, [x2, 0x10]
0x06d94df0  stp   xzr, xzr, [x2, 0x20]
0x06d94df4  stp   xzr, xzr, [x2, 0x30]
0x06d94df8  stp   xzr, xzr, [x2, 0x40]
0x06d94dfc  stp   xzr, xzr, [x2, 0x50]
0x06d94e00  mov   x1, x0
0x06d94e04  adrp  x0, 0x8dbe000
0x06d94e08  add   x0, x0, 0xfe0
0x06d94e0c  bl    fcn.04d35800
0x06d94e18  mov   x1, x3
0x06d94e1c  ldp   fp, lr, [sp, -8]
0x06d94e20  add   sp, sp, 0x130
0x06d94e24  ret
0x06d94e28  mov   x0, x1
0x06d94e2c  mov   x1, x2
0x06d94e30  ldp   fp, lr, [sp, -8]
0x06d94e34  add   sp, sp, 0x130
0x06d94e38  ret
0x06d94f3c  ldr   x1, [x1, 0x38]                        ; [0x38:4]=0x400010
0x06d94f40  ldr   x0, [arg_150h]                        ; [0x150:4]=32
0x06d94f44  blr   x1
0x06d94f48  str   x0, [arg_a0h]
0x06d94f4c  ldr   x1, [arg_48h]                         ; [0x48:4]=64
0x06d94f50  ldr   x1, [x1, 0x28]                        ; [0x28:4]=0xace39d8
0x06d94f54  ldr   x0, [arg_88h]                         ; [0x88:4]=0
0x06d94f58  blr   x1
0x06d94f5c  add   x2, arg_c8h
0x06d94f60  stp   xzr, xzr, [x2]
0x06d94f64  stp   xzr, xzr, [x2, 0x10]
0x06d94f68  stp   xzr, xzr, [x2, 0x20]
0x06d94f6c  stp   xzr, xzr, [x2, 0x30]
0x06d94f70  stp   xzr, xzr, [x2, 0x40]
0x06d94f74  stp   xzr, xzr, [x2, 0x50]
0x06d94f78  mov   x1, x0
0x06d94f7c  adrp  x0, 0x8dbe000
0x06d94f80  add   x0, x0, 0xfe0
0x06d94f84  bl    fcn.04d35800
0x06d94f88  b     0x6d94f94
0x06d9508c  ldr   x1, [arg_48h]                         ; [0x48:4]=64
0x06d95090  ldr   x1, [x1, 0x18]                        ; [0x18:4]=0x4d0f000 section..text
0x06d95094  ldr   x0, [arg_88h]                         ; [0x88:4]=0
0x06d95098  blr   x1
0x06d9509c  ldr   x2, [arg_148h]                        ; [0x148:4]=0x458
0x06d950a0  ldr   x2, [x2, 0x40]                        ; segment.PHDR
0x06d950a4  mov   x3, x0
0x06d950a8  ldr   x0, [arg_150h]                        ; [0x150:4]=32
0x06d950ac  mov   x4, x1
0x06d950b0  mov   x1, x3
0x06d950b4  mov   x5, x2
0x06d950b8  mov   x2, x4
0x06d950bc  blr   x5
0x06d950c0  ldp   fp, lr, [sp, -8]
0x06d950c4  add   sp, sp, 0x130

```
## AuthProvider_.GetCACertPath

```asm
0x06d94d58  mov   sp, x20
0x06d94d5c  sub   fp, sp, 8
0x06d94d60  stp   x0, x1, [arg_138h]                    ; arg2
0x06d94d64  stp   x2, x3, [arg_148h]                    ; arg4
0x06d94d68  ldr   x4, [x26, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
0x06d94d6c  str   x4, [arg_b0h]
0x06d94d70  ldp   x5, x6, [x26, 8]
0x06d94d74  str   x6, [arg_78h + 0x8]                   ; arg7
0x06d94d78  ldr   x0, [x4]
0x06d94d7c  ldr   x4, [x5, 0x60]                        ; [0x60:4]=0x380
0x06d94d80  mov   x1, x2                                ; arg3
0x06d94d84  mov   x2, x3                                ; arg4
0x06d94d88  mov   x3, x4
0x06d94d8c  bl    0x6d954f0
0x06d94d90  cbnz  x1, 0x6d94e28
0x06d94d94  ldr   x26, [arg_1b0h]                       ; [0x80:4]=0
0x06d94d98  ldr   x4, [x26]
0x06d94d9c  ldr   x5, [arg_b0h]                         ; [0xb0:4]=0x6474e551
0x06d94da0  ldr   x2, [x5, 0x38]                        ; [0x38:4]=0x400010
0x06d94da4  ldr   x1, [arg_140h]                        ; [0x140:4]=144
0x06d94da8  mov   x3, x0
0x06d94dac  ldr   x0, [arg_138h]                        ; [0x138:4]=0x799b960 segment.GNU_RELRO
0x06d94db0  blr   x4
0x06d94db8  str   x1, [arg_88h]
0x06d94dbc  str   x0, [arg_48h]
0x06d94dc0  ldr   x1, [arg_148h]                        ; [0x148:4]=0x458
0x06d94dc4  ldr   x1, [x1, 0x30]                        ; [0x30:4]=0
0x06d94dc8  ldr   x0, [arg_150h]                        ; [0x150:4]=32
0x06d94dcc  blr   x1
0x06d94dd0  str   x0, [arg_a8h]
0x06d94dd4  ldr   x1, [arg_48h]                         ; [0x48:4]=64
0x06d94dd8  ldr   x1, [x1, 0x20]                        ; [0x20:4]=64  ; "@"
0x06d94ddc  ldr   x0, [arg_88h]                         ; [0x88:4]=0
0x06d94de0  blr   x1
0x06d94de4  add   x2, arg_c8h
0x06d94de8  stp   xzr, xzr, [x2]
0x06d94dec  stp   xzr, xzr, [x2, 0x10]
0x06d94df0  stp   xzr, xzr, [x2, 0x20]
0x06d94df4  stp   xzr, xzr, [x2, 0x30]
0x06d94df8  stp   xzr, xzr, [x2, 0x40]
0x06d94dfc  stp   xzr, xzr, [x2, 0x50]
0x06d94e00  mov   x1, x0
0x06d94e04  adrp  x0, 0x8dbe000
0x06d94e08  add   x0, x0, 0xfe0
0x06d94e0c  bl    fcn.04d35800
0x06d94e18  mov   x1, x3
0x06d94e1c  ldp   fp, lr, [sp, -8]
0x06d94e20  add   sp, sp, 0x130
0x06d94e24  ret
0x06d94e28  mov   x0, x1
0x06d94e2c  mov   x1, x2
0x06d94e30  ldp   fp, lr, [sp, -8]
0x06d94e34  add   sp, sp, 0x130
0x06d94e38  ret
0x06d94f3c  ldr   x1, [x1, 0x38]                        ; [0x38:4]=0x400010
0x06d94f40  ldr   x0, [arg_150h]                        ; [0x150:4]=32
0x06d94f44  blr   x1
0x06d94f48  str   x0, [arg_a0h]
0x06d94f4c  ldr   x1, [arg_48h]                         ; [0x48:4]=64
0x06d94f50  ldr   x1, [x1, 0x28]                        ; [0x28:4]=0xace39d8
0x06d94f54  ldr   x0, [arg_88h]                         ; [0x88:4]=0
0x06d94f58  blr   x1
0x06d94f5c  add   x2, arg_c8h
0x06d94f60  stp   xzr, xzr, [x2]
0x06d94f64  stp   xzr, xzr, [x2, 0x10]
0x06d94f68  stp   xzr, xzr, [x2, 0x20]
0x06d94f6c  stp   xzr, xzr, [x2, 0x30]
0x06d94f70  stp   xzr, xzr, [x2, 0x40]
0x06d94f74  stp   xzr, xzr, [x2, 0x50]
0x06d94f78  mov   x1, x0
0x06d94f7c  adrp  x0, 0x8dbe000
0x06d94f80  add   x0, x0, 0xfe0
0x06d94f84  bl    fcn.04d35800
0x06d94f88  b     0x6d94f94
0x06d9508c  ldr   x1, [arg_48h]                         ; [0x48:4]=64
0x06d95090  ldr   x1, [x1, 0x18]                        ; [0x18:4]=0x4d0f000 section..text
0x06d95094  ldr   x0, [arg_88h]                         ; [0x88:4]=0
0x06d95098  blr   x1
0x06d9509c  ldr   x2, [arg_148h]                        ; [0x148:4]=0x458
0x06d950a0  ldr   x2, [x2, 0x40]                        ; segment.PHDR
0x06d950a4  mov   x3, x0
0x06d950a8  ldr   x0, [arg_150h]                        ; [0x150:4]=32
0x06d950ac  mov   x4, x1
0x06d950b0  mov   x1, x3
0x06d950b4  mov   x5, x2
0x06d950b8  mov   x2, x4
0x06d950bc  blr   x5
0x06d950c0  ldp   fp, lr, [sp, -8]
0x06d950c4  add   sp, sp, 0x130

```
## AuthProvider_.GetProxyURL

```asm
0x06d94d58  mov   sp, x20
0x06d94d5c  sub   fp, sp, 8
0x06d94d60  stp   x0, x1, [arg_138h]                    ; arg2
0x06d94d64  stp   x2, x3, [arg_148h]                    ; arg4
0x06d94d68  ldr   x4, [x26, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
0x06d94d6c  str   x4, [arg_b0h]
0x06d94d70  ldp   x5, x6, [x26, 8]
0x06d94d74  str   x6, [arg_78h + 0x8]                   ; arg7
0x06d94d78  ldr   x0, [x4]
0x06d94d7c  ldr   x4, [x5, 0x60]                        ; [0x60:4]=0x380
0x06d94d80  mov   x1, x2                                ; arg3
0x06d94d84  mov   x2, x3                                ; arg4
0x06d94d88  mov   x3, x4
0x06d94d8c  bl    0x6d954f0
0x06d94d90  cbnz  x1, 0x6d94e28
0x06d94d94  ldr   x26, [arg_1b0h]                       ; [0x80:4]=0
0x06d94d98  ldr   x4, [x26]
0x06d94d9c  ldr   x5, [arg_b0h]                         ; [0xb0:4]=0x6474e551
0x06d94da0  ldr   x2, [x5, 0x38]                        ; [0x38:4]=0x400010
0x06d94da4  ldr   x1, [arg_140h]                        ; [0x140:4]=144
0x06d94da8  mov   x3, x0
0x06d94dac  ldr   x0, [arg_138h]                        ; [0x138:4]=0x799b960 segment.GNU_RELRO
0x06d94db0  blr   x4
0x06d94db8  str   x1, [arg_88h]
0x06d94dbc  str   x0, [arg_48h]
0x06d94dc0  ldr   x1, [arg_148h]                        ; [0x148:4]=0x458
0x06d94dc4  ldr   x1, [x1, 0x30]                        ; [0x30:4]=0
0x06d94dc8  ldr   x0, [arg_150h]                        ; [0x150:4]=32
0x06d94dcc  blr   x1
0x06d94dd0  str   x0, [arg_a8h]
0x06d94dd4  ldr   x1, [arg_48h]                         ; [0x48:4]=64
0x06d94dd8  ldr   x1, [x1, 0x20]                        ; [0x20:4]=64  ; "@"
0x06d94ddc  ldr   x0, [arg_88h]                         ; [0x88:4]=0
0x06d94de0  blr   x1
0x06d94de4  add   x2, arg_c8h
0x06d94de8  stp   xzr, xzr, [x2]
0x06d94dec  stp   xzr, xzr, [x2, 0x10]
0x06d94df0  stp   xzr, xzr, [x2, 0x20]
0x06d94df4  stp   xzr, xzr, [x2, 0x30]
0x06d94df8  stp   xzr, xzr, [x2, 0x40]
0x06d94dfc  stp   xzr, xzr, [x2, 0x50]
0x06d94e00  mov   x1, x0
0x06d94e04  adrp  x0, 0x8dbe000
0x06d94e08  add   x0, x0, 0xfe0
0x06d94e0c  bl    fcn.04d35800
0x06d94e18  mov   x1, x3
0x06d94e1c  ldp   fp, lr, [sp, -8]
0x06d94e20  add   sp, sp, 0x130
0x06d94e24  ret
0x06d94e28  mov   x0, x1
0x06d94e2c  mov   x1, x2
0x06d94e30  ldp   fp, lr, [sp, -8]
0x06d94e34  add   sp, sp, 0x130
0x06d94e38  ret
0x06d94f3c  ldr   x1, [x1, 0x38]                        ; [0x38:4]=0x400010
0x06d94f40  ldr   x0, [arg_150h]                        ; [0x150:4]=32
0x06d94f44  blr   x1
0x06d94f48  str   x0, [arg_a0h]
0x06d94f4c  ldr   x1, [arg_48h]                         ; [0x48:4]=64
0x06d94f50  ldr   x1, [x1, 0x28]                        ; [0x28:4]=0xace39d8
0x06d94f54  ldr   x0, [arg_88h]                         ; [0x88:4]=0
0x06d94f58  blr   x1
0x06d94f5c  add   x2, arg_c8h
0x06d94f60  stp   xzr, xzr, [x2]
0x06d94f64  stp   xzr, xzr, [x2, 0x10]
0x06d94f68  stp   xzr, xzr, [x2, 0x20]
0x06d94f6c  stp   xzr, xzr, [x2, 0x30]
0x06d94f70  stp   xzr, xzr, [x2, 0x40]
0x06d94f74  stp   xzr, xzr, [x2, 0x50]
0x06d94f78  mov   x1, x0
0x06d94f7c  adrp  x0, 0x8dbe000
0x06d94f80  add   x0, x0, 0xfe0
0x06d94f84  bl    fcn.04d35800
0x06d94f88  b     0x6d94f94
0x06d9508c  ldr   x1, [arg_48h]                         ; [0x48:4]=64
0x06d95090  ldr   x1, [x1, 0x18]                        ; [0x18:4]=0x4d0f000 section..text
0x06d95094  ldr   x0, [arg_88h]                         ; [0x88:4]=0
0x06d95098  blr   x1
0x06d9509c  ldr   x2, [arg_148h]                        ; [0x148:4]=0x458
0x06d950a0  ldr   x2, [x2, 0x40]                        ; segment.PHDR
0x06d950a4  mov   x3, x0
0x06d950a8  ldr   x0, [arg_150h]                        ; [0x150:4]=32
0x06d950ac  mov   x4, x1
0x06d950b0  mov   x1, x3
0x06d950b4  mov   x5, x2
0x06d950b8  mov   x2, x4
0x06d950bc  blr   x5
0x06d950c0  ldp   fp, lr, [sp, -8]
0x06d950c4  add   sp, sp, 0x130

```
## AuthProvider_.GetProxyURLString

```asm
0x06d94d58  mov   sp, x20
0x06d94d5c  sub   fp, sp, 8
0x06d94d60  stp   x0, x1, [arg_138h]                    ; arg2
0x06d94d64  stp   x2, x3, [arg_148h]                    ; arg4
0x06d94d68  ldr   x4, [x26, 0x18]                       ; [0x18:4]=0x4d0f000 section..text
0x06d94d6c  str   x4, [arg_b0h]
0x06d94d70  ldp   x5, x6, [x26, 8]
0x06d94d74  str   x6, [arg_78h + 0x8]                   ; arg7
0x06d94d78  ldr   x0, [x4]
0x06d94d7c  ldr   x4, [x5, 0x60]                        ; [0x60:4]=0x380
0x06d94d80  mov   x1, x2                                ; arg3
0x06d94d84  mov   x2, x3                                ; arg4
0x06d94d88  mov   x3, x4
0x06d94d8c  bl    0x6d954f0
0x06d94d90  cbnz  x1, 0x6d94e28
0x06d94d94  ldr   x26, [arg_1b0h]                       ; [0x80:4]=0
0x06d94d98  ldr   x4, [x26]
0x06d94d9c  ldr   x5, [arg_b0h]                         ; [0xb0:4]=0x6474e551
0x06d94da0  ldr   x2, [x5, 0x38]                        ; [0x38:4]=0x400010
0x06d94da4  ldr   x1, [arg_140h]                        ; [0x140:4]=144
0x06d94da8  mov   x3, x0
0x06d94dac  ldr   x0, [arg_138h]                        ; [0x138:4]=0x799b960 segment.GNU_RELRO
0x06d94db0  blr   x4
0x06d94db8  str   x1, [arg_88h]
0x06d94dbc  str   x0, [arg_48h]
0x06d94dc0  ldr   x1, [arg_148h]                        ; [0x148:4]=0x458
0x06d94dc4  ldr   x1, [x1, 0x30]                        ; [0x30:4]=0
0x06d94dc8  ldr   x0, [arg_150h]                        ; [0x150:4]=32
0x06d94dcc  blr   x1
0x06d94dd0  str   x0, [arg_a8h]
0x06d94dd4  ldr   x1, [arg_48h]                         ; [0x48:4]=64
0x06d94dd8  ldr   x1, [x1, 0x20]                        ; [0x20:4]=64  ; "@"
0x06d94ddc  ldr   x0, [arg_88h]                         ; [0x88:4]=0
0x06d94de0  blr   x1
0x06d94de4  add   x2, arg_c8h
0x06d94de8  stp   xzr, xzr, [x2]
0x06d94dec  stp   xzr, xzr, [x2, 0x10]
0x06d94df0  stp   xzr, xzr, [x2, 0x20]
0x06d94df4  stp   xzr, xzr, [x2, 0x30]
0x06d94df8  stp   xzr, xzr, [x2, 0x40]
0x06d94dfc  stp   xzr, xzr, [x2, 0x50]
0x06d94e00  mov   x1, x0
0x06d94e04  adrp  x0, 0x8dbe000
0x06d94e08  add   x0, x0, 0xfe0
0x06d94e0c  bl    fcn.04d35800
0x06d94e18  mov   x1, x3
0x06d94e1c  ldp   fp, lr, [sp, -8]
0x06d94e20  add   sp, sp, 0x130
0x06d94e24  ret
0x06d94e28  mov   x0, x1
0x06d94e2c  mov   x1, x2
0x06d94e30  ldp   fp, lr, [sp, -8]
0x06d94e34  add   sp, sp, 0x130
0x06d94e38  ret
0x06d94f3c  ldr   x1, [x1, 0x38]                        ; [0x38:4]=0x400010
0x06d94f40  ldr   x0, [arg_150h]                        ; [0x150:4]=32
0x06d94f44  blr   x1
0x06d94f48  str   x0, [arg_a0h]
0x06d94f4c  ldr   x1, [arg_48h]                         ; [0x48:4]=64
0x06d94f50  ldr   x1, [x1, 0x28]                        ; [0x28:4]=0xace39d8
0x06d94f54  ldr   x0, [arg_88h]                         ; [0x88:4]=0
0x06d94f58  blr   x1
0x06d94f5c  add   x2, arg_c8h
0x06d94f60  stp   xzr, xzr, [x2]
0x06d94f64  stp   xzr, xzr, [x2, 0x10]
0x06d94f68  stp   xzr, xzr, [x2, 0x20]
0x06d94f6c  stp   xzr, xzr, [x2, 0x30]
0x06d94f70  stp   xzr, xzr, [x2, 0x40]
0x06d94f74  stp   xzr, xzr, [x2, 0x50]
0x06d94f78  mov   x1, x0
0x06d94f7c  adrp  x0, 0x8dbe000
0x06d94f80  add   x0, x0, 0xfe0
0x06d94f84  bl    fcn.04d35800
0x06d94f88  b     0x6d94f94
0x06d9508c  ldr   x1, [arg_48h]                         ; [0x48:4]=64
0x06d95090  ldr   x1, [x1, 0x18]                        ; [0x18:4]=0x4d0f000 section..text
0x06d95094  ldr   x0, [arg_88h]                         ; [0x88:4]=0
0x06d95098  blr   x1
0x06d9509c  ldr   x2, [arg_148h]                        ; [0x148:4]=0x458
0x06d950a0  ldr   x2, [x2, 0x40]                        ; segment.PHDR
0x06d950a4  mov   x3, x0
0x06d950a8  ldr   x0, [arg_150h]                        ; [0x150:4]=32
0x06d950ac  mov   x4, x1
0x06d950b0  mov   x1, x3
0x06d950b4  mov   x5, x2
0x06d950b8  mov   x2, x4
0x06d950bc  blr   x5
0x06d950c0  ldp   fp, lr, [sp, -8]
0x06d950c4  add   sp, sp, 0x130

```
## AuthProvider_.SetHTTPClient

```asm

```
## AuthProvider_.GetUserAgentName

```asm

```
## AuthProvider_.GetAuthMethod

```asm

```
## AuthProvider_.GetWIFProvider

```asm

```
## AuthProvider_.GetUserTier

```asm

```
## AuthProvider_.SetUserTier

```asm

```
## AuthProvider_.GetEndpointURL

```asm

```
## AuthProvider_.SetEndpointURL

```asm
0x06d95368  cbnz  w3, 0x6d9537c
0x06d9536c  ldr   x1, [arg_180h]                        ; [0xa0:4]=0x779b960
0x06d95370  ldr   x2, [arg_90h]                         ; [0x90:4]=0
0x06d95374  ldr   x3, [arg_e8h]                         ; [0xe8:4]=1
0x06d9537c  bl    0x4da3510
0x06d95380  ldr   x1, [arg_a0h]                         ; [0xa0:4]=0x779b960
0x06d95384  ldr   x2, [arg_90h]                         ; [0x90:4]=0
0x06d95388  stp   x1, x2, [x25]
0x06d9538c  ldr   x3, [arg_e8h]                         ; [0xe8:4]=1
0x06d95390  str   x3, [x25, 0x10]
0x06d95398  stp   x1, x2, [x0, 8]
0x06d9539c  str   x3, [x0, 0x18]
0x06d953a0  mov   x0, x1
0x06d953a4  bl    0x52d6250
0x06d953a8  str   x0, [arg_98h]
0x06d953ac  stp   x1, x2, [arg_38h]
0x06d953b0  add   x3, arg_58h
0x06d953b4  stp   xzr, xzr, [x3]
0x06d953b8  stp   xzr, xzr, [x3, 0x10]
0x06d953bc  stp   xzr, xzr, [x3, 0x20]
0x06d953c0  str   xzr, [x3, 0x30]
0x06d953c4  ldr   x3, [arg_a0h]                         ; [0xa0:4]=0x779b960
0x06d953c8  ldp   x4, x5, [x3, 0x40]
0x06d953cc  stp   x4, x5, [arg_70h]
0x06d953d0  ldp   x4, x5, [x3, 0x50]
0x06d953d4  stp   x4, x5, [arg_58h + 0x8]
0x06d953d8  ldrb  w4, [x3, 0x90]                        ; [0x90:4]=0
0x06d953dc  strb  w4, [arg_58h]
0x06d953e0  ldr   x3, [x3, 0x70]                        ; [0x70:4]=8
0x06d953e4  str   x3, [arg_70h + 0x18]
0x06d953e8  bl    0x52d7d00
0x06d953ec  str   x0, [arg_c0h]
0x06d953f0  ldr   x0, [arg_98h]                         ; [0x98:4]=0x779b960
0x06d953f4  ldp   x1, x2, [arg_38h]
0x06d953f8  bl    0x52d8130
0x06d953fc  str   x0, [arg_b8h]
0x06d95400  str   x1, [arg_50h]
0x06d95404  ldr   x0, [arg_98h]                         ; [0x98:4]=0x779b960
0x06d95408  ldp   x1, x2, [arg_38h]
0x06d9540c  bl    0x52d7e80
0x06d95410  str   x0, [arg_b0h]
0x06d95414  str   x1, [arg_48h]
0x06d95418  adrp  x0, 0x8b68000
0x06d9541c  add   x0, x0, 0x228
0x06d95420  bl    0x4d31c10
0x06d95424  adrp  x27, 0xb48f000
0x06d95428  ldr   w1, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9542c  cbz   w1, 0x6d9547c
0x06d95430  str   x0, [arg_a8h]
0x06d95434  ldp   x3, x4, [x0, 0x38]
0x06d95438  ldr   x5, [x0, 0x48]                        ; [0x48:4]=64
0x06d9543c  ldr   x6, [x0, 0x58]                        ; [0x58:4]=64
0x06d95440  bl    0x4da3560
0x06d95444  ldr   x7, [arg_c8h]                         ; [0xc8:4]=0
0x06d95448  stp   x7, x3, [x25]
0x06d9544c  ldr   x3, [arg_c0h]                         ; [0xc0:4]=0
0x06d95450  stp   x3, x4, [x25, 0x10]
0x06d95454  ldr   x3, [arg_b8h]                         ; [0xb8:4]=0
0x06d95458  stp   x3, x5, [x25, 0x20]
0x06d9545c  ldr   x3, [arg_b0h]                         ; [0xb0:4]=0x6474e551
0x06d95460  stp   x3, x6, [x25, 0x30]
0x06d95464  mov   x1, x0
0x06d95468  add   x2, arg_58h
0x06d9546c  adrp  x0, 0x8b68000
0x06d95470  add   x0, x0, 0x150
0x06d95474  bl    0x4d35930
0x06d95478  ldr   x0, [arg_a8h]                         ; [0xa8:4]=0x200000
0x06d9547c  add   x27, arg_58h
0x06d95480  ldp   q0, q1, [x27]
0x06d95484  ldp   q2, q3, [arg_70h]
0x06d95488  stp   q0, q1, [x0]
0x06d9548c  add   x27, x0, 0x18
0x06d95490  stp   q2, q3, [x27]
0x06d95494  ldp   x2, x1, [arg_c0h]
0x06d95498  stp   x1, x2, [x0, 0x38]
0x06d9549c  ldr   x1, [arg_b8h]                         ; [0xb8:4]=0
0x06d954a0  ldr   x2, [arg_50h]                         ; [0x50:4]=64
0x06d954a4  stp   x1, x2, [x0, 0x48]
0x06d954a8  ldr   x1, [arg_b0h]                         ; [0xb0:4]=0x6474e551
0x06d954ac  ldr   x2, [arg_48h]                         ; [0x48:4]=64
0x06d954b0  stp   x1, x2, [x0, 0x58]
0x06d954b4  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d954b8  ldr   lr, [sp], 0xe0                        ; [0xe0:4]=0

```
## AuthProvider_.GetWireProtocol

```asm
0x06d95368  cbnz  w3, 0x6d9537c
0x06d9536c  ldr   x1, [arg_180h]                        ; [0xa0:4]=0x779b960
0x06d95370  ldr   x2, [arg_90h]                         ; [0x90:4]=0
0x06d95374  ldr   x3, [arg_e8h]                         ; [0xe8:4]=1
0x06d9537c  bl    0x4da3510
0x06d95380  ldr   x1, [arg_a0h]                         ; [0xa0:4]=0x779b960
0x06d95384  ldr   x2, [arg_90h]                         ; [0x90:4]=0
0x06d95388  stp   x1, x2, [x25]
0x06d9538c  ldr   x3, [arg_e8h]                         ; [0xe8:4]=1
0x06d95390  str   x3, [x25, 0x10]
0x06d95398  stp   x1, x2, [x0, 8]
0x06d9539c  str   x3, [x0, 0x18]
0x06d953a0  mov   x0, x1
0x06d953a4  bl    0x52d6250
0x06d953a8  str   x0, [arg_98h]
0x06d953ac  stp   x1, x2, [arg_38h]
0x06d953b0  add   x3, arg_58h
0x06d953b4  stp   xzr, xzr, [x3]
0x06d953b8  stp   xzr, xzr, [x3, 0x10]
0x06d953bc  stp   xzr, xzr, [x3, 0x20]
0x06d953c0  str   xzr, [x3, 0x30]
0x06d953c4  ldr   x3, [arg_a0h]                         ; [0xa0:4]=0x779b960
0x06d953c8  ldp   x4, x5, [x3, 0x40]
0x06d953cc  stp   x4, x5, [arg_70h]
0x06d953d0  ldp   x4, x5, [x3, 0x50]
0x06d953d4  stp   x4, x5, [arg_58h + 0x8]
0x06d953d8  ldrb  w4, [x3, 0x90]                        ; [0x90:4]=0
0x06d953dc  strb  w4, [arg_58h]
0x06d953e0  ldr   x3, [x3, 0x70]                        ; [0x70:4]=8
0x06d953e4  str   x3, [arg_70h + 0x18]
0x06d953e8  bl    0x52d7d00
0x06d953ec  str   x0, [arg_c0h]
0x06d953f0  ldr   x0, [arg_98h]                         ; [0x98:4]=0x779b960
0x06d953f4  ldp   x1, x2, [arg_38h]
0x06d953f8  bl    0x52d8130
0x06d953fc  str   x0, [arg_b8h]
0x06d95400  str   x1, [arg_50h]
0x06d95404  ldr   x0, [arg_98h]                         ; [0x98:4]=0x779b960
0x06d95408  ldp   x1, x2, [arg_38h]
0x06d9540c  bl    0x52d7e80
0x06d95410  str   x0, [arg_b0h]
0x06d95414  str   x1, [arg_48h]
0x06d95418  adrp  x0, 0x8b68000
0x06d9541c  add   x0, x0, 0x228
0x06d95420  bl    0x4d31c10
0x06d95424  adrp  x27, 0xb48f000
0x06d95428  ldr   w1, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d9542c  cbz   w1, 0x6d9547c
0x06d95430  str   x0, [arg_a8h]
0x06d95434  ldp   x3, x4, [x0, 0x38]
0x06d95438  ldr   x5, [x0, 0x48]                        ; [0x48:4]=64
0x06d9543c  ldr   x6, [x0, 0x58]                        ; [0x58:4]=64
0x06d95440  bl    0x4da3560
0x06d95444  ldr   x7, [arg_c8h]                         ; [0xc8:4]=0
0x06d95448  stp   x7, x3, [x25]
0x06d9544c  ldr   x3, [arg_c0h]                         ; [0xc0:4]=0
0x06d95450  stp   x3, x4, [x25, 0x10]
0x06d95454  ldr   x3, [arg_b8h]                         ; [0xb8:4]=0
0x06d95458  stp   x3, x5, [x25, 0x20]
0x06d9545c  ldr   x3, [arg_b0h]                         ; [0xb0:4]=0x6474e551
0x06d95460  stp   x3, x6, [x25, 0x30]
0x06d95464  mov   x1, x0
0x06d95468  add   x2, arg_58h
0x06d9546c  adrp  x0, 0x8b68000
0x06d95470  add   x0, x0, 0x150
0x06d95474  bl    0x4d35930
0x06d95478  ldr   x0, [arg_a8h]                         ; [0xa8:4]=0x200000
0x06d9547c  add   x27, arg_58h
0x06d95480  ldp   q0, q1, [x27]
0x06d95484  ldp   q2, q3, [arg_70h]
0x06d95488  stp   q0, q1, [x0]
0x06d9548c  add   x27, x0, 0x18
0x06d95490  stp   q2, q3, [x27]
0x06d95494  ldp   x2, x1, [arg_c0h]
0x06d95498  stp   x1, x2, [x0, 0x38]
0x06d9549c  ldr   x1, [arg_b8h]                         ; [0xb8:4]=0
0x06d954a0  ldr   x2, [arg_50h]                         ; [0x50:4]=64
0x06d954a4  stp   x1, x2, [x0, 0x48]
0x06d954a8  ldr   x1, [arg_b0h]                         ; [0xb0:4]=0x6474e551
0x06d954ac  ldr   x2, [arg_48h]                         ; [0x48:4]=64
0x06d954b0  stp   x1, x2, [x0, 0x58]
0x06d954b4  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d954b8  ldr   lr, [sp], 0xe0                        ; [0xe0:4]=0

```
## AuthProvider_.SetWireProtocol

```asm
0x06d95528  ldr   x9, [x8]
0x06d9552c  ldr   w10, [x1, 0x10]                       ; [0x10:4]=0xb70003  ; arg2
0x06d95530  b     0x6d95750

```
## AuthProvider_.IsEval

```asm
0x06d95528  ldr   x9, [x8]
0x06d9552c  ldr   w10, [x1, 0x10]                       ; [0x10:4]=0xb70003  ; arg2
0x06d95530  b     0x6d95750

```
## AuthProvider_.ShouldUseCAICProjectAsOverride

```asm
0x06d95528  ldr   x9, [x8]
0x06d9552c  ldr   w10, [x1, 0x10]                       ; [0x10:4]=0xb70003  ; arg2
0x06d95530  b     0x6d95750

```
## AuthProvider_.SetHTTPHeaders

```asm
0x06d95528  ldr   x9, [x8]
0x06d9552c  ldr   w10, [x1, 0x10]                       ; [0x10:4]=0xb70003  ; arg2
0x06d95530  b     0x6d95750

```
## AuthProvider_.GetEntitlement

```asm
0x06d959f8  mov   x0, xzr
0x06d959fc  mov   x1, xzr
0x06d95a00  b     0x6d95a2c
0x06d95a30  ldr   x3, [arg_230h]                        ; [0x120:4]=7
0x06d95a34  ldr   x3, [x3, 0x18]                        ; [0x18:4]=0x4d0f000 section..text
0x06d95a38  ldr   x4, [arg_118h]                        ; [0x118:4]=0x200000
0x06d95a3c  ldr   x1, [x4, 0x28]                        ; [0x28:4]=0xace39d8
0x06d95a40  ldr   x0, [arg_128h]                        ; [0x128:4]=0x779b960
0x06d95a44  ldr   x2, [arg_f8h]                         ; [0xf8:4]=0x799b960 segment.GNU_RELRO
0x06d95a48  blr   x3
0x06d95a4c  str   x0, [arg_58h + 0x8]
0x06d95a50  str   x1, [arg_80h]
0x06d95a54  adrp  x27, 0x948e000
0x06d95a58  add   x27, x27, 0x3c0
0x06d95a5c  ldp   x2, x3, [x27]
0x06d95a60  bl    0x4daa700
0x06d95a64  tbnz  w0, 0, 0x6d95b54
0x06d95a68  ldr   x0, [arg_170h]                        ; [0x60:4]=0x380
0x06d95a70  mov   x1, x0
0x06d95a74  ldr   x0, [arg_80h]                         ; [0x80:4]=0
0x06d95a80  ldp   x0, x1, [arg_138h]
0x06d95a84  bl    0x4d98880
0x06d95a88  adrp  x2, 0x856f000
0x06d95a8c  add   x2, x2, 0x550
0x06d95a90  stp   x2, x0, [arg_d0h]
0x06d95a94  adrp  x0, 0x43e6000
0x06d95a98  add   x0, x0, 0x1bf                      ; "unary %s has multiple messagesForceBackgroundResearchRefreshCopyBuiltinWorkflowT"
0x06d95a9c  orr   x1, xzr, 0x1e
0x06d95aa0  add   x2, arg_d0h
0x06d95aa4  orr   x3, xzr, 1
0x06d95aa8  mov   x4, x3
0x06d95aac  bl    0x4e321f0
0x06d95ab4  nop
0x06d95ab8  orr   x0, xzr, 0x10
0x06d95abc  adrp  x1, 0x8757000
0x06d95ac0  add   x1, x1, 0x890
0x06d95ac4  orr   x2, xzr, 1
0x06d95ac8  bl    0x4d32500
0x06d95acc  adrp  x3, 0x43e6000
0x06d95ad0  add   x3, x3, 0x1bf                      ; "unary %s has multiple messagesForceBackgroundResearchRefreshCopyBuiltinWorkflowT"
0x06d95ad4  orr   x4, xzr, 0x1e
0x06d95ad8  stp   x3, x4, [x0]
0x06d95adc  mov   x1, x0
0x06d95ae0  adrp  x0, 0x8f07000
0x06d95ae4  add   x0, x0, 0x508
0x06d95aec  str   x1, [arg_78h]
0x06d95af0  orr   x0, xzr, 0x40
0x06d95af4  adrp  x1, 0x8b68000
0x06d95af8  add   x1, x1, 0x78
0x06d95afc  orr   x2, xzr, 1
0x06d95b00  bl    0x4d33310
0x06d95b04  orr   x3, xzr, 0xc
0x06d95b08  str   w3, [x0]
0x06d95b0c  ldr   x3, [arg_58h]                         ; [0x58:4]=64
0x06d95b10  str   x3, [x0, 8]
0x06d95b14  adrp  x27, 0xb48f000
0x06d95b18  ldr   w3, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d95b20  ldr   x3, [arg_188h]                        ; [0x78:4]=1
0x06d95b2c  ldr   x3, [arg_78h]                         ; [0x78:4]=1
0x06d95b30  str   x3, [x25]
0x06d95b38  adrp  x1, 0x8f07000
0x06d95b3c  add   x1, x1, 0x588
0x06d95b44  mov   x0, xzr
0x06d95b48  ldp   fp, lr, [sp, -8]
0x06d95b4c  add   sp, sp, 0x110
0x06d95b50  ret
0x06d95b54  ldr   x0, [arg_100h]                        ; [0x100:4]=0x799b960 segment.GNU_RELRO
0x06d95b58  mov   x1, xzr
0x06d95b5c  mov   x2, xzr
0x06d95b60  ldp   fp, lr, [sp, -8]
0x06d95b64  add   sp, sp, 0x110
0x06d95b68  ret
0x06d95b70  mov   x1, x0
0x06d95b74  mov   x0, xzr
0x06d95b78  ldp   fp, lr, [sp, -8]
0x06d95b7c  add   sp, sp, 0x110

```
## google3_third_party_jetski_gateway_gateway.LoadCACertPool

```asm
0x06d95a08  ldr   x7, [x9, 0x20]                        ; [0x20:4]=64  ; "@"
0x06d95a0c  ldrb  w9, [arg_98h]                         ; [0x98:4]=0x779b960
0x06d95a10  ldr   x10, [x0]                             ; arg1
0x06d95a14  ldrb  w5, [arg_c0h]                         ; [0xc0:4]=0
0x06d95a18  ldr   x6, [arg_c8h]                         ; [0xc8:4]=0
0x06d95a1c  ldr   x8, [arg_f8h]                         ; [0xf8:4]=0x799b960 segment.GNU_RELRO
0x06d95a20  mov   x26, x0                               ; arg1
0x06d95a24  mov   x0, x9

```
## google3_third_party_jetski_gateway_gateway.LoadProxyURL

```asm
0x06d95c68  adrp  x27, 0xb48f000
0x06d95c6c  ldr   w1, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d95c70  cbnz  w1, 0x6d95c7c
0x06d95c74  ldr   x1, [arg_28h]                         ; [0x18:4]=0x4d0f000 section..text
0x06d95c7c  ldr   x1, [arg_18h]                         ; [0x18:4]=0x4d0f000 section..text
0x06d95c80  ldr   x2, [x1, 8]                           ; [0x8:4]=0
0x06d95c84  bl    fcn.04da3500
0x06d95c88  stp   x0, x2, [x25]
0x06d95c90  mov   x0, x1
0x06d95c94  ldr   x0, [x0, 8]                           ; [0x8:4]=0
0x06d95c98  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d95c9c  ldr   lr, [sp], 0x10                        ; [0x10:4]=0xb70003

```
## google3_third_party_jetski_gateway_gateway.NewHTTPTransportWithOptions

```asm

```
## google3_third_party_jetski_gateway_gateway.ConfigureDefaultTransport

```asm
0x06d96128  str   x0, [arg_68h]                         ; arg1
0x06d9612c  strb  w1, [arg_70h]                         ; arg2
0x06d96130  str   x2, [arg_70h + 0x8]                   ; arg3
0x06d96134  str   d0, [arg_70h + 0x10]
0x06d96138  stp   x3, x4, [arg_70h + 0x18]              ; arg5
0x06d9613c  ldr   x0, [arg_68h]                         ; [0x68:4]=0x380
0x06d96140  cmp   x0, 1
0x06d96144  b.gt  0x6d961d4
0x06d96150  ldrb  w5, [arg_70h]                         ; [0x70:4]=8
0x06d96154  adrp  x6, 0x4a82000
0x06d96158  add   x6, x6, 0xb20
0x06d9615c  add   x1, x6, x5, lsl 3
0x06d96160  adrp  x5, 0x8f08000
0x06d96164  add   x5, x5, 0x88
0x06d96168  adrp  x6, 0x94f7000
0x06d9616c  add   x6, x6, 0xfe0
0x06d96170  stp   x5, x6, [arg_48h]
0x06d96174  adrp  x0, 0x856f000
0x06d96178  add   x0, x0, 0x950
0x06d9617c  add   x2, arg_48h
0x06d96180  orr   x3, xzr, 1
0x06d96184  mov   x4, x3
0x06d96188  bl    fcn.04f32280
0x06d9618c  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d96190  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380
0x06d96194  ret
0x06d9619c  adrp  x1, 0x81ab000
0x06d961a0  add   x1, x1, 0x6d8
0x06d961a4  orr   x2, xzr, 1
0x06d961a8  bl    fcn.04d33a20
0x06d961ac  mov   x3, 0x756e                            ; 'nu'
0x06d961b0  movk  x3, 0x6c6c, lsl 16
0x06d961b4  str   w3, [x0]
0x06d961b8  orr   x1, xzr, 4
0x06d961bc  mov   x2, x1
0x06d961c0  mov   x3, xzr
0x06d961c4  mov   x4, xzr
0x06d961c8  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d961cc  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380
0x06d961d0  ret
0x06d961d4  cmp   x0, 2
0x06d961d8  b.eq  0x6d96300
0x06d961dc  cmp   x0, 3
0x06d961e4  cmp   x0, 4
0x06d961f0  bl    fcn.04d98810
0x06d961f4  adrp  x1, 0x8571000
0x06d961f8  add   x1, x1, 0xd0
0x06d961fc  stp   x1, x0, [arg_38h]
0x06d96200  adrp  x0, 0x4358000
0x06d96204  add   x0, x0, 0xd10                      ; "unknown flagKind %dworkspace not foundAGY_LLM_GATEWAY_URLPythonFile1000LinesRevi"
0x06d96208  mov   x1, 0x13
0x06d9620c  add   x2, arg_38h
0x06d96210  orr   x3, xzr, 1
0x06d96214  mov   x4, x3
0x06d96218  bl    fcn.04e321f0
0x06d96220  nop
0x06d96224  orr   x0, xzr, 0x10
0x06d96228  adrp  x1, 0x8757000
0x06d9622c  add   x1, x1, 0x890
0x06d96230  orr   x2, xzr, 1
0x06d96234  bl    fcn.04d32500
0x06d96238  adrp  x3, 0x4358000
0x06d9623c  add   x3, x3, 0xd10                      ; "unknown flagKind %dworkspace not foundAGY_LLM_GATEWAY_URLPythonFile1000LinesRevi"
0x06d96240  mov   x4, 0x13
0x06d96244  stp   x3, x4, [x0]
0x06d96248  mov   x1, x0
0x06d9624c  adrp  x0, 0x8f07000
0x06d96250  add   x0, x0, 0x508
0x06d96258  mov   x3, x0
0x06d9625c  mov   x4, x1
0x06d96260  mov   x0, xzr
0x06d96264  mov   x1, xzr
0x06d96268  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d9626c  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380
0x06d96270  ret
0x06d96278  mov   x1, x4                                ; arg5
0x06d9627c  bl    fcn.04d98880
0x06d96280  adrp  x2, 0x8f08000
0x06d96284  add   x2, x2, 0x88
0x06d96288  adrp  x3, 0x94f7000
0x06d9628c  add   x3, x3, 0xfe0
0x06d96290  stp   x2, x3, [arg_48h]
0x06d96294  mov   x1, x0
0x06d96298  add   x2, arg_48h
0x06d9629c  orr   x3, xzr, 1
0x06d962a0  mov   x4, x3
0x06d962a4  adrp  x0, 0x856f000
0x06d962a8  add   x0, x0, 0x550
0x06d962ac  bl    fcn.04f32280
0x06d962b0  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d962b4  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380
0x06d962b8  ret
0x06d962c0  bl    fcn.04d98810
0x06d962c4  adrp  x1, 0x8f08000
0x06d962c8  add   x1, x1, 0x88
0x06d962cc  adrp  x2, 0x94f7000
0x06d962d0  add   x2, x2, 0xfe0
0x06d962d4  stp   x1, x2, [arg_48h]
0x06d962d8  mov   x1, x0
0x06d962dc  add   x2, arg_48h
0x06d962e0  orr   x3, xzr, 1
0x06d962e4  mov   x4, x3
0x06d962e8  adrp  x0, 0x833c000
0x06d962ec  add   x0, x0, 0xfc8
0x06d962f0  bl    fcn.04f32280
0x06d962f4  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d962f8  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380
0x06d962fc  ret
0x06d96300  mov   x0, x2                                ; arg3
0x06d96304  mov   x1, 0xa
0x06d96308  bl    fcn.04d1efa0
0x06d9630c  mov   x2, x1
0x06d96310  mov   x1, x0
0x06d96314  mov   x0, xzr
0x06d96318  bl    fcn.04d7d7c0
0x06d9631c  mov   x3, xzr
0x06d96320  mov   x4, xzr
0x06d96324  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d96328  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380

```
## google3_third_party_jetski_gateway_gateway.redactURL

```asm
0x06d96128  str   x0, [arg_68h]                         ; arg1
0x06d9612c  strb  w1, [arg_70h]                         ; arg2
0x06d96130  str   x2, [arg_70h + 0x8]                   ; arg3
0x06d96134  str   d0, [arg_70h + 0x10]
0x06d96138  stp   x3, x4, [arg_70h + 0x18]              ; arg5
0x06d9613c  ldr   x0, [arg_68h]                         ; [0x68:4]=0x380
0x06d96140  cmp   x0, 1
0x06d96144  b.gt  0x6d961d4
0x06d96150  ldrb  w5, [arg_70h]                         ; [0x70:4]=8
0x06d96154  adrp  x6, 0x4a82000
0x06d96158  add   x6, x6, 0xb20
0x06d9615c  add   x1, x6, x5, lsl 3
0x06d96160  adrp  x5, 0x8f08000
0x06d96164  add   x5, x5, 0x88
0x06d96168  adrp  x6, 0x94f7000
0x06d9616c  add   x6, x6, 0xfe0
0x06d96170  stp   x5, x6, [arg_48h]
0x06d96174  adrp  x0, 0x856f000
0x06d96178  add   x0, x0, 0x950
0x06d9617c  add   x2, arg_48h
0x06d96180  orr   x3, xzr, 1
0x06d96184  mov   x4, x3
0x06d96188  bl    fcn.04f32280
0x06d9618c  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d96190  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380
0x06d96194  ret
0x06d9619c  adrp  x1, 0x81ab000
0x06d961a0  add   x1, x1, 0x6d8
0x06d961a4  orr   x2, xzr, 1
0x06d961a8  bl    fcn.04d33a20
0x06d961ac  mov   x3, 0x756e                            ; 'nu'
0x06d961b0  movk  x3, 0x6c6c, lsl 16
0x06d961b4  str   w3, [x0]
0x06d961b8  orr   x1, xzr, 4
0x06d961bc  mov   x2, x1
0x06d961c0  mov   x3, xzr
0x06d961c4  mov   x4, xzr
0x06d961c8  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d961cc  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380
0x06d961d0  ret
0x06d961d4  cmp   x0, 2
0x06d961d8  b.eq  0x6d96300
0x06d961dc  cmp   x0, 3
0x06d961e4  cmp   x0, 4
0x06d961f0  bl    fcn.04d98810
0x06d961f4  adrp  x1, 0x8571000
0x06d961f8  add   x1, x1, 0xd0
0x06d961fc  stp   x1, x0, [arg_38h]
0x06d96200  adrp  x0, 0x4358000
0x06d96204  add   x0, x0, 0xd10                      ; "unknown flagKind %dworkspace not foundAGY_LLM_GATEWAY_URLPythonFile1000LinesRevi"
0x06d96208  mov   x1, 0x13
0x06d9620c  add   x2, arg_38h
0x06d96210  orr   x3, xzr, 1
0x06d96214  mov   x4, x3
0x06d96218  bl    fcn.04e321f0
0x06d96220  nop
0x06d96224  orr   x0, xzr, 0x10
0x06d96228  adrp  x1, 0x8757000
0x06d9622c  add   x1, x1, 0x890
0x06d96230  orr   x2, xzr, 1
0x06d96234  bl    fcn.04d32500
0x06d96238  adrp  x3, 0x4358000
0x06d9623c  add   x3, x3, 0xd10                      ; "unknown flagKind %dworkspace not foundAGY_LLM_GATEWAY_URLPythonFile1000LinesRevi"
0x06d96240  mov   x4, 0x13
0x06d96244  stp   x3, x4, [x0]
0x06d96248  mov   x1, x0
0x06d9624c  adrp  x0, 0x8f07000
0x06d96250  add   x0, x0, 0x508
0x06d96258  mov   x3, x0
0x06d9625c  mov   x4, x1
0x06d96260  mov   x0, xzr
0x06d96264  mov   x1, xzr
0x06d96268  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d9626c  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380
0x06d96270  ret
0x06d96278  mov   x1, x4                                ; arg5
0x06d9627c  bl    fcn.04d98880
0x06d96280  adrp  x2, 0x8f08000
0x06d96284  add   x2, x2, 0x88
0x06d96288  adrp  x3, 0x94f7000
0x06d9628c  add   x3, x3, 0xfe0
0x06d96290  stp   x2, x3, [arg_48h]
0x06d96294  mov   x1, x0
0x06d96298  add   x2, arg_48h
0x06d9629c  orr   x3, xzr, 1
0x06d962a0  mov   x4, x3
0x06d962a4  adrp  x0, 0x856f000
0x06d962a8  add   x0, x0, 0x550
0x06d962ac  bl    fcn.04f32280
0x06d962b0  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d962b4  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380
0x06d962b8  ret
0x06d962c0  bl    fcn.04d98810
0x06d962c4  adrp  x1, 0x8f08000
0x06d962c8  add   x1, x1, 0x88
0x06d962cc  adrp  x2, 0x94f7000
0x06d962d0  add   x2, x2, 0xfe0
0x06d962d4  stp   x1, x2, [arg_48h]
0x06d962d8  mov   x1, x0
0x06d962dc  add   x2, arg_48h
0x06d962e0  orr   x3, xzr, 1
0x06d962e4  mov   x4, x3
0x06d962e8  adrp  x0, 0x833c000
0x06d962ec  add   x0, x0, 0xfc8
0x06d962f0  bl    fcn.04f32280
0x06d962f4  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d962f8  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380
0x06d962fc  ret
0x06d96300  mov   x0, x2                                ; arg3
0x06d96304  mov   x1, 0xa
0x06d96308  bl    fcn.04d1efa0
0x06d9630c  mov   x2, x1
0x06d96310  mov   x1, x0
0x06d96314  mov   x0, xzr
0x06d96318  bl    fcn.04d7d7c0
0x06d9631c  mov   x3, xzr
0x06d96320  mov   x4, xzr
0x06d96324  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d96328  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380

```
## AuthProvider_.GetModelCatalog

```asm
0x06d96128  str   x0, [arg_68h]                         ; arg1
0x06d9612c  strb  w1, [arg_70h]                         ; arg2
0x06d96130  str   x2, [arg_70h + 0x8]                   ; arg3
0x06d96134  str   d0, [arg_70h + 0x10]
0x06d96138  stp   x3, x4, [arg_70h + 0x18]              ; arg5
0x06d9613c  ldr   x0, [arg_68h]                         ; [0x68:4]=0x380
0x06d96140  cmp   x0, 1
0x06d96144  b.gt  0x6d961d4
0x06d96150  ldrb  w5, [arg_70h]                         ; [0x70:4]=8
0x06d96154  adrp  x6, 0x4a82000
0x06d96158  add   x6, x6, 0xb20
0x06d9615c  add   x1, x6, x5, lsl 3
0x06d96160  adrp  x5, 0x8f08000
0x06d96164  add   x5, x5, 0x88
0x06d96168  adrp  x6, 0x94f7000
0x06d9616c  add   x6, x6, 0xfe0
0x06d96170  stp   x5, x6, [arg_48h]
0x06d96174  adrp  x0, 0x856f000
0x06d96178  add   x0, x0, 0x950
0x06d9617c  add   x2, arg_48h
0x06d96180  orr   x3, xzr, 1
0x06d96184  mov   x4, x3
0x06d96188  bl    fcn.04f32280
0x06d9618c  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d96190  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380
0x06d96194  ret
0x06d9619c  adrp  x1, 0x81ab000
0x06d961a0  add   x1, x1, 0x6d8
0x06d961a4  orr   x2, xzr, 1
0x06d961a8  bl    fcn.04d33a20
0x06d961ac  mov   x3, 0x756e                            ; 'nu'
0x06d961b0  movk  x3, 0x6c6c, lsl 16
0x06d961b4  str   w3, [x0]
0x06d961b8  orr   x1, xzr, 4
0x06d961bc  mov   x2, x1
0x06d961c0  mov   x3, xzr
0x06d961c4  mov   x4, xzr
0x06d961c8  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d961cc  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380
0x06d961d0  ret
0x06d961d4  cmp   x0, 2
0x06d961d8  b.eq  0x6d96300
0x06d961dc  cmp   x0, 3
0x06d961e4  cmp   x0, 4
0x06d961f0  bl    fcn.04d98810
0x06d961f4  adrp  x1, 0x8571000
0x06d961f8  add   x1, x1, 0xd0
0x06d961fc  stp   x1, x0, [arg_38h]
0x06d96200  adrp  x0, 0x4358000
0x06d96204  add   x0, x0, 0xd10                      ; "unknown flagKind %dworkspace not foundAGY_LLM_GATEWAY_URLPythonFile1000LinesRevi"
0x06d96208  mov   x1, 0x13
0x06d9620c  add   x2, arg_38h
0x06d96210  orr   x3, xzr, 1
0x06d96214  mov   x4, x3
0x06d96218  bl    fcn.04e321f0
0x06d96220  nop
0x06d96224  orr   x0, xzr, 0x10
0x06d96228  adrp  x1, 0x8757000
0x06d9622c  add   x1, x1, 0x890
0x06d96230  orr   x2, xzr, 1
0x06d96234  bl    fcn.04d32500
0x06d96238  adrp  x3, 0x4358000
0x06d9623c  add   x3, x3, 0xd10                      ; "unknown flagKind %dworkspace not foundAGY_LLM_GATEWAY_URLPythonFile1000LinesRevi"
0x06d96240  mov   x4, 0x13
0x06d96244  stp   x3, x4, [x0]
0x06d96248  mov   x1, x0
0x06d9624c  adrp  x0, 0x8f07000
0x06d96250  add   x0, x0, 0x508
0x06d96258  mov   x3, x0
0x06d9625c  mov   x4, x1
0x06d96260  mov   x0, xzr
0x06d96264  mov   x1, xzr
0x06d96268  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d9626c  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380
0x06d96270  ret
0x06d96278  mov   x1, x4                                ; arg5
0x06d9627c  bl    fcn.04d98880
0x06d96280  adrp  x2, 0x8f08000
0x06d96284  add   x2, x2, 0x88
0x06d96288  adrp  x3, 0x94f7000
0x06d9628c  add   x3, x3, 0xfe0
0x06d96290  stp   x2, x3, [arg_48h]
0x06d96294  mov   x1, x0
0x06d96298  add   x2, arg_48h
0x06d9629c  orr   x3, xzr, 1
0x06d962a0  mov   x4, x3
0x06d962a4  adrp  x0, 0x856f000
0x06d962a8  add   x0, x0, 0x550
0x06d962ac  bl    fcn.04f32280
0x06d962b0  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d962b4  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380
0x06d962b8  ret
0x06d962c0  bl    fcn.04d98810
0x06d962c4  adrp  x1, 0x8f08000
0x06d962c8  add   x1, x1, 0x88
0x06d962cc  adrp  x2, 0x94f7000
0x06d962d0  add   x2, x2, 0xfe0
0x06d962d4  stp   x1, x2, [arg_48h]
0x06d962d8  mov   x1, x0
0x06d962dc  add   x2, arg_48h
0x06d962e0  orr   x3, xzr, 1
0x06d962e4  mov   x4, x3
0x06d962e8  adrp  x0, 0x833c000
0x06d962ec  add   x0, x0, 0xfc8
0x06d962f0  bl    fcn.04f32280
0x06d962f4  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d962f8  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380
0x06d962fc  ret
0x06d96300  mov   x0, x2                                ; arg3
0x06d96304  mov   x1, 0xa
0x06d96308  bl    fcn.04d1efa0
0x06d9630c  mov   x2, x1
0x06d96310  mov   x1, x0
0x06d96314  mov   x0, xzr
0x06d96318  bl    fcn.04d7d7c0
0x06d9631c  mov   x3, xzr
0x06d96320  mov   x4, xzr
0x06d96324  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d96328  ldr   lr, [sp], 0x60                        ; [0x60:4]=0x380

```
## AuthProvider_.AdminFileSetsModels

```asm
0x06d96c28  adrp  x0, 0x8cd3000
0x06d96c2c  add   x0, x0, 0x38
0x06d96c30  bl    0x4d31c10
0x06d96c34  ldp   x1, x2, [arg_78h]
0x06d96c38  stp   x1, x2, [x0, 0x28]
0x06d96c3c  adrp  x27, 0xb48f000
0x06d96c40  ldr   w1, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d96c44  cbnz  w1, 0x6d96c50
0x06d96c48  ldr   x1, [arg_298h]                        ; [0xd8:4]=0
0x06d96c50  bl    fcn.04da34f0
0x06d96c54  ldr   x1, [arg_d8h]                         ; [0xd8:4]=0
0x06d96c58  str   x1, [x25]
0x06d96c60  ldp   fp, lr, [sp, -8]
0x06d96c64  add   sp, sp, 0x1c0
0x06d96c68  ret

```
## google3_third_party_jetski_gateway_gateway.isImageModel

```asm
0x06d96c28  adrp  x0, 0x8cd3000
0x06d96c2c  add   x0, x0, 0x38
0x06d96c30  bl    0x4d31c10
0x06d96c34  ldp   x1, x2, [arg_78h]
0x06d96c38  stp   x1, x2, [x0, 0x28]
0x06d96c3c  adrp  x27, 0xb48f000
0x06d96c40  ldr   w1, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d96c44  cbnz  w1, 0x6d96c50
0x06d96c48  ldr   x1, [arg_298h]                        ; [0xd8:4]=0
0x06d96c50  bl    fcn.04da34f0
0x06d96c54  ldr   x1, [arg_d8h]                         ; [0xd8:4]=0
0x06d96c58  str   x1, [x25]
0x06d96c60  ldp   fp, lr, [sp, -8]
0x06d96c64  add   sp, sp, 0x1c0
0x06d96c68  ret

```
## google3_third_party_jetski_gateway_gateway.findKnownModelInfo

```asm
0x06d96c28  adrp  x0, 0x8cd3000
0x06d96c2c  add   x0, x0, 0x38
0x06d96c30  bl    0x4d31c10
0x06d96c34  ldp   x1, x2, [arg_78h]
0x06d96c38  stp   x1, x2, [x0, 0x28]
0x06d96c3c  adrp  x27, 0xb48f000
0x06d96c40  ldr   w1, [x27, 0x450]                      ; [0x450:4]=0x58585858
0x06d96c44  cbnz  w1, 0x6d96c50
0x06d96c48  ldr   x1, [arg_298h]                        ; [0xd8:4]=0
0x06d96c50  bl    fcn.04da34f0
0x06d96c54  ldr   x1, [arg_d8h]                         ; [0xd8:4]=0
0x06d96c58  str   x1, [x25]
0x06d96c60  ldp   fp, lr, [sp, -8]
0x06d96c64  add   sp, sp, 0x1c0
0x06d96c68  ret

```
## google3_third_party_jetski_gateway_gateway.isSupportedMimeType

```asm

```
## google3_third_party_jetski_gateway_gateway.modelEntryToModelDetails

```asm
0x06d96df8  add   x4, arg_1c0h
0x06d96dfc  str   x4, [arg_1c0h + 0x218]
0x06d96e00  adrp  x0, 0x8333000
0x06d96e04  add   x0, x0, 0x198
0x06d96e08  bl    0x4d98fd0
0x06d96e0c  str   x0, [arg_1a0h]
0x06d96e10  add   x2, arg_3f8h
0x06d96e14  stp   xzr, xzr, [x2]
0x06d96e18  stp   xzr, xzr, [x2, 0x10]
0x06d96e1c  stp   xzr, xzr, [x2, 0x20]
0x06d96e20  stp   xzr, xzr, [x2, 0x30]
0x06d96e24  stp   xzr, xzr, [x2, 0x40]
0x06d96e28  stp   xzr, xzr, [x2, 0x50]
0x06d96e2c  adrp  x0, 0x8333000
0x06d96e30  add   x0, x0, 0x198
0x06d96e34  ldr   x1, [arg_190h]                        ; [0x190:4]=2
0x06d96e38  bl    fcn.04d35800
0x06d96e3c  b     0x6d96e78
0x06d96ee4  ldr   x2, [arg_88h]                         ; [0x88:4]=0
0x06d96ee8  cmp   x2, 3
0x06d96eec  b.eq  0x6d96ef8
0x06d96ef0  ldr   x1, [arg_188h]                        ; [0x188:4]=1
0x06d96ef8  ldr   x1, [arg_188h]                        ; [0x188:4]=1
0x06d96efc  ldrh  w7, [x1]
0x06d96f00  mov   x8, 0x5548                            ; 'HU'
0x06d96f04  cmp   w7, w8
0x06d96f08  b.ne  0x6d96f18
0x06d96f0c  ldrb  w7, [x1, 2]                           ; [0x2:4]=0x102464c
0x06d96f10  cmp   w7, 0x42                              ; 'B'
0x06d96f18  ldr   x4, [arg_1a8h]                        ; [0x1a8:4]=0x799bde0 segment.DYNAMIC
0x06d96f1c  ldr   x4, [x4, 8]                           ; [0x8:4]=0
0x06d96f20  adrp  x0, 0x8330000
0x06d96f24  add   x0, x0, 0xd78
0x06d96f28  mov   x3, x2
0x06d96f2c  mov   x2, x1
0x06d96f30  mov   x1, x4
0x06d96f34  bl    0x4d18950
0x06d96f38  ldr   x1, [x0]
0x06d96f3c  add   x2, arg_3f8h
0x06d96f40  stp   xzr, xzr, [x2]
0x06d96f44  stp   xzr, xzr, [x2, 0x10]
0x06d96f48  stp   xzr, xzr, [x2, 0x20]
0x06d96f4c  stp   xzr, xzr, [x2, 0x30]
0x06d96f50  stp   xzr, xzr, [x2, 0x40]
0x06d96f54  stp   xzr, xzr, [x2, 0x50]
0x06d96f58  adrp  x0, 0x8333000
0x06d96f5c  add   x0, x0, 0x198
0x06d96f60  bl    fcn.04d35800
0x06d96f64  b     0x6d97350

```
## google3_third_party_jetski_gateway_gateway.buildResponseFromAdminModelList

```asm
0x06d978e8  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d978ec  ldr   lr, [sp], 0lr                         ; [0x30:4]=0

```
## google3_third_party_jetski_gateway_gateway.effectivePort

```asm

```
## google3_third_party_jetski_gateway_gateway.shouldAttachGatewayHeaders

```asm

```
## AuthProvider_.fetchRemoteModelCatalog

```asm
0x06d98c68  add   x3, x3, 0xb38                         ; arg4
0x06d98c6c  bl    fcn.05116df0
0x06d98c70  tbnz  w0, 0, 0x6d98c8c
0x06d98c74  adrp  x27, 0x948d000
0x06d98c78  add   x27, x27, 0xde0
0x06d98c7c  ldp   x2, x3, [x27]
0x06d98c80  ldp   x0, x1, [arg_28h]
0x06d98c84  bl    fcn.05116df0
0x06d98c8c  mov   x0, xzr
0x06d98c90  mov   x1, xzr
0x06d98c94  mov   x2, xzr
0x06d98c98  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d98c9c  ldr   lr, [sp], 0x40                        ; segment.PHDR
0x06d98ca0  ret
0x06d98ca8  ldp   x1, x2, [arg_28h]
0x06d98cac  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d98cb0  ldr   lr, [sp], 0x40                        ; segment.PHDR

```
## AuthProvider_.fetchGatewayDiscoveredModels

```asm
0x06d994a8  b     0x6d9951c
0x06d99520  str   x0, [arg_48h]                         ; arg1
0x06d99524  str   x3, [sp, 0x30]                        ; arg4
0x06d99528  mov   x0, x1                                ; arg2
0x06d9952c  bl    fcn.06d99f50
0x06d99530  mov   x2, x1
0x06d99534  mov   x1, x0
0x06d99538  ldr   x0, [arg_48h]                         ; [0x48:4]=64
0x06d9953c  bl    fcn.06d99710
0x06d99540  tbz   w0, 0, 0x6d9955c
0x06d99544  ldr   x0, [arg_70h]                         ; [0x30:4]=0
0x06d99548  mov   x1, xzr
0x06d9954c  mov   x2, xzr
0x06d99550  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d99554  ldr   lr, [sp], 0x40                        ; segment.PHDR
0x06d99558  ret
0x06d9955c  mov   x0, xzr
0x06d99560  mov   x1, xzr
0x06d99564  mov   x2, xzr
0x06d99568  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d9956c  ldr   lr, [sp], 0x40                        ; segment.PHDR
0x06d99570  ret
0x06d99578  mov   x1, xzr
0x06d9957c  mov   x2, xzr
0x06d99580  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d99584  ldr   lr, [sp], 0x40                        ; segment.PHDR

```
## google3_third_party_jetski_gateway_gateway.buildDefaultGatewayCatalog

```asm
0x06d9aad8  add   x1, x1, 0x890                         ; arg2
0x06d9aadc  orr   x2, xzr, 1
0x06d9aae0  bl    fcn.04d32500
0x06d9aae4  adrp  x3, 0x4481000
0x06d9aae8  add   x3, x3, 0xd12                      ; "gateway URL %q has unsupported scheme %q; must be http or httpshttps://drive.goo"
0x06d9aaec  orr   x4, xzr, 0x3f
0x06d9aaf0  stp   x3, x4, [x0]
0x06d9aaf4  mov   x1, x0
0x06d9aaf8  adrp  x0, 0x8f07000
0x06d9aafc  add   x0, x0, 0x508
0x06d9ab00  ldp   fp, lr, [sp, -8]
0x06d9ab04  add   sp, sp, 0x100

```
## AuthProvider_.AdminSettings.deferwrap1

```asm
0x06d9aba8  stp   x2, x3, [arg_70h + 0x10]              ; arg4
0x06d9abac  adrp  x3, 0x79a4000
0x06d9abb0  add   x3, x3, 0x230
0x06d9abb4  stp   x2, x3, [arg_70h + 0x20]              ; arg3
0x06d9abb8  adrp  x0, 0x446d000
0x06d9abbc  add   x0, x0, 0x284                      ; "gateway wireProtocol %q is unsupported; must be %q or %q[Admin Control] Failed t"
0x06d9abc0  orr   x1, xzr, 0x38
0x06d9abc4  add   x2, arg_70h
0x06d9abc8  orr   x3, xzr, 3
0x06d9abcc  mov   x4, x3
0x06d9abd0  bl    fcn.04e321f0
0x06d9abd4  cbnz  x0, 0x6d9ac0c
0x06d9abd8  nop
0x06d9abdc  orr   x0, xzr, 0x10
0x06d9abe0  adrp  x1, 0x8757000
0x06d9abe4  add   x1, x1, 0x890
0x06d9abe8  orr   x2, xzr, 1
0x06d9abec  bl    fcn.04d32500
0x06d9abf0  adrp  x3, 0x446d000
0x06d9abf4  add   x3, x3, 0x284                      ; "gateway wireProtocol %q is unsupported; must be %q or %q[Admin Control] Failed t"
0x06d9abf8  orr   x4, xzr, 0x38
0x06d9abfc  stp   x3, x4, [x0]
0x06d9ac00  mov   x1, x0
0x06d9ac04  adrp  x0, 0x8f07000
0x06d9ac08  add   x0, x0, 0x508
0x06d9ac0c  ldp   fp, lr, [sp, -8]
0x06d9ac10  add   sp, sp, 0x100

```
## AuthProvider_.setLastDiscoveredCatalog.deferwrap1

```asm
0x06d9ac28  bl    fcn.04d98880
0x06d9ac2c  adrp  x2, 0x856f000
0x06d9ac30  add   x2, x2, 0x550
0x06d9ac34  stp   x2, x0, [arg_a0h]
0x06d9ac38  adrp  x0, 0x44a9000
0x06d9ac3c  add   x0, x0, 0x8c9                      ; "gateway URL %q is missing a hostname (e.g. http://localhost:4000 or https://gate"
0x06d9ac40  mov   x1, 0x59                              ; 'Y'
0x06d9ac44  add   x2, arg_a0h
0x06d9ac48  orr   x3, xzr, 1
0x06d9ac4c  mov   x4, x3
0x06d9ac50  bl    fcn.04e321f0
0x06d9ac54  cbnz  x0, 0x6d9ac8c
0x06d9ac58  nop
0x06d9ac5c  orr   x0, xzr, 0x10
0x06d9ac60  adrp  x1, 0x8757000
0x06d9ac64  add   x1, x1, 0x890
0x06d9ac68  orr   x2, xzr, 1
0x06d9ac6c  bl    fcn.04d32500
0x06d9ac70  adrp  x3, 0x44a9000
0x06d9ac74  add   x3, x3, 0x8c9                      ; "gateway URL %q is missing a hostname (e.g. http://localhost:4000 or https://gate"
0x06d9ac78  mov   x4, 0x59                              ; 'Y'
0x06d9ac7c  stp   x3, x4, [x0]
0x06d9ac80  mov   x1, x0
0x06d9ac84  adrp  x0, 0x8f07000
0x06d9ac88  add   x0, x0, 0x508
0x06d9ac8c  ldp   fp, lr, [sp, -8]
0x06d9ac90  add   sp, sp, 0x100

```
## AuthProvider_.getLastDiscoveredCatalog.deferwrap1

```asm
0x06d9ac28  bl    fcn.04d98880
0x06d9ac2c  adrp  x2, 0x856f000
0x06d9ac30  add   x2, x2, 0x550
0x06d9ac34  stp   x2, x0, [arg_a0h]
0x06d9ac38  adrp  x0, 0x44a9000
0x06d9ac3c  add   x0, x0, 0x8c9                      ; "gateway URL %q is missing a hostname (e.g. http://localhost:4000 or https://gate"
0x06d9ac40  mov   x1, 0x59                              ; 'Y'
0x06d9ac44  add   x2, arg_a0h
0x06d9ac48  orr   x3, xzr, 1
0x06d9ac4c  mov   x4, x3
0x06d9ac50  bl    fcn.04e321f0
0x06d9ac54  cbnz  x0, 0x6d9ac8c
0x06d9ac58  nop
0x06d9ac5c  orr   x0, xzr, 0x10
0x06d9ac60  adrp  x1, 0x8757000
0x06d9ac64  add   x1, x1, 0x890
0x06d9ac68  orr   x2, xzr, 1
0x06d9ac6c  bl    fcn.04d32500
0x06d9ac70  adrp  x3, 0x44a9000
0x06d9ac74  add   x3, x3, 0x8c9                      ; "gateway URL %q is missing a hostname (e.g. http://localhost:4000 or https://gate"
0x06d9ac78  mov   x4, 0x59                              ; 'Y'
0x06d9ac7c  stp   x3, x4, [x0]
0x06d9ac80  mov   x1, x0
0x06d9ac84  adrp  x0, 0x8f07000
0x06d9ac88  add   x0, x0, 0x508
0x06d9ac8c  ldp   fp, lr, [sp, -8]
0x06d9ac90  add   sp, sp, 0x100

```
## AuthProvider_.GetHTTPClient.deferwrap1

```asm
0x06d9ace8  add   x0, x0, 0x203                         ; arg1
0x06d9acec  mov   x1, 0x1d
0x06d9acf0  add   x2, arg_d0h
0x06d9acf4  orr   x3, xzr, 2
0x06d9acf8  mov   x4, x3
0x06d9acfc  bl    fcn.04e321f0
0x06d9ad00  cbnz  x0, 0x6d9ad38
0x06d9ad04  nop
0x06d9ad08  orr   x0, xzr, 0x10
0x06d9ad0c  adrp  x1, 0x8757000
0x06d9ad10  add   x1, x1, 0x890
0x06d9ad14  orr   x2, xzr, 1
0x06d9ad18  bl    fcn.04d32500
0x06d9ad1c  adrp  x3, 0x43de000
0x06d9ad20  add   x3, x3, 0x203                      ; "gateway URL %q is invalid: %wAGY_LLM_GATEWAY_WIRE_PROTOCOL%s?pageSize=1000&pageT"
0x06d9ad24  mov   x4, 0x1d
0x06d9ad28  stp   x3, x4, [x0]
0x06d9ad2c  mov   x1, x0
0x06d9ad30  adrp  x0, 0x8f07000
0x06d9ad34  add   x0, x0, 0x508
0x06d9ad38  ldp   fp, lr, [sp, -8]
0x06d9ad3c  add   sp, sp, 0x100

```
## AuthProvider_.GetCACertPool.deferwrap1

```asm
0x06d9ad68  mov   x1, x0                                ; arg1
0x06d9ad6c  adrp  x0, 0x8f07000
0x06d9ad70  add   x0, x0, 0x508
0x06d9ad74  ldp   fp, lr, [sp, -8]
0x06d9ad78  add   sp, sp, 0x100

```
## AuthProvider_.GetCACertPath.deferwrap1

```asm
0x06d9adf4  stp   xzr, xzr, [arg_60h]
0x06d9adf8  ldr   x0, [arg_38h]                         ; [0x38:4]=0x400010
0x06d9adfc  bl    fcn.04d98810
0x06d9ae00  adrp  x1, 0x856f000
0x06d9ae04  add   x1, x1, 0x790
0x06d9ae08  stp   x1, x0, [arg_60h]
0x06d9ae0c  adrp  x0, 0x4413000
0x06d9ae10  add   x0, x0, 0xba5                      ; "models.list[%d] has an empty modelIdthe chosen response is still workingunknown "
0x06d9ae14  mov   x1, 0x24                              ; '$'
0x06d9ae18  add   x2, arg_60h
0x06d9ae1c  orr   x3, xzr, 1
0x06d9ae20  mov   x4, x3
0x06d9ae24  bl    fcn.04e321f0
0x06d9ae28  cbnz  x0, 0x6d9ae60
0x06d9ae2c  nop
0x06d9ae30  orr   x0, xzr, 0x10
0x06d9ae34  adrp  x1, 0x8757000
0x06d9ae38  add   x1, x1, 0x890
0x06d9ae3c  orr   x2, xzr, 1
0x06d9ae40  bl    fcn.04d32500
0x06d9ae44  adrp  x3, 0x4413000
0x06d9ae48  add   x3, x3, 0xba5                      ; "models.list[%d] has an empty modelIdthe chosen response is still workingunknown "
0x06d9ae4c  mov   x4, 0x24                              ; '$'
0x06d9ae50  stp   x3, x4, [x0]
0x06d9ae54  mov   x1, x0
0x06d9ae58  adrp  x0, 0x8f07000
0x06d9ae5c  add   x0, x0, 0x508
0x06d9ae60  ldp   fp, lr, [sp, -8]
0x06d9ae64  add   sp, sp, 0x100
0x06d9ae68  ret

```
## AuthProvider_.GetProxyURL.deferwrap1

```asm
0x06d9adf4  stp   xzr, xzr, [arg_60h]
0x06d9adf8  ldr   x0, [arg_38h]                         ; [0x38:4]=0x400010
0x06d9adfc  bl    fcn.04d98810
0x06d9ae00  adrp  x1, 0x856f000
0x06d9ae04  add   x1, x1, 0x790
0x06d9ae08  stp   x1, x0, [arg_60h]
0x06d9ae0c  adrp  x0, 0x4413000
0x06d9ae10  add   x0, x0, 0xba5                      ; "models.list[%d] has an empty modelIdthe chosen response is still workingunknown "
0x06d9ae14  mov   x1, 0x24                              ; '$'
0x06d9ae18  add   x2, arg_60h
0x06d9ae1c  orr   x3, xzr, 1
0x06d9ae20  mov   x4, x3
0x06d9ae24  bl    fcn.04e321f0
0x06d9ae28  cbnz  x0, 0x6d9ae60
0x06d9ae2c  nop
0x06d9ae30  orr   x0, xzr, 0x10
0x06d9ae34  adrp  x1, 0x8757000
0x06d9ae38  add   x1, x1, 0x890
0x06d9ae3c  orr   x2, xzr, 1
0x06d9ae40  bl    fcn.04d32500
0x06d9ae44  adrp  x3, 0x4413000
0x06d9ae48  add   x3, x3, 0xba5                      ; "models.list[%d] has an empty modelIdthe chosen response is still workingunknown "
0x06d9ae4c  mov   x4, 0x24                              ; '$'
0x06d9ae50  stp   x3, x4, [x0]
0x06d9ae54  mov   x1, x0
0x06d9ae58  adrp  x0, 0x8f07000
0x06d9ae5c  add   x0, x0, 0x508
0x06d9ae60  ldp   fp, lr, [sp, -8]
0x06d9ae64  add   sp, sp, 0x100
0x06d9ae68  ret

```
## AuthProvider_.GetProxyURLString.deferwrap1

```asm
0x06d9aee8  add   x4, x4, 0xfe0                         ; arg5
0x06d9aeec  stp   x3, x4, [arg_80h]                     ; arg5
0x06d9aef0  ldr   x3, [arg_60h]                         ; [0x60:4]=0x380
0x06d9aef4  cmp   x3, 0
0x06d9aef8  cset  x3, ne
0x06d9aefc  eor   x3, x3, 1
0x06d9af00  tst   w3, 1                                 ; arg1
0x06d9af04  ldr   x3, [arg_58h]                         ; [0x58:4]=64
0x06d9af08  ldr   x4, [arg_b8h]                         ; [0xb8:4]=0
0x06d9af0c  csel  x2, x3, x4, ne
0x06d9af10  ldr   x3, [arg_50h]                         ; [0x50:4]=64
0x06d9af14  ldr   x4, [arg_b0h]                         ; [0xb0:4]=0x6474e551
0x06d9af18  csel  x1, x3, x4, ne
0x06d9af1c  ldr   x3, [arg_68h]                         ; [0x68:4]=0x380
0x06d9af20  ldr   x4, [arg_a8h]                         ; [0xa8:4]=0x200000
0x06d9af24  csel  x3, x3, x4, ne
0x06d9af28  mov   x4, x0                                ; arg1
0x06d9af2c  add   x5, arg_80h
0x06d9af30  orr   x6, xzr, 1
0x06d9af34  mov   x7, x6
0x06d9af38  mov   x0, x3
0x06d9af3c  adrp  x3, 0x7f44000
0x06d9af40  add   x3, x3, 0xd88
0x06d9af44  bl    0x4f32ac0
0x06d9af48  cbz   x0, 0x6d9afc8
0x06d9af4c  stp   xzr, xzr, [arg_70h]
0x06d9af54  ldr   x0, [x0, 8]                           ; [0x8:4]=0
0x06d9af5c  adrp  x0, 0x4425000
0x06d9af60  add   x0, x0, 0xa97                      ; "failed to parse admin settings JSON: %wShip the current changes to production.up"
0x06d9af64  mov   x1, 0x27                              ; '''
0x06d9af68  add   x2, arg_70h
0x06d9af6c  orr   x3, xzr, 1
0x06d9af70  mov   x4, x3
0x06d9af74  bl    fcn.04e321f0
0x06d9af7c  nop
0x06d9af80  orr   x0, xzr, 0x10
0x06d9af84  adrp  x1, 0x8757000
0x06d9af88  add   x1, x1, 0x890
0x06d9af8c  orr   x2, xzr, 1
0x06d9af90  bl    fcn.04d32500
0x06d9af94  adrp  x3, 0x4425000
0x06d9af98  add   x3, x3, 0xa97                      ; "failed to parse admin settings JSON: %wShip the current changes to production.up"
0x06d9af9c  mov   x4, 0x27                              ; '''
0x06d9afa0  stp   x3, x4, [x0]
0x06d9afa4  mov   x1, x0
0x06d9afa8  adrp  x0, 0x8f07000
0x06d9afac  add   x0, x0, 0x508
0x06d9afb4  mov   x1, x0
0x06d9afb8  mov   x0, xzr
0x06d9afbc  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d9afc0  ldr   lr, [sp], 0xa0                        ; [0xa0:4]=0x779b960
0x06d9afc4  ret
0x06d9afc8  ldr   x0, [arg_90h]                         ; [0x90:4]=0
0x06d9afcc  mov   x1, xzr
0x06d9afd0  mov   x2, xzr
0x06d9afd4  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d9afd8  ldr   lr, [sp], 0xa0                        ; [0xa0:4]=0x779b960

```
## AuthProvider_.SetHTTPClient.deferwrap1

```asm
0x06d9aee8  add   x4, x4, 0xfe0                         ; arg5
0x06d9aeec  stp   x3, x4, [arg_80h]                     ; arg5
0x06d9aef0  ldr   x3, [arg_60h]                         ; [0x60:4]=0x380
0x06d9aef4  cmp   x3, 0
0x06d9aef8  cset  x3, ne
0x06d9aefc  eor   x3, x3, 1
0x06d9af00  tst   w3, 1                                 ; arg1
0x06d9af04  ldr   x3, [arg_58h]                         ; [0x58:4]=64
0x06d9af08  ldr   x4, [arg_b8h]                         ; [0xb8:4]=0
0x06d9af0c  csel  x2, x3, x4, ne
0x06d9af10  ldr   x3, [arg_50h]                         ; [0x50:4]=64
0x06d9af14  ldr   x4, [arg_b0h]                         ; [0xb0:4]=0x6474e551
0x06d9af18  csel  x1, x3, x4, ne
0x06d9af1c  ldr   x3, [arg_68h]                         ; [0x68:4]=0x380
0x06d9af20  ldr   x4, [arg_a8h]                         ; [0xa8:4]=0x200000
0x06d9af24  csel  x3, x3, x4, ne
0x06d9af28  mov   x4, x0                                ; arg1
0x06d9af2c  add   x5, arg_80h
0x06d9af30  orr   x6, xzr, 1
0x06d9af34  mov   x7, x6
0x06d9af38  mov   x0, x3
0x06d9af3c  adrp  x3, 0x7f44000
0x06d9af40  add   x3, x3, 0xd88
0x06d9af44  bl    0x4f32ac0
0x06d9af48  cbz   x0, 0x6d9afc8
0x06d9af4c  stp   xzr, xzr, [arg_70h]
0x06d9af54  ldr   x0, [x0, 8]                           ; [0x8:4]=0
0x06d9af5c  adrp  x0, 0x4425000
0x06d9af60  add   x0, x0, 0xa97                      ; "failed to parse admin settings JSON: %wShip the current changes to production.up"
0x06d9af64  mov   x1, 0x27                              ; '''
0x06d9af68  add   x2, arg_70h
0x06d9af6c  orr   x3, xzr, 1
0x06d9af70  mov   x4, x3
0x06d9af74  bl    fcn.04e321f0
0x06d9af7c  nop
0x06d9af80  orr   x0, xzr, 0x10
0x06d9af84  adrp  x1, 0x8757000
0x06d9af88  add   x1, x1, 0x890
0x06d9af8c  orr   x2, xzr, 1
0x06d9af90  bl    fcn.04d32500
0x06d9af94  adrp  x3, 0x4425000
0x06d9af98  add   x3, x3, 0xa97                      ; "failed to parse admin settings JSON: %wShip the current changes to production.up"
0x06d9af9c  mov   x4, 0x27                              ; '''
0x06d9afa0  stp   x3, x4, [x0]
0x06d9afa4  mov   x1, x0
0x06d9afa8  adrp  x0, 0x8f07000
0x06d9afac  add   x0, x0, 0x508
0x06d9afb4  mov   x1, x0
0x06d9afb8  mov   x0, xzr
0x06d9afbc  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d9afc0  ldr   lr, [sp], 0xa0                        ; [0xa0:4]=0x779b960
0x06d9afc4  ret
0x06d9afc8  ldr   x0, [arg_90h]                         ; [0x90:4]=0
0x06d9afcc  mov   x1, xzr
0x06d9afd0  mov   x2, xzr
0x06d9afd4  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d9afd8  ldr   lr, [sp], 0xa0                        ; [0xa0:4]=0x779b960

```
## AuthProvider_.GetEndpointURL.deferwrap1

```asm
0x06d9aee8  add   x4, x4, 0xfe0                         ; arg5
0x06d9aeec  stp   x3, x4, [arg_80h]                     ; arg5
0x06d9aef0  ldr   x3, [arg_60h]                         ; [0x60:4]=0x380
0x06d9aef4  cmp   x3, 0
0x06d9aef8  cset  x3, ne
0x06d9aefc  eor   x3, x3, 1
0x06d9af00  tst   w3, 1                                 ; arg1
0x06d9af04  ldr   x3, [arg_58h]                         ; [0x58:4]=64
0x06d9af08  ldr   x4, [arg_b8h]                         ; [0xb8:4]=0
0x06d9af0c  csel  x2, x3, x4, ne
0x06d9af10  ldr   x3, [arg_50h]                         ; [0x50:4]=64
0x06d9af14  ldr   x4, [arg_b0h]                         ; [0xb0:4]=0x6474e551
0x06d9af18  csel  x1, x3, x4, ne
0x06d9af1c  ldr   x3, [arg_68h]                         ; [0x68:4]=0x380
0x06d9af20  ldr   x4, [arg_a8h]                         ; [0xa8:4]=0x200000
0x06d9af24  csel  x3, x3, x4, ne
0x06d9af28  mov   x4, x0                                ; arg1
0x06d9af2c  add   x5, arg_80h
0x06d9af30  orr   x6, xzr, 1
0x06d9af34  mov   x7, x6
0x06d9af38  mov   x0, x3
0x06d9af3c  adrp  x3, 0x7f44000
0x06d9af40  add   x3, x3, 0xd88
0x06d9af44  bl    0x4f32ac0
0x06d9af48  cbz   x0, 0x6d9afc8
0x06d9af4c  stp   xzr, xzr, [arg_70h]
0x06d9af54  ldr   x0, [x0, 8]                           ; [0x8:4]=0
0x06d9af5c  adrp  x0, 0x4425000
0x06d9af60  add   x0, x0, 0xa97                      ; "failed to parse admin settings JSON: %wShip the current changes to production.up"
0x06d9af64  mov   x1, 0x27                              ; '''
0x06d9af68  add   x2, arg_70h
0x06d9af6c  orr   x3, xzr, 1
0x06d9af70  mov   x4, x3
0x06d9af74  bl    fcn.04e321f0
0x06d9af7c  nop
0x06d9af80  orr   x0, xzr, 0x10
0x06d9af84  adrp  x1, 0x8757000
0x06d9af88  add   x1, x1, 0x890
0x06d9af8c  orr   x2, xzr, 1
0x06d9af90  bl    fcn.04d32500
0x06d9af94  adrp  x3, 0x4425000
0x06d9af98  add   x3, x3, 0xa97                      ; "failed to parse admin settings JSON: %wShip the current changes to production.up"
0x06d9af9c  mov   x4, 0x27                              ; '''
0x06d9afa0  stp   x3, x4, [x0]
0x06d9afa4  mov   x1, x0
0x06d9afa8  adrp  x0, 0x8f07000
0x06d9afac  add   x0, x0, 0x508
0x06d9afb4  mov   x1, x0
0x06d9afb8  mov   x0, xzr
0x06d9afbc  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d9afc0  ldr   lr, [sp], 0xa0                        ; [0xa0:4]=0x779b960
0x06d9afc4  ret
0x06d9afc8  ldr   x0, [arg_90h]                         ; [0x90:4]=0
0x06d9afcc  mov   x1, xzr
0x06d9afd0  mov   x2, xzr
0x06d9afd4  ldur  fp, [sp, -8]                          ; [0x8:4]=0
0x06d9afd8  ldr   lr, [sp], 0xa0                        ; [0xa0:4]=0x779b960

```
## AuthProvider_.SetEndpointURL.deferwrap1

```asm

```
## AuthProvider_.GetWireProtocol.deferwrap1

```asm

```
## AuthProvider_.SetWireProtocol.deferwrap1

```asm

```
## AuthProvider_.SetHTTPHeaders.deferwrap1

```asm

```
## AuthProvider_.fetchRemoteModelCatalog.deferwrap1

```asm

```
## google3_third_party_jetski_gateway_gateway.NewHTTPTransportWithOptions.func1

```asm
```
