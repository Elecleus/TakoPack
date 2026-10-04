pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    // Now, we use only `ureq` for http, so the Network varient only goes to `ureq::Error`.
    Network(ureq::Error),
    Io(std::io::Error),
    // Now, only serde_json would produce a DataContent error, so just write this.
    DataContent(serde_json::Error),
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<ureq::Error> for Error {
    fn from(value: ureq::Error) -> Self {
        Self::Network(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::DataContent(value)
    }
}
