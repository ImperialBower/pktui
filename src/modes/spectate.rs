//! Spectate mode: a read-only viewer of a live `pkdealer_service` table.
//!
//! Unlike Play/Arena/Replay, this mode owns no `pkcore` engine. A background
//! OS thread holds the gRPC stream and forwards [`SpectateMsg`]s through a
//! channel; [`SpectateState::drain`] applies them to the latest snapshot.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use pkdealer_proto::dealer::dealer_service_client::DealerServiceClient;
use pkdealer_proto::dealer::{
    EventType, GetTableConfigRequest, StreamEventsRequest, TableConfig, TableEvent, TableStatus,
};

use crate::error::{Error, Result};
use crate::log_panel::{LogPanel, Severity};

/// Default dealer endpoint, matching the gRPC port exposed by the demo stack.
pub const DEFAULT_ENDPOINT: &str = "http://localhost:50051";

/// Default spectator token, matching the dealer's `DEFAULT_SPECTATOR_TOKEN`.
/// Sent as the stream's `player_token` so the dealer reveals every seat's
/// hole cards (full-table visibility), as opposed to an empty token which
/// redacts them.
const DEFAULT_SPECTATOR_TOKEN: &str = "spectator";

/// Environment variable overriding [`DEFAULT_SPECTATOR_TOKEN`]; named to match
/// the dealer's own `PKDEALER_SPECTATOR_TOKEN`, so a custom token only has to
/// be set once and both sides agree.
const SPECTATOR_TOKEN_ENV: &str = "PKDEALER_SPECTATOR_TOKEN";

/// Resolves the spectator token from the environment, falling back to the
/// default the dealer also ships with.
fn spectator_token() -> String {
    std::env::var(SPECTATOR_TOKEN_ENV).unwrap_or_else(|_| DEFAULT_SPECTATOR_TOKEN.to_string())
}

/// Connection lifecycle of the background gRPC stream.
///
/// # Examples
///
/// ```
/// use pktui::modes::spectate::ConnState;
/// assert_eq!(ConnState::default(), ConnState::Connecting);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnState {
    /// Attempting to (re)connect to the dealer.
    #[default]
    Connecting,
    /// Stream is live.
    Connected,
    /// Stream dropped or the dealer is unreachable; a retry is scheduled.
    Disconnected,
}

impl ConnState {
    /// Short label for the header / status line.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Connecting => "connecting…",
            Self::Connected => "connected",
            Self::Disconnected => "disconnected — retrying",
        }
    }
}

/// A message produced by the background thread and consumed by the UI loop.
///
/// Network events are deliberately NOT routed through [`crate::update::Msg`]
/// (which is `Copy`); a `TableEvent` is large and owns heap data.
///
/// # Examples
///
/// ```
/// use pktui::modes::spectate::{ConnState, SpectateMsg};
/// let msg = SpectateMsg::Conn(ConnState::Connected);
/// assert!(matches!(msg, SpectateMsg::Conn(_)));
/// ```
pub enum SpectateMsg {
    /// A table event carrying a full status snapshot + a description line.
    Event(Box<TableEvent>),
    /// Static table configuration, fetched once after connecting.
    Config(Box<TableConfig>),
    /// A connection-state transition.
    Conn(ConnState),
}

/// All state for the read-only spectator view.
pub struct SpectateState {
    /// The dealer endpoint we are watching (for display).
    pub endpoint: String,
    /// Latest table snapshot, or `None` until the first event arrives.
    pub status: Option<TableStatus>,
    /// Static table config (blinds / variant), best-effort.
    pub config: Option<TableConfig>,
    /// Current connection lifecycle state.
    pub conn: ConnState,
    /// When true, incoming snapshots are dropped (display freezes).
    pub paused: bool,
    /// Snapshots of every hand that has ended this session, oldest first.
    /// Grows unbounded; `D` dumps the whole history.
    pub completed_hands: Vec<TableStatus>,
    /// Per-street odds cache for the Win% column.
    pub odds: crate::ui::odds::OddsCache,
    /// Per-seat hole cards seen this hand, keyed by seat number. Populated from
    /// every snapshot that still reveals a seat's cards and reset at hand start,
    /// so the action log can show a player's hand even after they fold (pkcore
    /// mucks folded hands, blanking them in later snapshots).
    hand_cards: std::collections::HashMap<u32, String>,
    /// Receiver drained each UI tick.
    rx: Receiver<SpectateMsg>,
    /// Kept alive so the worker thread is not detached prematurely.
    _handle: Option<JoinHandle<()>>,
}

