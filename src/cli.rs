//! Command-line interface (clap-derive).
//!
//! `pktui` exposes one binary with three subcommands that map to the three
//! UI modes:
//!
//! ```text
//! pktui play   [--variant nlhe|stud-hi] [--seed N] [--blinds 50/100] [--chips 10000]
//!              [--ante N] [--bring-in N] [--small-bet N] [--big-bet N]
//! pktui arena  [--variant nlhe|stud-hi] [--seed N] [--blinds 50/100] [--chips 10000]
//!              [--ante N] [--bring-in N] [--small-bet N] [--big-bet N] [--speed-ms 800]
//! pktui replay <FILE>
//! ```
//!
//! Default subcommand (no args) is `play`. Default variant is `nlhe`. For
//! stud-family variants, `--ante` / `--bring-in` / `--small-bet` / `--big-bet`
//! apply; for hold'em-family variants, `--small-blind` / `--big-blind` apply.
//! Irrelevant forced-bet flags are ignored for the chosen variant.

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Poker variant the engine will run.
///
/// Currently exposed: No-Limit Hold'em (default), Pot-Limit Omaha, Seven-Card
/// Stud Hi, and Razz. FLHE exists in pkcore but isn't wired to the CLI yet —
/// adding it is a one-line enum extension plus a match arm in
/// [`crate::modes`].
///
/// # Examples
///
/// ```
/// use pktui::cli::Variant;
/// assert_eq!(Variant::default(), Variant::Nlhe);
/// ```
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    /// No-Limit Hold'em — the historical default.
    #[default]
    Nlhe,
    /// Pot-Limit Omaha. 4 hole cards, community board, pot-limit bet sizing.
    Plo,
    /// Seven-Card Stud Hi. UI rendering is preliminary (the table/replay
    /// views still assume the hold'em 4-street + 5-card-board shape).
    StudHi,
    /// Seven-Card Stud lowball (A-5 evaluator). UI rendering is preliminary —
    /// same caveats as `StudHi`.
    Razz,
}

impl Variant {
    /// Maximum number of seats supported by this variant.
    ///
    /// Stud-family games deal 7 cards per player. The 52-card deck would
    /// technically seat 8 (8 × 7 = 56 with a card-recycle), but pktui caps
    /// at 6 to keep the table playable and the deck comfortable.
    /// NLHE / PLO use community cards, so they comfortably seat 9.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::cli::Variant;
    /// assert_eq!(Variant::Nlhe.max_seats(), 9);
    /// assert_eq!(Variant::Plo.max_seats(), 9);
    /// assert_eq!(Variant::StudHi.max_seats(), 6);
    /// assert_eq!(Variant::Razz.max_seats(), 6);
    /// ```
    #[must_use]
    pub fn max_seats(self) -> usize {
        match self {
            Self::Nlhe | Self::Plo => 9,
            Self::StudHi | Self::Razz => 6,
        }
    }
}

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
    /// Poker variant to deal. Defaults to NLHE.
    #[arg(long, value_enum, default_value_t = Variant::Nlhe)]
    pub variant: Variant,
    /// Override the RNG seed (useful for reproducible sessions and tests).
    #[arg(long)]
    pub seed: Option<u64>,
    /// Small blind in chips (hold'em-family variants only).
    #[arg(long, default_value_t = 50)]
    pub small_blind: usize,
    /// Big blind in chips (hold'em-family variants only).
    #[arg(long, default_value_t = 100)]
    pub big_blind: usize,
    /// Starting stack per seat.
    #[arg(long, default_value_t = 10_000)]
    pub chips: usize,
    /// Ante per seat (stud-family variants only).
    #[arg(long)]
    pub ante: Option<usize>,
    /// Bring-in (stud-family variants only).
    #[arg(long)]
    pub bring_in: Option<usize>,
    /// Small bet for fixed-limit games (falls back to `--small-blind`).
    #[arg(long)]
    pub small_bet: Option<usize>,
    /// Big bet for fixed-limit games (falls back to `--big-blind`).
    #[arg(long)]
    pub big_bet: Option<usize>,
}

impl Default for GameArgs {
    fn default() -> Self {
        Self {
            variant: Variant::Nlhe,
            seed: None,
            small_blind: 50,
            big_blind: 100,
            chips: 10_000,
            ante: None,
            bring_in: None,
            small_bet: None,
            big_bet: None,
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

    #[test]
    fn play_variant_default_is_nlhe() {
        let cli = Cli::try_parse_from(["pktui", "play"]).unwrap();
        match cli.resolved() {
            Command::Play(p) => assert_eq!(p.game.variant, Variant::Nlhe),
            _ => panic!(),
        }
    }

    #[test]
    fn parses_play_variant_stud_hi() {
        let cli = Cli::try_parse_from(["pktui", "play", "--variant", "stud-hi"]).unwrap();
        match cli.resolved() {
            Command::Play(p) => assert_eq!(p.game.variant, Variant::StudHi),
            _ => panic!(),
        }
    }

    #[test]
    fn parses_arena_variant_stud_hi() {
        let cli = Cli::try_parse_from(["pktui", "arena", "--variant", "stud-hi"]).unwrap();
        match cli.resolved() {
            Command::Arena(a) => assert_eq!(a.game.variant, Variant::StudHi),
            _ => panic!(),
        }
    }

    #[test]
    fn parses_stud_hi_with_ante_bring_in() {
        let cli = Cli::try_parse_from([
            "pktui",
            "play",
            "--variant",
            "stud-hi",
            "--ante",
            "5",
            "--bring-in",
            "15",
            "--small-bet",
            "25",
            "--big-bet",
            "50",
        ])
        .unwrap();
        match cli.resolved() {
            Command::Play(p) => {
                assert_eq!(p.game.variant, Variant::StudHi);
                assert_eq!(p.game.ante, Some(5));
                assert_eq!(p.game.bring_in, Some(15));
                assert_eq!(p.game.small_bet, Some(25));
                assert_eq!(p.game.big_bet, Some(50));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn nlhe_defaults_leave_stud_fields_none() {
        let cli = Cli::try_parse_from(["pktui", "play"]).unwrap();
        match cli.resolved() {
            Command::Play(p) => {
                assert_eq!(p.game.ante, None);
                assert_eq!(p.game.bring_in, None);
                assert_eq!(p.game.small_bet, None);
                assert_eq!(p.game.big_bet, None);
            }
            _ => panic!(),
        }
    }
}
