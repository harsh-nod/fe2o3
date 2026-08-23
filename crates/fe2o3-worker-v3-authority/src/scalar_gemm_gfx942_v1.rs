//! Owned, fail-closed verifier for the strict scalar GEMM Worker V3 profile.
//!
//! This is the production owner of the host verifier boundary, but it intentionally cannot yet
//! return an admitting decision. It authenticates the exact request and retained Verus execution,
//! retains that evidence for review, and reports every proof boundary that still prevents native
//! code authority.

use std::marker::PhantomData;
use std::path::Path;
use std::{error::Error, fmt};

use fe2o3_hsaco_finalize::{
    FinalizationError, ScalarGemmV1WorkerValidationErrorV1,
    validate_scalar_gemm_v1_kernel_descriptor_v1, verify_finalized,
};
use fe2o3_verifier::{
    AuthenticatedScalarGemmWorkerV3ProofV3, CompilerProofBindingValidationErrorV3,
    GeneralGemmRuntimeClosureErrorV2, GeneralGemmVerusRuntimeClosureLeaseV2,
    MAX_SCALAR_GEMM_VERUS_TIMEOUT_SECONDS_V2, ScalarGemmCompilerKirValidationErrorV3,
    ScalarGemmWorkerV3ProofErrorV3, ScalarGemmWorkerV3ProofInputErrorV3,
    ScalarGemmWorkerV3ProofInputV3, build_scalar_gemm_worker_v3_proof_input_v3,
    execute_scalar_gemm_worker_v3_proof_v3, validate_compiler_proof_binding_association_v3,
    validate_scalar_gemm_compiler_kir_v3,
};
use sha2::{Digest as _, Sha256};

use fe2o3_host::{
    CompilerGeneratedKernelExpectationV1, WorkerV3AuditorV1, WorkerV3VerificationRequestV1,
};

const SCALAR_GEMM_LOGICAL_NAME_V1: &str = "scalar_gemm_v1";
const SCALAR_GEMM_EXPORT_NAME_V1: &str = "scalar_gemm_v1";
const REQUIRED_PROCESSOR_V1: &str = "gfx942";
const REQUIRED_TARGET_V1: &str = "gfx942:xnack-";
const REQUIRED_CODE_OBJECT_VERSION_V1: u8 = 6;

/// Proof boundary that still prevents the scalar V3 verifier from authorizing native code.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ProductionScalarGemmWorkerV3OpenObligationV1 {
    /// An immutable compiler/rustc execution must be authenticated for the retained source/MIR.
    CompilerExecutionProvenance,
    /// Authenticated Rust/semantic MIR execution must refine the exact decoded KIR.
    SourceMirToKernelIrRefinement,
    /// Rust operations and the KIR numerical model must agree with required IEEE-754 behavior.
    RustIeeeF32Semantics,
    /// Canonical KIR must refine through upstream LLVM into the exact retained gfx942 machine code.
    EmittedMachineRefinement,
    /// The proof execution must bind the complete final executable and post-link memory effects.
    ProofExecutableBinding,
    /// The generated Rust ABI and final kernarg layout need an authenticated type/layout contract.
    RustTypeLayoutContract,
    /// All Rust, KIR, and machine memory effects need one authenticated effect contract.
    RustEffectContract,
}

/// Complete ordered set of authority obligations that remain open for scalar GEMM V1.
pub const PRODUCTION_SCALAR_GEMM_WORKER_V3_OPEN_OBLIGATIONS_V1:
    [ProductionScalarGemmWorkerV3OpenObligationV1; 7] = [
    ProductionScalarGemmWorkerV3OpenObligationV1::CompilerExecutionProvenance,
    ProductionScalarGemmWorkerV3OpenObligationV1::SourceMirToKernelIrRefinement,
    ProductionScalarGemmWorkerV3OpenObligationV1::RustIeeeF32Semantics,
    ProductionScalarGemmWorkerV3OpenObligationV1::EmittedMachineRefinement,
    ProductionScalarGemmWorkerV3OpenObligationV1::ProofExecutableBinding,
    ProductionScalarGemmWorkerV3OpenObligationV1::RustTypeLayoutContract,
    ProductionScalarGemmWorkerV3OpenObligationV1::RustEffectContract,
];

