// callconv: x0 reg (x0, x1, x2, x3, x4, x5, x6, x7);
void f31 (void) {
    x0 = [sp + 0xf8]
    x1 = [sp + 0xc0]
    x2 = 0x42a69dd
    x3 = 0xa
    x4 = sp + 0xd8
    x5 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x6 = x5       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4e349a0 (x0, x1, 0x42a69dd, 0xa)
    [sp + 0xc8] = (x1, 2)
    if (!x1) goto 0x55a5cb4 // likely
    goto 0x55a68e8
    x0 = [sp + 0x90]
    if (!x0) goto 0x55a5e0c // likely
    x1 = [x0 + 8]
    x5 = 0x949de70
    ldar x6, [x5]
    x7 = [x6]
    w8 = [x0 + 0x10]
    goto 0x55a68ac
    x5 = 0x856f550
    x6 = reloc.fixup.author__s___s_tagger_not_setreq
    [sp + 0xd8] = (x5, 2)
    x1 = [sp + 0xc0] // (cstr 0x042a69dd) "parent %s\nLANGUAGE_CLANGUAGE_RRepoCommitRepoUploadGitUnstageGit"
    x2 = sp + 0xd8
    x3 = 1        // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    x4 = x3       // (pstr 0x02464c45) "ils.(*ValidateStreamingClientConn).Spec"
    0x4e34bf0 (x0, x1, 0x100780d8, 0x1)
    [sp + 0xc8] = (x1, 2)
    if (x1) goto 0x55a5e60 // unlikely
    x0 = [sp + 0x90]
    if (!x0) goto 0x55a5e80 // likely
    x1 = [x0 + 8]
    x3 = 0x949de90
    ldar x4, [x3]
    x5 = [x4]
    return x0;
}

