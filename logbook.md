# PubSub Logbook

Technical decisions and progress. Most recent first.

---

## 2026-05-28 — Brainstorm with Spyros: SecureCyclon verdict and the path forward

**Modular split praised.** Spyros endorsed the orthogonality between the sampling and dissemination layers in the technical report. The structure lets the team defer or drop individual components without disturbing the overall engineering plan — a pragmatic foundation he found worth keeping.

**ID-grinding and dynamic ring positions.** Spyros pressed on adversary placement in the vicinity ring: an attacker who can pick their own descriptor can park near a target. Will noted the paper already assumes a secure node-descriptor procedure rather than free ID selection. Spyros's proposal: derive each node's ring position by hashing its public key together with global epoch randomness (e.g. Cardano epoch nonce), so the topology rotates each epoch. A delay of at least one epoch between descriptor commit and use prevents immediate exploitation. Open trade-offs: per-epoch recomputation cost, and the need for a barrier to entry (proof-of-work or similar) to keep descriptor churn bounded.

**Fan-out: 2 is the floor, 4–5 is the practical setting.** Denis raised that small fan-out makes interval capture easier even with frequent restructuring. Group consensus: the architecture document's fan-out of 2 is a minimum; production should run 4 or 5 to improve robustness without blowing up network overhead. Targeted eclipsing was discussed — if the ring is continuously rotating, an attacker cannot reliably choose *when* and *who* to mute.

**List-based does not preclude dynamic ring formation.** Spyros pushed back on framing list-based as the static alternative to peer sampling: even with global subscriber knowledge, the ring can still be formed dynamically each epoch by hashing IDs with the epoch seed. Same anti-positioning properties, simpler bootstrap.

**IP discovery stays off-chain.** Denis and Will confirmed the design intent: no IP addresses on-chain. Spyros sketched the hybrid model and Will confirmed alignment — on-chain topic registry and subscription list validate public keys; SecureCyclon/vicinity-style mechanisms (or their replacement) handle IP exchange at the network layer.

**Incentive layer is not as fragile as feared.** Spyros challenged the claim that navigation-layer incentive misalignment is a critical threat. His argument: even if one topic-group boycotts another, most nodes are indifferent to other topics and the system stays functional. Denis countered that security analysis must cover pessimistic edge cases. Both agreed that naive encryption does not solve principal-based incentive problems.

**Biased views vs link dropping.** Spyros sketched a bias mitigation: combine node IDs with a hash-derived random seed to force fair peer selection, optionally retain view history for verification. Denis flagged the computational cost and scaling concerns. The group identified **link dropping** by malicious nodes as the more impactful silent attack than biased view presentation — biased views can be detected and bounded; dropped links degrade delivery directly.

**Adversary amplification numbers reaffirmed.** Denis and Spyros walked the prior result: 15% malicious nodes drives the network to an equilibrium with ~8.9× amplification of adversarial influence — the phase shift past which the graph is dominated by the adversary view.

**Property D1.3 — uniformity is for analysis, not a hard requirement.** Denis clarified that indistinguishability from a uniform graph (D1.3) exists to make analytical modelling tractable. Spyros pushed on whether uniform randomness is the right goal at all, suggesting perfect-matching or other structures might perform better in practice and could be validated by large-scale simulations even where closed-form proofs are out of reach. Denis's bottom line: the protocol's output distribution must be *well-defined*, not necessarily uniform — without a known distribution, security bounds for the dissemination layer cannot be derived.

**Verdict on SecureCyclon (post-meeting, Denis).** After Spyros left, Denis stated the conclusion plainly: SecureCyclon is not viable for high-adversary environments like blockchain. The threat models published with these protocols are unrealistic for settings with financial incentives and reputational stake. Cyclon-family protocols may work for campus or research deployments but break down under adversarial fan-out. Two forward paths identified: (1) keep iterating on peer sampling — either fix SecureCyclon or search for a stronger protocol; (2) refocus on the dissemination layer and analyse its churn properties directly.

**Decisions.** *Aligned:* collaborate with Spyros on a more secure peer-sampling layer as a joint research publication. *Open:* whether view commitments are feasible and worth their cost as a mitigation for silent attacks and the link-drop problem.

**Project workstream clarification.** Will distinguished the two threads currently in play: improving the stack from the paper Spyros previously discussed, and a list-based implementation approach for surfacing concrete issues in the current system. The PR being shared is the latter and is unrelated to peer sampling.

