//! The message-reducer half of the Elm-style loop.
//!
//! Every [`Event`] emitted by the runtime is translated
//! into one or more [`Msg`]s by [`event_to_msg`], then [`update`] applies the
//! `Msg` to the [`App`]. `update` is the single mutation point — the renderer
//! reads `App` immutably and the event loop only ever calls `update`.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use pkcore::casino::action::PlayerAction;

use crate::app::{App, AppMode};
use crate::error::Result;
use crate::event::Event;
use crate::modes::play::{Awaiting, HERO_SEAT};

/// Discrete intent derived from an [`Event`].
///
/// `Msg` is what the [`update`] reducer dispatches on. Keeping it separate
/// from raw [`Event`]s makes it easy to unit-test mode transitions without a
/// terminal — every test in this module constructs `Msg` values directly.
///
/// # Examples
///
/// ```
/// use pktui::update::Msg;
/// let m = Msg::Quit;
/// matches!(m, Msg::Quit);
/// ```
#[derive(Debug, Clone, Copy)]
pub enum Msg {
    /// Idle tick (no key) — drives bot pacing.
    Tick,
    /// User asked to quit.
    Quit,
    /// User pressed `?` to toggle the help overlay.
    ToggleHelp,
    /// User pressed `D` to dump the current Play state to a YAML file.
    Dump,
    /// Live modes: user picked a [`PlayerAction`].
    Action(PlayerAction),
    /// Play mode: start the next hand after `HandComplete`.
    NextHand,
    /// Bet-field manipulation (Play mode only).
    BetSet(usize),
    /// Bet-field bump up by N.
    BetBump(usize),
    /// Bet-field cut down by N.
    BetCut(usize),
    /// Bet-field append decimal digit.
    BetDigit(u8),
    /// Bet-field remove last digit.
    BetBackspace,
    /// Confirm bet/raise using the current bet-field value (Play mode).
    BetConfirm,
    /// Arena: speed up bots.
    ArenaFaster,
    /// Arena: slow down bots.
    ArenaSlower,
    /// Replay: cursor moves.
    ReplayNextHand,
    /// Replay: previous hand.
    ReplayPrevHand,
    /// Replay: next street within the current hand.
    ReplayNextStreet,
    /// Replay: previous street.
    ReplayPrevStreet,
    /// Spectate: freeze/unfreeze the live snapshot display.
    SpectateTogglePause,
    /// No-op (used for unrecognised keys so loops stay simple).
    Noop,
}

/// Lowers a raw [`Event`] to a [`Msg`].
///
/// Mode-aware: the same keystroke produces different `Msg`s depending on
/// whether Play, Arena or Replay is active.
///
/// # Examples
///
/// ```
/// use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
/// use pktui::event::Event;
/// use pktui::update::{event_to_msg, Msg};
/// use pktui::App;
///
/// let app = App::play_default().unwrap();
/// let k = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
/// let m = event_to_msg(&app, &Event::Key(k));
/// matches!(m, Msg::Quit);
/// ```
#[must_use]
pub fn event_to_msg(app: &App, event: &Event) -> Msg {
    match event {
        Event::Tick => Msg::Tick,
        Event::Resize(_, _) => Msg::Noop,
        Event::Key(k) => key_to_msg(app, k),
    }
}

fn key_to_msg(app: &App, key: &KeyEvent) -> Msg {
    // crossterm 0.29 surfaces Press / Repeat / Release separately on most
    // platforms. Ignore Release so a single press doesn't fire twice.
    if matches!(key.kind, KeyEventKind::Release) {
        return Msg::Noop;
    }

    // Universal: Ctrl+C / `q` always quits; `?` toggles help.
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Msg::Quit;
    }
    if matches!(key.code, KeyCode::Char('q')) {
        return Msg::Quit;
    }
    if matches!(key.code, KeyCode::Char('?')) {
        return Msg::ToggleHelp;
    }
    if matches!(key.code, KeyCode::Char('D')) {
        return Msg::Dump;
    }

    match &app.mode {
        AppMode::Play(p) => play_key(p.awaiting, key),
        AppMode::Arena(_) => arena_key(key),
        AppMode::Replay(_) => replay_key(key),
        AppMode::Spectate(_) => spectate_key(key),
    }
}

