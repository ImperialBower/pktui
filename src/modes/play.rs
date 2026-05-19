//! Play mode: one human at seat 0, eight bots at seats 1-8.
//!
//! The mode owns the live [`PokerSession`], the bot roster, an RNG, the
//! current waiting state ([`Awaiting`]), and a numeric bet-amount field
//! ([`BetField`]) the user can adjust before confirming a bet/raise.

use std::str::FromStr;
use std::time::{Duration, Instant};

use pkcore::analysis::eval::Eval;
use pkcore::arrays::HandRanker;
use pkcore::arrays::seven::Seven;
use pkcore::bot::profile::BotProfile;
use pkcore::casino::action::PlayerAction;
use pkcore::casino::game::ForcedBets;
use pkcore::casino::session::{PokerSession, SessionStep};
use pkcore::casino::table_no_cell::{PlayerNoCell, SeatNoCell, SeatsNoCell, TableNoCell};
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;

use crate::cli::{PlayArgs, Variant};
use crate::error::Result;
use crate::log_panel::{LogPanel, Severity};
use crate::modes::seeded_rng;

/// What the engine is currently waiting on.
///
/// `Bot` means an automated decision is pending and should fire on the next
/// tick (paced so the user can read each move). `Human(seat)` means the UI
/// must collect a keystroke. `HandComplete` is the brief pause between hands
/// where results are visible. `SessionOver` is terminal — the human busted
/// or only one funded seat remains.
///
/// # Examples
///
/// ```
/// use pktui::modes::Awaiting;
/// let a = Awaiting::Bot;
/// matches!(a, Awaiting::Bot);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Awaiting {
    /// A bot will act on the next tick.
    Bot,
    /// Waiting for a human keystroke; seat is next-to-act.
    Human(u8),
    /// Hand finished; press Enter to deal next.
    HandComplete,
    /// Session over (hero busted or only one player left).
    SessionOver,
}

/// Numeric bet/raise amount the user is composing.
///
/// The TUI shows this as `[ Bet: 200 ]` next to the action bar. Pre-set
/// hotkeys (`1` = min, `2` = ½-pot, `3` = pot) overwrite the value;
/// `+`/`-` and digit keys adjust it.
///
/// # Examples
///
/// ```
/// use pktui::modes::BetField;
/// let mut b = BetField::default();
/// b.set(200);
/// assert_eq!(b.amount(), 200);
/// b.bump(50);
/// assert_eq!(b.amount(), 250);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct BetField {
    amount: usize,
}

impl BetField {
    /// Returns the current amount.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::modes::BetField;
    /// assert_eq!(BetField::default().amount(), 0);
    /// ```
    #[must_use]
    pub fn amount(&self) -> usize {
        self.amount
    }

    /// Replaces the amount.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::modes::BetField;
    /// let mut b = BetField::default();
    /// b.set(123);
    /// assert_eq!(b.amount(), 123);
    /// ```
    pub fn set(&mut self, n: usize) {
        self.amount = n;
    }

    /// Adds `delta` chips to the amount, saturating at `usize::MAX`.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::modes::BetField;
    /// let mut b = BetField::default();
    /// b.set(100);
    /// b.bump(25);
    /// assert_eq!(b.amount(), 125);
    /// ```
    pub fn bump(&mut self, delta: usize) {
        self.amount = self.amount.saturating_add(delta);
    }

    /// Subtracts `delta` chips, saturating at zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::modes::BetField;
    /// let mut b = BetField::default();
    /// b.set(100);
    /// b.cut(40);
    /// assert_eq!(b.amount(), 60);
    /// b.cut(999);
    /// assert_eq!(b.amount(), 0);
    /// ```
    pub fn cut(&mut self, delta: usize) {
        self.amount = self.amount.saturating_sub(delta);
    }

    /// Appends a single decimal digit (0-9) to the amount, ignoring overflow.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::modes::BetField;
    /// let mut b = BetField::default();
    /// b.push_digit(2);
    /// b.push_digit(5);
    /// assert_eq!(b.amount(), 25);
    /// ```
    pub fn push_digit(&mut self, d: u8) {
        if d > 9 {
            return;
        }
        self.amount = self.amount.saturating_mul(10).saturating_add(d as usize);
    }