**Next.** Will to add Spyros (GitHub handle provided) as reviewer on the architecture PR. Spyros to review the technical report and PR offline, simulate the bias-view behaviour against the reported malicious-node percentages, and examine the subscription-list prototype after his teaching class concludes. Spyros to share the older mathematical-analysis paper on the standard Cyclon protocol (Denis confirmed it has been located). The group to develop a testable formula sufficient to establish dissemination-layer security.

---

## 2026-05-26 — PubSub working session: node-flow spec, June scope, scaffolding

**Technical report circulated.** Will shared the draft technical report on Slack ahead of the meeting. It captures Denis's three-property analysis of Cyclon (two hold, the third is falsified) and the SecureCyclon defence inventory along with the attack vectors those defences fail to cover. Feedback requested on whether the report is complete or needs further development.

**Cyclon direction split — decision deferred.** Denis reported diverging views from co-authors: Jesus remains optimistic about salvaging the current protocol; Sandro is pushing for a literature sweep or a clean-sheet design. The patch-vs-rebuild call is deferred pending Will's Thursday consult with Spyros.

**Node-flow specification adopted as joint workstream.** Will identified five PubSub procedures worth formalising: joining, leaving, publishing, creating a topic, and changing topic subscriptions. He walked the joining flow — operator key-pair generation, on-chain registration check, bootstrap connect. Ezequiel flagged this overlaps the scaffolding effort already in progress; rather than parallel tracks, the two will co-specify these flows using the Spec Kit framework so research specs and implementation stay coherent.

**Scaffolding PR update.** Ezequiel's in-progress PR ships an in-memory network keyed off a hashmap, config-file reading, and basic message send/receive. Topic management, message structure, cryptography, and connection logic are the next iteration. Plan: merge the scaffolding PR, then publish a meta-spec describing the follow-on work so feature expansion has an explicit target.

**Product focus shifts to incentives.** Dana reported limited Stake Pool Operator appetite for new modules from the parallel Mithril PubSub conversations. Will argued the product team should pivot from feature checklists to fees and incentive design — give operators a concrete reason to run a node and a real cost for misbehaviour. Dana to prepare an incentive-model update for next week.

**IP discovery edge cases.** Will raised two open questions: how nodes should handle peers that are registered but offline, and the minimum connection count required to maintain delivery guarantees. Ezequiel's preferred handling: cycle through candidate peers and use a bidirectional handshake to qualify connection quality, rather than altering the core protocol structure. Will to write up the IP-discovery process for the technical report.

**Registration-list semantics.** Discussion on whether offline nodes should remain on the on-chain registration list. Denis: for the current prototype, fan-out is dynamic in network size and rejection sampling adequately handles the distribution of online nodes — no need to evict offline entries from the list at this stage.

**Publishers run a subscribed node.** Will's working position for the publishing spec: a publisher runs a node subscribed to the relevant topics so signing and routing inherit the dissemination layer's guarantees. Ezequiel agreed this is the logical shape for the dissemination layer. Will to draft the spec and circulate.

**June scope: prototype plus technical report, not a CIP.** Team agreed an end-of-June CIP is not achievable; the deliverable is a working prototype plus a technical report capturing the research findings. Funding for the work stream is unsettled — Will may need to present to leadership on Thursday to justify continuation. Ezequiel flagged the broader governance backdrop: of roughly 20–30B in distributed voting power, only ~4–5B is actively voting, and weight is currently a function of financial contribution rather than expertise.

**Decisions.** *Aligned:* node-flow specs to be co-developed via Spec Kit; June deliverable is prototype + technical report, not a full CIP. *Open:* whether to patch or abandon SecureCyclon, pending Spyros's input.

**Next.** Will + Ezequiel to define sequence diagrams for joining, leaving, and publishing, aligned with the scaffolding. Ezequiel to merge the scaffolding PR, publish the meta-spec, and implement topics, message structure, cryptography, and connection logic. Dana to research incentive models. Denis to analyse SecureCyclon mitigation strategies ahead of the Spyros session. Will to document IP discovery in the technical report, share project specs on Slack, consult David on whether a full CIP is expected, distribute the internal stream report, and notify the team after Thursday on funding and direction. Ezequiel to review existing project documentation and connect current coding work to the established requirements.

---

## 2026-05-19 — PubSub working session: list-based architecture adopted

**Decision.** Team aligned on collapsing peer sampling and navigation into an on-chain subscription list, one entry per node carrying its topic-interest set. Dissemination layer unchanged. Sandro reviewed the framing and supported list-based as the initial step. Findings and rationale captured in [docs/technical-report-1.md](docs/technical-report-1.md). Two alternatives weighed but not taken: continued research on three-layer extensions, and parallel SecureCyclon instances per topic.