/// Non-authoritative result of auditing one exact scalar GEMM Worker V3 request.
///
/// The result is move-only and remains bound to the host-derived challenge carried by its proof.
/// It cannot be converted into a host verification decision.
#[derive(Debug)]
pub struct ProductionScalarGemmWorkerV3AuditV1 {
    proof: AuthenticatedScalarGemmWorkerV3ProofV3,
    finalized_hsaco_sha256: [u8; 32],
    finalized_hsaco_length: u64,
}

/// Exact, non-authoritative scalar GEMM request prepared for retained Verus execution.
///
/// Construction re-inspects the finalized HSACO, validates its exact descriptor, authenticates
/// the compiler proof association and canonical scalar KIR, and generates the challenge-bound
/// Verus input. The value is move-only and cannot be converted into host execution authority.
#[derive(Debug)]
#[must_use = "prepared Worker V3 proof input grants no authority and should be audited or executed"]
pub struct PreparedProductionScalarGemmWorkerV3ProofV1 {
    input: ScalarGemmWorkerV3ProofInputV3,
    finalized_hsaco_sha256: [u8; 32],
    finalized_hsaco_length: u64,
}

impl PreparedProductionScalarGemmWorkerV3ProofV1 {
    /// Returns the exact request-bound Verus input without executing it.
    pub const fn proof_input(&self) -> &ScalarGemmWorkerV3ProofInputV3 {
        &self.input
    }

    /// Moves out the request-bound input and the artifact identity validated with it.
    pub fn into_parts(self) -> (ScalarGemmWorkerV3ProofInputV3, [u8; 32], u64) {
        (
            self.input,
            self.finalized_hsaco_sha256,
            self.finalized_hsaco_length,
        )
    }

    /// Returns the SHA-256 of the exact finalized HSACO that was re-inspected.
    pub const fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        self.finalized_hsaco_sha256
    }

    /// Returns the length of the exact finalized HSACO that was re-inspected.
    pub const fn finalized_hsaco_length(&self) -> u64 {
        self.finalized_hsaco_length
    }

    /// Request preparation does not authenticate retained Verus execution.
    pub const fn authenticates_verus_execution(&self) -> bool {
        false
    }

    /// A prepared request cannot enter the Worker V3 authority gate.
    pub const fn can_enter_worker_v3_gate(&self) -> bool {
        false
    }

    /// Request preparation grants no artifact, load, or launch authority.
    pub const fn grants_artifact_or_runtime_authority(&self) -> bool {
        false
    }
}

/// Zero-runtime auditor for the exact scalar GEMM Worker V3 request.
///
/// This auditor executes the same request, artifact, proof-association, and KIR validation used by
/// [`ProductionScalarGemmWorkerV3VerifierV1`], but stops before Verus execution. It exists so the
/// complete deterministic validation stage runs in ordinary CI without weakening protected
/// runtime-closure requirements.
pub struct ProductionScalarGemmWorkerV3RequestAuditorV1<K> {
    _marker: PhantomData<fn() -> K>,
}

impl<K> ProductionScalarGemmWorkerV3RequestAuditorV1<K> {
    /// Creates one stateless, non-authoritative request auditor.
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<K> Default for ProductionScalarGemmWorkerV3RequestAuditorV1<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K> fmt::Debug for ProductionScalarGemmWorkerV3RequestAuditorV1<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionScalarGemmWorkerV3RequestAuditorV1")
            .finish_non_exhaustive()
    }
}

impl ProductionScalarGemmWorkerV3AuditV1 {
    /// Returns the exact retained request-bound Verus proof.
    pub const fn proof(&self) -> &AuthenticatedScalarGemmWorkerV3ProofV3 {
        &self.proof
    }

