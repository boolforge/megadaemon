//! Go pcvalue stream decoder, following runtime.step/readvarint semantics.
use crate::{Error,Result};

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct PcRange { pub pc_start:u64, pub pc_end:u64, pub value:i32 }

fn uvarint32(data:&[u8], pos:&mut usize)->Result<u32>{
    let mut value=0u32; let mut shift=0u32;
    for _ in 0..5 {
        let b=*data.get(*pos).ok_or(Error::Invalid("truncated pcvalue varint"))?; *pos+=1;
        if shift==28 && (b&0xf0)!=0 { return Err(Error::Invalid("pcvalue varint overflow")); }
        value |= ((b&0x7f) as u32).checked_shl(shift).ok_or(Error::Overflow("varint shift"))?;
        if b&0x80==0 { return Ok(value); }
        shift+=7;
    }
    Err(Error::Invalid("overlong pcvalue varint"))
}
fn zigzag(v:u32)->i32 { ((v>>1) as i32) ^ (-((v&1) as i32)) }

/// Decode a complete pcvalue stream. The first value starts at pc_start with
/// an implicit initial value -1. A zero uvdelta terminates only after the first
/// pair. pcdelta is multiplied by the architecture's PC quantum (`minLC`).
pub fn decode_pcvalue(data:&[u8], pc_start:u64, pc_quantum:u8, max_entries:usize)->Result<Vec<PcRange>>{
    if pc_quantum==0 { return Err(Error::Invalid("pc quantum is zero")); }
    let mut p=0usize; let mut pc=pc_start; let mut value=-1i32; let mut first=true;
    let mut steps=0usize; let mut out=Vec::new();
    loop {
        if p>=data.len() { return Err(Error::Invalid("pcvalue stream has no terminator")); }
        let uv=uvarint32(data,&mut p)?;
        if uv==0 && !first { break; }
        steps+=1;
        if steps>max_entries { return Err(Error::Limit("pcvalue encoded pair count")); }
        value=value.checked_add(zigzag(uv)).ok_or(Error::Overflow("pcvalue value delta"))?;
        let pd=uvarint32(data,&mut p)?;
        let delta=(pd as u64).checked_mul(pc_quantum as u64).ok_or(Error::Overflow("pc delta * quantum"))?;
        let next=pc.checked_add(delta).ok_or(Error::Overflow("pc advance"))?;
        if next<pc { return Err(Error::Invalid("decreasing PC range")); }
        // A zero pcdelta is accepted as a same-PC state update, as runtime.step
        // does not reject it. It produces no non-empty interval to emit.
        if next>pc { out.push(PcRange{pc_start:pc,pc_end:next,value}); }
        pc=next; first=false;
    }
    Ok(out)
}

#[cfg(test)] mod tests {
 use super::*;
 #[test] fn one_entry_then_terminator(){
   // uvdelta=2 => +1, pcdelta=4, terminator=0
   let r=decode_pcvalue(&[2,4,0],0x1000,1,8).unwrap();
   assert_eq!(r,vec![PcRange{pc_start:0x1000,pc_end:0x1004,value:0}]);
 }
 #[test] fn zigzag_negative(){assert_eq!(zigzag(1),-1);assert_eq!(zigzag(3),-2);assert_eq!(zigzag(4),2);}
 #[test] fn pc_quantum_applied(){let r=decode_pcvalue(&[2,2,0],0x2000,4,8).unwrap();assert_eq!(r[0].pc_end,0x2008);}
 #[test] fn zero_pc_delta_updates_state_without_empty_range(){let r=decode_pcvalue(&[2,0,4,2,0],0x3000,1,8).unwrap();assert_eq!(r,vec![PcRange{pc_start:0x3000,pc_end:0x3002,value:2}]);}
}
