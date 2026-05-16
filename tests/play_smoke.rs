//! Smoke test: build a Play app, drive it via `Msg` until the hero has to
//! act, then fold and confirm the engine accepts it.
//!
//! Acts as a coarse integration check that the whole event → message →
//! update → engine pipeline is wired together.

use pkcore::casino::action::PlayerAction;
use pktui::App;
use pktui::app::AppMode;
use pktui::cli::{Command, PlayArgs};
use pktui::modes::Awaiting;
use pktui::update::{Msg, update};

#[test]
fn fold_when_human_to_act_advances_state() {
    let mut args = PlayArgs::default();
    args.game.seed = Some(42);
    let mut app = App::new(Command::Play(args)).expect("init");

    if let AppMode::Play(p) = &mut app.mode {
        p.speed = std::time::Duration::from_millis(0);
    }

    // Step until either the hero must act or the hand resolves without
    // touching the hero.
    let mut steps = 0;
    loop {
        if let AppMode::Play(p) = &app.mode {
            match p.awaiting {
                Awaiting::Human(_) | Awaiting::HandComplete | Awaiting::SessionOver => break,
                Awaiting::Bot => {}
            }
        }
        update(&mut app, Msg::Tick).unwrap();
        steps += 1;
        assert!(steps < 1000, "did not reach a terminal state in time");
    }

    // If the hero has to act, send a Fold and verify state advances.
    let hero_to_act = matches!(
        &app.mode,
        AppMode::Play(p) if matches!(p.awaiting, Awaiting::Human(_)),
    );
    if hero_to_act {
        update(&mut app, Msg::Action(PlayerAction::Fold)).unwrap();
        if let AppMode::Play(p) = &app.mode {
            assert!(matches!(
                p.awaiting,
                Awaiting::Bot | Awaiting::HandComplete | Awaiting::SessionOver
            ));
        }
    }
}
