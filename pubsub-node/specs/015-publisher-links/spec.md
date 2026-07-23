# Feature Specification: Publisher links and dissemination-model configurations (M3/M4/M5)

**Feature Branch**: `015-connection-link-model` (spec dir `015-publisher-links`; the branch predates the respecification — the directory name is the canonical feature identifier)

**Created**: 2026-07-15

**Status**: Draft

**Input**: User description: "Publisher links and dissemination-model configurations (M3/M4/M5) as a minimal extension of the existing connection layer. The node currently realises the M2 dissemination model; extend it so a node can be configured to run M3, M4, or M5 — per-node configuration, no --model preset, axes independently sweepable by the experiment harness. Minimal extension, not a new abstraction layer: exactly two new public shapes (a link-kind discriminator and a plain link key), the existing upstream/downstream collections extended by kind, existing strategy traits reused as second instances for the publisher seams, the link kind carried in the signed connection actions, fan-out gaining message origin, and receive-gate publisher admission as a config enum. Correctness requirements carried over from the exploration on archive/015-full-exploration. Pre-existing tests stay untouched except a mechanical getter rename."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Configure a node fleet for M3 (publisher links) (Priority: P1)

An experimenter configures every node in a simulated network with the M3 recipe: hash-gated relay links (as today) plus hash-gated **publisher links** — standing links each node establishes unconditionally toward a deterministic set of peers, used exclusively to send out that node's **own** publications. A message published anywhere in the network reaches all subscribers; publisher links inject the message into the relay mesh without inflating any node's relay degree.

**Why this priority**: M3 is the team's adopted primary network-analysis model (`../formal_spec/hybrid_dissemination/models/m3/`); the experiment harness needs it configurable end-to-end before the M2-vs-M3 comparison runs can start. It is the reason this feature exists.

**Independent Test**: Start a small network with relay + publisher strategies configured; verify each node establishes its publisher links immediately at readiness (regardless of its relay topology), that a locally-published message is sent over both link classes, and that a message merely being relayed is never sent over publisher links.

**Acceptance Scenarios**:

