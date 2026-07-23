---
type: Render Module
title: Table view (Play / Arena)
description: Renders the live 9-seat table with aligned columns, active-seat highlight, board, and pot.
resource: https://github.com/ImperialBower/pktui/blob/main/src/ui/table.rs
tags: [ui, render, table]
timestamp: '2026-07-22T00:00:00Z'
---

# Role

Draws the live seat table for [Play](/modes/play.md) and [Arena](/modes/arena.md)
via `render_table_view_play` / `render_table_view_arena`. It is the largest UI
module.

# Layout

```text
┌─ Table ─────────────────────────────────────┐
│  Seat 0  You          $10,000  [Kh As]       │
│  Seat 1  gto          $10,000  [??]   BTN    │
│  ...                                          │
│  Board: 2c 7d Th          Pot: 350            │
└───────────────────────────────────────────────┘
```

The seat list uses a `ratatui::widgets::Table` so columns stay aligned and the
active seat can be highlighted. In Hold'em modes it includes the per-street
Win% column fed by the [odds cache](/ui/odds-cache.md) — see
[street odds display](/decisions/street-odds-display.md).

# Citations

[1] [ui/table.rs](https://github.com/ImperialBower/pktui/blob/main/src/ui/table.rs)
