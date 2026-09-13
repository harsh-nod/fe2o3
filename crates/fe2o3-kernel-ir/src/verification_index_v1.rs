use std::cmp::Ordering;

use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    Function, FunctionId, Module,
};

const RADIX_BUCKETS_V1: usize = 256;
const U32_RADIX_PASSES_V1: usize = 4;

#[path = "verification_numeric_dense_span_v1.rs"]
mod numeric_dense;
use numeric_dense::{VerificationNumericDenseSpanV1, sort_numeric_rows_v1};

pub(crate) fn verification_ceil_log2_v1(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

/// Sorts in-place with a locally controlled comparison bound.
///
/// Each sift level performs at most two comparisons. Heap construction and
/// repeated root removal together perform fewer than `2*n*height` levels, so
/// `4*n*ceil_log2(n)` bounds every comparator invocation before mutation.
pub(crate) fn verification_bounded_sort_by_v1<T>(
    values: &mut [T],
    comparison_width: usize,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    mut compare: impl FnMut(&T, &T) -> Ordering,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    let count = values.len();
    if count < 2 {
        return Ok(());
    }
    let work = count
        .checked_mul(verification_ceil_log2_v1(count))
        .and_then(|work| work.checked_mul(4))
        .and_then(|work| work.checked_mul(comparison_width.max(1)))
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    budget.charge_work(work)?;

    for start in (0..count / 2).rev() {
        sift_down_v1(values, start, count, &mut compare);
    }
    for end in (1..count).rev() {
        values.swap(0, end);
        sift_down_v1(values, 0, end, &mut compare);
    }
    Ok(())
}

fn sift_down_v1<T>(
    values: &mut [T],
    mut root: usize,
    end: usize,
    compare: &mut impl FnMut(&T, &T) -> Ordering,
) {
    loop {
        let Some(child) = root.checked_mul(2).and_then(|child| child.checked_add(1)) else {
            return;
        };
        if child >= end {
            return;
        }
        let right = child + 1;
        let selected = if right < end && compare(&values[child], &values[right]).is_lt() {
            right
        } else {
            child
        };
        if !compare(&values[root], &values[selected]).is_lt() {
            return;
        }
        values.swap(root, selected);
        root = selected;
    }
}

/// Stably sorts sparse `u32` keys in linear work without allocating by the
/// largest observed ID. The caller supplies the abstract cells retained by one
/// scratch row so the temporary copy is admitted before allocation.
/// Keys must remain fixed for each row throughout this call.
pub(crate) fn verification_radix_sort_u32_by_key_v1<T: Copy>(
    values: &mut [T],
    scratch_cells_per_row: usize,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    key: impl Fn(&T) -> u32,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    let count = values.len();
    if count < 2 {
        return Ok(());
    }
    // Cache the first key, then prepay each next key read and comparison.
    // An ordered proof needs no scratch; an inversion uses the full radix path.
    budget.charge_work(1)?;
    let mut previous = key(&values[0]);
    let mut ordered = true;
    for value in &values[1..] {
        budget.charge_work(2)?;
        let current = key(value);
        if current < previous {
            ordered = false;
            break;
        }
        previous = current;
    }
    if ordered {
        return Ok(());
    }
    let per_pass = count
        .checked_mul(3)
        .and_then(|work| work.checked_add(2 * RADIX_BUCKETS_V1))
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    let work = per_pass
        .checked_mul(U32_RADIX_PASSES_V1)
        .and_then(|work| work.checked_add(count))
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    let scratch_storage = count
        .checked_mul(scratch_cells_per_row)
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    budget.charge_work(work)?;
    budget.reserve_storage(scratch_storage)?;

    let mut scratch = Vec::new();
    if scratch.try_reserve_exact(count).is_err() {
        budget.release_storage(scratch_storage)?;
        return Err(CanonicalKernelIrVerificationResourceErrorV1::Allocation);
    }
    scratch.extend_from_slice(values);

    for shift in [0_u32, 8, 16, 24] {
        let mut counts = [0_usize; RADIX_BUCKETS_V1];
        for value in values.iter() {
            counts[((key(value) >> shift) & 0xff) as usize] += 1;
        }
        let mut offset = 0_usize;
        for count in &mut counts {
            let next = offset + *count;
            *count = offset;
            offset = next;
        }
        for value in values.iter() {
            let bucket = ((key(value) >> shift) & 0xff) as usize;
            scratch[counts[bucket]] = *value;
            counts[bucket] += 1;
        }
        values.copy_from_slice(&scratch);
    }

    drop(scratch);
    budget.release_storage(scratch_storage)
}

/// Finds the last row whose key equals the needle using a charged local upper
/// bound search. The width must cover one key comparison, including a terminal
/// length/tag decision for variable-width keys.
pub(crate) fn verification_find_last_by_v1<T>(
    values: &[T],
    comparison_width: usize,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    mut compare_row_to_needle: impl FnMut(&T) -> Ordering,
) -> Result<Option<usize>, CanonicalKernelIrVerificationResourceErrorV1> {
    let width = comparison_width.max(1);
    budget.charge_work(1)?;
    let mut left = 0_usize;
    let mut right = values.len();
    while left < right {
        let middle = left + (right - left) / 2;
        budget.charge_work(width)?;
        if compare_row_to_needle(&values[middle]).is_gt() {
            right = middle;
        } else {
            left = middle + 1;
        }
    }
    let Some(candidate) = left.checked_sub(1) else {
        return Ok(None);
    };
    budget.charge_work(width)?;
    Ok(compare_row_to_needle(&values[candidate])
        .is_eq()
        .then_some(candidate))
}

#[derive(Clone, Copy)]
pub(crate) struct VerificationFunctionIndexRowV1<'module> {
    pub(crate) function: &'module Function,
    pub(crate) input_ordinal: usize,
}

