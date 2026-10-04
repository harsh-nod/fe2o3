// Distinct original N and erased E custody. No generic source/optimizer owner
// is manufactured, and all downstream source/output and publication gates stay.

include!("production_local_helper_erased_candidate_v1.rs");

/// New logical payload retained in addition to transferred original/ranked
/// reservations. The E graph receipt, owner header and actual map capacities
/// are included. Existing semantic/PLIRON engine domains are not heap accounting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionUnitLocalErasedStorageV1(usize);
impl ProductionUnitLocalErasedStorageV1 {
    /// Additional logical bytes to reserve while the erased owner remains live.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Move-only original source/N, complete ranked roots and independently checked E.
/// This establishes only the closed silent Unit-call deletion relation. Original
/// helper effects and source calls remain retained; they are not CompleteEmpty.
/// It is not final output, optimizer, descriptor, artifact or launch authority.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionUnitLocalErasedSourceOwnerV1;
/// fn duplicate(owner: ProductionUnitLocalErasedSourceOwnerV1) {
///     let first = owner;
///     let second = owner;
///     drop((first, second));
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionUnitLocalErasedSourceOwnerV1, ProductionSemanticKirOwnerV1};
/// fn substitute(owner: ProductionUnitLocalErasedSourceOwnerV1) -> ProductionSemanticKirOwnerV1 { owner }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionUnitLocalErasedSourceOwnerV1;
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
/// fn escape(owner: ProductionUnitLocalErasedSourceOwnerV1) -> &'static VerifiedCanonicalKernelIrModuleV12 { owner.erased() }
/// ```
pub struct ProductionUnitLocalErasedSourceOwnerV1 {
    original: ProductionPreRankedKirOwnerV1,
    roots: Vec<ProductionRankedSemanticProjectionRootV1>,
    erased: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    erased_storage: fe2o3_kernel_ir::CanonicalKernelIrReplayStorageV12,
    functions: Vec<Option<u32>>,
    operations: Vec<Option<ProductionUnitLocalOperationDeletionV1>>,
    deleted_functions: usize,
    deleted_calls: usize,
    input_storage: usize,
    additional_storage: ProductionUnitLocalErasedStorageV1,
    retained_storage: usize,
}

impl ProductionUnitLocalErasedSourceOwnerV1 {
    /// Required transferred input floor, including original N/origins/helper
    /// rows/source capture and the complete ranked Vec's actual capacity,
    /// diagnostic String capacities, boxed maps and existing PLIRON receipts.
    /// Reserve it before construction and keep it through the returned owner.
    /// A numeric floor is not graph or allocation custody; moved owners and
    /// fresh source/ranked/deletion checks provide those separate relations.
    pub fn input_storage_floor_v1(
        original: &ProductionPreRankedKirOwnerV1,
        roots: &Vec<ProductionRankedSemanticProjectionRootV1>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(6)?;
        let mut bytes = argument_sum_v1(&[
            original.unit_local_source_storage_floor_v1()?,
            std::mem::size_of::<Vec<ProductionRankedSemanticProjectionRootV1>>(),
            argument_product_v1(
                roots.capacity(),
                std::mem::size_of::<ProductionRankedSemanticProjectionRootV1>(),
            )?,
        ])?;
        for root in roots {
            budget.charge_work(7)?;
            bytes = argument_sum_v1(&[
                bytes,
                root.lowering
                    .production_analysis_retained_storage_upper_bound_v1(),
                root.ranked_ir.capacity(),
                argument_product_v1(
                    root.access_sources.len(),
                    std::mem::size_of::<ProductionRankedAccessSourceV1>(),
                )?,
                argument_product_v1(
                    root.executable_effect_sources.len(),
                    std::mem::size_of::<ProductionRankedExecutableEffectSourceV1>(),
                )?,
            ])?;
        }
        Ok(bytes)
    }

