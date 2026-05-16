//! Help overlay (centered modal with keyboard shortcuts).
//!
//! Triggered by `?`. Drawn after the main view so it sits on top.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

/// Renders the help overlay centered inside `area`.
pub fn render_overlay(frame: &mut Frame, area: Rect) {
    let popup = centered(60, 18, area);
    frame.render_widget(Clear, popup);

    let body = Text::from(vec![
        Line::from(Span::styled(
            "Keyboard shortcuts",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
        Line::raw("Global"),
        Line::raw("  ?  toggle this help"),
        Line::raw("  q / Ctrl+C  quit"),
        Line::raw(""),
        Line::raw("Play (your turn)"),
        Line::raw("  f fold   k check   c call   a all-in"),
        Line::raw("  b bet / r raise (uses bet amount)"),
        Line::raw("  1 min   2 half-pot   3 pot   digits / +/- adjust"),
        Line::raw("  Enter   confirm bet/raise or deal next hand"),
        Line::raw(""),
        Line::raw("Arena"),
        Line::raw("  + faster   - slower"),
        Line::raw(""),
        Line::raw("Replay"),
        Line::raw("  n / p   next/prev street"),
        Line::raw("  N / P   next/prev hand   (Enter = next hand)"),
    ]);

    let p = Paragraph::new(body)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help ")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(p, popup);
}

fn centered(width: u16, height: u16, area: Rect) -> Rect {
    let h = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area)[1];
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((area.width.saturating_sub(width)) / 2),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(h)[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn renders_in_small_area() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_overlay(f, f.area())).unwrap();
    }

    #[test]
    fn centered_inside_bounds() {
        let area = Rect::new(0, 0, 80, 24);
        let r = centered(40, 10, area);
        assert!(r.x + r.width <= area.x + area.width);
        assert!(r.y + r.height <= area.y + area.height);
    }

    #[test]
    fn centered_clamps_when_too_big() {
        let area = Rect::new(0, 0, 20, 6);
        let r = centered(60, 18, area);
        // Even when popup is bigger than area we should still produce a rect
        // strictly within bounds (height/width gets clipped to 0 by layout).
        assert!(r.x + r.width <= area.x + area.width || r.width == 0);
    }
}
