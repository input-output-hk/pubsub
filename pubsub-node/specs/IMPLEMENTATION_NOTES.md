# pubsub-node — implementation notes to revisit

**Purpose**: a running list of implementation questions that surfaced during pre-spec discussion of a feature but were deemed out of scope for that feature. Each entry records the question, the working answer (if any), and the trigger condition for revisiting.

Workstream-level (not feature-scoped). Sibling to `ROADMAP.md`. Migrated into a feature's spec when the trigger condition fires.

---

## N-001 — Local emission vs local receipt in `received_messages()`

**Surfaced during**: 002 (topic-subscription filtering) pre-spec discussion.

**Question**: when a Node's own send / emission path is invoked to publish a message `M`, does `M` appear in that Node's local `received_messages()` snapshot? I.e., is local emission also a local receipt?

**Working answer (002 scope)**: **No.** A Node does not see its own published messages in `received_messages()` unless a peer forwards them back. The snapshot is strictly inbound-from-the-network.

**Why deferred**: this question only becomes operationally interesting when there's an external admin / REST API driving the Node — an operator publishing through such an API would plausibly want a confirmation that "the message was accepted into the local view". 002 has no such API surface; the send path is invoked from within the same process, so a separate confirmation snapshot adds clutter without value.

**Trigger to revisit**: when a Node-facing REST / admin API is introduced. Until then the "strictly inbound" snapshot semantics hold.

---

## N-002 — Self-addressing semantics under connection-based communication

**Surfaced during**: 002 (topic-subscription filtering) /speckit-clarify Round 2 review.

**Question**: when a Node emits a message addressed to its own peer id, does the message reach the Node's own receive path? In 002 (in-memory, registry-routed), the answer is yes — the InMemoryNetwork's loopback routes the message through, subscription filtering applies, and a subscribed Node observes the message in `received_messages()`. The spec records this as an Edge Case bullet for 002.

**Working answer (002 scope)**: Self-addressing is a legitimate inbound delivery via the network's loopback. FR-005 ("network unchanged"), FR-009's "deliveries arriving through the receive path are valid receipts" carve-out, and the absence of any `from == to` short-circuit in `src/network.rs:50–72` together imply this behavior.

**Why deferred**: in connection-based transports (TCP in feature 009; the connection-oriented model in feature 004), "a connection to self" is operationally a different beast. Some transports refuse the self-connect; others permit it but it loops through OS networking; some applications model it as a no-op. Whichever model emerges, the self-addressing semantics defined here for the in-memory pipe may not survive unchanged.

**Trigger to revisit**: when feature 004 (connection-oriented network model) lands. The connection-lifecycle ADR for that feature should explicitly address self-connections; the receive-path filter behavior may need to be re-examined alongside it.

