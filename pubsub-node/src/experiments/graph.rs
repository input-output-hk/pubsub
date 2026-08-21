//! Realised-graph analytics: propagation-digraph extraction (behind the
//! [`DisseminationModel`] dispatch), iterative Kosaraju strongly-connected
//! components, condensation, the good-topology verdict, and topology-shape
//! statistics.
//!
//! Good-topology ⟺ every up-honest publisher reaches every up-honest node ⟺
//! the extracted up-honest propagation digraph is **one SCC** — evaluated
//! with no dissemination drain. Min publisher-coverage falls out of the same
//! pass: a publisher in a condensation sink component reaches only that
//! component, so the smallest sink bounds coverage from below.
// 016-FR-019…FR-022; research R5/R8; data-model §4.

use std::collections::BTreeSet;
use std::str::FromStr;

use crate::connection_state::LinkState;
use crate::peer::PeerId;

use super::population::Population;

/// The dissemination model an experiment runs under: the dispatch that owns
/// propagation-graph extraction, the per-publisher seed-set rule, and the
/// goodness criterion. The dispatch shape is what the experiment program's
/// model-family stage extends (ADR 0041).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisseminationModel {
    /// Push-only dissemination — M5's `k_in` = 0 boundary named: no relay
    /// mesh, every message pushed over the publisher (`k_out`) links.
    /// Shares M5's extraction and seed rules exactly; its own name keeps
    /// the boundary-row configurations self-describing (ADR 0041).
    M1,
    /// Uniform-relay dissemination: propagation edges are the relay
    /// `downstream` records between up-honest peers; the seed set is the
    /// publisher alone; good ⟺ one strongly connected component.
    M2,
    /// Relay dissemination plus standing initiation links: propagation
    /// edges stay relay-only (initiation links never relay), but a
    /// publisher's message starts from its **seed set** — itself plus its
    /// `Active` publisher-link targets — so goodness is the seed-aware
    /// criterion ([`seeded_goodness`]), not bare one-SCC (ADR 0041).
    M3,
    /// Bidirectional-relay dissemination (the symmetric handshake with a
    /// pick count). Shares M2's extraction and seed rules exactly: every
    /// relay link is mirrored, so the extracted digraph is symmetric by
    /// construction and the one-SCC criterion applies unchanged. Its own
    /// name keeps configurations self-describing (ADR 0041 ties it to the
    /// symmetric switch).
    M4,
    /// Directed k-in/k-out gossip: every held message flows over **both**
    /// link kinds (`forward-to-all`), so propagation edges are the union of
    /// relay and `Active` publisher downstream records; the seed set is the
    /// publisher alone; good ⟺ one SCC of the union digraph (ADR 0041).
    M5,
}

/// The error returned when a configuration string names no known
/// dissemination model.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("unknown dissemination model '{0}' (expected one of: m1, m2, m3, m4, m5)")]
pub struct UnknownDisseminationModel(pub String);

impl DisseminationModel {
    /// The canonical, lower-case configuration name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::M1 => "m1",
            Self::M2 => "m2",
            Self::M3 => "m3",
            Self::M4 => "m4",
            Self::M5 => "m5",
        }
    }

    /// The per-publisher seed set: the peers assumed to hold the message at
    /// wave 0 — the dispatch's parameterisation point. Every model but M3
    /// seeds the publisher alone; M3 adds the publisher's `Active`
    /// publisher-link targets on the population's topic (initiation links
    /// carry the owner's own publications). The list is the raw rule —
    /// entries that are not digraph vertices (adversarial or down targets)
    /// contribute nothing to propagation, and consumers intersect with the
    /// vertex set.
    #[must_use]
    pub fn publisher_seeds(self, population: &Population, publisher: &PeerId) -> Vec<PeerId> {
        match self {
            Self::M1 | Self::M2 | Self::M4 | Self::M5 => vec![publisher.clone()],
            Self::M3 => {
                let mut seeds = vec![publisher.clone()];
                if let Some(participant) = population.participant(publisher) {
                    let topic = population.topic();
                    for (target, edge_topic, state) in participant.publisher_downstream() {
                        if state == LinkState::Active && &edge_topic == topic {
                            seeds.push(target);
                        }
                    }
                }
                seeds
            }
        }
    }

    /// Extract the propagation digraph from the population's node states.
    ///
    /// Vertices are the honest participants of the requested phase —
    /// [`ChurnPhase::PostChurn`] takes up-honest only (the primary graph);
    /// [`ChurnPhase::PreChurn`] includes down honest nodes (the formed-
    /// topology diagnostic). Adversarial participants are never vertices:
    /// under every model the silent relay contributes nothing to
    /// propagation. An edge `u → v` exists iff `v` is in `u`'s fan-out
    /// target set — for M2/M3/M4, `u`'s relay `downstream` records
    /// restricted to the vertex set (initiation links never relay); for
    /// M5/M1 (`forward-to-all`), the union of `u`'s relay and `Active`
    /// publisher `downstream` records, deduplicated per pair.
    #[must_use]
    pub fn extract(self, population: &Population, phase: ChurnPhase) -> PropagationDigraph {
        let in_scope = |participant: &super::population::Participant| match phase {
            ChurnPhase::PreChurn => {
                participant.class() == super::population::ParticipantClass::Honest
            }
            ChurnPhase::PostChurn => participant.is_up_honest(),
        };
        let publisher_edges_propagate = matches!(self, Self::M1 | Self::M5);
        // Population iteration is peer-id-sorted, so the vertex list is
        // sorted by construction.
        let vertices: Vec<PeerId> = population
            .participants()
            .filter(|(_, participant)| in_scope(participant))
            .map(|(id, _)| id.clone())
            .collect();
        let vertex_set: BTreeSet<&PeerId> = vertices.iter().collect();
        let topic = population.topic();
        let mut edges = Vec::new();
        for from in &vertices {
            let participant = population.participant(from).expect("vertex in population");
            for (to, edge_topic) in participant.downstream() {
                if &edge_topic == topic && vertex_set.contains(&to) {
                    edges.push((from.clone(), to));
                }
            }
            if publisher_edges_propagate {
                for (to, edge_topic, state) in participant.publisher_downstream() {
                    if state == LinkState::Active
                        && &edge_topic == topic
                        && vertex_set.contains(&to)
                    {
                        edges.push((from.clone(), to));
                    }
                }
            }
        }
        PropagationDigraph::from_vertices_and_edges(vertices, &edges)
    }

    /// Extract and analyse in one call: the digraph, its goodness verdict
    /// (seed-aware under M3), and its topology shape.
    #[must_use]
    pub fn analyze(self, population: &Population, phase: ChurnPhase) -> GraphAnalysis {
        let digraph = self.extract(population, phase);
        let verdict = match self {
            Self::M1 | Self::M2 | Self::M4 | Self::M5 => goodness(&digraph),
            Self::M3 => {
                let seeds: Vec<Vec<PeerId>> = digraph
                    .vertices()
                    .iter()
                    .map(|id| self.publisher_seeds(population, id))
                    .collect();
                seeded_goodness(&digraph, &seeds)
            }
        };
        let shape = topology_shape(&digraph);
        GraphAnalysis {
            digraph,
            verdict,
            shape,
        }
    }
}

