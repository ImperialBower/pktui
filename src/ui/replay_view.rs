//! Render the YAML replay view.
//!
//! Layout: a header summarising the hand (id, button, stakes), a player
//! table, the currently-visible street's actions, and a results panel at
//! the showdown step.

use pkcore::hand_history::{Action, ActionType, HandHistory};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::modes::ReplayState;

/// Renders the replay view for the current hand.
pub fn render(state: &ReplayState, frame: &mut Frame, area: Rect) {
    if state.collection.is_empty() {
        let p = Paragraph::new("(empty collection — nothing to replay)")
            .block(Block::default().borders(Borders::ALL).title(" Replay "));
        frame.render_widget(p, area);
        return;
    }
    let Some(hand) = state.collection.hands().get(state.hand_index) else {
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(4)])
        .split(area);

    let holes = replay_holes(hand);
    let board = replay_board(hand, state.street_index);
    let eq = state.odds.equities(&holes, &board);
    render_header(hand, state.street_index, &eq, frame, chunks[0]);
    render_street(hand, state.street_index, frame, chunks[1]);
}

fn render_header(
    hand: &HandHistory,
    street: usize,
    eq: &[(u8, f64)],
    frame: &mut Frame,
    area: Rect,
) {
    let btn = hand
        .table
        .button
        .map_or_else(|| "?".to_string(), |b| b.to_string());
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                format!(" Hand {} ", hand.hand.id),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::raw(format!(
                "btn={btn}  blinds={}/{}  ts={}",
                chips(hand.table.stakes.small_blind),
                chips(hand.table.stakes.big_blind),
                hand.hand.timestamp.as_deref().unwrap_or("-"),
            )),
            Span::raw("  "),
            Span::styled(
                format!("[{}]", street_name(street)),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        Line::raw("seat  name                    stack  hole            win%"),
    ];
    for p in &hand.players {
        let hole = p
            .hole_cards
            .as_deref()
            .map_or_else(|| "??".to_string(), crate::ui::sort_hole_cards);
        let win = eq.iter().find(|(s, _)| *s == p.seat).map_or_else(
            || "  —".to_string(),
            |(_, e)| format!("  {:.1}%", e * 100.0),
        );
        lines.push(Line::raw(format!(
            "{:>4}  {:<22}  {:>5}  {:<14}{win}",
            p.seat,
            p.name,
            chips(p.stack),
            hole,
        )));
    }
    let widget = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(" Replay "))
        .wrap(Wrap { trim: false });
    frame.render_widget(widget, area);
}

