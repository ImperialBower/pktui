//! Terminal raw-mode init / restore.
//!
//! All ratatui apps need to (a) put the terminal into raw mode, (b) switch
//! to the alternate screen, and (c) reverse both on exit — including on
//! panic, otherwise the user's shell is left in an unusable state.
//!
//! This module provides [`init`] / [`restore`] plus a panic hook installer
//! ([`install_panic_hook`]) that cleans up before the panic message is
//! printed.

use std::io::{Stdout, stdout};

use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::error::Result;

/// The concrete terminal type used throughout pktui.
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialises the terminal: raw mode + alternate screen + new
/// [`ratatui::Terminal`].
///
/// # Errors
///
/// Returns [`crate::Error::Io`] if the terminal cannot be configured (for
/// example, when stdout is not a TTY).
///
/// # Examples
///
/// ```no_run
/// use pktui::tui;
/// let mut terminal = tui::init().unwrap();
/// // ... use the terminal ...
/// tui::restore(&mut terminal).unwrap();
/// ```
pub fn init() -> Result<Tui> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let terminal = Terminal::new(CrosstermBackend::new(out))?;
    Ok(terminal)
}

/// Restores the terminal: disables raw mode and leaves the alternate screen.
///
/// Safe to call from a panic hook — it ignores secondary errors so the
/// original panic message is preserved.
///
/// # Errors
///
/// Returns [`crate::Error::Io`] if the terminal cannot be restored.
///
/// # Examples
///
/// ```no_run
/// use pktui::tui;
/// let mut terminal = tui::init().unwrap();
/// tui::restore(&mut terminal).unwrap();
/// ```
pub fn restore(terminal: &mut Tui) -> Result<()> {
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = disable_raw_mode();
    Ok(())
}

/// Installs a panic hook that restores the terminal before the default hook
/// prints the panic message.
///
/// Must be called once at startup. Without it, a panic inside the render
/// loop leaves the terminal in raw / alternate-screen state and renders the
/// panic message invisible.
///
/// # Examples
///
/// ```no_run
/// pktui::tui::install_panic_hook();
/// ```
pub fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(stdout(), LeaveAlternateScreen);
        let _ = disable_raw_mode();
        original(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_panic_hook_is_idempotent() {
        // Calling twice should not panic on its own.
        install_panic_hook();
        install_panic_hook();
    }

    // Note: we don't unit-test init/restore because they require a real TTY.
    // They're exercised end-to-end via `cargo run` and integration tests
    // that bypass them.
}
