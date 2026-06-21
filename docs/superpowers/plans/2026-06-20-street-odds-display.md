# Per-Street Win% Display Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Display double-dummy per-seat winning percentages at every street (preflop → river) in pktui's Arena, Spectate, and Replay views, powered by a new unified `Game::street_equities()` API in pkcore.

**Architecture:** pkcore gains one normalizer (`Game::street_equities`) that dispatches across the four existing street evals and returns a uniform `Vec<StreetEquity>` (fractions, split-pot). pktui caches the result keyed on the contesting cards + board (so it recomputes only on deal/board-change/fold) and renders a `Win%` column. Rendering is immutable, so the cache uses interior mutability (`RefCell`).

**Tech Stack:** Rust 2024, pkcore (poker engine), ratatui (TUI), wincounter (win tallies).

## Global Constraints

- **No `cargo publish` by Claude.** After the pkcore work, STOP for the user to publish `pkcore 0.1.8`.
- **Rust test fn names must NOT be prefixed with `test_`** — use `subject__scenario` naming (matches both repos).
- **No `unwrap()`/`expect()`/`panic!()` in library code** (tests may use them).
- **pkcore version floor in pktui: `0.1.8`**, with feature list `["bot-profiles", "hand-histories", "equity"]`.
- **Equity rule (all streets):** `equity = win + tie / 2.0`; values are fractions `0.0..=1.0`.
- **Hold'em only:** non-2-card hands → no odds (`—`).
- **Never run state-changing git commands** — the plan shows commit commands for the user to run; the implementer surfaces them, the user executes.

---

## Phase A — pkcore (repo: `/Users/christoph/src/github.com/ImperialBower/pkcore`)

### Task 1: `StreetEquity` + `Game::street_equities()`

**Files:**
- Modify: `src/play/game.rs` (add `StreetEquity` struct + `street_equities` method + helper)
- Modify: `src/prelude.rs` (export `StreetEquity`)
- Test: inline `#[cfg(test)]` module in `src/play/game.rs`

**Interfaces:**
- Consumes: `DealEval::new(HoleCards) -> Result<DealEval, PKError>` (`.report.players: Vec<PlayerEquity>` with `win/tie/equity: f64`); `FlopEval::try_from(Game)`, `RiverEval::try_from(Game)`, `TurnEval::try_from(&Game)` — each has `.results: wincounter::results::WinResults`; `WinResults::wins_and_ties_percentages(usize) -> (f32, f32)` (percent 0–100); `Board { flop: Three, turn: Card, river: Card }` with `Three::is_dealt()` / `Card::is_dealt()`.
- Produces: `pkcore::play::game::StreetEquity { win: f64, tie: f64, equity: f64 }` and `Game::street_equities(&self) -> Result<Vec<StreetEquity>, PKError>` (one entry per `self.hands`, behind `feature = "equity"`).

- [ ] **Step 1: Write the failing tests**

Add to the existing `#[cfg(test)]` module at the bottom of `src/play/game.rs` (create one if absent, mirroring the file's style):

```rust
#[cfg(all(test, feature = "equity"))]
mod street_equities_tests {
    use super::*;
    use crate::play::board::Board;
    use crate::play::hole_cards::HoleCards;
    use std::str::FromStr;

    fn game(hands: &str, board: &str) -> Game {
        let board = if board.is_empty() {
            Board::default()
        } else {
            Board::from_str(board).unwrap()
        };
        Game::new(HoleCards::from_str(hands).unwrap(), board)
    }

    #[test]
    fn street_equities__preflop_aces_are_favorite() {
        // AA vs KK heads-up preflop ≈ 0.82 / 0.18.
        let eq = game("As Ah Ks Kh", "").street_equities().unwrap();
        assert_eq!(eq.len(), 2);
        assert!(eq[0].equity > 0.80 && eq[0].equity < 0.84, "AA equity {}", eq[0].equity);
        assert!(eq[1].equity < 0.20, "KK equity {}", eq[1].equity);
    }

    #[test]
    fn street_equities__sum_to_about_one_each_street() {
        for board in ["", "9c 6d 5h", "9c 6d 5h 5s", "9c 6d 5h 5s 8s"] {
            let eq = game("6s 6h 5d 5c", board).street_equities().unwrap();
            let sum: f64 = eq.iter().map(|e| e.equity).sum();
            assert!((sum - 1.0).abs() < 0.02, "board '{board}' summed to {sum}");
        }
    }

    #[test]
    fn street_equities__river_is_deterministic() {
        // Complete board: exactly one outcome, winner equity == 1.0.
        let eq = game("As Ks Qd Jd", "Ah Kh Qh 2c 3d").street_equities().unwrap();
        let total_full: usize = eq.iter().filter(|e| e.equity >= 0.999).count();
        assert_eq!(total_full, 1, "exactly one river winner: {eq:?}");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --features equity street_equities -- --nocapture`
Expected: FAIL — `no method named street_equities` / `cannot find type StreetEquity`.

- [ ] **Step 3: Add `StreetEquity` and the method**

In `src/play/game.rs`, add near the top of the file (after imports) the struct, and add `use wincounter::results::WinResults;` to the imports if not already present:

```rust
/// Per-seat odds at the current street, as fractions in `0.0..=1.0`.
///
/// `equity` is the split-pot value `win + tie / 2.0` (a two-way chop counts
/// as half a win). `win` and `tie` are kept separate so callers can show a
/// breakdown.
///
/// # Examples
///
/// ```
/// use pkcore::play::game::StreetEquity;
/// let e = StreetEquity { win: 0.80, tie: 0.04, equity: 0.82 };
/// assert!((e.equity - (e.win + e.tie / 2.0)).abs() < 1e-9);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StreetEquity {
    pub win: f64,
    pub tie: f64,
    pub equity: f64,
}

