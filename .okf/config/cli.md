---
type: Configuration
title: Command-line interface
description: clap-derive CLI with three subcommands (play/arena/replay) plus spectate, mapping to the UI modes; play is the default.
resource: https://github.com/ImperialBower/pktui/blob/main/src/cli.rs
tags: [config, cli, clap]
timestamp: '2026-07-22T00:00:00Z'
---

# Role

The `clap`-derive CLI. One binary, subcommands mapping to the
[modes](/modes/play.md). No subcommand defaults to `play`.

```text
pktui play    [--variant nlhe|plo|stud-hi|razz] [--seed N] [--blinds 50/100]
              [--chips 10000] [--ante N] [--bring-in N] [--small-bet N] [--big-bet N]
pktui arena   [...same...] [--speed-ms 800]
pktui replay  <FILE>
pktui spectate [--endpoint http://host:50051]
```

# Variant / forced-bet flags

`Variant` (`nlhe` default, plus `plo`, `stud-hi`, `razz`) selects the game.
For **stud-family** variants the `--ante` / `--bring-in` / `--small-bet` /
`--big-bet` flags apply; for **hold'em-family** the blind flags apply.
Irrelevant forced-bet flags are silently ignored for the chosen variant. Stud
variants are capped at 6 seats.

# Precedence

CLI flags override values from [user config](/config/user-config.md); anything
unspecified falls back to the config, then to built-in defaults.

# Citations

[1] [cli.rs](https://github.com/ImperialBower/pktui/blob/main/src/cli.rs)
[2] [README — Install / run](https://github.com/ImperialBower/pktui/blob/main/README.md)
