//! The central [`App`] model.
//!
//! [`App`] owns:
//!
//! * The current [`AppMode`] (Play / Arena / Replay) and its per-mode state.
//! * The shared rolling [`LogPanel`].
//! * A `should_quit` flag the event loop polls each iteration.
//! * A `help_visible` flag for the toggleable keyboard-shortcuts overlay.
//!
//! Mutation happens exclusively through [`update`](crate::update::update); the
//! UI layer reads `App` immutably to render.

use crate::cli::{ArenaArgs, Command, PlayArgs, ReplayArgs};
use crate::error::Result;
use crate::log_panel::LogPanel;
use crate::modes::{ArenaState, PlayState, ReplayState};

/// Top-level mode dispatch. Holds whichever per-mode state is live.
pub enum AppMode {
    /// One human vs eight bots.
    Play(Box<PlayState>),
    /// Nine bots, watch-only.
    Arena(Box<ArenaState>),
    /// Read-only YAML replay.
    Replay(Box<ReplayState>),
}

impl AppMode {
    /// Returns a short label (`"Play"`, `"Arena"`, `"Replay"`) for the
    /// titlebar.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::cli::PlayArgs;
    /// use pktui::log_panel::LogPanel;
    /// use pktui::modes::PlayState;
    /// use pktui::app::AppMode;
    ///
    /// let mut log = LogPanel::new();
    /// let m = AppMode::Play(Box::new(PlayState::new(&PlayArgs::default(), &mut log).unwrap()));
    /// assert_eq!(m.label(), "Play");
    /// ```
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Play(_) => "Play",
            Self::Arena(_) => "Arena",
            Self::Replay(_) => "Replay",
        }
    }
}

/// The full app model.
pub struct App {
    /// The active mode and its per-mode state.
    pub mode: AppMode,
    /// Shared rolling log of engine events.
    pub log: LogPanel,
    /// Set true to break out of the event loop.
    pub should_quit: bool,
    /// Whether the keyboard-shortcuts overlay is visible.
    pub help_visible: bool,
}

impl App {
    /// Builds an `App` for the given CLI subcommand, initialising the engine
    /// for live modes or loading the YAML for replay.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error`] if the engine refuses the table (live modes)
    /// or the YAML cannot be loaded (replay mode).
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::cli::{Command, PlayArgs};
    /// use pktui::App;
    ///
    /// let app = App::new(Command::Play(PlayArgs::default())).unwrap();
    /// assert_eq!(app.mode.label(), "Play");
    /// ```
    pub fn new(command: Command) -> Result<Self> {
        let mut log = LogPanel::new();
        let mode = match command {
            Command::Play(args) => AppMode::Play(Box::new(PlayState::new(&args, &mut log)?)),
            Command::Arena(args) => AppMode::Arena(Box::new(ArenaState::new(&args, &mut log)?)),
            Command::Replay(args) => {
                AppMode::Replay(Box::new(ReplayState::from_file(&args.path, &mut log)?))
            }
        };
        Ok(Self {
            mode,
            log,
            should_quit: false,
            help_visible: false,
        })
    }

    /// Convenience constructor: Play mode with default args.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error`] if the engine init fails.
    ///
    /// # Examples
    ///
    /// ```
    /// let app = pktui::App::play_default().unwrap();
    /// assert_eq!(app.mode.label(), "Play");
    /// ```
    pub fn play_default() -> Result<Self> {
        Self::new(Command::Play(PlayArgs::default()))
    }

    /// Convenience constructor: Arena mode with default args.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error`] if the engine init fails.
    ///
    /// # Examples
    ///
    /// ```
    /// let app = pktui::App::arena_default().unwrap();
    /// assert_eq!(app.mode.label(), "Arena");
    /// ```
    pub fn arena_default() -> Result<Self> {
        Self::new(Command::Arena(ArenaArgs::default()))
    }

    /// Toggles the help overlay.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut app = pktui::App::play_default().unwrap();
    /// assert!(!app.help_visible);
    /// app.toggle_help();
    /// assert!(app.help_visible);
    /// ```
    pub fn toggle_help(&mut self) {
        self.help_visible = !self.help_visible;
    }

    /// Sets `should_quit`, breaking the main loop on the next iteration.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut app = pktui::App::play_default().unwrap();
    /// app.quit();
    /// assert!(app.should_quit);
    /// ```
    pub fn quit(&mut self) {
        self.should_quit = true;
    }
}

// ReplayArgs is only used inside `App::new`; this re-export silences the
// "unused import" warning in lib consumers who only touch `App`.
#[allow(unused_imports)]
use ReplayArgs as _ReplayArgs;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_default_builds() {
        let app = App::play_default().unwrap();
        assert_eq!(app.mode.label(), "Play");
        assert!(!app.should_quit);
    }

    #[test]
    fn arena_default_builds() {
        let app = App::arena_default().unwrap();
        assert_eq!(app.mode.label(), "Arena");
    }

    #[test]
    fn toggle_help_flips() {
        let mut app = App::play_default().unwrap();
        app.toggle_help();
        app.toggle_help();
        assert!(!app.help_visible);
    }

    #[test]
    fn quit_sets_flag() {
        let mut app = App::play_default().unwrap();
        app.quit();
        assert!(app.should_quit);
    }
}
