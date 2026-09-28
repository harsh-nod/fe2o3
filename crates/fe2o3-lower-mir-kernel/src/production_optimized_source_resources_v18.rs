use super::*;

pub(super) fn headers<T, E, F>(_: &F) -> Result<usize, ArgumentResourceV1> {
    type Capture<'a, 'work, F> = (
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a Transition<'a, 'a, 'a, 'a>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a std::cell::Cell<usize>,
        F,
    );
    type Entry<'a, F> = (Control<'a, 'a, 'a>, SourceIndex, F);
    type EntryResult<'a, F> = SourceOwnedResultV18<Entry<'a, F>>;
    type Invoke<'a, 'work, F> = (
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        F,
    );
    argument_sum_v1(&[
        argument_product_v1(3, size_of::<Capture<'_, '_, F>>())?,
        argument_product_v1(3, std::mem::align_of::<Capture<'_, '_, F>>())?,
        argument_product_v1(
            2,
            size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_, F>>>(),
        )?,
        size_of::<Entry<'_, F>>(),
        argument_product_v1(2, size_of::<EntryResult<'_, F>>())?,
        argument_product_v1(2, size_of::<std::thread::Result<EntryResult<'_, F>>>())?,
        size_of::<std::panic::AssertUnwindSafe<Entry<'_, F>>>(),
        size_of::<std::thread::Result<()>>(),
        size_of::<std::cell::Cell<usize>>(),
        size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        size_of::<SourceOwnedResultV18<()>>(),
        size_of::<Invoke<'_, '_, F>>(),
        size_of::<std::panic::AssertUnwindSafe<Invoke<'_, '_, F>>>(),
        size_of::<ProductionOptimizedSourceCorrespondenceV18<'_>>(),
        size_of::<SourceIndex>(),
        size_of::<std::thread::Result<Result<T, E>>>(),
        size_of::<Result<T, E>>(),
        size_of::<std::panic::AssertUnwindSafe<Result<T, E>>>(),
        3 * size_of::<usize>(),
        source_reference_cleanup_headers_v29()?,
    ])
}

pub(super) fn owned_vector<T>(
    count: usize,
    credit: &std::cell::Cell<usize>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<T>> {
    budget.charge_work(1)?;
    optimized_source_consumer_resources_v18::reserve_entry(
        credit,
        argument_product_v1(count, size_of::<T>())?,
        budget,
    )?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    optimized_source_consumer_resources_v18::reserve_entry(
        credit,
        argument_product_v1(
            rows.capacity()
                .checked_sub(count)
                .ok_or(ArgumentResourceV1::Accounting)?,
            size_of::<T>(),
        )?,
        budget,
    )?;
    Ok(rows)
}

pub(super) fn vector<T>(
    count: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<T>> {
    budget.charge_work(1)?;
    budget.reserve_storage(argument_product_v1(count, size_of::<T>())?)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    budget.reserve_storage(argument_product_v1(
        rows.capacity()
            .checked_sub(count)
            .ok_or(ArgumentResourceV1::Accounting)?,
        size_of::<T>(),
    )?)?;
    Ok(rows)
}

pub(super) fn binding<T>(detail: &'static str) -> SourceOwnedResultV18<T> {
    Err(ProductionSourceOwnedViewErrorV18::Binding(detail))
}

pub(super) fn block_index(
    inventory: &Inventory<'_>,
    coordinate: Block,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    budget.charge_work(3)?;
    let function = inventory
        .functions()
        .get(coordinate.function.0 as usize)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized source function coordinate",
        ))?;
    let index = function
        .blocks
        .start
        .checked_add(coordinate.block as usize)
        .filter(|index| *index < function.blocks.end)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized source block range",
        ))?;
    if inventory
        .blocks()
        .get(index)
        .is_none_or(|row| row.coordinate != coordinate)
    {
        return binding("optimized source block coordinate");
    }
    Ok(index)
}

pub(super) fn operation_index(
    inventory: &Inventory<'_>,
    coordinate: OpCoordinate,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    let block = block_index(inventory, coordinate.block, budget)?;
    budget.charge_work(2)?;
    let range = &inventory.blocks()[block].operations;
    let index = range
        .start
        .checked_add(coordinate.operation as usize)
        .filter(|index| *index < range.end)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized source operation range",
        ))?;
    if inventory
        .operations()
        .get(index)
        .is_none_or(|row| row.coordinate != coordinate)
    {
        return binding("optimized source operation coordinate");
    }
    Ok(index)
}

pub(super) fn definition_index(
    inventory: &Inventory<'_>,
    coordinate: Definition,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    budget.charge_work(1)?;
    let (range, offset) = match coordinate {
        Definition::FunctionArgument { function, argument } => {
            budget.charge_work(1)?;
            let row = inventory.functions().get(function.0 as usize).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("optimized source argument function"),
            )?;
            let end = row
                .definitions
                .start
                .checked_add(row.function.signature.parameters.len())
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            (row.definitions.start..end, argument as usize)
        }
        Definition::BlockArgument { block, argument } => {
            let index = block_index(inventory, block, budget)?;
            (
                inventory.blocks()[index].parameters.clone(),
                argument as usize,
            )
        }
        Definition::Result { operation, result } => {
            let index = operation_index(inventory, operation, budget)?;
            (
                inventory.operations()[index].results.clone(),
                result as usize,
            )
        }
    };
    budget.charge_work(2)?;
    let index = range
        .start
        .checked_add(offset)
        .filter(|index| *index < range.end)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized source definition range",
        ))?;
    if inventory
        .definitions()
        .get(index)
        .is_none_or(|row| row.coordinate != coordinate)
    {
        return binding("optimized source definition coordinate");
    }
    Ok(index)
}
