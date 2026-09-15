//! Extra custody edges in the existing candidate graph, not a second SSA graph.
//! A joined carrier is publishable only after every connected root passes.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1;

#[derive(Default)]
pub(super) struct Joins {
    edges: BTreeSet<(usize, usize)>,
    proven: Vec<bool>,
}

fn key_work(len: usize) -> usize {
    1 + (usize::BITS - len.leading_zeros()) as usize
}

impl Joins {
    pub(super) fn connect(
        &mut self,
        routes: &math_capture_flow_v1::Routes<'_>,
        assignment: &SemanticAssignmentV1,
        index: usize,
        by_reference: &BTreeMap<u32, usize>,
        candidates: &mut [SemanticBorrowCandidateV1],
        budget: &mut Budget,
    ) -> Result<math_capture_flow_v1::CheckedFields, ProductionSemanticSsaErrorV1> {
        let mut checked_secondary_fields = [None; 2];
        // Ambiguity remains an obligation even if all of its operand locals
        // were excluded from the candidate map by duplicate definitions.
        if !routes.secondary_complete(assignment.destination().ty(), budget)? {
            candidates[index].valid = false;
            return Ok(checked_secondary_fields);
        }
        let sources = routes.secondary_sources(assignment, budget)?;
        for (slot, obligation) in sources.into_iter().enumerate() {
            let Some((role, field, source)) = obligation else {
                continue;
            };
            budget.charge(2)?;
            let Some(source) = source else {
                candidates[index].valid = false;
                continue;
            };
            budget.charge(2 + key_work(by_reference.len()))?;
            let Some(&parent) = by_reference.get(&source.local().index()) else {
                candidates[index].valid = false;
                continue;
            };
            if parent == index {
                candidates[index].valid = false;
                continue;
            }
            // Independently checked roles can share one existing parent edge.
            let primary_parent = if let Some(local) = candidates[index].source_reference {
                budget.charge(key_work(by_reference.len()))?;
                by_reference.get(&local).copied()
            } else {
                None
            };
            if primary_parent != Some(parent) {
                if self.edges.is_empty() {
                    budget.charge(3)?;
                }
                budget.charge(5 + key_work(self.edges.len()))?;
                if self.edges.insert((parent, index)) {
                    candidates[parent].consumers = candidates[parent].consumers.saturating_add(1);
                }
            }
            checked_secondary_fields[slot] = Some((role, field));
        }
        Ok(checked_secondary_fields)
    }

    pub(super) fn start(
        &mut self,
        children: &mut [Vec<usize>],
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        if self.edges.is_empty() {
            return Ok(());
        }
        budget.charge(3 + children.len().saturating_mul(2))?;
        self.proven = vec![false; children.len()];
        for &(parent, child) in &self.edges {
            // Logical visit, edge storage and current capacity-copy allowance.
            budget.charge(3 + children[parent].len())?;
            children[parent].push(child);
        }
        Ok(())
    }

    pub(super) fn record(
        &mut self,
        members: &BTreeSet<usize>,
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        if self.edges.is_empty() {
            return Ok(());
        }
        budget.charge(members.len())?;
        for &member in members {
            self.proven[member] = true;
        }
        Ok(())
    }

    pub(super) fn retain(
        &self,
        candidates: &[SemanticBorrowCandidateV1],
        children: &[Vec<usize>],
        lane_sites: &[Vec<SemanticTransparentBorrowSiteV1>],
        accepted: &mut BTreeSet<SemanticTransparentBorrowSiteV1>,
        budget: &mut Budget,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        if self.edges.is_empty() {
            return Ok(());
        }
        let count = candidates.len();
        // Logical element initialization and retained elements, plus headers.
        budget.charge(6 + count.saturating_mul(4))?;
        let mut parent: Vec<usize> = (0..count).collect();
        let mut blocked = vec![false; count];
        for (from, edges) in children.iter().enumerate() {
            budget.charge(1)?;
            for &to in edges {
                budget.charge(1)?;
                let a = representative(&mut parent, from, budget)?;
                let b = representative(&mut parent, to, budget)?;
                budget.charge(1)?;
                parent[b] = a;
            }
        }
        for (node, &proven) in self.proven.iter().enumerate() {
            budget.charge(1)?;
            if !proven {
                let root = representative(&mut parent, node, budget)?;
                budget.charge(1)?;
                blocked[root] = true;
            }
        }
        for (node, candidate) in candidates.iter().enumerate() {
            budget.charge(1)?;
            let root = representative(&mut parent, node, budget)?;
            if !blocked[root] {
                continue;
            }
            budget.charge(key_work(accepted.len()))?;
            accepted.remove(&candidate.site);
            if let Some(sites) = lane_sites.get(node) {
                for site in sites {
                    budget.charge(1 + key_work(accepted.len()))?;
                    accepted.remove(site);
                }
            }
        }
        Ok(())
    }
}

fn representative(
    parent: &mut [usize],
    mut node: usize,
    budget: &mut Budget,
) -> Result<usize, ProductionSemanticSsaErrorV1> {
    loop {
        budget.charge(1)?;
        let next = parent[node];
        if next == node {
            return Ok(node);
        }
        budget.charge(2)?;
        parent[node] = parent[next];
        node = next;
    }
}
