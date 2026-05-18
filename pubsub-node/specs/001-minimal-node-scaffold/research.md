# Phase 0 — Research

**Feature**: 001-minimal-node-scaffold
**Date**: 2026-05-18

Resolves plan-level questions left open by `spec.md`'s Clarifications session 2026-05-17 and records the dependency choices Constitution Principle III will require ADRs for.

Each entry: **Decision** / **Rationale** / **Alternatives considered**.

---

## 1. Async runtime

**Decision**: `tokio` (multi-thread runtime by default; `#[tokio::test]` for integration tests with default flavour). Planned ADR: `docs/decisions/0001-async-runtime-tokio.md`.

**Rationale**:
- De facto standard for Rust async; minimal ecosystem risk.
- First-class integration with `tracing`, `mpsc`, `oneshot`, `time::timeout`, all of which this scaffold uses.
- Every realistic future networked transport for pubsub-node (TCP, QUIC, libp2p) is tokio-native; choosing a different runtime now would force a churn later.

**Alternatives**:
- `async-std`: viable but smaller ecosystem; would lock us out of tokio-only crates later.
- `smol`: lightweight but the saved binary size is irrelevant for a node binary.
- No runtime / hand-rolled poll loop: contradicts FR-011's intent (use the async ecosystem to surface integration-test patterns).

---

## 2. TOML parsing

**Decision**: `serde` (derive) + `toml` (v0.8+). `PeerListConfig` derives `Deserialize`; the loader returns `Result<PeerListConfig, ConfigError>`. Planned ADR: `docs/decisions/0002-toml-via-serde.md`.

**Rationale**:
- Idiomatic Rust; trivially derived from the data model.
- `toml` crate produces line/column-aware parse errors, which we map to the actionable startup error required by US3 AS-2.
- `serde` is the universal Rust de/serialization framework — no risk of swapping it out.

**Alternatives**:
- `toml_edit`: supports round-tripping comments / formatting, which we do not need for a read-only loader.
- Hand-rolled parser: rejected; reinvents an audited dependency for no gain.

---

## 3. Structured logging

**Decision**: `tracing` (emit) + `tracing-subscriber` (collection). Default subscriber writes JSON to stderr; level filter via `RUST_LOG`. Planned ADR: `docs/decisions/0003-logging-via-tracing.md`.

**Rationale**:
- FR-010 mandates a *warn-level structured log entry that names the unknown identifier* — structured fields are required, which rules out `log` / `env_logger`.
- FR-006 permits (but does not mandate) structured logs alongside the queryable record; `tracing` makes this trivial.
- Engineering Standards "Observable state transitions" is satisfied by emitting events for register / send-accepted / recv-applied / unknown-peer-drop.

**Alternatives**:
- `log` + `env_logger`: no structured fields without an adapter; rejected.
- `slog`: structurally fine but its momentum has shifted to `tracing` since ~2022.

---

## 4. CLI parser

**Decision**: `clap` v4 with the `derive` feature. Two flags exposed: `--self-id <ID>` (required) and `--config <PATH>` (required). Planned ADR: `docs/decisions/0004-cli-via-clap.md`.

**Rationale**:
- Derive macro keeps the CLI definition co-located with the struct describing parsed args.
- Built-in `--help` / version generation satisfies SC-004 (a contributor needs to figure out invocation from the binary itself).
- Type-validated path argument (`PathBuf`) catches obvious operator errors before the loader runs.

**Alternatives**:
- `argh`: lighter but the ergonomics gap on a 2-flag CLI is invisible.
- `pico-args` / hand-rolled: saves a dep at the cost of every future flag costing ~10 lines.

---

## 5. Error model

**Decision**: `thiserror` for typed error enums in library code (`ConfigError`, `NetworkError`, `NodeError`). The `main.rs` binary uses `Result<(), Box<dyn std::error::Error>>` and prints the chain on failure. No `anyhow` in the library. Planned ADR: `docs/decisions/0005-typed-errors.md`.

