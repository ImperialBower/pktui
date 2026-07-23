---
type: Render Module
title: Help overlay
description: A centered modal of keyboard shortcuts, toggled with '?', drawn on top of the main view.
resource: https://github.com/ImperialBower/pktui/blob/main/src/ui/help.rs
tags: [ui, render, help, modal]
timestamp: '2026-07-22T00:00:00Z'
---

# Role

`render_overlay` draws a centered modal listing keyboard shortcuts. It is
triggered by `?` (which flips `App::help_visible`) and drawn **after** the main
view so it sits on top.

# Layout

A `centered(width, height, area)` helper computes the modal rectangle so the
overlay stays centered regardless of terminal size.

# Citations

[1] [ui/help.rs](https://github.com/ImperialBower/pktui/blob/main/src/ui/help.rs)