impl SpectateState {
    /// Connects to `endpoint` and starts the background streaming thread.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Io`] if the worker thread cannot be spawned.
    /// Connection failures themselves are NOT errors here — they surface as
    /// [`ConnState::Disconnected`] through the channel so the UI keeps running.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pktui::modes::spectate::SpectateState;
    /// let state = SpectateState::new("http://localhost:50051".to_string());
    /// assert!(state.is_ok());
    /// ```
    pub fn new(endpoint: String) -> Result<Self> {
        let (tx, rx) = mpsc::channel();
        let ep = endpoint.clone();
        let handle = thread::Builder::new()
            .name("pktui-spectate".to_string())
            .spawn(move || run_stream(&ep, &tx))
            .map_err(Error::Io)?;
        Ok(Self {
            endpoint,
            status: None,
            config: None,
            conn: ConnState::Connecting,
            paused: false,
            completed_hands: Vec::new(),
            odds: crate::ui::odds::OddsCache::new(),
            hand_cards: std::collections::HashMap::new(),
            rx,
            _handle: Some(handle),
        })
    }

    /// Drains all pending channel messages, applying each to `self`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pktui::modes::spectate::SpectateState;
    /// use pktui::log_panel::LogPanel;
    /// let mut state = SpectateState::new("http://localhost:50051".to_string()).unwrap();
    /// let mut log = LogPanel::new();
    /// state.drain(&mut log);
    /// ```
    pub fn drain(&mut self, log: &mut LogPanel) {
        while let Ok(msg) = self.rx.try_recv() {
            self.apply(msg, log);
        }
    }

    /// Applies a single [`SpectateMsg`]. Pure with respect to the network —
    /// unit-tested directly via `SpectateState::detached`.
    fn apply(&mut self, msg: SpectateMsg, log: &mut LogPanel) {
        match msg {
            SpectateMsg::Event(ev) => {
                if self.paused {
                    return;
                }
                let ev = *ev;
                let event_type = EventType::try_from(ev.event_type);
                let is_hand_end = event_type == Ok(EventType::HandEnded);
                // A new hand resets the per-seat card memory before we record
                // its fresh deal from the hand-start snapshot below.
                if event_type == Ok(EventType::HandStarted) {
                    self.hand_cards.clear();
                }
                if let Some(status) = ev.current_status {
                    self.status = Some(status);
                }
                // Remember each still-revealed seat's cards. pkcore blanks a
                // folded seat's cards in later snapshots, so we only ever store
                // (never overwrite with) revealed hands — the pre-fold cards
                // survive for the action log to show.
                if let Some(status) = self.status.as_ref() {
                    for seat in &status.seats {
                        if is_revealed_cards(&seat.cards) {
                            self.hand_cards
                                .insert(seat.seat_number, crate::ui::sort_hole_cards(&seat.cards));
                        }
                    }
                }
                // On hand end, archive the end-of-hand snapshot — preferring
                // the event's own status, falling back to the latest known.
                if is_hand_end && let Some(snapshot) = self.status.clone() {
                    self.completed_hands.push(snapshot);
                }
                if !ev.description.is_empty() {
                    // For player-action lines, append the chip amount (for
                    // betting actions) and the acting seat's hole cards from the
                    // remembered hand — so the log shows how much each player
                    // committed and what they held, even after a fold (when
                    // pkcore has already mucked the cards in the live snapshot).
                    let line = if event_type == Ok(EventType::PlayerAction) {
                        augment_action_line(&ev.description, self.status.as_ref(), &self.hand_cards)
                    } else {
                        ev.description
                    };
                    log.push(severity_for(ev.event_type), line);
                }
            }
            SpectateMsg::Config(cfg) => {
                self.config = Some(*cfg);
            }
            SpectateMsg::Conn(state) => {
                if self.conn != state {
                    log.push(
                        Severity::Info,
                        format!("{} ({})", state.label(), self.endpoint),
                    );
                }
                self.conn = state;
            }
        }
    }