    /// Moves out the retained proof and the artifact identity checked with it.
    pub fn into_parts(self) -> (AuthenticatedScalarGemmWorkerV3ProofV3, [u8; 32], u64) {
        (
            self.proof,
            self.finalized_hsaco_sha256,
            self.finalized_hsaco_length,
        )
    }

    /// Returns the finalized HSACO identity checked in the same retained audit call.
    pub const fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        self.finalized_hsaco_sha256
    }

    /// Returns the finalized HSACO length checked in the same retained audit call.
    pub const fn finalized_hsaco_length(&self) -> u64 {
        self.finalized_hsaco_length
    }

    /// Parallel retention is not yet a proof-to-executable refinement.
    pub const fn establishes_proof_executable_binding(&self) -> bool {
        false
    }

    /// Returns every authority obligation that remains open after this audit.
    pub const fn open_authority_obligations(
        &self,
    ) -> &'static [ProductionScalarGemmWorkerV3OpenObligationV1] {
        &PRODUCTION_SCALAR_GEMM_WORKER_V3_OPEN_OBLIGATIONS_V1
    }

    /// A non-authoritative audit can never enter the Worker V3 authority gate.
    pub const fn can_enter_worker_v3_gate(&self) -> bool {
        false
    }

    /// A non-authoritative audit grants no artifact, load, or launch authority.
    pub const fn grants_artifact_or_runtime_authority(&self) -> bool {
        false
    }
}

/// Production owner for exact scalar GEMM Worker V3 auditing.
///
/// The verifier owns a retained, protected Verus runtime closure. Each invocation independently
/// checks the request bytes and final artifact, executes a newly generated challenge-bound proof,
/// and returns move-only audit evidence. It implements only the safe,
/// non-authoritative host auditor interface. No production implementation of the unsafe authority
/// trait exists until all obligations in
/// [`PRODUCTION_SCALAR_GEMM_WORKER_V3_OPEN_OBLIGATIONS_V1`] are mechanically closed.
///
/// The verifier is move-only because it retains open runtime objects and the most recent proof:
///
/// ```compile_fail
/// use fe2o3_worker_v3_authority::ProductionScalarGemmWorkerV3VerifierV1;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<ProductionScalarGemmWorkerV3VerifierV1<()>>();
/// ```
pub struct ProductionScalarGemmWorkerV3VerifierV1<K> {
    runtime: GeneralGemmVerusRuntimeClosureLeaseV2,
    timeout_seconds: u32,
    _marker: PhantomData<fn() -> K>,
}

impl<K> fmt::Debug for ProductionScalarGemmWorkerV3VerifierV1<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionScalarGemmWorkerV3VerifierV1")
            .field("runtime", &self.runtime)
            .field("timeout_seconds", &self.timeout_seconds)
            .finish_non_exhaustive()
    }
}

impl<K> ProductionScalarGemmWorkerV3VerifierV1<K> {
    /// Opens the exact protected Verus runtime closure used for every verification request.
    pub fn open(
        runtime_root: impl AsRef<Path>,
        timeout_seconds: u32,
    ) -> Result<Self, ProductionScalarGemmWorkerV3VerifierErrorV1> {
        validate_timeout(timeout_seconds)?;
        let runtime = GeneralGemmVerusRuntimeClosureLeaseV2::open(runtime_root)
            .map_err(ProductionScalarGemmWorkerV3VerifierErrorV1::RuntimeClosure)?;
        Ok(Self {
            runtime,
            timeout_seconds,
            _marker: PhantomData,
        })
    }

    /// Constructs the verifier from an already retained runtime closure.
    pub fn from_runtime(
        runtime: GeneralGemmVerusRuntimeClosureLeaseV2,
        timeout_seconds: u32,
    ) -> Result<Self, ProductionScalarGemmWorkerV3VerifierErrorV1> {
        validate_timeout(timeout_seconds)?;
        Ok(Self {
            runtime,
            timeout_seconds,
            _marker: PhantomData,
        })
    }

