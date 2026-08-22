# Update Log

## 2026-07-22
* **Creation**: Scaffolded the bundle with `okf_init.py`.
* **Creation**: Rewrote [getting started](/getting-started.md) as the crate overview.
* **Creation**: Added the [architecture](/architecture/index.md) group — Elm loop, App model, update reducer, event loop, error handling.
* **Creation**: Added the [modes](/modes/index.md) group — Play, Arena, Replay, Spectate.
* **Creation**: Added the [UI](/ui/index.md) group — render dispatch, table view, action bar, odds cache, log panel, replay view, help overlay.
* **Creation**: Added the [configuration](/config/index.md) group — CLI and user config.
* **Creation**: Added the [decisions](/decisions/index.md) group — per-street Win% display, Spectate `D` dump.
* **Update**: Rebuilt the root [index](/index.md) as a progressive-disclosure map of all groups.

## 2026-08-21
* **Update**: Bumped pkcore `0.5.0` → `0.6.0`. Recorded the new `SessionStep::Failed` / `abort_hand` path in [Play](/modes/play.md) and [Arena](/modes/arena.md).
* **Update**: Raised the stud seat cap from 6 to 8 (`Table::MAX_STUD_SEATS`) in [getting started](/getting-started.md) and the [CLI](/config/cli.md), now that pkcore deals a 7th-street community card when the stub runs short.

## 2026-08-22
* **Update**: Bumped pkcore `0.6.0` → `0.7.0`. No pktui source change: the release's breaking signatures (`next_actor`, `Deck::get`, `KuhnCfr::train`, `Terminal::receive_usize`, `HUPResult::from_sorted_heads_up`) are all on APIs pktui does not call. Dropped the pinned engine version from the stud seat-cap note in [getting started](/getting-started.md) so it stops drifting.
