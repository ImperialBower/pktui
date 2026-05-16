//! Command-line interface (clap-derive).
//!
//! `pktui` exposes one binary with three subcommands that map to the three
//! UI modes:
//!
//! ```text
//! pktui play   [--seed N] [--blinds 50/100] [--chips 10000]
//! pktui arena  [--seed N] [--blinds 50/100] [--chips 10000] [--speed-ms 800]
//! pktui replay <FILE>
//! ```
//!
//! Default subcommand (no args) is `play`.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// Top-level CLI definition.
///
/// # Examples
///
/// ```
/// use clap::Parser;
/// use pktui::cli::Cli;
///
/// let cli = Cli::try_parse_from(["pktui"]).unwrap();
/// assert!(cli.command.is_none());
/// ```
#[derive(Parser, Debug)]
#[command(name = "pktui", about = "Ratatui poker table for the pkcore engine.")]
pub struct Cli {
    /// Which mode to launch. Defaults to `play`.
    #[command(subcommand)]
    pub command: Option<Command>,
}

impl Cli {
    /// Returns the chosen [`Command`], defaulting to `Play` with default args.
    ///
    /// # Examples
    ///
    /// ```
    /// use clap::Parser;
    /// use pktui::cli::{Cli, Command};
    ///
    /// let cli = Cli::try_parse_from(["pktui"]).unwrap();
    /// assert!(matches!(cli.resolved(), Command::Play(_)));
    /// ```
    #[must_use]
    pub fn resolved(self) -> Command {
        self.command.unwrap_or(Command::Play(PlayArgs::default()))
    }
}

/// A subcommand selects which mode the TUI starts in.
#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// One human (seat 0) vs eight bots.
    Play(PlayArgs),
    /// Nine bots, watch-only.
    Arena(ArenaArgs),
    /// Replay a saved YAML hand collection street-by-street.
    Replay(ReplayArgs),
}

/// Common knobs shared across `play` and `arena`.
///
/// Note: the [`Default`] impl is hand-written to match clap's `default_value_t`
/// attributes, because `#[derive(Default)]` would zero every field. A
/// `chips: 0` table will be rejected by the engine with `InsufficientChips`,
/// which is the surprise we'd otherwise spend an afternoon debugging.
#[derive(Args, Debug, Clone)]
pub struct GameArgs {
    /// Override the RNG seed (useful for reproducible sessions and tests).
    #[arg(long)]
    pub seed: Option<u64>,
    /// Small blind in chips.
    #[arg(long, default_value_t = 50)]
    pub small_blind: usize,
    /// Big blind in chips.
    #[arg(long, default_value_t = 100)]
    pub big_blind: usize,
    /// Starting stack per seat.
    #[arg(long, default_value_t = 10_000)]
    pub chips: usize,
}

impl Default for GameArgs {
    fn default() -> Self {
        Self {
            seed: None,
            small_blind: 50,
            big_blind: 100,
            chips: 10_000,
        }
    }
}

/// Arguments to the `play` subcommand.
///
/// # Examples
///
/// ```
/// use pktui::cli::PlayArgs;
/// let args = PlayArgs::default();
/// assert_eq!(args.game.big_blind, 100);
/// ```
#[derive(Args, Debug, Clone, Default)]
pub struct PlayArgs {
    /// Game knobs (blinds / starting chips / seed).
    #[command(flatten)]
    pub game: GameArgs,
}

/// Arguments to the `arena` subcommand.
///
/// # Examples
///
/// ```
/// use pktui::cli::ArenaArgs;
/// let args = ArenaArgs::default();
/// assert_eq!(args.speed_ms, 800);
/// ```
#[derive(Args, Debug, Clone)]
pub struct ArenaArgs {
    /// Game knobs (blinds / starting chips / seed).
    #[command(flatten)]
    pub game: GameArgs,
    /// Milliseconds between bot actions (lower = faster).
    #[arg(long, default_value_t = 800)]
    pub speed_ms: u64,
}

impl Default for ArenaArgs {
    fn default() -> Self {
        Self {
            game: GameArgs::default(),
            speed_ms: 800,
        }
    }
}

/// Arguments to the `replay` subcommand.
///
/// # Examples
///
/// ```
/// use pktui::cli::ReplayArgs;
/// use std::path::PathBuf;
/// let args = ReplayArgs { path: PathBuf::from("session.yaml") };
/// assert_eq!(args.path.to_str(), Some("session.yaml"));
/// ```
#[derive(Args, Debug, Clone)]
pub struct ReplayArgs {
    /// Path to a `HandCollection` YAML file (as produced by pkcore's
    /// `interactive_play` example or pktui's own session save).
    pub path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_no_subcommand() {
        let cli = Cli::try_parse_from(["pktui"]).unwrap();
        assert!(cli.command.is_none());
    }

    #[test]
    fn parses_play() {
        let cli = Cli::try_parse_from(["pktui", "play"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Play(_))));
    }

    #[test]
    fn parses_arena_with_speed() {
        let cli = Cli::try_parse_from(["pktui", "arena", "--speed-ms", "200"]).unwrap();
        match cli.resolved() {
            Command::Arena(a) => assert_eq!(a.speed_ms, 200),
            _ => panic!("expected arena"),
        }
    }

    #[test]
    fn parses_replay_path() {
        let cli = Cli::try_parse_from(["pktui", "replay", "x.yaml"]).unwrap();
        match cli.resolved() {
            Command::Replay(r) => assert_eq!(r.path.to_str().unwrap(), "x.yaml"),
            _ => panic!("expected replay"),
        }
    }

    #[test]
    fn play_default_blinds() {
        let cli = Cli::try_parse_from(["pktui", "play"]).unwrap();
        match cli.resolved() {
            Command::Play(p) => {
                assert_eq!(p.game.small_blind, 50);
                assert_eq!(p.game.big_blind, 100);
                assert_eq!(p.game.chips, 10_000);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn play_seed_override() {
        let cli = Cli::try_parse_from(["pktui", "play", "--seed", "42"]).unwrap();
        match cli.resolved() {
            Command::Play(p) => assert_eq!(p.game.seed, Some(42)),
            _ => panic!(),
        }
    }

    #[test]
    fn resolved_defaults_to_play() {
        let cli = Cli::try_parse_from(["pktui"]).unwrap();
        assert!(matches!(cli.resolved(), Command::Play(_)));
    }
}
