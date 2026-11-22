//! Address domains are deliberately non-interchangeable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileOffset(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rva(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtualAddress(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GoTextOffset(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GoFuncOffset(pub u32);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GoNameOffset(pub i32);

impl FileOffset {
    pub fn checked_add(self, n: u64) -> Option<Self> { self.0.checked_add(n).map(Self) }
}
impl VirtualAddress {
    pub fn checked_add(self, n: u64) -> Option<Self> { self.0.checked_add(n).map(Self) }
}
/// A mapping is explicit because file-offset/RVA/VA translation depends on the
/// container (ELF PT_LOAD, PE sections, Mach-O segments, Wasm linear offsets).
#[derive(Debug, Clone, Copy)]
pub struct LinearMapping { pub file: FileOffset, pub va: VirtualAddress, pub size: u64 }
impl LinearMapping {
    pub fn file_to_va(self, off: FileOffset) -> Option<VirtualAddress> {
        let delta = off.0.checked_sub(self.file.0)?;
        if delta >= self.size { return None; }
        self.va.checked_add(delta)
    }
}
