//! Render the live 9-seat table for Play and Arena modes.
//!
//! Layout:
//!
//! ```text
//! ┌─ Table ─────────────────────────────────────┐
//! │  Seat 0  You          $10,000  [Kh As]       │
//! │  Seat 1  gto          $10,000  [??]   BTN    │
//! │  ...                                          │
//! │                                               │
//! │  Board: 2c 7d Th          Pot: 350            │
//! └───────────────────────────────────────────────┘
//! ```
//!
//! The seat list uses a [`Table`](ratatui::widgets::Table) widget so columns
//! stay aligned and the active seat can be highlighted.

use pkcore::casino::table_no_cell::TableNoCell;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table as TableWidget};

use crate::modes::play::HERO_SEAT;
use crate::modes::{ArenaState, Awaiting, PlayState};

/// Renders the table view for Play mode.
pub fn render_table_view_play(state: &PlayState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(11), Constraint::Length(3)])
        .split(area);

    let active_seat = match state.awaiting {
        Awaiting::Human(s) => Some(s),
        _ => None,
    };
    let rows = seat_rows(&state.session.table, Some(HERO_SEAT), active_seat, |seat| {
        state.seat_name(seat)
    });
    render_seats(frame, chunks[0], rows);
    render_board(&state.session.table, frame, chunks[1]);
}

/// Renders the table view for Arena mode.
pub fn render_table_view_arena(state: &ArenaState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(11), Constraint::Length(3)])
        .split(area);

    let active_seat = if matches!(state.phase, crate::modes::arena::ArenaPhase::Running) {
        let next = state.session.table.next_to_act();
        Some(next)
    } else {
        None
    };
    let rows = seat_rows(&state.session.table, None, active_seat, |seat| {
        state.seat_name(seat)
    });
    render_seats(frame, chunks[0], rows);
    render_board(&state.session.table, frame, chunks[1]);
}

struct SeatRow {
    seat: u8,
    name: String,
    chips: usize,
    hole: String,
    bet: usize,
    tag: String,
    folded: bool,
    is_hero: bool,
    is_active: bool,
}

fn seat_rows<F: Fn(u8) -> String>(
    table: &TableNoCell,
    hero_seat: Option<u8>,
    active_seat: Option<u8>,
    name_of: F,
) -> Vec<SeatRow> {
    let btn = table.button;
    let sb = table.determine_small_blind();
    let bb = table.determine_big_blind();
    let n = table.seats.0.len() as u8;
    let mut out = Vec::with_capacity(n as usize);
    for i in 0..n {
        let Some(seat_data) = table.seats.get_seat(i) else {
            continue;
        };
        if seat_data.is_empty() {
            continue;
        }
        let chips = seat_data.player.chips;
        let bet = seat_data.player.bet;
        let folded = !seat_data.player.is_in_hand();
        let hole = if seat_data.cards.has_cards() {
            if hero_seat == Some(i) || hero_seat.is_none() {
                seat_data.cards.sorted_display()
            } else {
                "[??]".into()
            }
        } else {
            String::new()
        };
        let tag = position_tag(i, btn, sb, bb)
            .map(str::to_owned)
            .unwrap_or_default();
        out.push(SeatRow {
            seat: i,
            name: name_of(i),
            chips,
            hole,
            bet,
            tag,
            folded,
            is_hero: hero_seat == Some(i),
            is_active: active_seat == Some(i),
        });
    }
    out
}

fn render_seats(frame: &mut Frame, area: Rect, rows: Vec<SeatRow>) {
    let header = Row::new(vec![
        Cell::from("#"),
        Cell::from("Name"),
        Cell::from("Chips"),
        Cell::from("Bet"),
        Cell::from("Hole"),
        Cell::from("Pos"),
    ])
    .style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::Yellow),
    );

    let widths = [
        Constraint::Length(3),
        Constraint::Length(22),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Length(8),
    ];

    let body: Vec<Row> = rows
        .iter()
        .map(|r| {
            let mut style = Style::default();
            if r.folded {
                style = style.fg(Color::DarkGray).add_modifier(Modifier::DIM);
            } else if r.is_active {
                style = style.fg(Color::Yellow).add_modifier(Modifier::BOLD);
            } else if r.is_hero {
                style = style.fg(Color::Cyan);
            }
            let badge = if r.folded {
                "FOLD".to_string()
            } else if r.bet > 0 {
                format!("{}", r.bet)
            } else {
                String::new()
            };
            Row::new(vec![
                Cell::from(format!("{}", r.seat)),
                Cell::from(r.name.clone()),
                Cell::from(format!("{}", r.chips)),
                Cell::from(badge),
                Cell::from(r.hole.clone()),
                Cell::from(r.tag.clone()),
            ])
            .style(style)
        })
        .collect();

    let widget = TableWidget::new(body, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(" Table "));
    frame.render_widget(widget, area);
}

fn render_board(table: &TableNoCell, frame: &mut Frame, area: Rect) {
    let board_str = table.board.to_string();
    let board_display = if board_str.is_empty() {
        "(pre-flop)".to_string()
    } else {
        board_str
    };
    let pot = table.effective_pot();
    let phase = format!("{:?}", table.phase);
    let text = Text::from(vec![Line::from(vec![
        Span::styled("Board: ", Style::default().fg(Color::Gray)),
        Span::styled(
            board_display,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("    "),
        Span::styled("Pot: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{pot}"),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("    "),
        Span::styled(
            format!("phase: {phase}"),
            Style::default().fg(Color::DarkGray),
        ),
    ])]);
    let p = Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" Board "));
    frame.render_widget(p, area);
}

fn position_tag(seat: u8, btn: u8, sb: u8, bb: u8) -> Option<&'static str> {
    match (seat == btn, seat == sb, seat == bb) {
        (true, true, _) => Some("BTN/SB"),
        (true, _, _) => Some("BTN"),
        (_, true, _) => Some("SB"),
        (_, _, true) => Some("BB"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_tag_btn() {
        assert_eq!(position_tag(3, 3, 4, 5), Some("BTN"));
    }
    #[test]
    fn position_tag_sb() {
        assert_eq!(position_tag(4, 3, 4, 5), Some("SB"));
    }
    #[test]
    fn position_tag_bb() {
        assert_eq!(position_tag(5, 3, 4, 5), Some("BB"));
    }
    #[test]
    fn position_tag_none() {
        assert_eq!(position_tag(7, 3, 4, 5), None);
    }
    #[test]
    fn position_tag_heads_up_btn_sb() {
        assert_eq!(position_tag(0, 0, 0, 1), Some("BTN/SB"));
    }
}
