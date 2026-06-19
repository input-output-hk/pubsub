<!-- SPECKIT START -->
**Feature roadmap (read first if planning the next feature)**: `specs/ROADMAP.md` — feature list 002–012 with architectural anchors (edge-vs-golden config split, connection-direction inversion) and per-feature open questions. **Feature numbers are IDs, not a strict build order** — see the note at the top of ROADMAP.

**Active feature**: **006-fanout-policy** (branch `006-fanout-policy`, spec dir `specs/006-fanout-policy/`). **Specified + clarified (two rounds, converged) + planned + tasked (T001–T017) + analyzed (Session 1, GO)** — see [`specs/006-fanout-policy/plan.md`](specs/006-fanout-policy/plan.md); next `/speckit-implement` (critical → TDD). **Rebased onto merged 014; post-014 reconciliation in progress** — ADR renumbered **0020 → 0021** (014 holds 0020 on main); the receive-path authorization step now reads `TopicEntry::is_open`/`is_publisher_authorized` rather than a raw `BTreeSet<PublicKey>`; 013's "subscriptions ∩ registered_topics" is superseded by 014's maintained invariant (the check-chain *shape* is unchanged, so the R9 `validate_dissemination` extraction still holds). Message publishing + fan-out forwarding on top of the 004 connection topology: fire-and-forget `Node::publish(SignedMessage)` → `Event::Publish` → named `handle_publish` (receive-path checks **minus** the connection gate and severance; **proxy/injection allowed** — `publisher_id` need not be self); verbatim fan-out (no re-sign, reuses `Effect::Send`) to downstream peers on the topic via an injected `FanoutStrategy` seam (v1 `ForwardToAll`, the deliberate twin of 004's `ConnectionStrategy`) with receive-path split-horizon (exclude the deliverer); content-hash dedup (`seen: HashSet<MessageHash>`, unbounded) checked **after** signature verification at the shared record point on both paths (cause `duplicate`); `ReceivedDelivery.from` → `origin: Origin { Local, Peer(PeerId) }` (also fixes pre-existing doc drift). Subscriber-relay (a node never relays a topic it isn't a member of). Deliberately **not parity-preserving** (dissemination suites reworked: full-mesh dedup + scripted partial/line relay per Clarifications; receive-path unit tests unaffected beyond the new constructor arg since their downstream is empty). Artifacts: spec (verbatim Input + 2-round Clarifications), plan + plan-input.md (verbatim), research (R1–R9), data-model (decision flows + propagation walkthroughs + deferral catalogue D1–D5), contracts/fanout-protocol.md, quickstart, checklists/{requirements,traceability}.md (traceability 4→0), tasks T001–T017, analysis.md (Session 1, GO); ADR **0021** (fan-out strategy seam + content-hash dedup + `Origin`, refs 0018/N-005). **Out of scope**: pick-k fan-out (needs seeded RNG → would break deterministic `apply`), bounded `seen`, equivocation detection (012), `Message::Signed`→`Dissemination` rename, epochal re-dialer.

**Merged dependencies on `main`**:

- **004 — node event-loop refactor** (PR #50). The pure core: crate-internal `NodeState` + pure `apply` → `Vec<Effect>` (`Effect` uninhabited), named per-variant handlers (`apply` → `handle_message_received` → `handle_signed_message`), `Arc<Mutex<NodeState>>` shell with sync getters + the single event queue (`Event`/`EventQueue`) drained by one loop with node-owned producers via `spawn_producer` (`network_mailbox_loop`), spawn-in-constructor + drop-abort. ADR 0011/0012. (Connection model deferred to a follow-on `004-connections`.)
- **008 — subscription registry** (PR #51). The in-memory **subscription list** (node membership; distinct from the topic registry — no shared trait): `SubscriptionRegistry` (single node-keyed `watch(node)`) + `SubscriptionRegistryControl` (`set_topics`/`unregister`) + `InMemorySubscriptionRegistry` (`new()`/`from_file`); `MembershipEvent` (Joined/TopicsChanged/Left) / `MembershipWatch` (push, mirrors `NetworkHandle`/ADR 0007). `Event::MembershipUpdate` + `handle_membership_update` fold the node's **own** entry → `subscriptions` and **other** nodes → per-topic `candidates` (self-excluded, distinct from config bootstrap `peers`; N-007); `Node::candidates` getter. The subscription list — not config — is the source of truth for a node's own topics (ADR 0013/0014): `Node::new` takes the registry generically as `Arc<R>`, no `initial_subscriptions`, no fail-fast on a missing entry; the node is read-only and the local `subscribe`/`unsubscribe` mutators are removed (ADR 0015); 002's `subscribed_topics` config field is removed.
- **013 — topic registry** (merged 2026-06-12). The in-memory **topic registry**: which topics legitimately exist + each topic's authorized publisher keys (empty set = open topic), the topic-governance counterpart to 008's subscription list (no shared trait). `TopicRegistry` (single **global** `watch()`) + `TopicRegistryControl` (`set_topic`/`remove_topic`) + `InMemoryTopicRegistry` (`new()`/`from_file`); `TopicRegistryEvent` (Registered/PublishersChanged/Removed) / `TopicRegistryWatch`. `Event::TopicRegistryUpdate` + `handle_topic_registry_update` fold `registered_topics: HashMap<TopicId, BTreeSet<PublicKey>>` into `NodeState`; effective accept-filter = `subscriptions ∩ registered_topics` (single `Node::subscriptions()` getter returns it); `handle_signed_message` drops `topic_not_registered` and `publisher_not_authorized` before signature verification. `Node::new` gains a third generic registry param (named `subscription_registry`/`topic_registry`); `PublicKey` gains `Ord`. **ADR 0016** (topic-registry interface + node integration). Publisher keys ≡ subscription-list node id at 011 (N-009).

- **004-connections — logical connections** (merged 2026-06-16, PR #56). Per-`(peer, topic)` connections on `NodeState`: `upstream` (`AwaitingAccept`/`Active`) + `downstream`; strategy-driven establishment (`ConnectionStrategy`/`ConnectToAllCandidates`) on a setup event; signed control messages (`Message::Connection`, emitter inside the signed bytes); membership-validated idempotent acceptance; connection-gated receive path (connection → subscription → registered → authorized → signature) with silent signature-only severance (`Effect::Misbehaved`); consuming `Node::shutdown(self)`; **`PeerId` wraps `PublicKey`**; shared signing-bytes helper. ADRs **0017/0018/0019**. (014 then rebased onto this: extends the cascade to `upstream`/`downstream`, replaces the `connection_setup_delay` timer with an `Event::Synced` readiness transition pushed by the single registry indexer once both registry snapshots are folded, and resolves S7/N-015.)
- **014 — registry consistency** (merged 2026-06-18, PR #63). Elevates 013's read-time `subscriptions ∩ registered_topics` into a **maintained `NodeState` invariant** (`subscriptions`/`candidates ⊆ registered_topics.keys()`) via **strict-drop** folds (membership for an unregistered topic dropped + logged; no declared/pending buffer; no auto-promotion — **013 SC-004 removed**); **defensive registry fold** (create-only-on-`Registered`, no `or_default`); **atomic cascade** on `Removed` (clears subscriptions + candidates + projection + `upstream`/`downstream` in one fold); declarative **`TopicEntry`** (`pub(crate)`; `is_open`/`is_publisher_authorized`; `registered_topics: HashMap<TopicId, TopicEntry>`). **Readiness/dial**: each `watch()` returns a `(snapshot, live-watch)` pair (`TopicSnapshot`/`MembershipSnapshot`, no `SnapshotComplete` variants); a single `registry_indexer_loop` folds the topic snapshot before the membership snapshot, then pushes `Event::Synced` (readiness → `Syncing`/`Synced`, `Node::is_synced()`) which runs `handle_connection_setup` — **replacing the `connection_setup_delay` timer**; `Event::ConnectionSetup` is retained as the dial action (tests/operator/epochal). Resolves **S7/N-015**. **ADR 0020** (amends 0016).

The node event-queue boundary shared across the registry features is specified in **`specs/event-loop-and-registry-contract.md`**.

**Most recently completed feature**: **003-message-envelope-mock-crypto** — the signed message envelope + mock crypto. Canonical reference under `specs/003-message-envelope-mock-crypto/` (plan, research, data-model, contracts, quickstart). Key decisions:

- ADR 0009 `docs/decisions/0009-crypto-trait-shape.md` — crypto trait shape (concrete byte newtypes, no associated types, mock-crypto factory shape).
- ADR 0010 `docs/decisions/0010-protocol-message-type-hierarchy.md` — `Message` enum + `SignedMessage` / `PlainMessage` split + 001 `Envelope` → `RoutingFrame` rename + `MessageHash::of(&PlainMessage)` content-anchored hash.
- Introduced the `Signer` / `Verifier` traits + `crypto::mock`, the `Arc<dyn Verifier>` `Node::new` parameter, and the `message_dropped` / `cause` drop-event convention (which replaced 002's `topic_drop`). Everything 002 contributed (the `TopicId` newtype, the subscription set + `subscribe`/`unsubscribe` API, the `subscriptions()` snapshot, the TOML `subscribed_topics` field) propagates unchanged.

**Workstream-level docs (sibling to feature dirs)**:
- `specs/event-loop-and-registry-contract.md` — shared contract for the two active features (004 / 008); both their specs cite it.
- `specs/IMPLEMENTATION_NOTES.md` — deferred implementation questions to revisit (currently N-001 for local-emission/local-receipt under a future REST API; N-002 for self-addressing under connection-based transports in feature 004+; N-003 for chain-integrity / equivocation / publisher-authorization validation under the registry features (**publisher-authorization slice closed by 013**, ADR 0016; equivocation / parent-hash / sequence / deposit remain at 012); N-004 for CBOR-canonical encoding swap under feature 009 or first cross-language consumer; N-005 for the `MessageHash::of(&PlainMessage)` content-anchored hash decision, to revisit when downstream features first operationally consume the hash — chain-integrity validation at 008 / 012, future caching / dedup; N-006 for the construction-failure (duplicate-registration) integration test, deferred from 004's parity scope to 004-connections; N-007 for `peers` placement — shell field today; **resolved for 008**: the `MembershipUpdate` arm folds a topic-derived candidate set into `NodeState`, kept distinct from the config bootstrap `peers` shell field (ADR 0014); the static `peers` field stays on the shell for the future dialer; revisit again at 005 (`PeerView`)).
<!-- SPECKIT END -->

# pubsub-node — agent guidance

Rust implementation of the Cardano PubSub node. Project-level context is in the parent `pubsub/CLAUDE.md`; this file covers what an agent working inside `pubsub-node/` needs to know.

## Authoritative documents

- **Constitution**: `.specify/memory/constitution.md` (v1.2.0) — five principles (I–V) plus Engineering Standards and Development Workflow rules; honour all of them before authoring code or specs. (Notably: logs are operator UX, not a test surface; operator-facing strings carry no FR/spec citations; parse at the edge; forward-compatible interfaces are justified by a ROADMAP consumer; multi-step test state is built via declarative test-only helpers; `/speckit-analyze` findings are recorded in the feature's `analysis.md`.)
- **Feature specs** (agent-editable): `specs/NNN-<short-name>/spec.md`, produced by `/speckit-specify`.
- **ADRs** (agent-authored): `docs/decisions/NNNN-<title>.md` — structural decisions, per Constitution Principle III.
- **Protocol specifications (READ-ONLY per Principle V)**: `../formal_spec/`, `../docs/`, `../docs/extensions/`. Surface ambiguity via ADR or issue; do not edit.

## Spec Kit workflow

Features flow through: `/speckit-specify` → `/speckit-plan` → `/speckit-tasks` → `/speckit-implement`. Small fixes may bypass.

### Manual branch step — do this BEFORE `/speckit-specify`

`.specify/` lives inside the parent `pubsub/` git repo, not at the git root. Because of this subfolder layout, the Spec Kit git extension does **not** auto-create the feature branch — `/speckit-specify` emits:

    [specify] Warning: Git repository not detected; skipped branch creation

This is expected, not an error: Spec Kit tracks the feature by directory name (`specs/NNN-<short-name>/`); the git branch is **your** responsibility, and you MUST cut it yourself **before** running `/speckit-specify` so the feature work is isolated from `main`.

Procedure:

1. **Confirm you are on `main`** — `git branch --show-current` MUST print `main`. If you are already on a feature branch, do NOT branch again; go straight to `/speckit-specify`.
2. **Cut the branch from `main`**, named `NNN-<short-name>`:

       git checkout -b NNN-<short-name>     # e.g. 008-topic-registry

3. Then run `/speckit-specify <description>`.

Make the branch's short-name match what Spec Kit derives from your description so the branch and the `specs/NNN-<short-name>/` directory line up. If they diverge it's harmless — the directory name is the canonical feature identifier; the branch is for git/PR workflow only.

**Next-up branches** (per the two active features above):

- `004-node-event-loop` — the node event-loop refactor + connection model (project author).
- `008-topic-registry` — the mock topic registry (co-developing architect).

Per the parallel-work plan in `specs/event-loop-and-registry-contract.md`, the refactor (004) lands to `main` first; 008 then branches from the updated `main` so `Event` / `EventQueue` / `spawn_producer` already exist. If both must start from today's `main` at once, see the "seam commit" note in that doc.

## Commit discipline (Constitution §3)

- Every commit MUST compile and pass all non-ignored tests.
- Commits MUST be logical increments leaving the repo at a functional checkpoint.
- Skipped/ignored tests MUST carry a reason and a tracking issue or ADR reference.
