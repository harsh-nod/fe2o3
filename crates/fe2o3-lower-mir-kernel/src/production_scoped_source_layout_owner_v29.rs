// Admission and replay share the original demand/table producer. The unchanged
// MAIN emitter receives no new backing or reference capability from this stage.
type ScopedSourceLayoutArgumentsV29<'a, 'source, 'work> = (
    &'a ExecutionLifecycleSourceV29<'source>,
    ProductionSemanticKirLimitsV1,
    &'a ScopedSourceCleanupV29,
    &'a mut ArgumentBudgetV1<'work>,
);

fn scoped_source_layout_headers_v29<R, T, Consume, Finish>() -> Result<usize, ArgumentResourceV1> {
    type Error = ProductionSemanticKirErrorV1;
    type Table<'s> = source_storage_v29::SourceStorageLayoutsV29<'s>;
    type Demands<'s> = source_storage_demands_v29::SourceStorageDemandsV29<'s>;
    type ConsumeFrame<'a, 's, 'w, F> = (
        F,
        &'a Demands<'s>,
        &'a mut Table<'s>,
        &'a mut ArgumentBudgetV1<'w>,
    );
    type FinishFrame<'a, 's, 'w, R, F> =
        (F, R, Table<'s>, Demands<'s>, &'a mut ArgumentBudgetV1<'w>);
    argument_sum_v1(&[
        size_of::<Consume>(),
        std::mem::align_of::<Consume>(),
        size_of::<Finish>(),
        std::mem::align_of::<Finish>(),
        size_of::<ScopedSourceLayoutArgumentsV29<'_, '_, '_>>(),
        size_of::<(
            Consume,
            Finish,
            ScopedSourceLayoutArgumentsV29<'_, '_, '_>,
            usize,
        )>(),
        size_of::<
            std::panic::AssertUnwindSafe<(
                Consume,
                Finish,
                ScopedSourceLayoutArgumentsV29<'_, '_, '_>,
                usize,
            )>,
        >(),
        size_of::<ConsumeFrame<'_, '_, '_, Consume>>(),
        size_of::<
            std::panic::AssertUnwindSafe<(
                ConsumeFrame<'_, '_, '_, Consume>,
                &ScopedSourceCleanupV29,
                usize,
            )>,
        >(),
        size_of::<FinishFrame<'_, '_, '_, R, Finish>>(),
        size_of::<Table<'_>>(),
        size_of::<Result<Table<'_>, Error>>(),
        size_of::<Demands<'_>>(),
        size_of::<Result<Demands<'_>, Error>>(),
        size_of::<&[SemanticTypeIdV1]>(),
        size_of::<Result<&[SemanticTypeIdV1], Error>>(),
        size_of::<R>(),
        size_of::<T>(),
        size_of::<Result<R, Error>>(),
        size_of::<Result<T, Error>>(),
        size_of::<std::thread::Result<Result<R, Error>>>(),
        size_of::<std::thread::Result<Result<T, Error>>>(),
        size_of::<Box<dyn std::any::Any + Send>>(),
        4 * size_of::<usize>(),
        size_of::<bool>(),
    ])
}

fn with_scoped_source_layouts_v29<'source, 'work, R, T, Consume, Finish>(
    source: &ExecutionLifecycleSourceV29<'source>,
    limits: ProductionSemanticKirLimitsV1,
    cleanup: &ScopedSourceCleanupV29,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: Consume,
    finish: Finish,
) -> Result<T, ProductionSemanticKirErrorV1>
where
    Consume: FnOnce(
        &source_storage_demands_v29::SourceStorageDemandsV29<'source>,
        &mut source_storage_v29::SourceStorageLayoutsV29<'source>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
    Finish: FnOnce(
        R,
        source_storage_v29::SourceStorageLayoutsV29<'source>,
        source_storage_demands_v29::SourceStorageDemandsV29<'source>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<T, ProductionSemanticKirErrorV1>,
{
    let floor = budget.storage();
    scoped_source_attempt_v29(cleanup, budget, floor, |budget| {
        let catch_header = scoped_source_layout_headers_v29::<R, T, Consume, Finish>()?;
        budget.reserve_storage(catch_header)?;
        let demands =
            source_storage_demands_v29::SourceStorageDemandsV29::collect(source.owner, budget)?;
        let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
            source.owner,
            demands.types(source.owner, budget)?,
            limits.storage_layout_limits(),
            budget,
        )?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let value = consume(&demands, &mut layouts, budget)?;
            #[cfg(test)]
            cleanup.source_fault(floor, budget)?;
            Ok::<_, ProductionSemanticKirErrorV1>(value)
        }));
        cleanup.observe_table(source.owner, &layouts, budget);
        match result {
            Ok(Ok(value)) if !cleanup.is_denied() => {
                let output = finish(value, layouts, demands, budget)?;
                budget.release_storage(catch_header)?;
                Ok(output)
            }
            Ok(Ok(value)) => {
                drop((value, layouts, demands));
                Err(ArgumentResourceV1::Accounting.into())
            }
            Ok(Err(error)) => {
                drop((layouts, demands));
                Err(error)
            }
            Err(payload) => {
                drop((layouts, demands));
                std::panic::resume_unwind(payload)
            }
        }
    })
}

fn scoped_source_candidate_v29(
    source: &ExecutionLifecycleSourceV29<'_>,
    limits: ProductionSemanticKirLimitsV1,
    cleanup: &ScopedSourceCleanupV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(Module, Vec<ScopedModuleRootV29>, usize), ProductionSemanticKirErrorV1> {
    with_scoped_source_layouts_v29(
        source,
        limits,
        cleanup,
        budget,
        |demands, layouts, budget| {
            let emitted = scoped_module_roots_v29(source, demands, layouts, limits, budget)?;
            scoped_module_candidate_v29(source, emitted, limits, budget)
        },
        |(mut candidate, roots), layouts, demands, budget| {
            let row_storage = layouts.install_rows(source.owner, &mut candidate, budget)?;
            demands.discard(budget)?;
            Ok((candidate, roots, row_storage))
        },
    )
}