impl FromStr for DisseminationModel {
    type Err = UnknownDisseminationModel;

    /// Parse a model name case-insensitively.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "m1" => Ok(Self::M1),
            "m2" => Ok(Self::M2),
            "m3" => Ok(Self::M3),
            "m4" => Ok(Self::M4),
            "m5" => Ok(Self::M5),
            _ => Err(UnknownDisseminationModel(s.to_string())),
        }
    }
}

/// Which vertex set a graph pass runs over: the post-churn
/// up-honest graph is the primary; the pre-churn graph (down honest nodes
/// included) is the paired formation diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChurnPhase {
    /// The formed topology: all honest participants, down or not.
    PreChurn,
    /// The surviving topology: up-honest participants only.
    PostChurn,
}

/// An extracted propagation digraph: sorted vertices, index-keyed sorted
/// adjacency (deterministic iteration by construction).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropagationDigraph {
    vertices: Vec<PeerId>,
    adjacency: Vec<Vec<usize>>,
}

impl PropagationDigraph {
    /// Assemble a digraph from a sorted, deduplicated vertex list and edge
    /// pairs over it (edges to non-vertices are dropped by construction in
    /// the extraction; this constructor asserts vertex membership).
    fn from_vertices_and_edges(vertices: Vec<PeerId>, edges: &[(PeerId, PeerId)]) -> Self {
        debug_assert!(vertices.windows(2).all(|pair| pair[0] < pair[1]));
        let mut adjacency = vec![Vec::new(); vertices.len()];
        for (from, to) in edges {
            let from = vertices
                .binary_search(from)
                .expect("edge endpoints are vertices");
            let to = vertices
                .binary_search(to)
                .expect("edge endpoints are vertices");
            adjacency[from].push(to);
        }
        for targets in &mut adjacency {
            targets.sort_unstable();
            targets.dedup();
        }
        Self {
            vertices,
            adjacency,
        }
    }

    /// Test-support constructor: `n` scripted vertices with index edges.
    #[cfg(test)]
    pub(crate) fn from_indexed_edges(n: usize, edges: &[(usize, usize)]) -> Self {
        let vertices: Vec<PeerId> = (0..n).map(super::scripted::peer).collect();
        let edges: Vec<(PeerId, PeerId)> = edges
            .iter()
            .map(|(from, to)| (vertices[*from].clone(), vertices[*to].clone()))
            .collect();
        Self::from_vertices_and_edges(vertices, &edges)
    }

