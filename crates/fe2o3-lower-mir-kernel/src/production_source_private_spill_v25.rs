// Distinct source provenance for legacy scalar spills. This classifier is not
// access, initializedness, RHS-equivalence or native-completion evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourcePrivateSpillShapeV25 {
    local: u32,
    ty: SemanticTypeIdV1,
    element: PrivateRetainedSlotFactsV1,
}

enum SourcePrivateSpillAccessV26 {
    Removed,
    Global,
    Private(usize, SourcePrivateOperationV18),
}

fn source_private_spill_global_pair_v26(
    before: &Type,
    after: &Type,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<bool> {
    budget.charge_work(4)?;
    let before =
        matches!(before, Type::Pointer(pointer) if pointer.address_space == AddressSpace::Global);
    let after =
        matches!(after, Type::Pointer(pointer) if pointer.address_space == AddressSpace::Global);
    if before != after {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "private spill scope changed Global pointer classification",
        ));
    }
    Ok(before && after)
}

fn source_private_spill_global_origin_v26(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    pointer: ValueId,
    ty: &Type,
    origins: &SourceIssuedGlobalOriginsV26<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<bool> {
    budget.charge_work(5)?;
    match ty {
        Type::Pointer(pointer) if pointer.address_space == AddressSpace::Global => Ok(true),
        Type::Pointer(pointer_type) if pointer_type.address_space == AddressSpace::Generic => {
            let row = inventory
                .functions()
                .get(function.0 as usize)
                .filter(|row| row.coordinate == function)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "private spill Generic pointer function absent",
                ))?;
            origins
                .query(row.function, pointer, budget)
                .map(|origin| origin.is_some())
                .map_err(source_emission_error_v18)
        }
        _ => Ok(false),
    }
}

#[cfg(test)]
#[path = "production_source_private_spill_scope_v26_tests.rs"]
mod spill_scope_v26_tests;

fn source_private_spill_shape_headers_v25() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        source_private_header_v18::<SourcePrivateSpillShapeV25>()?,
        source_private_header_v18::<PrivateRetainedSlotFactsV1>()?,
        source_private_header_v18::<Option<PrivateRetainedSlotFactsV1>>()?,
        source_private_header_v18::<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>()?,
        source_private_header_v18::<&ScopedModuleRootV29>()?,
        source_private_header_v18::<ScopedScalarArraySlotV29>()?,
        source_private_header_v18::<(
            ScopedAllocationIdentityV29,
            ScopedAllocationSourceV29,
            ScopedSlotRepresentationV29,
        )>()?,
        source_private_header_v18::<(usize, u32)>()?,
        source_private_header_v18::<(
            &ProductionSourceCorrespondenceV18<'_>,
            usize,
            usize,
            &ScopedSourceSlotV29,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        source_private_header_v18::<(
            &ScopedSourceSlotV29,
            ScopedScalarArraySlotV29,
            SemanticTypeIdV1,
            PrivateRetainedSlotFactsV1,
        )>()?,
        source_private_header_v18::<SourceCorrespondenceWorkV18<'_, '_>>()?,
        source_private_header_v18::<Option<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>>(
        )?,
        source_private_header_v18::<(usize, &mut ArgumentBudgetV1<'_>)>()?,
    ])
}