fn play_key(awaiting: Awaiting, key: &KeyEvent) -> Msg {
    use KeyCode::{Backspace, Char, Enter};
    match (awaiting, key.code) {
        // Between hands: Enter deals next.
        (Awaiting::HandComplete, Enter | Char(' ')) => Msg::NextHand,
        // Hero's turn: action hotkeys.
        (Awaiting::Human(seat), code) if seat == HERO_SEAT => match code {
            Char('f') => Msg::Action(PlayerAction::Fold),
            Char('k') => Msg::Action(PlayerAction::Check),
            Char('c') => Msg::Action(PlayerAction::Call),
            Char('a') => Msg::Action(PlayerAction::AllIn),
            Char('b' | 'r') | Enter => Msg::BetConfirm,
            Char('1') => Msg::BetSet(0), // sentinel: "min" — resolved in update
            Char('2') => Msg::BetSet(1), // sentinel: "half pot"
            Char('3') => Msg::BetSet(2), // sentinel: "pot"
            Char('+' | '=') => Msg::BetBump(50),
            Char('-' | '_') => Msg::BetCut(50),
            Char(c) if c.is_ascii_digit() => Msg::BetDigit((c as u8) - b'0'),
            Backspace => Msg::BetBackspace,
            _ => Msg::Noop,
        },
        _ => Msg::Noop,
    }
}

fn arena_key(key: &KeyEvent) -> Msg {
    use KeyCode::Char;
    match key.code {
        Char('+' | '=') => Msg::ArenaFaster,
        Char('-' | '_') => Msg::ArenaSlower,
        _ => Msg::Noop,
    }
}

fn replay_key(key: &KeyEvent) -> Msg {
    use KeyCode::{Char, Down, Enter, Left, Right, Up};
    match key.code {
        Char('n') | Right => Msg::ReplayNextStreet,
        Char('p') | Left => Msg::ReplayPrevStreet,
        Char('N' | ' ') | Enter | Down => Msg::ReplayNextHand,
        Char('P') | Up => Msg::ReplayPrevHand,
        _ => Msg::Noop,
    }
}

fn spectate_key(key: &KeyEvent) -> Msg {
    use KeyCode::Char;
    match key.code {
        Char(' ') => Msg::SpectateTogglePause,
        _ => Msg::Noop,
    }
}

/// Applies a [`Msg`] to the [`App`], producing the new model state.
///
/// Engine errors are recorded into the log but never propagated out — a
/// rejected action (e.g. under-min raise) should not crash the UI, the user
/// just needs to pick a different action.
///
/// # Errors
///
/// Returns [`crate::Error`] only for IO failures (e.g. config save). Engine
/// rejections are absorbed into the log.
///
/// # Examples
///
/// ```
/// use pktui::App;
/// use pktui::update::{update, Msg};
///
/// let mut app = App::play_default().unwrap();
/// update(&mut app, Msg::Quit).unwrap();
/// assert!(app.should_quit);
/// ```
pub fn update(app: &mut App, msg: Msg) -> Result<()> {
    match msg {
        Msg::Quit => app.quit(),
        Msg::ToggleHelp => app.toggle_help(),
        Msg::Dump => dump_play_state(app),
        Msg::Noop => {}
        Msg::Tick => match &mut app.mode {
            AppMode::Play(p) => {
                let _ = p.tick(&mut app.log);
            }
            AppMode::Arena(a) => {
                let _ = a.tick(&mut app.log);
            }
            AppMode::Replay(_) => {}
            AppMode::Spectate(_) => {}
        },
        Msg::Action(action) => {
            if let AppMode::Play(p) = &mut app.mode
                && let Err(e) = p.apply_human(action, &mut app.log)
            {
                app.log
                    .push(crate::log_panel::Severity::Error, format!("Rejected: {e}"));
            }
        }
        Msg::NextHand => {
            if let AppMode::Play(p) = &mut app.mode {
                let _ = p.next_hand(&mut app.log);
            }
        }
        Msg::BetSet(preset) => bet_preset(app, preset),
        Msg::BetBump(n) => {
            if let AppMode::Play(p) = &mut app.mode {
                p.bet.bump(n);
            }
        }
        Msg::BetCut(n) => {
            if let AppMode::Play(p) = &mut app.mode {
                p.bet.cut(n);
            }
        }
        Msg::BetDigit(d) => {
            if let AppMode::Play(p) = &mut app.mode {
                p.bet.push_digit(d);
            }
        }
        Msg::BetBackspace => {
            if let AppMode::Play(p) = &mut app.mode {
                p.bet.pop_digit();
            }
        }
        Msg::BetConfirm => {
            if let AppMode::Play(p) = &mut app.mode {
                let Awaiting::Human(seat) = p.awaiting else {
                    return Ok(());
                };
                let to_call = p.session.table.to_call(seat);
                let amount = p.bet.amount();
                let action = if to_call == 0 {
                    PlayerAction::Bet(amount)
                } else {
                    PlayerAction::Raise(amount)
                };
                if let Err(e) = p.apply_human(action, &mut app.log) {
                    app.log
                        .push(crate::log_panel::Severity::Error, format!("Rejected: {e}"));
                }
            }
        }
        Msg::ArenaFaster => {
            if let AppMode::Arena(a) = &mut app.mode {
                a.speed_up();
            }
        }
        Msg::ArenaSlower => {
            if let AppMode::Arena(a) = &mut app.mode {
                a.speed_down();
            }
        }
        Msg::ReplayNextHand => {
            if let AppMode::Replay(r) = &mut app.mode {
                r.next_hand();
            }
        }
        Msg::ReplayPrevHand => {
            if let AppMode::Replay(r) = &mut app.mode {
                r.prev_hand();
            }
        }
        Msg::ReplayNextStreet => {
            if let AppMode::Replay(r) = &mut app.mode {
                r.next_street();
            }
        }
        Msg::ReplayPrevStreet => {
            if let AppMode::Replay(r) = &mut app.mode {
                r.prev_street();
            }
        }
        Msg::SpectateTogglePause => {
            if let AppMode::Spectate(s) = &mut app.mode {
                s.paused = !s.paused;
            }
        }
    }
    Ok(())
}

