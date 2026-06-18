# Data Model — 006 Message Publishing and Fan-out Forwarding

Entities the feature adds or reshapes, the receive/publish decision flow, and the deliberate deferrals. State lives in the crate-internal `NodeState`; nothing here is async or I/O-bearing.

## 1. New / reshaped entities

### 1.1 `Origin` (new, `src/received.rs`)

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Origin {
    /// The node itself published this message (no wire sender).
    Local,
    /// A peer forwarded this message; the id is the delivering peer.
    Peer(PeerId),
}
```

`ReceivedDelivery` changes:

```rust
pub struct ReceivedDelivery {
    pub origin: Origin,        // was: from: PeerId
    pub message: Message,
}
```

- The publisher identity is **not** here — it lives in `message` (`PlainMessage::publisher_id`). `Origin` answers "how did this reach me," not "who wrote it."
- This is a public-surface change to `received_messages()`. Corrects the pre-existing rustdoc drift (old field said "originated" but stored the forwarding peer).

### 1.2 `FanoutStrategy` + `ForwardToAll` (new, `src/fanout.rs`)

```rust
pub trait FanoutStrategy: Send + Sync {
    fn targets(
        &self,
        topic: &TopicId,
        downstream: &HashSet<(PeerId, TopicId)>,
        exclude: Option<&PeerId>,
    ) -> Vec<PeerId>;
}

pub struct ForwardToAll;
```

`ForwardToAll::targets` returns every `peer` where `(peer, topic) ∈ downstream` and `Some(peer.clone()) != exclude.cloned()`. Order unspecified (set iteration). Pure, synchronous, no state. Mirrors `connection::{ConnectionStrategy, ConnectToAllCandidates}`.

A `#[cfg(test)]` `fanout::test_support` module holds a no-op strategy (`ForwardToNobody`, `targets` returns `vec![]`) — never compiled into the production surface. Being `cfg(test)`, it is **invisible to integration crates** in `tests/` (compiled out when the crate is a dependency), so it is usable only by **in-crate unit tests**; integration connection-lifecycle suites use the public `ForwardToAll` (fan-out does not perturb their assertions).

### 1.3 `NodeState` new fields (`src/state.rs`)

```rust
seen: HashSet<MessageHash>,          // dedup; unbounded (in-memory)
fanout: Arc<dyn FanoutStrategy>,     // service handle, beside `strategy`
```

- `seen` written only at the record point (insert on first acceptance). Read at the dedup gate.
- `fanout` set at construction; immutable thereafter.
- A `received_snapshot`-style accessor is unchanged in shape; entries now carry `Origin`.

### 1.4 `Event::Publish` (`src/event.rs`)

```rust
#[non_exhaustive]
pub enum Event {
    // … existing variants …
    Publish(SignedMessage),
}
```

Dispatched by `apply` to `handle_publish`. Pushed by `Node::publish`.

### 1.5 `Node::new` / `Node::publish` (`src/node.rs`)

- `Node::new` gains a final parameter `fanout_strategy: Arc<dyn FanoutStrategy>` (after `strategy`), threaded into `NodeState::new`.
- `Node::publish(&self, message: SignedMessage)` → `()`: `self.events.push(Event::Publish(message))`. Fire-and-forget.

