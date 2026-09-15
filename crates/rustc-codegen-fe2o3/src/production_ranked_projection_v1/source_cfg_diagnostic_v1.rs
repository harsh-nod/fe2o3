//! Bounded observations on a rejected source CFG, never proof or pruning facts.

use super::{MAX_RANKED_BOUNDS_BLOCKS, MAX_RANKED_BOUNDS_EDGES, SemanticFunctionDeclV1};
use std::{collections::BTreeSet, fmt};

#[path = "source_cfg_diagnostic_v1/empty_goto_census_v1.rs"]
pub(crate) mod empty_goto_census_v1;

#[derive(Debug)]
pub(crate) struct SourceCfgLimitV1 {
    declared_blocks: usize,
    reachability: Reachability,
    empty_goto_census: Option<empty_goto_census_v1::Observation>,
}

impl SourceCfgLimitV1 {
    pub(super) fn capture(function: &SemanticFunctionDeclV1) -> Self {
        Self {
            declared_blocks: function.blocks().len(),
            reachability: Reachability::capture(
                function.blocks().len(),
                function.entry().index() as usize,
                MAX_RANKED_BOUNDS_BLOCKS + 1,
                MAX_RANKED_BOUNDS_EDGES,
                |block, visit| {
                    function.blocks()[block]
                        .terminator()
                        .kind()
                        .try_for_each_edge(|edge| visit(edge.target().index() as usize))
                },
            ),
            empty_goto_census: Some(empty_goto_census_v1::capture(function)),
        }
    }
}

impl fmt::Display for SourceCfgLimitV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "semantic-to-ranked projection rejected semantic CFG exceeds the ranked block limit before loop analysis; declared_source_blocks={} limit={MAX_RANKED_BOUNDS_BLOCKS}; complete-source-edge entry reachability: ",
            self.declared_blocks,
        )?;
        match self.reachability.stop {
            Some(Stop::Malformed) => formatter.write_str("unavailable (malformed edge or entry)"),
            stop => write!(
                formatter,
                "{}{} blocks, {} edge visits{}",
                if stop.is_some() { "at least " } else { "" },
                self.reachability.blocks,
                self.reachability.edges,
                if stop.is_some() {
                    " (diagnostic budget reached)"
                } else {
                    ""
                },
            ),
        }?;
        if let Some(census) = &self.empty_goto_census {
            write!(formatter, "; empty-Goto census: {census:?}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stop {
    Limit,
    Malformed,
}

#[derive(Debug, Eq, PartialEq)]
struct Reachability {
    blocks: usize,
    edges: usize,
    stop: Option<Stop>,
}

impl Reachability {
    fn capture(
        declared_blocks: usize,
        entry: usize,
        max_blocks: usize,
        max_edges: usize,
        mut visit_edges: impl FnMut(
            usize,
            &mut dyn FnMut(usize) -> Result<(), Stop>,
        ) -> Result<(), Stop>,
    ) -> Self {
        if entry >= declared_blocks || max_blocks == 0 {
            return Self {
                blocks: 0,
                edges: 0,
                stop: Some(Stop::Malformed),
            };
        }
        let mut seen = BTreeSet::from([entry]);
        let mut pending = vec![entry];
        let mut edges = 0;
        while let Some(block) = pending.pop() {
            // Mark on insertion, retaining at most one pending item per block.
            // Parallel, cleanup and imaginary edges all consume the edge budget.
            let result = visit_edges(block, &mut |target| {
                if edges == max_edges {
                    return Err(Stop::Limit);
                }
                edges += 1;
                if target >= declared_blocks {
                    return Err(Stop::Malformed);
                }
                if !seen.contains(&target) {
                    if seen.len() == max_blocks {
                        return Err(Stop::Limit);
                    }
                    seen.insert(target);
                    pending.push(target);
                }
                Ok(())
            });
            if let Err(stop) = result {
                return Self {
                    blocks: seen.len(),
                    edges,
                    stop: Some(stop),
                };
            }
        }
        Self {
            blocks: seen.len(),
            edges,
            stop: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observe(graph: &[&[usize]], entry: usize, blocks: usize, edges: usize) -> Reachability {
        Reachability::capture(graph.len(), entry, blocks, edges, |block, visit| {
            graph[block].iter().try_for_each(|&target| visit(target))
        })
    }

    #[test]
    fn source_cfg_diagnostic_counts_high_entry_cycles_and_parallel_edges_exactly() {
        let graph: &[&[usize]] = &[&[0], &[], &[], &[5, 5, 4], &[3], &[]];
        assert_eq!(
            observe(graph, 3, 3, 4),
            Reachability {
                blocks: 3,
                edges: 4,
                stop: None
            }
        );
        assert_eq!(observe(graph, 3, 3, 3).stop, Some(Stop::Limit));
        assert_eq!(observe(graph, 3, 2, 4).stop, Some(Stop::Limit));
    }

    #[test]
    fn source_cfg_diagnostic_does_not_call_an_unvisited_node_dead() {
        let graph: &[&[usize]] = &[&[1], &[2], &[3], &[]];
        let observed = observe(graph, 0, 3, 10);
        assert_eq!(
            observed,
            Reachability {
                blocks: 3,
                edges: 3,
                stop: Some(Stop::Limit)
            }
        );
        let diagnostic = SourceCfgLimitV1 {
            declared_blocks: 2000,
            reachability: observed,
            empty_goto_census: None,
        };
        assert!(diagnostic.to_string().contains("at least 3 blocks"));
        assert!(diagnostic.to_string().contains("diagnostic budget reached"));
    }

    #[test]
    fn source_cfg_diagnostic_is_independent_of_declared_graph_size() {
        let observed = Reachability::capture(usize::MAX, usize::MAX - 1, 2, 2, |block, visit| {
            visit(block)
        });
        assert_eq!(
            observed,
            Reachability {
                blocks: 1,
                edges: 1,
                stop: None
            }
        );
        assert_eq!(observe(&[&[1]], 0, 2, 2).stop, Some(Stop::Malformed));
        assert_eq!(observe(&[&[]], 1, 2, 2).stop, Some(Stop::Malformed));
    }
}
