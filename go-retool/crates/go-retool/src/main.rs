use std::{env,fs,process};
use go_retool::{parse_go118_plus, pclntab::ParseOptions, container::scan_container};
use go_retool::sha256::sha256_hex;
use go_retool::symbols::{build_export,to_rizin_script,to_tsv};
fn usage(){eprintln!("Usage:\n  go-retool scan <executable> [--out report.json]\n  go-retool pclntab <file> --offset <hex-or-decimal> [--text-base <hex-or-decimal>] [--module-index N] [--out result.json]\n  go-retool export <file> --offset <hex-or-decimal> [--text-base <hex-or-decimal>] [--json out.json] [--tsv out.tsv] [--rizin out.rizin]");}
fn number(s:&str)->Result<u64,String>{let t=s.trim();if let Some(x)=t.strip_prefix("0x"){u64::from_str_radix(x,16).map_err(|e|e.to_string())}else{t.parse().map_err(|e:std::num::ParseIntError|e.to_string())}}
fn output<T:serde::Serialize>(value:&T,path:Option<String>)->Result<(),String>{let json=serde_json::to_string_pretty(value).map_err(|e|e.to_string())?;if let Some(p)=path{fs::write(&p,json).map_err(|e|format!("write {p}: {e}"))?;}else{println!("{json}");}Ok(())}
fn run()->Result<(),String>{
 let a:Vec<String>=env::args().skip(1).collect();if a.len()<2{usage();return Err("missing command or input".into());}
 let command=&a[0];let path=a[1].clone();let mut out=None;
 if command=="export"{return run_export(&a);}
 if command=="scan"{
  let mut i=2;while i<a.len(){match a[i].as_str(){"--out"=>{i+=1;out=Some(a.get(i).ok_or("missing --out path")?.clone());},x=>return Err(format!("unknown scan option {x}"))}i+=1;}
  let bytes=fs::read(&path).map_err(|e|format!("read {path}: {e}"))?;
  let report=scan_container(&bytes).map_err(|e|e.to_string())?;return output(&report,out);
 }
 if command!="pclntab"{usage();return Err(format!("unknown command {command}"));}
 let mut offset=None;let mut text_base=None;let mut module_index=0u32;let mut i=2;
 while i<a.len(){match a[i].as_str(){"--offset"=>{i+=1;offset=Some(number(a.get(i).ok_or("missing --offset value")?)?);},"--text-base"=>{i+=1;text_base=Some(number(a.get(i).ok_or("missing --text-base value")?)?);},"--module-index"=>{i+=1;module_index=a.get(i).ok_or("missing module index")?.parse().map_err(|e|format!("bad module index: {e}"))?;},"--out"=>{i+=1;out=Some(a.get(i).ok_or("missing output path")?.clone());},x=>return Err(format!("unknown option {x}"))}i+=1;}
 let bytes=fs::read(&path).map_err(|e|format!("read {path}: {e}"))?;
 // Si el llamante NO da --text-base, se estima desde las secciones del
 // contenedor (ver container::text_base_hint).  Es una estimacion: el
 // valor definitivo sale de moduledata.text, que este crate no localiza.
 let (text_base,text_base_source)=match text_base{
  Some(v)=>(Some(v),Some("explicitly-supplied")),
  None=>{let rep=scan_container(&bytes).map_err(|e|e.to_string())?;
        let h=rep.text_base_hint.and_then(|h|h.strip_prefix("0x").and_then(|x|u64::from_str_radix(x,16).ok()));
        (h,h.map(|_|"estimated-from-container-sections (runtime.text NOT verified)"))}
 };
 let opt=ParseOptions{module_index,file_offset:offset.ok_or("--offset is required")?,text_base,text_base_source,..Default::default()};
 let result=parse_go118_plus(&bytes,opt).map_err(|e|e.to_string())?;output(&result,out)
}

/// `export`: escribe los simbolos en JSON + TSV (+ script de Rizin).
///
/// Es el comando que usan Ghidra y Rizin: la direccion es la REAL (base de
/// texto + entryoff) y el nombre es el REAL de Go (no el mangled).
fn run_export(a:&[String])->Result<(),String>{
 let path=a[1].clone();
 let mut offset=None;let mut text_base=None;let mut json_out=None;let mut tsv_out=None;let mut rizin_out=None;
 let mut i=2;
 while i<a.len(){match a[i].as_str(){
  "--offset"=>{i+=1;offset=Some(number(a.get(i).ok_or("missing --offset value")?)?);},
  "--text-base"=>{i+=1;text_base=Some(number(a.get(i).ok_or("missing --text-base value")?)?);},
  "--json"=>{i+=1;json_out=Some(a.get(i).ok_or("missing --json path")?.clone());},
  "--tsv"=>{i+=1;tsv_out=Some(a.get(i).ok_or("missing --tsv path")?.clone());},
  "--rizin"=>{i+=1;rizin_out=Some(a.get(i).ok_or("missing --rizin path")?.clone());},
  x=>return Err(format!("unknown option {x}")),
 }i+=1;}
 let bytes=fs::read(&path).map_err(|e|format!("read {path}: {e}"))?;
 let (text_base,text_base_source)=match text_base{
  Some(v)=>(Some(v),Some("explicitly-supplied")),
  None=>{let rep=scan_container(&bytes).map_err(|e|e.to_string())?;
        let h=rep.text_base_hint.and_then(|h|h.strip_prefix("0x").and_then(|x|u64::from_str_radix(x,16).ok()));
        (h,h.map(|_|"estimated-from-container-sections (runtime.text NOT verified)"))}
 };
 let opt=ParseOptions{module_index:0,file_offset:offset.ok_or("--offset is required")?,text_base,text_base_source,..Default::default()};
 let analysis=parse_go118_plus(&bytes,opt).map_err(|e|e.to_string())?;
 let export=build_export(&analysis,&path);
 if json_out.is_none()&&tsv_out.is_none()&&rizin_out.is_none(){return Err("indica al menos --json, --tsv o --rizin".into());}
 if let Some(p)=&json_out{
  let text=serde_json::to_string(&export).map_err(|e|e.to_string())?;
  fs::write(p,text.as_bytes()).map_err(|e|format!("write {p}: {e}"))?;
  println!("json  {p} funciones={} bytes={} sha256={}",export.count,text.len(),sha256_hex(text.as_bytes()));
 }
 if let Some(p)=&tsv_out{
  let text=to_tsv(&export);
  fs::write(p,text.as_bytes()).map_err(|e|format!("write {p}: {e}"))?;
  println!("tsv   {p} funciones={} bytes={} sha256={}",export.count,text.len(),sha256_hex(text.as_bytes()));
 }
 if let Some(p)=&rizin_out{
  let text=to_rizin_script(&export);
  fs::write(p,text.as_bytes()).map_err(|e|format!("write {p}: {e}"))?;
  println!("rizin {p} funciones={} bytes={} sha256={}",export.count,text.len(),sha256_hex(text.as_bytes()));
 }
 let con_fichero=export.functions.iter().filter(|f|f.file.is_some()).count();
 let con_linea=export.functions.iter().filter(|f|f.line.is_some()).count();
 println!("resumen funciones={} con_fichero={} con_linea={} base_texto={:?} checksum_filas={}",
  export.count,con_fichero,con_linea,export.source.text_base_va,export.rows_fnv1a64);
 Ok(())
}
fn main(){if let Err(e)=run(){eprintln!("error: {e}");process::exit(2);}}
