//! Uniform operands of an exact invocation-derived expression, not new roots of
//! invocation provenance. No loads, captures, or loop recurrences are resolved.
use super::*;

// O(1) source/physical ABI lookups. Caller retains type, initialization, escape,
// assignment, dominance and final invocation-dependence checks.
fn exact_by_value_argument(function: &SemanticFunctionDeclV1, local: usize) -> bool {
    let Some(local) = function.locals().get(local) else {
        return false;
    };
    let SemanticLocalRoleV1::Argument(origin) = local.role() else {
        return false;
    };
    let origin = origin as usize;
    let abi = function.abi();
    if abi.source_input_types().get(origin) != Some(&local.ty())
        || abi.source_argument_ownership().get(origin)
            != Some(&SemanticSourceArgumentOwnershipV1::ByValue)
    {
        return false;
    }
    let Some(argument) = abi.fixed_arguments().get(origin) else {
        return false;
    };
    argument.role() == SemanticAbiArgumentRoleV1::Source
        && argument.value().source_ty() == local.ty()
        && argument.value().adjusted_ty() == local.ty()
        && matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_))
}

impl TotalUnsignedIndexProjectorV1<'_, '_, '_> {
    pub(super) fn with_exclusive_source_arguments_v1(
        mut self,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        if !matches!(self.roots, TotalUnsignedIndexRootsV1::Invocation { .. })
            || self.optional_source.is_some()
            || self.node_work != 0
            || !self.states.is_empty()
        {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "exclusive source arguments require a fresh mandatory invocation query",
            ));
        }
        self.exclusive_source_arguments = true;
        Ok(self)
    }

    pub(super) fn exclusive_source_argument_v1(
        &mut self,
        local: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        if !self.exclusive_source_arguments
            || self.optional_source.is_some()
            || !matches!(self.roots, TotalUnsignedIndexRootsV1::Invocation { .. })
        {
            return Ok(false);
        }
        // Logical work for the local/ABI/ownership lookups and scalar binding
        // checks. Uses the existing shared owner, never a fresh allowance.
        self.assertion_proofs.charge(8)?;
        Ok(exact_by_value_argument(self.function, local))
    }
}

#[cfg(test)]
mod tests;
