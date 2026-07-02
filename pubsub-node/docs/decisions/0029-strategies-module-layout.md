# 0029 — `strategies/` module grouping; connection state stays in the core

**Status**: Accepted

**Context**: Feature 005 grew the strategy surface: three seams each became a directory (`connection/`, `acceptance/`, `fanout/` — a trait in `mod.rs`, one file per concrete policy, a `*StrategyKind` selector), and ADR 0028 added a construction module. That module sat at the crate root as a loose `strategy_config.rs` while the seams were top-level directories — an asymmetry — and the seeded PRNG sampler that connection's `SeededBoundedConnection` uses will be needed verbatim by 015's `SeededBoundedFanout`, with no shared home to avoid duplication.

Separately, `connection/mod.rs` conflated two different things: the connection-selection **strategy** (trait + impls + kind) and `UpstreamState` — the per-`(peer, topic)` upstream lifecycle enum (`AwaitingAccept`/`Active`). The latter is not a strategy: the strategy returns *which* upstreams to dial; the pure core (`crate::state`) assigns and stores the lifecycle state.

## Decision

1. **Group all strategy policy under `strategies/`.** `src/strategies/` contains the three seam submodules (`connection`, `acceptance`, `fanout`) plus `config` (the ADR 0028 two-phase construction: `StrategyParams` per seam, `StrategyConfigError`, `NodeStrategies`/`NodeStrategiesBuilder`). `strategy_config.rs` becomes `strategies::config`, dropping the prefix. A future `strategies::sampling` is the home for the shared seeded PRNG sampler when 015 adds its second consumer (not extracted now — no second consumer yet).
2. **Connection lifecycle state stays in the core.** `UpstreamState` (and the `test_support` lifecycle-event harness) move out to `crate::connection_state` — core domain vocabulary the transition owns, not policy. `Admission`, by contrast, is the acceptance seam's **return contract**, so it stays in `strategies::acceptance`.
3. **Public API is unchanged.** `lib.rs` re-points its `pub use` paths to `strategies::…` / `connection_state::…` but re-exports the identical names, so external consumers (and `main.rs`) are unaffected.

## Consequences

- All strategy-related code lives in one directory; the loose config file is gone; the split is uniform across seams and ready for `strategies::fanout`'s 015 growth to sit beside the others.
- The strategy-vs-core-state boundary is explicit: `strategies/` holds policy only; `connection_state` holds the lifecycle state the core assigns. `state.rs` imports `UpstreamState` from the core, not from a strategy module.
- Move-only, behaviour-preserving: no logic changed; full suite + clippy + fmt green after the move.
- Deferred: the shared PRNG-sampler extraction to `strategies::sampling` lands with 015 (its first second consumer), avoiding a premature one-caller abstraction.

## Alternatives rejected

- **Leave the layout as-is** — keeps the loose `strategy_config.rs` / directory asymmetry and gives the shared PRNG sampler no natural home before 015 duplicates it.
- **Move `connection/` wholesale, `UpstreamState` included** — least churn, but puts a non-strategy lifecycle type under `strategies/` (`state.rs` would import `crate::strategies::connection::UpstreamState`, which misrepresents it). Rejected in favour of the explicit policy/state split.
- **Extract the PRNG sampler to `strategies::sampling` now** — premature; a single caller until 015. Deferred to when the second consumer exists.
