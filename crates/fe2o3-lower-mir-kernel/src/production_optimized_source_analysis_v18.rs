use fe2o3_kernel_analysis::{
    CanonicalKirCallEffectErrorV1, CanonicalKirCallEffectsV18, CanonicalKirMemorySsaLimitsV1,
    CanonicalKirMemorySsaV18, CanonicalKirSparseLimitsV1, CanonicalKirSparseV18,
};

/// Lazy analyses of the exact checked endpoints. No report is rebound to another
/// inventory, and source custody remains live through the entire callback.
pub struct ProductionOptimizedSourceAnalysisV18<'scope> {
    original: &'scope ProductionSourceCorrespondenceV18<'scope>,
    optimized: &'scope ProductionOptimizedSourceCorrespondenceV18<'scope>,
    input: &'scope fe2o3_kernel_analysis::CanonicalKirInventoryV18<'scope>,
    output: &'scope fe2o3_kernel_analysis::CanonicalKirInventoryV18<'scope>,
    input_effects: Option<CanonicalKirCallEffectsV18<'scope, 'scope>>,
    output_effects: Option<CanonicalKirCallEffectsV18<'scope, 'scope>>,
    sparse: Option<CanonicalKirSparseV18<'scope, 'scope>>,
    input_sparse: Option<CanonicalKirSparseV18<'scope, 'scope>>,
    input_memory: Option<CanonicalKirMemorySsaV18<'scope, 'scope>>,
    output_memory: Option<CanonicalKirMemorySsaV18<'scope, 'scope>>,
    cleanup: fe2o3_pliron::CanonicalAnalysisCleanupV1<'scope>,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    storage: usize,
}

fn optimized_source_effect_error_v18(
    error: CanonicalKirCallEffectErrorV1,
) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        CanonicalKirCallEffectErrorV1::Resource(error) => error.into(),
        CanonicalKirCallEffectErrorV1::InvalidFunction(_)
        | CanonicalKirCallEffectErrorV1::Incomplete(_)
        | CanonicalKirCallEffectErrorV1::InconsistentInventory => {
            ProductionSourceOwnedViewErrorV18::Binding("optimized call-effect analysis association")
        }
    }
}

