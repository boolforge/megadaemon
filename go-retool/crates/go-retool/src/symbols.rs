//! Exportacion e importacion de simbolos Go en formatos que consumen
//! Ghidra y Rizin.
//!
//! FORMATOS
//! * JSON (`symbols.schema.json`): documento autocontenido con la cabecera
//!   (fichero de origen, offset de la pclntab, base de texto) y una fila por
//!   funcion con la direccion REAL, el nombre REAL de Go, el fichero y la
//!   linea.
//! * TSV: una linea por funcion con campos separados por tabulador:
//!   `0x<va>\t<nombre>\t<fichero>:<linea>\t0x<va_fin>\t<paquete>`.
//!   Es lo que consumen los scripts de Ghidra/Rizin (se lee linea a linea
//!   sin cargar 30 MB de JSON en memoria).
//! * Rizin: script de comandos `f` (flags) para `rizin -i`.
//!
//! ROUND-TRIP
//! `parse_tsv` y `parse_json` releen lo que escribe este modulo y detectan
//! la corrupcion (lineas truncadas, direcciones no hexadecimales, JSON
//! invalido, numero de filas que no cuadra con el checksum de filas).

use crate::model::Analysis;
use crate::{Error, Result};
use serde::{Deserialize,Serialize};

/// Version del documento de simbolos.
pub const SYMBOLS_SCHEMA_VERSION: &str = "1.0";

