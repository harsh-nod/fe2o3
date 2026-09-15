use super::*;
use dialect_kernel::*;
use pliron::common_traits::Verify;

pub(super) fn reads(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
) -> Result<Vec<PlironProvedSemanticReadV1>, PlironSemanticMemoryErrorV1> {
    use PlironSemanticMemoryErrorV1 as E;
    let mut consumed = HashSet::new();
    let mut symbols = HashSet::new();
    let mut reads = Vec::new();
    for location in inventory.operations() {
        let pointer = location.pointer();
        let raw = pointer.deref(context);
        let operation = Operation::get_op_dyn(pointer, context);
        let site = PlironSemanticMemorySiteV1 {
            block: location.block(),
            operation: location.operation(),
        };
        for operand in raw.operands() {
            consumed.insert(operand);
        }
        let symbol = operation
            .downcast_ref::<SemanticTypedSymbolOp>()
            .and_then(|op| op.symbol(context))
            .or_else(|| {
                operation
                    .downcast_ref::<SemanticSymbolOp>()
                    .and_then(|op| op.symbol(context))
            });
        if symbol.is_some_and(|symbol| symbol >= SEMANTIC_TYPED_READ_SYMBOL_BASE_V1) {
            return Err(E::UnboundLoadSymbol { site });
        }
        if let Some(access) = operation.downcast_ref::<RankedAccessOp>() {
            if !matches!(
                access.kind(context),
                Some(AccessKindAttr::Read | AccessKindAttr::Write)
            ) || access.atomic_ordering(context).is_some()
                || access.atomic_scope(context).is_some()
                || access.checked_success(context).is_some()
            {
                return Err(E::UnsupportedOperation { site });
            }
            let view = Operation::get_op_dyn(
                access.view(context).defining_op().ok_or(E::InvalidGraph)?,
                context,
            );
            if view
                .downcast_ref::<RankedViewOp>()
                .is_none_or(|view| view.memory_space(context) != Some(MemorySpaceAttr::Global))
            {
                return Err(E::UnsupportedOperation { site });
            }
        }
        if let Some(read) = operation.downcast_ref::<SemanticTypedReadOp>() {
            if read.guarded(context).is_some()
                || read.memory_space(context) != Some(MemorySpaceAttr::Global)
                || read.volatility(context) != Some(SemanticReadVolatilityAttr::NonVolatile)
                || read.ordering(context) != Some(SemanticReadOrderingAttr::Unordered)
            {
                return Err(E::UnsupportedRead { site });
            }
            let previous = location
                .operation()
                .checked_sub(1)
                .and_then(|index| inventory.block_operations(location.block()).get(index))
                .map(|previous| previous.pointer());
            let access = paired_read_access_v1(context, read, previous, site)?;
            let indices = read.indices(context).ok_or(E::InvalidGraph)?;
            let view = read.view(context);
            let definition =
                Operation::get_op_dyn(view.defining_op().ok_or(E::InvalidGraph)?, context);
            let view_op = definition
                .downcast_ref::<RankedViewOp>()
                .ok_or(E::InvalidGraph)?;
            let allocation = view_op
                .allocation_origin(context)
                .filter(|origin| *origin != 0)
                .ok_or(E::IncompleteProvenance)?;
            if !symbols.insert(read.symbol(context).ok_or(E::InvalidGraph)?) {
                return Err(E::DuplicateLoadIdentity);
            }
            reads.push(PlironProvedSemanticReadV1 {
                site,
                access,
                producer: pointer,
                result: read.result(context),
                view,
                indices,
                allocation,
                scalar: read.scalar(context).ok_or(E::InvalidGraph)?,
                instances: Vec::new(),
            });
        }
    }
    if reads.is_empty() {
        return Err(E::EmptyReadRoster);
    }
    if reads.iter().any(|read| !consumed.contains(&read.result)) {
        return Err(E::UnconsumedRead);
    }
    Ok(reads)
}

