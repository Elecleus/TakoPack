//! Cargo project that sources locally.

use std::path::PathBuf;

pub struct Local {
    path: PathBuf,
}

impl Local {
    pub fn new(path: PathBuf) -> Result<Self, LocalError> {
        if path.is_file() && path.ends_with("Cargo.toml") {
            // Single manifest file.
            Ok(Self { path })
        } else if path.is_dir() {
            let cargo_toml = path.join("Cargo.toml");

            if cargo_toml.is_file() {
                Ok(Self { path: cargo_toml })
            } else {
                Err(LocalError::Path)
            }
        } else {
            Err(LocalError::Path)
        }
    }
}

pub enum LocalError {
    Path,
}