    /// Produces an inert candidate through the existing metered V12 copy API,
    /// admits actual E, then independently checks its exact N/E relation.
    /// Input floor must already be reserved. Every exit restores that floor;
    /// on success reserve the additional receipt before another allocation.
    /// On error the moved inputs are dropped; the caller retires their floor.
    pub fn try_produce_v1(
        original: ProductionPreRankedKirOwnerV1,
        roots: Vec<ProductionRankedSemanticProjectionRootV1>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(Self, ProductionUnitLocalErasedStorageV1), ProductionPreRankedKirErrorV1> {
        with_unit_erased_owner_scratch_v1(budget, |budget| {
            let input_storage = Self::input_storage_floor_v1(&original, &roots, budget)?;
            require_unit_erased_floor_v1(input_storage, budget)?;
            let (candidate, storage) = produce_unit_erased_candidate_v1(&original, &roots, budget)?;
            budget.reserve_storage(storage.retained_storage())?;
            let result = Self::from_borrowed_candidate_inner_v1(
                original,
                roots,
                &candidate,
                input_storage,
                budget,
            );
            drop(candidate);
            budget.release_storage(storage.retained_storage())?;
            result
        })
    }

    /// Checks an independently supplied actual candidate without running the
    /// deletion producer. The borrowed Module and its allocation receipt remain
    /// caller-reserved separately. Fresh budgeted V12 admission creates this
    /// owner's E and exact receipt; no caller-provided E size is trusted.
    /// Original/ranked input reservations transfer as for `try_produce_v1`.
    pub fn try_from_candidate_v1(
        original: ProductionPreRankedKirOwnerV1,
        roots: Vec<ProductionRankedSemanticProjectionRootV1>,
        candidate: &Module,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(Self, ProductionUnitLocalErasedStorageV1), ProductionPreRankedKirErrorV1> {
        with_unit_erased_owner_scratch_v1(budget, |budget| {
            let input_storage = Self::input_storage_floor_v1(&original, &roots, budget)?;
            require_unit_erased_floor_v1(input_storage, budget)?;
            Self::from_borrowed_candidate_inner_v1(
                original,
                roots,
                candidate,
                input_storage,
                budget,
            )
        })
    }

    fn from_borrowed_candidate_inner_v1(
        original: ProductionPreRankedKirOwnerV1,
        roots: Vec<ProductionRankedSemanticProjectionRootV1>,
        candidate: &Module,
        input_storage: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(Self, ProductionUnitLocalErasedStorageV1), ProductionPreRankedKirErrorV1> {
        budget.charge_work(5)?;
        source_output_unit_local_replay_v1(&original, budget)?;
        budget.reserve_storage(std::mem::size_of::<Self>())?;
        let (erased, erased_storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(candidate, budget)
            .map_err(ProductionPreRankedKirErrorV1::Canonical)?;
        budget.reserve_storage(erased_storage.retained_storage())?;
        let counts = unit_erased_counts_v1(original.executable().module(), budget)?;
        let mut functions = unit_local_vec_v1(counts.0, budget)?;
        let mut operations = unit_local_vec_v1(counts.1, budget)?;
        budget.charge_work(argument_sum_v1(&[counts.0, counts.1])?)?;
        functions.resize(counts.0, None);
        operations.resize(counts.1, None);
        let (deleted_functions, deleted_calls) =
            original.with_checked_unit_local_ranked_stage_v1(&roots, budget, |stage, budget| {
                stage.with_checked_silent_unit_call_deletion_v1(
                    &erased,
                    budget,
                    |checked, budget| {
                        budget.charge_work(argument_sum_v1(&[
                            functions.len(),
                            operations.len(),
                            4,
                        ])?)?;
                        if functions.len() != checked.functions.len()
                            || operations.len() != checked.operations.len()
                        {
                            return Err(unit_local_mismatch_v1());
                        }
                        functions.copy_from_slice(checked.functions);
                        operations.copy_from_slice(checked.operations);
                        Ok((
                            checked.deleted_function_count(),
                            checked.deleted_call_count(),
                        ))
                    },
                )
            })?;
        let additional_storage = ProductionUnitLocalErasedStorageV1(argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            erased_storage.retained_storage(),
            argument_product_v1(functions.capacity(), std::mem::size_of::<Option<u32>>())?,
            argument_product_v1(
                operations.capacity(),
                std::mem::size_of::<Option<ProductionUnitLocalOperationDeletionV1>>(),
            )?,
        ])?);
        let retained_storage = argument_sum_v1(&[input_storage, additional_storage.0])?;
        Ok((
            Self {
                original,
                roots,
                erased,
                erased_storage,
                functions,
                operations,
                deleted_functions,
                deleted_calls,
                input_storage,
                additional_storage,
                retained_storage,
            },
            additional_storage,
        ))
    }

    /// The unchanged source/N owner; never an alias for the erased graph.
    pub const fn original_source(&self) -> &ProductionPreRankedKirOwnerV1 {
        &self.original
    }
    /// Actual independently verified E, distinct from source materialization N.
    pub const fn erased(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        &self.erased
    }
    /// Complete transferred original/ranked plus new E/map logical floor.
    pub const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_storage
    }
    /// Transferred original source/N and complete ranked-input reservation.
    pub const fn input_storage_floor(&self) -> usize {
        self.input_storage
    }
    /// Newly retained E, owner-header and deletion-map reservation.
    pub const fn additional_storage(&self) -> ProductionUnitLocalErasedStorageV1 {
        self.additional_storage
    }
    /// E's canonical graph receipt, not the complete original/ranked/E floor.
    pub const fn erased_storage(&self) -> fe2o3_kernel_ir::CanonicalKernelIrReplayStorageV12 {
        self.erased_storage
    }
    /// Physical Local helper definitions removed by the checked N/E relation.
    pub const fn deleted_function_count(&self) -> usize {
        self.deleted_functions
    }
    /// Exact silent Unit call occurrences removed from the original graph.
    pub const fn deleted_call_count(&self) -> usize {
        self.deleted_calls
    }
    /// Number of retained ranked roots in original source order.
    pub fn ranked_root_count(&self) -> usize {
        self.roots.len()
    }
    /// This intermediate source relation grants no artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    /// Paid borrowed export of one retained N-ranked recipe in complete source
    /// order. This is inert transport, not source/proof-origin authority.
    pub fn ranked_candidate_v1(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<crate::NativeRankedSourceCandidateV1<'_>>, ProductionSemanticKirErrorV1>
    {
        budget.charge_work(6)?;
        require_unit_erased_floor_v1(self.retained_storage, budget)?;
        Ok(self.roots.get(ordinal).map(|root| {
            crate::NativeRankedSourceCandidateV1::from_untrusted_parts(
                root.selected_root.index(),
                root.launch_rank,
                root.lowering.kernel(),
                &root.access_sources,
                &root.executable_effect_sources,
                &root.ranked_ir,
            )
        }))
    }

    /// Fresh same-owner source/ranked and exact N/E replay. Stored inert maps
    /// are compared to the fresh relation before the callback receives it.
    /// The scoped token cannot escape or be transplanted to another owner.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionUnitLocalErasedSourceOwnerV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape(owner: &ProductionUnitLocalErasedSourceOwnerV1, budget: &mut Budget<'_>) {
    ///     let mut saved = None;
    ///     owner.with_checked_erasure_v1(budget, |checked, _| { saved = Some(checked); Ok(()) }).unwrap();
    ///     drop(saved);
    /// }
    /// ```
    pub fn with_checked_erasure_v1<'w, R>(
        &self,
        budget: &mut ArgumentBudgetV1<'w>,
        next: impl for<'s> FnOnce(
            &CheckedUnitLocalCallDeletionV1<'s>,
            &mut ArgumentBudgetV1<'w>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        require_unit_erased_floor_v1(self.retained_storage, budget)?;
        source_output_unit_local_replay_v1(&self.original, budget)?;
        self.original.with_checked_unit_local_ranked_stage_v1(
            &self.roots,
            budget,
            |stage, budget| {
                stage.with_checked_silent_unit_call_deletion_v1(
                    &self.erased,
                    budget,
                    |checked, budget| {
                        budget.charge_work(argument_sum_v1(&[
                            self.functions.len(),
                            self.operations.len(),
                            6,
                        ])?)?;
                        if checked.functions != self.functions
                            || checked.operations != self.operations
                            || checked.deleted_function_count() != self.deleted_functions
                            || checked.deleted_call_count() != self.deleted_calls
                        {
                            return Err(unit_local_mismatch_v1());
                        }
                        next(checked, budget)
                    },
                )
            },
        )
    }

    /// Reconstructs original source/N and rechecks ranked custody and N/E deletion.
    pub fn verify_equivalence(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.with_checked_erasure_v1(budget, |_, _| Ok(()))
    }
}

fn require_unit_erased_floor_v1(
    floor: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    if budget.storage() < floor {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    Ok(())
}

fn unit_erased_counts_v1(
    module: &Module,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(usize, usize), ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    let mut operations = 0usize;
    for function in &module.functions {
        budget.charge_work(1)?;
        if let Some(body) = &function.body {
            for block in &body.blocks {
                budget.charge_work(2)?;
                operations = argument_sum_v1(&[operations, block.operations.len()])?;
            }
        }
    }
    Ok((module.functions.len(), operations))
}

fn with_unit_erased_owner_scratch_v1<R>(
    budget: &mut ArgumentBudgetV1<'_>,
    next: impl FnOnce(&mut ArgumentBudgetV1<'_>) -> Result<R, ProductionPreRankedKirErrorV1>,
) -> Result<R, ProductionPreRankedKirErrorV1> {
    budget.charge_work(2)?;
    let floor = budget.storage();
    let slot = budget as *const ArgumentBudgetV1<'_> as usize;
    let work = budget.work_ledger_identity_v1();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| next(budget)));
    if slot != budget as *const ArgumentBudgetV1<'_> as usize
        || work != budget.work_ledger_identity_v1()
    {
        drop(result);
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let extra = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    budget.release_storage(extra)?;
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
