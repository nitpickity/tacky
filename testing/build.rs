fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let simple_file = "protos/simple_message.proto";
    let importing_file = "protos/importing.proto";
    let simple_out = format!("{out_dir}/simple.rs");
    let importing_out = format!("{out_dir}/importing.rs");

    println!("cargo:rerun-if-changed={simple_file}");
    println!("cargo:rerun-if-changed={importing_file}");
    tacky_build::write_proto(simple_file, &simple_out);
    tacky_build::write_proto_with_includes(importing_file, &importing_out, &["."]);

    let proto3_file = "protos/proto3_message.proto";
    println!("cargo:rerun-if-changed={proto3_file}");
    tacky_build::write_proto(proto3_file, &format!("{out_dir}/proto3.rs"));

    prost_build::compile_protos(&[simple_file, proto3_file], &["."]).unwrap();
}