    /// Removes the last decimal digit (integer division by 10).
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::modes::BetField;
    /// let mut b = BetField::default();
    /// b.set(123);
    /// b.pop_digit();
    /// assert_eq!(b.amount(), 12);
    /// ```
    pub fn pop_digit(&mut self) {
        self.amount /= 10;
    }
}

/// The seat occupied by the human player.
pub const HERO_SEAT: u8 = 0;
/// The display name shown for the human player.
pub const HERO_NAME: &str = "You";

/// One row of the showdown reveal — one per active (non-folded) seat at the
/// moment the hand ends.
///
/// pktui captures this snapshot **before** calling
/// [`PokerSession::end_hand`](pkcore::casino::session::PokerSession::end_hand),
/// because that call resets the table and clears hole cards. The renderer
/// then displays the snapshot during [`Awaiting::HandComplete`] so the user
/// can see what every active player held — even hands they folded in.
///
/// `eval` is `Some` only when the board reached the river (5 community cards),
/// which is the only moment a 7-card evaluation is well-defined.
///
/// # Examples
///
/// ```
/// use pktui::modes::play::ShowdownSeat;
/// let s = ShowdownSeat { seat: 1, name: "gto".into(), hole: "Ah Kh".into(), hand_class: Some("Two Pair".into()) };
/// assert_eq!(s.seat, 1);
/// ```
#[derive(Debug, Clone)]
pub struct ShowdownSeat {
    /// Seat index (0-8 in 9-handed Hold'em).
    pub seat: u8,
    /// Display name (hero label or bot profile name).
    pub name: String,
    /// Hole cards as a display string, e.g. `"Ah Kh"`.
    pub hole: String,
    /// Best 5-card hand class (e.g. `"Pair of Kings"`), if a full 5-card
    /// board was available to evaluate against.
    pub hand_class: Option<String>,
}

/// All state needed to drive Play mode.
pub struct PlayState {
    /// The live engine session.
    pub session: PokerSession,
    /// The 8 bot profiles (indices 0..8 correspond to seats 1..=8).
    pub bots: Vec<BotProfile>,
    /// RNG used for bot decisions. Seedable for determinism.
    pub rng: SmallRng,
    /// What the engine is waiting on right now.
    pub awaiting: Awaiting,
    /// Composed bet/raise amount.
    pub bet: BetField,
    /// Wall-clock instant of the last applied bot action — used to throttle
    /// bot pacing.
    pub last_step_at: Instant,
    /// Minimum delay between consecutive bot actions.
    pub speed: Duration,
    /// Resolved RNG seed (so the user can reproduce the session).
    pub seed: u64,
    /// Snapshot of the most recent showdown — `Some` while
    /// [`Awaiting::HandComplete`] is showing the reveal, `None` otherwise.
    pub last_showdown: Option<Vec<ShowdownSeat>>,
}

/// Dispatches table construction to the variant-specific pkcore constructor.
///
/// NLHE uses blinds; Stud Hi uses ante + bring-in + small-bet/big-bet. When
/// `--small-bet`/`--big-bet` aren't supplied for Stud, they fall back to
/// `--small-blind`/`--big-blind` so the existing CLI defaults still produce
/// a playable table.
fn build_table(args: &PlayArgs, seats: SeatsNoCell) -> TableNoCell {
    match args.game.variant {
        Variant::Nlhe => TableNoCell::nlh_from_seats(
            seats,
            ForcedBets::new(args.game.small_blind, args.game.big_blind),
        ),
        Variant::StudHi => TableNoCell::stud_hi_from_seats(
            seats,
            args.game.ante.unwrap_or(10),
            args.game.bring_in.unwrap_or(25),
            args.game.small_bet.unwrap_or(args.game.small_blind),
            args.game.big_bet.unwrap_or(args.game.big_blind),
        ),
    }
}

