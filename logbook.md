# PubSub Logbook

Running record of meetings, decisions, and progress. Most recent first.

---

## 2026-04-21 — Incentive model review (async)

Addressed Ezequiel's review comments on `docs/incentive-model.md`:

- Fixed Bloom filter sizing (was 10x too optimistic), added salted hash scheme against adversarial bit-saturation
- Clarified proving flow: only Merkle root goes on-chain, proof verification is off-chain
- Reframed delegation stake as identity-binding, not economic barrier; recommended sublinear quota curve
- Made escrow release concrete with UTxO spend-and-recreate pattern
- Added message size to fee formula, described sealed-bid rate discovery
- Replaced flawed "loyalty effect" with race-to-bottom risk analysis
- Grounded collateral sizing in back-of-envelope revenue estimates (~0.7–7 ADA/week per server)
- Expanded open questions to 9 (quota curve, bidding API, collateral asset, minimum rate floor, ingestion path)

## 2026-04-21 — Delivery receipts, payments, and protocol simplification

**Attendees:** Will, Denis, Ezequiel

**Key discussion points:**

- Attestation as primary mechanism for incentivising message relaying — recipients sign delivery receipts, aggregated via threshold signatures or Merkle trees
- Publisher collateral model: publisher locks ADA, operators draw from it on aggregated delivery confirmation. Ezequiel proposed optimistic attestation (vector commitment, contestable)
- Denis flagged intermediate node incentive to withhold receipts and self-propagate; mitigated by signed chain-of-custody and race-to-sign dynamics
- Verifiable credentials (DIDs) identified as key to mitigating Sybil attacks on recipient side
- Simplest viable architecture: replication servers as inbox, pull-based subscriber communication
- Agreed to centralise documentation in GitHub discussions

**Decisions:**

- Three parallel workstreams going forward:
  1. Use case discovery (Will) — reach out to Lace and others for decentralised message queue use cases
  2. Mitigation model (Denis) — continue studies, explore simple ad hoc solutions for small networks
  3. Protocol documentation (Ezequiel) — describe simple pub/sub flow (replication servers, attestations) in current report

- Regular one-hour meetings, shifted back one hour

**Open tensions:**

- Complexity vs use case: full 3-layer gossip protocol may be overkill if the base case is just SPO notifications with pull-based replication servers
- Attestation requires knowing recipients, which conflicts with anonymity goals
- Incentive model in `docs/incentive-model.md` assumes cooperative dissemination (Tier 1) — delivery receipts would change that assumption if applied to the dissemination layer

---

## 2026-04-14 — Weekly sync

- RandCast replication factor should be RF=ln(N) for probabilistic coverage
- Reverse-link idea for protocol protocol efficiency
- Open question: is full gossip overkill at SPO scale?
- Identified gap: layer security model not yet documented
