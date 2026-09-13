//! Construction order is independent of numbered local identity and CFG order.
use super::*;

type E = ProductionRankedKernelErrorV1;

pub(super) struct RankedValueSlotsV1<T>(Vec<Option<T>>);
pub(super) type RankedLocalValuesV1 = RankedValueSlotsV1<Value>;

impl<T: Copy> RankedValueSlotsV1<T> {
    pub(super) fn new(count: usize) -> Result<Self, E> {
        if count > MAX_RANKED_BOUNDS_OPERATIONS {
            return Err(limit("materialized local slots", count));
        }
        Ok(Self(vec![None; count]))
    }

    pub(super) fn get(&self, index: usize) -> Option<T> {
        self.0.get(index).copied().flatten()
    }

    pub(super) fn bind(&mut self, id: ProductionRankedValueIdV1, value: T) -> Result<(), E> {
        let slot = self
            .0
            .get_mut(id.get() as usize)
            .ok_or(E::UndefinedValue(ProductionRankedValueV1::Local(id)))?;
        if slot.is_some() {
            return Err(E::Materialization(
                "numbered local was materialized more than once",
            ));
        }
        *slot = Some(value);
        Ok(())
    }

    pub(super) fn finish(&self) -> Result<(), E> {
        if self.0.iter().any(Option::is_none) {
            return Err(E::Materialization(
                "numbered local materialization is incomplete",
            ));
        }
        Ok(())
    }
}

pub(super) struct RankedOperationScheduleV1 {
    pub(super) operations: Vec<(usize, usize)>,
    pub(super) local_count: usize,
}

impl RankedOperationScheduleV1 {
    pub(super) fn new(
        kernel: &ProductionRankedKernelV1,
        reads: &RankedSemanticReadsV1<'_>,
    ) -> Result<Self, E> {
        Self::with_limit(kernel, reads, crate::MAX_RANKED_BOUNDS_WORK_UNITS)
    }

