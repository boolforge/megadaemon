use crate::{Error,Result};
use crate::model::{Analysis,ParserInfo,Module,Function};
use crate::filetab::SourceResolver;
use crate::reader::Reader;

const MAGIC_GO120:u32=0xffff_fff1;
const MAGIC_GO118:u32=0xffff_fff0;
const MAX_FUNCS:u64=250_000;
const MAX_NAME:u64=16*1024;
const MAX_AUX_ENTRIES:u64=8_000_000;
const MAX_NAME_BYTES_TOTAL:u64=128*1024*1024;

#[derive(Debug,Clone)]
pub struct ParseOptions { pub module_index:u32, pub file_offset:u64, pub text_base:Option<u64>, pub max_functions:u64,
 /// De donde salio el text_base ("explicit", "estimated-from-container-sections", ...). MEDIDO 2026-11-22.
 pub text_base_source:Option<String> }
impl Default for ParseOptions { fn default()->Self{Self{module_index:0,file_offset:0,text_base:None,max_functions:MAX_FUNCS,text_base_source:None}} }

/// Paquete Go de un nombre de funcion.
///
/// Regla: el paquete es la parte anterior al primer '.' que aparece DETRAS
/// del ultimo '/'.  Asi `internal/abi.(*RegArgs).Dump` -> `internal/abi` y
/// `google3/third_party/jetski/gateway/gateway.ParseAdminSettings` ->
/// `google3/third_party/jetski/gateway/gateway`.
/// Los nombres "planos" (`main.main`, `runtime.main`) no tienen '/' y se
/// devuelve `None` (no hay paquete que deducir).
pub fn go_package(name:&str)->Option<String>{
 let slash=name.rfind('/')?;
 let dot=name[slash+1..].find('.').map(|i|slash+1+i)?;
 if dot==0{return None;}
 Some(name[..dot].to_owned())
}