fn dump_play_state(app: &mut App) {
    if let AppMode::Play(p) = &app.mode {
        match p.dump_state(&app.log) {
            Ok(path) => app.log.push(
                crate::log_panel::Severity::Info,
                format!("Dumped state to {}", path.display()),
            ),
            Err(e) => app.log.push(
                crate::log_panel::Severity::Error,
                format!("Dump failed: {e}"),
            ),
        }
    }
}

fn bet_preset(app: &mut App, preset: usize) {
    let AppMode::Play(p) = &mut app.mode else {
        return;
    };
    let Awaiting::Human(seat) = p.awaiting else {
        return;
    };
    let table = &p.session.table;
    let pot = table.effective_pot();
    let to_call = table.to_call(seat);
    let big_blind = table.forced.big_blind;
    let min = if to_call == 0 {
        big_blind
    } else {
        table.bet + table.min_raise()
    };
    // Fixed-limit has only one legal bet/raise amount per street, so every
    // preset key collapses to that amount — the ½pot / pot presets don't
    // make sense in this structure.
    let is_fixed_limit = matches!(
        table.betting,
        pkcore::games::betting_structure::BettingStructure::FixedLimit { .. }
    );
    let amount = if is_fixed_limit {
        min
    } else {
        match preset {
            1 => (pot / 2).max(min), // "half pot"
            2 => pot.max(min),       // "pot"
            _ => min,                // 0 = "min" + any out-of-range sentinel
        }
    };
    p.bet.set(amount);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::App;
    use crate::cli::PlayArgs;

    #[test]
    fn quit_sets_flag() {
        let mut app = App::play_default().unwrap();
        update(&mut app, Msg::Quit).unwrap();
        assert!(app.should_quit);
    }

    #[test]
    fn toggle_help_round_trip() {
        let mut app = App::play_default().unwrap();
        update(&mut app, Msg::ToggleHelp).unwrap();
        assert!(app.help_visible);
        update(&mut app, Msg::ToggleHelp).unwrap();
        assert!(!app.help_visible);
    }

    #[test]
    fn tick_in_play_does_not_panic() {
        let mut app = App::play_default().unwrap();
        for _ in 0..50 {
            update(&mut app, Msg::Tick).unwrap();
        }
    }

    #[test]
    fn key_q_maps_to_quit() {
        let app = App::play_default().unwrap();
        let k = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let m = event_to_msg(&app, &Event::Key(k));
        assert!(matches!(m, Msg::Quit));
    }

    #[test]
    fn key_ctrl_c_maps_to_quit() {
        let app = App::play_default().unwrap();
        let k = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let m = event_to_msg(&app, &Event::Key(k));
        assert!(matches!(m, Msg::Quit));
    }

    #[test]
    fn question_mark_toggles_help() {
        let app = App::play_default().unwrap();
        let k = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
        let m = event_to_msg(&app, &Event::Key(k));
        assert!(matches!(m, Msg::ToggleHelp));
    }

    #[test]
    fn arena_plus_speeds_up() {
        let mut app = App::arena_default().unwrap();
        let k = KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE);
        let m = event_to_msg(&app, &Event::Key(k));
        assert!(matches!(m, Msg::ArenaFaster));
        update(&mut app, m).unwrap();
    }

    #[test]
    fn space_in_spectate_toggles_pause() {
        let cmd = crate::cli::Command::Spectate(crate::cli::SpectateArgs {
            endpoint: "http://localhost:1".to_string(),
        });
        let app = App::new(cmd).unwrap();
        let k = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        let m = event_to_msg(&app, &Event::Key(k));
        assert!(matches!(m, Msg::SpectateTogglePause));
    }

    #[test]
    fn play_seed_42_does_not_panic_on_many_ticks() {
        let mut args = PlayArgs::default();
        args.game.seed = Some(42);
        let mut app = App::new(crate::cli::Command::Play(args)).unwrap();
        for _ in 0..500 {
            if let AppMode::Play(p) = &app.mode
                && matches!(p.awaiting, Awaiting::Human(_) | Awaiting::SessionOver)
            {
                break;
            }
            update(&mut app, Msg::Tick).unwrap();
        }
    }
}
