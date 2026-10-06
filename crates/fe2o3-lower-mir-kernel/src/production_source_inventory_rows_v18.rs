fn source_block_row_v18<'a>(
    inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
    coordinate: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<&'a fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'a>> {
    budget.charge_work(3)?;
    let function = inventory
        .functions()
        .get(coordinate.function.0 as usize)
        .filter(|row| row.coordinate == coordinate.function)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized block function coordinate",
        ))?;
    function
        .blocks
        .start
        .checked_add(coordinate.block as usize)
        .filter(|index| *index < function.blocks.end)
        .and_then(|index| inventory.blocks().get(index))
        .filter(|row| row.coordinate == coordinate)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized block coordinate",
        ))
}

fn source_operation_row_v18<'a>(
    inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
    coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<&'a fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'a>> {
    let block = source_block_row_v18(inventory, coordinate.block, budget)?;
    budget.charge_work(2)?;
    block
        .operations
        .start
        .checked_add(coordinate.operation as usize)
        .filter(|index| *index < block.operations.end)
        .and_then(|index| inventory.operations().get(index))
        .filter(|row| row.coordinate == coordinate)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized operation coordinate",
        ))
}
