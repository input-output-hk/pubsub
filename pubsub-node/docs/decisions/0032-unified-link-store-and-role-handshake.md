# 0032 — Unified link store (role × direction) and the role-carrying handshake

**Status**: Accepted (feature 015; supersedes the `upstream`/`downstream` split of ADR 0017 as amended by 0020/0025/0031)

**Context**: `NodeState` held connections as two structures keyed by dial direction — `upstream: HashMap<(PeerId, TopicId), UpstreamState>` (dialed message sources) and `downstream: HashSet<(PeerId, TopicId)>` (accepted fan-out destinations). Two assumptions were baked in: every accepted link is a uniform relay, and a link's purpose is fully implied by who dialed (ROADMAP §1.2's connection-direction inversion). The M3 **publishing link** (S-link; logbook 2026-07-09, `docs/extensions/relay-tier-extension-proposal.md` §2.2 adapted to the hash-gated overlay) breaks the first assumption — a link that carries only the dialing publisher's own messages — and feature 016 (bidirectional links) strains the second. The 015 spec (Clarifications 2026-07-10/13) fixed: a unified `Link` abstraction, links keyed by role with coexisting roles per pair, and the canonical terms **publishing links** / **relaying links**.

## Decision

1. **One store.** `links: BTreeMap<(PeerId, TopicId, LinkRole, LinkDirection), LinkState>` replaces both structures. `LinkRole { Relay, Publisher }`; `LinkDirection { Out, In }` (who dialed); `LinkState { AwaitingAccept, Active }` is `UpstreamState` renamed — same two variants, same rule that terminal outcomes are removals, not stored states. `In` links are recorded `Active` at acceptance. Direction is part of the key because dial + accept between the same pair (the old upstream ∩ downstream case) are two links. `BTreeMap` (over `HashMap`/`HashSet`) makes shutdown-notice emission and snapshots deterministically ordered.
2. **Orientation is derived from role × direction**, not stored: `Relay`/`Out` = message source (gate: `Active`); `Relay`/`In` = fan-out destination; `Publisher`/`Out` = injection target for `Origin::Local` messages only; `Publisher`/`In` = source of that peer's own published messages only. One rule generalising the connection-direction inversion — for relay links the dialer receives; for publishing links the dialer sends.
3. **`Both` is emergent, not stored.** 016's bidirectional link is the Out + In *pair* a symmetric predicate produces when both ends dial each other; no merge/split lifecycle is introduced. `LinkDirection` is `#[non_exhaustive]` so 016 can still add a variant if its design requires one.
4. **The handshake carries the role.** Every `ConnectionAction` variant (`Request`/`Accepted`/`Terminated`/`Rejected`) gains a `role: LinkRole` field; `PlainConnection::signed_bytes` appends a role tag byte (`0x00` Relay, `0x01` Publisher) after the topic, so the signature binds emitter, action kind, topic, **and role** — an unauthenticated role would let a peer shift a request between the two caps. Pre-release wire-layout change, documented at the encoder per its standing contract.
5. **Public surface stays compatible where it was observed.** `Node::upstream_connections()` / `Node::downstream_connections()` are preserved as Relay-scoped views of the store; a new `Node::links()` exposes the full snapshot. `UpstreamState` the *name* is retired (call-site rename to `LinkState`; behaviour identical).

## Consequences

- Relay-only configurations are behaviour-preserving by construction: the store change is a re-keying, the wire change adds a constant tag, and the relay edge-predicate domain is untouched (015 SC-001).
- Every transition that touched `upstream`/`downstream` (heartbeat diff, request/accepted/rejected/terminated, shutdown, topic-removal cascade, the dissemination gate) re-keys on the quadruple; the acceptance prelude's downstream scan becomes role-scoped, which is what makes the publish cap disjoint from the relay `OC` (ADR 0033) fall out of the data model.
- Snapshot/getter surface grows one method; existing tests migrate by rename only.

## Alternatives rejected

- **Role sets on a per-pair entry** (`(peer, topic) → {roles}`) — murky lifecycle (one role `Active` while another `AwaitingAccept`), cap counting needs unpacking; rejected in the spec clarification.
- **New action variants per role** (`PublishRequest`, …) — doubles the variant set and duplicates every dispatch arm for a property orthogonal to the action kind.
- **A stored `Both` direction now** — would demand merge/split transitions 015 has no consumer for; the pair representation reuses the existing lifecycle unchanged.
