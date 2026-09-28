//! Shared source preparation predicates; original-ledger adapter is optional.
//! No synthetic owners, capability authentication or ranked admission.
use super::bf16_nominal_preparation_resources_v1::PreparationResourcesV1;
use super::*;

// Prepay every variable transparency/reborrow scan before invoking the shared
// helpers. Fixed statement classification does not cover source-sized spines.
pub(super) fn prepay_provenance_spines_v1(
    kind: &SemanticRvalueKindV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if !resources.is_metered() {
        return Ok(());
    }
    let operand_len = |operand: &SemanticOperandV1| {
        raw_operand_place(operand).map_or(0, |place| place.projections().len())
    };
    let (length, scans) = match kind {
        // Stable-origin and allocation-origin classification both inspect it.
        SemanticRvalueKindV1::Use(operand) => (operand_len(operand), 2),
        SemanticRvalueKindV1::Cast {
            kind: SemanticCastKindV1::Pointer,
            operand,
        } => (operand_len(operand), 2),
        SemanticRvalueKindV1::Cast { operand, .. } => (operand_len(operand), 1),
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. } => (place.projections().len(), 1),
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Offset,
            left,
            ..
        } => (operand_len(left), 1),
        _ => (0, 0),
    };
    resources.work(length.checked_mul(scans).ok_or_else(|| {
        bf16_nominal_preparation_resources_v1::resource(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
        )
    })?)
}

pub(super) fn local_provenance_with_resources_v1(
    callables: &[SemanticCallableDeclV1],
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    definitions: &[u8],
    address_escaped: &[bool],
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<LocalProvenanceV1, ProductionRankedProjectionErrorV1> {
    resources.reserve_storage(
        std::mem::size_of::<LocalProvenanceV1>()
            + 3 * std::mem::size_of::<Vec<Vec<usize>>>()
            + std::mem::size_of::<Vec<Option<u32>>>()
            + 4096,
    )?;
    let local_count = function.locals().len();
    if definitions.len() != local_count || address_escaped.len() != local_count {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "local provenance scalar custody tables do not match the semantic local table",
        ));
    }
    let exclusive_owner_origins = if resources.is_metered() {
        exclusive_owner_carrier_v1::exclusive_owner_value_origins_with_resources_v1(
            callables,
            function,
            definitions,
            resources,
        )?
    } else {
        exclusive_owner_carrier_v1::exclusive_owner_value_origins_v1(
            callables,
            function,
            definitions,
        )?
    };
    let mut stable_argument_origins = resources.filled(local_count, None)?;
    let mut allocation_origins = resources.filled(local_count, None)?;
    let mut allocation_provenance = resources.filled(local_count, None)?;
    let mut stable_edges = resources.nested(local_count)?;
    let mut allocation_edges = resources.nested(local_count)?;
    let mut allocation_contract_edges = resources.nested(local_count)?;
    for (local_index, local) in function.locals().iter().enumerate() {
        resources.work(1)?;
        if let SemanticLocalRoleV1::Argument(argument) = local.role() {
            if definitions[local_index] == 0 && !address_escaped[local_index] {
                stable_argument_origins[local_index] = Some(argument);
            }
            allocation_origins[local_index] = Some(argument);
            allocation_provenance[local_index] =
                Some(LocalAllocationProvenanceV1::Argument(argument));
        }
    }
    let mut edge_count = 0_usize;
    for block in function.blocks() {
        resources.work(1)?;
        for statement in block.statements() {
            resources.work(32)?;
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let destination = assignment.destination();
            if !destination.projections().is_empty()
                || definitions
                    .get(destination.local().index() as usize)
                    .copied()
                    != Some(1)
            {
                continue;
            }
            let destination = destination.local().index() as usize;
            prepay_provenance_spines_v1(assignment.value().kind(), resources)?;
            let stable_source = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) | SemanticRvalueKindV1::Cast { operand, .. } => {
                    simple_operand_local(operand)
                }
                _ => None,
            };
            if let Some(source) = stable_source {
                let source = source.index() as usize;
                if address_escaped.get(source).copied() == Some(false)
                    && !address_escaped[destination]
                {
                    push_local_provenance_edge_with_resources_v1(
                        &mut stable_edges,
                        source,
                        destination,
                        &mut edge_count,
                        resources,
                    )?;
                }
            };

            let allocation_source = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => {
                    allocation_operand_local_v1(types, function, operand)
                }
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Pointer,
                    operand,
                } => simple_operand_local(operand),
                SemanticRvalueKindV1::Borrow { place, .. } => {
                    borrowed_allocation_local_v1(function, place, &exclusive_owner_origins)
                }
                SemanticRvalueKindV1::AddressOf { place, .. } => {
                    reborrowed_allocation_local_v1(place)
                }
                _ => None,
            };
            if let Some(source) = allocation_source {
                push_local_provenance_edge_with_resources_v1(
                    &mut allocation_edges,
                    source.index() as usize,
                    destination,
                    &mut edge_count,
                    resources,
                )?;
                push_local_provenance_edge_with_resources_v1(
                    &mut allocation_contract_edges,
                    source.index() as usize,
                    destination,
                    &mut edge_count,
                    resources,
                )?;
            } else if let SemanticRvalueKindV1::Borrow { place, .. }
            | SemanticRvalueKindV1::AddressOf { place, .. } = assignment.value().kind()
                && place.projections().is_empty()
                && function
                    .locals()
                    .get(place.local().index() as usize)
                    .is_some_and(|local| !matches!(local.role(), SemanticLocalRoleV1::Argument(_)))
            {
                let origin = LocalAllocationProvenanceV1::Private(place.local());
                match allocation_provenance.get_mut(destination) {
                    Some(slot @ None) => *slot = Some(origin),
                    Some(Some(existing)) if *existing == origin => {}
                    Some(Some(_)) => {
                        return Err(ProductionRankedProjectionErrorV1::Incomplete(
                            "a local may alias multiple kernel allocation origins",
                        ));
                    }
                    None => {
                        return Err(ProductionRankedProjectionErrorV1::Unsupported(
                            "a private allocation root is outside the semantic local table",
                        ));
                    }
                }
            }

            if let SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Offset,
                left,
                ..
            } = assignment.value().kind()
                && let Some(source) = allocation_operand_local_v1(types, function, left)
            {
                // Pointer offsets retain only an authenticated external allocation
                // contract. Private roots have no size/range contract and must not
                // gain address-formation authority through raw pointer arithmetic.
                push_local_provenance_edge_with_resources_v1(
                    &mut allocation_contract_edges,
                    source.index() as usize,
                    destination,
                    &mut edge_count,
                    resources,
                )?;
            }
        }
    }

    propagate_exact_local_origins_with_resources_v1(
        &mut stable_argument_origins,
        &stable_edges,
        "a runtime index may derive from multiple kernel arguments",
        resources,
    )?;
    propagate_exact_local_origins_with_resources_v1(
        &mut allocation_provenance,
        &allocation_edges,
        "a local may alias multiple kernel allocation origins",
        resources,
    )?;
    propagate_exact_local_origins_with_resources_v1(
        &mut allocation_origins,
        &allocation_contract_edges,
        "a local may alias multiple kernel allocation origins",
        resources,
    )?;
    Ok(LocalProvenanceV1 {
        stable_argument_origins,
        allocation_origins,
        allocation_provenance,
    })
}

