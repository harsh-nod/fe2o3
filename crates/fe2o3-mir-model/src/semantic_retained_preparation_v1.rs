//! One-shot, caller-owned partial storage for the existing model analyses.
//!
//! Existing ordinary and metered return APIs remain unchanged. These additive
//! preparations retain every dynamic intermediate in the caller's object on
//! success, analysis failure, resource refusal and meter unwind. The caller must
//! retain all accepted ledger credits until this object drops. No refund occurs
//! here. Completed borrows are inert DATA, not source, ledger or compiler authority.
use super::*;
use std::mem;

#[cfg(test)]
#[path = "semantic_retained_preparation_v1_tests.rs"]
mod tests;

#[derive(Debug, Default, Eq, PartialEq)]
enum State {
    #[default]
    Fresh,
    Terminal,
    Complete,
}

fn occupied() -> SemanticOptionDominanceErrorV1 {
    SemanticOptionDominanceErrorV1::InvalidControlFlow(
        "retained model preparation is not a fresh empty owner",
    )
}

fn empty<T>(values: &Vec<T>) -> bool {
    values.is_empty() && values.capacity() == 0
}

#[derive(Debug, Default)]
struct RetainedDominators {
    entry: usize,
    successors: Vec<Vec<usize>>,
    predecessors: Vec<Vec<usize>>,
    visited: Vec<bool>,
    postorder: Vec<usize>,
    pending_rpo: Vec<(usize, bool)>,
    rpo_index: Vec<usize>,
    immediate: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
    preorder: Vec<usize>,
    subtree_end: Vec<usize>,
    pending_tree: Vec<(usize, bool)>,
}
impl RetainedDominators {
    fn pristine(&self) -> bool {
        self.entry == 0
            && empty(&self.successors)
            && empty(&self.predecessors)
            && empty(&self.visited)
            && empty(&self.postorder)
            && empty(&self.pending_rpo)
            && empty(&self.rpo_index)
            && empty(&self.immediate)
            && empty(&self.children)
            && empty(&self.preorder)
            && empty(&self.subtree_end)
            && empty(&self.pending_tree)
    }
}

/// One-shot producer DATA with its partial vector retained on every exit.
#[derive(Debug, Default)]
pub struct SemanticOptionProducerPreparationV1 {
    state: State,
    producers: Vec<SemanticOptionProducerV1>,
}

/// One-shot Option DATA and all intermediate physical storage.
#[derive(Debug, Default)]
pub struct SemanticOptionDominancePreparationV1 {
    state: State,
    definitions: Vec<u8>,
    dominators: RetainedDominators,
    discriminants: Vec<Vec<(usize, SemanticLocalIdV1)>>,
    availability: Vec<Option<SemanticOptionAvailabilityV1>>,
    targets: Vec<SemanticBlockIdV1>,
    availability_box: Option<Box<[Option<SemanticOptionAvailabilityV1>]>>,
    targets_box: Option<Box<[SemanticBlockIdV1]>>,
    preorder_box: Option<Box<[usize]>>,
    subtree_box: Option<Box<[usize]>>,
    completed: Option<SemanticOptionDominanceV1>,
}

/// One-shot enum-payload DATA and all intermediate physical storage.
#[derive(Debug, Default)]
pub struct SemanticEnumPayloadDominancePreparationV1 {
    state: State,
    definitions: Vec<u8>,
    dominators: RetainedDominators,
    discriminants: Vec<Vec<(usize, SemanticLocalIdV1)>>,
    availability: Vec<Vec<(u32, SemanticEnumPayloadAvailabilityV1)>>,
    targets: Vec<SemanticBlockIdV1>,
    availability_box: Option<Box<[Vec<(u32, SemanticEnumPayloadAvailabilityV1)>]>>,
    targets_box: Option<Box<[SemanticBlockIdV1]>>,
    preorder_box: Option<Box<[usize]>>,
    subtree_box: Option<Box<[usize]>>,
    completed: Option<SemanticEnumPayloadDominanceV1>,
}

// The unchanged analysis policy pays the existing algorithm's bounded lexical
// vertices. New owner/borrow/candidate/transfer/helper vertices are separate
// typed rows, not charged against the old 4096 policy term. Owner includes all
// retained Vec/Box headers (nested heap headers remain element reservations).
// This is an additive policy envelope, not a measurement of native stack bytes.
const RETAINED_FRAME_ROWS: usize = 34;

