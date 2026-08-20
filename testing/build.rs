/// The from-source C++ prefix, if one exists. `TACKY_PROTOBUF_PREFIX` wins, else the
/// repo-local path `scripts/build_cpp_static.sh` installs to. Tests for the lib rather than
/// the directory: a half-built tree isn't usable.
fn cpp_prefix() -> Option<String> {
    if let Ok(p) = std::env::var("TACKY_PROTOBUF_PREFIX") {
        return Some(p);
    }
    let root = std::env::var("TACKY_CPP_ROOT").unwrap_or_else(|_| {
        // CARGO_MANIFEST_DIR is `testing/`.
        format!(
            "{}/../third_party/protobuf-cpp",
            std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default()
        )
    });
    let p = format!("{root}/prefix");
    std::path::Path::new(&p)
        .join("lib/libprotobuf.a")
        .exists()
        .then_some(p)
}

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();

    // prost-build shells out to protoc, and distro packages run years behind — Fedora still
    // ships 3.19. If the C++ prefix has been built, reuse its protoc so the system one stops
    // mattering for both arms.
    println!("cargo:rerun-if-env-changed=PROTOC");
    if std::env::var_os("PROTOC").is_none() {
        if let Some(prefix) = cpp_prefix() {
            let protoc = std::path::Path::new(&prefix).join("bin/protoc");
            if protoc.exists() {
                std::env::set_var("PROTOC", protoc);
            }
        }
    }
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

    let pprof_file = "protos/pprof.proto";
    println!("cargo:rerun-if-changed={pprof_file}");
    tacky_build::write_proto(pprof_file, &format!("{out_dir}/pprof.rs"));

    let accesslog_file = "protos/accesslog.proto";
    println!("cargo:rerun-if-changed={accesslog_file}");
    tacky_build::write_proto(accesslog_file, &format!("{out_dir}/tacky_accesslog.rs"));

    // GoogleMessage1: protobuf's own benchmark corpus, vendored from tag v3.20.3.
    // `benchmarks.proto` is the `BenchmarkDataset` wrapper the payload files use;
    // only prost needs it, to unwrap them at bench setup.
    let m1p2_file = "protos/benchmark_message1_proto2.proto";
    let m1p3_file = "protos/benchmark_message1_proto3.proto";
    let dataset_file = "protos/benchmarks.proto";
    println!("cargo:rerun-if-changed={m1p2_file}");
    println!("cargo:rerun-if-changed={m1p3_file}");
    println!("cargo:rerun-if-changed={dataset_file}");
    tacky_build::write_proto(
        &strip_custom_defaults(m1p2_file, &out_dir),
        &format!("{out_dir}/tacky_message1_proto2.rs"),
    );
    tacky_build::write_proto(m1p3_file, &format!("{out_dir}/tacky_message1_proto3.rs"));

    // descriptor.proto, for `benches/descriptor_set.rs`. Only tacky needs codegen
    // here — prost's side of that bench comes from prost-types.
    let descriptor_file = "protos/descriptor.proto";
    println!("cargo:rerun-if-changed={descriptor_file}");
    tacky_build::write_proto(
        &strip_custom_defaults(descriptor_file, &out_dir),
        &format!("{out_dir}/tacky_descriptor.rs"),
    );

    prost_build::compile_protos(
        &[
            simple_file,
            proto3_file,
            pprof_file,
            accesslog_file,
            m1p2_file,
            m1p3_file,
            dataset_file,
        ],
        &["."],
    )
    .unwrap();

    // OTLP traces, for `benches/otlp_traces.rs`. Vendored from
    // open-telemetry/opentelemetry-proto tag v1.3.2, keeping the upstream
    // directory layout so the `import "opentelemetry/..."` lines are untouched.
    let otlp = [
        "protos/opentelemetry/proto/common/v1/common.proto",
        "protos/opentelemetry/proto/resource/v1/resource.proto",
        "protos/opentelemetry/proto/trace/v1/trace.proto",
        "protos/opentelemetry/proto/collector/trace/v1/trace_service.proto",
    ];
    for f in otlp {
        println!("cargo:rerun-if-changed={f}");
    }
    // tacky-build inlines a file's imports into the generated module, so pointing
    // it at the collector service file yields `ExportTraceServiceRequest` and the
    // whole `ResourceSpans` tree in one place. The include path has to be absolute:
    // pb-rs resolves a relative one against the *importing file's* directory, C
    // preprocessor style, which never matches a deep tree like this one.
    let protos_root = std::fs::canonicalize("protos").unwrap();
    tacky_build::write_proto_with_includes(
        otlp[3],
        &format!("{out_dir}/tacky_otlp.rs"),
        &[protos_root.to_str().unwrap()],
    );
    // prost spreads one module per proto package and cross-references them with
    // `super::super::`, so ask for the module tree in a single includable file
    // instead of stitching it together by hand in the bench.
    prost_build::Config::new()
        .include_file("otlp.rs")
        .compile_protos(&otlp, &["protos"])
        .unwrap();

    // `descriptor.proto` is deliberately absent: the C++ arm of
    // `benches/descriptor_set.rs` uses the runtime's own built-in
    // `google/protobuf/descriptor.pb.h`, because compiling our vendored copy would
    // register a second `google/protobuf/descriptor.proto` in the descriptor pool.
    #[cfg(feature = "cpp")]
    build_cpp(
        &out_dir,
        &[
            simple_file,
            pprof_file,
            accesslog_file,
            m1p2_file,
            m1p3_file,
            otlp[0],
            otlp[1],
            otlp[2],
            otlp[3],
        ],
        // proto3 only: proto2 schemas (simple_message, benchmark_message1_proto2,
        // descriptor) never hit `VerifyUtf8String`, so their `-cached` arm already
        // is the runtime's floor.
        &[
            pprof_file,
            accesslog_file,
            m1p3_file,
            otlp[0],
            otlp[1],
            otlp[2],
            otlp[3],
        ],
    );
}