/// Normalizes a `WinResults` (percent 0–100) into per-seat split-pot
/// `StreetEquity` fractions for `n` seats.
#[cfg(feature = "equity")]
fn street_equities_from_results(results: &WinResults, n: usize) -> Vec<StreetEquity> {
    (0..n)
        .map(|i| {
            let (w, t) = results.wins_and_ties_percentages(i);
            let win = f64::from(w) / 100.0;
            let tie = f64::from(t) / 100.0;
            StreetEquity { win, tie, equity: win + tie / 2.0 }
        })
        .collect()
}
```

Then add the method inside `impl Game { ... }`:

```rust
    /// Returns per-seat double-dummy odds for the current street, one entry
    /// per hand in `self.hands` order.
    ///
    /// Dispatches on how much of the board is dealt:
    /// - no flop  → preflop ([`DealEval`]: heads-up table lookup, 3–10 seats
    ///   seeded Monte Carlo)
    /// - flop only → [`FlopEval`]
    /// - flop+turn → [`TurnEval`]
    /// - complete  → [`RiverEval`]
    ///
    /// Every street is normalized to split-pot equity (`win + tie / 2`).
    ///
    /// # Errors
    ///
    /// Propagates the underlying eval errors — e.g. [`PKError::NotEnoughHands`]
    /// preflop with fewer than two seats, or duplicate-card errors.
    ///
    /// # Examples
    ///
    /// ```
    /// use pkcore::prelude::TestData;
    ///
    /// let game = TestData::the_hand();
    /// let eq = game.street_equities().unwrap();
    /// assert_eq!(eq.len(), game.hands.len());
    /// ```
    #[cfg(feature = "equity")]
    pub fn street_equities(&self) -> Result<Vec<StreetEquity>, PKError> {
        use crate::play::stages::deal_eval::DealEval;
        use crate::play::stages::flop_eval::FlopEval;
        use crate::play::stages::river_eval::RiverEval;
        use crate::play::stages::turn_eval::TurnEval;

        let n = self.hands.len();
        if !self.board.flop.is_dealt() {
            let eval = DealEval::new(self.hands.clone())?;
            Ok(eval
                .report
                .players
                .iter()
                .map(|p| StreetEquity { win: p.win, tie: p.tie, equity: p.equity })
                .collect())
        } else if !self.board.turn.is_dealt() {
            let eval = FlopEval::try_from(self.clone())?;
            Ok(street_equities_from_results(&eval.results, n))
        } else if !self.board.river.is_dealt() {
            let eval = TurnEval::try_from(self)?;
            Ok(street_equities_from_results(&eval.results, n))
        } else {
            let eval = RiverEval::try_from(self.clone())?;
            Ok(street_equities_from_results(&eval.results, n))
        }
    }
```

In `src/prelude.rs`, add beside the existing stage exports (around line 92–96):

```rust
pub use crate::play::game::StreetEquity;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --features equity street_equities -- --nocapture`
Expected: PASS (3 tests).

Run the doc test too: `cargo test --features equity --doc street_equities`
Expected: PASS.

- [ ] **Step 5: Verify clippy + full build are clean**

Run: `cargo clippy --features equity --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

Provide this command to the user (do not run it yourself):

