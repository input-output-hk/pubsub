# Year-end 2026 storyline

The narrative behind the [year-end deck](../web/slides/year-end-2026.draft.html), built from the [logbook](../logbook.md), the [biweekly reports](../biweekly-reports/), the [CPS](cps/README.md), the [CIP](cip/README.md) and the CIP repository pull requests ([CPS #1270](https://github.com/cardano-foundation/CIPs/pull/1270), [CIP #1271](https://github.com/cardano-foundation/CIPs/pull/1271)).

## The one-sentence version

We were handed an architecture whose security rested on a peer sampler that had not been analysed against a silent adversary. We set it aside for a simpler on-chain registry, rebuilt the design from evidence (closed-form laws checked against a simulator that runs the real node code), and proposed the result as CIP-0201 with CPS-0038, with what is still open stated as plainly as what is established.

## The through-line: evidence changed our minds three times

The talk is strongest when each act ends on a decision that the evidence forced, not one we started with:

1. **We set aside the foundation we were given.** SecureCyclon was the load-bearing assumption (5 May). Deposits already implied an on-chain list, so we moved to a list-based design (19 May) and dropped peer sampling for the on-chain registry (25 Jun) instead of patching it. The silent-attack report (28 Jun) then showed a silent adversary eclipsing a chosen victim without any of SecureCyclon's nine defences firing, confirming the risk we had stepped away from.
2. **We changed the selected design.** M3 was the implementation base in July (15 Jul). Once the gate, admission cap and churn were measured, M4 won on security and margin, and the group selected it on 20 Aug, security weighted ahead of efficiency (13 Aug).
3. **We re-scoped the deliverable twice.** The CIP was downgraded to optional in July so the experiments came first (15 Jul), then written as a technical report that shows its gaps (11 Aug), and submitted on 16 Sep with that framing intact.

## Act 1 · Starting point (April)

**Message:** the problem is real and specific, and we scoped it early.

- **The problem.** Cardano has no common way to send trustworthy off-chain messages: authenticated against Cardano identities and resistant to targeted suppression. The flagship case is an emergency alert to thousands of stake pool operators (CPS, *The gap*).
- **What we were given.** AUEB's three-layer architecture (D1/D2: peer sampling, navigation, dissemination), SecureCyclon (ICDCS 2023), and product documentation with five use cases.
- **Scoping.** On 28 Apr we prioritised one-way broadcasts (IOG alerts → SPOs, SPOs → delegators, dReps → delegators, dApps → users), folded in three use cases Phillip Daro confirmed, and deprioritised the bidirectional multi-sig branch. Product work continued throughout with Dana: the product doc and SPO onboarding (12 May), the restated use cases (7 Jul), the CIP motivation (4 Aug) and SPO trade-offs (11 Aug).

## Act 2 · Research and reframe (May – early July)

**Message:** we tested the riskiest assumption first, and replaced it.

- **Reviewing the architecture.** The March technical review and the AUEB gap analysis found no stated security model, no anti-spam and no incentives. On 5 May we stopped generating ideas and started generating output: the whole stack rested on SecureCyclon, so analysing it became the priority.
- **SecureCyclon findings.** A *silent* adversary forges nothing and breaks no protocol rule; it only chooses whom it contacts and what it withholds. On 28 May, with Spyros Voulgaris, one of SecureCyclon's authors, we walked through the earlier result that 15 % malicious nodes reach an equilibrium with about 8.9× amplification of adversarial influence. The team's silent-attack report (28 Jun; N = 200, view size 20, 3 seeds) then showed that at 20 % adversarial nodes a targeted victim's view is 73–80 % malicious and at least 80 % malicious in 37–68 % of cycles, near-total at 30 %, with zero detections. The finding holds for the tested configurations, not for every sampler.
- **The reframe.** Since deposits already imply a full on-chain list of participants, a sampling layer adds attack surface without adding security (Denis, 25 Jun). On 19 May we adopted a list-based design, and on 25 Jun we dropped peer sampling for the on-chain registry with rejection sampling. On 7 Jul SecureCyclon was formally retired. Bucketed hash gating against connection flooding dates from 8 May, and it survives into the CIP.
- **Phase 2 starts.** On 30 Jun we moved from building the prototype to running experiments, with one headline metric: does every subscriber receive every message?

## Act 3 · Formalization (June – July)

**Message:** one all-or-nothing bar, one adversary, five designs, and closed-form laws for each.

- **One bar.** A topology draw is *good* when every honest publisher reaches every honest subscriber. The central quantity is p_bad, sized to a target δ = 10⁻⁴ per epoch. Averages hide the failure that matters, because 99.99 % delivery can mean one publisher silenced completely.
- **One adversary.** A fraction μ = 0.2 of registered nodes are silent: they accept links and forward nothing. This is the least capable adversary that still defeats delivery. An eclipse reduces to this case, and churn folds in as a shift from μ to μ + p(1 − μ).
- **Five designs.** Every protocol is described by three per-topic questions (9 Jul): whom it dials, whom it accepts, and where it forwards. M1 push, M2 pull, M3 pull plus publication links, M4 one bidirectional link, M5 independent in and out links.
- **Laws.** Closed forms for coverage, messages, hops, degrees and downtime tolerance (Denis, with Sandro), and for adaptive eclipse cost (Mauro, who also corrected M4's figure on 11 Aug). The coverage laws are leading approximations, asymptotic in N: they hold across the measured range of thousands of nodes, not for topics of tens.

## Act 4 · Reference implementation (May – August)

**Message:** we built a second instrument on the real node code, and it agrees with the maths where failures can be counted.

- **Rust node.** It has a pure state-transition core with effects at the edges: topics, signed envelopes, registries and a strategy per model. The unified selection plane (PR #119) made all 20 model recipes (M1–M5 × gated/capped) one-command configurations.
- **Deterministic simulation on the real node code.** Runs are reproducible from a seed alone. Memory per run fell from ~30 GB to ~0.6 GB (PR #118), and a sweep went from minutes to seconds. Because it runs the prototype's own logic, agreement with the laws is also statistical evidence that the implementation matches the formal model.
- **Cross-validation.** We compared predicted against measured p_bad across 25 datasets at N = 4,000 and 20,000, including deep-tail reruns of 110,000–170,000 draws. 38 of 40 downtime predictions fall within their 95 % interval, and the ungated discrepancy averages about 2 %. That independence applies to the ungated laws; the gated closed forms behind the selected configuration were derived after the measurements on one instrument and reproduced in review, with no second implementation.
- **AI tooling.** We used Spec Kit (specify → plan → tasks → implement) under a project constitution, with 13 feature specs, ADRs and analysis ledgers. Claude helped build and review, which is not independent: on 2 Jul the team recorded that Claude writing both the implementation and the simulation risks correlated errors, and named the hand-derived laws as the independent check.

## Act 5 · The CIP and open questions (August – November)

**Message:** M4 is the evidence-backed choice, the CIP is proposed and in review, and it states its own limits.

- **Choosing M4 (20 Aug).** One symmetric link gives one eligibility draw per pair; either endpoint can supply an honest connection; and one link kind serves both publishing and relaying. In the ungated comparison at equal security, M4 needs 9 picks where M3 needs 19 and M1/M2 need 24. Under the gate and cap (N = 20,000, μ = 0.2), the models predict p_bad 5.1 × 10⁻⁶ for M4 at k = 10 (B = 500, C = 23) and 7.57 % honest downtime absorbed, against M3's 5.8 × 10⁻⁵ and 1.58 %; at 9 picks M4 absorbs about 2.6 %. These are predictions, not measured rates. Where failures were frequent enough to count, M3 failed 17 of 400 runs and M4 none of 400. The price is about 22 % more bandwidth in the ungated comparison, which binds nothing at realistic loads, while M3's connection count does.
- **A fix found along the way.** Hash gating on directed pairs doubles the chance of linking to an adversary once links are symmetric. Gating on the identity-sorted pair restores the 1/B density (13 Aug).
- **The CIP and CPS.** Both were submitted on 16 Sep and confirmed as CPS-0038 and CIP-0201 on 29 Sep. CIP-0201 specifies epochs, topology derivation, the verifiable gate, link establishment, signed messages, recovery caches, registries and parameters. CPS-0038 states the gap, four use cases, six ranked goals and seven open questions. Mauro's end-to-end review (1 Sep) shaped the final text.
- **What reviewers are asking.** How large the deposit must be to make μ = 0.2 implausible: we can state the outlay (4,000 simultaneous deposits at N = 20,000), not yet the threshold. What the protocol adds over the publish-only channels operators already run.
- **Gaps, stated plainly.** Tail probabilities are extrapolated, not measured. Rotation is argued, not simulated. The gated laws are reproduced in review but not formally derived. Small topics are outside the analysis: at k = 10 the gate is off up to 40 registrations, and a topic of 50 is not a small instance of the analysed case. Correlated failure is out of scope. μ is chosen, not derived. Behaviour through a chain halt or fork is unspecified. The relationship to CIP-0137 is unresolved. *Path to Active* lists the work: interoperability spec, deployment profile, the B = 512 / k = 10 / C = 24 validation run, two implementations, and an operator pilot.

## Act 6 · How we worked

**Message:** specification first, decisions in public, and a cadence that kept a seven-person team aligned.

- Weekly sessions plus brainstorms, a public logbook and biweekly reports, ADRs for every design decision, and a public repository and website since August.
- Specification-driven design was reaffirmed on 4 Aug: someone joining late, as Mauro did, can see why decisions were made, not only what was built.
- One lesson from each presenter (William, Mauro, Ezequiel), for each to confirm. Candidates from the record: return dropped messages as explicit effects, not logs (Ezequiel, 15 Jul); write plain prose first and keep dense data separate (20 Aug).

## Dates worth putting on a timeline slide

| Date | Moment |
| --- | --- |
| 28 Apr | Scope narrowed to four one-way use cases |
| 5 May | Pivot from research to output; SecureCyclon named the load-bearing assumption |
| 8 May | Bucketed hash gating proposed against flooding |
| 19 May | List-based architecture adopted |
| 28 May | SecureCyclon review with Spyros Voulgaris (8.9× amplification) |
| 25 Jun | Peer sampling dropped for the on-chain registry |
| 28 Jun | Silent-attack report on SecureCyclon |
| 30 Jun | Phase 2: from building to experiments |
| 7 Jul | SecureCyclon formally retired; M3 adopted as base |
| 3 Aug | Experiments framework and unified selection plane merged; repository public |
| 13 Aug | Security weighted ahead of efficiency; canonical-pair gate fix |
| 20 Aug | M4 selected |
| 1 Sep | Reader review's design questions resolved |
| 16 Sep | CPS and CIP submitted |
| 29 Sep | Confirmed as CPS-0038 and CIP-0201 |
