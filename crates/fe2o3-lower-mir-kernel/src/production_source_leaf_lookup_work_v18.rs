// Raw source addresses identify occurrences, but their ordering must not change
// the billed work of reconstructing the same immutable source program.
struct SourceLeafLookupWorkV18 {
    remaining: usize,
}

impl PrivateArrayChargeV1 for SourceLeafLookupWorkV18 {
    type Error = ProductionSourceOwnedViewErrorV18;

    fn charge_private_array_work(&mut self, amount: usize) -> SourceOwnedResultV18<()> {
        self.remaining = self
            .remaining
            .checked_sub(amount)
            .ok_or(ArgumentResourceV1::Accounting)?;
        Ok(())
    }
}

fn source_leaf_lookup_work_headers_v18() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            size_of::<Result<T, ArgumentResourceV1>>(),
            size_of::<SourceOwnedResultV18<T>>(),
        ])
    }
    type Sort<'a, 'w> = (
        &'a mut [SourceScalarLeafLookupV18],
        &'a mut ArgumentBudgetV1<'w>,
    );
    type Search<'a, 'w> = (
        &'a [SourceScalarLeafLookupV18],
        [usize; 3],
        &'a mut ArgumentBudgetV1<'w>,
    );
    type Debit<'a> = (&'a mut SourceLeafLookupWorkV18, usize);
    argument_sum_v1(&[
        h::<Sort<'_, '_>>()?,
        h::<Search<'_, '_>>()?,
        h::<Debit<'_>>()?,
        h::<SourceLeafLookupWorkV18>()?,
        h::<&mut SourceLeafLookupWorkV18>()?,
        h::<&SourceScalarLeafLookupV18>()?,
        h::<[usize; 3]>()?,
        h::<[usize; 3]>()?,
        h::<[usize; 8]>()?,
        h::<Option<usize>>()?,
        h::<u32>()?,
        h::<bool>()?,
        h::<&[usize]>()?,
        h::<Result<(), fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1>>()?,
        h::<()>()?,
    ])
}

fn source_leaf_partition_work_v18(length: usize) -> Result<usize, ArgumentResourceV1> {
    let levels = (usize::BITS - length.leading_zeros()) as usize;
    // Each midpoint: loop 1, midpoint 3, up to 3 key fields, branch 2,
    // optional left update 1. The empty-range terminal visit costs 1.
    argument_sum_v1(&[1, argument_product_v1(10, levels)?])
}

fn source_leaf_sort_work_v18(length: usize) -> Result<usize, ArgumentResourceV1> {
    let levels = (usize::BITS - length.leading_zeros()) as usize;
    let exchanges = length.saturating_sub(1);
    let sifts = argument_sum_v1(&[length / 2, exchanges])?;
    // A sift visits at most bit_width(n) levels. Its full two-comparison
    // path costs 4+2+3+3+3+3+1=19; shorter paths are covered by that bound.
    argument_sum_v1(&[
        1,
        exchanges,
        argument_product_v1(19, argument_product_v1(levels, sifts)?)?,
    ])
}

fn source_leaf_lookup_sort_v18(
    rows: &mut [SourceScalarLeafLookupV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let required = source_leaf_sort_work_v18(rows.len())?;
    budget.charge_work(required)?;
    let mut prepaid = SourceLeafLookupWorkV18 {
        remaining: required,
    };
    private_array_heapsort_v1(
        rows,
        |row| row.key,
        &mut prepaid,
        || ArgumentResourceV1::Arithmetic.into(),
    )
}

fn source_leaf_address_partition_v18(
    rows: &[SourceScalarLeafLookupV18],
    key: [usize; 3],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    let required = source_leaf_partition_work_v18(rows.len())?;
    budget.charge_work(required)?;
    let mut prepaid = SourceLeafLookupWorkV18 {
        remaining: required,
    };
    private_array_partition_v1(rows, |row| row.key, key, false, &mut prepaid)
}

#[cfg(test)]
mod source_leaf_lookup_work_tests {
    use super::*;

    include!("production_source_leaf_lookup_work_v18_tests.rs");
}
