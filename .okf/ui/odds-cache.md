---
type: Render Module
title: OddsCache — per-street win% caching
description: Change-keyed cache of double-dummy per-street equities; recomputes only when contesting cards or the board change.
resource: https://github.com/ImperialBower/pktui/blob/main/src/ui/odds.rs
tags: [ui, equity, cache, performance]
timestamp: '2026-08-30T00:00:00Z'
---

# Role

`OddsCache` computes the per-street **double-dummy** ("all-knowing TV view")
win% shown for each contesting seat, powered by pkcore's `Game::street_equities()`
(the `equity` feature).

# Change-keyed caching

Equity computation is expensive, so `OddsCache` keys on the contesting cards
and the board and only recomputes when they change (a deal, a board advance, or
a fold). Between changes it returns cached values.

# Sampling precision

Only the **multiway preflop** path (3–10 seats) is sampled; heads-up preflop
uses pkcore's exact `SortedHeadsUp` lookup, and flop / turn / river enumerate
exactly. From pkcore `0.11.0` that sampled path defaults to 25,000 samples
(was 100,000), so its worst-case error is ~0.7 pp rather than ~0.3 pp — enough
for an honest whole percent, not for a decimal place.
`Game::street_equities()` accepts no `EquityOptions`, so raising the count
would mean bypassing it. pktui instead renders the Win% column as a whole
percent (`82%`), in both the [table view](/ui/table-view.md) and the
[replay view](/ui/replay-view.md), so the displayed digits are all real.

# Where it appears

Wired into the [table view](/ui/table-view.md) (Arena) and the
[replay view](/ui/replay-view.md), plus Spectate. Deliberately **not** used in
Play mode. Full rationale in
[the street odds display decision](/decisions/street-odds-display.md).

# Citations

[1] [ui/odds.rs](https://github.com/ImperialBower/pktui/blob/main/src/ui/odds.rs)
[2] [Per-street win% design doc](https://github.com/ImperialBower/pktui/blob/main/docs/superpowers/specs/2026-06-20-street-odds-display-design.md)
