use serde::Deserialize;
use takopack_core::{error::Error as CoreError, network::fetch_jsonl};

use super::fetched::FromCratesIo;

pub struct CratesIoFetcher {
    name: String,
    version: Option<String>,

    pub(super) _tempdir: Option<tempfile::TempDir>,
}

impl CratesIoFetcher {
    pub fn new(name: &str, version: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            version: version.map(|s| s.to_string()),
            _tempdir: None,
        }
    }

    pub fn fetch(self) -> Result<FromCratesIo, Error> {
        let tempdir = tempfile::TempDir::new()?;

        let data_from_sprase_index = fetch_data_from_sprase_index(&self.name)?;

        let crate_info = match self.version.as_deref() {
            Some(v) => data_from_sprase_index
                .into_iter()
                .find(|c: &CrateInfo| c.vers == v) // We believe crates.io gives no duplicated version.
                .ok_or_else(|| Error::VersionNotFound {
                    name: self.name.to_owned(),
                    version: v.to_owned(),
                }),
            // Assume that the last one crateinfo matches the latest version.
            None => data_from_sprase_index
                .into_iter()
                .last()
                .ok_or_else(|| Error::CrateNotFound {
                    name: self.name.to_owned(),
                }),
        }?;

        let version = crate_info.vers;
        let id = format!("{}-{}", self.name, version);
        let filename = format!("{}.crate", &id);

        takopack_core::network::fetch_binary(
            &filename,
            tempdir.path(),
            &format!("https://static.crates.io/crates/{}/{}", self.name, filename),
        )?;

        Ok(FromCratesIo::new(
            &id,
            tempdir.path().join(&filename),
            tempdir,
        ))
    }
}

#[derive(Debug)]
pub enum Error {
    CoreFunc(CoreError),
    VersionNotFound { name: String, version: String },
    CrateNotFound { name: String },
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::CoreFunc(error) => error.fmt(f),
            Error::VersionNotFound { name, version } => write!(f, "{}-{} not found", name, version),
            Error::CrateNotFound { name } => write!(f, "crate {} not found", name),
        }
    }
}

impl From<takopack_core::error::Error> for Error {
    fn from(value: takopack_core::error::Error) -> Self {
        Self::CoreFunc(value)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        value.into()
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

#[derive(Debug, Deserialize)]
struct CrateInfo {
    pub vers: String,
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
