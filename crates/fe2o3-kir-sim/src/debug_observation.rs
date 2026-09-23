//! One atomic legacy-record observation callback. Retention belongs to the sink.
use crate::{
    SimulationAllocationWatermarkV1, SimulationDebugCheckpointFramesV1,
    SimulationDebugOriginContextV1,
};

#[derive(Clone, Copy, Debug)]
pub struct SimulationDebugObservationContextV1<'a> {
    operation_origin: SimulationDebugOriginContextV1,
    checkpoint_frames: SimulationDebugCheckpointFramesV1<'a>,
    allocation_lifecycle: SimulationAllocationWatermarkV1,
}
impl<'a> SimulationDebugObservationContextV1<'a> {
    pub(crate) fn new(
        operation_origin: SimulationDebugOriginContextV1,
        checkpoint_frames: SimulationDebugCheckpointFramesV1<'a>,
        allocation_lifecycle: SimulationAllocationWatermarkV1,
    ) -> Self {
        Self {
            operation_origin,
            checkpoint_frames,
            allocation_lifecycle,
        }
    }
    pub const fn operation_origin(self) -> SimulationDebugOriginContextV1 {
        self.operation_origin
    }
    pub const fn checkpoint_frames(self) -> SimulationDebugCheckpointFramesV1<'a> {
        self.checkpoint_frames
    }
    pub const fn allocation_lifecycle(self) -> SimulationAllocationWatermarkV1 {
        self.allocation_lifecycle
    }
}