    fn with_limit(
        kernel: &ProductionRankedKernelV1,
        reads: &RankedSemanticReadsV1<'_>,
        work_limit: usize,
    ) -> Result<Self, E> {
        let mut work = 0usize;
        let mut charge = |amount: usize| {
            work = work.checked_add(amount).ok_or(E::ResourceLimit {
                resource: "construction dependency work",
                limit: work_limit,
                actual: usize::MAX,
            })?;
            if work > work_limit {
                return Err(E::ResourceLimit {
                    resource: "construction dependency work",
                    limit: work_limit,
                    actual: work,
                });
            }
            Ok(())
        };
        // Weighted work units cover traversal, dependency storage and queue
        // processing. These are not literal B-tree comparison counts. The hard
        // node cap bounds ordered-set depth; occurrences are charged before deduplication.
        charge(reads.work().saturating_mul(17))?;
        let mut locations = Vec::new();
        let mut producers = Vec::new();
        let mut starts = Vec::new();
        for (block, body) in kernel.blocks.iter().enumerate() {
            charge(1)?;
            starts.push(locations.len());
            for (operation, op) in body.operations.iter().enumerate() {
                if locations.len() == MAX_RANKED_BOUNDS_OPERATIONS {
                    return Err(limit("construction operations", locations.len() + 1));
                }
                charge(64)?;
                let node = locations.len();
                locations.push((block, operation));
                for id in result_ids(op).into_iter().flatten() {
                    if id.get() as usize != producers.len() {
                        return Err(E::NonCanonicalValueId {
                            expected: producers.len() as u32,
                            actual: id.get(),
                        });
                    }
                    if producers.len() == MAX_RANKED_BOUNDS_OPERATIONS {
                        return Err(limit("materialized local slots", producers.len() + 1));
                    }
                    charge(2)?;
                    producers.push(node);
                }
            }
        }
        let node_at = |(block, operation): (u32, u32)| {
            let body = kernel
                .blocks
                .get(block as usize)
                .ok_or(E::InvalidBlockTarget(block))?;
            if operation as usize >= body.operations.len() {
                return Err(E::Materialization(
                    "construction dependency names a missing operation",
                ));
            }
            Ok(starts[block as usize] + operation as usize)
        };
        let mut dependencies = vec![BTreeSet::new(); locations.len()];
        for (node, &(block, operation)) in locations.iter().enumerate() {
            if operation != 0 {
                charge(32)?;
                dependencies[node].insert(node - 1);
            }
            visit_operation_values_v1(&kernel.blocks[block].operations[operation], |value| {
                charge(32)?;
                match value {
                    ProductionRankedValueV1::Local(id) => {
                        dependencies[node].insert(
                            *producers
                                .get(id.get() as usize)
                                .ok_or(E::UndefinedValue(value))?,
                        );
                    }
                    ProductionRankedValueV1::Argument(id)
                        if id as usize >= kernel.argument_count =>
                    {
                        return Err(E::UndefinedValue(value));
                    }
                    ProductionRankedValueV1::BlockArgument { block, argument }
                        if kernel
                            .blocks
                            .get(block as usize)
                            .is_none_or(|body| argument >= body.index_argument_count) =>
                    {
                        return Err(E::UndefinedValue(value));
                    }
                    _ => {}
                }
                Ok(())
            })?;
        }
        // A load is a dependency on its original access, not fresh memory at the
        // consumer. Its address operands belong to that access's dependencies.
        for load in reads.loads() {
            charge(32usize.saturating_add(load.indices.len()))?;
            let source =
                node_at((load.block, load.operation)).map_err(|_| E::InvalidReferenceContract)?;
            let (block, operation) = locations[source];
            let ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Read,
                view,
                indices,
            } = &kernel.blocks[block].operations[operation]
            else {
                return Err(E::InvalidReferenceContract);
            };
            if *view != load.view || indices.as_slice() != load.indices.as_ref() {
                return Err(E::InvalidReferenceContract);
            }
            let ProductionRankedValueV1::Local(id) = load.view else {
                return Err(E::InvalidReferenceContract);
            };
            let node = *producers
                .get(id.get() as usize)
                .ok_or(E::InvalidReferenceContract)?;
            let (block, operation) = locations[node];
            let (width, origin, space) = match &kernel.blocks[block].operations[operation] {
                ProductionRankedOperationV1::View {
                    element_width,
                    allocation_origin,
                    ..
                } => (*element_width, *allocation_origin, MemorySpaceAttr::Global),
                ProductionRankedOperationV1::ViewInSpace {
                    element_width,
                    allocation_origin,
                    memory_space,
                    ..
                } => (*element_width, *allocation_origin, *memory_space),
                _ => return Err(E::InvalidReferenceContract),
            };
            if space != MemorySpaceAttr::Global
                || origin != load.allocation_origin
                || width != u32::from(load.scalar.bit_width())
            {
                return Err(E::InvalidReferenceContract);
            }
        }
        for (source, consumer) in reads.dependencies() {
            charge(32)?;
            dependencies[node_at(consumer)?].insert(node_at(source)?);
        }
        for body in &kernel.blocks {
            visit_terminator_values_v1(&body.terminator, |value| {
                charge(4)?;
                match value {
                    ProductionRankedValueV1::Local(id) if id.get() as usize >= producers.len() => {
                        Err(E::UndefinedValue(value))
                    }
                    ProductionRankedValueV1::Argument(id)
                        if id as usize >= kernel.argument_count =>
                    {
                        Err(E::UndefinedValue(value))
                    }
                    ProductionRankedValueV1::BlockArgument { block, argument }
                        if kernel
                            .blocks
                            .get(block as usize)
                            .is_none_or(|body| argument >= body.index_argument_count) =>
                    {
                        Err(E::UndefinedValue(value))
                    }
                    _ => Ok(()),
                }
            })?;
        }
        let mut incoming = Vec::with_capacity(locations.len());
        let mut outgoing = vec![Vec::new(); locations.len()];
        for (node, dependencies) in dependencies.into_iter().enumerate() {
            incoming.push(dependencies.len());
            for predecessor in dependencies {
                outgoing[predecessor].push(node);
            }
        }
        let mut ready = incoming
            .iter()
            .enumerate()
            .filter_map(|(node, count)| (*count == 0).then_some(node))
            .collect::<BTreeSet<_>>();
        let mut operations = Vec::with_capacity(locations.len());
        while let Some(node) = ready.pop_first() {
            operations.push(locations[node]);
            for &next in &outgoing[node] {
                incoming[next] = incoming[next]
                    .checked_sub(1)
                    .ok_or(E::Materialization("construction dependency count changed"))?;
                if incoming[next] == 0 {
                    ready.insert(next);
                }
            }
        }
        if operations.len() != locations.len() {
            return Err(E::Materialization(
                "cyclic operation construction dependencies",
            ));
        }
        Ok(Self {
            operations,
            local_count: producers.len(),
        })
    }
}

