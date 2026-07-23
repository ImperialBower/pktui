---
type: Architecture
title: Msg and the update reducer
description: The message enum plus event_to_msg and update — the single mutation point that advances the App.
resource: https://github.com/ImperialBower/pktui/blob/main/src/update.rs
tags: [architecture, reducer, message]
timestamp: '2026-07-22T00:00:00Z'
---

# Role

This module is the **message + update** half of the
[Elm loop](/architecture/elm-loop.md).

* `event_to_msg` translates each runtime [`Event`](/architecture/event-loop.md)
  into one or more `Msg`s.
* `update(app, msg) -> bool` applies the `Msg` to the
  [`App`](/architecture/app-model.md) and returns whether the loop should keep
  running.

`update` is the **only** place `App` is mutated. The renderer reads `App`
immutably and the event loop only ever calls `update`.

# Msg

`Msg` is the normalised message enum. Every keystroke, tick, and internal
command becomes a `Msg` — this indirection is what lets tests drive the app
without a terminal, by constructing `Msg`s directly.

# Citations

[1] [update.rs](https://github.com/ImperialBower/pktui/blob/main/src/update.rs)
