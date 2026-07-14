//! Arena mode: nine bots, watch-only.
//!
//! Identical to [`PlayState`](crate::modes::PlayState) except seat 0 is also
//! a bot — there is no [`Awaiting::Human`](crate::modes::Awaiting::Human)
//! state. The user controls only the speed (`+`/`-` to adjust) and quitting.

use std::time::{Duration, Instant};

use pkcore::bot::profile::BotProfile;
use pkcore::casino::game::ForcedBets;
use pkcore::casino::session::{PokerSession, SessionStep};
use pkcore::casino::table::{Player, Seat, Seats, Table};
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;

use crate::cli::{ArenaArgs, Variant};
use crate::error::Result;
use crate::log_panel::{LogPanel, Severity};
use crate::modes::play::describe_action;
use crate::modes::seeded_rng;

/// Phase of the watch-only arena.
///
/// # Examples
///
/// ```
/// use pktui::modes::arena::ArenaPhase;
/// assert_eq!(ArenaPhase::default(), ArenaPhase::Running);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ArenaPhase {
    /// Bots are actively dealing / betting.
    #[default]
    Running,
    /// Hand finished; will auto-deal on next tick.
    HandComplete,
    /// Only one funded seat remains.
    SessionOver,
}

/// All state needed to drive Arena mode.
pub struct ArenaState {
    /// The live engine session.
    pub session: PokerSession,
    /// The 9 bot profiles, indexed by seat.
    pub bots: Vec<BotProfile>,
    /// RNG used for bot decisions.
    pub rng: SmallRng,
    /// Current phase.
    pub phase: ArenaPhase,
    /// Wall-clock instant of the last applied bot action.
    pub last_step_at: Instant,
    /// Minimum delay between consecutive bot actions.
    pub speed: Duration,
    /// Resolved RNG seed.
    pub seed: u64,
    /// Per-street odds cache for the Win% column.
    pub odds: crate::ui::odds::OddsCache,
    /// When `true`, auto-advance is suspended so spectators can study the
    /// current hands and odds; the spectator advances one action at a time
    /// via [`Self::step`].
    pub paused: bool,
}

/// Mirrors `play::build_table` for arena's args struct.
fn build_table(args: &ArenaArgs, seats: Seats) -> Table {
    match args.game.variant {
        Variant::Nlhe => Table::nlh_from_seats(
            seats,
            ForcedBets::new(args.game.small_blind, args.game.big_blind),
        ),
        Variant::Plo => Table::plo_from_seats(seats, (args.game.small_blind, args.game.big_blind)),
        Variant::StudHi => Table::stud_hi_from_seats(
            seats,
            args.game.ante.unwrap_or(10),
            args.game.bring_in.unwrap_or(25),
            args.game.small_bet.unwrap_or(args.game.small_blind),
            args.game.big_bet.unwrap_or(args.game.big_blind),
        ),
        Variant::Razz => Table::razz_from_seats(
            seats,
            args.game.ante.unwrap_or(10),
            args.game.bring_in.unwrap_or(25),
            args.game.small_bet.unwrap_or(args.game.small_blind),
            args.game.big_bet.unwrap_or(args.game.big_blind),
        ),
    }
}

fn start_log_line(args: &ArenaArgs, seed: u64) -> String {
    match args.game.variant {
        Variant::Nlhe => format!(
            "Arena started: NLHE blinds {}/{} starting {} chips, seed={seed}",
            args.game.small_blind, args.game.big_blind, args.game.chips
        ),
        Variant::Plo => format!(
            "Arena started: PLO blinds {}/{} starting {} chips, seed={seed}",
            args.game.small_blind, args.game.big_blind, args.game.chips
        ),
        Variant::StudHi => format!(
            "Arena started: Stud Hi ante {} / bring-in {} / bets {}-{} starting {} chips, seed={seed}",
            args.game.ante.unwrap_or(10),
            args.game.bring_in.unwrap_or(25),
            args.game.small_bet.unwrap_or(args.game.small_blind),
            args.game.big_bet.unwrap_or(args.game.big_blind),
            args.game.chips,
        ),
        Variant::Razz => format!(
            "Arena started: Razz ante {} / bring-in {} / bets {}-{} starting {} chips, seed={seed}",
            args.game.ante.unwrap_or(10),
            args.game.bring_in.unwrap_or(25),
            args.game.small_bet.unwrap_or(args.game.small_blind),
            args.game.big_bet.unwrap_or(args.game.big_blind),
            args.game.chips,
        ),
    }
}

