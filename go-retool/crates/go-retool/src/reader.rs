use crate::{Error, Result};

#[derive(Clone, Copy)]
pub struct Reader<'a> { data: &'a [u8] }
impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self { Self { data } }
    pub fn len(&self) -> u64 { self.data.len() as u64 }
    pub fn bytes(&self, off: u64, n: u64) -> Result<&'a [u8]> {
        let end = off.checked_add(n).ok_or(Error::Overflow("slice end"))?;
        if end > self.len() { return Err(Error::Truncated { at: off, need: n, len: self.len() }); }
        let a = usize::try_from(off).map_err(|_| Error::Overflow("offset usize"))?;
        let b = usize::try_from(end).map_err(|_| Error::Overflow("end usize"))?;
        Ok(&self.data[a..b])
    }
    pub fn u8(&self, o:u64)->Result<u8>{Ok(self.bytes(o,1)?[0])}
    pub fn u32le(&self,o:u64)->Result<u32>{Ok(u32::from_le_bytes(self.bytes(o,4)?.try_into().unwrap()))}
    pub fn u32be(&self,o:u64)->Result<u32>{Ok(u32::from_be_bytes(self.bytes(o,4)?.try_into().unwrap()))}
    pub fn uint(&self,o:u64,width:u8,little:bool)->Result<u64>{
        match (width,little) {
            (4,true)=>Ok(self.u32le(o)? as u64), (4,false)=>Ok(self.u32be(o)? as u64),
            (8,true)=>Ok(u64::from_le_bytes(self.bytes(o,8)?.try_into().unwrap())),
            (8,false)=>Ok(u64::from_be_bytes(self.bytes(o,8)?.try_into().unwrap())),
            _=>Err(Error::Invalid("pointer width must be 4 or 8")),
        }
    }
    pub fn c_string(&self, off:u64, max:u64)->Result<&'a [u8]> {
        let n = max.min(self.len().saturating_sub(off));
        let s = self.bytes(off,n)?;
        let end = s.iter().position(|b| *b==0).ok_or(Error::Invalid("unterminated string"))?;
        Ok(&s[..end])
    }
}