fn start_log_line(args: &PlayArgs, seed: u64) -> String {
    match args.game.variant {
        Variant::Nlhe => format!(
            "Play started: NLHE blinds {}/{} starting {} chips, seed={seed}",
            args.game.small_blind, args.game.big_blind, args.game.chips
        ),
        Variant::StudHi => format!(
            "Play started: Stud Hi ante {} / bring-in {} / bets {}-{} starting {} chips, seed={seed}",
            args.game.ante.unwrap_or(10),
            args.game.bring_in.unwrap_or(25),
            args.game.small_bet.unwrap_or(args.game.small_blind),
            args.game.big_bet.unwrap_or(args.game.big_blind),
            args.game.chips,
        ),
    }
}

impl PlayState {
    /// Initialises a Play session: builds nine seats (hero + 8 random bots),
    /// posts blinds, deals the first hand, and primes the engine.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Engine`] if `pkcore` rejects the table or
    /// initial deal — for example, blinds larger than the starting stack.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::cli::PlayArgs;
    /// use pktui::log_panel::LogPanel;
    /// use pktui::modes::PlayState;
    ///
    /// let mut log = LogPanel::new();
    /// let state = PlayState::new(&PlayArgs::default(), &mut log).unwrap();
    /// assert_eq!(state.bots.len(), 8);
    /// ```
    pub fn new(args: &PlayArgs, log: &mut LogPanel) -> Result<Self> {
        let (mut rng, seed) = seeded_rng(args.game.seed);

        // Pick bots out of the default pool + joker (matches pkarena0-web).
        // Stud-family games cap at 8 seats total (52-card deck constraint),
        // so we only seat `max_seats - 1` bots for those variants.
        let bot_count = args.game.variant.max_seats().saturating_sub(1);
        let mut pool = BotProfile::default_profiles();
        pool.push(BotProfile::joker());
        pool.shuffle(&mut rng);
        let bots: Vec<BotProfile> = pool.into_iter().take(bot_count).collect();

        let mut seats = vec![SeatNoCell::new(PlayerNoCell::new_with_chips(
            HERO_NAME.to_string(),
            args.game.chips,
        ))];
        for b in &bots {
            seats.push(SeatNoCell::new(PlayerNoCell::new_with_chips(
                b.name.clone(),
                args.game.chips,
            )));
        }

        let table = build_table(args, SeatsNoCell::new(seats));

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
            awaiting: Awaiting::Bot,
            bet: BetField::default(),
            last_step_at: Instant::now()
                .checked_sub(Duration::from_secs(1))
                .unwrap_or_else(Instant::now),
            speed: Duration::from_millis(600),
            seed,
            last_showdown: None,
        })
    }

    /// Returns the display name of the seat.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::cli::PlayArgs;
    /// use pktui::log_panel::LogPanel;
    /// use pktui::modes::PlayState;
    ///
    /// let mut log = LogPanel::new();
    /// let state = PlayState::new(&PlayArgs::default(), &mut log).unwrap();
    /// assert_eq!(state.seat_name(0), "You");
    /// assert_eq!(state.seat_name(1), state.bots[0].name);
    /// ```
    #[must_use]
    pub fn seat_name(&self, seat: u8) -> String {
        if seat == HERO_SEAT {
            HERO_NAME.to_string()
        } else {
            self.bots
                .get((seat as usize).saturating_sub(1))
                .map_or_else(|| format!("seat {seat}"), |b| b.name.clone())
        }
    }

    /// Returns true if it is currently the hero's turn.
    ///
    /// # Examples
    ///
    /// ```
    /// use pktui::cli::PlayArgs;
    /// use pktui::log_panel::LogPanel;
    /// use pktui::modes::PlayState;
    ///
    /// let mut log = LogPanel::new();
    /// let state = PlayState::new(&PlayArgs::default(), &mut log).unwrap();
    /// // After init it is a bot's turn (UTG, three left of BTN).
    /// assert!(!state.hero_to_act());
    /// ```
    #[must_use]
    pub fn hero_to_act(&self) -> bool {
        matches!(self.awaiting, Awaiting::Human(s) if s == HERO_SEAT)
    }

    /// Drives the engine forward by one [`SessionStep`] and updates
    /// [`Awaiting`] accordingly.
    ///
    /// Bot decisions are evaluated and applied in the same call so the UI
    /// sees one bot per tick rather than ten in a flash. When it becomes the
    /// hero's turn the state flips to `Awaiting::Human(seat)` and stays
    /// there until [`apply_human`](PlayState::apply_human) is called.
    ///
    /// Returns `Ok(true)` if a bot acted, `Ok(false)` otherwise (so the
    /// caller can decide whether to redraw immediately).
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Engine`] on any pkcore failure.
    pub fn tick(&mut self, log: &mut LogPanel) -> Result<bool> {
        if !matches!(self.awaiting, Awaiting::Bot) {
            return Ok(false);
        }
        if self.last_step_at.elapsed() < self.speed {
            return Ok(false);
        }

        match self.session.next_step() {
            SessionStep::PlayerToAct(seat) => {
                if seat == HERO_SEAT {
                    self.awaiting = Awaiting::Human(seat);
                    self.bet = BetField::default();
                    log.push(Severity::Info, format!("Your turn (seat {seat})"));
                    Ok(false)
                } else {
                    let profile_idx = (seat as usize).saturating_sub(1);
                    let action =
                        self.bots[profile_idx].decide(&self.session.table, seat, &mut self.rng);
                    let desc = describe_action(&self.session.table, seat, action);
                    self.session.apply_action(seat, action)?;
                    log.push(
                        severity_for(action),
                        format!("{}: {desc}", self.seat_name(seat)),
                    );
                    self.last_step_at = Instant::now();
                    Ok(true)
                }
            }
            SessionStep::StreetAdvanced => {
                let board = self.session.table.board.to_string();
                log.push(Severity::Info, format!("Board: {board}"));
                self.last_step_at = Instant::now();
                Ok(true)
            }
            SessionStep::HandComplete => {
                // Capture the showdown reveal BEFORE end_hand() resets the
                // table and zeros every seat's hole cards. Only meaningful
                // when 2+ seats are still in the hand — a single-seat win
                // (everyone else folded) doesn't show cards.
                let n = seat_count(&self.session.table);
                let showdown = capture_showdown(&self.session.table, n, |s| self.seat_name(s));
                if let Some(rows) = &showdown {
                    for row in rows {
                        let suffix = match &row.hand_class {
                            Some(c) => format!(" — {c}"),
                            None => String::new(),
                        };
                        log.push(
                            Severity::Info,
                            format!("Showdown: {} shows [{}]{suffix}", row.name, row.hole),
                        );
                    }
                }
                self.last_showdown = showdown;

                let winnings = self.session.end_hand()?;
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
                if self.session.count_funded() < 2
                    || self
                        .session
                        .table
                        .seats
                        .get_seat(HERO_SEAT)
                        .is_none_or(|s| s.is_empty() || s.player.chips == 0)
                {
                    self.awaiting = Awaiting::SessionOver;
                    log.push(Severity::Info, "Session over. Press q to quit.".to_string());
                } else {
                    self.awaiting = Awaiting::HandComplete;
                    log.push(Severity::Info, "Press Enter for next hand.".to_string());
                }
                Ok(true)
            }
        }
    }

    /// Writes a YAML snapshot of the current session state to the working
    /// directory. Filename: `pktui-dump-<seed>-<phase>-<unix_secs>.yaml`.
    /// Returns the path written, or an error if I/O fails.
    ///
    /// The dump captures table state, every seat's cards + per-card
    /// visibility, recent log lines, and `awaiting` — enough for a developer
    /// to reproduce a stuck-state bug from a single file.
    ///
    /// # Errors
    ///
    /// Returns `std::io::Error` if the file cannot be created or written.
    pub fn dump_state(&self, log: &LogPanel) -> std::io::Result<std::path::PathBuf> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let phase = format!("{:?}", self.session.table.phase);
        let name = format!("pktui-dump-{}-{}-{unix}.yaml", self.seed, phase);
        let path = std::path::PathBuf::from(&name);
        let yaml = self.render_state_yaml(log);
        std::fs::write(&path, yaml)?;
        Ok(path)
    }

    fn render_state_yaml(&self, log: &LogPanel) -> String {
        use std::fmt::Write;
        let t = &self.session.table;
        let mut s = String::with_capacity(4096);
        let _ = writeln!(s, "pktui_dump:");
        let _ = writeln!(s, "  seed: {}", self.seed);
        let _ = writeln!(s, "  hand_number: {}", self.session.hand_number);
        let _ = writeln!(s, "  phase: {:?}", t.phase);
        let _ = writeln!(s, "  pot: {}", t.pot);
        let _ = writeln!(s, "  bet: {}", t.bet);
        let _ = writeln!(s, "  button: {}", t.button);
        let _ = writeln!(s, "  raises_this_street: {}", t.raises_this_street);
        let _ = writeln!(s, "  awaiting: {:?}", self.awaiting);
        let _ = writeln!(s, "  forced:");
        let _ = writeln!(s, "    small_blind: {}", t.forced.small_blind);
        let _ = writeln!(s, "    big_blind: {}", t.forced.big_blind);
        let _ = writeln!(s, "    ante: {}", t.forced.ante);
        let _ = writeln!(s, "    bring_in: {}", t.forced.bring_in);
        let _ = writeln!(s, "  betting: {:?}", t.betting);
        let _ = writeln!(s, "  board: \"{}\"", t.board);
        let _ = writeln!(s, "  seats:");
        for (i, seat) in t.seats.0.iter().enumerate() {
            if seat.is_empty() {
                continue;
            }
            let cards_str = seat
                .cards
                .as_slice()
                .iter()
                .filter(|c| **c != pkcore::card::Card::BLANK)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ");
            let _ = writeln!(s, "    - seat: {i}");
            let _ = writeln!(s, "      name: \"{}\"", self.seat_name(i as u8));
            let _ = writeln!(s, "      chips: {}", seat.player.chips);
            let _ = writeln!(s, "      bet: {}", seat.player.bet);
            let _ = writeln!(s, "      in_hand: {}", seat.is_in_hand());
            let _ = writeln!(
                s,
                "      cards_dealt: {}",
                seat.cards.number_of_dealt_cards()
            );
            let _ = writeln!(s, "      cards: \"{cards_str}\"");
            let _ = writeln!(s, "      hand_len: {}", seat.hand.len());
            let _ = writeln!(s, "      hand:");
            for hc in seat.hand.iter() {
                let vis = if hc.is_up() { "Up" } else { "Down" };
                let _ = writeln!(s, "        - {{ card: \"{}\", visibility: {vis} }}", hc.card());
            }
        }
        let _ = writeln!(s, "  recent_log:");
        let lines: Vec<&str> = log.iter().map(|l| l.text.as_str()).collect();
        let start = lines.len().saturating_sub(40);
        for line in &lines[start..] {
            let escaped = line.replace('"', "\\\"");
            let _ = writeln!(s, "    - \"{escaped}\"");
        }
        s
    }

    /// Starts the next hand after [`Awaiting::HandComplete`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Engine`] if the engine fails to start a new
    /// hand (extremely rare — usually means no funded seats).
    pub fn next_hand(&mut self, log: &mut LogPanel) -> Result<()> {
        if !matches!(self.awaiting, Awaiting::HandComplete) {
            return Ok(());
        }
        self.last_showdown = None;
        self.session.start_hand()?;
        log.push(
            Severity::Info,
            format!("Hand {} dealt", self.session.hand_number),
        );
        self.awaiting = Awaiting::Bot;
        self.last_step_at = instant_minus(self.speed);
        Ok(())
    }

    /// Applies a human action and flips state back to [`Awaiting::Bot`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Engine`] if pkcore rejects the action (for
    /// example, an under-min raise). The state stays at `Awaiting::Human` so
    /// the user can try again.
    pub fn apply_human(&mut self, action: PlayerAction, log: &mut LogPanel) -> Result<()> {
        let Awaiting::Human(seat) = self.awaiting else {
            return Ok(());
        };
        let desc = describe_action(&self.session.table, seat, action);
        self.session.apply_action(seat, action)?;
        log.push(severity_for(action), format!("{HERO_NAME}: {desc}"));
        self.awaiting = Awaiting::Bot;
        self.last_step_at = instant_minus(self.speed);
        Ok(())
    }
}