/// Build the original blocks and operation order using a preflighted schedule.
/// This does not grant dominance or proof authority; callers verify the complete graph.
pub(super) fn materialize_body(
    context: &mut pliron::context::Context,
    function: &FuncOp,
    kernel: &ProductionRankedKernelV1,
    schedule: &RankedOperationScheduleV1,
    mut reads: RankedSemanticReadsV1<'_>,
    staging: &[ProductionPolicyCheckedRefinementStagingV2],
) -> Result<RankedLocalValuesV1, E> {
    let index: TypeHandle = IndexType::get(context).into();
    let mut blocks = vec![function.get_entry_block(context)];
    for (block_index, body) in kernel.blocks.iter().enumerate().skip(1) {
        let label: Identifier = format!("bb{block_index}")
            .as_str()
            .try_into()
            .map_err(|_| E::Materialization("generated block label could not be interned"))?;
        let block = BasicBlock::new(
            context,
            Some(label),
            vec![index; body.index_argument_count as usize],
        );
        block.insert_at_back(function.get_region(context), context);
        blocks.push(block);
    }
    let arguments = blocks[0].deref(context).arguments().collect::<Vec<_>>();
    let mut block_arguments = HashMap::new();
    for (block_index, block) in blocks.iter().enumerate().skip(1) {
        for (argument_index, argument) in block.deref(context).arguments().enumerate() {
            block_arguments.insert((block_index as u32, argument_index as u32), argument);
        }
    }
    let mut locals = RankedLocalValuesV1::new(schedule.local_count)?;
    for &(block, operation) in &schedule.operations {
        materialize_operation(
            context,
            blocks[block],
            &kernel.blocks[block].operations[operation],
            &arguments,
            &mut locals,
            &block_arguments,
            staging,
            (block as u32, operation as u32),
            &mut reads,
        )?;
    }
    locals.finish()?;
    reads.finish()?;
    for (block, body) in kernel.blocks.iter().enumerate() {
        materialize_terminator(
            context,
            blocks[block],
            &body.terminator,
            &blocks,
            &arguments,
            &locals,
            &block_arguments,
        )?;
    }
    Ok(locals)
}

fn limit(resource: &'static str, actual: usize) -> E {
    E::ResourceLimit {
        resource,
        limit: MAX_RANKED_BOUNDS_OPERATIONS,
        actual,
    }
}

fn result_ids(operation: &ProductionRankedOperationV1) -> [Option<ProductionRankedValueIdV1>; 2] {
    use ProductionRankedOperationV1 as O;
    match operation {
        O::PredicatedCheckedTiledIndex2D {
            result, success, ..
        }
        | O::PredicatedCheckedRowStripedIndex2D {
            result, success, ..
        } => [Some(*result), Some(*success)],
        O::View { result, .. }
        | O::ViewInSpace { result, .. }
        | O::PipelineCreate { result, .. }
        | O::IndexConstant { result, .. }
        | O::IndexUnsignedCast { result, .. }
        | O::IndexUnknown { result, .. }
        | O::InvocationIndex { result, .. }
        | O::IndexBinary { result, .. }
        | O::DeterministicJoin { result, .. }
        | O::CheckedTiledIndex2D { result, .. }
        | O::CheckedRowStripedIndex2D { result, .. }
        | O::Dimension { result, .. }
        | O::SemanticSymbol { result, .. }
        | O::SemanticConstant { result, .. }
        | O::SemanticBinary { result, .. }
        | O::SemanticExpression { result, .. }
        | O::TensorResultComponent { result, .. } => [Some(*result), None],
        O::ExecutionLayout { .. }
        | O::PipelineEvent { .. }
        | O::Access { .. }
        | O::PredicatedAccess { .. }
        | O::ValueAccess { .. }
        | O::AtomicAccess { .. }
        | O::AtomicValueAccess { .. }
        | O::OwnershipContract { .. }
        | O::AllocationEffect { .. }
        | O::Barrier { .. }
        | O::Fence { .. }
        | O::TensorLayout { .. }
        | O::RequireEquivalent { .. }
        | O::CollectiveSemantics { .. }
        | O::RequireAuthenticatedReferenceEquivalent { .. }
        | O::RequestAuthenticatedReferenceEquivalent { .. }
        | O::RequireEffectRefinement { .. }
        | O::RequestEffectRefinement { .. }
        | O::RequireNumericalRefinement { .. }
        | O::RequestNumericalRefinement { .. }
        | O::RequireTensorRefinement { .. }
        | O::RequestTensorRefinement { .. } => [None, None],
    }
}

#[cfg(test)]
#[path = "materialization_v1/tests.rs"]
mod tests;
