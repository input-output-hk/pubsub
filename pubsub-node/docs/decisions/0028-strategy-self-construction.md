# 0028 — Strategy self-construction and required-parameter validation

**Status**: Accepted

**Context**: With named strategy selectors (`ConnectionStrategyKind`, `AcceptanceStrategyKind`; `FanoutStrategyKind` at feature 015), the CLI edge (`main.rs`) held per-strategy construction *and* validation:

```rust
let upstream_degree = args.upstream_degree.unwrap_or_else(|| { eprintln!(…); exit(2) });
Arc::new(SeededBoundedSelection::new(args.seed, args.self_id.clone(), upstream_degree))
```

That is strategy-specific knowledge ("seeded-bounded requires an upstream degree") living at the edge. Every new strategy adds another bespoke `unwrap_or_else(exit)` block, and the "which params are required" rule drifts away from the strategy it belongs to.

## Decision

Each strategy-kind enum builds its own concrete strategy from parsed parameters and validates the parameters it requires:

- **`StrategyParams`** (`src/strategy_config.rs`) — a parse-at-the-edge struct of already-typed values (`self_id`, `seed`, `upstream_degree: Option`, `downstream_degree: Option`; `fanout_degree` at 015). No `clap` in the core.
- **`Kind::build(&self, &StrategyParams) -> Result<Arc<dyn XStrategy>, StrategyConfigError>`** — an inherent method per kind. It reads only the params it needs; a required param left `None` yields `StrategyConfigError::MissingParameter { strategy, parameter }`.
- The **edge stays lean**: it parses args into one `StrategyParams`, calls `kind.build(&params)` per seam, and maps a single `StrategyConfigError` to a clean exit. No per-strategy branching or validation at the edge.

## Consequences

- Parse-at-the-edge preserved: the core takes already-parsed values; the CLI owns only argument parsing + one error→exit mapping.
- Required-parameter validation is co-located with the strategy that requires it (correctness by locality).
- Adding a strategy = a new kind variant + its `build` arm (+ maybe a `StrategyParams` field) — **no edge churn**.
- The pattern is uniform across the connection and acceptance seams, and the fan-out seam adopts it at feature 015 (`FanoutStrategyKind::build`).

## Alternatives rejected

- **Keep construction/validation in `main`** — leaks per-strategy knowledge into the edge; grows with every strategy.
- **A shared `ConfigurableStrategy` trait** (associated `Output` + `build`) — a uniform trait fn, but the seams take heterogeneous params and return different trait objects, so an inherent per-kind `build` is simpler with no loss.
- **CLI-flag names baked into a machine-readable error consumed by `main`** — would re-introduce per-strategy knowledge at the edge; instead each `build` supplies its own operator-facing `parameter` phrase.