fn source_private_spill_rows_headers_v25() -> Result<usize, ArgumentResourceV1> {
    use fe2o3_kernel_analysis::{CanonicalKirDefinitionRefV1, CanonicalKirOperationRefV1};
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1, CanonicalKirOperationCoordinateV1,
        CanonicalKirUseCoordinateV1,
    };
    argument_sum_v1(&[
        source_private_spill_shape_headers_v25()?,
        original_private_expression_headers_v22()?,
        // The existing root frame covers the shared physical/currentness
        // lookup locals. These carriers are additional raw payload/AST frames.
        source_private_header_v18::<ProductionOptimizedSourceMemoryAccessV18<'_>>()?,
        source_private_header_v18::<Option<ProductionOptimizedSourceMemoryAccessV18<'_>>>()?,
        source_private_header_v18::<SourcePhysicalAccessV18<'_>>()?,
        source_private_header_v18::<Option<SourcePhysicalAccessV18<'_>>>()?,
        source_private_header_v18::<SourcePhysicalPayloadV18<'_>>()?,
        source_private_header_v18::<Option<SourcePhysicalPayloadV18<'_>>>()?,
        source_private_header_v18::<ProductionOptimizedSourcePayloadV18>()?,
        source_private_header_v18::<Option<ProductionOptimizedSourcePayloadV18>>()?,
        source_private_header_v18::<ScopedMemoryPayloadV29>()?,
        source_private_header_v18::<ScopedMemoryStoreSourceV29>()?,
        source_private_header_v18::<ScopedMemoryReadV29>()?,
        source_private_header_v18::<(usize, SourcePrivateOperationV18)>()?,
        source_private_header_v18::<SourcePrivateSpillAccessV26>()?,
        source_private_header_v18::<(&Type, &Type, usize, ValueId, ValueId, bool, bool, bool)>()?,
        source_private_header_v18::<(
            &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
            fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
            ValueId,
            &Type,
            &SourceIssuedGlobalOriginsV26<'_>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        source_private_header_v18::<&fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>>()?,
        source_private_header_v18::<(bool, bool, bool)>()?,
        source_private_header_v18::<(
            &SourceIssuedGlobalOriginsV26<'_>,
            &SourceIssuedGlobalOriginsV26<'_>,
        )>()?,
        source_private_header_v18::<(ValueId, ValueId, ValueId, ValueId, bool)>()?,
        source_private_header_v18::<ProductionSemanticExpressionV2>()?,
        source_private_header_v18::<CanonicalKirDefinitionCoordinateV1>()?,
        source_private_header_v18::<CanonicalKirUseCoordinateV1>()?,
        source_private_header_v18::<fe2o3_kernel_analysis::CanonicalKirOutputUseV1>()?,
        source_private_header_v18::<Option<fe2o3_kernel_analysis::CanonicalKirOutputUseV1>>()?,
        source_private_header_v18::<(
            CanonicalKirOperationCoordinateV1,
            CanonicalKirOperationCoordinateV1,
        )>()?,
        source_private_header_v18::<&CanonicalKirOperationRefV1<'_>>()?,
        source_private_header_v18::<&CanonicalKirOperationRefV1<'_>>()?,
        source_private_header_v18::<Option<&CanonicalKirDefinitionRefV1<'_>>>()?,
        source_private_header_v18::<
            Result<Option<usize>, fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1>,
        >()?,
        source_private_header_v18::<TileAttachmentKeyV29>()?,
        source_private_header_v18::<[TileAttachmentLocationV29; 3]>()?,
        source_private_header_v18::<ProductionSourceOperationV18>()?,
        source_private_header_v18::<(Option<(usize, usize)>, usize, usize)>()?,
        source_private_header_v18::<std::slice::Iter<'_, PendingSourceMemoryAlternativeV29>>()?,
        source_private_header_v18::<
            std::iter::Enumerate<std::slice::Iter<'_, ScopedMemoryAnchorV29>>,
        >()?,
        // Fixed helper arguments remain live while their nested paid queries
        // execute. Dynamic expression trees/backing are charged separately.
        source_private_header_v18::<(
            &CheckedSourcePrivatePhysicalV18<'_>,
            &CheckedOptimizedSourceMemoryV18<'_>,
            usize,
            usize,
            usize,
            CanonicalKirOperationCoordinateV1,
            (
                &SourceIssuedGlobalOriginsV26<'_>,
                &SourceIssuedGlobalOriginsV26<'_>,
            ),
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        source_private_header_v18::<(
            &ProductionSourceCorrespondenceV18<'_>,
            usize,
            usize,
            &ScopedMemoryAnchorV29,
            ScopedMemoryStoreSourceV29,
            SourcePrivateSpillShapeV25,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        source_private_header_v18::<(
            &CheckedSourcePrivatePhysicalV18<'_>,
            &CheckedOptimizedSourceMemoryV18<'_>,
            &OriginalEntryIndexV20<'_, '_>,
            &ProductionOptimizedSourceScalarLeavesV18<'_>,
            usize,
            usize,
            &mut [Option<SourcePrivateOperationV18>],
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        source_private_header_v18::<(
            usize,
            &mut [Option<SourcePrivateOperationV18>],
            usize,
            SourcePrivateOperationV18,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        source_private_header_v18::<(
            &ProductionOptimizedSourceCorrespondenceV18<'_>,
            usize,
            CanonicalKirOperationCoordinateV1,
            usize,
            usize,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        source_private_header_v18::<(
            &ProductionOptimizedSourceCorrespondenceV18<'_>,
            usize,
            CanonicalKirOperationCoordinateV1,
            Option<(usize, usize)>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        source_private_header_v18::<(
            &ProductionOptimizedSourceCorrespondenceV18<'_>,
            &usize,
            &CanonicalKirOperationCoordinateV1,
            &Option<(usize, usize)>,
            &mut &mut ArgumentBudgetV1<'_>,
        )>()?,
    ])
}

fn source_private_spill_shape_v25(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    slot: usize,
    backing: &ScopedSourceSlotV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<SourcePrivateSpillShapeV25> {
    original.retain_query((|| {
        original.query(budget)?;
        budget.charge_work(8)?;
        let owner = original.source.root_row(root)?;
        if owner
            .source_slots
            .slots
            .get(slot)
            .is_none_or(|actual| !std::ptr::eq(actual, backing))
        {
            return original
                .source
                .missing("private scalar spill is not the retained original slot");
        }
        let (
            ScopedAllocationIdentityV29::LegacyLocal(local),
            ScopedAllocationSourceV29::Legacy,
            ScopedSlotRepresentationV29::ScalarArray(scalar),
        ) = (
            backing.origin.identity,
            backing.origin.source,
            backing.representation,
        )
        else {
            return original
                .source
                .missing("private scalar spill requires its distinct legacy scalar origin");
        };
        let function = original
            .source
            .instance(root, backing.instance.index(), budget)?
            .0;
        let semantic = original.source.source_semantic(budget)?;
        let declaration = semantic
            .functions()
            .get(function.index() as usize)
            .and_then(|function| function.locals().get(local as usize))
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "private scalar spill original local is absent",
            ))?;
        let element = private_retained_slot_facts_v1(
            semantic.types(),
            declaration.ty(),
            &mut SourceCorrespondenceWorkV18(budget),
        )?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "private scalar spill original scalar layout is unsupported",
        ))?;
        budget.charge_work(8)?;
        if !source_private_spill_extent_v25(backing, scalar, declaration.ty(), element) {
            return original
                .source
                .missing("private scalar spill differs from its original scalar extent");
        }
        Ok(SourcePrivateSpillShapeV25 {
            local,
            ty: declaration.ty(),
            element,
        })
    })())
}

fn source_private_spill_extent_v25(
    backing: &ScopedSourceSlotV29,
    scalar: ScopedScalarArraySlotV29,
    ty: SemanticTypeIdV1,
    element: PrivateRetainedSlotFactsV1,
) -> bool {
    backing.origin.semantic_type == ty
        && scalar.element_type == ty
        && scalar.element == element
        && matches!(element.element, PrivateRetainedElementFactsV1::Scalar(_))
        && scalar.length == 1
        && scalar.count.is_none()
        && scalar.bytes == element.size
        && element.size != 0
}

fn source_private_spill_destination_v25(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    instance: usize,
    anchor: &ScopedMemoryAnchorV29,
    source: ScopedMemoryStoreSourceV29,
    shape: SourcePrivateSpillShapeV25,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<SourcePrivateOperationKindV18> {
    budget.charge_work(12)?;
    let (site, ty) = match source {
        ScopedMemoryStoreSourceV29::EntryArgument { local, ty } => {
            if local.index() != shape.local || ty != shape.ty || anchor.source.is_some() {
                return original
                    .source
                    .missing("private spill entry destination differs");
            }
            return Ok(SourcePrivateOperationKindV18::EntryWrite);
        }
        ScopedMemoryStoreSourceV29::Operand { site, ty, .. }
        | ScopedMemoryStoreSourceV29::Assignment { site, ty } => (site, ty),
        _ => {
            return original
                .source
                .missing("private spill source write family remains unresolved");
        }
    };
    let function = original.source.instance(root, instance, budget)?.0;
    let semantic = original.source.source_semantic(budget)?;
    let ExecutionSiteV29::Statement { block, statement } = site else {
        return original
            .source
            .missing("private spill write has no original assignment");
    };
    let assignment = semantic
        .functions()
        .get(function.index() as usize)
        .and_then(|function| function.blocks().get(block.get() as usize))
        .and_then(|block| block.statements().get(statement as usize))
        .and_then(|statement| match statement.kind() {
            SemanticStatementKindV1::Assign(assignment) => Some(assignment),
            _ => None,
        })
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "private spill original assignment absent",
        ))?;
    if ty != shape.ty
        || anchor.source
            != Some(ScopedMemoryFrameV29 {
                site,
                role: Some(ScopedMemoryRoleV29::Operand(
                    ExecutionOperandV29::Destination,
                )),
            })
        || assignment.destination().local().index() != shape.local
        || !assignment.destination().projections().is_empty()
    {
        return original
            .source
            .missing("private spill write changed its original destination");
    }
    Ok(SourcePrivateOperationKindV18::SourceWrite)
}

// Each call joins one actual original anchor to its checked output occurrence.
// No whole-root lookup is hidden in this local query.
fn source_private_spill_access_v25(
    core: &CheckedSourcePrivatePhysicalV18<'_>,
    currentness: &CheckedOptimizedSourceMemoryV18<'_>,
    root: usize,
    instance: usize,
    anchor: usize,
    input: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    origins: (
        &SourceIssuedGlobalOriginsV26<'_>,
        &SourceIssuedGlobalOriginsV26<'_>,
    ),
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<SourcePrivateSpillAccessV26> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
    let original = core.original;
    core.check(budget)?;
    let transport = core
        .optimized
        .scalar_memory_access_at_v25(root, input, instance, anchor, budget)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "private spill lacks scalar source transport",
        ))?;
    let Some(output) = transport.output() else {
        return Ok(SourcePrivateSpillAccessV26::Removed);
    };
    let inventory = core.optimized.output_inventory(budget)?;
    let before = source_operation_row_v18(original.inventory, input, budget)?.operation;
    let after = source_operation_row_v18(inventory, output, budget)?.operation;
    let (before_pointer, after_pointer, _read) = match (&before.kind, &after.kind) {
        (
            OperationKind::Load {
                pointer: before, ..
            }
            | OperationKind::GuardedLoad {
                pointer: before, ..
            },
            OperationKind::Load { pointer: after, .. }
            | OperationKind::GuardedLoad { pointer: after, .. },
        ) => (*before, *after, true),
        (
            OperationKind::Store {
                pointer: before, ..
            }
            | OperationKind::GuardedStore {
                pointer: before, ..
            },
            OperationKind::Store { pointer: after, .. }
            | OperationKind::GuardedStore { pointer: after, .. },
        ) => (*before, *after, false),
        _ => {
            return original
                .source
                .missing("private spill scalar access family changed");
        }
    };
    let ordinal = source_private_operation_index_v18(inventory, output, budget)?;
    let definition = inventory
        .definition_index_for_value(output.block.function, after_pointer, budget)
        .map_err(|error| {
            ProductionSourceOwnedViewErrorV18::from(
                fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
            )
        })?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "private spill pointer definition absent",
        ))?;
    let original_definition = original
        .inventory
        .definition_index_for_value(input.block.function, before_pointer, budget)
        .map_err(|error| {
            ProductionSourceOwnedViewErrorV18::from(
                fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
            )
        })?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "private spill original pointer definition absent",
        ))?;
    // Generic alone is not a Global classification. Follow the same typed
    // cast/CFG origin walker used by issued-source replay on both actual graphs.
    // This only selects the owner; the separate global census must still prove
    // the exact source root, pointer, guard and access domain.
    let before_type = original.inventory.definitions()[original_definition].ty;
    let after_type = inventory.definitions()[definition].ty;
    budget.charge_work(4)?;
    let global = if matches!(before_type, Type::Pointer(pointer) if pointer.address_space == AddressSpace::Generic)
        || matches!(after_type, Type::Pointer(pointer) if pointer.address_space == AddressSpace::Generic)
    {
        let before = source_private_spill_global_origin_v26(
            original.inventory,
            input.block.function,
            before_pointer,
            before_type,
            origins.0,
            budget,
        )?;
        let after = source_private_spill_global_origin_v26(
            inventory,
            output.block.function,
            after_pointer,
            after_type,
            origins.1,
            budget,
        )?;
        budget.charge_work(2)?;
        if before != after {
            return original
                .source
                .missing("private spill scope changed Global pointer classification");
        }
        before && after
    } else {
        source_private_spill_global_pair_v26(before_type, after_type, budget)?
    };
    if global {
        #[cfg(test)]
        SPILL_GLOBAL_VISITS_V26.with(|visits| {
            if let Some(mut counts) = visits.get() {
                assert!(!core.physical.operation(ordinal));
                assert!(core.physical.address(definition).is_none());
                counts[usize::from(!_read)] += 1;
                visits.set(Some(counts));
            }
        });
        return Ok(SourcePrivateSpillAccessV26::Global);
    }
    let (before_pointer, after_pointer, before_value, after_value, read) =
        match (&before.kind, &after.kind) {
            (
                OperationKind::Load {
                    pointer: before_pointer,
                    access: before_access,
                },
                OperationKind::Load {
                    pointer: after_pointer,
                    access: after_access,
                },
            ) => {
                let ([before_result], [after_result]) =
                    (before.results.as_slice(), after.results.as_slice())
                else {
                    return original
                        .source
                        .missing("private spill Load result census differs");
                };
                if before_access != after_access || before_access.volatile {
                    return original.source.missing("private spill Load access changed");
                }
                (
                    *before_pointer,
                    *after_pointer,
                    before_result.id,
                    after_result.id,
                    true,
                )
            }
            (
                OperationKind::Store {
                    pointer: before_pointer,
                    value: before_value,
                    access: before_access,
                },
                OperationKind::Store {
                    pointer: after_pointer,
                    value: after_value,
                    access: after_access,
                },
            ) => {
                if before_access != after_access || before_access.volatile {
                    return original
                        .source
                        .missing("private spill Store access changed");
                }
                (
                    *before_pointer,
                    *after_pointer,
                    *before_value,
                    *after_value,
                    false,
                )
            }
            _ => {
                return original
                    .source
                    .missing("private spill requires an ordinary Load or Store");
            }
        };
    let address =
        core.physical
            .address(definition)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "private spill has no checked physical address",
            ))?;
    let allocation = core
        .allocations
        .get(address.allocation())
        .and_then(Option::as_ref)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "private spill has no original allocation",
        ))?;
    let shape = source_private_spill_shape_v25(
        original,
        root,
        allocation.slot,
        allocation.original,
        budget,
    )?;
    let access = source_private_current_occurrence_v25(
        currentness,
        original,
        core.optimized,
        root,
        instance,
        input,
        output,
        before_pointer,
        after_pointer,
        budget,
    )?;
    budget.charge_work(14)?;
    if !core.physical.operation(ordinal)
        || allocation.root != root
        || allocation.slot != access.physical.slot
        || allocation.original.instance.index() != instance
        || allocation.pointer != after_pointer
        || access.instance.index() != instance
        || access.anchor != anchor
        || access.safe_object.is_some()
        || access.object_location.is_some()
        || address.length() != 1
        || address.offset() != 0
        || address.stride()
            != usize::try_from(shape.element.size).map_err(|_| ArgumentResourceV1::Arithmetic)?
        || address.alignment() != shape.element.alignment
    {
        return original
            .source
            .missing("private spill source occurrence or physical cell differs");
    }
    let alternatives = currentness
        .original
        .pending
        .alternatives
        .get(access.alternatives.clone())
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "private spill activation range absent",
        ))?;
    if alternatives.is_empty() {
        return original
            .source
            .missing("private spill has no original activation");
    }
    for alternative in alternatives {
        budget.charge_work(4)?;
        if alternative.instance.index() != instance
            || alternative.local.index() != shape.local
            || alternative.slot != allocation.slot
            || alternative.formation.is_some()
        {
            return original
                .source
                .missing("private spill activation changed original local");
        }
    }
    let sidecar = original.source.sidecar(root, instance, budget)?;
    let source = sidecar
        .scoped_memory_anchors
        .as_ref()
        .and_then(|anchors| anchors.rows.get(anchor))
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "private spill original anchor absent",
        ))?;
    let ScopedMemoryAnchorKindV29::Access {
        payload: Some(payload),
        ..
    } = source.kind
    else {
        return original
            .source
            .missing("private spill scalar payload absent");
    };
    let kind = match (read, payload, transport.payload()) {
        (
            true,
            ScopedMemoryPayloadV29::Load { read, .. },
            Some(ProductionOptimizedSourcePayloadV18::Load { output: actual }),
        ) => {
            if read.ty != shape.ty
                || actual
                    != (Definition::Result {
                        operation: output,
                        result: 0,
                    })
            {
                return original
                    .source
                    .missing("private spill read type or result differs");
            }
            let writer = core
                .physical
                .latest_stores()
                .get(ordinal)
                .copied()
                .flatten()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "private spill read has no exact intersected Store",
                ))?;
            SourcePrivateOperationKindV18::Read { writer }
        }
        (
            false,
            ScopedMemoryPayloadV29::Store { source: input, .. },
            Some(ProductionOptimizedSourcePayloadV18::Store { .. }),
        ) => source_private_spill_destination_v25(
            original, root, instance, source, input, shape, budget,
        )?,
        _ => {
            return original
                .source
                .missing("private spill original scalar role differs");
        }
    };
    Ok(SourcePrivateSpillAccessV26::Private(
        ordinal,
        SourcePrivateOperationV18 {
            root,
            instance,
            anchor: Some(anchor),
            input,
            output,
            allocation: address.allocation(),
            kind,
            input_value: Some(before_value),
            output_value: Some(after_value),
        },
    ))
}

