//! Isolated private-allocation execution frame. Large receipt/descriptors must
//! not enlarge every scalar/load/OOB evaluator stack frame in debug builds.
use super::*;

#[inline(never)]
pub(super) fn execute(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    operation: &Operation,
    site: CompactSite,
    frame_allocations: &mut Vec<FrameAllocation>,
) -> Result<SmallResults<RuntimeValue>, SimulationExecutionErrorV1> {
    let OperationKind::Alloca {
        element,
        count,
        address_space,
        alignment,
    } = &operation.kind
    else {
        return Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant(
                "outlined private allocation dispatch",
            ),
        ));
    };
    let Type::Scalar(element) = element else {
        return Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant("preflighted scalar allocation"),
        ));
    };
    if *address_space != AddressSpace::Private {
        return Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant("preflighted private allocation"),
        ));
    }
    let count = match count {
        Some(count) => {
            scalar_nonnegative_usize(scalar_value(engine, values, *count, &site)?, engine.target)
                .map_err(|kind| engine.at(site, kind))?
        }
        None => 1,
    };
    let element_bytes = engine.target.scalar_bytes(*element).ok_or_else(|| {
        engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant("preflighted allocation element"),
        )
    })?;
    let bytes = count.checked_mul(element_bytes).ok_or_else(|| {
        engine.at(
            site,
            SimulationExecutionErrorKindV1::AllocationBytesLimit {
                actual: usize::MAX,
                limit: engine.limits.max_allocation_bytes,
            },
        )
    })?;
    engine
        .memory
        .validate_allocation(bytes, engine.limits)
        .map_err(|kind| engine.at(site, kind))?;
    let allocation_bytes = try_filled(bytes, 0_u8).map_err(|kind| engine.at(site, kind))?;
    let initialized = try_filled(bytes, false).map_err(|kind| engine.at(site, kind))?;
    frame_allocations
        .try_reserve(1)
        .map_err(|_| engine.at(site, SimulationExecutionErrorKindV1::AllocationFailure))?;
    let creation = engine.allocation_creation_v1(site, AddressSpace::Private)?;
    let reserved = engine.reserve_event_closure(&site)?;
    let commit = match engine.memory.allocate(
        (AddressSpace::Private, AccessMode::ReadWrite, *alignment),
        (allocation_bytes, initialized),
        creation,
        engine.limits,
    ) {
        Ok(commit) => commit,
        Err(kind) => {
            engine.cancel_event_closure(reserved);
            return Err(engine.at(site, kind));
        }
    };
    let id = commit.id;
    frame_allocations.push(FrameAllocation {
        id,
        lifecycle_observed: reserved,
    });
    engine.deliver_allocation_lifecycle_v1(commit.transition);
    engine.emit_reserved_begin(
        &site,
        SimulationEventKindV1::AllocationCreated {
            allocation: id,
            address_space: AddressSpace::Private,
            bytes,
        },
        reserved,
    )?;
    Ok(SmallResults::One(RuntimeValue::Pointer(PointerValue {
        allocation: id,
        byte_offset: 0,
        element: *element,
        address_space: AddressSpace::Private,
        access: AccessMode::ReadWrite,
        lower_bound: 0,
        upper_bound: bytes,
        abi_argument_ordinal: NO_ABI_ARGUMENT_V1,
    })))
}
