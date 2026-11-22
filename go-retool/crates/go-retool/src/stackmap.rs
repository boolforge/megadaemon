//! Decoder for Go runtime.stackmap payloads (version/layout must be verified).
use crate::{Error,Result};

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct StackMap { pub n_maps:u32, pub n_bits:u32, pub maps:Vec<Vec<bool>> }

/// Decode the in-memory layout: int32 n, int32 nbit, then n bitmaps, each
/// `ceil(nbit/8)` bytes. Bit numbering is least-significant-bit first, matching
/// runtime.bitvector.ptrbit. This function does not locate FUNCDATA or validate
/// that the selected map is appropriate for a particular PC.
pub fn decode_stackmap(data:&[u8], max_maps:u32, max_bits:u32)->Result<StackMap>{
 if data.len()<8{return Err(Error::Invalid("truncated stackmap header"));}
 let n=i32::from_le_bytes(data[0..4].try_into().unwrap());
 let nbit=i32::from_le_bytes(data[4..8].try_into().unwrap());
 if n<0 || nbit<0{return Err(Error::Invalid("negative stackmap dimensions"));}
 let n=n as u32;let nbit=nbit as u32;
 if n>max_maps || nbit>max_bits{return Err(Error::Limit("stackmap dimensions"));}
 let row=((nbit as u64)+7)/8;
 let need=8u64.checked_add((n as u64).checked_mul(row).ok_or(Error::Overflow("stackmap size"))?).ok_or(Error::Overflow("stackmap size"))?;
 if need>data.len() as u64{return Err(Error::Invalid("truncated stackmap bitmap data"));}
 let mut maps=Vec::with_capacity(n as usize);
 for i in 0..n as usize{
   let start=8+i*row as usize;let mut bits=Vec::with_capacity(nbit as usize);
   for j in 0..nbit as usize{bits.push((data[start+j/8]>>(j%8))&1==1);}
   maps.push(bits);
 }
 Ok(StackMap{n_maps:n,n_bits:nbit,maps})
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn lsb_first_and_multiple_maps(){let d=[2,0,0,0,9,0,0,0,0b0000_0101,0b0000_0010,0b0000_0001,0b0000_0000];let m=decode_stackmap(&d,8,32).unwrap();assert_eq!(m.maps[0],vec![true,false,true,false,false,false,false,false,true]);assert_eq!(m.maps[1],vec![true,false,false,false,false,false,false,false,false]);}
 #[test]fn rejects_negative_dimensions(){let mut d=vec![0;8];d[..4].copy_from_slice(&(-1i32).to_le_bytes());assert!(decode_stackmap(&d,10,100).is_err());}
}
