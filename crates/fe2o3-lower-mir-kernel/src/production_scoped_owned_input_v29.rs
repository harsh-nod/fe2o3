// Owned copies of a checked projection, not authentication of its Rust producer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedRootRecipeV29 {
    root: SemanticFunctionIdV1,
    root_identity: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdentityV1,
    helper: SemanticFunctionIdV1,
    helper_identity: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdentityV1,
    issuer: fe2o3_mir_model::semantic_mir_v1::SemanticCallableIdV1,
    issuer_identity: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdentityV1,
    context_type: SemanticTypeIdV1,
    context_identity: fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1,
    issuance: crate::ProductionContextCallBoundaryV29,
    helper_call: crate::ProductionContextCallBoundaryV29,
    helper_context_local: SemanticLocalIdV1,
}

impl ScopedRootRecipeV29 {
    fn capture(root: &crate::ProductionContextRootInputV29<'_>) -> Self {
        let crate::ProductionContextRootInputV29 {
            semantic_sha256: _,
            root,
            root_identity,
            helper,
            helper_identity,
            issuer,
            issuer_identity,
            context_type,
            context_identity,
            issuance,
            helper_call,
            helper_context_local,
            helper_arguments: _,
        } = *root;
        Self {
            root,
            root_identity,
            helper,
            helper_identity,
            issuer,
            issuer_identity,
            context_type,
            context_identity,
            issuance,
            helper_call,
            helper_context_local,
        }
    }

    fn borrow<'a>(
        &self,
        semantic_sha256: &'a [u8; 32],
        owner: &'a ProductionSemanticSsaOwnerV1,
    ) -> Result<crate::ProductionContextRootInputV29<'a>, ProductionSemanticKirErrorV1> {
        let block = owner
            .source_semantic()
            .functions()
            .get(self.root.index() as usize)
            .and_then(|function| {
                function
                    .blocks()
                    .get(self.helper_call.block.index() as usize)
            })
            .ok_or_else(execution_lifecycle_error_v29)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            return Err(execution_lifecycle_error_v29());
        };
        Ok(crate::ProductionContextRootInputV29 {
            semantic_sha256,
            root: self.root,
            root_identity: self.root_identity,
            helper: self.helper,
            helper_identity: self.helper_identity,
            issuer: self.issuer,
            issuer_identity: self.issuer_identity,
            context_type: self.context_type,
            context_identity: self.context_identity,
            issuance: self.issuance,
            helper_call: self.helper_call,
            helper_context_local: self.helper_context_local,
            helper_arguments: call.arguments(),
        })
    }
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source replay remains gated")
)]
struct OwnedExecutionInputV29 {
    semantic_sha256: [u8; 32],
    ssa: ProductionSemanticSsaIdentityV1,
    roots: Vec<ScopedRootRecipeV29>,
    classes: Vec<ProductionScopeCallableCandidateV29>,
    events: Vec<crate::ProductionScopeEventCandidateV29>,
    launch: Vec<crate::ProductionSourceLaunchRootV1>,
    kernel_argument_abi: Option<kernel_argument_abi_v18::CapturedKernelArgumentAbiV18>,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    retained_storage: usize,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source replay remains gated")
)]
impl OwnedExecutionInputV29 {
    // Relative equality only: the backend must retain its authentic receipt.
    // This allocation-free query is safe inside the temporary projection scope;
    // recipe/native consumers run only after that scope's vectors have settled.
    fn check_candidate_v18(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        input: ProductionExecutionSourceInputV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), crate::ProductionContextRootErrorV29> {
        use crate::ProductionContextRootErrorV29 as Error;
        if self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.retained_storage
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(100)?;
        if self.semantic_sha256 != *owner.source_semantic_sha256()
            || self.ssa != owner.identity()
            || self.semantic_sha256 != *input.semantic_sha256
        {
            return Err(Error::Source);
        }
        if self.roots.len() != input.roots.len() {
            return Err(Error::RootCensus);
        }
        if self.classes.len() != input.classes.len() {
            return Err(Error::CallableCensus);
        }
        if self.events.len() != input.events.len() {
            return Err(Error::ScopeEventCensus);
        }
        budget.charge_work(argument_product_v1(
            self.classes.len(),
            size_of::<ProductionScopeCallableCandidateV29>(),
        )?)?;
        if self.classes != input.classes {
            return Err(Error::CallableCensus);
        }
        budget.charge_work(argument_product_v1(
            self.events.len(),
            size_of::<crate::ProductionScopeEventCandidateV29>(),
        )?)?;
        if self.events != input.events {
            return Err(Error::ScopeEventCensus);
        }
        for (retained, candidate) in self.roots.iter().zip(input.roots) {
            budget.charge_work(argument_sum_v1(&[size_of::<ScopedRootRecipeV29>(), 38])?)?;
            if *candidate.semantic_sha256 != self.semantic_sha256
                || *retained != ScopedRootRecipeV29::capture(candidate)
            {
                return Err(Error::RootCensus);
            }
            let original = retained
                .borrow(&self.semantic_sha256, owner)
                .map_err(|_| Error::CallBoundary)?;
            crate::production_context_roots_v1::check_arguments(
                candidate.helper_arguments,
                original.helper_arguments,
                budget,
            )?;
        }
        Ok(())
    }

