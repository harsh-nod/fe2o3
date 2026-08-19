#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod coverage;
mod fixture;

pub use coverage::{
    CONFORMANCE_CASE_NAME_MAX_BYTES_V1, ConformanceCaseV1, ConformanceExpectationV1,
    ConformanceSemanticV1, CoverageGapV1, CoverageLookupErrorV1, ExpectedRejectionV1,
    GFX942_CONFORMANCE_CORPUS_V1, MAX_CONFORMANCE_CASES_V1, conformance_case_v1,
};
pub use fixture::{
    FixtureCollectionOrderV1, FixtureDeviceLibrarySetV1, GFX942_FIXTURE_ADDRESS_SPACES_V1,
    GFX942_FIXTURE_ALIGNMENTS_V1, GFX942_FIXTURE_DEVICE_LIBRARIES_V1,
    GFX942_FIXTURE_OBLIGATIONS_V1, GFX942_FIXTURE_ORIGINS_V1, Gfx942FixtureBuilderV1,
    gfx942_fixture_v1,
};
