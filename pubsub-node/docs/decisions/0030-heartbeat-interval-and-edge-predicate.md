# 0030 — Heartbeat interval event and the shared verifiable edge predicate

**Status**: Accepted

**Context**: The bucketed-pull selection/acceptance (ADR 0024/0025) is defined **per round** — the predicate `H(genesis, T, requester, candidate, interval) mod B == 0` depends on an interval that both peers agree on. The existing single dial-trigger, `Event::ConnectionSetup`, carries no such counter, and both the dial and accept seams need the *same* predicate. This ADR pins how the interval flows and where the predicate lives.

## Decision

1. **`Event::ConnectionSetup` → `Event::Heartbeat { interval: u64 }`.** The dial trigger becomes an interval-carrying heartbeat, an advancing 0-based counter (offset from genesis). It is **driver-fired** — a producer / the experiment framework fires it; the pure core advances no wall-clock. `handle_heartbeat` stores the interval on `NodeState` (so the acceptor can verify against the current interval) and runs selection for it. `(genesis, interval)` stand in for the model's per-round beacon `nonce_R`; a real unbiasable beacon (block hash / VRF) is deferred.
2. **Interval threaded through both seams.** `ConnectionStrategy::expected_upstream(&self, subscriptions, candidates, interval)` and `ConnectionAcceptanceStrategy::admit(&self, …, interval)` gain the interval argument. `NodeState` holds `interval: u64` (default 0), set by `Heartbeat`; `handle_connection_request` reads it so the acceptor verifies the requester's edge against the current interval.
3. **Shared predicate module `strategies::edge`.** `is_valid_edge(genesis, topic, requester, candidate, interval, buckets) -> bool` and the small helpers `bucket_count(candidates_len, rf)` (`= max(1, round(len/rf))`) and `accept_cap(rf, c)` (`= ⌈rf + c·√rf⌉`) live here — the single source of the predicate/formulae the dial and accept seams both call. (This is the shared-helper home ADR 0029 anticipated; it is a *predicate*, not the earlier `rank_key`/PRNG sampler.)
4. **v1 fires a single interval.** The readiness path fires one `Heartbeat { interval: 0 }` where `ConnectionSetup` fired. **Periodic heartbeats and cross-interval rotation/teardown** (re-selecting the full edge set each interval and pruning edges no longer predicate-valid) are out of scope — but `Heartbeat { interval }` is the exact shape that layer needs, so it drops in without reshaping the seam.

## Consequences

- The interval is an *input* to the pure strategies (not a field), so they stay pure and reproducible; `NodeState` owns the interval as event-derived state (a fold over `Heartbeat`), consistent with "node state is a function of the event stream".
- Both seams share one predicate implementation — the dial side selects, the accept side verifies, and they cannot drift (SC-002).
- `Event::ConnectionSetup` references (readiness dial, tests, tooling) migrate to `Heartbeat { interval }`; the single-fire behaviour is unchanged for v1.
- Rotation/teardown is a clean follow-on: fire `Heartbeat { interval: n }` periodically and have `handle_heartbeat` diff the new edge set against the held connections (add new-valid, prune no-longer-valid) — no seam change.

## Alternatives rejected

- **Keep `ConnectionSetup` and pass the interval out-of-band** — the interval is intrinsic to the dial trigger; a separate channel invites dial/verify interval mismatch.
- **Wall-clock timer in the core** — breaks purity/reproducibility; the interval advances only via a driver-fired event.
- **Duplicate the predicate in each seam** — drift risk; the shared `strategies::edge` module is the single source.
- **Implement rotation/teardown now** — deferred; v1 is single-interval, and the event shape already accommodates it.
