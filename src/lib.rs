//! pktui — a [`ratatui`](https://ratatui.rs) terminal client for the
//! [`pkcore`](https://crates.io/crates/pkcore) poker engine.
//!
//! pktui mirrors the user-facing flow of the
//! [pkarena0-web](https://github.com/ImperialBower/pkarena0-web) front end
//! (one-human-vs-eight-bots **Play** mode, all-bot **Arena** mode, YAML
//! **Replay** mode) but renders to the terminal using ratatui widgets instead
//! of an SVG table.
//!
//! # Architecture
//!
//! The crate is structured around an Elm-style **Model / Message / Update**
//! loop:
//!
//! * [`App`] is the model — it owns the [`PokerSession`](pkcore::casino::session::PokerSession),
//!   bot roster, hand log and per-mode UI state.
//! * [`Msg`](crate::update::Msg) is the message — every key press, tick or
//!   internal command is normalised into a `Msg`.
//! * [`update`](crate::update::update) is the pure(-ish) reducer — it advances
//!   the model and returns whether the app should keep running.
//! * [`ui::view`] is the render — it draws the current model
//!   to a [`ratatui::Frame`].
//!
//! # Quick start
//!
//! Run interactively:
//! ```text
//! cargo run -- play           # one human vs eight bots (default)
//! cargo run -- arena          # nine bots, watch-only
//! cargo run -- replay session.yaml
//! ```
//!
//! # Modules
//!
//! * [`app`] — the central [`App`] state.
//! * [`cli`] — clap command-line definitions.
//! * [`config`] — TOML config loaded from `~/.config/pktui/config.toml`.
//! * [`event`] — crossterm event polling with a tick timer for bot pacing.
//! * [`log_panel`] — the rolling event-log buffer.
//! * [`modes`] — per-mode initialisation (`play`, `arena`, `replay`).
//! * [`tui`] — terminal raw-mode init / restore.
//! * [`ui`] — render functions for the table, action bar, log and help.
//! * [`update`] — message reducer.

#![warn(missing_docs)]

pub mod app;
pub mod cli;
pub mod config;
pub mod error;
pub mod event;
pub mod log_panel;
pub mod modes;
pub mod tui;
pub mod ui;
pub mod update;

pub use app::App;
pub use error::{Error, Result};
