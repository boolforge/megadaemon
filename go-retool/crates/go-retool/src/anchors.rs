//! Anclajes ("anchors"): funciones hito que se decompilan a mano.
//!
//! QUE SON
//! Un anchor no es un simbolo cualquiera: es una *funcion hito* elegida a
//! mano para tener un punto de entrada legible al binario.  En el binario de
//! Antigravity (`liblanguage_server.so`, Go 1.28) el juego de anclajes cubre
//! tres familias:
//!
//! * `entry`    -> los dos puntos de entrada del programa (`runtime.main` y
//!   `main.main`).  Desde `main.main` se ve todo el arranque.
//! * `mcp`      -> el SDK MCP de Google (`modelcontextprotocol/go-sdk`), que
//!   es el transporte del language server (NewClient, Connect, discover,
//!   AddTool, callTool, ListTools...).  Es la columna vertebral del binario.
//! * `provider` -> los clientes de modelo (Gemini API, Google GenAI, los
//!   conversores de Anthropic y de OpenAI).  Son donde se ve como se
//!   construye la peticion al modelo.
//!
//! QUE SENALAN
//! Cada anchor senala un *punto de observacion* del binario: donde se
//! arranca, donde se habla MCP y donde se habla con el proveedor del modelo.
//! Un anchor NO es una funcion que el binario "llame de forma especial": es
//! una funcion cuyo cuerpo permite entender una parte del programa.  Por eso
//! el export de go-retool guarda, para cada anchor, la direccion REAL, el
//! tamano (extent) que da la pclntab, el fichero y la linea de origen, y una
//! descripcion legible.
//!
//! FORMATO EN DISCO (`anchors.tsv`)
//! ```text
//! 0x<va>\t<nombre_de_simbolo>\t<nombre_real_de_go>\t<fichero>:<linea>\t<etiqueta>
//! ```
//! La 2ª columna es el nombre "de simbolo" (el de la tabla `go.funcs.tsv`, con
//! los `/` y los parentesis sustituidos por `_`) y la 3ª el nombre real de Go
//! (`google3/third_party/.../mcp.NewClient`).  Este modulo lo lee, lo cruza
//! con el export de simbolos (para anadir paquete, fichero, linea y tamano) y
//! lo vuelve a escribir.

use crate::symbols::{Symbol,SymbolExport};
use crate::{Error,Result};
use serde::{Deserialize,Serialize};

/// Version del documento de anclajes.
pub const ANCHORS_SCHEMA_VERSION:&str="1.0";

/// Una funcion hito.
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub struct Anchor{
 /// Direccion virtual real (`0x...`).
 pub addr:String,
 /// Nombre real de Go (p.ej. `main.main`).
 pub name:String,
 /// Nombre de simbolo original (columna 2 de `anchors.tsv`).
 pub symbol:String,
 /// Paquete Go, si se deduce del nombre.
 pub package:Option<String>,
 /// Fichero fuente.
 pub file:Option<String>,
 /// Linea fuente.
 pub line:Option<i32>,
 /// Tamano en bytes (extent de la pclntab: `end_va - va`).
 pub size:Option<u64>,
 /// Etiqueta de la familia (`entry`, `mcp`, `provider`, ...).
 pub tag:String,
 /// Que senala este anclaje.
 pub description:String,
}