    /// Writes a YAML snapshot of the current spectator view to the working
    /// directory. Filename: `pktui-spectate-dump-<street>-<unix_secs>.yaml`,
    /// where `<street>` is the latest snapshot's `current_street` (or
    /// `nostatus` before the first event arrives).
    ///
    /// The file has two sections: a Play-style `pktui_spectate_dump:` summary
    /// built from the proto [`TableStatus`], and a `raw_proto:` block holding
    /// the verbatim `{:#?}` debug rendering of the status and config — enough
    /// for a developer to reproduce a streaming bug from a single file.
    ///
    /// # Errors
    ///
    /// Returns `std::io::Error` if the file cannot be created or written.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pktui::modes::spectate::SpectateState;
    /// use pktui::log_panel::LogPanel;
    /// let state = SpectateState::new("http://localhost:50051".to_string()).unwrap();
    /// let path = state.dump_state(&LogPanel::new()).unwrap();
    /// println!("wrote {}", path.display());
    /// ```
    pub fn dump_state(&self, log: &LogPanel) -> std::io::Result<std::path::PathBuf> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let street = self.status.as_ref().map_or_else(
            || "nostatus".to_string(),
            |s| format!("s{}", s.current_street),
        );
        let name = format!("pktui-spectate-dump-{street}-{unix}.yaml");
        let path = std::path::PathBuf::from(&name);
        std::fs::write(&path, self.render_dump_yaml(log))?;
        Ok(path)
    }

    /// Builds the YAML body for [`Self::dump_state`].
    fn render_dump_yaml(&self, log: &LogPanel) -> String {
        use std::fmt::Write;
        let mut s = String::with_capacity(4096);
        let _ = writeln!(s, "pktui_spectate_dump:");
        let _ = writeln!(s, "  endpoint: \"{}\"", self.endpoint);
        let _ = writeln!(s, "  conn: {}", self.conn.label());
        let _ = writeln!(s, "  paused: {}", self.paused);
        let _ = writeln!(s, "  completed_hands_count: {}", self.completed_hands.len());
        match &self.status {
            None => {
                let _ = writeln!(s, "  status: none");
            }
            Some(t) => Self::write_status_summary(&mut s, t, "  "),
        }
        let _ = writeln!(s, "  recent_log:");
        let lines: Vec<&str> = log.iter().map(|l| l.text.as_str()).collect();
        let start = lines.len().saturating_sub(40);
        for line in &lines[start..] {
            let escaped = line.replace('"', "\\\"");
            let _ = writeln!(s, "    - \"{escaped}\"");
        }

        // Every hand that has ended this session, oldest first. Each entry is
        // a list item anchored by `index`; the rest of the fields are siblings
        // emitted by the shared summary helper at the list-item indent.
        let _ = writeln!(s, "completed_hands:");
        for (i, hand) in self.completed_hands.iter().enumerate() {
            let _ = writeln!(s, "  - index: {i}");
            Self::write_status_summary(&mut s, hand, "    ");
        }

        // Verbatim proto, indented as a YAML block scalar so the file stays
        // valid YAML regardless of the debug output's punctuation.
        let _ = writeln!(s, "raw_proto: |");
        let raw = format!("status: {:#?}\nconfig: {:#?}", self.status, self.config);
        for line in raw.lines() {
            let _ = writeln!(s, "  {line}");
        }
        s
    }

    /// Writes a [`TableStatus`] summary (scalar fields + seats) into `s`, with
    /// every key prefixed by `indent`. Shared by the live snapshot and each
    /// archived completed hand so both render identically.
    fn write_status_summary(s: &mut String, t: &TableStatus, indent: &str) {
        use std::fmt::Write;
        let _ = writeln!(s, "{indent}pot: {}", t.pot);
        let _ = writeln!(s, "{indent}board: \"{}\"", t.board);
        let _ = writeln!(s, "{indent}small_blind: {}", t.small_blind);
        let _ = writeln!(s, "{indent}big_blind: {}", t.big_blind);
        let _ = writeln!(s, "{indent}current_street: {}", t.current_street);
        let _ = writeln!(s, "{indent}hand_in_progress: {}", t.hand_in_progress);
        let _ = writeln!(s, "{indent}game_over: {}", t.game_over);
        let _ = writeln!(s, "{indent}next_to_act: {}", t.next_to_act);
        let _ = writeln!(s, "{indent}seats:");
        for seat in &t.seats {
            let _ = writeln!(s, "{indent}  - seat: {}", seat.seat_number);
            let _ = writeln!(s, "{indent}    name: \"{}\"", seat.player_name);
            let _ = writeln!(s, "{indent}    chips: {}", seat.chips);
            let _ = writeln!(s, "{indent}    bet: {}", seat.bet);
            let _ = writeln!(s, "{indent}    cards: \"{}\"", seat.cards);
            let _ = writeln!(s, "{indent}    state: {}", seat.state);
            let _ = writeln!(s, "{indent}    profit_loss: {}", seat.profit_loss);
        }
    }

    /// Test-only constructor: builds a detached state with no worker thread,
    /// returning the channel sender so tests can inject messages.
    #[cfg(test)]
    pub(crate) fn detached(endpoint: &str) -> (Self, Sender<SpectateMsg>) {
        let (tx, rx) = mpsc::channel();
        (
            Self {
                endpoint: endpoint.to_string(),
                status: None,
                config: None,
                conn: ConnState::Connecting,
                paused: false,
                completed_hands: Vec::new(),
                odds: crate::ui::odds::OddsCache::new(),
                hand_cards: std::collections::HashMap::new(),
                rx,
                _handle: None,
            },
            tx,
        )
    }
}

