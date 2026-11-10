use crate::cargo::source::remote::archive::FromRemote;

/// Builder for a generic remote archive source.
///
/// The URL must point to a gzip-compressed tar archive. A SHA-256 checksum is
/// optional; when supplied it is verified during [`CargoSource`] loading.
///
/// [`CargoSource`]: crate::cargo::source::CargoSource
pub struct FromRemoteBuilder {
    url: String,
    checksum: Option<String>,
}

impl FromRemoteBuilder {
    /// Create a source for a remote archive URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            checksum: None,
        }
    }

    /// Set the expected SHA-256 checksum of the archive.
    pub fn checksum(mut self, checksum: impl Into<String>) -> Self {
        self.checksum = Some(checksum.into());
        self
    }

    /// Finish building the source.
    ///
    /// The URL is fetched lazily by the source pipeline.
    pub fn build(self) -> FromRemote {
        FromRemote::new(self.url, self.checksum)
    }
}