    /// Returns the exact currently open authority obligations.
    pub const fn open_authority_obligations(
        &self,
    ) -> &'static [ProductionScalarGemmWorkerV3OpenObligationV1] {
        &PRODUCTION_SCALAR_GEMM_WORKER_V3_OPEN_OBLIGATIONS_V1
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> ProductionScalarGemmWorkerV3VerifierV1<K> {
    fn authenticate_exact_request(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<ProductionScalarGemmWorkerV3AuditV1, ProductionScalarGemmWorkerV3VerifierErrorV1>
    {
        let prepared = prepare_exact_request(request)?;
        let (proof_input, finalized_hsaco_sha256, finalized_hsaco_length) = prepared.into_parts();
        let proof = execute_scalar_gemm_worker_v3_proof_v3(
            &self.runtime,
            proof_input,
            self.timeout_seconds,
        )
        .map_err(ProductionScalarGemmWorkerV3VerifierErrorV1::ProofExecution)?;
        validate_retained_proof(request, &proof)?;
        Ok(ProductionScalarGemmWorkerV3AuditV1 {
            proof,
            finalized_hsaco_sha256,
            finalized_hsaco_length,
        })
    }
}

impl<K> WorkerV3AuditorV1<K> for ProductionScalarGemmWorkerV3RequestAuditorV1<K>
where
    K: CompilerGeneratedKernelExpectationV1,
{
    type Error = ProductionScalarGemmWorkerV3VerifierErrorV1;
    type Evidence = PreparedProductionScalarGemmWorkerV3ProofV1;

    fn audit(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<Self::Evidence, Self::Error> {
        prepare_exact_request(request)
    }
}

impl<K> WorkerV3AuditorV1<K> for ProductionScalarGemmWorkerV3VerifierV1<K>
where
    K: CompilerGeneratedKernelExpectationV1,
{
    type Error = ProductionScalarGemmWorkerV3VerifierErrorV1;
    type Evidence = ProductionScalarGemmWorkerV3AuditV1;

    fn audit(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<Self::Evidence, Self::Error> {
        self.authenticate_exact_request(request)
    }
}

fn prepare_exact_request<K: CompilerGeneratedKernelExpectationV1>(
    request: &WorkerV3VerificationRequestV1<'_, K>,
) -> Result<PreparedProductionScalarGemmWorkerV3ProofV1, ProductionScalarGemmWorkerV3VerifierErrorV1>
{
    validate_request_profile(request)?;
    validate_request_bytes(request)?;
    validate_finalized_artifact(request)?;

    let receipts = request.semantic_compiler_handoff().capsule().receipts();
    let association = validate_compiler_proof_binding_association_v3(
        receipts.proof_binding(),
        receipts.semantic_mir(),
        receipts.middle_end(),
        receipts.kernel_ir(),
        receipts.mir_to_kir_correspondence(),
        receipts.formal_memory(),
    )
    .map_err(ProductionScalarGemmWorkerV3VerifierErrorV1::CompilerProofBinding)?;
    let scalar_kir = validate_scalar_gemm_compiler_kir_v3(&association, receipts.kernel_ir())
        .map_err(ProductionScalarGemmWorkerV3VerifierErrorV1::ScalarKernelIr)?;
    let input = build_scalar_gemm_worker_v3_proof_input_v3(
        *request.challenge_identity().as_bytes(),
        &association,
        &scalar_kir,
    )
    .map_err(ProductionScalarGemmWorkerV3VerifierErrorV1::ProofInput)?;
    Ok(PreparedProductionScalarGemmWorkerV3ProofV1 {
        input,
        finalized_hsaco_sha256: request.finalized_hsaco_sha256(),
        finalized_hsaco_length: request.finalized_hsaco_length(),
    })
}

fn validate_timeout(
    timeout_seconds: u32,
) -> Result<(), ProductionScalarGemmWorkerV3VerifierErrorV1> {
    if timeout_seconds == 0 || timeout_seconds > MAX_SCALAR_GEMM_VERUS_TIMEOUT_SECONDS_V2 {
        return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::InvalidTimeout);
    }
    Ok(())
}

fn validate_request_profile<K: CompilerGeneratedKernelExpectationV1>(
    request: &WorkerV3VerificationRequestV1<'_, K>,
) -> Result<(), ProductionScalarGemmWorkerV3VerifierErrorV1> {
    if request.marker_logical_name() != SCALAR_GEMM_LOGICAL_NAME_V1
        || request.marker_export_name() != SCALAR_GEMM_EXPORT_NAME_V1
    {
        return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::UnsupportedKernel);
    }
    validate_scalar_gemm_v1_kernel_descriptor_v1(request.descriptor())
        .map_err(ProductionScalarGemmWorkerV3VerifierErrorV1::ScalarDescriptor)?;
    if request.descriptor().kernel_id().as_bytes() != &request.marker_binding_identity() {
        return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::MarkerDescriptorBindingMismatch);
    }
    let target = request.target();
    if target.processor() != REQUIRED_PROCESSOR_V1 || target.to_string() != REQUIRED_TARGET_V1 {
        return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::UnsupportedTarget);
    }
    if request.code_object_version().number() != REQUIRED_CODE_OBJECT_VERSION_V1 {
        return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::UnsupportedCodeObjectVersion);
    }
    Ok(())
}

