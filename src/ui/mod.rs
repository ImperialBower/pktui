//! Render functions.
//!
//! The top-level [`view`] function takes the immutable [`App`] and draws to
//! a [`Frame`]. Internally it dispatches to one of the per-mode views
//! ([`table`], [`replay_view`]) and overlays the help dialog if enabled.

use std::fmt::Write as _;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, AppMode};

pub mod action_bar;
pub mod help;
pub mod log_view;
pub mod replay_view;
pub mod table;

/// Reorders a space-separated hole-card string into descending rank order
/// (the convention pkcore uses elsewhere via `sorted_display`), preserving
/// each token's original glyph style.
///
/// Spectator and replay card strings arrive in dealt order using ASCII suits
/// (e.g. `"2h Ad"`), whereas the live Play/Arena views already sort their
/// hands. This brings the other views into line *without* restyling cards to
/// Unicode the way `sorted_display` would — so a sorted `"Ad 2h"` still sits
/// consistently beside an ASCII board.
///
/// Returns the input unchanged when it has fewer than two tokens, or when any
/// token is not a recognizable card (e.g. the `"??"` muck placeholder) — so
/// unknown markers are never reordered or dropped.
///
/// # Examples
///
/// ```
/// use pktui::ui::sort_hole_cards;
///
/// assert_eq!(sort_hole_cards("2h Ad"), "Ad 2h");
/// assert_eq!(sort_hole_cards("??"), "??");
/// ```
#[must_use]
pub fn sort_hole_cards(cards: &str) -> String {
    use std::str::FromStr as _;

    // Pair each token with its parsed Card for ordering. A single token has
    // nothing to reorder, so short-circuit and avoid re-allocating.
    let tokens: Vec<&str> = cards.split_whitespace().collect();
    if tokens.len() < 2 {
        return cards.to_string();
    }

    let mut keyed: Vec<(pkcore::card::Card, &str)> = Vec::with_capacity(tokens.len());
    for token in &tokens {
        match pkcore::card::Card::from_str(token) {
            Ok(card) => keyed.push((card, token)),
            // Any non-card token (e.g. the "??" muck marker) means we can't
            // meaningfully order the hand — hand it back untouched.
            Err(_) => return cards.to_string(),
        }
    }

    // Descending by rank — the same orientation as `sorted_display`, but we
    // emit the original token so ASCII suits stay ASCII.
    keyed.sort_unstable_by_key(|(card, _)| std::cmp::Reverse(*card));
    keyed
        .iter()
        .map(|(_, token)| *token)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Renders the whole app to `frame`.
///
/// # Examples
///
/// ```no_run
/// use pktui::App;
/// // Real rendering needs a Terminal; this snippet documents the entry point.
/// let _app = App::play_default().unwrap();
/// ```
pub fn view(app: &App, frame: &mut Frame) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(14),   // main body (table / replay)
            Constraint::Length(8), // log
            Constraint::Length(4), // action bar (2 lines + borders)
        ])
        .split(area);

    render_header(app, frame, chunks[0]);

    match &app.mode {
        AppMode::Play(p) => table::render_table_view_play(p, frame, chunks[1]),
        AppMode::Arena(a) => table::render_table_view_arena(a, frame, chunks[1]),
        AppMode::Replay(r) => replay_view::render(r, frame, chunks[1]),
        AppMode::Spectate(s) => table::render_table_view_spectate(s, frame, chunks[1]),
    }

    log_view::render(&app.log, frame, chunks[2]);
    action_bar::render(app, frame, chunks[3]);

    if app.help_visible {
        help::render_overlay(frame, area);
    }
}

/// Picks the blinds to show in the spectate header.
///
/// Prefers the live `TableStatus` snapshot (which tracks blind-schedule
/// escalation) when it carries non-zero blinds, and otherwise falls back to
/// the once-fetched static `TableConfig`. Returns `None` when neither source
/// is available yet.
fn spectate_header_blinds(
    status: Option<&pkdealer_proto::dealer::TableStatus>,
    config: Option<&pkdealer_proto::dealer::TableConfig>,
) -> Option<(u32, u32)> {
    status
        .map(|st| (st.small_blind, st.big_blind))
        .filter(|&(sb, bb)| sb != 0 || bb != 0)
        .or_else(|| config.map(|c| (c.small_blind, c.big_blind)))
}

