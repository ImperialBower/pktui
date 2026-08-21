# Changelog

All notable changes to **pktui** are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Note on early numbering:** the project's very first tag was `0.1.0`, which was
> immediately renumbered down to the `0.0.x` pre-1.0 line the following day. Entries
> below are ordered by release date (newest first), so `0.1.0` appears last even
> though it sorts highest.

## [0.0.7] - 2026-08-21

Tracks the `pkcore` engine from `0.2.1` up to `0.6.0`, and lifts the stud seat
cap that the older engine forced.

### Added

- **`PlayState::settle_between_hands`** — closes out a finished or aborted hand:
  moves the button, drops busted seats, then parks in `Awaiting::HandComplete`
  or `Awaiting::SessionOver`. Extracted from the two hand-ending paths in
  `PlayState::tick`, which had carried the same block twice.

### Changed

- **Requires `pkcore 0.6.0`** (was `0.2.1` in `0.0.6`; `0.5.0` was an
  intermediate step within this same unreleased line). Two of its breaking
  changes reach pktui:
  - `Table::stud_hi_from_seats` and `Table::razz_from_seats` are now fallible,
    so `build_table` in Play and Arena returns `Result<Table>` and the seat
    layout is validated by the engine instead of by pktui's own guess.
  - `SessionStep` gained a `Failed(PKError)` variant. Play and Arena now handle
    it: they log the error, call the new `PokerSession::abort_hand` to return
    every committed chip to the stack it came from, advance the button, and
    settle into the normal between-hands state. Previously a mid-hand deal
    failure was reported as `HandComplete`, which stranded the pot.

- **Stud-family tables seat 8, up from 6** (`Variant::max_seats`). The 6-seat
  cap was pktui's own workaround for pkcore's stud deck-exhaustion bug
  (pkcore `DEFECT_018`, fixed in 0.6.0). Eight is now the engine's real ceiling
  — `Table::MAX_STUD_SEATS` — because pkcore turns a single face-up community
  card on 7th street when the stub cannot serve everyone. Nine remains
  impossible and the engine rejects it. `pktui play --variant stud-hi` now
  seats the hero plus 7 bots; `arena` seats 8.

## [0.0.6] - 2026-07-13

Version-only bump; no code, dependency, or documentation changes.

## [0.0.5] - 2026-07-07

Completes **EPIC-44**: double-dummy per-street win% display.

### Added
- Per-street **Win%** column showing double-dummy ("all-knowing TV view") equity
  for each contesting seat at preflop, flop, turn, and river — wired into **Arena**,
  **Spectate**, and **Replay** modes (Hold'em only; Play mode intentionally excluded
  to avoid leaking hidden opponent strength).
- `OddsCache` (`src/ui/odds.rs`): change-keyed cache that recomputes equities only
  when the contesting cards or board change (deal / board advance / fold), powered by
  pkcore's new `Game::street_equities()` API.

### Changed
- Requires `pkcore 0.1.8` with the `equity` feature.
- Improved display of LLM winnings and losses.

## [0.0.4] - 2026-06-20

Introduces **Spectate** mode and the EPIC-44 token/cost columns.

### Added
- **Spectate mode**: background gRPC stream bridge (`tokio` / `tonic` /
  `pkdealer_proto`), `TableStatus`→`SeatRow` adapter, dedicated renderers, and the
  `spectate` subcommand wired through CLI, app, update, and UI.
- Spectator reveal of in-hand players' hole cards via a spectator token.
- EPIC-44 LLM **token usage** and **notional cost** columns sourced from `SeatInfo`.
- Per-seat last-action column, `BTN`/`SB`/`BB` position tags, and an `Analysis`
  column in the spectator table.
- `D` dump-state support in Spectate mode, plus unbounded completed-hand history.
- `pktui::ui::sort_hole_cards` helper (descending rank order, ASCII-suit preserving).

### Fixed
- Live blinds read from each status snapshot instead of a once-fetched config.
- Per-street bet tracking so Bet/Action columns clear between rounds.
- Active-seat highlight no longer applies to folded seats.
- `pkdealer_proto` pulled as a git dependency so CI resolves it.
- Dump-state test filename race under parallel execution.

## [0.0.3] - 2026-05-20

Adds the Stud/Razz/PLO game families.

### Added
- **Stud Hi**, **Razz**, and **PLO** CLI variants (`--variant stud-hi | razz | plo`)
  routed to the matching pkcore constructors.
- Stud-style hole-card rendering: bracketed down cards for hero, `??` for opponents,
  up cards face-up in dealt order, with alignment guards.
- Showdown reveal of best-5 hand plus rank class across all families (Hold'em river,
  Stud Hi 7th street, Razz A-5 lowball).
- Global `D` keybinding that dumps live Play state to a timestamped YAML file.
- `Variant::max_seats()` centralizing per-variant seat caps (NLHE 9, Stud Hi / Razz 6).

### Fixed
- Defensive `format_hole` treats `seat.cards` as the authoritative count so a short
  `seat.hand` can never silently drop a dealt card from the hero row.
- Hole-column constraint widened so 7-card stud hands and showdown strings are not
  truncated.

### Changed
- Raised the minimum CI Rust version and fixed the Miri run.
- Updated license.

## [0.0.2] - 2026-05-16

### Added
- Showdown snapshot capture that records revealed hole cards in the narrow window
  before `end_hand()` resets the table.

### Fixed
- `1` / `2` / `3` key handling.

### Changed
- Readability refactor: `Accent` enum replacing three booleans, `map_or` over
  `map().unwrap_or()`, an `instant_minus()` time helper, and a `chips()` cast helper.

## [0.1.0] - 2026-05-14

- Initial release: ratatui terminal client for the pkcore poker engine, with NLHE
  **Play** (one human vs. bots) and **Arena** (all-bot) modes. Renumbered to the
  `0.0.x` line the next day.

[0.0.5]: https://github.com/ImperialBower/pktui/releases/tag/v0.0.5
[0.0.4]: https://github.com/ImperialBower/pktui/releases/tag/v0.0.4
[0.0.3]: https://github.com/ImperialBower/pktui/releases/tag/v0.0.3
[0.0.2]: https://github.com/ImperialBower/pktui/releases/tag/v0.0.2
[0.1.0]: https://github.com/ImperialBower/pktui/releases/tag/v0.1.0
