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
    with_source_ranked_prefix_v1(
        materialized,
        inputs,
        references,
        budget,
        |_source, original, verification, effects, partition, recorders, budget| {
            require_source_functional_roster_v1(verification, inputs.len(), budget).and_then(|()| {
                budget.charge_work(2).map_err(|error| {
                    SourceJoinPipelineErrorV1::RankedProjection(source_join_resource_v1(error))
                })?;
                budget
                    .reserve_storage(std::mem::size_of::<SourceRankedCustodyV1<'_>>())
                    .map_err(|error| {
                        SourceJoinPipelineErrorV1::RankedProjection(source_join_resource_v1(error))
                    })?;
                next(
                    &SourceRankedCustodyV1 {
                        materialized,
                        original,
                        verification,
                        inputs,
                        references,
                        partition,
                        effects,
                        recorders,
                    },
                    budget,
                )
            })
        },
    )
}

// Source-only full projection and R1, without the capability-producing gate.
// Only the existing wrapper above constructs SourceRankedCustodyV1.
fn with_source_ranked_prefix_v1<T>(
    materialized: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    inputs: &[ProductionRankedRootInputV1],
    references: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl for<'scope> FnOnce(
        &'scope RankedProjectionSourceV1<'scope>,
        &'scope fe2o3_lower_mir_kernel::ProductionBorrowedRankedCorrespondenceV1<'scope>,
        &'scope AuthenticatedRankedVerificationRosterV1,
        &'scope DefinedCallableEmptyEffectSummariesV1,
        &'scope [crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1],
        &'scope [canonical_memory_control_v1::CanonicalMemoryControlRecorderV1],
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
                        let result = next(&source, original, verification, effects, partition, &recorders, budget);
                        Ok(result)
                    },
                );
                Ok(result.map_err(SourceJoinPipelineErrorV1::RankedVerification).and_then(|result| result))
            })
        })
    }).map_err(SourceJoinPipelineErrorV1::RankedProjection)?
}

// Additive, local mandatory-rule conjunction only. This does not construct the
// old SourceRankedCustodyV1, satisfy its Some gate, or activate any caller.
// Optional references remain the exact original object; nonempty references
// still require their existing genuine functional/aggregate producer.
#[allow(dead_code, clippy::too_many_arguments)]
fn with_source_preservation_output_v1<'owners>(
    view: &fe2o3_lower_mir_kernel::ProductionSourceOutputOccurrencesV1<'owners, 'owners>,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    inputs: &[ProductionRankedRootInputV1],
    references: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl for<'proof, 'scope, 'formal, 'analysis, 'source, 'output> FnOnce(
        &fe2o3_lower_mir_kernel::ProductionScopedSourcePreservationV1<'proof, 'scope>,
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
        &[fe2o3_lower_mir_kernel::ProductionScopedCompleteFormalMemoryV1<
            'formal,
            'analysis,
            'source,
            'output,
        >],
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        (),
        SourceJoinPipelineErrorV1,
    >,
) -> Result<(), SourceJoinPipelineErrorV1> {
    use fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1 as Output;
    let output_error = |error| {
        SourceJoinPipelineErrorV1::RankedProjection(
            ProductionRankedProjectionErrorV1::CanonicalAssertions(
                canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Output(error),
            ),
        )
    };
    with_source_ranked_prefix_v1(
        view.source(),
        inputs,
        references,
        budget,
        |source, original, verification, effects, partition, recorders, budget| {
            // Complete cardinalities before every indexed or zipped consumer.
            budget
                .charge_work(7)
                .map_err(|error| output_error(Output::Resource(error)))?;
            if inputs.is_empty()
                || original.root_count() != inputs.len()
                || verification.root_count() != inputs.len()
                || partition.len() != inputs.len()
                || recorders.len() != inputs.len()
                || !std::ptr::eq(original.materialized(), view.source())
            {
                return Err(output_error(Output::Invalid(
                    "source preservation consumer roster differs",
                )));
            }
            if !references.as_slice().is_empty() {
                require_source_functional_roster_v1(verification, inputs.len(), budget)?;
            }
            for (ordinal, input) in inputs.iter().enumerate() {
                let verified = &verification.roots()[ordinal];
                budget.charge_work(9usize.checked_add(input.logical_name.len())
                    .and_then(|n| n.checked_add(verified.logical_name().len()))
                    .ok_or_else(|| output_error(Output::Resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)))?)
                    .map_err(|error| output_error(Output::Resource(error)))?;
                if original.roots()[ordinal].selected_root() != verified.semantic_root()
                    || verified.kernel_binding() != &input.kernel_binding
                    || verified.logical_name() != input.logical_name
                    || (references.as_slice().is_empty()
                        && (verified
                            .verification()
                            .has_authenticated_functional_verification()
                            || verified
                                .verification()
                                .aggregate_verus_execution()
                                .is_some()))
                {
                    return Err(output_error(Output::Invalid(
                        "source preservation consumer original root/reference differs",
                    )));
                }
            }
            original.with_source_preservation_v1(budget, |preservation, budget| {
                preservation.require_original_v1(original, budget)?;
                let catalog = view.input_pipeline_catalog(budget)?;
                Ok(with_native_input_relations_v1(view.source(), view.bound(), catalog, profile, budget,
                    |_, _, budget| {
                        checked_output_session_v1::with_checked_output_assertions_view_budget_v1(view, budget, |session| {
                            with_prepared_canonical_memory_session_v1(source, inputs, effects, partition, session, |analyses, budget| {
                                budget.charge_work(2).map_err(Output::Resource)?;
                                if analyses.len() != preservation.roots().len() {
                                    return Err(Output::Invalid("source preservation consumer output roster differs"));
                                }
                                for (ordinal, (analysis, root)) in analyses.iter().zip(preservation.roots()).enumerate() {
                                    budget.charge_work(4).map_err(Output::Resource)?;
                                    if analysis.selected_root() != root.selected_root() {
                                        return Err(Output::Invalid("source preservation consumer output root differs"));
                                    }
                                    analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                                        relation.check_borrowed_ranked_addresses_v1(original, ordinal, recorders[ordinal].candidate(), budget)
                                    })?;
                                }
                                Ok(fe2o3_lower_mir_kernel::with_complete_formal_memory_module_v1(analyses, budget, |formals, budget| {
                                    preservation.require_original_v1(original, budget)
                                        .map_err(fe2o3_lower_mir_kernel::ProductionScopedFormalMemoryErrorV1::SourceOutput)?;
                                    Ok(next(preservation, references, formals, budget))
                                }))
                            })
                        }).map_err(SourceJoinPipelineErrorV1::RankedProjection)?
                            .map_err(|error| SourceJoinPipelineErrorV1::CheckedOutputMemoryTarget(
                                crate::production_pipeline::CheckedOutputMemoryTargetErrorV1::Join(
                                    CheckedOutputModuleJoinErrorV1::Formal(error),
                                ),
                            ))?
                    },
                ))
            }).map_err(output_error)?
        },
    )
}

