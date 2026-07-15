# 0036 — Flow-oriented link store: sources/sinks internally, role × direction as the stable view

**Status**: Accepted (feature 015; reshapes ADR 0032 §1/§3 and ADR 0034 §3 — the *shape* of the store, none of its semantics)

**Context**: review of PR #77 asked two things: whether the link abstraction really needs to distinguish publisher/relay on the upstream side, and whether refactors can stop rewriting tests. Tracing the reads showed the distinction is load-bearing in exactly two places — the M3 owner-binding on the receive gate (ADR 0033 §5) and the disjoint publish/relay accept caps — so the *information* is irreducible; but the four-cell shape (role × direction) forced every seam to consult multiple cells, unioned with per-peer dedup, and each store reshape rippled through tests bound to the cells.

## Decision

1. **Two maps, keyed `(peer, topic)`, entries with two facets each**:
   - `sources` — peers the node **receives from**: `{ pull: Option<LinkState>, push_accepted: bool }` (my relay pull link with dial lifecycle; an accepted inbound initiation link).
   - `sinks` — peers the node **sends to**: `{ relay_accepted: bool, push: Option<LinkState> }` (an accepted relay downstream; my standing initiation link with dial lifecycle).
2. **Each seam reads one map**: the receive gate does a single `sources` lookup; every fan-out policy is a single `sinks` pass selecting facets (`forward-to-all`: relay ∪ push-if-local; `role-scoped`: facet by origin; `role-agnostic`: any facet). One entry per peer makes per-peer target dedup **structural** — the duplicate-send class of bug (analysis A9 §2) can no longer exist.
3. **Role × direction remains the API**: every mutation (`insert`/`remove`/`activate_out`), the snapshots (`links()`, `iter()` emitting the same tuples in the same order), the getters, `inbound_scan(role, …)`, and the wire tag are untouched — the vocabulary is a *view* the store translates to facets (the mapping table lives on the type).
4. **Test-stability rule** (the lasting half of the decision): tests and callers bind to the role × direction views and the mutation API, never to the store's internal shape. This refactor is the proof: the store was rewritten and **no test file changed** (244 green before and after).
5. **Bidirectionality reads naturally**: an M4 edge is the peer present in `sources.pull` and `sinks.relay_accepted` — "I receive from and send to this peer" — rather than a four-cell pattern.

## Consequences

- Seams that used per-cell borrows read the facet iterators instead. (A materialised `cell()`/`LinkCell` view shipped initially and was removed in review — no consumers; add back when something needs it.)
- The dual-facet coexistence cases (a peer that is both my pull source and an accepted initiation source, etc.) are now visible as one entry with two facets — the coexistence rules of ADR 0032 unchanged, easier to audit.
- Cap accounting stays exact: emptied entries are dropped on facet removal so facet counts equal link counts.

## Alternatives rejected

- **Merging the upstream kinds into one undifferentiated source set** — loses the M3 owner-binding and merges the accept caps; a security-posture change needing formal-analysis sign-off, not a refactor.
- **Pure flat flow model without initiator/lifecycle facets** — the facets resurface immediately for dial lifecycle and per-role caps; pretending otherwise just moves the bits into parallel bookkeeping.
- **Renaming the public vocabulary to sources/sinks too** — maximal churn for zero information; the role × direction language matches the wire, the models' link classes, and the shipped tests.
