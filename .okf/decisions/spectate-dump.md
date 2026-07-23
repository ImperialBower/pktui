---
type: Design Decision
title: Spectate 'D' snapshot dump
description: A 'D' keypress in Spectate mode writes the current table snapshot to a YAML file for debugging and replay seeding.
tags: [decision, spectate, dump, yaml]
timestamp: '2026-07-22T00:00:00Z'
---

# Decision

In [Spectate mode](/modes/spectate.md), the `D` key dumps the current live
table snapshot to a YAML file on disk.

# Rationale

Spectate consumes a live gRPC stream from `pkdealer` and owns no engine state,
so reproducing a table state for debugging is otherwise hard. A one-key dump
captures the exact snapshot for later inspection or as seed data.

# Artifacts

The repo root contains example dumps produced by this feature:
`pktui-spectate-dump-s1-1780535100.yaml` and
`pktui-spectate-dump-s1-1780537030.yaml`.

# Citations

[1] [Design doc — Spectate Mode `D` Dump](https://github.com/ImperialBower/pktui/blob/main/docs/superpowers/specs/2026-06-03-spectate-dump-design.md)