// An extraction-only consistency diagnostic. No relation or proof owner escapes;
// the final callback receives only observed counts. It is not the Some gate.
pub(crate) fn observe_collected_ranked_addresses_v1<'owners>(
    view: &fe2o3_lower_mir_kernel::ProductionSourceOutputOccurrencesV1<'owners, 'owners>,
    inputs: &[ProductionRankedRootInputV1],
    references: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl FnOnce(
        usize,
        usize,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), String>,
) -> Result<(), String> {
    use fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1 as Output;
    // Entry/empty-reference/input checks and fixed report setup. No reservation
    // precedes the shared prefix's original live source/capture floor checks.
    budget.charge_work(6).map_err(|error| error.to_string())?;
    if !references.as_slice().is_empty() {
        return Err("collected address diagnostic requires original empty reference bindings; source-proof=not-run".to_owned());
    }
    if inputs.is_empty() {
        return Err(
            "collected address diagnostic requires a nonempty original root roster".to_owned(),
        );
    }
    let materialized = view.source();
    with_source_ranked_prefix_v1(
        materialized, inputs, references, budget,
        |source, original, verification, effects, partition, recorders, budget| {
            budget.charge_work(4).map_err(|error| SourceJoinPipelineErrorV1::RankedProjection(source_join_resource_v1(error)))?;
            if original.root_count() != inputs.len()
                || verification.root_count() != inputs.len()
                || recorders.len() != inputs.len()
                || partition.len() != inputs.len()
            {
                return Err(SourceJoinPipelineErrorV1::RankedProjection(
                    ProductionRankedProjectionErrorV1::Incomplete("collected address diagnostic source rosters differ"),
                ));
            }
            budget.reserve_storage(std::mem::size_of::<[usize; 2]>())
                .map_err(|error| SourceJoinPipelineErrorV1::RankedProjection(source_join_resource_v1(error)))?;
            let mut completed = [0usize; 2];
            checked_output_session_v1::with_checked_output_assertions_view_budget_v1(view, budget, |session| {
                with_prepared_canonical_memory_session_v1(source, inputs, effects, partition, session, |analyses, budget| {
                    // This final cardinality check is prepaid by entry work.
                    if analyses.len() != inputs.len() {
                        return Err(Output::Invalid("collected address diagnostic output roster differs"));
                    }
                    for (ordinal, ((analysis, recorder), verified)) in analyses.iter()
                        .zip(recorders).zip(verification.roots()).enumerate()
                    {
                        // Zipped row, None/aggregate check, relation wrapper,
                        // access count and two checked report additions.
                        budget.charge_work(6).map_err(Output::Resource)?;
                        if verified.verification().has_authenticated_functional_verification()
                            || verified.verification().aggregate_verus_execution().is_some()
                        {
                            return Err(Output::Invalid("collected address diagnostic expected actual functional None and absent aggregate"));
                        }
                        let accesses = analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                            relation.check_borrowed_ranked_addresses_v1(original, ordinal, recorder.candidate(), budget)?;
                            Ok(relation.global_access_count())
                        })?;
                        completed[0] = completed[0].checked_add(1).ok_or(Output::Resource(
                            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
                        ))?;
                        completed[1] = completed[1].checked_add(accesses).ok_or(Output::Resource(
                            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
                        ))?;
                    }
                    Ok(())
                })
            }).map_err(SourceJoinPipelineErrorV1::RankedProjection)?;
            budget.charge_work(2).map_err(|error| SourceJoinPipelineErrorV1::RankedProjection(source_join_resource_v1(error)))?;
            // D/P/R2 have completed and dropped. The source/R1 and outer
            // endpoint postflights still precede the driver's final marker.
            Ok(next(completed[0], completed[1], budget))
        },
    ).map_err(|error| error.to_string())?
}