fn validate_request_bytes<K: CompilerGeneratedKernelExpectationV1>(
    request: &WorkerV3VerificationRequestV1<'_, K>,
) -> Result<(), ProductionScalarGemmWorkerV3VerifierErrorV1> {
    let capsule = request.semantic_compiler_handoff().capsule();
    let capsule_identity = capsule.identity();
    if !capsule_identity.matches_canonical_bytes(request.semantic_capsule_bytes())
        || *capsule_identity.sha256() != request.capsule_sha256()
    {
        return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::SemanticCapsuleIdentityMismatch);
    }
    let formal_memory = capsule.receipts().formal_memory();
    if *formal_memory.identity().sha256() != request.formal_memory_receipt_sha256()
        || u64::try_from(request.formal_memory_receipt_bytes().len()).ok()
            != Some(formal_memory.identity().byte_len())
        || formal_memory.canonical_preimage() != request.formal_memory_receipt_bytes()
    {
        return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::FormalMemoryIdentityMismatch);
    }
    let proof_binding = capsule.receipts().proof_binding();
    if *proof_binding.identity().sha256() != request.proof_binding_receipt_sha256()
        || u64::try_from(request.proof_binding_receipt_bytes().len()).ok()
            != Some(proof_binding.identity().byte_len())
        || proof_binding.canonical_preimage() != request.proof_binding_receipt_bytes()
    {
        return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::ProofBindingIdentityMismatch);
    }
    Ok(())
}

fn validate_finalized_artifact<K: CompilerGeneratedKernelExpectationV1>(
    request: &WorkerV3VerificationRequestV1<'_, K>,
) -> Result<(), ProductionScalarGemmWorkerV3VerifierErrorV1> {
    let bytes = request.finalized_hsaco_bytes();
    if u64::try_from(bytes.len()).ok() != Some(request.finalized_hsaco_length()) {
        return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::FinalizedLengthMismatch);
    }
    if sha256(bytes) != request.finalized_hsaco_sha256() {
        return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::FinalizedIdentityMismatch);
    }
    let inspection = verify_finalized(bytes)
        .map_err(ProductionScalarGemmWorkerV3VerifierErrorV1::FinalizedInspection)?;
    if inspection.hsaco().target() != request.target()
        || inspection.hsaco().code_object_version() != request.code_object_version()
    {
        return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::FinalizedProfileMismatch);
    }
    let mut selected = inspection
        .descriptor_table()
        .kernels()
        .iter()
        .filter(|descriptor| descriptor.kernel_id() == request.descriptor().kernel_id());
    if selected.next() != Some(request.descriptor()) || selected.next().is_some() {
        return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::FinalizedDescriptorMismatch);
    }
    Ok(())
}

