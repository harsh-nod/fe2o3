//! Challenge-bound, non-authoritative hardware evidence for Scalar GEMM V1.
//!
//! Every observation accepted here is caller-reported. The recorder checks
//! completeness, ordering, exact identities, arithmetic, and cross-field
//! consistency, then seals the complete record under a canonical SHA-256
//! identity. It does not authenticate the observer and cannot turn hardware
//! observations into formal proof, load authority, or launch authority.
//!
//! Durable custody, signatures, freshness policy, and access control belong to
//! the external protected-evidence archive. The attempt challenge in this
//! record gives that service an exact replay-resistant value to bind.

use std::collections::BTreeSet;
use std::fmt;

use fe2o3_artifacts::{DigestAlgorithm, PayloadDigest};

use crate::{
    Digest, SCALAR_GEMM_COVERAGE_PROFILE_V1, SCALAR_GEMM_MAX_GRID_THREADS_V1,
    SCALAR_GEMM_ROOT_SYMBOL_V1, SCALAR_GEMM_TARGET_V1, SCALAR_GEMM_WORKGROUP_THREADS_V1,
};

pub const SCALAR_GEMM_HARDWARE_EVIDENCE_VERSION_V1: u16 = 1;
pub const SCALAR_GEMM_HARDWARE_EVIDENCE_DOMAIN_V1: [u8; 8] = *b"FE2SGHE\0";
pub const SCALAR_GEMM_HARDWARE_EXPECTATION_DOMAIN_V1: [u8; 8] = *b"FE2SGHX\0";
pub const SCALAR_GEMM_HARDWARE_MAX_CASES_V1: usize = 64;
pub const SCALAR_GEMM_HARDWARE_MAX_CASE_NAME_BYTES_V1: usize = 64;
pub const SCALAR_GEMM_EXPLICIT_KERNARG_BYTES_V1: u64 = 64;
pub const SCALAR_GEMM_COV6_IMPLICIT_KERNARG_BYTES_V1: u64 = 256;
pub const SCALAR_GEMM_TOTAL_KERNARG_BYTES_V1: u64 = 320;
pub const SCALAR_GEMM_KERNARG_ALIGNMENT_V1: u64 = 16;
pub const SCALAR_GEMM_WAVEFRONT_SIZE_V1: u32 = 64;

const F32_BYTES: u64 = size_of::<f32>() as u64;

/// Hardware observations do not establish any of these universal properties.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScalarGemmHardwareFormalClaimV1 {
    MemorySafety,
    RaceFreedom,
    UniversalFunctionalCorrectness,
    CompilerToMachineRefinement,
}

pub const SCALAR_GEMM_HARDWARE_FORMAL_CLAIMS_V1: [ScalarGemmHardwareFormalClaimV1; 0] = [];

/// Exact expected identity and shape of one hardware case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarGemmHardwareCaseExpectationV1 {
    name: String,
    dimensions: [u32; 3],
    input_profile_identity: Digest,
    oracle_identity: Digest,
    a_elements: u64,
    b_elements: u64,
    c_elements: u64,
    expected_groups: Option<[u32; 3]>,
}

impl ScalarGemmHardwareCaseExpectationV1 {
    pub fn new(
        name: impl Into<String>,
        dimensions: [u32; 3],
        input_profile_identity: Digest,
        oracle_identity: Digest,
    ) -> Result<Self, ScalarGemmHardwareEvidenceErrorV1> {
        let name = checked_case_name(name.into())?;
        require_nonzero(input_profile_identity, "input profile")?;
        require_nonzero(oracle_identity, "bit-exact oracle")?;
        let [m, n, k] = dimensions;
        let a_elements = u64::from(m) * u64::from(k);
        let b_elements = u64::from(k) * u64::from(n);
        let c_elements = u64::from(m) * u64::from(n);
        let expected_groups = if c_elements == 0 {
            None
        } else {
            let rounded_threads = c_elements
                .checked_add(SCALAR_GEMM_WORKGROUP_THREADS_V1 - 1)
                .ok_or(ScalarGemmHardwareEvidenceErrorV1::CaseExtentOverflow)?
                / SCALAR_GEMM_WORKGROUP_THREADS_V1
                * SCALAR_GEMM_WORKGROUP_THREADS_V1;
            if rounded_threads > SCALAR_GEMM_MAX_GRID_THREADS_V1 {
                return Err(ScalarGemmHardwareEvidenceErrorV1::CaseGridTooLarge);
            }
            let groups = rounded_threads / SCALAR_GEMM_WORKGROUP_THREADS_V1;
            Some([
                u32::try_from(groups)
                    .map_err(|_| ScalarGemmHardwareEvidenceErrorV1::CaseGridTooLarge)?,
                1,
                1,
            ])
        };
        Ok(Self {
            name,
            dimensions,
            input_profile_identity,
            oracle_identity,
            a_elements,
            b_elements,
            c_elements,
            expected_groups,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn dimensions(&self) -> [u32; 3] {
        self.dimensions
    }

    pub const fn input_profile_identity(&self) -> Digest {
        self.input_profile_identity
    }

    pub const fn oracle_identity(&self) -> Digest {
        self.oracle_identity
    }

    pub const fn a_elements(&self) -> u64 {
        self.a_elements
    }

    pub const fn b_elements(&self) -> u64 {
        self.b_elements
    }

    pub const fn c_elements(&self) -> u64 {
        self.c_elements
    }

    pub const fn expected_groups(&self) -> Option<[u32; 3]> {
        self.expected_groups
    }
}

/// Complete expected identity chain for one protected hardware attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarGemmHardwareEvidenceExpectationV1 {
    attempt_challenge: Digest,
    observer_identity: Digest,
    portable_mir_digest: Digest,
    frontend_authority_commitment: Digest,
    worker_exchange_identity: Digest,
    worker_request_identity: Digest,
    worker_response_identity: Digest,
    artifact_digest: PayloadDigest,
    artifact_byte_len: u64,
    kernel_admission_identity: Digest,
    abi_identity: Digest,
    cases: Vec<ScalarGemmHardwareCaseExpectationV1>,
    identity: Digest,
}

impl ScalarGemmHardwareEvidenceExpectationV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        attempt_challenge: Digest,
        observer_identity: Digest,
        portable_mir_digest: Digest,
        frontend_authority_commitment: Digest,
        worker_exchange_identity: Digest,
        worker_request_identity: Digest,
        worker_response_identity: Digest,
        artifact_digest: PayloadDigest,
        artifact_byte_len: u64,
        kernel_admission_identity: Digest,
        abi_identity: Digest,
        cases: Vec<ScalarGemmHardwareCaseExpectationV1>,
    ) -> Result<Self, ScalarGemmHardwareEvidenceErrorV1> {
        for (field, identity) in [
            ("attempt challenge", attempt_challenge),
            ("observer", observer_identity),
            ("portable MIR", portable_mir_digest),
            (
                "frontend authority commitment",
                frontend_authority_commitment,
            ),
            ("Worker V2 exchange", worker_exchange_identity),
            ("Worker V2 request", worker_request_identity),
            ("Worker V2 response", worker_response_identity),
            ("kernel admission", kernel_admission_identity),
            ("ABI", abi_identity),
        ] {
            require_nonzero(identity, field)?;
        }
        require_sha256(artifact_digest)?;
        if artifact_byte_len == 0 {
            return Err(ScalarGemmHardwareEvidenceErrorV1::ZeroArtifactLength);
        }
        if cases.is_empty() {
            return Err(ScalarGemmHardwareEvidenceErrorV1::MissingField {
                field: "hardware cases",
            });
        }
        if cases.len() > SCALAR_GEMM_HARDWARE_MAX_CASES_V1 {
            return Err(ScalarGemmHardwareEvidenceErrorV1::TooManyCases {
                max: SCALAR_GEMM_HARDWARE_MAX_CASES_V1,
            });
        }
        let mut names = BTreeSet::new();
        for case in &cases {
            if !names.insert(case.name.clone()) {
                return Err(ScalarGemmHardwareEvidenceErrorV1::DuplicateCase {
                    name: case.name.clone(),
                });
            }
        }
        let mut expectation = Self {
            attempt_challenge,
            observer_identity,
            portable_mir_digest,
            frontend_authority_commitment,
            worker_exchange_identity,
            worker_request_identity,
            worker_response_identity,
            artifact_digest,
            artifact_byte_len,
            kernel_admission_identity,
            abi_identity,
            cases,
            identity: Digest::from_bytes([0; 32]),
        };
        expectation.identity = sha256(&expectation.canonical_bytes_without_identity());
        Ok(expectation)
    }

