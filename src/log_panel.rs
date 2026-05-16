//! The rolling event-log buffer rendered below the table.
//!
//! Every action the engine reports (deal, fold, bet, raise, street advance,
//! showdown, etc.) is appended as one [`LogLine`]. The buffer caps at
//! [`LogPanel::CAPACITY`] entries, dropping the oldest when full so a long
//! session does not eat unbounded memory.

use std::collections::VecDeque;

/// A single line in the log, paired with a [`Severity`] for colouring.
///
/// # Examples
///
/// ```
/// use pktui::log_panel::{LogLine, Severity};
/// let l = LogLine::new(Severity::Info, "hello");
/// assert_eq!(l.text, "hello");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    /// Colour bucket.
    pub severity: Severity,
    /// The displayed text.
    pub text: String,
}

impl LogLine {
    /// Constructs a new log line.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::log_panel::{LogLine, Severity};
    /// let l = LogLine::new(Severity::Win, "Alice wins 500");
    /// assert!(matches!(l.severity, Severity::Win));
    /// ```
    pub fn new(severity: Severity, text: impl Into<String>) -> Self {
        Self {
            severity,
            text: text.into(),
        }
    }
}

/// Severity / category of a log line — drives colour in the UI.
///
/// # Examples
///
/// ```
/// use pktui::log_panel::Severity;
/// assert_eq!(Severity::default(), Severity::Info);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Severity {
    /// Generic information (deals, street advances).
    #[default]
    Info,
    /// A bet/raise/all-in — drawn in gold.
    Action,
    /// A fold — drawn in red.
    Fold,
    /// A pot win — drawn in green.
    Win,
    /// An engine error or rejected action.
    Error,
}

/// A bounded ring buffer of [`LogLine`]s.
///
/// # Examples
///
/// ```
/// use pktui::log_panel::{LogPanel, Severity};
/// let mut log = LogPanel::new();
/// log.push(Severity::Info, "Hand 1");
/// assert_eq!(log.len(), 1);
/// ```
#[derive(Debug, Clone, Default)]
pub struct LogPanel {
    lines: VecDeque<LogLine>,
}

impl LogPanel {
    /// Maximum lines held before the oldest is dropped.
    pub const CAPACITY: usize = 1024;

    /// Creates an empty panel.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::log_panel::LogPanel;
    /// let log = LogPanel::new();
    /// assert!(log.is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a new line. Drops the oldest when full.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::log_panel::{LogPanel, Severity};
    /// let mut log = LogPanel::new();
    /// log.push(Severity::Action, "Bob bets 100");
    /// assert_eq!(log.len(), 1);
    /// ```
    pub fn push(&mut self, severity: Severity, text: impl Into<String>) {
        if self.lines.len() == Self::CAPACITY {
            self.lines.pop_front();
        }
        self.lines.push_back(LogLine::new(severity, text));
    }

    /// Returns the number of buffered lines.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::log_panel::LogPanel;
    /// assert_eq!(LogPanel::new().len(), 0);
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Returns true when no lines are buffered.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::log_panel::LogPanel;
    /// assert!(LogPanel::new().is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Iterates over buffered lines in oldest-first order.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::log_panel::{LogPanel, Severity};
    /// let mut log = LogPanel::new();
    /// log.push(Severity::Info, "first");
    /// log.push(Severity::Info, "second");
    /// let texts: Vec<&str> = log.iter().map(|l| l.text.as_str()).collect();
    /// assert_eq!(texts, vec!["first", "second"]);
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = &LogLine> {
        self.lines.iter()
    }

    /// Returns a slice of the most recent `n` lines (oldest-first within the
    /// slice).
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::log_panel::{LogPanel, Severity};
    /// let mut log = LogPanel::new();
    /// for i in 0..5 {
    ///     log.push(Severity::Info, format!("line {i}"));
    /// }
    /// let last2: Vec<&str> = log.tail(2).iter().map(|l| l.text.as_str()).collect();
    /// assert_eq!(last2, vec!["line 3", "line 4"]);
    /// ```
    #[must_use]
    pub fn tail(&self, n: usize) -> Vec<&LogLine> {
        let start = self.lines.len().saturating_sub(n);
        self.lines.iter().skip(start).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_len() {
        let mut log = LogPanel::new();
        assert!(log.is_empty());
        log.push(Severity::Info, "x");
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn drops_oldest_when_full() {
        let mut log = LogPanel::new();
        for i in 0..(LogPanel::CAPACITY + 50) {
            log.push(Severity::Info, format!("{i}"));
        }
        assert_eq!(log.len(), LogPanel::CAPACITY);
        // Oldest should now be index 50.
        let first = log.iter().next().unwrap();
        assert_eq!(first.text, "50");
    }

    #[test]
    fn tail_handles_short_buffer() {
        let mut log = LogPanel::new();
        log.push(Severity::Info, "only");
        let t = log.tail(10);
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn severity_default_is_info() {
        assert_eq!(Severity::default(), Severity::Info);
    }

    #[test]
    fn log_line_construction() {
        let l = LogLine::new(Severity::Win, "won");
        assert_eq!(l.text, "won");
        assert_eq!(l.severity, Severity::Win);
    }
}
