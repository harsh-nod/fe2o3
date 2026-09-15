//! Exclusive temporary-storage scope. A success transfers only its checked
//! retained reservation; failure cannot refund the separate work counter.

use super::*;

const HEADER_CELLS: usize =
    std::mem::size_of::<WorkingScopeV1<'_>>().div_ceil(std::mem::size_of::<usize>());

pub(super) struct WorkingScopeV1<'a> {
    parent: &'a mut WorkingCells,
    cells: WorkingCells,
    inherited: usize,
    retained: usize,
}

impl<'a> WorkingScopeV1<'a> {
    pub(super) fn new(
        parent: &'a mut WorkingCells,
        work: &mut usize,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        project_loop_graph_charge_v1(work, HEADER_CELLS)?;
        let inherited = parent.live;
        let mut cells = WorkingCells::new(parent.limit);
        cells.live = inherited;
        #[cfg(test)]
        {
            cells.peak = parent.peak;
        }
        cells.reserve(HEADER_CELLS)?;
        Ok(Self {
            parent,
            cells,
            inherited,
            retained: 0,
        })
    }

    pub(super) fn cells(&mut self) -> &mut WorkingCells {
        &mut self.cells
    }

    pub(super) fn finish<T>(
        mut self,
        value: T,
        retained: usize,
    ) -> Result<T, ProductionRankedProjectionErrorV1> {
        let expected = self
            .inherited
            .checked_add(HEADER_CELLS)
            .and_then(|n| n.checked_add(retained));
        if expected != Some(self.cells.live) {
            // Failed output ownership dies before its reservation is released.
            drop(value);
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "helper range scope still owns temporary storage",
            ));
        }
        self.retained = retained;
        Ok(value)
    }
}

impl Drop for WorkingScopeV1<'_> {
    fn drop(&mut self) {
        self.parent.live = self
            .inherited
            .checked_add(self.retained)
            .expect("checked helper-range retained reservation");
        #[cfg(test)]
        {
            self.parent.peak = self.parent.peak.max(self.cells.peak);
        }
    }
}

#[cfg(test)]
#[path = "working_scope_v1/tests.rs"]
mod tests;
