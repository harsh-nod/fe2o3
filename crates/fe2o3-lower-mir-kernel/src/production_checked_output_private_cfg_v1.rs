use super::*;
use fe2o3_kernel_analysis::CanonicalKirPrivateDataflowQueueV1;

// The source CFG uses the same opaque paid worklist as the physical checker.
pub(super) struct Queue(CanonicalKirPrivateDataflowQueueV1);
impl Queue {
    pub(super) fn new(blocks: usize, budget: &mut AssertOriginBudgetV1<'_>) -> R<Self> {
        CanonicalKirPrivateDataflowQueueV1::new_retaining_scratch_v1(blocks, budget)
            .map(Self)
            .map_err(physical_error)
    }
    pub(super) fn reset(&mut self, budget: &mut AssertOriginBudgetV1<'_>) -> R<()> {
        self.0.reset(budget).map_err(physical_error)
    }
    pub(super) fn push(&mut self, block: usize, budget: &mut AssertOriginBudgetV1<'_>) -> R<()> {
        self.0.push(block, budget).map_err(physical_error)
    }
    pub(super) fn pop(&mut self, budget: &mut AssertOriginBudgetV1<'_>) -> R<Option<usize>> {
        self.0.pop(budget).map_err(physical_error)
    }
}

#[cfg(test)]
#[path = "production_checked_output_private_cfg_resource_v1_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "production_checked_output_private_cfg_v1_tests.rs"]
mod tests;
