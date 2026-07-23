---
type: Architecture
title: Elm-style Model / Message / Update loop
description: The core control-flow pattern that structures the whole crate — pure reducer over an immutable model, rendered each frame.
tags: [architecture, elm, event-loop]
timestamp: '2026-07-22T00:00:00Z'
---

# Pattern

pktui is organised as an **Elm architecture** (aka Model–View–Update):

1. **Model** — [`App`](/architecture/app-model.md) owns all state: the current
   mode, its per-mode state, the shared log panel, and quit/help flags.
2. **Message** — [`Msg`](/architecture/update-reducer.md) normalises every
   stimulus (keystroke, tick, internal command) into a single enum.
3. **Update** — `update(app, msg) -> bool` is the single mutation point. It
   advances the model and returns whether the app should keep running.
4. **View** — [`ui::view`](/ui/rendering.md) draws the current model to a
   `ratatui::Frame`, reading `App` **immutably**.

# Why it matters

The invariant "mutation happens only in `update`, rendering only reads" keeps
the UI layer trivially testable and side-effect free. Tests drive the app by
feeding `Msg`s and asserting on `App`, without a terminal.

# The runtime loop

`src/main.rs::run` implements: poll an [`Event`](/architecture/event-loop.md) →
translate to `Msg`(s) via `event_to_msg` → `update` → render. A tick timer
paces bot actions (one bot per tick) so all-bot play is watchable.

# Citations

[1] [lib.rs — Architecture section](https://github.com/ImperialBower/pktui/blob/main/src/lib.rs)
[2] [main.rs — run loop](https://github.com/ImperialBower/pktui/blob/main/src/main.rs)
