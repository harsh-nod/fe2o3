// Both nominal prefixes enter the same ranked coverage and pending native
// scope. This helper neither chooses a policy nor constructs a source recipe.
fn with_source_pending_native_v30<E, F, O>(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    layouts: fe2o3_kernel_ir::StorageLayoutLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
    missing: fn(&'static str) -> E,
    observe: O,
    consume: F,
) -> Result<(), E>
where
    E: From<ArgumentResourceV1>
        + From<fe2o3_kernel_analysis::CanonicalRankedViewErrorV1>
        + From<fe2o3_pliron::CanonicalRankedPolicyFailureV1>,
    O: Fn(&E),
    F: FnOnce(
        &mut fe2o3_pliron::PendingCanonicalRankedSourceRolesV18<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), E>,
{
    use fe2o3_kernel_analysis::{
        CanonicalRankedMetadataV18, CanonicalRankedViewErrorV1,
        build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
    };
    let result = (|| {
        let header = source_pending_native_headers_v30::<E, F, O>()?;
        budget.reserve_storage(header)?;
        let metadata = CanonicalRankedMetadataV18::new(inventory.owner(), &[]);
        let metadata_storage = metadata.storage_extent(budget)?;
        budget.reserve_storage(metadata_storage)?;
        let (candidate, receipt) =
            build_canonical_ranked_candidate_v18(inventory, &metadata, budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let mut selected = None;
        let result = with_checked_canonical_ranked_view_v18(
            inventory,
            &metadata,
            &candidate,
            budget,
            |checked, budget| {
                Ok::<_, CanonicalRankedViewErrorV1>(
                    fe2o3_pliron::with_pending_canonical_ranked_source_roles_v18(
                        checked,
                        layouts,
                        budget,
                        |pending, budget| {
                            let result = consume(pending, budget);
                            if let Err(error) = &result {
                                observe(error);
                            }
                            selected = Some(result);
                            Ok(())
                        },
                    ),
                )
            },
        );
        drop(candidate);
        drop(metadata);
        let called = selected.is_some();
        if let Some(Err(error)) = selected {
            return Err(error);
        }
        result??;
        if !called {
            return Err(missing("pending source-role callback absent"));
        }
        budget.release_storage(argument_sum_v1(&[
            header,
            metadata_storage,
            receipt.retained_storage(),
        ])?)?;
        Ok(())
    })();
    if let Err(error) = &result {
        observe(error);
    }
    result
}

fn source_pending_native_headers_v30<E, F, O>() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a, E, F, O> = (
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a mut ArgumentBudgetV1<'a>,
        fe2o3_kernel_ir::StorageLayoutLimitsV1,
        fe2o3_kernel_analysis::CanonicalRankedMetadataV18<'a, 'a>,
        fe2o3_kernel_analysis::CanonicalRankedCandidateV18<'a, 'a, 'a>,
        fe2o3_kernel_analysis::CanonicalRankedCandidateStorageV1,
        Option<Result<(), E>>,
        Result<(), E>,
        F,
        O,
        fn(&'static str) -> E,
        [usize; 4],
        [&'a (); 12],
    );
    argument_sum_v1(&[
        size_of::<Frame<'_, E, F, O>>(),
        std::mem::align_of::<Frame<'_, E, F, O>>(),
        // The checked-view and pending-role APIs separately prepay their
        // concrete callback captures. This is the caller's owned argument frame.
        size_of::<Option<Result<(), E>>>(),
        size_of::<Result<(), E>>(),
        size_of::<O>(),
        std::mem::align_of::<O>(),
        size_of::<
            Result<
                (
                    fe2o3_kernel_analysis::CanonicalRankedCandidateV18<'_, '_, '_>,
                    fe2o3_kernel_analysis::CanonicalRankedCandidateStorageV1,
                ),
                fe2o3_kernel_analysis::CanonicalRankedViewErrorV1,
            >,
        >(),
        size_of::<
            Result<
                Result<(), fe2o3_pliron::CanonicalRankedPolicyFailureV1>,
                fe2o3_kernel_analysis::CanonicalRankedViewErrorV1,
            >,
        >(),
        size_of::<Result<usize, fe2o3_kernel_analysis::CanonicalRankedViewErrorV1>>(),
    ])
}
