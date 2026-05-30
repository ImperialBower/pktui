# pktui

A [ratatui](https://ratatui.rs) terminal client for the
[pkcore](https://github.com/ImperialBower/pkcore) poker engine.

`pktui` is the terminal sibling of
[pkarena0-web](https://github.com/ImperialBower/pkarena0-web): same engine,
same three modes (Play / Arena / Replay), same bot roster, swap the SVG
table for a ratatui one.

## Install / run

```sh
# from the workspace root, with ../pkcore checked out alongside ../pktui
cargo run --release                                # default: Play / NLHE (you vs 8 bots)
cargo run --release -- play                        # same as above, explicit
cargo run --release -- play --variant plo          # Pot-Limit Omaha (you vs 8 bots)
cargo run --release -- play --variant stud-hi      # 7-card Stud Hi (you vs 5 bots)
cargo run --release -- play --variant razz         # 7-card Razz lowball (you vs 5 bots)
cargo run --release -- arena --speed-ms 400        # all-bot NLHE
cargo run --release -- arena --variant plo         # all-bot PLO (9 bots)
cargo run --release -- arena --variant stud-hi     # all-bot Stud Hi (6 bots)
cargo run --release -- arena --variant razz        # all-bot Razz (6 bots)
cargo run --release -- replay path/to/session.yaml # step through saved hand history
cargo run --release -- spectate                          # watch a live pkdealer table (http://localhost:50051)
cargo run --release -- spectate --endpoint http://host:50051
```

### Variants and seat caps

| Variant   | `--variant` flag | Max seats | Play table | Arena table |
|-----------|------------------|-----------|------------|-------------|
| NLHE      | `nlhe` (default) | 9         | 1 hero + 8 bots | 9 bots |
| PLO       | `plo`            | 9         | 1 hero + 8 bots | 9 bots |
| Stud Hi   | `stud-hi`        | 6         | 1 hero + 5 bots | 6 bots |
| Razz      | `razz`           | 6         | 1 hero + 5 bots | 6 bots |

Stud-family variants are capped at 6 seats to keep the 52-card deck
comfortable across 7 streets of dealing.

### Forced-bet flags by variant

Hold'em-family (NLHE, PLO) uses blinds; stud-family (Stud Hi, Razz) uses
ante + bring-in + small-bet / big-bet. Pass the flags applicable to the
variant you picked — others are ignored.

```sh
# NLHE / PLO — blinds + chips
cargo run --release -- play --small-blind 50 --big-blind 100 --chips 10000
cargo run --release -- play --variant plo --small-blind 50 --big-blind 100

# Stud Hi / Razz — ante / bring-in / small-bet / big-bet
cargo run --release -- play --variant stud-hi \
    --ante 10 --bring-in 25 --small-bet 50 --big-bet 100 --chips 10000
cargo run --release -- play --variant razz \
    --ante 10 --bring-in 25 --small-bet 50 --big-bet 100
```

If `--small-bet` / `--big-bet` are omitted for stud-family, they fall back
to `--small-blind` / `--big-blind` so the existing NLHE defaults still
produce a playable Stud / Razz table.

`pktui` uses Rust edition 2024 and pins `rust-version = 1.94.1`. The
[`Cargo.toml`](Cargo.toml) declares `pkcore` as a `crates.io` dependency but
overrides it with `[patch.crates-io] pkcore = { path = "../pkcore" }` so
local engine work is picked up without publishing. Comment out the patch
section to build against the published crate exclusively.

## Modes

| Mode     | Subcommand                      | Description                                          |
|----------|---------------------------------|------------------------------------------------------|
| Play     | `pktui play`                    | One human at seat 0; bots at the remaining seats. NLHE seats 8 bots, stud-family seats 5. |
| Arena    | `pktui arena`                   | Bots only, watch-only. NLHE seats 9, stud-family seats 6. Use `+` / `-` to adjust pace. |
| Replay   | `pktui replay <FILE>`           | Step through a saved `HandCollection` YAML file.     |
| Spectate | `pktui spectate [--endpoint …]` | Read-only live viewer of a running `pkdealer` table. |

All live modes accept `--variant {nlhe,plo,stud-hi,razz}`, `--seed N`, and
`--chips N`. Hold'em-family (NLHE, PLO) adds `--small-blind` / `--big-blind`;
stud-family adds `--ante` / `--bring-in` / `--small-bet` / `--big-bet`.
Arena additionally accepts `--speed-ms N` (default 800).

### Spectate mode

`spectate` is a read-only viewer of a live
[`pkdealer`](https://github.com/ImperialBower/pkdealer) table. It connects to
the dealer's gRPC `StreamEvents` endpoint and renders the table, a per-seat
profit/loss column, and a rolling event log — the terminal counterpart to the
web `pkspectator`. It needs the `pkdealer` repo checked out as a sibling
(`../pkdealer`) so the shared protobuf crate is available. Press `space` to
freeze/unfreeze the display, `q` to quit. The viewer auto-reconnects if the
dealer restarts.

## Keyboard

| Mode       | Key            | Action                                                          |
|------------|----------------|-----------------------------------------------------------------|
| Global     | `?`            | Toggle help overlay                                             |
| Global     | `q` / `Ctrl+C` | Quit                                                            |
| Global     | `D`            | Dump Play state to `./pktui-dump-<seed>-<phase>-<unix>.yaml`    |
| Play       | `f`            | Fold                                                            |
| Play       | `k`            | Check                                                           |
| Play       | `c`            | Call                                                            |
| Play       | `a`            | All-in                                                          |
| Play       | `b` / `r`      | Confirm bet/raise using the current bet amount                  |
| Play       | `Enter`        | Confirm bet/raise — or deal next hand between hands             |
| Play       | `1` / `2` / `3`| Set bet to min / ½-pot / pot                                    |
| Play       | digits         | Type bet amount digit-by-digit                                  |
| Play       | `+` / `-`      | Bump bet amount by 50                                           |
| Play       | `Backspace`    | Delete last digit of bet amount                                 |
| Arena      | `+` / `-`      | Faster / slower bots (100 ms steps)                             |
| Replay     | `n` / `→`      | Next street                                                     |
| Replay     | `p` / `←`      | Previous street                                                 |
| Replay     | `N` / `Enter`  | Next hand                                                       |
| Replay     | `P`            | Previous hand                                                   |
| Spectate   | `space`        | Freeze / unfreeze the display                                   |

## Config file

On first save, `pktui` writes
`$XDG_CONFIG_HOME/pktui/config.toml` (typically
`~/.config/pktui/config.toml` on Linux/macOS,
`%APPDATA%\pktui\config.toml` on Windows):

```toml
small_blind = 50
big_blind = 100
chips = 10000
arena_speed_ms = 800
play_speed_ms = 600
```

Anything on the command line overrides the config.

## Architecture

`pktui` follows an Elm-style Model / Message / Update loop:

```text
crossterm event ──► Event ──► event_to_msg ──► Msg ──► update(app, msg) ──► App
                                                                            │
                                                                            ▼
                                                                       ui::view ──► ratatui Frame
```

* [`src/app.rs`](src/app.rs) — the `App` model, plus the `AppMode` enum that
  dispatches to per-mode state.
* [`src/modes/`](src/modes/) — `PlayState`, `ArenaState`, `ReplayState`.
* [`src/update.rs`](src/update.rs) — `Msg` enum and the `update` reducer.
* [`src/event.rs`](src/event.rs) — crossterm polling with tick timer for bot
  pacing.
* [`src/ui/`](src/ui/) — render functions (`table`, `action_bar`, `log_view`,
  `help`, `replay_view`).
* [`src/main.rs`](src/main.rs) — thin entry point: parse CLI, init terminal,
  run loop, restore terminal on exit.

The engine boundary is small: pktui reads `session.table` for rendering,
calls `session.next_step()` to advance, and calls `session.apply_action(seat,
PlayerAction)` for both bot and human decisions.

## Development

```sh
cargo build                  # build the binary + library
cargo test                   # unit + integration tests
cargo test --doc             # doc tests (CLAUDE.md requires one per public fn)
make ayce                    # full pipeline: fmt + test + clippy + deny + docs
```

CLAUDE.md in the repo root captures the coding conventions (every public
function has a unit test and a doc test, `unwrap`/`expect`/`panic` are
forbidden outside tests, etc.). New code should follow them.

## Licence

Dual-licensed under MIT OR Apache-2.0 (your choice), matching the upstream
`pkcore` engine.
