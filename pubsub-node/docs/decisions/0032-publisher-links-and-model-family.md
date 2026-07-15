# 0032 — Publisher links and the dissemination-model family (M3/M4/M5) as a minimal extension

**Status**: Accepted (feature 015)

**Context**: the experiment program needs the node configurable to run the M3,
M4, and M5 dissemination models (`../../../formal_spec/hybrid_dissemination/models/`)
against the existing M2 baseline. M3 adds *publisher links* — standing links a
node establishes unconditionally and uses only for its **own** publications,
with the receive side admitting a message over such a link only from the link's
owner. M4 makes relay picks bidirectional. M5 makes both link classes carry
every held message. A first, fully-working realisation
(`archive/015-full-exploration`, the abandoned content of PR #77's first cut)
introduced a role×direction abstraction layer — `LinkRole`/`LinkDirection`
enums, a flow-oriented `LinkStore` with per-entry facets, view layers, a
unified selection/params machinery — and proved the semantics but at the cost
of many more public shapes than the information requires. This ADR records the
respecified minimal realisation; the archive branch's ADRs 0032–0036 are
superseded by it and remain reference-only (they never merged).

## Decision

1. **Kind-in-key state, two collections.** The irreducible per-`(peer, topic)`
   information is four facts (relay up/down, publisher up/down). `NodeState`
   keeps its two collections — `upstream` (peers the node receives from) and
   `downstream` (peers it sends to) — re-keyed by a plain
   `LinkKey { topic, peer, kind }` over `enum LinkKind { Relay, Publisher }`,
   each entry a `LinkState` (`AwaitingAccept`/`Active`, the renamed
   `UpstreamState`). *Direction is which field*, not an enum. Dialed entries
   (relay upstream, publisher downstream) carry the lifecycle; accepted entries
   are inserted `Active` — presence means accepted. Topic-first key order makes
   per-topic reads contiguous in the `BTreeMap`. A peer may hold both kinds in
   one direction — two adjacent entries, mutated independently.
2. **One signed wire bit.** `PlainConnection` gains `kind: LinkKind`, encoded
   as a trailing tag byte inside `signed_bytes()` (`0x00` relay, `0x01`
   publisher) — the acceptor needs it to pick the acceptance policy, hash
   domain, and capacity, and the signature binding prevents replaying a relay
   control message as a publisher one. Kind implies data direction: a relay
   request's dialer will receive; a publisher request's dialer will send.
3. **Second instances, not new seams.** The publisher selection/acceptance
   slots are `Option<Arc<dyn …>>` second instances of the *existing*
   `ConnectionStrategy` / `ConnectionAcceptanceStrategy` traits, configured
   with the publisher hash domain and their own degree/cap (`admit_prelude` is
   kind-aware: each acceptance instance scans only the collection it admits
   into, so relay and publisher capacities are disjoint by construction).
   `None` — the default — disables publisher links outright: no dial pass, and
   inbound publisher requests are dropped (`publisher_links_disabled`). The M2
   baseline is therefore preserved by construction, not by a null policy.
   `ConnectionStrategy::expected_upstream` is renamed `expected_links` (the
   publisher instance's picks are dialed *downstream*).
4. **Origin-aware fan-out.** `FanoutStrategy::targets` takes the downstream
   link map and the recorded message's `Origin`, returning per-peer
   deduplicated targets. `ForwardToAll` (default) sends to relay downstream
   always, and to `Active` publisher links **only** for local-origin messages —
   which *is* M3's exclusivity rule (`m3/README.md`); `AllLinks` (M5) unions
   both kinds for every origin.
5. **Publisher admission is a config enum, not a seam.**
   `PublisherAdmission { OwnerOnly (default), AnyVerified }` on `NodeState`
   governs the receive gate for inbound publisher links (M3's owner-binding vs
   M5's relaxation). Per-message admission with exactly two published-model
   variants — a trait would be unconsumed generality. Severance is
   policy-independent: an invalidly-signed payload severs the **admitting**
   link.
6. **Symmetric edges (M4) as a predicate mode.** `is_valid_edge_sym` hashes the
   canonically-ordered peer pair under a dedicated domain
   (`…/edge-sym/v1`, independent of the directional draws); one flag
   (`--symmetric-edges`) drives relay selection **and** acceptance together —
   a per-seam split would let the two sides disagree and silently drop every
   dial. Bidirectionality is *emergent* reciprocal dial pairs; no stored
   "both" direction, no wire change, no publisher links in M4.
7. **Per-class observability.** Snapshots/getters are renamed and split by
   class — `upstream_relays()` / `downstream_relays()` (exactly the pre-015
   snapshots on an M2 node) plus `upstream_publishers()` /
   `downstream_publishers()`. Keeping the old names as silent relay-only
   aliases was rejected as a trap.

## Consequences

- M2/M3/M4/M5 are per-node flag combinations (documented in the feature's
  quickstart); no `--model` preset — the axes stay independently sweepable.
  M5's two switches (`all-links` fan-out, `any-verified` admission) must be
  paired network-wide.
- The wire layout changed (appended kind byte): the layout-pin test was
  updated in the same commit — the one deliberate behavioural test edit.
- CLI: `--connection-strategy`/`--acceptance-strategy`/`--target-degree` are
  renamed `--relay-strategy`/`--relay-acceptance-strategy`/`--relay-degree`,
  with publisher mirrors; `--genesis`/`--bucket-count`/`--cap-buffer` stay
  shared across seams (pre-release, no deprecation aliases).
- Modelling caveat (inherited): the verifiable-hash realisation approximates
  the models' private exactly-k uniform picks with binomial-around-k predicate
  draws; for M4 that means expected degree ≈ RF with no min-degree guarantee.
  Quantifying the gap against the models' laws is the experiment harness's job.

## Alternatives rejected

- **Role×direction abstraction layer** (the archive branch) — proved the
  semantics; four more public shapes (role/direction enums, store type, facet
  entries, view layer) than the four facts require.
- **Four named collections** — no kind enum in the key, but every
  cascade/shutdown/termination site enumerates four structures, the M5 union
  still needs a merged pass, and the kind must exist for the wire anyway.
- **`none` strategy kinds instead of `Option` slots** — a null policy object
  plus a kind-name in two enums to express "feature off".
- **Kind field per `ConnectionAction` variant** — four edit sites and encoder
  branches for the same bit; the enclosing `PlainConnection` is the natural
  carrier (every action concerns exactly one link).
- **Publisher-symmetric hash domain** — no published model uses symmetric
  publisher links; consumer-less forward compatibility.
