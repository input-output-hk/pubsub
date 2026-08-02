# 0034 — Connection-message vocabulary per handshake kind; constructed reciprocity for symmetric links

**Status**: Accepted (feature 015, review round 5). Amends ADR 0032 (§2, §6).

**Context**: the 015 maintainer review (PR #77, round 5) found two related
problems in the first-merged shape. (1) The realised "M4" drew each unordered
peer pair i.i.d. via the symmetric predicate and hoped both ends' independent
handshakes would agree — reciprocity was *emergent*, conditional on
B-agreement between the two ends, and the configuration lacked the formal
model's defining minimum-degree floor. Three compensating artifacts existed
only to protect that emergence: four signed messages per edge, the
`--symmetric-edges` × capped-acceptance startup prohibition, and the
one-flag-couples-both-seams rule. (2) The link kind lived as a field on the
shared `PlainConnection`, so the type system could not route per-kind
behaviour — every shared handler recovered the distinction by testing the
field, which is also what made "accept into both maps" inexpressible without
a branch in a shared handler.

## Decision

1. **Kind is message vocabulary, not a message field.** `Message` carries one
   connection variant per handshake — `RelayConnection` /
   `PublisherConnection` / `SymmetricConnection`, each a `ConnectionMessage`
   — and `apply` routes each variant to a flat named handler module
   (`state/handlers/{relay,publisher,symmetric}.rs`). `PlainConnection`
   loses its `kind` field; `signed_bytes(kind: HandshakeKind)` binds the
   handshake tag into the preimage from the variant context (`0x00` relay,
   `0x01` publisher — byte-identical to the previous kind-field encoding —
   and `0x02` symmetric, a new tag: pre-split, symmetric-mode nodes spoke
   relay bytes), so a control message cannot be replayed across vocabularies
   and no inconsistent kind/variant pairing is representable. Shared
   lifecycle mechanics — the verification prelude, the readiness gate,
   refusal arms, dial promotion, kind-scoped teardown — are helper functions
   in `handlers/mod.rs` the per-kind handlers compose.
2. **Two taxonomies, deliberately not 1:1.** The message vocabulary
   (`HandshakeKind`) names **establishment protocol**; the stored `LinkKind`
   (unchanged: `Relay | Publisher`, in the `LinkKey`, per ADR 0032 §1) names
   **traffic class**. The symmetric handshake is a different establishment
   protocol for ordinary relay-class links: its accepted links are
   `LinkKind::Relay` entries, present in both collections. No `Bidirectional`
   stored kind exists — a bidirectional link *is* the same link in both maps;
   a tag would be a second encoding of a fact placement already stores, and
   every traffic decision on such a link takes the relay branch anyway.
3. **Constructed reciprocity (M4).** Bidirectionality is realised in the
   handshake mechanics, not the selection predicate: on accepting a symmetric
   request, the acceptor records the emitter in **both** `downstream` and
   `upstream` (`Active`); on receiving the acceptance, the dialer activates
   its pending upstream entry and inserts the downstream mirror. One accept
   decision per edge — reciprocity holds *unconditionally*, for any selection
   strategy and regardless of the two ends' bucket-count views (the
   B-agreement assumption now costs dropped dials at worst, never one-way
   edges). Teardown is atomic: `Terminated` and misbehaviour severance remove
   both halves (on a symmetric node every relay link is mirrored by
   construction — a node runs one model). The crossing case (both ends of a
   valid edge dial) resolves idempotently: an acceptance arriving for an
   already-`Active` upstream re-affirms the pair.
4. **`NodeStrategies.symmetric_edges`** switches the node: the relay dial
   pass speaks the symmetric vocabulary, inbound symmetric handshakes are
   admitted (by the relay acceptance instance — in symmetric mode configured
   with the symmetric predicate), and severance mirrors. `false` (default):
   inbound symmetric handshakes are dropped outright
   (`symmetric_edges_disabled`) — off by construction, like the publisher
   seam. The guard is mutual: a **symmetric** node likewise drops inbound
   *relay* handshakes (`relay_handshake_disabled`) — admitting a directional
   request would record a one-way link on a node whose teardown/severance
   mechanics assume every relay link is mirrored. The CLI flag is unchanged
   (`--symmetric-edges` sets the predicate on both relay seams and the
   vocabulary together), but the coupling is no longer load-bearing for
   correctness.
5. **Capped acceptance composes with symmetric mode.** A capacity refusal is
   a whole-edge refusal (explicit `Rejected`, nothing inserted on either
   end), so no one-sided half can survive it. The ADR 0032 startup
   prohibition and its rationale are deleted.

## Consequences

- The "M4" recipe remains an **approximation** and is labelled as such: the
  mechanics compose with any selection, but hash-gated selection at any B
  leaves the per-node pick count binomial (a node can draw zero valid edges),
  so the formal model's minimum-degree ≥ RF floor additionally requires a
  **uniform exactly-RF** selection kind — in (gate B, cap K) terms the
  (B = 1, K = RF) point, which no node selection kind provides yet. That
  selection kind is a follow-up feature (it belongs with the B-as-parameter
  fix tracked from the 005 review); until it lands the recipe table does not
  claim M4. Note the formal models specify *private, epoch-seeded uniform
  selection* — uniform picks in the node are what M4 prescribes; verifiable
  hash-gating is the protocol-track deviation to be quantified against it.
  *Amendment (2026-08-01, feature 017 / ADR 0039): that follow-up landed —
  the selection plane's pick count is the uniform exactly-RF selection, and
  the label is claimed: `--relay-pick-count RF --relay-symmetric` realises
  the formal M4 exactly, with the minimum-degree floor and mean ≈ 2·RF
  evidenced fleet-wide in `tests/model_family.rs`. The privacy note stands:
  the operator-supplied selection seed is a prototype stand-in for the
  models' private randomness (recorded with the 017 seed-derivation
  decision).*
- Two signed messages per edge when only one end picks it; the four-message
  crossing case persists only under symmetric *predicate* selection (both
  ends see the edge) and resolves idempotently.
- The wire preimage layout is unchanged for relay/publisher and gains the
  symmetric tag value; the layout-pin tests were re-pinned accordingly. No
  decoder exists to migrate (the real codec remains N-004 / feature 009 — the
  vocabulary split was priced for exactly this window).
- Handler code leaves `state.rs` (~500 lines) into per-vocabulary modules
  named by link class, not by MX number — models remain instantiations
  (vocabulary + strategies), not module owners. Per-vocabulary unit suites
  mirror the split (`state/tests/symmetric_links.rs` beside
  `publisher_links.rs`).

## Alternatives rejected

- **Placement-returning acceptance** (`Accept { store_into: … }` from the
  strategy): moves `NodeState`'s layout into the strategy contract — policy
  code dictating storage internals — needs the same information twice on the
  dial side, and leaves the receive-gate branch untouched.
- **A `Bidirectional` stored kind**: forces `Relay | Bidirectional` arms in
  the receive gate, both fan-out policies, and analytics — all of which
  always take the same branch — double-represents what placement stores, and
  folds an orthogonal axis into an enum that will grow on traffic grounds.
- **Keeping the kind field with per-kind match arms**: the pre-review shape;
  the type system cannot route behaviour, and every new handshake semantics
  lands as another branch inside shared handlers.