pub(super) fn push_local_provenance_edge_with_resources_v1(
    edges: &mut [Vec<usize>],
    source: usize,
    destination: usize,
    edge_count: &mut usize,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    resources.work(1)?;
    if source >= edges.len() || destination >= edges.len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a local provenance edge is outside the semantic local table",
        ));
    }
    *edge_count =
        edge_count
            .checked_add(1)
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "local provenance edge accounting overflowed",
            ))?;
    if *edge_count > MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "local provenance edges exceed the charged projection limit",
        ));
    }
    resources.push(&mut edges[source], destination)?;
    Ok(())
}

pub(super) fn propagate_exact_local_origins_with_resources_v1<T: Copy + Eq>(
    origins: &mut [Option<T>],
    edges: &[Vec<usize>],
    conflict: &'static str,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    resources.reserve_storage(std::mem::size_of::<Vec<usize>>() + 4096)?;
    if origins.len() != edges.len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "local provenance tables have inconsistent lengths",
        ));
    }
    let mut worklist = Vec::new();
    resources.reserve(&mut worklist, origins.len())?;
    for (local, origin) in origins.iter().enumerate() {
        resources.work(1)?;
        if origin.is_some() {
            resources.push(&mut worklist, local)?;
        }
    }
    let mut head = 0;
    let mut work = 0_usize;
    while let Some(&source) = worklist.get(head) {
        resources.work(1)?;
        head += 1;
        let Some(origin) = origins[source] else {
            continue;
        };
        for &destination in &edges[source] {
            resources.work(1)?;
            work = work
                .checked_add(1)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "local provenance dataflow work accounting overflowed",
                ))?;
            if work > MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "local provenance dataflow exceeds the charged projection limit",
                ));
            }
            match origins[destination] {
                None => {
                    origins[destination] = Some(origin);
                    resources.push(&mut worklist, destination)?;
                }
                Some(existing) if existing == origin => {}
                Some(_) => {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(conflict));
                }
            }
        }
    }
    Ok(())
}

