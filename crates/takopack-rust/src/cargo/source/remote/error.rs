use takopack_core::error::Error as CoreError;

/// Failure while downloading or checking a remote source archive.
#[derive(Debug)]
pub enum Error {
    /// Network or I/O failure from `takopack-core`.
    CoreFunc(Box<CoreError>),
    /// Source archive creation or checksum failure.
    Source(crate::cargo::source::Error),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CoreFunc(error) => Some(error.as_ref()),
            Self::Source(error) => Some(error),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CoreFunc(error) => error.as_ref().fmt(formatter),
            Self::Source(error) => error.fmt(formatter),
        }
    }
}

impl From<CoreError> for Error {
    fn from(error: CoreError) -> Self {
        Self::CoreFunc(Box::new(error))
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Source(crate::cargo::source::Error::Io(error))
    }
}

impl From<crate::cargo::source::Error> for Error {
    fn from(error: crate::cargo::source::Error) -> Self {
        Self::Source(error)
    }
}
