use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirLoopUnrollCopyV1 as CopyRole,
    CheckedCanonicalKirCrossBlockForwardingV1 as ForwardingPair,
    CheckedCanonicalKirInductionRefinementV1 as RefinementPair,
    CheckedCanonicalKirLoopUnrollPairV1 as UnrollPair,
};

// Neither the F permission nor inert rows alone admit U arithmetic. All actual
// relations and the exact U inventory stay borrowed for every permission query.
pub(super) struct CheckedUnrolledAdds<'p, 'r, 'f, 'i, 'o, 'rows> {
    previous: refined_forwarding::CheckedForwardedAdds<'p, 'r, 'f>,
    unroll: &'p UnrollPair<'i, 'o, 'rows>,
    input: &'p CanonicalKirInventoryV1<'f>,
    inventory: &'p CanonicalKirInventoryV1<'o>,
    sums: Vec<Option<usize>>,
}
impl<'p, 'r, 'f, 'i, 'o, 'rows> CheckedUnrolledAdds<'p, 'r, 'f, 'i, 'o, 'rows> {
    pub(super) fn new(
        refinement: &'p RefinementPair<'r>,
        forwarding: &'p ForwardingPair<'f>,
        intermediate: &CanonicalKirInventoryV1<'_>,
        input: &'p CanonicalKirInventoryV1<'f>,
        unroll: &'p UnrollPair<'i, 'o, 'rows>,
        inventory: &'p CanonicalKirInventoryV1<'o>,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> R<Self> {
        charge(budget, 9)?;
        if !std::ptr::eq(forwarding.output(), unroll.input())
            || !std::ptr::eq(input.owner(), unroll.input())
            || !std::ptr::eq(inventory.owner(), unroll.output())
        {
            return Err(refused("unrolled refined Add", "actual L/R/F/U endpoints"));
        }
        let header = std::mem::size_of::<Self>()
            .checked_sub(std::mem::size_of::<
                refined_forwarding::CheckedForwardedAdds<'_, '_, '_>,
            >())
            .and_then(|n| n.checked_sub(std::mem::size_of::<Vec<Option<usize>>>()))
            .ok_or_else(arithmetic)?;
        budget.reserve_storage(header).map_err(E::Resource)?;
        let previous = refined_forwarding::CheckedForwardedAdds::new(
            refinement,
            forwarding,
            intermediate,
            input,
            budget,
        )?;
        let count = inventory.operations().len();
        let mut sums = scratch::<Option<usize>>(count, budget)?;
        let mut mapping = scratch::<Option<(usize, CopyRole)>>(count, budget)?;
        charge(budget, count.checked_mul(2).ok_or_else(arithmetic)?)?;
        sums.resize(count, None);
        mapping.resize(count, None);
        for row in unroll.origins().operations {
            charge(budget, 9)?;
            let old = operation_ordinal(input, row.input)?;
            match (row.output, row.copy) {
                (None, CopyRole::OmittedBody) => {}
                (Some(output), CopyRole::Retained | CopyRole::Header(_) | CopyRole::Body(_)) => {
                    let at = operation_ordinal(inventory, output)?;
                    if mapping[at].is_some() {
                        return Err(refused("unrolled refined Add", "unique final occurrence"));
                    }
                    mapping[at] = Some((old, row.copy));
                }
                _ => return Err(refused("unrolled refined Add", "closed copy role")),
            }
        }
        for (ordinal, entry) in mapping.iter().enumerate() {
            charge(budget, 12)?;
            let (old, role) =
                entry.ok_or_else(|| refused("unrolled refined Add", "complete U operations"))?;
            if !previous.operation(input, old, budget)? {
                continue;
            }
            let old_flag = old.checked_add(1).ok_or_else(arithmetic)?;
            let flag = ordinal.checked_add(1).ok_or_else(arithmetic)?;
            let sum = &inventory.operations()[ordinal];
            let false_row = inventory
                .operations()
                .get(flag)
                .ok_or_else(|| refused("unrolled refined Add", "surviving same-copy false"))?;
            if mapping.get(flag) != Some(&Some((old_flag, role)))
                || sum.coordinate.block != false_row.coordinate.block
                || sum.coordinate.operation.checked_add(1) != Some(false_row.coordinate.operation)
                || !matches!(
                    sum.operation.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        ..
                    }
                )
                || sum.operation.results.len() != 1
                || sum.operation.results[0].ty != input.operations()[old].operation.results[0].ty
                || false_row.operation.kind
                    != OperationKind::Constant(fe2o3_kernel_ir::Constant::Bool(false))
                || !matches!(false_row.operation.results.as_slice(), [value] if value.ty == Type::BOOL)
            {
                return Err(refused(
                    "unrolled refined Add",
                    "mapped sum and false in the same clone",
                ));
            }
            sums[ordinal] = Some(old);
        }
        Ok(Self {
            previous,
            unroll,
            input,
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
        charge(budget, 6)?;
        if !std::ptr::eq(self.inventory, inventory)
            || !std::ptr::eq(self.input.owner(), self.unroll.input())
            || !std::ptr::eq(inventory.owner(), self.unroll.output())
            || self.sums.len() != inventory.operations().len()
        {
            return Err(refused(
                "unrolled refined Add",
                "retained three-pair final inventory",
            ));
        }
        match self
            .sums
            .get(ordinal)
            .copied()
            .ok_or_else(|| refused("unrolled refined Add", "actual final ordinal"))?
        {
            Some(original) => self.previous.operation(self.input, original, budget),
            None => Ok(false),
        }
    }
}

#[cfg(test)]
#[path = "production_checked_output_loop_unroll_native_census_v1_tests.rs"]
mod tests;
