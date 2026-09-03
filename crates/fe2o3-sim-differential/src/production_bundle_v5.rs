use core::fmt;
use std::collections::BTreeSet;
use std::error::Error;

use fe2o3_kernel_ir::ScalarType;
use fe2o3_kir_sim_cli::AdmittedSimulationBundleInputV5;
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const PRODUCTION_SEMANTIC_CONFORMANCE_SCHEMA_V3: &str =
    "fe2o3-production-semantic-conformance-v3";
pub const PRODUCTION_SEMANTIC_CAPABILITIES_SCHEMA_V3: &str =
    "fe2o3-production-semantic-capabilities-v3";
pub const MAX_PRODUCTION_CONFORMANCE_OUTPUTS_V3: usize = 32;
pub const MAX_PRODUCTION_CONFORMANCE_EXPECTED_BYTES_V3: usize = 1024 * 1024;
pub const MAX_PRODUCTION_CONFORMANCE_CASE_ID_BYTES_V3: usize = 96;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionSemanticDispositionV3 {
    ExactConformance,
    ProducerUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionSemanticUnavailableReasonV3 {
    OrdinaryFrontendIntrinsicRejected,
    OrdinaryFrontendTypeNotRetained,
    BundleAggregateInputNotAdmitted,
    OrdinaryFrontendProjectionIncomplete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ProductionSemanticCapabilityV3 {
    pub family: &'static str,
    pub disposition: ProductionSemanticDispositionV3,
    pub reason: Option<ProductionSemanticUnavailableReasonV3>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProductionSemanticCapabilitiesV3 {
    pub schema: &'static str,
    pub authority: &'static str,
    pub simulated: bool,
    pub hardware_observed: bool,
    pub performance_prediction: bool,
    pub bundle_version: u16,
    pub kir_version: u16,
    pub cases: Vec<ProductionSemanticCapabilityV3>,
}

pub fn production_semantic_capabilities_v3() -> ProductionSemanticCapabilitiesV3 {
    ProductionSemanticCapabilitiesV3 {
        schema: PRODUCTION_SEMANTIC_CAPABILITIES_SCHEMA_V3,
        authority: "exact-admitted-bundle-v5-cpu-simulation-only",
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
        bundle_version: 5,
        kir_version: 10,
        cases: vec![
            exact("integer_i8_i16_i32_i64_u8_u16_u32_u64_signedness"),
            exact("f32_f64_corner_tables"),
            exact("scalar_and_buffer_layout"),
            exact("bounds_checked_access"),
            exact("integer_switch"),
            unavailable(
                "atomic_rmw_u32",
                ProductionSemanticUnavailableReasonV3::OrdinaryFrontendProjectionIncomplete,
            ),
            unavailable(
                "pointer_distance",
                ProductionSemanticUnavailableReasonV3::OrdinaryFrontendIntrinsicRejected,
            ),
            unavailable(
                "volatile_memory",
                ProductionSemanticUnavailableReasonV3::OrdinaryFrontendIntrinsicRejected,
            ),
            unavailable(
                "copy_nonoverlapping",
                ProductionSemanticUnavailableReasonV3::OrdinaryFrontendIntrinsicRejected,
            ),
            unavailable(
                "integer_i128_u128_ordinary_source",
                ProductionSemanticUnavailableReasonV3::OrdinaryFrontendTypeNotRetained,
            ),
            unavailable(
                "f16_bf16_ordinary_source",
                ProductionSemanticUnavailableReasonV3::OrdinaryFrontendTypeNotRetained,
            ),
            unavailable(
                "recursive_aggregate_bundle_input",
                ProductionSemanticUnavailableReasonV3::BundleAggregateInputNotAdmitted,
            ),
        ],
    }
}

const fn exact(family: &'static str) -> ProductionSemanticCapabilityV3 {
    ProductionSemanticCapabilityV3 {
        family,
        disposition: ProductionSemanticDispositionV3::ExactConformance,
        reason: None,
    }
}

const fn unavailable(
    family: &'static str,
    reason: ProductionSemanticUnavailableReasonV3,
) -> ProductionSemanticCapabilityV3 {
    ProductionSemanticCapabilityV3 {
        family,
        disposition: ProductionSemanticDispositionV3::ProducerUnavailable,
        reason: Some(reason),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactBufferExpectationV3<'a> {
    pub argument_ordinal: usize,
    pub element: ScalarType,
    pub bytes: &'a [u8],
    pub initialized: &'a [bool],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticCaseV3<'a> {
    pub case_id: &'a str,
    pub outputs: &'a [ExactBufferExpectationV3<'a>],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProductionSemanticConformanceV3 {
    pub schema: &'static str,
    pub status: &'static str,
    pub authority: &'static str,
    pub simulated: bool,
    pub hardware_observed: bool,
    pub performance_prediction: bool,
    pub bundle_version: u16,
    pub kir_version: u16,
    pub case_id: String,
    pub bundle_sha256: String,
    pub bundle_subject_sha256: String,
    pub canonical_kir_sha256: String,
    pub canonical_kir_bytes: u64,
    pub request_sha256: String,
    pub request_bytes: u64,
    pub expected_bytes: usize,
    pub outputs: Vec<ExactBufferObservationV3>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExactBufferObservationV3 {
    pub argument_ordinal: usize,
    pub scalar_type: String,
    pub expected_sha256: String,
    pub observed_sha256: Option<String>,
    pub expected_bytes: usize,
    pub observed_bytes: Option<usize>,
    pub exact_bytes: bool,
    pub exact_initialization: bool,
    pub unavailable: Option<ExactBufferUnavailableV3>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExactBufferUnavailableV3 {
    MissingArgument,
    ArgumentIsNotBuffer,
    ScalarTypeMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionSemanticConformanceErrorV3 {
    EmptyCaseId,
    CaseIdTooLong,
    InvalidCaseId,
    EmptyOutputs,
    TooManyOutputs,
    DuplicateOutputOrdinal(usize),
    InitializationLengthMismatch { argument_ordinal: usize },
    ExpectedBytesTooLarge,
    BundleRevalidation(String),
    AuthorityEscalation,
    MissingBundleEvidence,
    BundleIdentityMismatch,
    BundleSubjectMismatch,
    SourceLineageMismatch,
    CanonicalKirIdentityMismatch,
    Simulation(String),
}

impl fmt::Display for ProductionSemanticConformanceErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCaseId => formatter.write_str("production conformance case id is empty"),
            Self::CaseIdTooLong => {
                formatter.write_str("production conformance case id is too long")
            }
            Self::InvalidCaseId => formatter.write_str("production conformance case id is invalid"),
            Self::EmptyOutputs => formatter.write_str("production conformance has no outputs"),
            Self::TooManyOutputs => {
                formatter.write_str("production conformance has too many outputs")
            }
            Self::DuplicateOutputOrdinal(ordinal) => {
                write!(formatter, "duplicate output argument ordinal {ordinal}")
            }
            Self::InitializationLengthMismatch { argument_ordinal } => write!(
                formatter,
                "output argument {argument_ordinal} byte and initialization lengths differ"
            ),
            Self::ExpectedBytesTooLarge => {
                formatter.write_str("production conformance expected bytes exceed the bound")
            }
            Self::BundleRevalidation(message) => {
                write!(formatter, "Bundle V5 revalidation failed: {message}")
            }
            Self::AuthorityEscalation => {
                formatter.write_str("Bundle V5 unexpectedly grants execution authority")
            }
            Self::MissingBundleEvidence => {
                formatter.write_str("admitted input is missing Bundle V5 evidence")
            }
            Self::BundleIdentityMismatch => {
                formatter.write_str("admitted Bundle V5 content identity mismatch")
            }
            Self::BundleSubjectMismatch => {
                formatter.write_str("admitted Bundle V5 subject identity mismatch")
            }
            Self::SourceLineageMismatch => {
                formatter.write_str("admitted Bundle V5 source-lineage evidence mismatch")
            }
            Self::CanonicalKirIdentityMismatch => {
                formatter.write_str("admitted canonical KIR V10 identity mismatch")
            }
            Self::Simulation(message) => write!(formatter, "simulation failed: {message}"),
        }
    }
}

impl Error for ProductionSemanticConformanceErrorV3 {}

pub fn run_production_semantic_conformance_v3(
    admitted: &AdmittedSimulationBundleInputV5,
    case: ProductionSemanticCaseV3<'_>,
) -> Result<ProductionSemanticConformanceV3, ProductionSemanticConformanceErrorV3> {
    let expected_bytes = validate_case(case)?;
    admitted.bundle().revalidate().map_err(|error| {
        ProductionSemanticConformanceErrorV3::BundleRevalidation(error.to_string())
    })?;
    if admitted.grants_proof_authority()
        || admitted.grants_artifact_authority()
        || admitted.grants_compiler_authority()
        || admitted.authenticates_compiler_execution()
        || admitted.grants_hardware_authority()
        || admitted.grants_load_authority()
        || admitted.grants_launch_authority()
    {
        return Err(ProductionSemanticConformanceErrorV3::AuthorityEscalation);
    }

    let bundle = admitted.bundle();
    let input = admitted.input();
    let evidence = input
        .simulation_bundle_evidence()
        .ok_or(ProductionSemanticConformanceErrorV3::MissingBundleEvidence)?;
    let bundle_identity = *bundle.identity().as_bytes();
    if input.simulation_bundle_identity() != Some(bundle_identity)
        || evidence.envelope_version != 5
        || evidence.envelope_identity != bundle_identity
    {
        return Err(ProductionSemanticConformanceErrorV3::BundleIdentityMismatch);
    }
    if input.simulation_bundle_subject() != Some(*bundle.subject_identity())
        || evidence.subject_identity != *bundle.subject_identity()
        || evidence.kernel_abi_identity != *bundle.kernel_abi_identity()
    {
        return Err(ProductionSemanticConformanceErrorV3::BundleSubjectMismatch);
    }
    let lineage = bundle.source_lineage();
    if evidence.identity_inventory_receipt_sha256
        != lineage.rustc_identity_inventory_receipt_sha256()
        || evidence.identity_inventory_receipt_bytes
            != lineage.rustc_identity_inventory_receipt_bytes()
        || evidence.preflight_plan_receipt_sha256 != lineage.rustc_preflight_plan_receipt_sha256()
        || evidence.preflight_plan_receipt_bytes != lineage.rustc_preflight_plan_receipt_bytes()
    {
        return Err(ProductionSemanticConformanceErrorV3::SourceLineageMismatch);
    }
    let module_identity = input.module.identity();
    if module_identity.wire_version() != 10
        || module_identity.digest() != bundle.canonical_kir_v10_digest()
        || module_identity.canonical_length() != bundle.canonical_kir_v10_length()
        || input.kir_sha256 != *bundle.canonical_kir_v10_digest()
        || evidence.production_kir_version != bundle.production_kir_identity().version()
        || evidence.production_kir_sha256 != bundle.production_kir_identity().digest()
        || evidence.production_kir_bytes != bundle.production_kir_identity().canonical_length()
    {
        return Err(ProductionSemanticConformanceErrorV3::CanonicalKirIdentityMismatch);
    }

    let execution = input
        .module
        .simulate(
            &input.request,
            input.simulation_target(),
            input.simulation_limits,
        )
        .map_err(|error| ProductionSemanticConformanceErrorV3::Simulation(error.to_string()))?;
    if execution.identity() != module_identity {
        return Err(ProductionSemanticConformanceErrorV3::CanonicalKirIdentityMismatch);
    }

    let outputs = case
        .outputs
        .iter()
        .map(|expected| observe_buffer(&execution, expected))
        .collect::<Vec<_>>();
    let exact = outputs.iter().all(|output| {
        output.unavailable.is_none() && output.exact_bytes && output.exact_initialization
    });
    Ok(ProductionSemanticConformanceV3 {
        schema: PRODUCTION_SEMANTIC_CONFORMANCE_SCHEMA_V3,
        status: if exact { "agreement" } else { "mismatch" },
        authority: "exact-admitted-bundle-v5-cpu-simulation-only",
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
        bundle_version: 5,
        kir_version: 10,
        case_id: case.case_id.to_owned(),
        bundle_sha256: hex(bundle_identity),
        bundle_subject_sha256: hex(*bundle.subject_identity()),
        canonical_kir_sha256: hex(*bundle.canonical_kir_v10_digest()),
        canonical_kir_bytes: bundle.canonical_kir_v10_length(),
        request_sha256: hex(input.request_sha256),
        request_bytes: input.request_bytes(),
        expected_bytes,
        outputs,
    })
}

fn validate_case(
    case: ProductionSemanticCaseV3<'_>,
) -> Result<usize, ProductionSemanticConformanceErrorV3> {
    if case.case_id.is_empty() {
        return Err(ProductionSemanticConformanceErrorV3::EmptyCaseId);
    }
    if case.case_id.len() > MAX_PRODUCTION_CONFORMANCE_CASE_ID_BYTES_V3 {
        return Err(ProductionSemanticConformanceErrorV3::CaseIdTooLong);
    }
    if !case.case_id.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
    }) {
        return Err(ProductionSemanticConformanceErrorV3::InvalidCaseId);
    }
    if case.outputs.is_empty() {
        return Err(ProductionSemanticConformanceErrorV3::EmptyOutputs);
    }
    if case.outputs.len() > MAX_PRODUCTION_CONFORMANCE_OUTPUTS_V3 {
        return Err(ProductionSemanticConformanceErrorV3::TooManyOutputs);
    }
    let mut ordinals = BTreeSet::new();
    let mut total = 0_usize;
    for output in case.outputs {
        if !ordinals.insert(output.argument_ordinal) {
            return Err(
                ProductionSemanticConformanceErrorV3::DuplicateOutputOrdinal(
                    output.argument_ordinal,
                ),
            );
        }
        if output.bytes.len() != output.initialized.len() {
            return Err(
                ProductionSemanticConformanceErrorV3::InitializationLengthMismatch {
                    argument_ordinal: output.argument_ordinal,
                },
            );
        }
        total = total
            .checked_add(output.bytes.len())
            .ok_or(ProductionSemanticConformanceErrorV3::ExpectedBytesTooLarge)?;
        if total > MAX_PRODUCTION_CONFORMANCE_EXPECTED_BYTES_V3 {
            return Err(ProductionSemanticConformanceErrorV3::ExpectedBytesTooLarge);
        }
    }
    Ok(total)
}

fn observe_buffer(
    execution: &fe2o3_kir_sim::SimulationExecutionV1,
    expected: &ExactBufferExpectationV3<'_>,
) -> ExactBufferObservationV3 {
    let expected_sha256 = hex(Sha256::digest(expected.bytes).into());
    let Some(argument) = execution.arguments().get(expected.argument_ordinal) else {
        return unavailable_observation(
            expected,
            expected_sha256,
            ExactBufferUnavailableV3::MissingArgument,
        );
    };
    let fe2o3_kir_sim::SimulationArgumentV1::Buffer(buffer) = argument else {
        return unavailable_observation(
            expected,
            expected_sha256,
            ExactBufferUnavailableV3::ArgumentIsNotBuffer,
        );
    };
    if buffer.element() != expected.element {
        return unavailable_observation(
            expected,
            expected_sha256,
            ExactBufferUnavailableV3::ScalarTypeMismatch,
        );
    }
    ExactBufferObservationV3 {
        argument_ordinal: expected.argument_ordinal,
        scalar_type: scalar_type_name(expected.element).to_owned(),
        expected_sha256,
        observed_sha256: Some(hex(Sha256::digest(buffer.bytes()).into())),
        expected_bytes: expected.bytes.len(),
        observed_bytes: Some(buffer.bytes().len()),
        exact_bytes: buffer.bytes() == expected.bytes,
        exact_initialization: buffer.initialized() == expected.initialized,
        unavailable: None,
    }
}

fn unavailable_observation(
    expected: &ExactBufferExpectationV3<'_>,
    expected_sha256: String,
    unavailable: ExactBufferUnavailableV3,
) -> ExactBufferObservationV3 {
    ExactBufferObservationV3 {
        argument_ordinal: expected.argument_ordinal,
        scalar_type: scalar_type_name(expected.element).to_owned(),
        expected_sha256,
        observed_sha256: None,
        expected_bytes: expected.bytes.len(),
        observed_bytes: None,
        exact_bytes: false,
        exact_initialization: false,
        unavailable: Some(unavailable),
    }
}

const fn scalar_type_name(element: ScalarType) -> &'static str {
    match element {
        ScalarType::Bool => "bool",
        ScalarType::I8 => "i8",
        ScalarType::I16 => "i16",
        ScalarType::I32 => "i32",
        ScalarType::I64 => "i64",
        ScalarType::I128 => "i128",
        ScalarType::U8 => "u8",
        ScalarType::U16 => "u16",
        ScalarType::U32 => "u32",
        ScalarType::U64 => "u64",
        ScalarType::U128 => "u128",
        ScalarType::Index => "index",
        ScalarType::F16 => "f16",
        ScalarType::Bf16 => "bf16",
        ScalarType::F32 => "f32",
        ScalarType::F64 => "f64",
    }
}

fn hex(bytes: [u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expectation(argument_ordinal: usize, bytes: &[u8]) -> ExactBufferExpectationV3<'_> {
        ExactBufferExpectationV3 {
            argument_ordinal,
            element: ScalarType::U8,
            bytes,
            initialized: &[true],
        }
    }

    #[test]
    fn capability_contract_keeps_unimplemented_producer_paths_typed_unavailable() {
        let capabilities = production_semantic_capabilities_v3();
        assert!(!capabilities.hardware_observed);
        assert!(!capabilities.performance_prediction);
        assert!(capabilities.cases.iter().any(|case| {
            case.family == "integer_switch"
                && case.disposition == ProductionSemanticDispositionV3::ExactConformance
                && case.reason.is_none()
        }));
        assert!(capabilities.cases.iter().any(|case| {
            case.family == "volatile_memory"
                && case.disposition == ProductionSemanticDispositionV3::ProducerUnavailable
                && case.reason
                    == Some(
                        ProductionSemanticUnavailableReasonV3::OrdinaryFrontendIntrinsicRejected,
                    )
        }));
    }

    #[test]
    fn case_validation_rejects_ambiguous_or_unbounded_expectations() {
        let one = [1_u8];
        let duplicate = [expectation(2, &one), expectation(2, &one)];
        assert_eq!(
            validate_case(ProductionSemanticCaseV3 {
                case_id: "duplicate",
                outputs: &duplicate,
            }),
            Err(ProductionSemanticConformanceErrorV3::DuplicateOutputOrdinal(2))
        );
        let oversized = vec![0_u8; MAX_PRODUCTION_CONFORMANCE_EXPECTED_BYTES_V3 + 1];
        let initialized = vec![true; oversized.len()];
        let outputs = [ExactBufferExpectationV3 {
            argument_ordinal: 0,
            element: ScalarType::U8,
            bytes: &oversized,
            initialized: &initialized,
        }];
        assert_eq!(
            validate_case(ProductionSemanticCaseV3 {
                case_id: "oversized",
                outputs: &outputs,
            }),
            Err(ProductionSemanticConformanceErrorV3::ExpectedBytesTooLarge)
        );
    }

    #[test]
    fn case_validation_rejects_hostile_ids_and_initialization_mismatch() {
        let bytes = [1_u8];
        let outputs = [expectation(0, &bytes)];
        assert_eq!(
            validate_case(ProductionSemanticCaseV3 {
                case_id: "cross/run",
                outputs: &outputs,
            }),
            Err(ProductionSemanticConformanceErrorV3::InvalidCaseId)
        );
        let no_initialization = [ExactBufferExpectationV3 {
            argument_ordinal: 0,
            element: ScalarType::U8,
            bytes: &bytes,
            initialized: &[],
        }];
        assert_eq!(
            validate_case(ProductionSemanticCaseV3 {
                case_id: "init-mismatch",
                outputs: &no_initialization,
            }),
            Err(
                ProductionSemanticConformanceErrorV3::InitializationLengthMismatch {
                    argument_ordinal: 0,
                }
            )
        );
    }
}
