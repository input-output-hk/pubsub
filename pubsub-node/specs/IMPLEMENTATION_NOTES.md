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
