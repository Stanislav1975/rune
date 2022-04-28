use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::codegen::RuneVersion;

/// Inputs used during the compilation process.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct BuildContext {
    /// The name of the Rune being compiled.
    pub name: String,
    /// The `Runefile.yml` source text.
    pub runefile: String,
    /// A directory that can be used for any temporary artifacts.
    pub working_directory: PathBuf,
    /// The directory that all paths (e.g. to models) are resolved relative to.
    pub current_directory: PathBuf,
    /// Generate an optimized build.
    pub optimized: bool,
    pub verbosity: Verbosity,
    /// The version of Rune being used.
    pub rune_version: Option<RuneVersion>,
}

impl BuildContext {
    /// Create a new [`BuildContext`] using the convention that the
    /// [`BuildContext.name`] is named after the
    /// [`BuildContext.current_directory`].
    pub fn for_directory(
        directory: impl Into<PathBuf>,
    ) -> Result<BuildContext, std::io::Error> {
        let current_directory = directory.into();
        let working_directory = current_directory.clone();

        let name = current_directory
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Unable to determine the current directory's name",
                )
            })?;

        let runefile = current_directory.join("Runefile.yml");
        let runefile = std::fs::read_to_string(runefile)?;

        Ok(BuildContext {
            name,
            runefile,
            working_directory,
            current_directory,
            optimized: true,
            verbosity: Verbosity::Normal,
            rune_version: Some(RuneVersion {
                version: env!("CARGO_PKG_VERSION").to_string(),
            }),
        })
    }

    #[cfg(test)]
    pub(crate) fn from_doc(doc: crate::parse::Document) -> Self {
        BuildContext {
            name: "rune".to_string(),
            runefile: serde_yaml::to_string(&doc).unwrap(),
            working_directory: PathBuf::from("."),
            current_directory: PathBuf::from("."),
            optimized: false,
            verbosity: Verbosity::Normal,
            rune_version: Some(RuneVersion {
                version: env!("CARGO_PKG_VERSION").to_string(),
            }),
        }
    }
}

#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum Verbosity {
    Quiet,
    Normal,
    Verbose,
}

impl Verbosity {
    pub fn from_quiet_and_verbose(quiet: bool, verbose: bool) -> Option<Self> {
        match (verbose, quiet) {
            (true, false) => Some(Verbosity::Verbose),
            (false, true) => Some(Verbosity::Quiet),
            (false, false) => Some(Verbosity::Normal),
            (true, true) => None,
        }
    }

    /// Add a `--quiet` or `--verbose` argument to the command if necessary.
    pub fn add_flags(&self, cmd: &mut Command) {
        match self {
            Verbosity::Quiet => {
                cmd.arg("--quiet");
            },
            Verbosity::Verbose => {
                cmd.arg("--verbose");
            },
            Verbosity::Normal => {},
        }
    }
}

/// Feature flags and other knobs that can be used during development.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FeatureFlags {
    pub(crate) rune_repo_dir: Option<PathBuf>,
}

impl FeatureFlags {
    pub fn development() -> Self {
        let hotg_repo_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .filter(|repo_root| repo_root.join(".git").exists())
            .map(PathBuf::from);

        FeatureFlags {
            rune_repo_dir: hotg_repo_dir,
        }
    }

    pub const fn production() -> Self {
        FeatureFlags {
            rune_repo_dir: None,
        }
    }

    /// If specified, Rune crates (e.g `hotg-rune-core`) will be patched
    /// to use crates from this directory instead of crates.io or GitHub.
    pub fn set_rune_repo_dir(
        &mut self,
        hotg_repo_dir: impl Into<Option<PathBuf>>,
    ) -> &mut Self {
        self.rune_repo_dir = hotg_repo_dir.into();
        self
    }
}

impl Default for FeatureFlags {
    fn default() -> Self { FeatureFlags::production() }
}