```bash
git add src/play/game.rs src/prelude.rs && git commit -m "feat(equity): unified Game::street_equities normalizer + StreetEquity"
```

---

### Task 2: Version bump + CHANGELOG, then HANDOFF

**Files:**
- Modify: `Cargo.toml` (version `0.1.7` → `0.1.8`)
- Modify: `CHANGELOG.md` (new entry)

- [ ] **Step 1: Bump the version**

In `src/.../Cargo.toml` (pkcore root `Cargo.toml`), change:

```toml
version = "0.1.7"
```
to
```toml
version = "0.1.8"
```

- [ ] **Step 2: Add the CHANGELOG entry**

Add a new section at the top of `CHANGELOG.md`'s unreleased/most-recent area:

```markdown
## 0.1.8

### Added
- `Game::street_equities()` and `StreetEquity` (behind the `equity` feature):
  a unified per-seat odds normalizer that dispatches across `DealEval`,
  `FlopEval`, `TurnEval`, and `RiverEval`, returning split-pot equity
  (`win + tie/2`) as fractions for every street.
```

- [ ] **Step 3: Verify it builds**

Run: `cargo build --features equity`
Expected: compiles; resolves as `pkcore v0.1.8`.

- [ ] **Step 4: Commit**

Provide this command to the user (do not run it yourself):

```bash
git add Cargo.toml CHANGELOG.md && git commit -m "chore: bump pkcore to 0.1.8 for street_equities"
```

- [ ] **Step 5: ⛔ STOP — hand off for publish**

Notify the user: pkcore work is complete and committed. **The user publishes `pkcore 0.1.8` to crates.io.** Do not start Phase B until the user confirms the crate is published.

---

## Phase B — pktui (repo: `/Users/christoph/src/github.com/ImperialBower/pktui`)

> Begin only after the user confirms `pkcore 0.1.8` is published.

### Task 3: Dependency bump

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Update the pkcore dependency**

In `Cargo.toml`, change:

```toml
pkcore = { version = "0.1.1", features = ["bot-profiles", "hand-histories"] }
```
to
```toml
pkcore = { version = "0.1.8", features = ["bot-profiles", "hand-histories", "equity"] }
```

(The `[patch.crates-io] pkcore = { path = "../pkcore" }` block may be temporarily un-commented for local iteration but must be re-commented before committing, per the note already in the file.)

- [ ] **Step 2: Verify it builds against the published crate**

Run: `cargo build`
Expected: resolves `pkcore v0.1.8`, compiles.

- [ ] **Step 3: Commit**

Provide this command to the user:

```bash
git add Cargo.toml Cargo.lock && git commit -m "build: require pkcore 0.1.8 with equity feature"
```

---

### Task 4: `OddsCache` module

**Files:**
- Create: `src/ui/odds.rs`
- Modify: `src/ui/mod.rs` (add `pub mod odds;`)
- Test: inline `#[cfg(test)]` in `src/ui/odds.rs`

**Interfaces:**
- Consumes: `pkcore::play::game::{Game, StreetEquity}`, `pkcore::play::board::Board`, `pkcore::play::hole_cards::HoleCards` (all `FromStr`; `Board: Default`).
- Produces: `OddsCache::new() -> OddsCache`; `OddsCache::equities(&self, holes: &[(u8, String)], board: &str) -> Vec<(u8, f64)>` — split-pot equity per active seat, recomputed only when `(holes, board)` change.

- [ ] **Step 1: Write the failing tests**

Create `src/ui/odds.rs` containing only the test module first:

```rust
//! Per-street double-dummy win% with change-keyed caching.

#[cfg(test)]
mod tests {
    use super::*;

    fn holes() -> Vec<(u8, String)> {
        vec![(0, "As Ah".to_string()), (1, "Ks Kh".to_string())]
    }

    #[test]
    fn equities__preflop_aces_favorite() {
        let cache = OddsCache::new();
        let eq = cache.equities(&holes(), "");
        assert_eq!(eq.len(), 2);
        let aa = eq.iter().find(|(s, _)| *s == 0).unwrap().1;
        assert!(aa > 0.80 && aa < 0.84, "AA equity {aa}");
    }

    #[test]
    fn equities__recomputes_only_on_key_change() {
        let cache = OddsCache::new();
        cache.equities(&holes(), "");
        let k1 = cache.inner.borrow().key.clone();
        cache.equities(&holes(), "");
        assert_eq!(cache.inner.borrow().key, k1, "same inputs must not change key");
        cache.equities(&holes(), "Ah Kd Qc");
        assert_ne!(cache.inner.borrow().key, k1, "new board must change key");
    }

    #[test]
    fn equities__rejects_single_seat_and_non_holdem() {
        let cache = OddsCache::new();
        assert!(cache.equities(&[(0, "As Ah".to_string())], "").is_empty());
        // 4-card hands (PLO) do not group into one Two per seat → empty.
        let plo = vec![(0, "As Ah Ad Ac".to_string()), (1, "Ks Kh Kd Kc".to_string())];
        assert!(cache.equities(&plo, "").is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p pktui odds:: -- --nocapture`
