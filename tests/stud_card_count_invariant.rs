//! Regression check: in a Stud Hi session, every in-hand seat should always
//! have the same number of dealt cards at any given moment (the engine
//! deals one card per active seat per street). pktui observed hero showing
//! one fewer card than opponents during 5th-street betting; this test
//! drives a deterministic session and asserts the invariant.

use pkcore::bot::profile::BotProfile;
use pkcore::casino::action::PlayerAction;
use pkcore::casino::game::ForcedBets;
use pkcore::casino::session::{PokerSession, SessionStep};
use pkcore::casino::table_no_cell::{PlayerNoCell, SeatNoCell, SeatsNoCell, TableNoCell};
use pktui::cli::{PlayArgs, Variant};
use pktui::log_panel::LogPanel;
use pktui::modes::{Awaiting, PlayState};
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;

fn make_session(stacks: usize) -> PokerSession {
    let names = [
        "You",
        "abc",
        "tight_aggressive",
        "loose_passive",
        "gto",
        "short_stack_ninja",
        "joker",
        "maniac",
        "loose_aggressive",
    ];
    let seats: Vec<SeatNoCell> = names
        .iter()
        .map(|n| SeatNoCell::new(PlayerNoCell::new_with_chips((*n).to_string(), stacks)))
        .collect();
    let _ = ForcedBets::new(0, 0);
    let table = TableNoCell::stud_hi_from_seats(SeatsNoCell::new(seats), 10, 25, 50, 100);
    let mut session = PokerSession::new(table);
    session.start_hand().unwrap();
    session
}

fn dealt_counts(session: &PokerSession) -> Vec<(u8, usize, usize)> {
    // (seat_idx, cards_dealt, hand_len)
    (0..session.table.seats.0.len())
        .filter_map(|i| {
            let i_u8 = u8::try_from(i).ok()?;
            let seat = session.table.seats.get_seat(i_u8)?;
            if seat.is_empty() || !seat.is_in_hand() {
                return None;
            }
            let dealt = seat.cards.number_of_dealt_cards();
            let hand = seat.hand.len();
            Some((i_u8, dealt, hand))
        })
        .collect()
}

/// Asserts the same invariant at EVERY step (not just after street advances).
/// If pkcore deals 6th-street cards to non-acting seats before pausing for
/// the to-act seat, this will catch it.
fn assert_counts_balanced(session: &PokerSession, label: &str) {
    let counts = dealt_counts(session);
    if let Some(&(seat0, c0, _h0)) = counts.first() {
        for &(seat_i, cards_i, hand_i) in &counts {
            assert_eq!(
                cards_i, c0,
                "[{label}] seat {seat_i} cards.dealt={cards_i} but seat {seat0} cards.dealt={c0} \
                 at phase {:?}; full counts: {counts:?}",
                session.table.phase
            );
            assert_eq!(
                cards_i, hand_i,
                "[{label}] seat {seat_i} cards.dealt={cards_i} but hand.len={hand_i} \
                 at phase {:?}",
                session.table.phase
            );
        }
    }
}

