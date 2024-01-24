pub mod errors;
mod parser;
pub mod types;

use errors::{Error, Result};
use std::path::{Path, PathBuf};
use types::Config;

/// A builder for [Config]
///
/// # Example build.rs
///
/// ```rust,no_run
/// use pb_rs::{types::FileDescriptor, ConfigBuilder};
/// use std::path::{Path, PathBuf};
/// use walkdir::WalkDir;
///
/// fn main() {
///     let out_dir = std::env::var("OUT_DIR").unwrap();
///     let out_dir = Path::new(&out_dir).join("protos");
///
///     let in_dir = PathBuf::from(::std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("protos");
///     // Re-run this build.rs if the protos dir changes (i.e. a new file is added)
///     println!("cargo:rerun-if-changed={}", in_dir.to_str().unwrap());
///
///     // Find all *.proto files in the `in_dir` and add them to the list of files
///     let mut protos = Vec::new();
///     let proto_ext = Some(Path::new("proto").as_os_str());
///     for entry in WalkDir::new(&in_dir) {
///         let path = entry.unwrap().into_path();
///         if path.extension() == proto_ext {
///             // Re-run this build.rs if any of the files in the protos dir change
///             println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
///             protos.push(path);
///         }
///     }
///
///     // Delete all old generated files before re-generating new ones
///     if out_dir.exists() {
///         std::fs::remove_dir_all(&out_dir).unwrap();
///     }
///     std::fs::DirBuilder::new().create(&out_dir).unwrap();
///     let config_builder = ConfigBuilder::new(&protos, None, Some(&out_dir), &[in_dir]).unwrap();
///     FileDescriptor::run(&config_builder.build()).unwrap()
/// }
/// ```
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    in_files: Vec<PathBuf>,
    out_file: Option<PathBuf>,
    include_paths: Vec<PathBuf>,
    single_module: bool,
    no_output: bool,
    error_cycle: bool,
    add_deprecated_fields: bool,
}

impl ConfigBuilder {
    pub fn new<P: AsRef<Path>>(
        in_files: &[P],
        output: Option<&P>,
        output_dir: Option<&P>,
        include_paths: &[P],
    ) -> Result<ConfigBuilder> {
        let in_files = in_files
            .iter()
            .map(|f| f.as_ref().into())
            .collect::<Vec<PathBuf>>();
        let output = output.map(|f| f.as_ref().into());
        let output_dir: Option<PathBuf> = output_dir.map(|f| f.as_ref().into());
        let mut include_paths = include_paths
            .iter()
            .map(|f| f.as_ref().into())
            .collect::<Vec<PathBuf>>();

        if in_files.is_empty() {
            return Err(Error::NoProto);
        }

        for f in &in_files {
            if !f.exists() {
                return Err(Error::InputFile(format!("{}", f.display())));
            }
        }

        let out_file = match (output, output_dir) {
            (Some(_), None) if in_files.len() > 1 => return Err(Error::OutputMultipleInputs),
            (Some(output), None) => Some(output),
            (None, Some(output_dir)) => {
                if !output_dir.is_dir() {
                    return Err(Error::OutputDirectory(format!("{}", output_dir.display())));
                }
                Some(output_dir)
            }
            (Some(_), Some(_)) => return Err(Error::OutputAndOutputDir),
            (None, None) => None,
        };

        let default = PathBuf::from(".");
        if include_paths.is_empty() || !include_paths.contains(&default) {
            include_paths.push(default);
        }

        Ok(ConfigBuilder {
            in_files,
            out_file,
            include_paths,
            ..Default::default()
        })
    }

    /// Omit generation of modules for each package when there is only one package
    pub fn single_module(mut self, val: bool) -> Self {
        self.single_module = val;
        self
    }

    /// Show enums and messages in this .proto file, including those imported. No code generated.
    /// `no_output` should probably only be used by the pb-rs cli.
    pub fn no_output(mut self, val: bool) -> Self {
        self.no_output = val;
        self
    }

    /// Error out if recursive messages do not have optional fields
    pub fn error_cycle(mut self, val: bool) -> Self {
        self.error_cycle = val;
        self
    }

    /// Add deprecated fields and mark them as `#[deprecated]`
    pub fn add_deprecated_fields(mut self, val: bool) -> Self {
        self.add_deprecated_fields = val;
        self
    }

    /// Build [Config] from this `ConfigBuilder`
    pub fn build(self) -> Vec<Config> {
        self.in_files
            .iter()
            .map(|in_file| {
                let mut out_file = in_file.with_extension("rs");

                if let Some(ref ofile) = self.out_file {
                    if ofile.is_dir() {
                        out_file = ofile.join(out_file.file_name().unwrap());
                    } else {
                        out_file = ofile.into();
                    }
                }

                Config {
                    in_file: in_file.to_owned(),
                    out_file,
                    import_search_path: self.include_paths.clone(),
                    single_module: self.single_module,
                    no_output: self.no_output,
                    error_cycle: self.error_cycle,
                    add_deprecated_fields: self.add_deprecated_fields,
                }
            })
            .collect()
    }
}
