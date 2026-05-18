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

use pkcore::casino::table_no_cell::{SeatNoCell, TableNoCell};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table as TableWidget};

use crate::modes::play::{HERO_SEAT, ShowdownSeat};
use crate::modes::{ArenaState, Awaiting, PlayState};

/// Renders the table view for Play mode.
///
/// During [`Awaiting::HandComplete`], if the engine recorded a showdown
/// snapshot, every active seat's hole cards are revealed in the table — even
/// for hands the hero folded. (Showdown isn't recorded when only one player
/// reached the end, since uncontested wins don't require a reveal.)
pub fn render_table_view_play(state: &PlayState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(11), Constraint::Length(3)])
        .split(area);

    let active_seat = match state.awaiting {
        Awaiting::Human(s) => Some(s),
        _ => None,
    };
    let reveal_at_showdown = matches!(state.awaiting, Awaiting::HandComplete)
        .then(|| state.last_showdown.as_deref())
        .flatten();
    let rows = seat_rows(
        &state.session.table,
        Some(HERO_SEAT),
        active_seat,
        reveal_at_showdown,
        |seat| state.seat_name(seat),
    );
    render_seats(frame, chunks[0], &rows);
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
    let rows = seat_rows(&state.session.table, None, active_seat, None, |seat| {
        state.seat_name(seat)
    });
    render_seats(frame, chunks[0], &rows);
    render_board(&state.session.table, frame, chunks[1]);
}

/// Mutually-exclusive emphasis a single seat row can carry, in priority order
/// (highest first). Packed into one field to keep [`SeatRow`] below clippy's
/// `struct_excessive_bools` threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Accent {
    /// The seat owns the active turn highlight (yellow + bold).
    Active,
    /// The seat is the human player (cyan).
    Hero,
    /// The seat's cards were just revealed at showdown (bold green hole).
    Revealed,
    #[default]
    None,
}

struct SeatRow {
    seat: u8,
    name: String,
    chips: usize,
    hole: String,
    bet: usize,
    tag: String,
    folded: bool,
    accent: Accent,
}

fn seat_rows<F: Fn(u8) -> String>(
    table: &TableNoCell,
    hero_seat: Option<u8>,
    active_seat: Option<u8>,
    showdown: Option<&[ShowdownSeat]>,
    name_of: F,
) -> Vec<SeatRow> {
    let btn = table.button;
    let sb = table.determine_small_blind();
    let bb = table.determine_big_blind();
    let n = u8::try_from(table.seats.0.len()).unwrap_or(u8::MAX);
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

        // Check the showdown snapshot first — those cards override even
        // hidden bot cards. Snapshots only contain still-active seats so
        // folded players never appear here.
        let revealed = showdown.and_then(|s| s.iter().find(|r| r.seat == i));
        let (hole, is_revealed) = if let Some(r) = revealed {
            let class = r
                .hand_class
                .as_deref()
                .map_or_else(String::new, |c| format!(" {c}"));
            (format!("{}{class}", r.hole), true)
        } else if seat_data.cards.has_cards() {
            let as_owner = hero_seat == Some(i) || hero_seat.is_none();
            (format_hole(seat_data, as_owner), false)
        } else {
            (String::new(), false)
        };

        let accent = if is_revealed {
            Accent::Revealed
        } else if active_seat == Some(i) {
            Accent::Active
        } else if hero_seat == Some(i) {
            Accent::Hero
        } else {
            Accent::None
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
            accent,
        });
    }
    out
}