    // Reservation remains live; consuming source custody adopts it once.
    fn capture(
        source: &ExecutionLifecycleSourceV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        if source.ledger != budget.work_ledger_identity_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let floor = budget.storage();
        scoped_slot_attempt_v29(budget, |budget| {
            budget.reserve_storage(size_of::<Self>())?;
            let mut roots = emission_vec_v1(source.input.roots.len(), budget)?;
            for root in source.input.roots {
                budget.charge_work(size_of::<ScopedRootRecipeV29>())?;
                roots.push(ScopedRootRecipeV29::capture(root));
            }
            let classes = scoped_copy_rows_v29(source.input.classes, budget)?;
            let events = scoped_copy_rows_v29(source.input.events, budget)?;
            let launch = scoped_copy_rows_v29(source.launch.roots(), budget)?;
            budget.charge_work(64)?;
            Ok(Self {
                semantic_sha256: *source.owner.source_semantic_sha256(),
                ssa: source.owner.identity(),
                roots,
                classes,
                events,
                launch,
                kernel_argument_abi: None,
                ledger: source.ledger,
                retained_storage: budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            })
        })
    }

    fn capture_kernel_argument_abi_v18(
        &mut self,
        owner: &ProductionSemanticSsaOwnerV1,
        input: ProductionKernelArgumentAbiInputV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        if self.kernel_argument_abi.is_some()
            || self.semantic_sha256 != *owner.source_semantic_sha256()
            || self.ssa != owner.identity()
        {
            return Err(source_reference_error_v29("kernel argument ABI capture differs from its original source owner"));
        }
        if self.ledger != budget.work_ledger_identity_v1() || budget.storage() < self.retained_storage {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        // The containing source capture owns rollback. No independent attempt
        // can refund credits if its enclosing concrete custody is later lost.
        let profile = kernel_argument_abi_v18::CapturedKernelArgumentAbiV18::capture(owner, input, budget)?;
        self.retained_storage = argument_sum_v1(&[self.retained_storage, profile.retained_storage()])?;
        self.kernel_argument_abi = Some(profile);
        Ok(())
    }

    fn with_source<'work, R>(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        launch: &crate::ProductionSourceLaunchRosterV1,
        budget: &mut ArgumentBudgetV1<'work>,
        visit: impl FnOnce(
            &ExecutionLifecycleSourceV29<'_>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<R, ScopedModuleErrorV29>,
    ) -> Result<R, ScopedModuleErrorV29> {
        if self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.retained_storage
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let floor = budget.storage();
        with_scoped_source_cleanup_v29(budget, floor, |cleanup, budget| {
            self.with_source_with_cleanup(owner, launch, cleanup, budget, visit)
        })
    }

    fn with_source_with_cleanup<'work, R>(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        launch: &crate::ProductionSourceLaunchRosterV1,
        cleanup: &ScopedSourceCleanupV29,
        budget: &mut ArgumentBudgetV1<'work>,
        visit: impl FnOnce(
            &ExecutionLifecycleSourceV29<'_>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<R, ScopedModuleErrorV29>,
    ) -> Result<R, ScopedModuleErrorV29> {
        if cleanup.is_denied() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        if self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.retained_storage
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(argument_sum_v1(&[
            98,
            argument_product_v1(
                self.launch.len(),
                size_of::<crate::ProductionSourceLaunchRootV1>(),
            )?,
        ])?)?;
        if self.semantic_sha256 != *owner.source_semantic_sha256()
            || self.ssa != owner.identity()
            || launch.semantic_sha256() != &self.semantic_sha256
            || launch.roots() != self.launch.as_slice()
        {
            return Err(execution_lifecycle_error_v29().into());
        }
        if let Some(profile) = &self.kernel_argument_abi {
            profile.check(owner, budget)?;
        }
        let floor = budget.storage();
        let slot = std::ptr::from_ref(budget) as usize;
        let roots = scoped_source_attempt_v29(cleanup, budget, floor, |budget| {
            let mut roots = emission_vec_v1(self.roots.len(), budget)?;
            for root in &self.roots {
                budget.charge_work(size_of::<ScopedRootRecipeV29>())?;
                roots.push(root.borrow(&self.semantic_sha256, owner)?);
            }
            Ok::<_, ScopedModuleErrorV29>(roots)
        })?;
        let scratch = argument_product_v1(
            roots.capacity(),
            size_of::<crate::ProductionContextRootInputV29<'_>>(),
        )?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let source = ExecutionLifecycleSourceV29::with_kernel_arguments(
                owner,
                launch,
                ProductionExecutionSourceInputV29 {
                    semantic_sha256: &self.semantic_sha256,
                    roots: &roots,
                    classes: &self.classes,
                    events: &self.events,
                },
                self.kernel_argument_abi.as_ref(),
                budget,
            )?;
            visit(&source, budget)
        }));
        drop(roots);
        // Only this view's backing is temporary; the visitor may retain output.
        if slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || floor
                .checked_add(scratch)
                .is_none_or(|minimum| budget.storage() < minimum)
        {
            cleanup.deny_refund();
        }
        let settlement = if !cleanup.is_denied() {
            budget
                .release_storage(scratch)
                .inspect_err(|_| cleanup.deny_refund())
        } else {
            Err(ArgumentResourceV1::Accounting)
        };
        match result {
            Ok(Ok(value)) => match settlement {
                Ok(()) => Ok(value),
                Err(error) => {
                    drop(value);
                    Err(error.into())
                }
            },
            Ok(Err(error)) => Err(error),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

fn scoped_copy_rows_v29<T: Copy>(
    rows: &[T],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
    let mut copied = emission_vec_v1(rows.len(), budget)?;
    budget.charge_work(argument_sum_v1(&[
        argument_product_v1(rows.len(), size_of::<T>())?,
        1,
    ])?)?;
    copied.extend_from_slice(rows);
    Ok(copied)
}
