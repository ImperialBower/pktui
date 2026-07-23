---
type: UI Mode
title: Spectate mode
description: Read-only viewer of a live pkdealer table over gRPC; owns no pkcore engine, applies streamed snapshots via a background thread.
resource: https://github.com/ImperialBower/pktui/blob/main/src/modes/spectate.rs
tags: [mode, spectate, grpc, pkdealer]
timestamp: '2026-07-22T00:00:00Z'
---

# Role

Spectate (`pktui spectate [--endpoint http://host:50051]`) is a read-only
viewer of a live [`pkdealer`](https://github.com/ImperialBower/pkdealer) table.
Unlike the other three [modes](/modes/play.md), it owns **no** `pkcore` engine.

# Architecture

A background OS thread holds the gRPC stream (via `tonic` +
`pkdealer_proto`) and forwards `SpectateMsg`s through a channel.
`SpectateState::drain` applies them to the latest snapshot, which the UI then
renders. `ConnState` tracks the connection lifecycle (connecting / connected /
error). A spectator token identifies the read-only client.

# Features

* Per-seat profit/loss tracking across hands.
* Per-street Win% column (Hold'em only) — see
  [street odds display](/decisions/street-odds-display.md).
* A `D` key dumps the current table snapshot to a YAML file — see
  [the spectate dump design](/decisions/spectate-dump.md). The repo's
  `pktui-spectate-dump-s1-*.yaml` files are example dumps.

# Citations

[1] [modes/spectate.rs](https://github.com/ImperialBower/pktui/blob/main/src/modes/spectate.rs)
