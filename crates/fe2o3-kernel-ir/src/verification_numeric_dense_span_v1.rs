use super::{VerificationNumericIndexRowV1, verification_radix_sort_u32_by_key_v1};
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
};

// Active dense-owner cells: minimum, three Vec header words, and mode tag.
// None owns no additional cells in this abstract ledger. This is not byte-exact
// Rust layout accounting, and does not consume conservative per-row spare cells.
const DENSE_METADATA_CELLS_V1: usize = 5;

pub(super) struct VerificationNumericDenseSpanV1 {
    minimum: u32,
    ends: Vec<usize>,
}

impl VerificationNumericDenseSpanV1 {
    pub(super) fn retained_storage(&self) -> usize {
        // The sum was checked before the owner was allocated.
        self.ends.len() + DENSE_METADATA_CELLS_V1
    }

    pub(super) fn find<T: Copy>(
        &self,
        rows: &[VerificationNumericIndexRowV1<T>],
        key: u32,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Option<usize>, CanonicalKernelIrVerificationResourceErrorV1> {
        budget.charge_work(1)?;
        let Some(offset) = key
            .checked_sub(self.minimum)
            .and_then(|offset| usize::try_from(offset).ok())
            .filter(|offset| *offset < self.ends.len())
        else {
            return Ok(None);
        };
        // Two endpoint decisions, including the implicit zero before bucket zero.
        budget.charge_work(2)?;
        let end = self.ends[offset];
        let start = offset
            .checked_sub(1)
            .map_or(0, |previous| self.ends[previous]);
        if start == end {
            return Ok(None);
        }
        budget.charge_work(1)?;
        let ordinal = end
            .checked_sub(1)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
        let row = rows
            .get(ordinal)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
        if start > end || row.key != key {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
        }
        Ok(Some(ordinal))
    }
}

pub(super) fn sort_numeric_rows_v1<T: Copy>(
    rows: &mut [VerificationNumericIndexRowV1<T>],
    row_storage: usize,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<Option<VerificationNumericDenseSpanV1>, CanonicalKernelIrVerificationResourceErrorV1> {
    let count = rows.len();
    if count < 2 {
        return Ok(None);
    }
    // First key read1; each later key read1 and min/max comparisons2.
    let census_work = count
        .checked_mul(3)
        .and_then(|work| work.checked_sub(2))
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    budget.charge_work(census_work)?;
    let mut minimum = rows[0].key;
    let mut maximum = minimum;
    for row in &rows[1..] {
        let key = row.key;
        minimum = minimum.min(key);
        maximum = maximum.max(key);
    }
    // Checked subtraction/addition, usize conversion, checked doubling, comparison.
    budget.charge_work(5)?;
    let span = u64::from(maximum)
        .checked_sub(u64::from(minimum))
        .and_then(|difference| difference.checked_add(1))
        .and_then(|span| usize::try_from(span).ok());
    let dense_span = span
        .zip(count.checked_mul(2))
        .and_then(|(span, bound)| (span <= bound).then_some(span));
    let Some(span) = dense_span else {
        verification_radix_sort_u32_by_key_v1(rows, row_storage, budget, |row| row.key)?;
        return Ok(None);
    };
    build_dense_span_v1(
        rows,
        minimum,
        span,
        row_storage,
        budget,
        #[cfg(test)]
        None,
    )
    .map(Some)
}

#[cfg(test)]
#[derive(Clone, Copy, Eq, PartialEq)]
enum AllocationPointV1 {
    Ends,
    Scratch,
}

fn build_dense_span_v1<T: Copy>(
    rows: &mut [VerificationNumericIndexRowV1<T>],
    minimum: u32,
    span: usize,
    row_storage: usize,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    #[cfg(test)] allocation_failure: Option<AllocationPointV1>,
) -> Result<VerificationNumericDenseSpanV1, CanonicalKernelIrVerificationResourceErrorV1> {
    let count = rows.len();
    // Same row/bucket-pass convention as radix: initialize scratch, count,
    // stable scatter, copy back; initialize buckets and convert counts to starts.
    let work = count
        .checked_mul(4)
        .and_then(|work| {
            span.checked_mul(2)
                .and_then(|buckets| work.checked_add(buckets))
        })
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    let retained_storage = span
        .checked_add(DENSE_METADATA_CELLS_V1)
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    let scratch_storage = count
        .checked_mul(row_storage)
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    let additional_storage = retained_storage
        .checked_add(scratch_storage)
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    budget.charge_work(work)?;
    let checkpoint = budget.storage_checkpoint();
    budget.reserve_storage(additional_storage)?;

    let prepared = (|| {
        let mut ends = Vec::new();
        #[cfg(test)]
        if allocation_failure == Some(AllocationPointV1::Ends) {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Allocation);
        }
        ends.try_reserve_exact(span)
            .map_err(|_| CanonicalKernelIrVerificationResourceErrorV1::Allocation)?;
        ends.resize(span, 0_usize);
        let mut scratch = Vec::new();
        #[cfg(test)]
        if allocation_failure == Some(AllocationPointV1::Scratch) {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Allocation);
        }
        scratch
            .try_reserve_exact(count)
            .map_err(|_| CanonicalKernelIrVerificationResourceErrorV1::Allocation)?;
        scratch.extend_from_slice(rows);

        // The complete census proved all offsets fit span/usize. Each bucket
        // count and prefix is bounded by count; scatter writes each row once.
        for row in rows.iter() {
            let offset = (u64::from(row.key) - u64::from(minimum)) as usize;
            ends[offset] += 1;
        }
        let mut start = 0_usize;
        for end in &mut ends {
            let next = start + *end;
            *end = start;
            start = next;
        }
        for row in rows.iter() {
            let offset = (u64::from(row.key) - u64::from(minimum)) as usize;
            scratch[ends[offset]] = *row;
            ends[offset] += 1;
        }
        rows.copy_from_slice(&scratch);
        drop(scratch);
        Ok(VerificationNumericDenseSpanV1 { minimum, ends })
    })();
    match prepared {
        Ok(dense) => {
            budget.release_storage(scratch_storage)?;
            Ok(dense)
        }
        Err(error) => {
            // Both temporary owners have dropped before their admission is released.
            budget.rollback_storage(checkpoint)?;
            Err(error)
        }
    }
}

#[cfg(test)]
#[path = "verification_numeric_dense_span_v1_tests.rs"]
mod tests;
