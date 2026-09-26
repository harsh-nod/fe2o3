// Closed pre-ranked nominal helper construction. Ordinary legacy construction
// remains unchanged; no source token can be reconstructed from this graph.
struct Bf16EmissionPartsV1 {
    executable: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    executable_storage: fe2o3_kernel_ir::CanonicalKernelIrReplayStorageV12,
    canonical: ProductionCanonicalKernelIrV1,
    correspondence: SemanticKirCorrespondenceV1,
    origins: SealedAssertOriginsV1,
    launches: Box<[RetainedRankedLaunchRootV1]>,
    helpers: SealedHelperMemoryV1,
}

// One original-ledger scope. There is no alternative Work ledger. Error/panic
// payloads are dropped before refund; a successful owner's explicit receipt is
// transferred with the still-live output for immediate caller reservation.
fn bf16_emission_scope_v1<T>(
    budget: &mut ArgumentBudgetV1<'_>,
    build: impl FnOnce(&mut ArgumentBudgetV1<'_>) -> Result<T, ProductionPreRankedKirErrorV1>,
) -> Result<T, ProductionPreRankedKirErrorV1> {
    if budget.failed_storage().is_some() || budget.failed_work().is_some() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let floor = budget.storage();
    let slot = budget as *mut _ as usize;
    let identity = budget.work_ledger_identity_v1();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| build(budget)));
    if slot != budget as *mut _ as usize
        || identity != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        drop(result);
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let result = match result {
        Ok(Ok(output)) if budget.failed_storage().is_some() || budget.failed_work().is_some() => {
            drop(output);
            Err(ArgumentResourceV1::Accounting.into())
        }
        Ok(value) => value,
        Err(payload) => {
            drop(payload);
            Err(bf16_emission_refusal_v1("BF16 emission unwound before owner transfer").into())
        }
    };
    budget.release_storage(budget.storage() - floor)?;
    result
}

