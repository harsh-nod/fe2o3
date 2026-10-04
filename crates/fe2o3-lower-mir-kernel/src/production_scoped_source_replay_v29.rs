// These attachments retain call-instance identity and full-module coordinates.
// They remain pending source-value equivalence, elision checks and discharge.

#[cfg(test)]
#[derive(Clone, Copy)]
enum SourceEmissionPhaseV29 {
    Admission,
    ConstructionReplay,
    ConsumerReplay,
}

#[cfg(test)]
#[derive(Clone, Copy)]
struct SourceEmissionObservationV29 {
    phase: SourceEmissionPhaseV29,
    owner: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

#[cfg(test)]
thread_local! {
    static SOURCE_EMISSION_OBSERVATION_V29: std::cell::Cell<Option<SourceEmissionObservationV29>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
struct SourceEmissionObservationGuardV29(Option<SourceEmissionObservationV29>);

#[cfg(test)]
impl SourceEmissionObservationGuardV29 {
    fn enter(
        phase: SourceEmissionPhaseV29,
        owner: &ProductionSemanticSsaOwnerV1,
        budget: &ArgumentBudgetV1<'_>,
    ) -> Self {
        Self(
            SOURCE_EMISSION_OBSERVATION_V29.replace(Some(SourceEmissionObservationV29 {
                phase,
                owner: std::ptr::from_ref(owner) as usize,
                slot: std::ptr::from_ref(budget) as usize,
                ledger: budget.work_ledger_identity_v1(),
            })),
        )
    }
}

#[cfg(test)]
impl Drop for SourceEmissionObservationGuardV29 {
    fn drop(&mut self) {
        SOURCE_EMISSION_OBSERVATION_V29.set(self.0);
    }
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source admission remains gated")
)]
struct ScopedSourceInputsV29 {
    owner: ProductionSemanticSsaOwnerV1,
    launch: crate::ProductionSourceLaunchRosterV1,
    input: OwnedExecutionInputV29,
}

// This owns reconstruction evidence, not a source-value/discharge/launch proof.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source admission remains gated")
)]
struct SourceOwnedScopedModuleV29 {
    source: ScopedSourceInputsV29,
    pending: PendingScopedModuleV29,
    limits: ProductionSemanticKirLimitsV1,
    capture: HelperOccurrenceCaptureV1,
    assertions: Vec<ReplayedInstanceAssertV1>,
    retained_storage: usize,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source admission remains gated")
)]
impl SourceOwnedScopedModuleV29 {
    // A foreign/missing reservation leaves the donor untouched. After adoption,
    // every failure drops the adopted payload before restoring the caller floor.
    // Existing MIR/SSA/launch allocations keep their upstream accounting domain;
    // owned input, wrapper header, graph, source sidecars and fresh capture are
    // retained here. A preexisting capture remains separately caller-reserved.
    fn try_new(
        donor: &mut Option<ScopedSourceInputsV29>,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ScopedModuleErrorV29> {
        let source = donor.as_ref().ok_or_else(scoped_module_error_v29)?;
        if source.input.ledger != budget.work_ledger_identity_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let floor = budget
            .storage()
            .checked_sub(source.input.retained_storage)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if source
            .owner
            .occurrence_storage()
            .is_some_and(|receipt| floor < receipt.retained_storage())
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let inherited = source.input.retained_storage;
        let boundary = match ScopedSourceCleanupBoundaryV29::new::<Self, ScopedModuleErrorV29>(
            floor, budget,
        ) {
            Ok(boundary) => boundary,
            Err(error) => {
                drop(donor.take());
                let _ = budget.release_storage(inherited);
                return Err(error.into());
            }
        };
        let result = boundary.run(budget, |cleanup, budget| {
            Self::try_new_with_cleanup(donor, limits, cleanup, budget)
        });
        // A callback-header refusal precedes adoption by the inner attempt.
        // The outer entry has already accepted custody of this exact input.
        if result.is_err() && donor.is_some() {
            drop(donor.take());
            let _ = budget.release_storage(inherited);
        }
        result
    }

    fn try_new_with_cleanup(
        donor: &mut Option<ScopedSourceInputsV29>,
        limits: ProductionSemanticKirLimitsV1,
        cleanup: &ScopedSourceCleanupV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ScopedModuleErrorV29> {
        let source = donor.as_ref().ok_or_else(scoped_module_error_v29)?;
        if source.input.ledger != budget.work_ledger_identity_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let floor = budget
            .storage()
            .checked_sub(source.input.retained_storage)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let preexisting = source.owner.occurrence_storage();
        if preexisting.is_some_and(|receipt| floor < receipt.retained_storage()) {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let mut source = donor.take().ok_or_else(scoped_module_error_v29)?;
        scoped_source_attempt_v29(cleanup, budget, floor, move |budget| {
            let retained_floor = budget
                .storage()
                .checked_sub(source.input.retained_storage)
                .ok_or(ArgumentResourceV1::Accounting)?;
            budget.reserve_storage(size_of::<Self>())?;
            source
                .owner
                .verify_replay()
                .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
            let capture = match preexisting {
                Some(receipt) => HelperOccurrenceCaptureV1::Preexisting(receipt),
                None => {
                    let receipt = source
                        .owner
                        .try_capture_occurrences_with_budget_v1(budget)
                        .map_err(ScopedModuleErrorV29::Occurrences)?;
                    budget.reserve_storage(receipt.retained_storage())?;
                    HelperOccurrenceCaptureV1::Transferred(receipt)
                }
            };
            let pending = source.input.with_source_with_cleanup(
                &source.owner,
                &source.launch,
                cleanup,
                budget,
                |view, budget| {
                    #[cfg(test)]
                    let _phase = SourceEmissionObservationGuardV29::enter(
                        SourceEmissionPhaseV29::Admission,
                        view.owner,
                        budget,
                    );
                    admit_pending_scoped_module_with_cleanup_v29(view, limits, cleanup, budget)
                },
            )?;
            let assertions = source.input.with_source_with_cleanup(
                &source.owner,
                &source.launch,
                cleanup,
                budget,
                |view, budget| {
                    #[cfg(test)]
                    let _phase = SourceEmissionObservationGuardV29::enter(
                        SourceEmissionPhaseV29::ConstructionReplay,
                        view.owner,
                        budget,
                    );
                    reconstruct_scoped_source_with_cleanup_v29(
                        &pending, view, limits, cleanup, budget,
                    )
                },
            )?;
            let retained_storage = budget
                .storage()
                .checked_sub(retained_floor)
                .ok_or(ArgumentResourceV1::Accounting)?;
            Ok(Self {
                source,
                pending,
                limits,
                capture,
                assertions,
                retained_storage,
            })
        })
    }

    fn replay(&self, budget: &mut ArgumentBudgetV1<'_>) -> Result<(), ScopedModuleErrorV29> {
        if self.source.input.ledger != budget.work_ledger_identity_v1()
            || self.pending.ledger != budget.work_ledger_identity_v1()
            || budget.storage()
                < argument_sum_v1(&[self.retained_storage, self.capture.preexisting_storage()])?
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let floor = budget.storage();
        with_scoped_source_cleanup_v29(budget, floor, |cleanup, budget| {
            self.replay_with_cleanup(cleanup, budget)
        })
    }

    fn replay_with_cleanup(
        &self,
        cleanup: &ScopedSourceCleanupV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ScopedModuleErrorV29> {
        if self.source.input.ledger != budget.work_ledger_identity_v1()
            || self.pending.ledger != budget.work_ledger_identity_v1()
            || budget.storage()
                < argument_sum_v1(&[self.retained_storage, self.capture.preexisting_storage()])?
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let floor = budget.storage();
        scoped_source_attempt_v29(cleanup, budget, floor, |budget| {
            self.source
                .owner
                .verify_replay()
                .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
            let rows = self.source.input.with_source_with_cleanup(
                &self.source.owner,
                &self.source.launch,
                cleanup,
                budget,
                |view, budget| {
                    #[cfg(test)]
                    let _phase = SourceEmissionObservationGuardV29::enter(
                        SourceEmissionPhaseV29::ConsumerReplay,
                        view.owner,
                        budget,
                    );
                    reconstruct_scoped_source_with_cleanup_v29(
                        &self.pending,
                        view,
                        self.limits,
                        cleanup,
                        budget,
                    )
                },
            )?;
            budget.charge_work(argument_sum_v1(&[
                1,
                argument_product_v1(rows.len(), size_of::<ReplayedInstanceAssertV1>())?,
            ])?)?;
            let matches = rows == self.assertions;
            assert_origin_drop_v1(rows, budget).map_err(ProductionSemanticKirErrorV1::from)?;
            if !matches {
                return Err(scoped_module_error_v29().into());
            }
            Ok(())
        })
    }
}

fn reconstruct_scoped_source_v29(
    pending: &PendingScopedModuleV29,
    source: &ExecutionLifecycleSourceV29<'_>,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<ReplayedInstanceAssertV1>, ScopedModuleErrorV29> {
    if source.ledger != budget.work_ledger_identity_v1()
        || pending.ledger != source.ledger
        || budget.storage() < pending.retained_storage
    {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let floor = budget.storage();
    with_scoped_source_cleanup_v29(budget, floor, |cleanup, budget| {
        reconstruct_scoped_source_with_cleanup_v29(pending, source, limits, cleanup, budget)
    })
}

fn reconstruct_scoped_source_with_cleanup_v29(
    pending: &PendingScopedModuleV29,
    source: &ExecutionLifecycleSourceV29<'_>,
    limits: ProductionSemanticKirLimitsV1,
    cleanup: &ScopedSourceCleanupV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<ReplayedInstanceAssertV1>, ScopedModuleErrorV29> {
    if source.ledger != budget.work_ledger_identity_v1()
        || pending.ledger != source.ledger
        || budget.storage() < pending.retained_storage
    {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let floor = budget.storage();
    scoped_source_attempt_v29(cleanup, budget, floor, |budget| {
        let scratch_floor = budget.storage();
        let (candidate, roots, _) = scoped_source_candidate_v29(source, limits, cleanup, budget)?;
        let scratch = budget
            .storage()
            .checked_sub(scratch_floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if !pending
            .graph
            .matches_module_with_budget_v18(&candidate, budget)
            .map_err(ScopedModuleErrorV29::Canonical)?
            || !scoped_replay_metadata_v29::matches_roots(&pending.roots, &roots, budget)?
        {
            return Err(scoped_module_error_v29().into());
        }
        let assertions = collect_scoped_module_assertions_v29(
            pending.graph.module(),
            &pending.roots,
            source,
            budget,
        )?;
        drop((candidate, roots));
        budget.release_storage(scratch)?;
        Ok(assertions)
    })
}
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source replay remains gated")
)]
fn collect_scoped_module_assertions_v29(
    module: &Module,
    roots: &[ScopedModuleRootV29],
    source: &ExecutionLifecycleSourceV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<ReplayedInstanceAssertV1>, ProductionSemanticKirErrorV1> {
    if source.ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    scoped_slot_attempt_v29(budget, |budget| {
        budget.charge_work(2)?;
        if roots.len() != source.launch.roots().len() || roots.len() != module.kernels.len() {
            return Err(scoped_module_error_v29());
        }
        let graph = AssertGraphIndexV1::build_functions(&module.functions, true, budget)?;
        let mut assertions = Vec::new();
        for (ordinal, root) in roots.iter().enumerate() {
            budget.charge_work(2)?;
            if root.function_ordinal != ordinal
                || root.coordinates.root != source.launch.roots()[ordinal].selected_root()
            {
                return Err(scoped_module_error_v29());
            }
            production_call_instances_v1::with_production_call_instances_v1(
                source.owner,
                source.launch.roots()[ordinal].selected_root(),
                budget,
                |instances, budget| {
                    Ok(replay_instance_asserts_in_functions_v1(
                        InstanceAssertReplaySubjectV1 {
                            functions: &module.functions,
                            function_ordinal: root.function_ordinal,
                            sidecars: &root.sidecars,
                            active_instances: &root.active_instances,
                            coordinates: &root.coordinates,
                            slot_relocation: root.slot_relocation.as_ref(),
                            insertions: &root.insertions,
                            terminal_failures: root.terminal_failures.as_ref(),
                        },
                        instances,
                        &graph,
                        budget,
                        &mut |row, budget| {
                            assert_origin_push_v1(&mut assertions, row, budget)?;
                            Ok(())
                        },
                    ))
                },
            )
            .map_err(scoped_root_instance_error_v29)??;
        }
        graph.release(budget)?;
        Ok(assertions)
    })
}
