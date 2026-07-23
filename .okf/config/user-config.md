---
type: Configuration
title: Persistent user config
description: TOML config at ~/.config/pktui/config.toml holding set-once defaults (blinds, stack, arena speed); missing file returns defaults.
resource: https://github.com/ImperialBower/pktui/blob/main/src/config.rs
tags: [config, toml, persistence]
timestamp: '2026-07-22T00:00:00Z'
---

# Role

`Config` persists knobs a user typically sets once rather than passing on every
invocation: default blinds, starting stack, and Arena bot speed.

# Behaviour

* Location: `~/.config/pktui/config.toml` (via the `dirs` crate).
* Reading a **missing** file returns `Config::default()` without error.
* The file is created on first save with sensible defaults.
* Anything on the [command line](/config/cli.md) **overrides** the config.

TOML parse failures surface through the crate
[error type](/architecture/error-handling.md).

# Citations

[1] [config.rs](https://github.com/ImperialBower/pktui/blob/main/src/config.rs)
