use core::fmt;
use std::collections::BTreeSet;
use std::error::Error;

use fe2o3_kernel_ir::ScalarType;
use fe2o3_kir_sim_cli::AdmittedSimulationBundleInputV6;
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const PRODUCTION_SEMANTIC_CONFORMANCE_SCHEMA_V4: &str =
    "fe2o3-production-semantic-conformance-v4";
pub const PRODUCTION_SEMANTIC_CAPABILITIES_SCHEMA_V4: &str =
    "fe2o3-production-semantic-capabilities-v4";
pub const MAX_PRODUCTION_CONFORMANCE_OUTPUTS_V4: usize = 32;
pub const MAX_PRODUCTION_CONFORMANCE_EXPECTED_BYTES_V4: usize = 1024 * 1024;
pub const MAX_PRODUCTION_CONFORMANCE_CASE_ID_BYTES_V4: usize = 96;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionSemanticDispositionV4 {
    ExactConformance,
    ProducerUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionSemanticUnavailableReasonV4 {
    OrdinaryFrontendIntrinsicRejected,
    OrdinaryFrontendTypeNotRetained,
    BundleAggregateInputNotAdmitted,
    OrdinaryFrontendProjectionIncomplete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ProductionSemanticCapabilityV4 {
    pub family: &'static str,
    pub disposition: ProductionSemanticDispositionV4,
    pub reason: Option<ProductionSemanticUnavailableReasonV4>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProductionSemanticCapabilitiesV4 {
    pub schema: &'static str,
    pub authority: &'static str,
    pub simulated: bool,
    pub hardware_observed: bool,
    pub performance_prediction: bool,
    pub bundle_version: u16,
    pub kir_version: u16,
    pub cases: Vec<ProductionSemanticCapabilityV4>,
}

pub fn production_semantic_capabilities_v4() -> ProductionSemanticCapabilitiesV4 {
    ProductionSemanticCapabilitiesV4 {
        schema: PRODUCTION_SEMANTIC_CAPABILITIES_SCHEMA_V4,
        authority: "exact-admitted-bundle-v6-cpu-simulation-only",
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
        bundle_version: 6,
        kir_version: 11,
        cases: vec![
            exact("integer_i8_i16_i32_i64_u8_u16_u32_u64_signedness"),
            exact("f32_f64_corner_tables"),
            exact("scalar_and_buffer_layout"),
            exact("bounds_checked_access"),
            exact("integer_switch"),
            unavailable(
                "atomic_rmw_u32",
                ProductionSemanticUnavailableReasonV4::OrdinaryFrontendProjectionIncomplete,
            ),
            unavailable(
                "pointer_distance",
                ProductionSemanticUnavailableReasonV4::OrdinaryFrontendIntrinsicRejected,
            ),
            unavailable(
                "volatile_memory",
                ProductionSemanticUnavailableReasonV4::OrdinaryFrontendIntrinsicRejected,
            ),
            unavailable(
                "copy_nonoverlapping",
                ProductionSemanticUnavailableReasonV4::OrdinaryFrontendIntrinsicRejected,
            ),
            unavailable(
                "integer_i128_u128_ordinary_source",
                ProductionSemanticUnavailableReasonV4::OrdinaryFrontendTypeNotRetained,
            ),
            unavailable(
                "f16_bf16_ordinary_source",
                ProductionSemanticUnavailableReasonV4::OrdinaryFrontendTypeNotRetained,
            ),
            unavailable(
                "recursive_aggregate_bundle_input",
                ProductionSemanticUnavailableReasonV4::BundleAggregateInputNotAdmitted,
            ),
        ],
    }
}

const fn exact(family: &'static str) -> ProductionSemanticCapabilityV4 {
    ProductionSemanticCapabilityV4 {
        family,
        disposition: ProductionSemanticDispositionV4::ExactConformance,
        reason: None,
    }
}

const fn unavailable(
    family: &'static str,
    reason: ProductionSemanticUnavailableReasonV4,
) -> ProductionSemanticCapabilityV4 {
    ProductionSemanticCapabilityV4 {
        family,
        disposition: ProductionSemanticDispositionV4::ProducerUnavailable,
        reason: Some(reason),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactBufferExpectationV4<'a> {
    pub argument_ordinal: usize,
    pub element: ScalarType,
    pub bytes: &'a [u8],
    pub initialized: &'a [bool],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticCaseV4<'a> {
    pub case_id: &'a str,
    pub outputs: &'a [ExactBufferExpectationV4<'a>],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProductionSemanticConformanceV4 {
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
    pub outputs: Vec<ExactBufferObservationV4>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExactBufferObservationV4 {
    pub argument_ordinal: usize,
    pub scalar_type: String,
    pub expected_sha256: String,
    pub observed_sha256: Option<String>,
    pub expected_bytes: usize,
    pub observed_bytes: Option<usize>,
    pub exact_bytes: bool,
    pub exact_initialization: bool,
    pub unavailable: Option<ExactBufferUnavailableV4>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExactBufferUnavailableV4 {
    MissingArgument,
    ArgumentIsNotBuffer,
    ScalarTypeMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionSemanticConformanceErrorV4 {
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

impl fmt::Display for ProductionSemanticConformanceErrorV4 {
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
                write!(formatter, "Bundle V6 revalidation failed: {message}")
            }
            Self::AuthorityEscalation => {
                formatter.write_str("Bundle V6 unexpectedly grants execution authority")
            }
            Self::MissingBundleEvidence => {
                formatter.write_str("admitted input is missing Bundle V6 evidence")
            }
            Self::BundleIdentityMismatch => {
                formatter.write_str("admitted Bundle V6 content identity mismatch")
            }
            Self::BundleSubjectMismatch => {
                formatter.write_str("admitted Bundle V6 subject identity mismatch")
            }
            Self::SourceLineageMismatch => {
                formatter.write_str("admitted Bundle V6 source-lineage evidence mismatch")
            }
            Self::CanonicalKirIdentityMismatch => {
                formatter.write_str("admitted canonical KIR V11 identity mismatch")
            }
            Self::Simulation(message) => write!(formatter, "simulation failed: {message}"),
        }
    }
}

impl Error for ProductionSemanticConformanceErrorV4 {}

pub fn run_production_semantic_conformance_v4(
    admitted: &AdmittedSimulationBundleInputV6,
    case: ProductionSemanticCaseV4<'_>,
) -> Result<ProductionSemanticConformanceV4, ProductionSemanticConformanceErrorV4> {
    let expected_bytes = validate_case(case)?;
    admitted.bundle().revalidate().map_err(|error| {
        ProductionSemanticConformanceErrorV4::BundleRevalidation(error.to_string())
    })?;
    if admitted.grants_proof_authority()
        || admitted.grants_artifact_authority()
        || admitted.grants_compiler_authority()
        || admitted.authenticates_compiler_execution()
        || admitted.grants_hardware_authority()
        || admitted.grants_load_authority()
        || admitted.grants_launch_authority()
    {
        return Err(ProductionSemanticConformanceErrorV4::AuthorityEscalation);
    }

    let bundle = admitted.bundle();
    let input = admitted.input();
    let evidence = input
        .simulation_bundle_evidence()
        .ok_or(ProductionSemanticConformanceErrorV4::MissingBundleEvidence)?;
    let bundle_identity = *bundle.identity().as_bytes();
    if input.simulation_bundle_identity() != Some(bundle_identity)
        || evidence.envelope_version != 6
        || evidence.envelope_identity != bundle_identity
    {
        return Err(ProductionSemanticConformanceErrorV4::BundleIdentityMismatch);
    }
    if input.simulation_bundle_subject() != Some(*bundle.subject_identity())
        || evidence.subject_identity != *bundle.subject_identity()
        || evidence.kernel_abi_identity != *bundle.kernel_abi_identity()
    {
        return Err(ProductionSemanticConformanceErrorV4::BundleSubjectMismatch);
    }
    let lineage = bundle.source_lineage();
    if evidence.identity_inventory_receipt_sha256
        != lineage.rustc_identity_inventory_receipt_sha256()
        || evidence.identity_inventory_receipt_bytes
            != lineage.rustc_identity_inventory_receipt_bytes()
        || evidence.preflight_plan_receipt_sha256 != lineage.rustc_preflight_plan_receipt_sha256()
        || evidence.preflight_plan_receipt_bytes != lineage.rustc_preflight_plan_receipt_bytes()
    {
        return Err(ProductionSemanticConformanceErrorV4::SourceLineageMismatch);
    }
    let module_identity = input.module.identity();
    if module_identity.wire_version() != 11
        || module_identity.digest() != bundle.canonical_kir_v11_digest()
        || module_identity.canonical_length() != bundle.canonical_kir_v11_length()
        || input.kir_sha256 != *bundle.canonical_kir_v11_digest()
        || evidence.production_kir_version != bundle.production_kir_identity().version()
        || evidence.production_kir_sha256 != bundle.production_kir_identity().digest()
        || evidence.production_kir_bytes != bundle.production_kir_identity().canonical_length()
    {
        return Err(ProductionSemanticConformanceErrorV4::CanonicalKirIdentityMismatch);
    }

    let execution = input
        .module
        .simulate(
            &input.request,
            input.simulation_target(),
            input.simulation_limits,
        )
        .map_err(|error| ProductionSemanticConformanceErrorV4::Simulation(error.to_string()))?;
    if execution.identity() != module_identity {
        return Err(ProductionSemanticConformanceErrorV4::CanonicalKirIdentityMismatch);
    }

    let outputs = case
        .outputs
        .iter()
        .map(|expected| observe_buffer(&execution, expected))
        .collect::<Vec<_>>();
    let exact = outputs.iter().all(|output| {
        output.unavailable.is_none() && output.exact_bytes && output.exact_initialization
    });
    Ok(ProductionSemanticConformanceV4 {
        schema: PRODUCTION_SEMANTIC_CONFORMANCE_SCHEMA_V4,
        status: if exact { "agreement" } else { "mismatch" },
        authority: "exact-admitted-bundle-v6-cpu-simulation-only",
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
        bundle_version: 6,
        kir_version: 11,
        case_id: case.case_id.to_owned(),
        bundle_sha256: hex(bundle_identity),
        bundle_subject_sha256: hex(*bundle.subject_identity()),
        canonical_kir_sha256: hex(*bundle.canonical_kir_v11_digest()),
        canonical_kir_bytes: bundle.canonical_kir_v11_length(),
        request_sha256: hex(input.request_sha256),
        request_bytes: input.request_bytes(),
        expected_bytes,
        outputs,
    })
}

fn validate_case(
    case: ProductionSemanticCaseV4<'_>,
) -> Result<usize, ProductionSemanticConformanceErrorV4> {
    if case.case_id.is_empty() {
        return Err(ProductionSemanticConformanceErrorV4::EmptyCaseId);
    }
    if case.case_id.len() > MAX_PRODUCTION_CONFORMANCE_CASE_ID_BYTES_V4 {
        return Err(ProductionSemanticConformanceErrorV4::CaseIdTooLong);
    }
    if !case.case_id.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
    }) {
        return Err(ProductionSemanticConformanceErrorV4::InvalidCaseId);
    }
    if case.outputs.is_empty() {
        return Err(ProductionSemanticConformanceErrorV4::EmptyOutputs);
    }
    if case.outputs.len() > MAX_PRODUCTION_CONFORMANCE_OUTPUTS_V4 {
        return Err(ProductionSemanticConformanceErrorV4::TooManyOutputs);
    }
    let mut ordinals = BTreeSet::new();
    let mut total = 0_usize;
    for output in case.outputs {
        if !ordinals.insert(output.argument_ordinal) {
            return Err(
                ProductionSemanticConformanceErrorV4::DuplicateOutputOrdinal(
                    output.argument_ordinal,
                ),
            );
        }
        if output.bytes.len() != output.initialized.len() {
            return Err(
                ProductionSemanticConformanceErrorV4::InitializationLengthMismatch {
                    argument_ordinal: output.argument_ordinal,
                },
            );
        }
        total = total
            .checked_add(output.bytes.len())
            .ok_or(ProductionSemanticConformanceErrorV4::ExpectedBytesTooLarge)?;
        if total > MAX_PRODUCTION_CONFORMANCE_EXPECTED_BYTES_V4 {
            return Err(ProductionSemanticConformanceErrorV4::ExpectedBytesTooLarge);
        }
    }
    Ok(total)
}

fn observe_buffer(
    execution: &fe2o3_kir_sim::SimulationExecutionV1,
    expected: &ExactBufferExpectationV4<'_>,
) -> ExactBufferObservationV4 {
    let expected_sha256 = hex(Sha256::digest(expected.bytes).into());
    let Some(argument) = execution.arguments().get(expected.argument_ordinal) else {
        return unavailable_observation(
            expected,
            expected_sha256,
            ExactBufferUnavailableV4::MissingArgument,
        );
    };
    let fe2o3_kir_sim::SimulationArgumentV1::Buffer(buffer) = argument else {
        return unavailable_observation(
            expected,
            expected_sha256,
            ExactBufferUnavailableV4::ArgumentIsNotBuffer,
        );
    };
    if buffer.element() != expected.element {
        return unavailable_observation(
            expected,
            expected_sha256,
            ExactBufferUnavailableV4::ScalarTypeMismatch,
        );
    }
    ExactBufferObservationV4 {
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
    expected: &ExactBufferExpectationV4<'_>,
    expected_sha256: String,
    unavailable: ExactBufferUnavailableV4,
) -> ExactBufferObservationV4 {
    ExactBufferObservationV4 {
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

    fn expectation(argument_ordinal: usize, bytes: &[u8]) -> ExactBufferExpectationV4<'_> {
        ExactBufferExpectationV4 {
            argument_ordinal,
            element: ScalarType::U8,
            bytes,
            initialized: &[true],
        }
    }

    #[test]
    fn capability_contract_keeps_unimplemented_producer_paths_typed_unavailable() {
        let capabilities = production_semantic_capabilities_v4();
        assert!(!capabilities.hardware_observed);
        assert!(!capabilities.performance_prediction);
        assert!(capabilities.cases.iter().any(|case| {
            case.family == "integer_switch"
                && case.disposition == ProductionSemanticDispositionV4::ExactConformance
                && case.reason.is_none()
        }));
        assert!(capabilities.cases.iter().any(|case| {
            case.family == "volatile_memory"
                && case.disposition == ProductionSemanticDispositionV4::ProducerUnavailable
                && case.reason
                    == Some(
                        ProductionSemanticUnavailableReasonV4::OrdinaryFrontendIntrinsicRejected,
                    )
        }));
    }

    #[test]
    fn case_validation_rejects_ambiguous_or_unbounded_expectations() {
        let one = [1_u8];
        let duplicate = [expectation(2, &one), expectation(2, &one)];
        assert_eq!(
            validate_case(ProductionSemanticCaseV4 {
                case_id: "duplicate",
                outputs: &duplicate,
            }),
            Err(ProductionSemanticConformanceErrorV4::DuplicateOutputOrdinal(2))
        );
        let oversized = vec![0_u8; MAX_PRODUCTION_CONFORMANCE_EXPECTED_BYTES_V4 + 1];
        let initialized = vec![true; oversized.len()];
        let outputs = [ExactBufferExpectationV4 {
            argument_ordinal: 0,
            element: ScalarType::U8,
            bytes: &oversized,
            initialized: &initialized,
        }];
        assert_eq!(
            validate_case(ProductionSemanticCaseV4 {
                case_id: "oversized",
                outputs: &outputs,
            }),
            Err(ProductionSemanticConformanceErrorV4::ExpectedBytesTooLarge)
        );
    }

    #[test]
    fn case_validation_rejects_hostile_ids_and_initialization_mismatch() {
        let bytes = [1_u8];
        let outputs = [expectation(0, &bytes)];
        assert_eq!(
            validate_case(ProductionSemanticCaseV4 {
                case_id: "cross/run",
                outputs: &outputs,
            }),
            Err(ProductionSemanticConformanceErrorV4::InvalidCaseId)
        );
        let no_initialization = [ExactBufferExpectationV4 {
            argument_ordinal: 0,
            element: ScalarType::U8,
            bytes: &bytes,
            initialized: &[],
        }];
        assert_eq!(
            validate_case(ProductionSemanticCaseV4 {
                case_id: "init-mismatch",
                outputs: &no_initialization,
            }),
            Err(
                ProductionSemanticConformanceErrorV4::InitializationLengthMismatch {
                    argument_ordinal: 0,
                }
            )
        );
    }
}
