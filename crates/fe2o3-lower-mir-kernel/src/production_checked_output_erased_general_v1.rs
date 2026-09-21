// Every source disposition below is queried inside a fresh same-owner N/E
// scope. The original grammar still includes deleted source helper bodies.
#[allow(clippy::too_many_arguments)]
fn check_erased_source_outputs_v1(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    coordinates: &fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    input: &CanonicalKirInventoryV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    with_erased_source_output_occurrences_v1(
        source,
        coordinates,
        checked,
        transition,
        control,
        budget,
        |view, budget| {
            Ok(check_erased_source_view_outputs_v1(
                source,
                view,
                checked.occurrences().candidate(),
                input,
                output,
                budget,
            ))
        },
    )
    .map_err(E::SourceOutput)?
}

#[allow(clippy::too_many_arguments)]
fn check_erased_source_pair_outputs_v1(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    coordinates: &fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1<'_, '_>,
    actual: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    audit: &[u8],
    required: usize,
    transition: &fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>,
    control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    input: &CanonicalKirInventoryV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    with_erased_source_output_pair_v1(
        source,
        coordinates,
        actual,
        rows,
        audit,
        required,
        transition,
        control,
        budget,
        |view, budget| {
            Ok(check_erased_source_view_outputs_v1(
                source, view, rows, input, output, budget,
            ))
        },
    )
    .map_err(E::SourceOutput)?
}

fn check_erased_source_view_outputs_v1(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    view: &ErasedSourceOutputOccurrencesV1<'_>,
    rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    input: &CanonicalKirInventoryV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    // The owning and decoded routes share the genuine erasure callback. No
    // producer witness, empty catalog or source permission predicate is forged.
    erased_general_scratch_v1(budget, |budget| {
        let map = view.coordinates();
        erased_source_associations_v1(source, map, budget)?;
        for row in source.original_source().correspondence.blocks() {
            charge(budget, 1)?;
            match view
                .block(
                    row.correspondence_owner(),
                    row.semantic_function(),
                    row.semantic_block(),
                    budget,
                )
                .map_err(E::SourceOutput)?
            {
                ErasedSourceBlockV1::Retained { .. }
                | ErasedSourceBlockV1::DeletedLocalHelper { .. } => {}
                ErasedSourceBlockV1::NotMaterialized => {
                    return Err(refused(
                        "erased source",
                        "complete materialized block mapping",
                    ));
                }
            }
        }
        let traps = erased_source_spans_v1(source, map, input, budget)?;
        let private_input = private_memory::check(
            input,
            source.original_source().limits.max_operations,
            budget,
        )?;
        private_memory::source_lifetimes_erased(source, map, &private_input, budget)?;
        check_native_outputs_v1(
            GeneralSourceContextV1::Erased(source),
            input,
            output,
            rows,
            &private_input,
            &traps,
            budget,
        )
    })
}

fn erased_source_associations_v1(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    map: &ErasedSourceCoordinateMapV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    for row in source.original_source().correspondence.lowered_functions() {
        charge(budget, 2)?;
        match map
            .function(row.correspondence_owner(), row.semantic_function(), budget)
            .map_err(E::SourceOutput)?
        {
            ErasedSourceOccurrenceV1::Retained(_) => {}
            ErasedSourceOccurrenceV1::DeletedLocalHelper
                if row.role() == SemanticKirFunctionRoleV1::InternalHelper => {}
            _ => {
                return Err(refused(
                    "erased source",
                    "complete retained/deleted source role partition",
                ));
            }
        }
    }
    Ok(())
}

