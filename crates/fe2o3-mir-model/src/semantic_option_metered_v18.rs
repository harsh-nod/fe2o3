//! Donor V18 live accounting over the existing inert Option fact identities.
//! MAIN's V1 preparation/accounting protocol remains in its original module.
use super::{
    Error, MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1, SemanticBlockIdV1, SemanticCallableDeclV1,
    SemanticFunctionDeclV1, SemanticLocalIdV1, SemanticOptionAvailabilityV1,
    SemanticOptionDominanceErrorV1, SemanticOptionDominanceV1, SemanticOptionProducerV1,
    SemanticPlaceV1, SemanticRvalueKindV1, SemanticStatementKindV1, SemanticTerminatorKindV1,
    dominates_with_intervals, exact_operand_local, fmt,
};

/// Keeps a live caller refusal distinct from an inert dominance-analysis error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticOptionDominanceMeteredErrorV18<E> {
    /// The unchanged analysis rejected the original semantic model.
    Analysis(SemanticOptionDominanceErrorV1),
    /// The exact caller work/storage refusal, without erasing its payload.
    Meter(E),
}

impl<E: fmt::Display> fmt::Display for SemanticOptionDominanceMeteredErrorV18<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analysis(error) => error.fmt(formatter),
            Self::Meter(error) => write!(formatter, "semantic dominance meter: {error}"),
        }
    }
}
impl<E: Error + 'static> Error for SemanticOptionDominanceMeteredErrorV18<E> {}

trait DominanceMeterV18 {
    fn work(&mut self, amount: usize, logical: bool) -> Result<(), SemanticOptionDominanceErrorV1>;
    fn storage(&mut self, amount: usize) -> Result<(), SemanticOptionDominanceErrorV1>;
}

struct DominanceMeterAdapterV18<'a, M: crate::SemanticAssertionMeterV1> {
    meter: &'a mut M,
    error: Option<M::Error>,
}

impl<M: crate::SemanticAssertionMeterV1> DominanceMeterV18 for DominanceMeterAdapterV18<'_, M> {
    fn work(&mut self, amount: usize, logical: bool) -> Result<(), SemanticOptionDominanceErrorV1> {
        let result = if logical {
            self.meter.charge_legacy_work(amount)
        } else {
            self.meter.charge_work(amount)
        };
        result.map_err(|error| {
            self.error.get_or_insert(error);
            SemanticOptionDominanceErrorV1::Storage
        })
    }
    fn storage(&mut self, amount: usize) -> Result<(), SemanticOptionDominanceErrorV1> {
        self.meter.reserve_storage(amount).map_err(|error| {
            self.error.get_or_insert(error);
            SemanticOptionDominanceErrorV1::Storage
        })
    }
}

fn with_dominance_meter_v18<T, M: crate::SemanticAssertionMeterV1, F>(
    meter: &mut M,
    action: F,
) -> Result<T, SemanticOptionDominanceMeteredErrorV18<M::Error>>
where
    F: FnOnce(&mut WorkBudgetV1<'_>) -> Result<T, SemanticOptionDominanceErrorV1>,
{
    let bytes = std::mem::size_of::<DominanceMeterAdapterV18<'_, M>>()
        .checked_add(std::mem::size_of::<WorkBudgetV1<'_>>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<F>()))
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<Result<T, SemanticOptionDominanceErrorV1>>())
        })
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<
                Result<T, SemanticOptionDominanceMeteredErrorV18<M::Error>>,
            >())
        })
        .ok_or(SemanticOptionDominanceMeteredErrorV18::Analysis(
            SemanticOptionDominanceErrorV1::Storage,
        ))?;
    meter
        .reserve_storage(bytes)
        .map_err(SemanticOptionDominanceMeteredErrorV18::Meter)?;
    let mut adapter = DominanceMeterAdapterV18 { meter, error: None };
    let result = action(&mut WorkBudgetV1 {
        used: 0,
        meter: Some(&mut adapter),
    });
    match adapter.error {
        Some(error) => Err(SemanticOptionDominanceMeteredErrorV18::Meter(error)),
        None => result.map_err(SemanticOptionDominanceMeteredErrorV18::Analysis),
    }
}