    /// Number of vertices.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Number of directed edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.adjacency.iter().map(Vec::len).sum()
    }

    /// The sorted vertex list.
    #[must_use]
    pub fn vertices(&self) -> &[PeerId] {
        &self.vertices
    }

    /// The out-degree of `peer`, if it is a vertex.
    #[must_use]
    pub fn out_degree(&self, peer: &PeerId) -> Option<usize> {
        let index = self.vertices.binary_search(peer).ok()?;
        Some(self.adjacency[index].len())
    }

    /// The in-degree of `peer`, if it is a vertex.
    ///
    /// This scans the whole adjacency structure — O(E) per call. To read
    /// every node's degrees, use [`PropagationDigraph::degree_vectors`]
    /// (one O(V+E) pass) instead of calling this per node.
    #[must_use]
    pub fn in_degree(&self, peer: &PeerId) -> Option<usize> {
        let index = self.vertices.binary_search(peer).ok()?;
        Some(
            self.adjacency
                .iter()
                .filter(|targets| targets.binary_search(&index).is_ok())
                .count(),
        )
    }

    /// Every vertex's (in-degree, out-degree), indexed like
    /// [`PropagationDigraph::vertices`] — computed in one O(V+E) pass, for
    /// callers that need all nodes' degrees.
    #[must_use]
    pub fn degree_vectors(&self) -> (Vec<usize>, Vec<usize>) {
        let mut out_degrees = Vec::with_capacity(self.vertices.len());
        let mut in_degrees = vec![0usize; self.vertices.len()];
        for targets in &self.adjacency {
            out_degrees.push(targets.len());
            for &to in targets {
                in_degrees[to] += 1;
            }
        }
        (in_degrees, out_degrees)
    }

    /// Every vertex reachable from `seed` (including `seed` itself when it is
    /// a vertex) — iterative traversal, the reachability half of the
    /// two-instrument cross-check.
    #[must_use]
    pub fn reachable_from(&self, seed: &PeerId) -> BTreeSet<PeerId> {
        let Ok(seed) = self.vertices.binary_search(seed) else {
            return BTreeSet::new();
        };
        let mut visited = vec![false; self.vertices.len()];
        visited[seed] = true;
        let mut stack = vec![seed];
        while let Some(vertex) = stack.pop() {
            for &target in &self.adjacency[vertex] {
                if !visited[target] {
                    visited[target] = true;
                    stack.push(target);
                }
            }
        }
        visited
            .into_iter()
            .enumerate()
            .filter(|(_, reached)| *reached)
            .map(|(index, _)| self.vertices[index].clone())
            .collect()
    }

    /// The condensation of this digraph: Kosaraju SCCs (iterative, explicit
    /// stack — recursion overflows at the target population sizes), component
    /// sizes, the component DAG, and its source/sink component sets.
    #[must_use]
    pub fn condensation(&self) -> Condensation {
        const UNASSIGNED: usize = usize::MAX;
        let n = self.vertices.len();

        // Kosaraju pass 1: DFS finishing order on the graph, iterative — the
        // stack holds (vertex, next-child index); a vertex whose children are
        // exhausted is finished.
        let mut visited = vec![false; n];
        let mut finish_order = Vec::with_capacity(n);
        let mut stack: Vec<(usize, usize)> = Vec::new();
        for start in 0..n {
            if visited[start] {
                continue;
            }
            visited[start] = true;
            stack.push((start, 0));
            while let Some((vertex, child)) = stack.pop() {
                if let Some(&target) = self.adjacency[vertex].get(child) {
                    stack.push((vertex, child + 1));
                    if !visited[target] {
                        visited[target] = true;
                        stack.push((target, 0));
                    }
                } else {
                    finish_order.push(vertex);
                }
            }
        }

        // Kosaraju pass 2: DFS on the transpose in reverse finishing order;
        // each root starts a component.
        let mut reversed = vec![Vec::new(); n];
        for (from, targets) in self.adjacency.iter().enumerate() {
            for &to in targets {
                reversed[to].push(from);
            }
        }
        let mut component_of = vec![UNASSIGNED; n];
        let mut component_sizes = Vec::new();
        let mut dfs = Vec::new();
        for &root in finish_order.iter().rev() {
            if component_of[root] != UNASSIGNED {
                continue;
            }
            let component = component_sizes.len();
            component_sizes.push(0usize);
            component_of[root] = component;
            dfs.push(root);
            while let Some(vertex) = dfs.pop() {
                component_sizes[component] += 1;
                for &source in &reversed[vertex] {
                    if component_of[source] == UNASSIGNED {
                        component_of[source] = component;
                        dfs.push(source);
                    }
                }
            }
        }

        // Condensation DAG: cross-component edges, deduplicated; sources and
        // sinks from the edge set.
        let mut edges = BTreeSet::new();
        for (from, targets) in self.adjacency.iter().enumerate() {
            for &to in targets {
                if component_of[from] != component_of[to] {
                    edges.insert((component_of[from], component_of[to]));
                }
            }
        }
        let mut sources: BTreeSet<usize> = (0..component_sizes.len()).collect();
        let mut sinks: BTreeSet<usize> = (0..component_sizes.len()).collect();
        for (from, to) in &edges {
            sinks.remove(from);
            sources.remove(to);
        }

        Condensation {
            component_of,
            component_sizes,
            edges,
            sources,
            sinks,
        }
    }
}

/// The condensation of a propagation digraph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Condensation {
    /// Component id per vertex index.
    pub component_of: Vec<usize>,
    /// Vertex count per component id.
    pub component_sizes: Vec<usize>,
    /// The component DAG's edges (deduplicated, sorted).
    pub edges: BTreeSet<(usize, usize)>,
    /// Components with no incoming DAG edge.
    pub sources: BTreeSet<usize>,
    /// Components with no outgoing DAG edge.
    pub sinks: BTreeSet<usize>,
}

