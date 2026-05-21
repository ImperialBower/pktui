//! Replay mode: walk through a saved YAML hand collection.
//!
//! Loads a [`HandCollection`] file (as produced by pkcore's
//! `interactive_play` example or pktui's own session save) and exposes
//! cursor controls for next/previous hand and next/previous street.
//!
//! Replay is a read-only viewer: no `apply_action` calls happen, the engine
//! is not touched. Each hand is rendered straight from the YAML record.

use std::path::Path;

use pkcore::hand_history::HandCollection;

use crate::error::{Error, Result};
use crate::log_panel::{LogPanel, Severity};

/// All state needed to drive Replay mode.
pub struct ReplayState {
    /// The loaded YAML collection.
    pub collection: HandCollection,
    /// Cursor: which hand we're viewing.
    pub hand_index: usize,
    /// Cursor: which street within the hand. 0=preflop, 1=flop, 2=turn,
    /// 3=river, 4=results.
    pub street_index: usize,
}

/// Number of "streets" pktui's replay viewer recognises (preflop, flop,
/// turn, river, results) — used for cursor bounds checking.
pub const STREET_COUNT: usize = 5;

impl ReplayState {
    /// Loads the collection from a YAML file.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] if the file cannot be read or [`Error::Yaml`]
    /// if its contents do not parse.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::path::PathBuf;
    /// use pktui::log_panel::LogPanel;
    /// use pktui::modes::ReplayState;
    ///
    /// let mut log = LogPanel::new();
    /// let _ = ReplayState::from_file(&PathBuf::from("session.yaml"), &mut log);
    /// ```
    pub fn from_file(path: &Path, log: &mut LogPanel) -> Result<Self> {
        let yaml = std::fs::read_to_string(path)?;
        let collection =
            HandCollection::from_yaml(&yaml).map_err(|e| Error::Yaml(e.to_string()))?;
        log.push(
            Severity::Info,
            format!(
                "Loaded {} hand(s) from {}",
                collection.len(),
                path.display()
            ),
        );
        Ok(Self {
            collection,
            hand_index: 0,
            street_index: 0,
        })
    }

    /// Total number of hands in the loaded collection.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::path::PathBuf;
    /// use pktui::log_panel::LogPanel;
    /// use pktui::modes::ReplayState;
    /// let mut log = LogPanel::new();
    /// let s = ReplayState::from_file(&PathBuf::from("x.yaml"), &mut log).unwrap();
    /// assert!(s.hand_count() > 0);
    /// ```
    #[must_use]
    pub fn hand_count(&self) -> usize {
        self.collection.len()
    }

    /// Advances cursor to the next hand (saturates at last hand).
    pub fn next_hand(&mut self) {
        if self.hand_index + 1 < self.hand_count() {
            self.hand_index += 1;
            self.street_index = 0;
        }
    }

    /// Moves cursor to the previous hand (saturates at first hand).
    pub fn prev_hand(&mut self) {
        if self.hand_index > 0 {
            self.hand_index -= 1;
            self.street_index = 0;
        }
    }

    /// Advances cursor to the next street within the current hand.
    pub fn next_street(&mut self) {
        if self.street_index + 1 < STREET_COUNT {
            self.street_index += 1;
        }
    }

    /// Moves cursor to the previous street within the current hand.
    pub fn prev_street(&mut self) {
        if self.street_index > 0 {
            self.street_index -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_minimal_yaml() -> NamedTempFile {
        // The simplest valid HandCollection is one with just a header.
        // We use HandCollection::new() to generate the canonical empty form
        // and then re-serialise so the format always matches the engine.
        let coll = HandCollection::new();
        let yaml = coll.to_yaml().expect("serialise");
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(yaml.as_bytes()).unwrap();
        f
    }

    #[test]
    #[cfg_attr(miri, ignore = "uses real filesystem; Miri has no useful semantics for fs syscalls")]
    fn loads_empty_collection() {
        let f = write_minimal_yaml();
        let mut log = LogPanel::new();
        let s = ReplayState::from_file(f.path(), &mut log).unwrap();
        assert_eq!(s.hand_count(), 0);
    }

    #[test]
    #[cfg_attr(miri, ignore = "uses real filesystem; Miri has no useful semantics for fs syscalls")]
    fn missing_file_yields_io_error() {
        let mut log = LogPanel::new();
        match ReplayState::from_file(
            std::path::Path::new("/nonexistent/__pktui_test__.yaml"),
            &mut log,
        ) {
            Ok(_) => panic!("expected IO error"),
            Err(e) => assert!(matches!(e, Error::Io(_))),
        }
    }

    #[test]
    #[cfg_attr(miri, ignore = "uses real filesystem; Miri has no useful semantics for fs syscalls")]
    fn malformed_yaml_yields_yaml_error() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"not: valid: yaml: at: all: : :").unwrap();
        let mut log = LogPanel::new();
        match ReplayState::from_file(f.path(), &mut log) {
            Ok(_) => panic!("expected YAML error"),
            Err(e) => assert!(matches!(e, Error::Yaml(_))),
        }
    }

    #[test]
    fn cursor_navigation_saturates() {
        let coll = HandCollection::new();
        let mut s = ReplayState {
            collection: coll,
            hand_index: 0,
            street_index: 0,
        };
        s.prev_hand();
        assert_eq!(s.hand_index, 0);
        s.prev_street();
        assert_eq!(s.street_index, 0);
        // next_hand on an empty collection is a no-op.
        s.next_hand();
        assert_eq!(s.hand_index, 0);
        // next_street saturates within STREET_COUNT-1.
        for _ in 0..10 {
            s.next_street();
        }
        assert_eq!(s.street_index, STREET_COUNT - 1);
    }
}