impl ArenaState {
    /// Initialises an Arena session with 9 bots seated.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Engine`] if pkcore rejects the table or the
    /// initial deal.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::cli::ArenaArgs;
    /// use pktui::log_panel::LogPanel;
    /// use pktui::modes::ArenaState;
    ///
    /// let mut log = LogPanel::new();
    /// let s = ArenaState::new(&ArenaArgs::default(), &mut log).unwrap();
    /// assert_eq!(s.bots.len(), 9);
    /// ```
    pub fn new(args: &ArenaArgs, log: &mut LogPanel) -> Result<Self> {
        let (mut rng, seed) = seeded_rng(args.game.seed);
        // Stud-family games cap at 8 seats total (52-card deck constraint).
        let bot_count = args.game.variant.max_seats();
        let mut pool = BotProfile::default_profiles();
        pool.push(BotProfile::joker());
        pool.shuffle(&mut rng);
        let bots: Vec<BotProfile> = pool.into_iter().take(bot_count).collect();

        let seats: Vec<Seat> = bots
            .iter()
            .map(|b| Seat::new(Player::new_with_chips(b.name.clone(), args.game.chips)))
            .collect();

        let table = build_table(args, Seats::new(seats));

        let mut session = PokerSession::new(table);
        session.start_hand()?;
        log.push(Severity::Info, start_log_line(args, seed));
        if args.game.variant != Variant::Nlhe {
            log.push(
                Severity::Info,
                "Note: UI rendering for non-NLHE variants is preliminary — \
                 board/street labels assume Hold'em."
                    .to_string(),
            );
        }
        log.push(Severity::Info, "Hand 1 dealt".to_string());

        Ok(Self {
            session,
            bots,
            rng,
            phase: ArenaPhase::Running,
            last_step_at: Instant::now()
                .checked_sub(Duration::from_secs(1))
                .unwrap_or_else(Instant::now),
            speed: Duration::from_millis(args.speed_ms),
            seed,
            odds: crate::ui::odds::OddsCache::new(),
            paused: false,
        })
    }

    /// Returns the seat's bot name.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::cli::ArenaArgs;
    /// use pktui::log_panel::LogPanel;
    /// use pktui::modes::ArenaState;
    ///
    /// let mut log = LogPanel::new();
    /// let s = ArenaState::new(&ArenaArgs::default(), &mut log).unwrap();
    /// assert_eq!(s.seat_name(0), s.bots[0].name);
    /// ```
    #[must_use]
    pub fn seat_name(&self, seat: u8) -> String {
        self.bots
            .get(seat as usize)
            .map_or_else(|| format!("seat {seat}"), |b| b.name.clone())
    }

    /// Speeds bots up (smaller delay) by 100 ms, floored at 50 ms.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use pktui::cli::ArenaArgs;
    /// use pktui::log_panel::LogPanel;
    /// use pktui::modes::ArenaState;
    ///
    /// let mut log = LogPanel::new();
    /// let mut s = ArenaState::new(&ArenaArgs::default(), &mut log).unwrap();
    /// s.speed = Duration::from_millis(800);
    /// s.speed_up();
    /// assert_eq!(s.speed, Duration::from_millis(700));
    /// ```
    pub fn speed_up(&mut self) {
        let ms = speed_millis(self.speed);
        self.speed = Duration::from_millis(ms.saturating_sub(100).max(50));
    }

    /// Slows bots down by 100 ms, capped at 5000 ms.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use pktui::cli::ArenaArgs;
    /// use pktui::log_panel::LogPanel;
    /// use pktui::modes::ArenaState;
    ///
    /// let mut log = LogPanel::new();
    /// let mut s = ArenaState::new(&ArenaArgs::default(), &mut log).unwrap();
    /// s.speed = Duration::from_millis(800);
    /// s.speed_down();
    /// assert_eq!(s.speed, Duration::from_millis(900));
    /// ```
    pub fn speed_down(&mut self) {
        let ms = speed_millis(self.speed);
        self.speed = Duration::from_millis((ms + 100).min(5000));
    }

