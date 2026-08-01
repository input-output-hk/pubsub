//! Model-family integration: M4 — bidirectional relay links from the
//! symmetric edge predicate (every link forms as a reciprocal pair, one
//! publication floods the predicate-connected graph) — plus the 017 selection
//! plane's coordinate points exercised through a real node and event loop.

mod common;

use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use common::{
    accept_all, assert_no_new_deliveries, await_candidates, await_publisher_target_active,
    await_upstream_active, dial_all, node_with_links, ping, trigger_setup, ConnectToExplicit,
};
use pubsub_node::{
    is_valid_edge, is_valid_edge_sym, FanoutStrategy, ForwardToAll, ForwardToRelays,
    InMemoryNetwork, InMemorySubscriptionRegistry, LinkKind, Message, Node, NodeStrategies, PeerId,
    Selection, SubscriptionRegistryControl, TopicId, UnifiedAcceptance,
};

fn topic(s: &str) -> TopicId {
    TopicId::from_str(s).expect("valid topic id")
}

fn peer(s: &str) -> PeerId {
    PeerId::from_str(s).expect("valid peer id")
}

const T: Duration = Duration::from_secs(2);
const BUCKETS: usize = 2; // fed to both seams AND the offline sweep

/// The symmetric-edge pairs (i < j) among `names` at `genesis` — the exact
/// edge set the fleet must realise (the predicate is pure and public).
fn sym_edges(names: &[&str], genesis: u64, t: &TopicId) -> Vec<(usize, usize)> {
    let mut edges = Vec::new();
    for i in 0..names.len() {
        for j in (i + 1)..names.len() {
            if is_valid_edge_sym(genesis, t, &peer(names[i]), &peer(names[j]), BUCKETS) {
                edges.push((i, j));
            }
        }
    }
    edges
}

/// Whether the sym-edge graph over `names` is connected at `genesis`.
fn is_connected(names: &[&str], genesis: u64, t: &TopicId) -> bool {
    let edges = sym_edges(names, genesis, t);
    let mut reached = vec![false; names.len()];
    let mut stack = vec![0usize];
    reached[0] = true;
    while let Some(u) = stack.pop() {
        for &(a, b) in &edges {
            let v = if a == u {
                b
            } else if b == u {
                a
            } else {
                continue;
            };
            if !reached[v] {
                reached[v] = true;
                stack.push(v);
            }
        }
    }
    reached.into_iter().all(|r| r)
}

/// A symmetric gated node: symmetric relay selection AND acceptance at one
/// fed bucket count (so the offline sweep and both seams agree by
/// construction); no publisher links.
fn m4_strategies(id: &str) -> NodeStrategies {
    NodeStrategies::relay_only(
        Arc::new(
            Selection::new(peer(id), [0u8; 32])
                .with_bucket_count(Some(BUCKETS))
                .with_symmetric(true),
        ),
        Arc::new(
            UnifiedAcceptance::new(peer(id))
                .with_gate(Some(BUCKETS))
                .with_symmetric(true),
        ),
    )
    // The symmetric handshake: picks dial under the symmetric vocabulary and
    // one accept decision records each edge in both directions on both ends.
    .with_symmetric_edges(true)
}

