// The complete checked view owns logical/adjusted/physical ABI correspondence.
// The native interpreter consumes only actual scalar slots, never source paths.
pub(super) trait NativeHelperMeter: Meter {
    fn check_call(&mut self, query: NativeHelperCallQuery<'_>) -> Result<bool, Error>;
}

pub(super) struct NativeHelperCallQuery<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    module: &'a Module,
    correspondence: &'a SemanticKirCorrespondenceV1,
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    source: &'a SemanticDirectCallV1,
    operation: &'a Operation,
    target: &'a Function,
}

impl NativeHelperCallQuery<'_> {
    pub(super) fn check(
        self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
        with_owner_call_v1(
            self.owner,
            self.module,
            self.correspondence,
            (self.root, self.caller, self.block),
            budget,
            |view| {
                if !std::ptr::eq(view.operation(), self.operation)
                    || !std::ptr::eq(view.source(), self.source)
                    || view.caller().correspondence_owner != self.root
                    || view.caller().semantic_function != self.caller
                    || view.callee().association().correspondence_owner != self.root
                    || view.callee().association().kernel_ir_function != self.target.id
                {
                    return Err(mismatch());
                }
                for argument in view.callee().source_arguments()? {
                    if argument.source_ownership() != SemanticSourceArgumentOwnershipV1::ByValue {
                        return Err(mismatch());
                    }
                }
                let OperationKind::Call { arguments, .. } = &self.operation.kind else {
                    return Err(mismatch());
                };
                let body = self.target.body.as_ref().ok_or_else(mismatch)?;
                if arguments.len() != self.target.signature.parameters.len()
                    || arguments.len() != body.parameters.len()
                {
                    return Err(mismatch());
                }
                for (slot, actual) in arguments.iter().enumerate() {
                    let physical = view.physical(slot)?.ok_or_else(mismatch)?;
                    let parameter = physical.parameter();
                    if physical.caller_value() != *actual
                        || parameter.value() != body.parameters[slot]
                        || parameter.ty() != &self.target.signature.parameters[slot]
                        || kir_semantic_scalar_v1(parameter.ty()).is_none()
                    {
                        return Err(mismatch());
                    }
                }
                if view.physical(arguments.len())?.is_some() {
                    return Err(mismatch());
                }
                Ok(())
            },
        )
    }
}

#[cfg(test)]
pub(super) fn check_test_call(
    query: NativeHelperCallQuery<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let result = budget.with_bounded_scratch_v1(
        usize::MAX - budget.work(),
        budget.storage_limit() - budget.storage(),
        |budget| query.check(budget),
    );
    match result {
        Ok(()) => Ok(true),
        Err(error @ ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_)) => Err(error),
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
pub(super) fn with_first_owner_call_for_test<R>(
    owner: &ProductionPreRankedKirOwnerV1,
    use_query: impl for<'q> FnOnce(NativeHelperCallQuery<'q>) -> R,
) -> R {
    let module = owner.executable.module();
    let root = owner.semantic_ssa.source_semantic().roots()[0];
    let block = owner
        .correspondence
        .call_returns
        .iter()
        .find(|row| {
            row.correspondence_owner == root
                && row.semantic_function == root
                && matches!(row.kind, SemanticKirCallReturnKindV1::Call { .. })
        })
        .unwrap()
        .semantic_block;
    // Setup borrows an already checked source/native call. The subject meter
    // below still rechecks it, on a separate budget with the limits under test.
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 512 << 20);
    owner
        .with_checked_call_v1(root, root, block, &mut budget, |view| {
            let target_id = &view.callee().association().kernel_ir_function;
            let target = module
                .functions
                .iter()
                .find(|function| &function.id == target_id)
                .unwrap();
            Ok(use_query(NativeHelperCallQuery {
                owner: &owner.semantic_ssa,
                module,
                correspondence: &owner.correspondence,
                root,
                caller: root,
                block,
                source: view.source(),
                operation: view.operation(),
                target,
            }))
        })
        .unwrap()
}
