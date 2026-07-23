# Architecture

The Elm-style control flow that structures the whole crate.

* [Elm-style Model / Message / Update loop](elm-loop.md) - the core control-flow pattern.
* [App — the central model](app-model.md) - root state; owns mode, log, and flags.
* [Msg and the update reducer](update-reducer.md) - the single mutation point.
* [Event polling and the tick timer](event-loop.md) - crossterm events + bot pacing.
* [Crate-wide error type](error-handling.md) - one Error enum for all sources.
