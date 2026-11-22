use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Truncated { at: u64, need: u64, len: u64 },
    Overflow(&'static str),
    Invalid(&'static str),
    /// Invalid con mensaje dinamico (formateado en el momento).
    InvalidMsg(String),
    UnsupportedMagic(u32),
    InvalidUtf8,
    Limit(&'static str),
}
pub type Result<T> = std::result::Result<T, Error>;
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated{at,need,len} => write!(f,"truncated input: at={at:#x}, need={need}, len={len}"),
            Error::Overflow(s) => write!(f,"integer overflow: {s}"),
            Error::Invalid(s) => write!(f,"invalid data: {s}"),
            Error::InvalidMsg(s) => write!(f,"invalid data: {s}"),
            Error::UnsupportedMagic(m) => write!(f,"unsupported pclntab magic {m:#010x}"),
            Error::InvalidUtf8 => write!(f,"invalid UTF-8 in function name"),
            Error::Limit(s) => write!(f,"safety limit exceeded: {s}"),
        }
    }
}
impl std::error::Error for Error {}