fn render_header(app: &App, frame: &mut Frame, area: Rect) {
    let mode = app.mode.label();
    let (subtitle, seed) = match &app.mode {
        AppMode::Play(p) => (
            format!(
                "blinds {}/{}  hand {}",
                p.session.table.forced.small_blind,
                p.session.table.forced.big_blind,
                p.session.hand_number
            ),
            Some(p.seed),
        ),
        AppMode::Arena(a) => (
            format!(
                "blinds {}/{}  hand {}  speed {}ms",
                a.session.table.forced.small_blind,
                a.session.table.forced.big_blind,
                a.session.hand_number,
                a.speed.as_millis()
            ),
            Some(a.seed),
        ),
        AppMode::Replay(r) => (
            format!("hand {}/{}", r.hand_index + 1, r.hand_count().max(1)),
            None,
        ),
        AppMode::Spectate(s) => {
            let mut subtitle = format!("{}  {}", s.endpoint, s.conn.label());
            if let Some((sb, bb)) = spectate_header_blinds(s.status.as_ref(), s.config.as_ref()) {
                let _ = write!(subtitle, "  blinds {sb}/{bb}");
            }
            (subtitle, None)
        }
    };
    let mut spans = vec![
        Span::styled(
            format!(" pktui · {mode} "),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(subtitle, Style::default().fg(Color::Gray)),
    ];
    if let Some(s) = seed {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("seed={s}"),
            Style::default().fg(Color::DarkGray),
        ));
    }
    spans.push(Span::raw("  "));
    spans.push(Span::styled(
        "?=help  q=quit",
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pkdealer_proto::dealer::{TableConfig, TableStatus};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn sort_hole_cards_orders_descending_by_rank() {
        // Dealt order in, high-card-first out — preserving ASCII glyphs.
        assert_eq!(sort_hole_cards("2h Ad"), "Ad 2h");
        assert_eq!(sort_hole_cards("7c Kc Ah"), "Ah Kc 7c");
    }

    #[test]
    fn sort_hole_cards_passes_through_unknown_tokens() {
        // The muck placeholder and any non-card string must come back verbatim.
        assert_eq!(sort_hole_cards("??"), "??");
        assert_eq!(sort_hole_cards("Ah ??"), "Ah ??");
        assert_eq!(sort_hole_cards(""), "");
    }

    #[test]
    fn sort_hole_cards_single_card_is_unchanged() {
        assert_eq!(sort_hole_cards("Ah"), "Ah");
    }

    #[test]
    fn spectate_header_blinds_prefers_live_status() {
        let status = TableStatus {
            small_blind: 200,
            big_blind: 400,
            ..Default::default()
        };
        let config = TableConfig {
            small_blind: 50,
            big_blind: 100,
            ..Default::default()
        };
        // Live status (escalated) wins over the once-fetched static config.
        assert_eq!(
            spectate_header_blinds(Some(&status), Some(&config)),
            Some((200, 400))
        );
    }

    #[test]
    fn spectate_header_blinds_falls_back_to_config_when_status_zero() {
        // A status snapshot from before any blinds are known (0/0) must not
        // mask the static config.
        let status = TableStatus::default();
        let config = TableConfig {
            small_blind: 50,
            big_blind: 100,
            ..Default::default()
        };
        assert_eq!(
            spectate_header_blinds(Some(&status), Some(&config)),
            Some((50, 100))
        );
    }

    #[test]
    fn spectate_header_blinds_none_when_nothing_known() {
        assert_eq!(spectate_header_blinds(None, None), None);
    }

    fn draw_and_assert(app: &App) {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| view(app, f)).unwrap();
        // No assertion on bytes — just verify rendering doesn't panic and the
        // top-left has the title.
        let buffer = terminal.backend().buffer().clone();
        let header = (0..30).map(|x| buffer[(x, 0)].symbol()).collect::<String>();
        assert!(header.contains("pktui"));
    }

    #[test]
    fn play_renders_without_panic() {
        let app = App::play_default().unwrap();
        draw_and_assert(&app);
    }

    #[test]
    fn arena_renders_without_panic() {
        let app = App::arena_default().unwrap();
        draw_and_assert(&app);
    }

    #[test]
    fn help_overlay_renders() {
        let mut app = App::play_default().unwrap();
        app.toggle_help();
        draw_and_assert(&app);
    }
}