fn source_private_spill_install_v25(
    first: usize,
    rows: &mut [Option<SourcePrivateOperationV18>],
    ordinal: usize,
    row: SourcePrivateOperationV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    budget.charge_work(2)?;
    let relative = ordinal
        .checked_sub(first)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let target = rows
        .get_mut(relative)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "private spill operation belongs to another root",
        ))?;
    if target.replace(row).is_some() {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "private spill operation has repeated source coverage",
        ));
    }
    Ok(())
}

fn source_private_spill_rows_v25(
    core: &CheckedSourcePrivatePhysicalV18<'_>,
    currentness: &CheckedOptimizedSourceMemoryV18<'_>,
    index: &OriginalEntryIndexV20<'_, '_>,
    leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
    root: usize,
    first: usize,
    rows: &mut [Option<SourcePrivateOperationV18>],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let original = core.original;
    core.check(budget)?;
    currentness.check_scope_v18(original, core.optimized, root, budget)?;
    index.check(budget)?;
    let source = leaves.original_leaves(budget)?;
    budget.charge_work(3)?;
    if !std::ptr::eq(index.source, original)
        || !std::ptr::eq(leaves.optimized, core.optimized)
        || !std::ptr::eq(source.leaves.relation, original)
        || source.leaves.root != root
    {
        return original
            .source
            .missing("private spill expression owners differ");
    }
    let original_function = original
        .inventory
        .functions()
        .get(original.source.root_row(root)?.function_ordinal)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "private spill original root function absent",
        ))?
        .function;
    let output_function =
        optimized_source_root_function_v18(original, core.optimized, root, budget)?.function;
    let before_origins = SourceIssuedGlobalOriginsV26::prepare(original_function, budget)
        .map_err(source_emission_error_v18)?;
    let after_origins = SourceIssuedGlobalOriginsV26::prepare(output_function, budget)
        .map_err(source_emission_error_v18)?;
    let visit = |disposition: &ProductionOptimizedSourceScalarStoreDispositionV18<'_>,
                 budget: &mut ArgumentBudgetV1<'_>| {
        let ProductionOptimizedSourceScalarStoreDispositionV18::Retained(request) = disposition
        else {
            return Ok(());
        };
        let floor = budget.storage();
        scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
            let (ordinal, row) = match source_private_spill_access_v25(
                core,
                currentness,
                root,
                request.original.instance,
                request.original.row,
                request.original.operation,
                (&before_origins, &after_origins),
                budget,
            )? {
                SourcePrivateSpillAccessV26::Global => return Ok(()),
                SourcePrivateSpillAccessV26::Private(ordinal, row) => (ordinal, row),
                SourcePrivateSpillAccessV26::Removed => {
                    return original
                        .source
                        .missing("retained private spill Store became removed");
                }
            };
            if row.output != request.output
                || row.input_value != Some(request.original.value)
                || row.output_value != Some(request.value)
                || !matches!(
                    row.kind,
                    SourcePrivateOperationKindV18::EntryWrite
                        | SourcePrivateOperationKindV18::SourceWrite
                )
            {
                return original
                    .source
                    .missing("private spill Store request differs from its source access");
            }
            let expression = index.store_source_expression_v23(leaves, request, budget)?;
            request.original.check_expression(&expression, budget)?;
            request.check_expression(&expression, budget)?;
            drop(expression);
            source_private_spill_install_v25(first, rows, ordinal, row, budget)
        })?;
        original.retain_query(
            budget
                .release_storage(
                    budget
                        .storage()
                        .checked_sub(floor)
                        .ok_or(ArgumentResourceV1::Accounting)?,
                )
                .map_err(Into::into),
        )
    };
    let headers = argument_sum_v1(&[
        std::mem::size_of_val(&visit),
        std::mem::align_of_val(&visit),
    ])?;
    original.retain_query(budget.reserve_storage(headers).map_err(Into::into))?;
    leaves.visit_store_inputs(budget, visit)?;
    let owner = original.source.root_row(root)?;
    for source in &owner.coordinates.sources.rows {
        budget.charge_work(1)?;
        let instance = source.instance.index();
        let Some(sidecar) = original.source.optional_sidecar(root, instance, budget)? else {
            continue;
        };
        let anchors = sidecar.scoped_memory_anchors.as_ref().ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("private spill source anchor census absent"),
        )?;
        for (anchor, row) in anchors.rows.iter().enumerate() {
            budget.charge_work(1)?;
            if !matches!(
                row.kind,
                ScopedMemoryAnchorKindV29::Access {
                    payload: Some(ScopedMemoryPayloadV29::Load { .. }),
                    ..
                }
            ) {
                continue;
            }
            let [position] = original.attachment_range(
                TileAttachmentKeyV29 {
                    root,
                    family: TileAttachmentFamilyV29::MemoryAnchor,
                    instance,
                    row: anchor,
                    field: TileAttachmentFieldV29::MemoryPosition,
                    component: 0,
                    part: 0,
                },
                budget,
            )?
            else {
                return original
                    .source
                    .missing("private spill read position census differs");
            };
            let ProductionSourceOperationV18::Operation(input) =
                original.mapped_source_operation(position.location, budget)?
            else {
                return original
                    .source
                    .missing("private spill read lost original operation");
            };
            if let SourcePrivateSpillAccessV26::Private(ordinal, row) =
                source_private_spill_access_v25(
                    core,
                    currentness,
                    root,
                    instance,
                    anchor,
                    input,
                    (&before_origins, &after_origins),
                    budget,
                )?
            {
                if !matches!(row.kind, SourcePrivateOperationKindV18::Read { .. }) {
                    return original.source.missing("private spill read became a write");
                }
                source_private_spill_install_v25(first, rows, ordinal, row, budget)?;
            }
        }
    }
    index.check(budget)?;
    core.check(budget)?;
    #[cfg(test)]
    test_spill_rows_v25(core, root, first, rows, budget)?;
    Ok(())
}

