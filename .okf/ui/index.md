# UI / Rendering

Render functions. All read the App immutably; none mutate state.

* [view — render dispatch](rendering.md) - top-level; routes to a per-mode view.
* [Table view (Play / Arena)](table-view.md) - the live 9-seat table.
* [Action bar](action-bar.md) - two-line context-sensitive hotkey hints.
* [OddsCache — per-street win% caching](odds-cache.md) - change-keyed double-dummy equities.
* [LogPanel — rolling event log](log-panel.md) - capped event buffer, auto-scrolled.
* [Replay view](replay-view.md) - renders a saved YAML hand.
* [Help overlay](help-overlay.md) - centered keyboard-shortcut modal.
