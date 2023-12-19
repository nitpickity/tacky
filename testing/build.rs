fn main() {
    let simple_file = "protos/simple_message.proto";
    let useful_file = "protos/use_case.proto";
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let simple_out = format!("{out_dir}/simple.rs");
    let useful_out = format!("{out_dir}/useful.rs");

    println!("cargo:rerun-if-changed={simple_file}");
    println!("cargo:rerun-if-changed={useful_file}");
    tacky_build::write_proto(simple_file, &simple_out);
    tacky_build::write_proto(useful_file, &useful_out);
    prost_build::compile_protos(&[simple_file, useful_file], &["."]).unwrap();
}