> **Reconciliation with 014 (rebased 2026-06-18).** `registered_topics` is `HashMap<TopicId, TopicEntry>` (014), so the `authorized?` step below is the `TopicEntry::is_publisher_authorized` / `is_open` predicate rather than a raw `BTreeSet` check — the open/authorized *semantics* are unchanged. 014 also makes `subscriptions ⊆ registered_topics` a **maintained invariant** (strict-drop folds), so the `registered?` step is, on the subscribed path, a *defensive* guard that the invariant already satisfies (kept, matching 014's own receive path). Test setup reflects this: the `node_state` helper registers each subscription topic open, and a "subscribed-but-unregistered" state is only reachable by constructing it directly.

## 2. Publish decision flow (`handle_publish`)

```text
handle_publish(state, signed):
  ── subscribed?            (topic ∈ state.subscriptions)         else drop: topic_not_subscribed
  ── registered?            (topic ∈ state.registered_topics)     else drop: topic_not_registered
  ── publisher authorized?  (open topic, or key ∈ authorized set) else drop: publisher_not_authorized
  ── signature verifies?    (verifier over plain.signed_bytes)    else drop: invalid_signature   (plain drop — no severance)
  ── dedup: hash ∈ seen?                                          then drop: duplicate
  ── record ReceivedDelivery { origin: Local, message }; seen.insert(hash)
  ── return fanout(state, topic, &signed, exclude = None)
```

Identical to the receive chain **minus the connection gate** and **minus severance**. `publish` does not require `publisher_id == self_id` (proxy/injection): authorization + signature are the only authenticity gates.

## 3. Receive decision flow (`handle_signed_message`, extended)

The 004/013 chain is unchanged up to recording; this feature appends dedup + fan-out at the record point:

```text
handle_signed_message(state, from, signed):
  ── connected?  (Active upstream for (from, topic))   else drop: not_connected
  ── subscribed?                                        else drop: topic_not_subscribed
  ── registered?                                        else drop: topic_not_registered
  ── publisher authorized?                              else drop: publisher_not_authorized
  ── signature verifies?                                else  SEVER (Effect::Misbehaved, remove upstream) — unchanged 004 behavior
  ── dedup: hash ∈ seen?                                then drop: duplicate          ← NEW
  ── record ReceivedDelivery { origin: Peer(from), message }; seen.insert(hash)   ← origin now explicit
  ── return fanout(state, topic, &signed, exclude = Some(&from))                  ← NEW
```

The severance behavior (FR-017 of 004) is untouched and still fires *before* dedup — a tampered message over an Active upstream severs and is never seen-marked.

## 4. Fan-out helper

```text
fanout(state, topic, signed, exclude):
  targets = state.fanout.targets(topic, &state.downstream, exclude)
  return [ Effect::Send { to: peer, message: Message::Signed(signed.clone()) } for peer in targets ]
```

- Verbatim: clones `signed`, no re-sign.
- Empty `downstream` (or all excluded) ⇒ empty vec ⇒ no effects (FR-016).
- Subscriber-relay property: `downstream` only ever holds `(peer, topic)` for topics the node accepted requests on, which 004 gates on the node's own membership — so a node never fans out a topic it is not a member of.

## 5. Worked propagation (acyclic line A→B→C, US2)

| Step | Node | Event | Records | Fan-out |
|------|------|-------|---------|---------|
| 1 | A | `Publish(M)` | `{origin: Local}` | → B (A's only downstream) |
| 2 | B | `MessageReceived{from: A, M}` | `{origin: Peer(A)}` | → C (downstream; A excluded) |
| 3 | C | `MessageReceived{from: B, M}` | `{origin: Peer(B)}` | — (no downstream) |

Each records once; relay is C's sole delivery path. No dedup needed (acyclic).

## 6. Worked propagation (triangle A,B,C full mesh, US3)

A publishes M. A→{B,C}; B records (from A), B→{C} (A excluded); C records (from A), C→{B} (A excluded). Then C receives M from B → `hash ∈ seen` → **drop: duplicate**; B receives M from C → drop: duplicate; A receives M back from B and C → drop: duplicate (A marked it seen at publish). Each node: exactly one record; total forwards finite. Dedup is what bounds it.

## 7. Deliberate deferrals (catalogued)

| Ref | Deferred | Why / when |
|-----|----------|------------|
| D1 | Bounded `seen` (LRU/TTL) | No PoC consumer; eviction is deployment tuning. Real-impl milestone. New `IMPLEMENTATION_NOTES` entry. |
| D2 | Pick-k / sampling fan-out strategy | Needs a seeded RNG in state to keep `apply` deterministic. ROADMAP 006/007. The `FanoutStrategy` seam is the insertion point. |
| D3 | Equivocation detection | Distinct content ⇒ distinct hash ⇒ both propagate. N-003 / feature 012. |
| D4 | `Message::Signed` → `Dissemination` rename | Mechanical refactor; separate pass. |
| D5 | Epochal/periodic re-dial | Connections concern; re-fires the setup event. Separate slice. |
