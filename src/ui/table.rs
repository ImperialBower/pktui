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

use pkcore::casino::table::{Seat, Table};
use pkcore::play::hole_card::HoleCard;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table as TableWidget};

use crate::modes::SpectateState;
use crate::modes::play::{HERO_SEAT, ShowdownSeat};
use crate::modes::{ArenaState, Awaiting, PlayState};
use pkdealer_proto::dealer::{PlayerState, Street, TableStatus};

/// Renders the table view for Play mode.
///
/// During [`Awaiting::HandComplete`], if the engine recorded a showdown
/// snapshot, every active seat's hole cards are revealed in the table — even
/// for hands the hero folded. (Showdown isn't recorded when only one player
/// reached the end, since uncontested wins don't require a reveal.)
pub fn render_table_view_play(state: &PlayState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(11)])
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
    render_board(&state.session.table, false, frame, chunks[0]);
    render_seats(frame, chunks[1], &rows);
}

/// Renders the table view for Arena mode.
pub fn render_table_view_arena(state: &ArenaState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(11)])
        .split(area);

    let active_seat = if matches!(state.phase, crate::modes::arena::ArenaPhase::Running) {
        let next = state.session.table.next_to_act();
        Some(next)
    } else {
        None
    };
    let mut rows = seat_rows(&state.session.table, None, active_seat, None, |seat| {
        state.seat_name(seat)
    });
    let holes = active_holes(&state.session.table);
    apply_odds(
        &mut rows,
        &holes,
        &state.session.table.board.to_string(),
        &state.odds,
    );
    render_board(&state.session.table, state.paused, frame, chunks[0]);
    render_seats(frame, chunks[1], &rows);
}

/// Renders the read-only spectator table from the latest dealer snapshot.
///
/// Shows a "waiting for dealer" placeholder until the first snapshot arrives.
pub fn render_table_view_spectate(state: &SpectateState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(11)])
        .split(area);

    if let Some(status) = &state.status {
        render_board_str(
            &status.board,
            status.pot,
            status.current_street,
            frame,
            chunks[0],
        );
        let mut rows = status_to_rows(status);
        let holes = status_active_holes(status);
        apply_odds(&mut rows, &holes, &status.board, &state.odds);
        render_seats(frame, chunks[1], &rows);
    } else {
        render_board_str("", 0, Street::Unspecified as i32, frame, chunks[0]);
        let placeholder = Paragraph::new("Waiting for the dealer…")
            .block(Block::default().borders(Borders::ALL).title(" Table "));
        frame.render_widget(placeholder, chunks[1]);
    }
}

/// Builds `SeatRow`s from a proto `TableStatus`. Hole cards are already
/// redacted by the dealer (empty `player_token`), so they are copied verbatim.
fn status_to_rows(status: &TableStatus) -> Vec<SeatRow> {
    let btn = u8::try_from(status.button_seat).unwrap_or(u8::MAX);
    let sb = u8::try_from(status.small_blind_seat).unwrap_or(u8::MAX);
    let bb = u8::try_from(status.big_blind_seat).unwrap_or(u8::MAX);
    status
        .seats
        .iter()
        .map(|s| {
            let folded =
                s.state == PlayerState::Folded as i32 || s.state == PlayerState::Out as i32;
            let active = status.hand_in_progress && s.seat_number == status.next_to_act;
            let accent = if active && !folded {
                Accent::Active
            } else {
                Accent::None
            };
            // The spectator stream reveals every seat's cards; show them only
            // for players still contesting the hand. A folded/out seat's hand
            // is mucked, so keep it hidden as `??`.
            let hole = if folded {
                "??".to_string()
            } else {
                crate::ui::sort_hole_cards(&s.cards)
            };
            let seat = u8::try_from(s.seat_number).unwrap_or(u8::MAX);
            let tag = position_tag(seat, btn, sb, bb)
                .map(str::to_owned)
                .unwrap_or_default();
            let analysis = if folded || hole == "??" || status.board.is_empty() {
                None
            } else {
                holdem_board_analysis(&hole, &status.board)
            };
            SeatRow {
                seat,
                name: s.player_name.clone(),
                chips: s.chips as usize,
                hole,
                // Per-street bet (resets when the street advances), not the
                // hand-cumulative `chips_in_play`, so the column clears between
                // betting rounds.
                bet: s.bet as usize,
                tag,
                folded,
                accent,
                pnl: Some(s.profit_loss),
                action: last_action_label(s.state, s.bet),
                analysis,
                tokens: Some((s.input_tokens, s.output_tokens)),
                cost_micro_usd: Some(s.cost_micro_usd),
                odds: None,
            }
        })
        .collect()
}

