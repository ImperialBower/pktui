---
type: Render Module
title: Replay view
description: Renders a saved YAML hand — header, player table, the visible street's actions, and a showdown results panel.
resource: https://github.com/ImperialBower/pktui/blob/main/src/ui/replay_view.rs
tags: [ui, render, replay]
timestamp: '2026-07-22T00:00:00Z'
---

# Role

The renderer for [Replay mode](/modes/replay.md). Reads `ReplayState` and draws
the currently-visible hand and street.

# Layout

* **Header** — hand summary (id, button, stakes).
* **Player table** — seats and stacks, with the Hold'em per-street Win% column
  from the [odds cache](/ui/odds-cache.md).
* **Street actions** — the actions of the street the cursor is on.
* **Results panel** — shown at the showdown step.

# Citations

[1] [ui/replay_view.rs](https://github.com/ImperialBower/pktui/blob/main/src/ui/replay_view.rs)