/// Parse a Go 1.18+ (modern two-u32 functab) pclntab beginning at file offset.
/// This is intentionally a metadata parser, not a container parser. The caller
/// must supply the actual pclntab offset and, optionally, a verified text VA.
pub fn parse_go118_plus(data:&[u8], opt:ParseOptions)->Result<Analysis>{
 let r=Reader::new(data); let base=opt.file_offset;
 let magic=r.u32le(base)?;
 if magic!=MAGIC_GO118 && magic!=MAGIC_GO120 { return Err(Error::UnsupportedMagic(magic)); }
 let ptr=r.u8(base+7)?;
 if ptr!=4 && ptr!=8 {return Err(Error::Invalid("pcHeader ptrSize must be 4 or 8"));}
 let min_lc=r.u8(base+6)?;
 if min_lc==0 {return Err(Error::Invalid("pcHeader minLC is zero"));}
 if r.u8(base+4)?!=0 || r.u8(base+5)?!=0 {return Err(Error::Invalid("pcHeader pad bytes are not zero"));}
 let nfunc=r.uint(base+8,ptr,true)?;
 let nfiles=r.uint(base+8+ptr as u64,ptr,true)?;
 if nfiles>10_000_000 { return Err(Error::Limit("nfiles count")); }
 if nfunc==0 || nfunc>opt.max_functions {return Err(Error::Limit("nfunc outside configured bounds"));}
 let header_size=8u64.checked_add(8u64*(ptr as u64)).ok_or(Error::Overflow("header size"))?;
 r.bytes(base,header_size)?;
 // Six pointer-sized fields follow nfunc/nfiles: textStart, then
 // funcname/cutab/filetab/pctab/pcln offsets.
 //
 // MEDIDO 2026-11-22 (liblanguage_server.so de Antigravity, Go 1.28 RC,
 // layout 1.20, minLC=4): el primer campo NO es "reservado" como decia la
 // version anterior de este parser => es `textStart` (pcHeader.textStart del
 // runtime Go), que es la VA de inicio del texto.  Con el, los `entryoff`
 // de la functab se convierten en VAs absolutas y los simbolos ya se pueden
 // aplicar en Ghidra/Rizin.  Antes este parser los dejaba como offsets
 // relativos ("text_base_status: unresolved").
 let field=base+8+2*(ptr as u64);
 let text_start=r.uint(field,ptr,true)?;
 let funcname_off=r.uint(field+ptr as u64,ptr,true)?;
 // Base de texto efectiva: la que da el llamante o, si no, la del pcHeader.
 let effective_base=opt.text_base.or(if text_start>0 {Some(text_start)} else {None});
 let cutab_off=r.uint(field+2*(ptr as u64),ptr,true)?;
 let filetab_off=r.uint(field+3*(ptr as u64),ptr,true)?;
 let pctab_off=r.uint(field+4*(ptr as u64),ptr,true)?;
 let pcln_off=r.uint(field+5*(ptr as u64),ptr,true)?;
 let pcln_base=base.checked_add(pcln_off).ok_or(Error::Overflow("pcln base"))?;
 let funcname_base=base.checked_add(funcname_off).ok_or(Error::Overflow("funcname base"))?;
 // Resolutor de fichero:linea (cutab/filetab/pctab del pcHeader).
 // MEDIDO 2026-11-22: con este mecanismo se reproduce el fichero:linea de
 // la tabla de referencia en 161.680 de 161.682 funciones del binario real
 // (las 2 restantes no tienen fichero en la referencia tampoco).
 let resolver=SourceResolver::new(
  base.checked_add(cutab_off).ok_or(Error::Overflow("cutab base"))?,
  base.checked_add(filetab_off).ok_or(Error::Overflow("filetab base"))?,
  base.checked_add(pctab_off).ok_or(Error::Overflow("pctab base"))?,
  base.checked_add(pcln_off).ok_or(Error::Overflow("pcln base"))?,
  min_lc,
 );
 let table_bytes=(nfunc+1).checked_mul(8).ok_or(Error::Overflow("functab length"))?;
 r.bytes(pcln_base,table_bytes)?;
 let mut raw=Vec::with_capacity(nfunc as usize);
 for i in 0..nfunc {
   let at=pcln_base+i*8;
   let entry=r.u32le(at)?;
   let funcoff=r.u32le(at+4)?;
   raw.push((entry,funcoff));
 }
 let sentinel_at=pcln_base+nfunc*8;
 let sentinel=r.u32le(sentinel_at)?;
 let mut functions=Vec::with_capacity(nfunc as usize);
 let mut total_aux_entries=0u64;
 let mut total_name_bytes=0u64;
 for i in 0..nfunc as usize {
   let (entry,funcoff)=raw[i];
   let next=if i+1<raw.len(){raw[i+1].0}else{sentinel};
   if i>0 && entry<=raw[i-1].0 {return Err(Error::Invalid("functab entryoff not strictly increasing"));}
   if next<=entry {return Err(Error::Invalid("function extent is empty or reversed"));}
   let fbase=pcln_base.checked_add(funcoff as u64).ok_or(Error::Overflow("_func offset"))?;
   // Go runtime _func fixed prefix is 11*4 bytes; variable arrays follow.
   r.bytes(fbase,44)?;
   let nameoff=r.u32le(fbase+4)? as i32;
   let args_size=r.u32le(fbase+8)? as i32;
   let deferreturn=r.u32le(fbase+12)?;
   let pcsp=r.u32le(fbase+16)?;
   let pcfile=r.u32le(fbase+20)?;
   let pcln=r.u32le(fbase+24)?;
   let npcdata=r.u32le(fbase+28)?;
   let cu_offset=r.u32le(fbase+32)?;
   let start_line=r.u32le(fbase+36)? as i32;
   let func_id=r.u8(fbase+40)?;
   let flags=r.u8(fbase+41)?;
   let nfuncdata=r.u8(fbase+43)?;
   if npcdata > 4096 { return Err(Error::Limit("_func npcdata count")); }
   total_aux_entries=total_aux_entries.checked_add(npcdata as u64+nfuncdata as u64).ok_or(Error::Overflow("total _func auxiliary entries"))?;
   if total_aux_entries>MAX_AUX_ENTRIES{return Err(Error::Limit("total _func auxiliary entries"));}
   let pcdata_start=fbase+44;
   let pcdata_bytes=(npcdata as u64).checked_mul(4).ok_or(Error::Overflow("pcdata array size"))?;
   let funcdata_start=pcdata_start.checked_add(pcdata_bytes).ok_or(Error::Overflow("funcdata array start"))?;
   let funcdata_bytes=(nfuncdata as u64).checked_mul(4).ok_or(Error::Overflow("funcdata array size"))?;
   r.bytes(pcdata_start,pcdata_bytes)?; r.bytes(funcdata_start,funcdata_bytes)?;
   let mut pcdata_offsets=Vec::with_capacity(npcdata as usize);
   for j in 0..npcdata { pcdata_offsets.push(r.u32le(pcdata_start+(j as u64)*4)?); }
   let mut funcdata_offsets=Vec::with_capacity(nfuncdata as usize);
   for j in 0..nfuncdata { let v=r.u32le(funcdata_start+(j as u64)*4)?; funcdata_offsets.push(if v==u32::MAX {None}else{Some(v)}); }
   let mut name_status=if nameoff<0 {"unresolved"} else {"invalid_offset"};
   let name=if nameoff>=0 {
      let no=funcname_base.checked_add(nameoff as u64).ok_or(Error::Overflow("name offset"))?;
      match r.c_string(no,MAX_NAME) {
        Ok(b)=>{
          total_name_bytes=total_name_bytes.checked_add(b.len() as u64).ok_or(Error::Overflow("total function-name bytes"))?;
          if total_name_bytes>MAX_NAME_BYTES_TOTAL{return Err(Error::Limit("total function-name bytes"));}
          match std::str::from_utf8(b) { Ok(v)=>{name_status="observed";Some(v.to_owned())}, Err(_)=>{name_status="invalid_utf8";None} }
        },
        Err(_)=>{name_status="invalid_string_range";None}
      }
   } else {None};
   let start_va=effective_base.and_then(|b|b.checked_add(entry as u64)).map(|v|format!("0x{v:x}"));
   let end_va=effective_base.and_then(|b|b.checked_add(next as u64)).map(|v|format!("0x{v:x}"));
   let name_status=name_status.to_string();
   // Fichero y linea: se resuelven con cutab/filetab/pctab (ver filetab.rs).
   // `entry_va` es la PC de entrada de la funcion (lo que el runtime usa
   // como `f.entry()`); si la base de texto no esta resuelta, no hay linea.
   let entry_va=effective_base.and_then(|b|b.checked_add(entry as u64));
   let (source_file,source_line)=match entry_va {
    Some(ev)=>{
      let f=resolver.file(&r,cu_offset,pcfile,ev).unwrap_or(None);
      let l=resolver.line(&r,pcln,ev).unwrap_or(None);
      (f,l)
    },
    None=>(None,None),
   };
   let package=name.as_deref().and_then(go_package);
   functions.push(Function{index:i as u64,start_va,end_va,text_section_index:None,entryoff:entry,funcoff,nameoff,args_size,deferreturn,pcsp,pcfile,pcln,npcdata,cu_offset,start_line,func_id,flags,nfuncdata,pcdata_offsets,funcdata_offsets,observed_name:name,name_status,source_file,source_line,package,evidence:vec!["pclntab:pcHeader".into(),"pclntab:functab(entryoff,funcoff)".into(),"pclntab:_func.nameOff".into(),"pclntab:_func.variable_arrays".into(),"pclntab:_func.pcfile+pctab->cutab->filetab".into(),"pclntab:_func.pcln+pctab->line".into()]});
 }
 Ok(Analysis{schema_version:"1.2".into(),parser:ParserInfo{name:"go-retool".into(),version:env!("CARGO_PKG_VERSION").into(),pclntab_layout:"go1.18+ modern functab; Go1.20 magic family; V5 semantic coverage not implied".into()},modules:vec![Module{module_index:opt.module_index,module_identity:"offline-pclntab-module".into(),pclntab_file_offset:format!("0x{base:x}"),go_version:None,text_base_va:effective_base.map(|v|format!("0x{v:x}")),text_base_status:match (opt.text_base,effective_base){
   (Some(_),_)=>match opt.text_base_source{Some(src)=>format!("{src}; caller attests the mapping"),None=>"explicitly-supplied; caller must attest mapping".into()},
   // MEDIDO 2026-11-23: `moduledata.text` es la base REAL (verificada con
   // los invariantes minpc/maxpc); la estimacion por secciones NO lo es.
   (None,Some(v))=>format!("from pcHeader.textStart (0x{v:x}); Go runtime text base"),
   (None,None)=>"unresolved; relative offsets only".into(),
  },magic:format!("0x{magic:08x}"),pointer_size:ptr,min_instruction_size:min_lc,nfunc,functions,warnings:vec!["This parser does not locate pclntab in ELF/PE/Mach-O/Wasm.".into(),"textStart is read from pcHeader and used as the text base when the caller supplies none; for PIE/ET_DYN the value is a load-relative virtual address, not a runtime ASLR address".into(),"It does not parse moduledata/textsectmap, type descriptors, itabs, DWARF, or resolve pcdata/funcdata payloads; it exports only the raw _func offsets.".into(),"Go 1.27 V5 support must be validated against pinned official source and fixtures; magic acceptance is not full release compatibility.".into(),"A supplied text_base is valid only when the caller has resolved the applicable text mapping; secondary text sections/textsectmap are not handled here.".into()]}],warnings:vec!["Conservative parser; no container mapping or moduledata discovery.".into()]})
}

