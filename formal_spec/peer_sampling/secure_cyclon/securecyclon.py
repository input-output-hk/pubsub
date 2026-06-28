"""
SecureCyclon discrete-cycle simulator.

Faithful re-implementation of the peer-sampling protocol from
  Antonov & Voulgaris, "SecureCyclon: Dependable Peer Sampling" (arXiv:2309.02952v1)
built to analyse *silent* attacks -- adversarial behaviours that produce no
cryptographic evidence and therefore never trip SecureCyclon's detection
machinery (frequency check + ownership/cloning check).

Implements:
  * Cyclon shuffle (oldest-as-partner, fresh-self injection, swap of s, gap-fill integration)
  * SecureCyclon ownership chains (creator -> owner1 -> owner2 -> ...; transfer = append + "sign")
  * sample caching + the two provable checks (frequency, ownership/cloning)
  * redemption cache (size r, gossiped as samples)
  * tit-for-tat one-at-a-time ownership transfer (toggleable), with the loss falling on the initiator
  * non-swappable empty-slot repair (V-A) with the 3 abuse limits
  * violation -> proof -> network blacklist

Adversary strategies (sim.NodeKind):
  HONEST        -- follows the protocol
  DROP          -- silent link drop (attack a): aborts exchanges to deplete legit views
  BIAS          -- biased subset (attack b): adversary-descriptors-first + hoards legit descriptors
  CLONE         -- CONTROL: forks an ownership chain (provable cloning) -- detector MUST fire
  OVERINJECT    -- CONTROL: mints >1 fresh self per cycle (provable frequency) -- detector MUST fire

The detector is always active; the whole point is that DROP and BIAS leave it at zero
while CLONE and OVERINJECT trip it.
"""

from __future__ import annotations

import random
from collections import deque, defaultdict
from dataclasses import dataclass, field
from enum import Enum
from typing import Optional


# --------------------------------------------------------------------------- #
#  Descriptor (the "communication certificate")
# --------------------------------------------------------------------------- #

class Descriptor:
    """A SecureCyclon node descriptor.

    Identity for cross-checking is (creator, ts).  `chain` is the ownership
    chain: chain[0] == creator, chain[-1] == current owner.  Ownership transfer
    A->B appends B (modelling: append B's pubkey + A's signature; signatures are
    assumed unforgeable, so a chain is a trustworthy list).
    """
    __slots__ = ("creator", "ts", "age", "chain", "swappable")

    def __init__(self, creator: int, ts: float, chain: tuple, age: int = 0,
                 swappable: bool = True):
        self.creator = creator
        self.ts = ts                 # creation timestamp (wall-clock; we use fractional cycle)
        self.age = age               # cycles since creation (Cyclon staleness)
        self.chain = chain           # tuple of owner ids, chain[0]==creator
        self.swappable = swappable

    @property
    def owner(self) -> int:
        return self.chain[-1]

    @property
    def key(self) -> tuple:
        return (self.creator, self.ts)

    def transferred_to(self, new_owner: int) -> "Descriptor":
        """Return this descriptor with ownership moved to new_owner (chain extended).

        The instance is unique (no cloning): we mutate the chain in place and
        hand the same object to the new owner.
        """
        self.chain = self.chain + (new_owner,)
        return self

    def copy_as_sample(self) -> "Descriptor":
        """A non-owning copy used purely for violation cross-checking."""
        return Descriptor(self.creator, self.ts, self.chain, self.age, swappable=False)

    def __repr__(self):
        return f"D(c={self.creator},ts={self.ts},chain={self.chain})"


def chains_compatible(c1: tuple, c2: tuple) -> bool:
    """Two chains for the SAME (creator, ts) are legitimate iff one is a prefix
    of the other (same single traversal path, observed at different points).
    Divergence == a node forked the chain == cloning."""
    n = min(len(c1), len(c2))
    return c1[:n] == c2[:n]


def fork_node(c1: tuple, c2: tuple) -> Optional[int]:
    """The provable offender (the node that double-transferred) if c1, c2
    diverge; else None."""
    n = min(len(c1), len(c2))
    for i in range(n):
        if c1[i] != c2[i]:
            return c1[i - 1]      # last common owner forked to two successors
    return None


# --------------------------------------------------------------------------- #
#  Violation proof
# --------------------------------------------------------------------------- #

@dataclass
class Proof:
    kind: str            # "clone" | "frequency"
    offender: int
    cycle: int
    evidence: tuple      # the two conflicting (creator, ts, chain) tuples / timestamps


# --------------------------------------------------------------------------- #
#  Node
# --------------------------------------------------------------------------- #

class NodeKind(Enum):
    HONEST = "honest"
    DROP = "drop"
    BIAS = "bias"
    CLONE = "clone"
    OVERINJECT = "overinject"
    CLONE_SPARSE = "clone_sparse"   # biased subset + tunable sparse chain-forking
    PASSIVE = "passive"             # flagged malicious but behaves honestly (for isolating silence)


