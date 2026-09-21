use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInductionRefinementOriginV1 as Origin,
    CheckedCanonicalKirInductionRefinementV1 as Pair,
};

// This table is only a cache of a live sealed actual-pair witness. No caller
// predicate, raw mask or source name can construct an integer-Add allowance.
pub(super) struct CheckedAdds<'p, 'i, 'g> {
    pair: &'p Pair<'g>,
    inventory: &'i CanonicalKirInventoryV1<'g>,
    sums: Vec<bool>,
}
impl<'p, 'i, 'g> CheckedAdds<'p, 'i, 'g> {
    pub(super) fn new(
        pair: &'p Pair<'g>,
        inventory: &'i CanonicalKirInventoryV1<'g>,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> R<Self> {
        charge(budget, 3)?;
        if !std::ptr::eq(pair.output(), inventory.owner()) {
            return Err(refused("checked induction Add", "actual pair/output owner"));
        }
        let header = std::mem::size_of::<Self>()
            .checked_sub(std::mem::size_of::<Vec<bool>>())
            .ok_or_else(arithmetic)?;
        budget.reserve_storage(header).map_err(E::Resource)?;
        let mut sums = scratch::<bool>(inventory.operations().len(), budget)?;
        charge(budget, inventory.operations().len())?;
        sums.resize(inventory.operations().len(), false);
        for row in pair.origins() {
            charge(budget, 12)?;
            if let Origin::CheckedAddSplit {
                sum_output,
                false_output,
                ..
            } = *row
            {
                let ordinal = operation_ordinal(inventory, sum_output)?;
                let false_ordinal = operation_ordinal(inventory, false_output)?;
                if sums[ordinal]
                    || ordinal.checked_add(1) != Some(false_ordinal)
                    || inventory.operations()[ordinal].coordinate != sum_output
                    || inventory.operations()[false_ordinal].coordinate != false_output
                    || !matches!(
                        inventory.operations()[ordinal].operation.kind,
                        OperationKind::Binary {
                            op: BinaryOp::Add,
                            ..
                        }
                    )
                    || !matches!(inventory.operations()[ordinal].operation.results.as_slice(),
                        [value] if matches!(value.ty, Type::Scalar(fe2o3_kernel_ir::ScalarType::U8 | fe2o3_kernel_ir::ScalarType::U16 | fe2o3_kernel_ir::ScalarType::U32 | fe2o3_kernel_ir::ScalarType::U64)))
                    || inventory.operations()[false_ordinal].operation.kind
                        != OperationKind::Constant(fe2o3_kernel_ir::Constant::Bool(false))
                {
                    return Err(refused(
                        "checked induction Add",
                        "exact adjacent checked sum",
                    ));
                }
                sums[ordinal] = true;
            }
        }
        Ok(Self {
            pair,
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
        charge(budget, 3)?;
        if !std::ptr::eq(self.inventory, inventory)
            || !std::ptr::eq(self.pair.output(), inventory.owner())
            || self.sums.len() != inventory.operations().len()
        {
            return Err(refused(
                "checked induction Add",
                "retained pair/output inventory",
            ));
        }
        self.sums
            .get(ordinal)
            .copied()
            .ok_or_else(|| refused("checked induction Add", "actual output ordinal"))
    }
}

#[cfg(test)]
#[path = "production_checked_output_induction_refinement_native_census_v1_tests.rs"]
mod tests;