#[cfg(test)] mod tests {
 use super::*;
 fn fixture()->Vec<u8>{
  // Synthetic 64-bit header + name table + modern functab + _func records.
  let mut b=vec![0u8;512];
  b[0..4].copy_from_slice(&MAGIC_GO120.to_le_bytes()); b[6]=1;b[7]=8;
  b[8..16].copy_from_slice(&1u64.to_le_bytes()); b[16..24].copy_from_slice(&1u64.to_le_bytes());
  // Header 72; funcname at 72, pcln at 96.
  b[32..40].copy_from_slice(&72u64.to_le_bytes()); b[72..84].copy_from_slice(b"main.answer\0");
  b[64..72].copy_from_slice(&96u64.to_le_bytes());
  b[96..100].copy_from_slice(&0x20u32.to_le_bytes()); b[100..104].copy_from_slice(&24u32.to_le_bytes());
  b[104..108].copy_from_slice(&0x40u32.to_le_bytes()); // sentinel
  b[120..124].copy_from_slice(&0x20u32.to_le_bytes()); b[124..128].copy_from_slice(&0i32.to_le_bytes());
  b
 }
 #[test] fn parse_name_and_offsets(){let a=parse_go118_plus(&fixture(),ParseOptions::default()).unwrap();let f=&a.modules[0].functions[0];assert_eq!(f.observed_name.as_deref(),Some("main.answer"));assert_eq!(f.start_va,None);assert_eq!(f.end_va,None);}
 #[test] fn explicit_text_base_maps_va(){let a=parse_go118_plus(&fixture(),ParseOptions{text_base:Some(0x1000),..Default::default()}).unwrap();assert_eq!(a.modules[0].functions[0].start_va.as_deref(),Some("0x1020"));}
 #[test] fn parses_32bit_header(){
  let mut b=vec![0u8;256];b[0..4].copy_from_slice(&MAGIC_GO120.to_le_bytes());b[6]=1;b[7]=4;
  b[8..12].copy_from_slice(&1u32.to_le_bytes());b[12..16].copy_from_slice(&1u32.to_le_bytes());
  b[20..24].copy_from_slice(&40u32.to_le_bytes());b[36..40].copy_from_slice(&56u32.to_le_bytes());
  b[40..52].copy_from_slice(b"main.answer\0");
  b[56..60].copy_from_slice(&0x20u32.to_le_bytes());b[60..64].copy_from_slice(&24u32.to_le_bytes());b[64..68].copy_from_slice(&0x40u32.to_le_bytes());
  b[80..84].copy_from_slice(&0x20u32.to_le_bytes());
  let a=parse_go118_plus(&b,ParseOptions::default()).unwrap();assert_eq!(a.modules[0].pointer_size,4);assert_eq!(a.modules[0].functions[0].observed_name.as_deref(),Some("main.answer"));
 }
}
