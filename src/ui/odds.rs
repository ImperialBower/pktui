//! Per-street double-dummy win% with change-keyed caching.

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

#[cfg(test)]
#[allow(non_snake_case)]
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
        let _ = cache.equities(&holes(), "");
        let k1 = cache.inner.borrow().key.clone();
        let _ = cache.equities(&holes(), "");
        assert_eq!(
            cache.inner.borrow().key,
            k1,
            "same inputs must not change key"
        );
        let _ = cache.equities(&holes(), "Ah Kd Qc");
        assert_ne!(cache.inner.borrow().key, k1, "new board must change key");
    }

    #[test]
    fn equities__rejects_single_seat_and_non_holdem() {
        let cache = OddsCache::new();
        assert!(cache.equities(&[(0, "As Ah".to_string())], "").is_empty());
        // 4-card hands (PLO) do not group into one Two per seat → empty.
        let plo = vec![
            (0, "As Ah Ad Ac".to_string()),
            (1, "Ks Kh Kd Kc".to_string()),
        ];
        assert!(cache.equities(&plo, "").is_empty());
    }
}