**Rationale**:
- Library callers (tests, future binaries) need to match on error variants — typed enums are the only honest API.
- The binary boundary is the only place a generic error wrapper is acceptable; `Box<dyn Error>` is sufficient and avoids a second dependency.

**Alternatives**:
- `anyhow` in the library: collapses error variants into opaque strings; rejected.
- Hand-rolled error enums with manual `Display` / `Error` impls: works but `thiserror` is the lowest-overhead expression of the same shape.

---

## 6. Receive-side processing model (FR-013 driver)

**Decision**: Each `Node` spawns a background `tokio::task` during `Node::new(...)`. The task listens on a `tokio::sync::mpsc::UnboundedReceiver<Envelope>` returned from the network at registration; on each message it appends an entry to the node's `ReceivedRecord`. `Node::new(...)` does not return until registration + spawn are complete, so the node is fully observable by the time the constructor's future resolves.

**Rationale**:
- FR-013 requires `send().await` resolution to be decoupled from the recipient updating its record — a separate receive task is the simplest mechanism that delivers this and keeps acceptance scenarios honest (something must drive the recipient).
- Putting the task spawn inside `Node::new` means callers (tests, the CLI) never forget to start it. The lifecycle bookend (a `JoinHandle` retained by the Node and aborted on `Drop`) is small.

