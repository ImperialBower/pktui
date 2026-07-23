---
type: Design Decision
title: Per-street Win% display
description: Show double-dummy per-street equity in Arena / Spectate / Replay (Hold'em only), excluding Play to avoid leaking hidden strength.
tags: [decision, equity, epic-44]
timestamp: '2026-07-22T00:00:00Z'
---

# Decision

Add a per-street **Win%** column showing double-dummy ("all-knowing TV view")
equity for each contesting seat at preflop, flop, turn, and river.

# Scope

* **Enabled** in [Arena](/modes/arena.md), [Spectate](/modes/spectate.md), and
  [Replay](/modes/replay.md).
* **Excluded** from [Play](/modes/play.md) — a human opponent's hidden strength
  must not be leaked to the seated player.
* **Hold'em only** (NLHE / PLO), not stud variants.

# Implementation

Backed by [`OddsCache`](/ui/odds-cache.md) (`src/ui/odds.rs`), a change-keyed
cache over pkcore's `Game::street_equities()` (the `equity` feature), recomputing
only on a deal, board advance, or fold. Delivered as **EPIC-44** in pktui
`0.0.5`.

# Citations

[1] [Design doc — Per-street win% in pktui](https://github.com/ImperialBower/pktui/blob/main/docs/superpowers/specs/2026-06-20-street-odds-display-design.md)
[2] [Implementation plan](https://github.com/ImperialBower/pktui/blob/main/docs/superpowers/plans/2026-06-20-street-odds-display.md)
[3] [CHANGELOG 0.0.5](https://github.com/ImperialBower/pktui/blob/main/CHANGELOG.md)