/// Writes a copy of `src` into `OUT_DIR` with proto2 `[default = ...]` field
/// options removed, and returns its path.
///
/// tacky-build rejects custom defaults outright, and they make no difference to
/// what this bench measures: a default only decides what a *reader* sees for an
/// absent field, so every byte on the wire is unchanged. Keeping the vendored
/// proto pristine matters more than avoiding the copy — the file is upstream's.
///
/// Only handles a `[default = ...]` that is the whole option list. A field
/// carrying another option alongside it would need real option parsing.
fn strip_custom_defaults(src: &str, out_dir: &str) -> String {
    let text = std::fs::read_to_string(src).unwrap();
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        match (line.find("[default ="), line.rfind(']')) {
            (Some(open), Some(close)) if close > open => {
                out.push_str(&line[..open]);
                out.push_str(&line[close + 1..]);
            }
            _ => out.push_str(line),
        }
        out.push('\n');
    }

    let name = std::path::Path::new(src)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
    let dst = format!("{out_dir}/{name}");
    std::fs::write(&dst, out).unwrap();
    dst
}

/// Writes an edition-2023 copy of a proto3 file into `OUT_DIR/noutf8/`, mirroring
/// its path under `protos/`, with UTF-8 validation switched off. Returns the
/// copy's path relative to `OUT_DIR`.
///
/// C++ runs every proto3 `string` through `WireFormatLite::VerifyUtf8String` on
/// serialize; Rust pays nothing for that because `&str`/`String` are UTF-8 by
/// construction. Leaving it on measures a language guarantee rather than
/// serializer design, so the bench reports an arm with it off too. proto2 schemas
/// need no copy — the C++ runtime never validates their strings.
///
/// `field_presence = IMPLICIT` plus the edition-2023 defaults for
/// `repeated_field_encoding` and `enum_type` reproduce proto3 semantics, so the
/// wire bytes are unchanged — the bench asserts that against prost's output.
///
/// The package is *prefixed* with `noutf8.` rather than suffixed, and imports are
/// repointed at the sibling copies, so a multi-file tree like OTLP stays
/// self-consistent: protobuf resolves a relative type reference such as
/// `opentelemetry.proto.common.v1.KeyValue` innermost-scope-first, so inside
/// `noutf8.opentelemetry.…` it finds the copy and never the UTF-8-validating
/// original.
#[cfg(feature = "cpp")]
fn derive_noutf8_proto(src: &str, out_dir: &str) -> String {
    let text = std::fs::read_to_string(src).unwrap();
    let mut out = String::with_capacity(text.len() + 128);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "syntax = \"proto3\";" {
            out.push_str("edition = \"2023\";\n");
        } else if let Some(pkg) = trimmed.strip_prefix("package ") {
            out.push_str(&format!("package noutf8.{pkg}\n"));
            out.push_str("option features.field_presence = IMPLICIT;\n");
            out.push_str("option features.utf8_validation = NONE;\n");
        } else if let Some(path) = trimmed.strip_prefix("import \"") {
            out.push_str(&format!("import \"noutf8/{path}\n"));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }

    let rel = format!(
        "noutf8/{}",
        src.strip_prefix("protos/")
            .expect("proto path is under protos/")
    );
    let dst = format!("{out_dir}/{rel}");
    std::fs::create_dir_all(std::path::Path::new(&dst).parent().unwrap()).unwrap();
    std::fs::write(&dst, out).unwrap();
    rel
}