**First step, not endpoint.** Denis emphasised the list-based shape is a starting point. The research handoff is the *multi-peer-sampling problem*: given a network where each node carries a topic-interest set, design a protocol that lets a node sample uniformly from the subscribers of any topic without holding the full list. Future substitution then becomes a module swap rather than a re-architecture.

**Local cheating.** Full per-topic visibility makes uniform sampling trivial but allows operator-side deviation from the prescribed sampling that no peer can detect. Acknowledged cost; removing it is what the multi-peer-sampling protocol buys.

**Off-chain endpoints, trusted bootstrap.** Endpoints stay off-chain. New nodes ask bootstrap nodes for endpoints matching their topic interests, cache locally, and propagate updates over dissemination links. Bootstrap nodes are treated as trusted infrastructure for the initial deployment: explicit, narrow, revisable. ID-grinding still needs an anti-grinding mechanism (per-epoch Cardano nonce a candidate).

**Open: identity anchoring.** Jesus flagged on-chain lookup cost for sub-keys signed by SPO/DRep certificates, especially under parallel SecureCyclon. The system already depends on chain data; what is needed is efficient indexing.

**Prototype scope.** Ezequiel focusing on a static peer list with basic communication contracts and a testing harness; discovery, dissemination, and navigation deferred. PR for the architecture specifications going out today.

**Research framing.** Jesus suggested defining the ideal functionality via universal composability. Group preference: hand research specific requirements anchored on the multi-peer-sampling primitive rather than free rein, so the future protocol composes with current progress.

**Next.** Denis to finish quantifying the silent-attack analysis (descriptor drop, biased view) by Thursday. Ezequiel to develop a Python script on network size against security, particularly the constant-nodes ("golden nodes") regime. Will to document joining and peer-discovery flows and sketch Thursday's brainstorm topics. Dana confirmed DataDog, Prometheus, and GL Live View integrations look tractable on the product side.

---

## 2026-05-12 — PubSub working session: Cyclone Property 3, SPO onboarding

### Cyclone protocol analysis

**Why these properties.** The dissemination security analysis (Ringcast eclipsing bounds and similar) routinely substitutes *"sample uniformly from a node's view"* for *"sample uniformly from the network"*. That substitution is load-bearing — if Cyclone doesn't actually produce uniform-looking views, the bounds derived on top of it are unsound. Rather than assume what Cyclone delivers, Denis worked through it from first principles: start with the weakest statement that *should* hold if Cyclone is doing its job, then progressively strengthen it and probe each level by simulation, since the protocol is too mechanical for clean analytical proof. The aim was to find the strongest property Cyclone actually satisfies, and check that it's strong enough for the downstream analysis.

**The three properties Denis tested.** Three statements of increasing strength.

*Property 1 — descriptor marginal uniformity.* For all distinct nodes $u, v \in V$:

$$\Pr_{\text{Cyclon}}\left[u \in \text{view}_v\right] = \frac{c}{N - 1}$$

The weakest of the three: each node has the same marginal probability of appearing in any other node's view. It says nothing about correlations *within* a view — two entries of $\text{view}_v$ may still be jointly biased. Simulation: holds with very high confidence.

*Property 2 — view distribution uniformity.* For every $v \in V$:

$$\text{view}_v \;\sim\; \text{Uniform}\bigl(\,\{\, S \subset V \setminus \{v\} \;:\; |S| = c \,\}\,\bigr)$$

Strictly stronger than Property 1: each individual view is a uniformly random size-$c$ subset of the other nodes. This closes the within-view correlation gap. It still leaves open correlations *across* views — the joint distribution of $(\text{view}_u, \text{view}_v)$ can deviate from independence. Simulation: also holds with very high confidence.

*Property 3 — graph distribution uniformity.* Let $G_{N,c}$ be the set of all $c$-out digraphs on $V$. Then:

$$\text{Cyclon} \;\sim\; \text{Uniform}(G_{N,c})$$

The strongest: the whole graph Cyclone produces is statistically indistinguishable from a uniformly random $c$-out digraph. This is the property that licences the substitution *"sample from view ≡ sample from network"* used in the dissemination security analysis.

Implications: $\;3 \Rightarrow 2 \Rightarrow 1\;$ (strongest to weakest).

**Property 3 fails — distribution gap grows with $N$.** Simulation indicates Properties 1 and 2 hold, but Property 3 does not. The gap between Cyclone's graph distribution and $\text{Uniform}(G_{N,c})$ grows asymptotically with $N$. Downstream consequence: the view-as-proxy-for-network substitution is incorrect, and the inaccuracy worsens at the scales we care about.

