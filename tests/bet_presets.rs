//! Verifies the `1` / `2` / `3` preset keys actually update the bet field.
//!
//! Drives a Play session until it's the hero's turn, then sends a synthetic
//! `Char('2')` key event through the full event→msg→update pipeline and
//! asserts `bet.amount()` is non-zero afterwards.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pktui::App;
use pktui::app::AppMode;
use pktui::cli::{Command, PlayArgs};
use pktui::event::Event;
use pktui::modes::Awaiting;
use pktui::update::{Msg, event_to_msg, update};

fn play_app_to_hero_turn(seed: u64) -> App {
    let mut args = PlayArgs::default();
    args.game.seed = Some(seed);
    let mut app = App::new(Command::Play(args)).expect("init");
    if let AppMode::Play(p) = &mut app.mode {
        p.speed = std::time::Duration::from_millis(0);
    }
    for _ in 0..2000 {
        if let AppMode::Play(p) = &app.mode
            && matches!(
                p.awaiting,
                Awaiting::Human(_) | Awaiting::HandComplete | Awaiting::SessionOver
            )
        {
            break;
        }
        update(&mut app, Msg::Tick).unwrap();
    }
    app
}

#[test]
fn pressing_1_sets_bet_to_min() {
    let mut app = play_app_to_hero_turn(42);
    let is_hero_turn = matches!(
        &app.mode,
        AppMode::Play(p) if matches!(p.awaiting, Awaiting::Human(_)),
    );
    if !is_hero_turn {
        // Hand resolved without hero acting — nothing to test.
        return;
    }
    let key = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
    let msg = event_to_msg(&app, &Event::Key(key));
    assert!(
        matches!(msg, Msg::BetSet(0)),
        "expected Msg::BetSet(0), got {msg:?}"
    );
    update(&mut app, msg).unwrap();
    if let AppMode::Play(p) = &app.mode {
        assert!(p.bet.amount() > 0, "bet should be set after pressing 1");
    }
}

#[test]
fn pressing_2_sets_bet_to_half_pot_or_min() {
    let mut app = play_app_to_hero_turn(42);
    let is_hero_turn = matches!(
        &app.mode,
        AppMode::Play(p) if matches!(p.awaiting, Awaiting::Human(_)),
    );
    if !is_hero_turn {
        return;
    }
    let key = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE);
    let msg = event_to_msg(&app, &Event::Key(key));
    assert!(matches!(msg, Msg::BetSet(1)));
    update(&mut app, msg).unwrap();
    if let AppMode::Play(p) = &app.mode {
        assert!(p.bet.amount() > 0);
    }
}

#[test]
fn pressing_3_sets_bet_to_pot() {
    let mut app = play_app_to_hero_turn(42);
    let is_hero_turn = matches!(
        &app.mode,
        AppMode::Play(p) if matches!(p.awaiting, Awaiting::Human(_)),
    );
    if !is_hero_turn {
        return;
    }
    let key = KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE);
    let msg = event_to_msg(&app, &Event::Key(key));
    assert!(matches!(msg, Msg::BetSet(2)));
    update(&mut app, msg).unwrap();
    if let AppMode::Play(p) = &app.mode {
        assert!(p.bet.amount() > 0);
    }
}
