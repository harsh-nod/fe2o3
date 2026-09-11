//! Original-parent operations; construction order stays in the shared driver.

use super::*;

pub(in crate::queue::live) type PreparationResultV1 = Result<
    (
        Result<(), ComputeAqlQueueSessionErrorV1>,
        Result<(), ComputeAqlQueueSessionErrorV1>,
    ),
    ComputeAqlQueueSessionErrorV1,
>;

pub(in crate::queue::live) trait AuxiliaryParentV1 {
    type Environment: PrimaryEnvironmentV1;
    type TerminalParent;

    fn check_currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn with_preparation_custody(
        &mut self,
        work: impl FnOnce(
            &mut <Self::Environment as PrimaryEnvironmentV1>::Memory,
        ) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    ) -> PreparationResultV1;
    fn target(&mut self) -> AuxiliaryQueueTargetV1<'_, Self::Environment>;
    // A nonallocating, nonfallible move; the caller stores it before cleanup.
    fn take_terminal_parent(&mut self) -> Self::TerminalParent;
}

impl AuxiliaryParentV1 for &mut ComputeAqlQueueSessionV1 {
    type Environment = Platform;
    type TerminalParent = ComputeAqlQueueSessionV1;

    fn check_currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        ComputeAqlQueueSessionV1::check_currentness(self)
    }

    fn with_preparation_custody(
        &mut self,
        work: impl FnOnce(&mut SharedGttMemorySessionV1) -> Result<(), ComputeAqlQueueSessionErrorV1>,
    ) -> PreparationResultV1 {
        self.with_live_queue_memory_model_custody(work)
    }

    fn target(&mut self) -> AuxiliaryQueueTargetV1<'_, Platform> {
        AuxiliaryQueueTargetV1 {
            engine: self.engine.as_mut().expect("checked queue engine"),
            primary: &self.observation,
            lanes: &mut self.auxiliary_compute_lanes,
            sdma: self.sdma.as_ref(),
            striped_sdma: self.striped_sdma.as_ref(),
        }
    }

    fn take_terminal_parent(&mut self) -> ComputeAqlQueueSessionV1 {
        self.take_for_terminal_auxiliary_construction_v1()
    }
}
