---
type: Architecture
title: Crate-wide error type
description: A single Error enum plus Result alias that funnels pkcore, IO, YAML, and TOML errors into one ?-friendly type.
resource: https://github.com/ImperialBower/pktui/blob/main/src/error.rs
tags: [architecture, errors]
timestamp: '2026-07-22T00:00:00Z'
---

# Role

Every fallible operation returns `Result<T>`, a thin alias for
`std::result::Result<T, Error>`. The `Error` enum bundles all upstream error
categories into one type so `?` propagation works uniformly.

# Sources folded in

`Error` implements `From` for each upstream category, enabling `?`:

| Source | Category |
|--------|----------|
| `pkcore::PKError` | Engine errors from the poker core. |
| `std::io::Error` | Terminal / file IO. |
| `serde_yaml` / YAML parse | [Replay](/modes/replay.md) file loading. |
| `toml::de::Error` | [User config](/config/user-config.md) parsing. |

`Error` implements `Display` and `std::error::Error`, consistent with the
project's convention of custom domain error types over `unwrap`/`panic`.

# Citations

[1] [error.rs](https://github.com/ImperialBower/pktui/blob/main/src/error.rs)
