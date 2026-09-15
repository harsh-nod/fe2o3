use super::*;

/// A complete GPU write-path predicate and its original extraction budget.
/// This receipt is ephemeral and cannot be constructed from a CPU contract.
pub(crate) struct ScalarWriteGuardV1<'a> {
    pub(super) predicate: ReferencePathPredicateV1,
    pub(super) expressions: GpuGuardExpressionsV2<'a>,
    pub(super) exclusive_output: bool,
    pub(super) work: ReferenceSymbolicWorkBudgetV2,
}

impl ScalarWriteGuardV1<'_> {
    pub(crate) fn is_write(&self, write: &RankedGpuWriteV2) -> bool {
        std::ptr::eq(self.expressions.write, write)
    }

    pub(crate) fn proves_less_than(
        &mut self,
        write: &RankedGpuWriteV2,
        lhs: ProductionRankedValueV1,
        rhs: ProductionRankedValueV1,
    ) -> Result<bool, &'static str> {
        if !self.is_write(write) {
            return Err("scalar selection guard belongs to another write");
        }
        let lhs = self
            .expressions
            .index(lhs, 0, &mut self.work)
            .map_err(|_| "scalar selection index lacks an exact GPU guard expression")?;
        let rhs = self
            .expressions
            .index(rhs, 0, &mut self.work)
            .map_err(|_| "scalar selection extent lacks an exact GPU guard expression")?;
        let condition = ReferenceEffectExpressionV1::Binary {
            operation: ReferenceBinaryOpV1::LessThan,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
            checked: false,
        };
        // This existing algebra sees only the already extracted GPU path
        // clauses. Source CPU assertions/guards are not premises here.
        transparent_guard_split_v1::preceding_bound(
            &self.predicate,
            &condition,
            write.block,
            &mut self.work,
        )
        .map_err(|_| "scalar selection guard proof exhausted its retained work budget")
    }
}

#[cfg(test)]
pub(crate) fn source_scalar_test_guard_v1<'a>(
    kernel: &'a ProductionRankedKernelV1,
    effect_ir: &'a ReferenceEffectIrV1,
    write: &'a RankedGpuWriteV2,
) -> Result<ScalarWriteGuardV1<'a>, ProductionReferenceEffectJoinErrorV2> {
    gpu_write_path_scope_v2(kernel, effect_ir, write)
}
