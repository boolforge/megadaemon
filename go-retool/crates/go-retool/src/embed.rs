//! Decoder for `embed.file` records once a caller has mapped the image into a
//! contiguous memory view. Discovery and container VA mapping are separate.
use crate::{Error,Result};

#[derive(Debug,Clone)]
pub struct MemoryImage<'a>{pub base_va:u64,pub bytes:&'a [u8]}
impl<'a> MemoryImage<'a>{
 pub fn resolve(&self,va:u64,len:u64)->Result<&'a [u8]>{
  let off=va.checked_sub(self.base_va).ok_or(Error::Invalid("VA below mapped image"))?;
  let end=off.checked_add(len).ok_or(Error::Overflow("memory image range"))?;
  if end>self.bytes.len() as u64{return Err(Error::Invalid("VA range outside mapped image"));}
  Ok(&self.bytes[off as usize..end as usize])
 }
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct EmbeddedFile{pub name_bytes:Vec<u8>,pub data:Vec<u8>,pub hash16:[u8;16],pub is_directory:bool}

/// Parse `count` consecutive records of `embed.file { name string; data string;
/// hash [16]byte }`. Caller supplies the VA of the first record and the exact
/// Go pointer width. This does not locate the slice header or verify hash algo.
pub fn decode_embed_records(image:&MemoryImage,records_va:u64,count:u64,ptr_size:u8,max_records:u64,max_file_size:u64)->Result<Vec<EmbeddedFile>>{
 if ptr_size!=4&&ptr_size!=8{return Err(Error::Invalid("embed pointer width must be 4 or 8"));}
 if count>max_records{return Err(Error::Limit("embed record count"));}
 let rec_size=(ptr_size as u64)*4+16;
 let total=count.checked_mul(rec_size).ok_or(Error::Overflow("embed record array"))?;
 let records=image.resolve(records_va,total)?;
 let read_u=|b:&[u8]|->u64{if ptr_size==4{u32::from_le_bytes(b[..4].try_into().unwrap()) as u64}else{u64::from_le_bytes(b[..8].try_into().unwrap())}};
 let mut out=Vec::with_capacity(count as usize);
 for i in 0..count as usize{
  let p=i*rec_size as usize;let w=ptr_size as usize;
  let name_va=read_u(&records[p..p+w]);let name_len=read_u(&records[p+w..p+2*w]);
  let data_va=read_u(&records[p+2*w..p+3*w]);let data_len=read_u(&records[p+3*w..p+4*w]);
  if name_len>max_file_size||data_len>max_file_size{return Err(Error::Limit("embedded file size"));}
  let name=image.resolve(name_va,name_len)?.to_vec();
  let data=if data_len==0{Vec::new()}else{image.resolve(data_va,data_len)?.to_vec()};
  let h=p+4*w;let mut hash=[0u8;16];hash.copy_from_slice(&records[h..h+16]);
  let is_directory=name.last()==Some(&b'/');
  if is_directory&&(data_len!=0||data_va!=0){return Err(Error::Invalid("directory embed record unexpectedly has data"));}
  out.push(EmbeddedFile{name_bytes:name,data,hash16:hash,is_directory});
 }
 Ok(out)
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn parse_one_64bit_record(){
  let mut b=vec![0u8;256];let base=0x1000u64;
  // record at VA 0x1000; name at 0x1100, payload at 0x1120.
  let rec=0usize;b[rec..rec+8].copy_from_slice(&0x1100u64.to_le_bytes());b[8..16].copy_from_slice(&5u64.to_le_bytes());b[16..24].copy_from_slice(&0x1120u64.to_le_bytes());b[24..32].copy_from_slice(&3u64.to_le_bytes());
  b[32..48].copy_from_slice(&[1;16]);b[0x100..0x105].copy_from_slice(b"a.txt");b[0x120..0x123].copy_from_slice(b"abc");
  let im=MemoryImage{base_va:base,bytes:&b};let v=decode_embed_records(&im,base,1,8,10,1024).unwrap();assert_eq!(v[0].name_bytes,b"a.txt");assert_eq!(v[0].data,b"abc");
 }
}