impl ProductionSourceCorrespondenceV18<'_> {
    /// Rechecks each original root's memory currentness against fresh analyses
    /// of the exact actual output. Reuses the original source/activation solver;
    /// no input-memory report or pre-optimization pointer birth authorizes output.
    /// The returned root count is diagnostic, not a transferable certificate.
    /// Whole-value typed reads/writes and checked erased Select births use the
    /// shared source/output currentness equations. Other typed footprints and
    /// unsupported erased-birth recipes remain explicit refusals.
    pub fn check_optimized_source_currentness_v18(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.with_optimized_analysis_v18(optimized, budget, |analyses, budget| {
            analyses.with_memory_versions(budget, |input, output, budget| {
                let roots = self.source.root_count(budget)?;
                let mut checked_roots = 0_usize;
                for root in 0..roots {
                    scoped_raw_admission_v29::with_checked_optimized_source_memory_v18(
                        self,
                        optimized,
                        root,
                        input,
                        output,
                        budget,
                        |memory, budget| {
                            memory.check_scope_v18(self, optimized, root, budget)?;
                            budget
                                .charge_work(1)
                                .map_err(|error| self.retain_query_resource_error_v18(error))?;
                            checked_roots = checked_roots.checked_add(1).ok_or_else(|| {
                                self.retain_query_resource_error_v18(ArgumentResourceV1::Arithmetic)
                            })?;
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        },
                    )?;
                }
                Ok::<_, ProductionSourceOwnedViewErrorV18>(checked_roots)
            })
        })
    }

    /// Lends lazy analyses bound to this exact original inventory and the
    /// transition's distinct output inventory. The enclosing source scope and
    /// shared budget remain live through callback completion and cleanup.
    /// Cached analyses do not establish source equivalence or memory safety.
    pub fn with_optimized_analysis_v18<'work, T, E>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &mut ProductionOptimizedSourceAnalysisV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        let floor = budget.storage();
        let slot = std::ptr::from_ref(budget) as usize;
        let ledger = budget.work_ledger_identity_v1();
        let accepted = std::cell::Cell::new(0_usize);
        // Own the callback through early query/refusal cleanup as well as the
        // paid handoff. Queries precede any charge to a possibly foreign ledger.
        let caught = {
            let budget = &mut *budget;
            let accepted = &accepted;
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                self.query(budget)?;
                let input = optimized.input_inventory(budget)?;
                let output = optimized.output_inventory(budget)?;
                self.retain_query((|| {
                    budget.charge_work(2)?;
                    if !std::ptr::eq(input, self.inventory)
                        || !std::ptr::eq(optimized.original_source(budget)?, self.source)
                    {
                        return self
                            .source
                            .missing("optimized analysis changed original source or inventory");
                    }
                    Ok(())
                })())?;
                let storage = self.retain_query(
                    optimized_source_consumer_resources_v18::analysis_headers::<T, E, _>(&consume)
                        .map_err(Into::into),
                )?;
                self.retain_query(budget.reserve_storage(storage).map_err(Into::into))?;
                accepted.set(storage);
                budget.charge_work(1 + SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29)?;
                Ok::<_, ProductionSourceOwnedViewErrorV18>((input, output, storage, consume))
            }))
        };
        let entry_custody =
            self.observe_optimized_analysis_entry_v18(budget, floor, accepted.get(), slot, ledger);
        let (input, output, storage, consume) = match caught {
            Ok(Ok(entry)) if entry_custody.is_ok() => entry,
            Ok(Ok(entry)) => {
                // The successful handoff still owns F. Dispose it within a
                // catch before settlement; a single Drop panic stays raw.
                let disposed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    drop(entry);
                }));
                let _ = self.observe_optimized_analysis_entry_v18(
                    budget,
                    floor,
                    accepted.get(),
                    slot,
                    ledger,
                );
                if let Err(payload) = disposed {
                    std::panic::resume_unwind(payload);
                }
                return self
                    .retain_query(Err(ArgumentResourceV1::Accounting.into()))
                    .map_err(Into::into);
            }
            Ok(Err(error)) => {
                // F is already dead. Never refund a callback's unrelated
                // storage delta or replace its selected typed diagnostic.
                if entry_custody.is_ok() && budget.release_storage(accepted.get()).is_err() {
                    self.source.cleanup.deny_refund();
                }
                return self.retain_query(Err(error)).map_err(Into::into);
            }
            Err(payload) => {
                if entry_custody.is_ok() && budget.release_storage(accepted.get()).is_err() {
                    self.source.cleanup.deny_refund();
                }
                std::panic::resume_unwind(payload);
            }
        };
        let mut analyses = ProductionOptimizedSourceAnalysisV18 {
            original: self,
            optimized,
            input,
            output,
            input_effects: None,
            output_effects: None,
            sparse: None,
            input_sparse: None,
            input_memory: None,
            output_memory: None,
            cleanup: fe2o3_pliron::CanonicalAnalysisCleanupV1::linked(&self.source.cleanup.denied),
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            storage,
        };
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            consume(&mut analyses, budget)
        }));
        let prior = self.source.guard.first.get();
        let postflight = if matches!(&caught, Ok(Ok(_))) {
            analyses.check(budget)
        } else {
            analyses.observe_custody(budget)
        };
        let storage = analyses.storage;
        drop(analyses);
        let released = if self.source.cleanup.is_denied() {
            Err(ArgumentResourceV1::Accounting)
        } else {
            budget
                .release_storage(storage)
                .inspect_err(|_| self.source.cleanup.deny_refund())
        };
        match caught {
            Err(payload) => std::panic::resume_unwind(payload),
            Ok(Err(error)) => {
                let selected = match prior {
                    Some(first) => {
                        source_reference_discard_v29(Err::<T, E>(error));
                        first.error().into()
                    }
                    None => error,
                };
                Err(selected)
            }
            Ok(Ok(value)) => {
                match self.retain_query(postflight.and(released.map_err(Into::into))) {
                    Ok(()) => Ok(value),
                    Err(error) => {
                        source_reference_discard_v29(Ok::<T, E>(value));
                        Err(error.into())
                    }
                }
            }
        }
    }

    fn observe_optimized_analysis_entry_v18(
        &self,
        budget: &ArgumentBudgetV1<'_>,
        floor: usize,
        storage: usize,
        slot: usize,
        ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    ) -> SourceOwnedResultV18<()> {
        if slot != std::ptr::from_ref(budget) as usize
            || ledger != budget.work_ledger_identity_v1()
            || floor
                .checked_add(storage)
                .is_none_or(|required| budget.storage() < required)
        {
            self.source.cleanup.deny_refund();
        }
        self.observe_custody(budget)
    }
}

