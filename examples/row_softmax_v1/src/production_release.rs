//! Exact production mechanics for the fixed gfx942 row-softmax V1 slice.
//!
//! This module joins source-tested evidence to an already authenticated compiler/worker
//! exchange and executes one typed launch. It does not claim a Verus proof, compiler refinement,
//! artifact memory safety, or support for another row width, mask, target, or math provider.

use core::fmt;
use std::error::Error;
use std::time::Duration;

use fe2o3_core::{DeviceBuffer, GpuContext};
use fe2o3_host::{
    GeneratedProtectedRowSoftmaxV1HostAdapterV1, ObservedContext, ProtectedRowSoftmaxV1HostTokenV1,
    join_protected_row_softmax_v1, prepare_protected_row_softmax_v1_host_token_v1,
};
use fe2o3_hsa_runtime::ReviewedHsaRuntimeAdapterV1;
use fe2o3_hsaco_finalize::{
    InertFirstBuildWorkerV2EvidenceV1, RowSoftmaxV1DirectWorkerExpectationV1,
    inspect_row_softmax_v1_direct_worker_hsaco_v1, prepare_protected_row_softmax_v1_admission_v1,
};
use fe2o3_verifier::{
    RowSoftmaxVerificationCertificateObservationV1, RowSoftmaxVerificationFileObservationV1,
    authenticate_row_softmax_verification_certificate_v1,
};
use sha2::{Digest, Sha256};

use crate::numerical_contract::{
    GFX942_OCML_COMPARISON_POLICY_V1, SoftmaxComparisonErrorV1, SoftmaxContractErrorV1,
    compare_row_softmax_v1, row_softmax_oracle_v1,
};
use crate::verification_certificate::{
    ROW_SOFTMAX_VERIFICATION_MANIFEST_V1, validate_row_softmax_verification_manifest_v1,
};

/// Exact target admitted by this production slice.
pub const ROW_SOFTMAX_V1_PRODUCTION_TARGET: &str = "gfx942:xnack-";
/// Exact comparison-policy selector admitted by this production slice.
pub const ROW_SOFTMAX_V1_PRODUCTION_POLICY: &str = "gfx942-ocml-unmasked-64-v1";

const GUARD_ELEMENTS: usize = 32;
const ROW_ELEMENTS_V1: usize = 64;
const INPUT_PREFIX: f32 = f32::from_bits(0x7fc0_a001);
const INPUT_SUFFIX: f32 = f32::from_bits(0x7fc0_a002);
const OUTPUT_PREFIX: f32 = f32::from_bits(0x7fc0_d001);
const OUTPUT_SUFFIX: f32 = f32::from_bits(0x7fc0_d002);
const OUTPUT_POISON: f32 = f32::from_bits(0x7fc0_d0ff);

/// Built-in deterministic workload selected by an authenticated release configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactRowSoftmaxV1CaseV1 {
    /// A finite nonuniform row with repeated values.
    Normal,
    /// Sixty-four equal finite inputs.
    Equal,
    /// One dominant finite input and sixty-three finite low inputs.
    Dominant,
    /// A deliberately non-finite row used to prove pre-launch rejection.
    Exceptional,
}

/// Activity policy requested for the fixed workload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowSoftmaxV1MaskProfileV1 {
    /// Every one of the fixed 64 positions participates.
    Unmasked,
    /// Deliberately unsupported alternating activity, used for rejection tests.
    Alternating,
}

/// Complete non-authoritative workload selector for one release invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxV1ReleaseWorkloadV1<'policy> {
    /// Built-in deterministic input class.
    pub case: ExactRowSoftmaxV1CaseV1,
    /// Requested physical row width.
    pub row_elements: u32,
    /// Requested activity policy.
    pub mask: RowSoftmaxV1MaskProfileV1,
    /// Requested numerical comparison policy.
    pub comparison_policy: &'policy str,
}