#[test]
fn hero_has_same_card_count_as_opponents_at_every_to_act_moment() {
    // Mirror the screenshot scenario: hero is button (seat 0), 9 seats,
    // bots raise occasionally on each street. At every PlayerToAct moment,
    // assert hero and opponents have the same card count.
    let mut session = make_session(10_000);
    let mut step = 0;
    let mut raise_counter = 0;
    while step < 500 {
        step += 1;
        match session.next_step() {
            SessionStep::PlayerToAct(seat) => {
                assert_counts_balanced(&session, &format!("PlayerToAct({seat})"));
                let to_call = session.table.to_call(seat);
                // Bots raise every 3rd action; hero always calls/checks.
                let action = if seat == 0 {
                    if to_call == 0 {
                        PlayerAction::Check
                    } else {
                        PlayerAction::Call
                    }
                } else {
                    raise_counter += 1;
                    if raise_counter % 3 == 0 && to_call > 0 {
                        // Raise by big-bet (100) on top of current bet.
                        PlayerAction::Raise(session.table.bet + 100)
                    } else if to_call == 0 {
                        // First to act: bet small-bet or big-bet.
                        PlayerAction::Bet(100)
                    } else {
                        PlayerAction::Call
                    }
                };
                if session.apply_action(seat, action).is_err() {
                    // Fall back to call/check on legal-action errors.
                    let fallback = if to_call == 0 {
                        PlayerAction::Check
                    } else {
                        PlayerAction::Call
                    };
                    if session.apply_action(seat, fallback).is_err() {
                        break;
                    }
                }
            }
            SessionStep::StreetAdvanced => {
                assert_counts_balanced(&session, "StreetAdvanced");
            }
            SessionStep::HandComplete => return,
        }
    }
}

/// Reproduces the pktui scenario from the bug report: 9 bot-driven seats
/// playing Stud Hi with the seed used in the screenshot, asserting the
/// card-count invariant at every `PlayerToAct`.
#[test]
fn bot_driven_session_keeps_seat_card_counts_balanced() {
    // Mirror pktui's setup: 1 hero + 8 bots, bot pool shuffled with the
    // same RNG that's later used for bot decisions. Seat 0 = hero (no
    // bot); seats 1-8 = shuffled bots.
    let mut session = make_session(10_000);
    let mut rng = SmallRng::seed_from_u64(3_596_220_112_812_468_068);
    let mut pool = BotProfile::default_profiles();
    pool.push(BotProfile::joker());
    pool.shuffle(&mut rng);
    let bots: Vec<BotProfile> = pool.into_iter().take(8).collect();
    assert_eq!(bots.len(), 8);
    let mut step = 0;
    while step < 1000 {
        step += 1;
        match session.next_step() {
            SessionStep::PlayerToAct(seat) => {
                assert_counts_balanced(&session, &format!("PlayerToAct({seat}) step={step}"));
                let action = if seat == 0 {
                    // Hero: just call/check (mirrors a passive human).
                    if session.table.to_call(seat) == 0 {
                        PlayerAction::Check
                    } else {
                        PlayerAction::Call
                    }
                } else {
                    let bot_idx = (seat as usize) - 1;
                    bots[bot_idx].decide(&session.table, seat, &mut rng)
                };
                if session.apply_action(seat, action).is_err() {
                    let _ = session.apply_action(seat, PlayerAction::Fold);
                }
            }
            SessionStep::StreetAdvanced => {
                assert_counts_balanced(&session, &format!("StreetAdvanced step={step}"));
            }
            SessionStep::HandComplete => return,
        }
    }
}

