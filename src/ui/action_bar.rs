//! Bottom action bar: context-sensitive hotkey hints.
//!
//! The bar uses **two lines**:
//!
//! 1. The current bet/raise amount with the preset values that `1`/`2`/`3`
//!    will set it to (so the user can see at a glance what each preset
//!    does).
//! 2. The available action hotkeys (`f`/`k`/`c`/`b`/`r`/`a`).
//!
//! Splitting the bar across two lines is intentional: a single line gets
//! truncated on narrower terminals and the bet field — the bit that
//! actually changes when you press `1`/`2`/`3` — ends up off-screen.

use pkcore::games::betting_structure::{BetTier, BettingStructure};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, AppMode};
use crate::modes::Awaiting;
use crate::modes::PlayState;
use crate::modes::play::HERO_SEAT;

/// Renders the action-bar widget.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let lines: Vec<Line> = match &app.mode {
        AppMode::Play(p) => play_hints(p),
        AppMode::Arena(_) => vec![arena_hints()],
        AppMode::Replay(_) => vec![replay_hints()],
    };
    let widget = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(" Action "));
    frame.render_widget(widget, area);
}

/// Resolves the minimum legal bet/raise amount.
///
/// Preflop with no bets yet (only blinds posted): minimum bet is one big
/// blind. With an outstanding bet: minimum is `table.bet + table.min_raise()`.
#[must_use]
pub(crate) fn min_for(p: &PlayState, seat: u8) -> usize {
    let table = &p.session.table;
    if table.to_call(seat) == 0 {
        table.forced.big_blind
    } else {
        table.bet + table.min_raise()
    }
}

/// Returns the (min, `half_pot`, pot) preset triple displayed in the action
/// bar so the user knows what each `1`/`2`/`3` key will set the bet to.
#[must_use]
pub(crate) fn preset_values(p: &PlayState, seat: u8) -> (usize, usize, usize) {
    let pot = p.session.table.effective_pot();
    let min = min_for(p, seat);
    let half = (pot / 2).max(min);
    let full = pot.max(min);
    (min, half, full)
}

