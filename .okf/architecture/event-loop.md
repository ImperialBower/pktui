---
type: Architecture
title: Event polling and the tick timer
description: Converts crossterm key/resize events plus a regular tick into a single Event enum consumed by the reducer.
resource: https://github.com/ImperialBower/pktui/blob/main/src/event.rs
tags: [architecture, crossterm, tick]
timestamp: '2026-07-22T00:00:00Z'
---

# Role

`next_event(tick_interval, last_tick)` is the runtime's single source of
stimuli. It converts every external event into an `Event`, which
[`event_to_msg`](/architecture/update-reducer.md) then turns into a `Msg`.

# Event flavours

| Variant | Meaning |
|---------|---------|
| `Event::Key` | A user keystroke, polled from crossterm. |
| `Event::Tick` | Emitted at a regular cadence. |

The **tick** is what makes all-bot play watchable: bots act one per tick, and
time-based displays refresh on ticks. [Arena](/modes/arena.md) exposes `+`/`-`
to change the tick interval (bot speed).

# Terminal lifecycle

Raw mode and the alternate screen are set up and torn down in `src/tui.rs`
(`init` / `restore`), which also installs a panic hook so a crash restores the
terminal before printing — otherwise the user's shell is left unusable.

# Citations

[1] [event.rs](https://github.com/ImperialBower/pktui/blob/main/src/event.rs)
[2] [tui.rs](https://github.com/ImperialBower/pktui/blob/main/src/tui.rs)