/// Compiles the C++ runtime's generated code plus `cpp/shim.cc` into a static lib the
/// benches link against. Needs `protoc` and a pkg-config-discoverable protobuf;
/// `scripts/bench_cpp.sh` sets both up. A *shared* protobuf also satisfies the probe but
/// understates the C++ arms, so it is fine for a smoke test and not for published numbers.
#[cfg(feature = "cpp")]
fn build_cpp(out_dir: &str, protos: &[&str], noutf8: &[&str]) {
    println!("cargo:rerun-if-changed=cpp/shim.cc");
    println!("cargo:rerun-if-env-changed=TACKY_PROTOBUF_PREFIX");
    println!("cargo:rerun-if-env-changed=TACKY_CPP_ROOT");
    let prefix = cpp_prefix();

    // protoc mirrors each input's include-relative path into `--cpp_out`, so track
    // these as paths rather than stems: the OTLP tree is nested, and `-Iprotos`
    // makes `protos/opentelemetry/.../trace.proto` come out as
    // `{out_dir}/opentelemetry/.../trace.pb.cc`. Flat files are unaffected.
    let mut files: Vec<String> = protos
        .iter()
        .map(|p| {
            p.strip_prefix("protos/")
                .expect("proto path is under protos/")
                .to_string()
        })
        .collect();
    // The derived no-UTF8 copies live under `OUT_DIR/noutf8/`, found via `-I`.
    files.extend(noutf8.iter().map(|p| derive_noutf8_proto(p, out_dir)));

    let protoc = prefix
        .as_ref()
        .map_or_else(|| "protoc".to_string(), |p| format!("{p}/bin/protoc"));
    let status = std::process::Command::new(&protoc)
        .arg("-Iprotos")
        .arg(format!("-I{out_dir}"))
        .arg(format!("--cpp_out={out_dir}"))
        .args(&files)
        .current_dir(".")
        .status()
        .expect("protoc not found on PATH; install your platform's protobuf compiler");
    assert!(status.success(), "protoc --cpp_out failed");

    // The pkg-config crate emits the link directives itself. `statik` is what pulls in the
    // whole abseil dependency graph, in link order.
    if let Some(p) = &prefix {
        std::env::set_var("PKG_CONFIG_PATH", format!("{p}/lib/pkgconfig"));
    }
    let protobuf = pkg_config::Config::new()
        .statik(prefix.is_some())
        .probe("protobuf")
        .expect("protobuf not found; run scripts/bench_cpp.sh to build it");

    // Codegen and runtime must share a protobuf major version. Mixing them is what produces
    // the otherwise baffling `google/protobuf/runtime_version.h: No such file` — that header
    // arrived in v27, so gencode from a newer protoc cannot find it in an older runtime's
    // includes. Distro protobuf packages are a common way to end up here.
    let major = |v: &str| v.trim().split('.').next().unwrap_or_default().to_string();
    let protoc_ver = std::process::Command::new(&protoc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.split_whitespace().nth(1).map(str::to_string))
        .unwrap_or_default();
    assert_eq!(
        major(&protoc_ver),
        major(&protobuf.version),
        "protoc {protoc_ver} generates code the protobuf {} runtime cannot compile; \
         run scripts/bench_cpp.sh to build a matching pair",
        protobuf.version,
    );

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .opt_level(3)
        .define("NDEBUG", None)
        .flag_if_supported("-fno-omit-frame-pointer")
        .include(out_dir)
        .file("cpp/shim.cc");

    if prefix.is_none() {
        build.define("PROTOBUF_USE_DLLS", None);
    }

    for f in &files {
        let stem = f.strip_suffix(".proto").unwrap();
        build.file(format!("{out_dir}/{stem}.pb.cc"));
    }

    // Link flags were emitted by the probe; only includes are ours to forward.
    for dir in &protobuf.include_paths {
        build.include(dir);
    }

    build.compile("tacky_cpp_shim");
}