/// `Instant::now().checked_sub(d).unwrap_or_else(Instant::now)` — extracted
/// so we can reuse it without retyping the clippy-pedantic dance.
#[must_use]
fn instant_minus(d: Duration) -> Instant {
    Instant::now().checked_sub(d).unwrap_or_else(Instant::now)
}

/// Casts the seat count (always ≤ 9 in NLHE) to `u8`.
///
/// Wraps the cast in [`u8::try_from`] so the pedantic
/// `cast_possible_truncation` lint stays quiet; the `unwrap_or(u8::MAX)`
/// fallback would only trigger on a >255-seat table, which pkcore does not
/// support.
#[must_use]
fn seat_count(table: &TableNoCell) -> u8 {
    u8::try_from(table.seats.0.len()).unwrap_or(u8::MAX)
}

/// Builds a one-line description of `action` for the log.
///
/// The function reads the table to compute call amounts so messages like
/// `"calls 200"` stay accurate even when [`PlayerAction::Call`] doesn't carry
/// the chip amount.
///
/// # Examples
///
/// ```
/// use pkcore::casino::action::PlayerAction;
/// use pkcore::casino::game::ForcedBets;
/// use pkcore::casino::table_no_cell::{PlayerNoCell, SeatNoCell, SeatsNoCell, TableNoCell};
/// use pktui::modes::play::describe_action;
///
/// let seats = SeatsNoCell::new(vec![
///     SeatNoCell::new(PlayerNoCell::new_with_chips("A".into(), 1000)),
///     SeatNoCell::new(PlayerNoCell::new_with_chips("B".into(), 1000)),
/// ]);
/// let table = TableNoCell::nlh_from_seats(seats, ForcedBets::new(10, 20));
/// assert_eq!(describe_action(&table, 0, PlayerAction::Fold), "folds");
/// assert_eq!(describe_action(&table, 0, PlayerAction::Bet(50)), "bets 50");
/// ```
#[must_use]
pub fn describe_action(table: &TableNoCell, seat: u8, action: PlayerAction) -> String {
    match action {
        PlayerAction::Fold => "folds".into(),
        PlayerAction::Check => "checks".into(),
        PlayerAction::Call if table.to_call(seat) == 0 => "checks".into(),
        PlayerAction::Call => format!("calls {}", table.to_call(seat)),
        PlayerAction::Bet(n) => format!("bets {n}"),
        PlayerAction::Raise(n) => format!("raises to {n}"),
        PlayerAction::AllIn => {
            let chips = table.seats.get_seat(seat).map_or(0, |s| s.player.chips);
            format!("ALL-IN ({chips})")
        }
    }
}