/// Enriches a player-action log line with the chip amount and the acting
/// seat's hole cards: `"Seat N name: Raise"` → `"Seat N name: Raise 600 [A♠ K♦]"`.
///
/// `desc` is the dealer's pre-formatted line; the seat index is parsed from its
/// `"Seat N"` prefix. The amount is the acting seat's per-street `bet` from
/// `status`, appended only for betting verbs (bet / call / raise / all-in) —
/// never for check / fold. Cards come from `hand_cards` (the remembered
/// pre-fold hand). Returns `desc` unchanged when the seat can't be parsed, so a
/// malformed line is never corrupted.
fn augment_action_line(
    desc: &str,
    status: Option<&TableStatus>,
    hand_cards: &std::collections::HashMap<u32, String>,
) -> String {
    let Some(seat) = parse_seat_prefix(desc) else {
        return desc.to_string();
    };
    let mut line = desc.to_string();
    if action_has_amount(desc)
        && let Some(status) = status
        && let Some(info) = status.seats.iter().find(|s| s.seat_number == seat)
        && info.bet > 0
    {
        line = format!("{line} {}", info.bet);
    }
    if let Some(cards) = hand_cards.get(&seat).filter(|c| !c.is_empty()) {
        line = format!("{line} [{cards}]");
    }
    line
}

/// Whether the action verb ending a player-action line carries a chip amount
/// (`Bet` / `Call` / `Raise` / `AllIn`) versus none (`Check` / `Fold`). The
/// verb is the token after the final `:` — the prost `Debug` of the proto
/// `ActionType`, e.g. `"Seat 3 gto_1: Raise"` → `"Raise"`.
fn action_has_amount(desc: &str) -> bool {
    matches!(
        desc.rsplit(':').next().map(str::trim),
        Some("Bet" | "Call" | "Raise" | "AllIn")
    )
}

/// Whether a proto `cards` string holds real, revealed hole cards — i.e. not
/// empty, not the `??` muck marker, and not a blanked (`__`) mucked hand.
fn is_revealed_cards(cards: &str) -> bool {
    !cards.is_empty() && cards != "??" && !cards.contains('_')
}