/// Workload that passed every shape, mask, policy, and CPU-oracle check.
#[derive(Debug)]
pub struct AdmittedRowSoftmaxV1WorkloadV1 {
    case: ExactRowSoftmaxV1CaseV1,
    input: [f32; ROW_ELEMENTS_V1],
    expected: [f32; ROW_ELEMENTS_V1],
}

/// Terminal observation from one exact typed launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxV1ProductionReceiptV1 {
    case: ExactRowSoftmaxV1CaseV1,
    unload_identity: [u8; 32],
}

impl RowSoftmaxV1ProductionReceiptV1 {
    /// Returns the deterministic case that reached the GPU.
    pub const fn case(&self) -> ExactRowSoftmaxV1CaseV1 {
        self.case
    }

    /// Returns the exact terminal unload identity.
    pub const fn unload_identity(&self) -> &[u8; 32] {
        &self.unload_identity
    }

    /// This fixed profile never proves masked execution.
    pub const fn proves_masked_execution(&self) -> bool {
        false
    }

    /// Source-tested evidence is not a Verus/refinement proof.
    pub const fn proves_verus_refinement(&self) -> bool {
        false
    }
}

/// First exact production stage that rejected a workload or launch.
#[derive(Debug)]
#[non_exhaustive]
pub enum RowSoftmaxV1ProductionErrorV1 {
    /// The requested physical shape differs from the fixed ABI.
    Shape,
    /// The requested activity policy differs from the fixed unmasked ABI.
    Mask,
    /// The requested comparison policy differs from the reviewed OCML policy.
    Policy,
    /// The independent CPU oracle rejected the physical input.
    Oracle(SoftmaxContractErrorV1),
    /// Source-tested evidence or compiler/artifact admission failed.
    Admission(String),
    /// The observed GPU target differs from the fixed gfx942 profile.
    Target(String),
    /// Typed runtime preparation or execution failed.
    Runtime(String),
    /// Input, output guard, or numerical comparison failed after completion.
    Postcondition(String),
}

impl fmt::Display for RowSoftmaxV1ProductionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shape => write!(
                formatter,
                "stage=workload-shape: fixed row width must be 64"
            ),
            Self::Mask => write!(formatter, "stage=workload-mask: fixed profile is unmasked"),
            Self::Policy => write!(
                formatter,
                "stage=workload-policy: comparison policy differs from the fixed gfx942 OCML profile"
            ),
            Self::Oracle(error) => write!(
                formatter,
                "stage=cpu-oracle: fixed workload was rejected: {error:?}"
            ),
            Self::Admission(error) => {
                write!(formatter, "stage=artifact-admission: {error}")
            }
            Self::Target(target) => write!(
                formatter,
                "stage=runtime-target: expected {ROW_SOFTMAX_V1_PRODUCTION_TARGET}, found {target}"
            ),
            Self::Runtime(error) => write!(formatter, "stage=typed-runtime: {error}"),
            Self::Postcondition(error) => {
                write!(formatter, "stage=postcondition: {error}")
            }
        }
    }
}

impl Error for RowSoftmaxV1ProductionErrorV1 {}

/// Rejects unsupported workload state before any GPU context or launch authority is created.
pub fn preflight_row_softmax_v1_workload_v1(
    request: RowSoftmaxV1ReleaseWorkloadV1<'_>,
) -> Result<AdmittedRowSoftmaxV1WorkloadV1, RowSoftmaxV1ProductionErrorV1> {
    if request.row_elements != ROW_ELEMENTS_V1 as u32 {
        return Err(RowSoftmaxV1ProductionErrorV1::Shape);
    }
    if request.mask != RowSoftmaxV1MaskProfileV1::Unmasked {
        return Err(RowSoftmaxV1ProductionErrorV1::Mask);
    }
    if request.comparison_policy != ROW_SOFTMAX_V1_PRODUCTION_POLICY {
        return Err(RowSoftmaxV1ProductionErrorV1::Policy);
    }
    let input = case_input(request.case);
    let mut expected = [0.0; ROW_ELEMENTS_V1];
    row_softmax_oracle_v1(&input, None, &mut expected)
        .map_err(RowSoftmaxV1ProductionErrorV1::Oracle)?;
    Ok(AdmittedRowSoftmaxV1WorkloadV1 {
        case: request.case,
        input,
        expected,
    })
}