pub(super) fn local_allocation_contracts_with_resources_v1(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    origins: &[Option<u32>],
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<Vec<Option<AllocationContractV1>>, ProductionRankedProjectionErrorV1> {
    resources
        .reserve_storage(2 * std::mem::size_of::<Vec<Option<AllocationContractV1>>>() + 4096)?;
    if origins.len() != function.locals().len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "allocation-origin and semantic-local tables have different lengths",
        ));
    }
    let source_types = function.abi().source_input_types();
    let source_ownership = function.abi().source_argument_ownership();
    let abi_arguments = function.abi().adjusted_arguments();
    if source_ownership.len() != source_types.len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "source ownership and semantic argument tables have different lengths",
        ));
    }
    let mut arguments = resources.filled(source_types.len(), None)?;
    for (argument_index, &ty) in source_types.iter().enumerate() {
        resources.work(24)?;
        let type_decl = types.get(ty.index() as usize).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a kernel argument type is outside the semantic type table",
            ),
        )?;
        let abi_argument = abi_arguments.get(argument_index).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a kernel source argument is missing its authenticated FnAbi record",
            ),
        )?;
        let pointee = abi_argument
            .value()
            .pointee_override()
            .or(type_decl.abi_properties().first_pointee());
        let allocation_origin = u64::try_from(argument_index)
            .ok()
            .and_then(|index| index.checked_add(1))
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "a kernel argument index does not fit the allocation identity space",
            ))?;
        let first_pointer_noalias = match abi_argument.mode() {
            SemanticAbiPassModeV1::Direct(attributes) => attributes.regular().no_alias(),
            SemanticAbiPassModeV1::Pair { first, .. } => first.regular().no_alias(),
            SemanticAbiPassModeV1::Ignore
            | SemanticAbiPassModeV1::Cast { .. }
            | SemanticAbiPassModeV1::Indirect { .. } => false,
        };
        let Some(pointee) = pointee else {
            continue;
        };
        let abi_contract = allocation_contract_from_pointee(
            pointee.kind(),
            first_pointer_noalias,
            allocation_origin,
        );
        let singleton_object = matches!(
            source_ownership[argument_index],
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner
        ) && matches!(type_decl.layout().backend_repr(), SemanticBackendReprV1::Scalar(scalar) if matches!(scalar.primitive(), SemanticBackendPrimitiveV1::Pointer { .. }));
        arguments[argument_index] = Some(authenticated_source_allocation_contract_v1(
            source_ownership[argument_index],
            pointee.kind(),
            AllocationContractV1 {
                singleton_object,
                ..abi_contract
            },
        )?);
    }
    let mut result = Vec::new();
    resources.reserve(&mut result, origins.len())?;
    for origin in origins {
        resources.work(1)?;
        resources.push(
            &mut result,
            origin.and_then(|origin| arguments.get(origin as usize).copied().flatten()),
        )?;
    }
    Ok(result)
}

pub(super) fn assertion_definition_inventory_with_resources_v1(
    function: &SemanticFunctionDeclV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<AssertionDefinitionInventoryV1, ProductionRankedProjectionErrorV1> {
    resources.reserve_storage(
        std::mem::size_of::<AssertionDefinitionInventoryV1>()
            + std::mem::size_of::<Vec<usize>>()
            + 4096,
    )?;
    let local_count = function.locals().len();
    let mut counts = resources.filled(local_count, 0_u8)?;
    let mut assignments = resources.filled(local_count, None)?;
    let mut address_escaped = resources.filled(local_count, false)?;
    let mut blocks = Vec::new();
    resources.reserve(&mut blocks, function.blocks().len())?;
    for (block_index, block) in function.blocks().iter().enumerate() {
        resources.work(4)?;
        let mut definitions = Vec::new();
        let capacity = block
            .statements()
            .len()
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "assertion proof block-definition capacity overflowed",
            ))?;
        resources.reserve(&mut definitions, capacity)?;
        for (statement_index, statement) in block.statements().iter().enumerate() {
            // At most two definition places; their pushes are within the
            // prepaid 2*statements+1 capacity and fixed statement scan.
            resources.work(16)?;
            if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                if assignment.destination().projections().is_empty()
                    && let Some(local) = local_definition_index(assignment.destination())
                {
                    let Some(slot) = assignments.get_mut(local) else {
                        return Err(ProductionRankedProjectionErrorV1::Unsupported(
                            "an assertion proof assignment is outside the semantic local table",
                        ));
                    };
                    if slot.is_none() {
                        *slot = Some(ScalarAssignmentSiteV1 {
                            block: block_index,
                            statement: statement_index,
                        });
                    }
                }
                if let Some(slot) = address_escaped_local_index_v1(assignment.value().kind())
                    .and_then(|local| address_escaped.get_mut(local))
                {
                    *slot = true;
                }
            }
            visit_statement_definition_places(statement.kind(), &mut |place| {
                if let Some(local) = local_definition_index(place) {
                    if let Some(slot) = counts.get_mut(local) {
                        *slot = slot.saturating_add(1);
                    }
                    definitions.push(local);
                }
            });
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(local) = call
                .destination()
                .and_then(|destination| local_definition_index(destination.place()))
        {
            if let Some(slot) = counts.get_mut(local) {
                *slot = slot.saturating_add(1);
            }
            definitions.push(local);
        }
        resources.sort_unique_indices(&mut definitions)?;
        resources.push(&mut blocks, definitions)?;
    }
    for (local, assignment) in assignments.iter_mut().enumerate() {
        resources.work(1)?;
        if counts.get(local).copied() != Some(1) {
            *assignment = None;
        }
    }
    Ok(AssertionDefinitionInventoryV1 {
        counts,
        blocks,
        assignments,
        address_escaped,
    })
}