**Alternatives**:
- Caller-driven `node.poll()` loop: shifts complexity to every test; rejected.
- Synchronous delivery inside `Network::send()` (mutating the receiver's record directly): contradicts FR-013 and removes the test-harness exercise we explicitly asked for in the Clarifications session.
- Lazy spawn on first send/recv: opaque lifecycle; would race with the await-on-delivery helper.

---

## 7. Mailbox bounding

**Decision**: `tokio::sync::mpsc::unbounded_channel` per registered node in v1. A `// FUTURE:` note in `network.rs` records that v2 will swap to bounded `mpsc::channel` when a real transport introduces backpressure.

**Rationale**:
- Trust + Liveness assumptions in the spec eliminate the failure modes that would justify a bounded channel (slow consumer, hostile sender).
- 100 sequential sends (SC-002 / SC-005) sit comfortably in any reasonable memory budget.
- Unbounded queues let `network.send(...)` always succeed-then-resolve in O(1), which is the simplest match for FR-013's "accepted for delivery" contract.

**Alternatives**:
- Bounded mpsc (e.g. capacity 1024): forces a backpressure policy decision (drop? block? error?) that none of the FRs require; rejected.
- `flume` / `crossbeam`: zero ergonomic gain at this scale.
- `tokio::sync::broadcast`: wrong shape — pubsub-style fan-out is explicitly out of scope (FR-005, one-to-one only).

---

## 8. Network registration timing

**Decision**: `Network::register(&self, id, sender) -> Result<Receiver, NetworkError>` is invoked by `Node::new` during construction. The Node is fully registered, fully task-spawned, and ready to send & receive by the time its constructor's future resolves.

**Rationale**:
- Aligns with FR-002 ("multiple in-process node instances [that] register themselves").
- Eliminates the `node.start()` footgun: tests cannot accidentally send before the recv loop is alive.
- If registration fails (duplicate id — FR-009 says detection is *not required* but we may still surface it later), the failure surfaces as a constructor error rather than a silent dropped message.

**Alternatives**:
- Two-step `Node::new(...)` + `node.attach(network).await`: extra ceremony for every test; rejected.
- Implicit attach on first `send` / `recv`: race-prone with the await-on-delivery helper; rejected.

---

## 9. Ordering across multiple sends

**Decision**: From sender S to receiver R, deliveries appear in R's `received_messages()` in the order S awaited `send`. Across distinct senders, interleaving is unspecified.

**Rationale**:
- `tokio::sync::mpsc` is a per-channel FIFO; preserving sender-local order falls out of the implementation at zero cost.
- SC-005 ("every delivered Ping with the original N value intact, verified across at least 100 sends") becomes a one-line assertion when per-sender order holds.
- No FR requires a global total order; insisting on one would force expensive synchronisation we have no use for.

**Alternatives**:
- Unordered (multiset semantics): test friction with no engineering payoff.
- Strict total order across all senders: needs global serialization or per-network sequence numbers; over-engineered for a scaffold.

---

## 10. Await-on-delivery primitive (test helper shape)

**Decision**: A `tests/common::await_delivery` helper:

```rust
pub async fn await_delivery(
    node: &Node,
    expected_sender: &PeerId,
    expected_message: &Message,
    timeout: Duration,
) -> Result<(), AwaitError>;
```

Polls `node.received_messages()` on a short interval (default 1 ms) until a matching entry appears, returning `Ok(())` on hit or `Err(AwaitError::Timeout)` on exhaustion. The `timeout` parameter is **mandatory and injectable** — no wall-clock defaults inside the helper.

**Rationale**:
- Matches FR-013's contract: tests cannot assume immediate observability after `send().await`.
- Honours Engineering Standards "Reproducible tests" — no wall-clock dependency the test doesn't itself supply.
- Polling-based implementation keeps the helper independent of `Node` internals (no shared `Notify` handle); when v2 introduces a real network, the helper can stay verbatim or be upgraded to subscribe to a `Notify` if desired.

**Alternatives**:
- Internal `tokio::sync::Notify` exposed from `Node`: tighter, but couples the helper to Node internals; rejected for v1.
- Test fixture wraps `node` and intercepts the recv loop: more code for the same outcome.

---

## 11. Peer identifier and descriptor types

**Decision**:
- `pub struct PeerId(String);` — UTF-8 newtype with `Display`, `Debug`, `Eq`, `Hash`, `Clone`. Implements `FromStr` so `clap` and `serde` derive it cheaply.
- `pub trait PeerDescriptor: Clone + Send + Sync + 'static { fn id(&self) -> &PeerId; }`.
- V1 concrete impl: `pub struct BasicPeerDescriptor { id: PeerId }` implementing `PeerDescriptor`; this is what the TOML loader produces.

**Rationale**:
- Matches FR-009 (abstract type, `id()` accessor).
- Newtype around `String` prevents accidental id/payload confusion in function signatures (`fn send(to: &PeerId, …)` is unambiguous; a bare `&str` is not).
- Trait-based abstraction means future iterations can introduce `struct NetworkedPeerDescriptor { id: PeerId, addr: SocketAddr, key: PublicKey, … }` without changing any consumer code that only needs `descriptor.id()`.

**Alternatives**:
- Expose `PeerId` everywhere, no trait: loses the FR-009 abstraction; the next iteration would have to rename callers.
- Make `PeerDescriptor` a struct with optional fields (e.g., `Option<SocketAddr>`): bakes future-shape into v1 and contradicts the spec's "carries no other fields in v1" wording.

---

## ADR slot summary (Principle III deliverables)

| ADR # | Title (planned) | Triggering decision |
|-------|-----------------|---------------------|
| 0001 | Async runtime: tokio | Research §1, plan dependency |
| 0002 | Config via serde + toml | Research §2 |
| 0003 | Structured logging via tracing | Research §3 |
| 0004 | CLI via clap derive | Research §4 |
| 0005 | Typed errors via thiserror (no anyhow in library) | Research §5 |
| 0006 | Receive-task model & registration timing | Research §6 + §8 (single ADR; the two decisions are conjoined) |

`/speckit-tasks` will materialise the six ADR-authoring tasks as logical-increment commits per Development Workflow.

---

## Open follow-ups (deferred past v1 plan)

These are *not* NEEDS CLARIFICATION — they are deliberate v2+ items, recorded so they don't get rediscovered later:

- Bounded mailbox with backpressure policy (Research §7) — re-opens when v2 introduces a real transport.
- Cryptographic identity (key-derived `PeerId`) — re-opens when the project's identity story converges (cross-ref the parent project's [[project-pubsub-design-synthesis]] notes).
- Duplicate-id detection on register (FR-009 says not required in v1) — natural to add once cryptographic identity is in.
- Logging volume tuning and shipping (Engineering Standards note) — deployment concern; out of scope for the scaffold.
