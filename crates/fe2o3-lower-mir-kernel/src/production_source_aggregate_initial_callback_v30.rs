// Initial source correspondence over the actual first scalar pair. Only
// constructors use rollback; arbitrary callback-owned backing stays paid.
fn aggregate_initial_callback_headers_v30<T, F>() -> Result<usize, ArgumentResourceV1> {
    type Error = ProductionAggregateSourceErrorV30;
    type Frame<'a, 'w, T, F> = (
        SourceCallbackCustodyV29<F>,
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        &'a mut ArgumentBudgetV1<'w>,
        fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'a, 'a, 'a, 'a>,
        fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
        fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
        fe2o3_kernel_analysis::CanonicalKirTransitionStorageV1,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        std::cell::Cell<usize>,
        [usize; 12],
        [SourceOwnedResultV18<()>; 2],
        Result<T, Error>,
        std::thread::Result<Result<T, Error>>,
        Option<SourceOwnedQueryFailureV18>,
        Result<
            (
                (
                    fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
                    fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
                    usize,
                ),
                SourceCallbackCustodyV29<F>,
            ),
            Error,
        >,
        Result<
            (
                fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'a, 'a, 'a, 'a>,
                usize,
            ),
            Error,
        >,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_, '_, T, F>>(),
        std::mem::align_of::<Frame<'_, '_, T, F>>(),
    ])
}

fn with_aggregate_initial_callback_v30<T, F>(
    source: &ProductionSourceOwnedViewV18<'_>,
    chain: &fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: F,
) -> Result<T, ProductionAggregateSourceErrorV30>
where
    F: for<'scope, 'work> FnOnce(
        &ProductionSourceCorrespondenceV18<'scope>,
        &ProductionOptimizedSourceCorrespondenceV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<T, ProductionAggregateSourceErrorV30>,
{
    use ProductionAggregateSourceErrorV30 as Error;
    use fe2o3_kernel_analysis::{
        CanonicalKirInventoryV18 as Inventory, check_canonical_kir_transition_v18,
    };
    let consume = SourceCallbackCustodyV29::new(consume);
    source.check_query_v18(budget)?;
    let floor = budget.storage();
    let slot = std::ptr::from_ref(&*budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let prepared = scoped_source_attempt_v29(source.cleanup, budget, floor, move |budget| {
        consume.prepare(|| {
            let result = (|| {
                let entry = budget.storage();
                let headers = argument_sum_v1(&[
                    aggregate_initial_callback_headers_v30::<T, F>()?,
                    source_callback_custody_finish_preflight_v29::<T, Error>(budget)?,
                    source_owned_finish_preflight_v26::<T, Error>(budget)?,
                ])?;
                budget.reserve_storage(headers)?;
                budget.charge_work(32)?;
                let scalar = chain
                    .rounds()
                    .first()
                    .ok_or_else(|| {
                        source
                            .missing::<()>("nonempty actual Policy12 chain")
                            .unwrap_err()
                    })?
                    .scalar();
                let original = source.canonical(budget)?;
                budget.charge_work(original.canonical_bytes().len())?;
                if scalar.input_audit_bytes() != original.canonical_bytes() {
                    return Err(source
                        .missing::<()>("Policy12 initial scalar source differs")
                        .unwrap_err()
                        .into());
                }
                let (input, input_storage) =
                    Inventory::derive_v18(original, budget).map_err(Error::Inventory)?;
                budget.reserve_storage(input_storage.retained_storage())?;
                let (output, output_storage) =
                    Inventory::derive_v18(scalar.owner(), budget).map_err(Error::Inventory)?;
                budget.reserve_storage(output_storage.retained_storage())?;
                let retained = argument_sum_v1(&[
                    headers,
                    input_storage.retained_storage(),
                    output_storage.retained_storage(),
                ])?;
                if budget.storage() != argument_sum_v1(&[entry, retained])? {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                Ok((input, output, retained))
            })();
            source.retain_aggregate_result_v30(result)
        })
    })?;
    let ((input, output, retained), mut consume) = prepared;
    let credit = std::cell::Cell::new(retained);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // The checked transition borrows the now-owned inventory pair. It has
        // a separate constructor transaction, never the user's callback.
        let transition_floor = budget.storage();
        let (checked, storage) =
            scoped_source_attempt_v29(source.cleanup, budget, transition_floor, |budget| {
                let result = (|| {
                    let entry = budget.storage();
                    let scalar = chain
                        .rounds()
                        .first()
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "nonempty actual Policy12 chain",
                        ))?
                        .scalar();
                    let (checked, receipt) = check_canonical_kir_transition_v18(
                        &input,
                        &output,
                        scalar.occurrences().candidate(),
                        budget,
                    )
                    .map_err(Error::Transition)?;
                    let storage = receipt.retained_storage();
                    budget.reserve_storage(storage)?;
                    if budget.storage() != argument_sum_v1(&[entry, storage])? {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    Ok((checked, storage))
                })();
                source.retain_aggregate_result_v30(result)
            })?;
        // Arithmetic is prepaid before retaining this new credit; on failure
        // the checked backing drops and its known receipt is settled here.
        match argument_sum_v1(&[credit.get(), storage]) {
            Ok(total) => credit.set(total),
            Err(error) => {
                drop(checked);
                budget.release_storage(storage)?;
                return Err(error.into());
            }
        }
        source.with_ranked_correspondence_v18(&input, budget, |original, budget| {
            original.with_optimized_correspondence_v18(&checked, budget, |optimized, budget| {
                let Some(consume) = consume.take() else {
                    source.cleanup.deny_refund();
                    return Err(ArgumentResourceV1::Accounting.into());
                };
                consume(original, optimized, budget)
                    .inspect_err(|error| source.deny_aggregate_accounting_v30(error))
            })
        })
    }));
    let callback_floor = budget.storage();
    let caught = consume.finish(caught);
    if budget.storage() < callback_floor
        || slot != std::ptr::from_ref(&*budget) as usize
        || ledger != budget.work_ledger_identity_v1()
        || floor
            .checked_add(credit.get())
            .is_none_or(|minimum| budget.storage() < minimum)
    {
        source.cleanup.deny_refund();
    }
    let prior = source.guard.first.get();
    let postflight = source.guard.observe_custody(source.cleanup, budget);
    drop(output);
    drop(input);
    let result = source_owned_finish_callback_v18(
        caught,
        prior,
        postflight,
        source.cleanup,
        budget,
        credit.get(),
    );
    source.retain_aggregate_result_v30(result)
}
