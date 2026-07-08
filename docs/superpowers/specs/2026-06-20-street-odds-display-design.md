# Per-Street Win% Display in pktui

**Date:** 2026-06-20
**Status:** Approved design, pre-implementation
**Repos touched:** `pkcore` (new API) + `pktui` (consumer)

## Goal

Display double-dummy ("all-knowing TV view") winning percentages for each
seat at every street — preflop, flop, turn, river — in pktui's table view,
mirroring the odds graphics on televised poker. The percentages already exist
inside pkcore's `examples/calc.rs` via `DealEval` / `FlopEval` / `TurnEval` /
`RiverEval`; this feature normalizes them behind one API and surfaces them in
the UI.

## Scope

**In scope**
- New unified pkcore API: `Game::street_equities()`.
- A new `Win%` column in pktui's seat table.
- Wiring for **Arena**, **Spectate**, and **Replay** modes.
- Hold'em (NLHE) only.

**Out of scope**
- **Play** mode (one human vs. bots): showing per-seat odds would leak
  opponents' hidden strength, so it is intentionally excluded.
- PLO / Stud Hi / Razz: the underlying equity APIs take 2-card hands, so
  non-Hold'em tables show `—`.
- Any gRPC / proto schema change. Spectate computes locally from the cards
  already present in `TableStatus`.

## Key design principles

