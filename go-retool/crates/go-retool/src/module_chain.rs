//! Generic cycle-safe walker for a version-specific moduledata.next reader.
//! The caller must decode each node using a pinned Go runtime layout.
use std::collections::HashSet;
use crate::{Error,Result};

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ModuleNodeRef{pub address:u64,pub next:Option<u64>}

pub fn walk_module_chain<F>(first:u64,max_nodes:usize,mut read:F)->Result<Vec<ModuleNodeRef>>
where F:FnMut(u64)->Result<Option<u64>>{
 let mut seen=HashSet::new();let mut out=Vec::new();let mut current=Some(first);
 while let Some(addr)=current{
  if out.len()>=max_nodes{return Err(Error::Limit("moduledata chain node count"));}
  if !seen.insert(addr){return Err(Error::Invalid("cycle in moduledata.next chain"));}
  let next=read(addr)?;
  out.push(ModuleNodeRef{address:addr,next});
  current=next;
 }
 Ok(out)
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn walks_chain(){let v=walk_module_chain(1,8,|a|Ok(match a{1=>Some(2),2=>Some(3),_=>None})).unwrap();assert_eq!(v.iter().map(|x|x.address).collect::<Vec<_>>(),vec![1,2,3]);}
 #[test]fn detects_cycle(){assert!(walk_module_chain(1,8,|a|Ok(Some(if a==1{2}else{1}))).is_err());}
 #[test]fn enforces_limit(){assert!(walk_module_chain(1,1,|_|Ok(Some(2))).is_err());}
}
