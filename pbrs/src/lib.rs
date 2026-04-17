pub mod errors;
mod parser;
pub mod types;

use errors::{Error, Result};
use std::path::{Path, PathBuf};
use types::Config;

/// A builder for [Config]
///
/// # Example
///
/// ```rust,no_run
/// use pb_rs::{types::FileDescriptor, ConfigBuilder};
/// use std::path::Path;
///
/// fn main() {
///     let proto = Path::new("protos/my_message.proto");
///     let include_dir = Path::new("protos");
///
///     let configs = ConfigBuilder::new(&[proto], &[include_dir]).unwrap();
///     for cfg in configs.build() {
///         let desc = FileDescriptor::read_proto(&cfg.in_file, &cfg.import_search_path).unwrap();
///         println!("parsed {} messages", desc.messages.len());
///     }
/// }
/// ```
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    in_files: Vec<PathBuf>,
    include_paths: Vec<PathBuf>,
}

impl ConfigBuilder {
    pub fn new<P: AsRef<Path>>(in_files: &[P], include_paths: &[P]) -> Result<ConfigBuilder> {
        let in_files = in_files
            .iter()
            .map(|f| f.as_ref().into())
            .collect::<Vec<PathBuf>>();
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

        let default = PathBuf::from(".");
        if include_paths.is_empty() || !include_paths.contains(&default) {
            include_paths.push(default);
        }

        Ok(ConfigBuilder {
            in_files,
            include_paths,
        })
    }

    /// Build [Config] from this `ConfigBuilder`
    pub fn build(self) -> Vec<Config> {
        self.in_files
            .iter()
            .map(|in_file| Config {
                in_file: in_file.to_owned(),
                import_search_path: self.include_paths.clone(),
            })
            .collect()
    }
}
