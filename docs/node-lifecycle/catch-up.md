# Catch-up / replay

**Status: TBD — placeholder.**

This procedure will describe how a node that was offline catches up on messages it missed while disconnected. It is expected to rely on the per-`(topic, publisher)` message chain (`parentHash` + `sequence`) already defined in [publishing.md](./publishing.md), so the type carries the chain fields from day one even though relayers do not yet enforce them.

Out of scope for the first iteration; covered by the deferred replication layer (see `docs/list-based-architecture.md` §7).
