//! Renders the rolling log panel.
//!
//! Most recent lines appear at the bottom; the panel auto-scrolls.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::log_panel::{LogPanel, Severity};

/// Renders `log` into `area` as a bordered "Log" panel.
pub fn render(log: &LogPanel, frame: &mut Frame, area: Rect) {
    let max_lines = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = log
        .tail(max_lines)
        .iter()
        .map(|l| {
            let style = match l.severity {
                Severity::Info => Style::default().fg(Color::Gray),
                Severity::Action => Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
                Severity::Fold => Style::default().fg(Color::Red),
                Severity::Win => Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                Severity::Error => Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
            };
            Line::from(Span::styled(l.text.clone(), style))
        })
        .collect();
    let p = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(" Log "))
        .wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn renders_empty_log_without_panic() {
        let log = LogPanel::new();
        let backend = TestBackend::new(40, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(&log, f, f.area())).unwrap();
    }

    #[test]
    fn renders_with_lines() {
        let mut log = LogPanel::new();
        for i in 0..20 {
            log.push(Severity::Info, format!("line {i}"));
        }
        let backend = TestBackend::new(40, 8);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(&log, f, f.area())).unwrap();
    }
}
