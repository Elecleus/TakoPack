use std::path::Path;

use crate::cargo::source::{ArchiveMetadata, CargoSource, TarGzArchive, remote::error};

/// A [`CargoSource`] backed by a remote gzip-compressed tar archive.
///
/// The archive is downloaded lazily on the first [`CargoSource::fetch`] call.
/// The source has no crate identity of its own: package names and versions
/// come from the extracted Cargo workspace.
#[derive(Debug)]
pub struct FromRemote {
    url: String,
    checksum: Option<String>,
}

impl FromRemote {
    pub(crate) fn new(url: String, checksum: Option<String>) -> Self {
        Self { url, checksum }
    }
}

impl CargoSource for FromRemote {
    type Error = error::Error;
    type Archive = TarGzArchive;

    fn fetch(&self, destination: &Path) -> Result<Self::Archive, Self::Error> {
        takopack_core::network::fetch_binary(destination, &self.url)?;

        let metadata = match self.checksum.as_deref() {
            Some(expected) => {
                ArchiveMetadata::try_new_hashchecked(destination, Some(self.url.clone()), expected)?
            }
            None => ArchiveMetadata::try_new(destination, Some(self.url.clone()))?,
        };

        Ok(TarGzArchive::new(metadata))
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use flate2::Compression;
    use flate2::write::GzEncoder;
    use sha2::{Digest, Sha256};
    use tar::{Builder, Header};

    use crate::cargo::source::Archive;
    use crate::cargo::source::remote::FromRemoteBuilder;

    use super::*;

    fn tar_gz(entries: &[(&str, &str)]) -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut builder = Builder::new(encoder);

        for (name, contents) in entries {
            let mut header = Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, *name, contents.as_bytes())
                .unwrap();
        }

        let encoder = builder.into_inner().unwrap();
        encoder.finish().unwrap()
    }

    fn serve_once(body: Vec<u8>, path: &str) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request);

            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/gzip\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
            stream.flush().unwrap();
        });

        (format!("http://{address}/{path}"), handle)
    }

    #[test]
    fn downloads_checks_and_extracts_the_archive() {
        let body = tar_gz(&[
            (
                "demo-0.1.0/Cargo.toml",
                "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
            ),
            ("demo-0.1.0/lib.rs", ""),
        ]);
        let checksum = format!("{:x}", Sha256::digest(&body));
        let (url, server) = serve_once(body, "demo-0.1.0.tar.gz");

        let source = FromRemoteBuilder::new(url.clone())
            .checksum(checksum.clone())
            .build();
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("source.archive");
        let archive = source.fetch(&archive_path).unwrap();

        assert_eq!(archive.metadata().path, archive_path);
        assert_eq!(archive.metadata().checksum, checksum);
        assert_eq!(archive.metadata().url.as_deref(), Some(url.as_str()));
        assert!(archive.metadata().path.is_file());

        let extracted = archive.extract(&dir.path().join("extracted")).unwrap();
        assert!(extracted.manifest.ends_with("Cargo.toml"));
        server.join().unwrap();
    }

    fn cargo_available() -> bool {
        std::process::Command::new("cargo")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn loads_a_raw_git_workspace_archive() {
        if !cargo_available() {
            return;
        }

        let body = tar_gz(&[
            (
                "repo-main/Cargo.toml",
                "[workspace]\nmembers = [\"a\", \"b\"]\nresolver = \"2\"\n",
            ),
            (
                "repo-main/a/Cargo.toml",
                "[package]\nname = \"a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"lib.rs\"\n",
            ),
            ("repo-main/a/lib.rs", ""),
            (
                "repo-main/b/Cargo.toml",
                "[package]\nname = \"b\"\nversion = \"0.2.0\"\nedition = \"2021\"\n\n[lib]\npath = \"lib.rs\"\n",
            ),
            ("repo-main/b/lib.rs", ""),
        ]);
        let (url, server) = serve_once(body, "repo-main.tar.gz");

        let source = FromRemoteBuilder::new(url).build();
        let dir = tempfile::tempdir().unwrap();
        let mut process = crate::cargo::Process::from(source, dir.path());
        let output = process.load().unwrap();

        assert_eq!(output.packages.len(), 2);
        let mut names: Vec<&str> = output
            .packages
            .iter()
            .map(|package| package.name.as_str())
            .collect();
        names.sort_unstable();
        assert_eq!(names, vec!["a", "b"]);

        server.join().unwrap();
    }
}
