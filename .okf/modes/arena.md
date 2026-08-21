---
type: UI Mode
title: Arena mode
description: All-bot, watch-only table; identical to Play except seat 0 is a bot and the user only controls speed and quit.
resource: https://github.com/ImperialBower/pktui/blob/main/src/modes/arena.rs
tags: [mode, arena, watch-only]
timestamp: '2026-08-21T00:00:00Z'
---

# Role

Arena (`pktui arena`) runs a full table of bots with no human. It is nearly
identical to [Play](/modes/play.md) except there is no `Awaiting::Human`
state — every seat is a bot.

# State (`ArenaState` / `ArenaPhase`)

Owns the same live session and bot roster as Play. `ArenaPhase` tracks where
the auto-advancing table is in the hand cycle. The user controls only:

* `+` / `-` — adjust bot speed (the [tick](/architecture/event-loop.md)
  interval, also settable via `--speed-ms`).
* quit.

# Aborted hands

Arena's step loop handles `SessionStep::Failed(PKError)` the same way Play does
— log, `abort_hand`, then carry on with the next hand. See
[Play mode](/modes/play.md).

# Odds

Arena **shows** the per-street Win% column (Hold'em only) — unlike Play, there
is no human to protect from hidden information. See
[street odds display](/decisions/street-odds-display.md).

# Citations

[1] [modes/arena.rs](https://github.com/ImperialBower/pktui/blob/main/src/modes/arena.rs)
