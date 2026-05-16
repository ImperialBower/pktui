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

    render_header(hand, state.street_index, frame, chunks[0]);
    render_street(hand, state.street_index, frame, chunks[1]);
}

fn render_header(hand: &HandHistory, street: usize, frame: &mut Frame, area: Rect) {
    let btn = hand
        .table
        .button
        .map(|b| b.to_string())
        .unwrap_or_else(|| "?".to_string());
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
                hand.table.stakes.small_blind as usize,
                hand.table.stakes.big_blind as usize,
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
        Line::raw("seat  name                    stack  hole"),
    ];
    for p in &hand.players {
        lines.push(Line::raw(format!(
            "{:>4}  {:<22}  {:>5}  {}",
            p.seat,
            p.name,
            p.stack as usize,
            p.hole_cards.as_deref().unwrap_or("??"),
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
                .map(|s| s.cards.clone())
                .unwrap_or_else(|| "-".into()),
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
                .map(|s| s.card.clone())
                .unwrap_or_else(|| "-".into()),
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
                .map(|s| s.card.clone())
                .unwrap_or_else(|| "-".into()),
            streets
                .river
                .as_ref()
                .map(|s| s.actions.iter().collect())
                .unwrap_or_default(),
            streets.river.as_ref().and_then(|s| s.pot),
        ),
        _ => return render_results(hand, frame, area),
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
            format!("pot: {}", pot.map(|p| p as usize).unwrap_or(0)),
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
                .map(|p| p.name.as_str())
                .unwrap_or("?");
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
        .map(|p| p.name.as_str())
        .unwrap_or("?");
    let verb = match a.action {
        ActionType::Post => format!("posts {}", a.amount.map(|x| x as usize).unwrap_or(0)),
        ActionType::Fold => "folds".to_string(),
        ActionType::Check => "checks".to_string(),
        ActionType::Call => format!("calls {}", a.amount.map(|x| x as usize).unwrap_or(0)),
        ActionType::Bet => format!("bets {}", a.amount.map(|x| x as usize).unwrap_or(0)),
        ActionType::Raise => format!("raises to {}", a.amount.map(|x| x as usize).unwrap_or(0)),
        ActionType::AllIn => format!("ALL-IN ({})", a.amount.map(|x| x as usize).unwrap_or(0)),
    };
    format!("  {name:<22}  {verb}")
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
}
