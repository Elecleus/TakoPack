use takopack_core::error::Error as CoreError;

#[derive(Debug)]
pub enum Error {
    CoreFunc(CoreError),
    VersionNotFound { name: String, version: String },
    CrateNotFound { name: String },
    VersionNotSet,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::CoreFunc(error) => error.fmt(f),
            Error::VersionNotFound { name, version } => write!(f, "{}-{} not found", name, version),
            Error::CrateNotFound { name } => write!(f, "crate {} not found", name),
            Error::VersionNotSet => write!(f, "version not set"),
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
        Self::CoreFunc(value.into())
    }
}
