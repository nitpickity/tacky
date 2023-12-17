fn main() {
    let file = "protos/simple_message.proto";
    let mut out_dir = std::env::var("OUT_DIR").unwrap();
    out_dir += "/simple.rs";

    println!("cargo:rerun-if-changed={file}");
    tacky_build::write_proto(file, &out_dir);
    prost_build::compile_protos(&[file], &["."]).unwrap();
}