/// Documento de simbolos exportado.
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct SymbolExport {
    pub schema_version: String,
    pub generator: Generator,
    pub source: Source,
    pub count: u64,
    /// Checksum FNV-1a 64 de la concatenacion de las filas (orden de salida).
    pub rows_fnv1a64: String,
    pub functions: Vec<Symbol>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Generator {
    pub name: String,
    pub version: String,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Source {
    /// Ruta del binario analizado.
    pub file: String,
    /// Offset de fichero del pcHeader.
    pub pclntab_file_offset: String,
    /// Base de texto usada para convertir `entryoff` en VA.
    pub text_base_va: Option<String>,
    /// Como se obtuvo la base de texto.
    pub text_base_status: String,
    /// Numero de funciones declaradas por el pcHeader.
    pub nfunc: u64,
    /// Numero de ficheros declarados por el pcHeader.
    pub nfiles: Option<u64>,
}

/// Una funcion Go con su direccion real.
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub struct Symbol {
    /// Direccion virtual real (base de texto + entryoff), en hex.
    pub va: String,
    /// Direccion virtual final (base de texto + siguiente entryoff).
    pub end_va: String,
    /// Nombre REAL de Go (p.ej. `internal/abi.(*RegArgs).Dump`).
    pub name: String,
    /// Paquete Go (todo lo anterior al ultimo '/').
    pub package: Option<String>,
    /// Fichero fuente.
    pub file: Option<String>,
    /// Linea fuente.
    pub line: Option<i32>,
    /// Estado de la resolucion del nombre.
    pub name_status: String,
}

/// FNV-1a de 64 bits (checksum barato para el round-trip).
pub fn fnv1a64(data:&[u8])->u64{
 let mut h:u64=0xcbf2_9ce4_8422_2325;
 for b in data{ h^=*b as u64; h=h.wrapping_mul(0x100_0000_01b3); }
 h
}

/// Construye el documento de export a partir de un `Analysis`.
pub fn build_export(analysis:&Analysis, origin:&str)->SymbolExport{
 let module=analysis.modules.first();
 let mut functions=Vec::new();
 if let Some(m)=module{
  for f in &m.functions{
   let (Some(va),Some(name))=(f.start_va.clone(),f.observed_name.clone()) else {continue};
   functions.push(Symbol{
    va,
    end_va:f.end_va.clone().unwrap_or_default(),
    name,
    package:f.package.clone(),
    file:f.source_file.clone(),
    line:f.source_line,
    name_status:f.name_status.clone(),
   });
  }
 }
 let mut rows=Vec::new();
 for s in &functions{ rows.extend_from_slice(tsv_row(s).as_bytes()); }
 SymbolExport{
  schema_version:SYMBOLS_SCHEMA_VERSION.to_string(),
  generator:Generator{name:"go-retool".to_string(),version:env!("CARGO_PKG_VERSION").to_string()},
  source:Source{
   file:origin.to_string(),
   pclntab_file_offset:module.map(|m|m.pclntab_file_offset.clone()).unwrap_or_default(),
   text_base_va:module.and_then(|m|m.text_base_va.clone()),
   text_base_status:module.map(|m|m.text_base_status.clone()).unwrap_or_default(),
   nfunc:module.map(|m|m.nfunc).unwrap_or(0),
   nfiles:None,
  },
  count:functions.len() as u64,
  rows_fnv1a64:format!("{:016x}",fnv1a64(&rows)),
  functions,
 }
}

/// Fila TSV de un simbolo (sin salto de linea).
pub fn tsv_row(s:&Symbol)->String{
 let loc=match (&s.file,s.line){
  (Some(f),Some(l))=>format!("{f}:{l}"),
  (Some(f),None)=>f.clone(),
  (None,Some(l))=>format!("?:{l}"),
  (None,None)=>"?".to_string(),
 };
 format!("{}\t{}\t{}\t{}\t{}",s.va,s.name,loc,s.end_va,s.package.clone().unwrap_or_default())
}

/// Documento TSV completo.
pub fn to_tsv(exp:&SymbolExport)->String{
 let mut out=String::with_capacity(exp.functions.len()*96);
 for s in &exp.functions{ out.push_str(&tsv_row(s)); out.push('\n'); }
 out
}

/// Script de Rizin (`rizin -i script.riz`): un flag por funcion.
pub fn to_rizin_script(exp:&SymbolExport)->String{
 let mut out=String::new();
 out.push_str("# go-retool export - flags de Rizin\n");
 out.push_str("# uso: rizin -q -i simbolos.riz binario\n");
 for s in &exp.functions{
  let name=rizin_flag_name(&s.name);
  out.push_str(&format!("f {} {}\n",name,s.va));
 }
 out
}

/// Nombre de flag valido para Rizin: los `/`, `(`, `)`, `*` y `.` no valen.
pub fn rizin_flag_name(name:&str)->String{
 let mut out=String::with_capacity(name.len());
 for c in name.chars(){
  if c.is_ascii_alphanumeric()||c=='_'||c=='.'||c=='-' { out.push(c); }
  else { out.push('_'); }
 }
 if out.is_empty(){ out.push_str("go_unknown"); }
 out
}

/// Relee el TSV exportado. Detecta corrupcion.
pub fn parse_tsv(text:&str)->Result<SymbolExport>{
 let mut functions=Vec::new();
 for (n,line) in text.lines().enumerate(){
  if line.is_empty(){continue;}
  let p:Vec<&str>=line.split('\t').collect();
  if p.len()<4{ return Err(Error::InvalidMsg(format!("linea TSV {n}: se esperaban 4 campos, hay {}",p.len()))); }
  let va=parse_hex(p[0]).ok_or_else(||Error::InvalidMsg(format!("linea TSV {n}: direccion invalida '{}'",p[0])))?;
  let end_va=parse_hex(p[3]).ok_or_else(||Error::InvalidMsg(format!("linea TSV {n}: direccion final invalida '{}'",p[3])))?;
  let (file,line_no)=match p[2].rsplit_once(':'){
   Some((f,l))=>{
    let l=l.parse::<i32>().ok();
    match l{Some(v)=>(Some(f.to_string()),Some(v)),None=>(Some(p[2].to_string()),None)}
   },
   None=>{ if p[2]=="?"||p[2].is_empty(){(None,None)}else{(Some(p[2].to_string()),None)} },
  };
  let package=if p.len()>4&&!p[4].is_empty(){Some(p[4].to_string())}else{crate::pclntab::go_package(p[1])};
  functions.push(Symbol{va:format!("0x{va:x}"),end_va:format!("0x{end_va:x}"),name:p[1].to_string(),package,file,line:line_no,name_status:"observed".to_string()});
 }
 let mut rows=Vec::new();
 for s in &functions{ rows.extend_from_slice(tsv_row(s).as_bytes()); }
 Ok(SymbolExport{
  schema_version:SYMBOLS_SCHEMA_VERSION.to_string(),
  generator:Generator{name:"go-retool".to_string(),version:env!("CARGO_PKG_VERSION").to_string()},
  source:Source{file:String::new(),pclntab_file_offset:String::new(),text_base_va:None,text_base_status:String::new(),nfunc:functions.len() as u64,nfiles:None},
  count:functions.len() as u64,
  rows_fnv1a64:format!("{:016x}",fnv1a64(&rows)),
  functions,
 })
}

/// Relee el JSON exportado y verifica su checksum de filas.
pub fn parse_json(text:&str)->Result<SymbolExport>{
 let exp:SymbolExport=serde_json::from_str(text).map_err(|e|Error::InvalidMsg(format!("JSON invalido: {e}")))?;
 if exp.schema_version!=SYMBOLS_SCHEMA_VERSION{
  return Err(Error::InvalidMsg(format!("schema_version inesperado: {}",exp.schema_version)));
 }
 if exp.count!=exp.functions.len() as u64{
  return Err(Error::InvalidMsg(format!("count {} no cuadra con {} funciones",exp.count,exp.functions.len())));
 }
 let mut rows=Vec::new();
 for s in &exp.functions{ rows.extend_from_slice(tsv_row(s).as_bytes()); }
 let checksum=format!("{:016x}",fnv1a64(&rows));
 if checksum!=exp.rows_fnv1a64{
  return Err(Error::InvalidMsg(format!("checksum de filas corrupto: {} != {}",checksum,exp.rows_fnv1a64)));
 }
 Ok(exp)
}

/// Parsea `0x...` o decimal en u64.
pub fn parse_hex(s:&str)->Option<u64>{
 let t=s.trim();
 if let Some(h)=t.strip_prefix("0x").or_else(||t.strip_prefix("0X")){
  u64::from_str_radix(h,16).ok()
 } else {
  t.parse::<u64>().ok()
 }
}

#[cfg(test)]
mod tests {
 use super::*;
 fn sym(va:&str,name:&str,file:Option<&str>,line:Option<i32>)->Symbol{
  Symbol{va:va.to_string(),end_va:format!("0x{:x}",parse_hex(va).unwrap_or(0)+0x10),name:name.to_string(),package:crate::pclntab::go_package(name),file:file.map(|s|s.to_string()),line,name_status:"observed".to_string()}
 }
 #[test] fn round_trip_tsv(){
  let exp=SymbolExport{
   schema_version:SYMBOLS_SCHEMA_VERSION.to_string(),
   generator:Generator{name:"go-retool".to_string(),version:"0.1.0".to_string()},
   source:Source{file:"x".into(),pclntab_file_offset:"0x1".into(),text_base_va:Some("0x2".into()),text_base_status:"s".into(),nfunc:2,nfiles:None},
   count:2,
   rows_fnv1a64:String::new(),
   functions:vec![sym("0x4d06cb8","internal/abi.(*RegArgs).Dump",Some("a/b.go"),Some(47)),sym("0x4d06ea8","runtime.main",None,None)],
  };
  let mut e=exp.clone();
  let mut rows=Vec::new();
  for s in &e.functions{rows.extend_from_slice(tsv_row(s).as_bytes());}
  e.rows_fnv1a64=format!("{:016x}",fnv1a64(&rows));
  let tsv=to_tsv(&e);
  let back=parse_tsv(&tsv).expect("releer el TSV");
  assert_eq!(back.functions.len(),2);
  assert_eq!(back.functions[0].name,"internal/abi.(*RegArgs).Dump");
  assert_eq!(back.functions[0].file.as_deref(),Some("a/b.go"));
  assert_eq!(back.functions[0].line,Some(47));
  assert_eq!(back.functions[0].package.as_deref(),Some("internal/abi"));
  assert_eq!(back.functions[1].name,"runtime.main");
  assert_eq!(back.functions[1].file,None);
  assert_eq!(back.rows_fnv1a64,e.rows_fnv1a64);
 }
 #[test] fn round_trip_json(){
  let mut e=SymbolExport{
   schema_version:SYMBOLS_SCHEMA_VERSION.to_string(),
   generator:Generator{name:"go-retool".to_string(),version:"0.1.0".to_string()},
   source:Source{file:"x".into(),pclntab_file_offset:"0x1".into(),text_base_va:Some("0x2".into()),text_base_status:"s".into(),nfunc:1,nfiles:None},
   count:1,
   rows_fnv1a64:String::new(),
   functions:vec![sym("0x4d06cb8","main.main",Some("main.go"),Some(3))],
  };
  let mut rows=Vec::new();
  for s in &e.functions{rows.extend_from_slice(tsv_row(s).as_bytes());}
  e.rows_fnv1a64=format!("{:016x}",fnv1a64(&rows));
  let json=serde_json::to_string(&e).unwrap();
  let back=parse_json(&json).expect("releer el JSON");
  assert_eq!(back.functions, e.functions);
 }
 #[test] fn detecta_tsv_corrupto(){
  let tsv="0x4d06cb8\tmain.main\n";
  assert!(parse_tsv(tsv).is_err(),"una linea con 2 campos debe fallar");
  let tsv="zzz\tmain.main\ta:1\t0x10\t\n";
  assert!(parse_tsv(tsv).is_err(),"una direccion no hex debe fallar");
 }
 #[test] fn detecta_json_corrupto(){
  let mut e=SymbolExport{
   schema_version:SYMBOLS_SCHEMA_VERSION.to_string(),
   generator:Generator{name:"go-retool".to_string(),version:"0.1.0".to_string()},
   source:Source{file:"x".into(),pclntab_file_offset:"0x1".into(),text_base_va:None,text_base_status:"s".into(),nfunc:1,nfiles:None},
   count:1,
   rows_fnv1a64:"deadbeefdeadbeef".to_string(),
   functions:vec![sym("0x4d06cb8","main.main",None,None)],
  };
  let json=serde_json::to_string(&e).unwrap();
  assert!(parse_json(&json).is_err(),"checksum de filas incorrecto debe fallar");
  e.rows_fnv1a64="0".repeat(16);
  let mut rows=Vec::new();
  for s in &e.functions{rows.extend_from_slice(tsv_row(s).as_bytes());}
  e.rows_fnv1a64=format!("{:016x}",fnv1a64(&rows));
  let json=serde_json::to_string(&e).unwrap();
  assert!(parse_json(&json).is_ok());
  let truncated=&json[..json.len()/2];
  assert!(parse_json(truncated).is_err(),"JSON truncado debe fallar");
 }
 #[test] fn nombres_rizin_sin_caracteres_ilegales(){
  assert_eq!(rizin_flag_name("internal/abi.(*RegArgs).Dump"),"internal_abi.__RegArgs_.Dump");
 }
}