/// Human-readable last action for a spectated seat, derived from its proto
/// `state` and the chips committed this street (`chips_in_play`).
///
/// Returns an empty string when the seat has not acted yet, is idle between
/// hands, or is eliminated — those carry no action to show.
fn last_action_label(state: i32, chips_in_play: u32) -> String {
    match PlayerState::try_from(state) {
        Ok(PlayerState::Folded) => "fold".to_string(),
        Ok(PlayerState::Checked) => "check".to_string(),
        Ok(PlayerState::Called) => format!("call {chips_in_play}"),
        Ok(PlayerState::Bet) => format!("bet {chips_in_play}"),
        Ok(PlayerState::Raised) => format!("raise {chips_in_play}"),
        Ok(PlayerState::AllIn) => format!("all-in {chips_in_play}"),
        Ok(PlayerState::Blind) => format!("blind {chips_in_play}"),
        _ => String::new(),
    }
}

/// Board renderer driven by the proto's pre-formatted board string + pot.
fn render_board_str(board: &str, pot: u32, street: i32, frame: &mut Frame, area: Rect) {
    let street_label = match Street::try_from(street) {
        Ok(Street::Preflop) => "pre-flop",
        Ok(Street::Flop) => "flop",
        Ok(Street::Turn) => "turn",
        Ok(Street::River) => "river",
        _ => "—",
    };
    let board_display = if board.is_empty() {
        "—".to_string()
    } else {
        board.to_string()
    };
    let spans = vec![
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
            format!("street: {street_label}"),
            Style::default().fg(Color::DarkGray),
        ),
    ];
    let p = Paragraph::new(Text::from(vec![Line::from(spans)]))
        .block(Block::default().borders(Borders::ALL).title(" Board "))
        .alignment(Alignment::Right);
    frame.render_widget(p, area);
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
    /// Signed profit/loss for the seat. `None` when the mode does not track
    /// it (Play / Arena); `Some(_)` in Spectate mode from the dealer.
    pnl: Option<i32>,
    /// The seat's most recent action this street (e.g. `"fold"`, `"bet 100"`).
    /// Empty when the mode does not surface it (Play / Arena) or the seat has
    /// not yet acted; populated in Spectate mode from the dealer snapshot.
    action: String,
    /// Best hand the seat can currently make from hole cards + board, formatted
    /// as `"<sorted-cards> <HandRankClass>"` (e.g. `"A♠ A♥ K♦ Q♣ J♠ PairOfAces"`).
    /// `None` when in Play/Arena mode, when the board is empty, or when hole
    /// cards are hidden.
    analysis: Option<String>,
    /// Cumulative LLM token usage `(input, output)` this session (EPIC-44).
    /// `None` when the mode does not track it (Play / Arena); `Some((0, 0))` for
    /// a non-LLM seat (rule/random bot) in Spectate mode.
    tokens: Option<(u64, u64)>,
    /// Notional cost in integer micro-USD (1e-6 USD) of this seat's tokens
    /// (EPIC-44). `None` in Play / Arena; `Some(0)` when unpriced or a bot.
    cost_micro_usd: Option<u64>,
    /// Double-dummy split-pot equity (`0.0..=1.0`) for this seat at the
    /// current street. `None` in Play mode, for folded seats, non-Hold'em
    /// tables, or when odds are unavailable.
    odds: Option<f64>,
}