fn retained_frame_storage<M: SemanticEnumPayloadMeterV1, Facts, Owner>() -> Option<usize> {
    let inherited = metered_fact_frame_storage_v1::<M, Facts>()?;
    let rows: [usize; RETAINED_FRAME_ROWS] = [
        inherited,
        size_of::<Owner>(),
        size_of::<(&mut Owner, &SemanticFunctionDeclV1, &mut M, Option<usize>)>(),
        size_of::<(
            &[SemanticOptionProducerV1],
            &[SemanticCallableDeclV1],
            &[SemanticTypeDeclV1],
        )>(),
        size_of::<(&mut WorkBudgetV1<'_>, &mut RetainedDominators, &mut Vec<u8>)>(),
        // Both retained-into result transfer sites plus completed DATA transfer.
        size_of::<Result<(), SemanticOptionDominanceErrorV1>>(),
        size_of::<Result<(), SemanticOptionDominanceErrorV1>>(),
        size_of::<Result<(), SemanticEnumPayloadMeteredErrorV1<M::Error>>>(),
        size_of::<Result<(), SemanticEnumPayloadMeteredErrorV1<M::Error>>>(),
        size_of::<Option<Facts>>(),
        size_of::<Facts>(),
        // Additional aliases replacing original owning local Vec bindings.
        size_of::<(&mut Vec<Vec<usize>>, &mut Vec<Vec<usize>>, &mut Vec<bool>)>(),
        size_of::<(&mut Vec<usize>, &mut Vec<(usize, bool)>, &mut Vec<usize>)>(),
        size_of::<(
            &mut Vec<Option<usize>>,
            &mut Vec<Vec<usize>>,
            &mut Vec<usize>,
        )>(),
        size_of::<(&mut Vec<usize>, &mut Vec<(usize, bool)>)>(),
        size_of::<(
            &Vec<u8>,
            &RetainedDominators,
            &mut Vec<Vec<(usize, SemanticLocalIdV1)>>,
        )>(),
        // New fill/nested/box helper vertices, instantiated at every used type.
        fill_frame::<bool>(),
        fill_frame::<usize>(),
        fill_frame::<Option<usize>>(),
        fill_frame::<Option<SemanticOptionAvailabilityV1>>(),
        nested_frame::<usize>(),
        nested_frame::<(usize, SemanticLocalIdV1)>(),
        nested_frame::<(u32, SemanticEnumPayloadAvailabilityV1)>(),
        box_frame::<Option<SemanticOptionAvailabilityV1>>(),
        box_frame::<Vec<(u32, SemanticEnumPayloadAvailabilityV1)>>(),
        box_frame::<SemanticBlockIdV1>(),
        box_frame::<usize>(),
        box_frame::<usize>(),
        // Freshness/getter/empty helpers have only borrowed DATA and scalar state.
        size_of::<(&Owner, &State, bool, SemanticOptionDominanceErrorV1)>(),
        size_of::<(&Vec<usize>, bool, usize)>(),
        size_of::<(&SemanticOptionDominancePreparationV1, bool)>(),
        size_of::<(&SemanticEnumPayloadDominancePreparationV1, bool)>(),
        size_of::<(&RetainedDominators, bool)>(),
        // Formula construction/array-to-iterator/result transfer is a NEW
        // vertex; its own typed row is not spent against the inherited policy.
        size_of::<(
            [usize; RETAINED_FRAME_ROWS],
            [usize; RETAINED_FRAME_ROWS],
            std::array::IntoIter<usize, RETAINED_FRAME_ROWS>,
            usize,
            usize,
            Option<usize>,
            Option<usize>,
        )>(),
    ];
    rows.into_iter().try_fold(0usize, usize::checked_add)
}

fn fill_frame<T>() -> usize {
    size_of::<(
        &mut Vec<T>,
        usize,
        T,
        &mut WorkBudgetV1<'_>,
        Result<(), SemanticOptionDominanceErrorV1>,
    )>()
}
fn nested_frame<T>() -> usize {
    size_of::<(
        &mut Vec<Vec<T>>,
        usize,
        &mut WorkBudgetV1<'_>,
        Result<(), SemanticOptionDominanceErrorV1>,
    )>()
}
fn box_frame<T>() -> usize {
    size_of::<(
        &mut Vec<T>,
        &mut Option<Box<[T]>>,
        &mut WorkBudgetV1<'_>,
        usize,
        Vec<T>,
        Box<[T]>,
        Option<Box<[T]>>,
        Result<(), SemanticOptionDominanceErrorV1>,
    )>()
}

fn retained_fill<T: Clone>(
    values: &mut Vec<T>,
    count: usize,
    value: T,
    budget: &mut WorkBudgetV1<'_>,
) -> Result<(), SemanticOptionDominanceErrorV1> {
    budget.extra(count)?;
    budget.reserve(values, count)?;
    values.resize(count, value);
    Ok(())
}
fn retained_nested<T>(
    values: &mut Vec<Vec<T>>,
    count: usize,
    budget: &mut WorkBudgetV1<'_>,
) -> Result<(), SemanticOptionDominanceErrorV1> {
    budget.extra(count)?;
    budget.reserve(values, count)?;
    values.resize_with(count, Vec::new);
    Ok(())
}
fn retained_box<T>(
    values: &mut Vec<T>,
    destination: &mut Option<Box<[T]>>,
    budget: &mut WorkBudgetV1<'_>,
) -> Result<(), SemanticOptionDominanceErrorV1> {
    // The source stays attached across EVERY fallible work/storage debit. Only
    // after those return successfully is ownership transferred without any
    // intervening meter callback or fallible semantic operation.
    if values.len() != values.capacity() {
        budget.extra(values.len())?;
        let bytes = budget.product(values.len(), size_of::<T>())?;
        budget.reserve_bytes(bytes)?;
    }
    *destination = Some(mem::take(values).into_boxed_slice());
    Ok(())
}

impl SemanticOptionProducerPreparationV1 {
    /// Creates empty caller-owned storage without allocating.
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs once on the supplied meter and retains partial storage on every exit.
    ///
    /// A failed or unwound owner cannot be retried, even on another meter.
    /// The caller owns the ledger's accepted credits until this owner drops.
    /// Source authenticity, cross-analysis ordering and backend readiness remain
    /// the caller's responsibility; this method produces inert model DATA only.
    pub fn prepare_into<M: SemanticEnumPayloadMeterV1>(
        &mut self,
        function: &SemanticFunctionDeclV1,
        callables: &[SemanticCallableDeclV1],
        meter: &mut M,
    ) -> Result<(), SemanticEnumPayloadMeteredErrorV1<M::Error>> {
        let fresh = self.state == State::Fresh && empty(&self.producers);
        self.state = State::Terminal;
        if !fresh {
            return Err(SemanticEnumPayloadMeteredErrorV1::Analysis(occupied()));
        }
        let Some(frame_storage) =
            retained_frame_storage::<M, Vec<SemanticOptionProducerV1>, Self>()
        else {
            return Err(SemanticEnumPayloadMeteredErrorV1::Arithmetic);
        };
        let mut adapter = Adapter {
            original: meter,
            failure: None,
        };
        let result = {
            let mut budget = WorkBudgetV1 {
                used: 0,
                meter: Some(&mut adapter),
            };
            match budget.reserve_bytes(frame_storage) {
                Ok(()) => retained_producers(function, callables, &mut self.producers, &mut budget),
                Err(error) => Err(error),
            }
        };
        match adapter.failure {
            Some(error) => Err(error),
            None => match result {
                Ok(()) => {
                    self.state = State::Complete;
                    Ok(())
                }
                Err(error) => Err(SemanticEnumPayloadMeteredErrorV1::Analysis(error)),
            },
        }
    }

    /// Borrows completed inert DATA only; partial candidates never escape.
    pub fn completed(&self) -> Option<&[SemanticOptionProducerV1]> {
        if self.state == State::Complete {
            Some(self.producers.as_slice())
        } else {
            None
        }
    }
}

impl SemanticOptionDominancePreparationV1 {
    /// Creates empty caller-owned storage without allocating.
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs once on the supplied meter and retains partial storage on every exit.
    ///
    /// A failed or unwound owner cannot be retried, even on another meter.
    /// The caller owns the ledger's accepted credits until this owner drops.
    /// Source authenticity, cross-analysis ordering and backend readiness remain
    /// the caller's responsibility; this method produces inert model DATA only.
    pub fn prepare_into<M: SemanticEnumPayloadMeterV1>(
        &mut self,
        function: &SemanticFunctionDeclV1,
        producers: &[SemanticOptionProducerV1],
        meter: &mut M,
    ) -> Result<(), SemanticEnumPayloadMeteredErrorV1<M::Error>> {
        let fresh = self.state == State::Fresh && self.pristine();
        self.state = State::Terminal;
        if !fresh {
            return Err(SemanticEnumPayloadMeteredErrorV1::Analysis(occupied()));
        }
        let Some(frame_storage) = retained_frame_storage::<M, SemanticOptionDominanceV1, Self>()
        else {
            return Err(SemanticEnumPayloadMeteredErrorV1::Arithmetic);
        };
        let mut adapter = Adapter {
            original: meter,
            failure: None,
        };
        let result = {
            let mut budget = WorkBudgetV1 {
                used: 0,
                meter: Some(&mut adapter),
            };
            match budget.reserve_bytes(frame_storage) {
                Ok(()) => retained_option(function, producers, self, &mut budget),
                Err(error) => Err(error),
            }
        };
        match adapter.failure {
            Some(error) => Err(error),
            None => match result {
                Ok(()) => {
                    self.state = State::Complete;
                    Ok(())
                }
                Err(error) => Err(SemanticEnumPayloadMeteredErrorV1::Analysis(error)),
            },
        }
    }

    /// Borrows completed inert DATA only; partial candidates never escape.
    pub fn completed(&self) -> Option<&SemanticOptionDominanceV1> {
        if self.state == State::Complete {
            self.completed.as_ref()
        } else {
            None
        }
    }

    fn pristine(&self) -> bool {
        empty(&self.definitions)
            && self.dominators.pristine()
            && empty(&self.discriminants)
            && empty(&self.availability)
            && empty(&self.targets)
            && self.availability_box.is_none()
            && self.targets_box.is_none()
            && self.preorder_box.is_none()
            && self.subtree_box.is_none()
            && self.completed.is_none()
    }
}

impl SemanticEnumPayloadDominancePreparationV1 {
    /// Creates empty caller-owned storage without allocating.
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs once on the supplied meter and retains partial storage on every exit.
    ///
    /// A failed or unwound owner cannot be retried, even on another meter.
    /// The caller owns the ledger's accepted credits until this owner drops.
    /// Source authenticity, cross-analysis ordering and backend readiness remain
    /// the caller's responsibility; this method produces inert model DATA only.
    pub fn prepare_into<M: SemanticEnumPayloadMeterV1>(
        &mut self,
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        meter: &mut M,
    ) -> Result<(), SemanticEnumPayloadMeteredErrorV1<M::Error>> {
        let fresh = self.state == State::Fresh && self.pristine();
        self.state = State::Terminal;
        if !fresh {
            return Err(SemanticEnumPayloadMeteredErrorV1::Analysis(occupied()));
        }
        let Some(frame_storage) =
            retained_frame_storage::<M, SemanticEnumPayloadDominanceV1, Self>()
        else {
            return Err(SemanticEnumPayloadMeteredErrorV1::Arithmetic);
        };
        let mut adapter = Adapter {
            original: meter,
            failure: None,
        };
        let result = {
            let mut budget = WorkBudgetV1 {
                used: 0,
                meter: Some(&mut adapter),
            };
            match budget.reserve_bytes(frame_storage) {
                Ok(()) => retained_enum(function, types, self, &mut budget),
                Err(error) => Err(error),
            }
        };
        match adapter.failure {
            Some(error) => Err(error),
            None => match result {
                Ok(()) => {
                    self.state = State::Complete;
                    Ok(())
                }
                Err(error) => Err(SemanticEnumPayloadMeteredErrorV1::Analysis(error)),
            },
        }
    }

    /// Borrows completed inert DATA only; partial candidates never escape.
    pub fn completed(&self) -> Option<&SemanticEnumPayloadDominanceV1> {
        if self.state == State::Complete {
            self.completed.as_ref()
        } else {
            None
        }
    }

    fn pristine(&self) -> bool {
        empty(&self.definitions)
            && self.dominators.pristine()
            && empty(&self.discriminants)
            && empty(&self.availability)
            && empty(&self.targets)
            && self.availability_box.is_none()
            && self.targets_box.is_none()
            && self.preorder_box.is_none()
            && self.subtree_box.is_none()
            && self.completed.is_none()
    }
}

// The following algorithms have exact frozen original inverses.
fn retained_producers(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    producers: &mut Vec<SemanticOptionProducerV1>,
    budget: &mut WorkBudgetV1<'_>,
) -> Result<(), SemanticOptionDominanceErrorV1> {
    budget.reserve(producers, function.blocks().len())?;
    for block in function.blocks() {
        // Classifier and exact destination checks have constant work. This is
        // external-only admission: collection had no legacy diagnostic counter.
        budget.extra(8)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
            callables.get(call.callee().index() as usize)
        else {
            continue;
        };
        if let Some(producer) = SemanticOptionProducerV1::from_compiler_intrinsic(operation, call)?
        {
            budget.push(producers, producer)?;
        }
    }
    Ok(())
}

fn retained_definitions(
    function: &SemanticFunctionDeclV1,
    definitions: &mut Vec<u8>,
    budget: &mut WorkBudgetV1,
) -> Result<(), SemanticOptionDominanceErrorV1> {
    budget.charge(function.locals().len())?;
    budget.reserve(definitions, function.locals().len())?;
    definitions.extend(
        function
            .locals()
            .iter()
            .map(|local| u8::from(local.role().is_entry_argument())),
    );
    let mut record = |place: &SemanticPlaceV1| {
        let Some(slot) = definitions.get_mut(place.local().index() as usize) else {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "a semantic definition is outside the local table",
            ));
        };
        *slot = slot.saturating_add(1);
        Ok(())
    };
    for block in function.blocks() {
        budget.charge(block.statements().len().saturating_add(1))?;
        for statement in block.statements() {
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => record(assignment.destination())?,
                SemanticStatementKindV1::Store(store) => record(store.destination())?,
                SemanticStatementKindV1::AtomicRmw(atomic) => record(atomic.destination())?,
                SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                    record(atomic.destination())?
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => record(place)?,
                SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Assume(_)
                | SemanticStatementKindV1::Nop => {}
            }
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(destination) = call.destination()
        {
            record(destination.place())?;
        }
    }
    Ok(())
}