**Root cause: oldest-descriptor selection.** Spyros traced the source: picking the oldest-aged descriptor to gossip with and then expending it produces a narrow lifetime distribution per descriptor, which narrows in-degree variance below the random-graph baseline.

**Fix A — random descriptor selection (partial).** Spyros proposed selecting a descriptor at random instead of the oldest. Denis's follow-up simulation: this halves the variance gap between Cyclone and uniform, but does **not** fully restore Property 3 — the asymptotic gap persists, just smaller.

**Fix B — Poisson-distributed exchanges per cycle (full).** Denis also tested replacing the hard-coded "one exchange per cycle" with a Poisson-distributed number (mean 1, but variable). Simulation shows this fully restores Property 3 — and once Fix B is in place, Fix A (the descriptor-selection rule change) becomes unnecessary. Cost: Cyclone's main defence against hub attacks is detecting *frequency violations* — peers initiating exchanges more often than they should. Allowing a Poisson-varying number of exchanges per cycle erodes the signal that detector relies on. So Fix B trades a structural-uniformity gain for a degraded hub-attack defence.

**Dissemination layer needs both randomness and determinism.** Spyros: Ringcast requires randomness (robustness, exponential spread) plus determinism (the Harari structure that guarantees full coverage). Property 3 is the random-link half — if it can be recovered, the rest of the Ringcast guarantees follow. Denis's next analytical step is eclipsing-attack bounds redone without the uniform-view assumption, folding in Cyclone's real bias.

**Per-topic views lose the property.** Ezequiel flagged that with dissemination organised per topic via the navigation layer, per-topic views don't inherit Cyclone's uniformity even if the core property holds. Spyros agreed; the mitigation, if needed, is to run a separate Cyclone instance per topic — costly but available.

**Byzantine descriptor-drop bounded.** Ezequiel raised the case of a malicious peer absorbing a descriptor and not forwarding it — eclipsing the originator by one descriptor. Spyros confirmed this is real but bounded: $K$ outgoing links makes a fully-malicious neighbourhood unlikely, and the originator re-shares next round.

*(Spyros departed at this point; discussion shifted to product.)*

### SPO onboarding and product strategy

**Minimise friction for SPOs.** Dana presented the product doc and argued for auto-enrolling SPOs into the emergency-alert channel — friction kills the foundational use case. Team converged: ship a minimal cost-free SPO alert path first, demonstrate utility, then layer cost structure on for the broader publisher set (DReps, SPOs, dApps) once value is established. Will's suggestion to ship the SPO path as a lightweight feature inside the existing Haskell node — rather than a separate process — was accepted. Ezequiel's counter: for emergency alerts specifically, the alert channel must remain separable from the main network, otherwise a network-failure alert can't reach you when the network is failing.

**Cost-per-identity still required for non-SPO publishers.** Ezequiel reiterated that a per-identity token deposit remains a security/slashing primitive for the publisher set generally — the minimal-friction SPO path doesn't remove that requirement for DReps and dApps.

**SPO tooling integration.** Dana noted SPOs primarily run Prometheus, DataDog, and G Live View; she'll investigate how alerts can surface in those dashboards, plus Twitter/Discord bot relays for smaller SPOs. Subscription mechanics for wallet and dApp providers also on her plate.

### Identity and key management

**Reuse the SPO on-chain deposit.** Will proposed leveraging the existing SPO on-chain registration (~500 ADA deposit) as the legitimacy signal — no additional transaction needed.

**Hash the pubkey for node ID.** For the pubsub node ID itself, hashing a public key (e.g., ED25519) gives a uniform fixed-size ID that's independent of which key material is reused (SPO, DRep, dApp operator). Jesus flagged the cost: this requires real key-management logic to support rotation.

**Don't reuse sensitive operational keys directly.** Ezequiel pushed back on reusing SPO/DRep operational credentials as pubsub hot keys — a delegation step from cold key to pubsub key is likely needed. Jesus to scope anonymous/pseudonymous credentials in parallel.

### Algorithm documentation

**Draft exists, with known gaps.** Jesus has a draft of the secure Cyclone algorithm covering peer sampling, navigation, and dissemination layers — each with select / send / receive / run interfaces. Known errors and simplifications remain.

**Moves to GitHub for collaborative editing.** Jesus uploads the LaTeX sources to the repo and grants attendee access; Will sets up a CI workflow for LaTeX → PDF auto-conversion; Will and Ezequiel review and report unclear or incorrect content.

### Next