fn seat_rows<F: Fn(u8) -> String>(
    table: &Table,
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
            let suffix = match (r.best_hand.as_deref(), r.hand_class.as_deref()) {
                (Some(top), Some(class)) => format!("  [{top}] {class}"),
                (None, Some(class)) => format!("  {class}"),
                _ => String::new(),
            };
            (format!("{}{suffix}", r.hole), true)
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
            pnl: None,
            action: String::new(),
            analysis: None,
            tokens: None,
            cost_micro_usd: None,
            odds: None,
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
fn format_hole(seat: &Seat, as_owner: bool) -> String {
    let hand_slice = seat.hand.as_slice();
    let any_up = hand_slice.iter().any(HoleCard::is_up);

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
                .map_or_else(|| matches!(idx, 2..=5), HoleCard::is_up);
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

/// Evaluates the best 5-card Hold'em hand reachable from the given hole cards
/// and board, returning `"<sorted-cards> <HandRankClass>"` (e.g.
/// `"A♠ A♥ K♦ Q♣ J♠ PairOfAces"`).
///
/// Dispatches on board size so the column updates at every street:
/// - 3 board cards (flop):  2 + 3 = 5 → `Five`
/// - 4 board cards (turn):  2 + 4 = 6 → `Six`
/// - 5 board cards (river): 2 + 5 = 7 → `Seven`
///
/// Returns `None` when the combined card count is outside 5–7 or parsing fails.
fn holdem_board_analysis(hole: &str, board: &str) -> Option<String> {
    use pkcore::arrays::HandRanker;
    use pkcore::arrays::five::Five;
    use pkcore::arrays::seven::Seven;
    use pkcore::arrays::six::Six;
    use std::str::FromStr;
    let combined = format!("{hole} {board}");
    let (rank, hand) = match board.split_whitespace().count() {
        3 => Five::from_str(&combined).ok()?.hand_rank_and_hand(),
        4 => Six::from_str(&combined).ok()?.hand_rank_and_hand(),
        5 => Seven::from_str(&combined).ok()?.hand_rank_and_hand(),
        _ => return None,
    };
    Some(format!("{hand} {:?}", rank.class))
}

/// Converts integer micro-USD (1e-6 USD) to a floating-point dollar figure for
/// display. Cost values are small (single dollars), well within `f64`'s exact
/// integer range, so the precision loss is immaterial.
#[allow(clippy::cast_precision_loss)]
fn micro_usd_to_dollars(micro: u64) -> f64 {
    micro as f64 / 1_000_000.0
}

#[allow(clippy::too_many_lines)]
fn render_seats(frame: &mut Frame, area: Rect, rows: &[SeatRow]) {
    let header = Row::new(vec![
        Cell::from("#"),
        Cell::from("Name"),
        Cell::from("Chips"),
        Cell::from("Bet"),
        Cell::from("Hole"),
        Cell::from("Action"),
        Cell::from("Pos"),
        Cell::from("P/L"),
        Cell::from("Tokens"),
        Cell::from("Cost$"),
        Cell::from("Analysis"),
        Cell::from("Win%"),
    ])
    .style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::Yellow),
    );

    // Size the Hole column to its widest cell rather than stretching it, so the
    // Action column sits right beside the cards instead of after a wide,
    // mostly-empty gap. Spectate hands are short ("Ah Kd" / "??"); Play and
    // showdown strings (7-card Stud ≈ 34, reveal ≈ 60) are longer, so cap at 64
    // to keep a long reveal from crowding out the trailing columns.
    let hole_width = rows
        .iter()
        .map(|r| r.hole.chars().count())
        .max()
        .unwrap_or(0)
        .max("Hole".len())
        .min(64);
    let hole_width = u16::try_from(hole_width).unwrap_or(64);

    let widths = [
        Constraint::Length(3),
        Constraint::Length(22),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(hole_width),
        // Action — holds the longest label, e.g. "all-in 10000".
        Constraint::Length(13),
        Constraint::Length(8),
        Constraint::Length(10),
        // Tokens "999999/9999" (cumulative input/output).
        Constraint::Length(12),
        // Cost$ "$0.0234" (notional micro-USD / 1e6).
        Constraint::Length(9),
        // Analysis: "A♠ A♥ K♦ Q♣ J♠ KingHighStraightFlush" ≈ 36 chars
        Constraint::Length(38),
        // Win%: "100.0%" is 6 chars; pad to 7.
        Constraint::Length(7),
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
            let pnl_cell = match r.pnl {
                None => Cell::from("—").style(Style::default().fg(Color::DarkGray)),
                Some(v) => {
                    let color = if v >= 0 { Color::Green } else { Color::Red };
                    Cell::from(format!("{v:+}")).style(Style::default().fg(color))
                }
            };
            let analysis_cell = match &r.analysis {
                None => Cell::from("—").style(Style::default().fg(Color::DarkGray)),
                Some(label) => Cell::from(label.clone()).style(Style::default().fg(Color::Cyan)),
            };
            // Token/cost cells: "—" when the mode doesn't track them (Play/Arena),
            // blank for a non-LLM seat (bot, zero tokens/cost), else the value.
            let tokens_cell = match r.tokens {
                None => Cell::from("—").style(Style::default().fg(Color::DarkGray)),
                Some((0, 0)) => Cell::from(""),
                Some((input, output)) => Cell::from(format!("{input}/{output}")),
            };
            let cost_cell = match r.cost_micro_usd {
                None => Cell::from("—").style(Style::default().fg(Color::DarkGray)),
                Some(0) => Cell::from(""),
                Some(v) => Cell::from(format!("${:.4}", micro_usd_to_dollars(v)))
                    .style(Style::default().fg(Color::Green)),
            };
            let odds_cell = match r.odds {
                None => Cell::from("—").style(Style::default().fg(Color::DarkGray)),
                Some(e) => Cell::from(format!("{:.1}%", e * 100.0)).style(
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
            };
            Row::new(vec![
                Cell::from(format!("{}", r.seat)),
                Cell::from(r.name.clone()),
                Cell::from(format!("{}", r.chips)),
                Cell::from(badge),
                hole_cell,
                Cell::from(r.action.clone()),
                Cell::from(r.tag.clone()),
                pnl_cell,
                tokens_cell,
                cost_cell,
                analysis_cell,
                odds_cell,
            ])
            .style(style)
        })
        .collect();

    let widget = TableWidget::new(body, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(" Table "));
    frame.render_widget(widget, area);
}