// SC-002: 100% link reciprocity and 100% delivery over a predicate-connected
// symmetric graph.
#[tokio::test]
async fn m4_symmetric_edges_form_reciprocal_pairs_and_flood() {
    let names = ["n0", "n1", "n2", "n3", "n4", "n5"];
    let t = topic("t1");

    // Deterministically find a genesis whose symmetric graph is connected —
    // the predicate is public, so the experiment (and this test) can sweep it.
    let genesis = (0..512u64)
        .find(|g| is_connected(&names, *g, &t))
        .expect("some genesis under 512 yields a connected symmetric graph");
    let edges = sym_edges(&names, genesis, &t);

    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let mut nodes: Vec<Node> = Vec::new();
    for id in names {
        nodes.push(
            node_with_links(
                &registry,
                &network,
                id,
                std::slice::from_ref(&t),
                m4_strategies(id),
                Arc::new(ForwardToRelays),
                genesis,
            )
            .await,
        );
    }

    // Candidate convergence on every node (the readiness dial fired against a
    // partial view; the follow-up heartbeat below is the retry pass).
    for (i, node) in nodes.iter().enumerate() {
        let others: Vec<&str> = names
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, n)| *n)
            .collect();
        await_candidates(node, &t, &others, T)
            .await
            .expect("candidates converge");
    }
    for node in &nodes {
        trigger_setup(node);
    }

    // Establishment barrier: every predicate edge is Active from BOTH ends.
    for &(i, j) in &edges {
        await_upstream_active(&nodes[i], nodes[j].id(), &t, T)
            .await
            .unwrap_or_else(|e| panic!("{}→{} upstream: {e}", names[i], names[j]));
        await_upstream_active(&nodes[j], nodes[i].id(), &t, T)
            .await
            .unwrap_or_else(|e| panic!("{}→{} upstream: {e}", names[j], names[i]));
    }

    // Reciprocity: each node's upstream peer set equals its downstream peer
    // set — every link is a pair, none dangles one-way. And the realised edge
    // set is exactly the predicate's.
    for (i, node) in nodes.iter().enumerate() {
        assert_reciprocal_and_exact(node, i, &names, &edges);
    }

    // Full-coverage flood: one publication reaches every node over the mesh.
    // (The delivering peer varies per hop, so poll for the CONTENT, not the
    // origin.)
    let message = ping(t.clone(), 42);
    let Message::Dissemination(signed) = message.clone() else {
        unreachable!()
    };
    nodes[0].publish(signed);
    for node in &nodes[1..] {
        await_content(node, &message, T).await;
    }
}

/// Assert node `i`'s upstream peer set equals its downstream peer set (every
/// link is a reciprocal pair) and equals exactly the predicate's edge set.
fn assert_reciprocal_and_exact(node: &Node, i: usize, names: &[&str], edges: &[(usize, usize)]) {
    let mut up: Vec<String> = node
        .upstream_relays()
        .into_iter()
        .map(|(p, _, _)| p.to_string())
        .collect();
    let mut down: Vec<String> = node
        .downstream_relays()
        .into_iter()
        .map(|(p, _)| p.to_string())
        .collect();
    up.sort();
    up.dedup();
    down.sort();
    down.dedup();
    assert_eq!(
        up, down,
        "{}: upstream and downstream peers must match",
        names[i]
    );

    let mut expected: Vec<String> = edges
        .iter()
        .filter_map(|&(a, b)| {
            if a == i {
                Some(names[b])
            } else if b == i {
                Some(names[a])
            } else {
                None
            }
        })
        .map(|n| peer(n).to_string())
        .collect();
    expected.sort();
    assert_eq!(
        up, expected,
        "{}: realised edges must equal the predicate's",
        names[i]
    );
}

