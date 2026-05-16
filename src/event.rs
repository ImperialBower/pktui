//! Event polling: crossterm key/resize events + a tick timer for bot pacing.
//!
//! The event loop converts every external stimulus into an [`Event`]. The
//! [`update`](crate::update::update) reducer then translates each event into
//! a [`Msg`](crate::update::Msg) and advances the [`App`](crate::App).
//!
//! Two flavours of event matter:
//!
//! * `Event::Key` — a user keystroke. Polled from crossterm.
//! * `Event::Tick` — emitted at a regular cadence so the UI can pace bot
//!   actions (one bot per tick) and refresh time-based displays.

use std::time::{Duration, Instant};

use crossterm::event::{self, Event as CtEvent, KeyEvent};

use crate::error::Result;

/// An event the app reacts to.
///
/// # Examples
///
/// ```
/// use pktui::event::Event;
/// let t = Event::Tick;
/// matches!(t, Event::Tick);
/// ```
#[derive(Debug, Clone)]
pub enum Event {
    /// The terminal was resized — ratatui will redraw on next frame regardless
    /// but we expose the event so the model can react if needed.
    Resize(u16, u16),
    /// A keystroke from the user.
    Key(KeyEvent),
    /// A tick timer fired. Used to pace bot actions.
    Tick,
}

/// Polls crossterm for the next event, falling back to a [`Event::Tick`] when
/// `tick_interval` elapses with no input.
///
/// `last_tick` is updated in-place so the caller can carry the deadline across
/// loop iterations without re-computing it.
///
/// # Errors
///
/// Returns [`crate::Error::Io`] if crossterm event polling fails.
///
/// # Examples
///
/// ```no_run
/// use std::time::{Duration, Instant};
/// use pktui::event::next_event;
///
/// let mut last = Instant::now();
/// let _evt = next_event(Duration::from_millis(50), &mut last).unwrap();
/// ```
pub fn next_event(tick_interval: Duration, last_tick: &mut Instant) -> Result<Event> {
    let timeout = tick_interval
        .checked_sub(last_tick.elapsed())
        .unwrap_or_else(|| Duration::from_millis(0));

    if event::poll(timeout)? {
        match event::read()? {
            CtEvent::Key(k) => Ok(Event::Key(k)),
            CtEvent::Resize(w, h) => Ok(Event::Resize(w, h)),
            // Mouse / focus / paste events are ignored for now.
            _ => {
                // Recurse-once — but bounded by the deadline so we don't spin.
                if last_tick.elapsed() >= tick_interval {
                    *last_tick = Instant::now();
                    Ok(Event::Tick)
                } else {
                    next_event(tick_interval, last_tick)
                }
            }
        }
    } else {
        *last_tick = Instant::now();
        Ok(Event::Tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_variants_are_constructible() {
        // Smoke-construct each variant and confirm the discriminant is what
        // we expect — guards against an accidental enum reshuffle.
        assert!(matches!(Event::Resize(80, 24), Event::Resize(80, 24)));
        assert!(matches!(Event::Tick, Event::Tick));
        // Key is harder to construct without crossterm internals; the resolver
        // is exercised via integration tests that drive the full loop.
    }
}