fn validate_retained_proof<K: CompilerGeneratedKernelExpectationV1>(
    request: &WorkerV3VerificationRequestV1<'_, K>,
    proof: &AuthenticatedScalarGemmWorkerV3ProofV3,
) -> Result<(), ProductionScalarGemmWorkerV3VerifierErrorV1> {
    if proof.input().challenge() != *request.challenge_identity().as_bytes()
        || !proof.authenticates_retained_verus_execution()
        || !proof.binds_worker_v3_challenge()
        || !proof.establishes_exact_scalar_gemm_kir_profile()
        || !proof.authenticates_exact_decoded_kir_projection()
        || !proof.authenticates_total_typed_projection_token_decoding()
        || !proof.authenticates_total_structural_projection_ast_decoding()
        || !proof.authenticates_reviewed_exact_projection_ast()
        || !proof.authenticates_projection_operational_correspondence()
        || !proof.establishes_kir_to_integer_model_refinement()
    {
        return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::RetainedProofInvariant);
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Failure to configure or execute exact production scalar GEMM V3 verification.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProductionScalarGemmWorkerV3VerifierErrorV1 {
    /// The proof timeout is zero or exceeds the retained verifier limit.
    InvalidTimeout,
    /// The protected Verus runtime closure could not be opened or revalidated.
    RuntimeClosure(GeneralGemmRuntimeClosureErrorV2),
    /// The generated marker is not the exact scalar GEMM V1 profile.
    UnsupportedKernel,
    /// The retained descriptor is not the exact scalar GEMM ABI and launch profile.
    ScalarDescriptor(ScalarGemmV1WorkerValidationErrorV1),
    /// The generated marker binding differs from the exact retained descriptor identity.
    MarkerDescriptorBindingMismatch,
    /// The request is not for exact `gfx942:xnack-`.
    UnsupportedTarget,
    /// The request is not for AMDHSA code-object version 6.
    UnsupportedCodeObjectVersion,
    /// The semantic capsule bytes differ from their independently retained identity.
    SemanticCapsuleIdentityMismatch,
    /// The formal-memory receipt bytes differ from their independently retained identity.
    FormalMemoryIdentityMismatch,
    /// The proof-binding receipt bytes differ from their independently retained identity.
    ProofBindingIdentityMismatch,
    /// The retained finalized artifact length differs from the admitted lineage.
    FinalizedLengthMismatch,
    /// The retained finalized artifact bytes differ from the admitted lineage.
    FinalizedIdentityMismatch,
    /// The retained final artifact is not a valid finalized HSACO.
    FinalizedInspection(FinalizationError),
    /// The re-inspected target or code-object version differs from the request.
    FinalizedProfileMismatch,
    /// The selected descriptor is absent, duplicated, or differs from the request.
    FinalizedDescriptorMismatch,
    /// The exact compiler proof-binding receipt does not associate every retained input.
    CompilerProofBinding(CompilerProofBindingValidationErrorV3),
    /// The associated KIR is not the exact reviewed scalar GEMM profile.
    ScalarKernelIr(ScalarGemmCompilerKirValidationErrorV3),
    /// The exact challenge-bound Verus source could not be generated.
    ProofInput(ScalarGemmWorkerV3ProofInputErrorV3),
    /// Retained Verus execution did not produce the exact accepted result.
    ProofExecution(ScalarGemmWorkerV3ProofErrorV3),
    /// An invariant expected from the authenticated retained proof was absent.
    RetainedProofInvariant,
}

impl fmt::Display for ProductionScalarGemmWorkerV3VerifierErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTimeout => formatter.write_str("invalid scalar Worker V3 proof timeout"),
            Self::RuntimeClosure(error) => {
                write!(formatter, "Verus runtime closure rejected: {error}")
            }
            Self::UnsupportedKernel => formatter.write_str("unsupported Worker V3 kernel profile"),
            Self::ScalarDescriptor(error) => {
                write!(formatter, "scalar GEMM descriptor rejected: {error}")
            }
            Self::MarkerDescriptorBindingMismatch => formatter
                .write_str("scalar GEMM marker binding differs from retained descriptor identity"),
            Self::UnsupportedTarget => formatter.write_str("Worker V3 target is not gfx942:xnack-"),
            Self::UnsupportedCodeObjectVersion => {
                formatter.write_str("Worker V3 artifact is not code-object version 6")
            }
            Self::SemanticCapsuleIdentityMismatch => {
                formatter.write_str("semantic capsule bytes do not match retained identity")
            }
            Self::FormalMemoryIdentityMismatch => {
                formatter.write_str("formal-memory receipt bytes do not match retained identity")
            }
            Self::ProofBindingIdentityMismatch => {
                formatter.write_str("proof-binding receipt bytes do not match retained identity")
            }
            Self::FinalizedLengthMismatch => {
                formatter.write_str("finalized HSACO length does not match retained lineage")
            }
            Self::FinalizedIdentityMismatch => {
                formatter.write_str("finalized HSACO bytes do not match retained identity")
            }
            Self::FinalizedInspection(error) => {
                write!(formatter, "finalized HSACO rejected: {error}")
            }
            Self::FinalizedProfileMismatch => {
                formatter.write_str("finalized HSACO profile differs from the verifier request")
            }
            Self::FinalizedDescriptorMismatch => {
                formatter.write_str("finalized HSACO descriptor differs from the verifier request")
            }
            Self::CompilerProofBinding(error) => {
                write!(formatter, "compiler proof binding rejected: {error}")
            }
            Self::ScalarKernelIr(error) => write!(formatter, "scalar Kernel IR rejected: {error}"),
            Self::ProofInput(error) => write!(formatter, "scalar proof input rejected: {error}"),
            Self::ProofExecution(error) => {
                write!(formatter, "scalar proof execution rejected: {error}")
            }
            Self::RetainedProofInvariant => formatter
                .write_str("retained scalar proof omitted a required established invariant"),
        }
    }
}

