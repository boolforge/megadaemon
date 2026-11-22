use md_fusion_harness::re_fusion::{fusionar, leer_simbolos, OpcionesFusion};
use std::path::PathBuf;

fn main() {
    let o = OpcionesFusion {
        binario: PathBuf::from("/workspace/agy-re/liblanguage_server.so"),
        simbolos_tsv: Some(PathBuf::from("/workspace/agy-re/tools-out/go.funcs.tsv")),
        anchors_tsv: None,
        out_dir: PathBuf::from("/workspace/agy-re/tools-out/fusion"),
        ..Default::default()
    };
    let sims = leer_simbolos(o.simbolos_tsv.as_ref().unwrap());
    println!("simbolos leidos: {}", sims.len());
    let r = fusionar(&o, &[]);
    println!("{}", serde_json::to_string_pretty(&r).unwrap_or_default());
}