fn render_street(hand: &HandHistory, street: usize, frame: &mut Frame, area: Rect) {
    let Some(streets) = hand.streets.as_ref() else {
        let p = Paragraph::new("(no street data)").block(
            Block::default()
                .borders(Borders::ALL)
                .title(street_label(street)),
        );
        frame.render_widget(p, area);
        return;
    };

    let (cards, actions, pot): (String, Vec<&Action>, Option<f64>) = match street {
        0 => (
            "(pre-flop)".to_string(),
            streets
                .preflop
                .as_ref()
                .map(|s| s.actions.iter().collect())
                .unwrap_or_default(),
            streets.preflop.as_ref().and_then(|s| s.pot),
        ),
        1 => (
            streets
                .flop
                .as_ref()
                .map_or_else(|| "-".into(), |s| s.cards.clone()),
            streets
                .flop
                .as_ref()
                .map(|s| s.actions.iter().collect())
                .unwrap_or_default(),
            streets.flop.as_ref().and_then(|s| s.pot),
        ),
        2 => (
            streets
                .turn
                .as_ref()
                .map_or_else(|| "-".into(), |s| s.card.clone()),
            streets
                .turn
                .as_ref()
                .map(|s| s.actions.iter().collect())
                .unwrap_or_default(),
            streets.turn.as_ref().and_then(|s| s.pot),
        ),
        3 => (
            streets
                .river
                .as_ref()
                .map_or_else(|| "-".into(), |s| s.card.clone()),
            streets
                .river
                .as_ref()
                .map(|s| s.actions.iter().collect())
                .unwrap_or_default(),
            streets.river.as_ref().and_then(|s| s.pot),
        ),
        _ => {
            render_results(hand, frame, area);
            return;
        }
    };

    let mut lines = vec![Line::from(vec![
        Span::styled(
            format!("{}: ", street_name(street)),
            Style::default().fg(Color::Gray),
        ),
        Span::styled(
            cards,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(
            format!("pot: {}", pot.map_or(0, chips)),
            Style::default().fg(Color::Green),
        ),
    ])];
    lines.push(Line::raw(""));
    for a in actions {
        lines.push(Line::raw(format_action(a, hand)));
    }
    let w = Paragraph::new(Text::from(lines))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(street_label(street)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(w, area);
}

fn render_results(hand: &HandHistory, frame: &mut Frame, area: Rect) {
    let mut lines = vec![Line::from(Span::styled(
        "Results",
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    ))];
    if let Some(results) = hand.results.as_ref() {
        for r in results {
            let name = hand
                .players
                .iter()
                .find(|p| p.seat == r.seat)
                .map_or("?", |p| p.name.as_str());
            let outcome = format!("{:?}", r.outcome).to_lowercase();
            let net = r.net.map(|n| format!("{n:+.0}")).unwrap_or_default();
            let hand_str = r.best_hand.as_deref().unwrap_or("");
            lines.push(Line::raw(format!(
                "  {name:<22}  {outcome:<5}  net={net:>7}  {hand_str}"
            )));
        }
    } else {
        lines.push(Line::raw("(no results recorded)"));
    }
    let w = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(" Showdown "))
        .wrap(Wrap { trim: false });
    frame.render_widget(w, area);
}

fn format_action(a: &Action, hand: &HandHistory) -> String {
    let name = hand
        .players
        .iter()
        .find(|p| p.seat == a.seat)
        .map_or("?", |p| p.name.as_str());
    let verb = match a.action {
        ActionType::Post => format!("posts {}", a.amount.map_or(0, chips)),
        ActionType::Fold => "folds".to_string(),
        ActionType::Check => "checks".to_string(),
        ActionType::Call => format!("calls {}", a.amount.map_or(0, chips)),
        ActionType::Bet => format!("bets {}", a.amount.map_or(0, chips)),
        ActionType::Raise => format!("raises to {}", a.amount.map_or(0, chips)),
        ActionType::AllIn => format!("ALL-IN ({})", a.amount.map_or(0, chips)),
    };
    format!("  {name:<22}  {verb}")
}

/// Returns `(seat, "card card")` for every player with exactly two recorded
/// hole cards. Players with no hole cards or non-Hold'em hand sizes are
/// silently skipped.
fn replay_holes(hand: &HandHistory) -> Vec<(u8, String)> {
    hand.players
        .iter()
        .filter_map(|p| {
            let cards = p.hole_cards.as_deref()?;
            if cards.split_whitespace().count() != 2 {
                return None;
            }
            Some((p.seat, cards.to_string()))
        })
        .collect()
}

/// Returns the community board string visible at `street`.
///
/// - 0 (preflop) → `""`
/// - 1 (flop)    → flop cards (e.g. `"Ah Kd Qc"`)
/// - 2 (turn)    → flop + turn card (e.g. `"Ah Kd Qc 2s"`)
/// - 3+ (river)  → flop + turn + river card (e.g. `"Ah Kd Qc 2s 7h"`)
fn replay_board(hand: &HandHistory, street: usize) -> String {
    let Some(streets) = hand.streets.as_ref() else {
        return String::new();
    };
    let mut parts: Vec<String> = Vec::new();
    if street >= 1
        && let Some(f) = streets.flop.as_ref()
    {
        parts.push(f.cards.clone());
    }
    if street >= 2
        && let Some(t) = streets.turn.as_ref()
    {
        parts.push(t.card.clone());
    }
    if street >= 3
        && let Some(r) = streets.river.as_ref()
    {
        parts.push(r.card.clone());
    }
    parts.join(" ").trim().to_string()
}

/// pkcore's hand-history YAML stores chip amounts as `f64` (to allow
/// fractional chips in future variants); in practice every value is a
/// non-negative integer. This helper performs the lossy cast in one place
/// with the pedantic lints silenced.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
#[must_use]
fn chips(v: f64) -> usize {
    if v < 0.0 { 0 } else { v as usize }
}

fn street_name(s: usize) -> &'static str {
    match s {
        0 => "Preflop",
        1 => "Flop",
        2 => "Turn",
        3 => "River",
        _ => "Showdown",
    }
}