impl Error for ProductionScalarGemmWorkerV3VerifierErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RuntimeClosure(error) => Some(error),
            Self::ScalarDescriptor(error) => Some(error),
            Self::FinalizedInspection(error) => Some(error),
            Self::CompilerProofBinding(error) => Some(error),
            Self::ScalarKernelIr(error) => Some(error),
            Self::ProofInput(error) => Some(error),
            Self::ProofExecution(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_and_excessive_timeouts_fail_before_runtime_admission() {
        for timeout in [0, MAX_SCALAR_GEMM_VERUS_TIMEOUT_SECONDS_V2 + 1] {
            assert!(matches!(
                ProductionScalarGemmWorkerV3VerifierV1::<()>::open(
                    "relative-path-is-never-opened",
                    timeout
                ),
                Err(ProductionScalarGemmWorkerV3VerifierErrorV1::InvalidTimeout)
            ));
        }
    }

    #[test]
    fn unprotected_runtime_paths_fail_closed() {
        assert!(matches!(
            ProductionScalarGemmWorkerV3VerifierV1::<()>::open("relative-path-is-rejected", 1),
            Err(ProductionScalarGemmWorkerV3VerifierErrorV1::RuntimeClosure(
                _
            ))
        ));
    }

    #[test]
    fn open_obligations_are_complete_and_stably_ordered() {
        assert_eq!(
            PRODUCTION_SCALAR_GEMM_WORKER_V3_OPEN_OBLIGATIONS_V1,
            [
                ProductionScalarGemmWorkerV3OpenObligationV1::CompilerExecutionProvenance,
                ProductionScalarGemmWorkerV3OpenObligationV1::SourceMirToKernelIrRefinement,
                ProductionScalarGemmWorkerV3OpenObligationV1::RustIeeeF32Semantics,
                ProductionScalarGemmWorkerV3OpenObligationV1::EmittedMachineRefinement,
                ProductionScalarGemmWorkerV3OpenObligationV1::ProofExecutableBinding,
                ProductionScalarGemmWorkerV3OpenObligationV1::RustTypeLayoutContract,
                ProductionScalarGemmWorkerV3OpenObligationV1::RustEffectContract,
            ]
        );
    }
}