/// The caller supplies only the immediately preceding operation in this block.
/// This binds a result to an existing access; it proves no memory equality.
pub(crate) fn paired_read_access_v1(
    context: &Context,
    read: &SemanticTypedReadOp,
    previous: Option<Ptr<Operation>>,
    site: PlironSemanticMemorySiteV1,
) -> Result<Ptr<Operation>, PlironSemanticMemoryErrorV1> {
    use PlironSemanticMemoryErrorV1 as E;
    read.verify(context).map_err(|_| E::InvalidGraph)?;
    let previous = previous.ok_or(E::UnpairedRead { site })?;
    let operation = Operation::get_op_dyn(previous, context);
    let access = operation
        .downcast_ref::<RankedAccessOp>()
        .ok_or(E::UnpairedRead { site })?;
    let indices = read.indices(context).ok_or(E::InvalidGraph)?;
    if access.kind(context) != Some(AccessKindAttr::Read)
        || access.view(context) != read.view(context)
        || access.indices(context) != indices
        || access.atomic_ordering(context).is_some()
        || access.atomic_scope(context).is_some()
        || access.checked_success(context).is_some()
        || read.guarded(context).is_some()
        || previous.deref(context).get_num_operands() != indices.len() + 1
    {
        return Err(E::UnpairedRead { site });
    }
    Ok(previous)
}

pub(super) fn events(
    traces: &[crate::pliron_invocation_trace::PlironInvocationTraceV1],
) -> Result<Vec<state::MemoryEvent>, PlironSemanticMemoryErrorV1> {
    use PlironSemanticMemoryErrorV1 as E;
    let mut events = Vec::new();
    let mut words = 0usize;
    for (invocation, trace) in traces.iter().enumerate() {
        for (sequence, event) in trace.events.iter().enumerate() {
            let PlironTraceEventV1::Memory {
                location,
                memory_space: MemorySpaceAttr::Global,
                access,
                atomic_ordering: None,
                atomic_scope: None,
                indices,
                allocation_origin,
                view_signature,
                ..
            } = event
            else {
                return Err(E::IncompleteTrace);
            };
            if *allocation_origin == 0 {
                return Err(E::IncompleteProvenance);
            }
            let site = PlironSemanticMemorySiteV1 {
                block: location.block,
                operation: location.operation,
            };
            words = words
                .checked_add(12 + indices.len())
                .ok_or(E::ResourceLimit)?;
            if words > MAX_PLIRON_TRACE_TOTAL_STEPS_V1 {
                return Err(E::ResourceLimit);
            }
            if indices.len() != view_signature.1.len()
                || view_signature.0 == 0
                || indices
                    .iter()
                    .zip(&view_signature.1)
                    .any(|(index, extent)| index.is_none_or(|index| index >= *extent))
            {
                return Err(E::UnprovedBounds { site });
            }
            let write = match access {
                AccessKindAttr::Read => false,
                AccessKindAttr::Write => true,
                _ => return Err(E::UnsupportedOperation { site }),
            };
            events.push(state::MemoryEvent {
                invocation,
                sequence,
                block: location.block,
                operation: location.operation,
                allocation: *allocation_origin,
                indices: indices
                    .iter()
                    .copied()
                    .collect::<Option<Vec<_>>>()
                    .ok_or(E::UnprovedBounds { site })?,
                write,
            });
        }
    }
    Ok(events)
}

pub(super) fn supported(operation: &dyn Op) -> bool {
    macro_rules! any {
        ($($ty:ty),+ $(,)?) => { false $(|| operation.downcast_ref::<$ty>().is_some())+ };
    }
    any!(
        RankedViewOp,
        RankedAccessOp,
        SemanticTypedReadOp,
        ReturnOp,
        BranchOp,
        BranchArgsOp,
        IndexLessThanBranchOp,
        IndexLessThanBranchArgsOp,
        IndexEqualBranchOp,
        IndexEqualBranchArgsOp,
        IndexConstantOp,
        IndexUnknownOp,
        IndexUnsignedCastOp,
        InvocationIndexOp,
        IndexBinaryOp,
        CheckedTiledIndex2DOp,
        CheckedRowStripedIndex2DOp,
        DimensionOp,
        OwnershipContractOp,
        SemanticSymbolOp,
        SemanticConstantOp,
        SemanticBinaryOp,
        SemanticExpressionCommitmentOp,
        SemanticTypedSymbolOp,
        SemanticTypedConstantOp,
        SemanticTypedUnaryOp,
        SemanticTypedBinaryOp,
        SemanticTypedCompareOp,
        SemanticTypedSelectOp,
        SemanticTypedCastOp,
        SemanticTypedExpressionRootOp,
        RequireEquivalentOp,
        dialect_gpu::ExecutionLayoutOp,
        dialect_proof::ObligationOp,
        dialect_proof::EvidenceRefOp,
        dialect_proof::RequireRefinementOp,
        dialect_proof::RequireNumericalRefinementOp,
        dialect_proof::RequireEffectRefinementOp
    )
}