/// Formats a seat's hand for the table view.
///
/// Branches on visibility — for stud-family variants where `seat.hand`
/// carries face-up cards, opponents see those up-cards in dealt order and
/// `??` for each face-down card; the seat's owner sees the full hand with
/// face-down cards bracketed (`[K♠]`) to indicate concealment from the
/// other players. For NLHE/PLO where every card is face-down, opponents
/// see `[??]` (unchanged from before) and the owner sees the sorted
/// display (also unchanged).
fn format_hole(seat: &SeatNoCell, as_owner: bool) -> String {
    let hand_slice = seat.hand.as_slice();
    let any_up = hand_slice.iter().any(|hc| hc.is_up());

    // Dealt cards in dealt order (skipping unfilled blank slots). This is
    // the always-populated source — pkcore's dealer mirrors every dealt
    // card here regardless of variant. We use it as the count of truth
    // so the display never drops a dealt card even if `seat.hand` is
    // somehow shorter (which shouldn't happen, but lets us survive it).
    let dealt: Vec<pkcore::card::Card> = seat
        .cards
        .as_slice()
        .iter()
        .copied()
        .filter(|c| *c != pkcore::card::Card::BLANK)
        .collect();

    if !any_up {
        // NLHE/PLO: every card face-down. Owner sees the sorted hand;
        // opponents see a single hidden placeholder.
        return if as_owner {
            seat.cards.sorted_display()
        } else {
            "[??]".to_string()
        };
    }

    // Stud-style: walk the dealt cards in order, looking up per-card
    // visibility from `seat.hand`. For any tail card the hand doesn't
    // cover, fall back to the stud dealing pattern (positions 0–1 down,
    // 2–5 up, 6 down) so a length mismatch can't silently hide a card.
    dealt
        .iter()
        .enumerate()
        .map(|(idx, card)| {
            let is_up = hand_slice
                .get(idx)
                .map(|hc| hc.is_up())
                .unwrap_or_else(|| matches!(idx, 2..=5));
            if as_owner {
                if is_up {
                    pad_card_slot(&card.to_string())
                } else {
                    pad_card_slot(&format!("[{card}]"))
                }
            } else if is_up {
                pad_card_slot(&card.to_string())
            } else {
                " ?? ".to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Right-aligns a card representation to a 4-char slot so `[A♠]`, ` A♠`,
/// `  ??`, and `  Q♥` all occupy the same column. Width is in Unicode
/// scalar values (chars), which matches Rust's `format!` width semantics.
fn pad_card_slot(s: &str) -> String {
    format!("{s:>4}")
}

fn render_seats(frame: &mut Frame, area: Rect, rows: &[SeatRow]) {
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
        // Wide enough for a 7-card Stud hand with bracketed down cards or
        // ?? placeholders (e.g. "[A♠] [K♠] Q♥ J♥ T♥ 9♥ [4♣]").
        Constraint::Length(36),
        Constraint::Length(8),
    ];

    let body: Vec<Row> = rows
        .iter()
        .map(|r| {
            let style = if r.folded {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM)
            } else {
                match r.accent {
                    Accent::Active => Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                    Accent::Hero => Style::default().fg(Color::Cyan),
                    Accent::Revealed | Accent::None => Style::default(),
                }
            };
            let badge = if r.folded {
                "FOLD".to_string()
            } else if r.bet > 0 {
                format!("{}", r.bet)
            } else {
                String::new()
            };
            // Showdown reveals are drawn in bold green so the user's eye
            // jumps straight to them when a hand resolves.
            let hole_cell = if r.accent == Accent::Revealed {
                Cell::from(r.hole.clone()).style(
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Cell::from(r.hole.clone())
            };
            Row::new(vec![
                Cell::from(format!("{}", r.seat)),
                Cell::from(r.name.clone()),
                Cell::from(format!("{}", r.chips)),
                Cell::from(badge),
                hole_cell,
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
    let has_board = table.game.family().uses_community_board();
    let pot = table.effective_pot();
    let phase = format!("{:?}", table.phase);

    let mut spans = Vec::with_capacity(7);
    if has_board {
        let board_str = table.board.to_string();
        let board_display = if board_str.is_empty() {
            "(pre-flop)".to_string()
        } else {
            board_str
        };
        spans.push(Span::styled("Board: ", Style::default().fg(Color::Gray)));
        spans.push(Span::styled(
            board_display,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw("    "));
    }
    spans.push(Span::styled("Pot: ", Style::default().fg(Color::Gray)));
    spans.push(Span::styled(
        format!("{pot}"),
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::raw("    "));
    spans.push(Span::styled(
        format!("phase: {phase}"),
        Style::default().fg(Color::DarkGray),
    ));

    let p = Paragraph::new(Text::from(vec![Line::from(spans)]))
        .block(Block::default().borders(Borders::ALL).title(" Board "));
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
    use pkcore::card::Card;
    use pkcore::casino::table_no_cell::PlayerNoCell;

    fn seat_with(downs: &[Card], ups: &[Card]) -> SeatNoCell {
        use std::str::FromStr;
        let mut s = SeatNoCell::new(PlayerNoCell::new_with_chips("X".to_string(), 1_000));
        // Mirror the dealt cards into the legacy `cards: BoxedCards` storage so
        // `cards.has_cards()` and the NLHE-hero sorted_display path both work.
        let joined = downs
            .iter()
            .chain(ups.iter())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        s.cards = pkcore::arrays::sliced::BoxedCards::from_str(&joined).unwrap();
        s.hand.extend_down(downs.iter().copied());
        s.hand.extend_up(ups.iter().copied());
        s
    }

    #[test]
    fn format_hole_nlhe_opponent_is_hidden() {
        let seat = seat_with(&[Card::ACE_SPADES, Card::KING_SPADES], &[]);
        assert_eq!(format_hole(&seat, false), "[??]");
    }

    #[test]
    fn format_hole_stud_opponent_shows_up_cards_only() {
        // 5th street: 2 down + 3 up. Each slot is 4 chars wide; "??" sits
        // at slot cols 2-3 (` ?? `) so it aligns with the card content
        // inside hero's bracketed down cards. Up cards stay right-aligned
        // at slot cols 3-4 so they don't shift.
        let seat = seat_with(
            &[Card::ACE_SPADES, Card::KING_SPADES],
            &[Card::QUEEN_HEARTS, Card::JACK_HEARTS, Card::TEN_HEARTS],
        );
        assert_eq!(format_hole(&seat, false), " ??   ??    Q♥   J♥   T♥");
    }

    #[test]
    fn format_hole_stud_hero_brackets_down_cards() {
        // Hero sees own down cards (bracketed) plus own up cards (bare).
        // "[A♠]" already fills the 4-char slot; "Q♥" gets 2 leading spaces
        // so its card characters line up under the bracketed-card chars.
        let seat = seat_with(
            &[Card::ACE_SPADES, Card::KING_SPADES],
            &[Card::QUEEN_HEARTS],
        );
        assert_eq!(format_hole(&seat, true), "[A♠] [K♠]   Q♥");
    }

    #[test]
    fn format_hole_stud_opponent_third_street_one_upcard() {
        let seat = seat_with(
            &[Card::ACE_SPADES, Card::KING_SPADES],
            &[Card::QUEEN_HEARTS],
        );
        assert_eq!(format_hole(&seat, false), " ??   ??    Q♥");
    }

    #[test]
    fn format_hole_renders_all_dealt_cards_even_if_hand_is_short() {
        // Regression: pkcore was observed dealing 6 cards into seat.cards
        // but only pushing 5 into seat.hand for seat 0, which caused the
        // hero to show one fewer card than opponents in the table view.
        // We now iterate seat.cards (always populated by the dealer) and
        // fall back to stud-position-based visibility for any tail entry
        // the hand doesn't cover. For a 6-card stud-style row, position 5
        // (the 6th card) should default to face-up.
        use std::str::FromStr;
        let mut s = SeatNoCell::new(PlayerNoCell::new_with_chips("X".to_string(), 1_000));
        s.cards = pkcore::arrays::sliced::BoxedCards::from_str("5♠ 4♥ 7♥ K♦ 2♠ 9♣").unwrap();
        // Push only 5 entries into the hand (simulating the observed
        // pkcore short-hand state): 2 down + 3 up.
        s.hand.extend_down([Card::FIVE_SPADES, Card::FOUR_HEARTS]);
        s.hand
            .extend_up([Card::SEVEN_HEARTS, Card::KING_DIAMONDS, Card::DEUCE_SPADES]);
        let out = format_hole(&s, true);
        // All 6 cards should appear, with the 6th treated as face-up.
        assert!(out.contains("[5♠]"));
        assert!(out.contains("[4♥]"));
        assert!(out.contains("7♥"));
        assert!(out.contains("K♦"));
        assert!(out.contains("2♠"));
        assert!(out.contains("9♣"));
        // 6 slots × 4 chars + 5 separator spaces = 29 chars.
        assert_eq!(out.chars().count(), 29);
    }

    #[test]
    fn format_hole_question_marks_align_with_bracketed_card_content() {
        // Hero's slot 1 = `[A♠]` — card chars `A` and `♠` sit at slot
        // cols 2 and 3 (inside the brackets). Opponent's `??` placeholder
        // should sit at the same slot cols so the `?`s line up vertically
        // with the `A♠` chars. Each row needs at least one up card so
        // both take the stud-style branch (otherwise opp falls into the
        // NLHE-style `[??]` shortcut).
        let hero = seat_with(&[Card::ACE_SPADES], &[Card::DEUCE_CLUBS]);
        let opp = seat_with(&[Card::ACE_DIAMONDS], &[Card::DEUCE_CLUBS]);
        let h_chars: Vec<char> = format_hole(&hero, true).chars().collect();
        let o_chars: Vec<char> = format_hole(&opp, false).chars().collect();
        assert_eq!(&h_chars[0..4], &['[', 'A', '♠', ']']);
        assert_eq!(&o_chars[0..4], &[' ', '?', '?', ' ']);
    }

    #[test]
    fn format_hole_stud_hero_and_opponent_align_per_slot() {
        // Both rows should have card slots that start at the same column
        // indices: slot N begins at char index 5*(N-1).
        let hero = seat_with(
            &[Card::ACE_SPADES, Card::KING_SPADES],
            &[Card::QUEEN_HEARTS],
        );
        let opp = seat_with(&[Card::DEUCE_CLUBS, Card::TREY_CLUBS], &[Card::JACK_HEARTS]);
        let h = format_hole(&hero, true);
        let o = format_hole(&opp, false);
        // Both should be the same total length (3 cards × 4 chars + 2 spaces).
        assert_eq!(h.chars().count(), o.chars().count());
        // Third-card slot (chars 11..15) is the only up card on either row.
        let hero_slot3: String = h.chars().skip(10).take(4).collect();
        let opp_slot3: String = o.chars().skip(10).take(4).collect();
        assert_eq!(hero_slot3, "  Q♥");
        assert_eq!(opp_slot3, "  J♥");
    }

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
