#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod debug;
mod execute;
mod explore;
mod model;
mod preflight;
mod resident;
mod schedule;
mod soft_float;

pub use debug::{
    MAX_DEBUG_ALLOCATIONS_PER_CHECKPOINT_V1, MAX_DEBUG_FRAMES_PER_CHECKPOINT_V1,
    MAX_DEBUG_MEMORY_BYTES_PER_CHECKPOINT_V1, MAX_DEBUG_VALUES_PER_CHECKPOINT_V1,
    NoopSimulationDebugSinkV1, SimulationDebugAllocationV1, SimulationDebugBarrierActionV1,
    SimulationDebugBindingV1, SimulationDebugCaptureLimitFieldV1,
    SimulationDebugCaptureLimitsErrorV1, SimulationDebugCaptureLimitsV1,
    SimulationDebugCheckpointPhaseV1, SimulationDebugCollectionV1, SimulationDebugFrameV1,
    SimulationDebugMemoryAccessV1, SimulationDebugRecordKindV1, SimulationDebugRecordV1,
    SimulationDebugScheduleV1, SimulationDebugSinkControlV1, SimulationDebugSinkV1,
    SimulationDebugSiteV1, SimulationDebugUnavailableReasonV1, SimulationDebugValueV1,
};
pub use execute::{
    DivergentWaveV1, DivergentWorkgroupBarrierV1, DivergentWorkgroupBarrierV2, IncompleteWaveV1,
    MismatchedWaveV1, MismatchedWorkgroupBarrierV1, NoopSimulationEventSinkV1, SimulationAbiViewV1,
    SimulationConflictAssessmentV1, SimulationDataRaceV1, SimulationErrorV1, SimulationEventKindV1,
    SimulationEventSinkControlV1, SimulationEventSinkErrorV1, SimulationEventSinkV1,
    SimulationEventSiteV1, SimulationEventV1, SimulationExecutionErrorKindV1,
    SimulationExecutionErrorV1, SimulationExecutionOutcomeV1, SimulationExecutionV1,
    SimulationHappensBeforeReasonV1, SimulationMemoryConflictV1, SimulationObservationFailureV1,
    SimulationOrderedMemoryConflictV1, SimulationOutOfBoundsV2, SimulationRaceAssessmentV1,
    WorkgroupBarrierMismatchV1, WorkgroupParticipantV1,
};
pub use explore::{
    MAX_EXPLORATION_RETAINED_DECISIONS_V1, MAX_EXPLORATION_SCHEDULES_V1,
    SimulationExplorationFailureV1, SimulationExplorationRequestErrorV1,
    SimulationExplorationRequestV1, SimulationExplorationV1, SimulationExplorationWitnessV1,
};
pub use model::{
    AdmittedSimulationModuleV1, BufferArgumentErrorV1, BufferArgumentV1, BufferBackingIdV1,
    BufferViewArgumentV1, EventPolicyV1, GridShapeV1, IndexWidthV1, ScalarBitsErrorV1,
    ScalarBitsV1, SharedBufferV1, SimulationAdmissionErrorV1, SimulationArgumentV1,
    SimulationInvocationV1, SimulationLimitsErrorV1, SimulationLimitsV1, SimulationRequestV1,
    SimulationSiteV1, SimulationTargetV1, WorkgroupShapeV1,
};
pub use preflight::{
    MAX_REPORTED_UNSUPPORTED_FINDINGS_V1, MAX_REPORTED_UNSUPPORTED_IDENTIFIER_BYTES_V1,
    SimulationPlanV1, SimulationPreflightErrorV1, UnsupportedFeatureV1,
    UnsupportedSimulationReportV1, UnsupportedSimulationSiteV1,
};
pub use schedule::{
    MAX_PERSISTED_SCHEDULE_BYTES_V1, MAX_SCHEDULE_DECISIONS_V1,
    PersistedSimulationScheduleArtifactV1, PersistedSimulationScheduleBindingV1,
    PersistedSimulationScheduleCodecErrorV1, PersistedSimulationScheduleDocumentV1,
    SimulationScheduleCoverageV1, SimulationScheduleDecisionV1, SimulationScheduleIdentityV1,
    SimulationScheduleRecordV1, SimulationScheduleReplayErrorV1, SimulationScheduleRequestV1,
};
