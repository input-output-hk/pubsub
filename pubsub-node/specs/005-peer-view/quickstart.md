# Quickstart: exercising the verifiable strategies

Illustrative — exact names settle at implementation. Strategies use the current injection shape (and migrate unchanged if the parallel refactor later passes them as `apply` arguments). Realises the bucketed-pull overlay mechanics (`docs/extensions/bucketed-pull.md`, ADR 0024/0025/0030/0031).

## Construct a verifiable node

```text
let selection  = HashGatedConnection::new(self_id, target_degree);                 // dial side
let acceptance = HashGatedBoundedAcceptance::new(self_id, target_degree, cap_buffer); // inbound side
// strategies injected at node construction (ADR 0028); `genesis` — the initial
// epoch nonce both seams hash — is node state, passed to `Node::new` (ADR 0031)
```

Omit the verifiable strategies (or the `--genesis`/`--target-degree`/`--cap-buffer` params at the edge) → today's full-mesh `connect-to-all` / `accept-from-all` behaviour, unchanged (SC-006).

**Mixed pairs are legal** (an experiment axis — [[N-031]]): the seams are configured independently, so e.g. a `connect-to-all` dialer facing hash-gated-bounded acceptors builds cleanly. Note the cost: the acceptors silently drop the ~`(1 − 1/B)` of dials the predicate refuses, and in v1 (single readiness heartbeat, no retry) each such dial persists as a pending `AwaitingAccept` entry until shutdown; the realized Active topology still converges to the predicate-selected ~`target_degree` density.

## Exercise (test shape)

1. Seed candidate membership (fixed before readiness).
2. Drive a node (or small set) to readiness (`Event::Synced` — both seams are gated on it), then drive dialing by firing the parameterless `Event::Heartbeat` (v1 fires one on the readiness edge) — no timers. `Event::Epoch { nonce }` advances the randomness context; a following `Heartbeat` re-dials under it (ADR 0031).
3. To exercise rejection: drive a node past its per-topic cap `OC = ⌈target_degree + c·√target_degree⌉` with legitimate (predicate-valid) inbound requests; observe the over-capacity `Rejected`. On the dialer, after a `Rejected`, observe that the matching pending upstream is dropped and nothing further happens — no retry/back-fill (deferred to a future strategy family); the realized upstream degree may settle below `target_degree`.
4. To exercise the silent drop: send a membership-invalid request or one whose edge predicate fails under the current epoch nonce; observe it is dropped with no reply (distinct log causes `membership_validation_failed` / `illegitimate_request`).

## Assert via getters/snapshots (never logs)

- **Degree ≈ target_degree** — `upstream_connections()` per topic tracks the fixed `target_degree`; `downstream_connections()` ≤ `OC = ⌈target_degree + c·√target_degree⌉` per topic (SC-004).
- **Reproducibility** — same genesis + membership → identical upstream set on rebuild, incl. across machines (SC-001).
- **Verifiability** — the acceptor's predicate result equals the dialer's for the same `(requester, candidate, topic, epoch nonce)` (SC-002).
- **Small topic** — `≤ ~target_degree` candidates ⇒ `B = 1` ⇒ connect-to-all / accept-all (SC-006).
- **Rejection / under-fill** — a `Rejected` drops the matching pending upstream and produces no further effects; no retry/back-fill, so the realized degree may settle below `target_degree` (FR-008/FR-009, SC-007).
- **No amplification** — a single id spamming a victim has its accepted fraction bounded by the `1/B` density; predicate-failing requests are all dropped (SC-005).
- **Uniformity** — over a sweep of ≥1,000 epoch nonces on a fixed candidate set with `B > 1`, per-candidate frequency within tolerance (SC-003; research R5).

## Out of scope here

The topology builder, multi-node scale, and delivery-percentile/latency/propagation metrics are the separate experiment-framework feature; discovery/view sampling (`H_v`), periodic heartbeats + rotation/teardown, the real unbiasable beacon, the incentive/chain layer (deposits, sybil bound, slashing), and golden/relay tiers are later features.
