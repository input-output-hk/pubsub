# Year-end 2026 storyline

The narrative behind the [year-end deck](../web/slides/year-end-2026.draft.html), built from the [logbook](../logbook.md), the [biweekly reports](../biweekly-reports/), the [CPS](cps/README.md), the [CIP](cip/README.md) and the CIP repository pull requests ([CPS #1270](https://github.com/cardano-foundation/CIPs/pull/1270), [CIP #1271](https://github.com/cardano-foundation/CIPs/pull/1271)).

## The one-sentence version

We were handed an architecture that rested on an untested peer sampler; we tested it, found it could be broken silently, rebuilt the design from evidence with two independent instruments that agree, and turned the result into CIP-0201 and CPS-0038, with what is still open stated as plainly as what is established.

## The through-line: evidence changed our minds three times

The talk is strongest when each act ends on a decision that the evidence forced, not one we started with:

1. **We dropped the foundation we were given.** SecureCyclon was the load-bearing assumption (5 May). A silent adversary eclipses a chosen victim without any of its nine defences firing, so we replaced peer sampling with an on-chain registry (19 May – 7 Jul) instead of patching it.
2. **We changed the selected design.** M3 was the implementation base in July (15 Jul). Once the gate, admission cap and churn were measured, M4 won on security and margin, and the group selected it on 20 Aug, security weighted ahead of efficiency (13 Aug).
3. **We re-scoped the deliverable twice.** The CIP was downgraded to optional in July so the experiments came first (15 Jul), then written as a technical report that shows its gaps (11 Aug), and submitted on 16 Sep with that framing intact.

## Act 1 · Starting point (April)

**Message:** the problem is real and specific; the brief was thin, so we sharpened it ourselves.

- **The problem.** Cardano has no common way to send trustworthy off-chain messages: authenticated against Cardano identities and resistant to targeted suppression. The flagship case is an emergency alert to thousands of stake pool operators (CPS, *The gap*).
- **What we were given.** AUEB's three-layer architecture (D1/D2: peer sampling, navigation, dissemination), SecureCyclon (ICDCS 2023), and product documentation with five use cases.
- **The sparse product input.** Little steering after the initial use cases. On 28 Apr we narrowed scope to one-way broadcasts: IOG alerts → SPOs, SPOs → delegators, dReps → delegators, dApps → users, and dropped the bidirectional multi-sig branch.

## Act 2 · Research and reframe (May – early July)

**Message:** we tested the riskiest assumption first, and it failed.

- **Reviewing the architecture.** The March technical review and the AUEB gap analysis found no stated security model, no anti-spam and no incentives. On 5 May we stopped generating ideas and started generating output: the whole stack rested on SecureCyclon, so analysing it became the priority.
- **SecureCyclon findings.** A *silent* adversary, one that forges nothing and breaks no protocol rule but only chooses whom it contacts and what it withholds, amplifies its share of views. At 20 % adversarial nodes, a targeted victim's view is about 74–80 % malicious and fully eclipsed in 37–68 % of cycles. At 30 % the eclipse is total. Detection fired zero times (SecureCyclon team report). Spyros Voulgaris, one of its authors, reviewed the result with us on 28 May.
- **The reframe.** Since deposits already imply a full on-chain list of participants, a sampling layer adds attack surface without adding security. On 19 May we adopted a list-based design, and on 25 Jun we dropped peer sampling for the on-chain registry with rejection sampling. On 7 Jul SecureCyclon was formally retired. Bucketed hash gating against connection flooding dates from 8 May, and it survives into the CIP.
- **Phase 2 starts.** On 30 Jun we moved from building the prototype to running experiments, with one headline metric: does every subscriber receive every message?

## Act 3 · Formalization (June – July)

**Message:** one all-or-nothing bar, one adversary, five designs, and closed-form laws for each.

- **One bar.** A topology draw is *good* when every honest publisher reaches every honest subscriber. The central quantity is p_bad, sized to a target δ = 10⁻⁴ per epoch. Averages hide the failure that matters, because 99.99 % delivery can mean one publisher silenced completely.
- **One adversary.** A fraction μ = 0.2 of registered nodes are silent: they accept links and forward nothing. This is the least capable adversary that still defeats delivery. An eclipse reduces to this case, and churn folds in as a shift from μ to μ + p(1 − μ).
- **Five designs.** Every protocol is described by three per-topic questions (9 Jul): whom it dials, whom it accepts, and where it forwards. M1 push, M2 pull, M3 pull plus publication links, M4 one bidirectional link, M5 independent in and out links.
- **Laws.** Closed forms for coverage, messages, hops, degrees, eclipse cost and downtime tolerance (Denis and Sandro), so each design can be read at any N, μ and δ.

## Act 4 · Reference implementation (May – August)

**Message:** we built a second, independent instrument, and it agrees with the maths.

- **Rust node.** It has a pure state-transition core with effects at the edges: topics, signed envelopes, registries and a strategy per model. The unified selection plane (PR #119) made all 20 model recipes (M1–M5 × gated/capped) one-command configurations.
- **Deterministic simulation on the real node code.** Runs are reproducible from a seed alone. Memory per run fell from ~30 GB to ~0.6 GB (PR #118), and a sweep went from minutes to seconds. Because it runs the prototype's own logic, agreement with the laws is also evidence that the implementation matches the formal model.
- **Cross-validation.** We compared predicted against measured p_bad across 25 datasets at N = 4,000 and 20,000, including deep-tail reruns of 110,000–170,000 draws. 38 of 40 downtime predictions fall within their 95 % interval, and ungated discrepancy averages about 2 %.
- **AI tooling.** We used Spec Kit (specify → plan → tasks → implement) under a project constitution, with 16 feature specs, ADRs and analysis ledgers. Claude acted as an independent second reviewer, and we were clear about where humans had to check.

## Act 5 · The CIP and open questions (August – November)

**Message:** M4 is the evidence-backed choice, the CIP is in review, and it states its own limits.

- **Choosing M4 (20 Aug).** One symmetric link gives one eligibility draw per pair; either endpoint can supply an honest connection; and one link kind serves both publishing and relaying. At equal security, M4 needs 9 picks where M3 needs 19 and M1/M2 need 24. Under the gate and cap (N = 20,000, μ = 0.2), M4 predicts p_bad 5.1 × 10⁻⁶ and absorbs 7.57 % honest downtime, against M3's 5.8 × 10⁻⁵ and 1.58 %. Where failures were frequent enough to count, M3 failed 17 of 400 runs and M4 none of 400. The price is about 22 % more bandwidth, which binds nothing at realistic loads, while M3's connection count does.
- **A fix found along the way.** Hash gating on directed pairs doubles the chance of linking to an adversary once links are symmetric. Gating on the identity-sorted pair restores the 1/B density (13 Aug).
- **The CIP and CPS.** Both were submitted on 16 Sep and confirmed as CPS-0038 and CIP-0201 on 29 Sep. CIP-0201 specifies epochs, topology derivation, the verifiable gate, link establishment, signed messages, recovery caches, registries and parameters. CPS-0038 states the gap, four use cases, six ranked goals and seven open questions. Mauro's end-to-end review (1 Sep) shaped the final text.
- **What reviewers are asking.** How large the deposit must be to make μ = 0.2 implausible: we can state the outlay (4,000 simultaneous deposits at N = 20,000), not yet the threshold. What the protocol adds over the publish-only channels operators already run.
- **Gaps, stated plainly.** Tail probabilities are extrapolated, not measured. Rotation is argued, not simulated. Small topics (below roughly 37 participants the gate switches off) are outside the analysis. Correlated failure is out of scope. μ is chosen, not derived. Behaviour through a chain halt or fork is unspecified. The relationship to CIP-0137 is unresolved. *Path to Active* lists the work: interoperability spec, deployment profile, the B = 512 / k = 10 / C = 24 validation run, two implementations, and an operator pilot.

## Act 6 · How we worked

**Message:** specification first, decisions in public, and a cadence that kept six people aligned.

- Weekly sessions plus brainstorms, a public logbook and biweekly reports, ADRs for every design decision, and a public repository and website since August.
- Specification-driven design was reaffirmed on 4 Aug: someone joining late, as Mauro did, can see why decisions were made, not only what was built.
- One lesson from each presenter. Candidates from the record: return dropped messages as explicit effects, not logs (Ezequiel, 15 Jul); weight security before efficiency explicitly (Denis, 13 Aug); write plain prose first and keep dense data separate (20 Aug).

## Dates worth putting on a timeline slide

| Date | Moment |
| --- | --- |
| 28 Apr | Scope narrowed to four one-way use cases |
| 5 May | Pivot from research to output; SecureCyclon named the load-bearing assumption |
| 8 May | Bucketed hash gating proposed against flooding |
| 19 May | List-based architecture adopted |
| 28 May | SecureCyclon verdict reviewed with Spyros Voulgaris |
| 25 Jun | Peer sampling dropped for the on-chain registry |
| 30 Jun | Phase 2: from building to experiments |
| 7 Jul | SecureCyclon formally retired; M3 adopted as base |
| 3 Aug | Experiments framework and unified selection plane merged; repository public |
| 13 Aug | Security weighted ahead of efficiency; canonical-pair gate fix |
| 20 Aug | M4 selected |
| 1 Sep | Reader review's design questions resolved |
| 16 Sep | CPS and CIP submitted |
| 29 Sep | Confirmed as CPS-0038 and CIP-0201 |
