//! Capacity-owned append builder and immutable operand lookup. The caller still
//! authenticates the exact borrowed body, types, and operand coordinates.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Record {
    operand: *const SemanticOperandV1,
    block: usize,
    statement: usize,
    ordinal: usize,
    range: UnsignedRangeProofV1,
}

impl Record {
    fn key(&self) -> (*const SemanticOperandV1, usize) {
        (self.operand, self.ordinal)
    }
}

const RECORD_CELLS: usize = std::mem::size_of::<Record>().div_ceil(std::mem::size_of::<usize>());
const BUILDER_CELLS: usize =
    std::mem::size_of::<FactBuilderV1>().div_ceil(std::mem::size_of::<usize>());
const RETAINED_CELLS: usize =
    std::mem::size_of::<Option<RetainedFactsV1>>().div_ceil(std::mem::size_of::<usize>());

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct FactBuilderV1 {
    records: Vec<Record>,
}

pub(super) struct RetainedFactsV1 {
    records: Vec<Record>,
    cells: WorkingCells,
}

fn storage_error() -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::Unsupported(STORAGE_LIMIT)
}

fn payload_cells(capacity: usize) -> Result<usize, ProductionRankedProjectionErrorV1> {
    capacity.checked_mul(RECORD_CELLS).ok_or_else(storage_error)
}

impl FactBuilderV1 {
    pub(super) fn new(
        cells: &mut WorkingCells,
        work: &mut usize,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        project_loop_graph_charge_v1(work, BUILDER_CELLS)?;
        cells.reserve(BUILDER_CELLS)?;
        Ok(Self {
            records: Vec::new(),
        })
    }

    pub(super) fn retained_cells(&self) -> Result<usize, ProductionRankedProjectionErrorV1> {
        payload_cells(self.records.capacity())?
            .checked_add(BUILDER_CELLS)
            .ok_or_else(storage_error)
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    #[cfg(test)]
    pub(super) fn capacity(&self) -> usize {
        self.records.capacity()
    }

    #[cfg(test)]
    pub(super) fn get(
        &self,
        key: &*const SemanticOperandV1,
    ) -> Option<(usize, usize, UnsignedRangeProofV1)> {
        self.records.iter().rev().find_map(|record| {
            (record.operand == *key).then_some((record.block, record.statement, record.range))
        })
    }

    pub(super) fn insert(
        &mut self,
        operand: &SemanticOperandV1,
        site: Stamp,
        range: UnsignedRangeProofV1,
        cells: &mut WorkingCells,
        work: &mut usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        if self.records.len() >= MAX_CELLS {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "helper-result range operand storage limit",
            ));
        }
        project_loop_graph_charge_v1(work, RECORD_CELLS)?;
        if self.records.len() == self.records.capacity() {
            self.grow(cells, work)?;
        }
        let ordinal = self.records.len();
        self.records.push(Record {
            operand,
            block: site.0,
            statement: site.1,
            ordinal,
            range,
        });
        Ok(())
    }

    fn grow(
        &mut self,
        cells: &mut WorkingCells,
        work: &mut usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        let old_payload = payload_cells(self.records.capacity())?;
        let requested = self
            .records
            .capacity()
            .checked_mul(2)
            .ok_or_else(storage_error)?
            .max(1);
        let requested_cells = payload_cells(requested)?
            .checked_add(BUILDER_CELLS)
            .ok_or_else(storage_error)?;
        // The old allocation stays owned while the replacement and scope are
        // live. Reuse the helper engine's existing rollback scope on all errors.
        let mut scope = working_scope_v1::WorkingScopeV1::new(cells, work)?;
        scope.cells().reserve(requested_cells)?;
        project_loop_graph_charge_v1(work, requested_cells)?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(requested)
            .map_err(|_| storage_error())?;
        let replacement_cells = payload_cells(replacement.capacity())?
            .checked_add(BUILDER_CELLS)
            .ok_or_else(storage_error)?;
        let extra = replacement_cells
            .checked_sub(requested_cells)
            .ok_or_else(storage_error)?;
        scope.cells().reserve(extra)?;
        project_loop_graph_charge_v1(work, extra)?;
        project_loop_graph_charge_v1(work, payload_cells(self.records.len())?)?;
        let mut replacement = scope.finish(replacement, replacement_cells)?;
        // No fallible operation follows the first mutation of the old builder.
        replacement.extend(self.records.drain(..));
        let old = std::mem::replace(&mut self.records, replacement);
        drop(old);
        cells.release(old_payload + BUILDER_CELLS);
        Ok(())
    }

    pub(super) fn finish(
        mut self,
        mut cells: WorkingCells,
        work: &mut usize,
    ) -> Result<RetainedFactsV1, ProductionRankedProjectionErrorV1> {
        if cells.live != self.retained_cells()? {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "helper range builder still owns non-fact storage",
            ));
        }
        cells.reserve(RETAINED_CELLS)?;
        project_loop_graph_charge_v1(work, RETAINED_CELLS)?;
        sort_records(&mut self.records, work)?;
        // Ascending ordinals preserve HashMap::insert's last-write-wins
        // behavior, including repeated observations at changed source sites.
        let mut written = 0;
        for read in 0..self.records.len() {
            project_loop_graph_charge_v1(work, 1)?;
            let repeated =
                written > 0 && self.records[written - 1].operand == self.records[read].operand;
            let destination = if repeated { written - 1 } else { written };
            if destination != read {
                project_loop_graph_charge_v1(work, RECORD_CELLS)?;
                self.records[destination] = self.records[read];
            }
            if !repeated {
                written += 1;
            }
        }
        self.records.truncate(written);
        let Self { records } = self;
        // The empty builder field left by mem::take remains inside the outer
        // facts owner. Its header reservation is retained, not refunded.
        Ok(RetainedFactsV1 { records, cells })
    }
}