fn bf16_exact_graph_v1(
    original: &Module,
    fresh: &Module,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    // The complete finite source/output comparison was prepaid with the
    // reconstruction envelope; no detached digest stands in for this equality.
    budget.charge_work(1)?;
    if original != fresh {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(())
}

fn bf16_source_parts_v1(
    semantic_ssa: &ProductionSemanticSsaOwnerV1,
    source_launch: &crate::ProductionSourceLaunchRosterV1,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Bf16EmissionPartsV1, ProductionPreRankedKirErrorV1> {
    let mut parts = None;
    let mut failure = None;
    let relation = with_checked_bf16_call_instance_v1(semantic_ssa, budget, |source, budget| {
        let result = (|| -> Result<Bf16EmissionPartsV1, ProductionPreRankedKirErrorV1> {
            let envelope = bf16_emission_profile_v1(source, budget)?;
            // Refusal before any new planner, output graph, launch allocation
            // or allocating type/CFG calculator. This reservation survives in
            // the result receipt, even after transient maps have been dropped.
            budget.reserve_storage(envelope.storage)?;
            budget.charge_work(envelope.work)?;
            let launches = materialization_launch_roots_v1(semantic_ssa, source_launch)?;
            if launches.len() != 1
                || launches[0].selected_root != source.root()
                || launches[0].launch_rank != 1
                || launches[0].global_extents != [64, 1, 1]
                || launches[0].workgroup_extents != [64, 1, 1]
                || !launches[0].full_physical_workgroups
            {
                return Err(bf16_emission_refusal_v1("BF16 exact source launch roster").into());
            }
            require_execution_free_types_v29(semantic_ssa.source_semantic().types(), budget)?;
            let bounds = ProductionSemanticKirLimitsV1 {
                max_functions: limits.max_functions.min(2),
                max_blocks: limits.max_blocks.min(32),
                max_statements: limits.max_statements.min(4096),
                max_operations: limits.max_operations.min(1024),
                ..limits
            };
            let mut closure = ReachableClosureBudgetV1::new(bounds.max_blocks);
            let mut private_arrays = PrivateArrayLazyBudgetV1::new(1, bounds.max_operations);
            let mut admission =
                HelperLoweringAdmissionV1::PendingBf16Nominal(Bf16CallEmissionStateV1 {
                    source,
                    capture: Bf16CallEmissionCaptureV1::new(),
                });
            // SAME ordinary function/Call/Return engine, selected root and real
            // source plans. Generic argument correspondence is replaced below
            // by complete closed-profile source/nominal replay, never skipped.
            let (module, correspondence, _private_payload) = lower_single_root_module(
                semantic_ssa,
                bounds,
                source.root(),
                Some(launches[0]),
                &mut closure,
                false,
                None,
                &mut private_arrays,
                None,
                budget,
                &mut admission,
            )?;
            let HelperLoweringAdmissionV1::PendingBf16Nominal(state) = admission else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch.into());
            };
            let capture = state.capture;
            let (executable, executable_storage) =
                fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(&module, budget)
                    .map_err(ProductionPreRankedKirErrorV1::Canonical)?;
            budget.reserve_storage(executable_storage.retained_storage())?;
            let relation = bf16_seal_call_relation_v1(
                source,
                executable.module(),
                &correspondence,
                capture,
                launches[0],
                budget,
            )?;
            // The ordinary legacy wire is retained unchanged for owner layout,
            // not admitted as an alternative source or execution authority.
            let canonical = ProductionCanonicalKernelIrV1::from_module_ref(&module)?;
            drop(module);
            // A genuinely empty recorder still seals complete source/canonical
            // block coverage. Any retained semantic Assert was refused earlier.
            let origins = AssertOriginEmissionV1::new(budget).seal(
                semantic_ssa,
                &correspondence,
                &executable,
            )?;
            let helpers = SealedHelperMemoryV1::bf16_nominal_v1(
                relation,
                envelope,
                &executable,
                &correspondence,
                (
                    executable_storage.retained_storage(),
                    origins.storage.payload_storage(),
                ),
                semantic_ssa,
                budget,
            )?;
            Ok(Bf16EmissionPartsV1 {
                executable,
                executable_storage,
                canonical,
                correspondence,
                origins,
                launches,
                helpers,
            })
        })();
        match result {
            Ok(value) => {
                parts = Some(value);
                Ok(())
            }
            Err(error) => {
                failure = Some(error);
                Err(Bf16CallInstanceErrorV1::Unavailable(
                    "BF16 ordinary emission refused",
                ))
            }
        }
    });
    if let Err(error) = relation {
        // Drop every pending allocation before the outer scope restores its
        // own floor. Work and sticky denials are never refunded.
        drop(parts.take());
        return Err(failure.unwrap_or_else(|| bf16_call_error_v1(error).into()));
    }
    parts.ok_or_else(|| ProductionSemanticKirErrorV1::CorrespondenceMismatch.into())
}

