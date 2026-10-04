// Private consumers must qualify the exact source/P5 prefix and replay the
// sealed P6 continuation first. A generic output graph is never accepted here.
pub(super) fn check_integer_continued_output_v1(
    source: &ProductionSemanticKirOwnerV1,
    checked: &fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    check_integer_continued_context_v1(GeneralSourceContextV1::Direct(source), checked, budget)
}

pub(super) fn check_erased_integer_continued_output_v1(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    checked: &fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    check_integer_continued_context_v1(GeneralSourceContextV1::Erased(source), checked, budget)
}

fn check_integer_continued_context_v1(
    source: GeneralSourceContextV1<'_>,
    checked: &fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    charge(budget, 3)?;
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let qualified = checked.intermediate_policy5().owner();
        let output = checked.owner();
        let (input, storage) =
            CanonicalKirInventoryV1::derive(qualified, budget).map_err(inventory_error)?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(E::Resource)?;
        let (actual, storage) =
            CanonicalKirInventoryV1::derive(output, budget).map_err(inventory_error)?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(E::Resource)?;
        let candidate = checked.continuation().occurrences().candidate();
        let private = private_memory::check(&actual, source.limits().max_operations, budget)?;
        let division = unsigned_division::check(&actual, source.semantic().target(), budget)?;
        let helpers = scalar_helpers::check(&actual, budget)?;
        census::native(
            &actual,
            &private,
            &division,
            &helpers,
            "integer-continued I",
            |ordinal, coordinate| {
                let row = candidate
                    .operations
                    .get(ordinal)
                    .ok_or_else(|| refused("I", "complete operation census"))?;
                if row.output != coordinate {
                    return Err(refused("I", "exact operation coordinate"));
                }
                match row.origin {
                    CanonicalKirOperationOriginV1::Retained(origin) => {
                        let old = &input.operations()[operation_ordinal(&input, origin)?];
                        let new = &actual.operations()[operation_ordinal(&actual, coordinate)?];
                        // DCE may change coordinates. A final trap requires its
                        // exact qualified O origin, not the old numeric ordinal.
                        Ok(old.coordinate == origin && old.operation == new.operation)
                    }
                    CanonicalKirOperationOriginV1::ConstantFrom(_) => Ok(false),
                }
            },
            budget,
        )?;
        let reports =
            derive_checked_output_guarded_obligations_v1(output, source.limits().max_operations)
                .map_err(E::Formal)?;
        census::formal(&actual, &private, &reports, budget)?;
        Ok(reports)
    }));
    if ledger != budget.work_ledger_identity_v1() || budget.storage() < floor {
        drop(result);
        return Err(E::Resource(AssertOriginResourceV1::Accounting));
    }
    budget
        .release_storage(budget.storage() - floor)
        .map_err(E::Resource)?;
    match result {
        Ok(result) => result,
        Err(_) => Err(E::SourceOutput(ProductionSourceOutputErrorV1::Panicked)),
    }
}