class Node:
    def __init__(self, nid: int, kind: NodeKind, sim: "Simulator"):
        self.id = nid
        self.kind = kind
        self.sim = sim
        # swappable view: id -> Descriptor (current owned, swappable links)
        self.view: dict[int, Descriptor] = {}
        # non-swappable repair stubs: id -> Descriptor (redeemable, not swappable)
        self.nonswap: dict[int, Descriptor] = {}
        # sample cache for violation discovery: (creator,ts) -> longest chain seen
        self.samples: dict[tuple, tuple] = {}
        self.sample_birth: dict[tuple, int] = {}        # (creator,ts) -> cycle first cached
        self.ts_by_creator: dict[int, list] = defaultdict(list)  # creator -> [ts,...]
        # redemption cache: last r redeemed descriptors (gossiped as samples)
        self.redcache: deque = deque(maxlen=sim.r)
        # per-cycle non-swappable redemption accounting (abuse limits, V-A)
        self.ns_redemptions_this_cycle = 0
        self.ns_redeemed_keys_this_cycle: set = set()
        # UNBOUNDED adversary store of distinct victim-tokens (ts -> Descriptor); not l-capped,
        # not creator-keyed, never evicted -- models an adversary not bound by honest view rules.
        self.tstockpile: dict[float, Descriptor] = {}
        self.contacts_serviced = 0                      # contacts serviced THIS cycle (volume limit)

    # ---- helpers ---------------------------------------------------------- #
    @property
    def malicious(self) -> bool:
        return self.kind is not NodeKind.HONEST

    def is_malicious_target(self, other_id: int) -> bool:
        return self.sim.nodes[other_id].malicious

    def view_size(self) -> int:
        return len(self.view) + len(self.nonswap)

    def all_owned(self) -> list:
        return list(self.view.values()) + list(self.nonswap.values())

    # ---- descriptor creation --------------------------------------------- #
    def fresh_self(self, ts: float) -> Descriptor:
        return Descriptor(creator=self.id, ts=ts, chain=(self.id,), age=0, swappable=True)

    # ---- sample caching + the two provable checks ------------------------- #
    def cache_sample(self, creator: int, ts: float, chain: tuple) -> Optional[Proof]:
        """Run frequency + ownership checks on an observed descriptor, then cache.
        Returns a Proof if a provable violation is discovered, else None."""
        cyc = self.sim.cycle
        # ---- frequency check: two same-creator timestamps closer than the period
        period = self.sim.gossip_period
        for prev_ts in self.ts_by_creator[creator]:
            if prev_ts != ts and abs(prev_ts - ts) < period:
                return Proof("frequency", creator, cyc, (prev_ts, ts))
        # ---- ownership / cloning check
        key = (creator, ts)
        prev = self.samples.get(key)
        if prev is not None:
            if not chains_compatible(prev, chain):
                off = fork_node(prev, chain)
                return Proof("clone", off, cyc, (prev, chain))
            # keep the longer (more-updated) chain
            if len(chain) > len(prev):
                self.samples[key] = chain
                self.sample_birth[key] = self.sample_birth.get(key, cyc)
            return None
        # new sample
        self.samples[key] = chain
        self.sample_birth[key] = cyc
        self.ts_by_creator[creator].append(ts)
        return None

    def prune_samples(self):
        """Bound memory: drop samples older than the detection window.  Clones /
        frequency violations surface within a few cycles, and honest descriptors
        older than ~2l are long dead, so this never hides a real violation."""
        cyc = self.sim.cycle
        w = self.sim.sample_window
        if w <= 0:
            return
        dead = [k for k, b in self.sample_birth.items() if cyc - b > w]
        for k in dead:
            self.samples.pop(k, None)
            self.sample_birth.pop(k, None)
        if dead:
            for creator in list(self.ts_by_creator):
                kept = [t for t in self.ts_by_creator[creator]
                        if (creator, t) in self.samples]
                if kept:
                    self.ts_by_creator[creator] = kept
                else:
                    del self.ts_by_creator[creator]


# --------------------------------------------------------------------------- #
#  Simulator
# --------------------------------------------------------------------------- #

