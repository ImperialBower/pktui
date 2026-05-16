//! Verifies that when a hand reaches showdown with 2+ active players,
//! [`PlayState::last_showdown`] is populated **before** `end_hand()` resets
//! the table — so the renderer can show every active hand to the user, even
//! ones the hero folded out of.

use std::time::Duration;

use pktui::App;
use pktui::app::AppMode;
use pktui::cli::{Command, PlayArgs};
use pktui::modes::Awaiting;
use pktui::update::{Msg, update};

#[test]
fn multiway_showdown_populates_last_showdown_snapshot() {
    // Sweep a handful of seeds — the engine's internal deck shuffle uses
    // thread_rng so we can't pin a specific multiway-river outcome. With
    // 32 attempts of 200 ticks each, hitting at least one multiway
    // showdown is statistically certain.
    let mut saw_multiway_showdown = false;

    'outer: for seed in 1..=32u64 {
        let mut args = PlayArgs::default();
        args.game.seed = Some(seed);
        let mut app = App::new(Command::Play(args)).expect("init");
        if let AppMode::Play(p) = &mut app.mode {
            p.speed = Duration::from_millis(0);
        }

        // If it's the hero's turn, fold immediately so the bots play it out.
        for _ in 0..200 {
            if let AppMode::Play(p) = &app.mode {
                match p.awaiting {
                    Awaiting::Human(_) => {
                        update(
                            &mut app,
                            Msg::Action(pkcore::casino::action::PlayerAction::Fold),
                        )
                        .unwrap();
                    }
                    Awaiting::HandComplete | Awaiting::SessionOver => break,
                    Awaiting::Bot => {
                        update(&mut app, Msg::Tick).unwrap();
                    }
                }
            }
        }

        if let AppMode::Play(p) = &app.mode
            && let Some(showdown) = &p.last_showdown
        {
            // Snapshot must hold at least 2 seats (the definition of
            // showdown) and each seat must have non-empty hole cards.
            assert!(
                showdown.len() >= 2,
                "showdown snapshot must include 2+ seats, got {}",
                showdown.len()
            );
            for row in showdown {
                assert!(
                    !row.hole.is_empty(),
                    "every showdown seat must have hole cards"
                );
            }
            saw_multiway_showdown = true;
            break 'outer;
        }
    }

    assert!(
        saw_multiway_showdown,
        "in 32 hands no multiway showdown was reached — engine may be wrong or fold rate too high"
    );
}

#[test]
fn next_hand_clears_showdown_snapshot() {
    let mut args = PlayArgs::default();
    args.game.seed = Some(7);
    let mut app = App::new(Command::Play(args)).expect("init");
    if let AppMode::Play(p) = &mut app.mode {
        p.speed = Duration::from_millis(0);
        // Inject a fake showdown snapshot so we don't depend on a hand
        // actually reaching showdown for this assertion.
        p.last_showdown = Some(vec![pktui::modes::play::ShowdownSeat {
            seat: 1,
            name: "fake".into(),
            hole: "Ah Kh".into(),
            hand_class: None,
        }]);
        // Pretend the hand just ended.
        p.awaiting = Awaiting::HandComplete;
    }
    update(&mut app, Msg::NextHand).unwrap();
    if let AppMode::Play(p) = &app.mode {
        assert!(
            p.last_showdown.is_none(),
            "next_hand must clear the previous showdown snapshot"
        );
    }
}