/// The good-topology verdict and its graded refinements.
#[derive(Clone, Debug, PartialEq)]
pub struct GoodnessVerdict {
    /// Every up-honest publisher reaches every up-honest node — one SCC
    /// under the publisher-alone seed rule ([`goodness`]); under M3's seed
    /// sets, every publisher's seed closure covers the whole graph
    /// ([`seeded_goodness`]).
    pub good: bool,
    /// Worst-case coverage over all up-honest publishers: under the
    /// publisher-alone rule, (smallest condensation-sink component − 1) /
    /// (up-honest − 1); under seed sets, the worst per-publisher seed
    /// closure fraction.
    pub min_publisher_coverage: f64,
    /// Number of strongly connected components.
    pub sccs: u64,
    /// Size of the largest strongly connected component.
    pub largest_scc: u64,
    /// **Deaf** vertices: not reachable *from* the largest component — the
    /// giant's messages never arrive (the in-defect, "eclipsed" in the
    /// formal severity tables). A vertex disconnected in both directions
    /// counts in both classes, so `deaf + mute` can exceed the stranded
    /// count `vertices − largest_scc`; the formal classifier instead cuts
    /// those vertices into a disjoint third class, so joining these
    /// columns onto its tables double-counts unless the overlap
    /// (`deaf + mute − stranded`) is subtracted first. Both counts are
    /// relative to the raw digraph under every model — M3's seed rescue is
    /// deliberately not reflected here (its goodness criterion is; see
    /// [`seeded_goodness`]).
    pub deaf: u64,
    /// **Mute** vertices: cannot reach the largest component — their
    /// messages never arrive at the giant (the out-defect; a muted
    /// publisher is the canonical case).
    pub mute: u64,
}

/// Degree/sink statistics over the extracted digraph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopologyShape {
    /// Vertex count per in-degree (index = degree).
    pub in_degree_hist: Vec<u64>,
    /// Vertex count per out-degree (index = degree).
    pub out_degree_hist: Vec<u64>,
    /// Vertices with out-degree 0 (honest sinks).
    pub sinks: u64,
}

/// One graph pass's full output: the digraph and both derived summaries.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphAnalysis {
    /// The extracted propagation digraph.
    pub digraph: PropagationDigraph,
    /// The goodness verdict.
    pub verdict: GoodnessVerdict,
    /// Degree/sink statistics.
    pub shape: TopologyShape,
}

/// Classify the stranded vertices relative to the giant component: deaf =
/// in components the largest component does not reach; mute = in components
/// that do not reach it (both directions counted independently — an
/// isolated vertex is both). The classes are the formal severity tables'
/// two stranding directions, with one convention difference: the formal
/// classifier is three-way disjoint (deaf, mute, and a separate cut class
/// for both-disconnected vertices), so its columns exclude exactly what is
/// counted here in both — the disjoint counts stay recoverable as
/// overlap = deaf + mute − stranded. Computed on the condensation DAG in
/// one forward and one backward walk from the giant (ties on size break to
/// the first component in Kosaraju order — deterministic; at the measured
/// operating shapes the giant dominates and ties do not arise).
fn classify_strandings(condensation: &Condensation) -> (u64, u64) {
    let sccs = condensation.component_sizes.len();
    if sccs <= 1 {
        return (0, 0);
    }
    let mut giant = 0usize;
    for (component, &size) in condensation.component_sizes.iter().enumerate() {
        if size > condensation.component_sizes[giant] {
            giant = component;
        }
    }
    let mut forward = vec![Vec::new(); sccs];
    let mut backward = vec![Vec::new(); sccs];
    for &(from, to) in &condensation.edges {
        forward[from].push(to);
        backward[to].push(from);
    }
    let reach = |adjacency: &[Vec<usize>]| {
        let mut visited = vec![false; sccs];
        visited[giant] = true;
        let mut stack = vec![giant];
        while let Some(component) = stack.pop() {
            for &next in &adjacency[component] {
                if !visited[next] {
                    visited[next] = true;
                    stack.push(next);
                }
            }
        }
        visited
    };
    let hears_giant = reach(&forward);
    let heard_by_giant = reach(&backward);
    let mut deaf = 0u64;
    let mut mute = 0u64;
    for component in 0..sccs {
        if !hears_giant[component] {
            deaf += condensation.component_sizes[component] as u64;
        }
        if !heard_by_giant[component] {
            mute += condensation.component_sizes[component] as u64;
        }
    }
    (deaf, mute)
}

/// The goodness verdict from one condensation pass.
///
/// Degenerate inputs (which configuration validation excludes from real
/// runs): a single-vertex graph is one SCC and vacuously fully covered
/// (min coverage 1.0); an empty graph is not good and has zero coverage.
#[must_use]
pub fn goodness(digraph: &PropagationDigraph) -> GoodnessVerdict {
    let condensation = digraph.condensation();
    let sccs = condensation.component_sizes.len();
    let largest_scc = condensation
        .component_sizes
        .iter()
        .copied()
        .max()
        .unwrap_or(0);
    let (deaf, mute) = classify_strandings(&condensation);
    let up_honest = digraph.vertex_count();
    let smallest_sink = condensation
        .sinks
        .iter()
        .map(|&component| condensation.component_sizes[component])
        .min();
    #[allow(clippy::cast_precision_loss)] // population sizes ≪ 2^52
    let min_publisher_coverage = match (up_honest, smallest_sink) {
        (0, _) | (_, None) => 0.0,
        (1, Some(_)) => 1.0,
        (_, Some(sink)) => (sink - 1) as f64 / (up_honest - 1) as f64,
    };
    GoodnessVerdict {
        good: sccs == 1,
        min_publisher_coverage,
        sccs: sccs as u64,
        largest_scc: largest_scc as u64,
        deaf,
        mute,
    }
}