impl CheckedSourcePrivatePhysicalV18<'_> {
    pub(super) fn with_root_memory_spills_v25<'work, T, E>(
        &self,
        root: usize,
        currentness: &CheckedOptimizedSourceMemoryV18<'_>,
        entries: &ProductionCheckedSourceEntryWritesV18<'_>,
        index: &OriginalEntryIndexV20<'_, '_>,
        leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &CheckedSourcePrivateMemoryV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.with_root_memory_extend_v25(
            root,
            currentness,
            entries,
            budget,
            |first, rows, budget| {
                self.original.retain_query(
                    budget
                        .reserve_storage(source_private_spill_rows_headers_v25()?)
                        .map_err(Into::into),
                )?;
                source_private_spill_rows_v25(
                    self,
                    currentness,
                    index,
                    leaves,
                    root,
                    first,
                    rows,
                    budget,
                )
            },
            consume,
        )
    }
}

#[cfg(test)]
thread_local! {
    pub(super) static SPILL_FAULT_V25: std::cell::Cell<Option<u8>> = const { std::cell::Cell::new(None) };
    pub(super) static SPILL_FAULT_FINISHED_V25: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(super) static SPILL_GLOBAL_VISITS_V26: std::cell::Cell<Option<[usize; 2]>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn test_spill_rows_v25(
    core: &CheckedSourcePrivatePhysicalV18<'_>,
    root: usize,
    first: usize,
    rows: &mut [Option<SourcePrivateOperationV18>],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let Some(fault) = SPILL_FAULT_V25.with(std::cell::Cell::get) else {
        return Ok(());
    };
    if root != 0 {
        return Ok(());
    }
    budget.reserve_storage(source_private_header_v18::<(
        [usize; 8],
        [Option<SourcePrivateOperationV18>; 2],
    )>()?)?;
    let mut selected = None;
    for (relative, row) in rows.iter().enumerate() {
        budget.charge_work(3)?;
        if let Some(row) = row
            && matches!(row.kind, SourcePrivateOperationKindV18::Read { .. })
            && core.allocations[row.allocation].is_some_and(|allocation| {
                matches!(
                    allocation.original.origin.identity,
                    ScopedAllocationIdentityV29::LegacyLocal(_)
                )
            })
        {
            selected = Some((relative, *row));
            break;
        }
    }
    let (relative, row) = selected.expect("actual completed scalar spill read");
    match fault {
        0 => {}
        1 => rows[relative] = None,
        2 => {
            let error =
                source_private_spill_install_v25(first, rows, first + relative, row, budget)
                    .unwrap_err();
            assert!(matches!(
                error,
                ProductionSourceOwnedViewErrorV18::Binding(
                    "private spill operation has repeated source coverage"
                )
            ));
            SPILL_FAULT_FINISHED_V25.with(|flag| flag.set(true));
            return Err(error);
        }
        3 => {
            rows[relative].as_mut().unwrap().kind = SourcePrivateOperationKindV18::Read {
                writer: first + relative,
            }
        }
        4 => rows[relative].as_mut().unwrap().root = usize::MAX,
        _ => panic!("unknown spill row mutation"),
    }
    SPILL_FAULT_FINISHED_V25.with(|flag| flag.set(true));
    Ok(())
}

#[cfg(test)]
pub(super) fn test_spill_shape_v25(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    slot: usize,
    copy: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    original.query(budget)?;
    budget.reserve_storage(source_private_spill_shape_headers_v25()?)?;
    let backing = &original.source.root_row(root)?.source_slots.slots[slot];
    let copied = *backing;
    let actual = if copy { &copied } else { backing };
    let shape = source_private_spill_shape_v25(original, root, slot, actual, budget)?;
    assert_eq!(
        shape.local,
        match backing.origin.identity {
            ScopedAllocationIdentityV29::LegacyLocal(local) => local,
            _ => panic!("not a genuine scalar spill"),
        }
    );
    let ScopedSlotRepresentationV29::ScalarArray(scalar) = backing.representation else {
        panic!("genuine scalar spill representation");
    };
    for fault in 0..8 {
        let mut changed = scalar;
        let mut expected = shape.element;
        match fault {
            0 => changed.length = 2,
            1 => changed.bytes += 1,
            2 => changed.element_type = SemanticTypeIdV1::from_index(u32::MAX),
            3 => changed.element.alignment += 1,
            4 => changed.element.size += 1,
            5 => {
                expected.size = 0;
                changed.element = expected;
                changed.bytes = 0;
            }
            6 => {
                expected.element = PrivateRetainedElementFactsV1::ThinPointer {
                    element: ScalarType::U32,
                    space: AddressSpace::Private,
                    access: AccessMode::ReadWrite,
                };
                changed.element = expected;
            }
            7 => changed.count = Some((backing.origin.pointer, backing.allocation)),
            _ => unreachable!(),
        }
        assert!(!source_private_spill_extent_v25(
            backing, changed, shape.ty, expected
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn test_spill_query_headers_v25() -> Result<usize, ArgumentResourceV1> {
    source_private_spill_rows_headers_v25()
}

#[cfg(test)]
pub(super) fn test_spill_entry_destination_v25(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    instance: usize,
    slot: usize,
    anchor: usize,
    short: bool,
    limit: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    original.query(budget)?;
    budget.reserve_storage(source_private_spill_rows_headers_v25()?)?;
    let backing = &original.source.root_row(root)?.source_slots.slots[slot];
    let shape = source_private_spill_shape_v25(original, root, slot, backing, budget)?;
    let source = &original
        .source
        .sidecar(root, instance, budget)?
        .scoped_memory_anchors
        .as_ref()
        .unwrap()
        .rows[anchor];
    let ScopedMemoryAnchorKindV29::Access {
        payload:
            Some(ScopedMemoryPayloadV29::Store {
                source: value @ ScopedMemoryStoreSourceV29::EntryArgument { .. },
                ..
            }),
        ..
    } = source.kind
    else {
        panic!("actual spill entry Store anchor");
    };
    let remaining = 12 - usize::from(short);
    budget.charge_work(limit - budget.work() - remaining)?;
    let before = budget.work();
    let result = original.retain_query(source_private_spill_destination_v25(
        original, root, instance, source, value, shape, budget,
    ));
    if short {
        assert!(
            matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Work(refusal)
        )) if refusal.actual() == limit + 1 && refusal.limit() == limit)
        );
    } else {
        assert!(matches!(
            result,
            Ok(SourcePrivateOperationKindV18::EntryWrite)
        ));
        assert_eq!(budget.work() - before, 12);
    }
    // Exact success has intentionally spent the work limit, so return a typed
    // sentinel instead of allowing unrelated outer postflight queries to run.
    result.map(|_| ()).and_then(|_| {
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "selected spill entry boundary stop",
        ))
    })
}