/// Collects the same original-source inventory with live fallible accounting.
///
/// The second result is the checked retained header and actual vector capacity.
/// It excludes dropped scratch and confers no authority over optimized output.
pub fn semantic_option_producers_with_meter_v18<M: crate::SemanticAssertionMeterV1>(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    meter: &mut M,
) -> Result<(Vec<SemanticOptionProducerV1>, usize), SemanticOptionDominanceMeteredErrorV18<M::Error>>
{
    with_dominance_meter_v18(meter, |budget| {
        let producers = semantic_option_producers_core_v18(function, callables, budget)?;
        let retained = std::mem::size_of::<Vec<SemanticOptionProducerV1>>()
            .checked_add(
                producers
                    .capacity()
                    .checked_mul(std::mem::size_of::<SemanticOptionProducerV1>())
                    .ok_or(SemanticOptionDominanceErrorV1::Storage)?,
            )
            .ok_or(SemanticOptionDominanceErrorV1::Storage)?;
        Ok((producers, retained))
    })
}

fn semantic_option_producers_core_v18(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    budget: &mut WorkBudgetV1<'_>,
) -> Result<Vec<SemanticOptionProducerV1>, SemanticOptionDominanceErrorV1> {
    let mut producers = budget.new_vec()?;
    budget.reserve(&mut producers, function.blocks().len(), false)?;
    for block in function.blocks() {
        budget.source_work(1)?;
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
            budget.push(&mut producers, producer)?;
        }
    }
    Ok(producers)
}

impl SemanticOptionDominanceV1 {
    /// Runs the unchanged inert analysis against a live work/storage meter.
    ///
    /// Returns the report and its checked retained bytes. Original-source facts
    /// do not certify an optimized owner or replace output correspondence.
    pub fn analyze_with_meter_v18<M: crate::SemanticAssertionMeterV1>(
        function: &SemanticFunctionDeclV1,
        producers: &[SemanticOptionProducerV1],
        meter: &mut M,
    ) -> Result<(Self, usize), SemanticOptionDominanceMeteredErrorV18<M::Error>> {
        with_dominance_meter_v18(meter, |budget| {
            let report = Self::analyze_core_v18(function, producers, budget)?;
            let bytes = std::mem::size_of::<Self>()
                .checked_add(std::mem::size_of_val(&*report.availability_by_local))
                .and_then(|bytes| bytes.checked_add(std::mem::size_of_val(&*report.some_targets)))
                .and_then(|bytes| {
                    bytes.checked_add(std::mem::size_of_val(&*report.dominator_preorder))
                })
                .and_then(|bytes| {
                    bytes.checked_add(std::mem::size_of_val(&*report.dominator_subtree_end))
                })
                .ok_or(SemanticOptionDominanceErrorV1::Storage)?;
            Ok((report, bytes))
        })
    }

    fn analyze_core_v18(
        function: &SemanticFunctionDeclV1,
        producers: &[SemanticOptionProducerV1],
        budget: &mut WorkBudgetV1<'_>,
    ) -> Result<Self, SemanticOptionDominanceErrorV1> {
        let local_count = function.locals().len();
        budget.storage(std::mem::size_of::<Self>())?;
        let definitions = local_definition_counts(function, budget)?;
        let dominators = DominatorIntervalsV1::analyze(function, budget)?;
        let mut discriminants_by_option = budget.filled(local_count, Vec::new())?;
        for (block_index, block) in function.blocks().iter().enumerate() {
            budget.charge(block.statements().len().saturating_add(1))?;
            for statement in block.statements() {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                let SemanticRvalueKindV1::Discriminant(place) = assignment.value().kind() else {
                    continue;
                };
                if !assignment.destination().projections().is_empty()
                    || !place.projections().is_empty()
                {
                    continue;
                }
                let Some(bindings) =
                    discriminants_by_option.get_mut(place.local().index() as usize)
                else {
                    return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                        "an Option discriminator source is outside the local table",
                    ));
                };
                budget.reserve(bindings, 1, false)?;
                budget.push(bindings, (block_index, assignment.destination().local()))?;
            }
        }

        let mut availability_by_local = budget.filled(local_count, None)?;
        let mut some_targets = budget.new_vec()?;
        budget.reserve(&mut some_targets, producers.len(), false)?;
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
            budget.push(&mut some_targets, some_target)?;
        }
        Ok(Self {
            availability_by_local: budget.boxed(availability_by_local)?,
            some_targets: budget.boxed(some_targets)?,
            dominator_preorder: budget.boxed(dominators.preorder)?,
            dominator_subtree_end: budget.boxed(dominators.subtree_end)?,
            work_units: budget.used,
        })
    }
}