/// The seed-aware goodness verdict — the M3 dispatch (ADR 0041).
///
/// A publisher's message spreads from its **seed set** over the digraph's
/// edges, so the one-SCC criterion generalises: a seed set's downward
/// closure is the whole graph exactly when every **source component** of
/// the condensation contains a seed (a source has no incoming edges, so
/// nothing outside it can reach it) — checked here as closure size =
/// vertex count, per publisher. `good` ⟺ every vertex, as publisher,
/// covers the whole graph; `min_publisher_coverage` is the worst
/// per-publisher closure fraction over eligible receivers. This is the
/// formal M3 study's exact every-publisher check, computed on the
/// condensation instead of the raw graph.
///
/// `seeds` is vertex-aligned with [`PropagationDigraph::vertices`]; seed
/// entries that are not vertices (adversarial or down targets) contribute
/// nothing, matching honest-targets-only spreading. Each vertex's seed
/// list is expected to contain the vertex itself.
#[must_use]
pub fn seeded_goodness(digraph: &PropagationDigraph, seeds: &[Vec<PeerId>]) -> GoodnessVerdict {
    const UNSTAMPED: usize = usize::MAX;
    debug_assert_eq!(seeds.len(), digraph.vertex_count());
    let condensation = digraph.condensation();
    let sccs = condensation.component_sizes.len();
    let largest_scc = condensation
        .component_sizes
        .iter()
        .copied()
        .max()
        .unwrap_or(0);
    let (deaf, mute) = classify_strandings(&condensation);
    let up_honest = digraph.vertex_count();
    // One component (or an empty graph): seeds cannot change the verdict.
    if sccs <= 1 {
        return GoodnessVerdict {
            good: sccs == 1,
            min_publisher_coverage: if up_honest == 0 { 0.0 } else { 1.0 },
            sccs: sccs as u64,
            largest_scc: largest_scc as u64,
            deaf,
            mute,
        };
    }

    // Component-DAG adjacency for the per-publisher closure walks.
    let mut dag = vec![Vec::new(); sccs];
    for &(from, to) in &condensation.edges {
        dag[from].push(to);
    }

    let mut stamp = vec![UNSTAMPED; sccs];
    let mut stack: Vec<usize> = Vec::new();
    let mut good = true;
    let mut min_publisher_coverage = 1.0f64;
    for (vertex, seed_peers) in seeds.iter().enumerate() {
        // Seed components: the closure walk's roots, stamped per publisher
        // so the scratch vectors are reused without clearing.
        for seed in seed_peers {
            if let Ok(index) = digraph.vertices.binary_search(seed) {
                let component = condensation.component_of[index];
                if stamp[component] != vertex {
                    stamp[component] = vertex;
                    stack.push(component);
                }
            }
        }
        let mut closure_size = 0usize;
        while let Some(component) = stack.pop() {
            closure_size += condensation.component_sizes[component];
            for &child in &dag[component] {
                if stamp[child] != vertex {
                    stamp[child] = vertex;
                    stack.push(child);
                }
            }
        }
        good &= closure_size == up_honest;
        #[allow(clippy::cast_precision_loss)] // population sizes ≪ 2^52
        let coverage = if up_honest <= 1 {
            1.0
        } else {
            (closure_size.saturating_sub(1)) as f64 / (up_honest - 1) as f64
        };
        min_publisher_coverage = min_publisher_coverage.min(coverage);
    }

    GoodnessVerdict {
        good,
        min_publisher_coverage,
        sccs: sccs as u64,
        largest_scc: largest_scc as u64,
        deaf,
        mute,
    }
}

/// Degree/sink statistics over the digraph.
#[must_use]
pub fn topology_shape(digraph: &PropagationDigraph) -> TopologyShape {
    let (in_degrees, out_degrees) = digraph.degree_vectors();
    TopologyShape {
        in_degree_hist: degree_histogram(&in_degrees),
        out_degree_hist: degree_histogram(&out_degrees),
        sinks: out_degrees.iter().filter(|&&degree| degree == 0).count() as u64,
    }
}

/// Standing links held per up-honest node, in vertex order.
///
/// The propagation digraph deliberately omits links that carry no
/// dissemination traffic — under M3 the initiation links are seed edges and
/// never relay — so its degrees understate what a node actually holds open.
/// This counts the connections instead: every distinct `(peer, kind)` a node
/// has an established link with, in either direction and regardless of the
/// counterparty's class, since an adversary still occupies a connection slot.
///
/// A symmetric relay link registers on both the upstream and downstream side
/// for the same peer and kind, and is counted once — which is what makes the
/// figure comparable across the family: it is the chooser-plus-acceptor total,
/// twice the nominal budget under protocol-compliant opening.
#[must_use]
pub fn standing_degrees(population: &Population, phase: ChurnPhase) -> Vec<usize> {
    let topic = population.topic();
    let in_scope = |participant: &super::population::Participant| match phase {
        ChurnPhase::PreChurn => participant.class() == super::population::ParticipantClass::Honest,
        ChurnPhase::PostChurn => participant.is_up_honest(),
    };
    population
        .participants()
        .filter(|(_, participant)| in_scope(participant))
        .map(|(_, participant)| {
            // (peer, is_publisher_kind) — dedupes the symmetric case.
            let mut held: BTreeSet<(PeerId, bool)> = BTreeSet::new();
            for (peer, edge_topic, state) in participant.upstream() {
                if &edge_topic == topic && state == LinkState::Active {
                    held.insert((peer, false));
                }
            }
            for (peer, edge_topic) in participant.downstream() {
                if &edge_topic == topic {
                    held.insert((peer, false));
                }
            }
            for (peer, edge_topic) in participant.publisher_upstream() {
                if &edge_topic == topic {
                    held.insert((peer, true));
                }
            }
            for (peer, edge_topic, state) in participant.publisher_downstream() {
                if &edge_topic == topic && state == LinkState::Active {
                    held.insert((peer, true));
                }
            }
            held.len()
        })
        .collect()
}