**Resolved (004-connections)**: self-connections are unrepresentable end to end. The connection-selection strategy never selects self (candidates exclude the node's own id — FR-009, SC-007), and a control message whose carried emitter is the node itself is dropped (`self_emitter`, FR-015), so a node can never hold an Active upstream to itself. The payload receive path is now connection-gated (FR-016): a self-addressed payload — even one delivered through the in-memory loopback — finds no Active self-upstream and is dropped `not_connected`. The residual frame-vs-emitter identity-binding question is a distinct, security-flavoured deferral, now tracked in N-013.

---

## N-003 — Arrival-time chain validation under registry availability

**Surfaced during**: 003 (message envelope + mock crypto) pre-spec discussion.

**Question**: at message arrival, which validations beyond signature checking should be performed before the message is appended to the arrival log?

**Working answer (003 scope)**: **Signature-only at arrival.** Any signature-valid message is appended to the arrival log. Chain-relatedness facts — inconsistencies between message pairs (e.g., consecutive by hash but not by sequence, or vice versa), gaps in sequence numbers, and equivocation proofs (two valid-signature messages with the same `(publisher, parent_hash)` and different content) — are computed on demand via stateless queries in a `chain` module over the log. Arrival-time validation does **not** consult prior messages and does **not** drop messages based on chain inconsistency at this iteration.

**Why deferred**: until the topic registry (feature 008) is incorporated, the Node cannot validate that a `publisher_id` is authorized to publish to the claimed topic. Arrival-time enforcement of publisher legitimacy, topic-authorization checks, and policy-based responses to inconsistent or equivocating publishers all depend on registry-driven trust data and on protocol definitions (catch-up, misbehavior punishment) that don't exist yet. Per the staged-design-synthesis's "be permissive in early stages" framing, the system records what arrives with valid signatures and lets observational queries surface chain-integrity facts for future consumers to interpret. Baking arrival-time policy in 003 risks shipping logic that has to be ripped out once protocols mature.

**Trigger to revisit**: when feature 008 (mock registry abstraction) lands, and again when 012 (real on-chain registry feed) lands. At each point, re-examine:

1. Whether arrival-time validation should additionally drop messages whose `publisher_id` is not authorized for the claimed topic per the registry.
2. Whether detected inconsistencies between signed message pairs from the same publisher should produce a misbehavior report / proof, what surface delivers it (callback, channel, query), and what action the Node takes (continue accepting, blacklist publisher, propagate proof to peers).
3. Whether equivocation proofs warrant a distinct surface from generic "inconsistency" reports, given that a publisher signing two contradictory messages is unambiguously misbehavior with no benign explanation.
4. Whether catch-up / replication protocols want gap reports delivered push-style (stream, channel) rather than pulled via on-demand query.

The 003 feature spec should record an explicit note pointing back to this entry so the revisit trigger isn't lost across sessions.

**Partially resolved (013 — topic registry)**: revisit item **1** (drop messages whose `publisher_id` is not authorized for the claimed topic per the registry) is **closed** by feature 013 (ADR 0016, spec FR-015): on the inbound signed-message path the node drops a message whose publisher key is not in its topic's non-empty authorized-publisher set (open topics — empty set — accept any), checked before signature verification. Items **2–4** (equivocation proofs / misbehavior reports, parent-hash + sequence chain-integrity, gap/catch-up reports) and deposit/anti-Sybil remain deferred to **012** (the real on-chain feed), where chain history and the full topic-registry contract exist.

---

## N-004 — Canonical encoding for envelope bytes (signing + hashing)

**Surfaced during**: 003 (message envelope + mock crypto) pre-spec discussion.

**Question**: which byte encoding does the signature cover, and which encoding feeds the hash function that produces `MessageHash` (consumed by the next message's `parent_hash`)?

**Working answer (003 scope)**: **Hand-rolled length-prefixed concatenation**, exposed as a single helper `PlainMessage::signed_bytes(&self) -> Vec<u8>` (the method lives on `PlainMessage`, not `Message`, per ADR 0010). Approximate shape:

```text
len_u32(topic)        || topic_bytes
|| len_u32(publisher_id) || publisher_id_bytes
|| parent_hash_bytes (32 bytes, all-zeros sentinel when absent)
|| sequence.to_be_bytes()  (u64, big-endian)
|| timestamp.to_be_bytes() (u64, big-endian)
|| len_u32(payload)      || payload_bytes
```

No leading version byte. In 003 the same Rust code base produces and verifies signatures over the same in-memory `Message` struct (the in-memory network passes Rust objects, not serialized bytes), so cross-version interop is not yet a concern; a version distinguisher would only ever differentiate two different builds of `TestSigner` / `TestVerifier`, which is a test-only artefact. When the encoding swaps to CBOR canonical at feature 009, version identification rides on the CBOR scheme itself rather than an envelope-level tag.

`MessageHash` is a fixed 32-byte newtype (`MessageHash([u8; 32])`) over SHA-256 of those bytes. The same SHA-256 input feeds both the signing operation and the hash that becomes the next message's `parent_hash`.

The `parent_hash` field on `Message` is typed `Option<MessageHash>` at the Rust API surface (idiomatic absence for the publisher's first message), but is encoded into `signed_bytes` as a fixed-width 32-byte field — using `MessageHash::ZERO` (`[0u8; 32]`) as the sentinel for `None`. The two layers are independent: the encoder does `parent_hash.unwrap_or(MessageHash::ZERO).as_bytes()` at the boundary. Rationale: Rust pattern-matching catches the first-message case at compile time; fixed-width encoding removes a branch in the byte producer, makes `signed_bytes` length deterministic given the other field lengths, and matches the standard shape used by Bitcoin / Cardano / Ethereum hash chains for genesis/coinbase parents. SHA-256 collision into all-zeros is 2^-256, i.e., not a practical concern.

**Coding guidance for an easy future swap**: the canonical encoding lives in **exactly one place** — the `signed_bytes` helper. The `Signer` / `Verifier` trait impls and parent-hash computation both call through this single function. Concretely:

- `signed_bytes(&self) -> Vec<u8>` is the single seam. Swapping the body from hand-rolled to CBOR-canonical is the entire migration.
- Callers never construct the canonical bytes themselves. They always go through `signed_bytes`.
- `MessageHash` stays a fixed-width 32-byte newtype regardless of encoding choice — only the bytes fed *into* SHA-256 change at the swap, not the hash output type.
- The `Signer` / `Verifier` traits take `&[u8]` for the message argument (per ADR 0009), so the trait shape stays the same — only the byte producer changes.
- The `PlainMessage::signed_bytes` rustdoc MUST document the byte layout in full (field order, widths, endianness, the `MessageHash::ZERO` sentinel for absent `parent_hash`). The docstring is the canonical reference for what the signature covers — anyone implementing a new `Signer` / `Verifier` (real Ed25519 in 011, a different transport encoding in 009) reads it to confirm the bytes they hash match the bytes the verifier expects. Treat the rustdoc as part of the protocol surface; changes to the encoding require a rustdoc update in the same commit.

**Why deferred**: hand-rolled is the cheapest path for a prototype with no cross-process or cross-language consumers yet. A real network transport (TCP in 009) plus any cross-language publisher integration is when ecosystem-standard determinism (CBOR canonical per RFC 8949 §4.2.2) earns its dep weight: well-specified canonicalisation, human-readable diagnostic notation for on-the-wire debugging, and interop with Haskell / TypeScript / etc. signers if those become part of the system.

**Trigger to revisit**: when feature 009 (TCP transport) lands, OR when cross-language publishers / verifiers become part of the system, whichever comes first. At that point:

1. Swap the `signed_bytes` body to CBOR canonical (likely `ciborium`, with `serde_cbor` as a fallback if `ciborium`'s canonical-mode coverage is incomplete for our envelope shape).
2. Consider whether the on-the-wire encoding and the signing-input encoding should be the same (probable yes — one encoding to vet, one set of test vectors).
3. Identify the encoding version on the wire / in storage via the CBOR scheme's own conventions (e.g., a top-level CBOR tag, a schema-version field inside the CBOR map, or framing-level metadata when 009 lands). Signed messages produced under the hand-rolled encoding will not verify under the new encoding; at prototype-stage with no persisted-history requirement this is acceptable, but document the discontinuity.

The 003 feature spec should record an explicit note pointing back to this entry so the revisit trigger isn't lost across sessions.

---

## N-005 — `MessageHash` input: `PlainMessage` content vs `SignedMessage` full form

**Surfaced during**: 003 (message envelope + mock crypto) post-`/speckit-plan` design discussion on the protocol-message type hierarchy (ADR 0010 drafting round).

**Question**: should `MessageHash::of` consume `&PlainMessage` (hashing the signed-over content only) or `&SignedMessage` (hashing content plus signature)?

**Working answer (003 scope)**: **`MessageHash::of(plain: &PlainMessage)`** — hash the content only; the signature is excluded from the hash input. The `parent_hash` chain is content-anchored. Reasoning (full version in ADR 0010's Consequences section):

- **Signature-malleability immunity.** Chain stays valid across signing-scheme changes (feature 011's Ed25519 swap; any later scheme migration) and across non-deterministic signing schemes. The canonical Bitcoin pre-SegWit lesson: ECDSA signature malleability broke transaction addressing because `TXID = hash(body || signature)`. Hashing the body (only) avoids the failure mode.
- **Cardano ecosystem alignment.** Cardano's `tx_hash = blake2b(tx_body)` hashes body and witnesses separately. The pubsub-node lives in the Cardano workstream and aligning the convention has zero cost.
- **Content addressing.** `MessageHash` represents "the identity of what this publisher committed to" — content-level, not witness-level.
- **Cross-scheme stability.** Future signing-scheme transitions don't break existing chains' validity.

**Why deferred**: 003 itself does not operationally consume `MessageHash` — no chain validation (deferred per N-003), no replay detection, no wire-level dedup. The function exists for downstream features (replication / catch-up; chain-integrity validation once it lifts post-008 / 012; any future dedup or caching layer). The decision shapes how those future features interpret the hash, but does not affect 003's runtime behavior.

**Trigger to revisit**: when a feature first **operationally consumes** `MessageHash`. Specifically:

1. **Chain-integrity validation** (feature 008 / 012, once N-003 lifts): the validator compares an incoming message's `parent_hash` against `MessageHash::of(&previous.plain)`. Confirm the option-(a) shape works for malleability-resistance and content-stable chain extension. Also confirm the equivocation-detection logic (two valid-signature messages with the same `(publisher_id, parent_hash)` but different `plain` content) is testable under content-hashing semantics.
2. **Replay / dedup logic** (any future caching, retransmission, or dedup feature on the network or application layer): determine whether the content hash is sufficient or a separate wire-level hash over `SignedMessage` is needed. If signature-different / content-identical messages need to be distinguished by hash (e.g., to track who-re-signed-what), introduce a sibling `WireHash` type alongside `MessageHash`. The two coexist; neither replaces the other.
3. **Future signing-scheme changes** (post-feature 011 Ed25519 swap, if a future scheme is introduced — e.g., a BLS variant for signature aggregation, or post-quantum schemes much later): verify that chains produced under the previous scheme are still valid under the new scheme. Option (a) guarantees this; option (b) would force a chain-format migration.

At each trigger, weigh whether option (a) still serves, or whether a parallel "wire hash" / "full-form hash" is needed alongside it. The 003 feature spec's FR-011 and ADR 0010 capture the current rationale; this entry tracks the revisit obligation.

## N-006 — Construction-failure integration test (duplicate registration)

**Surfaced during**: 004 (node event-loop refactor) checklist walk (CHK018/CHK019 in `specs/004-node-event-loop/checklists/refactor.md`).

**Question**: 004's FR-016 pins construction-failure parity — a failed `Node::new` (e.g. `DuplicateRegistration` of an already-taken id) surfaces the existing typed error and leaves no background activity running. The success path is exercised by every integration test (15 `Node::new` call sites), but **no test anywhere triggers the failure path**: `DuplicateRegistration` exists only as the error definition (`src/error.rs`) and the `register` guard (`src/network.rs`). The failure path is also unreachable from the CLI quickstarts — separate processes own separate `InMemoryNetwork`s, so the collision is library-level only.

**Working answer (004 scope)**: do **not** add the test in 004. The refactor's contract is "no new behavior is added or newly tested" (behavioral parity; existing suite passes unmodified); adding a new integration test inside the parity feature would itself trip later consistency passes and force a cascade of rewordings. FR-016's failure clause is verified in 004 by structural review (registration precedes any task spawn in `Node::new`, so a failed construction has nothing to leak — same review-based verification as the CHK007 drop-abort ruling).

**Trigger to revisit**: **feature 004-connections** (the follow-on that reshapes construction and the dial/accept paths). Add a proper integration test there: construct a node, attempt a second construction with the same id on the same network, assert the typed error (`NodeError` from `NetworkError::DuplicateRegistration`) — and extend it to whatever construction/dial failure modes the connection model introduces. 004-connections touches exactly the constructor region whose ordering currently makes the no-leak property true, so the test lands where the risk does.

**Resolved (004-connections, T016)**: `tests/connections.rs` covers the construction-failure path. `construction_fails_on_duplicate_registration` asserts the typed `NodeError::Network(NetworkError::DuplicateRegistration)` on a second `Node::new` for the same id, and that the failed attempt left the id free for a later coherent construction. `construction_fails_on_identity_mismatch` covers the connection model's new failure mode — `NodeError::IdentityMismatch`, checked **before** network registration (FR-024, ADR 0017) so nothing leaks.

## N-007 — `peers` placement: shell field today, `NodeState` when a transition consumes peer data

> **Closed (016, 2026-07-17)**: the config bootstrap `peers` list (and the
> whole TOML node-config subsystem, `PeerDescriptor`/`BasicPeerDescriptor`,
> `Node::peers()`) was removed — ADR 0033. Registry-derived candidates fully
> replaced it; the anticipated dialer consumer never materialised.

**Surfaced during**: 004 (node event-loop refactor) checkpoint-2 review — maintainer asked whether the static `peers` list should live in `NodeState` alongside the rest of the node's state.

**Question**: `Node.peers` (`Vec<BasicPeerDescriptor>`, config-derived) stayed on the shell when 004 consolidated mutable state into `NodeState`. Shouldn't peer knowledge be part of the node's state value?

**Working answer (004 scope)**: **No — deliberately shell-resident.** Three reasons:

1. **Parity**: `Node::peers()` returns `&[BasicPeerDescriptor]` (a borrow). Inside `Arc<Mutex<NodeState>>` that signature is unimplementable (cannot borrow out of a dropped `MutexGuard`); the getter would have to become a clone-out `Vec` — a public-API change 004's parity contract (SC-004, contracts §A) forbids.
2. **The `NodeState` rule**: the struct holds *mutable, transition-relevant* state (FR-008, CHK021). Today's `peers` is neither — static for the node's lifetime (001 contract: no mutation API) and consulted by no `apply` transition.
3. **Wrong shape to lock in**: ROADMAP 005 replaces the config list with a per-topic `PeerView` behind a `PeerSource` trait; pre-moving today's `Vec` would lock the wrong shape into the core just before the feature that defines the right one.

**Trigger to revisit**: **feature 008** (first transition that writes peer/registry-derived data: the `Event::MembershipUpdate` arm makes that knowledge mutable, transition-written state → it belongs in `NodeState` per the same rule that excludes today's list), and **feature 005** (the `PeerView` the epochal dialer reads). The seam contract §1.1 already reserves the slot ("registry-derived state and logical connection/peer metadata land here later"); this note makes the revisit obligation explicit. The static config-derived list may remain a shell field even then if nothing transitions it — the rule is "what `apply` reads or writes lives in `NodeState`", not "everything peer-shaped".

**Resolved (008)**: the per-topic candidate set (`HashMap<TopicId, HashSet<PeerId>>`, folded by `handle_membership_update`) now lives in `NodeState`; the static config `[[peers]]` bootstrap list stays a `Node` shell field (nothing transitions it). Two distinct sources, per ADR 0014.

## N-008 — Restart-time registration-state recovery under a persistent / on-chain registry

**Context**: In the in-memory mock (008), a node's topics come from its subscription-registry entry, looked up at startup (`entry(self_id)`); there is no persistence, and the node performs no registry writes. A node that restarts simply re-reads its entry from the shared registry.

**Deferred**: Under a real on-chain subscription list (feature 012), a restarting node must recover its registration state from the chain — confirm its own entry is live (the operator's registration may lag the tip) and resume gap-free. `joining.md` step 3 specifies the faithful behaviour: read the list, look up the node's own pubkey, **retry with exponential backoff** (warn → error escalation) until the entry appears. The mock instead **fails fast** when the entry is absent (spec FR-018 / `/speckit-clarify` 2026-06-10); the retry/recovery path is out of scope here.

**Trigger to revisit**: feature 012 (the on-chain registry feed), where chain lag makes read-and-wait the correct startup behaviour and a persisted prior registration becomes recoverable.

## N-009 — Identity unification: topic-registry publisher key ≡ subscription-list node id

**Surfaced during**: 013 (topic registry) design review (2026-06-11).

**Question**: the topic registry's authorized publishers are `PublicKey`s (formal model `publishers: Set[PublicKey]`; the message `publisher_id` wraps a `PublicKey`, consulted on verification). The subscription list (008) identifies nodes by `PeerId`. Are these the same identity?

**Working answer (013 scope)**: In the **protocol** they are the same identity space — a node is identified by its **pubkey**, which is both its subscription-list key and (when it publishes) its authorized-publisher key. In the **current mock** they are distinct Rust types: `PeerId` is an opaque `String` (002/008), `PublicKey` is `Vec<u8>` bytes (003). 013 therefore keys authorized publishers by `PublicKey` (matching the formal model and the verification path), **not** by `PeerId`. No translation layer is introduced — the message's publisher is already a `PublicKey`, so authorization is a direct set membership check.

**Why deferred**: unifying `PeerId` and `PublicKey` is feature **011**'s concern (real Ed25519; "`PeerId` grows to carry a public-key fingerprint", ROADMAP open-question 3). Forcing the unification now — before real crypto — would be premature and would touch 002/003/008 identity surfaces for no 013 benefit.

**Trigger to revisit**: feature **011** (real crypto / identity model). At that point, confirm that the subscription list's node id and the topic registry's publisher key resolve to one identity type, and decide whether authorization is expressed against `PeerId`, `PublicKey`, or a unified identity newtype. Until then, 013 keys publishers by `PublicKey` and the subscription list keys members by `PeerId`, with the node folding both independently.

## N-010 — In-memory network has no deregistration; a literal same-alias restart is not expressible

**Surfaced during**: 004-connections Phase 6 (US4 graceful shutdown & restart recovery), writing the T026 integration tests.

**Question**: US4-AS4 / SC-004 describe a node that drops abruptly and then **restarts under the same identity**, whose re-dial is idempotently re-accepted by the survivor, returning it to `Active`. Can this be exercised end to end as an integration test on the in-memory network?

**Working answer (004-connections scope)**: **No — not a literal restart.** `InMemoryNetwork` has no deregistration: `register` inserts the peer's mailbox sender into a registry map and **nothing removes it** — neither `NetworkHandle`/`Node` `Drop` (which only aborts tasks) nor any explicit teardown. So a dropped node's id stays registered, and reconstructing a node under the same alias returns `NetworkError::DuplicateRegistration`. The two **expressible** US4 flows are tested in `tests/connections.rs` (graceful shutdown clears the survivor's entries; abrupt drop leaves stale entries). The **healing mechanic** restart relies on — a counterpart idempotently re-accepting a re-dialing peer's duplicate `Request`, keeping the entry and re-sending `Accepted` — is covered at the state level by `duplicate_request_idempotent_then_stale_on_failed_revalidation` in `src/state.rs`.

**Why deferred**: adding deregistration to the mock (`Network::deregister`, or freeing the slot on `NetworkHandle` drop) is a transport-shape decision that belongs with the real connection-oriented transport, where node departure/restart and slot reuse have concrete semantics (a TCP peer that disconnects frees nothing in a registry by itself; reuse is an application/registry concern). Baking a deregistration API into the in-memory mock now — solely to make one integration test literal — would lock a shape before the feature that defines it, for no behavior the state-level test does not already cover.

**Trigger to revisit**: when real node restarts are defined — feature **009** (real transport: connection teardown / reconnection) or **011/012** (persistent identity + on-chain registration recovery, cf. N-008). At that point decide whether the network grows a deregistration/slot-reuse API, and if so, promote the restart-recovery flow to a literal end-to-end integration test (drop → re-register same id → re-dial → idempotent re-accept → `Active`). Until then the mechanic is verified at the state level and the in-memory restart stays inexpressible by design.

---

The next five entries are 004-connections' deferred-dynamics package — the deliberate non-reconciliations the connection model ships, each mapped to its row in the feature's data-model staleness catalog (`specs/004-connections/data-model.md` §3, rows S1–S7). All are safe to defer for the same reason the catalog records: a stale entry only ever *admits* traffic it already would, or *sends into drops* — none creates traffic or corrupts the received record.

## N-011 — Stale-`AwaitingAccept` GC and peer-membership-drift removal

**Surfaced during**: 004-connections (logical connections) — data-model staleness catalog **S1** (stuck `AwaitingAccept`) and **S5** (peer-membership drift, both roles).

**Question**: when do connection entries that selection no longer expects get **removed**? A request to an absent peer pins an `AwaitingAccept` upstream indefinitely (S1); candidates that shrink between setups leave held pairs (upstream and downstream) for ex-members untouched (S5).

**Working answer (004-connections scope)**: **No removal.** Selection only ever *adds* (FR-007: "expected-set membership never removes anything"); a recurring setup re-dials pending pairs but prunes nothing. Stuck/`AwaitingAccept` entries stay visible diagnostics (admit nothing); drifted entries persist (their payload is still gated by subscription/registration/severance). Removal is the connection set becoming **dynamic**, explicitly out of scope (FR-027). This is the **peer-side** of membership-loss-retains-connections; the **self-side** (the node's *own* unsubscribe likewise retaining its connections) is [[N-019]]. Both contrast with the topic-`Removed` **cascade** ([[N-017]]), which does tear connections down — the three should converge on one teardown rule when dynamic transitions land.

**Trigger to revisit**: the **dynamic-connection-transitions** feature (re-selection on membership change, GC of stale `AwaitingAccept`, removal of no-longer-expected pairs). At that point selection gains a remove side and the diff stops being add-only.

## N-012 — Active-connection liveness / heartbeat

**Surfaced during**: 004-connections — data-model staleness catalog **S2** (one-sided connection after an acceptor's abrupt restart) and **S3** (survivor-side stale entries after an abrupt drop).

**Question**: how does a node discover that an `Active` upstream has gone silent — the counterpart restarted abruptly (losing its downstream) or vanished without a `Terminated`? The survivor holds a permanently quiet `Active` entry that its own add-only diff will never re-request.

**Working answer (004-connections scope)**: **No liveness probing.** Graceful `shutdown` sends `Terminated` for every entry; the abrupt path sends nothing and the survivor keeps stale entries (FR-021, accepted v1 state). The requester-restart re-dial heals the restarted node's *own* direction via idempotent re-accept; the survivor's quiet `Active` entry is not healed. No heartbeat exists (FR-028).

**Trigger to revisit**: feature **009** (real transport + liveness/heartbeat), where connection liveness is probeable and a dead `Active` entry can be detected and reaped.

## N-013 — Handshake identity-binding hardening (frame sender vs signed emitter)

**Surfaced during**: 004-connections — ADR 0017 §4 and spec Assumptions (the transport frame's sender is *trusted as delivered*); FR-028 (no identity-binding hardening). Not a staleness-catalog row — a security-hardening deferral.

**Question**: control-message handling keys entirely on the **carried emitter** (verified by signature), and no cross-check is performed between the transport frame's sender and that emitter. Under a real transport with spoofable frames, should the two be reconciled?

**Working answer (004-connections scope)**: **No cross-check.** The in-memory network stamps frames itself, so a frame cannot misattribute its sender (spec Assumptions); the control path uses the carried emitter, the payload path uses the frame's delivering peer (a payload carries a publisher identity, not the sender's), and that asymmetry is deliberate (FR-011/FR-016). Mock crypto binds identity only symbolically anyway (the 003 caveat).

**Trigger to revisit**: feature **011** (real crypto) and/or **009** (real transport), where frame tampering enters the threat model. Decide whether a frame-vs-emitter cross-check, or authenticated transport framing, is required, and where it sits relative to signature verification.

**Updated (015, post-round-5 — the owner-binding removal)**: the stakes are now recorded as a **transport obligation**, not just a hardening question. On the dissemination path the unsigned frame sender does real work: it keys the **receive-gate lookup** (which Active upstream admits) and it selects the **admitting link for signature-failure severance** — so under a transport without per-hop sender authentication, a spoofed frame could get a message admitted via another peer's link or get an innocent peer's link severed. The 015 review also removed the one check that leaned on this field *as enforcement* (the publisher-link owner-binding compared the signed `publisher_id` against the frame sender — unsound for exactly this reason; ADR 0032 §5 supersession note). The recorded rule: a receive-side **restriction** must be checkable from the signed bytes alone; frame-sender **authentication** (so the routing/severance uses of `from` stay sound) is an obligation on the real transport (009/011).

## N-014 — Misbehavior follow-ups: blacklist, re-selection, topic-scoped misbehavior

**Surfaced during**: 004-connections — data-model staleness catalog **S6** (misbehavior asymmetry).

**Question**: severance removes only the receiver's *upstream* entry and is silent — the offender keeps its downstream and keeps sending into `not_connected` drops; nothing blacklists it or prevents re-establishment, and misbehavior is signature-only (topic-scoped misbehavior is not modelled). What consumes the `Effect::Misbehaved` signal beyond a log line?

**Working answer (004-connections scope)**: **Log only.** The misbehavior signal is surfaced as `connection_severed` (warn) and otherwise unconsumed; `Effect::Misbehaved` exists precisely so a future blacklist can consume it without reshaping `apply`'s output (the spec's stated forward intent). No deny path, no re-selection avoidance, no topic-scoped misbehavior (FR-018, FR-027).

**Trigger to revisit**: the **misbehavior/deny-path** package (blacklist that consumes `Effect::Misbehaved`; re-selection that avoids blacklisted peers; a `Rejected` control message when a deny path exists; any topic-scoped misbehavior model). Returns with dynamic transitions and the deny-path work.

## N-015 — Acceptance validates membership only, not topic registration (cross-registry ordering) — **RESOLVED by 014-registry-consistency (2026-06-17)**

**Resolution**: 014 adopts the cross-registry chain-order invariant as a **maintained `NodeState` property** with strict drop: the node enforces `subscriptions ⊆ registered_topics` (and the same for candidate topics) by dropping membership events for unregistered topics, gated by a construction-time readiness signal so the topic projection is warm before membership folds. Consequently an unregistered topic is **never** in the subscription/candidate sets, so a connection `Request` on it fails membership validation and is **rejected** — acceptance is now consistent with registration, closing the S7 connection-level exposure. A topic-registry `Removed` additionally **cascades** into the `upstream`/`downstream` structures (ADR 0020; 014 FR-002/FR-010). No standalone registration check was added to the acceptance path — strict drop makes the unregistered-topic case unreachable. The historical context is retained below.

**Surfaced during**: 004-connections — data-model staleness catalog **S7**; spec Clarifications 2026-06-12 (post-013 reconciliation). Revisit-flagged at planning.

**Question**: connection acceptance validates the **membership-derived** subscription set only (FR-012) — it does **not** require the topic to be registered in the topic registry. So connections can establish and persist on a topic the registry does not recognise (e.g. one deregistered because a publisher key was compromised); their payload delivers nothing (013's `topic_not_registered` gate), but the connection-level exposure is real.

**Working answer (004-connections scope)**: **Membership-only acceptance**, deliberate and revisit-flagged. The maintainer's preferred resolution is a **cross-registry event-ordering invariant**: both registries are chain-derived, and a faithful follower delivers their events in chain order, making membership ⊆ registered *structural* (raised on the 013 PR). Until that invariant is adopted, acceptance is intentionally inconsistent with the active-topics set, with delivery still gated by registration.

**Trigger to revisit**: resolution of the **cross-registry chain-order invariant**. If adopted, membership ⊆ registered holds structurally and no acceptance change is needed. If rejected, acceptance must gain the registration check (validate against `subscriptions ∩ registered_topics`) **or** topic removal must cascade into membership so an unregistered topic loses its members.

## N-016 — Signature domain separation across message kinds

**Surfaced during**: PR #56 review (004-connections), 2026-06-15 — raised by the reviewing architect.

**Question**: the node signs control messages (`PlainConnection`) and dissemination messages (`PlainMessage`) with the **same** key shape, but neither `signed_bytes()` encoding carries a tag committing to *which kind* it is. The two are distinguished only because their byte layouts happen not to collide — an implicit, unproven property. Could a signature produced for one kind be replayed by re-wrapping the bytes in the other `Message` variant?

**Working answer (004-connections scope)**: **No domain tag; deferred.** A practical reuse attack is not reachable at this stage:

- The node signs **only control messages** it creates; it does not sign dissemination messages at all (payload messages carry a *publisher* signature, and `Node::send` only routes). So the "one key signs both layers" premise does not hold in the current code.
- A reuse would require a **structured-layout collision** between `PlainMessage::signed_bytes()` and `PlainConnection::signed_bytes()` (e.g. forcing a topic field to contain raw public-key bytes with matching `u32` length prefixes); the victim does not control the attacker's target content, so engineering the collision is infeasible.
- Even a forged dissemination message still faces the full receive gate (connection → subscription → registration → authorization → signature), so the blast radius is near-nil.
- Everything is shared as in-memory objects over the mock network; there is no wire format yet.

**The fix, when it lands** (the architect's concrete proposal): prepend a per-kind **domain tag** (e.g. a 1-byte `0x01` dissemination / `0x02` control, or a short context string) at the start of each `signed_bytes()`, so the signature commits to the message kind and a re-wrapped message recomputes to the wrong domain and fails verification **by construction**. The PR #56 refactor that lifted `push_len_prefixed` into a single crate-internal helper (`src/message.rs`) leaves this a one-touch change — the tag is written first in each encoder (and the helper's rustdoc points here).

**Why deferred**: domain separation is a **canonical-encoding** concern and belongs with the same decision as the CBOR-canonical swap, i.e. the first real serialization / cross-language consumer — exactly N-004's milestone. Introducing a hand-rolled tag now would be re-specified under the CBOR scheme anyway and would amend the protocol layout docs (ADR 0010, contracts §1.1) for a property that is not load-bearing at PoC.

**Trigger to revisit**: with **N-004** (canonical encoding swap) — feature **009** (TCP transport) or the first cross-language publisher/verifier, and/or **011** (real crypto). At that point the domain tag rides on the canonical scheme: the signed bytes for each message kind must be unambiguously self-describing so cross-protocol signature reuse fails by construction, not by layout coincidence.

## N-017 — Topic-removal cascade tears down connections without notifying peers

**Surfaced during**: 014-registry-consistency PR review (2026-06-18). Relates to ADR 0020's atomic cascade (014 FR-002/FR-010).

**Question**: `handle_topic_registry_update`'s `Removed` arm clears the topic from `subscriptions`, `candidates`, `upstream`, and `downstream` in one fold, but returns no `Effect` — no `ConnectionAction::Terminated` is sent to the affected peers. A downstream peer keeps believing it holds a live connection and may keep forwarding on the topic; an upstream peer is never told this node has stopped consuming. This is asymmetric with `handle_shutdown`, which emits `Terminated` for every entry.

**Working answer (014 scope)**: **Silent local teardown.** No delivery-correctness impact — now-unregistered traffic is dropped at the receive path's `topic_not_registered` gate — but peer state goes stale. Consistent with 004-connections' accepted "abrupt path sends nothing" stance ([[N-012]]); v1 carries no liveness/notification obligation on removal.

**Trigger to revisit**: the **dynamic connection lifecycle** work — when a re-establish / garbage-collect mechanism (a timer or other) is added. Decide whether topic-removal teardown should emit `Terminated` to downstream (and cancel pending upstream `Request`s), reusing the shutdown notice path, or whether peer-side GC makes notification unnecessary.

## N-018 — Readiness (`synced`) gates dialing but not acceptance / receive

**Surfaced during**: 014-registry-consistency PR review (2026-06-18). Relates to ADR 0020's `Syncing → Synced` readiness lifecycle.

**Question**: the `synced` flag gates the node's **own dialing** (`handle_synced` → `handle_heartbeat`) but is not consulted by `handle_connection_request` (acceptance), the message receive path, or the publish path (`handle_publish` — a `publish` while `!synced` validates against the cold `subscriptions` set and drops `topic_not_subscribed`, benign and the same class as the receive case; 006-fanout-policy). Because the registry indexer and the network mailbox are independent producers on the single FIFO queue, an inbound `Request` that lands before the registry snapshots are folded is evaluated against cold `subscriptions`/`candidates` and silently rejected (`membership_validation_failed`), with no retry hint or deferral. The node knows it is still syncing but applies that knowledge only to dialing.

**Working answer (014 scope)**: **Readiness scoped to dialing only.** Invisible in the test suite (registries are populated before `Node::new` and triggers are deterministic, so snapshots always fold first). Not a regression — pre-014 there was no readiness notion and the same cold-state race existed. Accepted v1 state.

**Trigger to revisit**: the **dynamic connection lifecycle** work (re-establish / GC). When peers re-dial on a timer an early rejection self-heals; decide at that point whether acceptance (and the receive path) should also gate on `synced` — defer or reject-with-retry while `!synced` — or whether peer re-dial makes it moot.

## N-019 — Own-membership unsubscribe retains established connections (asymmetric with the topic-removal cascade)

**Surfaced during**: 006-fanout-policy rebase onto merged 014 (2026-06-18) — review of `handle_membership_update` against the topic-`Removed` cascade ([[N-017]]).

**Question**: when the node's **own** membership drops a topic — `MembershipEvent::TopicsChanged { removed }` or `Left` — the fold removes the topic from `subscriptions` (and `candidates`) but does **not** touch `upstream`/`downstream`, so connections already established on that topic persist as stale entries. This is asymmetric with `handle_topic_registry_update`'s `Removed`, which **does** cascade into `upstream`/`downstream` ([[N-017]]). Should a self-unsubscribe also tear down (and/or notify) connections on the dropped topic?

**Working answer (006 / current scope)**: **No teardown** — connections on a self-unsubscribed topic are retained. No delivery-correctness impact: inbound payload on the topic is dropped at the receive path's `topic_not_subscribed` gate, and the node will not publish on it (publish requires subscription), so a retained downstream is never fanned to. Consistent with the add-only, no-removal stance of [[N-011]] (selection only adds; removal is dynamic-connection work) and the stale-entry posture of [[N-012]]. The asymmetry with the topic-`Removed` cascade is deliberate-by-omission — documented here rather than reconciled now, to avoid touching the established connection structures outside the dynamic-connections feature.

**Trigger to revisit**: the **dynamic-connection-transitions** feature (with [[N-011]] / [[N-017]]). When selection gains a remove side, decide whether a self-unsubscribe should cascade into `upstream`/`downstream` (matching the topic-`Removed` cascade) and whether it emits `Terminated`, so the two membership-loss paths converge on one teardown rule.

## N-020 — `Synced` atomically triggers dialing (readiness coupled to establishment)

**Surfaced during**: 006-fanout-policy (US2 fan-out relay). Relates to ADR 0020's `Syncing → Synced` lifecycle and [[N-018]] (readiness gates dialing, not acceptance/receive).

**Question**: the `Synced` readiness transition runs `handle_heartbeat` atomically, so reaching readiness and dialing the connection policy's full expected-upstream set are a single, inseparable step — a node cannot become ready without immediately dialing. Should readiness and establishment be separable?

**Working answer (current scope)**: **Keep coupled.** Autonomous startup wants readiness to trigger the dial, and `Event::Heartbeat` remains separately injectable for any caller that needs to dial deliberately, so nothing is blocked. No change.

**Trigger to revisit**: the **dynamic-connection-transitions** feature (with [[N-011]] / [[N-017]] / [[N-018]] / [[N-019]] — the connection-lifecycle cluster). Reconsider whether `Synced` should only flip the readiness flag and emit `Event::Heartbeat` as a separate queued event, decoupling readiness from automatic dialing.

## N-021 — Bounded `seen` store (eviction) for duplicate suppression

**Surfaced during**: 006-fanout-policy (US3 dedup; data-model §7 D1).

**Question**: the `seen: HashSet<MessageHash>` that suppresses forwarding loops grows without bound — every accepted message's content hash is retained forever. A long-running node accumulates unbounded memory. Should it be a bounded store (LRU / TTL)?

**Working answer (current scope)**: **Unbounded.** Correct for the in-memory PoC — it keeps `apply` deterministic and the tests reproducible, and there is no PoC consumer that runs long enough to matter. An eviction policy is a deployment-tuning concern (window size, TTL) with its own correctness trade-off (an evicted hash re-admits a late duplicate).

**Trigger to revisit**: the **real-implementation** milestone (persistent / long-running node). At that point choose an eviction policy — likely TTL keyed on the message timestamp or an LRU sized to the dissemination window — and document the re-admission window it implies.

## N-022 — Pick-k / sampling fan-out strategy (deterministic RNG in state)

**Surfaced during**: 006-fanout-policy (US2 fan-out seam; data-model §7 D2).

**Question**: the v1 `ForwardToAll` forwards to every downstream peer on the topic. A scalable dissemination policy forwards to a random *k*-subset (degree cap), which needs a source of randomness. How is that introduced without breaking the deterministic `apply`?

**Working answer (current scope)**: **`ForwardToAll` only.** Pick-k would require a seeded RNG held in `NodeState` (so `apply` stays a pure, reproducible function of state + event), which is state shape this feature does not add. The `FanoutStrategy` trait is the insertion point — a future `PickK`/degree-cap strategy slots in behind it without reshaping the transition.

**Trigger to revisit**: **ROADMAP 006/007** (pick-k / golden-mode fan-out). Add a seeded RNG to `NodeState` (threaded deterministically, like any other state), implement the sampling `FanoutStrategy`, and keep the order-insensitive test convention (sort targets).

## N-023 — Equivocation / conflicting-message detection

**Surfaced during**: 006-fanout-policy (US3 dedup keys on content hash; data-model §7 D3).

**Question**: content-hash dedup suppresses *identical* copies, but an equivocating publisher emitting two **distinct** messages under the same `(publisher, sequence)` produces two different hashes — so both propagate and both are recorded. Should the node detect and act on the conflict?

**Working answer (current scope)**: **Not detected.** Distinct content ⇒ distinct hash ⇒ both disseminate; this is the documented out-of-scope stance (dedup is loop-prevention, not chain-integrity). Detecting `(publisher, sequence)` collisions with differing content is a separate validation concern.

**Trigger to revisit**: **feature 012** (chain-integrity / equivocation), with [[N-003]] (arrival-time chain validation). Decide the detection key (`(publisher_id, sequence)` or `(publisher_id, parent_hash)`) and the response (drop / slash / blacklist).

## N-024 — `Message::Signed` → `Message::Dissemination` rename — **RESOLVED by the connection-acceptance-strategy refactor (2026-06-23)**

**Resolution**: the dissemination variant is renamed `Message::Signed` → `Message::Dissemination` (and its named handler `handle_signed_message` → `handle_dissemination`) in the dedicated low-risk refactor pass anticipated below. The inner `SignedMessage` / `PlainMessage` types are **unchanged** — the rename is variant-only, and `SignedMessage` remains accurate (it is the signed dissemination message). ADR 0010 carries an amendment recording the rename (body examples kept as the point-in-time record). Behavior-preserving. The historical context is retained below.

**Surfaced during**: 006-fanout-policy (data-model §7 D4).

**Question**: the dissemination payload variant is still named `Message::Signed` (with `SignedMessage` / `PlainMessage`), a name from before connection-control messages were also signed. It now reads as if it were the only signed kind. Rename to `Message::Dissemination`?

**Working answer (current scope)**: **Deferred.** A purely mechanical rename touching every dissemination call site; bundling it into a behavioural feature would inflate the diff and obscure the functional change. Left for a dedicated rename pass.

**Trigger to revisit**: any time a low-risk mechanical-refactor pass is scheduled (or opportunistically alongside the next change that already touches the message type hierarchy — ADR 0010).

## N-025 — Epochal / periodic re-dialer

**Surfaced during**: 006-fanout-policy (out of scope; data-model §7 D5).

**Question**: connection establishment fires once (the `Synced` readiness dial, or an injected `Event::Heartbeat`). Nothing re-selects or re-dials on an interval, so a node that missed a peer (absent at sync, or a dropped request) never retries autonomously. Should there be a periodic re-dial?

**Working answer (current scope)**: **No periodic re-dial.** `Event::Heartbeat` is idempotent and re-injectable (a recurring setup re-dials pending pairs, skips Active ones — [[N-011]]), so the *mechanism* exists; only the periodic *trigger* is absent. Adding a timer is connection-dynamics work, out of this feature.

**Trigger to revisit**: the **dynamic-connection-transitions** feature, together with [[N-020]] (decoupling readiness from the dial) — the re-dialer is the periodic counterpart of that one-shot trigger. Decide the interval/backoff and whether re-selection also prunes (the remove-side of [[N-011]]).

## N-026 — Retire settle-`sleep`s across the pre-existing suites (adopt the async-test synchronization strategy) — **RESOLVED by the settle-sleep sweep (2026-06-24)**

**Resolution**: every settle-`sleep` in the pre-existing suites is retired per ADR 0022's selection rule. Outcome-positives that share a sender→receiver FIFO became **real-event barriers** (send the would-be-dropped message first, then a deliverable one, and `await_delivery` it — the delivery proves the earlier one was processed); no-trace non-events became **`assert_no_new_deliveries`** (delivery state) or the new **`assert_no_connection_change`** (connection state), each backed by a named state-machine test. Two non-delivery cases gained observable barriers instead of waits: the no-registry-entry case awaits `is_synced` (`await_synced`), and the registration-then-membership ordering awaits the registration fold (`await_topic_registration`, reading the new `Node::is_registered` getter) — eliminating a cross-stream race the old sleep had only masked. The catalogue below missed `n_node_graph.rs` and `two_node_ping.rs` (reworked after this note was drafted); their settles were swept too. The only `sleep`s left are the 1 ms poll-ticks inside hand-rolled `await_*` loops (an implementation detail of waiting, per ADR 0022). Behavior-preserving: the sorted multiset of leaf test names is unchanged (166) and the full suite stays 166 ok / 17 suites. The historical context is retained below.

**Surfaced during**: 006-fanout-policy PR review (PR #67). Codifies ADR 0022 (barriers vs. bounded-negative checks).

**Question**: 006-fanout-policy's dissemination suite removed every wall-clock settle-`sleep` (positive outcomes await a real event or a FIFO real-event barrier; genuine non-events use `tests/common::assert_no_new_deliveries`). Pre-existing suites still synchronize on raw `tokio::time::sleep(…)` settles — `tests/connections.rs`, `candidate_set.rs`, `topic_validity.rs`, `topic_registry_network.rs`, `topic_filter.rs`, and the `SETTLE`-const ones in `signed_message.rs` / `filter_composition.rs` / `multi_publisher.rs`. Should they migrate to the ADR 0022 strategy?

**Working answer (current scope)**: **Left as-is.** Those are the 002/003/004/013 suites, outside 006-fanout-policy's charter; converting them now would be an unrelated cross-feature churn on an approved PR. They are green and the sleeps are not incorrect, only non-idiomatic.

**Trigger to revisit**: a dedicated **test-hygiene sweep** (its own small PR). For each settle: a **positive** outcome → switch to an `await_*` barrier (or a later-real-event barrier for a processed no-op); a genuine **non-event** → `assert_no_new_deliveries(&[…], window)`. Follow ADR 0022's selection rule, and back any no-event property with a deterministic state-machine test rather than the window alone.

## N-027 — `state.rs` unit-test module dominates the file; split into per-concern test files — **RESOLVED by the state-tests-split refactor (2026-06-24)**

**Resolution**: done in two verified steps. (1) The whole `#[cfg(test)] mod tests` moved verbatim out of `src/state.rs` into the Rust-2018 file form (`mod tests;`), leaving the pure core at ~921 lines. (2) That file split into `src/state/tests/` — `mod.rs` holding the shared imports (`pub(crate)`-re-exported for submodules) and all ~28 helpers, with the 60 tests routed by concern into 8 files: `apply_basics`, `membership`, `setup`, `connection`, `gated_receive`, `severance`, `shutdown`, `fanout`. Submodules reach the pure core's private items (`handle_*`, `NodeState` fields) as descendants via `use super::super::*`, and the helpers/re-exports via `use super::*` — the visibility crux below held. Behavior-preserving and externally verified: the sorted multiset of leaf test names is byte-identical pre/post (166), each test landed in its intended group module, and the full suite stays 166 ok / 17 suites. The historical context is retained below.

**Surfaced during**: the connection-acceptance-strategy refactor (2026-06-23), reviewing module sizes.

**Question**: `src/state.rs` is ~3019 lines, of which only ~919 is production code (the pure core: `apply` + the `handle_*` chain + `NodeState`); the other ~2100 lines (70%) are the single inline `#[cfg(test)] mod tests` with ~60 tests. The core itself is reasonably sized — the bloat is that `apply` funnels *every* handler's tests into one module. Should those tests be restructured?

**Working answer (current scope)**: **Left inline.** These are genuine unit tests of the pure core — they call `apply`/`handle_*` directly and assert on private `NodeState` fields and the returned `Vec<Effect>` synchronously, which is their value (fast, deterministic, no event loop). They **cannot** move to `tests/` without going through the async public `Node` API, which would change their nature and lose the direct-state assertions. So the fix is reorganization within the crate, not relocation — a large, different-axis mechanical move that would bury a focused refactor's diff, so it is deferred to its own PR.

**Trigger to revisit**: a dedicated **test-structure PR**. Keep `src/state.rs` as the production core with `#[cfg(test)] mod tests;`, and split the test module into per-concern files under `src/state/tests/` (Rust 2018 form, no rename of `state.rs`):

- `src/state/tests/mod.rs` — shared helpers (`node_state`, `strategy`, `alias_signer`, script imports) + `mod` declarations;
- per-concern files grouped by handler: `dissemination` (receive path), `publish`, `connection` (request/accepted/terminated), `membership` (membership + topic-registry folds), `lifecycle` (synced/setup/shutdown), `fanout` (fan-out + dedup).

The grouping mirrors the handler chain the existing test comments already key off, so the partition is mostly mechanical. **Crux to validate**: descendant test modules must still reach `state`'s private items — they can, since a private item is visible to its module and all descendants, so a file under `src/state/tests/` reaches `handle_*`/private fields via `super::super::*` (and `crate::state` for the `pub(crate)` surface). Pure move, zero test-logic changes, behavior-preserving. **Trade-off**: breaks the inline-`#[cfg(test)] mod tests` uniformity that `connection`/`fanout`/`acceptance` follow — a deliberate exception justified by `state` being 7–20× their size. **Explicitly not** the deeper alternative of splitting the production handlers across domain modules: that touches the pure-core cohesion (ADR 0011 keeps `apply` + handlers together) and is an architectural change, not a test reorg.

## N-028 — Feature 005 (verifiable hash-gated strategies) deferrals

Recorded on completing 005 (verifiable hash-gated connection-selection + verifiable bounded acceptance, per the bucketed-pull redesign 2026-07-02):

- **Incentive / chain layer** — the deposits (`D`), sybil-count bound (`K`), on-chain identity, and over-capacity slashing reports of `docs/extensions/bucketed-pull.md` are the chain/incentive layer, **not** implemented by these overlay strategies. Deferred to the on-chain workstream.
- **Real per-round beacon + discovery view** — `(genesis, interval)` stand in for the doc's unbiasable `nonce_R`; v1 uses `view = the full candidate set` (no `H_v` discovery sampling). Both arrive with the discovery/beacon layer; the seam (interval input, per-topic candidate view) already accommodates them.

- **Experiment/testing framework** — the topology builder (network-default + per-node-override strategy assignment) and the delivery-percentile / propagation-depth / convergence metrics are a **separate later feature**. 005 ships only the strategies + their tests.
- **Determinism/purity refactor** — strategies-as-`apply`-arguments + deterministic event-loop scheduling are the co-developing architect's parallel workstream; 005 kept its strategy objects pure and the current strategy injection (coordinate, not blocked).
- **Retry / back-fill to a minimum degree** — 005 ships the *no-retry* baseline: on rejection the dialer only drops the pending upstream, realized degree may under-fill. A retry-to-a-minimum policy (with a sticky/decaying failed-set) is a **separate future strategy family** (`BackfillingHashGatedConnection`) — see [[N-029]].
- **Dynamic re-selection / epochal rotation** — selection operates over the candidate set fixed at readiness; re-selection on membership *change* and periodic epochal rotation are deferred.
- **Golden nodes (push-based M2)** + edge/golden mode flag + adversarial/Byzantine node behaviour — later features.
- **Bounded/seeded fan-out** — considered (former feature 015) and **dropped** in the bucketed-pull redesign; fan-out stays `ForwardToAll` (disseminate to all downstream on the topic). A degree-capped fan-out variant would return only with the propagation/replication experiments that need it.
- **N-007 (`PeerView`) pointer resolved** — 005 uses the existing subscription-registry candidate view as the peer view; no separate `PeerView`/`PeerSource` abstraction was introduced.

## N-029 — Retry/back-fill strategy family; `Rejected` as a (currently unused) liveness signal

**Surfaced during**: 005-peer-view (PR #73). Relates to ADR 0024/0025 and [[N-025]] (epochal re-dialer), [[N-020]] (decoupling readiness from the dial).

**Question**: an over-capacity `ConnectionAction::Rejected` tells the dialer a candidate is *alive but at its per-topic out-degree cap* — a positive liveness + capacity signal. Should the node use it to retry/back-fill toward a minimum degree, and to filter/re-rank candidates across heartbeat intervals (keep alive-but-full peers as retryable future candidates; hard-filter genuinely offline ones)?

**Working answer (current scope)**: **Not in 005.** 005 ships the *no-retry* baseline: on `Rejected` the dialer only removes the pending `AwaitingAccept` upstream, and the realized degree may under-fill. An earlier revision added a sticky `failed_upstream` set + `Heartbeat`-driven back-fill; both were **removed** (spec Clarifications, Session 2026-07-02) so no persistent rejection state is carried — the node keeps no failed-set and no rejection counter. The signal's cross-interval value is also unrealized because the in-memory substrate answers every dial, so *offline* is unmodelled (no timeout to distinguish alive-but-full from unreachable — ADR 0025).

**Trigger to revisit**: the **dynamic-connection-transitions / experiment** feature (with [[N-025]] and [[N-020]]). It should introduce a **retry/back-fill strategy family** (`BackfillingHashGatedConnection` / `RetryingHashGatedConnection`) that retries toward a minimum degree, treating `Rejected` as a **soft, re-rankable** signal (alive-but-full → deprioritized/retryable next interval, not permanently dropped) with a sticky-or-decaying failed-set it owns; and **offline detection** (a dial timeout over a real/faulty transport) that hard-filters unreachable candidates. Comparing the no-retry baseline against the retry family is itself an experiment.

## N-030 — Round agreement between requester and acceptor; heartbeat/dial coupling

**Surfaced during**: 005-peer-view (PR #73), review of the verifiable edge predicate. Relates to ADR 0030 (heartbeat interval + shared predicate) and [[N-028]] (the `(genesis, interval)` beacon stand-in).

**Question**: the edge predicate hashes `interval`, but a `Request` carries no interval — the acceptor verifies against its **own** `state.interval`. Rounds advance per node, independently, whenever that node's event loop processes a `Heartbeat`; there is no synchronization. Around a round boundary a requester at interval *k* and an acceptor at *m ≠ k* hash different tuples, so ~(1 − 1/B) of legitimately-selected dials fail verification as **silent** `illegitimate_request` drops. What is the agreement mechanism?

**Working answer (005)**: **Unreachable in this PR, documented, not solved.** Only readiness fires a heartbeat (`handle_synced` → `Heartbeat{0}`), so both sides always sit at interval 0 and the tuples always agree. A pinned `--bucket-count` (this PR) removes the *B*-disagreement axis of the same problem but not the *interval*-disagreement axis. The proper fix belongs with the periodic-heartbeat layer:

- **Carrying the interval in the `Request` is not sufficient by itself.** Verifying against a requester-*claimed* round lets an adversary grind intervals offline until the predicate passes for a chosen victim, defeating the spam resistance the predicate exists for. A tolerance window (accept ±1 of the local round) bounds the grinding but does not eliminate boundary skew.
- **The robust direction is a round anchored to an external, independently observable event** (block hash / slot / epoch boundary) — the unbiasable beacon ADR 0030 defers (`(genesis, interval)` stand in for `nonce_R`; [[N-028]]). That deferral did not name the *agreement* problem; it is the same problem. If `genesis` is intended as a slower epoch-level nonce above the round, it eventually needs the same anchoring.
- **`Heartbeat` currently conflates round-advance and dial-trigger.** Even with an externally agreed round, a node dials the instant it processes the event, so a window exists where one side has advanced and dialed while the other still verifies the previous round. Splitting "advance round" from "dial on current round" would let a driver run a two-phase barrier (advance all, then dial) — which experiments need and an anchored round benefits from.

**Trigger to revisit**: the **periodic-heartbeat / epochal-rotation** layer (with [[N-025]]). Before it ships, decide the round-agreement mechanism (external anchor vs. carried-interval-plus-tolerance) and whether to decouple round-advance from the dial trigger.

**Partially resolved (ADR 0031, 2026-07-07)**: the heartbeat/dial coupling is **solved** — `Event::Heartbeat` is now a parameterless dial tick and `Event::Epoch { nonce }` folds the randomness context separately, so a driver can run the two-phase advance-then-dial barrier this entry called for; the predicate no longer hashes a per-node counter at all (`is_valid_edge(nonce, …)`, genesis = the initial epoch nonce on node state). The **agreement problem remains open** but is reframed: it now bites only at *epoch* boundaries (rarer than per-round), and the nonce is designed to arrive from the externally observable event this entry identified as the robust direction. The grinding argument against a requester-carried value is unchanged. Revisit trigger unchanged (the periodic-heartbeat/rotation layer).

## N-031 — Decomposed acceptance: bounded-only and hash-gated-only variants

**Surfaced during**: 005-peer-view (PR #73), Ezequiel's follow-up. Relates to ADR 0025 and [[N-028]] (experiment framework deferral).

**Question**: to simulate all stages in the experiment framework, do we also want a **bounded-only** acceptance (membership + cap, no edge predicate) and a **hash-gated-only** acceptance (membership + edge predicate, no cap), alongside the shipped `AcceptFromAll` (neither) and `VerifiableBoundedAcceptance` (both)?

**Working answer (005)**: **Not in this PR** — Ezequiel flagged it as not a blocker, to discuss after the experiments-document review. Feasibility is **trivial**: the checks are already factored into shared, independent pieces — `is_membership_valid` (`strategies::acceptance`) and `is_valid_edge` / `accept_cap` / `bucket_count` (`strategies::edge`) — and `VerifiableBoundedAcceptance::admit` merely sequences them. Each missing variant is a ~15-line `admit` reusing the same helpers, plus an `AcceptanceStrategyKind` variant and a `build` arm. Two candidate shapes:

- **Two concrete thin structs** (`BoundedAcceptance`, `HashGatedAcceptance`) — matches the current one-file-per-strategy layout; simplest for the four fixed combinations.
- **An ordered `AcceptanceCheck` combinator** AND-combined (first reject wins; `Admission` already names the distinct reject reasons) — more general, but likely over-engineering for four combinations.

The dial seam's mirror is only a 2-way split (`ConnectToAll` / `HashGatedConnection`) — a bound is meaningless on the dial side.

**Why deferred**: the experiments document defines *which* stages need simulating, which in turn decides whether concrete structs or a combinator is the right shape; building before that review risks the wrong decomposition.

**Trigger to revisit**: the experiment/testing-framework feature ([[N-028]]), after the experiments-document review.

**Resolved (ADR 0031, 2026-07-07)**: implemented ahead of the experiments-document review per the agreed empirical one-dimensional-baseline approach — `BoundedAcceptance` (cap only) and `HashGatedAcceptance` (gate only) ship beside `accept-from-all` and the compound (renamed `hash-gated-bounded` for dial-seam naming symmetry). The **two concrete thin structs** shape won; the combinator was rejected as over-general for four fixed combinations, with the shared `admit_prelude` capturing the one invariant (membership → already-downstream re-Accept) that must not drift. Compounding beyond the four kinds (blacklists, deposits) stays deferred to the experiment framework.

## N-032 — Capped acceptance on a symmetric node: what does the cap bound?

**Surfaced during**: 015-publisher-links (PR #77), review round 5 / audit A12. Relates to ADR 0034 (constructed symmetric reciprocity legalised the capped × symmetric combination the earlier startup check had rejected) and [[N-029]] (retry/back-fill).

**Question**: on a symmetric node every accepted edge is mirrored into both collections, and the node's **own** accepted dials bypass the acceptance gate while still occupying the capacity its scan counts. So what should a bounded acceptance strategy bound — the peer-initiated in-degree only, or the node's total degree — and should the node's own mirrored dials count toward it?

**Working answer (015, per the maintainer's direction)**: **deferred — do not resolve, do not extend.** No experiment or published recipe needs a capped bidirectional strategy today (M4 defines no caps), so the combination must not grow semantics ahead of a consumer. On the substance the dilemma partly dissolves: an acceptance strategy structurally caps only **inbound requests** — it cannot instruct the dialer, so it never bounds out-degree; and an `Accepted` answering the node's own pending request is not an admission decision. What the current code does (recorded so the behaviour is not mistaken for a decision): the cap's link scan counts the whole mirrored link set — own-dial mirrors included — but the gate fires only on peer-initiated requests, so realised degree can exceed the cap and the outcome is arrival-order-dependent.

**Trigger to revisit**: the first experiment that requires the symmetric × capped combination (an E11/E12 extension) — which may never arrive. (The uniform exactly-RF selection landed with 017 and completed the real M4; the combination is expressible as knobs — `--relay-symmetric` with `--relay-accept-cap` — with the recorded semantics above unchanged, and the 017 quickstart notes that a symmetric node's caps anchor on ≈ 2× the pick count.)

**Resolved (ADR 0042, 2026-08-15)**: the trigger fired — the symmetric flooding pass (the E12 analogue under the symmetric handshake) is the consumer. The cap is an **admissions budget**: it bounds peer-initiated admissions of edges the node did not itself select, at C per (topic, kind) per epoch. The node's own picks (bounded by the pick count by construction) never count toward it and are never vetoed by it; an inbound request matching the node's own pending `AwaitingAccept` (a crossing) short-circuits ahead of gate and cap and spends no budget — answering one's own selection is not an admission decision; no decrement on severance (direction erasure makes the attribution impossible without the per-link state [[N-039]] records against; the budget is per-epoch). Implementation: an admitted-count on `NodeState` folded at the symmetric admission site, surfaced through `NodeView`; the directional and publisher seams keep the link scan, which already implements the same semantics (their scanned kind-set is the admitted set). Total symmetric degree ≤ pick count + C by construction, order-independently; the defensive invariant is ≤ C non-chosen edges per node per epoch. The 015-recorded scan behaviour is retired after being measured once (the contrast cells, pinned to their pre-change tool commit); ADR 0042 records the domination argument and the rejected alternatives. The 017 quickstart's ≈ 2×-pick-count cap anchor is superseded for symmetric seams — caps anchor on the fresh-arrival load K(1−m).

## N-033 — Experiment population memory: full candidate views bound N to ~20 000 per in-flight run

**Surfaced during**: 016-experiments-framework implementation (T026 execution sizing).

**Observation**: v1's "view = full candidate set" means every driver-owned node core holds all N−1 peer ids in its `candidates` map, so one experiment population costs O(N²) memory — measured ~30 GB peak RSS for one N = 20 000 run (release build; ~1.3 GB at N = 4 000). The worker count doubles as the memory knob (each in-flight run holds a full population), which makes the shipped operating point run at `--workers 1` on a 64 GB machine. The plan's "populations up to ~10⁵ nodes per run" is **not** reachable with per-node owned candidate sets (~750 GB).

**Candidate directions when needed**: share the (identical) candidate view across driver-owned cores behind a read-only handle (an experiments-side optimization, but the core owns the `candidates` field type today); intern peer ids; or let the future discovery/view-sampling work (`H_v`, the 005 out-of-scope item) shrink the per-node view to a sample, which removes the O(N²) term for real nodes and driver alike.

**Trigger to revisit**: the first experiment needing N ≫ 20 000, or the peer-sampling/view-discovery feature — whichever lands first.

**Resolved (ADR 0038, 2026-07-27)**: the shared-view direction was implemented. `candidates` stores each topic's **full membership including the node itself** as `Arc<BTreeSet<PeerId>>` behind self-excluding `NodeView` read accessors, and the fast-path registration hands every core the same shared set — one N-element set per run instead of N of them, with registration work down from O(N²) to O(N) as well. Measured: one N = 20 000 operating-point run peaks at ~0.63 GB RSS (was ~30 GB), so the worker count stops being a memory knob and the plan's ~10⁵-node populations become reachable. The peer-id-interning direction is superseded; the `H_v`/view-sampling direction remains open as protocol work, now decoupled from the driver's memory budget.

## N-034 — Experiment round budget: multi-round runs as an explicit, deterministic knob

**Surfaced during**: 016 PR review (PR #102), a reviewer design suggestion.

**Idea**: the run phase machine is hardcoded to a single establish + single measure. An explicit round budget (config knob, default 1 ≡ today's "measure the first graph" semantics) would let longer/dynamic experiments — epoch rotation, multi-heartbeat steady state — attach cleanly, with connection-lifecycle teardown activating only for budgets > 1. It would also make candidate-memory reclamation principled ("free after the last round that reads `candidates`" is result-neutral; a hardcoded free-after-establish would have to be reverted by any multi-round mode — relates to [[N-033]]). An unbounded "run forever" mode would fight the byte-reproducibility contract, so any extension must keep a deterministic stop (fixed rounds or a convergence rule).

**Why deferred**: 016 deliberately pinned single-epoch runs (the driver never advances the nonce — a clarify-session resolution), and the core itself has no epoch rotation or connection teardown yet, so a budget knob today would be a result-affecting parameter whose values > 1 cannot be exercised meaningfully — the forward-compatible-interfaces standard wants the named consumer to exist first.

**Trigger to revisit**: the epoch-advancement/rotation feature (the heartbeat/epoch seams from ADR 0031 are where it plugs in), or the first steady-state experiment need.

## N-035 — Experiment dial-drain time: the sampler's per-node candidate materialisation is the remaining O(N²) term

**Surfaced during**: the PR #77/#102 instrument-performance follow-up (ADR 0038). Relates to [[N-033]] (resolved — the memory half of the same scaling story; this is the time half).

**Observation**: `UniformSampler::expected_links` collects all N−1 candidate references into a `Vec` before drawing its RF index samples — O(N) per node, O(N²) per run across the dial drain. Invisible at today's populations (a fraction of a ~4.4 s run at N = 20 000), but it scales quadratically: at the plan's ~10⁵-node populations it is ~10¹⁰ pointer pushes per run — tens of seconds, the successor to the memory bound ADR 0038 removed as the thing that decides how large N can get.

**Sketch when needed**: the driver already shares one sorted membership set per topic (ADR 0038); expose it (or a once-per-run sorted slice) to the sampler so it samples **indices** against `candidates_len` and maps them through skip-self index arithmetic (an index at or above the node's own rank shifts by one) instead of materialising the per-node list. Byte-identity is achievable — same sample length, same index→peer mapping — but the skip-self mapping is a razor edge of exactly the `stored_self_does_not_shift_the_sample` kind: land it against the recorded baselines (`notes/experiments-baselines/`, byte-diff must come out identical) and extend that test family.

**Trigger to revisit**: the first experiment needing N ≫ 20 000 (the same trigger the resolved [[N-033]] carried for memory).

## N-036 — Gate-failing dials: provable misbehaviour, deliberately unrecorded

**Surfaced during**: 017-unified-selection (the verifiable-region restatement; ADR 0039).

**Observation**: with the bucket count fed on both ends, a dial whose edge fails the seam's predicate is **provable misbehaviour**: the request is signed and the predicate is publicly recomputable, so the pair (signed request, failed recomputation) is exactly the evidence shape the bucketed-pull slashing rule needs (`docs/extensions/bucketed-pull.md`, read-only). v1 deliberately does not collect it: a predicate-failing request is a silent `RejectIllegitimate` — no reply, no `Misbehaved` effect, no record — because the incentive/chain layer (deposits, reports, slashing) has been out of scope since 005.

**Working answer (017 scope)**: keep the silent drop; name the acceptance gate — `UnifiedAcceptance`'s predicate check — as the future evidence-collection point. When evidence lands it is a strictly local change at that site: the decision input (the verified request plus the recomputed predicate) is already in hand there.

**Trigger to revisit**: the incentive/chain layer (on-chain identity, deposits, over-capacity/misbehaviour reports).

## N-037 — Selection-seed privacy: the operator flag is a prototype stand-in

**Surfaced during**: 017-unified-selection (the seed chain; ADR 0040).

**Observation**: the formal models prescribe *private, unpredictable* per-node selection randomness for uniform picks. `--selection-seed <u64>` is a low-entropy operator flag: reproducibility, not secrecy. Anyone who knows the seed (and the public self-identity, epoch nonce, and membership) can recompute a node's picks. Fleet-shared seed values still yield per-node-independent draws — self-identity is in the draw preimage — but independence is not privacy.

**Working answer (017 scope)**: model-adequate against **oblivious adversaries**, which covers the entire current experiment program (silent relays do not choose victims by predicting picks). The experiments driver's per-participant seeds derived from a master seed have the same posture by design — reproducible science, not confidentiality.

**Trigger to revisit**: the first adaptive-adversary experiment (Stage 5 / E15 survivors), or the real-crypto identity work — whichever lands first. Provisioning must then become per-node secret material, or derive from the identity key under proper domain separation.

## N-038 — Sampled selection under view growth: pick sets are view-functions, and add-only dialing unions them

**Surfaced during**: 017-unified-selection, Phase 4 checkpoint (the M4 fleet test's first failing run; analysis.md I3).

**Observation**: a sampled pick set is a function of the **whole candidate view** — index sampling over the sorted survivor list means adding one candidate re-shuffles which K survive the draw. Two consequences, both distinct from hash-gating's behaviour:

- **A single dial over a partial view draws subset-sized picks, not subset picks**: min(pick count, partial-view survivors) — below the pick count if the view is smaller than K, with no retry or back-fill; and the picks themselves need not be contained in what the full view would select.
- **A re-dial after view growth draws a different sample, and the add-only dial model (selection only adds — [[N-011]]) unions the two**: realised out-degree inflates past the pick count. Measured: the 017 M4 fleet test's first run inflated mean degree beyond the 2K bound because each node's readiness dial fired against a partially-folded view and the retry heartbeat then drew a second, different sample over the full view.

ADR 0031's "repeated heartbeats re-dial the SAME expected set" idempotence is therefore **conditional for the sampling arm**: it holds while the candidate view is stable within the epoch. Hash-gating keeps the stronger unconditional property — the predicate is per-candidate, so a partial-view gated dial is a *subset* of the full-view one and re-dials are monotone-consistent, never inflating.

**Why today's surfaces are safe by construction**: the node fires exactly one readiness heartbeat, after `Synced` — i.e. after both registry snapshots are folded, so the view it samples is the full bootstrap membership; the experiments driver's establishment runs behind its all-synced registration barrier. Test fleets must reproduce that shape (seed every membership before constructing nodes — the driver's barrier applied to the harness), which the 017 fleet tests now do.

**Trigger to revisit**: periodic heartbeats / epoch rotation (the ADR 0031 seams), or the first staggered-boot fleet or experiment — any surface where a sampling node re-dials across view growth. The remove-side of selection ([[N-011]]) is the natural companion: re-selection that prunes would restore exactly-K instead of unioning.

## N-039 — Hash-gate predicate inputs match the protocol object: symmetric links draw the unordered pair because they erase who dialed

**Surfaced during**: the 017 / PR #119 review round (the gated-symmetric plane point — `--relay-symmetric` with a bucket count; `is_valid_edge_sym`, ADR 0024/0034 lineage).

**Observation**: gated symmetric selection draws the edge predicate for the **unordered pair** — the two peer keys hashed in canonical byte order under a dedicated domain (`edge-sym/v1`) — rather than reusing the directional predicate over whichever direction dialed. The alternative construction (directional gate on the dialer, directional verification on the acceptor, reciprocity constructed on accept exactly as today) is mechanically coherent, and per-request verification is equally strong under it; the dialer-side confirmation it needs is the existing pending-entry match. It is not used because its edge validity is **initiation-dependent**, and the symmetric link model deliberately erases initiation (one accept records the link in both maps on both ends — bidirectionality is emergent, not stored). Recorded costs of the alternative, so the choice stays reviewable:

- **Re-validation and audit need an initiation bit the state no longer has.** Epoch rotation ("which of my links survive the new nonce?") and any standing-topology audit need validity as a pure function of (nonce, topic, pair); direction-dependent validity would force an initiation flag into symmetric link state, on both ends.
- **The two ends stop agreeing on the edge set.** Under the pair draw both ends compute identical survivor sets (both dial; the crossing resolves idempotently). Under directional-OR each gate sees only its out-half, the in-half arrives as edges the holder would never have selected, and "every held link passes my predicate" becomes history-dependent.
- **The statistics shift against the gate.** Pair-edge density becomes 1 − (1 − 1/B)² ≈ 2/B (the balanced-point guidance would need a ~2× correction), and a Sybil reaches a victim's serving slots if **either** direction's draw holds — roughly doubling adversarial slot access and diluting the ≈ cap/B concentration bound gating exists to provide (the E12 story).

The principle is two-sided — **the predicate's input matches the protocol object and the retained state**. A directional pull link is an ordered tuple ("requester may pull from candidate"; the state keeps the direction), so the directional seams hash the ordered tuple; a symmetric link is an unordered edge (the state erases initiation), so the symmetric gate hashes the pair. The inverse substitution — the pair draw on the directional seams — breaks nothing mechanically (validity stays recomputable; per-node out-degree statistics look unchanged) but silently changes the model: every valid edge becomes mutual, so the overlay is M4-shaped while claiming a directional family (half the distinct neighbours per link budget); in-degree becomes identical to out-degree instead of an independent binomial (the ⌈K + c·√K⌉ cap-headroom guidance mis-models); one coin per pair grants an adversary the pull-eclipse and slot-occupancy surfaces together where directional draws keep them independent; and on the publisher seam it correlates two role-asymmetric edges with unrelated meanings.

**Working answer (017 state)**: keep the unordered-pair predicate under its own domain (its independence from the directional draw is pinned by `symmetric_domain_is_an_independent_draw`). Boundary stated honestly: the gated-symmetric point is the crate's own composition — the bucketed-pull analysis (read-only) treats directional pull, and no published model covers hash-gated bidirectional selection. The real M4 never touches this predicate: with the bucket count absent the gate is skipped entirely, and acceptance is membership-only.

**Trigger to revisit**: the first experiment or protocol decision that actually exercises the gated-symmetric point (a gated E7 variant, a bidirectional protocol-track configuration, or the symmetric × capped combination of [[N-032]]) — verify the pair-draw's assumed properties (per-pair 1/B density, both-ends agreement, adversarial occupancy) against whatever analysis that consumer brings, before results rely on them.

**Trigger fired (E18, 2026-08-13 — `docs/experiments/gated-symmetric.md`)**: all three assumed properties verified against the eleven-cell pass (per-pair 1/B density directly by the gate-only cells, degree = (N−1)/B to three digits; both-ends agreement through the shared-pool degree law; occupancy through the isolation constants, matched twice at 8 000 runs). The pass also measured what the assumptions did not cover: the **empty-pool channel** — isolation e^(−(1−μ)(N−1)/B), independent of the pick count, so RF cannot compensate — giving the pool floor (N−1)/B ≥ ln(H/δ)/(1−μ) and channel crossover at r ≈ 3, and the report's §4 prices the ordered-predicate alternative (≈ 2/B admissibility at equal B; the saturation frontier λ_floor/(N−1) is predicate-independent). The unordered-pair choice stands, now with a measured operating window. The symmetric × capped combination remained with [[N-032]]/[[N-040]] — E18 ran open acceptance, and its benefit-side follow-up (the E12 analogue) is the consumer those notes' triggers name; those triggers have since fired, and ADR 0042 resolves the combination as the admissions budget.

## N-040 — Detail slot columns are direction-blind under the symmetric handshake

**Surfaced during**: the PR #152 review (the per-node connection-accounting detail columns; E12).

**Observation**: the detail columns `downstream_honest`/`downstream_adversarial` count relay-kind `downstream` entries by the linked peer's class. On directional configurations that is exactly the node's granted serving slots — the E12 measurand, and every E12 cell is directional. Under the symmetric handshake, reciprocity writes **both ends of every edge into `downstream`** (one accept records the link in both maps on both ends; the dialer mirrors on `Accepted`), so the columns count both roles — ≈ 2× the pick count — where the serving-slots reading suggests inbound grants only. Two structural facts frame this:

- **Direction is unrecoverable from end-state, by design.** The link model stores *kind*, never who dialed; direction is map placement, and the symmetric handshake deliberately collapses it. Direction-aware slot attribution would need either stored initiation state (the cost [[N-039]] records against direction-dependent designs) or drain-time attribution like the refusal maps.
- **The acceptance cap on a symmetric node scans the same both-role total** — the cap there bounds something like total symmetric degree, not inbound serving load, which is exactly the deferred [[N-032]] question. The columns therefore report faithfully what the cap sees; the directional serving-slots language is what does not transfer.

**Working answer**: documented on the fields (relay-kind downstream entries; serving-slots reading scoped to directional configurations). No shipped experiment reads the columns under a symmetric configuration.

**Trigger to revisit**: the first M4/symmetric consumer of the detail slot columns, or [[N-032]]'s own resolution — whichever lands first decides whether the columns need drain-time direction attribution or the cap semantics make the both-role count the right measurand anyway.

**Resolved (ADR 0042, 2026-08-15)**: both trigger arms landed at once — the symmetric flooding pass is the first symmetric consumer, and [[N-032]]'s resolution (the admissions budget) decided the direction: the cap now enforces a direction-attributed quantity, so the both-role count cannot verify it. The detail rows gain **drain-time route attribution** (driver-side, the refusal-map precedent — the driver routes every dial, accept, and refusal, so no initiation state enters the node): per-node relay-edge counts split own-only / mutual / admitted × peer class, plus refusals attributed fresh-arrival vs crossing. The both-role columns stay, with the serving-slots reading still scoped to directional configurations; on symmetric configurations the route sums equal the both-role count (a per-run identity). Run rows untouched ⇒ no re-baseline expected (the `72bf76c` precedent; byte-identity spot-checked before any cell).

## N-041 — The detail row's slot columns are relay-only while its refusal columns are kind-agnostic

**Surfaced during**: the PR #152 review (the per-node connection-accounting detail columns; E12).

**Observation**: `downstream_honest`/`downstream_adversarial` count the relay seam only, but `dials_refused` and `refusals_issued_*` tally every routed over-capacity `Rejected`, publisher-seam ones included (acceptance capacities are disjoint per kind since 015). An M3/M5 configuration with a `publisher.accept_cap` would therefore show publisher-seam refusals with no publisher-seam slot columns to reconcile them against — the per-node refusal counts would exceed anything the relay slot columns can explain.

**Why today's surfaces are safe**: no shipped configuration caps the publisher seam, and every E12 cell is relay-only; the columns' one consumer (`docs/experiments/summarise_flooding_cell.py`) reconciles totals against `rejected_over_capacity`, which is seam-pooled on both sides of the identity.

**Trigger to revisit**: the first publisher-seam capped experiment — the M3/M5 publisher-acceptance flooding surface named as unmeasured in `docs/experiments/e12-flooding-mitigation.md` §6, or a publisher-seam congestion variant of E11. The natural completion is a `downstream_publisher_honest`/`_adversarial` pair (detail-only, so no re-baseline), landed together with that experiment.

**Resolved (the M4 synthesis pass, 2026-08-17)**: the trigger fired — the first publisher-seam capped cells (seeds 1147–1148, `configs/experiments/m4-synthesis/pubseam-*`) landed together with the completion exactly as planned: the detail rows gain `downstream_publisher_honest`/`_adversarial` (each node's `Active` seed targets by linked-peer class; detail-only, all seven baseline sweeps byte-identical at the landing commit). The cells measured the seed-intake denial attack and surfaced the **seed-rescue coverage coupling**: the seam's refusals starve exactly the rescuing (honest-target) seeds — adversarial targets accept and sit silent — so a seed fails at f = μ + (1−μ)·ρ_p per pick and a binding publisher cap reaches coverage through the mute channel (the inverted-seam analogue of the E19 §6 composition term). The publisher cap-sizing rule therefore anchors on the intake load with headroom, like every other cap in the program. Per-node mute tails under a saturated seed budget are additionally rank-amplified by the instrument ([[N-042]], whose trigger this cell fired).

## N-042 — The wavefront budget race is class-fair but rank-concentrated on the dialer side

**Surfaced during**: the symmetric-flooding pass (ADR 0042/0043) — the ordered flooder cell's coverage dissection.

**Observation**: wave canonicalisation sorts deliveries by (sender rank, recipient rank, identity), so every victim processes its arrivals in the same global sender-rank order, and budget admission is first-come. Across **classes** this is exactly fair — ranks are independent of the class draw (verified: uniform adversarial rank distribution), so refusals split by class-load share and every class-level mean the passes predicted matched. Per **node** it concentrates: once arrivals exceed budgets population-wide, a low-ranked dialer wins every race and a high-ranked dialer loses every race (measured in the ordered flooder cell: bottom-400-rank honest nodes 0 refused dials, top-400-rank 12.65 ≈ their whole honest pick count). Per-node tail events composed of many dial outcomes — total out-side death, hence starvation isolation — are amplified by orders of magnitude relative to the independent-per-victim arrival orders a real network approximates: the cell measured 14/400 bad graphs (single high-rank strandings) where independent orders predict ≈ 0.

- **Class-level measurands are unaffected** — E12, E18, and the symmetric-flooding grid all predicted class-level means and identities, which the canonical order satisfies exactly.
- The E12 §1 envelope statement covers class fairness; this note records the per-dialer boundary: the driver sits between the real network's decorrelated orders and the analytic race-winner worst case, coupling one global order across all victims.

**Working answer**: recorded; the symmetric-flooding report's ordered flooder coverage row carries the attribution. Candidate instrument change when a consumer needs per-node tail fidelity under saturated budgets: a per-victim seeded arrival order (a function of (victim, run seed) rather than the global rank) — byte-affecting, hence a re-baseline generation per ADR 0036, and a natural batch partner for the parked failure-severity run-row change.

**Trigger to revisit**: the first pass whose measurand is a per-node tail under saturated budgets (attacker-timing studies, retry design), or the next re-baseline-forcing instrument change — whichever lands first.

**Trigger fired (the M4 synthesis pass, 2026-08-17)**: the publisher-seam cap cells (seeds 1147–1148) are the first pass whose measurand — mute-stranding, a per-node tail composed of s−1 seed-dial outcomes — sits under a saturated budget (ρ_p ≈ 0.25 and 0.49). The μ = 0.4 cell measured 188/400 bad against the corrected independent-order form's 84.8; the rank dissection (regenerated detail, within-run rank) reproduced this note's signature exactly — seed-dial losses a step function of rank (bottom deciles lose ~0 of ~3.6 honest seed dials, top deciles ~all; honest seed targets kept fall 3.58 → 0.04 across deciles; every sampled stranding in the top four deciles; the sample shows ~2× the independent-order stranding count). Class-level columns were exact throughout (refusals/run within 0.1 % of registration). Reading recorded in the synthesis report: real decorrelated networks sit at the independent-order form (~0.21 at that shape); the canonical order's 0.47 is the instrument's amplified upper bound. The parked per-victim seeded arrival order now has two consumers (the E19 ordered flooder row and this cell) — its falsifiable tests are re-runs of both.

**Resolved (the instrument pass, 2026-08-20, ADR 0044)**: the wave sort's per-victim seeded arrival order landed at `0764da3` — each recipient's intra-wave arrivals follow `SHA-256(lp("experiments/arrival-order/v1") ‖ run_seed ‖ lp(recipient) ‖ lp(sender))`, decorrelated between victims, deterministic and worker-count-independent as before; batched with the failure-severity `deaf`/`mute` run-row pair into one re-baseline generation. Both frozen validation re-runs passed at the new instrument: **seed 1148 measured 80/400 bad against the independent-order form's 84.8 (z = −0.59)**, split 72 mute-class / 8 deaf-class (registered ~83 %/17 %; the deaf count sits at z = −1.72 against the registered share's 14.4 expected of 400, binomial on the registered p — within noise under every estimator, and the registered split was a first-order channel decomposition, not a measured ratio), refusals 23 060/run vs the registration's 23 063; **the E19 ordered flooder measured 400/400 good** (canonical order: 386/400 — all 14 strandings were the instrument's), with the race and dial columns **identical run-for-run** — zero of the 400 rows differ on `rejected_over_capacity`/`dial_sends`/`dial_waves` (40 617.4 rejected/run, 230 394.6 dial sends/run at the mean), and the fields that do differ are exactly the topology outcomes the fix is meant to move. The budget refuses the same load under both orders; only the per-node race allocation changed. The attribution stands confirmed: class-level measurands were exact under both orders, and per-node tails now measure the decorrelated-order quantity the closed forms model.