fn street_label(s: usize) -> &'static str {
    match s {
        0 => " Preflop ",
        1 => " Flop ",
        2 => " Turn ",
        3 => " River ",
        _ => " Showdown ",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── HandHistory fixture ──────────────────────────────────────────────────

    fn sample_hand() -> pkcore::hand_history::HandHistory {
        use pkcore::hand_history::{
            FlopStreet, HandHistory, HandMeta, HandVariant, PlayerEntry, PreflopStreet,
            RiverStreet, Stakes, Streets, TableInfo, TurnStreet,
        };
        HandHistory {
            pkcore_version: None,
            format_version: 1,
            hand: HandMeta {
                id: "test-replay-001".to_string(),
                game: HandVariant::Holdem,
                timestamp: None,
                source: None,
                description: None,
            },
            table: TableInfo {
                name: None,
                seats: Some(2),
                button: Some(0),
                stakes: Stakes {
                    small_blind: 5.0,
                    big_blind: 10.0,
                    ante: None,
                    straddle: None,
                    bring_in: None,
                },
                betting_structure: Default::default(),
            },
            players: vec![
                PlayerEntry {
                    seat: 0,
                    name: "Alice".to_string(),
                    stack: 1000.0,
                    player_id: None,
                    hole_cards: Some("Ts 9s".to_string()),
                    posted: None,
                    hole_cards_visibility: None,
                    withdrawn: None,
                },
                PlayerEntry {
                    seat: 1,
                    name: "Bob".to_string(),
                    stack: 1000.0,
                    player_id: None,
                    hole_cards: Some("8d 7d".to_string()),
                    posted: None,
                    hole_cards_visibility: None,
                    withdrawn: None,
                },
            ],
            board: None,
            streets: Some(Streets {
                preflop: Some(PreflopStreet {
                    actions: vec![],
                    pot: None,
                }),
                flop: Some(FlopStreet {
                    cards: "Ah Kd Qc".to_string(),
                    actions: vec![],
                    pot: None,
                }),
                turn: Some(TurnStreet {
                    card: "2s".to_string(),
                    actions: vec![],
                    pot: None,
                }),
                river: Some(RiverStreet {
                    card: "7h".to_string(),
                    actions: vec![],
                    pot: None,
                }),
            }),
            results: None,
            analysis: None,
            shuffled_deck: None,
        }
    }

    // ── replay_board tests ───────────────────────────────────────────────────

    #[test]
    fn replay_board_accumulates_per_street() {
        let hh = sample_hand();
        assert_eq!(replay_board(&hh, 0), "");
        assert_eq!(replay_board(&hh, 1), "Ah Kd Qc");
        assert_eq!(replay_board(&hh, 2), "Ah Kd Qc 2s");
        assert_eq!(replay_board(&hh, 3), "Ah Kd Qc 2s 7h");
    }

    // ── replay_holes tests ───────────────────────────────────────────────────

    #[test]
    fn replay_holes_collects_recorded_hands() {
        let hh = sample_hand();
        let holes = replay_holes(&hh);
        assert_eq!(holes.len(), 2);
        assert!(holes.iter().all(|(_, c)| c.split_whitespace().count() == 2));
    }

    // ── existing tests ───────────────────────────────────────────────────────

    #[test]
    fn street_name_table() {
        assert_eq!(street_name(0), "Preflop");
        assert_eq!(street_name(1), "Flop");
        assert_eq!(street_name(2), "Turn");
        assert_eq!(street_name(3), "River");
        assert_eq!(street_name(4), "Showdown");
        assert_eq!(street_name(99), "Showdown");
    }

    #[test]
    fn street_label_starts_and_ends_with_space() {
        for s in 0..5 {
            let l = street_label(s);
            assert!(l.starts_with(' '));
            assert!(l.ends_with(' '));
        }
    }

    #[test]
    fn chips_clamps_negative_to_zero() {
        assert_eq!(chips(-1.0), 0);
        assert_eq!(chips(0.0), 0);
        assert_eq!(chips(150.0), 150);
        assert_eq!(chips(150.7), 150);
    }
}
