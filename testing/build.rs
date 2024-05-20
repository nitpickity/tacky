fn main() {
    let simple_file = "protos/simple_message.proto";
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let simple_out = format!("{out_dir}/simple.rs");

    println!("cargo:rerun-if-changed={simple_file}");
    tacky_build::write_proto(simple_file, &simple_out);
    prost_build::compile_protos(&[simple_file], &["."]).unwrap();
}