// Spans address original N. After deletion a span need not have a contiguous
// E range, so callers must map each individual operation, including zero-length
// range validation here. No first-coordinate-plus-count rebasing is allowed.
#[allow(clippy::too_many_arguments)]
fn erased_original_span_v1(
    map: &ErasedSourceCoordinateMapV1<'_>,
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    block: BlockId,
    first: u32,
    count: u32,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<std::ops::Range<usize>> {
    let function = map
        .source_function(root, function, budget)
        .map_err(E::SourceOutput)?;
    let block = map
        .inventory()
        .block_for_id(function, block, budget)
        .map_err(inventory_error)?
        .ok_or_else(|| refused("erased source", "exact original span block"))?;
    charge(budget, 4)?;
    let start = block
        .operations
        .start
        .checked_add(first as usize)
        .ok_or_else(arithmetic)?;
    let end = start.checked_add(count as usize).ok_or_else(arithmetic)?;
    if start > block.operations.end || end > block.operations.end {
        return Err(refused("erased source", "bounded original operation span"));
    }
    Ok(start..end)
}

fn erased_source_spans_v1(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    map: &ErasedSourceCoordinateMapV1<'_>,
    input: &CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Vec<u8>> {
    let original = source.original_source();
    let correspondence = &original.correspondence;
    let mut traps = scratch::<u8>(input.operations().len(), budget)?;
    charge(budget, input.operations().len())?;
    traps.resize(input.operations().len(), 0);
    for span in correspondence.statement_operation_spans() {
        charge(budget, 2)?;
        let range = erased_original_span_v1(
            map,
            span.correspondence_owner(),
            span.semantic_function(),
            span.kernel_ir_block(),
            span.first_operation_ordinal(),
            span.operation_count(),
            budget,
        )?;
        for ordinal in range {
            charge(budget, 9)?;
            match map
                .operation(map.inventory().operations()[ordinal].coordinate, budget)
                .map_err(E::SourceOutput)?
            {
                ErasedSourceOccurrenceV1::Retained(coordinate) => {
                    operation_ordinal(input, coordinate)?;
                }
                ErasedSourceOccurrenceV1::DeletedLocalHelper => {}
                ErasedSourceOccurrenceV1::DeletedUnitCall => {
                    return Err(refused(
                        "erased source",
                        "statement is not a removed call terminator",
                    ));
                }
            }
        }
    }
    for span in correspondence.terminator_operation_spans() {
        charge(budget, 2)?;
        let range = erased_original_span_v1(
            map,
            span.correspondence_owner(),
            span.semantic_function(),
            span.kernel_ir_block(),
            span.first_operation_ordinal(),
            span.operation_count(),
            budget,
        )?;
        for ordinal in range {
            charge(budget, 9)?;
            match map
                .operation(map.inventory().operations()[ordinal].coordinate, budget)
                .map_err(E::SourceOutput)?
            {
                ErasedSourceOccurrenceV1::Retained(coordinate) => {
                    operation_ordinal(input, coordinate)?;
                }
                ErasedSourceOccurrenceV1::DeletedLocalHelper => {}
                ErasedSourceOccurrenceV1::DeletedUnitCall => {
                    charge(budget, 3)?;
                    let source_block = original
                        .semantic_ssa
                        .source_semantic()
                        .functions()
                        .get(span.semantic_function().index() as usize)
                        .and_then(|function| {
                            function
                                .blocks()
                                .get(span.semantic_block().index() as usize)
                        })
                        .ok_or_else(|| refused("erased source", "removed call source block"))?;
                    if !matches!(
                        source_block.terminator().kind(),
                        SemanticTerminatorKindV1::Call(_)
                    ) {
                        return Err(refused(
                            "erased source",
                            "removed operation is an exact source Call",
                        ));
                    }
                }
            }
        }
    }
    for span in correspondence.synthetic_operation_spans() {
        charge(budget, 2)?;
        let trap = match span.rule() {
            SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap => {
                if span.operation_count() != 1 {
                    return Err(refused("erased source", "one exact synthetic trap"));
                }
                true
            }
            SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage
            | SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage => false,
        };
        let range = erased_original_span_v1(
            map,
            span.correspondence_owner(),
            span.semantic_function(),
            span.kernel_ir_block(),
            span.first_operation_ordinal(),
            span.operation_count(),
            budget,
        )?;
        for ordinal in range {
            charge(budget, 9)?;
            match map
                .operation(map.inventory().operations()[ordinal].coordinate, budget)
                .map_err(E::SourceOutput)?
            {
                ErasedSourceOccurrenceV1::Retained(coordinate) => {
                    let retained = operation_ordinal(input, coordinate)?;
                    if trap {
                        // Repeated root aliases of the exact freshly replayed
                        // source assertion may name one physical retained trap.
                        traps[retained] = 1;
                    }
                }
                ErasedSourceOccurrenceV1::DeletedLocalHelper => {}
                ErasedSourceOccurrenceV1::DeletedUnitCall => {
                    return Err(refused(
                        "erased source",
                        "synthetic operation is not a removed call",
                    ));
                }
            }
        }
    }
    Ok(traps)
}