/// Drives a real `PlayState` (the same struct pktui's binary uses) through
/// a Stud Hi session. Asserts the card-count invariant after every
/// successful tick (the same boundary at which the UI would re-render).
#[test]
fn playstate_tick_loop_keeps_card_counts_balanced() {
    let mut args = PlayArgs::default();
    args.game.variant = Variant::StudHi;
    args.game.seed = Some(3_596_220_112_812_468_068);
    let mut log = LogPanel::new();
    let mut state = PlayState::new(&args, &mut log).unwrap();
    state.speed = std::time::Duration::from_millis(0);
    state.last_step_at = std::time::Instant::now()
        .checked_sub(std::time::Duration::from_secs(10))
        .unwrap_or_else(std::time::Instant::now);

    let mut max_phase: Option<pkcore::games::GamePhase> = None;
    let mut hero_turns = 0u32;
    let mut just_acted_as_hero = false;
    for step in 0..2000 {
        let did_progress = match state.tick(&mut log) {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "step {step}: tick err {e:?}; phase={:?}",
                    state.session.table.phase
                );
                // Treat as "progress" so we continue; pktui would surface
                // the error in production. For invariant-checking we just
                // want to keep advancing.
                break;
            }
        };
        let counts = dealt_counts(&state.session);
        let current_phase = state.session.table.phase;
        if max_phase != Some(current_phase) {
            max_phase = Some(current_phase);
            eprintln!(
                "step {step}: phase {current_phase:?}, awaiting {:?}, counts {counts:?}",
                state.awaiting
            );
        }
        if let Some(&(seat0, c0, _)) = counts.first() {
            for &(seat_i, cards_i, hand_i) in &counts {
                assert_eq!(
                    cards_i, c0,
                    "[playstate step={step}] seat {seat_i} dealt={cards_i} but seat {seat0} \
                     dealt={c0}; phase={:?}; awaiting={:?}; full counts: {counts:?}",
                    state.session.table.phase, state.awaiting,
                );
                assert_eq!(
                    cards_i, hand_i,
                    "[playstate step={step}] seat {seat_i} cards.dealt={cards_i} != hand.len={hand_i}; \
                     phase={:?}",
                    state.session.table.phase,
                );
            }
        }
        match state.awaiting {
            Awaiting::Human(0) => {
                hero_turns += 1;
                eprintln!(
                    "step {step}: HERO TURN #{hero_turns} on {current_phase:?}, counts {counts:?}",
                );
                let to_call = state.session.table.to_call(0);
                let action = if to_call == 0 {
                    PlayerAction::Check
                } else {
                    PlayerAction::Call
                };
                if state.session.apply_action(0, action).is_err() {
                    let _ = state.session.apply_action(0, PlayerAction::Fold);
                }
                state.awaiting = Awaiting::Bot;
                just_acted_as_hero = true;
            }
            Awaiting::HandComplete | Awaiting::SessionOver => {
                eprintln!("step {step}: hand/session over on {current_phase:?}");
                return;
            }
            _ => {}
        }
        state.last_step_at = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(10))
            .unwrap_or_else(std::time::Instant::now);
        if !did_progress && !just_acted_as_hero {
            eprintln!(
                "step {step}: did_progress=false; awaiting={:?}; phase={current_phase:?}",
                state.awaiting
            );
            break;
        }
        just_acted_as_hero = false;
    }
}

#[test]
fn every_in_hand_seat_has_same_card_count_each_street() {
    let mut session = make_session(10_000);
    // Drive 200 steps max; have every player call/check whenever it's their turn.
    for _ in 0..200 {
        match session.next_step() {
            SessionStep::PlayerToAct(seat) => {
                let to_call = session.table.to_call(seat);
                let action = if to_call == 0 {
                    PlayerAction::Check
                } else {
                    PlayerAction::Call
                };
                if session.apply_action(seat, action).is_err() {
                    // Bring-in / forced actions may need different action; try Bet/MinRaise
                    // fallbacks. For now, break the loop if we hit an illegal action.
                    break;
                }
            }
            SessionStep::StreetAdvanced => {
                // After every street advance, every in-hand seat must have
                // the same dealt-card count.
                let counts = dealt_counts(&session);
                if let Some(&(seat0, c0, h0)) = counts.first() {
                    for &(seat_i, cards_i, hand_i) in &counts {
                        assert_eq!(
                            cards_i, c0,
                            "seat {seat_i} has {cards_i} dealt cards but seat {seat0} has {c0} \
                             at phase {:?}; full counts: {counts:?}",
                            session.table.phase
                        );
                        assert_eq!(
                            hand_i, h0,
                            "seat {seat_i} hand.len={hand_i} but seat {seat0} hand.len={h0} \
                             at phase {:?}; full counts: {counts:?}",
                            session.table.phase
                        );
                        assert_eq!(
                            cards_i, hand_i,
                            "seat {seat_i} cards.dealt={cards_i} but hand.len={hand_i} \
                             at phase {:?}",
                            session.table.phase
                        );
                    }
                }
            }
            SessionStep::HandComplete => return,
        }
    }
}
