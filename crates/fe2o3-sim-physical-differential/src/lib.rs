#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod physical_v1;

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
