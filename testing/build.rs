fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let tacky_dir = format!("{out_dir}/tacky");

    let simple_file = "protos/simple_message.proto";
    let importing_file = "protos/importing.proto";
    let proto3_file = "protos/proto3_message.proto";
    let pprof_file = "protos/pprof.proto";
    let accesslog_file = "protos/accesslog.proto";

    println!("cargo:rerun-if-changed={simple_file}");
    println!("cargo:rerun-if-changed={importing_file}");
    println!("cargo:rerun-if-changed={proto3_file}");
    println!("cargo:rerun-if-changed={pprof_file}");
    println!("cargo:rerun-if-changed={accesslog_file}");

    // Compile all tacky protos. Produces one .rs per package plus _includes.rs.
    tacky_build::compile_protos(
        &[
            simple_file,
            importing_file,
            proto3_file,
            pprof_file,
            accesslog_file,
        ],
        &tacky_dir,
        &["."],
    );

    prost_build::compile_protos(
        &[simple_file, proto3_file, pprof_file, accesslog_file],
        &["."],
    )
    .unwrap();
}