    pub const fn attempt_challenge(&self) -> Digest {
        self.attempt_challenge
    }

    pub const fn observer_identity(&self) -> Digest {
        self.observer_identity
    }

    pub const fn portable_mir_digest(&self) -> Digest {
        self.portable_mir_digest
    }

    pub const fn frontend_authority_commitment(&self) -> Digest {
        self.frontend_authority_commitment
    }

    pub const fn worker_exchange_identity(&self) -> Digest {
        self.worker_exchange_identity
    }

    pub const fn worker_request_identity(&self) -> Digest {
        self.worker_request_identity
    }

    pub const fn worker_response_identity(&self) -> Digest {
        self.worker_response_identity
    }

    pub const fn artifact_digest(&self) -> PayloadDigest {
        self.artifact_digest
    }

    pub const fn artifact_byte_len(&self) -> u64 {
        self.artifact_byte_len
    }

    pub const fn kernel_admission_identity(&self) -> Digest {
        self.kernel_admission_identity
    }

    pub const fn abi_identity(&self) -> Digest {
        self.abi_identity
    }

    pub fn cases(&self) -> &[ScalarGemmHardwareCaseExpectationV1] {
        &self.cases
    }

    pub const fn identity(&self) -> Digest {
        self.identity
    }

    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        self.canonical_bytes_without_identity()
    }

    fn canonical_bytes_without_identity(&self) -> Vec<u8> {
        let mut writer = IdentityWriter::with_domain(SCALAR_GEMM_HARDWARE_EXPECTATION_DOMAIN_V1);
        writer.digest(self.attempt_challenge);
        writer.digest(self.observer_identity);
        writer.digest(self.portable_mir_digest);
        writer.digest(self.frontend_authority_commitment);
        writer.digest(self.worker_exchange_identity);
        writer.digest(self.worker_request_identity);
        writer.digest(self.worker_response_identity);
        writer.payload_digest(self.artifact_digest);
        writer.u64(self.artifact_byte_len);
        writer.text(SCALAR_GEMM_TARGET_V1);
        writer.text(SCALAR_GEMM_COVERAGE_PROFILE_V1);
        writer.text(SCALAR_GEMM_ROOT_SYMBOL_V1);
        writer.digest(self.kernel_admission_identity);
        writer.digest(self.abi_identity);
        writer.u64(SCALAR_GEMM_EXPLICIT_KERNARG_BYTES_V1);
        writer.u64(SCALAR_GEMM_COV6_IMPLICIT_KERNARG_BYTES_V1);
        writer.u64(SCALAR_GEMM_TOTAL_KERNARG_BYTES_V1);
        writer.u64(SCALAR_GEMM_KERNARG_ALIGNMENT_V1);
        writer.u64(SCALAR_GEMM_WORKGROUP_THREADS_V1);
        writer.u32(SCALAR_GEMM_WAVEFRONT_SIZE_V1);
        writer.u32(self.cases.len() as u32);
        for case in &self.cases {
            writer.case_expectation(case);
        }
        writer.finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmFrontendObservationV1 {
    portable_mir_digest: Digest,
    authority_commitment: Digest,
}

impl ScalarGemmFrontendObservationV1 {
    pub const fn new(portable_mir_digest: Digest, authority_commitment: Digest) -> Self {
        Self {
            portable_mir_digest,
            authority_commitment,
        }
    }

    pub const fn portable_mir_digest(self) -> Digest {
        self.portable_mir_digest
    }

    pub const fn authority_commitment(self) -> Digest {
        self.authority_commitment
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmWorkerExchangeObservationV1 {
    exchange_identity: Digest,
    request_identity: Digest,
    response_identity: Digest,
}

impl ScalarGemmWorkerExchangeObservationV1 {
    pub const fn new(
        exchange_identity: Digest,
        request_identity: Digest,
        response_identity: Digest,
    ) -> Self {
        Self {
            exchange_identity,
            request_identity,
            response_identity,
        }
    }

    pub const fn exchange_identity(self) -> Digest {
        self.exchange_identity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarGemmArtifactObservationV1 {
    digest: PayloadDigest,
    byte_len: u64,
    target: String,
    coverage_profile: String,
}

impl ScalarGemmArtifactObservationV1 {
    pub fn new(
        digest: PayloadDigest,
        byte_len: u64,
        target: impl Into<String>,
        coverage_profile: impl Into<String>,
    ) -> Self {
        Self {
            digest,
            byte_len,
            target: target.into(),
            coverage_profile: coverage_profile.into(),
        }
    }

    pub const fn digest(&self) -> PayloadDigest {
        self.digest
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarGemmKernelAdmissionObservationV1 {
    root_symbol: String,
    kernel_admission_identity: Digest,
    abi_identity: Digest,
    explicit_kernarg_bytes: u64,
    implicit_kernarg_bytes: u64,
    total_kernarg_bytes: u64,
    kernarg_alignment: u64,
    required_workgroup: [u32; 3],
    wavefront_size: u32,
}

impl ScalarGemmKernelAdmissionObservationV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        root_symbol: impl Into<String>,
        kernel_admission_identity: Digest,
        abi_identity: Digest,
        explicit_kernarg_bytes: u64,
        implicit_kernarg_bytes: u64,
        total_kernarg_bytes: u64,
        kernarg_alignment: u64,
        required_workgroup: [u32; 3],
        wavefront_size: u32,
    ) -> Self {
        Self {
            root_symbol: root_symbol.into(),
            kernel_admission_identity,
            abi_identity,
            explicit_kernarg_bytes,
            implicit_kernarg_bytes,
            total_kernarg_bytes,
            kernarg_alignment,
            required_workgroup,
            wavefront_size,
        }
    }

    pub const fn kernel_admission_identity(&self) -> Digest {
        self.kernel_admission_identity
    }

    pub const fn abi_identity(&self) -> Digest {
        self.abi_identity
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarGemmHsaLoadObservationV1 {
    load_identity: Digest,
    artifact_digest: PayloadDigest,
    artifact_byte_len: u64,
    target: String,
    coverage_profile: String,
    kernel_admission_identity: Digest,
    abi_identity: Digest,
}

impl ScalarGemmHsaLoadObservationV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        load_identity: Digest,
        artifact_digest: PayloadDigest,
        artifact_byte_len: u64,
        target: impl Into<String>,
        coverage_profile: impl Into<String>,
        kernel_admission_identity: Digest,
        abi_identity: Digest,
    ) -> Self {
        Self {
            load_identity,
            artifact_digest,
            artifact_byte_len,
            target: target.into(),
            coverage_profile: coverage_profile.into(),
            kernel_admission_identity,
            abi_identity,
        }
    }

    pub const fn load_identity(&self) -> Digest {
        self.load_identity
    }
}

/// Bitwise before/after observations for both immutable inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmInputImmutabilityObservationV1 {
    a_elements: u64,
    a_before_bits_digest: Digest,
    a_after_bits_digest: Digest,
    b_elements: u64,
    b_before_bits_digest: Digest,
    b_after_bits_digest: Digest,
}

impl ScalarGemmInputImmutabilityObservationV1 {
    pub const fn new(
        a_elements: u64,
        a_before_bits_digest: Digest,
        a_after_bits_digest: Digest,
        b_elements: u64,
        b_before_bits_digest: Digest,
        b_after_bits_digest: Digest,
    ) -> Self {
        Self {
            a_elements,
            a_before_bits_digest,
            a_after_bits_digest,
            b_elements,
            b_before_bits_digest,
            b_after_bits_digest,
        }
    }

    pub fn a_was_bitwise_immutable(self) -> bool {
        self.a_before_bits_digest.as_bytes() == self.a_after_bits_digest.as_bytes()
    }

    pub fn b_was_bitwise_immutable(self) -> bool {
        self.b_before_bits_digest.as_bytes() == self.b_after_bits_digest.as_bytes()
    }
}

/// Bit-exact oracle and output observations for one case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmOutputObservationV1 {
    elements: u64,
    oracle_bits_digest: Digest,
    observed_bits_digest: Digest,
    observed_positive_zero_elements: u64,
}

impl ScalarGemmOutputObservationV1 {
    pub const fn new(
        elements: u64,
        oracle_bits_digest: Digest,
        observed_bits_digest: Digest,
        observed_positive_zero_elements: u64,
    ) -> Self {
        Self {
            elements,
            oracle_bits_digest,
            observed_bits_digest,
            observed_positive_zero_elements,
        }
    }

    pub fn was_bit_exact(self) -> bool {
        self.oracle_bits_digest.as_bytes() == self.observed_bits_digest.as_bytes()
    }
}

/// One allocation-relative observation of canaries physically adjacent to C.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmAdjacentCanaryObservationV1 {
    allocation_identity: Digest,
    allocation_byte_len: u64,
    left_byte_offset: u64,
    left_elements: u64,
    output_byte_offset: u64,
    output_elements: u64,
    right_byte_offset: u64,
    right_elements: u64,
    left_expected_bits_digest: Digest,
    left_observed_bits_digest: Digest,
    right_expected_bits_digest: Digest,
    right_observed_bits_digest: Digest,
}

impl ScalarGemmAdjacentCanaryObservationV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        allocation_identity: Digest,
        allocation_byte_len: u64,
        left_byte_offset: u64,
        left_elements: u64,
        output_byte_offset: u64,
        output_elements: u64,
        right_byte_offset: u64,
        right_elements: u64,
        left_expected_bits_digest: Digest,
        left_observed_bits_digest: Digest,
        right_expected_bits_digest: Digest,
        right_observed_bits_digest: Digest,
    ) -> Self {
        Self {
            allocation_identity,
            allocation_byte_len,
            left_byte_offset,
            left_elements,
            output_byte_offset,
            output_elements,
            right_byte_offset,
            right_elements,
            left_expected_bits_digest,
            left_observed_bits_digest,
            right_expected_bits_digest,
            right_observed_bits_digest,
        }
    }

    pub const fn allocation_identity(self) -> Digest {
        self.allocation_identity
    }
}

