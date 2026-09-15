use crate::ssa::*;
use std::collections::BTreeSet;

pub(crate) struct Oracle {
    pub(crate) parents: Vec<Option<usize>>,
    pub(crate) frontiers: Vec<Vec<u32>>,
    pub(crate) capacities: Vec<usize>,
}

// Small test graphs only: meet complete dominator sets, then use the defining
// predecessor relation. This does not reproduce the production tree walk.
pub(crate) fn inspect(input: &SsaConstructionInputV1) -> Oracle {
    let n = input.blocks().len();
    assert!((1..=96).contains(&n), "bounded test oracle");
    let entry = input.entry().get() as usize;
    let mut reachable = BTreeSet::new();
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        if reachable.insert(block) {
            pending.extend(
                input.blocks()[block]
                    .edges()
                    .iter()
                    .map(|e| e.target().get() as usize),
            );
        }
    }
    let mut predecessors = vec![BTreeSet::new(); n];
    for &source in &reachable {
        for edge in input.blocks()[source].edges() {
            predecessors[edge.target().get() as usize].insert(source);
        }
    }
    let mut dom = vec![BTreeSet::new(); n];
    for &block in &reachable {
        dom[block] = if block == entry {
            BTreeSet::from([entry])
        } else {
            reachable.clone()
        };
    }
    loop {
        let old = dom.clone();
        for &block in &reachable {
            if block == entry {
                continue;
            }
            let mut next = reachable.clone();
            for &predecessor in &predecessors[block] {
                next = next.intersection(&old[predecessor]).copied().collect();
            }
            next.insert(block);
            dom[block] = next;
        }
        if dom == old {
            break;
        }
    }
    let mut parents = vec![None; n];
    for &block in &reachable {
        if block != entry {
            parents[block] = dom[block]
                .iter()
                .copied()
                .filter(|d| *d != block)
                .max_by_key(|d| dom[*d].len());
        }
    }
    let mut frontiers = vec![Vec::new(); n];
    for &block in &reachable {
        for &target in &reachable {
            let strictly_dominates = block != target && dom[target].contains(&block);
            if !strictly_dominates
                && predecessors[target]
                    .iter()
                    .any(|p| dom[*p].contains(&block))
            {
                frontiers[block].push(target as u32);
            }
        }
    }
    let mut capacities = vec![0; n];
    for &block in &reachable {
        capacities[block] = input.blocks()[block].edges().len();
        for (child, parent) in parents.iter().enumerate() {
            if *parent == Some(block) {
                capacities[block] += frontiers[child].len();
            }
        }
    }
    Oracle {
        parents,
        frontiers,
        capacities,
    }
}

pub(crate) fn saving(input: &SsaConstructionInputV1) -> usize {
    inspect(input)
        .capacities
        .into_iter()
        .map(|n| (n * size_of::<usize>()).div_ceil(8) - (n * size_of::<SsaBlockIdV1>()).div_ceil(8))
        .sum()
}