pub(crate) struct VerificationFunctionIndexV1<'module> {
    rows: Vec<VerificationFunctionIndexRowV1<'module>>,
    maximum_identifier_bytes: usize,
    retained_storage: usize,
}

impl<'module> VerificationFunctionIndexV1<'module> {
    const ROW_STORAGE: usize = 2;

    pub(crate) fn build(
        module: &'module Module,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Self, CanonicalKernelIrVerificationResourceErrorV1> {
        let count = module.functions.len();
        budget.charge_work(count)?;
        let maximum_identifier_bytes = module
            .functions
            .iter()
            .map(|function| function.id.as_str().len())
            .max()
            .unwrap_or(0);
        let retained_storage = count
            .checked_mul(Self::ROW_STORAGE)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        budget.charge_work(count)?;
        let comparison_width = maximum_identifier_bytes
            .checked_add(2)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        budget.reserve_storage(retained_storage)?;
        let mut rows = Vec::new();
        if rows.try_reserve_exact(count).is_err() {
            drop(rows);
            budget.release_storage(retained_storage)?;
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Allocation);
        }
        rows.extend(
            module
                .functions
                .iter()
                .enumerate()
                .map(|(input_ordinal, function)| VerificationFunctionIndexRowV1 {
                    function,
                    input_ordinal,
                }),
        );
        if let Err(error) =
            verification_bounded_sort_by_v1(&mut rows, comparison_width, budget, |left, right| {
                left.function
                    .id
                    .cmp(&right.function.id)
                    .then_with(|| left.input_ordinal.cmp(&right.input_ordinal))
            })
        {
            drop(rows);
            budget.release_storage(retained_storage)?;
            return Err(error);
        }
        Ok(Self {
            rows,
            maximum_identifier_bytes,
            retained_storage,
        })
    }

    pub(crate) fn rows(&self) -> &[VerificationFunctionIndexRowV1<'module>] {
        &self.rows
    }

    pub(crate) fn find(
        &self,
        needle: &FunctionId,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Option<&'module Function>, CanonicalKernelIrVerificationResourceErrorV1> {
        self.find_row(needle, budget)
            .map(|row| row.map(|row| row.function))
    }

    pub(crate) fn find_row(
        &self,
        needle: &FunctionId,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        Option<VerificationFunctionIndexRowV1<'module>>,
        CanonicalKernelIrVerificationResourceErrorV1,
    > {
        let comparison_width = self
            .maximum_identifier_bytes
            .max(needle.as_str().len())
            .checked_add(1)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        verification_find_last_by_v1(&self.rows, comparison_width, budget, |row| {
            row.function.id.cmp(needle)
        })
        .map(|ordinal| ordinal.map(|ordinal| self.rows[ordinal]))
    }

    pub(crate) fn release(
        self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        drop(self.rows);
        budget.release_storage(self.retained_storage)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct VerificationNumericIndexRowV1<T: Copy> {
    pub(crate) key: u32,
    pub(crate) value: T,
}

pub(crate) struct VerificationNumericIndexV1<T: Copy> {
    rows: Vec<VerificationNumericIndexRowV1<T>>,
    dense: Option<VerificationNumericDenseSpanV1>,
    retained_storage: usize,
}

impl<T: Copy> VerificationNumericIndexV1<T> {
    pub(crate) fn build(
        count: usize,
        row_storage: usize,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        mut row: impl FnMut(
            usize,
        ) -> Result<
            VerificationNumericIndexRowV1<T>,
            CanonicalKernelIrVerificationResourceErrorV1,
        >,
    ) -> Result<Self, CanonicalKernelIrVerificationResourceErrorV1> {
        let checkpoint = budget.storage_checkpoint();
        let retained_storage = count
            .checked_mul(row_storage)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        budget.charge_work(count)?;
        budget.reserve_storage(retained_storage)?;
        let mut rows = Vec::new();
        if rows.try_reserve_exact(count).is_err() {
            drop(rows);
            budget.release_storage(retained_storage)?;
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Allocation);
        }
        for ordinal in 0..count {
            let next = match row(ordinal) {
                Ok(next) => next,
                Err(error) => {
                    drop(rows);
                    budget.release_storage(retained_storage)?;
                    return Err(error);
                }
            };
            rows.push(next);
        }
        let sorted = sort_numeric_rows_v1(&mut rows, row_storage, budget);
        let dense = match sorted {
            Ok(dense) => dense,
            Err(error) => {
                drop(rows);
                budget.release_storage(retained_storage)?;
                return Err(error);
            }
        };
        let Some(retained_storage) = retained_storage.checked_add(
            dense
                .as_ref()
                .map_or(0, VerificationNumericDenseSpanV1::retained_storage),
        ) else {
            drop(dense);
            drop(rows);
            budget.rollback_storage(checkpoint)?;
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic);
        };
        Ok(Self {
            rows,
            dense,
            retained_storage,
        })
    }

    pub(crate) fn rows(&self) -> &[VerificationNumericIndexRowV1<T>] {
        &self.rows
    }

    pub(crate) fn find(
        &self,
        key: u32,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Option<T>, CanonicalKernelIrVerificationResourceErrorV1> {
        let result = match &self.dense {
            Some(dense) => dense.find(&self.rows, key, budget),
            None => verification_find_last_by_v1(&self.rows, 1, budget, |row| row.key.cmp(&key)),
        };
        result.map(|ordinal| ordinal.map(|ordinal| self.rows[ordinal].value))
    }

    pub(crate) fn release(
        self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        drop(self.dense);
        drop(self.rows);
        budget.release_storage(self.retained_storage)
    }
}

#[cfg(test)]
#[path = "verification_radix_ordered_v1_tests.rs"]
mod ordered_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CanonicalKernelIrWorkBudgetV1;

    fn budget(
        work: &mut CanonicalKernelIrWorkBudgetV1,
        storage: usize,
    ) -> CanonicalKernelIrVerificationResourceBudgetV1<'_> {
        CanonicalKernelIrVerificationResourceBudgetV1::new(work, storage)
    }

    #[test]
    fn bounded_sort_matches_standard_order_for_representative_inputs() {
        const WIDTH: usize = 7;
        for input in [
            vec![9, 7, 5, 3, 1],
            vec![1, 2, 3, 4, 5],
            vec![4, 4, 4, 4, 4],
            vec![7, 1, 9, 1, 4, 7, 2],
        ] {
            let mut expected = input.clone();
            expected.sort();
            let mut actual = input;
            let exact_work = 4 * actual.len() * verification_ceil_log2_v1(actual.len()) * WIDTH;
            let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work);
            verification_bounded_sort_by_v1(
                &mut actual,
                WIDTH,
                &mut budget(&mut work, 0),
                |left, right| left.cmp(right),
            )
            .unwrap();
            assert_eq!(actual, expected);
            assert_eq!(work.work(), exact_work);
        }
    }

    #[test]
    fn bounded_sort_rejects_one_under_before_mutation() {
        const WIDTH: usize = 5;
        let mut values = [3, 1, 2];
        let original = values;
        let exact_work = 4 * values.len() * verification_ceil_log2_v1(values.len()) * WIDTH;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work - 1);
        assert!(
            verification_bounded_sort_by_v1(
                &mut values,
                WIDTH,
                &mut budget(&mut work, 0),
                |left, right| left.cmp(right),
            )
            .is_err()
        );
        assert_eq!(values, original);
        assert_eq!(work.work(), 0);
    }

    #[test]
    fn radix_sort_accepts_sparse_ids_and_has_exact_boundaries() {
        const ROW_CELLS: usize = 3;
        let original = [(u32::MAX, 0_u8), (7, 1), (0, 2), (7, 3), (1 << 31, 4)];
        const COUNT: usize = 5;
        // The first adjacent pair is inverted: initial key1 plus read/compare2.
        const EXACT_WORK: usize = 3 + 4 * (3 * COUNT + 2 * RADIX_BUCKETS_V1) + COUNT;
        const EXACT_STORAGE: usize = COUNT * ROW_CELLS;
        let expected = [(0, 2), (7, 1), (7, 3), (1 << 31, 4), (u32::MAX, 0)];

        let mut actual = original;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(EXACT_WORK);
        let mut exact = budget(&mut work, EXACT_STORAGE);
        verification_radix_sort_u32_by_key_v1(&mut actual, ROW_CELLS, &mut exact, |row| row.0)
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(exact.storage(), 0);
        assert_eq!(exact.peak_storage(), EXACT_STORAGE);
        assert_eq!(exact.work(), EXACT_WORK);

        let mut under = original;
        let mut under_work = CanonicalKernelIrWorkBudgetV1::new(EXACT_WORK - 1);
        assert!(
            verification_radix_sort_u32_by_key_v1(
                &mut under,
                ROW_CELLS,
                &mut budget(&mut under_work, EXACT_STORAGE),
                |row| row.0,
            )
            .is_err()
        );
        assert_eq!(under, original);
    }

    #[test]
    fn charged_search_covers_non_power_population_and_terminal_match() {
        const WIDTH: usize = 11;
        let values = [1_u32, 3, 5];
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1 + 3 * WIDTH);
        let found =
            verification_find_last_by_v1(&values, WIDTH, &mut budget(&mut work, 0), |row| {
                row.cmp(&5)
            })
            .unwrap();
        assert_eq!(found, Some(2));
        assert_eq!(work.work(), 1 + 3 * WIDTH);

        let mut empty_work = CanonicalKernelIrWorkBudgetV1::new(1);
        assert_eq!(
            verification_find_last_by_v1::<u32>(
                &[],
                WIDTH,
                &mut budget(&mut empty_work, 0),
                |row| row.cmp(&5),
            )
            .unwrap(),
            None
        );
        assert_eq!(empty_work.work(), 1);

        let mut under_work = CanonicalKernelIrWorkBudgetV1::new(3 * WIDTH);
        assert!(
            verification_find_last_by_v1(&values, WIDTH, &mut budget(&mut under_work, 0), |row| {
                row.cmp(&5)
            },)
            .is_err()
        );
        assert_eq!(under_work.work(), 1 + 2 * WIDTH);
    }

    #[test]
    fn function_index_preserves_last_definition_and_long_prefix_queries() {
        use crate::{FunctionRole, Signature};

        let prefix = "function-name-prefix-".repeat(64);
        let duplicate = FunctionId::new(format!("{prefix}duplicate"));
        let first = Function::declaration(duplicate.clone(), Signature::new(vec![], vec![]));
        let mut second = Function::declaration(duplicate.clone(), Signature::new(vec![], vec![]));
        second.role = FunctionRole::DeviceFfiExport;
        let foreign =
            Function::declaration(format!("{prefix}foreign"), Signature::new(vec![], vec![]));
        let mut module = Module::new("module");
        module.functions = vec![first, foreign, second];
        let mut under_work = CanonicalKernelIrWorkBudgetV1::new(2 * module.functions.len() - 1);
        let mut under_resources = budget(&mut under_work, usize::MAX);
        assert!(VerificationFunctionIndexV1::build(&module, &mut under_resources).is_err());
        assert_eq!(under_resources.storage(), 0);
        assert_eq!(under_resources.peak_storage(), 0);

        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut resources = budget(&mut work, usize::MAX);
        let index = VerificationFunctionIndexV1::build(&module, &mut resources).unwrap();
        assert_eq!(
            index.find(&duplicate, &mut resources).unwrap(),
            Some(&module.functions[2])
        );
        let duplicate_rows = index
            .rows()
            .windows(2)
            .find(|rows| rows[0].function.id == rows[1].function.id)
            .unwrap();
        assert_eq!(duplicate_rows[0].input_ordinal, 0);
        assert_eq!(duplicate_rows[1].input_ordinal, 2);
        index.release(&mut resources).unwrap();
        assert_eq!(resources.storage(), 0);
    }

    #[test]
    fn function_index_sort_denial_releases_rows_and_preserves_caller_floor() {
        let mut module = Module::new("module");
        module.functions = ["b", "a"]
            .map(|name| Function::declaration(name, crate::Signature::new(vec![], vec![])))
            .to_vec();
        // Two census visits, two row publications, then 4*n*height*(id_bytes+2).
        const WORK: usize = 2 + 2 + 4 * 2 * 3;
        const FLOOR: usize = 7;
        const ROWS: usize = 2 * 2;
        for limit in [WORK - 1, WORK] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut resources = budget(&mut work, FLOOR + ROWS);
            resources.reserve_storage(FLOOR).unwrap();
            let result = VerificationFunctionIndexV1::build(&module, &mut resources);
            if limit < WORK {
                assert!(matches!(result,
                    Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                        if error.actual() == WORK && error.limit() == limit));
                assert_eq!(resources.work(), 4);
            } else {
                let index = result.unwrap();
                assert_eq!(index.rows()[0].function.id.as_str(), "a");
                assert_eq!(resources.work(), WORK);
                index.release(&mut resources).unwrap();
            }
            assert_eq!(resources.storage(), FLOOR);
            assert_eq!(resources.peak_storage(), FLOOR + ROWS);
        }
    }

    #[test]
    fn numeric_index_retains_sparse_duplicate_input_order() {
        let input = [(u32::MAX, 'a'), (9, 'b'), (0, 'c'), (9, 'd')];
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut resources = budget(&mut work, usize::MAX);
        let index =
            VerificationNumericIndexV1::build(input.len(), 3, &mut resources, |input_ordinal| {
                let (key, value) = input[input_ordinal];
                Ok(VerificationNumericIndexRowV1 { key, value })
            })
            .unwrap();
        assert_eq!(index.find(9, &mut resources).unwrap(), Some('d'));
        assert_eq!(index.rows()[1].value, 'b');
        assert_eq!(index.rows()[2].value, 'd');
        index.release(&mut resources).unwrap();
        assert_eq!(resources.storage(), 0);
    }
}
