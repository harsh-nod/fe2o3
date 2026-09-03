#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod application_v1;
mod physical_v1;
mod qualification_v2;

pub use application_v1::{
    PhysicalApplicationExecutionErrorV1, PhysicalApplicationPreparationErrorV1,
    PhysicalDifferentialSimulationInputsV1, PreparedWorkerV3PhysicalDifferentialV1,
    prepare_generated_worker_v3_physical_differential_v1,
};

pub use physical_v1::{
    MAX_PHYSICAL_DIFFERENTIAL_BYTES_V1, PHYSICAL_DIFFERENTIAL_CAPABILITIES_SCHEMA_V1,
    PHYSICAL_DIFFERENTIAL_SCHEMA_V1, PHYSICAL_DIFFERENTIAL_SIMULATOR_CONTRACT_V1,
    PhysicalDifferentialBufferV1, PhysicalDifferentialByteMismatchV1,
    PhysicalDifferentialCapabilitiesV1, PhysicalDifferentialDispositionV1,
    PhysicalDifferentialErrorV1, PhysicalDifferentialLimitsV1, PhysicalDifferentialReportV1,
    PhysicalDifferentialUnavailableV1, PreparedPhysicalDifferentialV1,
    physical_differential_capabilities_v1, physical_differential_production_readiness_v1,
    prepare_physical_differential_v1,
};
pub use qualification_v2::{
    PHYSICAL_DIFFERENTIAL_QUALIFICATION_SCHEMA_V2, PhysicalDifferentialAvailableBoundaryV2,
    PhysicalDifferentialPrerequisiteRecordV2, PhysicalDifferentialPrerequisiteStatusV2,
    PhysicalDifferentialPrerequisiteUnavailableV2, PhysicalDifferentialPrerequisiteV2,
    PhysicalDifferentialQualificationV2, physical_differential_qualification_v2,
};