/// Joins exact source-tested evidence to one already authenticated compiler/worker artifact.
pub fn admit_row_softmax_v1_source_tested_artifact_v1(
    evidence: InertFirstBuildWorkerV2EvidenceV1,
    expectation: RowSoftmaxV1DirectWorkerExpectationV1,
) -> Result<ProtectedRowSoftmaxV1HostTokenV1, RowSoftmaxV1ProductionErrorV1> {
    let inspected = inspect_row_softmax_v1_direct_worker_hsaco_v1(evidence, expectation)
        .map_err(|error| RowSoftmaxV1ProductionErrorV1::Admission(error.to_string()))?;
    let admission =
        prepare_protected_row_softmax_v1_admission_v1(source_tested_certificate()?, inspected)
            .map_err(|error| RowSoftmaxV1ProductionErrorV1::Admission(error.to_string()))?;
    prepare_protected_row_softmax_v1_host_token_v1(admission)
        .map_err(|error| RowSoftmaxV1ProductionErrorV1::Admission(error.to_string()))
}

/// Executes exactly one guarded typed launch for a preflighted fixed workload.
pub fn execute_row_softmax_v1_production_workload_v1(
    token: ProtectedRowSoftmaxV1HostTokenV1,
    workload: AdmittedRowSoftmaxV1WorkloadV1,
) -> Result<RowSoftmaxV1ProductionReceiptV1, RowSoftmaxV1ProductionErrorV1> {
    let context = GpuContext::new(0)
        .map_err(|error| RowSoftmaxV1ProductionErrorV1::Runtime(error.to_string()))?;
    let observed = ObservedContext::observe(&context)
        .map_err(|error| RowSoftmaxV1ProductionErrorV1::Runtime(error.to_string()))?;
    if observed.device().target() != ROW_SOFTMAX_V1_PRODUCTION_TARGET {
        return Err(RowSoftmaxV1ProductionErrorV1::Target(
            observed.device().target().to_owned(),
        ));
    }

    let input_initial = guarded(&workload.input, INPUT_PREFIX, INPUT_SUFFIX);
    let output_initial = guarded(
        &[OUTPUT_POISON; ROW_ELEMENTS_V1],
        OUTPUT_PREFIX,
        OUTPUT_SUFFIX,
    );
    let stream = context
        .create_stream()
        .map_err(|error| RowSoftmaxV1ProductionErrorV1::Runtime(error.to_string()))?;
    let input = DeviceBuffer::from_host(&stream, &input_initial)
        .map_err(|error| RowSoftmaxV1ProductionErrorV1::Runtime(error.to_string()))?;
    let mut output = DeviceBuffer::from_host(&stream, &output_initial)
        .map_err(|error| RowSoftmaxV1ProductionErrorV1::Runtime(error.to_string()))?;
    let body = GUARD_ELEMENTS..GUARD_ELEMENTS + ROW_ELEMENTS_V1;
    let host = GeneratedProtectedRowSoftmaxV1HostAdapterV1::prepare(
        &observed,
        input
            .view(body.clone())
            .map_err(|error| RowSoftmaxV1ProductionErrorV1::Runtime(error.to_string()))?,
        output
            .view_mut(body.clone())
            .map_err(|error| RowSoftmaxV1ProductionErrorV1::Runtime(error.to_string()))?,
    )
    .map_err(|error| RowSoftmaxV1ProductionErrorV1::Runtime(error.to_string()))?;
    let joined = join_protected_row_softmax_v1(token, host)
        .map_err(|error| RowSoftmaxV1ProductionErrorV1::Runtime(error.to_string()))?;
    let adapter = ReviewedHsaRuntimeAdapterV1::new(context.clone())
        .map_err(|error| RowSoftmaxV1ProductionErrorV1::Runtime(error.to_string()))?;
    if adapter.completion_timeout_v1() != Duration::from_secs(5) {
        return Err(RowSoftmaxV1ProductionErrorV1::Runtime(
            "fixed completion timeout drifted".to_owned(),
        ));
    }
    let completed = joined
        .load(adapter)
        .map_err(|error| RowSoftmaxV1ProductionErrorV1::Runtime(error.to_string()))?
        .dispatch_and_wait()
        .map_err(|error| RowSoftmaxV1ProductionErrorV1::Runtime(error.to_string()))?;
    let unloaded = completed.unload();
    if unloaded.proves_masked_execution() {
        return Err(RowSoftmaxV1ProductionErrorV1::Postcondition(
            "terminal receipt overclaimed masked execution".to_owned(),
        ));
    }

    let input_after = input
        .to_host_vec(&stream)
        .map_err(|error| RowSoftmaxV1ProductionErrorV1::Runtime(error.to_string()))?;
    let output_after = output
        .to_host_vec(&stream)
        .map_err(|error| RowSoftmaxV1ProductionErrorV1::Runtime(error.to_string()))?;
    verify_exact(&input_after, &input_initial, "guarded immutable input")?;
    verify_exact(
        &output_after[..GUARD_ELEMENTS],
        &output_initial[..GUARD_ELEMENTS],
        "output prefix canary",
    )?;
    verify_exact(
        &output_after[GUARD_ELEMENTS + ROW_ELEMENTS_V1..],
        &output_initial[GUARD_ELEMENTS + ROW_ELEMENTS_V1..],
        "output suffix canary",
    )?;
    compare_row_softmax_v1(
        &workload.expected,
        &output_after[body],
        None,
        GFX942_OCML_COMPARISON_POLICY_V1,
    )
    .map_err(comparison_error)?;

    Ok(RowSoftmaxV1ProductionReceiptV1 {
        case: workload.case,
        unload_identity: *unloaded.unload_identity().as_bytes(),
    })
}

