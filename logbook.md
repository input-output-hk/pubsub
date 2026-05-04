# PubSub Logbook

Technical decisions and progress. Most recent first.

---

## 2026-04-30 — Spyros sync: solution-space brainstorm

**Scope confirmed unidirectional.** Spyros agreed pubsub is fundamentally single-directional. A bidirectional response back to the publisher can be done point-to-point, but at large subscriber counts that pattern degenerates into a DOS shape — reinforcing the decision to defer multi-sig-style flows.

**Eclipse analysis on the Harary graph.** Denis presented early Byzantine analysis. Even without identity grinding, churn alone makes the D-links insecure; R-links are the real security backbone because they rotate. With fan-out F=2, the lower bound on eclipsing probability converges to a non-negligible constant (~13–20%) as N→∞. Combinatorial, not asymptotically vanishing.

**Identity assumptions.** Spyros pushed on whether grinding (Sybil-style identity selection) was assumed. Will proposed making it a core protocol assumption that nodes cannot manipulate their position in the Harary graph — candidate mechanisms: stake-linked identity, VRFs.

**No incentive model — by design (so far).** Spyros confirmed pubsub work has no incentive layer; the focus has been organisation for fast/reliable dissemination. SecureCyclon's philosophy is non-bias: a network with X% malicious nodes yields X% malicious links, preventing over-representation but not protecting against silencing.

**Silencing threat acknowledged.** Even with a well-formed overlay, malicious nodes can drop messages for topics they disagree with. Spyros agreed this is not addressed in the current design. Two mitigations surfaced, both pointing the same way: apply the scheme only among trusted nodes (e.g., SPO relays), or keep a trusted core for reliability while using the open scheme for scale. Ezequiel noted this dovetails directly with our trusted-relays extension from 2026-04-28.

**Layer information flow.** Spyros walked the three layers: SecureCyclon acts as an oracle, periodically supplying descriptors for randomly picked live nodes; the navigation layer keeps links to nearby topics to speed convergence; the dissemination layer forms the topic-specific Harary overlay. Each higher layer can keep or discard descriptors flowing up.

**Message sequencing already in scope.** The architecture's persistent-storage component assigns IDs by (topic, publisher, sequence number). Gaps in the sequence let a node detect missed messages and pull them from storage — which we'll lean on for delivery detection rather than building something new.

**Delivery attestation — Spyros skeptical.** Will floated subscriber-signed receipts feeding a publisher-funded payout. Spyros's intuition: positive incentives are weak when adversaries are willing to absorb a financial loss to silence a group. Worth keeping but not load-bearing. Ezequiel also flagged the tension with replication servers: if subscribers can just pull, they bypass the dissemination structure entirely.

**Next steps.** Spyros to review the gap analysis offline and the documented vulnerability list (especially attacks on SecureCyclon). Future syncs will be short, ad-hoc 15–30 min sessions coordinated via Slack rather than long written exchanges. Spyros has a CSM time-conflict but can occasionally step out.

---

## 2026-04-28 — Weekly: trusted relays + unidirectional scope

**Scope decision.** We're focusing strictly on simpler, unidirectional use cases and deprioritising the bidirectional multi-sig branch. The primary set going forward is:

- IOG alerts → SPOs
- dApps → users
- SPOs → delegators
- dReps → delegators

To this we'll fold in the three Phillip Daro confirmed:

- Security pings (alerts/notifications)
- Data availability
- Off-chain signature broadcasting — e.g., updating the price of a limit order via a signed message without submitting an on-chain tx; only the eventual fill settles on-chain. Conceptually similar to Sunday Swap's signer/scooper pattern, where our protocol just provides the channel from delegated signer to batcher.

**Trusted relays extension.** Ezequiel proposed simplifying delivery in two stages. Stage 1 hardcodes the topology with a list of IP addresses for key actors (e.g., wallet backends), guaranteeing delivery by construction. Stage 2 introduces "relay nodes" — a set of on-chain identifiable, trusted (or "sort of trusted") parties operating at much higher fan-out, adding pathways for honest delivery on top of the base topology.

The proposal landed well technically but raised concerns. Will flagged that it introduces an implicit tiered delivery: nodes that don't share a neighbour with a trusted relay see degraded guarantees, even when headline numbers look fine. Denis noted the proposal fits "beautifully" with SecureCyclon, where trusted nodes are the answer to the eclipsing-attack gap — though he cautioned that the formulas we'll derive assume a perfect uniform view, and SecureCyclon's real bias will need to be folded back in later. Not catastrophic, but it'll be embedded in the final results.

**Modelling parameters.** Denis will model the relay network using N (total nodes), K (number of trusted nodes), fan-out for normal nodes, fan-out for super-trusted nodes, and the number of malicious nodes. Initial bracket from Will: N ≈ 20–30k, with K from zero or one up to ~10 depending on the use case.

**Open thread — signing keys.** Jesus to scope what key material SPOs, dReps, and dApps can use to authenticate messages — whether existing keys (stake, dRep) suffice or whether new keys are needed. The question is primarily application-specific, but relay nodes may also do verification checks.

**Prototyping kickoff.** We're starting on an Aiken on-chain topic registry as the stable foundation we can build on, with the understanding that the design will need to be extended once trusted relays integrate. Ezequiel sketched a progressive plan: registry plus nodes plus direct link first, three-layer protocol added on top later. SRL-wise, the use-case work suggests we're near or just past SRL 2; reaching SRL 3 will need a full requirement spec, outlined design, and a published proof of concept (simulations qualify).

**Spyros sync.** Will invited Spyros to Thursday's (mostly technical) meeting to discuss parameterisation, especially fan-out. Will also received a link to Spyros's three-layer protocol implementation — plan is to extract it, push to a branch, and aim to have it running by tomorrow.

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
