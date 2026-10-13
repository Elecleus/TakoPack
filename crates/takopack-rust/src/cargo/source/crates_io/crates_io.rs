use std::path::Path;

use takopack_core::{error::Error as CoreError, network::fetch_jsonl, tarball::extract_tar_gz};

use crate::cargo::source::crates_io::{error::Error, prepared::PreparedCrate};

pub struct FromCratesIo {
    name: String,
    version: Option<String>,
}

impl FromCratesIo {
    pub fn new(name: String) -> Self {
        Self {
            name,
            version: None,
        }
    }

    pub fn version(&mut self, version: String) -> &mut Self {
        self.version = Some(version);
        self
    }

    pub fn latest(&mut self) -> Result<&mut Self, Error> {
        let mut data_from_sprase_index = fetch_data_from_sprase_index(&self.name)?
            .into_iter()
            .filter(|info| !info.yanked);

        let crate_info = match self.version.as_deref() {
            Some(v) => data_from_sprase_index
                .find(|c: &CrateInfo| c.vers == v) // We believe crates.io gives no duplicated version.
                .ok_or_else(|| Error::VersionNotFound {
                    name: self.name.to_owned(),
                    version: v.to_owned(),
                }),
            // Assume that the last one crateinfo matches the latest version.
            None => data_from_sprase_index
                .last()
                .ok_or_else(|| Error::CrateNotFound {
                    name: self.name.to_owned(),
                }),
        }?;

        Ok(self.version(crate_info.vers))
    }

    pub fn prepare(&mut self, base_dir: impl AsRef<Path>) -> Result<PreparedCrate, Error> {
        let base_dir = base_dir.as_ref();

        let name = self.name.clone();
        let version = self.version.clone().ok_or(Error::VersionNotSet)?;
        let id = format!("{}-{}", &name, &version);

        let filename = id.to_owned() + ".crate";

        takopack_core::network::fetch_binary(
            &filename,
            &base_dir,
            &format!("https://static.crates.io/crates/{}/{}", self.name, filename),
        )?;

        let archive = base_dir.join(&filename);
        let extract_dir = base_dir.join(&id);

        extract_tar_gz(&archive, &extract_dir)?;

        Ok(PreparedCrate::new(
            name,
            version,
            archive,
            extract_dir.join(&id),
        ))
    }
}

fn fetch_data_from_sprase_index(name: &str) -> Result<Vec<CrateInfo>, Error> {
    let url = sprase_index_url(name);

    match fetch_jsonl(&url) {
        Ok(v) => Ok(v),
        Err(CoreError::Network(ureq::Error::Status(404, _))) => Err(Error::CrateNotFound {
            name: name.to_owned(),
        }),
        Err(e) => Err(e.into()),
    }
}

fn sprase_index_url(name: &str) -> String {
    let len = name.len();
    let first_part = match len {
        0 => unreachable!(),
        1 => "1",
        2 => "2",
        3 => "3",
        _ => &name[0..=1],
    };
    let second_part = match len {
        0 => unreachable!(),
        1 => &name,
        2 => &name,
        3 => &name[0..=0],
        _ => &name[2..=3],
    };
    let third_part = match len {
        0 => unreachable!(),
        1 => None,
        2 => None,
        3 => Some(&name),
        _ => Some(&name),
    };

    let mut result = format!("https://index.crates.io/{}/{}", first_part, second_part);
    if let Some(third_part) = third_part {
        result.push('/');
        result.push_str(*third_part);
    }

    result
}

#[derive(Debug, serde::Deserialize)]
struct CrateInfo {
    pub vers: String,
    pub cksum: String,
    pub yanked: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sprase_index_url() {
        assert_eq!(sprase_index_url("a"), "https://index.crates.io/1/a");
        assert_eq!(
            sprase_index_url("anyhow"),
            "https://index.crates.io/an/yh/anyhow"
        );
    }
}
