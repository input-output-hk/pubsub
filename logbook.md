# PubSub Logbook

Technical decisions and progress. Most recent first.

---

## 2026-08-20 — Brainstorm: M4 selected as the dissemination design, oversaturation for small topics, bucket count as a published parameter

**M4 is the dissemination design.** The group selected M4 over M3 and M5, on the strength of its symmetric link structure and the structural advantages that follow from it. That confirms the direction the CIP draft already takes rather than changing it.

**Small topics are answered with redundancy, not with tuning.** For topics on the order of ten participants the agreed approach is oversaturation — let the topology form a clique — rather than adjusting parameters dynamically for small networks, since the structure moves away from a clique of its own accord as a topic grows. Simulations are still needed to price this in connections per node, because a small topic pays proportionally more per node than a large one.

**Bucket count as a published parameter — a direction, not yet a mechanism.** The bucket count B relates to network size and the nominal attacker budget, so it would need revising if a topic's population shrinks. Publishing it on chain, potentially per topic, was discussed as the route to updating it as network size fluctuates, with the specification staying high level about how that happens. The detail is unresolved and this is recorded as a direction under discussion rather than a settled mechanism.

**The documentation is too dense to read comfortably.** The specification and rationale text as it stands was found hard going. Agreed direction: plain, clear, human-readable prose is the primary artefact, with dense data and detailed results kept separately for technical reference.

**The CIP draft goes out for review.** A formal CIP draft is being prepared for team review and will be submitted as a pull request, with the group reviewing once it is available. Two follow-ups on the experiments side: remove a statistical bias in the reported experimental results, and circulate the probability computation. The identity documentation is to be reviewed for possible refactoring.

**Decisions.** *Aligned:* M4 is the selected dissemination design; small topics are handled by redundancy and oversaturation rather than size-dependent parameter tuning; human-readable prose in plain English is the primary documentation artefact, with dense data held separately. *Needs further discussion:* whether the bucket count is published on chain, and per topic, with the specification staying high level about it.

**Next.** Prepare the formal CIP draft and submit it as a pull request for team review; simulate the per-node connection cost of oversaturated small topics; remove the statistical bias from the reported experimental results and circulate the probability computation; review the identity documentation for possible refactoring. The group: review the CIP draft once available.

---

## 2026-08-13 — Brainstorm: security-first weighting, canonical hash gating, and what a chain fork does to an epoch topology

**The instrument and the laws now agree.** Will reported a week and a half of near-continuous simulation, including long overnight sweeps, and the outcome is that the closed-form laws and the simulation results line up closely once the residual noise and bias are removed. That removes the last standing doubt about whether the comparison figures are an artefact of the measurement rather than a property of the models.

**Security first, efficiency second — the comparison gets an ordering.** The open question going into the session was how to turn seven measured axes into one decision. Denis Firsov proposed evaluating candidates lexicographically, or with explicit weights, and argued that security must be the primary metric with efficiency strictly secondary. Ezequiel Postan agreed that security is the most relevant factor for the decision at hand. That is the session's one firm alignment: candidate models are compared by weighted properties, security weighted first. Will takes the follow-up of choosing weights that can be defended rather than asserted, and will fold them into the existing comparison.

**What the CIP is being asked to settle.** The group revisited the proposal's scope — the dissemination model, the structure of the registration network, node identity, and address discovery. Will's position is that the document should narrow to a single dissemination model rather than present a family; the group has not closed on that and continues to weigh whether carrying more than one candidate is the more honest presentation given what the evidence currently supports. Mauro Jaskelioff raised whether the property set itself is complete, pointing at multi-topic scaling as an axis that is not currently measured and on which M4 may well come out behind M3.

**Parameters are not equally changeable after deployment.** Ezequiel made the practical point that the replication factor and the bucket size are not interchangeable dials once a network is live — changing one is considerably cheaper than changing the other. That reframes the parameter discussion around headroom: how far a network can grow, and how far the adversarial fraction can drift, before the configuration has to be revised at all. Which input parameters drive that headroom is now part of what the comparison has to say.

**Why M4 tolerates losing bidirectional links better than M3.** Will flagged as surprising that M4 degrades more gracefully than M3 when bidirectional connections are lost. Ezequiel's explanation is structural rather than incidental: a bidirectional link in M4 serves in both directions, so the model has effectively twice the usable degree of M3's unidirectional relay arrangement at the same nominal count. The same reasoning explains the broader observation that the symmetric models are performing better than the group had assumed when they were first sketched — the shape of the results is the one the symmetric structure predicts.

**Hash gating loses the 1/B property when links go symmetric.** The sharpest technical result of the session. Ezequiel detailed how hash gating behaves once links are bidirectional: because a pair can be admitted from either side, the current construction doubles the probability of ending up connected to an adversary, which is precisely the guarantee the gate exists to provide. The repair is to gate on the canonical, identity-sorted pair rather than on a direction-dependent value, which restores the one-over-B density. It is not free: the guarantee depends on the candidate pool staying a few times larger than the pick count, so the bucket count has to be derived to preserve that headroom rather than chosen independently.

**Where the minimum network size actually falls.** The bucket-count consequence lands on the small-network gap named in the previous session, and the direction is worth stating carefully, because the session's working assumption was the other way round. The gate switches off once the bucket count cannot reach two, which happens below roughly four times the pick count: about thirty-seven participants for M4 at a pick count of nine, and about fifty-three for M3's relay kind at thirteen. On that measure the symmetric model reaches further down a small topic rather than less far. The concern raised in the session — that symmetry forces a wider bucket and so a higher floor — holds for the direction-dependent gate, which needs twice the bucket count for the same pair density; it does not hold for the identity-sorted pair the measurements went on to select. What symmetry does add is a floor on the candidate pool that a larger fanout cannot buy back, where the directional gate's headroom rule can be traded against the pick count. Denis suggested the answer is not a single parameter set but several, selected by network size, so that guarantees survive a network transitioning between growth stages. Will will document the gaps for networks below those thresholds for review in a later session.

