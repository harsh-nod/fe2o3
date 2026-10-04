//! Ephemeral exact-name joins. Rows borrow names and retain physical ordinals;
//! sorted positions are never source, output or descriptor-canonical authority.
use super::{Budget, Resource};
use fe2o3_compiler_lineage::MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3 as MAX_ROOTS;
use std::{cmp::Ordering, mem::size_of};

#[derive(Debug, Eq, PartialEq)]
pub(super) enum RootNameMatchV1 {
    Unique(usize),
    Missing,
    Duplicate,
}

struct Row<'a> {
    name: &'a str,
    ordinal: usize,
}

/// The existing enclosing receipt scope owns temporary storage cleanup. Neither
/// this index nor its borrowed rows can escape that scope's component call.
pub(super) struct ExactRootNameIndexV1<'a> {
    rows: Vec<Row<'a>>,
}

impl<'a> ExactRootNameIndexV1<'a> {
    pub(super) fn build(
        names: impl ExactSizeIterator<Item = &'a str>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Resource> {
        budget.charge_work(1)?;
        let count = names.len();
        if count > MAX_ROOTS {
            return Err(Resource::Accounting);
        }
        let prepaid = count
            .checked_mul(size_of::<Row<'_>>())
            .and_then(|n| n.checked_add(size_of::<Self>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(prepaid)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        let extra = rows
            .capacity()
            .checked_sub(count)
            .ok_or(Resource::Accounting)?
            .checked_mul(size_of::<Row<'_>>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(extra)?;
        for name in names {
            budget.charge_work(1)?;
            if rows.len() >= count || rows.len() >= rows.capacity() {
                return Err(Resource::Accounting);
            }
            rows.push(Row {
                name,
                ordinal: rows.len(),
            });
        }
        budget.charge_work(1)?;
        if rows.len() != count {
            return Err(Resource::Accounting);
        }
        sort(&mut rows, budget)?;
        Ok(Self { rows })
    }

    pub(super) fn find(
        &self,
        wanted: &str,
        budget: &mut Budget<'_>,
    ) -> Result<RootNameMatchV1, Resource> {
        let (mut first, mut end) = (0, self.rows.len());
        while first < end {
            budget.charge_work(1)?;
            let middle = first + (end - first) / 2;
            match compare_names(self.rows[middle].name, wanted, budget)? {
                Ordering::Less => first = middle + 1,
                Ordering::Greater => end = middle,
                Ordering::Equal => {
                    // Reject only duplicate matches for this requested name,
                    // preserving the old scan's error order for unrelated runs.
                    for neighbor in [middle.checked_sub(1), middle.checked_add(1)] {
                        budget.charge_work(1)?;
                        if let Some(row) = neighbor.and_then(|index| self.rows.get(index))
                            && compare_names(row.name, wanted, budget)? == Ordering::Equal
                        {
                            budget.charge_work(1)?;
                            return Ok(RootNameMatchV1::Duplicate);
                        }
                    }
                    budget.charge_work(1)?;
                    return Ok(RootNameMatchV1::Unique(self.rows[middle].ordinal));
                }
            }
        }
        budget.charge_work(1)?;
        Ok(RootNameMatchV1::Missing)
    }
}

fn compare_names(left: &str, right: &str, budget: &mut Budget<'_>) -> Result<Ordering, Resource> {
    budget.charge_work(
        left.len()
            .checked_add(right.len())
            .and_then(|n| n.checked_add(2))
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(left.cmp(right))
}

fn compare_rows(
    left: &Row<'_>,
    right: &Row<'_>,
    budget: &mut Budget<'_>,
) -> Result<Ordering, Resource> {
    let ordering = compare_names(left.name, right.name, budget)?;
    if ordering != Ordering::Equal {
        return Ok(ordering);
    }
    budget.charge_work(1)?;
    Ok(left.ordinal.cmp(&right.ordinal))
}

// Same bounded fallible heapsort as the private lowerer origin indexes. Keeping
// this name-specialized child private avoids exposing cross-crate internals.
fn sort(rows: &mut [Row<'_>], budget: &mut Budget<'_>) -> Result<(), Resource> {
    fn sift(
        rows: &mut [Row<'_>],
        mut root: usize,
        end: usize,
        budget: &mut Budget<'_>,
    ) -> Result<(), Resource> {
        loop {
            budget.charge_work(1)?;
            let child = root
                .checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .ok_or(Resource::Arithmetic)?;
            if child >= end {
                return Ok(());
            }
            let next = if child + 1 < end {
                budget.charge_work(1)?;
                if compare_rows(&rows[child], &rows[child + 1], budget)? == Ordering::Less {
                    child + 1
                } else {
                    child
                }
            } else {
                child
            };
            budget.charge_work(1)?;
            if compare_rows(&rows[root], &rows[next], budget)? != Ordering::Less {
                return Ok(());
            }
            budget.charge_work(1)?;
            rows.swap(root, next);
            root = next;
        }
    }
    let length = rows.len();
    if length < 2 {
        return Ok(());
    }
    for root in (0..length / 2).rev() {
        sift(rows, root, length, budget)?;
    }
    for end in (1..length).rev() {
        budget.charge_work(1)?;
        rows.swap(0, end);
        sift(rows, 0, end, budget)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_native_receipt_root_index_v1_tests.rs"]
mod tests;
