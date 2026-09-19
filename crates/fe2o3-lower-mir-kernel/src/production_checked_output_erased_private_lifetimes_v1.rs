// Included inside private_memory; reuse the exact source kill/Moves algorithm.
// A source span can cross an erased call, so every original operation is mapped.
pub(super) fn source_lifetimes_erased(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    map: &ErasedSourceCoordinateMapV1<'_>,
    proof: &PrivateMemory<'_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<()> {
    let sites = erased_source_statement_sites_v1(source, map, proof.inventory, budget)?;
    source_lifetimes_from_sites(
        source.original_source().semantic_ssa.source_semantic(), proof, &sites, budget,
    )
}

pub(super) fn erased_source_statement_sites_v1(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    map: &ErasedSourceCoordinateMapV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Vec<Option<(SemanticFunctionIdV1, SemanticBlockIdV1, u32)>>> {
    let mut sites = scratch::<Option<(SemanticFunctionIdV1, SemanticBlockIdV1, u32)>>(
        inventory.operations().len(),
        budget,
    )?;
    charge(budget, inventory.operations().len())?;
    sites.resize(inventory.operations().len(), None);
    for span in source
        .original_source()
        .correspondence
        .statement_operation_spans()
    {
        charge(budget, 3)?;
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
            charge(budget, 10)?;
            match map
                .operation(map.inventory().operations()[ordinal].coordinate, budget)
                .map_err(E::SourceOutput)?
            {
                ErasedSourceOccurrenceV1::Retained(coordinate) => {
                    let ordinal = operation_ordinal(inventory, coordinate)?;
                    record_source_statement(
                        &mut sites[ordinal],
                        (
                            span.semantic_function(),
                            span.semantic_block(),
                            span.statement_ordinal(),
                        ),
                    )?;
                }
                ErasedSourceOccurrenceV1::DeletedLocalHelper => {}
                ErasedSourceOccurrenceV1::DeletedUnitCall => {
                    return Err(refused(
                        "private source",
                        "statement cannot authorize an erased Unit call",
                    ));
                }
            }
        }
    }
    Ok(sites)
}