/// Parses the seat index from a `"Seat N…"` description prefix, or `None` if
/// the line does not start with `"Seat "` followed by digits.
fn parse_seat_prefix(desc: &str) -> Option<u32> {
    let rest = desc.strip_prefix("Seat ")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Maps a proto `EventType` discriminant to a log [`Severity`].
fn severity_for(event_type: i32) -> Severity {
    match EventType::try_from(event_type) {
        Ok(EventType::PlayerAction) => Severity::Action,
        Ok(EventType::HandEnded) => Severity::Win,
        _ => Severity::Info,
    }
}

/// Worker-thread entry point: owns a current-thread tokio runtime and the
/// reconnect loop. Exits when the receiver is dropped (UI quit).
fn run_stream(endpoint: &str, tx: &Sender<SpectateMsg>) {
    let Ok(rt) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        let _ = tx.send(SpectateMsg::Conn(ConnState::Disconnected));
        return;
    };
    rt.block_on(async {
        loop {
            let _ = connect_and_stream(endpoint, tx).await;
            // Either a connect failure or a clean stream end: signal and retry.
            if tx.send(SpectateMsg::Conn(ConnState::Disconnected)).is_err() {
                break; // receiver dropped → UI quit → stop the thread
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            if tx.send(SpectateMsg::Conn(ConnState::Connecting)).is_err() {
                break;
            }
        }
    });
}

/// Connects, fetches config once, then forwards every streamed event.
/// `Err(())` means either a transport failure or that the receiver is gone.
async fn connect_and_stream(
    endpoint: &str,
    tx: &Sender<SpectateMsg>,
) -> std::result::Result<(), ()> {
    let mut client = DealerServiceClient::connect(endpoint.to_string())
        .await
        .map_err(|_| ())?;
    tx.send(SpectateMsg::Conn(ConnState::Connected))
        .map_err(|_| ())?;

    if let Ok(resp) = client.get_table_config(GetTableConfigRequest {}).await
        && let Some(cfg) = resp.into_inner().config
    {
        tx.send(SpectateMsg::Config(Box::new(cfg)))
            .map_err(|_| ())?;
    }

    let mut stream = client
        .stream_events(StreamEventsRequest {
            player_token: spectator_token(),
        })
        .await
        .map_err(|_| ())?
        .into_inner();

    while let Some(ev) = stream.message().await.map_err(|_| ())? {
        tx.send(SpectateMsg::Event(Box::new(ev))).map_err(|_| ())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log_panel::LogPanel;
    use pkdealer_proto::dealer::{SeatInfo, TableEvent, TableStatus};

    fn sample_status() -> TableStatus {
        TableStatus {
            seats: vec![SeatInfo {
                seat_number: 0,
                player_name: "gto".into(),
                chips: 9_500,
                cards: "??".into(),
                state: 4, // CALLED
                withdrawn: 10_000,
                chips_in_play: 500,
                profit_loss: -500,
                bet: 500,
                ..Default::default()
            }],
            board: "Ah Kd Qc".into(),
            pot: 1_000,
            next_to_act: 0,
            hand_in_progress: true,
            game_over: false,
            current_street: 2, // FLOP
            small_blind: 50,
            big_blind: 100,
            ..Default::default()
        }
    }

    #[test]
    fn apply_event_swaps_snapshot_and_logs() {
        let (mut state, _tx) = SpectateState::detached("http://localhost:50051");
        let mut log = LogPanel::new();
        let ev = TableEvent {
            timestamp: 1,
            event_type: 4, // PLAYER_ACTION
            description: "gto calls 500".into(),
            current_status: Some(sample_status()),
        };
        state.apply(SpectateMsg::Event(Box::new(ev)), &mut log);
        assert_eq!(state.status.as_ref().unwrap().pot, 1_000);
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn parse_seat_prefix_reads_index_or_none() {
        assert_eq!(parse_seat_prefix("Seat 0 gto_1: Fold"), Some(0));
        assert_eq!(parse_seat_prefix("Seat 12 lag: Raise"), Some(12));
        assert_eq!(parse_seat_prefix("Hand ended. tag wins 2850"), None);
        assert_eq!(parse_seat_prefix("Seat: malformed"), None);
    }

    #[test]
    fn is_revealed_cards_distinguishes_real_from_mucked() {
        assert!(is_revealed_cards("Ah Kd"));
        assert!(is_revealed_cards("J♠ 8♣"));
        assert!(!is_revealed_cards(""));
        assert!(!is_revealed_cards("??"));
        assert!(!is_revealed_cards("__ __")); // pkcore-mucked, blanked hand
    }

    #[test]
    fn augment_action_line_adds_cards_and_amount() {
        let mut cards = std::collections::HashMap::new();
        cards.insert(0u32, "Ah Kd".to_string());
        let mut status = sample_status();
        status.seats[0].bet = 600;

        // Betting verbs get the per-street amount AND the remembered cards.
        assert_eq!(
            augment_action_line("Seat 0 gto_1: Raise", Some(&status), &cards),
            "Seat 0 gto_1: Raise 600 [Ah Kd]"
        );
        assert_eq!(
            augment_action_line("Seat 0 gto_1: Call", Some(&status), &cards),
            "Seat 0 gto_1: Call 600 [Ah Kd]"
        );
        // Check/Fold never get an amount, even when bet > 0.
        assert_eq!(
            augment_action_line("Seat 0 gto_1: Check", Some(&status), &cards),
            "Seat 0 gto_1: Check [Ah Kd]"
        );
        // Seat with no remembered cards / unparseable prefix → unchanged.
        assert_eq!(
            augment_action_line("Seat 7 ghost: Call", Some(&status), &cards),
            "Seat 7 ghost: Call"
        );
        assert_eq!(
            augment_action_line("Hand ended. gto wins 2100", Some(&status), &cards),
            "Hand ended. gto wins 2100"
        );
    }

    #[test]
    fn action_has_amount_only_for_betting_verbs() {
        assert!(action_has_amount("Seat 0 gto_1: Bet"));
        assert!(action_has_amount("Seat 0 gto_1: Call"));
        assert!(action_has_amount("Seat 0 gto_1: Raise"));
        assert!(action_has_amount("Seat 0 gto_1: AllIn"));
        assert!(!action_has_amount("Seat 0 gto_1: Check"));
        assert!(!action_has_amount("Seat 0 gto_1: Fold"));
    }

    #[test]
    fn fold_reveals_cards_remembered_from_earlier_snapshot() {
        // Reproduces the live bug: a seat shows real cards while in the hand,
        // then folds — pkcore blanks the cards in the fold snapshot. The log
        // must still show the hand, recovered from the earlier snapshot.
        let (mut state, _tx) = SpectateState::detached("http://localhost:50051");
        let mut log = LogPanel::new();

        // Hand start: seat 0 holds Ah Kd (full spectator visibility).
        let mut started = sample_status();
        started.seats[0].cards = "Ah Kd".into();
        state.apply(
            SpectateMsg::Event(Box::new(TableEvent {
                timestamp: 1,
                event_type: 3, // HAND_STARTED
                description: "Hand started — blinds 300/600".into(),
                current_status: Some(started),
            })),
            &mut log,
        );

        // Seat 0 folds: pkcore has mucked the cards → "__ __" in this snapshot.
        let mut folded = sample_status();
        folded.seats[0].cards = "__ __".into();
        folded.seats[0].state = 8; // FOLDED
        state.apply(
            SpectateMsg::Event(Box::new(TableEvent {
                timestamp: 2,
                event_type: 4, // PLAYER_ACTION
                description: "Seat 0 gto_1: Fold".into(),
                current_status: Some(folded),
            })),
            &mut log,
        );

        let line = log.iter().last().expect("a log line").text.clone();
        assert_eq!(line, "Seat 0 gto_1: Fold [Ah Kd]");
    }

    #[test]
    fn hand_start_resets_remembered_cards() {
        let (mut state, _tx) = SpectateState::detached("http://localhost:50051");
        let mut log = LogPanel::new();
        let mut h1 = sample_status();
        h1.seats[0].cards = "Ah Kd".into();
        state.apply(
            SpectateMsg::Event(Box::new(TableEvent {
                timestamp: 1,
                event_type: 3, // HAND_STARTED
                description: "Hand started".into(),
                current_status: Some(h1),
            })),
            &mut log,
        );
        assert_eq!(state.hand_cards.get(&0).map(String::as_str), Some("Ah Kd"));

        // A new hand-start with the seat's cards hidden clears the old memory.
        let mut h2 = sample_status();
        h2.seats[0].cards = "??".into();
        state.apply(
            SpectateMsg::Event(Box::new(TableEvent {
                timestamp: 2,
                event_type: 3,
                description: "Hand started".into(),
                current_status: Some(h2),
            })),
            &mut log,
        );
        assert_eq!(state.hand_cards.get(&0), None);
    }

    #[test]
    fn apply_conn_change_updates_state_and_logs_once() {
        let (mut state, _tx) = SpectateState::detached("http://localhost:50051");
        let mut log = LogPanel::new();
        state.apply(SpectateMsg::Conn(ConnState::Connected), &mut log);
        assert_eq!(state.conn, ConnState::Connected);
        assert_eq!(log.len(), 1);
        // Same state again does not re-log.
        state.apply(SpectateMsg::Conn(ConnState::Connected), &mut log);
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn paused_drops_snapshot_updates() {
        let (mut state, _tx) = SpectateState::detached("http://localhost:50051");
        let mut log = LogPanel::new();
        state.paused = true;
        let ev = TableEvent {
            timestamp: 1,
            event_type: 4,
            description: "ignored while paused".into(),
            current_status: Some(sample_status()),
        };
        state.apply(SpectateMsg::Event(Box::new(ev)), &mut log);
        assert!(state.status.is_none());
        assert_eq!(log.len(), 0);
    }

    #[test]
    fn hand_ended_events_accumulate_in_history() {
        let (mut state, _tx) = SpectateState::detached("http://localhost:50051");
        let mut log = LogPanel::new();
        for _ in 0..2 {
            let ev = TableEvent {
                timestamp: 1,
                event_type: EventType::HandEnded as i32,
                description: "hand over".into(),
                current_status: Some(sample_status()),
            };
            state.apply(SpectateMsg::Event(Box::new(ev)), &mut log);
        }
        assert_eq!(state.completed_hands.len(), 2);

        // Assert on the rendered body, not a written file: dump_state's
        // filename is time+street keyed, so two parallel tests sharing a
        // street would race on the same path. render_dump_yaml is pure.
        let body = state.render_dump_yaml(&log);
        assert!(body.contains("completed_hands_count: 2"));
        assert!(body.contains("completed_hands:"));
    }

    #[test]
    fn hand_ended_without_status_falls_back_to_last_snapshot() {
        let (mut state, _tx) = SpectateState::detached("http://localhost:50051");
        let mut log = LogPanel::new();
        // A normal event first establishes a snapshot.
        state.apply(
            SpectateMsg::Event(Box::new(TableEvent {
                timestamp: 1,
                event_type: EventType::PlayerAction as i32,
                description: "gto calls 500".into(),
                current_status: Some(sample_status()),
            })),
            &mut log,
        );
        // HandEnded carrying no status of its own.
        state.apply(
            SpectateMsg::Event(Box::new(TableEvent {
                timestamp: 2,
                event_type: EventType::HandEnded as i32,
                description: "hand over".into(),
                current_status: None,
            })),
            &mut log,
        );
        assert_eq!(state.completed_hands.len(), 1);
        assert_eq!(state.completed_hands[0].pot, 1_000);
    }

    #[test]
    fn non_hand_ended_events_do_not_grow_history() {
        let (mut state, _tx) = SpectateState::detached("http://localhost:50051");
        let mut log = LogPanel::new();
        state.apply(
            SpectateMsg::Event(Box::new(TableEvent {
                timestamp: 1,
                event_type: EventType::PlayerAction as i32,
                description: "gto calls 500".into(),
                current_status: Some(sample_status()),
            })),
            &mut log,
        );
        assert!(state.completed_hands.is_empty());
    }

    #[test]
    fn render_dump_includes_both_sections() {
        let (mut state, _tx) = SpectateState::detached("http://localhost:50051");
        let mut log = LogPanel::new();
        let ev = TableEvent {
            timestamp: 1,
            event_type: 4,
            description: "gto calls 500".into(),
            current_status: Some(sample_status()),
        };
        state.apply(SpectateMsg::Event(Box::new(ev)), &mut log);

        // Pure renderer — no filesystem, so no cross-test path race.
        let body = state.render_dump_yaml(&log);

        assert!(
            body.contains("pktui_spectate_dump:"),
            "missing summary section"
        );
        assert!(body.contains("raw_proto:"), "missing raw proto section");
        assert!(body.contains("pot: 1000"), "summary should carry the pot");
        assert!(body.contains("gto"), "summary should carry the seat name");
    }

    #[test]
    fn dump_state_handles_missing_status() {
        let (state, _tx) = SpectateState::detached("http://localhost:50051");
        let log = LogPanel::new();

        let path = state
            .dump_state(&log)
            .expect("dump should succeed without a snapshot");
        let body = std::fs::read_to_string(&path).expect("dump file should be readable");
        let _ = std::fs::remove_file(&path);

        assert!(body.contains("pktui_spectate_dump:"));
        assert!(
            body.contains("status: none"),
            "should note the absent snapshot"
        );
    }

    #[test]
    fn conn_state_default_is_connecting() {
        assert_eq!(ConnState::default(), ConnState::Connecting);
    }

    #[test]
    fn conn_state_label_variants() {
        assert_eq!(ConnState::Connecting.label(), "connecting…");
        assert_eq!(ConnState::Connected.label(), "connected");
        assert_eq!(ConnState::Disconnected.label(), "disconnected — retrying");
    }
}
