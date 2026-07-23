---
type: Render Module
title: view — render dispatch
description: The top-level render function; takes the immutable App, dispatches to a per-mode view, and overlays help.
resource: https://github.com/ImperialBower/pktui/blob/main/src/ui/mod.rs
tags: [ui, render, dispatch]
timestamp: '2026-07-22T00:00:00Z'
---

# Role

`ui::view(app, frame)` is the **view** in the [Elm loop](/architecture/elm-loop.md).
It reads [`App`](/architecture/app-model.md) immutably and draws to a
`ratatui::Frame` — it never mutates state.

# Dispatch

`view` selects a per-mode renderer based on `AppMode`:

| Mode | Renderer |
|------|----------|
| Play / Arena | [table view](/ui/table-view.md) |
| Replay | [replay view](/ui/replay-view.md) |
| Spectate | table-style snapshot view |

It then overlays the [help dialog](/ui/help-overlay.md) when `help_visible`.
The rolling [log panel](/ui/log-panel.md) is drawn below the main area, and the
[action bar](/ui/action-bar.md) at the bottom in Play mode.

# Helpers

`sort_hole_cards` normalises a hole-card string for stable display.

# Citations

[1] [ui/mod.rs](https://github.com/ImperialBower/pktui/blob/main/src/ui/mod.rs)