/// Dispatch geometry and completion observation for one case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmDispatchObservationV1 {
    dispatched: bool,
    groups: Option<[u32; 3]>,
    workgroup: Option<[u32; 3]>,
    dynamic_shared_bytes: u32,
    synchronously_completed: bool,
    completion_identity: Option<Digest>,
    completed_load_identity: Option<Digest>,
}

impl ScalarGemmDispatchObservationV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        dispatched: bool,
        groups: Option<[u32; 3]>,
        workgroup: Option<[u32; 3]>,
        dynamic_shared_bytes: u32,
        synchronously_completed: bool,
        completion_identity: Option<Digest>,
        completed_load_identity: Option<Digest>,
    ) -> Self {
        Self {
            dispatched,
            groups,
            workgroup,
            dynamic_shared_bytes,
            synchronously_completed,
            completion_identity,
            completed_load_identity,
        }
    }

    pub const fn dispatched(self) -> bool {
        self.dispatched
    }

    pub const fn synchronously_completed(self) -> bool {
        self.synchronously_completed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarGemmHardwareCaseObservationV1 {
    name: String,
    dimensions: [u32; 3],
    input_profile_identity: Digest,
    oracle_identity: Digest,
    dispatch: ScalarGemmDispatchObservationV1,
    inputs: ScalarGemmInputImmutabilityObservationV1,
    output: ScalarGemmOutputObservationV1,
    canaries: ScalarGemmAdjacentCanaryObservationV1,
}

impl ScalarGemmHardwareCaseObservationV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: impl Into<String>,
        dimensions: [u32; 3],
        input_profile_identity: Digest,
        oracle_identity: Digest,
        dispatch: ScalarGemmDispatchObservationV1,
        inputs: ScalarGemmInputImmutabilityObservationV1,
        output: ScalarGemmOutputObservationV1,
        canaries: ScalarGemmAdjacentCanaryObservationV1,
    ) -> Self {
        Self {
            name: name.into(),
            dimensions,
            input_profile_identity,
            oracle_identity,
            dispatch,
            inputs,
            output,
            canaries,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn dimensions(&self) -> [u32; 3] {
        self.dimensions
    }

    pub const fn dispatch(&self) -> ScalarGemmDispatchObservationV1 {
        self.dispatch
    }

    pub const fn inputs(&self) -> ScalarGemmInputImmutabilityObservationV1 {
        self.inputs
    }

    pub const fn output(&self) -> ScalarGemmOutputObservationV1 {
        self.output
    }

    pub const fn canaries(&self) -> ScalarGemmAdjacentCanaryObservationV1 {
        self.canaries
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmUnloadObservationV1 {
    load_identity: Digest,
    released: bool,
}

impl ScalarGemmUnloadObservationV1 {
    pub const fn new(load_identity: Digest, released: bool) -> Self {
        Self {
            load_identity,
            released,
        }
    }

    pub const fn released(self) -> bool {
        self.released
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarGemmHardwareObservedFactsV1 {
    frontend: ScalarGemmFrontendObservationV1,
    worker: ScalarGemmWorkerExchangeObservationV1,
    artifact: ScalarGemmArtifactObservationV1,
    admission: ScalarGemmKernelAdmissionObservationV1,
    load: ScalarGemmHsaLoadObservationV1,
    cases: Vec<ScalarGemmHardwareCaseObservationV1>,
    unload: ScalarGemmUnloadObservationV1,
}

impl ScalarGemmHardwareObservedFactsV1 {
    pub const fn frontend(&self) -> ScalarGemmFrontendObservationV1 {
        self.frontend
    }

    pub const fn worker(&self) -> ScalarGemmWorkerExchangeObservationV1 {
        self.worker
    }

    pub const fn artifact(&self) -> &ScalarGemmArtifactObservationV1 {
        &self.artifact
    }

    pub const fn admission(&self) -> &ScalarGemmKernelAdmissionObservationV1 {
        &self.admission
    }

    pub const fn load(&self) -> &ScalarGemmHsaLoadObservationV1 {
        &self.load
    }

    pub fn cases(&self) -> &[ScalarGemmHardwareCaseObservationV1] {
        &self.cases
    }

    pub const fn unload(&self) -> ScalarGemmUnloadObservationV1 {
        self.unload
    }
}

/// Canonically sealed, challenge-bound set of caller-reported observations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarGemmProtectedHardwareEvidenceV1 {
    expectation: ScalarGemmHardwareEvidenceExpectationV1,
    observed: ScalarGemmHardwareObservedFactsV1,
    identity: Digest,
}

impl ScalarGemmProtectedHardwareEvidenceV1 {
    pub const fn version(&self) -> u16 {
        SCALAR_GEMM_HARDWARE_EVIDENCE_VERSION_V1
    }

    pub const fn expectation(&self) -> &ScalarGemmHardwareEvidenceExpectationV1 {
        &self.expectation
    }

    pub const fn observed_facts(&self) -> &ScalarGemmHardwareObservedFactsV1 {
        &self.observed
    }

    pub const fn identity(&self) -> Digest {
        self.identity
    }

    pub const fn formal_claims(&self) -> &[ScalarGemmHardwareFormalClaimV1] {
        &SCALAR_GEMM_HARDWARE_FORMAL_CLAIMS_V1
    }

    pub const fn records_caller_reported_observations(&self) -> bool {
        true
    }

    pub const fn authenticates_observer(&self) -> bool {
        false
    }

    pub const fn proves_memory_safety(&self) -> bool {
        false
    }

    pub const fn proves_race_freedom(&self) -> bool {
        false
    }

    pub const fn proves_universal_functional_correctness(&self) -> bool {
        false
    }

    pub const fn proves_compiler_to_machine_refinement(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        canonical_evidence_bytes(&self.expectation, &self.observed)
    }

    pub fn validate_against(
        &self,
        expected: &ScalarGemmHardwareEvidenceExpectationV1,
    ) -> Result<(), ScalarGemmHardwareEvidenceErrorV1> {
        if &self.expectation != expected {
            return Err(ScalarGemmHardwareEvidenceErrorV1::ExpectationMismatch);
        }
        if sha256(&self.to_canonical_bytes()) != self.identity {
            return Err(ScalarGemmHardwareEvidenceErrorV1::EvidenceIdentityMismatch);
        }
        Ok(())
    }
}

/// One-shot recorder. Duplicate fields and missing prerequisites fail closed.
#[derive(Debug)]
pub struct ScalarGemmHardwareEvidenceRecorderV1 {
    expectation: ScalarGemmHardwareEvidenceExpectationV1,
    frontend: Option<ScalarGemmFrontendObservationV1>,
    worker: Option<ScalarGemmWorkerExchangeObservationV1>,
    artifact: Option<ScalarGemmArtifactObservationV1>,
    admission: Option<ScalarGemmKernelAdmissionObservationV1>,
    load: Option<ScalarGemmHsaLoadObservationV1>,
    cases: Vec<ScalarGemmHardwareCaseObservationV1>,
    observed_case_names: BTreeSet<String>,
    unload: Option<ScalarGemmUnloadObservationV1>,
}

impl ScalarGemmHardwareEvidenceRecorderV1 {
    pub fn new(expectation: ScalarGemmHardwareEvidenceExpectationV1) -> Self {
        Self {
            expectation,
            frontend: None,
            worker: None,
            artifact: None,
            admission: None,
            load: None,
            cases: Vec::new(),
            observed_case_names: BTreeSet::new(),
            unload: None,
        }
    }

    pub fn record_frontend(
        &mut self,
        observation: ScalarGemmFrontendObservationV1,
    ) -> Result<(), ScalarGemmHardwareEvidenceErrorV1> {
        require_empty(&self.frontend, "frontend observation")?;
        require_identity(
            observation.portable_mir_digest,
            self.expectation.portable_mir_digest,
            "portable MIR",
        )?;
        require_identity(
            observation.authority_commitment,
            self.expectation.frontend_authority_commitment,
            "frontend authority commitment",
        )?;
        self.frontend = Some(observation);
        Ok(())
    }

    pub fn record_worker_exchange(
        &mut self,
        observation: ScalarGemmWorkerExchangeObservationV1,
    ) -> Result<(), ScalarGemmHardwareEvidenceErrorV1> {
        require_empty(&self.worker, "Worker V2 exchange observation")?;
        for (field, actual, expected) in [
            (
                "Worker V2 exchange",
                observation.exchange_identity,
                self.expectation.worker_exchange_identity,
            ),
            (
                "Worker V2 request",
                observation.request_identity,
                self.expectation.worker_request_identity,
            ),
            (
                "Worker V2 response",
                observation.response_identity,
                self.expectation.worker_response_identity,
            ),
        ] {
            require_identity(actual, expected, field)?;
        }
        self.worker = Some(observation);
        Ok(())
    }

    pub fn record_artifact(
        &mut self,
        observation: ScalarGemmArtifactObservationV1,
    ) -> Result<(), ScalarGemmHardwareEvidenceErrorV1> {
        require_empty(&self.artifact, "artifact observation")?;
        require_sha256(observation.digest)?;
        if observation.digest != self.expectation.artifact_digest {
            return Err(ScalarGemmHardwareEvidenceErrorV1::ArtifactDigestMismatch);
        }
        if observation.byte_len != self.expectation.artifact_byte_len {
            return Err(ScalarGemmHardwareEvidenceErrorV1::ArtifactLengthMismatch);
        }
        require_text(&observation.target, SCALAR_GEMM_TARGET_V1, "target")?;
        require_text(
            &observation.coverage_profile,
            SCALAR_GEMM_COVERAGE_PROFILE_V1,
            "coverage profile",
        )?;
        self.artifact = Some(observation);
        Ok(())
    }

    pub fn record_kernel_admission(
        &mut self,
        observation: ScalarGemmKernelAdmissionObservationV1,
    ) -> Result<(), ScalarGemmHardwareEvidenceErrorV1> {
        require_empty(&self.admission, "kernel admission observation")?;
        require_text(
            &observation.root_symbol,
            SCALAR_GEMM_ROOT_SYMBOL_V1,
            "root symbol",
        )?;
        require_identity(
            observation.kernel_admission_identity,
            self.expectation.kernel_admission_identity,
            "kernel admission",
        )?;
        require_identity(
            observation.abi_identity,
            self.expectation.abi_identity,
            "ABI",
        )?;
        if observation.explicit_kernarg_bytes != SCALAR_GEMM_EXPLICIT_KERNARG_BYTES_V1
            || observation.implicit_kernarg_bytes != SCALAR_GEMM_COV6_IMPLICIT_KERNARG_BYTES_V1
            || observation.total_kernarg_bytes != SCALAR_GEMM_TOTAL_KERNARG_BYTES_V1
            || observation.kernarg_alignment != SCALAR_GEMM_KERNARG_ALIGNMENT_V1
        {
            return Err(ScalarGemmHardwareEvidenceErrorV1::AbiMismatch);
        }
        if observation.required_workgroup != [SCALAR_GEMM_WORKGROUP_THREADS_V1 as u32, 1, 1]
            || observation.wavefront_size != SCALAR_GEMM_WAVEFRONT_SIZE_V1
        {
            return Err(ScalarGemmHardwareEvidenceErrorV1::KernelProfileMismatch);
        }
        self.admission = Some(observation);
        Ok(())
    }

    pub fn record_hsa_load(
        &mut self,
        observation: ScalarGemmHsaLoadObservationV1,
    ) -> Result<(), ScalarGemmHardwareEvidenceErrorV1> {
        require_empty(&self.load, "HSA load observation")?;
        for field in [
            ("frontend observation", self.frontend.is_some()),
            ("Worker V2 exchange observation", self.worker.is_some()),
            ("artifact observation", self.artifact.is_some()),
            ("kernel admission observation", self.admission.is_some()),
        ] {
            if !field.1 {
                return Err(ScalarGemmHardwareEvidenceErrorV1::MissingField { field: field.0 });
            }
        }
        require_nonzero(observation.load_identity, "HSA load")?;
        if observation.artifact_digest != self.expectation.artifact_digest {
            return Err(ScalarGemmHardwareEvidenceErrorV1::ArtifactDigestMismatch);
        }
        if observation.artifact_byte_len != self.expectation.artifact_byte_len {
            return Err(ScalarGemmHardwareEvidenceErrorV1::ArtifactLengthMismatch);
        }
        require_text(&observation.target, SCALAR_GEMM_TARGET_V1, "loaded target")?;
        require_text(
            &observation.coverage_profile,
            SCALAR_GEMM_COVERAGE_PROFILE_V1,
            "loaded coverage profile",
        )?;
        require_identity(
            observation.kernel_admission_identity,
            self.expectation.kernel_admission_identity,
            "loaded kernel admission",
        )?;
        require_identity(
            observation.abi_identity,
            self.expectation.abi_identity,
            "loaded ABI",
        )?;
        self.load = Some(observation);
        Ok(())
    }

    pub fn record_case(
        &mut self,
        observation: ScalarGemmHardwareCaseObservationV1,
    ) -> Result<(), ScalarGemmHardwareEvidenceErrorV1> {
        if self.load.is_none() {
            return Err(ScalarGemmHardwareEvidenceErrorV1::MissingField {
                field: "HSA load observation",
            });
        }
        if self.unload.is_some() {
            return Err(ScalarGemmHardwareEvidenceErrorV1::CaseAfterUnload);
        }
        if self.observed_case_names.contains(&observation.name) {
            return Err(ScalarGemmHardwareEvidenceErrorV1::DuplicateCase {
                name: observation.name,
            });
        }
        let Some(expected) = self.expectation.cases.get(self.cases.len()) else {
            return Err(ScalarGemmHardwareEvidenceErrorV1::UnexpectedCase {
                name: observation.name,
            });
        };
        if observation.name != expected.name {
            return Err(ScalarGemmHardwareEvidenceErrorV1::CaseOrderMismatch {
                expected: expected.name.clone(),
                observed: observation.name,
            });
        }
        validate_case(
            expected,
            self.load.as_ref().expect("load was checked"),
            &observation,
        )?;
        self.observed_case_names.insert(observation.name.clone());
        self.cases.push(observation);
        Ok(())
    }

    pub fn record_unload(
        &mut self,
        observation: ScalarGemmUnloadObservationV1,
    ) -> Result<(), ScalarGemmHardwareEvidenceErrorV1> {
        require_empty(&self.unload, "HSA unload observation")?;
        let load = self
            .load
            .as_ref()
            .ok_or(ScalarGemmHardwareEvidenceErrorV1::MissingField {
                field: "HSA load observation",
            })?;
        if self.cases.len() != self.expectation.cases.len() {
            return Err(ScalarGemmHardwareEvidenceErrorV1::MissingHardwareCases {
                expected: self.expectation.cases.len(),
                observed: self.cases.len(),
            });
        }
        require_identity(
            observation.load_identity,
            load.load_identity,
            "unloaded HSA object",
        )?;
        if !observation.released {
            return Err(ScalarGemmHardwareEvidenceErrorV1::UnloadNotReleased);
        }
        self.unload = Some(observation);
        Ok(())
    }

    pub fn finish(
        self,
    ) -> Result<ScalarGemmProtectedHardwareEvidenceV1, ScalarGemmHardwareEvidenceErrorV1> {
        let observed = ScalarGemmHardwareObservedFactsV1 {
            frontend: required(self.frontend, "frontend observation")?,
            worker: required(self.worker, "Worker V2 exchange observation")?,
            artifact: required(self.artifact, "artifact observation")?,
            admission: required(self.admission, "kernel admission observation")?,
            load: required(self.load, "HSA load observation")?,
            cases: self.cases,
            unload: required(self.unload, "HSA unload observation")?,
        };
        if observed.cases.len() != self.expectation.cases.len() {
            return Err(ScalarGemmHardwareEvidenceErrorV1::MissingHardwareCases {
                expected: self.expectation.cases.len(),
                observed: observed.cases.len(),
            });
        }
        let identity = sha256(&canonical_evidence_bytes(&self.expectation, &observed));
        Ok(ScalarGemmProtectedHardwareEvidenceV1 {
            expectation: self.expectation,
            observed,
            identity,
        })
    }
}

fn validate_case(
    expected: &ScalarGemmHardwareCaseExpectationV1,
    load: &ScalarGemmHsaLoadObservationV1,
    observed: &ScalarGemmHardwareCaseObservationV1,
) -> Result<(), ScalarGemmHardwareEvidenceErrorV1> {
    if observed.dimensions != expected.dimensions {
        return Err(ScalarGemmHardwareEvidenceErrorV1::CaseDimensionsMismatch {
            name: expected.name.clone(),
        });
    }
    require_identity(
        observed.input_profile_identity,
        expected.input_profile_identity,
        "case input profile",
    )?;
    require_identity(
        observed.oracle_identity,
        expected.oracle_identity,
        "case bit-exact oracle",
    )?;

    let dispatch = observed.dispatch;
    match expected.expected_groups {
        None => {
            if dispatch.dispatched
                || dispatch.groups.is_some()
                || dispatch.workgroup.is_some()
                || dispatch.dynamic_shared_bytes != 0
                || dispatch.synchronously_completed
                || dispatch.completion_identity.is_some()
                || dispatch.completed_load_identity.is_some()
            {
                return Err(ScalarGemmHardwareEvidenceErrorV1::DispatchStateMismatch {
                    name: expected.name.clone(),
                });
            }
        }
        Some(expected_groups) => {
            if !dispatch.dispatched
                || dispatch.groups != Some(expected_groups)
                || dispatch.workgroup != Some([SCALAR_GEMM_WORKGROUP_THREADS_V1 as u32, 1, 1])
                || dispatch.dynamic_shared_bytes != 0
            {
                return Err(ScalarGemmHardwareEvidenceErrorV1::GeometryMismatch {
                    name: expected.name.clone(),
                });
            }
            if !dispatch.synchronously_completed {
                return Err(ScalarGemmHardwareEvidenceErrorV1::IncompleteDispatch {
                    name: expected.name.clone(),
                });
            }
            let completion = dispatch.completion_identity.ok_or_else(|| {
                ScalarGemmHardwareEvidenceErrorV1::MissingCaseField {
                    name: expected.name.clone(),
                    field: "completion identity",
                }
            })?;
            require_nonzero(completion, "completion")?;
            let completed_load = dispatch.completed_load_identity.ok_or_else(|| {
                ScalarGemmHardwareEvidenceErrorV1::MissingCaseField {
                    name: expected.name.clone(),
                    field: "completed HSA load identity",
                }
            })?;
            require_identity(completed_load, load.load_identity, "completed HSA load")?;
        }
    }

    let inputs = observed.inputs;
    if inputs.a_elements != expected.a_elements || inputs.b_elements != expected.b_elements {
        return Err(ScalarGemmHardwareEvidenceErrorV1::InputExtentMismatch {
            name: expected.name.clone(),
        });
    }
    for (field, identity) in [
        ("A before bits", inputs.a_before_bits_digest),
        ("A after bits", inputs.a_after_bits_digest),
        ("B before bits", inputs.b_before_bits_digest),
        ("B after bits", inputs.b_after_bits_digest),
    ] {
        require_nonzero(identity, field)?;
    }
    if !inputs.a_was_bitwise_immutable() || !inputs.b_was_bitwise_immutable() {
        return Err(ScalarGemmHardwareEvidenceErrorV1::InputMutation {
            name: expected.name.clone(),
        });
    }

    let output = observed.output;
    if output.elements != expected.c_elements {
        return Err(ScalarGemmHardwareEvidenceErrorV1::OutputExtentMismatch {
            name: expected.name.clone(),
        });
    }
    require_nonzero(output.oracle_bits_digest, "oracle output bits")?;
    require_nonzero(output.observed_bits_digest, "observed output bits")?;
    if !output.was_bit_exact() {
        return Err(ScalarGemmHardwareEvidenceErrorV1::OutputMismatch {
            name: expected.name.clone(),
        });
    }
    if output.observed_positive_zero_elements > output.elements {
        return Err(
            ScalarGemmHardwareEvidenceErrorV1::InvalidPositiveZeroCount {
                name: expected.name.clone(),
            },
        );
    }
    if expected.dimensions[2] == 0 && output.observed_positive_zero_elements != expected.c_elements
    {
        return Err(ScalarGemmHardwareEvidenceErrorV1::ZeroKNotPositiveZero {
            name: expected.name.clone(),
        });
    }

    validate_canaries(expected, observed.canaries)
}

fn validate_canaries(
    expected: &ScalarGemmHardwareCaseExpectationV1,
    canaries: ScalarGemmAdjacentCanaryObservationV1,
) -> Result<(), ScalarGemmHardwareEvidenceErrorV1> {
    require_nonzero(canaries.allocation_identity, "guarded output allocation")?;
    if canaries.left_elements == 0 || canaries.right_elements == 0 {
        return Err(ScalarGemmHardwareEvidenceErrorV1::EmptyCanary {
            name: expected.name.clone(),
        });
    }
    if canaries.output_elements != expected.c_elements {
        return Err(ScalarGemmHardwareEvidenceErrorV1::OutputExtentMismatch {
            name: expected.name.clone(),
        });
    }
    let left_bytes = canaries
        .left_elements
        .checked_mul(F32_BYTES)
        .ok_or(ScalarGemmHardwareEvidenceErrorV1::CanaryExtentOverflow)?;
    let output_bytes = canaries
        .output_elements
        .checked_mul(F32_BYTES)
        .ok_or(ScalarGemmHardwareEvidenceErrorV1::CanaryExtentOverflow)?;
    let right_bytes = canaries
        .right_elements
        .checked_mul(F32_BYTES)
        .ok_or(ScalarGemmHardwareEvidenceErrorV1::CanaryExtentOverflow)?;
    let left_end = canaries
        .left_byte_offset
        .checked_add(left_bytes)
        .ok_or(ScalarGemmHardwareEvidenceErrorV1::CanaryExtentOverflow)?;
    let output_end = canaries
        .output_byte_offset
        .checked_add(output_bytes)
        .ok_or(ScalarGemmHardwareEvidenceErrorV1::CanaryExtentOverflow)?;
    let right_end = canaries
        .right_byte_offset
        .checked_add(right_bytes)
        .ok_or(ScalarGemmHardwareEvidenceErrorV1::CanaryExtentOverflow)?;
    if left_end != canaries.output_byte_offset
        || output_end != canaries.right_byte_offset
        || right_end > canaries.allocation_byte_len
    {
        return Err(ScalarGemmHardwareEvidenceErrorV1::CanariesNotAdjacent {
            name: expected.name.clone(),
        });
    }
    for (field, identity) in [
        (
            "left expected canary bits",
            canaries.left_expected_bits_digest,
        ),
        (
            "left observed canary bits",
            canaries.left_observed_bits_digest,
        ),
        (
            "right expected canary bits",
            canaries.right_expected_bits_digest,
        ),
        (
            "right observed canary bits",
            canaries.right_observed_bits_digest,
        ),
    ] {
        require_nonzero(identity, field)?;
    }
    if canaries.left_expected_bits_digest != canaries.left_observed_bits_digest
        || canaries.right_expected_bits_digest != canaries.right_observed_bits_digest
    {
        return Err(ScalarGemmHardwareEvidenceErrorV1::CanaryMutation {
            name: expected.name.clone(),
        });
    }
    Ok(())
}

fn canonical_evidence_bytes(
    expectation: &ScalarGemmHardwareEvidenceExpectationV1,
    observed: &ScalarGemmHardwareObservedFactsV1,
) -> Vec<u8> {
    let mut writer = IdentityWriter::with_domain(SCALAR_GEMM_HARDWARE_EVIDENCE_DOMAIN_V1);
    writer.digest(expectation.identity);
    writer.bytes(&expectation.to_canonical_bytes());
    writer.digest(observed.frontend.portable_mir_digest);
    writer.digest(observed.frontend.authority_commitment);
    writer.digest(observed.worker.exchange_identity);
    writer.digest(observed.worker.request_identity);
    writer.digest(observed.worker.response_identity);
    writer.payload_digest(observed.artifact.digest);
    writer.u64(observed.artifact.byte_len);
    writer.text(&observed.artifact.target);
    writer.text(&observed.artifact.coverage_profile);
    writer.text(&observed.admission.root_symbol);
    writer.digest(observed.admission.kernel_admission_identity);
    writer.digest(observed.admission.abi_identity);
    writer.u64(observed.admission.explicit_kernarg_bytes);
    writer.u64(observed.admission.implicit_kernarg_bytes);
    writer.u64(observed.admission.total_kernarg_bytes);
    writer.u64(observed.admission.kernarg_alignment);
    writer.u32x3(observed.admission.required_workgroup);
    writer.u32(observed.admission.wavefront_size);
    writer.digest(observed.load.load_identity);
    writer.payload_digest(observed.load.artifact_digest);
    writer.u64(observed.load.artifact_byte_len);
    writer.text(&observed.load.target);
    writer.text(&observed.load.coverage_profile);
    writer.digest(observed.load.kernel_admission_identity);
    writer.digest(observed.load.abi_identity);
    writer.u32(observed.cases.len() as u32);
    for case in &observed.cases {
        writer.case_observation(case);
    }
    writer.digest(observed.unload.load_identity);
    writer.bool(observed.unload.released);
    writer.u32(SCALAR_GEMM_HARDWARE_FORMAL_CLAIMS_V1.len() as u32);
    writer.finish()
}

fn checked_case_name(name: String) -> Result<String, ScalarGemmHardwareEvidenceErrorV1> {
    if name.is_empty()
        || name.len() > SCALAR_GEMM_HARDWARE_MAX_CASE_NAME_BYTES_V1
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(ScalarGemmHardwareEvidenceErrorV1::InvalidCaseName);
    }
    Ok(name)
}

fn require_sha256(digest: PayloadDigest) -> Result<(), ScalarGemmHardwareEvidenceErrorV1> {
    if digest.algorithm() != DigestAlgorithm::Sha256 {
        return Err(ScalarGemmHardwareEvidenceErrorV1::UnsupportedArtifactDigest);
    }
    if digest.bytes().as_bytes().iter().all(|byte| *byte == 0) {
        return Err(ScalarGemmHardwareEvidenceErrorV1::ZeroIdentity {
            field: "artifact digest",
        });
    }
    Ok(())
}

fn require_nonzero(
    identity: Digest,
    field: &'static str,
) -> Result<(), ScalarGemmHardwareEvidenceErrorV1> {
    if identity.as_bytes().iter().all(|byte| *byte == 0) {
        Err(ScalarGemmHardwareEvidenceErrorV1::ZeroIdentity { field })
    } else {
        Ok(())
    }
}

fn require_identity(
    actual: Digest,
    expected: Digest,
    field: &'static str,
) -> Result<(), ScalarGemmHardwareEvidenceErrorV1> {
    require_nonzero(actual, field)?;
    if actual != expected {
        Err(ScalarGemmHardwareEvidenceErrorV1::IdentityMismatch { field })
    } else {
        Ok(())
    }
}

fn require_text(
    actual: &str,
    expected: &'static str,
    field: &'static str,
) -> Result<(), ScalarGemmHardwareEvidenceErrorV1> {
    if actual != expected {
        Err(ScalarGemmHardwareEvidenceErrorV1::TextMismatch { field })
    } else {
        Ok(())
    }
}

fn require_empty<T>(
    value: &Option<T>,
    field: &'static str,
) -> Result<(), ScalarGemmHardwareEvidenceErrorV1> {
    if value.is_some() {
        Err(ScalarGemmHardwareEvidenceErrorV1::DuplicateField { field })
    } else {
        Ok(())
    }
}

fn required<T>(
    value: Option<T>,
    field: &'static str,
) -> Result<T, ScalarGemmHardwareEvidenceErrorV1> {
    value.ok_or(ScalarGemmHardwareEvidenceErrorV1::MissingField { field })
}

fn sha256(bytes: &[u8]) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    Digest::from_bytes(*digest.bytes().as_bytes())
}

struct IdentityWriter {
    bytes: Vec<u8>,
}

impl IdentityWriter {
    fn with_domain(domain: [u8; 8]) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&domain);
        bytes.extend_from_slice(&SCALAR_GEMM_HARDWARE_EVIDENCE_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        Self { bytes }
    }

    fn bool(&mut self, value: bool) {
        self.bytes.push(u8::from(value));
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32x3(&mut self, value: [u32; 3]) {
        for component in value {
            self.u32(component);
        }
    }

    fn digest(&mut self, value: Digest) {
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn optional_digest(&mut self, value: Option<Digest>) {
        self.bool(value.is_some());
        self.digest(value.unwrap_or(Digest::from_bytes([0; 32])));
    }

    fn payload_digest(&mut self, value: PayloadDigest) {
        self.u32(match value.algorithm() {
            DigestAlgorithm::Sha256 => 1,
            _ => 0,
        });
        self.bytes.extend_from_slice(value.bytes().as_bytes());
    }

    fn text(&mut self, value: &str) {
        self.u32(value.len() as u32);
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.u32(value.len() as u32);
        self.bytes.extend_from_slice(value);
    }

    fn case_expectation(&mut self, case: &ScalarGemmHardwareCaseExpectationV1) {
        self.text(&case.name);
        self.u32x3(case.dimensions);
        self.digest(case.input_profile_identity);
        self.digest(case.oracle_identity);
        self.u64(case.a_elements);
        self.u64(case.b_elements);
        self.u64(case.c_elements);
        self.bool(case.expected_groups.is_some());
        self.u32x3(case.expected_groups.unwrap_or([0; 3]));
    }

    fn case_observation(&mut self, case: &ScalarGemmHardwareCaseObservationV1) {
        self.text(&case.name);
        self.u32x3(case.dimensions);
        self.digest(case.input_profile_identity);
        self.digest(case.oracle_identity);
        self.bool(case.dispatch.dispatched);
        self.bool(case.dispatch.groups.is_some());
        self.u32x3(case.dispatch.groups.unwrap_or([0; 3]));
        self.bool(case.dispatch.workgroup.is_some());
        self.u32x3(case.dispatch.workgroup.unwrap_or([0; 3]));
        self.u32(case.dispatch.dynamic_shared_bytes);
        self.bool(case.dispatch.synchronously_completed);
        self.optional_digest(case.dispatch.completion_identity);
        self.optional_digest(case.dispatch.completed_load_identity);
        self.u64(case.inputs.a_elements);
        self.digest(case.inputs.a_before_bits_digest);
        self.digest(case.inputs.a_after_bits_digest);
        self.u64(case.inputs.b_elements);
        self.digest(case.inputs.b_before_bits_digest);
        self.digest(case.inputs.b_after_bits_digest);
        self.u64(case.output.elements);
        self.digest(case.output.oracle_bits_digest);
        self.digest(case.output.observed_bits_digest);
        self.u64(case.output.observed_positive_zero_elements);
        self.digest(case.canaries.allocation_identity);
        self.u64(case.canaries.allocation_byte_len);
        self.u64(case.canaries.left_byte_offset);
        self.u64(case.canaries.left_elements);
        self.u64(case.canaries.output_byte_offset);
        self.u64(case.canaries.output_elements);
        self.u64(case.canaries.right_byte_offset);
        self.u64(case.canaries.right_elements);
        self.digest(case.canaries.left_expected_bits_digest);
        self.digest(case.canaries.left_observed_bits_digest);
        self.digest(case.canaries.right_expected_bits_digest);
        self.digest(case.canaries.right_observed_bits_digest);
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ScalarGemmHardwareEvidenceErrorV1 {
    ZeroIdentity { field: &'static str },
    UnsupportedArtifactDigest,
    ZeroArtifactLength,
    InvalidCaseName,
    TooManyCases { max: usize },
    DuplicateCase { name: String },
    UnexpectedCase { name: String },
    CaseOrderMismatch { expected: String, observed: String },
    MissingField { field: &'static str },
    DuplicateField { field: &'static str },
    MissingCaseField { name: String, field: &'static str },
    MissingHardwareCases { expected: usize, observed: usize },
    CaseAfterUnload,
    CaseExtentOverflow,
    CaseGridTooLarge,
    IdentityMismatch { field: &'static str },
    TextMismatch { field: &'static str },
    ArtifactDigestMismatch,
    ArtifactLengthMismatch,
    AbiMismatch,
    KernelProfileMismatch,
    CaseDimensionsMismatch { name: String },
    DispatchStateMismatch { name: String },
    GeometryMismatch { name: String },
    IncompleteDispatch { name: String },
    InputExtentMismatch { name: String },
    InputMutation { name: String },
    OutputExtentMismatch { name: String },
    OutputMismatch { name: String },
    InvalidPositiveZeroCount { name: String },
    ZeroKNotPositiveZero { name: String },
    EmptyCanary { name: String },
    CanaryExtentOverflow,
    CanariesNotAdjacent { name: String },
    CanaryMutation { name: String },
    UnloadNotReleased,
    ExpectationMismatch,
    EvidenceIdentityMismatch,
}

impl fmt::Display for ScalarGemmHardwareEvidenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Scalar GEMM V1 hardware evidence: {self:?}"
        )
    }
}

impl std::error::Error for ScalarGemmHardwareEvidenceErrorV1 {}
