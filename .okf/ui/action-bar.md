---
type: Render Module
title: Action bar
description: Two-line context-sensitive hotkey bar showing the current bet/raise amount, the 1/2/3 presets, and available actions.
resource: https://github.com/ImperialBower/pktui/blob/main/src/ui/action_bar.rs
tags: [ui, render, hotkeys, betting]
timestamp: '2026-07-22T00:00:00Z'
---

# Role

The bottom bar in [Play mode](/modes/play.md) that tells the human what they
can do right now.

# Two-line layout

1. The current bet/raise amount plus the preset values `1`/`2`/`3` will set.
2. The available action hotkeys: `f`/`k`/`c`/`b`/`r`/`a`.

Splitting across two lines is intentional — a single line truncates on narrow
terminals and the bet field (the part that changes as you press `1`/`2`/`3`)
would scroll off-screen.

# Helpers

* `min_for(state, seat)` — minimum legal bet/raise for a seat.
* `preset_values(state, seat)` — the three preset amounts (covered by
  `tests/bet_presets.rs`).

# Citations

[1] [ui/action_bar.rs](https://github.com/ImperialBower/pktui/blob/main/src/ui/action_bar.rs)
