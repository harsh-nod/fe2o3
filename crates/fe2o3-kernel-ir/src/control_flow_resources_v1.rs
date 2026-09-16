use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum MeteredControlFlowErrorV1 {
    ControlFlow(ControlFlowError),
    Resource(CanonicalKernelIrVerificationResourceErrorV1),
}

impl From<ControlFlowError> for MeteredControlFlowErrorV1 {
    fn from(error: ControlFlowError) -> Self {
        Self::ControlFlow(error)
    }
}

impl From<CanonicalKernelIrVerificationResourceErrorV1> for MeteredControlFlowErrorV1 {
    fn from(error: CanonicalKernelIrVerificationResourceErrorV1) -> Self {
        Self::Resource(error)
    }
}

struct ControlFlowResourcesV1<'budget, 'work> {
    budget: Option<&'budget mut CanonicalKernelIrVerificationResourceBudgetV1<'work>>,
}

impl ControlFlowResourcesV1<'_, '_> {
    fn charge(&mut self, work: usize) -> Result<(), MeteredControlFlowErrorV1> {
        if let Some(budget) = self.budget.as_deref_mut() {
            budget.charge_work(work)?;
        }
        Ok(())
    }

    fn allocate<T>(
        &mut self,
        capacity: usize,
        row_cells: usize,
    ) -> Result<Vec<T>, MeteredControlFlowErrorV1> {
        let Some(budget) = self.budget.as_deref_mut() else {
            return Ok(Vec::with_capacity(capacity));
        };
        let cells = capacity
            .checked_mul(row_cells)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        budget.charge_work(1)?;
        budget.reserve_storage(cells)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| CanonicalKernelIrVerificationResourceErrorV1::Allocation)?;
        Ok(values)
    }

    fn filled<T: Copy>(
        &mut self,
        count: usize,
        value: T,
        row_cells: usize,
    ) -> Result<Vec<T>, MeteredControlFlowErrorV1> {
        let mut values = self.allocate(count, row_cells)?;
        self.charge(count)?;
        values.resize(count, value);
        Ok(values)
    }

    fn rows(&mut self, counts: &[usize]) -> Result<Vec<Vec<usize>>, MeteredControlFlowErrorV1> {
        let mut rows = self.allocate(counts.len(), 1)?;
        self.charge(checked_mul(
            ControlFlowResource::AnalysisWork,
            counts.len(),
            2,
        )?)?;
        for count in counts {
            rows.push(self.allocate(*count, 1)?);
        }
        Ok(rows)
    }

    fn free<T>(
        &mut self,
        values: Vec<T>,
        logical_capacity: usize,
        row_cells: usize,
    ) -> Result<(), MeteredControlFlowErrorV1> {
        drop(values);
        if let Some(budget) = self.budget.as_deref_mut() {
            let cells = logical_capacity
                .checked_mul(row_cells)
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
            budget.release_storage(cells)?;
        }
        Ok(())
    }

    fn free_rows(
        &mut self,
        rows: Vec<Vec<usize>>,
        entries: usize,
    ) -> Result<(), MeteredControlFlowErrorV1> {
        let count = rows.len();
        drop(rows);
        if let Some(budget) = self.budget.as_deref_mut() {
            let cells = count
                .checked_add(entries)
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
            budget.release_storage(cells)?;
        }
        Ok(())
    }

    fn sort_blocks(
        &mut self,
        positions: &mut [(BlockId, usize)],
    ) -> Result<(), MeteredControlFlowErrorV1> {
        if let Some(budget) = self.budget.as_deref_mut() {
            crate::verification_index_v1::verification_radix_sort_u32_by_key_v1(
                positions,
                2,
                budget,
                |row| row.0.0,
            )?;
        } else {
            // The ordinal tie-breaker matches the stable metered radix sort.
            positions.sort_unstable();
        }
        Ok(())
    }

    fn sort<T: Ord>(&mut self, values: &mut [T]) -> Result<(), MeteredControlFlowErrorV1> {
        if let Some(budget) = self.budget.as_deref_mut() {
            crate::verification_index_v1::verification_bounded_sort_by_v1(
                values,
                1,
                budget,
                Ord::cmp,
            )?;
        } else {
            values.sort_unstable();
        }
        Ok(())
    }

    fn block_position(
        &mut self,
        positions: &[(BlockId, usize)],
        block: BlockId,
    ) -> Result<Option<usize>, MeteredControlFlowErrorV1> {
        // Even an empty or out-of-range lookup pays its fixed entry action.
        self.charge(1)?;
        if let Some((candidate, position)) = positions.get(block.0 as usize) {
            self.charge(1)?;
            if *candidate == block {
                return Ok(Some(*position));
            }
        }
        let mut left = 0;
        let mut right = positions.len();
        while left < right {
            self.charge(2)?;
            let middle = left + (right - left) / 2;
            match positions[middle].0.cmp(&block) {
                std::cmp::Ordering::Less => left = middle + 1,
                std::cmp::Ordering::Greater => right = middle,
                std::cmp::Ordering::Equal => return Ok(Some(positions[middle].1)),
            }
        }
        Ok(None)
    }
}

pub(crate) struct MeteredIndexedControlFlowV1 {
    flow: IndexedControlFlow,
    retained_storage: usize,
}

impl MeteredIndexedControlFlowV1 {
    /// Borrows the existing CFG. Callers charge every lookup, edge and state visit.
    pub(crate) fn indexed_v15(&self) -> &IndexedControlFlow {
        &self.flow
    }

    pub(crate) fn dominates(
        &self,
        definition: BlockId,
        use_block: BlockId,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
        let mut resources = ControlFlowResourcesV1 {
            budget: Some(budget),
        };
        let result = (|| {
            let definition = resources.block_position(&self.flow.block_positions, definition)?;
            let use_block = resources.block_position(&self.flow.block_positions, use_block)?;
            let (Some(definition), Some(use_block)) = (definition, use_block) else {
                return Ok(false);
            };
            resources.charge(4)?;
            Ok::<_, MeteredControlFlowErrorV1>(self.flow.dominates_positions(definition, use_block))
        })();
        result.map_err(|error| match error {
            MeteredControlFlowErrorV1::Resource(error) => error,
            MeteredControlFlowErrorV1::ControlFlow(_) => {
                CanonicalKernelIrVerificationResourceErrorV1::Accounting
            }
        })
    }

    pub(crate) fn release(
        self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let Self {
            flow,
            retained_storage,
        } = self;
        drop(flow);
        budget.release_storage(retained_storage)
    }
}

pub(crate) fn analyze_control_flow_with_verification_budget_v1(
    function: &Function,
    limits: ControlFlowLimits,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<MeteredIndexedControlFlowV1, MeteredControlFlowErrorV1> {
    let floor = budget.storage_checkpoint();
    let result = analyze_control_flow_shared_v1(
        function,
        limits,
        &mut ControlFlowResourcesV1 {
            budget: Some(budget),
        },
    );
    match result {
        Ok(flow) => {
            let retained_storage = budget
                .storage()
                .checked_sub(floor)
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
            Ok(MeteredIndexedControlFlowV1 {
                flow,
                retained_storage,
            })
        }
        Err(error) => {
            budget.rollback_storage(floor)?;
            Err(error)
        }
    }
}
