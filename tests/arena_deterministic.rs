//! End-to-end smoke test for Arena mode.
//!
//! Two things we can assert deterministically:
//!
//! * The bot roster pktui seats at a given seed is reproducible (pktui owns
//!   the shuffle for `BotProfile::default_profiles()`).
//! * A whole hand runs to completion in a bounded number of ticks without
//!   panicking.
//!
//! We do NOT assert that the action sequence is identical across runs of
//! the same seed, because `pkcore`'s deck shuffle uses `rand::thread_rng()`
//! internally — that's an engine concern, not a UI concern.

use std::time::Duration;

use pktui::cli::ArenaArgs;
use pktui::log_panel::LogPanel;
use pktui::modes::ArenaState;
use pktui::modes::arena::ArenaPhase;

fn make(seed: u64) -> (ArenaState, LogPanel) {
    let mut log = LogPanel::new();
    let mut args = ArenaArgs::default();
    args.game.seed = Some(seed);
    let mut state = ArenaState::new(&args, &mut log).expect("init arena");
    state.speed = Duration::from_millis(0);
    (state, log)
}

#[test]
fn bot_lineup_is_reproducible_for_same_seed() {
    let (a, _) = make(20_260_514);
    let (b, _) = make(20_260_514);
    let names_a: Vec<String> = a.bots.iter().map(|b| b.name.clone()).collect();
    let names_b: Vec<String> = b.bots.iter().map(|b| b.name.clone()).collect();
    assert_eq!(
        names_a, names_b,
        "Same seed must seat the same 9 bots in the same order"
    );
    assert_eq!(names_a.len(), 9);
}

#[test]
fn arena_runs_a_hand_to_completion() {
    let (mut state, mut log) = make(7);
    for _ in 0..2000 {
        if !matches!(state.phase, ArenaPhase::Running) {
            break;
        }
        let _ = state.tick(&mut log);
    }
    // After 2000 ticks one hand must have completed (or the whole session
    // ended). If we're still "Running" the engine is stuck — fail loudly.
    assert!(
        !matches!(state.phase, ArenaPhase::Running),
        "expected hand to complete within 2000 ticks; log so far:\n{:?}",
        log.iter().map(|l| l.text.clone()).collect::<Vec<_>>()
    );
    assert!(log.iter().any(|l| l.text.contains("wins")));
}
