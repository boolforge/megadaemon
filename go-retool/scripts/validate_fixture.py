#!/usr/bin/env python3
"""Independent sanity checker for the synthetic Go fixture's modern pclntab.
It is not the production parser and does not discover container mappings."""
import argparse, json, struct
from pathlib import Path

def u(data, off, width): return int.from_bytes(data[off:off+width], 'little')
def main():
    ap=argparse.ArgumentParser(); ap.add_argument('binary',type=Path); ap.add_argument('--out',type=Path); a=ap.parse_args()
    b=a.binary.read_bytes(); magic=b'\xf1\xff\xff\xff'; bases=[]; pos=0
    while True:
        pos=b.find(magic,pos)
        if pos<0: break
        if pos+8<=len(b) and b[pos+6] in (1,2,4) and b[pos+7] in (4,8): bases.append(pos)
        pos+=1
    reports=[]
    for base in bases:
        ptr=b[base+7]; nfunc=u(b,base+8,ptr); fields=base+8+2*ptr
        offs=[u(b,fields+i*ptr,ptr) for i in range(6)]
        if nfunc==0 or nfunc>2_000_000: continue
        pcln=base+offs[5]; fn=base+offs[1]
        if pcln+(nfunc+1)*8>len(b): continue
        names=0; last=-1; bad=[]
        for i in range(nfunc):
            entry,funcoff=struct.unpack_from('<II',b,pcln+i*8)
            if entry<=last: bad.append('non-monotonic entryoff at %d'%i)
            last=entry; f=pcln+funcoff
            if f+44>len(b): bad.append('truncated _func at %d'%i); continue
            nameoff=struct.unpack_from('<i',b,f+4)[0]
            if nameoff<0 or fn+nameoff>=len(b): bad.append('invalid nameoff at %d'%i); continue
            end=b.find(b'\0',fn+nameoff,min(len(b),fn+nameoff+(1<<20)))
            if end<0: bad.append('unterminated name at %d'%i); continue
            names+=1
        sentinel=u(b,pcln+nfunc*8,4)
        if sentinel<=last: bad.append('invalid final sentinel')
        reports.append({'pclntab_file_offset':hex(base),'magic':hex(u(b,base,4)),'pointer_size':ptr,'min_lc':b[base+6],'nfunc':nfunc,'funcname_offset':hex(offs[1]),'pcln_offset':hex(offs[5]),'names_resolved':names,'sentinel_entryoff':hex(sentinel),'errors':bad})
    if not reports: raise SystemExit('no plausible Go pclntab found')
    result={'binary':str(a.binary),'pclntab_candidates':reports,'note':'Independent Python structural sanity check; not proof of complete Go release support.'}
    out=json.dumps(result,indent=2)
    if a.out: a.out.write_text(out+'\n')
    print(out)
    if all(x['errors'] for x in reports): raise SystemExit(1)
if __name__=='__main__': main()