#[derive(Default)]
struct WorkBudgetV1<'m> {
    used: usize,
    meter: Option<&'m mut dyn DominanceMeterV18>,
}

impl WorkBudgetV1<'_> {
    fn charge(&mut self, amount: usize) -> Result<(), SemanticOptionDominanceErrorV1> {
        self.used =
            self.used
                .checked_add(amount)
                .ok_or(SemanticOptionDominanceErrorV1::WorkLimit {
                    actual: usize::MAX,
                    limit: MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1,
                })?;
        if self.used > MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1 {
            return Err(SemanticOptionDominanceErrorV1::WorkLimit {
                actual: self.used,
                limit: MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1,
            });
        }
        if let Some(meter) = &mut self.meter {
            meter.work(amount, true)?;
        }
        Ok(())
    }

    fn source_work(&mut self, amount: usize) -> Result<(), SemanticOptionDominanceErrorV1> {
        if let Some(meter) = &mut self.meter {
            meter.work(amount, false)?;
        }
        Ok(())
    }

    fn storage(&mut self, bytes: usize) -> Result<(), SemanticOptionDominanceErrorV1> {
        if let Some(meter) = &mut self.meter {
            meter.storage(bytes)?;
        }
        Ok(())
    }

    fn new_vec<T>(&mut self) -> Result<Vec<T>, SemanticOptionDominanceErrorV1> {
        self.storage(std::mem::size_of::<Vec<T>>())?;
        Ok(Vec::new())
    }

    fn reserve<T>(
        &mut self,
        rows: &mut Vec<T>,
        additional: usize,
        exact: bool,
    ) -> Result<(), SemanticOptionDominanceErrorV1> {
        use SemanticOptionDominanceErrorV1::Storage;
        let requested = rows.len().checked_add(additional).ok_or(Storage)?;
        let grows = requested > rows.capacity();
        if grows && self.meter.is_some() {
            self.source_work(rows.len())?;
            self.storage(
                requested
                    .checked_mul(std::mem::size_of::<T>())
                    .ok_or(Storage)?,
            )?;
        }
        if exact {
            rows.try_reserve_exact(additional)
        } else {
            rows.try_reserve(additional)
        }
        .map_err(|_| Storage)?;
        if grows && self.meter.is_some() {
            self.storage(
                rows.capacity()
                    .checked_sub(requested)
                    .and_then(|extra| extra.checked_mul(std::mem::size_of::<T>()))
                    .ok_or(Storage)?,
            )?;
        }
        Ok(())
    }

    fn filled<T: Clone>(
        &mut self,
        count: usize,
        value: T,
    ) -> Result<Vec<T>, SemanticOptionDominanceErrorV1> {
        if self.meter.is_none() {
            return Ok(vec![value; count]);
        }
        self.source_work(count)?;
        let mut rows = self.new_vec()?;
        self.reserve(&mut rows, count, true)?;
        rows.resize(count, value);
        Ok(rows)
    }

    fn capacity<T>(&mut self, count: usize) -> Result<Vec<T>, SemanticOptionDominanceErrorV1> {
        if self.meter.is_none() {
            return Ok(Vec::with_capacity(count));
        }
        let mut rows = self.new_vec()?;
        self.reserve(&mut rows, count, true)?;
        Ok(rows)
    }

    fn push<T>(
        &mut self,
        rows: &mut Vec<T>,
        value: T,
    ) -> Result<(), SemanticOptionDominanceErrorV1> {
        if self.meter.is_some() {
            self.source_work(1)?;
            self.reserve(rows, 1, false)?;
        }
        rows.push(value);
        Ok(())
    }

    fn boxed<T>(&mut self, rows: Vec<T>) -> Result<Box<[T]>, SemanticOptionDominanceErrorV1> {
        if self.meter.is_none() || rows.capacity() == rows.len() {
            return Ok(rows.into_boxed_slice());
        }
        let count = rows.len();
        let mut exact = self.new_vec()?;
        self.reserve(&mut exact, count, true)?;
        if exact.capacity() != count {
            return Err(SemanticOptionDominanceErrorV1::Storage);
        }
        self.source_work(count)?;
        exact.extend(rows);
        Ok(exact.into_boxed_slice())
    }
}

struct DominatorIntervalsV1 {
    entry: usize,
    preorder: Vec<usize>,
    subtree_end: Vec<usize>,
    predecessors: Vec<Vec<usize>>,
}