/// Poll `node.received_messages()` until it contains `message` (any origin) or
/// `timeout` elapses.
async fn await_content(node: &Node, message: &Message, timeout: Duration) {
    let start = tokio::time::Instant::now();
    loop {
        if node
            .received_messages()
            .iter()
            .any(|d| &d.message == message)
        {
            return;
        }
        assert!(
            start.elapsed() < timeout,
            "{} did not receive the flooded message within {timeout:?}",
            node.id(),
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

// ---- M5: directed publisher chain, everything-carrying ----------------------

/// An M5-chain node: no relay links at all (pick count 0 on the dial side,
/// serve-none cap on the acceptance side); publisher links to an explicit
/// target list (the directed `k_out` picks); publisher acceptance open.
fn chain_strategies(id: &str, targets: &[(&str, &TopicId)]) -> NodeStrategies {
    NodeStrategies {
        relay_connection: Arc::new(Selection::new(peer(id), [0u8; 32]).with_pick_count(Some(0))),
        relay_acceptance: Arc::new(UnifiedAcceptance::new(peer(id)).with_accept_cap(Some(0))),
        publisher_connection: Some(Arc::new(ConnectToExplicit(
            targets
                .iter()
                .map(|(p, t)| (peer(p), (*t).clone()))
                .collect(),
        ))),
        publisher_acceptance: Some(Arc::new(
            UnifiedAcceptance::new(peer(id)).for_kind(LinkKind::Publisher),
        )),
        symmetric_edges: false,
    }
}

/// Build the a→b→c publisher-link chain under the given fan-out and return
/// the three nodes with all links Active.
async fn chain_fleet(fanout: fn() -> Arc<dyn FanoutStrategy>) -> (Node, Node, Node) {
    let t = topic("t1");
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let a = node_with_links(
        &registry,
        &network,
        "a",
        std::slice::from_ref(&t),
        chain_strategies("a", &[("b", &t)]),
        fanout(),
        0,
    )
    .await;
    let b = node_with_links(
        &registry,
        &network,
        "b",
        std::slice::from_ref(&t),
        chain_strategies("b", &[("c", &t)]),
        fanout(),
        0,
    )
    .await;
    let c = node_with_links(
        &registry,
        &network,
        "c",
        std::slice::from_ref(&t),
        chain_strategies("c", &[]),
        fanout(),
        0,
    )
    .await;
    for node in [&a, &b, &c] {
        trigger_setup(node); // retry pass once every membership has folded
    }
    await_publisher_target_active(&a, b.id(), &t, T)
        .await
        .expect("a→b publisher link");
    await_publisher_target_active(&b, c.id(), &t, T)
        .await
        .expect("b→c publisher link");
    (a, b, c)
}

// SC-003: with all-links fan-out, a foreign publisher's message hops a→b→c
// over standing publisher links only — b relays a's message to c (the
// receive gate is kind-agnostic; only the fan-out distinguishes M5 from M3).
#[tokio::test]
async fn m5_chain_relays_foreign_publisher_over_standing_links() {
    let (a, _b, c) = chain_fleet(|| Arc::new(ForwardToAll)).await;
    let t = topic("t1");

    let message = alias_ping_m5("a", &t, 5);
    let Message::Dissemination(signed) = message.clone() else {
        unreachable!()
    };
    a.publish(signed);

    // c holds no link to a — the ONLY path is the b hop, admitted by
    // any-verified and forwarded by all-links.
    await_content(&c, &message, T).await;
}

// The M3 exclusivity pin: the SAME topology under the default fan-out does
// NOT deliver a's message to c — forward-to-relays never carries a held
// (foreign) message over publisher links, so the sender side alone stops the
// chain at b.
#[tokio::test]
async fn m3_defaults_do_not_relay_over_the_chain() {
    let (a, b, c) = chain_fleet(|| Arc::new(ForwardToRelays)).await;
    let t = topic("t1");

    let message = alias_ping_m5("a", &t, 6);
    let Message::Dissemination(signed) = message.clone() else {
        unreachable!()
    };
    a.publish(signed);

    // b receives it (a owns the a→b link)…
    await_content(&b, &message, T).await;
    // …and it stops there.
    assert_no_new_deliveries(&[&c], Duration::from_millis(80)).await;
}

/// A `Ping(n)` signed with `alias`'s own key (the chain publisher).
fn alias_ping_m5(alias: &str, t: &TopicId, n: u64) -> Message {
    use pubsub_node::{MessagePayload, MockCryptoScheme, PublisherId, SignedMessage, Signer};
    let scheme = MockCryptoScheme::with_seed([0u8; 32]);
    let signer = scheme.signer(scheme.keypair_from_alias(alias).private);
    let plain = pubsub_node::PlainMessage {
        topic: t.clone(),
        publisher_id: PublisherId::new(signer.public_key()),
        parent_hash: None,
        sequence: 0,
        timestamp: pubsub_node::Timestamp::from_millis(0),
        payload: MessagePayload::Ping(n),
    };
    let signature = signer.sign(&plain.signed_bytes());
    Message::Dissemination(SignedMessage { plain, signature })
}

// ---- 017 US1: the selection plane's coordinate points -----------------------
//
// One selection implementation over two knobs, exercised through a real node
// and event loop. The candidate ids are registry members only — not live
// nodes — so dials stay `AwaitingAccept` and the node's upstream SET is
// exactly its selection (the 005 bounded_selection harness pattern).

fn candidate_names(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("c{i:02}")).collect()
}

/// Poll until the node holds exactly `n` upstream entries, or time out.
async fn await_upstream_count(node: &Node, n: usize, timeout: Duration) {
    let start = tokio::time::Instant::now();
    loop {
        let len = node.upstream_relays().len();
        if len == n {
            return;
        }
        assert!(
            start.elapsed() < timeout,
            "upstream count never reached {n} (last saw {len})",
        );
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

/// Build one node running `strategies` on a topic with `candidates`
/// pre-seeded registry members, await `expected_len` upstream entries, and
/// return the selected peer set.
async fn selected_upstreams(
    id: &str,
    candidates: &[String],
    strategies: NodeStrategies,
    expected_len: usize,
) -> BTreeSet<PeerId> {
    let t = topic("t1");
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let network = Arc::new(InMemoryNetwork::new());
    for c in candidates {
        registry
            .set_topics(peer(c), std::iter::once(t.clone()).collect())
            .await
            .expect("seed candidate membership");
    }
    let node = node_with_links(
        &registry,
        &network,
        id,
        std::slice::from_ref(&t),
        strategies,
        Arc::new(ForwardToAll),
        0,
    )
    .await;
    let names: Vec<&str> = candidates.iter().map(String::as_str).collect();
    await_candidates(&node, &t, &names, T)
        .await
        .expect("candidates converge");
    trigger_setup(&node); // the retry heartbeat over the full candidate view
    await_upstream_count(&node, expected_len, T).await;
    node.upstream_relays()
        .into_iter()
        .map(|(p, _, _)| p)
        .collect()
}

/// The predicate-survivor set for `id` over `candidates` at bucket count `b`
/// under genesis 0 — the offline twin of the gate.
fn gate_survivors(id: &str, candidates: &[String], b: usize) -> BTreeSet<PeerId> {
    candidates
        .iter()
        .map(|c| peer(c))
        .filter(|c| is_valid_edge(0, &topic("t1"), &peer(id), c, b))
        .collect()
}

// 017 US1 scenario 4: both knobs absent — every candidate is selected (the
// pre-017 connect-to-all default, preserved as the default behaviour).
#[tokio::test]
async fn plane_origin_selects_every_candidate() {
    let candidates = candidate_names(6);
    let strategies =
        NodeStrategies::relay_only(dial_all(&peer("origin")), accept_all(&peer("origin")));
    let selected = selected_upstreams("origin", &candidates, strategies, 6).await;
    assert_eq!(selected, candidates.iter().map(|c| peer(c)).collect());
}

// 017 US1 scenario 1: pick count only — exactly min(pick count, candidates)
// uniform picks, and the same seed over the same membership reproduces the
// identical set (repeated heartbeats within the epoch re-dial it).
#[tokio::test]
async fn pick_count_only_selects_exactly_that_many() {
    let candidates = candidate_names(6);
    let strategies = |seed: [u8; 32]| {
        NodeStrategies::relay_only(
            Arc::new(Selection::new(peer("picker"), seed).with_pick_count(Some(3))),
            accept_all(&peer("picker")),
        )
    };
    let first = selected_upstreams("picker", &candidates, strategies([7u8; 32]), 3).await;
    assert!(first.iter().all(|p| candidates.contains(&p.to_string())));
    let again = selected_upstreams("picker", &candidates, strategies([7u8; 32]), 3).await;
    assert_eq!(first, again, "same seed, same membership, same picks");
}

// 017 US1 scenario 2: bucket count only — exactly the predicate survivors
// (the previous hash-gated behaviour, preserved as a plane point).
#[tokio::test]
async fn bucket_count_only_selects_the_predicate_survivors() {
    let candidates = candidate_names(8);
    let survivors = gate_survivors("gated", &candidates, BUCKETS);
    let strategies = NodeStrategies::relay_only(
        Arc::new(Selection::new(peer("gated"), [0u8; 32]).with_bucket_count(Some(BUCKETS))),
        accept_all(&peer("gated")),
    );
    let selected = selected_upstreams("gated", &candidates, strategies, survivors.len()).await;
    assert_eq!(selected, survivors);
}

// 017 US1 scenario 3: both knobs — min(pick count, survivors) picks, every
// dialed edge inside the predicate-survivor set.
#[tokio::test]
async fn gated_picks_stay_inside_the_survivor_set() {
    let candidates = candidate_names(8);
    let survivors = gate_survivors("both", &candidates, BUCKETS);
    let expected_len = 2.min(survivors.len());
    let strategies = NodeStrategies::relay_only(
        Arc::new(
            Selection::new(peer("both"), [7u8; 32])
                .with_bucket_count(Some(BUCKETS))
                .with_pick_count(Some(2)),
        ),
        accept_all(&peer("both")),
    );
    let selected = selected_upstreams("both", &candidates, strategies, expected_len).await;
    assert_eq!(selected.len(), expected_len);
    assert!(
        selected.is_subset(&survivors),
        "every dialed edge must pass the predicate",
    );
}

// 017 US1 scenario 5: pick count 0 dials no relay links while the acceptance
// side still serves inbound requests (the push-only M1 shape).
#[tokio::test]
async fn pick_count_zero_dials_none_but_still_serves() {
    let t = topic("t1");
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let network = Arc::new(InMemoryNetwork::new());
    let m1 = node_with_links(
        &registry,
        &network,
        "m1",
        std::slice::from_ref(&t),
        NodeStrategies::relay_only(
            Arc::new(Selection::new(peer("m1"), [0u8; 32]).with_pick_count(Some(0))),
            accept_all(&peer("m1")),
        ),
        Arc::new(ForwardToAll),
        0,
    )
    .await;
    let dialer = node_with_links(
        &registry,
        &network,
        "dialer",
        std::slice::from_ref(&t),
        NodeStrategies::relay_only(
            Arc::new(ConnectToExplicit(vec![(peer("m1"), t.clone())])),
            accept_all(&peer("dialer")),
        ),
        Arc::new(ForwardToAll),
        0,
    )
    .await;
    for node in [&m1, &dialer] {
        await_candidates(
            node,
            &t,
            &[if node.id() == m1.id() { "dialer" } else { "m1" }],
            T,
        )
        .await
        .expect("candidates converge");
        trigger_setup(node);
    }
    // The M1 node's acceptance served the dial…
    await_upstream_active(&dialer, m1.id(), &t, T)
        .await
        .expect("the pick-count-0 node still accepts inbound requests");
    // …while its own dial side selected nothing.
    assert!(
        m1.upstream_relays().is_empty(),
        "pick count 0 must dial no relay links",
    );
}

// 017 US1 scenario 6: the publisher seam is off by construction without its
// pair (inbound publisher requests are dropped) and active with it.
#[tokio::test]
async fn publisher_seam_presence_activates_acceptance() {
    let t = topic("t1");
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let network = Arc::new(InMemoryNetwork::new());
    // "served" carries the publisher pair; "off" does not.
    let served = node_with_links(
        &registry,
        &network,
        "served",
        std::slice::from_ref(&t),
        NodeStrategies {
            relay_connection: Arc::new(
                Selection::new(peer("served"), [0u8; 32]).with_pick_count(Some(0)),
            ),
            relay_acceptance: accept_all(&peer("served")),
            publisher_connection: Some(Arc::new(
                Selection::new(peer("served"), [0u8; 32])
                    .for_kind(LinkKind::Publisher)
                    .with_pick_count(Some(0)),
            )),
            publisher_acceptance: Some(Arc::new(
                UnifiedAcceptance::new(peer("served")).for_kind(LinkKind::Publisher),
            )),
            symmetric_edges: false,
        },
        Arc::new(ForwardToAll),
        0,
    )
    .await;
    let off = node_with_links(
        &registry,
        &network,
        "off",
        std::slice::from_ref(&t),
        NodeStrategies::relay_only(
            Arc::new(Selection::new(peer("off"), [0u8; 32]).with_pick_count(Some(0))),
            accept_all(&peer("off")),
        ),
        Arc::new(ForwardToAll),
        0,
    )
    .await;
    let dialer = node_with_links(
        &registry,
        &network,
        "dialer",
        std::slice::from_ref(&t),
        NodeStrategies {
            relay_connection: Arc::new(
                Selection::new(peer("dialer"), [0u8; 32]).with_pick_count(Some(0)),
            ),
            relay_acceptance: accept_all(&peer("dialer")),
            publisher_connection: Some(Arc::new(ConnectToExplicit(vec![
                (peer("served"), t.clone()),
                (peer("off"), t.clone()),
            ]))),
            publisher_acceptance: Some(Arc::new(
                UnifiedAcceptance::new(peer("dialer")).for_kind(LinkKind::Publisher),
            )),
            symmetric_edges: false,
        },
        Arc::new(ForwardToAll),
        0,
    )
    .await;
    for (node, others) in [
        (&served, ["off", "dialer"]),
        (&off, ["served", "dialer"]),
        (&dialer, ["served", "off"]),
    ] {
        await_candidates(node, &t, &others, T)
            .await
            .expect("candidates converge");
    }
    for node in [&served, &off, &dialer] {
        trigger_setup(node);
    }

    // The seam-carrying node accepts the standing publisher link…
    await_publisher_target_active(&dialer, served.id(), &t, T)
        .await
        .expect("the publisher-configured node accepts the initiation dial");
    // …the seam-less node drops it silently: the dial never activates.
    let still_pending = dialer
        .downstream_publishers()
        .into_iter()
        .any(|(p, _, state)| &p == off.id() && state != pubsub_node::LinkState::Active);
    assert!(
        still_pending,
        "a node without the publisher seam must drop inbound publisher requests",
    );
}

// ---- 017 US2: the real M4 — uniform picks over the symmetric handshake -----
//
// (bucket count absent, pick count = K) + constructed reciprocity (ADR 0034)
// is the formal M4 exactly: own picks all land (the acceptor has no cap), so
// minimum degree ≥ K by construction, and mean degree ≈ 2K (own picks plus
// inbound picks, less the mutual-pick overlap ≈ K²/(N−1), which the fleet
// size keeps inside the 5% bound). 017 SC-003; spec US2 scenarios 1–2.

const T_M4: Duration = Duration::from_secs(10);

/// The deduplicated upstream/downstream peer-name sets of a node.
fn peer_sets(node: &Node) -> (BTreeSet<String>, BTreeSet<String>) {
    let up = node
        .upstream_relays()
        .into_iter()
        .map(|(p, _, _)| p.to_string())
        .collect();
    let down = node
        .downstream_relays()
        .into_iter()
        .map(|(p, _)| p.to_string())
        .collect();
    (up, down)
}

/// Poll until every node's upstream peer set equals its downstream peer set
/// AND meets its expected minimum size — the symmetric fleet's quiescence
/// signal (mid-handshake a dialer holds a pending upstream with no mirror
/// yet) — then hold a short per-node no-change window.
async fn await_symmetric_quiescence(nodes: &[Node], min_len: &[usize], timeout: Duration) {
    let start = tokio::time::Instant::now();
    loop {
        let settled = nodes.iter().zip(min_len).all(|(node, min)| {
            let entries = node.upstream_relays();
            let all_active = entries
                .iter()
                .all(|(_, _, state)| *state == pubsub_node::LinkState::Active);
            let (up, down) = peer_sets(node);
            all_active && up == down && up.len() >= *min
        });
        if settled {
            break;
        }
        assert!(
            start.elapsed() < timeout,
            "the symmetric fleet did not reach reciprocal quiescence within {timeout:?}",
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    for node in nodes {
        common::assert_no_connection_change(node, Duration::from_millis(30)).await;
    }
}

// 017 SC-003 / US2 scenarios 1–2: a symmetric uniform-pick fleet exhibits
// full reciprocity, minimum degree ≥ the pick count, and mean degree within
// 5% of 2× the pick count.
#[tokio::test]
async fn m4_uniform_symmetric_fleet_meets_the_formal_floor() {
    const N: usize = 41;
    const PICKS: usize = 2;
    let t = topic("t1");
    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    let names: Vec<String> = (0..N).map(|i| format!("m{i:02}")).collect();

    // Seed EVERY membership before any node constructs, so each node's
    // readiness dial already sees the full candidate view: a sampled pick set
    // is a function of the candidate SET, and a partial-view readiness dial
    // would union extra picks into the add-only dial model.
    for name in &names {
        registry
            .set_topics(peer(name), std::iter::once(t.clone()).collect())
            .await
            .expect("pre-seed fleet membership");
    }

    let mut nodes: Vec<Node> = Vec::new();
    for (i, name) in names.iter().enumerate() {
        // Per-node sampling seeds (a fleet does not share one seed here; the
        // fleet-shared-seed independence property is the commit-B derivation's
        // and is unit-tested there).
        let mut seed = [0u8; 32];
        seed[0] = u8::try_from(i).expect("fleet fits u8");
        let strategies = NodeStrategies::relay_only(
            Arc::new(
                Selection::new(peer(name), seed)
                    .with_pick_count(Some(PICKS))
                    .with_symmetric(true),
            ),
            accept_all(&peer(name)),
        )
        .with_symmetric_edges(true);
        nodes.push(
            node_with_links(
                &registry,
                &network,
                name,
                std::slice::from_ref(&t),
                strategies,
                Arc::new(ForwardToRelays),
                0,
            )
            .await,
        );
    }

    for (i, node) in nodes.iter().enumerate() {
        let others: Vec<&str> = names
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, n)| n.as_str())
            .collect();
        await_candidates(node, &t, &others, T_M4)
            .await
            .expect("candidates converge");
    }
    for node in &nodes {
        trigger_setup(node);
    }

    // Own picks all land: every node reaches degree ≥ PICKS.
    await_symmetric_quiescence(&nodes, &vec![PICKS; N], T_M4).await;

    let mut degrees: Vec<usize> = Vec::with_capacity(N);
    let mut peer_map: Vec<BTreeSet<String>> = Vec::with_capacity(N);
    for (i, node) in nodes.iter().enumerate() {
        let (up, down) = peer_sets(node);
        assert_eq!(
            up, down,
            "{}: every link must be recorded in both collections",
            names[i],
        );
        assert!(
            node.upstream_relays()
                .iter()
                .all(|(_, _, state)| *state == pubsub_node::LinkState::Active),
            "{}: no half-open links at quiescence",
            names[i],
        );
        degrees.push(up.len());
        peer_map.push(up);
    }

    // Cross-end reciprocity: a link known to one end is known to the other.
    for (i, peers) in peer_map.iter().enumerate() {
        for other in peers {
            let j = names.iter().position(|n| &peer(n).to_string() == other);
            let j = j.expect("linked peer is a fleet member");
            assert!(
                peer_map[j].contains(&peer(&names[i]).to_string()),
                "{} holds {} but not vice versa",
                names[i],
                names[j],
            );
        }
    }

    // The formal floor and the mean (017 SC-003).
    let min_degree = *degrees.iter().min().expect("nonempty fleet");
    assert!(
        min_degree >= PICKS,
        "minimum degree {min_degree} must be ≥ the pick count {PICKS}",
    );
    #[allow(clippy::cast_precision_loss)] // fleet-sized counts, ≪ 2^52
    let (mean, target) = (
        degrees.iter().sum::<usize>() as f64 / N as f64,
        (2 * PICKS) as f64,
    );
    assert!(
        (mean - target).abs() <= 0.05 * target,
        "mean degree {mean:.3} must be within 5% of {target}",
    );
}

// 017 US2 scenario 3: the symmetric flag with a bucket count — the
// unordered-pair predicate gates candidates BEFORE the uniform draw, so
// every realized edge passes it and reciprocity still holds (the
// protocol-track symmetric point remains expressible as coordinates).
#[tokio::test]
async fn m4_symmetric_gate_composes_with_picks() {
    const PICKS: usize = 2;
    let names = ["g0", "g1", "g2", "g3", "g4", "g5", "g6", "g7"];
    let t = topic("t1");

    // A genesis with a non-trivial symmetric edge set at BUCKETS, so the
    // fleet realises at least one gated link (predicate is public, so the
    // survivor sets are computable offline).
    let genesis = (0..512u64)
        .find(|g| sym_edges(&names, *g, &t).len() >= 4)
        .expect("some genesis under 512 yields at least four symmetric edges");
    let edges = sym_edges(&names, genesis, &t);
    let neighbors = |i: usize| -> BTreeSet<String> {
        edges
            .iter()
            .filter_map(|&(a, b)| {
                if a == i {
                    Some(peer(names[b]).to_string())
                } else if b == i {
                    Some(peer(names[a]).to_string())
                } else {
                    None
                }
            })
            .collect()
    };

    let network = Arc::new(InMemoryNetwork::new());
    let registry = Arc::new(InMemorySubscriptionRegistry::new());
    // Full candidate view before any readiness dial (see the fleet test).
    for name in names {
        registry
            .set_topics(peer(name), std::iter::once(t.clone()).collect())
            .await
            .expect("pre-seed fleet membership");
    }
    let mut nodes: Vec<Node> = Vec::new();
    for (i, id) in names.iter().enumerate() {
        let mut seed = [7u8; 32];
        seed[0] = u8::try_from(i).expect("fleet fits u8");
        let strategies = NodeStrategies::relay_only(
            Arc::new(
                Selection::new(peer(id), seed)
                    .with_bucket_count(Some(BUCKETS))
                    .with_pick_count(Some(PICKS))
                    .with_symmetric(true),
            ),
            Arc::new(
                UnifiedAcceptance::new(peer(id))
                    .with_gate(Some(BUCKETS))
                    .with_symmetric(true),
            ),
        )
        .with_symmetric_edges(true);
        nodes.push(
            node_with_links(
                &registry,
                &network,
                id,
                std::slice::from_ref(&t),
                strategies,
                Arc::new(ForwardToRelays),
                genesis,
            )
            .await,
        );
    }

    for (i, node) in nodes.iter().enumerate() {
        let others: Vec<&str> = names
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, n)| *n)
            .collect();
        await_candidates(node, &t, &others, T_M4)
            .await
            .expect("candidates converge");
    }
    for node in &nodes {
        trigger_setup(node);
    }

    // Each node's own picks — min(PICKS, survivors) — all land.
    let min_len: Vec<usize> = (0..names.len())
        .map(|i| PICKS.min(neighbors(i).len()))
        .collect();
    await_symmetric_quiescence(&nodes, &min_len, T_M4).await;

    for (i, node) in nodes.iter().enumerate() {
        let (up, down) = peer_sets(node);
        assert_eq!(up, down, "{}: reciprocity holds under the gate", names[i]);
        assert!(
            up.is_subset(&neighbors(i)),
            "{}: every realized edge must pass the unordered-pair predicate",
            names[i],
        );
    }
    assert!(
        nodes
            .iter()
            .map(|n| n.upstream_relays().len())
            .sum::<usize>()
            > 0,
        "the fixture genesis must realise at least one gated link",
    );
}
