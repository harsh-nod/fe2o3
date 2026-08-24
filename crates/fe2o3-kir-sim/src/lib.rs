#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod execute;
mod model;
mod preflight;
mod resident;

pub use execute::{
    NoopSimulationEventSinkV1, SimulationConflictAssessmentV1, SimulationErrorV1,
    SimulationEventKindV1, SimulationEventSinkControlV1, SimulationEventSinkErrorV1,
    SimulationEventSinkV1, SimulationEventSiteV1, SimulationEventV1,
    SimulationExecutionErrorKindV1, SimulationExecutionErrorV1, SimulationExecutionOutcomeV1,
    SimulationExecutionV1, SimulationMemoryConflictV1, SimulationObservationFailureV1,
    SimulationScheduleIdentityV1,
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