impl DominatorIntervalsV1 {
    fn analyze(
        function: &SemanticFunctionDeclV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<Self, SemanticOptionDominanceErrorV1> {
        let block_count = function.blocks().len();
        let entry = function.entry().index() as usize;
        if block_count == 0 || entry >= block_count {
            return Err(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "the semantic CFG has no valid entry block",
            ));
        }
        budget.charge(block_count)?;
        budget.storage(std::mem::size_of::<Self>())?;
        let mut successors = budget.filled(block_count, Vec::new())?;
        let mut predecessors = budget.filled(block_count, Vec::new())?;
        for (source, block) in function.blocks().iter().enumerate() {
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

        let mut visited = budget.filled(block_count, false)?;
        let mut postorder = budget.capacity(block_count)?;
        let mut pending = budget.filled(1, (entry, false))?;
        while let Some((block, finish)) = pending.pop() {
            budget.charge(1)?;
            if finish {
                budget.push(&mut postorder, block)?;
            } else if !visited[block] {
                visited[block] = true;
                budget.push(&mut pending, (block, true))?;
                for successor in successors[block].iter().rev() {
                    budget.charge(1)?;
                    if !visited[*successor] {
                        budget.push(&mut pending, (*successor, false))?;
                    }
                }
            }
        }
        budget.source_work(postorder.len())?;
        postorder.reverse();
        let mut rpo_index = budget.filled(block_count, usize::MAX)?;
        budget.source_work(postorder.len())?;
        for (index, block) in postorder.iter().copied().enumerate() {
            rpo_index[block] = index;
        }
        let mut immediate = budget.filled(block_count, None)?;
        immediate[entry] = Some(entry);
        loop {
            budget.charge(1)?;
            let mut changed = false;
            for block in postorder.iter().copied().skip(1) {
                budget.charge(1)?;
                budget.source_work(predecessors[block].len())?;
                let mut processed = predecessors[block]
                    .iter()
                    .copied()
                    .filter(|predecessor| immediate[*predecessor].is_some());
                let Some(mut next) = processed.next() else {
                    continue;
                };
                for predecessor in processed {
                    budget.charge(1)?;
                    next = intersect_dominator_paths(
                        predecessor,
                        next,
                        &immediate,
                        &rpo_index,
                        budget,
                    )?;
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
        let mut children = budget.filled(block_count, Vec::new())?;
        budget.source_work(immediate.len())?;
        for (block, parent) in immediate.iter().copied().enumerate() {
            if let Some(parent) = parent {
                budget.push(&mut children[parent], block)?;
            }
        }
        let mut preorder = budget.filled(block_count, usize::MAX)?;
        let mut subtree_end = budget.filled(block_count, usize::MAX)?;
        let mut clock = 0_usize;
        let mut pending = budget.filled(1, (entry, false))?;
        while let Some((block, finish)) = pending.pop() {
            budget.charge(1)?;
            if finish {
                subtree_end[block] = clock;
            } else {
                preorder[block] = clock;
                clock += 1;
                budget.push(&mut pending, (block, true))?;
                for child in children[block].iter().rev() {
                    budget.push(&mut pending, (*child, false))?;
                }
            }
        }
        Ok(Self {
            entry,
            preorder,
            subtree_end,
            predecessors,
        })
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

fn intersect_dominator_paths(
    mut left: usize,
    mut right: usize,
    immediate: &[Option<usize>],
    rpo_index: &[usize],
    budget: &mut WorkBudgetV1,
) -> Result<usize, SemanticOptionDominanceErrorV1> {
    while left != right {
        budget.charge(1)?;
        while rpo_index[left] > rpo_index[right] {
            budget.charge(1)?;
            left = immediate[left].ok_or(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "a processed dominator path has no parent",
            ))?;
        }
        while rpo_index[right] > rpo_index[left] {
            budget.charge(1)?;
            right = immediate[right].ok_or(SemanticOptionDominanceErrorV1::InvalidControlFlow(
                "a processed dominator path has no parent",
            ))?;
        }
    }
    Ok(left)
}

fn local_definition_counts(
    function: &SemanticFunctionDeclV1,
    budget: &mut WorkBudgetV1,
) -> Result<Vec<u8>, SemanticOptionDominanceErrorV1> {
    budget.charge(function.locals().len())?;
    let mut definitions = budget.new_vec()?;
    budget.reserve(&mut definitions, function.locals().len(), true)?;
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
    Ok(definitions)
}

#[cfg(test)]
#[path = "semantic_option_metered_v18_tests.rs"]
mod tests;