impl RetainedFactsV1 {
    pub(super) fn at(
        &self,
        operand: &SemanticOperandV1,
        block: usize,
        statement: usize,
        work: &mut usize,
    ) -> Result<Option<UnsignedRangeProofV1>, ProductionRankedProjectionErrorV1> {
        debug_assert_eq!(
            self.cells.live,
            BUILDER_CELLS + RETAINED_CELLS + self.records.capacity() * RECORD_CELLS
        );
        let key = operand as *const _;
        let mut start = 0;
        let mut end = self.records.len();
        while start < end {
            project_loop_graph_charge_v1(work, 1)?;
            let middle = start + (end - start) / 2;
            let candidate = &self.records[middle];
            match candidate.operand.cmp(&key) {
                std::cmp::Ordering::Less => start = middle + 1,
                std::cmp::Ordering::Greater => end = middle,
                std::cmp::Ordering::Equal => {
                    return Ok(
                        (candidate.block == block && candidate.statement == statement)
                            .then_some(candidate.range),
                    );
                }
            }
        }
        Ok(None)
    }
}

fn sort_records(
    records: &mut [Record],
    work: &mut usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    for root in (0..records.len() / 2).rev() {
        sift_down(records, root, work)?;
    }
    for end in (1..records.len()).rev() {
        project_loop_graph_charge_v1(work, 2 * RECORD_CELLS)?;
        records.swap(0, end);
        sift_down(&mut records[..end], 0, work)?;
    }
    Ok(())
}

fn sift_down(
    records: &mut [Record],
    mut root: usize,
    work: &mut usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    loop {
        project_loop_graph_charge_v1(work, 1)?;
        let mut child = root
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(storage_error)?;
        if child >= records.len() {
            return Ok(());
        }
        if child + 1 < records.len() {
            project_loop_graph_charge_v1(work, 1)?;
            if records[child].key() < records[child + 1].key() {
                child += 1;
            }
        }
        project_loop_graph_charge_v1(work, 1)?;
        if records[root].key() >= records[child].key() {
            return Ok(());
        }
        project_loop_graph_charge_v1(work, 2 * RECORD_CELLS)?;
        records.swap(root, child);
        root = child;
    }
}

#[cfg(test)]
#[path = "fact_builder_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "retained_facts_v1/tests.rs"]
mod retained_tests;