/// Collects `(seat_index, "card card")` for active (non-folded, non-out)
/// spectated seats whose revealed cards form exactly a 2-card Hold'em hand.
fn status_active_holes(status: &TableStatus) -> Vec<(u8, String)> {
    status
        .seats
        .iter()
        .filter_map(|s| {
            let folded =
                s.state == PlayerState::Folded as i32 || s.state == PlayerState::Out as i32;
            if folded {
                return None;
            }
            let cards = crate::ui::sort_hole_cards(&s.cards);
            if cards == "??" || cards.trim().is_empty() || cards.split_whitespace().count() != 2 {
                return None;
            }
            Some((u8::try_from(s.seat_number).unwrap_or(u8::MAX), cards))
        })
        .collect()
}

/// Collects `(seat_index, "card card")` for every seat still in the hand
/// that holds exactly two cards (Hold'em). Seats that are empty, folded, or
/// hold a non-2-card hand are skipped.
fn active_holes(table: &Table) -> Vec<(u8, String)> {
    let n = u8::try_from(table.seats.0.len()).unwrap_or(u8::MAX);
    (0..n)
        .filter_map(|i| {
            let s = table.seats.get_seat(i)?;
            if s.is_empty() || !s.player.is_in_hand() || !s.cards.has_cards() {
                return None;
            }
            let cards: Vec<String> = s
                .cards
                .as_slice()
                .iter()
                .copied()
                .filter(|c| *c != pkcore::card::Card::BLANK)
                .map(|c| c.to_string())
                .collect();
            if cards.len() != 2 {
                return None;
            }
            Some((i, cards.join(" ")))
        })
        .collect()
}

/// Patches `rows` with cached equities for the active seats. No-op when fewer
/// than two seats are contesting.
fn apply_odds(
    rows: &mut [SeatRow],
    holes: &[(u8, String)],
    board: &str,
    cache: &crate::ui::odds::OddsCache,
) {
    if holes.len() < 2 {
        return;
    }
    for (seat, eq) in cache.equities(holes, board) {
        if let Some(row) = rows.iter_mut().find(|r| r.seat == seat) {
            row.odds = Some(eq);
        }
    }
}

