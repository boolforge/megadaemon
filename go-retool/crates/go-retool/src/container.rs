//! Read-only container inspection and conservative modern pclntab discovery.
//! `object` handles ELF/PE/Mach-O and other supported object formats; this
//! module never assumes that a section VA is the Go module text base.
use serde::Serialize;
use object::{Object,ObjectSection,SectionKind};
use crate::{Error,Result};
use crate::pclntab::{parse_go118_plus,ParseOptions};

#[derive(Debug,Serialize)]
pub struct SectionInfo{
 pub name:String,pub kind:String,pub section_address:String,pub address_domain:String,pub size:u64,
 pub file_offset:Option<String>,pub file_size:Option<u64>,
}
#[derive(Debug,Serialize)]
pub struct PclnCandidate{
 pub file_offset:String,pub magic:String,pub pointer_size:u8,pub min_lc:u8,
 pub nfunc:u64,pub names_resolved:u64,pub section:Option<String>,
 pub section_address:Option<String>,pub address_domain:String,pub address_status:String,
}
#[derive(Debug,Serialize)]
pub struct ContainerReport{
 pub format:String,pub architecture:String,pub endianness:String,
 pub is_64:bool,pub sections:Vec<SectionInfo>,pub pclntab_candidates:Vec<PclnCandidate>,
 /// Base de texto del modulo Go.
 ///
 /// MEDIDO 2026-11-23 (liblanguage_server.so de Antigravity, Go 1.28 RC02,
 /// layout 1.20, minLC=4): el `pcHeader.textStart` de una BIBLIOTECA
 /// compartida vale 0 (el linker no puede conocer la VA final), asi que la
 /// base hay que sacarla de `moduledata.text`, que el propio binario lleva
 /// en la seccion `.go.module` (ver `crate::moduledata`).
 ///
 /// El valor real es **0x4d0f0b0**.  La estimacion por secciones (la menor
 /// direccion ".text*") daba 0x4d06cb8 = `.text.hot`, una seccion de TAMANO
 /// 0 => **error de 0x83F8 bytes en todas las direcciones**.
 ///
 /// `text_base_source` dice de donde sale el valor; si dice "estimated" no
 /// esta verificado y no se debe confiar en el.
 pub text_base_hint:Option<String>,
 /// De donde sale `text_base_hint` (ver `crate::moduledata`).
 pub text_base_source:Option<String>,
 pub warnings:Vec<String>,
}

/// ESTIMA la base de texto de un modulo Go a partir de las secciones.
///
/// MEDIDO 2026-11-23 en el binario real de Antigravity: esta estimacion es
/// **INCORRECTA** (da 0x4d06cb8 en vez de 0x4d0f0b0) porque `.text.hot` es
/// una seccion de TAMANO 0 que va antes que `.text`.  Se conserva SOLO como
/// respaldo cuando el binario no tiene `.go.module`; el valor bueno sale de
/// `crate::moduledata::read_moduledata`.
///
/// AVISO: dos implementaciones que compartan esta heuristica CONCuerdan
/// entre si aunque ambas esten equivocadas: comparar "go-retool" con
/// "unstrip" asi NO valida nada.  La validacion real es
/// `base+entryoff[0]==moduledata.minpc` y `base+sentinel==moduledata.maxpc`.
pub fn text_base_hint(sections:&[SectionInfo])->Option<u64>{
 let mut best:Option<u64>=None;
 for s in sections{
  if !s.name.starts_with(".text"){continue;}
  let addr=s.section_address.strip_prefix("0x").and_then(|h|u64::from_str_radix(h,16).ok())?;
  best=Some(match best{Some(b) if b<=addr=>b,_=>addr});
 }
 best
}
#[derive(Debug,Clone)]
struct Mapping{start:u64,size:u64,address:u64,name:String}

