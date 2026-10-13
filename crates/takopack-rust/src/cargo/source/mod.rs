pub mod crates_io;

use std::{fs::File, path::Path};

use sha2::{Digest, Sha256};

pub trait CargoSource {
    /// Extract into cargo_dir and return the path.
    /// Expect that Cargo.toml locates at `<dest_dir>/Cargo.toml`
    fn cargo_dir(&self) -> &Path;

    fn name_version(&self) -> (&str, &str);

    fn archive(&self) -> &Path;

    fn checksum(&self) -> Result<String, std::io::Error> {
        let mut file = File::open(self.archive())?;
        let mut hasher = Sha256::new();

        std::io::copy(&mut file, &mut hasher)?;

        Ok(format!("{:X}", hasher.finalize()))
    }
}
