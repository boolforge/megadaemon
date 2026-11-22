use serde::{Serialize,Deserialize};

#[derive(Debug,Serialize,Deserialize)]
pub struct Analysis {
    pub schema_version: String,
    pub parser: ParserInfo,
    pub modules: Vec<Module>,
    pub warnings: Vec<String>,
}
#[derive(Debug,Serialize,Deserialize)]
pub struct ParserInfo { pub name:String, pub version:String, pub pclntab_layout:String }
#[derive(Debug,Serialize,Deserialize)]
pub struct Module {
    pub module_index:u32,
    pub module_identity:String,
    pub pclntab_file_offset:String,
    pub go_version:Option<String>,
    pub text_base_va:Option<String>,
    pub text_base_status:String,
    pub magic:String,
    pub pointer_size:u8,
    pub min_instruction_size:u8,
    pub nfunc:u64,
    pub functions:Vec<Function>,
    pub warnings:Vec<String>,
}
#[derive(Debug,Serialize,Deserialize)]
pub struct Function {
    pub index:u64,
    pub start_va:Option<String>,
    pub end_va:Option<String>,
    pub text_section_index:Option<u32>,
    pub entryoff:u32,
    pub funcoff:u32,
    pub nameoff:i32,
    pub args_size:i32,
    pub deferreturn:u32,
    pub pcsp:u32,
    pub pcfile:u32,
    pub pcln:u32,
    pub npcdata:u32,
    pub cu_offset:u32,
    pub start_line:i32,
    pub func_id:u8,
    pub flags:u8,
    pub nfuncdata:u8,
    pub pcdata_offsets:Vec<u32>,
    pub funcdata_offsets:Vec<Option<u32>>,
    pub observed_name:Option<String>,
    pub name_status:String,
    /// Fichero fuente (cutab/filetab del pcHeader). `None` si no se resuelve.
    pub source_file:Option<String>,
    /// Linea en la PC de entrada (tabla pcvalue de `_func.pcln`).
    pub source_line:Option<i32>,
    /// Paquete Go (todo lo anterior al ultimo '/' del nombre).
    pub package:Option<String>,
    pub evidence:Vec<String>,
}