pub fn scan_container(data:&[u8])->Result<ContainerReport>{
 let file=object::File::parse(data).map_err(|_|Error::Invalid("unsupported or malformed executable container"))?;
 let format_name=format!("{:?}",file.format());
 let address_domain=match format_name.as_str(){"Pe"=>"rva","Elf"|"MachO"=>"virtual_address",_=>"format_specific"}.to_string();
 let mut sections=Vec::new();let mut maps=Vec::new();
 for sec in file.sections(){
  let name=sec.name().unwrap_or("<unnamed>").to_string();
  let kind=match sec.kind(){SectionKind::Text=>"text",SectionKind::Data=>"data",SectionKind::ReadOnlyData=>"rodata",SectionKind::ReadOnlyString=>"rostring",SectionKind::UninitializedData=>"bss",_=>"other"}.to_string();
  let range=sec.file_range();
  let (file_offset,file_size)=match range{Some((o,n))=>{maps.push(Mapping{start:o,size:n,address:sec.address(),name:name.clone()});(Some(format!("0x{o:x}")),Some(n))},None=>(None,None)};
  sections.push(SectionInfo{name,kind,section_address:format!("0x{:x}",sec.address()),address_domain:address_domain.clone(),size:sec.size(),file_offset,file_size});
 }
 let mut candidates=Vec::new();
 let mut attempts=0usize;
 for (off,window) in data.windows(4).enumerate(){
  let magic=u32::from_le_bytes(window.try_into().unwrap());
  if magic!=0xffff_fff0 && magic!=0xffff_fff1 {continue;}
  attempts+=1;
  if attempts>512 || candidates.len()>=16 {break;}
  let opts=ParseOptions{file_offset:off as u64,max_functions:250_000,..Default::default()};
  let parsed=match parse_go118_plus(data,opts){Ok(v)=>v,Err(_)=>continue};
  let module=&parsed.modules[0];
  let names=module.functions.iter().filter(|f|f.observed_name.is_some()).count() as u64;
  let mapping=maps.iter().find(|m|(off as u64)>=m.start&&(off as u64)<m.start.saturating_add(m.size));
  let section=mapping.map(|m|m.name.clone());
  let section_address=mapping.and_then(|m|m.address.checked_add(off as u64-m.start)).map(|v|format!("0x{v:x}"));
  candidates.push(PclnCandidate{file_offset:format!("0x{off:x}"),magic:format!("0x{magic:08x}"),pointer_size:module.pointer_size,min_lc:module.min_instruction_size,nfunc:module.nfunc,names_resolved:names,section,section_address,address_domain:address_domain.clone(),address_status:"container section address only; Go text base unresolved".into()});
 }
 // Base de texto: PRIMERO el `moduledata` del propio binario (medido y
 // verificado con los invariantes minpc/maxpc); si no esta, la estimacion
 // por secciones, que NO esta verificada (medido: falla en 0x83F8 bytes).
 let md=crate::moduledata::read_moduledata(data);
 let (tbh,tbs)=match md.as_ref(){
  Some(m)=>(Some(m.text),Some(m.source.clone())),
  None=>(text_base_hint(&sections),Some("estimated-from-container-sections (moduledata.text NOT found; NOT verified)".to_string())),
 };
 let mut warnings=vec!["Only little-endian Go 1.18/1.20 magic-family pclntab candidates are parsed by this MVP.".into(),"Section address is the address of the pclntab bytes in the container section (format-specific, e.g. PE RVA), not moduledata.text; no function VA is derived from it.".into(),"Container section mappings do not resolve relocations, ASLR, secondary text sections, or runtime-loaded plugins.".into()];
 if md.is_none(){warnings.push("moduledata.text NOT found: the text base is an ESTIMATION from section addresses and is NOT verified; the function VAs it produces can be wrong (measured: 0x83F8 bytes off on the Antigravity binary)".into());}
 Ok(ContainerReport{format:format_name,text_base_hint:tbh.map(|v|format!("0x{v:x}")),text_base_source:tbs,architecture:format!("{:?}",file.architecture()),endianness:if file.is_little_endian(){"little"}else{"big"}.into(),is_64:file.is_64(),sections,pclntab_candidates:candidates,warnings})
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn rejects_random_bytes(){assert!(scan_container(b"not an executable").is_err());}
}