fn severity_for(action: PlayerAction) -> Severity {
    match action {
        PlayerAction::Fold => Severity::Fold,
        PlayerAction::Check | PlayerAction::Call => Severity::Info,
        PlayerAction::Bet(_) | PlayerAction::Raise(_) | PlayerAction::AllIn => Severity::Action,
    }
}

/// Snapshots every active (non-folded) seat's hole cards just before
/// [`PokerSession::end_hand`](pkcore::casino::session::PokerSession::end_hand)
/// would clear them.
///
/// Returns `None` when fewer than two players are still active — in that case
/// nobody is required to show, so we don't reveal anything.
///
/// When the board is complete (5 community cards), each row also carries the
/// 5-card hand class derived from `Seven::hand_rank_and_hand`.
#[must_use]
pub fn capture_showdown<F: Fn(u8) -> String>(
    table: &TableNoCell,
    n_seats: u8,
    name_of: F,
) -> Option<Vec<ShowdownSeat>> {
    let mut active: Vec<ShowdownSeat> = (0..n_seats)
        .filter_map(|i| {
            let s = table.seats.get_seat(i)?;
            if s.is_empty() || !s.player.is_in_hand() || !s.cards.has_cards() {
                return None;
            }
            let hole = s.cards.sorted_display();
            Some(ShowdownSeat {
                seat: i,
                name: name_of(i),
                hole,
                hand_class: None,
            })
        })
        .collect();

    if active.len() < 2 {
        return None;
    }

    let board = table.board.to_string();
    if board.split_whitespace().count() == 5 {
        for row in &mut active {
            row.hand_class = hand_class(&row.hole, &board);
        }
    }

    Some(active)
}

