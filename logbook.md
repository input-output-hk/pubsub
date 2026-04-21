# PubSub Logbook

Technical decisions and progress. Most recent first.

---

## 2026-04-21 — Incentive model review (async)

Addressed Ezequiel's review on `docs/incentive-model.md`: fixed Bloom filter sizing (10x off), added salted hash against adversarial bit-saturation, clarified on-chain proving flow (only Merkle root on-chain), reframed delegation stake as identity-binding with sublinear quota curve, made escrow UTxO flow concrete, added message size to fee formula with sealed-bid rate discovery, replaced flawed "loyalty effect" with race-to-bottom analysis, grounded collateral in revenue estimates (~0.7–7 ADA/week/server). Open questions now at 9.

## 2026-04-21 — Delivery receipts and protocol simplification

**Decisions:**
- Attestation (recipient-signed delivery receipts) as primary relay incentive, aggregated via threshold sigs or Merkle trees
- Publisher collateral model with optimistic attestation (vector commitment, contestable)
- Simplest architecture: replication servers as inbox, pull-based subscribers
- Three workstreams: use case discovery (Will), mitigation model (Denis), protocol docs (Ezequiel)
- Documentation moves to GitHub discussions

**Open tensions:**
- Full 3-layer gossip may be overkill if base case is pull-based replication servers
- Attestation requires knowing recipients — conflicts with anonymity
- Delivery receipts would extend incentives into Tier 1 (currently scoped as cooperative)
- Intermediate nodes incentivised to withhold receipts and self-propagate; signed chain-of-custody and race-to-sign proposed as mitigations

---

## 2026-04-14 — Weekly sync

- RandCast RF=ln(N) for probabilistic coverage
- Reverse-link idea for efficiency
- Is full gossip overkill at SPO scale?
- Gap: layer security model not yet documented