fn source_tested_certificate() -> Result<
    fe2o3_verifier::AuthenticatedRowSoftmaxVerificationCertificateV1,
    RowSoftmaxV1ProductionErrorV1,
> {
    let inert = validate_row_softmax_verification_manifest_v1(ROW_SOFTMAX_VERIFICATION_MANIFEST_V1)
        .map_err(|error| {
            RowSoftmaxV1ProductionErrorV1::Admission(format!(
                "source-tested verification manifest mismatch: {error:?}"
            ))
        })?;
    let manifest = inert.canonical_manifest_bytes();
    let evidence = [
        Some(RowSoftmaxVerificationFileObservationV1::new(
            "examples/row_softmax_v1/src/kernel.rs",
            include_bytes!("kernel.rs"),
        )),
        Some(RowSoftmaxVerificationFileObservationV1::new(
            "examples/row_softmax_v1/src/numerical_contract.rs",
            include_bytes!("numerical_contract.rs"),
        )),
        Some(RowSoftmaxVerificationFileObservationV1::new(
            "examples/row_softmax_v1/verus/row_softmax_v1.rs",
            include_bytes!("../verus/row_softmax_v1.rs"),
        )),
        Some(RowSoftmaxVerificationFileObservationV1::new(
            "examples/row_softmax_v1/verus/VERUS_CLOSURE_MANIFEST",
            include_bytes!("../verus/VERUS_CLOSURE_MANIFEST"),
        )),
        Some(RowSoftmaxVerificationFileObservationV1::new(
            "examples/row_softmax_v1/verus/VERUS_TRUST_VOCABULARY",
            include_bytes!("../verus/VERUS_TRUST_VOCABULARY"),
        )),
    ];
    authenticate_row_softmax_verification_certificate_v1(
        RowSoftmaxVerificationCertificateObservationV1::new(
            manifest,
            Sha256::digest(manifest).into(),
            evidence,
        ),
    )
    .map_err(|error| RowSoftmaxV1ProductionErrorV1::Admission(error.to_string()))
}

