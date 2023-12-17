use std::{io::Result, path::Path};
fn main() -> Result<()> {
    let file = "protos/simple_message.proto";
    println!("cargo:rerun-if-changed={file}");
    let mut out_dir = std::env::var("OUT_DIR").unwrap();
    out_dir += "/simple.rs";

    tacky_build::write_proto(file, &out_dir);
    // let mut out_dir = std::env::var("OUT_DIR").unwrap();
    // out_dir += "/protos";

    // let cfg = pb_rs::ConfigBuilder::new(&["protos/simple_message.proto"], None, Some(&&out_dir.as_str()), &["."]).unwrap();
    // pb_rs::types::FileDescriptor::run(&cfg.build()).unwrap();
    // let cfg = cfg.dont_use_cow(true).build();
    // let mut out = Vec::new();
    // for cfg in cfg {
    //     let file = pb_rs::types::FileDescriptor::read_proto(&cfg.in_file, &cfg.import_search_path)
    //         .unwrap();
    //     out.push(file)
    // }
    prost_build::compile_protos(&[file], &["."])?;
    Ok(())
}
