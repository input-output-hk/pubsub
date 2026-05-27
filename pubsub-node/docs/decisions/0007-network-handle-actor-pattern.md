# ADR 0007: NetworkHandle as an actor-handle (tx/rx split, channels over callbacks)

**Status**: Accepted
**Date**: 2026-05-20
**Feature**: 001-minimal-node-scaffold
**Source**: `specs/001-minimal-node-scaffold/research.md` §12

## Context

`Network::register` returns a per-peer attach token. Three structural
choices were on the table:

1. A callback-style API where the network drives a handler on each delivery
   (`Node::on_message(impl Fn(Envelope) -> impl Future)`).
2. A stream-typed handle exposing `recv` as `impl Stream<Item = Envelope>`.
3. An actor-handle bundling a cloneable send-half (`tx`) and a
   single-consumer receive-half (`rx`), with the Node's own recv task
   draining `rx`.

The choice flows into FR-005 (one-to-one send shape), FR-006 (where the
logical peer identity is supplied), FR-010 (drop-and-warn path),
FR-011 (async send), and FR-013 (enqueue-vs-observability split).

## Decision

Use the actor-handle pattern. `NetworkHandle` bundles:

- `self_id: PeerId` (the value `register(id)` was called with).
- `tx: NetworkSender` (cloneable, crate-internal; wraps an `Arc` into the
  registry hashmap so any sender can enqueue into any registered peer's
  mailbox).
- `rx: UnboundedReceiver<Envelope>` (single-consumer drain on the
  per-peer mailbox).

`Node::new` calls `handle.take_receiver()` once during construction to
move `rx` into the spawned recv task (ADR 0006). The remaining handle —
carrying `self_id` and `tx` — stays with the Node and serves `&self`
sends.

`NetworkHandle` is intentionally **not** `Clone` (single-consumer recv
discipline). `Node::send` forwards through `self.handle.send`; FR-006's
sender attribution comes from the handle's `self_id`, never from the
caller.

## Consequences

- The substrate is shape-compatible with future networked transports.
  Lighthouse's `NetworkSenders` and Substrate's `sc_network::NetworkService`
  both expose their network surfaces as cloneable handles wrapping `mpsc`
  endpoints; this design follows the same shape so only the contents of
  `tx` and the source of `rx` change at swap time.
- The handler body lives inside an ordinary `async fn` (the Node's recv
  task loop), composing naturally with `tokio::select!` and
  graceful-shutdown patterns that a stored callback breaks.
- Lifecycle, backpressure, and cancellation are explicit: closing the
  channel produces `None` from `recv`; a bounded channel (v2) would park
  senders under backpressure.
- A `Stream`-typed adapter could layer on top later without breaking
  changes — channels remain the underlying primitive.

## Alternatives considered

- **Callback-based handler** (`Node::on_message(Fn(Envelope))`): requires
  `Arc<dyn Fn(...) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send +
  Sync + 'static>` — six type words to do what a lambda does for free in
  JS, Erlang, or Go. Async closures are second-class until further trait
  stabilisation. Lifecycle questions (when unregistered? in-flight
  invocations on drop? panic?) have no clean answer.
- **Stream-typed `recv`**: viable, adds `StreamExt` to consumer imports,
  discourages Node-internal recv ownership. Can layer on top of the
  channel-based core later.
- **Caller-driven `Network::next_for(peer_id)`**: forces a global lookup
  per recv; doesn't match the per-connection model future transports will
  have.
- **Single shared inbox on the Node (no Network-issued receiver)**: forces
  the Node to poll a Network-internal routing structure, re-coupling Node
  ↔ Network in ways `register`'s return value cleanly severs.

## Sources

Re-walked 2026-05-20 (per research.md §12's "Note on URL stability"); URLs
are floating and survey a *pattern*, not a specific algorithm:

- Alice Ryhl, "Actors with Tokio" — <https://ryhl.io/blog/actors-with-tokio/>
- Lighthouse `NetworkSenders` — <https://github.com/sigp/lighthouse/blob/stable/beacon_node/network/src/service.rs>
- Substrate `sc_network` — <https://paritytech.github.io/polkadot-sdk/master/sc_network/index.html>
- `tokio::net::TcpStream::into_split` — <https://docs.rs/tokio/latest/tokio/net/struct.TcpStream.html>
- `tokio_util::codec::Framed` — <https://docs.rs/tokio-util/latest/tokio_util/codec/struct.Framed.html>

If any link rots, the pattern claim still stands — search the repository
or rustdoc index for the corresponding type and re-anchor.
