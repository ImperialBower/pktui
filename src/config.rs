//! Persistent user config: `~/.config/pktui/config.toml`.
//!
//! The config holds knobs a user typically wants to set once (default blinds,
//! starting stack, bot speed for Arena) rather than passing on every `pktui`
//! invocation. Anything specified on the command line overrides the config.
//!
//! The file is created on first save with sensible defaults; reading a missing
//! file returns [`Config::default`] without error.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::Result;

/// User-tunable defaults loaded from `~/.config/pktui/config.toml`.
///
/// # Examples
///
/// ```
/// use pktui::config::Config;
/// let cfg = Config::default();
/// assert_eq!(cfg.big_blind, 100);
/// assert_eq!(cfg.chips, 10_000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    /// Default small blind in chips.
    pub small_blind: usize,
    /// Default big blind in chips.
    pub big_blind: usize,
    /// Default starting chip stack per seat.
    pub chips: usize,
    /// Default delay between bot actions in Arena mode (milliseconds).
    pub arena_speed_ms: u64,
    /// Default delay between bot actions in Play mode (milliseconds).
    pub play_speed_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            small_blind: 50,
            big_blind: 100,
            chips: 10_000,
            arena_speed_ms: 800,
            play_speed_ms: 600,
        }
    }
}

impl Config {
    /// Returns the path the config is loaded from / saved to.
    ///
    /// On Linux/macOS this is `$XDG_CONFIG_HOME/pktui/config.toml`
    /// (defaulting to `~/.config/pktui/config.toml`). On Windows it is
    /// `%APPDATA%\pktui\config.toml`.
    ///
    /// Returns `None` if the platform's config dir cannot be resolved
    /// (extremely rare — primarily test sandboxes without a HOME).
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::config::Config;
    /// // On normal hosts the path resolves.
    /// let _ = Config::path();
    /// ```
    #[must_use]
    pub fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("pktui").join("config.toml"))
    }

    /// Loads the config from disk, returning [`Config::default`] if the file
    /// does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Toml`] if the file exists but is malformed,
    /// or [`crate::Error::Io`] if it exists but cannot be read.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pktui::config::Config;
    /// let cfg = Config::load().unwrap_or_default();
    /// assert!(cfg.big_blind >= cfg.small_blind);
    /// ```
    pub fn load() -> Result<Self> {
        let Some(path) = Self::path() else {
            return Ok(Self::default());
        };
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path)?;
        Self::from_toml(&text)
    }

    /// Parses a config value from a TOML string.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Toml`] if the input is not valid TOML for
    /// this struct.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::config::Config;
    /// let cfg = Config::from_toml(
    ///     "small_blind = 25\nbig_blind = 50\nchips = 5000\narena_speed_ms = 200\nplay_speed_ms = 200\n"
    /// ).unwrap();
    /// assert_eq!(cfg.big_blind, 50);
    /// ```
    pub fn from_toml(s: &str) -> Result<Self> {
        Ok(toml::from_str(s)?)
    }

    /// Serialises the config to a TOML string.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Toml`] if serialisation fails (in practice
    /// it cannot for this struct).
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::config::Config;
    /// let s = Config::default().to_toml().unwrap();
    /// assert!(s.contains("big_blind"));
    /// ```
    pub fn to_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Saves the config to disk at [`Config::path`], creating the directory
    /// if necessary.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Io`] if the file or directory cannot be
    /// written, or [`crate::Error::Other`] if no config dir is available.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pktui::config::Config;
    /// Config::default().save().unwrap();
    /// ```
    pub fn save(&self) -> Result<()> {
        let path = Self::path()
            .ok_or_else(|| crate::Error::Other("no config dir on this platform".into()))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, self.to_toml()?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_sensible_blinds() {
        let cfg = Config::default();
        assert!(cfg.small_blind < cfg.big_blind);
        assert!(cfg.chips >= cfg.big_blind * 50);
    }

    #[test]
    fn round_trip_toml() {
        let original = Config::default();
        let s = original.to_toml().unwrap();
        let parsed = Config::from_toml(&s).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn from_toml_rejects_garbage() {
        let err = Config::from_toml("this is not toml = = =").unwrap_err();
        assert!(matches!(err, crate::Error::Toml(_)));
    }

    #[test]
    fn path_resolves_on_test_host() {
        // On any reasonable test host (CI included) dirs::config_dir() works.
        assert!(Config::path().is_some());
    }
}