impl ProductionPreRankedKirOwnerV1 {
    /// Constructs the closed BF16 nominal helper profile from the same actual
    /// source/SSA owner, occurrence capture and source launch roster.
    ///
    /// This is PRE-RANKED custody only. It preserves a real helper Function,
    /// Call and Return and retains actual Rust FnABI in the source owner. The
    /// canonical scalar transport is not a physical Rust ABI assertion.
    /// Legacy ranked/normal/call consumers refuse the Bf16Nominal category.
    /// No CPU/native/artifact/launch authority is granted.
    ///
    /// The caller must already reserve the same owner's occurrence receipt.
    /// On return reserve retained_analysis_storage_v1() until this owner drops.
    /// That receipt deliberately retains the conservative construction envelope
    /// in addition to actual V12/origin receipts. It is not RSS accounting.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
    /// fn raw(bytes: Vec<u8>) {
    ///     ProductionPreRankedKirOwnerV1::try_materialize_bf16_nominal_with_budget_v1(bytes);
    /// }
    /// ```
    pub fn try_materialize_bf16_nominal_with_budget_v1(
        semantic_ssa: ProductionSemanticSsaOwnerV1,
        source_launch: crate::ProductionSourceLaunchRosterV1,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionPreRankedKirErrorV1> {
        bf16_emission_scope_v1(budget, |budget| {
            let parts = bf16_source_parts_v1(&semantic_ssa, &source_launch, limits, budget)?;
            Ok(Self {
                semantic_ssa,
                source_launch,
                limits,
                executable: parts.executable,
                executable_storage: parts.executable_storage,
                canonical_kernel_ir: parts.canonical,
                correspondence: parts.correspondence,
                assert_origins: parts.origins,
                launch_roots: parts.launches,
                helper_memory: parts.helpers,
            })
        })
    }

    /// Reconstructs this closed profile from the SAME retained source plans,
    /// under the caller's cumulative ledger, and compares complete graph,
    /// correspondence, canonical bytes, sparse SSA rows and assertion origins.
    /// Stored component rows are outputs, never inputs to the reconstruction.
    pub fn verify_bf16_nominal_equivalence_with_budget_v1(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionPreRankedKirErrorV1> {
        if self.helper_source_policy_v1() != ProductionHelperSourcePolicyV1::Bf16Nominal {
            return Err(bf16_emission_refusal_v1("BF16 nominal owner required").into());
        }
        let required = argument_sum_v1(&[
            self.retained_analysis_storage_v1(),
            self.helper_memory.capture.preexisting_storage(),
        ])?;
        if budget.storage() < required {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        bf16_emission_scope_v1(budget, |budget| {
            let fresh =
                bf16_source_parts_v1(&self.semantic_ssa, &self.source_launch, self.limits, budget)?;
            bf16_exact_graph_v1(self.executable.module(), fresh.executable.module(), budget)?;
            if fresh.canonical != self.canonical_kernel_ir
                || fresh.correspondence != self.correspondence
                || fresh.helpers.bf16_nominal != self.helper_memory.bf16_nominal
                || fresh.launches != self.launch_roots
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch.into());
            }
            compare_unit_local_origin_rows_v1(&self.assert_origins, &fresh.origins, budget)?;
            drop(fresh);
            Ok(())
        })
    }
}

impl SealedHelperMemoryV1 {
    fn bf16_nominal_v1(
        relation: SealedBf16CallRelationV1,
        envelope: Bf16EmissionEnvelopeV1,
        executable: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        correspondence: &SemanticKirCorrespondenceV1,
        payload_storage: (usize, usize),
        semantic_ssa: &ProductionSemanticSsaOwnerV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionPreRankedKirErrorV1> {
        let mut functions = helper_memory_vec_v1(executable.module().functions.len(), budget)?;
        functions.resize(
            executable.module().functions.len(),
            RetainedHelperKindV1::NotHelper,
        );
        let mut associations =
            helper_memory_vec_v1(correspondence.lowered_functions.len(), budget)?;
        for row in &correspondence.lowered_functions {
            budget.charge_work(executable.module().functions.len())?;
            let physical = executable
                .module()
                .functions
                .iter()
                .position(|f| f.id == row.kernel_ir_function)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if row.semantic_function == relation.helper {
                functions[physical] = RetainedHelperKindV1::Bf16Nominal;
            }
            associations.push(RetainedHelperAssociationV1 { physical });
        }
        let storage = argument_sum_v1(&[
            envelope.storage,
            std::mem::size_of::<Self>(),
            argument_product_v1(
                functions.capacity(),
                std::mem::size_of::<RetainedHelperKindV1>(),
            )?,
            argument_product_v1(
                associations.capacity(),
                std::mem::size_of::<RetainedHelperAssociationV1>(),
            )?,
        ])?;
        let capture = semantic_ssa
            .occurrence_storage()
            .ok_or_else(|| bf16_emission_refusal_v1("BF16 capture receipt absent"))?;
        if budget.storage() < capture.retained_storage() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.reserve_storage(std::mem::size_of::<Self>())?;
        Ok(Self {
            functions,
            associations,
            allocations: Vec::new(),
            accesses: Vec::new(),
            control: Vec::new(),
            edge_bindings: Vec::new(),
            unit_source: SealedUnitLocalSourceV1::empty(),
            // The retained emission envelope already prepays the relation payload.
            // Keep only its pointer inline: ordinary source owners traverse
            // deeply nested compiler transactions and need no nominal payload.
            bf16_nominal: Some(Box::new(relation)),
            capture: HelperOccurrenceCaptureV1::Preexisting(capture),
            storage: ProductionHelperMemoryStorageV1(storage),
            analysis_storage: helper_memory_live_storage_v1(
                payload_storage.0,
                payload_storage.1,
                storage,
            )?,
        })
    }
}
