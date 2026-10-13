#[derive(Debug, Default)]
pub struct PkgdataBuilder {
    source: Option<String>,
    checksum: Option<String>,
}

impl PkgdataBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn source(&mut self, source: String) -> &mut Self {
        self.source = Some(source);
        self
    }

    pub fn checksum(&mut self, checksum: String) -> &mut Self {
        self.checksum = Some(checksum);
        self
    }
}