class Simulator:
    def __init__(self,
                 n: int = 1000,
                 view_len: int = 20,
                 swap: int = 3,
                 r: int = 5,
                 malicious_frac: float = 0.0,
                 attack_kind: NodeKind = NodeKind.HONEST,
                 attack_start: int = 50,
                 tit_for_tat: bool = True,
                 churn: float = 0.0,
                 drop_role: str = "both",      # "both" | "partner" | "initiator"
                 drop_mode: str = "bait",      # "empty" (pure non-response) | "bait" (extract 1)
                 clone_rate: float = 0.0,      # CLONE_SPARSE: forks per malicious node per cycle
                 clone_min_age: int = 0,       # CLONE_SPARSE: only fork descriptors at least this old
                 flood_delay: int = 0,         # cycles between proof discovery and global blacklist
                 clock_skew: float = 0.0,      # malicious future-date their fresh self-descriptors by this
                 d1_tol: float = 1e9,          # D1: reject a descriptor whose ts exceeds now+d1_tol (clock-skew check)
                 eclipse_targets=None,         # victim ids to silently eclipse (concentrate on)
                 eclipse_hoard: bool = True,   # if False: contact-victim but still fill offers to s (no under-delivery)
                 eclipse_starve: bool = False, # malicious return empty subsets to non-victim honest nodes (concentrate ammo on victim)
                 eclipse_refuse_invites: bool = False,  # malicious refuse to REPLY when a non-victim honest node invites them
                 eclipse_token_dup: bool = False,  # LINEAR prefix-extension cloning of victim-tokens across the coalition
                 eclipse_nonswap_tokens: bool = False,  # adversary holds victim-tokens as NON-SWAPPABLE (test: does it amplify?)
                 eclipse_stockpile: bool = False,  # adversary HOARDS distinct victim-tokens in an unbounded store and BURSTS
                 eclipse_covert: bool = False,  # covert channel: harvest victim-tokens into a SHARED coalition pool + load-balance
                 covert_harvest_frac: float = 1.0,  # HYBRID: fraction of adversaries that harvest (reply); rest stay silent injectors
                 eclipse_burst: int = 1000,        # max stockpiled tokens an adversary redeems per cycle (throughput)
                 volume_limit: int = 0,            # MITIGATION: max contacts a node services per cycle (0 = off)
                 redeem_dedup: bool = False,   # REJECTED (kept off; not a viable defense): refusing repeat-ts
                                               # redemptions blindly drops legitimate §V-A repair-stub redemptions
                                               # (~12% of honest contacts, det=0), more honest than adversary -- it
                                               # cannot distinguish a repair stub from a stale hoarded token. Use
                                               # volume_limit (ts-agnostic) + D4 (signed forks) instead.
                 redeem_window: int = 60,      # how many cycles the redeemed-ts table remembers
                 eclipse_inject_youngest: bool = False,  # inject freshest (lowest-staleness) adversary descriptors into victims
                 eclipse_victim=None,          # the single node to eclipse (for sample-derived healer-targeting)
                 healer_from_samples: bool = False,  # estimate the victim's healers from disseminated samples (realistic)
                 healer_recency: int = 5,      # how many cycles a sample-observed healer stays "known"
                 eclipse_accumulate_until: int = 0,  # hoard victim tokens (don't redeem) until this cycle, then burst
                 sample_window: int = 60,
                 seed: int = 0):
        self.n = n
        self.l = view_len
        self.s = swap
        self.r = r
        self.malicious_frac = malicious_frac
        self.attack_kind = attack_kind
        self.attack_start = attack_start
        self.tit_for_tat = tit_for_tat
        self.churn = churn
        self.drop_role = drop_role
        self.drop_mode = drop_mode
        self.clone_rate = clone_rate
        self.clone_min_age = clone_min_age
        self.clone_events = 0          # total forks attempted (for detection-rate measurement)
        self.flood_delay = flood_delay
        self.clock_skew = clock_skew
        self.d1_tol = d1_tol
        self._pending_blacklist: dict = {}     # offender -> (commit_cycle, proof)
        self.eclipse_targets = set(eclipse_targets) if eclipse_targets else set()
        self.eclipse_hoard = eclipse_hoard
        self.eclipse_starve = eclipse_starve
        self.eclipse_refuse_invites = eclipse_refuse_invites
        self.eclipse_token_dup = eclipse_token_dup
        self.eclipse_nonswap_tokens = eclipse_nonswap_tokens
        self.eclipse_stockpile = eclipse_stockpile
        self.eclipse_covert = eclipse_covert
        self.covert_harvest_frac = covert_harvest_frac
        self.covert_pool: dict = {}   # SHARED coalition store of harvested victim-tokens: (creator,ts)->Descriptor
        self.eclipse_burst = eclipse_burst
        self.volume_limit = volume_limit
        self.redeem_dedup = redeem_dedup
        self.redeem_window = redeem_window
        self._redeem_seen = defaultdict(dict)   # node_id -> {ts_of_own_descriptor: cycle_seen}
        self.redeem_refusals = {"mal": 0, "legit": 0}   # redeem_dedup refusals, split by redeemer kind
        self.eclipse_inject_youngest = eclipse_inject_youngest
        self.eclipse_victim = eclipse_victim
        self.healer_from_samples = healer_from_samples
        self.healer_recency = healer_recency
        self._seen_healers: dict = {}      # node_id -> last cycle a malicious node saw it holding a victim-token
        self.eclipse_accumulate_until = eclipse_accumulate_until
        self.victim_contacts = {"mal": 0, "legit": 0}
        self.sample_window = sample_window
        self.gossip_period = 1.0
        self.rng = random.Random(seed)
        self.cycle = 0

        # ---- node assignment
        kinds = [NodeKind.HONEST] * n
        n_mal = int(round(malicious_frac * n))
        mal_ids = self.rng.sample(range(n), n_mal) if n_mal else []
        self.mal_set = set(mal_ids)
        # HYBRID covert role-split: a fraction of adversaries are "harvesters" (reply to honest
        # contacters to grab their victim-tokens for the shared pool); the rest are "injectors"
        # (refuse non-victims -> stay pure, pull pooled tokens, inject).  frac=1 -> all harvest.
        self.covert_harvesters = set(mal_ids[:int(round(self.covert_harvest_frac * n_mal))])
        for i in mal_ids:
            kinds[i] = attack_kind
        self.nodes = [Node(i, kinds[i], self) for i in range(n)]
        self.legit_ids = [i for i in range(n) if i not in self.mal_set]

        # ---- global blacklist (proof flooded instantly; we model perfect dissemination)
        self.blacklist: set = set()
        self.detections: list[Proof] = []
        self.detection_cycle_count: list[int] = []   # cumulative detections per cycle

        # ---- ts disambiguation: each created descriptor gets a unique fractional ts
        #      honest -> integer cycle; we add a tiny per-node offset so distinct
        #      creation events never collide while staying >> gossip_period apart only
        #      when they should.  Honest nodes create once/cycle => ts spaced >= ~1.
        self._ts_counter = 0

        self._bootstrap()

    # ---- bootstrap: random near-regular overlay, then let it converge ----- #
    def _bootstrap(self):
        # Each initial descriptor models a distinct historical self-injection by its
        # creator.  Give every one a globally-unique NEGATIVE INTEGER timestamp so
        # (a) no two share a (creator, ts) -> no spurious cloning flag, and
        # (b) any two from the same creator differ by >= 1 (>= the gossip period)
        #     -> no spurious frequency flag.
        # Per-creator ts counter: the k-th descriptor pointing to creator o gets ts=-k,
        # so staleness (= cycle - ts) starts at a small value k (like a converged overlay),
        # ts are distinct per creator (no clone flag) and spaced >=1 period (no freq flag).
        per_creator: dict = defaultdict(int)
        for node in self.nodes:
            others = self.rng.sample(range(self.n), min(self.l, self.n - 1))
            others = [o for o in others if o != node.id][: self.l]
            for o in others:
                per_creator[o] += 1
                d = Descriptor(creator=o, ts=-per_creator[o],
                               chain=(o, node.id), age=0)
                node.view[o] = d

    def _new_ts(self, node_id: int, fractional: float = 0.0) -> float:
        """Creation timestamp.  Honest descriptors get the integer cycle so two
        from the same creator are always >= 1 (>= period) apart.  `fractional`
        lets an OVERINJECT adversary mint a second one within the same period."""
        return self.cycle + fractional

    # ---- attack activation ------------------------------------------------ #
    def attack_active(self) -> bool:
        return self.cycle >= self.attack_start

    def node_active_kind(self, node: Node) -> NodeKind:
        if node.malicious and not self.attack_active():
            return NodeKind.HONEST           # behave correctly until attack_start
        return node.kind

    # ------------------------------------------------------------------ #
    #  One simulation cycle
    # ------------------------------------------------------------------ #
    def staleness(self, d: Descriptor) -> float:
        """Cyclon staleness, computed from the SIGNED creation timestamp -- the only
        integrity-protected age signal the protocol has (there is no separate signed
        age field, Sec IV-A).  A node future-dating its own ts (within the clock-skew
        tolerance) thus appears younger and is redeemed/evicted later."""
        return self.cycle - d.ts

    def d1_reject(self, ts: float) -> bool:
        """SecureCyclon's clock-skew check (D1, Sec IV-A): reject a descriptor whose
        timestamp is implausibly far in the future relative to the receiver's clock.
        With a large tolerance (the default), future-dating is accepted; tightening it
        below the future-date amount neutralises the age-immortality attack."""
        return ts > self.cycle + self.d1_tol

    def _duplicate_victim_tokens(self):
        """LINEAR prefix-extension cloning of victim-tokens (the user's A->B->C / A->B->C->D
        idea).  For each victim-token a malicious node holds, walk it down a LINEAR ladder of
        colluding adversaries that currently hold NO token for that creator: each step appends
        one adversary id (chain + (Ai,)) and the recipient keeps that copy.  Because every copy
        is a prefix of the next, the (creator,ts) copies are pairwise chains_compatible -> the
        clone check (cache_sample) never fires.  This manufactures many redeemable victim-tokens
        from one, multiplying the coalition's contact rate on the victim -- silently.
        NOTE: strictly linear (no node hands the same prefix to two different next-owners, which
        WOULD diverge -> provable fork).  Copies enter the real sample/clone machinery on gossip."""
        if not self.eclipse_targets or not self.attack_active():
            return
        mal = [self.nodes[i] for i in self.mal_set if i not in self.blacklist]
        # Group every victim-token copy the coalition holds by (creator, ts), and find the
        # LONGEST chain (the tail of the single linear path).  Extend strictly from that tail
        # so every copy ever created for this (creator,ts) lies on ONE path -> all pairwise
        # prefix-compatible -> the clone check never fires, even across cycles/redemptions.
        tails = {}
        for M in mal:
            for d in M.view.values():
                if d.creator in self.eclipse_targets and d.creator not in self.blacklist:
                    k = (d.creator, d.ts)
                    if k not in tails or len(d.chain) > len(tails[k]):
                        tails[k] = d.chain
        for (creator, ts), chain in tails.items():
            cur = chain
            for A in mal:
                if A.id in cur:                       # already on this path (no loop, no branch)
                    continue
                if creator in A.view or creator in A.nonswap:
                    continue                          # A already holds a token for this creator
                if A.view_size() >= self.l:
                    # A's view is full; it isn't bound by l for what it CHOOSES to hold, and it
                    # wants this victim-token -> evict its oldest NON-victim swappable link to make
                    # room (keeps view at l, no overflow, no descriptor loss it cares about).
                    evictable = [d for d in A.view.values() if d.creator not in self.eclipse_targets]
                    if not evictable:
                        continue
                    old = max(evictable, key=lambda x: self.staleness(x))
                    del A.view[old.creator]
                cur = cur + (A.id,)                   # extend the SINGLE longest tail (linear)
                A.view[creator] = Descriptor(creator, ts, cur, 0, swappable=True)

    def _stockpile_burst(self):
        """Each adversary redeems up to `eclipse_burst` DISTINCT hoarded victim-tokens to
        contact the victim this cycle (on top of its one normal initiation).  Every token is a
        distinct (creator,ts), so the redcache samples are distinct -> no clone fork AND no
        redeemed-ts repeat (the dedup table never fires on it).  Throughput is bounded only by
        what the adversary has hoarded (acquisition-bounded on average; bursty after a quiet
        accumulation phase)."""
        if self.cycle < self.eclipse_accumulate_until:
            return                                    # accumulation phase: hoard, do not burst
        for nid in list(self.mal_set):
            if nid in self.blacklist:
                continue
            M = self.nodes[nid]
            if not M.tstockpile:
                continue
            done = 0
            for ts in sorted(M.tstockpile.keys()):    # oldest-ts first
                if done >= self.eclipse_burst:
                    break
                tok = M.tstockpile[ts]
                if tok.creator not in self.eclipse_targets or tok.creator in self.blacklist \
                        or tok.creator == M.id:
                    continue
                del M.tstockpile[ts]
                M.redcache.append(tok.copy_as_sample())   # redeemed token gossiped as a sample
                self._contact_victim(M, self.nodes[tok.creator], tok)
                done += 1

    def _contact_victim(self, M: Node, T: Node, tok: Descriptor):
        """An extra adversary-initiated exchange with victim T, redeeming `tok`.  Runs the real
        sample/clone machinery and BOTH mitigations (volume limit, redeemed-ts dedup)."""
        if T.id in self.blacklist:
            return
        self.victim_contacts["mal"] += 1
        if self.volume_limit and T.contacts_serviced >= self.volume_limit:
            self._left_short(M, [], reason="rate-limit")
            return
        if self.redeem_dedup:
            seen = self._redeem_seen[T.id]
            if tok.ts in seen and self.cycle - seen[tok.ts] <= self.redeem_window:
                self.redeem_refusals["mal" if M.id in self.mal_set else "legit"] += 1
                self._left_short(M, [], reason="redeem-dup")
                return
            seen[tok.ts] = self.cycle
        T.contacts_serviced += 1
        self._exchange_samples(M, T)
        self._exchange_samples(T, M)
        # NOTE: include_self_first=False -- a burst exchange must NOT mint a fresh self-descriptor,
        # or bursting k times/cycle = k mints of the same creator within one gossip period = a D3
        # frequency violation.  A rate-honest adversary advertises itself once/cycle (its normal
        # initiation) and the burst only injects already-owned adversary descriptors.
        i_offer = self._swap_offer(M, T.id, self.s, include_self_first=False,
                                   avoid=set(T.view) | set(T.nonswap))
        p_offer = self._swap_offer(T, M.id, self.s, include_self_first=False,
                                   avoid=set(M.view) | set(M.nonswap))
        i_budget = self._handover_budget(M, role="initiator", offer=i_offer)
        p_budget = self._handover_budget(T, role="partner", offer=p_offer)
        if self.tit_for_tat:
            i_sent, p_sent = self._titfortat(M, T, i_offer, p_offer, i_budget, p_budget)
        else:
            i_sent, p_sent = self._atomic(M, T, i_offer, p_offer, i_budget, p_budget)
        self._integrate(M, received=p_sent, given=i_sent)
        self._integrate(T, received=i_sent, given=p_sent)

    def step(self):
        self.cycle += 1
        # NOTE: no per-cycle age increment -- staleness is derived from ts (see staleness()).
        for node in self.nodes:
            node.ns_redemptions_this_cycle = 0
            node.ns_redeemed_keys_this_cycle.clear()
            node.contacts_serviced = 0
        # sample-derived healer-targeting: target the victim + recently-observed healers
        if self.healer_from_samples and self.eclipse_victim is not None and self.attack_active():
            recent = {h for h, c in self._seen_healers.items()
                      if self.cycle - c <= self.healer_recency and h not in self.blacklist}
            self.eclipse_targets = recent | {self.eclipse_victim}

        if self.eclipse_token_dup:
            self._duplicate_victim_tokens()

        order = self.legit_ids + list(self.mal_set)
        self.rng.shuffle(order)
        for nid in order:
            node = self.nodes[nid]
            if nid in self.blacklist:
                continue
            self._initiate(node)

        # STOCKPILE BURST: each adversary redeems up to `eclipse_burst` distinct hoarded
        # victim-tokens to contact the victim, on top of its one normal initiation.
        if self.eclipse_stockpile and self.attack_active():
            self._stockpile_burst()

        # sparse cloning: after exchanges, malicious nodes may fork chains
        if self.clone_rate > 0 and self.attack_active():
            for nid in list(self.mal_set):
                if nid not in self.blacklist:
                    self._sparse_clone(self.nodes[nid])

        # commit any blacklists whose flooding delay has elapsed
        if self._pending_blacklist:
            due = [o for o, (c, _) in self._pending_blacklist.items() if self.cycle >= c]
            for o in due:
                _, proof = self._pending_blacklist.pop(o)
                self._commit_blacklist(o, proof)

        # bound the covert pool to FRESH tokens: a stale pooled victim-token is useless
        # (it would be the oldest in T's view and evicted at once), and keeping the pool
        # small keeps the load-balance pull cheap.
        if self.eclipse_covert and self.covert_pool:
            for k in [k for k, d in self.covert_pool.items() if self.staleness(d) > 15]:
                del self.covert_pool[k]

        # bound sample memory
        if self.cycle % 10 == 0:
            for node in self.nodes:
                node.prune_samples()
            if self.redeem_dedup:
                for nid, seen in self._redeem_seen.items():
                    for ts in [t for t, c in seen.items() if self.cycle - c > self.redeem_window]:
                        del seen[ts]

        self.detection_cycle_count.append(len(self.detections))

    # ---- sparse cloning (the probabilistic-detection boundary) ------------ #
    def _sparse_clone(self, M: Node):
        """With probability `clone_rate`, M forks the ownership chain of one
        adversary-pointing descriptor it owns (age >= clone_min_age), pushing two
        DIVERGENT copies into two legitimate views.  This duplicates a malicious
        reference (proliferation beyond the m*l budget) at the cost of creating a
        provable fork that the ownership check may catch -- when and only when the
        two divergent copies (or their disseminated samples / redemption-cache
        echoes) co-locate at an honest node.  Targeting OLD descriptors (high
        clone_min_age) lowers that probability (they are redeemed sooner and spread
        less), exactly the Fig-7 evasion."""
        if self.rng.random() >= self.clone_rate:
            return
        # candidate descriptors: adversary-pointing, old enough
        cands = [d for d in M.view.values()
                 if d.creator in self.mal_set and d.creator not in self.blacklist
                 and self.staleness(d) >= self.clone_min_age]
        if not cands:
            return
        d = max(cands, key=lambda x: self.staleness(x))   # oldest qualifying (max evasion)
        targets = [t for t in self.legit_ids if t not in self.blacklist]
        if len(targets) < 2:
            return
        t1, t2 = self.rng.sample(targets, 2)
        self.clone_events += 1
        del M.view[d.creator]                       # M gives up its single copy...
        # ...and emits TWO divergent successors (fork at M)
        for t, in ((t1,), (t2,)):
            fork = Descriptor(d.creator, d.ts, d.chain + (t,), d.age, swappable=True)
            self._inject(self.nodes[t], fork)

    def _inject(self, node: Node, d: Descriptor):
        """Place an owned descriptor into node's view, displacing the oldest
        swappable entry if the view is full.  Runs the detector on it."""
        proof = node.cache_sample(d.creator, d.ts, d.chain)
        if proof is not None:
            self._handle_proof(proof)
        if d.creator == node.id or d.creator in self.blacklist:
            return
        if d.creator in node.view:
            if self.staleness(d) <= self.staleness(node.view[d.creator]):
                node.view[d.creator] = d
            return
        node.nonswap.pop(d.creator, None)
        if node.view_size() >= self.l and node.view:
            oldest = max(node.view.values(), key=lambda x: self.staleness(x))
            del node.view[oldest.creator]
        if node.view_size() < self.l:
            node.view[d.creator] = d

    # ---- partner selection (redeem oldest) -------------------------------- #
    def _select_partner(self, node: Node) -> Optional[tuple]:
        """Return (descriptor, from_nonswap) of the oldest redeemable descriptor,
        removing it from the node's structures (redemption)."""
        # ECLIPSE: a malicious node preferentially redeems a token pointing at a
        # victim, to contact it this cycle and inject adversary descriptors into it.
        # In hoard-then-burst mode it does NOT redeem victim tokens during the
        # accumulation phase (it stockpiles them, redeeming other descriptors), then
        # redeems the whole stockpile once the accumulation window ends -> a burst of
        # many simultaneous contacts on the victim in one cycle.
        if node.malicious and self.eclipse_targets and self.attack_active() \
                and self.cycle >= self.eclipse_accumulate_until:
            vics = [d for d in node.view.values()
                    if d.creator in self.eclipse_targets and d.creator not in self.blacklist]
            if vics:
                best = max(vics, key=lambda x: self.staleness(x))
                del node.view[best.creator]
                node.redcache.append(best.copy_as_sample())
                return best, False
            # COVERT LOAD-BALANCE: no victim-token in view -> pull the freshest one from the
            # shared coalition pool (covert transfer: append self, single chain) so this
            # adversary still contacts T this round instead of sparing it.
            if self.eclipse_covert and self.covert_pool:
                cands = [(k, d) for k, d in self.covert_pool.items()
                         if k[0] in self.eclipse_targets and k[0] not in self.blacklist
                         and node.id not in d.chain]
                if cands:
                    k, d = min(cands, key=lambda kd: self.staleness(kd[1]))   # freshest
                    del self.covert_pool[k]
                    tok = Descriptor(d.creator, d.ts, d.chain + (node.id,), d.age, swappable=False)
                    node.redcache.append(tok.copy_as_sample())
                    return tok, False
        # during the accumulation phase, a malicious node hoards victim tokens:
        # it never redeems them (so they pile up for the burst), redeeming other
        # descriptors as its "oldest" instead.
        hoarding = (node.malicious and self.eclipse_targets and self.attack_active()
                    and self.cycle < self.eclipse_accumulate_until)
        best = None
        best_stale = None
        best_ns = False
        # swappable
        for d in node.view.values():
            if d.creator in self.blacklist:
                continue
            if hoarding and d.creator in self.eclipse_targets:
                continue
            s = self.staleness(d)
            if best is None or s > best_stale:
                best, best_stale, best_ns = d, s, False
        # non-swappable tokens are also redeemable -- but only under §V-A abuse limit 2
        # (a node may redeem at most ONE non-swappable descriptor per cycle).  (Limit 1,
        # at most one redemption per non-swappable descriptor, holds because redemption
        # deletes the stub below.)
        if node.ns_redemptions_this_cycle < 1:
            for d in node.nonswap.values():
                if d.creator in self.blacklist:
                    continue
                s = self.staleness(d)
                if best is None or s > best_stale:
                    best, best_stale, best_ns = d, s, True
        if best is None:
            return None
        if best_ns:
            del node.nonswap[best.creator]
            node.ns_redemptions_this_cycle += 1
            node.ns_redeemed_keys_this_cycle.add((best.creator, best.ts))
        else:
            del node.view[best.creator]
        # push to redemption cache (gossiped as samples thereafter)
        node.redcache.append(best.copy_as_sample())
        return best, best_ns

    # ---- build the ordered list of descriptors to offer for ownership swap - #
    def _swap_offer(self, node: Node, partner_id: int, count: int,
                    include_self_first: bool, avoid: set | None = None) -> list:
        # Each peer has just sent its whole view as samples, so the sender knows
        # which creators the recipient already holds and avoids offering duplicates
        # (otherwise a dedup collision would needlessly cost the recipient a slot).
        kind = self.node_active_kind(node)
        avoid = avoid or set()
        pool = [d for d in node.view.values()
                if d.creator != partner_id and d.creator not in avoid]
        # ECLIPSE hoarding: a malicious node never hands a victim's token to anyone
        # (it keeps them to redeem for contacting the victim), so legitimate nodes
        # cannot acquire tokens to reach the victim and heal its view.  This also makes
        # the node offer ONLY adversary descriptors -> it under-delivers when short of
        # them, which is what trips the honest non-swappable repair.  With
        # eclipse_hoard=False the node still contacts victims but fills its offer to s
        # (adversary-first, legit fallback) -> NO under-delivery, isolating the lever.
        if (node.malicious and self.eclipse_targets and self.attack_active()
                and self.eclipse_hoard):
            pool = [d for d in pool if d.creator not in self.eclipse_targets]
        offer = []
        if include_self_first:
            ts = self._new_ts(node.id)
            # AGE-IMMORTALITY: a malicious node future-dates its fresh self-descriptor by
            # clock_skew, so its staleness (now-ts) is minimal -> it is never the oldest,
            # never redeemed/evicted, maximising residence in honest views.  The constant
            # offset preserves >=1-period spacing between its mints, so the frequency check
            # stays idle; ts is genuinely signed, so nothing is forged.
            if node.malicious and self.attack_active() and self.clock_skew:
                ts = ts + self.clock_skew
            offer.append(node.fresh_self(ts))
        adversary_first = (kind in (NodeKind.BIAS, NodeKind.DROP, NodeKind.CLONE_SPARSE)
                           and partner_id in self.legit_ids)
        # ECLIPSE-STARVE concentration: hoard adversary descriptors for the victim by
        # offering legit-FIRST to non-victim honest partners (don't waste ammo on them),
        # while still reciprocating fully (stays engaged -> keeps acquiring victim-tokens).
        if (self.eclipse_starve and self.eclipse_targets and self.attack_active()
                and node.malicious and partner_id in self.legit_ids
                and partner_id not in self.eclipse_targets):
            adversary_first = False
            mal_pool = [d for d in pool if d.creator in self.mal_set]
            leg_pool = [d for d in pool if d.creator not in self.mal_set]
            self.rng.shuffle(mal_pool); self.rng.shuffle(leg_pool)
            pool = leg_pool + mal_pool        # legit-first to non-victims
        if adversary_first:
            # adversary-descriptors-first: push malicious-pointing descriptors,
            # hoard legitimate ones (offer them only as a last resort).  For BIAS
            # this concentrates malicious links in legit views; for DROP it makes the
            # single tit-for-tat bait a useless/polluting link rather than a healthy one.
            mal_pool = [d for d in pool if d.creator in self.mal_set]
            leg_pool = [d for d in pool if d.creator not in self.mal_set]
            # inject-youngest: send the freshest adversary descriptors to victims so they
            # reside longest before the victim's redeem-oldest evicts them (residence boost
            # without any timestamp manipulation -- uses genuinely fresh descriptors).
            if self.eclipse_inject_youngest and partner_id in self.eclipse_targets:
                mal_pool.sort(key=lambda d: self.staleness(d))   # youngest first
            else:
                self.rng.shuffle(mal_pool)
            self.rng.shuffle(leg_pool)
            ordered = mal_pool + leg_pool
        else:
            self.rng.shuffle(pool)
            ordered = pool
        need = count - len(offer)
        offer.extend(ordered[:max(0, need)])
        return offer

    # ---- the gossip exchange (tit-for-tat or atomic) ---------------------- #
    def _initiate(self, initiator: Node):
        sel = self._select_partner(initiator)
        if sel is None:
            return
        qdesc, _ = sel
        partner = self.nodes[qdesc.creator]
        if partner.id in self.blacklist or partner.id == initiator.id:
            return
        if partner.id in self.eclipse_targets:        # diagnostic: who contacts the victim
            self.victim_contacts["mal" if initiator.malicious else "legit"] += 1

        # MITIGATION (volume limit): the contacted node services at most `volume_limit`
        # contacts per cycle, regardless of which token is presented -- ts-agnostic, so it
        # caps a burst whether it uses one duplicated token or many distinct hoarded ones.
        if self.volume_limit and partner.contacts_serviced >= self.volume_limit:
            self._left_short(initiator, [], reason="rate-limit")
            return
        partner.contacts_serviced += 1

        # MITIGATION: the contacted node refuses a contact that redeems a (creator,ts) of its
        # OWN descriptor it has already serviced.  NOTE: a repeat ts is NOT a reliable
        # duplication signal -- the SECTION V-A non-swappable repair legitimately produces
        # prefix-compatible duplicate copies (a node refills a gap with a stub copy of a
        # descriptor it transferred onward), so the same (creator,ts) is legitimately redeemed
        # by both the longer live copy and the shorter repair stub, often longer-first.  This
        # refusal therefore ALSO drops honest repair-stub redemptions (collateral); we count
        # honest vs adversary refusals so the mitigation's true (compromised) nature is visible.
        if self.redeem_dedup and qdesc.creator == partner.id:
            seen = self._redeem_seen[partner.id]
            if qdesc.ts in seen and self.cycle - seen[qdesc.ts] <= self.redeem_window:
                bucket = "mal" if initiator.id in self.mal_set else "legit"
                self.redeem_refusals[bucket] += 1
                self._left_short(initiator, [], reason="redeem-dup")
                return
            seen[qdesc.ts] = self.cycle

        # churn / silent-drop: does the *contacted* node respond at all?
        if self.rng.random() < self.churn:
            self._left_short(initiator, [], reason="churn")
            return

        ikind = self.node_active_kind(initiator)
        pkind = self.node_active_kind(partner)

        # samples: rest-of-view + redemption cache, exchanged (no ownership), both ways
        self._exchange_samples(initiator, partner)
        self._exchange_samples(partner, initiator)

        # owned descriptors each side is willing to swap (ordered), avoiding
        # creators the recipient already holds (known from the exchanged samples)
        i_avoid = set(partner.view) | set(partner.nonswap)
        p_avoid = set(initiator.view) | set(initiator.nonswap)
        i_offer = self._swap_offer(initiator, partner.id, self.s,
                                   include_self_first=True, avoid=i_avoid)
        p_offer = self._swap_offer(partner, initiator.id, self.s,
                                   include_self_first=False, avoid=p_avoid)

        # how many each side is actually *willing* to hand over before aborting.
        i_budget = self._handover_budget(initiator, role="initiator", offer=i_offer)
        p_budget = self._handover_budget(partner, role="partner", offer=p_offer)
        # FRIEND'S IDEA: a malicious node refuses to REPLY when a non-victim honest peer
        # invites it (it is the partner/responder).  Under tit-for-tat the responder bears
        # zero descriptor-loss risk and the non-response is undetectable (churn-shaped), so
        # the adversary keeps its full adversary-pointing stockpile -- but it then receives
        # only the inviter's fresh-self (1 descriptor), forgoing the rest of the inviter's
        # view, which is the channel through which it would otherwise harvest victim-tokens.
        if (self.eclipse_refuse_invites and partner.malicious and self.attack_active()
                and initiator.id in self.legit_ids
                and initiator.id not in self.eclipse_targets
                and not (self.eclipse_covert and partner.id in self.covert_harvesters)):
            # in covert mode, designated harvesters reply (to grab tokens); injectors stay silent
            p_budget = 0
        # (ECLIPSE-STARVE concentration is applied in _swap_offer: a malicious node offers
        # legit-first to non-victim honest nodes -- reciprocating to stay engaged and keep
        # acquiring victim-tokens -- while hoarding adversary descriptors for the victim.)

        if self.tit_for_tat:
            i_sent, p_sent = self._titfortat(initiator, partner, i_offer, p_offer,
                                             i_budget, p_budget)
        else:
            i_sent, p_sent = self._atomic(initiator, partner, i_offer, p_offer,
                                          i_budget, p_budget)

        # integrate received owned descriptors; repair shortfalls with non-swappable
        self._integrate(initiator, received=p_sent, given=i_sent)
        self._integrate(partner, received=i_sent, given=p_sent)

    def _handover_budget(self, node: Node, role: str, offer: list) -> int:
        """Number of owned descriptors `node` will actually transfer before aborting.
        Honest nodes hand over everything they offered.  A DROP adversary aborts to
        deplete the counterpart while staying churn-shaped."""
        kind = self.node_active_kind(node)
        if kind is NodeKind.DROP:
            attacking = ((role == "partner" and self.drop_role in ("both", "partner")) or
                         (role == "initiator" and self.drop_role in ("both", "initiator")))
            if attacking:
                if not self.tit_for_tat:
                    # atomic exchange: take the initiator's whole batch, return nothing
                    return 0
                # tit-for-tat: the only way to make the counterpart lose a real link
                # is to hand over exactly ONE descriptor (a bait) so the protocol lets
                # the counterpart go one transfer ahead, then abort -- extracting the
                # protocol-max of 1 net descriptor.  As initiator the node still injects
                # its fresh-self first (so it keeps a healthy indegree and is contacted
                # often, maximising how many victim links it can sink), then aborts.
                if self.drop_mode == "empty":
                    return 0
                return 1
        return len(offer)

    def _titfortat(self, I: Node, P: Node, i_offer, p_offer, i_budget, p_budget):
        """One-at-a-time ownership transfer, initiator first.  The initiator can get
        at most one transfer ahead of the partner, so only the initiator risks ending
        one descriptor short -- exactly the paper's guarantee."""
        i_sent, p_sent = [], []
        ii = pi = 0
        # R1: initiator transfers its fresh self first
        if ii < len(i_offer) and ii < i_budget:
            i_sent.append(self._transfer(I, P, i_offer[ii])); ii += 1
        while True:
            # partner responds ONLY after the initiator has gone ahead of it
            # (ii > pi).  This is what guarantees the contacted node never gets
            # ahead, hence never ends a descriptor short -- the paper's "the
            # contacted node runs zero risk of losing a descriptor."
            if ii > pi and pi < len(p_offer) and pi < p_budget:
                p_sent.append(self._transfer(P, I, p_offer[pi])); pi += 1
            else:
                break
            # initiator responds with next
            if ii < len(i_offer) and ii < i_budget:
                i_sent.append(self._transfer(I, P, i_offer[ii])); ii += 1
            else:
                break
        return i_sent, p_sent

    def _atomic(self, I: Node, P: Node, i_offer, p_offer, i_budget, p_budget):
        """Classic single-message swap: initiator hands its whole batch, then the
        partner may abort having taken everything (the link-depletion attack)."""
        i_sent = [self._transfer(I, P, d) for d in i_offer[:i_budget]]
        p_sent = [self._transfer(P, I, d) for d in p_offer[:p_budget]]
        return i_sent, p_sent

    def _transfer(self, src: Node, dst: Node, d: Descriptor) -> Descriptor:
        """Move ownership of d from src to dst (append dst to the chain).  Remove the
        instance from src's view.  CLONE adversary forks here."""
        kind = self.node_active_kind(src)
        if d.creator in src.view and src.view[d.creator] is d:
            del src.view[d.creator]
        if kind is NodeKind.CLONE and dst.id in self.legit_ids and self.rng.random() < 0.5:
            # provable cloning: keep a forked copy and ALSO transfer -> two divergent
            # chains for the same (creator, ts) will collide at some honest node.
            forked = Descriptor(d.creator, d.ts, d.chain + (dst.id,), d.age, swappable=True)
            clone_keep = Descriptor(d.creator, d.ts, d.chain, d.age, swappable=True)
            src.view[d.creator] = clone_keep      # src keeps a copy it will transfer again
            return forked
        return d.transferred_to(dst.id)

    # ---- samples ---------------------------------------------------------- #
    def _exchange_samples(self, src: Node, dst: Node):
        """src sends dst its non-swapped view + redemption cache as samples
        (no ownership transfer).  A DROP/crashed node that doesn't respond sends
        nothing -- but here both parties already engaged, so honest sample
        dissemination happens; the silence is in the *ownership* handover."""
        skind = self.node_active_kind(src)
        # HEALER-FROM-SAMPLES: a malicious receiver learns the victim's healers from the
        # samples it gets -- if the sender's view contains a victim-token, the sender is a
        # current healer.  This is what the protocol's sample dissemination leaks.
        if (self.healer_from_samples and dst.malicious and self.attack_active()
                and self.eclipse_victim is not None and self.eclipse_victim in src.view):
            self._seen_healers[src.id] = self.cycle
        if skind is NodeKind.OVERINJECT and src is dst:
            pass
        samples = []
        for d in src.view.values():
            samples.append((d.creator, d.ts, d.chain))
        for d in src.redcache:
            samples.append((d.creator, d.ts, d.chain))
        # OVERINJECT: mint an extra fresh self within the period (provable frequency)
        if skind is NodeKind.OVERINJECT and dst.id in self.legit_ids:
            extra_ts = self._new_ts(src.id, fractional=self.rng.uniform(0.05, 0.45))
            samples.append((src.id, extra_ts, (src.id,)))
        for (creator, ts, chain) in samples:
            if creator in self.blacklist:
                continue
            if self.d1_reject(ts):
                continue
            proof = dst.cache_sample(creator, ts, chain)
            if proof is not None:
                self._handle_proof(proof)

    # ---- integration + repair --------------------------------------------- #
    def _integrate(self, node: Node, received: list, given: list):
        """Insert received owned descriptors, then repair only a *genuine*
        under-delivery (partner reciprocated fewer transfers than we made) with
        non-swappable copies of the un-reciprocated descriptors (V-A).

        Ordinary dedup gaps (a received descriptor duplicates a node already in the
        view) are left as transient under-fill that the next exchange refills with
        SWAPPABLE links -- they are NOT frozen, so an honest fail-free overlay keeps
        ~0 non-swappable links (matching the paper's pre-attack baseline)."""
        # cross-check received owned descriptors (they are observed too)
        for d in received:
            proof = node.cache_sample(d.creator, d.ts, d.chain)
            if proof is not None:
                self._handle_proof(proof)

        for d in received:
            if d.creator == node.id or d.creator in self.blacklist:
                continue                      # never point at self / at a violator
            if self.d1_reject(d.ts):
                continue                      # clock-skew check rejects future-dated descriptors
            if (self.eclipse_covert and node.malicious and self.attack_active()
                    and d.creator in self.eclipse_targets and d.creator not in self.blacklist):
                # HARVEST + COVERT POOL: any victim-token an adversary receives (as initiator or
                # responder) goes into the SHARED coalition store instead of being lost to the
                # creator-keyed view's dedup-overwrite.  It's a transfer of T's existing token
                # (no mint -> D3-safe), and pulled exactly once below (no copy -> D4-safe).
                self.covert_pool[(d.creator, d.ts)] = Descriptor(d.creator, d.ts, d.chain,
                                                                 d.age, swappable=True)
                continue
            if (self.eclipse_stockpile and node.malicious and self.attack_active()
                    and d.creator in self.eclipse_targets and d.creator not in self.blacklist):
                # HOARD: keep every DISTINCT (creator,ts) victim-token in the unbounded store,
                # never evicted -- the adversary is not bound by the l-capped, creator-keyed view.
                node.tstockpile[d.ts] = Descriptor(d.creator, d.ts, d.chain, d.age, swappable=True)
                continue
            if (self.eclipse_nonswap_tokens and node.malicious and self.attack_active()
                    and d.creator in self.eclipse_targets):
                # adversary parks victim-tokens as NON-SWAPPABLE (still redeemable via
                # _select_partner, never offered in swaps).  Counts against l like any slot.
                if d.creator in node.view:
                    del node.view[d.creator]
                if d.creator in node.nonswap or node.view_size() < self.l:
                    node.nonswap[d.creator] = Descriptor(d.creator, d.ts, d.chain,
                                                         d.age, swappable=False)
                continue
            if d.creator in node.view:
                # dedup: keep the FRESHER copy (lower staleness = later ts)
                if self.staleness(d) <= self.staleness(node.view[d.creator]):
                    node.view[d.creator] = d
                continue
            if d.creator in node.nonswap:
                del node.nonswap[d.creator]    # a fresh swappable link supersedes a stub
                node.view[d.creator] = d
                continue
            if node.view_size() < self.l:
                d.swappable = True
                node.view[d.creator] = d
            # else: view full -> drop (any remaining gap is refilled by the V-A repair below)

        # V-A repair: SecureCyclon keeps views full by refilling EVERY empty slot (from the
        # redeemed-partner turnover, dedup collisions, OR a non-responding/under-delivering
        # partner) with a non-swappable copy of a descriptor we transferred away.  This is the
        # protocol's substitute for Cyclon's view-preserving shuffle, which the ownership/anti-
        # clone model forbids (a node cannot re-advertise its self-descriptor to every contact).
        # Consequence: honest views stay full but legitimately carry some non-swappable stubs.
        for d in reversed(given):
            if node.view_size() >= self.l:
                break
            if d.creator == node.id or d.creator in self.blacklist:
                continue
            if d.creator in node.view or d.creator in node.nonswap:
                continue
            # Keep the descriptor as WE held it: _transfer mutated its chain to end in the
            # recipient, so strip that hop to restore the pre-transfer chain (owner = us).
            stub_chain = d.chain[:-1] if len(d.chain) >= 2 else d.chain
            node.nonswap[d.creator] = Descriptor(d.creator, d.ts, stub_chain, d.age,
                                                 swappable=False)

    def _left_short(self, node: Node, given: list, reason: str):
        """Initiator's partner never responded (churn or silent crash): the
        redeemed slot is simply gone; repair from `given` if any."""
        if node.view_size() < self.l:
            for d in given:
                if node.view_size() >= self.l:
                    break
                if d.creator == node.id or d.creator in node.view or d.creator in node.nonswap:
                    continue
                node.nonswap[d.creator] = Descriptor(d.creator, d.ts, d.chain, d.age,
                                                     swappable=False)

    # ---- blacklisting ----------------------------------------------------- #
    def _handle_proof(self, proof: Proof):
        off = proof.offender
        if off in self.blacklist or off in self._pending_blacklist:
            return
        if self.flood_delay <= 0:
            self._commit_blacklist(off, proof)
        else:
            # model flooding latency: the proof exists but takes flood_delay cycles
            # to propagate before the offender is globally blacklisted
            self._pending_blacklist[off] = (self.cycle + self.flood_delay, proof)

    def _commit_blacklist(self, off: int, proof: Proof):
        self.blacklist.add(off)
        self.detections.append(proof)
        for node in self.nodes:
            node.view.pop(off, None)
            node.nonswap.pop(off, None)

    # ------------------------------------------------------------------ #
    #  Metrics
    # ------------------------------------------------------------------ #
    def metrics(self) -> dict:
        legit = [self.nodes[i] for i in self.legit_ids if i not in self.blacklist]
        # malicious-link fraction in legitimate views (the hub-attack metric)
        mal_links = tot_links = 0
        nonswap_links = swap_links = 0
        view_fill = []
        for node in legit:
            v = list(node.view.values())
            ns = list(node.nonswap.values())
            tot_links += len(v) + len(ns)
            mal_links += sum(1 for d in v + ns if d.creator in self.mal_set)
            nonswap_links += len(ns)
            swap_links += len(v)
            view_fill.append(node.view_size())
        # indegree over legitimate targets
        indeg = defaultdict(int)
        for node in legit:
            for d in list(node.view.values()) + list(node.nonswap.values()):
                indeg[d.creator] += 1
        legit_indeg = [indeg.get(i, 0) for i in self.legit_ids if i not in self.blacklist]
        # live malicious descriptor population (all owned copies across the net)
        # and legitimate descriptors "sunk" into malicious views (link-drop damage)
        mal_pop = 0
        legit_in_mal = 0
        legit_total = 0
        for node in self.nodes:
            for d in node.all_owned():
                if d.creator in self.mal_set:
                    mal_pop += 1
                else:
                    legit_total += 1
                    if node.id in self.mal_set:
                        legit_in_mal += 1
        ttl = max(1, tot_links)
        nlegit = max(1, len(legit))
        return {
            "cycle": self.cycle,
            "mal_link_frac": mal_links / ttl,
            "nonswap_frac": nonswap_links / ttl,
            "swap_frac": swap_links / ttl,
            "avg_view_fill": sum(view_fill) / nlegit,
            "min_view_fill": min(view_fill) if view_fill else 0,
            "avg_legit_indeg": sum(legit_indeg) / nlegit,
            "min_legit_indeg": min(legit_indeg) if legit_indeg else 0,
            "mal_pop": mal_pop,
            "legit_sunk_frac": legit_in_mal / max(1, legit_total),
            "blacklisted": len(self.blacklist),
            "detections": len(self.detections),
        }

    def run(self, cycles: int, record_from: int = 0) -> list[dict]:
        history = []
        for _ in range(cycles):
            self.step()
            if self.cycle >= record_from:
                history.append(self.metrics())
        return history

    # ---- connectivity (legitimate sub-overlay) ---------------------------- #
    def legit_components(self) -> int:
        idset = set(i for i in self.legit_ids if i not in self.blacklist)
        adj = defaultdict(set)
        for i in idset:
            node = self.nodes[i]
            for d in list(node.view.values()) + list(node.nonswap.values()):
                if d.creator in idset:
                    adj[i].add(d.creator)
                    adj[d.creator].add(i)
        seen = set()
        comps = 0
        for s in idset:
            if s in seen:
                continue
            comps += 1
            stack = [s]
            seen.add(s)
            while stack:
                u = stack.pop()
                for w in adj[u]:
                    if w not in seen:
                        seen.add(w)
                        stack.append(w)
        return comps
