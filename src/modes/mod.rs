//! Per-mode state and initialisation.
//!
//! pktui runs in one of three modes, selected at startup via the CLI
//! subcommand:
//!
//! * [`play`] — one human vs eight bots.
//! * [`arena`] — nine bots, watch-only.
//! * [`replay`] — read a saved YAML hand collection street-by-street.
//!
//! Each submodule owns the mode-specific state. The top-level
//! [`AppMode`](crate::app::AppMode) enum then dispatches to the right one.

pub mod arena;
pub mod play;
pub mod replay;

pub use arena::ArenaState;
pub use play::{Awaiting, BetField, PlayState};
pub use replay::ReplayState;

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::SmallRng;

/// Builds the RNG used to seat bots and drive bot decisions.
///
/// Honours `seed` if `Some`, otherwise pulls 64 bits of OS entropy. Returning
/// the resolved seed (whether it was supplied or generated) lets the caller
/// log it so the user can reproduce the session later with `--seed N`.
///
/// # Examples
///
/// ```
/// use pktui::modes::seeded_rng;
/// let (_rng, seed) = seeded_rng(Some(42));
/// assert_eq!(seed, 42);
/// ```
#[must_use]
pub fn seeded_rng(seed: Option<u64>) -> (SmallRng, u64) {
    match seed {
        Some(s) => (SmallRng::seed_from_u64(s), s),
        None => {
            let s = rand::rng().random::<u64>();
            (SmallRng::seed_from_u64(s), s)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_seed_is_preserved() {
        let (_, seed) = seeded_rng(Some(1234));
        assert_eq!(seed, 1234);
    }

    #[test]
    fn missing_seed_yields_some_value() {
        let (_, seed) = seeded_rng(None);
        // Vacuously true but ensures the function returns.
        let _ = seed;
    }

    #[test]
    fn deterministic_for_same_seed() {
        use rand::Rng;
        let (mut a, _) = seeded_rng(Some(7));
        let (mut b, _) = seeded_rng(Some(7));
        let av: u64 = a.random();
        let bv: u64 = b.random();
        assert_eq!(av, bv);
    }
}
