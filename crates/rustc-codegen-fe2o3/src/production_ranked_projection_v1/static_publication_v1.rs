// Only these two exact source terminals may introduce a static publication pair.
// The live ranked proof separately authenticates role, geometry and effect order.
include!("static_publication_audit_v1.rs");
include!("static_publication_metadata_v1.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StaticPublicationRoleV1 {
    Producer,
    Consumer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StaticPublicationRootV1 {
    argument: u32,
    local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
    allocation: AllocationContractV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StaticPublicationSiteV1 {
    block: usize,
    role: StaticPublicationRoleV1,
    cell: SemanticLocalIdV1,
    result: SemanticTypeIdV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StaticPublicationSourceV1 {
    payload: StaticPublicationRootV1,
    flags: StaticPublicationRootV1,
    producer: StaticPublicationSiteV1,
    consumer: StaticPublicationSiteV1,
    metadata_snapshot: Option<StaticPublicationMetadataSnapshotV1>,
}

fn static_publication_reject_v1() -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::Incomplete(
        "a static publication root lacks exact once-only custody or escapes its closed protocol",
    )
}

fn static_publication_intrinsic_v1(
    callables: &[SemanticCallableDeclV1],
    call: &SemanticDirectCallV1,
) -> Option<(
    StaticPublicationRoleV1,
    SemanticTypeIdV1,
    SemanticTypeIdV1,
    SemanticTypeIdV1,
)> {
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } =
        callables.get(call.callee().index() as usize)?
    else {
        return None;
    };
    match operation {
        SemanticCompilerIntrinsicOperationV1::StaticPublication128PublishF32 {
            payload,
            flags,
            result,
        } => Some((StaticPublicationRoleV1::Producer, *payload, *flags, *result)),
        SemanticCompilerIntrinsicOperationV1::StaticPublication128TryReadF32 {
            payload,
            flags,
            result,
        } => Some((StaticPublicationRoleV1::Consumer, *payload, *flags, *result)),
        _ => None,
    }
}

fn static_publication_original_root_v1(
    function: &SemanticFunctionDeclV1,
    operand: &SemanticOperandV1,
    expected: SemanticTypeIdV1,
    ownership: SemanticSourceArgumentOwnershipV1,
    allocations: &[Option<AllocationContractV1>],
) -> Option<StaticPublicationRootV1> {
    let (SemanticOperandV1::Move(place) | SemanticOperandV1::Copy(place)) = operand else {
        return None;
    };
    if !place.projections().is_empty() || place.ty() != expected {
        return None;
    }
    let declaration = function.locals().get(place.local().index() as usize)?;
    let SemanticLocalRoleV1::Argument(argument) = declaration.role() else {
        return None;
    };
    if declaration.ty() != expected
        || function.abi().source_input_types().get(argument as usize) != Some(&expected)
        || function
            .abi()
            .source_argument_ownership()
            .get(argument as usize)
            != Some(&ownership)
    {
        return None;
    }
    let allocation = allocations
        .get(place.local().index() as usize)
        .copied()
        .flatten()?;
    if allocation.allocation_origin != u64::from(argument) + 1
        || !allocation.writable
        || allocation.singleton_object
        || allocation.noalias_class
            != if ownership == SemanticSourceArgumentOwnershipV1::ExclusiveOwner {
                allocation.allocation_origin + 1
            } else {
                1
            }
    {
        return None;
    }
    Some(StaticPublicationRootV1 {
        argument,
        local: place.local(),
        ty: expected,
        allocation,
    })
}

fn audit_static_publication_source_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    allocations: &[Option<AllocationContractV1>],
) -> Result<Option<StaticPublicationSourceV1>, ProductionRankedProjectionErrorV1> {
    let mut work = 0;
    let mut sites = Vec::new();
    for (block, body) in function.blocks().iter().enumerate() {
        charge_capability_dataflow_work_v1(&mut work, 1)?;
        if let SemanticTerminatorKindV1::Call(call) = body.terminator().kind()
            && let Some((role, payload, flags, result)) =
                static_publication_intrinsic_v1(callables, call)
        {
            if sites.len() == 2 {
                return Err(static_publication_reject_v1());
            }
            sites.push((block, role, payload, flags, result, call));
        }
    }
    if sites.is_empty() {
        return Ok(None);
    }
    if sites.len() != 2 || sites[0].1 == sites[1].1 || allocations.len() != function.locals().len()
    {
        return Err(static_publication_reject_v1());
    }
    let mut roots = None;
    let mut producer = None;
    let mut consumer = None;
    let mut proofs = SemanticAssertProofsV1::new(types, function)?;
    proofs.charge(work)?;
    for (block, role, payload_type, flags_type, result_type, call) in sites {
        let count = if role == StaticPublicationRoleV1::Producer {
            4
        } else {
            3
        };
        if call.arguments().len() != count
            || call.destination().is_none_or(|destination| {
                !destination.place().projections().is_empty()
                    || destination.place().ty() != result_type
            })
            || matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
        {
            return Err(static_publication_reject_v1());
        }
        let payload = static_publication_original_root_v1(
            function,
            &call.arguments()[0],
            payload_type,
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            allocations,
        )
        .ok_or_else(static_publication_reject_v1)?;
        let flags = static_publication_original_root_v1(
            function,
            &call.arguments()[1],
            flags_type,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            allocations,
        )
        .ok_or_else(static_publication_reject_v1)?;
        if payload.argument == flags.argument
            || atomic_slice_argument_v1(types, function, flags.local).is_none()
            || unsigned_index_bits_v1(types, call.arguments()[2].ty()) != Some(64)
            || (count == 4
                && !matches!(
                    types
                        .get(call.arguments()[3].ty().index() as usize)
                        .map(SemanticTypeDeclV1::shape),
                    Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float {
                        bits: 32
                    }))
                ))
        {
            return Err(static_publication_reject_v1());
        }
        let cell =
            simple_operand_local(&call.arguments()[2]).ok_or_else(static_publication_reject_v1)?;
        if !indexed_atomic_index_stable_v1(&mut proofs, cell)?
            || !indexed_atomic_block_acyclic_v1(&mut proofs, block)?
        {
            return Err(static_publication_reject_v1());
        }
        if roots
            .replace((payload, flags))
            .is_some_and(|previous| previous != (payload, flags))
        {
            return Err(static_publication_reject_v1());
        }
        let site = StaticPublicationSiteV1 {
            block,
            role,
            cell,
            result: result_type,
        };
        match role {
            StaticPublicationRoleV1::Producer => producer = Some(site),
            StaticPublicationRoleV1::Consumer => consumer = Some(site),
        }
    }
    let (payload, flags) = roots.ok_or_else(static_publication_reject_v1)?;
    let mut source = StaticPublicationSourceV1 {
        payload,
        flags,
        producer: producer.ok_or_else(static_publication_reject_v1)?,
        consumer: consumer.ok_or_else(static_publication_reject_v1)?,
        metadata_snapshot: None,
    };
    if source.producer.cell != source.consumer.cell
        || source.producer.result != source.consumer.result
    {
        return Err(static_publication_reject_v1());
    }
    source.metadata_snapshot =
        static_publication_metadata_snapshot_v1(&mut proofs, callables, source.payload)?;
    work = proofs.work;
    static_publication_audit_uses_v1(types, callables, function, allocations, &source, &mut work)?;
    Ok(Some(source))
}