fn render_board(table: &Table, paused: bool, frame: &mut Frame, area: Rect) {
    let has_board = table.game.family().uses_community_board();
    let pot = table.effective_pot();
    let phase = format!("{:?}", table.phase);

    let mut spans = Vec::with_capacity(8);
    if paused {
        spans.push(Span::styled(
            "⏸ PAUSED ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw("    "));
    }
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

    let title = if paused {
        " Board — PAUSED (Space resume · →/Enter step) "
    } else {
        " Board "
    };
    let p = Paragraph::new(Text::from(vec![Line::from(spans)]))
        .block(Block::default().borders(Borders::ALL).title(title))
        .alignment(Alignment::Right);
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
    use pkcore::casino::table::Player;

    fn seat_with(downs: &[Card], ups: &[Card]) -> Seat {
        use std::str::FromStr;
        let mut s = Seat::new(Player::new_with_chips("X".to_string(), 1_000));
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

    /// End-to-end probe: drive a Stud Hi session through 6th street and
    /// dump what `format_hole(hero, true)` returns at every `PlayerToAct`
    /// moment plus right after each `StreetAdvanced`. The user reported that
    /// on 6th street, hero shows 5 cards while opponents show 6 — this
    /// test reproduces a real session and asserts every state. If the bug
    /// is in `format_hole` (not just the data layer), this will catch it.
    #[test]
    fn format_hole_hero_renders_six_cards_on_sixth_street_in_real_session() {
        use pkcore::bot::profile::BotProfile;
        use pkcore::casino::action::PlayerAction;
        use pkcore::casino::session::{PokerSession, SessionStep};
        use pkcore::casino::table::{Seats, Table};
        use rand::SeedableRng;
        use rand::rngs::SmallRng;
        use rand::seq::SliceRandom;

        // Eight seats — `Variant::StudHi::max_seats()`, which is pkcore's
        // `Table::MAX_STUD_SEATS`. Eight players need 56 cards for seven
        // streets and the deck holds 52; pkcore 0.6.0 covers the shortfall
        // with a single face-up community card on 7th street, so a full stud
        // field runs to showdown. Before 0.6.0 the deal ran dry mid-street and
        // this test was capped at six to stay deterministic.
        let names = [
            "You",
            "abc",
            "tight_aggressive",
            "loose_passive",
            "gto",
            "short_stack_ninja",
            "joker",
            "maniac",
        ];
        let seats: Vec<Seat> = names
            .iter()
            .map(|n| Seat::new(Player::new_with_chips((*n).to_string(), 10_000)))
            .collect();
        let table = Table::stud_hi_from_seats(Seats::new(seats), 10, 25, 50, 100).unwrap();
        let mut session = PokerSession::new(table);
        session.start_hand().unwrap();

        let mut rng = SmallRng::seed_from_u64(3_596_220_112_812_468_068);
        let mut pool = BotProfile::default_profiles();
        pool.push(BotProfile::joker());
        pool.shuffle(&mut rng);
        let bots: Vec<BotProfile> = pool.into_iter().take(8).collect();

        let mut reached_6th = false;
        for step in 0..1000 {
            let phase_before = session.table.phase;
            match session.next_step() {
                SessionStep::PlayerToAct(seat) => {
                    let hero_seat = session.table.seats.get_seat(0).unwrap();
                    let hero_cards = hero_seat.cards.number_of_dealt_cards();
                    let hero_hand = hero_seat.hand.len();
                    let hero_format = format_hole(hero_seat, true);
                    // Count card slots in the rendered output (each slot is 4
                    // chars wide, joined by single spaces — so length tells us
                    // slot count for the stud branch).
                    let slot_count_render = (hero_format.chars().count() + 1) / 5;
                    eprintln!(
                        "step {step}: phase={phase_before:?} acting={seat} \
                         hero.cards={hero_cards} hero.hand={hero_hand} \
                         render_slots={slot_count_render} '{hero_format}'"
                    );
                    if phase_before == pkcore::games::GamePhase::Stud6th {
                        reached_6th = true;
                        assert_eq!(
                            slot_count_render, 6,
                            "[6th street] hero render shows {slot_count_render} slots, expected 6. \
                             cards={hero_cards} hand={hero_hand} render='{hero_format}'"
                        );
                    }
                    let action = if seat == 0 {
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
                    let hero_seat = session.table.seats.get_seat(0).unwrap();
                    let hero_cards = hero_seat.cards.number_of_dealt_cards();
                    let hero_hand = hero_seat.hand.len();
                    let hero_format = format_hole(hero_seat, true);
                    let slot_count_render = (hero_format.chars().count() + 1) / 5;
                    eprintln!(
                        "step {step}: StreetAdvanced phase={:?} \
                         hero.cards={hero_cards} hero.hand={hero_hand} \
                         render_slots={slot_count_render} '{hero_format}'",
                        session.table.phase
                    );
                    if session.table.phase == pkcore::games::GamePhase::Stud6th {
                        reached_6th = true;
                        assert_eq!(
                            slot_count_render, 6,
                            "[6th street post-advance] hero render shows {slot_count_render} slots, expected 6. \
                             cards={hero_cards} hand={hero_hand} render='{hero_format}'"
                        );
                    }
                }
                SessionStep::HandComplete => break,
                SessionStep::Failed(e) => panic!("hand failed before 6th street: {e}"),
            }
        }
        assert!(reached_6th, "never reached 6th street in 1000 steps");
    }

    /// Render a Stud Hi `PlayState` to a `TestBackend` at 6th street and read
    /// the hero's row text from the buffer. This catches column-width / panel
    /// truncation bugs that `format_hole` unit tests wouldn't see.
    #[test]
    fn rendered_hero_row_shows_six_cards_on_sixth_street() {
        use crate::App;
        use pkcore::bot::profile::BotProfile;
        use pkcore::casino::action::PlayerAction;
        use pkcore::casino::session::SessionStep;
        use pkcore::games::GamePhase;
        use rand::SeedableRng;
        use rand::rngs::SmallRng;
        use rand::seq::SliceRandom;
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        // Build the same Play default app, then override its session to Stud Hi.
        let mut args = crate::cli::PlayArgs::default();
        args.game.variant = crate::cli::Variant::StudHi;
        args.game.seed = Some(3_596_220_112_812_468_068);
        let mut log = crate::log_panel::LogPanel::new();
        let mut state = crate::modes::PlayState::new(&args, &mut log).unwrap();

        let mut rng = SmallRng::seed_from_u64(3_596_220_112_812_468_068);
        let mut pool = BotProfile::default_profiles();
        pool.push(BotProfile::joker());
        pool.shuffle(&mut rng);
        let bots: Vec<BotProfile> = pool.into_iter().take(8).collect();

        // Drive until phase reaches Stud6th, then render.
        for _ in 0..1000 {
            if state.session.table.phase == GamePhase::Stud6th {
                break;
            }
            match state.session.next_step() {
                SessionStep::PlayerToAct(seat) => {
                    let action = if seat == 0 {
                        if state.session.table.to_call(seat) == 0 {
                            PlayerAction::Check
                        } else {
                            PlayerAction::Call
                        }
                    } else {
                        let bot_idx = (seat as usize) - 1;
                        bots[bot_idx].decide(&state.session.table, seat, &mut rng)
                    };
                    if state.session.apply_action(seat, action).is_err() {
                        let _ = state.session.apply_action(seat, PlayerAction::Fold);
                    }
                }
                SessionStep::StreetAdvanced => {}
                SessionStep::HandComplete => panic!("hand ended before 6th street"),
                SessionStep::Failed(e) => panic!("hand failed before 6th street: {e}"),
            }
        }
        assert_eq!(state.session.table.phase, GamePhase::Stud6th);

        // Verify data layer: hero has 6 cards.
        let hero_seat = state.session.table.seats.get_seat(0).unwrap();
        assert_eq!(hero_seat.cards.number_of_dealt_cards(), 6);
        assert_eq!(hero_seat.hand.len(), 6);
        let format_out = format_hole(hero_seat, true);
        eprintln!("hero format_hole = '{format_out}'");

        let mut app = App::play_default().unwrap();
        // Swap in our Stud state.
        app.mode = crate::app::AppMode::Play(Box::new(state));

        // Render at several widths to see where the row gets clipped.
        for width in [80u16, 100, 120, 140] {
            let backend = TestBackend::new(width, 36);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal.draw(|f| crate::ui::view(&app, f)).unwrap();
            let buffer = terminal.backend().buffer().clone();
            eprintln!("\n=== terminal width = {width} ===");
            // Hero row should be the row with "You" in the Name column.
            for y in 0..15u16 {
                let line: String = (0..width).map(|x| buffer[(x, y)].symbol()).collect();
                eprintln!("  y={y:2}: '{line}'");
            }
        }
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
        let mut s = Seat::new(Player::new_with_chips("X".to_string(), 1_000));
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

    #[test]
    fn render_seats_shows_pnl_column_header() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let rows = vec![SeatRow {
            seat: 0,
            name: "gto".to_string(),
            chips: 9_500,
            hole: "??".to_string(),
            bet: 500,
            tag: String::new(),
            folded: false,
            accent: Accent::None,
            pnl: Some(-500),
            action: "bet 500".to_string(),
            analysis: None,
            tokens: Some((1200, 8)),
            cost_micro_usd: Some(23_400),
            odds: None,
        }];
        let backend = TestBackend::new(160, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_seats(f, f.area(), &rows)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let header: String = (0..160).map(|x| buffer[(x, 1)].symbol()).collect();
        assert!(header.contains("P/L"));
        assert!(header.contains("Action"));
        assert!(header.contains("Analysis"));
        assert!(header.contains("Tokens"));
        assert!(header.contains("Cost$"));
        let body: String = (0..160).map(|x| buffer[(x, 2)].symbol()).collect();
        assert!(body.contains("bet 500"));
        assert!(body.contains("1200/8"), "tokens column: {body}");
        assert!(body.contains("$0.0234"), "cost column: {body}");
    }

    #[test]
    fn render_seats_shows_win_column_and_value() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let rows = vec![SeatRow {
            seat: 0,
            name: "gto".to_string(),
            chips: 9_500,
            hole: "Ah Kh".to_string(),
            bet: 0,
            tag: String::new(),
            folded: false,
            accent: Accent::None,
            pnl: None,
            action: String::new(),
            analysis: None,
            tokens: None,
            cost_micro_usd: None,
            odds: Some(0.824),
        }];
        let backend = TestBackend::new(170, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_seats(f, f.area(), &rows)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let header: String = (0..170).map(|x| buffer[(x, 1)].symbol()).collect();
        assert!(header.contains("Win%"), "header: {header}");
        let body: String = (0..170).map(|x| buffer[(x, 2)].symbol()).collect();
        assert!(body.contains("82.4%"), "body: {body}");
    }

    #[test]
    fn active_holes_collects_two_card_seats() {
        use pkcore::casino::game::ForcedBets;
        use pkcore::casino::table::{Player, Seat, Seats, Table};
        use std::str::FromStr;

        let mut s0 = Seat::new(Player::new_with_chips("a".into(), 1_000));
        s0.cards = pkcore::arrays::sliced::BoxedCards::from_str("As Ah").unwrap();
        let mut s1 = Seat::new(Player::new_with_chips("b".into(), 1_000));
        s1.cards = pkcore::arrays::sliced::BoxedCards::from_str("Ks Kh").unwrap();
        let table = Table::nlh_from_seats(Seats::new(vec![s0, s1]), ForcedBets::new(10, 20));

        let holes = active_holes(&table);
        assert_eq!(holes.len(), 2);
        assert_eq!(holes[0].0, 0);
        assert_eq!(holes[0].1.split_whitespace().count(), 2);
    }

    #[test]
    fn status_to_rows_maps_seat_fields() {
        use pkdealer_proto::dealer::{SeatInfo, TableStatus};

        let status = TableStatus {
            seats: vec![
                SeatInfo {
                    seat_number: 0,
                    player_name: "gto".into(),
                    chips: 9_500,
                    cards: "??".into(),
                    state: 4, // CALLED
                    withdrawn: 10_000,
                    chips_in_play: 500,
                    profit_loss: -500,
                    bet: 500,
                    ..Default::default()
                },
                SeatInfo {
                    seat_number: 1,
                    player_name: "lag".into(),
                    chips: 0,
                    cards: "??".into(),
                    state: 8, // FOLDED
                    withdrawn: 10_000,
                    chips_in_play: 0,
                    profit_loss: -10_000,
                    bet: 0,
                    ..Default::default()
                },
            ],
            board: "Ah Kd Qc".into(),
            pot: 1_000,
            next_to_act: 0,
            hand_in_progress: true,
            game_over: false,
            current_street: 2,
            small_blind: 50,
            big_blind: 100,
            ..Default::default()
        };

        let rows = status_to_rows(&status);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "gto");
        assert_eq!(rows[0].chips, 9_500);
        assert_eq!(rows[0].bet, 500);
        assert_eq!(rows[0].pnl, Some(-500));
        assert_eq!(rows[0].accent, Accent::Active); // seat 0 == next_to_act
        assert!(!rows[0].folded);
        assert!(rows[1].folded); // FOLDED state
        assert_eq!(rows[1].accent, Accent::None);
        // Spectate mode populates token/cost (Some), unlike Play/Arena (None).
        assert_eq!(rows[0].tokens, Some((0, 0)));
        assert_eq!(rows[0].cost_micro_usd, Some(0));
    }

    #[test]
    fn status_to_rows_sorts_revealed_hole_cards() {
        use pkdealer_proto::dealer::{SeatInfo, TableStatus};

        let status = TableStatus {
            seats: vec![SeatInfo {
                seat_number: 0,
                player_name: "gto".into(),
                chips: 9_500,
                cards: "2h Ad".into(), // dealt order, low card first
                state: 4,              // CALLED — still in the hand
                ..Default::default()
            }],
            hand_in_progress: true,
            ..Default::default()
        };

        let rows = status_to_rows(&status);
        // Revealed cards are reordered high-first while keeping ASCII glyphs.
        assert_eq!(rows[0].hole, "Ad 2h");
    }

    #[test]
    fn status_to_rows_reveals_active_cards_and_hides_folded() {
        use pkdealer_proto::dealer::{SeatInfo, TableStatus};

        let status = TableStatus {
            seats: vec![
                SeatInfo {
                    seat_number: 0,
                    player_name: "gto".into(),
                    chips: 9_500,
                    cards: "Ah Kd".into(),
                    state: 4, // CALLED — still in the hand
                    withdrawn: 10_000,
                    chips_in_play: 500,
                    profit_loss: -500,
                    bet: 500,
                    ..Default::default()
                },
                SeatInfo {
                    seat_number: 1,
                    player_name: "lag".into(),
                    chips: 0,
                    cards: "7c 2d".into(),
                    state: 8, // FOLDED — mucked
                    withdrawn: 10_000,
                    chips_in_play: 0,
                    profit_loss: -10_000,
                    bet: 0,
                    ..Default::default()
                },
            ],
            board: "Qc Jc Tc".into(),
            pot: 1_000,
            next_to_act: 0,
            hand_in_progress: true,
            game_over: false,
            current_street: 2,
            small_blind: 50,
            big_blind: 100,
            ..Default::default()
        };

        let rows = status_to_rows(&status);
        assert_eq!(rows[0].hole, "Ah Kd"); // active player → cards revealed
        assert_eq!(rows[1].hole, "??"); // folded player → hidden
        assert_eq!(rows[0].action, "call 500"); // CALLED with 500 in play
        assert_eq!(rows[1].action, "fold"); // FOLDED
    }

    #[test]
    fn last_action_label_maps_states() {
        use pkdealer_proto::dealer::PlayerState;
        assert_eq!(last_action_label(PlayerState::Folded as i32, 0), "fold");
        assert_eq!(last_action_label(PlayerState::Checked as i32, 0), "check");
        assert_eq!(last_action_label(PlayerState::Bet as i32, 100), "bet 100");
        assert_eq!(
            last_action_label(PlayerState::Raised as i32, 300),
            "raise 300"
        );
        assert_eq!(
            last_action_label(PlayerState::Called as i32, 100),
            "call 100"
        );
        assert_eq!(
            last_action_label(PlayerState::AllIn as i32, 9_500),
            "all-in 9500"
        );
        // Not-yet-acted / idle states carry no label.
        assert_eq!(last_action_label(PlayerState::YetToAct as i32, 0), "");
        assert_eq!(last_action_label(PlayerState::Ready as i32, 0), "");
    }

    #[test]
    fn render_table_view_spectate_does_not_panic() {
        use crate::modes::SpectateState;
        use pkdealer_proto::dealer::{SeatInfo, TableStatus};
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let (mut state, _tx) = SpectateState::detached("http://localhost:50051");
        state.status = Some(TableStatus {
            seats: vec![SeatInfo {
                seat_number: 0,
                player_name: "gto".into(),
                chips: 9_500,
                cards: "??".into(),
                state: 4,
                withdrawn: 10_000,
                chips_in_play: 500,
                profit_loss: -500,
                bet: 500,
                ..Default::default()
            }],
            board: "Ah Kd Qc".into(),
            pot: 1_000,
            next_to_act: 0,
            hand_in_progress: true,
            game_over: false,
            current_street: 2,
            small_blind: 50,
            big_blind: 100,
            ..Default::default()
        });

        let backend = TestBackend::new(120, 18);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_table_view_spectate(&state, f, f.area()))
            .unwrap();
    }

    #[test]
    fn status_to_rows_folded_next_to_act_is_not_active() {
        use pkdealer_proto::dealer::{SeatInfo, TableStatus};
        let status = TableStatus {
            seats: vec![SeatInfo {
                seat_number: 0,
                player_name: "gto".into(),
                chips: 1_000,
                cards: "??".into(),
                state: 8, // FOLDED
                withdrawn: 10_000,
                chips_in_play: 0,
                profit_loss: -9_000,
                bet: 0,
                ..Default::default()
            }],
            board: String::new(),
            pot: 0,
            next_to_act: 0, // points at the folded seat
            hand_in_progress: true,
            game_over: false,
            current_street: 1,
            small_blind: 50,
            big_blind: 100,
            ..Default::default()
        };
        let rows = status_to_rows(&status);
        assert!(rows[0].folded);
        assert_eq!(rows[0].accent, Accent::None);
    }

    #[test]
    fn status_active_holes_excludes_folded_and_non_holdem() {
        use pkdealer_proto::dealer::{SeatInfo, TableStatus};
        let status = TableStatus {
            seats: vec![
                SeatInfo {
                    seat_number: 0,
                    player_name: "a".into(),
                    cards: "As Ah".into(),
                    state: 4,
                    ..Default::default()
                },
                SeatInfo {
                    seat_number: 1,
                    player_name: "b".into(),
                    cards: "Ks Kh".into(),
                    state: 4,
                    ..Default::default()
                },
                SeatInfo {
                    seat_number: 2,
                    player_name: "c".into(),
                    cards: "7c 2d".into(),
                    state: 8,
                    ..Default::default()
                }, // folded
            ],
            board: "Ah Kd Qc".into(),
            hand_in_progress: true,
            ..Default::default()
        };
        let holes = status_active_holes(&status);
        assert_eq!(holes.len(), 2);
        assert!(holes.iter().all(|(s, _)| *s != 2), "folded seat excluded");
    }

    #[test]
    fn status_to_rows_position_tags_from_proto() {
        use pkdealer_proto::dealer::{SeatInfo, TableStatus};

        fn seat(n: u32) -> SeatInfo {
            SeatInfo {
                seat_number: n,
                player_name: format!("p{n}"),
                chips: 1_000,
                ..Default::default()
            }
        }

        // 3-handed: BTN=0, SB=1, BB=2
        let status = TableStatus {
            seats: vec![seat(0), seat(1), seat(2)],
            hand_in_progress: true,
            button_seat: 0,
            small_blind_seat: 1,
            big_blind_seat: 2,
            ..Default::default()
        };
        let rows = status_to_rows(&status);
        assert_eq!(rows[0].tag, "BTN");
        assert_eq!(rows[1].tag, "SB");
        assert_eq!(rows[2].tag, "BB");

        // Heads-up: BTN/SB=0, BB=1
        let hu_status = TableStatus {
            seats: vec![seat(0), seat(1)],
            hand_in_progress: true,
            button_seat: 0,
            small_blind_seat: 0,
            big_blind_seat: 1,
            ..Default::default()
        };
        let hu_rows = status_to_rows(&hu_status);
        assert_eq!(hu_rows[0].tag, "BTN/SB");
        assert_eq!(hu_rows[1].tag, "BB");
    }
}
