# Spectate Mode `D` Dump — Design

**Date:** 2026-06-03

## Goal

Make the `D` (dump) command work in Spectate mode, not just Play mode.

## Background

The `D` key is already wired globally: `key_to_msg` (`src/update.rs`) returns
`Msg::Dump` in every mode. The gate is `dump_play_state` (`src/update.rs`),
which only handles `AppMode::Play(p)` — every other mode logs *"D (dump) is only
available in Play mode"*.

The reason it is Play-only: `Play::dump_state` serializes the live `pkcore`
engine (`session.table`, seats, per-card visibility). Spectate mode owns no
engine — it holds a proto `TableStatus` snapshot streamed over gRPC, plus a
best-effort `TableConfig`. So enabling `D` requires a second dump path that
serializes the snapshot Spectate actually has.

## Design

### 1. `SpectateState::dump_state(&self, log: &LogPanel) -> io::Result<PathBuf>`

New public method on `SpectateState` in `src/modes/spectate.rs`.

- **Filename:** `pktui-spectate-dump-<street>-<unix>.yaml`, where `<street>` is
  derived from `status.current_street` (or `nostatus` when no snapshot has
  arrived yet) and `<unix>` is the current Unix timestamp in seconds. Distinct
  from Play's `pktui-dump-<seed>-<phase>-<unix>.yaml`.
- **Two sections in one file:**
  - `pktui_spectate_dump:` — Play-style summary built from `status`:
    `endpoint`, `conn`, `paused`, `pot`, `board`, `small_blind`, `big_blind`,
    `current_street`, `hand_in_progress`, `next_to_act`, a `seats:` list
    (seat_number, name, chips, bet, cards, state, profit_loss), and the last
    ~40 log lines.
  - `raw_proto:` — `TableStatus` and `TableConfig` rendered verbatim via
    `{:#?}` Debug, indented as a YAML block scalar (`|`) so the file stays
    valid YAML.
- Handles `status: None` gracefully (writes a stub noting no snapshot yet).

### 2. Wire into `update.rs`

Rename `dump_play_state` → `dump_current_state`. Add an `AppMode::Spectate(s)`
arm that calls `s.dump_state(&app.log)` with the same Ok/Err logging as Play.
Arena/Replay keep the "not available here" message (reworded since it is no
longer Play-only).

### 3. Help text

Broaden the `D` line in `src/ui/help.rs` to note it works in Play and Spectate.

### 4. Tests

- Doc test on `dump_state` (per project CLAUDE.md).
- Unit test via existing `SpectateState::detached`: inject a sample status,
  call `dump_state`, assert the file exists and contains both
  `pktui_spectate_dump:` and `raw_proto:`. Clean up the file afterward.
- Unit test for the `status: None` path.

## Addendum (2026-06-03): completed-hand history

Spectate accumulates a history of completed hands so `D` can dump more than the
single live snapshot.

- **Field:** `SpectateState.completed_hands: Vec<TableStatus>` (oldest first),
  initialized empty. Unbounded — keeps every hand that ends this session.
- **Trigger:** in `apply`, after updating `self.status`, when a `TableEvent`
  has `event_type == EventType::HandEnded`, push the end-of-hand snapshot
  (the event's own `current_status`, falling back to `self.status`). Gated by
  `paused` like all event handling, so a frozen display skips accumulation.
- **Dump:** `render_dump_yaml` gains a `completed_hands_count:` line and a
  `completed_hands:` list (one summary entry per hand) rendered via a shared
  `write_status_summary` helper extracted from the live-snapshot rendering.
- No cap and no env knob (matches the "unbounded" decision).

## Non-goals / YAGNI

Do not refactor Play's `render_state_yaml` to share code with Spectate. The
field sets differ (engine model vs proto snapshot); sharing would couple two
unrelated data models for little gain.
