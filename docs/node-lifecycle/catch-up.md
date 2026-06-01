# Catch-up / replay

**Status: deferred to the future replication layer.**

This procedure will cover **long-range** historical replay: a node that was offline for long enough that its `sequence` gaps exceed peer cache retention reconstructs missed messages with help from dedicated replication nodes (longer retention than the equivocation cache) and, optionally, periodic on-chain Merkle anchors of topic state.

For the v1 operational primitive — short-range loss recovery while a node is online, served from peers' recently-seen caches — see [gap-recovery.md](./gap-recovery.md).

Open shape questions (to be specified when the replication layer lands):

- Replication-node selection and incentive model.
- Cache retention horizon for replication nodes vs. the per-topic equivocation cache.
- Whether on-chain anchors are needed, and if so, anchor cadence and who pays.
- Replay across publisher key rotation/revocation, requiring topic-registry **history** reads (the existing Quint spec at `formal_spec/topic_registry/` may need to confirm or extend this).
- Handling of equivocation that landed during the gap.
