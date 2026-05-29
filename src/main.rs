//! Binary entry point for `pktui`.
//!
//! Responsibilities:
//!
//! 1. Parse the CLI (`pktui [play|arena|replay]`).
//! 2. Build the [`App`](pktui::App).
//! 3. Initialise the terminal (raw mode + alt screen) and panic hook.
//! 4. Run the event loop: poll → reduce → render.
//! 5. Restore the terminal on exit (success or panic).
//!
//! The bin stays thin on purpose so the testable surface lives in `lib.rs`.

use std::time::{Duration, Instant};

use clap::Parser;

use pktui::cli::Cli;
use pktui::error::Result;
use pktui::event::next_event;
use pktui::update::{event_to_msg, update};
use pktui::{App, tui, ui};

const TICK_MS: u64 = 50;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let command = cli.resolved();

    let mut app = App::new(command).map_err(|e| color_eyre::eyre::eyre!(e.to_string()))?;

    tui::install_panic_hook();
    let mut terminal = tui::init().map_err(|e| color_eyre::eyre::eyre!(e.to_string()))?;
    let result = run(&mut terminal, &mut app);
    let _ = tui::restore(&mut terminal);

    result.map_err(|e| color_eyre::eyre::eyre!(e.to_string()))?;
    Ok(())
}

fn run(terminal: &mut tui::Tui, app: &mut App) -> Result<()> {
    let mut last_tick = Instant::now();
    let tick = Duration::from_millis(TICK_MS);

    while !app.should_quit {
        terminal.draw(|f| ui::view(app, f))?;
        app.poll_spectate();
        let event = next_event(tick, &mut last_tick)?;
        let msg = event_to_msg(app, &event);
        update(app, msg)?;
    }
    Ok(())
}
