//! Exact component order shortcuts over one immutable complete edge owner.
use super::*;

pub(super) struct SccGraph {
    edges: Vec<Vec<u32>>,
    index: Option<Index>,
    #[cfg(test)]
    dfs_only: bool,
}

impl SccGraph {
    pub(super) fn new(edges: Vec<Vec<u32>>) -> Self {
        Self {
            edges,
            index: None,
            #[cfg(test)]
            dfs_only: DFS_REFERENCE.get(),
        }
    }

    #[cfg(test)]
    pub(super) fn dfs_only(edges: Vec<Vec<u32>>) -> Self {
        with_dfs_reference(|| Self::new(edges))
    }

    pub(super) fn edges(&self) -> &[Vec<u32>] {
        &self.edges
    }

    /// None requires the original exact path query; it never means no path.
    pub(super) fn shortcut(
        &mut self,
        start: u32,
        end: u32,
        budget: &mut Budget,
    ) -> Result<Option<bool>> {
        #[cfg(test)]
        if self.dfs_only {
            return Ok(None);
        }
        budget.charge(1)?;
        if start as usize >= self.edges.len() || end as usize >= self.edges.len() {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        if self.index.is_none() {
            // Failed construction cannot publish a partial component index.
            self.index = Some(Index::build(&self.edges, start, budget)?);
        }
        let index = self.index.as_ref().unwrap();
        let a = index.component[start as usize];
        let b = index.component[end as usize];
        Ok(if a == b {
            Some(start != end || index.flags[a as usize] & CYCLIC != 0)
        } else if a < b {
            // Tarjan emits sink components first. Every cross-component edge
            // strictly decreases this ID; its reverse cannot be a path.
            Some(false)
        } else {
            budget.charge(1)?;
            // The first DFS root emits exactly its reachable SCCs before
            // emitting its own SCC. Later roots may prove a smaller interval.
            if index.flags[a as usize] & FIRST_ROOT != 0 {
                Some(true)
            } else {
                budget.charge(1)?;
                // Only components emitted during this SCC root's DFS subtree
                // are known reachable. Earlier components still require DFS.
                (b >= index.first_descendant[start as usize]).then_some(true)
            }
        })
    }
}

#[cfg(test)]
thread_local! {
    static DFS_REFERENCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Test reference mode changes only the query implementation, never loan facts.
#[cfg(test)]
pub(in super::super) fn with_dfs_reference<T>(action: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            DFS_REFERENCE.set(self.0);
        }
    }
    let _restore = Restore(DFS_REFERENCE.replace(true));
    action()
}

const CYCLIC: u8 = 1;
const FIRST_ROOT: u8 = 2;

struct Index {
    component: Vec<u32>,
    flags: Vec<u8>,
    // Per vertex: first SCC emitted during its SCC root's DFS subtree.
    // This is the former lowlink allocation, not another graph or path cache.
    first_descendant: Vec<u32>,
}

struct Frame {
    node: u32,
    first_component: u32,
    next_edge: usize,
}

impl Index {
    fn build(edges: &[Vec<u32>], first: u32, budget: &mut Budget) -> Result<Self> {
        let count = edges.len();
        budget.charge(1)?;
        let first = first as usize;
        if count > u32::MAX as usize || first >= count {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        // Charge all seven vector headers and every requested scratch/output
        // element before allocation, including padded frame words. Scalar
        // arrays are conservatively charged at a full word per element.
        let frame_words = size_of::<Frame>().div_ceil(size_of::<usize>());
        let charge = count
            .checked_mul(6 + frame_words)
            .and_then(|n| n.checked_add(7 * size_of::<Vec<u32>>() / size_of::<usize>()))
            .unwrap_or(usize::MAX);
        budget.charge(charge)?;
        let mut number = vec![u32::MAX; count];
        let mut low = vec![0u32; count];
        let mut component = vec![u32::MAX; count];
        let mut active = vec![false; count];
        let mut members = Vec::with_capacity(count);
        let mut frames = Vec::<Frame>::with_capacity(count);
        let mut flags = vec![0u8; count];
        let mut clock = 0u32;
        let mut next_component = 0u32;
        for root in std::iter::once(first).chain(0..first).chain(first + 1..count) {
            budget.charge(1)?;
            if number[root] != u32::MAX {
                continue;
            }
            Self::enter(
                root as u32,
                &mut clock,
                next_component,
                &mut number,
                &mut low,
                &mut active,
                &mut members,
                &mut frames,
                budget,
            )?;
            while let Some(frame) = frames.last() {
                let node = frame.node as usize;
                let edge = frame.next_edge;
                if let Some(&next) = edges[node].get(edge) {
                    budget.charge(1)?;
                    if next as usize >= count {
                        return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
                    }
                    frames.last_mut().unwrap().next_edge += 1;
                    if number[next as usize] == u32::MAX {
                        Self::enter(
                            next,
                            &mut clock,
                            next_component,
                            &mut number,
                            &mut low,
                            &mut active,
                            &mut members,
                            &mut frames,
                            budget,
                        )?;
                    } else if active[next as usize] {
                        low[node] = low[node].min(number[next as usize]);
                    }
                    continue;
                }
                budget.charge(1)?;
                let first_component = frame.first_component;
                frames.pop();
                // Popped members no longer need lowlinks. Preserve the value
                // used by the still-active parent before reusing that storage.
                let parent_low = low[node];
                if parent_low == number[node] {
                    let mut size = 0usize;
                    loop {
                        budget.charge(2)?;
                        let member = members
                            .pop()
                            .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?
                            as usize;
                        active[member] = false;
                        component[member] = next_component;
                        low[member] = first_component;
                        size += 1;
                        if member == node {
                            break;
                        }
                    }
                    let cycle = if size > 1 {
                        true
                    } else {
                        budget.charge(edges[node].len())?;
                        edges[node].contains(&(node as u32))
                    };
                    flags[next_component as usize] = if cycle { CYCLIC } else { 0 };
                    next_component += 1;
                }
                if let Some(parent) = frames.last() {
                    let parent = parent.node as usize;
                    low[parent] = low[parent].min(parent_low);
                }
            }
            if root == first {
                budget.charge(1)?;
                flags[component[first] as usize] |= FIRST_ROOT;
            }
        }
        Ok(Self { component, flags, first_descendant: low })
    }

    #[allow(clippy::too_many_arguments)]
    fn enter(
        node: u32,
        clock: &mut u32,
        first_component: u32,
        number: &mut [u32],
        low: &mut [u32],
        active: &mut [bool],
        members: &mut Vec<u32>,
        frames: &mut Vec<Frame>,
        budget: &mut Budget,
    ) -> Result<()> {
        budget.charge(2)?;
        let i = node as usize;
        number[i] = *clock;
        low[i] = *clock;
        *clock += 1;
        active[i] = true;
        members.push(node);
        frames.push(Frame { node, first_component, next_edge: 0 });
        Ok(())
    }
}

#[cfg(test)]
#[path = "scc_order_v1/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "scc_order_v1/seed_closure_tests.rs"]
mod seed_closure_tests;

#[cfg(test)]
#[path = "scc_order_v1/descendant_interval_tests.rs"]
mod descendant_interval_tests;