    /// Toggles the paused state.
    ///
    /// While paused, [`Self::tick`] stops auto-advancing so spectators can
    /// read each seat's hand and odds; they advance manually with
    /// [`Self::step`].
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::cli::ArenaArgs;
    /// use pktui::log_panel::LogPanel;
    /// use pktui::modes::ArenaState;
    ///
    /// let mut log = LogPanel::new();
    /// let mut s = ArenaState::new(&ArenaArgs::default(), &mut log).unwrap();
    /// assert!(!s.paused);
    /// s.toggle_pause();
    /// assert!(s.paused);
    /// ```
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    /// Drives the arena forward by one step (paced by [`Self::speed`]).
    ///
    /// No-op while [`Self::paused`] is set — a paused arena advances only via
    /// [`Self::step`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Engine`] on any pkcore failure.
    pub fn tick(&mut self, log: &mut LogPanel) -> Result<bool> {
        if matches!(self.phase, ArenaPhase::SessionOver) {
            return Ok(false);
        }
        if self.paused {
            return Ok(false);
        }
        if self.last_step_at.elapsed() < self.speed {
            return Ok(false);
        }
        self.step_once(log)
    }

    /// Manually advances exactly one step while paused, ignoring the speed
    /// gate. Lets a spectator step through a hand one action at a time. No-op
    /// once the session is over.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Engine`] on any pkcore failure.
    pub fn step(&mut self, log: &mut LogPanel) -> Result<bool> {
        if matches!(self.phase, ArenaPhase::SessionOver) {
            return Ok(false);
        }
        self.step_once(log)
    }

    /// Performs a single arena step: deal the next hand if one just finished,
    /// otherwise apply the next bot action / street advance. Shared by the
    /// timed [`Self::tick`] and the manual [`Self::step`]; it neither checks
    /// [`Self::paused`] nor the speed gate.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Engine`] on any pkcore failure.
    fn step_once(&mut self, log: &mut LogPanel) -> Result<bool> {
        if matches!(self.phase, ArenaPhase::HandComplete) {
            self.session.start_hand()?;
            log.push(
                Severity::Info,
                format!("Hand {} dealt", self.session.hand_number),
            );
            self.phase = ArenaPhase::Running;
            self.last_step_at = Instant::now();
            return Ok(true);
        }

        match self.session.next_step() {
            SessionStep::PlayerToAct(seat) => {
                let action =
                    self.bots[seat as usize].decide(&self.session.table, seat, &mut self.rng);
                let desc = describe_action(&self.session.table, seat, action);
                self.session.apply_action(seat, action)?;
                log.push(
                    severity_for_action(action),
                    format!("{}: {desc}", self.seat_name(seat)),
                );
                self.last_step_at = Instant::now();
                Ok(true)
            }
            SessionStep::StreetAdvanced => {
                let board = self.session.table.board.to_string();
                log.push(Severity::Info, format!("Board: {board}"));
                self.last_step_at = Instant::now();
                Ok(true)
            }
            SessionStep::HandComplete => {
                let winnings = self.session.end_hand()?;
                let n = u8::try_from(self.session.table.seats.0.len()).unwrap_or(u8::MAX);
                for w in winnings.vec() {
                    let chips = w.equity.chips;
                    for seat in 0..n {
                        if w.equity.seats.contains(seat) {
                            log.push(
                                Severity::Win,
                                format!("{} wins {} chips", self.seat_name(seat), chips),
                            );
                        }
                    }
                }
                self.session.table.button_up();
                let busted = self.session.eliminate_busted();
                for s in busted {
                    log.push(Severity::Error, format!("{} eliminated", self.seat_name(s)));
                }
                if self.session.count_funded() < 2 {
                    log.push(Severity::Info, "Arena over. Press q to quit.".to_string());
                    self.phase = ArenaPhase::SessionOver;
                } else {
                    self.phase = ArenaPhase::HandComplete;
                }
                self.last_step_at = Instant::now();
                Ok(true)
            }
        }
    }
}