**Acceptance caps mean something different under symmetry.** The cap on how many connections a node will accept is less strictly defined in the symmetric model, because an incoming request that results in a bidirectional connection is not the same object as an incoming-only request and need not be charged against the same budget. Ezequiel took the action of pinning down what connection state a node actually has to track to make the distinction — and whether symmetric links need additional state management at all — before the hash-gating experiments can be run meaningfully. Since settled, in ADR 0042 on [PR #177](https://github.com/input-output-hk/pubsub/pull/177): the cap becomes an *admissions budget* that bounds only edges a peer initiated, so a node's own selections can never be vetoed by a flooder, and total degree is bounded by the pick count plus the cap.

**Epoch randomness: unbiased against stable.** The group worked through where the per-epoch randomness comes from and found the tension is inherent. Randomness must be unbiased, or an adversary grinds toward a favourable topology; but it must also yield a topology stable enough to be worth building. Chain-derived randomness such as block hashes is the obvious source and carries the obvious weakness: an adversary who can predict the next beacon gets advance knowledge of the next topology.

**Chain forks, and the case for not depending on the chain.** A fork causes nodes to compute different epoch topologies from different views, splitting the overlay along the same line. Denis's framing is the load-bearing one: PubSub exists for emergencies, so it has to keep functioning when the chain halts or forks — which is exactly when a chain-derived topology would be least trustworthy. Two mitigations came out of this. First, topology cadence: Will suggested nodes be allowed to retain connections from previous epochs rather than tear everything down at a boundary, so a node that draws badly is not muted or left at a dead end; Ezequiel noted heartbeats could support intra-epoch rotation, re-establishing connections under a fresh seed without waiting for a full epoch transition. Second, manual trust tables: a node keeps a list of known peers with which it establishes bidirectional connections that are immune to a fork, because they were never derived from chain state. Neither is a guarantee, and the group was explicit that they are heuristics — but they are documentable, and the partition resilience they buy is measurable. Ezequiel took an analytical follow-up on the other side of the same question: the probability that nodes on opposite sides of a partition fail to connect at all, and how many random connections are enough to make that risk negligible.

**Bidirectional links in Cardano networking — an open question of provenance.** Will raised why the Cardano networking team chose bidirectional links at the start of their work, and whether that choice was made to prevent a specific failure this project has not yet named. Worth knowing before settling on a symmetric model for the same reasons, or for different ones. Will will approach the networking team directly.

**Decisions.** *Aligned:* candidate models are evaluated by weighting their properties, with security metrics weighted ahead of efficiency. *Needs further discussion:* whether the proposal narrows to a single dissemination model or presents more than one; whether multi-topic scaling joins the measured property set.

**Next.** Ezequiel: run the experiments for both flavours of M4 hash gating to inform the specification section — since opened as [PR #177](https://github.com/input-output-hk/pubsub/pull/177); determine what connection state symmetric links require before those runs; compute the cross-partition connection probability analytically. Will: document the design gaps for networks below the size thresholds; add heartbeat and manual-trust connections to the CIP as implementation heuristics against partitions; define defensible weights for the comparison metrics; ask the Cardano networking team about the original rationale for bidirectional links. The group: select the topology model, on the simulation data and the experiments now in flight.

---

## 2026-08-11 — Weekly session: the CIP becomes a technical report, M4 leads on six axes, small networks named as the gap

**The CIP is a draft technical report, not a specification.** Ezequiel argued the document should say plainly what it is: a report of what the evidence establishes, with the open questions visible, rather than a finished design proposal. Will agreed and the framing is now explicit in the document. The reasoning is that several pieces a full proposal needs are absent — the dissemination design is not selected, the admission parameters have no closed-form model, fees and incentives are untouched, and the persistence layer is deferred — and a reader is better served by an inventory of those than by prose that implies they are settled. The CIP's `Path to Active` section now carries that inventory in three groups: what blocks a buildable specification, what are deployment choices the analysis prices rather than makes, and what belongs to layers this proposal does not define.

**Where the model comparison stands.** Holding the design target fixed at a bad-graph probability of 10⁻⁴, M3 at (13, 7) and M4 at RF = 9 are the two candidates and M4 now leads six of seven measured axes: connections held (18 against 38 on average, 37 against 64 at the busiest node), churn tolerance (7.4 % against 2.2 % of the honest population offline), hops to the last subscriber, margin below the failure target, and — after Mauro's correction — the cost of an adaptive eclipse. M3 leads bandwidth by 22 %. The asymmetry that matters is one of consequence rather than count: at twenty-five topics the bandwidth difference is 2.1 against 2.4 Mbit/s, which binds nothing, while the connection difference is 950 against 400, which is file-descriptor territory. Will confirmed the comparison holds parameters fixed across models deliberately; changing a model's configuration to improve its showing would forfeit the like-for-like comparison the fixed target exists to enable.

**Small networks are the named gap.** Ezequiel reported that the hash-gating results put the safe zone at the gate leaving each node at least twice as many eligible peers as it must pick, and that this bounds the network size below which the gate stops contributing: around thirty-seven participants at the pick counts in use, the gate has no room to bucket and switches off, leaving the serving cap as the only defence against a flooder. Sybil resistance is structurally harder at that scale because an attacker's identity count is a larger fraction of a smaller population; raising the registration deposit is one lever, at a cost borne by honest participants. Aggregating several small topics into one to raise density was discussed as a scalability-against-security trade. Trusted bootstrap nodes and optional on-chain endpoints were raised as an alternative route to peer discovery at that scale. Nothing is settled, and the CIP now states that a topic of fifty participants is outside the analysis rather than a small instance of it.

**Adaptive eclipse cost, and what an adversary can be assumed to know.** Mauro presented the eclipse-cost property and corrected the figure the backlog had carried for M4, which was exactly twice the honest degree: a bidirectional link is one connection and one corruption removes it, so the cost is the degree itself. That moves M4 from the most eclipse-resistant model to second cheapest at the published operating points, though at the points this proposal actually recommends the ordering favours M4. Ezequiel questioned whether an attacker can realistically hold a view of the whole topology; the group's position is that the analysis is a worst case rather than a forecast, and useful on those terms.

**Instrument agreement, and the parameters behind the figures.** Will presented the cross-validation of the closed-form laws against the Rust simulations, which agree across the measured range within the Wilson intervals. Mauro asked how sensitive the comparison figures are to parameter choices; the answer is that the design target is what is held fixed, and the relative ordering of models under changes to the adversarial fraction and the safety target is a question the sweeps now partly answer. Adversarial-fraction sweeps to 0.40 have been run and the configuration constants behind each figure will be stated in the rationale rather than left implicit.

**Babel fees over PubSub: still open, now with a specific obstacle.** Polina described the on-chain side of Babel fees and the group tested whether PubSub could carry the off-chain intent traffic. Two obstacles surfaced. Registration is required to publish, which sits awkwardly with a use case whose point is that a user need not hold ADA; the suggested route is that wallet providers register and publish on their users' behalf. And the protocol offers no fairness property over who receives an intent first, which matters if solver economics depend on it — a free market of specialised solvers may not need fairness, or a broker may be the better shape. Mithril and the Decentralised Message Queue were noted as adjacent designs worth understanding before committing.

**Decisions.** *Aligned:* the CIP is positioned as a work-in-progress draft and technical report rather than a final specification; configuration parameters are held consistent across models so the comparison stays like-for-like. *Needs further discussion:* whether PubSub suits Babel fees, pending the registration, fairness and network-size questions above.

**Next.** Ezequiel: run the hash-gating and acceptance-cap experiments (E10 and E12) under M4, which the existing grids exclude; review the CIP pull requests. Will: state the draft technical-report framing in the document, add the configuration constants to the rationale, share the draft with stakeholders. Dana: evaluate the design trade-offs from a product standpoint with stake pool operators, and investigate the Decentralised Message Queue. Polina: evaluate PubSub against the Babel-fee requirements and gather Blink Labs' feedback on their implementation.

---

## 2026-08-06 — Brainstorm: five-model review with Spyros, hybrid node identity, accountability without provable silence

**Team.** Mauro Jaskelioff joins the project, picking up the analysis contributions as Denis moves most of his time to another workstream — Denis stays in the weekly sessions and remains available for review. Spyros Voulgaris joined this session to catch up on where the architecture landed.

**The five models, re-explained end to end.** For Mauro's and Spyros's benefit the group walked M1 through M5 and what separates them: which peers a node dials, which requests it accepts, and where it forwards — M1 downstream-only, M2 upstream peers, M3 extending M2 with dedicated links for a node's own publications, M4 bidirectional, M5 combining inbound and outbound with both relay and publishing links. Security level is held fixed across models so the comparison is purely one of efficiency and stability. Spyros and Denis converged on why M3 comes out ahead: it pairs the fast initial spread of a push mechanism with the reliability of a pull mechanism at the tail of dissemination. Its cost is structural complexity — the asymmetric use of link kinds is what makes it harder to reason about than the symmetric models.

**Connection flooding: the hash gate, restated.** Spyros asked what stops an adversary from burying a target node under connection requests. The answer is the hash-based gating mechanism (BASALT-inspired): which nodes may connect to which is strictly constrained per interval, and because every node is registered on-chain, a receiving node can verify that an incoming request is one it is actually obliged to consider. This is the protocol's answer to targeted isolation, not a general DoS defence.

**What the simulations do and do not yet assume.** Ezequiel was explicit that the current runs use an unbounded-resource model — honest nodes accept every incoming connection — and do not yet apply hash-gating or acceptance caps. Metrics in place: message volume, hop counts, and propagation waves. A *bad graph* remains one where dissemination fails to reach every honest node, and the modelled adversary is still the silent one. Monte Carlo simulations, the closed-form laws, and the prototype implementation all yield aligned results at this point. The next experiment batch introduces resource caps and hash-gating so resilience against a more strategic adversary can be measured rather than assumed.

**Node identity: vertical anchors plus horizontal deposits.** Jesus presented his proposal for [#103](https://github.com/input-output-hk/pubsub/issues/103), starting from the fact that no single universal trust anchor covers every role. A payment address is not a root of trust — cheap to generate, carrying no reputation beyond the ADA it holds. The sketch: every node generates a fresh key pair; where a role does have a known root of trust, that anchor is bound into the derivation of the node identifier, with valid signatures required under both the node key and the anchor key; where no anchor exists, the anchor terms are omitted. On topics whose roots of trust are well known — SPOs being the clear case — a valid anchor becomes mandatory for registration, and because it carries reputational weight it can buy a *lower* deposit. Topics without well-known anchors price the unknown participant with a higher deposit. The result combines vertical identity management where a hierarchy exists with horizontal, economically-priced identity where it does not. Grinding is out of scope for M3 through M5, which have no position-based topology for an adversary to grind toward.

**Deposit sizing needs the security budget first.** Denis noted the deposit figure cannot be settled ahead of an explicit security budget: the cost of an attack is derived from the adversarial fraction μ the chosen model tolerates, so the calibration runs from μ to the required number of identities to the price per identity, not the other way around.

**Slashing: the evidence problem is the blocker.** Jesus also sketched a slashing mechanism — downstream nodes return signed acknowledgements over the normalised message, aggregatable under BLS, which the upstream node accumulates and must present a random subset of at deregistration, facing partial slashing if it falls short of the expected count. Its appeal is that it needs no decentralised reputation infrastructure. Two problems were recorded with it: a downstream node can frame an honest upstream simply by withholding its acknowledgement, and the obvious mitigation of dropping non-acking peers may not compose with hash-gated link selection. The wider conclusion held across the discussion: when the threat is a node that stays silent, defining misbehaviour precisely is the hard part, and while local grading functions exist, extracting verifiable proof of malicious intent from them does not follow. The group's position for this phase is to establish the economic model and treat slashing as later research rather than block the specification on an unsolved proof-of-negative. That conclusion turns out to be a theorem rather than a gap in our reasoning: Denis subsequently shared *Accountable Liveness* (Lewis-Pye, Neu, Roughgarden and Zanolini, [eprint 2025/693](https://eprint.iacr.org/2025/693)), which separates **safety accountability** — faults evidenced by a message that was sent — from **liveness accountability**, faults consisting of the absence of messages, and proves the latter impossible without both a more-often-synchronous network and an honest majority among potential attesters. Our models assume neither: there is no timing model at all, and attribution here is local, where the honest-majority condition is precisely what fails in the tail the bad-graph analysis measures. The phase position is unchanged — establish the economic model, treat punishment as later research — but now rests on a citation rather than an open question. Follow-up analysis on [#103](https://github.com/input-output-hk/pubsub/issues/103).

**Decisions.** *Aligned:* security level held fixed across M1–M5 so the comparison isolates efficiency and stability; grinding out of scope for M3–M5; economic model prioritised over a slashing implementation. *Needs further discussion:* the hybrid identity approach — vertical trust anchors for verifiable roles, horizontal deposit-based registration otherwise, with the deposit differentiated by anchor presence.

**Next.** Jesus: refine the identity proposal. Denis: locate and share the paper on provable misbehaviour in distributed protocols — delivered, *Accountable Liveness* (above). Ezequiel and Will: run the model experiments with hash-based connection gating and resource caps enabled. Will and Ezequiel: visualisations comparing the performance metrics across all five models. Will: onboard Mauro.

---

## 2026-08-04 — Weekly session: framework goes multicore, every model strategy representable, delivery guarantees for the CIP

**The instrument got fast.** Ezequiel merged the optimisation and memory work ([PR #118](https://github.com/input-output-hk/pubsub/pull/118)), so experiments now run multicore and drop from several minutes to seconds. With that in place the implementation of all model strategies is complete: every model is representable, including the variants that add acceptance caps and the variants that apply hash gating, and their combinations. Integration into the experiments framework and the comparison runs against the closed formulas and the Monte Carlo simulations began the same week, with everything aligning so far; two long-tail comparisons at 30 000 rounds remained outstanding. One framing worth keeping: because the framework runs the same code as the prototype, agreement with the analytical models is not only a replication of results — it is statistical evidence that the implementation itself matches the formal models.

**Delivery guarantees: from good-graph probability to something a reader can use.** Will pushed on how to state reliability in the CIP, arguing that the fraction of bad graphs alone does not tell a reader what they want to know — what are the odds that *my* messages get delivered if I publish through a node picked at random. Denis clarified the relationship: the bad-graph probability is a deliberately conservative global measure, the union bound over the whole network, and it says only that *some* node's messages go undelivered in that epoch, not that yours do. The per-node probability of non-delivery is substantially lower, and stating it per node via a union bound is either conservative or uninformative depending on network size, which is why the property was defined globally in the first place. Ezequiel added the further reason a bad graph is not the same as broken delivery: coverage still holds for a given publisher unless that publisher happens to be a sink or is surrounded entirely by adversarial peers, and in practice a sender publishes through several peers rather than one. Reference figure on record: at Cardano epoch length the parameters put one bad graph in roughly forty years. The CIP will present both numbers — the conservative global bound and the per-node figure — rather than only the first.

**Peer sampling: where the abstraction still cheats, and what it costs.** Denis recalled the rule adopted when the standalone peer-sampling protocol was dropped in favour of the on-chain list: the code may back its sampling with the full registry, but it must only ever consume the partial-view API — sample a uniform peer — so the list can later be swapped for a real sampling protocol without changing the models. One place still breaks that rule, the perfect single-element bucketing in hash-gated selection, which reads the full registry. Concretely the list costs: a 20 000-node view occupies roughly 600 MB per node, about 30 KB per entry once the public key, topic interests, authorised publishers, and deposit are counted. High, but manageable for a prototype. Two exits exist when it matters — hold the registry in a database rather than memory, or bound the view to a small multiple of RF, which at around 60 entries lands in single-digit megabytes and restores the honest sampling interface at the same time. There is no view-size parameter today. For context on how far the instrument has already come, a single-worker experiment run took 30 GB a week earlier.

**Babel fees and intents over PubSub — explored, not adopted.** Dana asked whether a Babel-fee and intent-forwarding service could ride the same PubSub protocol, on the reasoning that the more live use cases a single messaging node serves, the easier the pitch to SPOs for hosting one. The objection is structural: a node that receives an intent it could profitably fulfil is economically motivated *not* to forward it, so the use case raises adversarial behaviour by construction. Ezequiel added that a fulfiller marketplace also wants a fairness property the collaborative relay strategies do not offer — nothing in the protocol governs who sees an intent first, so with a population of fulfillers whoever receives it simply fulfils it and forwards nothing, which points toward a broker or sequencer rather than a gossip overlay. Will noted a possible edge: an incentive scheme that pays into a shared pool, distributing to solvers that provably participated over time rather than rewarding whoever settles a given intent, would not carry the per-intent race that makes withholding rational. Recorded as an open exploration; Dana is looking at how other ecosystems handle intents and at the sequencer role, and will point the Babel-fee designer at the PubSub work.

**Specification-driven design, reaffirmed.** The group discussed development methodology and agreed the specification-driven approach is the one to carry into future project design: protocols that exist only as code are hard to reason about, hard to review, and hard to pick up later. Will noted the concrete benefit already visible here — a specified design lets someone joining the project late understand not just what was built but why past design decisions were made, which is precisely what the ADR and spec trail in this repository is for.

**CIP and housekeeping.** Dana's motivation draft is complete and lands in the repository shortly ([#105](https://github.com/input-output-hk/pubsub/issues/105)); the abstract merged ([PR #115](https://github.com/input-output-hk/pubsub/pull/115)). Will added the remaining CIP tickets in anticipation of the sections still to be written — dissemination protocol ([#131](https://github.com/input-output-hk/pubsub/issues/131)), network identity and address discovery ([#132](https://github.com/input-output-hk/pubsub/issues/132)), topic registration and publisher authorisation ([#133](https://github.com/input-output-hk/pubsub/issues/133)), and the rationale ([#134](https://github.com/input-output-hk/pubsub/issues/134)) — alongside the existing node registration and identity-cost ticket ([#103](https://github.com/input-output-hk/pubsub/issues/103)). The rationale section drew particular attention: it is the higher-level account of *why* the specification looks the way it does, part technical and part driven by what the ecosystem actually needs, which is where use-case input belongs. On the transparency side the epic closed: the repository is public, the website is up to date on its own subdomain, and visitors with a use case in mind can now open a GitHub discussion directly from the use-case section of the site.

**Scope note.** Address discovery is explicitly separated from network identity: identity is the current work, turning a registered key into a dialable address is its own section and its own open design question ([#132](https://github.com/input-output-hk/pubsub/issues/132)).

**Decisions.** *Aligned:* defer prototype memory optimisation and prioritise simulation execution speed; present per-node delivery guarantees alongside the conservative global bad-graph bound in the CIP; keep the weekly session as a standing cadence. *Open:* whether an intent or Babel-fee use case can be served by this protocol at all, and whether a pooled-reward incentive scheme removes the withholding motive.

**Next.** Ezequiel: finish the remaining comparison simulations and open the experiments pull request — landed as [PR #138](https://github.com/input-output-hk/pubsub/pull/138). Will: review it before Thursday's session, and outline the structure and required components of the CIP. Jesus: a complete draft identity proposal before going on leave. Dana: finalise the motivation section and commit it; look into the sequencer role for Babel fees and how other ecosystems facilitate intents.

---

## 2026-08-03 — Merge digest (async): 015/016/017 landed, experiments unblocked, repository public

Catch-up entry covering the merge activity of 23 July – 3 August (no weekly session held), compiled from the merged PRs.

**015 publisher links merged.** [PR #77](https://github.com/input-output-hk/pubsub/pull/77) landed the publisher-link connection model: M3 and M5 as per-node configurations of one node, plus a symmetric-edge configuration approximating M4 — standing publisher links carrying a node's own publications (sender-side exclusivity, kind-agnostic receive gate), constructed symmetric reciprocity via a dedicated handshake, and `forward-to-all` fan-out as the single M3→M5 switch. Absent the new flags the node remains the unchanged M2 baseline. Design rationale in ADR 0032 and ADR 0034; the exact M4 (uniform exactly-RF selection) was recorded as the agreed follow-up.

**016 deterministic experiments framework merged.** [PR #102](https://github.com/input-output-hk/pubsub/pull/102) shipped the feature-gated `experiments` module and binary: populations of the crate's real pure core under a round-based wavefront scheduler, seeded honest churn and silent Level-1 adversaries, graph analytics (SCC/condensation goodness, coverage with miss-cause decomposition, a per-run accounting identity), Wilson-interval statistics, and parallel sweeps with a byte-reproducible three-artifact output contract — any run replays from its manifest seed alone. Shipped configurations include the reference operating point (N = 20 000, μ = 0.2, RF = 24) and the bulk-regime validation point; `docs/experiments-program.md` was rewritten as the program of record.

**Instrument performance: the 30 GB wall removed.** Ezequiel's [PR #118](https://github.com/input-output-hk/pubsub/pull/118) (ADR 0038) moved per-topic candidate views behind a shared, self-excluding read seam: one N = 20 000 run now peaks at ~0.6 GB (was ~30 GB), unlocking multicore sweeps. The operating-point sweep drops from ~15–20 min (memory-forced single worker) to ~25 s, and the bulk regime from ~30–40 min to ~6 min. Both changes were proven result-neutral by byte-diff against the recorded M2 baselines.

**017 unified selection plane merged.** [PR #119](https://github.com/input-output-hk/pubsub/pull/119) — the substantial follow-up to PR #77 — collapsed four dial-side strategies and four acceptance baselines into one selection implementation per seam over two fed knobs: bucket count (hash-gate width, no longer locally derived) and pick count (exactly-min uniform picks, the formal selection family promoted from the experiments framework into the node). The **exact formal M4** is now node-expressible with fleet evidence, upgrading the earlier "approximation" label; all twenty model recipes (M1–M5 × gated/capped variants) are single-command knob configurations, misconfigurations fail at startup, and experiment blockers E7, E10 and E12 now read ready in the experiments program. Behaviour-neutrality of the collapse was gated on byte-identical baseline sweeps. ADRs 0039 and 0040.

**Repository public; website and housekeeping.** With the repository now public, the website's "open-sourcing soon" badges were removed ([PR #120](https://github.com/input-output-hk/pubsub/pull/120), closing [#121](https://github.com/input-output-hk/pubsub/issues/121) under the transparency epic [#93](https://github.com/input-output-hk/pubsub/issues/93)), third-party paper PDFs were replaced with links to their canonical sources ([PR #117](https://github.com/input-output-hk/pubsub/pull/117)), and the Pages deploy workflow moved to the Node 24 action majors ([PR #122](https://github.com/input-output-hk/pubsub/pull/122)).

**Next.** Ezequiel: connect the final parts to the experiments framework (small work contrasted with the 017 refactor) and run all experiments — the node strategies no longer block any model. Will: the 10–23 July biweekly report, drafted alongside this entry.

---

## 2026-07-21 — Weekly session: simulation framework validated for M2–M5, robustness via condensation sets, publisher/relay split proposal

**Simulation framework validated across M2–M5.** Ezequiel reported the experimentation framework now handles Monte Carlo evaluations for models M2 through M5, and results align with the prior analytical findings — the cross-validation loop is working as designed. Potential memory optimisations were identified along the way (peer cloning, message caching) but judged non-urgent; the group agreed to prioritise completing all experiment simulations before touching engineering optimisations, keeping the framework stable through the data-collection phase. Experiments are being rerun at 150 rounds, with the five identified experiments targeted within days and raw data files committed once sizes are verified.

**Reference parameters and the M3-vs-M5 puzzle.** Denis pinned the working simulation parameters: network size 20,000, adversarial fraction 20%, target degree 24, with the goal of keeping the probability of a bad graph at or below 10⁻⁴. Discussion continues on why M3 comes out unexpectedly efficient compared to M5 — still unresolved and worth watching as experiment data lands.

**Robustness: measuring the collapse, not just the threshold.** Beyond the operating point, the group wants to know how fast security degrades once the adversarial fraction μ exceeds the optimised level. A graph counts as *collapsed* at a 0.5 probability of a bad graph, and the derivative of the analytical model gives the degradation rate. Importantly, model failures manifest mostly as isolated nodes rather than large-scale disconnections, so the framework measures strongly connected components and *condensation sets* to quantify partial collapse. Condensation-set-size data will be generated and visualised for the CIP.

**Engineering bounds: message complexity and bandwidth.** Denis raised the constraint side: the models must stay within reasonable message-complexity and bandwidth budgets, and the current prototype lacks per-link bandwidth metrics, so per-node traffic measurements are needed. Will is digging up previous topology simulation data on bandwidth properties per link; Ezequiel is evaluating message complexity against engineering bounds; Denis adds a per-model throughput paragraph to the node-degrees property file. On capacity modelling, the group debated message processing time and cross-continental latency; Will suggested large messages could be handled via sharding or parallel processing rather than blocking entire links.

**Proposal: separate publisher and relay nodes.** Ezequiel proposed splitting designated *publisher* nodes from *relay* nodes: if only publishers use specific links, the system could retain M3-level security while relaxing requirements on relay-only nodes. Will and Denis saw this as a promising direction that could integrate naturally with an incentive layer — e.g. collateral posted by publishers.

**Gossip-style hash exchanges deferred.** I-Have/I-Want hash exchanges (à la GossipSub) could cut bandwidth by fetching messages on demand rather than pushing them blindly, but Will flagged a potential new attack surface — nodes advertising messages they do not actually hold. Hash-based message fetching and similar advanced throughput optimisations are deferred to a future development phase.

**CIP writing underway.** New GitHub tickets distribute the CIP sections (motivation, specification) and the identity-cost design question, with the aim of having sections drafted, reviewed, and finalised by mid-August.

**Decisions.** *Aligned:* complete all experiment simulations before implementing memory optimisations; generate and visualise graph-collapse data via condensation-set sizes for the CIP; defer advanced throughput optimisations (hash-based message fetching) to a future phase. *Open:* why M3 outperforms M5 in efficiency; whether the publisher/relay node split holds up under analysis, and how it couples to incentives/collateral.

**Next.** Group: run the five identified experiments within two days; rerun at 150 rounds; merge the open PRs — Ezequiel publishes the pending review and coordinates with Will on finalising the M4 implementation merge. Will: share previous topology simulation data on per-link bandwidth. Denis: per-model throughput paragraphs in the node-degrees property file. Near-term focus: landing the open PRs.

---

## 2026-07-15 — Weekly session + working sessions: M3 as base model, node view split into relay/publishing sets, CIP downgraded to optional

**M3 confirmed as the implementation base.** After evaluating the five network models, the group settled on M3 as the best balance of security and efficiency and prioritised its implementation as the base for M4 and M5. Peer terminology was sharpened along the way: *seed peers* exist for publishing only, with relaying handled separately — per Denis's Model 3 documentation. Model-specific strategy requirements were pinned down: M3 needs seed-specific publishing, while M5's fan-out is role-agnostic, publishing to all available downstreams regardless of whether they are publishers or relays. Hash-gating of connection requests remains a requirement for both M3 and M5, even though earlier simulation analyses may not have included it. One open question went to Denis — whether M5's symmetric K-degree configuration is a deliberate design requirement — alongside the correct RF/S parameter combination for M3 optimisation.

**M5 symmetry resolved: not symmetric.** Denis confirmed k_in and k_out are two distinct variables. For the M5 solution at N = 20k, μ = 0.2, Pr[bad] ≤ 10⁻⁴, the sweet spot is k_in = 9, k_out = 8 (see `m5/properties/full_coverage.md`). Context on model choice: M5 resembles what some IO protocols already use, but M3 delivers ~30% better traffic efficiency — M3 initially builds a "bad graph" and then repairs exactly the broken aspect with its s seeds, whereas M5's more conservative construction pays for it in inefficiency. Denis flagged these claims as worth cross-validating experimentally.

**Node view split: explicit relay and publishing sets.** The node view moves from a unified peer list to distinct *relaying* and *publishing* downstream sets. Will initially proposed an abstract link type to standardise link definitions and roles; Ezequiel pushed back on the structural overhead, and the group settled on the simpler design — explicit `relay_downstreams` and `publish_downstreams` fields — with connection strategies renamed to *relay selection* and *publisher selection*. Implementation proceeds with minimal disruption to existing tests: Ezequiel continues on the current framework code, adjusting after Will's node-view PR merges. The framework gains a trait exposing both relay and publishing downstreams so graph construction and metrics (e.g. strongly connected components) work consistently across models.

**Simulation framework and Byzantine modelling.** The experimentation framework runs multiple independent, parallelised Monte Carlo iterations: generate topologies from the connection strategies, remove Byzantine nodes and their connections, and check whether the remaining honest graph maintains strong connectivity — a computationally cheap linear algorithm. The group concluded Byzantine nodes need no complex dissemination strategies of their own; modelling them as silent participants and evaluating the surviving honest graph suffices to confirm secure topologies. For reproducibility, weekly experiment runs are pinned to GitHub tags rather than new CLI flags or manifest files.

**Scope: empirical data over a full CIP.** With the phase running to mid/end of August and roughly four to five weeks of runway, delivering a complete Cardano Improvement Proposal was judged too ambitious and downgraded to an optional goal. Priority is collecting empirical experiment data from the M3–M5 models to ground design proposals in evidence rather than intuition. Within the resource-constraint work, the deposit and attack-budget analysis is mandatory; synthetic profiling stays optional. Establishing the cost of node identity (deposits against Sybil attacks) remains the critical unsolved design question — Jesus is analysing identity-pricing strategies across the models.

**Implementation lesson noted.** Ezequiel flagged that the prototype would have been more testable had suppressed/dropped messages been returned as explicit effects rather than only logged. Current code stays as is; recorded as an improvement for future iterations.

**Decisions.** *Aligned:* node view categorises peers into distinct publishing and relaying sets (explicit fields, no abstract link type); M3 implementation prioritised as the base for M4/M5; M5 fan-out confirmed role-agnostic; framework trait exposes relay and publishing downstreams for graph construction and metrics; experiment reproducibility via weekly GitHub tags; full CIP optional, empirical data mandatory; deposit/attack-budget analysis mandatory, synthetic profiling optional. *Resolved in follow-up:* M5 is not symmetric — k_in and k_out are independent (k_in = 9, k_out = 8 at N = 20k, μ = 0.2). *Open:* RF/S parameter combination for M3 optimisation; node-identity cost mechanism; experimental cross-validation of the ~30% M3-over-M5 traffic advantage.

**Next.** Will: node-view refactor PR (relay/publishing fields, strategy renames), midyear-report slides, polish the phase parent issue [#46](https://github.com/input-output-hk/pubsub/issues/46). Ezequiel: metrics trait in the experiment framework, RF/S clarification with Denis, GitHub issues and test-document updates for the five models. Group: finalise the M-model implementations and experimentation framework so performance testing can start next week; check in Friday.

---

## 2026-07-09 — Brainstorm: experimental metrics for M2/M3, state-transition framework, three-question protocol decomposition

**Metrics prioritise total propagation over Request Factor.** The group settled the metric set for evaluating experiments: good-graph probability, efficiency (latency / hop counts), and bandwidth (total messages). Denis argued that measurements should target total expected quantities — total messages forwarded across the network — rather than the local Request Factor, which can mislead on true efficiency. Ezequiel will run the M2 measurements as a baseline against M3 (M3 is expected to carry meaningfully higher propagation efficiency once optimised), with a comparison table cross-validating both models against the defined parameters. Concrete tracking targets: maximum hop counts, total forwards, and rejected forwards.

**Golden nodes excluded from the analytical model.** Ezequiel raised that including golden nodes may hide partition problems and reduce the relevance of syncs, since they contribute directly to dissemination. Denis's position — adopted — is to leave golden nodes out of the initial analytical model: their contribution dilutes as the network grows (they lower the Request Factor by only one), so they are not critical for long-term scalability accuracy.

**State-transition experiment framework.** Ezequiel reported progress on a pure state-transition framework with no external async dependency (no Tokio), giving full control over event ordering, message-delivery history, and data export — enough to analyse *why* a delivery succeeds or fails, not just whether it did. The framework's event-based interface lets new strategies be configured in parallel without conflict, and because strategies are encapsulated closures, switching between them (M2 vs M3, directed vs undirected) or adding new ones needs minimal code. Extensive analysis is supported by running simulations repeatedly and storing raw outcomes to capture the distribution.

**Publishing vs relaying links.** The team sharpened link terminology, distinguishing **publishing links** from **relaying (downstream) links** to avoid implementation ambiguity. In M3, S-links (seed links) exist purely to initiate dissemination when a node has no upstream connection — avoiding an artificial bump to the Request Factor — and are established hash-based, like upstream links; they function as publish connections. Will and Denis noted M2 bidirectional and M2 undirected are effectively the same. Publishing and fan-out may still warrant distinct strategies since their behaviour differs, managed within the existing framework; Will flagged code-duplication risk and will propose an abstraction for bidirectional connections.

**Three-question protocol decomposition.** To keep the framework model-agnostic, Ezequiel proposed expressing any protocol by answering three per-topic questions: to whom connection requests are sent, from whom connection requests are accepted, and to whom data is forwarded. Current dissemination and publishing events are already distinguished by origin, so the existing framework is sufficient. Decision-making stays tied to specific strategies and operates per topic — merging connections across shared topics would deviate too far from current progress. Denis, working with Sandro, is building formulas for three properties to compare protocol models, with the aim of a generic system: a superior model can be dropped in as a strategy and cross-validated against prior findings.

**Churn vs adversarial, epoch transitions, retries.** Disgraceful (abrupt) node leaves are classified as adversarial; joins and graceful departures are treated as honest. Distinguishing honest churn from Byzantine behaviour matters for methodological accuracy, though folding churn into the adversarial budget simplifies analysis. For epoch transitions, two topologies may coexist temporarily so handovers are seamless before disconnecting the old one — this preserves graph safety at the cost of temporary message overhead. On rejections, the group ruled out recursive retries (an attack vector if nodes ignore bucketing rules): a rejected node accepts the result rather than reconnecting indefinitely, assuming a sufficiently low adversarial fraction (mu).

**Decisions.** *Aligned:* parameter sweeping (network size, topic counts) to find safe/unsafe operating boundaries rather than fixed conservative parameters; dynamic periodic re-connection and retry logic deferred to keep the initial framework simple; disgraceful leaves counted as adversarial, joins/graceful departures as honest; experiment metric set fixed as good-graph probability, efficiency (latency/hops), and bandwidth (messages); protocol modelling standardised on the three per-topic operations (request targets, acceptance, forwarding). *Open:* the bidirectional-connection abstraction (Will to propose); whether publishing and fan-out ultimately need separate strategies.

**Next.** Ezequiel: measure M2 as the M3 baseline; finalise the simulation harness (data collection + CSV export) by next week; add hop-count and coverage tracking; update the experiments document with the refined M3 model (without referencing bidirectional connections). Group: build the M2-vs-M3 comparison table and cross-validate against the defined parameters. Denis: compile and send the metrics list for the experiments documentation. Will: propose a strategy for abstracting bidirectional connections to avoid code duplication, and share it.

---

## 2026-07-07 — Weekly session: Raven/PubSub alignment, SecureCyclon formally retired, M3 adopted, resource analysis over attack budgets

**Blink Labs joins the table — Raven and Adder.** Chris Gianelloni (Blink Labs) attended to explore synergies with their secure messenger **Raven**, built on the Cardano node using DMQ and SIP 137 with the Signal protocol for messaging. Raven's current design is SPO-driven over a trusted topology; Chris expressed willingness to adapt it toward the discoverable, topic-based architecture PubSub is building, explicitly to avoid duplicated effort or competing protocols. Blink Labs also brings **Adder**, a lightweight Go-based ETL/notification pipeline (chain-sync or mempool input; Discord, Telegram, webhook outputs) that doubles as an embeddable library for wallet in-app notifications. Chris joins the weekly meetings going forward rather than running a separate track.

**SecureCyclon formally retired.** The architecture shift away from SecureCyclon — long signalled in these pages — is now the settled position presented externally: the three-layer gossip stack is abandoned in favour of the simpler on-chain anchored approach, with no pure peer-sampling protocol at this stage. On-chain registries for topics and nodes replace the sampling layer; each node samples locally from the registries to establish connections, with ongoing experiments determining peer-selection and topology-building strategies.

**Collaboration anchor: SPO notifications via Adder.** Initial Raven/PubSub collaboration centres on the SPO notification use case — adding a PubSub input to the Adder pipeline so notifications reach SPOs and end-users through their preferred channels. This slots into the four standing use cases Dana restated: emergency SPO notifications during consensus issues, SPO-to-delegator notifications, dApp-to-user messaging, and governance communication for dReps and delegators.

**M3 model adopted.** Denis presented the **M3 model**, which removes golden nodes as initial seeds and instead uses hashes to control fan-out and seeding, targeting global graph properties for reliability. Initial results: for a 20,000-node network the probability of forming a "good graph" is high, with bad graph formations expected to be rare. M3 is now the primary framework for network analysis, to be validated against experimental data, with a full-coverage M3-vs-M2 comparison (including parameter analysis) as follow-up.

**Byzantine resistance scoped honestly.** The current model tolerates a specific adversarial fraction (~20%) but does not cover resource exhaustion or DoS. The agreed posture for this iteration: provide empirical evidence for the protocol's security bounds and identify the gaps — incentives, fees, and slashing are future research, not current deliverables. The next 1.5 months (phase two closes mid-August) deliver experimental and analytical results, not a full formal Byzantine-resistant solution.

**Resource analysis and capacity caps.** Denis proposed folding resource analysis into M3 against Sybil attacks — resources consumed per node per unit time, message cost, and total network budget to bound attacks. Ezequiel identified the M3 worst case as an attacker saturating capacities while withholding messages; the agreed mitigation is bounding the connections a node accepts for S (requests) and RF (downstream requests). Parameter exploration showed increasing S and RF forces an attacker to acquire more identities, lowering the effective attack budget — though S saturates past a point. Denis cautioned against curve-based approximations outside their operating modes; simulations or gradient-based searches are the candidates for parameter optimisation. Napkin figure: a $100,000 attack budget under M3 with RF 10 and S 3 implies a registration cost of roughly $25. If required registration costs prove prohibitive, GossipSub-style scoring functions (tolerating a higher adversarial fraction) are the fallback direction.

**Registration deposits — economics sketched.** A node-registration deposit is the straightforward participation cost. Debated: fixed absolute value vs a function of per-topic network size, with Will suggesting deposits should *decrease* as the network grows — expensive to join while small and vulnerable, cheaper at maturity. Ezequiel flagged the de-register/re-register arbitrage risk as the network matures. The group deliberately declined to fix a dollar-based attack budget now: the analytical focus is the tolerable adversarial fraction (K) and the cost of identity, with resource implications derived from there.

**Prototype and experiments.** Ezequiel walked through the experimental framework (peer connection, acceptance logic, fan-out strategies) aimed at measuring resilience against eclipsing and message withholding and determining the parameters for full network coverage. Next: sweep N from 10 up to 1,000 to surface bottlenecks before the first real experiment run.

**Decisions.** *Aligned:* SecureCyclon abandoned in favour of the on-chain anchored approach; Raven/PubSub collaboration focused on the SPO notification use case via a PubSub input to Adder; iteration scope is experimental and analytical results, not a formal Byzantine-resistant solution; analyse tolerable adversarial fraction and its resource implications before fixing any dollar-based attack budget; M3 as the primary analysis framework, validated by experiment. *Open:* deposit shape (fixed vs network-size-relative, and the re-registration arbitrage); parameter-optimisation methodology (simulation vs gradient search vs curve fitting); defence escalation if registration costs prove prohibitive (GossipSub-style scoring).

**Next.** Group: merge the open connection/dissemination-strategy PR by end of week; define individual focus areas and research goals for the next 1.5 months; move roadmap planning onto GitHub tickets. Chris: add the PubSub input to Adder. Will + Ezequiel: deepen the experimentation-framework design and implementation, coordinate the pending PR merge, and check for remaining bottlenecks before the first experiment. Denis: prototype demonstrating sync occurrence linked to seed-introduction strategy, and the full M3-vs-M2 coverage comparison with parameter analysis. Will: share roadmap materials for the upcoming phase, and cross-validate the model comparison against experiment results. Ezequiel: extend the meeting duration going forward.

---

## 2026-07-02 — Brainstorm: baseline strategies first, analytical/simulation cross-validation workflow

**Baseline before complexity.** The session focused on simulation methodology for the phase-2 experiment set. Ezequiel has compared the Blink Labs Raven repository against the Orus network material and steered the discussion toward the experimental set and future metrics. Will flagged that the strategy space is not one-dimensional — it is three interconnected buckets (connection to upstream nodes, acceptance of requests, dissemination/fan-out policy), so the combinatorial solution space needs deliberate management. The group's answer: model the simplest foundational strategy first — connect to a fixed number of peers, accept all requests, forward to all connections — to establish a verification baseline before layering in dynamic metrics or adversarial mitigations.

**Analytical/simulation cross-validation workflow.** Experiments will be analysed systematically, one by one, prioritising those with closed-form solutions or approximations so analytical models and independent simulations can cross-validate each other. Denis stressed a skeptical review posture: compare simulation results against the implementation to surface discrepancies. A specific risk was identified in using Claude for both the implementation and the simulation — correlated error patterns could leave inaccuracies undetected in both — which the independent analytical derivations are there to catch. Current analytical work targets end-state topologies; Ezequiel noted the simulation framework could later track dynamic behaviour (message request counts, node resource consumption) that analytical models cannot easily capture.

**Hash-gated connection requests in scope.** Rather than accepting all incoming requests blindly, nodes will use deterministic hash-based verification — verifiably selecting upstream peers from local IDs and round numbers — to block the trivial adversarial attack of exhausting a node's degree slots by spamming connection requests. Bucketing does not eliminate adversarial impact but raises the resource cost of interfering with the network. Protocol event semantics were also sharpened: epoch transitions imply a complete network reshuffle, while heartbeat-driven retries handle smaller connection issues.

**Idealisation acknowledged, deferred deliberately.** Denis and Will acknowledged the analytical models assume an idealised, converged state and ignore operational constraints such as handshake durations and node churn. Those real-world constraints are deferred to a later phase, after the foundational baseline and cross-validation loop are established.

**Golden nodes deprioritised.** Will proposed postponing golden-node strategies — implementation is at least two weeks out — in favour of strategies that yield early results. Denis noted that including golden-node logic in the simulation would be simple and reduce duplication, but the group agreed to test the non-golden-node case first to keep progress moving.

**Tooling: concise prompting.** To counter Claude's verbosity in formulas, tables, and simulation output, the team will adopt lean, task-based prompting (Will pointed to the "caveman mode" skill on GitHub) — the goal being precise, machine-like output that is easy to review. Extreme conciseness in the analytical documentation is an explicit requirement.

**Roadmap and chain dependency.** Chris from Blink may attend the next weekly meeting; the team should prepare GitHub issues and roadmap planning in advance. Will raised the robustness question of a chain-halting bug on Cardano if the architecture stays fully dependent on chain state; Ezequiel confirmed registry and latency issues are already identified, with the current focus remaining on the prototype phase.

**Decisions.** *Aligned:* systematic one-by-one experiment analysis, closed-form/approximable cases first, cross-validated against independent simulations; hash-based verification of connection requests instead of blind acceptance; foundational strategies and experiment analysis prioritised, real-world constraints (e.g. handshake timings) deferred; sequential workflow of analytical sketch → simulation per strategy, with concise, readable documentation; golden-node strategies deprioritised in favour of early results.

**Next.** Group: define experiments individually with metrics and analytical approximations per strategy; run independent simulations and cross-check against the analytical formulas; review GitHub issues and prepare roadmap planning for the next meeting. Ezequiel: document the discussed strategies and plans and share via the GitHub repository; provide detailed experiment tables with metrics and expected behaviour. Denis: sketch concise analytical approximations for each identified item (every parameter explained) and run the corresponding simulations with lean output.

---

## 2026-06-30 — Weekly session: phase 1 → phase 2, deterministic simulation framework, delivery-completeness metric

**Phase 1 → phase 2.** Phase 1 wraps up this week; everyone closes their open tickets and drafts phase-2 tickets/topics for next week's review. With the prototype's core build essentially done, phase 2 pivots from building to **experiments** — empirical trade-offs and adversarial cases — and a more end-to-end setup than today's in-memory prototype.

**Deterministic simulation framework.** Ezequiel's phase-2 design removes the async/Tokio non-determinism by driving communication and events directly against the node's **pure core** — calling `NodeState`/`apply`, feeding seeds, and dispatching the returned effects itself in place of the network. It covers the M2 golden-node model and a candidate experiment set — the pay-off of the pure-transition / side-effects-at-the-edges design: reproducible runs and clearer data.

**Delivery-completeness metric.** The phase-2 headline: does every subscriber to a topic receive all published messages given the formed topology (percentiles of any misses)? Runs will compare topologies with vs without **golden nodes** to test whether they improve delivery — which Denis noted also admits a precise analytical treatment alongside the simulations.

**Persistence out of scope.** Alerts/notifications need no long-term storage, so the design stays strictly in-memory; a short (~≤24 h) history a joining node can pull directly from peers is an easy in-memory add if a use case ever needs one — no storage server.

**Related-work leads.** Three protocols to revisit later: GossipSub (scoring function), Drum (application-level DoS resistance), and Murmur (layered; its dissemination layer mirrors ours but on an **undirected** rather than directed graph). Whether any become prototype strategies to trial is a separate call.

**Decisions** *(aligned)*: pure/deterministic simulation framework (no async/Tokio); retention servers excluded (in-memory only); delivery completeness (± golden nodes) as the primary phase-2 metric.

**Next.** All: draft phase-2 tickets for next week. Ezequiel: build the framework. Will + Denis: define the delivery-completeness measurement. Team: weigh the GossipSub/Drum/Murmur leads on Thursday.

---

## 2026-06-25 — Brainstorm: peer sampling dropped for on-chain list, experimentation framework prioritised

**Is a peer-sampling layer needed at all?** With the prototype now caught up to the architecture, Denis questioned whether a dedicated peer-sampling protocol is still warranted given the design already carries an on-chain registry of deposited participants. Ezequiel noted that several earlier choices — keeping the participant list on-chain among them — were made to unblock prototyping rather than as deliberate protocol design, so the assumption was worth re-testing now.

**Offline nodes as the deciding factor.** The pivot point is whether the protocol must actively accommodate offline nodes. If it does not, the design simplifies sharply; if it does, a sampling layer reappears. The group settled on treating offline behaviour as a *form of misbehaviour* with an established tolerance metric, rather than a first-class protocol feature — adversarial/offline nodes fold into the existing tolerance formulas. Sybil resistance continues to rest on deposits, which already imply a full on-chain node list.

**Rejection sampling over a sampling protocol.** Denis argued that adding a peer-sampling layer introduces a fresh attack surface and *reduces* overall security. Since the full list is available, a node that draws an offline peer simply re-samples — rejection sampling gives the needed online/offline handling without a new infrastructure layer. Online/offline status can be surfaced via IP-discovery and gossip heartbeats flagging peers each cycle. Scale focus reaffirmed at realistic sizes (below ~100k nodes), with the security argument expected to scale with network size. On-chain registry scalability was judged bounded by transaction-processing throughput rather than storage, with data-availability layers or indexers (e.g. DB Sync) serving peer-status queries.

**Dissemination-layer flooding surface.** A node can request to be the downstream of an unbounded number of participants. Mitigations discussed: sequence numbers on requests, bucketing via hashes or VRFs, and deposit tuning to bound the impact of any single identity.

**Misbehaviour and punishment.** Proving *non-delivery* of a message is hard; observable misbehaviour (e.g. exceeding agreed connection parameters) can be signed and proven, opening a path to slashing or reputation loss. The team favours quantifying measurable misbehaviour over building delivery proofs.

**Verdict — stop researching peer sampling.** The list-based approach is sufficient for current needs; the peer-sampling investigation is closed out of the immediate roadmap. Focus shifts to an experimentation framework — message-propagation paths, maximum hop counts, configurable scenarios, and performance metrics — and to representing **golden nodes** in the prototype via connection-acceptance and discovery policies. The next phase quantifies attack budgets (e.g. eclipse cost) against deposit values, capturing simulation data to average performance and surface adversaries.

**Decisions.** *Aligned:* exclude a dedicated peer-sampling protocol in favour of the on-chain list with rejection sampling for offline peers; prioritise building the experimentation framework and golden-node representation over further peer-sampling research. *Open:* the precise misbehaviour-proof / slashing surface, and the downstream-flooding mitigation (sequence numbers vs hash/VRF bucketing vs deposit tuning).

**Next.** Will and the team to define which experiments to explore with the prototype; Ezequiel to build the framework that describes, configures, and runs them, plus golden-node configuration, sharing alternative plans on Slack. Denis to finalise the gossip and broadcast documentation, removing peer sampling from the active roadmap, and to bring a fuller plan to next week's session.

---

## 2026-06-23 — Weekly session: SecureCyclon dynamics diverge from Cyclon, view-violation eclipsing

**SecureCyclon is not Cyclon-plus-defences.** Denis reported that the team's working assumption — that SecureCyclon behaves like Cyclon with added defence mechanisms — is wrong. SecureCyclon prevents certain descriptor reuse to block silent cloning, and those non-swappable descriptors change the protocol's internal dynamics. Treating it as standard Cyclon therefore yields unintended dynamics and missed vulnerabilities; prior Cyclon analysis does not carry over (issue [#43](https://github.com/input-output-hk/pubsub/issues/43)).

**View-violation eclipsing quantified.** While the descriptor rules mitigate simple silent attacks, *view-violation* attacks remain a significant risk. Concentration-based attacks let an adversary bias views or eclipse nodes: roughly **15% view bias with 5% of the network adversarial, rising to a full node eclipse at 20%**. The results were verified against a simulator built in coordination with Spyros (issue [#72](https://github.com/input-output-hk/pubsub/issues/72)). Denis proposed **commitments on the view** as a candidate defence to neutralise these attacks, to be evaluated for feasibility.

**Related-work survey.** Five protocols meet the inclusion criteria (Byzantine-resilient, broker-free). Fireflies was singled out for its clever node arrangement but criticised for linear scaling, which makes it impractical beyond hundreds of thousands of nodes (issue [#65](https://github.com/input-output-hk/pubsub/issues/65)). Target: a final report per protocol ahead of the 6 July phase boundary, with the analysis kept current in the repository.

**Light/edge clients stay out of the topology.** The current strategy relies on mobile apps and wallet providers acting as proxies for push notifications, rather than integrating light clients directly into the network topology.

**Sybil resistance via registration deposit.** The working plan is a registration cost framed as a deposit/collateral (not staking) to deter misbehaviour. Jesus noted that while a deposit is the universal approach, alternatives — registering keys, or reusing existing certificates for specific entities — may be more efficient per use case. All participants must register their keys regardless of which Sybil-resistance mechanism is chosen. Whether nodes deregister when going offline was left for the follow-up brainstorm, since it bears on whether pure sampling is needed.

**Incentives anchored on applications.** dApps and B2B/B2C use cases (e.g. embedded intents for mobile and web apps) are prioritised as the primary driver for network incentives, rather than relying on governance or emergency alerts alone, with application revenue potentially subsidising those public-good channels. Persistence/data-availability requirements should sit at the application layer rather than being baked into the core protocol, keeping the protocol focused on dissemination.

**Prototype and a practical sanity check.** Ezequiel reported ongoing work refactoring connection logic and encapsulating message handling, including abstracting the sampling module so the node queries a peer view rather than a raw registry list. Ezequiel pushed for a practical sanity check on the value of Byzantine resistance — using Cardano SPOs as an example, a simple "leave and rejoin on detecting an issue" response may beat over-engineering — and Denis noted the difficulty of quantifying that without a detection algorithm. This fed a broader concern that SecureCyclon's growing complexity may not be justified versus simpler protocols such as Basalt with a smaller attack surface.

**Decisions.** *Aligned:* edge/light clients excluded from the protocol topology (wallet providers and proxies instead); mandatory key registration for all participants, independent of the Sybil-resistance mechanism; dApps and B2B/B2C prioritised as the primary incentive driver; incorporate practical sanity checks into the protocol analysis to avoid unnecessary complexity. *Open:* feasibility and cost of view commitments as a mitigation; whether offline nodes deregister, and the resulting need for pure sampling.

**Next.** Team to update the protocol analysis in the repository and prepare a final report per protocol ahead of the 6 July phase boundary. Denis to evaluate the feasibility of view commitments. Offline-node / pure-sampling questions carried into the Thursday brainstorm.

---

## 2026-06-16 — Weekly session: node strategy modules, GossipSub findings

**Node strategy modules — three pluggable interfaces.** Ezequiel reported connection strategies are now encapsulated as objects on the event-driven node, allowing behaviour to be switched per configuration. The team defined three strategy interfaces: **peer-view** (selecting peers from a candidate set), **connection-acceptance** (deciding whether to accept inbound connection requests), and **dissemination / fan-out** (selecting peers to relay messages to). Default fan-out is a *flat* strategy — forward to all connected peers except the source — designed to be switchable. Encapsulation lets the team test different protocol configurations, including churn and adversarial scenarios, and run larger in-memory simulations (20–50 nodes). Current connection setup is static and event-driven; future work adds epochal/periodic re-establishment. Denis flagged the open question of how a node estimates network size to bound its connection count, with future statistical anomaly/bound detection as a possible optimisation.

**GossipSub literature findings.** Denis updated on the peer-to-peer pub-sub survey, with inclusion criteria settled: Byzantine resilience, non-broker-based design, and peer-reviewed or archival publication. GossipSub is the contemporary reference and gets the deep-dive; older 2000s protocols get a superficial pass to avoid missing related work. GossipSub claims four properties — liveness, fairness, monotonicity, misbehaviour detection — but formal-methods analysis shows liveness and misbehaviour detection are violated in certain configurations, specifically the one Ethereum uses. The critical weakness is the scoring function: it is **averaged over all topics rather than per-topic**, so a node can behave well across most topics while acting maliciously on one. DHT-based peer discovery lacks Byzantine resilience and is susceptible to eclipse attacks. Protocol Labs' adversarial model (every honest node outnumbered four-to-one) was judged less impressive as a worst case, since an adversary behaving honestly before switching is not captured — the good-then-malicious worst case lacks empirical study. Team conclusion: the scoring function is the critical element to evaluate; the discovery layer is secondary to current objectives.

**Decisions.** *Aligned:* three node strategy modules defined — peer-view, connection-acceptance, fan-out dissemination; flat dissemination (forward to all peers except source) as the default; literature inclusion criteria (Byzantine resilience, non-broker, peer-reviewed/archival); review depth — deep on GossipSub, superficial on older protocols. *Open:* how a node estimates network size to bound connection count.

**Next.** Group to perform a superficial review of the remaining pub-sub literature. Denis to draft a neutral summary of the GossipSub findings and recommendations.

---

## 2026-06-11 — Brainstorm: peer-sampling protocol evaluation, state-machine prototype, per-topic vs global sampling

**Peer-sampling literature update — Basalt vs Honeybee.** Denis gave an update on the peer-sampling service survey, comparing Basalt and Honeybee. Honeybee uses a random-walk strategy for uniformity and Byzantine resistance — efficient with respect to network size, but it demands significant up-front computation. Basalt's cost instead scales with the view-size requirement, which itself grows with the desired security level. Neither is a clear winner yet; the trade-off is computational overhead versus the strength of the security model.

**Evaluation methodology — categorise before committing.** Will and Denis agreed on a deep-dive rather than an arbitrary pick. Denis will sort the candidate protocols into families by strategy (random walk vs verifiable brokers) to produce a manageable shortlist, sync with Sandro to refine it, then present to the wider team to surface downstream effects on the current stack. Open question deliberately left open: whether a comparative trade-off table is enough to select a candidate, or whether formal modelling is required first. Claude floated as a tool to synthesise core structural concepts from the literature into that table.

**Evaluation criteria.** Denis outlined the primary criteria: maximum uniformity, acceptable resampling speed, and Sybil/Byzantine resistance. Critically, the protocol must be *analysable* — some protocols with highly structured adversarial models (e.g. SecureCyclon) are harder to validate than others like Basalt. Ezequiel stressed *elegance*: simpler, well-defined software lowers the chance of complex, undiscovered errors. GossipSub and Discv5 noted in passing, with the same filter — favour protocols with clear, analysable properties.

**Evaluate now to avoid architectural rework.** Will and Denis judged it critical to settle peer sampling now rather than defer to stages 3–5. Pushing it later risks engineering teams discovering, late, that the chosen protocol cannot meet production-ready Byzantine-resistance requirements — exactly the rework the team wants to avoid.

**Prototype refactored to a state machine.** Ezequiel reported the prototype is now a state machine driven by a single event queue processing state transitions. Will noted the node is now effectively read-only with respect to on-chain interactions, sourcing configuration from a static file that stands in for the registry. This sharpens testability: developers can feed specific event queues into the system and verify the derived state directly.

**Navigation layer — per-topic vs network-wide sampling.** Will and Ezequiel questioned whether to run peer sampling per-topic or network-wide. The existing navigation layer is vulnerable to adversarial attacks; a per-topic gossip/dissemination layer could offer better scaling and domain-specific optimisations. Denis and Ezequiel weighed forward-all-messages against topic-based propagation: topic-based systems risk encouraging selfish behaviour, where nodes selectively forward to save resources. Denis flagged the deeper concern — if the navigation layer imposes structured biases, the randomness peer sampling is supposed to provide may not be preserved.

**Incentives and deposits as an adversarial defence.** Will proposed deposit mechanisms as a defence against adversaries subscribing to all topics to gain influence or connect to peers for free. Jesus offered an alternative: separate network instances by expected load (e.g. grouping low-frequency topics together) to optimise resources, plus encryption for specific use cases. Ezequiel's conclusion: even if an adversary subscribes to every topic, the security analysis should focus on setting deposit and penalty levels high enough to make misbehaviour costly regardless of how many topics exist. Future extensions — rate limiting, aggregating topics into channels — deferred until the core structure is finalised.

**Decisions.** *Aligned:* run a comparative evaluation and categorisation of promising peer-sampling protocols (trade-off table) before selecting one for formal modelling and implementation; pause formal analysis of Basalt until the broader landscape study is complete, to avoid wasted effort. *Open:* per-topic vs global/uniform peer-sampling architecture — deferred pending further research into protocol capabilities; whether a trade-off table suffices or formal modelling is needed to choose.

**Next.** Denis to prepare an initial protocol categorisation and shortlist structure for the next meeting (Tuesday), syncing with Sandro beforehand; Will to reserve agenda time for it. Team to revisit navigation-layer extensions (rate limiting, channels) after the prototype core is finalised.

---

## 2026-06-04 — Brainstorm: key rotation, sequence integrity, SecureCyclon presentation framing

**SecureCyclon and key rotation.** Jesus walked through the SecureCyclon misbehaviour-eviction path: nodes are banned via cryptographic signatures, so a compromised key lets an attacker fabricate evidence of past misbehaviour. Mitigations discussed: key-evolving signature schemes or epoch-based key rotation. Jesus flagged that rotation costs more in space and signing time than vanilla ED25519, but the trade-off prevents an attacker from signing messages for past epochs.

**Publisher sequence numbers and message integrity.** Will raised an issue where a malicious publisher could skip sequence numbers (publishing message 5 after message 3) to manipulate history. Will and Jesus debated whether a hash chain proves a message was intentionally omitted rather than dropped by the network. Ezequiel observed that distinguishing the two cases is difficult without sacrificing liveness. Open shape.

**Threat model and forward security.** Group revisited what forward security buys. Jesus clarified: forward security protects against *past* compromises by rotating and removing old keys, but it does not prevent *future* compromises — that requires proactively secure digital-signature schemes where a cold key derives epoch-specific keys. Necessary if the design wants to prevent attackers from forging valid forks after a key compromise.

**Necessity of linear ordering.** Denis questioned whether linear message ordering is strictly required or whether wall-clock timestamps could substitute. Jesus and Ezequiel concluded the hash chain remains cheap and useful for detecting gaps in message sequences, even though it does not prevent every attack. The discussion noted a shift in the threat model: participants are moving toward verifying individual peer responses rather than relying solely on statistical network analysis.

**Standardising key derivation — deferred.** Jesus and Ezequiel agreed standardising key-derivation methods is not an immediate priority and does not block current development. Decision: defer until the gossiping protocols are more clearly defined, then pick the most constrained scheme that fits.

**Implementation update and signature malleability.** Ezequiel reported the implementation work to restructure messages with mock crypto is taking longer than expected. Brief touch on ED25519 signature malleability: Jesus confirmed the Cardano implementation uses canonical points to avoid it. Agreed to continue narrow technical discussion on GitHub.

**Monday SecureCyclon presentation.** Denis planning a presentation on SecureCyclon for next Monday. Suggested use cases to illustrate practical applications: notifications, security alerts, governance signals. Denis and Ezequiel agreed to frame the narrative around the project's evolution — identifying issues in the original architecture, applying fixes to the dissemination layer, analysing SecureCyclon's residual limitations and excluded attack classes. Ezequiel emphasised the methodology itself is the core value: pen-and-paper analysis, model checking, and simulations used to validate the protocol and its improved dissemination layer.


**Protocol analysis findings to highlight.** The discovery: the original protocol was not well-studied with respect to its composition. Through formal model checking, simulations, and manual analysis, the team identified critical issues and proposed adjusting edge priorities as a defence. These findings were derived systematically from the models rather than being random observations; the original paper's model lacked sufficient strictness. Denis and Ezequiel agreed to revise slides so the value comes across — the contribution went beyond identifying a broken protocol: they successfully formalised the lower layers and implemented fixes. Ezequiel stressed conveying the risk of rushing implementation without proper modelling.

**Decisions.** *Aligned:* defer standardising key derivation; continue ED25519-malleability discussion on GitHub; Monday presentation framed around methodology and evolution narrative. *Open:* whether to adopt key-evolving / forward-secure signatures, and how to prove intentional vs accidental sequence-number gaps.

**Next.** Denis to refine the SecureCyclon presentation slides — simplified problem statement, methodology-first framing — and share next day. Ezequiel available for feedback the following morning. Both to stay in sync on the final shape ahead of Monday.

---

## 2026-06-02 — PubSub working session: state-machine refactor, gap validation deferred

**Technical report over CIP for July.** Team prioritised a technical report rather than a formal CIP for the July deliverable — the CPS plus a secondary technical report demonstrate concrete research use cases and progress toward real-world Cardano applications. Will confirmed a report focused on ideas and solutions (not a formal CIP) is feasible by end of July, reusing the existing CPS structure and current prototype outcomes to validate the use cases.

**Timeline.** The workstream runs to September, with the team prioritising completion by end of August.

**Second technical report as a collective design doc.** Will proposed the second report serve as a shared document gathering designs and ideas across all topics — prototype details, node lifecycle flows, protocol extensions, and research on prototype developments. Group effort to compile.

**Node refactored to a state machine.** Decision adopted: refactor the node implementation into a state machine to create a clearer separation between the model and the state-transition function. Ezequiel's rationale — the split improves performance by allowing parallel processing while keeping a clean structure. Denis and Will agreed with the concept. The group discussed formally specifying expected behaviour with TLA+ or a similar modern language, but the focus is on *specification*, not formal proofs. Whether the system's maturity yet warrants formal verification (e.g. a TLA+ model) remains an open question — Denis and Ezequiel both flagged it needs further discussion.

**Message gap / ordering validation deferred.** Ezequiel raised that the architecture validates messages by signature but has no defined strategy for missing messages in a sequence or out-of-order arrival. The core difficulty: gaps may arise from network failures, and there is no mechanism to distinguish unintentional gaps from malicious behaviour. Will argued many of these concerns can be handled by submission validation — the node tracks the latest message per publisher per topic in a cache to check that an incoming message properly extends the existing hash chain. Ezequiel countered that this is insufficient: publishers can intentionally omit messages and recipients can't verify the validity of what's missing, which breaks catch-up mechanisms. Unlike a blockchain, the system doesn't consistently check all messages, so submission validation alone can't carry it. Will suggested exploring a ratchet-like data structure to cryptographically enforce sequence numbers and integrity. **Decision:** defer the gap/ordering logic from the current PR; revisit once the relevant flows are specified. Ezequiel is documenting the deferred items in the implementation notes.

**Peer sampling — rejection-sampling research strategy.** Denis recommended reviewing the Basalt protocol (2023) to determine suitability. Group adopted a rejection-sampling approach to the literature: study candidates to either accept one or reject it on constructive grounds, with the rejection then defining the requirements for future research. Denis and Sandro to study Basalt independently and share assessments next week.

**Prototype scope and simulation.** Will and Ezequiel scoped June around aligning protocols for integration. Denis: no specific experiments stand out yet, but analytical sanity checks and metrics like in-degree / out-degree distributions are valuable for verifying the implementation — and data should be separated by node maturity (new vs. long-established) to keep noise from skewing metrics.

**Identity and cryptography.** Jesus reported on identity work: time-dependent misbehaviour detection may need forward-secure signatures, falling back to standard ED25519 if forward security proves unfeasible. Proposal to derive keys for both the peer-sampling and publishing layers from HD wallets (ZIP 1852). Trust anchors — SPO cold keys or DRep certificates — would verify identities and mitigate Sybil attacks via deterministic node-descriptor derivation, letting participants check signatures against known roots of trust to confirm a node is an authorised SPO. For publishing, Will and Jesus explored extending wallet interfaces with a new "pop-up" key type derived through HD paths, so entities can sign messages directly from existing wallets for seamless delegator verification.

**Node lifecycle and on-chain contracts.** Will reviewed the lifecycle flows. Architecture relies on two on-chain contracts: a Topic Registry (authorised publisher keys) and a Subscription List (node registration). Setup: operators generate identity off-chain, fetch registered topics, register public keys and deposits on-chain. Startup: node reads the registered-peer list from chain, filters by topic, and connects via signed descriptors proving public-key ownership. Brief discussion of transaction-size constraints and future options (Merkle trees, minting policies) for managing topic subscriptions. Ezequiel questioned whether the subscription list could move off-chain to reduce on-chain complexity; the group also weighed the bootstrap-node dependency in the current topology.

**Next.** Denis and Sandro to study the Basalt paper independently and share assessments next week. The group to compile the second technical report. Will to re-evaluate the protocol structure and paper around the hash-chain / gap concerns and judge whether more formal modelling is needed. Ezequiel to submit the PR with the new structure and implementation. Will to notify the team about the Thursday meeting (note: availability limited to 30 minutes).

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

**Next.** Will to add Spyros (GitHub handle provided) as reviewer on the architecture PR. Spyros to review the technical report and PR offline, simulate the bias-view behaviour against the reported malicious-node percentages, and examine the subscription-list prototype. Spyros to share the older mathematical-analysis paper on the standard Cyclon protocol (Denis confirmed it has been located). The group to develop a testable formula sufficient to establish dissemination-layer security.

---

## 2026-05-26 — PubSub working session: node-flow spec, June scope, scaffolding

**Technical report circulated.** Will shared the draft technical report on Slack ahead of the meeting. It captures Denis's three-property analysis of Cyclon (two hold, the third is falsified) and the SecureCyclon defence inventory along with the attack vectors those defences fail to cover. Feedback requested on whether the report is complete or needs further development.

**Cyclon direction split — decision deferred.** Denis reported diverging views from co-authors: Jesus remains optimistic about salvaging the current protocol; Sandro is pushing for a literature sweep or a clean-sheet design. The patch-vs-rebuild call is deferred pending Will's Thursday consult with Spyros.

**Node-flow specification adopted as joint workstream.** Will identified five PubSub procedures worth formalising: joining, leaving, publishing, creating a topic, and changing topic subscriptions. He walked the joining flow — operator key-pair generation, on-chain registration check, bootstrap connect. Ezequiel flagged this overlaps the scaffolding effort already in progress; rather than parallel tracks, the two will co-specify these flows using the Spec Kit framework so research specs and implementation stay coherent.

**Scaffolding PR update.** Ezequiel's in-progress PR ships an in-memory network keyed off a hashmap, config-file reading, and basic message send/receive. Topic management, message structure, cryptography, and connection logic are the next iteration. Plan: merge the scaffolding PR, then publish a meta-spec describing the follow-on work so feature expansion has an explicit target.

**Product focus shifts to incentives.** Will argued the product team should pivot from feature checklists to fees and incentive design — give operators a concrete reason to run a node and a real cost for misbehaviour. Dana to prepare an incentive-model update for next week.

**IP discovery edge cases.** Will raised two open questions: how nodes should handle peers that are registered but offline, and the minimum connection count required to maintain delivery guarantees. Ezequiel's preferred handling: cycle through candidate peers and use a bidirectional handshake to qualify connection quality, rather than altering the core protocol structure. Will to write up the IP-discovery process for the technical report.

**Registration-list semantics.** Discussion on whether offline nodes should remain on the on-chain registration list. Denis: for the current prototype, fan-out is dynamic in network size and rejection sampling adequately handles the distribution of online nodes — no need to evict offline entries from the list at this stage.

**Publishers run a subscribed node.** Will's working position for the publishing spec: a publisher runs a node subscribed to the relevant topics so signing and routing inherit the dissemination layer's guarantees. Ezequiel agreed this is the logical shape for the dissemination layer. Will to draft the spec and circulate.

**June scope: prototype plus technical report, not a CIP.** Team agreed an end-of-June CIP is not achievable; the deliverable is a working prototype plus a technical report capturing the research findings. Ezequiel flagged the broader governance backdrop: of roughly 20–30B in distributed voting power, only ~4–5B is actively voting, and weight is currently a function of financial contribution rather than expertise.

**Decisions.** *Aligned:* node-flow specs to be co-developed via Spec Kit; June deliverable is prototype + technical report, not a full CIP. *Open:* whether to patch or abandon SecureCyclon, pending Spyros's input.

**Next.** Will + Ezequiel to define sequence diagrams for joining, leaving, and publishing, aligned with the scaffolding. Ezequiel to merge the scaffolding PR, publish the meta-spec, and implement topics, message structure, cryptography, and connection logic. Dana to research incentive models. Denis to analyse SecureCyclon mitigation strategies ahead of the Spyros session. Will to document IP discovery in the technical report and share project specs. Ezequiel to review existing project documentation and connect current coding work to the established requirements.

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
