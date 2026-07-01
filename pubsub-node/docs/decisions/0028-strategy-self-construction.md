# 0028 — Two-phase strategy construction and required-parameter validation

**Status**: Accepted

**Context**: With named strategy selectors (`ConnectionStrategyKind`, `AcceptanceStrategyKind`; `FanoutStrategyKind` at feature 015), the CLI edge (`main.rs`) held per-strategy construction *and* validation:

```rust
let upstream_degree = args.upstream_degree.unwrap_or_else(|| { eprintln!(…); exit(2) });
Arc::new(SeededBoundedSelection::new(args.seed, args.self_id.clone(), upstream_degree))
```

That is strategy-specific knowledge ("seeded-bounded requires an upstream degree") living at the edge. Every new strategy adds another bespoke `unwrap_or_else(exit)` block, and the "which params are required" rule drifts away from the strategy it belongs to.

## Decision

Construct strategies in **two explicit phases**, uniform across every seam.

**Phase 1 — key → builder.** The edge parses each seam's strategy *key* into its `*StrategyKind` (the existing `clap`-derived enums via `FromStr`): an absent flag resolves to the seam default, an unknown key is rejected **at CLI parse**. `NodeStrategies::builder(connection_kind, acceptance_kind)` captures the resolved kinds and constructs nothing yet.

**Phase 2 — params → strategy.** `NodeStrategiesBuilder::build(&ConnectionParams, &AcceptanceParams)` binds each seam's **own** params struct and constructs every seam, returning `NodeStrategies { connection, acceptance }`. Each `Kind::build(&SeamParams) -> Result<Arc<dyn XStrategy>, StrategyConfigError>` reads only its seam's params and validates the ones the chosen variant requires; a required param left `None` yields `StrategyConfigError::MissingParameter { strategy, parameter }`.

Supporting types (`src/strategy_config.rs`):

- **Per-seam params** — `ConnectionParams { self_id, seed, upstream_degree: Option }`, `AcceptanceParams { downstream_degree: Option }` (`FanoutParams` at 015). Already-typed values; no `clap` in the core. Each kind sees only its own seam's params — no shared grab-bag from which a strategy fishes out what it needs.
- **`NodeStrategies` / `NodeStrategiesBuilder`** — the aggregate two-phase builder. `main.rs` makes **one** `.build(...)` call and maps **one** `StrategyConfigError` to a clean exit; no per-strategy branching, `unwrap`, or validation at the edge.

## Consequences

- Parse-at-the-edge preserved: `clap` owns key parsing (default + unknown-key rejection) and one error→exit mapping; the core takes already-parsed values.
- Required-parameter validation is co-located with the strategy that requires it (correctness by locality); each seam's params are typed to that seam.
- The edge no longer repeats `build → unwrap_or_else(exit)` per seam — one aggregate build, one error site.
- Adding a strategy = a new kind variant + its `build` arm (+ maybe a field on that seam's params struct) — **no edge churn**.
- The pattern is uniform across the connection and acceptance seams; the fan-out seam joins the same `NodeStrategies` builder at feature 015 (`FanoutStrategyKind` + `FanoutParams`).

## Alternatives rejected

- **Keep construction/validation in `main`** — leaks per-strategy knowledge into the edge; grows with every strategy.
- **A single flat `StrategyParams` grab-bag** passed to every `build` — the initial cut. Each seam's `build` reached into a struct carrying *all* seams' params (connection ignored `downstream_degree`, etc.); a strategy "picking what it needs" from a shared bag reads like brute-forcing and blurs which params belong to which seam. Replaced by per-seam params.
- **A shared `ConfigurableStrategy` trait** (associated `Output` + `build`) — a uniform trait fn, but the seams take heterogeneous params and return different trait objects, so inherent per-kind `build` methods behind one aggregate builder are simpler with no loss.
- **CLI-flag names baked into a machine-readable error consumed by `main`** — would re-introduce per-strategy knowledge at the edge; instead each `build` supplies its own operator-facing `parameter` phrase.