fn severity_for_action(action: pkcore::casino::action::PlayerAction) -> Severity {
    use pkcore::casino::action::PlayerAction::{AllIn, Bet, Call, Check, Fold, Raise};
    match action {
        Fold => Severity::Fold,
        Check | Call => Severity::Info,
        Bet(_) | Raise(_) | AllIn => Severity::Action,
    }
}

/// `Duration::as_millis()` returns `u128`; pktui's speed is always set from
/// `u64` milliseconds and clamped to ≤ `5_000`, so saturating to `u64::MAX`
/// here can never lose information.
#[must_use]
fn speed_millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{ArenaArgs, Variant};

    fn arena_with_seed(seed: u64) -> ArenaState {
        let mut log = LogPanel::new();
        let mut args = ArenaArgs::default();
        args.game.seed = Some(seed);
        ArenaState::new(&args, &mut log).unwrap()
    }

    #[test]
    fn nine_bots_seated() {
        let s = arena_with_seed(1);
        assert_eq!(s.session.table.seats.0.len(), 9);
    }

    #[test]
    fn new_stud_hi_seats_six_bots_and_logs_warning() {
        let mut log = LogPanel::new();
        let mut args = ArenaArgs::default();
        args.game.seed = Some(8);
        args.game.variant = Variant::StudHi;
        let s = ArenaState::new(&args, &mut log).unwrap();
        assert_eq!(s.session.table.seats.0.len(), 6);
        let logged: String = log
            .iter()
            .map(|line| line.text.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(logged.contains("Stud Hi"), "log was: {logged}");
        assert!(logged.contains("preliminary"), "log was: {logged}");
    }

    #[test]
    fn deterministic_for_same_seed() {
        let a: Vec<_> = arena_with_seed(5)
            .bots
            .iter()
            .map(|b| b.name.clone())
            .collect();
        let b: Vec<_> = arena_with_seed(5)
            .bots
            .iter()
            .map(|b| b.name.clone())
            .collect();
        assert_eq!(a, b);
    }

    #[test]
    fn toggle_pause_flips() {
        let mut s = arena_with_seed(3);
        assert!(!s.paused);
        s.toggle_pause();
        assert!(s.paused);
        s.toggle_pause();
        assert!(!s.paused);
    }

    #[test]
    fn paused_tick_does_not_advance() {
        let mut s = arena_with_seed(11);
        let mut log = LogPanel::new();
        // Zero delay means an un-paused tick would always advance.
        s.speed = Duration::from_millis(0);
        s.paused = true;
        // Even across many ticks, a paused arena never advances.
        for _ in 0..50 {
            assert!(!s.tick(&mut log).unwrap());
        }
    }

    #[test]
    fn step_advances_while_paused() {
        let mut s = arena_with_seed(11);
        let mut log = LogPanel::new();
        // Long delay so a timed tick could never fire — only step can advance.
        s.speed = Duration::from_secs(3600);
        s.paused = true;
        assert!(
            !s.tick(&mut log).unwrap(),
            "tick must not advance while paused"
        );
        assert!(
            s.step(&mut log).unwrap(),
            "step must advance one action while paused"
        );
    }

    #[test]
    fn speed_controls_clamp() {
        let mut s = arena_with_seed(3);
        s.speed = Duration::from_millis(60);
        s.speed_up();
        assert!(s.speed >= Duration::from_millis(50));
        s.speed = Duration::from_millis(4990);
        s.speed_down();
        assert!(s.speed <= Duration::from_secs(5));
    }

    #[test]
    fn ticks_eventually_complete_a_hand() {
        let mut s = arena_with_seed(11);
        let mut log = LogPanel::new();
        s.speed = Duration::from_millis(0);
        for _ in 0..500 {
            if !matches!(s.phase, ArenaPhase::Running) {
                break;
            }
            let _ = s.tick(&mut log);
        }
        assert!(matches!(
            s.phase,
            ArenaPhase::HandComplete | ArenaPhase::SessionOver
        ));
    }
}
