# PubSub Logbook

Technical decisions and progress. Most recent first.

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

## 2026-06-23 — Weekly session: SecureCyclon dynamics diverge from Cyclon, view-violation eclipsing, dual-repo split

**SecureCyclon is not Cyclon-plus-defences.** Denis reported that the team's working assumption — that SecureCyclon behaves like Cyclon with added defence mechanisms — is wrong. SecureCyclon prevents certain descriptor reuse to block silent cloning, and those non-swappable descriptors change the protocol's internal dynamics. Treating it as standard Cyclon therefore yields unintended dynamics and missed vulnerabilities; prior Cyclon analysis does not carry over (issue [#43](https://github.com/input-output-hk/pubsub/issues/43)).

**View-violation eclipsing quantified.** While the descriptor rules mitigate simple silent attacks, *view-violation* attacks remain a significant risk. Concentration-based attacks let an adversary bias views or eclipse nodes: roughly **15% view bias with 5% of the network adversarial, rising to a full node eclipse at 20%**. The results were verified against a simulator built in coordination with Spyros (issue [#72](https://github.com/input-output-hk/pubsub/issues/72)). Denis proposed **commitments on the view** as a candidate defence to neutralise these attacks, to be evaluated for feasibility.

**Related-work survey.** Five protocols meet the inclusion criteria (Byzantine-resilient, broker-free). Fireflies was singled out for its clever node arrangement but criticised for linear scaling, which makes it impractical beyond hundreds of thousands of nodes (issue [#65](https://github.com/input-output-hk/pubsub/issues/65)). Target: a final report per protocol ahead of the 6 July phase boundary, with the analysis kept current in the repository.

**Light/edge clients stay out of the topology.** The current strategy relies on mobile apps and wallet providers acting as proxies for push notifications, rather than integrating light clients directly into the network topology.

**Sybil resistance via registration deposit.** The working plan is a registration cost framed as a deposit/collateral (not staking) to deter misbehaviour. Jesus noted that while a deposit is the universal approach, alternatives — registering keys, or reusing existing certificates for specific entities — may be more efficient per use case. All participants must register their keys regardless of which Sybil-resistance mechanism is chosen. Whether nodes deregister when going offline was left for the follow-up brainstorm, since it bears on whether pure sampling is needed.

**Dual-repository strategy.** To keep sensitive security analysis private while still publishing progress, the team adopted a dual-repo split: prototype code and documentation public; security analysis, issues, and pull requests private, with periodic sanitised snapshots to the public side.

**Incentives anchored on applications.** dApps and B2B/B2C use cases (e.g. embedded intents for mobile and web apps) are prioritised as the primary driver for network incentives, rather than relying on governance or emergency alerts alone, with application revenue potentially subsidising those public-good channels. Persistence/data-availability requirements should sit at the application layer rather than being baked into the core protocol, keeping the protocol focused on dissemination.

**Prototype and a practical sanity check.** Ezequiel reported ongoing work refactoring connection logic and encapsulating message handling, including abstracting the sampling module so the node queries a peer view rather than a raw registry list. Ezequiel pushed for a practical sanity check on the value of Byzantine resistance — using Cardano SPOs as an example, a simple "leave and rejoin on detecting an issue" response may beat over-engineering — and Denis noted the difficulty of quantifying that without a detection algorithm. This fed a broader concern that SecureCyclon's growing complexity may not be justified versus simpler protocols such as Basalt with a smaller attack surface.

**Decisions.** *Aligned:* edge/light clients excluded from the protocol topology (wallet providers and proxies instead); mandatory key registration for all participants, independent of the Sybil-resistance mechanism; dual-repository strategy (public prototype + docs, private analysis/issues/PRs); dApps and B2B/B2C prioritised as the primary incentive driver; incorporate practical sanity checks into the protocol analysis to avoid unnecessary complexity. *Open:* feasibility and cost of view commitments as a mitigation; whether offline nodes deregister, and the resulting need for pure sampling.

**Next.** Team to update the protocol analysis in the repository and prepare a final report per protocol ahead of the 6 July phase boundary. Denis to evaluate the feasibility of view commitments. Offline-node / pure-sampling questions carried into the Thursday brainstorm.

---

## 2026-06-16 — Weekly session: monetisation pivot, node strategy modules, GossipSub findings, CSM framing

**Monetisation pivot — from public goods toward revenue.** Dana reported on the incentive-model investigation, shifting focus from a public-goods framing (SPOs, DReps, ecosystem entities) toward identifying customers and revenue-generating use cases, as requested by management. Push Protocol served as the primary case study: a decentralised push-notification system on Ethereum whose low 50-token channel-creation fee is a Sybil-resistance measure rather than a revenue lever, since expanded to multi-chain with fees routed to stakers — suggesting the original Ethereum revenue model proved insufficient. Dana proposed monetising around trading influencers (à la Telegram/Discord), integrated into wallets such as Lace to enable direct trading and transaction fees. Will pushed for a use case requiring reliable, trustless information distribution, not necessarily blockchain-bound. Group leaned toward building pub-sub messaging directly into wallets — not Cardano-restricted, allowing fee collection across cryptocurrencies.

**Node strategy modules — three pluggable interfaces.** Ezequiel reported connection strategies are now encapsulated as objects on the event-driven node, allowing behaviour to be switched per configuration. The team defined three strategy interfaces: **peer-view** (selecting peers from a candidate set), **connection-acceptance** (deciding whether to accept inbound connection requests), and **dissemination / fan-out** (selecting peers to relay messages to). Default fan-out is a *flat* strategy — forward to all connected peers except the source — designed to be switchable. Encapsulation lets the team test different protocol configurations, including churn and adversarial scenarios, and run larger in-memory simulations (20–50 nodes). Current connection setup is static and event-driven; future work adds epochal/periodic re-establishment. Denis flagged the open question of how a node estimates network size to bound its connection count, with future statistical anomaly/bound detection as a possible optimisation.

**GossipSub literature findings.** Denis updated on the peer-to-peer pub-sub survey, with inclusion criteria settled: Byzantine resilience, non-broker-based design, and peer-reviewed or archival publication. GossipSub is the contemporary reference and gets the deep-dive; older 2000s protocols get a superficial pass to avoid missing related work. GossipSub claims four properties — liveness, fairness, monotonicity, misbehaviour detection — but formal-methods analysis shows liveness and misbehaviour detection are violated in certain configurations, specifically the one Ethereum uses. The critical weakness is the scoring function: it is **averaged over all topics rather than per-topic**, so a node can behave well across most topics while acting maliciously on one. DHT-based peer discovery lacks Byzantine resilience and is susceptible to eclipse attacks. Protocol Labs' adversarial model (every honest node outnumbered four-to-one) was judged less impressive as a worst case, since an adversary behaving honestly before switching is not captured — the good-then-malicious worst case lacks empirical study. Team conclusion: the scoring function is the critical element to evaluate; the discovery layer is secondary to current objectives.

**CSM presentation framing.** Jesus relayed that managers want ARC's gossip-stream findings presented at this week's Customer Success Meeting. With the scope unclear (philosophical overview vs technical deep-dive), the group agreed on a neutral high-level summary of the most relevant findings, with details offered on request. Peer-sampling critiques will be framed as an opportunity for better formalisation and proof generation rather than criticism of existing work, clearly distinguishing correctness problems (e.g. uniformity) from security problems (e.g. silent attacks). To manage the social sensitivity of presenting negative results on others' work, Will suggested repurposing 3–5 slides from prior presentations; Denis will present neutrally, with Will and Jesus supporting.

**Decisions.** *Aligned:* three node strategy modules defined — peer-view, connection-acceptance, fan-out dissemination; flat dissemination (forward to all peers except source) as the default; literature inclusion criteria (Byzantine resilience, non-broker, peer-reviewed/archival); review depth — deep on GossipSub, superficial on older protocols; CSM delivery — neutral high-level ARC summary, Denis presenting. *Open:* how a node estimates network size to bound connection count; which revenue/customer use case to pursue (wallet-integrated messaging vs dApp-subsidised public goods).

**Next.** Dana to research the push-protocol 300k-subscriber figure and post findings to the project thread, review Ezequiel's referenced protocol, and continue developing the wallet influencer-trading concept for the next meeting. Group to perform a superficial review of the remaining pub-sub literature. Denis to draft 3–5 neutral slides summarising the GossipSub findings and recommendations. Will and Denis to hold a 1-hour sync to finalise the CSM delivery approach. Jesus to notify Miriam that Denis will lead the topic introduction at the CSM.

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

**Next.** Denis to refine the SecureCyclon presentation slides — simplified problem statement, methodology-first framing — and share next day. Ezequiel available for feedback around 11:00 local after a medical appointment. Both to stay in sync on the final shape ahead of Monday.

---

## 2026-06-02 — PubSub working session: state-machine refactor, gap validation deferred

**Technical report over CIP for July.** Team prioritised a technical report rather than a formal CIP for the July deliverable. David framed the CPS plus a secondary technical report as the value demonstration — it shows the team has identified concrete research use cases and is working toward real-world Cardano applications. Will confirmed a report focused on ideas and solutions (not a formal CIP) is feasible by end of July, reusing the existing CPS structure and current prototype outcomes to validate the use cases.

**Timeline tightened.** David: the deadline was cut from nine months to six, ending in September, but the team should prioritise completing by end of August. David to confirm the completion deadline with Nicolas and align everyone on the end date.

**Second technical report as a collective design doc.** Will proposed the second report serve as a shared document gathering designs and ideas across all topics — prototype details, node lifecycle flows, protocol extensions, and research on prototype developments. Group effort to compile.

**Node refactored to a state machine.** Decision adopted: refactor the node implementation into a state machine to create a clearer separation between the model and the state-transition function. Ezequiel's rationale — the split improves performance by allowing parallel processing while keeping a clean structure. Denis and Will agreed with the concept. The group discussed formally specifying expected behaviour with TLA+ or a similar modern language, but the focus is on *specification*, not formal proofs. Whether the system's maturity yet warrants formal verification (e.g. a TLA+ model) remains an open question — Denis and Ezequiel both flagged it needs further discussion.

**Message gap / ordering validation deferred.** Ezequiel raised that the architecture validates messages by signature but has no defined strategy for missing messages in a sequence or out-of-order arrival. The core difficulty: gaps may arise from network failures, and there is no mechanism to distinguish unintentional gaps from malicious behaviour. Will argued many of these concerns can be handled by submission validation — the node tracks the latest message per publisher per topic in a cache to check that an incoming message properly extends the existing hash chain. Ezequiel countered that this is insufficient: publishers can intentionally omit messages and recipients can't verify the validity of what's missing, which breaks catch-up mechanisms. Unlike a blockchain, the system doesn't consistently check all messages, so submission validation alone can't carry it. Will suggested exploring a ratchet-like data structure to cryptographically enforce sequence numbers and integrity. **Decision:** defer the gap/ordering logic from the current PR; revisit once the relevant flows are specified. Ezequiel is documenting the deferred items in the implementation notes.

**Peer sampling — rejection-sampling research strategy.** Denis recommended reviewing the Basalt protocol (2023) to determine suitability. Group adopted a rejection-sampling approach to the literature: study candidates to either accept one or reject it on constructive grounds, with the rejection then defining the requirements for future research. Denis and Sandro to study Basalt independently and share assessments next week.

**Prototype scope and simulation.** Will and Ezequiel scoped June around aligning protocols for integration. Denis: no specific experiments stand out yet, but analytical sanity checks and metrics like in-degree / out-degree distributions are valuable for verifying the implementation — and data should be separated by node maturity (new vs. long-established) to keep noise from skewing metrics.

**Identity and cryptography.** Jesus reported on identity work: time-dependent misbehaviour detection may need forward-secure signatures, falling back to standard ED25519 if forward security proves unfeasible. Proposal to derive keys for both the peer-sampling and publishing layers from HD wallets (ZIP 1852). Trust anchors — SPO cold keys or DRep certificates — would verify identities and mitigate Sybil attacks via deterministic node-descriptor derivation, letting participants check signatures against known roots of trust to confirm a node is an authorised SPO. For publishing, Will and Jesus explored extending wallet interfaces with a new "pop-up" key type derived through HD paths, so entities can sign messages directly from existing wallets for seamless delegator verification.

**Node lifecycle and on-chain contracts.** Will reviewed the lifecycle flows. Architecture relies on two on-chain contracts: a Topic Registry (authorised publisher keys) and a Subscription List (node registration). Setup: operators generate identity off-chain, fetch registered topics, register public keys and deposits on-chain. Startup: node reads the registered-peer list from chain, filters by topic, and connects via signed descriptors proving public-key ownership. Brief discussion of transaction-size constraints and future options (Merkle trees, minting policies) for managing topic subscriptions. Ezequiel questioned whether the subscription list could move off-chain to reduce on-chain complexity; the group also weighed the bootstrap-node dependency in the current topology.

**Next.** David to confirm the completion deadline with Nicolas. Denis and Sandro to study the Basalt paper independently and share assessments next week. The group to compile the second technical report. Will to re-evaluate the protocol structure and paper around the hash-chain / gap concerns and judge whether more formal modelling is needed. Ezequiel to submit the PR with the new structure and implementation. Will to notify the team about the Thursday meeting (note: availability limited to 30 minutes).

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