impl RetainedDominators {
    fn prepare_into(
        storage: &mut Self,
        function: &SemanticFunctionDeclV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<(), SemanticOptionDominanceErrorV1> {
        let block_count = function.blocks().len();
        let entry = function.entry().index() as usize;
        if block_count == 0 || entry >= block_count {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "the semantic CFG has no valid entry block",
            ));
        }
        storage.entry = entry;
        budget.charge(block_count)?;
        retained_nested(&mut storage.successors, block_count, budget)?;
        let successors = &mut storage.successors;
        retained_nested(&mut storage.predecessors, block_count, budget)?;
        let predecessors = &mut storage.predecessors;
        for (source, block) in function.blocks().iter().enumerate() {
            budget.extra(1)?;
            block
                .terminator()
                .kind()
                .try_for_each_edge::<SemanticOptionDominanceErrorV1>(|edge| {
                    budget.charge(1)?;
                    let target = edge.target().index() as usize;
                    if target >= block_count {
                        return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                            "a semantic CFG edge is outside the block table",
                        ));
                    }
                    budget.push(&mut successors[source], target)?;
                    budget.push(&mut predecessors[target], source)?;
                    Ok(())
                })?;
        }

        retained_fill(&mut storage.visited, block_count, false, budget)?;
        let visited = &mut storage.visited;
        let postorder = &mut storage.postorder;
        budget.reserve(postorder, block_count)?;
        let pending = &mut storage.pending_rpo;
        budget.push(pending, (entry, false))?;
        while let Some((block, finish)) = pending.pop() {
            budget.charge(1)?;
            if finish {
                budget.push(postorder, block)?;
            } else if !visited[block] {
                visited[block] = true;
                budget.push(pending, (block, true))?;
                for successor in successors[block].iter().rev() {
                    budget.charge(1)?;
                    if !visited[*successor] {
                        budget.push(pending, (*successor, false))?;
                    }
                }
            }
        }
        budget.extra(postorder.len())?;
        postorder.reverse();
        retained_fill(&mut storage.rpo_index, block_count, usize::MAX, budget)?;
        let rpo_index = &mut storage.rpo_index;
        for (index, block) in postorder.iter().copied().enumerate() {
            budget.extra(1)?;
            rpo_index[block] = index;
        }
        retained_fill(&mut storage.immediate, block_count, None, budget)?;
        let immediate = &mut storage.immediate;
        immediate[entry] = Some(entry);
        loop {
            budget.charge(1)?;
            let mut changed = false;
            for block in postorder.iter().copied().skip(1) {
                budget.charge(1)?;
                budget.extra(predecessors[block].len())?;
                let mut processed = predecessors[block]
                    .iter()
                    .copied()
                    .filter(|predecessor| immediate[*predecessor].is_some());
                let Some(mut next) = processed.next() else {
                    continue;
                };
                for predecessor in processed {
                    budget.charge(1)?;
                    next =
                        intersect_dominator_paths(predecessor, next, immediate, rpo_index, budget)?;
                }
                if immediate[block] != Some(next) {
                    immediate[block] = Some(next);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        immediate[entry] = None;
        retained_nested(&mut storage.children, block_count, budget)?;
        let children = &mut storage.children;
        for (block, parent) in immediate.iter().copied().enumerate() {
            budget.extra(1)?;
            if let Some(parent) = parent {
                budget.push(&mut children[parent], block)?;
            }
        }
        retained_fill(&mut storage.preorder, block_count, usize::MAX, budget)?;
        let preorder = &mut storage.preorder;
        retained_fill(&mut storage.subtree_end, block_count, usize::MAX, budget)?;
        let subtree_end = &mut storage.subtree_end;
        let mut clock = 0_usize;
        let pending = &mut storage.pending_tree;
        budget.push(pending, (entry, false))?;
        while let Some((block, finish)) = pending.pop() {
            budget.charge(1)?;
            if finish {
                subtree_end[block] = clock;
            } else {
                preorder[block] = clock;
                clock += 1;
                budget.push(pending, (block, true))?;
                for child in children[block].iter().rev() {
                    budget.push(pending, (*child, false))?;
                }
            }
        }
        Ok(())
    }

    fn has_unique_predecessor(&self, block: usize, predecessor: usize) -> bool {
        // Entry also has the implicit edge from the function invocation.
        block != self.entry
            && matches!(
                self.predecessors.get(block).map(Vec::as_slice),
                Some([exact]) if *exact == predecessor
            )
    }

    fn is_reachable(&self, block: usize) -> bool {
        self.preorder.get(block).copied() != Some(usize::MAX)
    }

    fn dominates(&self, dominator: usize, block: usize) -> bool {
        dominates_with_intervals(&self.preorder, &self.subtree_end, dominator, block)
    }
}

fn retained_option(
    function: &SemanticFunctionDeclV1,
    producers: &[SemanticOptionProducerV1],
    storage: &mut SemanticOptionDominancePreparationV1,
    budget: &mut WorkBudgetV1<'_>,
) -> Result<(), SemanticOptionDominanceErrorV1> {
    let local_count = function.locals().len();
    retained_definitions(function, &mut storage.definitions, budget)?;
    let definitions = &storage.definitions;
    RetainedDominators::prepare_into(&mut storage.dominators, function, budget)?;
    let dominators = &storage.dominators;
    retained_nested(&mut storage.discriminants, local_count, budget)?;
    let discriminants_by_option = &mut storage.discriminants;
    for (block_index, block) in function.blocks().iter().enumerate() {
        budget.charge(block.statements().len().saturating_add(1))?;
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Discriminant(place) = assignment.value().kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() || !place.projections().is_empty()
            {
                continue;
            }
            let Some(bindings) = discriminants_by_option.get_mut(place.local().index() as usize)
            else {
                return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                    "an Option discriminator source is outside the local table",
                ));
            };
            budget.push(bindings, (block_index, assignment.destination().local()))?;
        }
    }

    retained_fill(&mut storage.availability, local_count, None, budget)?;
    let availability_by_local = &mut storage.availability;
    let some_targets = &mut storage.targets;
    budget.reserve(some_targets, producers.len())?;
    for producer in producers {
        budget.charge(1)?;
        let destination_index = producer.option_local().index() as usize;
        if definitions.get(destination_index).copied() != Some(1) {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability local does not have one exact producer",
            ));
        }
        let [(switch_block, discriminator)] = discriminants_by_option
            .get(destination_index)
            .map(Vec::as_slice)
            .ok_or(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "an Option capability destination is outside the local table",
            ))?
        else {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability does not have one exact discriminant binding",
            ));
        };
        if definitions.get(discriminator.index() as usize).copied() != Some(1) {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability discriminator does not have one exact definition",
            ));
        }
        if !dominators.dominates(producer.continuation().index() as usize, *switch_block) {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability discriminator is not dominated by its producer continuation",
            ));
        }
        let switch = function.blocks().get(*switch_block).ok_or(
            SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "an Option discriminator block is outside the block table",
            ),
        )?;
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = switch.terminator().kind()
        else {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability discriminator is not consumed by its defining block",
            ));
        };
        if exact_operand_local(discriminant) != Some(*discriminator) {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability switch is not bound to its unique discriminator",
            ));
        }
        let some_target = match targets.values() {
            [target] => match target.value() {
                0 => targets.otherwise().target(),
                1 => target.edge().target(),
                _ => {
                    return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                        "an Option capability switch has no exact Some edge",
                    ));
                }
            },
            [zero, one] if zero.value() == 0 && one.value() == 1 => one.edge().target(),
            _ => {
                return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                    "an Option capability switch is not an exact 0/1 branch",
                ));
            }
        };
        if !dominators.is_reachable(some_target.index() as usize) {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "the authenticated Some edge is unreachable",
            ));
        }
        if !dominators.has_unique_predecessor(some_target.index() as usize, *switch_block) {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "an Option capability Some target is not uniquely controlled by its exact branch",
            ));
        }
        let slot = availability_by_local.get_mut(destination_index).ok_or(
            SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "an Option capability destination is outside the local table",
            ),
        )?;
        if slot
            .replace(SemanticOptionAvailabilityV1(some_targets.len()))
            .is_some()
        {
            return Err(SemanticOptionDominanceErrorV1::InexactCapability(
                "one local has multiple Option capability producers",
            ));
        }
        budget.push(some_targets, some_target)?;
    }
    retained_box(
        &mut storage.availability,
        &mut storage.availability_box,
        budget,
    )?;
    retained_box(&mut storage.targets, &mut storage.targets_box, budget)?;
    retained_box(
        &mut storage.dominators.preorder,
        &mut storage.preorder_box,
        budget,
    )?;
    retained_box(
        &mut storage.dominators.subtree_end,
        &mut storage.subtree_box,
        budget,
    )?;
    // All four candidates are attached before these infallible private transfers.
    // No meter/user callback or fallible semantic operation occurs below.
    storage.completed = Some(SemanticOptionDominanceV1 {
        availability_by_local: storage
            .availability_box
            .take()
            .expect("attached availability"),
        some_targets: storage.targets_box.take().expect("attached targets"),
        dominator_preorder: storage.preorder_box.take().expect("attached preorder"),
        dominator_subtree_end: storage.subtree_box.take().expect("attached subtree"),
        work_units: budget.used,
    });
    Ok(())
}

