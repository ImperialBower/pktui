---
type: Architecture
title: App — the central model
description: The root state object; owns the active AppMode, the shared LogPanel, and the should_quit / help_visible flags.
resource: https://github.com/ImperialBower/pktui/blob/main/src/app.rs
tags: [architecture, model, state]
timestamp: '2026-07-22T00:00:00Z'
---

# Role

`App` is the **model** in the [Elm loop](/architecture/elm-loop.md). It is
mutated exclusively through [`update`](/architecture/update-reducer.md); the
[UI layer](/ui/rendering.md) reads it immutably.

# What it owns

| Field | Purpose |
|-------|---------|
| `AppMode` | The active mode + its per-mode state (Play / Arena / Replay / Spectate). |
| `LogPanel` | The shared rolling [event log](/ui/log-panel.md) rendered below the table. |
| `should_quit` | Flag the event loop polls each iteration to decide whether to exit. |
| `help_visible` | Toggles the [help overlay](/ui/help-overlay.md) (`?`). |

# AppMode

`AppMode` is the enum that dispatches to one of the four
[modes](/modes/play.md). Each variant carries that mode's own state struct
(`PlayState`, `ArenaState`, `ReplayState`, spectate state), so mode-specific
data never leaks across modes.

# Citations

[1] [app.rs](https://github.com/ImperialBower/pktui/blob/main/src/app.rs)
