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

**Question**: the `synced` flag gates the node's **own dialing** (`handle_synced` → `handle_connection_setup`) but is not consulted by `handle_connection_request` (acceptance), the message receive path, or the publish path (`handle_publish` — a `publish` while `!synced` validates against the cold `subscriptions` set and drops `topic_not_subscribed`, benign and the same class as the receive case; 006-fanout-policy). Because the registry indexer and the network mailbox are independent producers on the single FIFO queue, an inbound `Request` that lands before the registry snapshots are folded is evaluated against cold `subscriptions`/`candidates` and silently rejected (`membership_validation_failed`), with no retry hint or deferral. The node knows it is still syncing but applies that knowledge only to dialing.

**Working answer (014 scope)**: **Readiness scoped to dialing only.** Invisible in the test suite (registries are populated before `Node::new` and triggers are deterministic, so snapshots always fold first). Not a regression — pre-014 there was no readiness notion and the same cold-state race existed. Accepted v1 state.

**Trigger to revisit**: the **dynamic connection lifecycle** work (re-establish / GC). When peers re-dial on a timer an early rejection self-heals; decide at that point whether acceptance (and the receive path) should also gate on `synced` — defer or reject-with-retry while `!synced` — or whether peer re-dial makes it moot.

## N-019 — Own-membership unsubscribe retains established connections (asymmetric with the topic-removal cascade)

**Surfaced during**: 006-fanout-policy rebase onto merged 014 (2026-06-18) — review of `handle_membership_update` against the topic-`Removed` cascade ([[N-017]]).

**Question**: when the node's **own** membership drops a topic — `MembershipEvent::TopicsChanged { removed }` or `Left` — the fold removes the topic from `subscriptions` (and `candidates`) but does **not** touch `upstream`/`downstream`, so connections already established on that topic persist as stale entries. This is asymmetric with `handle_topic_registry_update`'s `Removed`, which **does** cascade into `upstream`/`downstream` ([[N-017]]). Should a self-unsubscribe also tear down (and/or notify) connections on the dropped topic?

**Working answer (006 / current scope)**: **No teardown** — connections on a self-unsubscribed topic are retained. No delivery-correctness impact: inbound payload on the topic is dropped at the receive path's `topic_not_subscribed` gate, and the node will not publish on it (publish requires subscription), so a retained downstream is never fanned to. Consistent with the add-only, no-removal stance of [[N-011]] (selection only adds; removal is dynamic-connection work) and the stale-entry posture of [[N-012]]. The asymmetry with the topic-`Removed` cascade is deliberate-by-omission — documented here rather than reconciled now, to avoid touching the established connection structures outside the dynamic-connections feature.

**Trigger to revisit**: the **dynamic-connection-transitions** feature (with [[N-011]] / [[N-017]]). When selection gains a remove side, decide whether a self-unsubscribe should cascade into `upstream`/`downstream` (matching the topic-`Removed` cascade) and whether it emits `Terminated`, so the two membership-loss paths converge on one teardown rule.

## N-020 — `Synced` atomically triggers dialing (readiness coupled to establishment)

**Surfaced during**: 006-fanout-policy (US2 fan-out relay). Relates to ADR 0020's `Syncing → Synced` lifecycle and [[N-018]] (readiness gates dialing, not acceptance/receive).

**Question**: the `Synced` readiness transition runs `handle_connection_setup` atomically, so reaching readiness and dialing the connection policy's full expected-upstream set are a single, inseparable step — a node cannot become ready without immediately dialing. Should readiness and establishment be separable?

**Working answer (current scope)**: **Keep coupled.** Autonomous startup wants readiness to trigger the dial, and `Event::ConnectionSetup` remains separately injectable for any caller that needs to dial deliberately, so nothing is blocked. No change.

**Trigger to revisit**: the **dynamic-connection-transitions** feature (with [[N-011]] / [[N-017]] / [[N-018]] / [[N-019]] — the connection-lifecycle cluster). Reconsider whether `Synced` should only flip the readiness flag and emit `Event::ConnectionSetup` as a separate queued event, decoupling readiness from automatic dialing.

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

## N-024 — `Message::Signed` → `Message::Dissemination` rename

**Surfaced during**: 006-fanout-policy (data-model §7 D4).

**Question**: the dissemination payload variant is still named `Message::Signed` (with `SignedMessage` / `PlainMessage`), a name from before connection-control messages were also signed. It now reads as if it were the only signed kind. Rename to `Message::Dissemination`?

**Working answer (current scope)**: **Deferred.** A purely mechanical rename touching every dissemination call site; bundling it into a behavioural feature would inflate the diff and obscure the functional change. Left for a dedicated rename pass.

**Trigger to revisit**: any time a low-risk mechanical-refactor pass is scheduled (or opportunistically alongside the next change that already touches the message type hierarchy — ADR 0010).

## N-025 — Epochal / periodic re-dialer

**Surfaced during**: 006-fanout-policy (out of scope; data-model §7 D5).

**Question**: connection establishment fires once (the `Synced` readiness dial, or an injected `Event::ConnectionSetup`). Nothing re-selects or re-dials on an interval, so a node that missed a peer (absent at sync, or a dropped request) never retries autonomously. Should there be a periodic re-dial?

**Working answer (current scope)**: **No periodic re-dial.** `Event::ConnectionSetup` is idempotent and re-injectable (a recurring setup re-dials pending pairs, skips Active ones — [[N-011]]), so the *mechanism* exists; only the periodic *trigger* is absent. Adding a timer is connection-dynamics work, out of this feature.

**Trigger to revisit**: the **dynamic-connection-transitions** feature, together with [[N-020]] (decoupling readiness from the dial) — the re-dialer is the periodic counterpart of that one-shot trigger. Decide the interval/backoff and whether re-selection also prunes (the remove-side of [[N-011]]).

## N-026 — Retire settle-`sleep`s across the pre-existing suites (adopt the async-test synchronization strategy)

**Surfaced during**: 006-fanout-policy PR review (PR #67). Codifies ADR 0022 (barriers vs. bounded-negative checks).

**Question**: 006-fanout-policy's dissemination suite removed every wall-clock settle-`sleep` (positive outcomes await a real event or a FIFO real-event barrier; genuine non-events use `tests/common::assert_no_new_deliveries`). Pre-existing suites still synchronize on raw `tokio::time::sleep(…)` settles — `tests/connections.rs`, `candidate_set.rs`, `topic_validity.rs`, `topic_registry_network.rs`, `topic_filter.rs`, and the `SETTLE`-const ones in `signed_message.rs` / `filter_composition.rs` / `multi_publisher.rs`. Should they migrate to the ADR 0022 strategy?

**Working answer (current scope)**: **Left as-is.** Those are the 002/003/004/013 suites, outside 006-fanout-policy's charter; converting them now would be an unrelated cross-feature churn on an approved PR. They are green and the sleeps are not incorrect, only non-idiomatic.

**Trigger to revisit**: a dedicated **test-hygiene sweep** (its own small PR). For each settle: a **positive** outcome → switch to an `await_*` barrier (or a later-real-event barrier for a processed no-op); a genuine **non-event** → `assert_no_new_deliveries(&[…], window)`. Follow ADR 0022's selection rule, and back any no-event property with a deterministic state-machine test rather than the window alone.
