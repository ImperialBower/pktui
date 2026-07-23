---
type: UI Mode
title: Replay mode
description: Read-only walk-through of a saved YAML HandCollection with next/previous hand and street cursors; the engine is never touched.
resource: https://github.com/ImperialBower/pktui/blob/main/src/modes/replay.rs
tags: [mode, replay, yaml, read-only]
timestamp: '2026-07-22T00:00:00Z'
---

# Role

Replay (`pktui replay <FILE>`) loads a `HandCollection` YAML file — as produced
by pkcore's `interactive_play` example or a pktui session save — and lets the
user step through it.

# State (`ReplayState`)

Holds the loaded collection plus cursors for next/previous **hand** and
next/previous **street**. It is a pure viewer: no `apply_action` calls occur
and the `pkcore` engine is not touched — each hand renders straight from the
YAML record.

# Rendering

Drawn by the [replay view](/ui/replay-view.md): a hand header (id, button,
stakes), a player table, the visible street's actions, and a results panel at
the showdown step. Shows the per-street Win% column (Hold'em only) — see
[street odds display](/decisions/street-odds-display.md).

# Citations

[1] [modes/replay.rs](https://github.com/ImperialBower/pktui/blob/main/src/modes/replay.rs)