Expected: FAIL — `cannot find type OddsCache`.

- [ ] **Step 3: Implement the module**

Prepend the implementation above the test module in `src/ui/odds.rs`:

```rust
use std::cell::RefCell;
use std::str::FromStr;

use pkcore::play::board::Board;
use pkcore::play::game::Game;
use pkcore::play::hole_cards::HoleCards;

/// Caches per-seat split-pot equity, recomputing only when the contesting
/// cards or the board change.
///
/// Rendering borrows state immutably, so the cache stores its mutable
/// innards behind a `RefCell`.
#[derive(Default)]
pub struct OddsCache {
    inner: RefCell<Inner>,
}

#[derive(Default)]
struct Inner {
    key: String,
    value: Vec<(u8, f64)>,
}

impl OddsCache {
    /// Creates an empty cache.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::ui::odds::OddsCache;
    /// let _ = OddsCache::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns split-pot equity (`0.0..=1.0`) per active seat.
    ///
    /// `holes` is `(seat_index, "two-card string")` in seat order; `board` is
    /// the board display string (`""` preflop). Recomputes via
    /// [`Game::street_equities`] only when the inputs differ from the last
    /// call. Returns an empty vec for fewer than two seats, non-Hold'em hands,
    /// or unparseable input.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::ui::odds::OddsCache;
    /// let cache = OddsCache::new();
    /// let holes = vec![(0u8, "As Ah".to_string()), (1u8, "Ks Kh".to_string())];
    /// let eq = cache.equities(&holes, "");
    /// assert_eq!(eq.len(), 2);
    /// ```
    #[must_use]
    pub fn equities(&self, holes: &[(u8, String)], board: &str) -> Vec<(u8, f64)> {
        let key = signature(holes, board);
        let mut inner = self.inner.borrow_mut();
        if key != inner.key {
            inner.key = key;
            inner.value = compute(holes, board).unwrap_or_default();
        }
        inner.value.clone()
    }
}

/// Stable signature of the inputs — changes exactly when a card displayed
/// changes (deal, board advance, or a seat dropping out of `holes`).
fn signature(holes: &[(u8, String)], board: &str) -> String {
    let mut s = String::with_capacity(64);
    for (seat, cards) in holes {
        s.push_str(&seat.to_string());
        s.push(':');
        s.push_str(cards);
        s.push('|');
    }
    s.push('#');
    s.push_str(board);
    s
}

