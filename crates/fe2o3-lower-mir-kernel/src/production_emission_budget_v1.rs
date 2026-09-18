// Erase the shared ledger's borrow lifetime without changing its identity.
trait SemanticEmissionBudgetV1 {
    fn work_ledger_identity_v1(&self) -> fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;
    fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1>;
}

impl SemanticEmissionBudgetV1 for ArgumentBudgetV1<'_> {
    fn work_ledger_identity_v1(&self) -> fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 {
        ArgumentBudgetV1::work_ledger_identity_v1(self)
    }

    fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        ArgumentBudgetV1::charge_work(self, amount).map_err(Into::into)
    }
}

impl SemanticFunctionLoweringV1<'_> {
    fn with_emission_budget_v1<T>(
        &mut self,
        body: impl FnOnce(
            &mut Self,
            &mut dyn SemanticEmissionBudgetV1,
        ) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        let budget = self.emission_work.take().ok_or_else(|| {
            unsupported(
                self.semantic_function.index(),
                None,
                None,
                "semantic emission has no shared resource ledger",
            )
        })?;
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(self, &mut *budget)));
        self.emission_work = Some(budget);
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}