fn hand_class(hole: &str, board: &str) -> Option<String> {
    let seven = Seven::from_str(&format!("{hole} {board}")).ok()?;
    let (hand_rank, hand) = seven.hand_rank_and_hand();
    let eval = Eval::new(hand_rank, hand);
    Some(format!("{:?}", eval.hand_rank.class))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{PlayArgs, Variant};

    fn play_with_seed(seed: u64) -> PlayState {
        let mut log = LogPanel::new();
        let mut args = PlayArgs::default();
        args.game.seed = Some(seed);
        PlayState::new(&args, &mut log).unwrap()
    }

    #[test]
    fn new_seats_nine_players() {
        let s = play_with_seed(1);
        assert_eq!(s.bots.len(), 8);
        assert_eq!(s.session.table.seats.0.len(), 9);
    }

    #[test]
    fn new_stud_hi_seats_eight_players_and_logs_warning() {
        let mut log = LogPanel::new();
        let mut args = PlayArgs::default();
        args.game.seed = Some(7);
        args.game.variant = Variant::StudHi;
        let s = PlayState::new(&args, &mut log).unwrap();
        assert_eq!(s.bots.len(), 7);
        assert_eq!(s.session.table.seats.0.len(), 8);
        let logged: String = log
            .iter()
            .map(|line| line.text.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(logged.contains("Stud Hi"), "log was: {logged}");
        assert!(logged.contains("preliminary"), "log was: {logged}");
    }

    #[test]
    fn deterministic_bots_for_same_seed() {
        let a = play_with_seed(99)
            .bots
            .iter()
            .map(|b| b.name.clone())
            .collect::<Vec<_>>();
        let b = play_with_seed(99)
            .bots
            .iter()
            .map(|b| b.name.clone())
            .collect::<Vec<_>>();
        assert_eq!(a, b);
    }

    #[test]
    fn seat_name_hero_and_bots() {
        let s = play_with_seed(2);
        assert_eq!(s.seat_name(0), "You");
        assert_eq!(s.seat_name(1), s.bots[0].name);
        assert_eq!(s.seat_name(8), s.bots[7].name);
    }

    #[test]
    fn bet_field_arithmetic() {
        let mut b = BetField::default();
        b.bump(100);
        b.bump(50);
        assert_eq!(b.amount(), 150);
        b.cut(75);
        assert_eq!(b.amount(), 75);
    }

    #[test]
    fn bet_field_digit_entry() {
        let mut b = BetField::default();
        for d in [1, 2, 3, 4] {
            b.push_digit(d);
        }
        assert_eq!(b.amount(), 1234);
        b.pop_digit();
        assert_eq!(b.amount(), 123);
    }

    #[test]
    fn describe_action_variants() {
        let seats = SeatsNoCell::new(vec![
            SeatNoCell::new(PlayerNoCell::new_with_chips("A".into(), 1000)),
            SeatNoCell::new(PlayerNoCell::new_with_chips("B".into(), 1000)),
        ]);
        let table = TableNoCell::nlh_from_seats(seats, ForcedBets::new(10, 20));
        assert_eq!(describe_action(&table, 0, PlayerAction::Fold), "folds");
        assert_eq!(describe_action(&table, 0, PlayerAction::Bet(50)), "bets 50");
        assert_eq!(
            describe_action(&table, 0, PlayerAction::Raise(80)),
            "raises to 80"
        );
        assert!(describe_action(&table, 0, PlayerAction::AllIn).starts_with("ALL-IN"));
    }

    #[test]
    fn tick_advances_until_hero() {
        let mut s = play_with_seed(7);
        let mut log = LogPanel::new();
        s.speed = Duration::from_millis(0);
        for _ in 0..200 {
            if matches!(
                s.awaiting,
                Awaiting::Human(_) | Awaiting::HandComplete | Awaiting::SessionOver
            ) {
                break;
            }
            let _ = s.tick(&mut log);
        }
        // Either the hero must act, or the bots all folded preflop and the
        // hand completed without the hero seeing a turn.
        assert!(matches!(
            s.awaiting,
            Awaiting::Human(_) | Awaiting::HandComplete | Awaiting::SessionOver
        ));
    }
}