/// Builds a `Game` from the active seats and returns split-pot equity per
/// seat. Hold'em only; `None` for fewer than two hands or unparseable input.
fn compute(holes: &[(u8, String)], board: &str) -> Option<Vec<(u8, f64)>> {
    if holes.len() < 2 {
        return None;
    }
    let joined = holes
        .iter()
        .map(|(_, c)| c.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let hands = HoleCards::from_str(&joined).ok()?;
    // Each seat must contribute exactly one 2-card Hold'em hand.
    if hands.len() != holes.len() {
        return None;
    }
    let board = if board.trim().is_empty() {
        Board::default()
    } else {
        Board::from_str(board).ok()?
    };
    let game = Game::new(hands, board);
    let eq = game.street_equities().ok()?;
    Some(
        holes
            .iter()
            .enumerate()
            .map(|(i, (seat, _))| (*seat, eq.get(i).map_or(0.0, |e| e.equity)))
            .collect(),
    )
}
```

Add to `src/ui/mod.rs` (alongside the other `pub mod` lines):

```rust
pub mod odds;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p pktui odds:: -- --nocapture`
Expected: PASS (3 tests).

Run: `cargo test --doc -p pktui`
Expected: PASS (the two new doc tests included).

- [ ] **Step 5: Commit**

Provide this command to the user:

```bash
git add src/ui/odds.rs src/ui/mod.rs && git commit -m "feat(ui): OddsCache — change-keyed per-street equity"
```

---

### Task 5: `Win%` column + Arena wiring

**Files:**
- Modify: `src/ui/table.rs` (add `SeatRow.odds`; header + cell; `active_holes` + `apply_odds` helpers; Arena render call; update all `SeatRow` constructions)
- Modify: `src/modes/arena.rs` (add `pub odds: OddsCache` field + init)
- Test: inline tests in `src/ui/table.rs`

**Interfaces:**
- Consumes: `OddsCache::equities`; `TableNoCell` (`seats.get_seat(i)`, `seat.cards.as_slice()`, `seat.player.is_in_hand()`, `seat.cards.has_cards()`); `pkcore::card::Card::BLANK`; `state.session.table.board.to_string()`.
- Produces: `SeatRow.odds: Option<f64>`; a `Win%` column; `active_holes(&TableNoCell) -> Vec<(u8, String)>`; `apply_odds(&mut [SeatRow], &[(u8, String)], &str, &OddsCache)`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `src/ui/table.rs`:

```rust
    #[test]
    fn render_seats_shows_win_column_and_value() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let rows = vec![SeatRow {
            seat: 0,
            name: "gto".to_string(),
            chips: 9_500,
            hole: "Ah Kh".to_string(),
            bet: 0,
            tag: String::new(),
            folded: false,
            accent: Accent::None,
            pnl: None,
            action: String::new(),
            analysis: None,
            tokens: None,
            cost_micro_usd: None,
            odds: Some(0.824),
        }];
        let backend = TestBackend::new(170, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_seats(f, f.area(), &rows)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let header: String = (0..170).map(|x| buffer[(x, 1)].symbol()).collect();
        assert!(header.contains("Win%"), "header: {header}");
        let body: String = (0..170).map(|x| buffer[(x, 2)].symbol()).collect();
        assert!(body.contains("82.4%"), "body: {body}");
    }

    #[test]
    fn active_holes_collects_two_card_seats() {
        use pkcore::casino::game::ForcedBets;
        use pkcore::casino::table_no_cell::{PlayerNoCell, SeatNoCell, SeatsNoCell, TableNoCell};
        use std::str::FromStr;

        let mut s0 = SeatNoCell::new(PlayerNoCell::new_with_chips("a".into(), 1_000));
        s0.cards = pkcore::arrays::sliced::BoxedCards::from_str("As Ah").unwrap();
        let mut s1 = SeatNoCell::new(PlayerNoCell::new_with_chips("b".into(), 1_000));
        s1.cards = pkcore::arrays::sliced::BoxedCards::from_str("Ks Kh").unwrap();
        let table = TableNoCell::nlh_from_seats(SeatsNoCell::new(vec![s0, s1]), ForcedBets::new(10, 20));

        let holes = active_holes(&table);
        assert_eq!(holes.len(), 2);
        assert_eq!(holes[0].0, 0);
        assert_eq!(holes[0].1.split_whitespace().count(), 2);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p pktui --lib table::tests::render_seats_shows_win_column_and_value table::tests::active_holes_collects_two_card_seats`
Expected: FAIL — missing `odds` field on `SeatRow` / `active_holes` not found.

- [ ] **Step 3: Add the `odds` field and update every `SeatRow` construction**

In `src/ui/table.rs`, add to `struct SeatRow`:

```rust
    /// Double-dummy split-pot equity (`0.0..=1.0`) for this seat at the
    /// current street. `None` in Play mode, for folded seats, non-Hold'em
    /// tables, or when odds are unavailable.
    odds: Option<f64>,
```

Then set `odds: None` in the two existing constructions: in `seat_rows` (the `out.push(SeatRow { ... })`) and in `status_to_rows` (the `SeatRow { ... }` returned per seat). Spectate fills it in Task 6; Arena patches it below; Play leaves it `None`.

- [ ] **Step 4: Add the header, width, and cell**

In `render_seats`, append `Cell::from("Win%")` to the `header` row (after `"Analysis"`), append `Constraint::Length(7)` to the `widths` array (after the Analysis constraint), and add the cell to each body `Row::new(vec![...])` after `analysis_cell`:

```rust
            let odds_cell = match r.odds {
                None => Cell::from("—").style(Style::default().fg(Color::DarkGray)),
                Some(e) => Cell::from(format!("{:.1}%", e * 100.0))
                    .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            };
```

Add `odds_cell` as the final entry of the `Row::new(vec![...])`.

- [ ] **Step 5: Add the `active_holes` + `apply_odds` helpers**

Add to `src/ui/table.rs`:

```rust
/// Collects `(seat_index, "card card")` for every seat still in the hand
/// that holds exactly two cards (Hold'em). Seats that are empty, folded, or
/// hold a non-2-card hand are skipped.
fn active_holes(table: &TableNoCell) -> Vec<(u8, String)> {
    let n = u8::try_from(table.seats.0.len()).unwrap_or(u8::MAX);
    (0..n)
        .filter_map(|i| {
            let s = table.seats.get_seat(i)?;
            if s.is_empty() || !s.player.is_in_hand() || !s.cards.has_cards() {
                return None;
            }
            let cards: Vec<String> = s
                .cards
                .as_slice()
                .iter()
                .copied()
                .filter(|c| *c != pkcore::card::Card::BLANK)
                .map(|c| c.to_string())
                .collect();
            if cards.len() != 2 {
                return None;
            }
            Some((i, cards.join(" ")))
        })
        .collect()
}

/// Patches `rows` with cached equities for the active seats. No-op when fewer
/// than two seats are contesting.
fn apply_odds(rows: &mut [SeatRow], holes: &[(u8, String)], board: &str, cache: &crate::ui::odds::OddsCache) {
    if holes.len() < 2 {
        return;
    }
    for (seat, eq) in cache.equities(holes, board) {
        if let Some(row) = rows.iter_mut().find(|r| r.seat == seat) {
            row.odds = Some(eq);
        }
    }
}
```

- [ ] **Step 6: Wire Arena**

In `src/modes/arena.rs`, add to the `ArenaState` struct:

```rust
    /// Per-street odds cache for the Win% column.
    pub odds: crate::ui::odds::OddsCache,
```

and initialize it (`odds: crate::ui::odds::OddsCache::new(),`) wherever `ArenaState { ... }` is constructed (the compiler will flag each site).

In `src/ui/table.rs`, update `render_table_view_arena` so that after building `rows`:

```rust
    let mut rows = seat_rows(&state.session.table, None, active_seat, None, |seat| {
        state.seat_name(seat)
    });
    let holes = active_holes(&state.session.table);
    apply_odds(&mut rows, &holes, &state.session.table.board.to_string(), &state.odds);
    render_board(&state.session.table, frame, chunks[0]);
    render_seats(frame, chunks[1], &rows);
```

- [ ] **Step 7: Update the existing `render_seats_shows_pnl_column_header` test**

That test builds a `SeatRow` literal — add `odds: None,` to it so it compiles.

- [ ] **Step 8: Run tests + clippy**

Run: `cargo test -p pktui --lib table::`
Expected: PASS (new + existing table tests).

Run: `cargo clippy -p pktui --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 9: Commit**

Provide this command to the user:

```bash
git add src/ui/table.rs src/modes/arena.rs && git commit -m "feat(ui): Win% column with Arena double-dummy odds"
```

---

### Task 6: Spectate wiring

**Files:**
- Modify: `src/modes/spectate.rs` (add `pub odds: OddsCache` field + init in every constructor)
- Modify: `src/ui/table.rs` (`status_active_holes` helper; apply odds in `render_table_view_spectate`)
- Test: inline test in `src/ui/table.rs`

**Interfaces:**
- Consumes: `TableStatus { seats: Vec<SeatInfo>, board: String }`, `SeatInfo { seat_number, cards, state }`, `PlayerState`, `crate::ui::sort_hole_cards`, `apply_odds` (Task 5).
- Produces: `status_active_holes(&TableStatus) -> Vec<(u8, String)>`.

- [ ] **Step 1: Write the failing test**

Add to `#[cfg(test)] mod tests` in `src/ui/table.rs`:

```rust
    #[test]
    fn status_active_holes_excludes_folded_and_non_holdem() {
        use pkdealer_proto::dealer::{SeatInfo, TableStatus};
        let status = TableStatus {
            seats: vec![
                SeatInfo { seat_number: 0, player_name: "a".into(), cards: "As Ah".into(), state: 4, ..Default::default() },
                SeatInfo { seat_number: 1, player_name: "b".into(), cards: "Ks Kh".into(), state: 4, ..Default::default() },
                SeatInfo { seat_number: 2, player_name: "c".into(), cards: "7c 2d".into(), state: 8, ..Default::default() }, // folded
            ],
            board: "Ah Kd Qc".into(),
            hand_in_progress: true,
            ..Default::default()
        };
        let holes = status_active_holes(&status);
        assert_eq!(holes.len(), 2);
        assert!(holes.iter().all(|(s, _)| *s != 2), "folded seat excluded");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pktui --lib table::tests::status_active_holes_excludes_folded_and_non_holdem`
Expected: FAIL — `status_active_holes` not found.

- [ ] **Step 3: Add the helper**

In `src/ui/table.rs`:

```rust
/// Collects `(seat_index, "card card")` for active (non-folded, non-out)
/// spectated seats whose revealed cards form exactly a 2-card Hold'em hand.
fn status_active_holes(status: &TableStatus) -> Vec<(u8, String)> {
    status
        .seats
        .iter()
        .filter_map(|s| {
            let folded =
                s.state == PlayerState::Folded as i32 || s.state == PlayerState::Out as i32;
            if folded {
                return None;
            }
            let cards = crate::ui::sort_hole_cards(&s.cards);
            if cards == "??" || cards.trim().is_empty() || cards.split_whitespace().count() != 2 {
                return None;
            }
            Some((u8::try_from(s.seat_number).unwrap_or(u8::MAX), cards))
        })
        .collect()
}
```

- [ ] **Step 4: Add the cache field to `SpectateState`**

In `src/modes/spectate.rs`, add to the struct:

```rust
    /// Per-street odds cache for the Win% column.
    pub odds: crate::ui::odds::OddsCache,
```

and add `odds: crate::ui::odds::OddsCache::new(),` to every `SpectateState { ... }` construction (including the `detached` test constructor — the compiler will flag each).

- [ ] **Step 5: Apply odds in the spectate render**

In `render_table_view_spectate`, where `status` is present:

```rust
        let mut rows = status_to_rows(status);
        let holes = status_active_holes(status);
        apply_odds(&mut rows, &holes, &status.board, &state.odds);
        render_seats(frame, chunks[1], &rows);
```

- [ ] **Step 6: Run tests + clippy**

Run: `cargo test -p pktui --lib table::`
Expected: PASS.

Run: `cargo clippy -p pktui --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 7: Commit**

Provide this command to the user:

```bash
git add src/ui/table.rs src/modes/spectate.rs && git commit -m "feat(ui): spectate Win% odds from revealed seats"
```

---

### Task 7: Replay wiring

**Files:**
- Modify: `src/modes/replay.rs` (add `pub odds: OddsCache` field + init in `from_file`)
- Modify: `src/ui/replay_view.rs` (`replay_holes` + `replay_board` helpers; annotate player rows with win%)
- Test: inline tests in `src/ui/replay_view.rs`

**Interfaces:**
- Consumes: `HandHistory` (`.players[].seat: u8`, `.players[].hole_cards: Option<String>`; `.streets: Option<_>` with `.flop.cards: String`, `.turn.card: String`, `.river.card: String`); `OddsCache::equities`.
- Produces: `replay_holes(&HandHistory) -> Vec<(u8, String)>`; `replay_board(&HandHistory, usize) -> String`.

**Note:** Replay shows the full-field double-dummy odds among all seats with recorded hole cards at the displayed board. It does **not** fold-adjust (which would require replaying the action log) — a deliberate simplification appropriate to a study/replay view; folded-seat exclusion can be a later refinement.

- [ ] **Step 1: Write the failing tests**

Add a test module section to `src/ui/replay_view.rs` (extend the existing `#[cfg(test)] mod tests`). Build a minimal `HandHistory` via the same path the replay loader uses, or assert the pure helpers directly:

```rust
    #[test]
    fn replay_board_accumulates_per_street() {
        // Stub a HandHistory with a known board; uses pkcore's hand_history types.
        use pkcore::hand_history::HandHistory;
        let yaml = sample_hand_yaml();
        let hh: HandHistory = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(replay_board(&hh, 0), "");                  // preflop
        assert_eq!(replay_board(&hh, 1), "Ah Kd Qc");          // flop
        assert_eq!(replay_board(&hh, 2), "Ah Kd Qc 2s");       // turn
        assert_eq!(replay_board(&hh, 3), "Ah Kd Qc 2s 7h");    // river
    }

    #[test]
    fn replay_holes_collects_recorded_hands() {
        use pkcore::hand_history::HandHistory;
        let hh: HandHistory = serde_yaml::from_str(&sample_hand_yaml()).unwrap();
        let holes = replay_holes(&hh);
        assert!(holes.len() >= 2);
        assert!(holes.iter().all(|(_, c)| c.split_whitespace().count() == 2));
    }
```

> The implementer writes `sample_hand_yaml()` as a `fn -> String` returning a minimal valid hand-history YAML with two players (each with `hole_cards`) and `streets.flop.cards = "Ah Kd Qc"`, `streets.turn.card = "2s"`, `streets.river.card = "7h"`. Derive its exact shape by serializing a `HandHistory` the loader already accepts (e.g. round-trip an existing fixture under `tests/` or `pkcore`'s `interactive_play` output) so the field names match the deserializer. If `serde_yaml` is not already a dev-dependency, reuse `HandCollection::from_yaml` + `.hands()[0]` instead to avoid adding a dependency.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p pktui --lib replay_view::tests::replay_board_accumulates_per_street replay_view::tests::replay_holes_collects_recorded_hands`
Expected: FAIL — helpers not found.

- [ ] **Step 3: Implement the helpers**

In `src/ui/replay_view.rs`:

```rust
/// `(seat, "card card")` for every player with two recorded hole cards.
fn replay_holes(hand: &HandHistory) -> Vec<(u8, String)> {
    hand.players
        .iter()
        .filter_map(|p| {
            let cards = p.hole_cards.as_deref()?;
            if cards.split_whitespace().count() != 2 {
                return None;
            }
            Some((p.seat, cards.to_string()))
        })
        .collect()
}

/// The board string visible at `street` (0=preflop … 3=river, 4=results),
/// accumulating flop + turn + river cards.
fn replay_board(hand: &HandHistory, street: usize) -> String {
    let Some(streets) = hand.streets.as_ref() else {
        return String::new();
    };
    let mut parts: Vec<String> = Vec::new();
    if street >= 1 {
        if let Some(f) = streets.flop.as_ref() {
            parts.push(f.cards.clone());
        }
    }
    if street >= 2 {
        if let Some(t) = streets.turn.as_ref() {
            parts.push(t.card.clone());
        }
    }
    if street >= 3 {
        if let Some(r) = streets.river.as_ref() {
            parts.push(r.card.clone());
        }
    }
    parts.join(" ").trim().to_string()
}
```

- [ ] **Step 4: Show win% in the header rows**

Add the cache field to `ReplayState` in `src/modes/replay.rs`:

```rust
    /// Per-street odds cache for the replay win% display.
    pub odds: crate::ui::odds::OddsCache,
```

and initialize it in `from_file` (`odds: crate::ui::odds::OddsCache::new(),`).

In `src/ui/replay_view.rs::render`, compute equities for the current street and pass them to `render_header`. Change `render_header`'s signature to accept `&[(u8, f64)]` and append the per-seat figure to each player line:

```rust
    // in render(), after `let Some(hand) = ...`:
    let holes = replay_holes(hand);
    let board = replay_board(hand, state.street_index);
    let eq = state.odds.equities(&holes, &board);
    // pass `&eq` into render_header(hand, state.street_index, &eq, frame, chunks[0]);
```

and in `render_header`, when building each player line, look up the seat's equity and append it:

```rust
        let win = eq
            .iter()
            .find(|(s, _)| *s == p.seat)
            .map_or_else(|| "  —".to_string(), |(_, e)| format!("  {:.1}%", e * 100.0));
        lines.push(Line::raw(format!(
            "{:>4}  {:<22}  {:>5}  {:<14}{win}",
            p.seat, p.name, chips(p.stack), hole,
        )));
```

(Adjust the header legend line `"seat  name ... hole"` to include a trailing `win%` label.)

- [ ] **Step 5: Run tests + clippy**

Run: `cargo test -p pktui --lib replay_view::`
Expected: PASS.

Run: `cargo clippy -p pktui --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Full test sweep**

Run: `cargo test -p pktui`
Expected: PASS (all unit + integration + doc tests).

- [ ] **Step 7: Commit**

Provide this command to the user:

```bash
git add src/ui/replay_view.rs src/modes/replay.rs && git commit -m "feat(ui): replay per-street win% display"
```

---

## Self-Review Notes

- **Spec coverage:** pkcore API (Task 1), version/handoff (Task 2), dep bump (Task 3), `OddsCache`/cache key = refresh trigger (Task 4), Win% column + Arena (Task 5), Spectate (Task 6), Replay (Task 7). Play deliberately untouched (odds stays `None`). ✓
- **Equity rule** `win + tie/2` applied at the pkcore source (Task 1), so all consumers inherit it. ✓
- **Hold'em-only guard** enforced in `compute` (`hands.len() != holes.len()`), `active_holes`, `status_active_holes`, `replay_holes` (2-card checks). ✓
- **Type consistency:** `StreetEquity { win, tie, equity }` (Task 1) ↔ `OddsCache::equities -> Vec<(u8, f64)>` (Task 4) ↔ `SeatRow.odds: Option<f64>` (Task 5). `apply_odds` (Task 5) reused by Spectate (Task 6). ✓
- **Known simplification:** Replay is not fold-adjusted (documented in Task 7).
