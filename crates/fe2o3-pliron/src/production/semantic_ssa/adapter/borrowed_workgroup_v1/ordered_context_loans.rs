//! Ordered Context reborrows add address transparency, never source issuance.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticMutabilityV1, SemanticPointerKindV1, SemanticPointerMetadataV1,
};

type Site = SemanticTransparentBorrowSiteV1;
type Result<T> = std::result::Result<T, ProductionSemanticSsaErrorV1>;

#[path = "ordered_context_loans/reachability_scratch_v1.rs"]
mod reachability_scratch_v1;
use reachability_scratch_v1::PathScratch;
#[path = "ordered_context_loans/scc_order_v1.rs"]
mod scc_order_v1;
use scc_order_v1::SccGraph;

#[cfg(test)]
pub(super) use scc_order_v1::with_dfs_reference;

pub(super) fn shared_reborrow(
    types: Option<&[SemanticTypeDeclV1]>,
    source: SemanticTypeIdV1,
    destination: SemanticTypeIdV1,
    owned: SemanticTypeIdV1,
    shared_pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    mutable_pairs: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
) -> bool {
    let Some(types) = types else { return false };
    if source == destination
        || mutable_pairs.get(&source) != Some(&owned)
        || shared_pairs.get(&destination) != Some(&owned)
        || mutable_pairs.contains_key(&destination)
    {
        return false;
    }
    let (Some(SemanticTypeShapeV1::Pointer(from)), Some(SemanticTypeShapeV1::Pointer(to))) = (
        types
            .get(source.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        types
            .get(destination.index() as usize)
            .map(SemanticTypeDeclV1::shape),
    ) else {
        return false;
    };
    from.kind() == SemanticPointerKindV1::Reference
        && to.kind() == SemanticPointerKindV1::Reference
        && from.mutability() == SemanticMutabilityV1::Mutable
        && to.mutability() == SemanticMutabilityV1::Immutable
        && from.metadata() == SemanticPointerMetadataV1::None
        && to.metadata() == SemanticPointerMetadataV1::None
        && from.pointee() == owned
        && to.pointee() == owned
        && from.address_space() == to.address_space()
        && from.pointer_width_bits() == to.pointer_width_bits()
}

struct Order {
    graph: SccGraph,
    entry: u32,
    cache: BTreeMap<(u32, u32), bool>,
    scratch: Option<PathScratch>,
}

impl Order {
    fn new(function: &SemanticFunctionDeclV1, budget: &mut Budget) -> Result<Self> {
        budget.scoped(FlowWorkStage::Cfg, |budget| Self::new_inner(function, budget))
    }

    fn new_inner(function: &SemanticFunctionDeclV1, budget: &mut Budget) -> Result<Self> {
        if function.entry().index() as usize >= function.blocks().len() {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        budget.charge(function.blocks().len())?;
        let mut edges = Vec::with_capacity(function.blocks().len());
        for block in function.blocks() {
            let term = block.terminator().kind();
            budget.charge(term.edge_count())?;
            let mut next = Vec::with_capacity(term.edge_count());
            term.try_for_each_edge(|edge| {
                let target = edge.target().index();
                if target as usize >= function.blocks().len() {
                    return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
                }
                next.push(target);
                Ok(())
            })?;
            edges.push(next);
        }
        Ok(Self {
            graph: SccGraph::new(edges),
            entry: function.entry().index(),
            cache: BTreeMap::new(),
            scratch: None,
        })
    }

    // A nonempty path, including a cycle when start == end. All edge roles count.
    fn path(&mut self, start: u32, end: u32, budget: &mut Budget) -> Result<bool> {
        budget.scoped(FlowWorkStage::Paths, |budget| self.path_inner(start, end, budget))
    }

    fn path_inner(&mut self, start: u32, end: u32, budget: &mut Budget) -> Result<bool> {
        budget.charge(1)?;
        if let Some(&found) = self.cache.get(&(start, end)) {
            return Ok(found);
        }
        if start as usize >= self.graph.edges().len() || end as usize >= self.graph.edges().len() {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        let found = match self.graph.shortcut(start, end, budget)? {
            Some(found) => found,
            None => {
                let scratch = match &mut self.scratch {
                    Some(scratch) => scratch,
                    empty @ None => empty.insert(PathScratch::new(self.graph.edges().len(), budget)?),
                };
                scratch.path(self.graph.edges(), start, end, budget)?
            }
        };
        budget.charge(1)?;
        self.cache.insert((start, end), found);
        Ok(found)
    }

    fn reachable(&mut self, site: Site, budget: &mut Budget) -> Result<bool> {
        budget.charge(1)?;
        Ok(site.block == self.entry || self.path(self.entry, site.block, budget)?)
    }

    fn may_follow(&mut self, first: Site, second: Site, budget: &mut Budget) -> Result<bool> {
        budget.charge(1)?;
        if first.block == second.block && first.statement <= second.statement {
            return Ok(true);
        }
        self.path(first.block, second.block, budget)
    }

    fn before(&mut self, first: Site, second: Site, budget: &mut Budget) -> Result<bool> {
        Ok(first != second
            && self.reachable(first, budget)?
            && self.reachable(second, budget)?
            && self.may_follow(first, second, budget)?
            && !self.may_follow(second, first, budget)?)
    }
}

fn descendants(
    root: usize,
    candidates: &[SemanticBorrowCandidateV1],
    children: &[Vec<usize>],
    budget: &mut Budget,
) -> Result<Option<Vec<usize>>> {
    budget.scoped(FlowWorkStage::Descendants, |budget| {
        descendants_inner(root, candidates, children, budget)
    })
}

fn descendants_inner(
    root: usize,
    candidates: &[SemanticBorrowCandidateV1],
    children: &[Vec<usize>],
    budget: &mut Budget,
) -> Result<Option<Vec<usize>>> {
    budget.charge(1)?;
    let mut pending = vec![root];
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    while let Some(index) = pending.pop() {
        budget.charge(3 + children[index].len())?;
        if !candidates[index].valid || !seen.insert(index) {
            return Ok(None);
        }
        result.push(index);
        pending.extend(children[index].iter().copied());
    }
    Ok(Some(result))
}

fn ends_before(
    root: usize,
    end: Site,
    candidates: &[SemanticBorrowCandidateV1],
    children: &[Vec<usize>],
    uses: &[Vec<Site>],
    order: &mut Order,
    budget: &mut Budget,
) -> Result<bool> {
    let Some(nodes) = descendants(root, candidates, children, budget)? else {
        return Ok(false);
    };
    for node in nodes {
        if !order.before(candidates[node].site, end, budget)? {
            return Ok(false);
        }
        for &site in &uses[node] {
            if !order.before(site, end, budget)? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn children_are_ordered(
    siblings: &[usize],
    direct: &[Site],
    candidates: &[SemanticBorrowCandidateV1],
    children: &[Vec<usize>],
    uses: &[Vec<Site>],
    mutable: &BTreeSet<usize>,
    order: &mut Order,
    budget: &mut Budget,
) -> Result<bool> {
    for (position, &left) in siblings.iter().enumerate() {
        for &right in &siblings[position + 1..] {
            budget.charge(1)?;
            if !mutable.contains(&left) && !mutable.contains(&right) {
                continue;
            }
            if !ends_before(
                left,
                candidates[right].site,
                candidates,
                children,
                uses,
                order,
                budget,
            )? && !ends_before(
                right,
                candidates[left].site,
                candidates,
                children,
                uses,
                order,
                budget,
            )? {
                return Ok(false);
            }
        }
        for &site in direct {
            budget.charge(1)?;
            // An exclusive use of the parent cannot overlap any descendant loan.
            if !ends_before(left, site, candidates, children, uses, order, budget)?
                && !order.before(site, candidates[left].site, budget)?
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

// Reuse canonical operand traversal. No move, lifetime marker or owner rewrite is erased.
fn owner_changes(
    function: &SemanticFunctionDeclV1,
    owner: u32,
    budget: &mut Budget,
) -> Result<Vec<Site>> {
    budget.scoped(FlowWorkStage::OwnerChanges, |budget| {
        owner_changes_inner(function, owner, budget)
    })
}

fn owner_changes_inner(
    function: &SemanticFunctionDeclV1,
    owner: u32,
    budget: &mut Budget,
) -> Result<Vec<Site>> {
    let mut changes = Vec::new();
    for (block, body) in function.blocks().iter().enumerate() {
        for (statement, item) in body.statements().iter().enumerate() {
            budget.charge(1)?;
            let mut changed = false;
            let mut moved = |operand: &SemanticOperandV1| -> Result<()> {
                budget.charge(1)?;
                if let SemanticOperandV1::Move(place) = operand {
                    changed |= place.local().index() == owner;
                }
                Ok(())
            };
            match item.kind() {
                SemanticStatementKindV1::Assign(a) => {
                    a.value().kind().try_visit_operands(&mut moved)?;
                    changed |= a.destination().local().index() == owner;
                }
                SemanticStatementKindV1::Store(s) => {
                    moved(s.value())?;
                    changed |= s.destination().local().index() == owner;
                }
                SemanticStatementKindV1::AtomicRmw(a) => {
                    moved(a.value())?;
                    changed |= a.destination().local().index() == owner
                        || a.address().local().index() == owner;
                }
                SemanticStatementKindV1::AtomicCompareExchange(a) => {
                    moved(a.expected())?;
                    moved(a.replacement())?;
                    changed |= a.destination().local().index() == owner
                        || a.address().local().index() == owner;
                }
                SemanticStatementKindV1::Assume(value) => moved(value)?,
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => {
                    changed = place.local().index() == owner
                }
                SemanticStatementKindV1::StorageLive(local)
                | SemanticStatementKindV1::StorageDead(local) => changed = local.index() == owner,
                SemanticStatementKindV1::Nop => (),
            }
            if changed {
                budget.charge(1)?;
                changes.push(Site {
                    block: block as u32,
                    statement: statement as u32,
                });
            }
        }
        budget.charge(1)?;
        let term = body.terminator().kind();
        let mut changed = false;
        let mut moved = |operand: &SemanticOperandV1| -> Result<()> {
            budget.charge(1)?;
            changed |= matches!(operand, SemanticOperandV1::Move(p) if p.local().index() == owner);
            Ok(())
        };
        match term {
            SemanticTerminatorKindV1::Call(call) => {
                for operand in call.arguments() {
                    moved(operand)?;
                }
                changed |= call
                    .destination()
                    .is_some_and(|d| d.place().local().index() == owner);
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                for operand in call.arguments() {
                    moved(operand)?;
                }
            }
            SemanticTerminatorKindV1::Drop { place, .. } => {
                changed = place.local().index() == owner
            }
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => moved(discriminant)?,
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                moved(condition)?;
                match message {
                    SemanticAssertMessageV1::BoundsCheck { length, index } => {
                        moved(length)?;
                        moved(index)?;
                    }
                    SemanticAssertMessageV1::Overflow { left, right, .. } => {
                        moved(left)?;
                        moved(right)?;
                    }
                    SemanticAssertMessageV1::DivisionByZero(value)
                    | SemanticAssertMessageV1::RemainderByZero(value) => moved(value)?,
                    SemanticAssertMessageV1::MisalignedPointerDereference {
                        required_alignment,
                        found_alignment,
                    } => {
                        moved(required_alignment)?;
                        moved(found_alignment)?;
                    }
                    SemanticAssertMessageV1::NullPointerDereference
                    | SemanticAssertMessageV1::ResumedAfterReturn
                    | SemanticAssertMessageV1::ResumedAfterPanic => (),
                }
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => (),
        }
        if changed {
            budget.charge(1)?;
            changes.push(Site {
                block: block as u32,
                statement: body.statements().len() as u32,
            });
        }
    }
    Ok(changes)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    function: &SemanticFunctionDeclV1,
    candidates: &[SemanticBorrowCandidateV1],
    children: &[Vec<usize>],
    uses: &[Vec<Site>],
    mutable: &BTreeSet<usize>,
    root: usize,
    context_transfers: Option<&super::context_call_transfers::CheckedTransfers<'_>>,
    budget: &mut Budget,
) -> Result<bool> {
    prove_with_phase(function, candidates, children, uses, mutable, root, context_transfers, None, budget)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prove_with_phase(
    function: &SemanticFunctionDeclV1,
    candidates: &[SemanticBorrowCandidateV1],
    children: &[Vec<usize>],
    uses: &[Vec<Site>],
    mutable: &BTreeSet<usize>,
    root: usize,
    context_transfers: Option<&super::context_call_transfers::CheckedTransfers<'_>>,
    phase: Option<&super::phase_borrows::Facts<'_>>,
    budget: &mut Budget,
) -> Result<bool> {
    budget.profile.proof_calls = budget.profile.proof_calls.saturating_add(1);
    budget.scoped(FlowWorkStage::OwnerRoots, |budget| {
        prove_inner(function, candidates, children, uses, mutable, root, context_transfers, phase, budget)
    })
}

#[allow(clippy::too_many_arguments)]
fn prove_inner(
    function: &SemanticFunctionDeclV1,
    candidates: &[SemanticBorrowCandidateV1],
    children: &[Vec<usize>],
    uses: &[Vec<Site>],
    mutable: &BTreeSet<usize>,
    root: usize,
    context_transfers: Option<&super::context_call_transfers::CheckedTransfers<'_>>,
    phase: Option<&super::phase_borrows::Facts<'_>>,
    budget: &mut Budget,
) -> Result<bool> {
    let owned = candidates[root].source_type;
    let owner = candidates[root].source_local;
    let mut roots = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        budget.charge(1)?;
        if candidate.source_reference.is_none() && candidate.source_local == owner {
            if candidate.source_type != owned {
                return Ok(false);
            }
            budget.charge(1)?;
            roots.push(index);
        }
    }
    let mut order = Order::new(function, budget)?;
    let changes = owner_changes(function, owner, budget)?;
    budget.profile.stage = FlowWorkStage::Loans;
    let mut resumed = false;
    for &top in &roots {
        let Some(nodes) = descendants(top, candidates, children, budget)? else {
            return Ok(false);
        };
        for &node in &nodes {
            let candidate = candidates[node];
            budget.charge(1)?;
            if candidate.source_type != owned
                || candidate.consumers as usize != children[node].len() + uses[node].len()
                || !order.reachable(candidate.site, budget)?
            {
                return Ok(false);
            }
            if mutable.contains(&node) {
                resumed |= candidate.intrinsic_consumer;
                if candidate.value_alias {
                    let item = &function.blocks()[candidate.site.block as usize].statements()
                        [candidate.site.statement as usize];
                    if !matches!(item.kind(), SemanticStatementKindV1::Assign(a)
                        if matches!(a.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(_))))
                    {
                        let accepted = budget.scoped(FlowWorkStage::Transfers, |budget| {
                            if let Some(transfers) = context_transfers {
                                if transfers.accepts(function, candidate, budget)? { return Ok(true); }
                            }
                            match phase {
                                Some(phase) => phase.accepts_alias(function, candidate, budget),
                                None => Ok(false),
                            }
                        })?;
                        if !accepted {
                            return Ok(false);
                        }
                    }
                }
                if !children_are_ordered(
                    &children[node],
                    &uses[node],
                    candidates,
                    children,
                    uses,
                    mutable,
                    &mut order,
                    budget,
                )? {
                    return Ok(false);
                }
            } else {
                budget.charge(children[node].len())?;
                if children[node].iter().any(|child| mutable.contains(child)) {
                    return Ok(false);
                }
            }
            for point in uses[node]
                .iter()
                .copied()
                .chain(children[node].iter().map(|&c| candidates[c].site))
            {
                if !order.before(candidate.site, point, budget)? {
                    return Ok(false);
                }
            }
            for point in std::iter::once(candidate.site).chain(uses[node].iter().copied()) {
                for &change in &changes {
                    budget.charge(1)?;
                    if order.may_follow(candidates[top].site, change, budget)?
                        && order.may_follow(change, point, budget)?
                    {
                        return Ok(false);
                    }
                }
            }
        }
    }
    // No mutable-to-shared exception merely because a Context-shaped value exists.
    Ok(resumed
        && children_are_ordered(
            &roots,
            &[],
            candidates,
            children,
            uses,
            mutable,
            &mut order,
            budget,
        )?)
}