1. **Double-dummy among contesting seats.** Equity is computed over the seats
   *still in the hand*. Folded seats are excluded from the calculation (and are
   already hidden in Spectate's stream), so a fold naturally raises every
   survivor's percentage.

2. **The cache key is the refresh trigger.** Odds are cached keyed on
   `(active hole cards + board)`. That signature changes *exactly* when a hand
   is dealt, the board advances, or a seat folds — the three (and only) moments
   the displayed cards change. No explicit event wiring is needed; a key
   comparison subsumes all three triggers and avoids recomputing per frame.

3. **Normalization lives in pkcore.** The four street evals return different
   shapes; pkcore owns the logic that flattens them into one type, so pkdealer
   and any future consumer can reuse it.

## pkcore API

Behind the existing `equity` feature flag (the preflop path uses `DealEval`,
which is already `#[cfg(feature = "equity")]`).

```rust
/// Per-seat odds at the current street, expressed as fractions in 0.0..=1.0.
///
/// `equity` is the split-pot value: `win + tie / 2.0` — a two-way chop counts
/// as half a win. `win` and `tie` are exposed separately so callers can render
/// either the chip-EV figure or a win/tie breakdown.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StreetEquity {
    pub win: f64,
    pub tie: f64,
    pub equity: f64,
}

impl Game {
    /// Returns per-seat odds for the current street, one entry per hand in
    /// `self.hands` order.
    ///
    /// Dispatches on board completeness:
    /// - 0 board cards  → `DealEval`  (preflop; heads-up = table lookup,
    ///   3–10 seats = seeded Monte Carlo)
    /// - 3 board cards  → `FlopEval`
    /// - 4 board cards  → `TurnEval`
    /// - 5 board cards  → `RiverEval`
    ///
    /// # Errors
    /// Propagates the underlying eval errors (e.g. `NotEnoughHands`,
    /// duplicate cards, an incomplete/invalid board card count).
    pub fn street_equities(&self) -> Result<Vec<StreetEquity>, PKError>;
}
```

### Normalization rules (applied per street, producing fractions)

| Street | Source           | win / tie source                                   | equity        |
|--------|------------------|----------------------------------------------------|---------------|
| Preflop| `DealEval.report.players[i]` | `pe.win`, `pe.tie` (already fractions) | `pe.equity` (already `win + tie/2`) |
| Flop   | `FlopEval.results`  | `wins_and_ties_percentages(i)` / 100.0          | `win + tie/2` |
| Turn   | `TurnEval.results`  | `wins_and_ties_percentages(i)` / 100.0          | `win + tie/2` |
| River  | `RiverEval.results` | `wins_and_ties_percentages(i)` / 100.0          | `win + tie/2` |

`WinResults` returns percentages (0–100 `f32`); divide by 100.0 and widen to
`f64` so the API speaks a single unit (fractions). The `equity = win + tie/2`
rule is applied uniformly; for preflop this already matches `DealEval`'s own
`equity` field, so all four streets agree.

### Tests (pkcore)

- Doc test on `street_equities` (the canonical "The Hand" `TestData`).
- Unit: preflop heads-up AA vs KK ≈ 0.82 / 0.18 for the aces.
- Unit: equities sum to ~1.0 at every street (within MC tolerance preflop,
  exact post-flop).
- Unit: river is deterministic — winner `equity == 1.0`, loser `0.0`; a board
  that chops yields `0.5 / 0.5` with `tie == 1.0`.
- Unit: board-card-count dispatch picks the right eval (1/2 board cards error).
- Version bump to `0.1.8` + CHANGELOG entry.

### ⛔ Handoff gate after pkcore

When the pkcore work above is complete (API + tests green + version bump +
CHANGELOG), **stop and notify the user.** The user publishes `pkcore 0.1.8` to
crates.io themselves — Claude never runs `cargo publish`. pktui work does not
begin until the published crate is available.

## pktui changes

### Dependency

- `Cargo.toml`: bump `pkcore` from `0.1.1` to `0.1.8` (which the user will have
  published to crates.io after the pkcore handoff gate); add `"equity"` to the
  feature list (`["bot-profiles", "hand-histories", "equity"]`). The
  `[patch.crates-io] pkcore = { path = "../pkcore" }` block may be un-commented
  for local iteration but is unnecessary once 0.1.8 is published; re-comment
  before committing, per the existing convention in the file.

### New module: `src/ui/odds.rs`

```rust
/// Cache that recomputes odds only when the contesting cards or board change.
pub struct OddsCache {
    key: String,                       // signature of active holes + board
    value: Vec<(u8, f64)>,             // (seat_index, equity) for active seats
}

impl OddsCache {
    /// Recomputes via `Game::street_equities` only when `key` changes;
    /// otherwise returns the cached value. `holes` carries (seat_index, Two)
    /// for each still-contesting Hold'em seat.
    pub fn equities(&mut self, holes: &[(u8, Two)], board: &Board) -> &[(u8, f64)];
}
```

- The signature string is built from the seat indices + sorted hole-card
  strings + board string — cheap to compare, stable across frames.
- Returns split-pot `equity` per active seat, mapped back to seat indices so
  the renderer can look up by seat.
- Unit-tested: same inputs → no recompute (assert via a call counter or by
  observing the key); a changed board / dropped seat → recompute.

### Rendering (`src/ui/table.rs`)

- Add `odds: Option<f64>` to `SeatRow`.
- Add a `Win%` column header after `Analysis`; width ~7 (e.g. `100.0%`).
  - `Some(eq)` → `format!("{:.1}%", eq * 100.0)`, styled (green-ish).
  - `None` (folded seat, non-Hold'em, or odds unavailable) → `—` dim.
- The existing `analysis` and token/cost columns stay; this is purely additive.

### Per-mode wiring

- **Arena** (`render_table_view_arena`): collect `(seat, Two)` for active seats
  from `state.session.table`, read `state.session.table.board`, call the
  `OddsCache` (held on `ArenaState`), populate `SeatRow.odds`.
- **Spectate** (`status_to_rows`): parse active seats' card strings + the
  board string from `TableStatus` into `Two`/`Board` (reusing the parsing
  proven by `holdem_board_analysis`). Cache on `SpectateState`. Only NLHE
  tables (detect by 2-card hands); otherwise `None`.
- **Replay** (`render` path): extract per-street hole cards + board from the
  current `HandCollection` record at `(hand_index, street_index)`, build the
  active set, cache on `ReplayState`. This mode needs the most new
  plumbing (the YAML record → cards/board extraction); if it proves fiddly it
  is the last of the three to land but is in scope.

### Tests (pktui)

- `OddsCache` recompute-on-change / reuse-on-stable behavior.
- A `TestBackend` render asserting the `Win%` header and a formatted cell
  (mirrors the existing `render_seats_shows_pnl_column_header` test).
- Spectate: `status_to_rows` produces `Some(_)` odds for active NLHE seats and
  `None` for folded seats / empty board.

## Performance note

The only heavy call is preflop multiway equity: `DealEval` runs a
100k-sample seeded Monte Carlo for 3–10 seats (heads-up is an instant table
lookup). With the cache this is a single computation at the *start* of each
hand, not per frame. Ship it straight first; if Arena's per-hand startup
hitches, optimize later by (a) calling `EquityRequest` directly with a lower
`max_samples` for the UI, or (b) computing off-thread. Flop enumeration is
parallel (`mpsc`) and turn/river are trivial (44 cases / 1 case).

## Build sequence

1. pkcore: `StreetEquity` + `Game::street_equities()` + tests + 0.1.8 bump.
   **→ stop here; user publishes pkcore 0.1.8 to crates.io.**
2. pktui: dependency bump (to published 0.1.8) + `src/ui/odds.rs` (`OddsCache`) + tests.
3. pktui: `Win%` column in `table.rs` + Arena wiring + tests.
4. pktui: Spectate wiring + tests.
5. pktui: Replay wiring + tests.
