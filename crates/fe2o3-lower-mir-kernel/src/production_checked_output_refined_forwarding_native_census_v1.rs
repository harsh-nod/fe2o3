use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInductionRefinementOriginV1 as Origin,
    CheckedCanonicalKirCrossBlockForwardingV1 as ForwardingPair,
    CheckedCanonicalKirInductionRefinementV1 as RefinementPair,
};

// Two sealed actual relations, not a detached mask or a claim that R's witness
// ends at F. Both split operations must survive the forwarding pair unchanged.
pub(super) struct CheckedForwardedAdds<'p, 'r, 'g> {
    refinement: &'p RefinementPair<'r>,
    forwarding: &'p ForwardingPair<'g>,
    inventory: &'p CanonicalKirInventoryV1<'g>,
    sums: Vec<bool>,
}
impl<'p, 'r, 'g> CheckedForwardedAdds<'p, 'r, 'g> {
    pub(super) fn new(
        refinement: &'p RefinementPair<'r>,
        forwarding: &'p ForwardingPair<'g>,
        intermediate: &CanonicalKirInventoryV1<'_>,
        inventory: &'p CanonicalKirInventoryV1<'g>,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> R<Self> {
        charge(budget, 7)?;
        if !std::ptr::eq(refinement.output(), forwarding.input())
            || !std::ptr::eq(refinement.output(), intermediate.owner())
            || !std::ptr::eq(forwarding.output(), inventory.owner())
            || forwarding.origins().len() != intermediate.operations().len()
            || inventory.operations().len() != intermediate.operations().len()
        {
            return Err(refused(
                "refined forwarding Add",
                "actual L-to-R-to-F endpoints",
            ));
        }
        let header = std::mem::size_of::<Self>()
            .checked_sub(std::mem::size_of::<Vec<bool>>())
            .ok_or_else(arithmetic)?;
        budget.reserve_storage(header).map_err(E::Resource)?;
        let mut sums = scratch::<bool>(inventory.operations().len(), budget)?;
        charge(budget, inventory.operations().len())?;
        sums.resize(inventory.operations().len(), false);
        charge(
            budget,
            intermediate
                .owner()
                .canonical()
                .canonical_bytes()
                .len()
                .checked_add(inventory.owner().canonical().canonical_bytes().len())
                .ok_or_else(arithmetic)?,
        )?;
        for row in refinement.origins() {
            charge(budget, 28)?;
            let Origin::CheckedAddSplit {
                sum_output,
                false_output,
                ..
            } = *row
            else {
                continue;
            };
            let sum = operation_ordinal(intermediate, sum_output)?;
            let flag = operation_ordinal(intermediate, false_output)?;
            let final_sum = operation_ordinal(inventory, sum_output)?;
            let final_flag = operation_ordinal(inventory, false_output)?;
            let sum_row = forwarding
                .origins()
                .get(sum)
                .ok_or_else(|| refused("refined forwarding Add", "complete retained split rows"))?;
            let flag_row = forwarding
                .origins()
                .get(flag)
                .ok_or_else(|| refused("refined forwarding Add", "complete retained split rows"))?;
            let actual_sum = inventory.operations()[final_sum].operation;
            let actual_flag = inventory.operations()[final_flag].operation;
            if sum != final_sum
                || flag != final_flag
                || sum.checked_add(1) != Some(flag)
                || sums[final_sum]
                || sum_row.input != sum_output
                || sum_row.output != sum_output
                || sum_row.store.is_some()
                || flag_row.input != false_output
                || flag_row.output != false_output
                || flag_row.store.is_some()
                || actual_sum != intermediate.operations()[sum].operation
                || actual_flag != intermediate.operations()[flag].operation
                || !matches!(
                    actual_sum.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        ..
                    }
                )
                || !matches!(actual_sum.results.as_slice(), [value] if matches!(value.ty,
                    Type::Scalar(fe2o3_kernel_ir::ScalarType::U8 | fe2o3_kernel_ir::ScalarType::U16
                        | fe2o3_kernel_ir::ScalarType::U32 | fe2o3_kernel_ir::ScalarType::U64)))
                || actual_flag.kind
                    != OperationKind::Constant(fe2o3_kernel_ir::Constant::Bool(false))
                || !matches!(actual_flag.results.as_slice(), [value] if value.ty == Type::BOOL)
            {
                return Err(refused(
                    "refined forwarding Add",
                    "exact retained checked sum and false",
                ));
            }
            sums[final_sum] = true;
        }
        Ok(Self {
            refinement,
            forwarding,
            inventory,
            sums,
        })
    }
    pub(super) fn operation(
        &self,
        inventory: &CanonicalKirInventoryV1<'_>,
        ordinal: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> R<bool> {
        charge(budget, 5)?;
        if !std::ptr::eq(self.inventory, inventory)
            || !std::ptr::eq(self.refinement.output(), self.forwarding.input())
            || !std::ptr::eq(self.forwarding.output(), inventory.owner())
            || self.sums.len() != inventory.operations().len()
        {
            return Err(refused(
                "refined forwarding Add",
                "retained two-pair final inventory",
            ));
        }
        self.sums
            .get(ordinal)
            .copied()
            .ok_or_else(|| refused("refined forwarding Add", "actual final ordinal"))
    }
}

#[cfg(test)]
#[path = "production_checked_output_refined_forwarding_native_census_v1_tests.rs"]
mod tests;