Denis to weigh Fix A vs Fix B given the hub-attack-detection cost of Fix B, and report back. Spyros to share the Cyclone paper covering the analysis and the proposed fixes. Dana to research SPO dashboard integration (Prometheus, DataDog, G Live View) and subscription mechanics for wallets and dApps. Jesus to upload LaTeX sources, fix known errors in the algorithm doc, and scope pseudonymous credentials. Will to set up the LaTeX → PDF workflow. Will + Ezequiel to review the algorithm doc.

---

## 2026-05-08 — bucketing proposal to limit flood attacks

**bucketing proposal analysis report.**
- Proposed Sybil cost via on-chain deposit D; for a budget β, an attacker can create up to K ≤ β/D identities.
- New protocol parameter B (number of buckets): we added a per-(round, topic) hash bucketing approach to gate pulls — cuts attacker concentration at any victim from K to K/B per round.
- Deposit floor, D, binds on `min(eclipse k_max, flood K_max)`; we can compute optimal based on curves intersection.
- Some slashing surfaces identified: bucket mismatch, duplicate (id, round), overcapacity reports.
- Small-topic regime degrades gracefully; falls back on relay-tier and local-relays extensions.
- Write-up in `docs/bucketed-pull-gist.md`.

---

## 2026-05-07 — Spyros brainstorm: extensions reviewed

**Pull-based dissemination approved.** Spyros endorsed flipping in/out degree — each node requests forwarders once per epoch. Asymptotic O(N) security improvement, dissemination metrics preserved. He explicitly liked receiver-selects-provider as the right primitive.

**Forwarder-flooding mitigations.** Three converging proposals on the table: VRF proof of honest forwarder selection (Denis); deterministic bucket assignment via hash(epoch nonce, ID, topic) mod B, with the contacted peer cross-checking the match (Ezequiel); equivocation proof via signed assignments — gossip the assignments, blacklist any node that double-assigns a slot (Spyros). All require a high identity-creation cost so the attacker's outbound budget stays bounded.

**Epoch nonce as rotation source.** Will proposed feeding the Cardano per-epoch nonce into the Harary graph layout, forcing a rearrangement every ~5 days and bounding any successful grinding to a single epoch.

**Golden nodes — Spyros caveat.** Trusted relays must source their data reliably; Spyros's pushback was that they should have hardcoded interconnections among themselves so they can't be fed false information from outside the trusted set.

**On-chain IPs flagged.** Ezequiel raised that putting IPs on-chain is undesirable — IPs change, and on-chain identification makes nodes DOS targets. Open question for the bootstrap design.

**Next.** Will + Ezequiel kicking off a small-scale Rust prototype to surface implementation gaps. Spyros to review the documented vulnerability list offline.

---

## 2026-05-05 — Weekly: pivot to concrete output

**From research to prototype.** SRL 2 essentially cleared via the existing research and Denis's synthetic experiments; we're on the path to SRL 3. Decision: stop generating ideas, start generating output. Workstream split — Ezequiel prototypes a simplified spec, team documents the full spec, Denis identifies remaining experiments. Target: an end-to-end prototype deployable to a test net within under two months.

**SecureCyclon is the load-bearing assumption.** Denis: the dissemination-layer patches combine well and quantitatively improve security, but the whole stack rests on SecureCyclon being sufficient. Any implementation must explicitly call this out. SL 4 priority is therefore formal analysis of SecureCyclon — deficiencies there force re-thinking the stack.

**Layer collapse on the table.** Ezequiel exploring whether the three layers can be simplified. With a bootstrap list in the topic registry, the navigation layer may become removable, eliminating several attack vectors. Confirmed his "trusted relays" and Denis's "golden nodes" are the same concept — two proposals for one extension.

**RF and N estimation.** Replication factor's good values are logarithmic in N. SecureCyclon supplies only a partial view, so we need a mechanism to estimate order-of-magnitude N over time; an initially high RF buys headroom. Denis's analysis also requires nodes to tolerate ~2× the expected downstream load rather than rigidly rejecting requests.

**Anti-grinding.** Ezequiel surveying cheap PoW in requests and IP-bucketed view slots — both raise the cost of occupying many slots without a globally distributed IP set.

**Algorithm unification (Jesus).** Mapping the Cardano architecture, SecureCyclon, and vicinity paper algorithms into a common p2p interface (setup / select / send/receive per layer). Target property for SecureCyclon: when a higher layer requests a random node, the sample is uniform over the full "god view"; anything weaker breaks the security analysis.

**Bootstrapping.** Don't reinvent — reuse seed-peer patterns from Cardano/Bitcoin; topic registry is a viable candidate. We'll catalog the prototype's explicit and implicit assumptions (e.g., hard identities) up front.

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