/// Documento de anclajes exportado.
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct AnchorExport{
 pub schema_version:String,
 pub generator:Generator,
 pub count:u64,
 /// Checksum FNV-1a 64 de las filas (mismo mecanismo que en los simbolos).
 pub rows_fnv1a64:String,
 pub anchors:Vec<Anchor>,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Generator{ pub name:String, pub version:String }

/// Familias de anclajes conocidas y su significado.
pub const TAG_DESCRIPTIONS:&[(&str,&str)]=&[
 ("entry","Punto de entrada del programa (runtime.main / main.main): aqui se ve el arranque completo."),
 ("mcp","Cliente/servidor del SDK MCP de Google: el transporte del language server."),
 ("provider","Cliente o conversor de proveedor de modelo (Gemini API, GenAI, Anthropic, OpenAI)."),
 ("model","Resolucion del catalogo de modelos o de la seleccion de modelo."),
 ("config","Lectura de ficheros de configuracion (admin_settings.json y similares)."),
];

/// Descripcion legible de una etiqueta.
pub fn describe_tag(tag:&str)->String{
 for (t,d) in TAG_DESCRIPTIONS{ if *t==tag { return d.to_string(); } }
 format!("Anclaje de la familia '{tag}'.")
}

/// Fila TSV de un anclaje (sin salto de linea).
pub fn tsv_row(a:&Anchor)->String{
 let loc=match (&a.file,a.line){
  (Some(f),Some(l))=>format!("{f}:{l}"),
  (Some(f),None)=>f.clone(),
  (None,Some(l))=>format!("?:{l}"),
  (None,None)=>"?".to_string(),
 };
 let size=a.size.map(|s|s.to_string()).unwrap_or_default();
 format!("{}\t{}\t{}\t{}\t{}\t{}",a.addr,a.name,a.symbol,loc,a.tag,size)
}

/// Documento TSV completo de anclajes.
pub fn to_tsv(exp:&AnchorExport)->String{
 let mut out=String::new();
 for a in &exp.anchors{ out.push_str(&tsv_row(a)); out.push('\n'); }
 out
}

/// Relee el TSV de anclajes exportado.
pub fn parse_tsv(text:&str)->Result<AnchorExport>{
 let mut anchors=Vec::new();
 for (n,line) in text.lines().enumerate(){
  if line.is_empty(){continue;}
  let p:Vec<&str>=line.split('\t').collect();
  if p.len()<5{
   return Err(Error::InvalidMsg(format!("linea TSV de anclajes {n}: se esperaban 5 campos, hay {}",p.len())));
  }
  let addr=crate::symbols::parse_hex(p[0])
   .ok_or_else(||Error::InvalidMsg(format!("linea TSV de anclajes {n}: direccion invalida '{}'",p[0])))?;
  let size=if p.len()>5&&!p[5].is_empty(){crate::symbols::parse_hex(p[5])}else{None};
  let (file,line_no)=match p[3].rsplit_once(':'){
   Some((f,l))=>match l.parse::<i32>(){Ok(v)=>(Some(f.to_string()),Some(v)),Err(_)=>(Some(p[3].to_string()),None)},
   None=>if p[3]=="?"||p[3].is_empty(){(None,None)}else{(Some(p[3].to_string()),None)},
  };
  // Columna 2 = nombre de simbolo (mangled); columna 3 = nombre real de Go.
  // Si la 3ª viene vacia, se usa la 2ª (caso de los nombres sin caracteres
  // especiales, como `runtime.main`, donde ambas coinciden).
  let symbol=p[1].to_string();
  let real=if p[2].is_empty(){symbol.clone()}else{p[2].to_string()};
  let package=crate::pclntab::go_package(&real);
  anchors.push(Anchor{
   addr:format!("0x{addr:x}"),
   name:real,
   symbol,
   package,
   file,
   line:line_no,
   size,
   tag:p[4].to_string(),
   description:describe_tag(p[4]),
  });
 }
 Ok(build_export(&anchors))
}

/// Construye el documento de export a partir de una lista de anclajes.
pub fn build_export(anchors:&[Anchor])->AnchorExport{
 let mut rows=Vec::new();
 for a in anchors{ rows.extend_from_slice(tsv_row(a).as_bytes()); }
 AnchorExport{
  schema_version:ANCHORS_SCHEMA_VERSION.to_string(),
  generator:Generator{name:"go-retool".to_string(),version:env!("CARGO_PKG_VERSION").to_string()},
  count:anchors.len() as u64,
  rows_fnv1a64:format!("{:016x}",crate::symbols::fnv1a64(&rows)),
  anchors:anchors.to_vec(),
 }
}

/// Cruza los anclajes con el export de simbolos: anade paquete, fichero,
/// linea y tamano (extent) que si estan en la pclntab.
pub fn enrich(anchors:&mut [Anchor],symbols:&SymbolExport){
 let by_addr:std::collections::HashMap<&str,&Symbol>=
  symbols.functions.iter().map(|s|(s.va.as_str(),s)).collect();
 for a in anchors.iter_mut(){
  if let Some(s)=by_addr.get(a.addr.as_str()){
   if a.package.is_none(){a.package=s.package.clone();}
   if a.file.is_none(){a.file=s.file.clone();}
   if a.line.is_none(){a.line=s.line;}
   if a.size.is_none(){a.size=s.size;}
   if a.name.is_empty(){a.name=s.name.clone();}
  }
 }
}

/// Anclajes por defecto: los "hitos" que siempre interestsan.
///
/// Se seleccionan por el nombre real de Go (no por direccion), de modo que el
/// mismo conjunto sirve para cualquier build del mismo producto.
pub fn select_default(symbols:&SymbolExport)->Vec<Anchor>{
 let mut out=Vec::new();
 for s in &symbols.functions{
  let name=s.name.as_str();
  let tag=match name{
   "runtime.main"=>Some("entry"),
   "main.main"=>Some("entry"),
   _=>None,
  };
  let Some(tag)=tag else {continue};
  out.push(anchor_from_symbol(s,tag));
 }
 out
}

/// Anclajes de las familias conocidas (MCP y proveedores) por patron de paquete.
pub fn select_by_pattern(symbols:&SymbolExport,tag:&str,needles:&[&str])->Vec<Anchor>{
 let mut out=Vec::new();
 for s in &symbols.functions{
  let name=s.name.as_str();
  if needles.iter().any(|n|name.contains(n)){
   out.push(anchor_from_symbol(s,tag));
  }
 }
 out
}

/// Construye un anclaje a partir de un simbolo exportado.
pub fn anchor_from_symbol(s:&Symbol,tag:&str)->Anchor{
 Anchor{
  addr:s.va.clone(),
  name:s.name.clone(),
  symbol:s.name.clone(),
  package:s.package.clone(),
  file:s.file.clone(),
  line:s.line,
  size:s.size,
  tag:tag.to_string(),
  description:describe_tag(tag),
 }
}

#[cfg(test)]
mod tests{
 use super::*;
 use crate::symbols::{Source,SymbolExport,Generator,Symbol};
 fn sym(va:&str,name:&str,size:u64)->Symbol{
  Symbol{va:va.to_string(),end_va:format!("0x{:x}",crate::symbols::parse_hex(va).unwrap_or(0)+size),
   name:name.to_string(),package:crate::pclntab::go_package(name),file:Some("a/b.go".into()),line:Some(7),
   size:Some(size),entryoff:0x1234,name_status:"observed".to_string()}
 }
 fn export(symbols:Vec<Symbol>)->SymbolExport{
  SymbolExport{schema_version:"1.0".into(),generator:Generator{name:"go-retool".into(),version:"0.1.0".into()},
   source:Source{file:"x".into(),pclntab_file_offset:"0x1".into(),text_base_va:Some("0x2".into()),text_base_status:"s".into(),nfunc:symbols.len() as u64,nfiles:None},
   count:symbols.len() as u64,rows_fnv1a64:String::new(),functions:symbols,anchors:Vec::new()}
 }
 #[test] fn tsv_ida_y_vuelta(){
  let mut a=Anchor{addr:"0x4d59d68".into(),name:"runtime.main".into(),symbol:"runtime.main".into(),package:None,
   file:Some("third_party/go/gc/src/runtime/proc.go".into()),line:Some(153),size:Some(0x1f0),tag:"entry".into(),description:"x".into()};
  a.package=crate::pclntab::go_package(&a.name);
  let exp=build_export(&[a.clone()]);
  let tsv=to_tsv(&exp);
  let back=parse_tsv(&tsv).expect("releer");
  assert_eq!(back.anchors.len(),1);
  assert_eq!(back.anchors[0].addr,a.addr);
  assert_eq!(back.anchors[0].name,a.name);
  assert_eq!(back.anchors[0].tag,a.tag);
  assert_eq!(back.anchors[0].line,a.line);
  assert_eq!(back.anchors[0].size,a.size);
 }
 #[test] fn tsv_incompleto_falla(){
  assert!(parse_tsv("0x1\truntime.main\n").is_err(),"faltan campos");
  assert!(parse_tsv("zz\truntime.main\truntime.main\ta.go:1\tentry\n").is_err(),"direccion invalida");
 }
 #[test] fn enrich_une_con_los_simbolos(){
  let s=sym("0x6409518","google3/third_party/jetski/models/geminimodels/geminimodels.Catalog",0xab0);
  let exp=export(vec![s]);
  let mut anchors=vec![Anchor{addr:"0x6409518".into(),name:"google3/third_party/jetski/models/geminimodels/geminimodels.Catalog".into(),
   symbol:"x".into(),package:None,file:None,line:None,size:None,tag:"model".into(),description:"x".into()}];
  enrich(&mut anchors,&exp);
  assert_eq!(anchors[0].size,Some(0xab0));
  assert_eq!(anchors[0].line,Some(7));
  assert_eq!(anchors[0].package.as_deref(),Some("google3/third_party/jetski/models/geminimodels/geminimodels"));
 }
 #[test] fn select_default_encuentra_los_entrantes(){
  let exp=export(vec![sym("0x1","runtime.main",0x10),sym("0x2","main.main",0x20),sym("0x3","runtime.gcMarkDone",0x30)]);
  let a=select_default(&exp);
  assert_eq!(a.len(),2);
  assert_eq!(a[0].name,"runtime.main");
  assert_eq!(a[1].tag,"entry");
 }
}