impl ProductionOptimizedSourceAnalysisV18<'_> {
    fn observe_custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        if self.cleanup.refund_denied()
            || self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            self.original.source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.original.observe_custody(budget)
    }

    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.original.retain_query(self.observe_custody(budget))?;
        self.original.query(budget)?;
        self.original.retain_query((|| {
            budget.charge_work(2)?;
            if !std::ptr::eq(self.input, self.optimized.input_inventory(budget)?)
                || !std::ptr::eq(self.output, self.optimized.output_inventory(budget)?)
            {
                return self
                    .original
                    .source
                    .missing("optimized analysis changed checked endpoints");
            }
            Ok(())
        })())
    }

    fn reserve_report(
        &mut self,
        retained: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.original.retain_query((|| {
            let storage = self
                .storage
                .checked_add(retained)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            let floor = self
                .floor
                .checked_add(retained)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            budget.reserve_storage(retained)?;
            self.storage = storage;
            self.floor = floor;
            Ok(())
        })())
    }

    /// Lends the output sparse report and separate original/output call-effect
    /// reports, deriving each once on its own immutable endpoint. Incomplete
    /// summaries remain incomplete; an empty summary is not inferred from a
    /// missing call, eliminated source span, or foreign inventory.
    pub fn with_projection_facts<T, E>(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
        consume: impl FnOnce(
            &CanonicalKirSparseV18<'_, '_>,
            &CanonicalKirCallEffectsV18<'_, '_>,
            &CanonicalKirCallEffectsV18<'_, '_>,
            &mut ArgumentBudgetV1<'_>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.ensure_projection_facts(budget)?;
        consume(
            self.sparse.as_ref().expect("installed sparse report"),
            self.input_effects
                .as_ref()
                .expect("installed input effects"),
            self.output_effects
                .as_ref()
                .expect("installed output effects"),
            budget,
        )
    }

    fn ensure_projection_facts(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        if self.sparse.is_none() {
            let (report, receipt) = self.original.retain_query(
                CanonicalKirSparseV18::derive_v18(
                    self.output,
                    CanonicalKirSparseLimitsV1::default(),
                    budget,
                )
                .map_err(|error| fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Sparse(error).into()),
            )?;
            self.reserve_report(receipt.retained_storage(), budget)?;
            self.sparse = Some(report);
        }
        if self.input_effects.is_none() {
            let (report, receipt) = self.original.retain_query(
                CanonicalKirCallEffectsV18::derive_v18(self.input, budget)
                    .map_err(optimized_source_effect_error_v18),
            )?;
            self.reserve_report(receipt.retained_storage(), budget)?;
            self.input_effects = Some(report);
        }
        if self.output_effects.is_none() {
            let (report, receipt) = self.original.retain_query(
                CanonicalKirCallEffectsV18::derive_v18(self.output, budget)
                    .map_err(optimized_source_effect_error_v18),
            )?;
            self.reserve_report(receipt.retained_storage(), budget)?;
            self.output_effects = Some(report);
        }
        self.check(budget)
    }

    /// Lends original and output reports together to the nominal source facts
    /// decorator. Existing original queries still receive their original report.
    pub fn with_source_projection_facts<T, E>(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
        consume: impl FnOnce(
            &CanonicalKirSparseV18<'_, '_>,
            &CanonicalKirSparseV18<'_, '_>,
            &CanonicalKirCallEffectsV18<'_, '_>,
            &CanonicalKirCallEffectsV18<'_, '_>,
            &fe2o3_pliron::CanonicalAnalysisCleanupV1<'_>,
            &mut ArgumentBudgetV1<'_>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.ensure_projection_facts(budget)?;
        if self.input_sparse.is_none() {
            let (report, receipt) = self.original.retain_query(
                CanonicalKirSparseV18::derive_v18(
                    self.input,
                    CanonicalKirSparseLimitsV1::default(),
                    budget,
                )
                .map_err(|error| fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Sparse(error).into()),
            )?;
            self.reserve_report(receipt.retained_storage(), budget)?;
            self.input_sparse = Some(report);
        }
        self.check(budget)?;
        consume(
            self.input_sparse
                .as_ref()
                .expect("installed input sparse report"),
            self.sparse.as_ref().expect("installed sparse report"),
            self.input_effects
                .as_ref()
                .expect("installed input effects"),
            self.output_effects
                .as_ref()
                .expect("installed output effects"),
            &self.cleanup,
            budget,
        )
    }

    /// Lends freshly derived memory-version analyses of both checked endpoints.
    /// These reports describe physical memory dependencies; consumers must
    /// still check source currentness, initialization, and exact access roles.
    pub fn with_memory_versions<'work, T, E>(
        &mut self,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl FnOnce(
            &CanonicalKirMemorySsaV18<'_, '_>,
            &CanonicalKirMemorySsaV18<'_, '_>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.check(budget)?;
        if self.input_memory.is_none() {
            let (report, receipt) = self.original.retain_query(
                CanonicalKirMemorySsaV18::derive_v18(
                    self.input,
                    CanonicalKirMemorySsaLimitsV1::default(),
                    budget,
                )
                .map_err(|error| {
                    fe2o3_pliron::CanonicalAnalysisScopeErrorV1::MemorySsa(error).into()
                }),
            )?;
            self.reserve_report(receipt.retained_storage(), budget)?;
            self.input_memory = Some(report);
        }
        if self.output_memory.is_none() {
            let (report, receipt) = self.original.retain_query(
                CanonicalKirMemorySsaV18::derive_v18(
                    self.output,
                    CanonicalKirMemorySsaLimitsV1::default(),
                    budget,
                )
                .map_err(|error| {
                    fe2o3_pliron::CanonicalAnalysisScopeErrorV1::MemorySsa(error).into()
                }),
            )?;
            self.reserve_report(receipt.retained_storage(), budget)?;
            self.output_memory = Some(report);
        }
        self.check(budget)?;
        consume(
            self.input_memory
                .as_ref()
                .expect("installed input MemorySSA"),
            self.output_memory
                .as_ref()
                .expect("installed output MemorySSA"),
            budget,
        )
    }
}
