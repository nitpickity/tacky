fn main() {
    let simple_file = "protos/simple_message.proto";
    let importing_file = "protos/importing.proto";
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let simple_out = format!("{out_dir}/simple.rs");
    let importing_out = format!("{out_dir}/importing.rs");

    println!("cargo:rerun-if-changed={simple_file}");
    println!("cargo:rerun-if-changed={importing_file}");
    tacky_build::write_proto(simple_file, &simple_out);
    tacky_build::write_proto_with_includes(importing_file, &importing_out, &["."]);
    prost_build::compile_protos(&[simple_file], &["."]).unwrap();
}