/// Builds line 1 of the action bar — the prominent bet amount with
/// structure-appropriate preset values. Fixed-limit games have a single
/// legal amount per street, so the ½pot / pot / tune hints are dropped
/// in that branch.
fn bet_line(p: &PlayState, seat: u8, verb: &str) -> Line<'static> {
    let amount_span = Span::styled(
        format!("[ {:>6} ]", p.bet.amount()),
        Style::default()
            .fg(Color::Yellow)
            .bg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );
    let verb_span = Span::styled(format!("{verb}: "), Style::default().fg(Color::Gray));
    if let BettingStructure::FixedLimit {
        small_bet,
        big_bet,
        raise_cap,
    } = p.session.table.betting
    {
        let (tier_amount, tier_label) = match p.session.table.current_bet_tier() {
            BetTier::Small => (small_bet, "small bet"),
            BetTier::Big => (big_bet, "big bet"),
        };
        Line::from(vec![
            verb_span,
            amount_span,
            Span::raw("   "),
            keystyle(" 1 "),
            Span::styled(
                format!(" {tier_label}({tier_amount})"),
                Style::default().fg(Color::Gray),
            ),
            Span::raw("   "),
            Span::styled(
                format!("fixed limit · small {small_bet} / big {big_bet} · cap {raise_cap}"),
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else {
        let (min, half, full) = preset_values(p, seat);
        Line::from(vec![
            verb_span,
            amount_span,
            Span::raw("   "),
            keystyle(" 1 "),
            Span::styled(format!(" min({min})"), Style::default().fg(Color::Gray)),
            Span::raw("   "),
            keystyle(" 2 "),
            Span::styled(format!(" ½pot({half})"), Style::default().fg(Color::Gray)),
            Span::raw("   "),
            keystyle(" 3 "),
            Span::styled(format!(" pot({full})"), Style::default().fg(Color::Gray)),
            Span::raw("   "),
            keystyle(" +/- "),
            Span::styled(" tune", Style::default().fg(Color::Gray)),
        ])
    }
}

fn play_hints(p: &PlayState) -> Vec<Line<'static>> {
    match p.awaiting {
        Awaiting::HandComplete => vec![
            Line::from(vec![Span::styled(
                "Hand complete",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![
                keystyle(" Enter "),
                Span::raw(" next hand   "),
                keystyle(" q "),
                Span::raw(" quit"),
            ]),
        ],
        Awaiting::SessionOver => vec![
            Line::from(Span::styled(
                "Session over",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![keystyle(" q "), Span::raw(" quit")]),
        ],
        Awaiting::Human(seat) if seat == HERO_SEAT => {
            let table = &p.session.table;
            let to_call = table.to_call(seat);
            let verb = if to_call == 0 { "Bet" } else { "Raise" };
            let line1 = bet_line(p, seat, verb);

            // Line 2: the discrete action hotkeys.
            let mut line2 = vec![keystyle(" f "), Span::raw(" fold  ")];
            if to_call == 0 {
                line2.push(keystyle(" k "));
                line2.push(Span::raw(" check  "));
                line2.push(keystyle(" b "));
                line2.push(Span::raw(" bet (use amount above)  "));
            } else {
                line2.push(keystyle(" c "));
                line2.push(Span::raw(format!(" call {to_call}  ")));
                line2.push(keystyle(" r "));
                line2.push(Span::raw(" raise (use amount above)  "));
            }
            line2.push(keystyle(" a "));
            line2.push(Span::raw(" all-in   "));
            line2.push(keystyle(" Enter "));
            line2.push(Span::raw(" confirm"));

            vec![line1, Line::from(line2)]
        }
        _ => vec![
            Line::from(Span::styled(
                "Bots acting…",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )),
            Line::from(vec![keystyle(" q "), Span::raw(" quit")]),
        ],
    }
}

fn arena_hints() -> Line<'static> {
    Line::from(vec![
        Span::styled(
            "Arena (watch-only)",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        keystyle(" + "),
        Span::raw(" faster   "),
        keystyle(" - "),
        Span::raw(" slower   "),
        keystyle(" q "),
        Span::raw(" quit"),
    ])
}

fn replay_hints() -> Line<'static> {
    Line::from(vec![
        keystyle(" n "),
        Span::raw(" next street   "),
        keystyle(" p "),
        Span::raw(" prev street   "),
        keystyle(" N/Enter "),
        Span::raw(" next hand   "),
        keystyle(" P "),
        Span::raw(" prev hand   "),
        keystyle(" q "),
        Span::raw(" quit"),
    ])
}

fn keystyle(key: &str) -> Span<'static> {
    Span::styled(
        key.to_string(),
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::App;

    #[test]
    fn keystyle_produces_span_with_bg() {
        let s = keystyle(" x ");
        assert!(s.style.bg.is_some());
    }

    #[test]
    fn arena_hints_contains_plus() {
        let line = arena_hints();
        let joined: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(joined.contains("faster"));
    }

    #[test]
    fn replay_hints_contains_next() {
        let line = replay_hints();
        let joined: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(joined.contains("next"));
    }

    #[test]
    fn min_for_returns_big_blind_when_nothing_to_call() {
        let app = App::play_default().unwrap();
        if let AppMode::Play(p) = &app.mode {
            // At start, nobody has acted — to_call is the BB for everyone
            // except the BB itself, so we just sanity-check the function
            // is non-zero for some seat.
            let v = min_for(p, 0);
            assert!(v > 0);
        }
    }

    #[test]
    fn preset_values_monotonic() {
        let app = App::play_default().unwrap();
        if let AppMode::Play(p) = &app.mode {
            let (min, half, full) = preset_values(p, 0);
            assert!(min <= half);
            assert!(half <= full);
        }
    }
}