fn retained_enum(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    storage: &mut SemanticEnumPayloadDominancePreparationV1,
    budget: &mut WorkBudgetV1<'_>,
) -> Result<(), SemanticOptionDominanceErrorV1> {
    let local_count = function.locals().len();
    retained_definitions(function, &mut storage.definitions, budget)?;
    let definitions = &storage.definitions;
    RetainedDominators::prepare_into(&mut storage.dominators, function, budget)?;
    let dominators = &storage.dominators;
    retained_nested(&mut storage.discriminants, local_count, budget)?;
    let discriminants_by_enum = &mut storage.discriminants;
    for (block_index, block) in function.blocks().iter().enumerate() {
        budget.charge(block.statements().len().saturating_add(1))?;
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Discriminant(place) = assignment.value().kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() || !place.projections().is_empty()
            {
                continue;
            }
            let Some(bindings) = discriminants_by_enum.get_mut(place.local().index() as usize)
            else {
                return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                    "an enum discriminator source is outside the local table",
                ));
            };
            budget.push(bindings, (block_index, assignment.destination().local()))?;
        }
    }

    retained_nested(&mut storage.availability, local_count, budget)?;
    let availability_by_local = &mut storage.availability;
    let payload_targets = &mut storage.targets;
    budget.reserve(payload_targets, local_count)?;
    for (local_index, bindings) in discriminants_by_enum.iter().enumerate() {
        budget.extra(1)?;
        let [(switch_block, discriminator)] = bindings.as_slice() else {
            continue;
        };
        if definitions.get(local_index).copied() != Some(1)
            || definitions.get(discriminator.index() as usize).copied() != Some(1)
        {
            continue;
        }
        let Some(local) = function.locals().get(local_index) else {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "an enum payload local is outside the local table",
            ));
        };
        let Some(ty) = types.get(local.ty().index() as usize) else {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "an enum payload type is outside the type table",
            ));
        };
        let SemanticTypeShapeV1::Enum { variants, .. } = ty.shape() else {
            continue;
        };
        let Some(block) = function.blocks().get(*switch_block) else {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "an enum payload switch is outside the block table",
            ));
        };
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = block.terminator().kind()
        else {
            continue;
        };
        if exact_operand_local(discriminant) != Some(*discriminator) {
            continue;
        }
        budget.charge(
            variants
                .len()
                .saturating_mul(targets.values().len().saturating_add(1)),
        )?;
        let mut otherwise_variant = None;
        let mut otherwise_is_ambiguous = false;
        for (variant_index, variant) in variants.iter().enumerate() {
            if variant.is_uninhabited()
                || targets
                    .values()
                    .iter()
                    .any(|target| target.value() == variant.discriminant())
            {
                continue;
            }
            if otherwise_variant.replace(variant_index as u32).is_some() {
                otherwise_is_ambiguous = true;
                break;
            }
        }
        // The second variant/target scan is separately admitted externally.
        let second_scan = variants
            .len()
            .checked_mul(
                targets
                    .values()
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| budget.fail(true))?,
            )
            .ok_or_else(|| budget.fail(true))?;
        budget.extra(second_scan)?;
        for (variant_index, variant) in variants.iter().enumerate() {
            if variant.is_uninhabited() {
                continue;
            }
            let explicit = targets
                .values()
                .iter()
                .find(|target| target.value() == variant.discriminant())
                .map(|target| target.edge().target());
            let target = explicit.or_else(|| {
                (!otherwise_is_ambiguous && otherwise_variant == Some(variant_index as u32))
                    .then(|| targets.otherwise().target())
            });
            let Some(target) = target else {
                continue;
            };
            // A shared predecessor block does not establish which edge
            // arrived, including an otherwise edge with no variant fact.
            if !enum_payload_target_is_unique_v1(targets, target, budget)? {
                continue;
            }
            if !dominators.is_reachable(target.index() as usize)
                || !dominators.has_unique_predecessor(target.index() as usize, *switch_block)
            {
                continue;
            }
            let availability = SemanticEnumPayloadAvailabilityV1(payload_targets.len());
            budget.push(payload_targets, target)?;
            budget.push(
                &mut availability_by_local[local_index],
                (variant_index as u32, availability),
            )?;
        }
    }
    retained_box(
        &mut storage.availability,
        &mut storage.availability_box,
        budget,
    )?;
    retained_box(&mut storage.targets, &mut storage.targets_box, budget)?;
    retained_box(
        &mut storage.dominators.preorder,
        &mut storage.preorder_box,
        budget,
    )?;
    retained_box(
        &mut storage.dominators.subtree_end,
        &mut storage.subtree_box,
        budget,
    )?;
    // All four candidates are attached before these infallible private transfers.
    // No meter/user callback or fallible semantic operation occurs below.
    storage.completed = Some(SemanticEnumPayloadDominanceV1 {
        availability_by_local: storage
            .availability_box
            .take()
            .expect("attached availability"),
        payload_targets: storage.targets_box.take().expect("attached targets"),
        dominator_preorder: storage.preorder_box.take().expect("attached preorder"),
        dominator_subtree_end: storage.subtree_box.take().expect("attached subtree"),
        work_units: budget.used,
    });
    Ok(())
}