fn case_input(case: ExactRowSoftmaxV1CaseV1) -> [f32; ROW_ELEMENTS_V1] {
    match case {
        ExactRowSoftmaxV1CaseV1::Normal => {
            core::array::from_fn(|index| ((index * 17 + 3) % 29) as f32 * 0.25 - 3.5)
        }
        ExactRowSoftmaxV1CaseV1::Equal => [0.5; ROW_ELEMENTS_V1],
        ExactRowSoftmaxV1CaseV1::Dominant => {
            let mut input = [-32.0; ROW_ELEMENTS_V1];
            input[37] = 32.0;
            input
        }
        ExactRowSoftmaxV1CaseV1::Exceptional => {
            let mut input = [0.0; ROW_ELEMENTS_V1];
            input[11] = f32::NAN;
            input
        }
    }
}

fn guarded(body: &[f32], prefix: f32, suffix: f32) -> Vec<f32> {
    let mut result = vec![prefix; GUARD_ELEMENTS];
    result.extend_from_slice(body);
    result.resize(GUARD_ELEMENTS + ROW_ELEMENTS_V1 + GUARD_ELEMENTS, suffix);
    result
}

fn verify_exact(
    actual: &[f32],
    expected: &[f32],
    role: &str,
) -> Result<(), RowSoftmaxV1ProductionErrorV1> {
    if actual.len() != expected.len()
        || actual
            .iter()
            .zip(expected)
            .any(|(actual, expected)| actual.to_bits() != expected.to_bits())
    {
        return Err(RowSoftmaxV1ProductionErrorV1::Postcondition(format!(
            "{role} changed"
        )));
    }
    Ok(())
}

fn comparison_error(error: SoftmaxComparisonErrorV1) -> RowSoftmaxV1ProductionErrorV1 {
    RowSoftmaxV1ProductionErrorV1::Postcondition(format!("CPU oracle comparison failed: {error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(case: ExactRowSoftmaxV1CaseV1) -> RowSoftmaxV1ReleaseWorkloadV1<'static> {
        RowSoftmaxV1ReleaseWorkloadV1 {
            case,
            row_elements: ROW_ELEMENTS_V1 as u32,
            mask: RowSoftmaxV1MaskProfileV1::Unmasked,
            comparison_policy: ROW_SOFTMAX_V1_PRODUCTION_POLICY,
        }
    }

    #[test]
    fn normal_equal_and_dominant_cases_pass_cpu_preflight() {
        for case in [
            ExactRowSoftmaxV1CaseV1::Normal,
            ExactRowSoftmaxV1CaseV1::Equal,
            ExactRowSoftmaxV1CaseV1::Dominant,
        ] {
            assert!(preflight_row_softmax_v1_workload_v1(request(case)).is_ok());
        }
    }

    #[test]
    fn shape_mask_policy_and_exceptional_input_fail_before_token_creation() {
        let mut wrong_shape = request(ExactRowSoftmaxV1CaseV1::Normal);
        wrong_shape.row_elements = 63;
        assert!(matches!(
            preflight_row_softmax_v1_workload_v1(wrong_shape),
            Err(RowSoftmaxV1ProductionErrorV1::Shape)
        ));

        let mut masked = request(ExactRowSoftmaxV1CaseV1::Normal);
        masked.mask = RowSoftmaxV1MaskProfileV1::Alternating;
        assert!(matches!(
            preflight_row_softmax_v1_workload_v1(masked),
            Err(RowSoftmaxV1ProductionErrorV1::Mask)
        ));

        let mut wrong_policy = request(ExactRowSoftmaxV1CaseV1::Normal);
        wrong_policy.comparison_policy = "wrong-policy";
        assert!(matches!(
            preflight_row_softmax_v1_workload_v1(wrong_policy),
            Err(RowSoftmaxV1ProductionErrorV1::Policy)
        ));

        assert!(matches!(
            preflight_row_softmax_v1_workload_v1(request(ExactRowSoftmaxV1CaseV1::Exceptional)),
            Err(RowSoftmaxV1ProductionErrorV1::Oracle(
                SoftmaxContractErrorV1::NonFiniteInput { index: 11 }
            ))
        ));
    }
}