pub(super) fn constant_locals_with_resources_v1(
    function: &SemanticFunctionDeclV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<Vec<Option<u64>>, ProductionRankedProjectionErrorV1> {
    resources.reserve_storage(
        std::mem::size_of::<Vec<ConstantDefinitionV1>>()
            + std::mem::size_of::<Vec<u8>>()
            + std::mem::size_of::<Vec<Option<u64>>>()
            + std::mem::size_of::<Vec<usize>>()
            + 4096,
    )?;
    let local_count = function.locals().len();
    let mut definitions = resources.filled(local_count, ConstantDefinitionV1::Missing)?;
    for block in function.blocks() {
        resources.work(4)?;
        for statement in block.statements() {
            resources.work(16)?;
            if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                && let Some(definition) = address_escaped_local_index_v1(assignment.value().kind())
                    .and_then(|local| definitions.get_mut(local))
            {
                *definition = ConstantDefinitionV1::Invalid;
            }
            if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                if local_definition_index(assignment.destination()).is_some() {
                    record_constant_definition(
                        &mut definitions,
                        assignment.destination().local(),
                        match (
                            assignment.destination().projections(),
                            assignment.value().kind(),
                        ) {
                            ([], SemanticRvalueKindV1::Use(operand)) => {
                                constant_definition(operand)
                            }
                            _ => ConstantDefinitionV1::Invalid,
                        },
                    );
                }
            } else {
                visit_statement_definition_places(statement.kind(), &mut |place| {
                    if local_definition_index(place).is_some() {
                        record_constant_definition(
                            &mut definitions,
                            place.local(),
                            ConstantDefinitionV1::Invalid,
                        );
                    }
                });
            }
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(destination) = call.destination()
            && local_definition_index(destination.place()).is_some()
        {
            record_constant_definition(
                &mut definitions,
                destination.place().local(),
                ConstantDefinitionV1::Invalid,
            );
        }
    }
    let mut states = resources.filled(local_count, 0_u8)?;
    let mut values = resources.filled(local_count, None)?;
    let mut path = Vec::new();
    resources.reserve(&mut path, local_count)?;
    for index in 0..definitions.len() {
        resolve_constant_iterative_with_resources_v1(
            index,
            &definitions,
            &mut states,
            &mut values,
            &mut path,
            resources,
        )?;
    }
    Ok(values)
}

pub(super) fn resolve_constant_iterative_with_resources_v1(
    index: usize,
    definitions: &[ConstantDefinitionV1],
    states: &mut [u8],
    values: &mut [Option<u64>],
    path: &mut Vec<usize>,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<Option<u64>, ProductionRankedProjectionErrorV1> {
    resources.work(1)?;
    match states.get(index).copied() {
        Some(2) => return Ok(values[index]),
        Some(1) | None => return Ok(None),
        Some(_) => {}
    }

    resources.work(path.len())?;
    path.clear();
    let mut current = index;
    // Constant aliases form a functional graph. Retain one reusable heap path
    // so every traversed node can be finalized without recursive call depth.
    loop {
        resources.work(1)?;
        match states.get(current).copied() {
            Some(2) | Some(1) | None => break,
            Some(_) => {}
        }
        states[current] = 1;
        resources.push(path, current)?;
        match definitions[current] {
            ConstantDefinitionV1::Direct(_) => break,
            ConstantDefinitionV1::Alias(local) => current = local.index() as usize,
            ConstantDefinitionV1::Missing | ConstantDefinitionV1::Invalid => break,
        }
    }

    resources.work(path.len())?;
    for current in path.drain(..).rev() {
        let resolved = match definitions[current] {
            ConstantDefinitionV1::Direct(value) => Some(value),
            ConstantDefinitionV1::Alias(local) => {
                values.get(local.index() as usize).copied().flatten()
            }
            ConstantDefinitionV1::Missing | ConstantDefinitionV1::Invalid => None,
        };
        states[current] = 2;
        values[current] = resolved;
    }
    Ok(values[index])
}