/// Dense degree histogram: index = degree, value = vertex count; length =
/// realised max degree + 1 (empty for an empty graph) — degree-bounded, never
/// population-sized.
pub(crate) fn degree_histogram(degrees: &[usize]) -> Vec<u64> {
    let Some(max) = degrees.iter().copied().max() else {
        return Vec::new();
    };
    let mut histogram = vec![0u64; max + 1];
    for &degree in degrees {
        histogram[degree] += 1;
    }
    histogram
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::str::FromStr;

    use super::{goodness, topology_shape, ChurnPhase, DisseminationModel, PropagationDigraph};
    use crate::experiments::scripted::{self, peer};

    /// The multi-component worked example: three SCCs — a 3-cycle {0,1,2},
    /// a 2-cycle {3,4}, a singleton {5} — chained A → B → C.
    fn worked_example() -> PropagationDigraph {
        PropagationDigraph::from_indexed_edges(
            6,
            &[
                (0, 1),
                (1, 2),
                (2, 0), // A: 3-cycle
                (3, 4),
                (4, 3), // B: 2-cycle
                (2, 3), // A → B
                (4, 5), // B → C (singleton sink)
            ],
        )
    }

    // 016-FR-022: M2 extraction takes downstream records between the phase's
    // honest vertices — adversaries never appear; down honest nodes appear
    // pre-churn only.
    #[test]
    fn m2_extraction_scopes_vertices_by_phase_and_class() {
        let mut population = scripted::full_mesh(4).silent(3).build();
        population
            .participant_mut(&peer(2))
            .expect("node exists")
            .mark_down();

        let post = DisseminationModel::M2.extract(&population, ChurnPhase::PostChurn);
        assert_eq!(post.vertices(), &[peer(0), peer(1)]);
        assert_eq!(post.edge_count(), 2, "0↔1 only");

        let pre = DisseminationModel::M2.extract(&population, ChurnPhase::PreChurn);
        assert_eq!(pre.vertices(), &[peer(0), peer(1), peer(2)]);
        assert_eq!(pre.edge_count(), 6, "full mesh over the three honest");
    }

    // ADR 0041: `m4` parses to its own name and aliases M2's extraction and
    // seed rules exactly — the digraph is identical on any population.
    #[test]
    fn m4_parses_and_aliases_m2_extraction() {
        use std::str::FromStr;
        let m4 = DisseminationModel::from_str("m4").expect("known model");
        assert_eq!(m4, DisseminationModel::M4);
        assert_eq!(m4.name(), "m4");

        let population = scripted::full_mesh(4).silent(3).build();
        let via_m4 = m4.extract(&population, ChurnPhase::PostChurn);
        let via_m2 = DisseminationModel::M2.extract(&population, ChurnPhase::PostChurn);
        assert_eq!(via_m4.vertices(), via_m2.vertices());
        assert_eq!(via_m4.edge_count(), via_m2.edge_count());
        assert_eq!(
            m4.publisher_seeds(&population, &peer(0)),
            DisseminationModel::M2.publisher_seeds(&population, &peer(0)),
        );
    }

    // ADR 0041: every model name parses; M1 aliases M5's extraction.
    #[test]
    fn all_model_names_parse_and_m1_aliases_m5() {
        use std::str::FromStr;
        for (name, model) in [
            ("m1", DisseminationModel::M1),
            ("m3", DisseminationModel::M3),
            ("m5", DisseminationModel::M5),
        ] {
            assert_eq!(DisseminationModel::from_str(name).expect("known"), model);
            assert_eq!(model.name(), name);
        }
        let population = scripted::nodes(3).link(0, 1).publisher_link(1, 2).build();
        let via_m1 = DisseminationModel::M1.extract(&population, ChurnPhase::PostChurn);
        let via_m5 = DisseminationModel::M5.extract(&population, ChurnPhase::PostChurn);
        assert_eq!(via_m1, via_m5);
    }

    // ADR 0041: M5 extraction takes the union of relay and Active publisher
    // downstream records, deduplicated per pair; M3 extraction stays
    // relay-only (initiation links never relay).
    #[test]
    fn m5_unions_link_kinds_and_m3_stays_relay_only() {
        let population = scripted::nodes(3)
            .link(0, 1)
            .publisher_link(0, 1) // both kinds to the same pair: one edge
            .publisher_link(0, 2)
            .build();
        let m5 = DisseminationModel::M5.extract(&population, ChurnPhase::PostChurn);
        assert_eq!(m5.edge_count(), 2, "0→1 deduped across kinds, plus 0→2");
        assert_eq!(m5.out_degree(&peer(0)), Some(2));

        let m3 = DisseminationModel::M3.extract(&population, ChurnPhase::PostChurn);
        assert_eq!(m3.edge_count(), 1, "the relay edge only");
        assert_eq!(m3.out_degree(&peer(0)), Some(1));
    }

    // ADR 0041 / the formal M3 check: seeding heals the muted publisher.
    // Node 0 receives from the {1, 2} cycle but sends to no one (a relay
    // sink) — bare M2 goodness calls the topology bad; with 0's initiation
    // link into the cycle, every publisher's seed closure covers the graph.
    #[test]
    fn m3_seeding_heals_the_muted_publisher() {
        let bare = scripted::nodes(3)
            .link(1, 2)
            .link(2, 1)
            .link(1, 0)
            .link(2, 0)
            .build();
        let m2 = DisseminationModel::M2.analyze(&bare, ChurnPhase::PostChurn);
        assert!(!m2.verdict.good, "muted publisher 0 ⇒ bad under M2");
        assert!(m2.verdict.min_publisher_coverage.abs() < f64::EPSILON);

        let seeded = scripted::nodes(3)
            .link(1, 2)
            .link(2, 1)
            .link(1, 0)
            .link(2, 0)
            .publisher_link(0, 1)
            .build();
        let m3 = DisseminationModel::M3.analyze(&seeded, ChurnPhase::PostChurn);
        assert_eq!(m3.verdict.sccs, 2, "the relay digraph is unchanged");
        assert!(m3.verdict.good, "0's seed into the cycle covers everyone");
        assert!((m3.verdict.min_publisher_coverage - 1.0).abs() < f64::EPSILON);
    }

    // ADR 0041 / the formal M3 law's in-isolated class: a node with no
    // relay in-edges cannot be supplied by other publishers' initiation
    // links — the topology stays bad, and the worst publisher's closure
    // fraction is exact. Node 3 relays into the cycle but receives nothing.
    #[test]
    fn m3_in_isolated_node_stays_bad() {
        let population = scripted::nodes(4)
            .link(1, 2)
            .link(2, 1)
            .link(1, 0)
            .link(2, 0)
            .link(3, 1)
            .publisher_link(0, 1)
            .build();
        let analysis = DisseminationModel::M3.analyze(&population, ChurnPhase::PostChurn);
        assert!(!analysis.verdict.good, "no publisher's seeds reach node 3");
        // Publishers 0, 1, 2 cover {0, 1, 2}: (3−1)/(4−1) = 2/3; publisher 3
        // covers everything.
        assert!((analysis.verdict.min_publisher_coverage - 2.0 / 3.0).abs() < 1e-12);
    }

    // 016-FR-020: the iterative Kosaraju condensation on the worked example —
    // three components, sizes, DAG edges, source and sink sets.
    #[test]
    fn kosaraju_condensation_worked_example() {
        let condensation = worked_example().condensation();
        assert_eq!(condensation.component_sizes.len(), 3);

        let component = |i: usize| condensation.component_of[i];
        // Same component ⟺ same cycle.
        assert_eq!(component(0), component(1));
        assert_eq!(component(1), component(2));
        assert_eq!(component(3), component(4));
        assert_ne!(component(0), component(3));
        assert_ne!(component(3), component(5));

        assert_eq!(condensation.component_sizes[component(0)], 3);
        assert_eq!(condensation.component_sizes[component(3)], 2);
        assert_eq!(condensation.component_sizes[component(5)], 1);

        assert_eq!(
            condensation.edges,
            [(component(0), component(3)), (component(3), component(5)),]
                .into_iter()
                .collect::<BTreeSet<_>>(),
        );
        assert_eq!(
            condensation.sources,
            [component(0)].into_iter().collect::<BTreeSet<_>>()
        );
        assert_eq!(
            condensation.sinks,
            [component(5)].into_iter().collect::<BTreeSet<_>>()
        );
    }

    // 016-FR-020: good ⟺ one SCC; a strongly-connected line is good with
    // full min coverage.
    #[test]
    fn one_scc_is_good_with_full_min_coverage() {
        let population = scripted::line(3).build();
        let analysis = DisseminationModel::M2.analyze(&population, ChurnPhase::PostChurn);
        assert!(analysis.verdict.good);
        assert_eq!(analysis.verdict.sccs, 1);
        assert_eq!(analysis.verdict.largest_scc, 3);
        assert!((analysis.verdict.min_publisher_coverage - 1.0).abs() < f64::EPSILON);
    }

    // 016-FR-020: churn pairing — the same population is good pre-churn and
    // bad post-churn once the middle node goes down.
    #[test]
    fn goodness_pre_vs_post_churn() {
        let mut population = scripted::line(3).build();
        population
            .participant_mut(&peer(1))
            .expect("node exists")
            .mark_down();

        let pre = DisseminationModel::M2.analyze(&population, ChurnPhase::PreChurn);
        assert!(pre.verdict.good);

        let post = DisseminationModel::M2.analyze(&population, ChurnPhase::PostChurn);
        assert!(!post.verdict.good);
        assert_eq!(post.verdict.sccs, 2);
        assert_eq!(post.verdict.largest_scc, 1);
        // Two isolated survivors: each a sink of size 1 → (1−1)/(2−1) = 0.
        assert!(post.verdict.min_publisher_coverage.abs() < f64::EPSILON);
        assert_eq!(post.shape.sinks, 2);
    }

    // 016-FR-021: min publisher-coverage = (smallest sink − 1)/(up-honest − 1)
    // on the worked example: sink = the singleton → (1−1)/(6−1) = 0; and on a
    // two-component chain X(3) → Y(2): sink Y → (2−1)/(5−1) = 0.25.
    #[test]
    fn min_publisher_coverage_comes_from_the_smallest_sink() {
        let verdict = goodness(&worked_example());
        assert!(!verdict.good);
        assert_eq!(verdict.sccs, 3);
        assert_eq!(verdict.largest_scc, 3);
        assert!(verdict.min_publisher_coverage.abs() < f64::EPSILON);

        let chain = PropagationDigraph::from_indexed_edges(
            5,
            &[(0, 1), (1, 2), (2, 0), (3, 4), (4, 3), (0, 3)],
        );
        let verdict = goodness(&chain);
        assert_eq!(verdict.sccs, 2);
        assert!((verdict.min_publisher_coverage - 0.25).abs() < 1e-12);
    }

    // The stranded-node classification relative to the giant — the formal
    // severity tables' deaf/mute split. On the worked example the giant A(3)
    // reaches everything downstream, so nothing is deaf and B ∪ C (3 nodes)
    // are mute; a singleton feeding *into* a cycle is heard but never hears
    // (deaf, not mute); an isolated vertex counts in both classes; a good
    // graph counts zero in each.
    #[test]
    fn deaf_and_mute_classify_relative_to_the_giant() {
        let verdict = goodness(&worked_example());
        assert_eq!(verdict.deaf, 0, "the giant reaches every component");
        assert_eq!(verdict.mute, 3, "B and C never reach the giant");

        // 3 → the {0, 1, 2} cycle: heard by the giant, hears nothing.
        let feeder = PropagationDigraph::from_indexed_edges(4, &[(0, 1), (1, 2), (2, 0), (3, 0)]);
        let verdict = goodness(&feeder);
        assert_eq!(verdict.deaf, 1);
        assert_eq!(verdict.mute, 0);

        // 3 is disconnected in both directions: one node, both classes.
        let isolated = PropagationDigraph::from_indexed_edges(4, &[(0, 1), (1, 2), (2, 0)]);
        let verdict = goodness(&isolated);
        assert_eq!(verdict.deaf, 1);
        assert_eq!(verdict.mute, 1);

        let good = PropagationDigraph::from_indexed_edges(2, &[(0, 1), (1, 0)]);
        let verdict = goodness(&good);
        assert_eq!((verdict.deaf, verdict.mute), (0, 0));
    }

    // The M3 verdict carries the same raw-digraph classification: seed
    // rescue flips `good`, never the deaf/mute counts (the muted-publisher
    // fixture from `m3_seeding_heals_the_muted_publisher`).
    #[test]
    fn seeded_goodness_keeps_the_raw_classification() {
        let seeded = scripted::nodes(3)
            .link(1, 2)
            .link(2, 1)
            .link(1, 0)
            .link(2, 0)
            .publisher_link(0, 1)
            .build();
        let m3 = DisseminationModel::M3.analyze(&seeded, ChurnPhase::PostChurn);
        assert!(m3.verdict.good, "seed-rescued");
        assert_eq!(m3.verdict.deaf, 0, "0 hears the cycle");
        assert_eq!(m3.verdict.mute, 1, "0 still never relays back");
    }

    // 016-FR-019: degree histograms and the honest-sink count.
    #[test]
    fn degree_and_sink_statistics() {
        let population = scripted::line(3).build();
        let digraph = DisseminationModel::M2.extract(&population, ChurnPhase::PostChurn);
        let shape = topology_shape(&digraph);
        // Line of three: ends have degree 1, the middle 2 (both directions).
        assert_eq!(shape.out_degree_hist, vec![0, 2, 1]);
        assert_eq!(shape.in_degree_hist, vec![0, 2, 1]);
        assert_eq!(shape.sinks, 0);

        let silent_middle = scripted::line(3).silent(1).build();
        let digraph = DisseminationModel::M2.extract(&silent_middle, ChurnPhase::PostChurn);
        let shape = topology_shape(&digraph);
        // The two honest ends survive with no edges between them.
        assert_eq!(shape.out_degree_hist, vec![2]);
        assert_eq!(shape.sinks, 2);
    }

    // 016-SC-003 (reachability half): reach shrinks toward condensation sinks.
    #[test]
    fn reachability_follows_the_condensation_chain() {
        let digraph = worked_example();
        let all: BTreeSet<_> = (0..6).map(peer).collect();
        assert_eq!(digraph.reachable_from(&peer(0)), all);
        assert_eq!(
            digraph.reachable_from(&peer(3)),
            [peer(3), peer(4), peer(5)].into_iter().collect(),
        );
        assert_eq!(
            digraph.reachable_from(&peer(5)),
            [peer(5)].into_iter().collect(),
        );
    }

    // 016-FR-022: the model dispatch parses from its configuration name and
    // owns the seed-set rule (M2: the publisher alone; M3: the publisher
    // plus its Active publisher-link targets).
    #[test]
    fn model_dispatch_parses_and_seeds() {
        assert_eq!(
            DisseminationModel::from_str("m2").expect("known model"),
            DisseminationModel::M2,
        );
        assert_eq!(DisseminationModel::M2.name(), "m2");
        assert!(DisseminationModel::from_str("m6").is_err());
        let population = scripted::nodes(3)
            .publisher_link(0, 1)
            .publisher_link(0, 2)
            .build();
        assert_eq!(
            DisseminationModel::M2.publisher_seeds(&population, &peer(0)),
            vec![peer(0)],
        );
        assert_eq!(
            DisseminationModel::M3.publisher_seeds(&population, &peer(0)),
            vec![peer(0), peer(1), peer(2)],
        );
    }
}
