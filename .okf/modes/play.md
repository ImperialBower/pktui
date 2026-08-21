---
type: UI Mode
title: Play mode
description: One human at seat 0 vs eight bots; owns the live PokerSession, bot roster, RNG, and the bet-amount field.
resource: https://github.com/ImperialBower/pktui/blob/main/src/modes/play.rs
tags: [mode, play, interactive]
timestamp: '2026-08-21T00:00:00Z'
---

# Role

Play is the default mode (`pktui play`, or no subcommand). A human sits at
seat 0; seats 1–8 (or 1–7 for stud) are bots. The count comes from
`Variant::max_seats`, so it follows the [CLI](/config/cli.md) rather than being
fixed in the mode.

# State (`PlayState`)

Owns the live `pkcore::PokerSession`, the bot roster, an RNG (seedable via
`--seed`), the current waiting state (`Awaiting`), and a numeric `BetField`
the user adjusts before confirming a bet or raise.

* `Awaiting::Human` — the loop pauses for the human's action.
* Bots act one per [tick](/architecture/event-loop.md).

# Aborted hands

`PokerSession::next_step` returns `SessionStep::Failed(PKError)` when a deal or
a chip collection fails part-way through a hand. There was no showdown, so
`end_hand` would refuse to resolve it. The tick loop logs the error, calls
`PokerSession::abort_hand` — which returns every committed chip to the stack it
came from — then advances the button and settles into `Awaiting::HandComplete`
or `Awaiting::SessionOver` exactly as a normal hand end would.
[Arena](/modes/arena.md) handles the same step identically.

# Interaction

The [action bar](/ui/action-bar.md) shows context-sensitive hotkeys
(`f`/`k`/`c`/`b`/`r`/`a`) and the bet presets (`1`/`2`/`3`). Configured by the
[CLI](/config/cli.md) (`--variant`, `--blinds`, `--chips`, stud forced-bet flags)
and defaults from [user config](/config/user-config.md).

# Note on odds

Play mode is **intentionally excluded** from the per-street Win% display so it
does not leak hidden opponent strength to the human — see
[street odds display](/decisions/street-odds-display.md).

# Related

Contrast with [Arena](/modes/arena.md) (same table, but seat 0 is also a bot).

# Citations

[1] [modes/play.rs](https://github.com/ImperialBower/pktui/blob/main/src/modes/play.rs)
