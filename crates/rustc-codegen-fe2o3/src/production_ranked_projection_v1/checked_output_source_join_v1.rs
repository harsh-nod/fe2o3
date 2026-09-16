use crate::production_pipeline::ProductionPipelineError as SourceJoinPipelineErrorV1;

// Borrowed local conjunction only. This is not a source/O functional theorem,
// a final owner, or a replacement for the original authenticated references.
pub(crate) struct SourceRankedCustodyV1<'scope> {
    materialized: &'scope fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    original: &'scope fe2o3_lower_mir_kernel::ProductionBorrowedRankedCorrespondenceV1<'scope>,
    verification: &'scope AuthenticatedRankedVerificationRosterV1,
    inputs: &'scope [ProductionRankedRootInputV1],
    references: &'scope crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    partition: &'scope [crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1],
    effects: &'scope DefinedCallableEmptyEffectSummariesV1,
    recorders: &'scope [canonical_memory_control_v1::CanonicalMemoryControlRecorderV1],
}

struct SourceRankedRootCustodyV1<'scope> {
    source: &'scope SourceRankedCustodyV1<'scope>,
    ordinal: usize,
}

// B1 borrows original source inputs only. No R1 owner, O facts, wire codec or
// independently constructible proof capability crosses this unit-returning API.
pub(crate) fn with_source_export_inputs_v1(
    custody: &SourceRankedCustodyV1<'_>,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl for<'evidence> FnOnce(
        &'evidence fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
        &'evidence [AuthenticatedRankedVerificationRootV1],
        &'evidence [crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1],
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), SourceJoinPipelineErrorV1>,
) -> Result<(), SourceJoinPipelineErrorV1> {
    budget.charge_work(2).map_err(|error| {
        SourceJoinPipelineErrorV1::RankedProjection(source_join_resource_v1(error))
    })?;
    require_source_functional_roster_v1(custody.verification, custody.inputs.len(), budget)?;
    let floor = budget.storage();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        next(
            custody.materialized,
            custody.verification.roots(),
            custody.references.as_slice(),
            budget,
        )
    }));
    let released = budget.storage().checked_sub(floor).ok_or_else(|| {
        SourceJoinPipelineErrorV1::RankedProjection(source_join_resource_v1(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting,
        ))
    })?;
    budget.release_storage(released).map_err(|error| {
        SourceJoinPipelineErrorV1::RankedProjection(source_join_resource_v1(error))
    })?;
    match outcome {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn source_join_resource_v1(
    error: fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1,
) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::CanonicalAssertions(
        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error),
    )
}

fn source_recorders_v1(
    count: usize,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<
    Vec<canonical_memory_control_v1::CanonicalMemoryControlRecorderV1>,
    ProductionRankedProjectionErrorV1,
> {
    use canonical_memory_control_v1::CanonicalMemoryControlRecorderV1 as Recorder;
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    // Header, requested payload and immediate actual-capacity reconciliation.
    budget.charge_work(6).map_err(source_join_resource_v1)?;
    let bytes = count
        .checked_mul(std::mem::size_of::<Recorder>())
        .and_then(|n| n.checked_add(std::mem::size_of::<Vec<Recorder>>()))
        .ok_or_else(|| source_join_resource_v1(Resource::Arithmetic))?;
    budget
        .reserve_storage(bytes)
        .map_err(source_join_resource_v1)?;
    let mut rows = Vec::<Recorder>::new();
    rows.try_reserve_exact(count)
        .map_err(|_| source_join_resource_v1(Resource::Allocation))?;
    let extra = rows
        .capacity()
        .checked_sub(count)
        .and_then(|n| n.checked_mul(std::mem::size_of::<Recorder>()))
        .ok_or_else(|| source_join_resource_v1(Resource::Arithmetic))?;
    budget
        .reserve_storage(extra)
        .map_err(source_join_resource_v1)?;
    Ok(rows)
}

fn require_source_functional_roster_v1(
    verification: &AuthenticatedRankedVerificationRosterV1,
    count: usize,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), SourceJoinPipelineErrorV1> {
    budget.charge_work(2).map_err(|error| {
        SourceJoinPipelineErrorV1::RankedProjection(source_join_resource_v1(error))
    })?;
    if count == 0 || verification.root_count() != count {
        return Err(SourceJoinPipelineErrorV1::RankedVerification(
            ProductionRankedVerificationErrorV1::RosterMetadata(
                "source-first functional roster is incomplete",
            ),
        ));
    }
    for root in verification.roots() {
        budget.charge_work(4).map_err(|error| {
            SourceJoinPipelineErrorV1::RankedProjection(source_join_resource_v1(error))
        })?;
        let verification = root.verification();
        if !verification.has_authenticated_functional_verification()
            || !verification.retained_functional_verification_is_coherent()
            || verification.aggregate_verus_execution().is_none()
        {
            return Err(SourceJoinPipelineErrorV1::RankedVerification(
                ProductionRankedVerificationErrorV1::RosterMetadata(
                    "every source-first root requires retained functional and aggregate custody",
                ),
            ));
        }
    }
    Ok(())
}

impl SourceRankedRootCustodyV1<'_> {
    fn require_v1(
        &self,
        semantic_ssa: &ProductionSemanticSsaOwnerV1,
        selection: SemanticKernelBodySelectionV1,
        input: &ProductionRankedRootInputV1,
        references: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
        facts: &mut impl ProjectedAssertionFactsV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        facts.charge_private_array_work(8)?;
        let root = self.source.original.roots().get(self.ordinal);
        if !std::ptr::eq(semantic_ssa, self.source.materialized.semantic_ssa())
            || !self
                .source
                .inputs
                .get(self.ordinal)
                .is_some_and(|item| std::ptr::eq(item, input))
            || !self
                .source
                .partition
                .get(self.ordinal)
                .is_some_and(|item| std::ptr::eq(item, references))
            || root.is_none_or(|root| root.selected_root() != selection.root())
            || self.source.verification.root_count() != self.source.inputs.len()
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "source-first reference preparation changed its exact root or source borrow",
            ));
        }
        Ok(())
    }
}

