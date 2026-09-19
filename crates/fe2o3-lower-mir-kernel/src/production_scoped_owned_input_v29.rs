// Owned copies of a checked projection, not authentication of its Rust producer.
#[derive(Clone, Copy, Debug)]
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
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    retained_storage: usize,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source replay remains gated")
)]
impl OwnedExecutionInputV29 {
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
                ledger: source.ledger,
                retained_storage: budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            })
        })
    }

    fn with_source<R>(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        launch: &crate::ProductionSourceLaunchRosterV1,
        budget: &mut ArgumentBudgetV1<'_>,
        visit: impl FnOnce(
            &ExecutionLifecycleSourceV29<'_>,
            &mut ArgumentBudgetV1<'_>,
        ) -> Result<R, ScopedModuleErrorV29>,
    ) -> Result<R, ScopedModuleErrorV29> {
        if self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.retained_storage
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(argument_sum_v1(&[
            66,
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
        let roots = scoped_slot_attempt_v29(budget, |budget| {
            let mut roots = emission_vec_v1(self.roots.len(), budget)?;
            for root in &self.roots {
                budget.charge_work(size_of::<ScopedRootRecipeV29>())?;
                roots.push(root.borrow(&self.semantic_sha256, owner)?);
            }
            Ok(roots)
        })?;
        let scratch = argument_product_v1(
            roots.capacity(),
            size_of::<crate::ProductionContextRootInputV29<'_>>(),
        )?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let source = ExecutionLifecycleSourceV29::new(
                owner,
                launch,
                ProductionExecutionSourceInputV29 {
                    semantic_sha256: &self.semantic_sha256,
                    roots: &roots,
                    classes: &self.classes,
                    events: &self.events,
                },
                budget,
            )?;
            visit(&source, budget)
        }));
        drop(roots);
        // Only this view's backing is temporary; the visitor may retain output.
        let cleanup = if self.ledger == budget.work_ledger_identity_v1() {
            budget.release_storage(scratch)
        } else {
            Err(ArgumentResourceV1::Accounting)
        };
        match result {
            Ok(result) => {
                cleanup?;
                result
            }
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
