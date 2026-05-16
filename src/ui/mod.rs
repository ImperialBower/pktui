//! Render functions.
//!
//! The top-level [`view`] function takes the immutable [`App`] and draws to
//! a [`Frame`]. Internally it dispatches to one of the per-mode views
//! ([`table`], [`replay_view`]) and overlays the help dialog if enabled.

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
    }

    log_view::render(&app.log, frame, chunks[2]);
    action_bar::render(app, frame, chunks[3]);

    if app.help_visible {
        help::render_overlay(frame, area);
    }
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
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

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
