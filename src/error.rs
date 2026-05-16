//! Crate-wide error type.
//!
//! Every fallible operation in pktui returns [`Result<T>`], a thin alias for
//! [`std::result::Result<T, Error>`]. The [`Error`] enum bundles all upstream
//! error categories — engine errors from `pkcore`, terminal IO errors, YAML
//! parse failures, and config TOML errors — into a single `?`-friendly type.

use std::fmt;

/// A `Result` whose error type is [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// All errors pktui can produce.
///
/// The variants intentionally wrap their underlying error rather than
/// flattening into a single string, so callers can match on the specific
/// failure kind when they need to (for example, distinguishing "the user
/// supplied an invalid YAML file" from "the engine refused an action").
///
/// # Examples
///
/// ```
/// use pktui::Error;
/// let e = Error::Other("boom".into());
/// assert!(format!("{e}").contains("boom"));
/// ```
#[derive(Debug)]
pub enum Error {
    /// Wraps a `pkcore` engine error.
    Engine(pkcore::PKError),
    /// Wraps a terminal / crossterm IO error.
    Io(std::io::Error),
    /// Wraps a YAML parse failure (HandCollection load).
    Yaml(String),
    /// Wraps a TOML serialise/deserialise failure (config file).
    Toml(String),
    /// Catch-all for anything else.
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine(e) => write!(f, "engine error: {e}"),
            Self::Io(e) => write!(f, "io error: {e}"),
            Self::Yaml(s) => write!(f, "yaml error: {s}"),
            Self::Toml(s) => write!(f, "toml error: {s}"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<pkcore::PKError> for Error {
    fn from(e: pkcore::PKError) -> Self {
        Self::Engine(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<toml::de::Error> for Error {
    fn from(e: toml::de::Error) -> Self {
        Self::Toml(e.to_string())
    }
}

impl From<toml::ser::Error> for Error {
    fn from(e: toml::ser::Error) -> Self {
        Self::Toml(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_message() {
        let e = Error::Other("oops".into());
        assert_eq!(format!("{e}"), "oops");
    }

    #[test]
    fn from_io_error() {
        let io = std::io::Error::other("x");
        let e: Error = io.into();
        assert!(matches!(e, Error::Io(_)));
    }

    #[test]
    fn from_toml_de_error() {
        let de: toml::de::Error = toml::from_str::<toml::Value>("invalid = ").unwrap_err();
        let e: Error = de.into();
        assert!(matches!(e, Error::Toml(_)));
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Error>();
    }
}
