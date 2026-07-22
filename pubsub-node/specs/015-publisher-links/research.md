# Research: publisher links and dissemination-model configurations (015)

Decisions for the minimal realisation of M3/M4/M5. The full exploration lives on
`archive/015-full-exploration` (the abandoned first cut of feature 015, formerly
PR #77's content): it validated the model semantics, found the correctness traps
recorded in the spec, and demonstrated that a role×direction abstraction layer
(LinkRole/LinkDirection enums, a LinkStore type with flow facets, view layers, a
unified selection/params machinery) is *sufficient* — this respecification keeps
its behaviour and deletes the layer. Each decision below notes what the archive
did where the two differ.

## R1 — State shape: kind-in-key, two collections

**Decision**: keep `NodeState.upstream` / `NodeState.downstream` as the two
collections; change their key to a plain struct `LinkKey { topic, peer, kind }`
and their type to `BTreeMap<LinkKey, LinkState>`. Direction is *which field*;
no direction enum, no store type, no view layer. Field order in `LinkKey` is
topic-first so derived `Ord` gives per-topic contiguity — `downstream_on(topic)`
walks a `BTreeMap` range instead of scanning the whole map.

**Rationale**: the information content is four per-(peer,topic) facts (relay
up, relay down, publisher up, publisher down); kind-in-key represents them with
one new enum and one key struct. A peer holding both kinds in one direction is
two adjacent entries — no facet struct, no eviction hazard. The archive's flow
store solved the same coexistence with per-entry facet structs and a mutation
API + views; three shapes more for the same facts.

**Alternatives rejected**:
- *Flow-oriented LinkStore with facets* (archive ADR 0036) — right semantics,
  but a store type + facet entries + role×direction view layer for what two
  maps and a key struct express.
- *Four named collections* (`upstream`, `downstream`, `publisher_sources`,
  `publisher_targets`) — no new enum, but every cascade/shutdown/termination
  site must enumerate four structures, and the M5 fan-out union needs a merged
  iteration anyway; the kind enum must exist for the wire regardless.
- *Kind in the value* (entry struct with two optional facets) — the flow store
  again, minus the name.
- *`HashMap` retained* — loses the ordered per-topic range walk; `BTreeMap`
  matches `candidates` and makes snapshots deterministic.

## R2 — Lifecycle: `UpstreamState` renamed `LinkState`; accepted links insert `Active`

**Decision**: rename the existing two-variant lifecycle enum to `LinkState`
(`AwaitingAccept` / `Active`) — it now also tracks the node's *downstream*
publisher dials, so the old name is wrong. Accepted inbound links (relay
downstream, publisher upstream) are inserted directly as `Active`: presence ==
accepted, exactly today's `downstream`-set semantics expressed in the map.

**Rationale**: one lifecycle enum for all four facts; the invariants (which
field × kind is a dial vs an accept) live in doc comments and the handlers, not
in the type system — the minimal-shapes constraint. The rename is mechanical
(the archive did the same rename and nothing else needed it).

## R3 — Wire: `kind` on `PlainConnection`, one trailing signed byte

*(Superseded in review round 5 by ADR 0034: the kind is now message
vocabulary — one `Message` variant per handshake, `PlainConnection` loses the
field, and `signed_bytes(kind)` takes the tag from the variant context. The
trailing-tag preimage layout below stands, relay/publisher byte-identical,
plus a new `0x02` symmetric tag.)*

**Decision**: add `kind: LinkKind` to `PlainConnection` (not to each
`ConnectionAction` variant). `signed_bytes()` appends one tag byte after the
topic: `0x00` Relay, `0x01` Publisher. All four actions carry it: `Terminated`
and `Rejected` need the kind to know *which* coexisting link they concern.

**Rationale**: every control action concerns exactly one link, and a link has
exactly one kind — the enclosing plain struct is the natural carrier; one field
instead of four variant edits, one encoder line instead of four. Appending
(rather than inserting before the action tag) keeps the existing byte prefix
intact so the layout-pin test change is additive. The signature covers the byte,
so an attacker cannot replay a relay accept as a publisher accept.

**Alternatives rejected**: per-variant `role` field (archive) — four edit sites
and four encoder branches for the same bit; separate `PublishRequest` action
variants — doubles the action vocabulary and the dispatch.

**Deliberate test exception**: the signed-bytes layout-pin test *must* change
because the layout changed; that edit is the pinned new layout, not test churn.

## R4 — Publisher seams: `Option<Arc<dyn …>>` second instances, no `none` kinds

**Decision**: `NodeState` gains `publisher_strategy:
Option<Arc<dyn ConnectionStrategy>>` and `publisher_acceptance:
Option<Arc<dyn ConnectionAcceptanceStrategy>>`. `None` (the default; CLI flag
absent) means: no publisher dial pass on the heartbeat, and inbound publisher
`Request`s are silently dropped (`publisher_links_disabled`). Spec FR-014 —
the M2 baseline — is then true *by construction*, not by a policy that returns
nothing.

**Rationale**: reuses the existing traits untouched; avoids a `NoneSelection`
strategy type and a `none` kind in two enums (the archive's approach). Two
`if let Some` sites (heartbeat pass, request dispatch) are the whole cost.

## R5 — Strategy reuse: rename `expected_upstream` → `expected_links`; kind-aware scan

**Decision**:
- `ConnectionStrategy::expected_upstream` → `expected_links` — for the relay
  instance the result set is dialed as upstream sources; for the publisher
  instance, as downstream targets. One-word doc contract: "the links this node
  should have dialed"; the caller knows which field they land in.
- `HashGatedConnection` and the hash-gated acceptance baselines gain a
  `kind: LinkKind` field (default `Relay` via the existing constructors, so
  current call sites and tests compile unchanged) selecting the hash domain.
- `admit_prelude`/`downstream_scan` become kind-aware: the relay instance
  counts `downstream` × Relay (today's semantics), the publisher instance
  counts `upstream` × Publisher — each instance scans the collection *it
  admits into*. Renamed `link_scan(map, kind, emitter, topic)`.
- `NodeView.downstream: &HashSet<(PeerId,TopicId)>` is replaced by borrows of
  both maps: `upstream: &BTreeMap<LinkKey, LinkState>`, `downstream: same`.
  The strategy test-support `view()` builders absorb the type change.

**Alternatives rejected**: a unified `LinkSelectionStrategy` kind/params family
(archive) — new machinery where two instances of the existing traits suffice;
keeping the `expected_upstream` name — actively wrong for the publisher
instance, and the rename is a two-impl mechanical edit.

## R6 — Edge predicate: three domains, existing signature untouched

**Decision**: internal `is_valid_edge_in(domain, nonce, topic, a, b, buckets)`;
public functions:
- `is_valid_edge` — existing signature; relay domain
  `pubsub/bucketed-pull/relay-edge/v1` (renamed from `…/edge/v1` in review
  round 3: the tag became relay-exclusive, and no experiment results existed
  yet to keep reproducible — same genesis now yields a different M2 topology
  than pre-015, deliberately);
- `is_valid_edge_publisher` — domain `pubsub/bucketed-pull/publisher-edge/v1`
  (independent draw, so a node's publisher targets are uncorrelated with its
  relay upstreams);
- `is_valid_edge_sym` — domain `pubsub/bucketed-pull/edge-sym/v1` over the
  **canonically ordered** pair (lexicographic on raw public-key bytes), so both
  ends compute the identical draw. Independent of the directional domains —
  pairs whose directional preimage happens to be in canonical order must not
  correlate with the symmetric draw.

No `publisher-edge-sym` domain: no published model uses symmetric publisher
links; adding the domain now would be consumer-less forward compatibility
(constitution, Engineering Standards).

## R7 — Fan-out: origin parameter; two kinds; not in the two-phase builder

**Decision**: `FanoutStrategy::targets(topic, downstream, origin, exclude)`
where `downstream` is the new map borrow and `origin: &Origin` is the recorded
message's origin (already computed at both call sites).
- `ForwardToAll` (default, name kept): relay entries ∪ (publisher entries at
  `Active` **iff** `origin == Local`) — this *is* M3 per `m3/README.md`
  (an M2 node simply has no publisher entries, so behaviour is unchanged).
- `AllLinks` (CLI `all-links`, M5): relay ∪ publisher-Active for **every**
  origin. *(Superseded in review round 3: the pair shipped as
  `ForwardToRelays`/`forward-to-relays` (default) and
  `ForwardToAll`/`forward-to-all` (M5) — "forward" names the relayed-traffic
  path; the publisher-link sends of the default kind are seeding.)*
Both dedup per peer by collecting into a `BTreeSet<PeerId>` before the exclude
filter. A `FanoutStrategyKind` (`forward-to-all` | `all-links`) parses the CLI
value; fan-out stays injected directly (as today), not routed through the
two-phase builder — it has no parameters.

**Alternatives rejected**: the archive's third kind (`role-scoped`, publisher
links for local origin *instead of* relay links) — an experiment variant with
no published model behind it; drop it until a model needs it.

## R8 — Publisher admission: a config enum, not a seam

**Decision**: `PublisherAdmission { OwnerOnly, AnyVerified }` (default
`OwnerOnly`) as a plain `NodeState` field, CLI `--publisher-admission
owner-only|any-verified`, `FromStr` at the edge. Owner-binding compares the
message's `publisher_id` public key with the link peer's public key.

**Rationale**: per-message admission with exactly two published-model variants;
a trait would be unconsumed generality (same conclusion as archive ADR 0035,
kept). Severance is policy-independent: an invalid signature severs the
admitting link under either policy.

## R9 — Symmetric edges: one flag, both relay seams, emergent pairs

*(Superseded in review round 5 by ADR 0034: bidirectionality is now
constructed by the symmetric handshake — one accept records both directions
on both ends — so reciprocity no longer depends on the two ends' independent
draws agreeing, and capped acceptance composes. The flag still sets the
predicate on both relay seams and additionally switches the handshake
vocabulary.)*

**Decision**: `--symmetric-edges` sets `symmetric: bool` on **both**
`ConnectionParams` and `AcceptanceParams` from the single CLI flag; the
hash-gated relay selection and acceptance consult `is_valid_edge_sym` instead
of the directional predicate. Publisher instances are always directional. M4
bidirectionality is *emergent*: the predicate holds for the unordered pair, so
both ends dial and both accept — each side ends with the peer in `upstream` ×
Relay and `downstream` × Relay. No stored "both" direction, no new control
flow, no wire change.

**Rationale**: a per-seam flag would let the two sides disagree and silently
drop every dial as illegitimate (the trap the spec pins). Carried unchanged
from archive ADR 0035.

## R10 — Getters renamed; per-class snapshots

**Decision**: `upstream_snapshot`/`downstream_snapshot` (state) and the `Node`
getters are renamed and filtered by kind:
- `upstream_relays()` → `Vec<(PeerId, TopicId, LinkState)>` (dial lifecycle)
- `downstream_relays()` → `Vec<(PeerId, TopicId)>` (accepted; presence-only)
- `upstream_publishers()` → `Vec<(PeerId, TopicId)>` (accepted; presence-only)
- `downstream_publishers()` → `Vec<(PeerId, TopicId, LinkState)>` (dial lifecycle)
The relay getters return exactly what the old getters returned on an M2 node,
so pre-existing tests change only the call name (the permitted mechanical
rename). Keeping the old names as relay-only aliases was rejected: a getter
named `upstream_snapshot` silently meaning "relay only" is a trap once four
link classes exist.

## R11 — CLI: relay-prefixed renames + publisher mirrors

**Decision**: rename for symmetry, add mirrors:

| Old | New |
|---|---|
| `--connection-strategy` | `--relay-strategy` |
| `--acceptance-strategy` | `--relay-acceptance-strategy` |
| `--target-degree` | `--relay-degree` |
| — | `--publisher-strategy` (optional; absent = publisher links disabled) |
| — | `--publisher-acceptance-strategy` (optional; absent = inbound publisher requests dropped) |
| — | `--publisher-degree` |
| — | `--fanout-strategy` (`forward-to-all` default, `all-links`) |
| — | `--publisher-admission` (`owner-only` default, `any-verified`) |
| — | `--symmetric-edges` (flag; both relay seams) |

`--genesis`, `--bucket-count`, `--cap-buffer` stay shared across seams (the
epoch nonce and B must agree network-wide anyway; the cap buffer is one `c` for
both bounded acceptance instances). Pre-release: no deprecation aliases.

## R12 — Construction: `NodeStrategies` grows to four slots

**Decision**: `NodeStrategies { relay_connection, relay_acceptance,
publisher_connection: Option<_>, publisher_acceptance: Option<_> }`, built by
the existing two-phase builder from the four kind selections (publisher kinds
`Option<…Kind>`) and the two params structs (each gaining `symmetric: bool`;
`ConnectionParams.target_degree` doubles for the publisher instance via a
separate `publisher_degree` field on the params — one params struct per seam
family, relay and publisher values side by side, rather than four params
structs). `Node::new` takes `NodeStrategies` as one argument plus
`fanout_strategy` and `publisher_admission` — call sites (main, integration
test builders) restructure once instead of growing three more positional
arguments.

## R13 — Correctness requirements carried from the exploration

Pinned as tests (spec SC-006), mechanism per this design:
- **Unconditional readiness-gated publisher dials**: the publisher pass runs in
  `handle_heartbeat` after the relay diff, behind the same `synced` gate,
  consulting only the publisher strategy (never the relay maps).
- **Admitting-link severance**: `handle_dissemination` remembers *which* gate
  admitted the message (relay upstream Active vs publisher upstream) and severs
  that `LinkKey` on signature failure.
- **Per-peer dedup**: structural in both fan-out kinds (`BTreeSet<PeerId>`).
- **Dedicated symmetric domain**: R6.
- **Emergent reciprocity**: R9; integration test asserts pairwise symmetry of
  `upstream_relays`/`downstream_relays` across nodes.

The archive's M4/M5 integration tests (`model_family.rs`) and publisher-link
injection test port with getter-name and constructor adjustments only.

## R14 — ADR

One ADR — `0032-publisher-links-and-model-family.md` — records the structural
decisions: kind-in-key state shape, wire kind byte, seam reuse via second
instances, origin-aware fan-out + `all-links`, `PublisherAdmission` enum,
symmetric edge mode, and the relationship to the archived exploration (which
ADRs 0032–0036 on that branch it supersedes). Numbering restarts after main's
0031; the archive branch's 0032–0036 never merged and remain reference-only.
