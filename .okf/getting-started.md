---
type: Rust Crate
title: pktui — overview
description: A ratatui terminal client for the pkcore poker engine, with Play, Arena, Replay, and Spectate modes.
resource: https://github.com/ImperialBower/pktui
tags: [overview, ratatui, poker, tui]
timestamp: '2026-08-22T00:00:00Z'
---

# What pktui is

`pktui` is a [ratatui](https://ratatui.rs) **terminal client** for the
[`pkcore`](https://github.com/ImperialBower/pkcore) poker engine. It is the
terminal sibling of
[pkarena0-web](https://github.com/ImperialBower/pkarena0-web): the same engine
and bot roster, rendered to a terminal table instead of an SVG one.

The binary is thin (`src/main.rs`); the testable surface lives in the library
(`src/lib.rs`). It is **not** publishable to crates.io because it depends on
`pkdealer_proto` via a cross-repo git dependency.

# Modes

Selected at startup by CLI subcommand (see [the CLI](/config/cli.md)):

* [Play](/modes/play.md) — one human at seat 0 vs eight bots.
* [Arena](/modes/arena.md) — all bots, watch-only.
* [Replay](/modes/replay.md) — step through a saved YAML hand collection.
* [Spectate](/modes/spectate.md) — read-only viewer of a live `pkdealer` table over gRPC.

# Architecture at a glance

pktui is built around an Elm-style loop — see [the Elm loop](/architecture/elm-loop.md):

* [`App`](/architecture/app-model.md) is the **model**.
* [`Msg`](/architecture/update-reducer.md) is the **message**; `update` is the reducer.
* [`Event`](/architecture/event-loop.md) polling turns keystrokes and ticks into messages.
* [`ui::view`](/ui/rendering.md) is the **render**, reading `App` immutably.
* All fallible paths funnel through one [error type](/architecture/error-handling.md).

# Supported variants

| Variant | `--variant` | Max seats | Family |
|---------|-------------|-----------|--------|
| NLHE    | `nlhe` (default) | 9 | Hold'em |
| PLO     | `plo`       | 9 | Hold'em |
| Stud Hi | `stud-hi`   | 8 | Stud |
| Razz    | `razz`      | 8 | Stud |

Stud-family variants are capped at 8 seats, which is pkcore's
`Table::MAX_STUD_SEATS`. Eight players need 56 cards across 7 streets and the
deck holds 52; pkcore covers the shortfall the way the rulebook does, by
turning a single face-up community card on 7th street that every remaining
player counts as their seventh. Nine runs dry two streets earlier, where no
community card can rescue it, so pkcore rejects a nine-handed stud table
outright. Per-street Win% display (see
[street odds](/decisions/street-odds-display.md)) is Hold'em-only.

# Citations

[1] [pktui README](https://github.com/ImperialBower/pktui/blob/main/README.md)
[2] [pktui lib.rs](https://github.com/ImperialBower/pktui/blob/main/src/lib.rs)
