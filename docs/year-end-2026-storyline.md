# Year-end 2026 storyline

The narrative behind the [year-end deck](../web/slides/year-end-2026.draft.html), built from the [logbook](../logbook.md), the [biweekly reports](../biweekly-reports/), the [CPS](cps/README.md), the [CIP](cip/README.md), the March technical review, the AUEB gap analysis and the CIP repository pull requests ([CPS #1270](https://github.com/cardano-foundation/CIPs/pull/1270), [CIP #1271](https://github.com/cardano-foundation/CIPs/pull/1271)).

## The shape of the talk

Outcome first, then how we got there. The audience sees CIP-0201 on slide 3, so every later slide answers one question: why does the protocol look like this?

1. **The outcome** (slide 3)
2. **How we got there: replace the foundation** (4–7)
3. **How we got there: make it measurable** (8–12, Mauro and Ezequiel)
4. **How we got there: let the evidence choose** (13–15, with a live demo of the compare-designs tool on 14)
5. **What's open** (16–17)
6. **How we worked: lessons learned** (18)

The through-line is that evidence changed our minds three times: we set SecureCyclon aside, we moved from M3 to M4, and we re-scoped the CIP into a report that states its own limits.

## 1 · The outcome

CIP-0201 is a topic-based pub/sub layer anchored on Cardano, with four mechanisms:

- **Registered nodes:** keys on chain, each with a refundable deposit.
- **A verifiable gate:** public randomness fixes which pairs may connect each epoch; each node picks privately from that set.
- **Symmetric links:** one link per pair carries messages both ways, so either end can supply an honest path.
- **Signed messages:** every subscriber verifies, relays and caches for recovery.

The guarantee, stated with its conditions: at 20,000 nodes with 20 % silent, at the reference configuration (k = 10, B = 500, C = 23), the models predict that some honest publisher fails to reach every honest subscriber in about 1 of 200,000 epochs (p_bad 5.1 × 10⁻⁶), redrawn every epoch. This is a prediction, not a measurement.

Status: CPS-0038 and CIP-0201 were submitted on 16 Sep, confirmed by the editors on 29 Sep, and are Proposed and in review.

## 2 · Replace the foundation

**What we were given.** Four inputs:

- AUEB's deliverables D1 and D2: a three-layer overlay (peer sampling, navigation, dissemination), replication servers and an on-chain topic registry.
- SecureCyclon (ICDCS 2023) as the peer-sampling layer that every guarantee above it inherited from.
- The product documentation: five use cases with requirements.
- A direct line to Spyros Voulgaris, SecureCyclon's co-author, in our sessions from 30 Apr.

The March review reduced the five use cases to one primitive: identifiable parties send verifiable events to interested subscribers. On 28 Apr we scoped to one-way broadcasts and deprioritised the bidirectional multi-sig branch.

**Reviewing the architecture.** The March review and the gap analysis found no security model, no anti-spam protection and no incentives. The framing finding came on 30 Apr: the threat is silencing, not forgery. SecureCyclon gives non-bias (X % malicious nodes yield X % malicious links), which does not stop silencing, and Spyros agreed the design did not address it (gap analysis S-02, S-04). On 5 May we stopped generating ideas and turned to SecureCyclon.

**SecureCyclon findings.** A silent adversary forges nothing and breaks no rule; it only chooses whom it contacts and what it withholds. The team's silent-attack report (28 Jun; N = 200, view size 20, 3 seeds) found that at 20 % adversarial nodes a targeted victim's view is 72.5–80.3 % malicious, and at least 80 % malicious in 37–68 % of cycles. At 30 % it is 98.6–99.3 %. No detection fired. The finding holds for the tested configurations, not for every sampler. Earlier, on 28 May with Spyros, we revisited the result that 15 % malicious nodes reach about 8.9× amplification.

**A deposit implies a list, and a list replaces sampling.** Sybil resistance needs a cost per identity (23 Jun). A deposit implies an on-chain list, and once every node is known, a sampling layer adds attack surface without adding security (25 Jun). A known list invites targeted flooding, hence the verifiable gate (bucketed hash gating, 8 May; restated 25 Jun). Timeline: 8 May bucketed gating, 19 May list-based design, 25 Jun on-chain registry with rejection sampling, 28 Jun silent-attack report, 30 Jun phase 2 moves to experiments, 7 Jul SecureCyclon formally retired.

## 3 · Make it measurable (Mauro, Ezequiel)

Their slides define the problem (good topology, silent fraction μ, p_bad ≤ δ, downtime as a shift in μ), give the five models' leading coverage laws, and show one protocol core run two ways (prototype node and deterministic simulator), AI-assisted development with structure, and predictions checked against execution.

Background for questions: the ungated laws agree with the simulator in 38 of 40 downtime predictions within 95 % intervals; the gated closed forms were derived after the measurements on one instrument and reproduced in review, without a second implementation. The 2 Jul logbook records why the hand-derived laws matter: Claude helped write both the node and the simulator, so correlated errors were possible.

## 4 · Let the evidence choose

**Evidence moved us from M3 to M4.** M3 was the implementation base in July (7 and 15 Jul), chosen from comparisons without the gate or cap. With them, and with security weighted ahead of efficiency (13 Aug), M4 won and was selected on 20 Aug.

| | M3 | M4 | Source |
| --- | ---: | ---: | --- |
| Picks to meet the target (ungated) | 19 | 9 | CIP Table 9 |
| Mean links held (ungated) | 38.0 | 18.0 | Companion Table 4 |
| Copies received per node (ungated) | 10.4 | 13.4 | Companion Table 4 |
| Predicted p_bad (gated reference) | 5.8 × 10⁻⁵ | 5.1 × 10⁻⁶ | CIP Table 10 |
| Predicted downtime absorbed (gated) | 1.58 % | 7.57 % | CIP Table 10 |
| Failed runs at 32 eligible peers | 17 / 400 | 0 / 400 | CIP, Alternatives considered |

M3 keeps the bandwidth edge: it uses about 22 % less bandwidth (10.4 against 13.4 copies per node). Gated settings: M3 RF = 13, s = 7, B = 769; M4 k = 10, B = 500, C = 23.

**Why it looks like this.** Each mechanism answers a finding:

| Finding | Date | What CIP-0201 does |
| --- | --- | --- |
| The threat is silencing, not forgery | 30 Apr | Signed messages settle forgery; one bar for the rest: every honest publisher reaches every honest subscriber |
| Identities must cost something | S-04 · 23 Jun | Refundable deposit and an on-chain node registry |
| A full list makes peer sampling unnecessary | 25 and 28 Jun | Nodes select locally from the registry |
| A known list invites targeted flooding | 8 May · 25 Jun | A verifiable gate from epoch randomness, plus an admissions budget (ADR 0042) |
| One link serving both directions needs the fewest picks and leaves the most margin | 11–20 Aug | The symmetric relay link, gated on the identity-sorted pair (13 Aug) |
| A bad draw must not last | 13 Aug | Epoch rotation and recovery caches |

## 5 · What's open

**Where review stands.** Submitted 16 Sep, confirmed 29 Sep, in review with no activity since 29 Sep; the README targets approval for October–November. Reviewers asked how large the deposit must be (we can state the outlay, 4,000 simultaneous deposits to hold μ = 0.2 of 20,000, not yet the threshold) and what the protocol adds over the publish-only channels operators already run. Feedback is invited on the relationship to CIP-0137 and on the deployment parameter profile.

**Gaps.** The CIP states each of these itself, under Limits of this evidence or Path to Active.

- *Evidence:* tail probabilities extrapolated; rotation argued, not simulated; gated laws not formally derived; small topics outside the analysis (the gate is off up to 40 registrations at k = 10); correlated failure out of scope.
- *Specification:* gate, beacon, handshake and epochs; recovery and catch-up exchanges; behaviour through a chain halt or fork; the relationship to CIP-0137.
- *Deployment:* μ and deposit economics; rerun of the candidate profile B = 512, k = 10, C = 24; two interoperating implementations; an operator pilot and adoption threshold.

## 6 · Lessons learned

Candidates for the three presenters to confirm or replace.

**What worked**

- *Decisions written down.* The logbook and ADRs let Mauro join on 6 Aug and correct M4's eclipse-cost figure by 11 Aug.
- *Two instruments checking each other.* Hand-derived laws against a simulator running the real node code (2 Jul, 13 Aug).
- *A CIP that states its limits.* Framed as a technical report with its gaps visible (11 Aug).

**What we'd do differently**

- *Question the foundation sooner.* "Is full gossip overkill at SPO scale?" is in the 14 Apr notes; peer sampling went on 25 Jun, once the on-chain list (added to unblock the prototype) was recognised as the design.
- *Compare under the real rules from the start.* M3 became the base on 7 and 15 Jul from comparisons without the gate or cap; with them, M4 won on 20 Aug.
- *Size for the real audience earlier.* We sized at N = 20,000 (bracket from 28 Apr); on 1 Sep chain data showed about 2,700 pools ever registered and about 900 holding 99 % of stake, and wallet-mediated topics may hold tens. Small topics are the largest open gap.
- *Treat chain failure as a first-class requirement.* Alerts matter most when the chain is degraded; raised 2 Jul, analysed 13 Aug, still unspecified in the CIP.

## Tools on the website worth showing

- [Compare designs](../web/experiments/compare-designs/): ungated M1–M5 estimates with population, adversarial share and pick count as controls. Linked from slide 13 and embedded live on slide 14.
- [Parameter surface](../web/experiments/parameters/): exploratory M4 isolation estimates, bucket-count ceilings and an admissions-budget recipe. Linked from slide 17.
- [Model simulator](../web/experiments/models/): watch each model draw its links and a message propagate, at toy scale.
- [How the design was derived](../web/experiments/): the comparisons behind selecting M4.