/// Source N facts remain source-only; this continuation never borrows O facts.
/// Its result remains inert even after the genuine functional presence gate.
pub(crate) fn with_source_ranked_custody_v1<T>(
    materialized: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    inputs: &[ProductionRankedRootInputV1],
    references: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl for<'scope> FnOnce(
        &SourceRankedCustodyV1<'scope>,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<T, SourceJoinPipelineErrorV1>,
) -> Result<T, SourceJoinPipelineErrorV1> {
    let source = RankedProjectionSourceV1::from_legacy(materialized)
        .map_err(SourceJoinPipelineErrorV1::RankedProjection)?;
    source
        .require_floor(budget)
        .map_err(SourceJoinPipelineErrorV1::RankedProjection)?;
    let capture = materialized.semantic_ssa().occurrence_storage().ok_or(
        SourceJoinPipelineErrorV1::RankedVerification(
            ProductionRankedVerificationErrorV1::RosterMetadata("source-first SSA capture absent"),
        ),
    )?;
    budget.charge_work(4).map_err(|error| {
        SourceJoinPipelineErrorV1::RankedProjection(source_join_resource_v1(error))
    })?;
    let retained = materialized
        .executable_storage()
        .retained_storage()
        .checked_add(materialized.assert_origin_storage().payload_storage())
        .and_then(|bytes| bytes.checked_add(capture.retained_storage()))
        .ok_or_else(|| {
            SourceJoinPipelineErrorV1::RankedProjection(source_join_resource_v1(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
            ))
        })?;
    if budget.storage() < retained {
        return Err(SourceJoinPipelineErrorV1::RankedProjection(
            source_join_resource_v1(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting,
            ),
        ));
    }
    with_ranked_root_preparation_v1(&source, inputs, references, |effects, partition| {
        with_canonical_assertions_source_budget_v1(&source, budget, |session| {
            let mut recorders = session.with_recording_budget_v1(|budget| source_recorders_v1(inputs.len(), budget))?;
            let roots = project_prepared_ranked_roots_v1(
                &source, inputs, partition, |selection, input, source_root, reference| {
                    let root = {
                        let mut facts = session.for_source(source_root.selected_root(), selection.body());
                        let mut recorder = canonical_memory_control_v1::CanonicalMemoryControlRecorderV1::new(&mut facts)?;
                        let root = project_and_verify_ranked_root_control_with_address_claims_v1(
                            source.semantic_ssa(), effects, selection, input, source_root,
                            reference, &mut facts, Some(&mut recorder),
                        )?;
                        if recorders.len() >= recorders.capacity() {
                            return Err(source_join_resource_v1(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting));
                        }
                        recorders.push(recorder);
                        root
                    };
                    session.with_recording_budget_v1(|budget| {
                        budget.charge_work(1).map_err(source_join_resource_v1)?;
                        // The moved header now lives in an already prepaid Vec slot.
                        budget.release_storage(std::mem::size_of::<canonical_memory_control_v1::CanonicalMemoryControlRecorderV1>())
                            .map_err(source_join_resource_v1)
                    })?;
                    Ok(root)
                },
            )?;
            session.with_recording_budget_v1(|budget| {
                let result = with_authenticated_borrowed_ranked_source_roster_v1(
                    materialized, roots, budget, |original, verification, budget| {
                        let result = require_source_functional_roster_v1(verification, inputs.len(), budget)
                            .and_then(|()| {
                                budget.charge_work(2).map_err(|error| SourceJoinPipelineErrorV1::RankedProjection(source_join_resource_v1(error)))?;
                                budget.reserve_storage(std::mem::size_of::<SourceRankedCustodyV1<'_>>())
                                    .map_err(|error| SourceJoinPipelineErrorV1::RankedProjection(source_join_resource_v1(error)))?;
                                next(&SourceRankedCustodyV1 {
                                materialized, original, verification, inputs, references, partition, effects,
                                recorders: &recorders,
                                }, budget)
                            });
                        Ok(result)
                    },
                );
                Ok(result.map_err(SourceJoinPipelineErrorV1::RankedVerification).and_then(|result| result))
            })
        })
    }).map_err(SourceJoinPipelineErrorV1::RankedProjection)?
}
