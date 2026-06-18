# Contract — Fan-out, Publish, and Dedup

The behavioral contract this feature must satisfy and the public-surface delta it introduces. Cross-referenced by `/speckit-tasks` and the post-implementation analyze pass (which verifies the public-surface claims here against `lib.rs` re-exports and module visibility).

## 1. Publish contract

- **1.1** `Node::publish(&self, message: SignedMessage)` returns `()` and enqueues `Event::Publish(message)`. It performs no validation itself and never blocks on a verdict.
- **1.2** A published message is accepted iff, in order: its topic is in the node's membership-derived subscriptions; its topic is registered; its publisher is authorized for the topic (open topic accepts any); its signature verifies over `plain.signed_bytes()`; and its content hash is not already in `seen`.
- **1.3** Acceptance records `ReceivedDelivery { origin: Origin::Local, message }`, inserts the hash into `seen`, and fans out with `exclude = None`.
- **1.4** A published message that fails a validation check is dropped (`message_dropped`, cause per §4), not recorded, not fanned out, and **never severs a connection**.
- **1.5** The publisher need not be the node (`publisher_id == self_id` is not required): a validly-signed, authorized message from any publisher is accepted (proxy/injection).
- **1.6** A second publish of identical content is dropped as `duplicate` (content-hash dedup, §3).

## 2. Fan-out contract

- **2.1** Fan-out applies at the record point on both paths: after recording a published message, and after recording a received message.
- **2.2** Targets are `FanoutStrategy::targets(topic, &downstream, exclude)`. `ForwardToAll` returns every `peer` with `(peer, topic) ∈ downstream`, minus `exclude`.
- **2.3** On the receive path `exclude = Some(delivering_peer)` (split-horizon); on the publish path `exclude = None`.
- **2.4** Each target yields `Effect::Send { to: peer, message: Message::Signed(original.clone()) }` — **verbatim**, no re-signing. No new `Effect` variant is introduced.
- **2.5** Empty or fully-excluded `downstream` ⇒ no `Effect::Send` (recording still occurs).
- **2.6** A node only holds downstream entries on topics it is a member of ⇒ it never fans out a topic it is not subscribed to (subscriber-relay).
- **2.7** `ForwardToAll` is deterministic in the *set* of targets; target *order* is unspecified.

## 3. Dedup contract

- **3.1** The dedup key is `MessageHash::of(&signed.plain)`.
- **3.2** The check runs **after** signature verification and **before** recording, on both paths.
- **3.3** First-seen: record + `seen.insert(hash)` + fan-out. Already-seen: drop (`duplicate`), no record, no fan-out.
- **3.4** A message that fails any check before the dedup gate is never inserted into `seen` (no poisoning).
- **3.5** Dedup spans both paths: a message published (and seen-marked) is dropped if later relayed back.
- **3.6** `seen` is unbounded (in-memory model).

## 4. Drop / log vocabulary (operator UX — never a test surface)

| Cause (`cause=`) | Path | Meaning |
|------------------|------|---------|
| `topic_not_subscribed` | publish + receive | topic not in node's subscriptions |
| `topic_not_registered` | publish + receive | topic not registered |
| `publisher_not_authorized` | publish + receive | publisher key not in a non-open topic's authorized set |
| `invalid_signature` | publish (plain drop) / receive (severs) | signature does not verify |
| `duplicate` | publish + receive | content hash already in `seen` (NEW) |

All under the existing `event = "message_dropped"` info-level convention. Severance (`connection_severed`, warn) is unchanged from 004 and applies only on the receive path.

## 5. Public-surface delta (verify against `lib.rs` post-implementation)

| Item | Change | Visibility |
|------|--------|------------|
| `fanout::FanoutStrategy` | NEW trait | `pub`, re-exported from `lib.rs` |
| `fanout::ForwardToAll` | NEW struct | `pub`, re-exported |
| `received::Origin` | NEW enum | `pub`, re-exported |
| `ReceivedDelivery.from` | → `origin: Origin` (field reshape) | `pub` field |
| `Event::Publish` | NEW variant | `pub` (enum is `#[non_exhaustive]`) |
| `Node::publish` | NEW method | `pub` |
| `Node::new` | + `fanout_strategy: Arc<dyn FanoutStrategy>` param | `pub` |

Internal-only (not re-exported): `NodeState.seen`, `NodeState.fanout`, the `fanout` helper, `handle_publish`.

## 6. Acceptance traceability

| Spec item | Contract clause(s) |
|-----------|--------------------|
| US1 / FR-001..005 | §1 |
| US2 / FR-006..009 | §2.1–2.6 |
| US3 / FR-012,013,015 | §3 |
| FR-007 (verbatim) | §2.4 |
| FR-010 (seam) | §2.2, §5 |
| FR-011 (Effect::Send) | §2.4 |
| FR-014 (Origin) | §1.3, §5 |
| FR-016 (empty downstream) | §2.5 |
| SC-001/002 coverage | §2.1–2.6 + dedup §3 |
| SC-003/005 termination | §3.3, §3.5 |
| SC-004 split-horizon | §2.3 |
