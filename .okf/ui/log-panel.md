---
type: Render Module
title: LogPanel — rolling event log
description: A capped ring of LogLine entries (deal, fold, bet, showdown, …) rendered and auto-scrolled below the table.
resource: https://github.com/ImperialBower/pktui/blob/main/src/log_panel.rs
tags: [ui, log, buffer]
timestamp: '2026-07-22T00:00:00Z'
---

# Role

`LogPanel` is the shared rolling event log owned by [`App`](/architecture/app-model.md).
Every action the engine reports is appended as one `LogLine` with a `Severity`.

# Capping

The buffer caps at `LogPanel::CAPACITY`, dropping the oldest entry when full,
so a long session cannot consume unbounded memory.

# Rendering

Drawn by `src/ui/log_view.rs`: most recent lines at the bottom, auto-scrolled.

# Citations

[1] [log_panel.rs](https://github.com/ImperialBower/pktui/blob/main/src/log_panel.rs)
[2] [ui/log_view.rs](https://github.com/ImperialBower/pktui/blob/main/src/ui/log_view.rs)
