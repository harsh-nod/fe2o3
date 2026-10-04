//! Checked independent observation budgets; legacy record/value/memory limits are unchanged.
use super::*;
use fe2o3_kir_sim::SimulationAllocationReuseV1;

pub const MAX_RUNTIME_ALLOCATION_TRANSITIONS_V1: usize = 65_536;
pub const MAX_RUNTIME_ALLOCATION_METADATA_BYTES_V1: usize = 16 * 1024 * 1024;
pub const MAX_RUNTIME_ALLOCATION_VALIDATION_WORK_V1: usize = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeObservationConfigErrorV1 {
    OriginLimits,
    FrameLimits,
    AllocationLimits,
    FramesRequireOrigins,
    CaptureInstanceExhausted,
}
impl std::fmt::Display for RuntimeObservationConfigErrorV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "runtime observation configuration: {self:?}")
    }
}
impl std::error::Error for RuntimeObservationConfigErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeOriginCaptureLimitsV1 {
    pub(super) inner: Limits,
}
impl RuntimeOriginCaptureLimitsV1 {
    pub fn new(
        max_rows: usize,
        max_metadata_bytes: usize,
    ) -> Result<Self, RuntimeObservationConfigErrorV1> {
        Limits::new(max_rows, max_metadata_bytes)
            .map(|inner| Self { inner })
            .map_err(|_| RuntimeObservationConfigErrorV1::OriginLimits)
    }
    pub const fn max_rows(self) -> usize {
        self.inner.rows
    }
    pub const fn max_metadata_bytes(self) -> usize {
        self.inner.bytes
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimeOriginCaptureModeV1 {
    #[default]
    Disabled,
    Enabled(RuntimeOriginCaptureLimitsV1),
}
impl RuntimeOriginCaptureModeV1 {
    pub(super) fn limits(self) -> Option<Limits> {
        match self {
            Self::Disabled => None,
            Self::Enabled(value) => Some(value.inner),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeFrameCaptureLimitsV1 {
    pub(super) records: usize,
    pub(super) rows: usize,
    pub(super) bytes: usize,
}
impl RuntimeFrameCaptureLimitsV1 {
    pub fn new(
        max_records: usize,
        max_frame_rows: usize,
        max_metadata_bytes: usize,
    ) -> Result<Self, RuntimeObservationConfigErrorV1> {
        let minimum = frames::minimum_metadata_bytes();
        if !(1..=MAX_ROWS).contains(&max_records)
            || !(1..=MAX_ROWS).contains(&max_frame_rows)
            || !(minimum..=MAX_METADATA).contains(&max_metadata_bytes)
        {
            return Err(RuntimeObservationConfigErrorV1::FrameLimits);
        }
        Ok(Self {
            records: max_records,
            rows: max_frame_rows,
            bytes: max_metadata_bytes,
        })
    }
    pub const fn max_records(self) -> usize {
        self.records
    }
    pub const fn max_frame_rows(self) -> usize {
        self.rows
    }
    pub const fn max_metadata_bytes(self) -> usize {
        self.bytes
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimeFrameCaptureModeV1 {
    #[default]
    Disabled,
    Enabled(RuntimeFrameCaptureLimitsV1),
}
impl RuntimeFrameCaptureModeV1 {
    pub(super) fn limits(self) -> Option<RuntimeFrameCaptureLimitsV1> {
        match self {
            Self::Disabled => None,
            Self::Enabled(value) => Some(value),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAllocationCaptureLimitsV1 {
    pub(super) records: usize,
    pub(super) transitions: usize,
    pub(super) bytes: usize,
    pub(super) validation_work: usize,
}
impl RuntimeAllocationCaptureLimitsV1 {
    pub fn new(
        max_records: usize,
        max_transitions: usize,
        max_metadata_bytes: usize,
    ) -> Result<Self, RuntimeObservationConfigErrorV1> {
        Self::new_with_validation_work(
            max_records,
            max_transitions,
            max_metadata_bytes,
            MAX_RUNTIME_ALLOCATION_VALIDATION_WORK_V1,
        )
    }
    pub fn new_with_validation_work(
        max_records: usize,
        max_transitions: usize,
        max_metadata_bytes: usize,
        max_validation_work: usize,
    ) -> Result<Self, RuntimeObservationConfigErrorV1> {
        let minimum = allocations::minimum_metadata_bytes();
        if !(1..=MAX_ROWS).contains(&max_records)
            || !(1..=MAX_RUNTIME_ALLOCATION_TRANSITIONS_V1).contains(&max_transitions)
            || !(minimum..=MAX_RUNTIME_ALLOCATION_METADATA_BYTES_V1).contains(&max_metadata_bytes)
            || max_validation_work > MAX_RUNTIME_ALLOCATION_VALIDATION_WORK_V1
        {
            return Err(RuntimeObservationConfigErrorV1::AllocationLimits);
        }
        Ok(Self {
            records: max_records,
            transitions: max_transitions,
            bytes: max_metadata_bytes,
            validation_work: max_validation_work,
        })
    }
    pub const fn max_records(self) -> usize {
        self.records
    }
    pub const fn max_transitions(self) -> usize {
        self.transitions
    }
    pub const fn max_validation_work(self) -> usize {
        self.validation_work
    }
    pub const fn max_metadata_bytes(self) -> usize {
        self.bytes
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RuntimeAllocationCaptureModeV1 {
    #[default]
    Disabled,
    Enabled(RuntimeAllocationCaptureLimitsV1),
}
impl RuntimeAllocationCaptureModeV1 {
    pub(super) fn limits(self) -> Option<RuntimeAllocationCaptureLimitsV1> {
        match self {
            Self::Disabled => None,
            Self::Enabled(value) => Some(value),
        }
    }
}

/// An explicit new capture profile. None/Disabled never enables allocator reuse.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeObservationOptionsV1 {
    pub(super) origins: RuntimeOriginCaptureModeV1,
    pub(super) frames: RuntimeFrameCaptureModeV1,
    pub(super) allocations: RuntimeAllocationCaptureModeV1,
    pub(super) allocation_reuse: Option<SimulationAllocationReuseV1>,
}
impl RuntimeObservationOptionsV1 {
    pub fn new(
        origins: RuntimeOriginCaptureModeV1,
        frames: RuntimeFrameCaptureModeV1,
        allocations: RuntimeAllocationCaptureModeV1,
        allocation_reuse: Option<SimulationAllocationReuseV1>,
    ) -> Result<Self, RuntimeObservationConfigErrorV1> {
        if matches!(frames, RuntimeFrameCaptureModeV1::Enabled(_))
            && matches!(origins, RuntimeOriginCaptureModeV1::Disabled)
        {
            return Err(RuntimeObservationConfigErrorV1::FramesRequireOrigins);
        }
        Ok(Self {
            origins,
            frames,
            allocations,
            allocation_reuse,
        })
    }
    pub const fn disabled() -> Self {
        Self {
            origins: RuntimeOriginCaptureModeV1::Disabled,
            frames: RuntimeFrameCaptureModeV1::Disabled,
            allocations: RuntimeAllocationCaptureModeV1::Disabled,
            allocation_reuse: None,
        }
    }
    pub const fn origins(self) -> RuntimeOriginCaptureModeV1 {
        self.origins
    }
    pub const fn frames(self) -> RuntimeFrameCaptureModeV1 {
        self.frames
    }
    pub const fn allocations(self) -> RuntimeAllocationCaptureModeV1 {
        self.allocations
    }
    pub const fn allocation_reuse(self) -> Option<SimulationAllocationReuseV1> {
        self.allocation_reuse
    }
}
impl Default for RuntimeObservationOptionsV1 {
    fn default() -> Self {
        Self::disabled()
    }
}
