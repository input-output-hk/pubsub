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

use crate::peer::PeerId;

use super::population::Population;

/// The dissemination model an experiment runs under: the dispatch that owns
/// propagation-graph extraction, the per-publisher seed-set rule, and the
/// goodness criterion. v1 ships exactly one variant; the
/// dispatch shape is what the experiment program's later stages and the
/// in-flight publisher-links work extend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisseminationModel {
    /// Uniform-relay dissemination: propagation edges are the `downstream`
    /// records between up-honest peers; the seed set is the publisher alone;
    /// good ⟺ one strongly connected component.
    M2,
}

/// The error returned when a configuration string names no known
/// dissemination model.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("unknown dissemination model '{0}' (expected one of: m2)")]
pub struct UnknownDisseminationModel(pub String);

impl DisseminationModel {
    /// The canonical, lower-case configuration name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::M2 => "m2",
        }
    }

    /// The per-publisher seed set: the vertices assumed to hold the message
    /// at wave 0 — the dispatch's parameterisation point. M2 seeds the
    /// publisher alone.
    #[must_use]
    pub fn publisher_seeds(self, publisher: &PeerId) -> Vec<PeerId> {
        match self {
            Self::M2 => vec![publisher.clone()],
        }
    }

    /// Extract the propagation digraph from the population's node states.
    ///
    /// Vertices are the honest participants of the requested phase —
    /// [`ChurnPhase::PostChurn`] takes up-honest only (the primary graph);
    /// [`ChurnPhase::PreChurn`] includes down honest nodes (the formed-
    /// topology diagnostic). Adversarial participants are never vertices:
    /// under M2 the silent relay contributes nothing to propagation. An edge
    /// `u → v` exists iff `v` is in `u`'s fan-out target set — for M2, `u`'s
    /// `downstream` records restricted to the vertex set.
    #[must_use]
    pub fn extract(self, population: &Population, phase: ChurnPhase) -> PropagationDigraph {
        let Self::M2 = self;
        let in_scope = |participant: &super::population::Participant| match phase {
            ChurnPhase::PreChurn => {
                participant.class() == super::population::ParticipantClass::Honest
            }
            ChurnPhase::PostChurn => participant.is_up_honest(),
        };
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
        }
        PropagationDigraph::from_vertices_and_edges(vertices, &edges)
    }

    /// Extract and analyse in one call: the digraph, its goodness verdict,
    /// and its topology shape.
    #[must_use]
    pub fn analyze(self, population: &Population, phase: ChurnPhase) -> GraphAnalysis {
        let digraph = self.extract(population, phase);
        let verdict = goodness(&digraph);
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
            "m2" => Ok(Self::M2),
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
    /// One SCC ⟺ every up-honest publisher reaches every up-honest node.
    pub good: bool,
    /// Worst-case coverage over all up-honest publishers:
    /// (smallest condensation-sink component − 1) / (up-honest − 1).
    pub min_publisher_coverage: f64,
    /// Number of strongly connected components.
    pub sccs: u64,
    /// Size of the largest strongly connected component.
    pub largest_scc: u64,
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

/// Dense degree histogram: index = degree, value = vertex count; length =
/// realised max degree + 1 (empty for an empty graph) — degree-bounded, never
/// population-sized.
fn degree_histogram(degrees: &[usize]) -> Vec<u64> {
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
    // owns the seed-set rule (M2: the publisher alone).
    #[test]
    fn model_dispatch_parses_and_seeds() {
        assert_eq!(
            DisseminationModel::from_str("m2").expect("known model"),
            DisseminationModel::M2,
        );
        assert_eq!(DisseminationModel::M2.name(), "m2");
        assert!(DisseminationModel::from_str("m3").is_err());
        assert_eq!(
            DisseminationModel::M2.publisher_seeds(&peer(7)),
            vec![peer(7)],
        );
    }
}