1. **Given** a node with a publisher-link strategy and degree configured, **When** it reaches readiness and its dial event fires, **Then** it requests its expected publisher links unconditionally — even if it already has relay downstream peers.
2. **Given** a node with active publisher links and relay downstream peers, **When** it publishes its own message, **Then** the message is sent over both its relay downstream and its active publisher links, with at most one send per peer.
3. **Given** the same node, **When** it receives a message published by some other node (relaying), **Then** the message is forwarded over relay downstream links only — never over publisher links.
4. **Given** a node that accepted an inbound publisher link from peer P, **When** a message arrives over that link, **Then** it is validated exactly like any message (signature, topic registration, publisher authorization, subscription, dedup) — the receive gate is kind-agnostic. *(Amended per the maintainer answer to the A12 owner-binding question: M3's exclusivity is the sender-side fan-out policy — scenario 3 — not a receiver check; the original owner-binding clause compared signed content against an unsigned transport field and is removed.)*

---

### User Story 2 - Configure a node fleet for bidirectional links (the M4 approximation) (Priority: P2)

*(Amended in review round 5, A11/ADR 0034: reciprocity is constructed by a
dedicated symmetric handshake — one accept decision records the link in both
directions on both ends — rather than emerging from two independent
directional handshakes. The recipe approximates M4 pending a uniform
exactly-RF selection kind and does not claim the label.)*

An experimenter enables symmetric edges: relay-link selection evaluates an order-independent edge predicate, and each valid edge is established by the symmetric handshake — one accept records the link in each direction on both ends. Every message floods over all incident links; no publisher links exist.

**Why this priority**: M4 (`models/m4/`) is one of the three target models for the cross-model experiments, but secondary to M3.

**Independent Test**: Start a network with `--symmetric-edges`; verify every established relay link is reciprocated (A upstream-of B ⟺ B upstream-of A) and a single publication reaches every node over the flooded mesh.

**Acceptance Scenarios**:

1. **Given** two nodes for which the symmetric edge predicate holds, **When** the dial event fires and the request is accepted, **Then** each node holds the peer as relay upstream **and** relay downstream — whichever end dialed.
2. **Given** a symmetric-edge network whose predicate graph is connected, **When** any node publishes, **Then** every subscribed node receives the message.
3. **Given** the symmetric configuration, **Then** the symmetric edge decisions are statistically independent of the directional ones (dedicated randomness domain), so directional and symmetric sweeps of the same network are independent draws.

---

### User Story 3 - Configure a node fleet for M5 (directed k_in/k_out, everything-carrying) (Priority: P3)

An experimenter configures directed in-links (relay, k_in) and out-links (publisher-shaped, k_out) where **both** classes carry every held message: fan-out sends everything to the union of both downstream kinds. *(Amended: the receive gate is kind-agnostic for every model — M5 differs from M3 only in the fan-out switch.)*

**Why this priority**: M5 (`models/m5/`) completes the target model family; it reuses every mechanism from stories 1–2 plus two configuration switches.

**Independent Test**: Three nodes a→b→c connected only via publisher links with the union fan-out; a publication by a traverses b to c — the exact hop the default fan-out does not forward.

**Acceptance Scenarios**:

1. **Given** a node with the union fan-out configured, **When** it holds any message (own or relayed), **Then** the message is sent over relay downstream **and** active publisher links, deduplicated per peer.
2. **Given** any node, **When** a verified message published by node X arrives over an inbound publisher link from node Y ≠ X, **Then** it is admitted — the receive gate is kind-agnostic. *(Amended: formerly gated by a per-node admission policy, removed with FR-008.)*
3. **Given** the default fan-out on the delivering side, the same message is never *sent* over a publisher link in the first place — the exclusivity lives at the sender.

---

### User Story 4 - M2 baseline unchanged (Priority: P1)

A node configured without publisher links, without symmetric edges, and with the default fan-out behaves exactly as before this feature: the M2 baseline the harness measures against must not shift.

**Why this priority**: the experiment program's comparisons are only valid if the baseline is stable; regression here invalidates existing results.

**Independent Test**: The pre-existing test suite passes without behavioural modification (a mechanical getter rename is the only permitted edit).

**Acceptance Scenarios**:

1. **Given** the default configuration, **When** the existing test suite runs, **Then** all tests pass with no changes other than the getter rename.
2. **Given** a node with no publisher strategy configured, **Then** it never dials publisher links and never accepts an inbound publisher request.

---

### Edge Cases

- A peer legitimately holds links of **both kinds in the same direction** on one topic (e.g. it is my relay pick and also pushes its publications to me): both must coexist without either evicting the other, and sends to that peer are deduplicated.
- The two ends of a symmetric pair must never disagree about reciprocity: one accept decision records both directions on both ends (A11/ADR 0034), so a refused or dropped handshake leaves **no** one-sided half — at worst a dial is dropped and the edge does not form.
- An invalidly-signed payload arriving over an inbound publisher link severs **that** link — not a relay entry that may not exist for the peer.
- Publisher dials are unconditional: a node with zero relay downstream and a node with a full relay downstream establish the same publisher links.
- Publisher degree exceeding the candidate set: the node establishes links to all matching candidates (same behaviour class as small-topic relay selection today).
- Termination/removal of one kind's link to a peer must not remove the other kind's link to the same peer.
- Topic removal and node shutdown cascade over links of both kinds.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The node MUST support two classes of per-topic links — **relay links** (existing) and **publisher links** (new) — with links of both kinds to the same peer on the same topic coexisting independently.
- **FR-002**: Publisher-link establishment MUST be strategy-driven exactly like relay links: a selection strategy instance with its own degree parameter, drawing from a randomness domain distinct from the relay domain, dialing on the same readiness-gated event as relay dials, **unconditionally** (never contingent on the node's relay topology).
- **FR-003**: Connection control actions (request / accept / reject / terminate) MUST identify the link kind, covered by the message signature, so the acceptor applies the acceptance policy, randomness domain, and capacity of that kind. The kind implies the data direction: a relay request means the dialer will receive; a publisher request means the dialer will send.
- **FR-004**: Acceptance capacity MUST be accounted per link kind — relay admissions never consume publisher capacity and vice versa.
- **FR-005** (default fan-out — M3): A locally-published message MUST be sent over relay downstream links **and** active publisher links; a message received from a peer MUST be forwarded over relay downstream links **only**.
- **FR-006** (kind-agnostic receive gate; amended per the maintainer answer to the A12 owner-binding question): A message arriving over **any** Active upstream link — relay or publisher — MUST be validated exactly like any message (signature, topic registration, publisher authorization, subscription, dedup) and admitted when the chain passes. No owner-binding: a receive-side restriction exists only if it is checkable from the signed bytes alone, and the original owner-binding compared the signed publisher against the unsigned transport sender. M3's exclusivity is FR-005's sender-side fan-out, honest-behaviour compliance rather than receiver enforcement.
- **FR-007** (M5 fan-out): A node MUST be configurable to send **every** held message — regardless of origin — over the union of relay downstream and active publisher links.
- **FR-008**: *Removed with FR-006's amendment* — with a kind-agnostic gate there is no owner-binding to relax; the former `any-verified` behaviour is every node's behaviour and the admission-policy configuration surface is deleted. M5 needs only FR-007.
- **FR-009** (symmetric edges — the M4 approximation; amended A11/ADR 0034): The node MUST support a symmetric edge mode in which relay selection evaluates an order-independent predicate over the peer pair, under a randomness domain dedicated to symmetric evaluation (independent of the directional domains), and each valid edge is established by a dedicated symmetric handshake: one accept decision MUST record the relay-class link in both directions on both ends, and teardown/severance MUST remove both halves together. One configuration switch drives the predicate and the handshake together. No new stored link class is introduced (the entries are relay-class links present in both collections).
- **FR-010**: An invalidly-signed payload MUST sever the link that admitted it — the inbound publisher link when that was the admission path.
- **FR-011**: When a peer is reachable over both downstream kinds, each outgoing message MUST be sent to that peer at most once.
- **FR-012**: All axes (relay selection/acceptance, publisher selection/acceptance, fan-out behaviour, symmetric mode, per-kind degrees) MUST be independently configurable per node; no bundled model preset is provided. *(The admission-policy axis was removed with FR-008.)*
- **FR-013**: Node state snapshots MUST expose the two link classes distinctly in both directions (upstream/downstream × relay/publisher) so tests and the experiment harness can observe topology per class.
- **FR-014**: A node with no publisher strategy configured MUST neither dial publisher links nor accept inbound publisher requests, and its behaviour MUST be indistinguishable from the pre-feature node (M2 baseline preservation).
- **FR-015**: Terminating or removing a link of one kind MUST NOT affect a coexisting link of the other kind to the same peer/topic; topic removal and shutdown cascade over both kinds.

### Key Entities

- **Link**: a per-(topic, peer, kind) relationship with a lifecycle (awaiting-accept for links the node dialed; active). Grouped by direction: **upstream** (peers the node receives from) and **downstream** (peers the node sends to).
- **Link kind**: relay (the existing pull-based dissemination mesh) or publisher (standing links carrying, by default, only the owner's own publications).
- **Message origin**: whether a held message was published locally or received from a peer — the discriminator the default fan-out uses.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A network configured with the M3 recipe delivers a message published by any node to 100% of subscribed nodes, with publisher links carrying only their owner's publications (zero foreign messages observed over publisher links).
- **SC-002**: A network configured with the M4 recipe shows 100% link reciprocity (every relay link has its reverse) and 100% delivery over a predicate-connected graph.
- **SC-003**: A network configured with the M5 recipe delivers a foreign publisher's message across a chain of standing publisher links (a→b→c) — the hop the M3 default fan-out provably does not forward under the same topology.
- **SC-004**: The pre-existing test suite passes with zero behavioural test edits; the only permitted change is a mechanical accessor rename.
- **SC-005**: Every model axis is a per-node configuration switch; the three recipes are expressible as documented flag combinations with no code change.
- **SC-006**: Each of the correctness requirements carried from the exploration (unconditional readiness-gated publisher dials, admitting-link severance, per-peer dedup, dedicated symmetric domains, constructed reciprocity) is pinned by at least one test.

## Assumptions

- The model semantics in `../formal_spec/hybrid_dissemination/models/m3|m4|m5/` (read-only per Constitution Principle V) are authoritative; where this spec paraphrases them, the READMEs win.
- The vocabulary is **publisher links** — "seed"/"S-link" naming from earlier discussions is deliberately avoided.
- The realisation approximates the models' private exactly-k uniform picks with verifiable-hash predicate draws (binomial around k) — the same approximation class the existing M2/M3 relay realisation carries; quantifying the gap is the experiment harness's job, out of scope here.
- Publisher links reuse the existing connection lifecycle (signed control actions, membership validation, idempotent acceptance, readiness gating); no new handshake is introduced.
- The exploration branch `archive/015-full-exploration` (formerly PR #77's content) is the reference for design rationale and portable integration tests; this feature re-implements its behaviour with a minimal state/API footprint as specified in the Input.
- Incentive/anti-Sybil mechanics, churn handling, retry logic, and a `--model` preset are out of scope.
