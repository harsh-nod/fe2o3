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
use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectErrorV1, AuthenticatedPhysicalMachineEffectLimitsV1,
    AuthenticatedPhysicalMachineEffectWorkerV1,
    AuthenticatedScalarGemmV1PhysicalMachineEffectEvidenceV1, PhysicalMachineEffectKindV1,
    PhysicalMachineEffectWorkerPolicyV1, ScalarGemmV1PhysicalMachineEffectErrorV1,
    ScalarGemmV1PhysicalMachineEffectProfileV1,
};
use fe2o3_verifier::{
    AuthenticatedScalarGemmWorkerV3ProofV3, CompilerProofBindingValidationErrorV3,
    GeneralGemmRuntimeClosureErrorV2, GeneralGemmVerusRuntimeClosureLeaseV2,
    MAX_SCALAR_GEMM_VERUS_TIMEOUT_SECONDS_V2, PreparedScalarGemmWorkerV3ProofV3,
    ScalarGemmCompilerKirValidationErrorV3, ScalarGemmWorkerV3ExecutableBindingComponentsV1,
    ScalarGemmWorkerV3ExecutableBindingErrorV1, ScalarGemmWorkerV3ExecutableBindingV1,
    ScalarGemmWorkerV3MachineEffectKindV1, ScalarGemmWorkerV3MachineEffectSiteV1,
    ScalarGemmWorkerV3MeasuredIdentityV1, ScalarGemmWorkerV3ProofErrorV3,
    ScalarGemmWorkerV3ProofInputErrorV3, build_scalar_gemm_worker_v3_proof_input_v3,
    execute_scalar_gemm_worker_v3_proof_v3, prepare_scalar_gemm_worker_v3_proof_v3,
    validate_compiler_proof_binding_association_v3, validate_scalar_gemm_compiler_kir_v3,
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
    /// The generated Rust ABI and final kernarg layout need an authenticated type/layout contract.
    RustTypeLayoutContract,
    /// All Rust, KIR, and machine memory effects need one authenticated effect contract.
    RustEffectContract,
}

/// Complete ordered set of authority obligations that remain open for scalar GEMM V1.
pub const PRODUCTION_SCALAR_GEMM_WORKER_V3_OPEN_OBLIGATIONS_V1:
    [ProductionScalarGemmWorkerV3OpenObligationV1; 6] = [
    ProductionScalarGemmWorkerV3OpenObligationV1::CompilerExecutionProvenance,
    ProductionScalarGemmWorkerV3OpenObligationV1::SourceMirToKernelIrRefinement,
    ProductionScalarGemmWorkerV3OpenObligationV1::RustIeeeF32Semantics,
    ProductionScalarGemmWorkerV3OpenObligationV1::EmittedMachineRefinement,
    ProductionScalarGemmWorkerV3OpenObligationV1::RustTypeLayoutContract,
    ProductionScalarGemmWorkerV3OpenObligationV1::RustEffectContract,
];

/// Authority obligation mechanically closed by the joined machine-analysis and Verus execution.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionScalarGemmWorkerV3ClosedObligationV1 {
    ProofExecutableBinding,
}

pub const PRODUCTION_SCALAR_GEMM_WORKER_V3_CLOSED_OBLIGATIONS_V1:
    [ProductionScalarGemmWorkerV3ClosedObligationV1; 1] =
    [ProductionScalarGemmWorkerV3ClosedObligationV1::ProofExecutableBinding];

/// Evidence that the exact scalar GEMM audit authenticates without granting authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1 {
    /// The generated marker, descriptor, gfx942 target, and code-object version agree.
    ExactRequestProfile,
    /// The complete semantic capsule matches its independently retained canonical identity.
    CanonicalSemanticCapsule,
    /// The formal-memory receipt matches its independently retained canonical identity.
    CanonicalFormalMemoryReceipt,
    /// The compiler proof-binding receipt matches its independently retained canonical identity.
    CanonicalCompilerProofBindingReceipt,
    /// The exact finalized HSACO bytes match their retained length and SHA-256.
    FinalizedHsacoIdentity,
    /// The final HSACO has the required target, COV6 metadata, and one exact descriptor.
    FinalizedHsacoStructure,
    /// The compiler receipts form the exact expected semantic-to-KIR association.
    CompilerProofAssociation,
    /// The associated KIR is the reviewed scalar GEMM profile.
    ExactScalarKernelIr,
    /// A retained Verus execution authenticates the challenge-bound scalar KIR proof.
    RetainedChallengeBoundVerusExecution,
    /// The upstream-LLVM analyzer ran under the exact caller-pinned executable and runtime policy.
    AuthenticatedPhysicalMachineEffectExecution,
    /// The analyzer result is the exact reviewed scalar gfx942 descriptor and static-site profile.
    ExactPhysicalMachineEffectProfile,
    /// Retained Verus directly checked the executable binding built from that machine occurrence.
    RetainedProofExecutableBinding,
}

/// Exact authoritative evidence still required at one scalar GEMM proof boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ProductionScalarGemmWorkerV3MissingEvidenceV1 {
    /// A measured compiler/rustc execution receipt bound to this compilation is absent.
    AuthenticatedCompilerExecution,
    /// An authenticated Rust source/MIR-to-KIR refinement receipt is absent.
    AuthenticatedSourceMirToKernelIrRefinement,
    /// An authenticated Rust-to-KIR IEEE-754 semantics receipt is absent.
    AuthenticatedRustIeeeF32Semantics,
    /// An authenticated KIR-to-final-gfx942-machine refinement receipt is absent.
    AuthenticatedKernelIrToMachineRefinement,
    /// An authenticated Rust type/layout-to-kernarg ABI receipt is absent.
    AuthenticatedRustTypeLayoutContract,
    /// An authenticated Rust-to-KIR-to-machine effect refinement receipt is absent.
    AuthenticatedEndToEndEffectContract,
}

/// One fail-closed authority assessment after a complete retained scalar GEMM audit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionScalarGemmWorkerV3ObligationStatusV1 {
    obligation: ProductionScalarGemmWorkerV3OpenObligationV1,
    authenticated_prerequisites: &'static [ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1],
    missing_evidence: ProductionScalarGemmWorkerV3MissingEvidenceV1,
}

impl ProductionScalarGemmWorkerV3ObligationStatusV1 {
    /// Returns the authority boundary being assessed.
    pub const fn obligation(self) -> ProductionScalarGemmWorkerV3OpenObligationV1 {
        self.obligation
    }

    /// Returns evidence authenticated by the exact audit before this boundary.
    ///
    /// These prerequisites are audit evidence only. They do not discharge the missing authority
    /// evidence and cannot be used independently to load or launch code.
    pub const fn authenticated_prerequisites(
        self,
    ) -> &'static [ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1] {
        self.authenticated_prerequisites
    }

    /// Returns the exact authoritative evidence still missing at this boundary.
    pub const fn missing_evidence(self) -> ProductionScalarGemmWorkerV3MissingEvidenceV1 {
        self.missing_evidence
    }

    /// No current scalar GEMM authority obligation is closed.
    pub const fn is_closed(self) -> bool {
        false
    }
}

const REQUEST_AND_CAPSULE_EVIDENCE: &[ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1] = &[
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::ExactRequestProfile,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::CanonicalSemanticCapsule,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::CanonicalCompilerProofBindingReceipt,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::CompilerProofAssociation,
];
const SOURCE_KIR_EVIDENCE: &[ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1] = &[
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::CanonicalSemanticCapsule,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::CompilerProofAssociation,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::ExactScalarKernelIr,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::RetainedChallengeBoundVerusExecution,
];
const MACHINE_EVIDENCE: &[ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1] = &[
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::ExactScalarKernelIr,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::FinalizedHsacoIdentity,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::FinalizedHsacoStructure,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::AuthenticatedPhysicalMachineEffectExecution,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::ExactPhysicalMachineEffectProfile,
];
const LAYOUT_EVIDENCE: &[ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1] = &[
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::ExactRequestProfile,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::CanonicalSemanticCapsule,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::FinalizedHsacoStructure,
];
const EFFECT_EVIDENCE: &[ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1] = &[
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::CanonicalFormalMemoryReceipt,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::ExactScalarKernelIr,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::RetainedChallengeBoundVerusExecution,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::FinalizedHsacoStructure,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::AuthenticatedPhysicalMachineEffectExecution,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::ExactPhysicalMachineEffectProfile,
    ProductionScalarGemmWorkerV3AuthenticatedEvidenceV1::RetainedProofExecutableBinding,
];

/// Stable, exhaustive scalar GEMM authority assessment after a complete retained audit.
pub const PRODUCTION_SCALAR_GEMM_WORKER_V3_OBLIGATION_STATUS_V1:
    [ProductionScalarGemmWorkerV3ObligationStatusV1; 6] = [
    ProductionScalarGemmWorkerV3ObligationStatusV1 {
        obligation: ProductionScalarGemmWorkerV3OpenObligationV1::CompilerExecutionProvenance,
        authenticated_prerequisites: REQUEST_AND_CAPSULE_EVIDENCE,
        missing_evidence:
            ProductionScalarGemmWorkerV3MissingEvidenceV1::AuthenticatedCompilerExecution,
    },
    ProductionScalarGemmWorkerV3ObligationStatusV1 {
        obligation: ProductionScalarGemmWorkerV3OpenObligationV1::SourceMirToKernelIrRefinement,
        authenticated_prerequisites: SOURCE_KIR_EVIDENCE,
        missing_evidence: ProductionScalarGemmWorkerV3MissingEvidenceV1::AuthenticatedSourceMirToKernelIrRefinement,
    },
    ProductionScalarGemmWorkerV3ObligationStatusV1 {
        obligation: ProductionScalarGemmWorkerV3OpenObligationV1::RustIeeeF32Semantics,
        authenticated_prerequisites: SOURCE_KIR_EVIDENCE,
        missing_evidence:
            ProductionScalarGemmWorkerV3MissingEvidenceV1::AuthenticatedRustIeeeF32Semantics,
    },
    ProductionScalarGemmWorkerV3ObligationStatusV1 {
        obligation: ProductionScalarGemmWorkerV3OpenObligationV1::EmittedMachineRefinement,
        authenticated_prerequisites: MACHINE_EVIDENCE,
        missing_evidence: ProductionScalarGemmWorkerV3MissingEvidenceV1::AuthenticatedKernelIrToMachineRefinement,
    },
    ProductionScalarGemmWorkerV3ObligationStatusV1 {
        obligation: ProductionScalarGemmWorkerV3OpenObligationV1::RustTypeLayoutContract,
        authenticated_prerequisites: LAYOUT_EVIDENCE,
        missing_evidence:
            ProductionScalarGemmWorkerV3MissingEvidenceV1::AuthenticatedRustTypeLayoutContract,
    },
    ProductionScalarGemmWorkerV3ObligationStatusV1 {
        obligation: ProductionScalarGemmWorkerV3OpenObligationV1::RustEffectContract,
        authenticated_prerequisites: EFFECT_EVIDENCE,
        missing_evidence:
            ProductionScalarGemmWorkerV3MissingEvidenceV1::AuthenticatedEndToEndEffectContract,
    },
];

/// Non-forgeable report returned only after the full request and retained Verus audit succeeds.
///
/// ```compile_fail
/// use fe2o3_worker_v3_authority::ProductionScalarGemmWorkerV3AuthorityClosureV1;
///
/// let forged = ProductionScalarGemmWorkerV3AuthorityClosureV1 { _private: () };
/// ```
#[derive(Debug)]
pub struct ProductionScalarGemmWorkerV3AuthorityClosureV1 {
    _private: (),
}

impl ProductionScalarGemmWorkerV3AuthorityClosureV1 {
    /// Returns the exhaustive, stable authority assessment.
    pub const fn obligation_statuses(
        &self,
    ) -> &'static [ProductionScalarGemmWorkerV3ObligationStatusV1] {
        &PRODUCTION_SCALAR_GEMM_WORKER_V3_OBLIGATION_STATUS_V1
    }

    /// The production verifier remains unavailable until every status is closed.
    pub const fn is_complete(&self) -> bool {
        false
    }

    /// An incomplete closure can never enter the Worker V3 authority gate.
    pub const fn can_enter_worker_v3_gate(&self) -> bool {
        false
    }

    /// An incomplete closure grants no artifact, load, or launch authority.
    pub const fn grants_artifact_or_runtime_authority(&self) -> bool {
        false
    }
}

/// Non-authoritative result of auditing one exact scalar GEMM Worker V3 request.
///
/// The result is move-only and remains bound to the host-derived challenge carried by its proof.
/// It cannot be converted into a host verification decision.
///
/// ```compile_fail
/// use fe2o3_worker_v3_authority::ProductionScalarGemmWorkerV3AuditV1;
///
/// fn require_clone<T: Clone>() {}
/// require_clone::<ProductionScalarGemmWorkerV3AuditV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_worker_v3_authority::ProductionScalarGemmWorkerV3AuditV1;
///
/// fn cannot_split(audit: ProductionScalarGemmWorkerV3AuditV1) {
///     let _ = audit.into_parts();
/// }
/// ```
#[derive(Debug)]
pub struct ProductionScalarGemmWorkerV3AuditV1 {
    proof: AuthenticatedScalarGemmWorkerV3ProofV3,
    machine_effect: AuthenticatedScalarGemmV1PhysicalMachineEffectEvidenceV1,
}

/// Exact, non-authoritative scalar GEMM request prepared for retained Verus execution.
///
/// Construction re-inspects the finalized HSACO, validates its exact descriptor, authenticates
/// the compiler proof association and canonical scalar KIR, and retains semantic proof state.
/// It cannot generate executable Verus input until authenticated machine evidence is joined. The
/// value is move-only and cannot be converted into host execution authority.
#[derive(Debug)]
#[must_use = "prepared Worker V3 proof state grants no authority and requires machine evidence"]
pub struct PreparedProductionScalarGemmWorkerV3ProofV1 {
    prepared: PreparedScalarGemmWorkerV3ProofV3,
    finalized_hsaco_sha256: [u8; 32],
    finalized_hsaco_length: u64,
}

impl PreparedProductionScalarGemmWorkerV3ProofV1 {
    /// Returns semantic proof state that cannot execute until machine evidence is joined.
    pub const fn prepared_proof(&self) -> &PreparedScalarGemmWorkerV3ProofV3 {
        &self.prepared
    }

    /// Moves out semantic proof state and the artifact identity validated with it.
    fn into_parts(self) -> (PreparedScalarGemmWorkerV3ProofV3, [u8; 32], u64) {
        (
            self.prepared,
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

    /// Returns the retained authenticated analyzer occurrence joined into the proof input.
    pub const fn machine_effect(
        &self,
    ) -> &AuthenticatedScalarGemmV1PhysicalMachineEffectEvidenceV1 {
        &self.machine_effect
    }

    /// Returns the finalized HSACO identity checked in the same retained audit call.
    pub const fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        self.proof
            .input()
            .executable_binding()
            .finalized_hsaco()
            .sha256()
    }

    /// Returns the finalized HSACO length checked in the same retained audit call.
    pub const fn finalized_hsaco_length(&self) -> u64 {
        self.proof
            .input()
            .executable_binding()
            .finalized_hsaco()
            .byte_len()
    }

    /// The exact authenticated analyzer occurrence is embedded in and checked by retained Verus.
    /// This is an identity binding, not compiler-to-machine semantic refinement.
    pub const fn establishes_proof_executable_binding(&self) -> bool {
        true
    }

    /// Returns the typed fail-closed authority assessment produced by this complete audit.
    pub const fn authority_closure(&self) -> ProductionScalarGemmWorkerV3AuthorityClosureV1 {
        ProductionScalarGemmWorkerV3AuthorityClosureV1 { _private: () }
    }

    /// Returns every authority obligation that remains open after this audit.
    pub const fn open_authority_obligations(
        &self,
    ) -> &'static [ProductionScalarGemmWorkerV3OpenObligationV1] {
        &PRODUCTION_SCALAR_GEMM_WORKER_V3_OPEN_OBLIGATIONS_V1
    }

    pub const fn closed_authority_obligations(
        &self,
    ) -> &'static [ProductionScalarGemmWorkerV3ClosedObligationV1] {
        &PRODUCTION_SCALAR_GEMM_WORKER_V3_CLOSED_OBLIGATIONS_V1
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
    machine_worker: AuthenticatedPhysicalMachineEffectWorkerV1,
    machine_limits: AuthenticatedPhysicalMachineEffectLimitsV1,
    timeout_seconds: u32,
    _marker: PhantomData<fn() -> K>,
}

impl<K> fmt::Debug for ProductionScalarGemmWorkerV3VerifierV1<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionScalarGemmWorkerV3VerifierV1")
            .field("runtime", &self.runtime)
            .field("machine_worker", &self.machine_worker)
            .field("machine_limits", &self.machine_limits)
            .field("timeout_seconds", &self.timeout_seconds)
            .finish_non_exhaustive()
    }
}

impl<K> ProductionScalarGemmWorkerV3VerifierV1<K> {
    /// Opens the exact protected Verus runtime closure used for every verification request.
    pub fn open(
        runtime_root: impl AsRef<Path>,
        timeout_seconds: u32,
        machine_worker_path: impl AsRef<Path>,
        machine_worker_policy: PhysicalMachineEffectWorkerPolicyV1,
        machine_limits: AuthenticatedPhysicalMachineEffectLimitsV1,
    ) -> Result<Self, ProductionScalarGemmWorkerV3VerifierErrorV1> {
        validate_timeout(timeout_seconds)?;
        let runtime = GeneralGemmVerusRuntimeClosureLeaseV2::open(runtime_root)
            .map_err(ProductionScalarGemmWorkerV3VerifierErrorV1::RuntimeClosure)?;
        let machine_worker = AuthenticatedPhysicalMachineEffectWorkerV1::open(
            machine_worker_path,
            machine_worker_policy,
            machine_limits,
        )
        .map_err(ProductionScalarGemmWorkerV3VerifierErrorV1::MachineWorker)?;
        Ok(Self {
            runtime,
            machine_worker,
            machine_limits,
            timeout_seconds,
            _marker: PhantomData,
        })
    }

    /// Constructs the verifier from an already retained runtime closure.
    pub fn from_retained(
        runtime: GeneralGemmVerusRuntimeClosureLeaseV2,
        machine_worker: AuthenticatedPhysicalMachineEffectWorkerV1,
        machine_limits: AuthenticatedPhysicalMachineEffectLimitsV1,
        timeout_seconds: u32,
    ) -> Result<Self, ProductionScalarGemmWorkerV3VerifierErrorV1> {
        validate_timeout(timeout_seconds)?;
        Ok(Self {
            runtime,
            machine_worker,
            machine_limits,
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
        let (prepared_proof, finalized_hsaco_sha256, finalized_hsaco_length) =
            prepared.into_parts();
        let profile = ScalarGemmV1PhysicalMachineEffectProfileV1::pinned(
            request.finalized_hsaco_bytes().to_vec(),
        )
        .map_err(ProductionScalarGemmWorkerV3VerifierErrorV1::MachineEffect)?;
        let machine_effect = profile
            .analyze(&self.machine_worker, self.machine_limits)
            .map_err(ProductionScalarGemmWorkerV3VerifierErrorV1::MachineEffect)?;
        let executable_binding = build_executable_binding(request, &self.runtime, &machine_effect)?;
        let proof_input =
            build_scalar_gemm_worker_v3_proof_input_v3(prepared_proof, executable_binding)
                .map_err(ProductionScalarGemmWorkerV3VerifierErrorV1::ProofInput)?;
        let proof = execute_scalar_gemm_worker_v3_proof_v3(
            &self.runtime,
            proof_input,
            self.timeout_seconds,
        )
        .map_err(ProductionScalarGemmWorkerV3VerifierErrorV1::ProofExecution)?;
        validate_retained_proof(request, &machine_effect, &proof)?;
        if proof
            .input()
            .executable_binding()
            .finalized_hsaco()
            .sha256()
            != finalized_hsaco_sha256
            || proof
                .input()
                .executable_binding()
                .finalized_hsaco()
                .byte_len()
                != finalized_hsaco_length
        {
            return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::RetainedProofInvariant);
        }
        Ok(ProductionScalarGemmWorkerV3AuditV1 {
            proof,
            machine_effect,
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
    let prepared = prepare_scalar_gemm_worker_v3_proof_v3(
        *request.challenge_identity().as_bytes(),
        *request.lineage_identity().as_bytes(),
        request.generated_host_contract_identity(),
        &association,
        &scalar_kir,
    )
    .map_err(ProductionScalarGemmWorkerV3VerifierErrorV1::ProofInput)?;
    Ok(PreparedProductionScalarGemmWorkerV3ProofV1 {
        prepared,
        finalized_hsaco_sha256: request.finalized_hsaco_sha256(),
        finalized_hsaco_length: request.finalized_hsaco_length(),
    })
}

fn build_executable_binding<K: CompilerGeneratedKernelExpectationV1>(
    request: &WorkerV3VerificationRequestV1<'_, K>,
    runtime: &GeneralGemmVerusRuntimeClosureLeaseV2,
    machine_effect: &AuthenticatedScalarGemmV1PhysicalMachineEffectEvidenceV1,
) -> Result<ScalarGemmWorkerV3ExecutableBindingV1, ProductionScalarGemmWorkerV3VerifierErrorV1> {
    let execution = machine_effect.authenticated_execution();
    let evidence = machine_effect.evidence();
    let machine_request = execution.request();
    let payload = machine_effect.finalized_hsaco_identity();
    if payload.sha256() != request.finalized_hsaco_sha256()
        || payload.byte_len() != request.finalized_hsaco_length()
        || machine_request.payload_identity() != payload
        || evidence.payload_identity() != payload
    {
        return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::MachineBindingInvariant);
    }
    let [entry] = evidence.entry_points() else {
        return Err(ProductionScalarGemmWorkerV3VerifierErrorV1::MachineBindingInvariant);
    };
    let effects = evidence
        .effects()
        .iter()
        .map(|effect| {
            ScalarGemmWorkerV3MachineEffectSiteV1::new(
                effect.instruction_offset(),
                match effect.kind() {
                    PhysicalMachineEffectKindV1::GlobalAddress => {
                        ScalarGemmWorkerV3MachineEffectKindV1::GlobalAddress
                    }
                    PhysicalMachineEffectKindV1::GlobalRead => {
                        ScalarGemmWorkerV3MachineEffectKindV1::GlobalRead
                    }
                    PhysicalMachineEffectKindV1::GlobalWrite => {
                        ScalarGemmWorkerV3MachineEffectKindV1::GlobalWrite
                    }
                    PhysicalMachineEffectKindV1::Return => {
                        ScalarGemmWorkerV3MachineEffectKindV1::Return
                    }
                },
                effect.byte_width(),
            )
        })
        .collect();
    let request_identity = machine_request.identity();
    let evidence_identity = evidence.identity();
    let receipt_identity = execution.identity();
    let policy = execution.policy();
    let worker_executable = policy.executable();
    let runtime_closure = policy.runtime_closure();
    let runtime_mapping = execution.runtime_mapping_identity();
    ScalarGemmWorkerV3ExecutableBindingV1::new(ScalarGemmWorkerV3ExecutableBindingComponentsV1 {
        finalized_hsaco: measured(payload.sha256(), payload.byte_len()),
        logical_descriptor_identity: *request.descriptor().kernel_id().as_bytes(),
        raw_descriptor_identity: machine_effect.descriptor_identity().as_bytes(),
        machine_execution_challenge: execution.execution_challenge().as_bytes(),
        analyzer_identity: evidence.analyzer_identity().as_bytes(),
        toolchain_identity: evidence.toolchain_identity().as_bytes(),
        machine_request_identity: measured(request_identity.sha256(), request_identity.byte_len()),
        machine_evidence_identity: measured(
            evidence_identity.sha256(),
            evidence_identity.byte_len(),
        ),
        authenticated_receipt_identity: measured(
            receipt_identity.sha256(),
            receipt_identity.byte_len(),
        ),
        worker_executable_identity: measured(
            worker_executable.sha256(),
            worker_executable.byte_len(),
        ),
        machine_runtime_closure_identity: measured(
            runtime_closure.sha256(),
            runtime_closure.byte_len(),
        ),
        machine_runtime_mapping_identity: measured(
            runtime_mapping.sha256(),
            runtime_mapping.byte_len(),
        ),
        verus_runtime_closure_identity: runtime.identity().as_bytes(),
        entry_code_offset: entry.code_offset(),
        entry_code_size: entry.code_size(),
        effects,
        canonical_machine_request: machine_request.canonical_bytes().to_vec(),
        canonical_machine_evidence: evidence.canonical_bytes().to_vec(),
        canonical_authenticated_receipt: execution.canonical_receipt_bytes().to_vec(),
    })
    .map_err(ProductionScalarGemmWorkerV3VerifierErrorV1::ExecutableBinding)
}

const fn measured(sha256: [u8; 32], byte_len: u64) -> ScalarGemmWorkerV3MeasuredIdentityV1 {
    ScalarGemmWorkerV3MeasuredIdentityV1::new(sha256, byte_len)
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
    machine_effect: &AuthenticatedScalarGemmV1PhysicalMachineEffectEvidenceV1,
    proof: &AuthenticatedScalarGemmWorkerV3ProofV3,
) -> Result<(), ProductionScalarGemmWorkerV3VerifierErrorV1> {
    let execution = machine_effect.authenticated_execution();
    let machine_request = execution.request();
    let evidence = execution.evidence();
    let policy = execution.policy();
    let binding = proof.input().executable_binding();
    let request_identity = machine_request.identity();
    let evidence_identity = evidence.identity();
    let receipt_identity = execution.identity();
    let worker_identity = policy.executable();
    let runtime_closure_identity = policy.runtime_closure();
    let runtime_mapping_identity = execution.runtime_mapping_identity();
    if proof.input().challenge() != *request.challenge_identity().as_bytes()
        || proof.input().lineage_identity() != *request.lineage_identity().as_bytes()
        || proof.input().generated_host_contract_identity()
            != request.generated_host_contract_identity()
        || binding.finalized_hsaco().sha256() != request.finalized_hsaco_sha256()
        || binding.finalized_hsaco().byte_len() != request.finalized_hsaco_length()
        || binding.logical_descriptor_identity() != *request.descriptor().kernel_id().as_bytes()
        || binding.raw_descriptor_identity() != machine_effect.descriptor_identity().as_bytes()
        || binding.machine_execution_challenge() != execution.execution_challenge().as_bytes()
        || binding.analyzer_identity() != evidence.analyzer_identity().as_bytes()
        || binding.toolchain_identity() != evidence.toolchain_identity().as_bytes()
        || binding.machine_request_identity().sha256() != request_identity.sha256()
        || binding.machine_request_identity().byte_len() != request_identity.byte_len()
        || binding.machine_evidence_identity().sha256() != evidence_identity.sha256()
        || binding.machine_evidence_identity().byte_len() != evidence_identity.byte_len()
        || binding.authenticated_receipt_identity().sha256() != receipt_identity.sha256()
        || binding.authenticated_receipt_identity().byte_len() != receipt_identity.byte_len()
        || binding.worker_executable_identity().sha256() != worker_identity.sha256()
        || binding.worker_executable_identity().byte_len() != worker_identity.byte_len()
        || binding.machine_runtime_closure_identity().sha256() != runtime_closure_identity.sha256()
        || binding.machine_runtime_closure_identity().byte_len()
            != runtime_closure_identity.byte_len()
        || binding.machine_runtime_mapping_identity().sha256() != runtime_mapping_identity.sha256()
        || binding.machine_runtime_mapping_identity().byte_len()
            != runtime_mapping_identity.byte_len()
        || binding.canonical_machine_request() != machine_request.canonical_bytes()
        || binding.canonical_machine_evidence() != evidence.canonical_bytes()
        || binding.canonical_authenticated_receipt() != execution.canonical_receipt_bytes()
        || !proof.authenticates_retained_verus_execution()
        || !proof.binds_worker_v3_challenge()
        || !proof.binds_exact_executable_machine_profile()
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
    /// The caller-pinned upstream-LLVM machine worker could not be retained or authenticated.
    MachineWorker(AuthenticatedPhysicalMachineEffectErrorV1),
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
    /// Exact scalar machine-effect analysis or profile validation failed.
    MachineEffect(ScalarGemmV1PhysicalMachineEffectErrorV1),
    /// Authenticated machine evidence could not form the reviewed executable proof input.
    ExecutableBinding(ScalarGemmWorkerV3ExecutableBindingErrorV1),
    /// Authenticated request, evidence, artifact, or descriptor identities disagreed.
    MachineBindingInvariant,
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
            Self::MachineWorker(error) => {
                write!(formatter, "machine-effect worker rejected: {error}")
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
            Self::MachineEffect(error) => {
                write!(
                    formatter,
                    "scalar machine-effect evidence rejected: {error}"
                )
            }
            Self::ExecutableBinding(error) => {
                write!(
                    formatter,
                    "scalar executable proof binding rejected: {error}"
                )
            }
            Self::MachineBindingInvariant => formatter
                .write_str("authenticated scalar machine evidence disagrees with the request"),
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
            Self::MachineWorker(error) => Some(error),
            Self::ScalarDescriptor(error) => Some(error),
            Self::FinalizedInspection(error) => Some(error),
            Self::CompilerProofBinding(error) => Some(error),
            Self::ScalarKernelIr(error) => Some(error),
            Self::MachineEffect(error) => Some(error),
            Self::ExecutableBinding(error) => Some(error),
            Self::ProofInput(error) => Some(error),
            Self::ProofExecution(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use fe2o3_kernel_analysis::{
        PhysicalMachineAnalyzerIdentityV1, PhysicalMachineRuntimeClosureIdentityV1,
        PhysicalMachineToolchainIdentityV1, PhysicalMachineWorkerExecutableIdentityV1,
    };

    fn test_machine_policy() -> PhysicalMachineEffectWorkerPolicyV1 {
        PhysicalMachineEffectWorkerPolicyV1::new(
            PhysicalMachineWorkerExecutableIdentityV1::from_parts([1; 32], 1),
            PhysicalMachineRuntimeClosureIdentityV1::from_parts([2; 32], 2),
            PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([3; 32]),
            PhysicalMachineToolchainIdentityV1::from_sha256_bytes([4; 32]),
        )
        .unwrap()
    }

    fn test_machine_limits() -> AuthenticatedPhysicalMachineEffectLimitsV1 {
        AuthenticatedPhysicalMachineEffectLimitsV1::new(Duration::from_secs(1), 1024, 1024).unwrap()
    }

    #[test]
    fn zero_and_excessive_timeouts_fail_before_runtime_admission() {
        for timeout in [0, MAX_SCALAR_GEMM_VERUS_TIMEOUT_SECONDS_V2 + 1] {
            assert!(matches!(
                ProductionScalarGemmWorkerV3VerifierV1::<()>::open(
                    "relative-path-is-never-opened",
                    timeout,
                    "relative-machine-worker-is-never-opened",
                    test_machine_policy(),
                    test_machine_limits(),
                ),
                Err(ProductionScalarGemmWorkerV3VerifierErrorV1::InvalidTimeout)
            ));
        }
    }

    #[test]
    fn unprotected_runtime_paths_fail_closed() {
        assert!(matches!(
            ProductionScalarGemmWorkerV3VerifierV1::<()>::open(
                "relative-path-is-rejected",
                1,
                "relative-machine-worker-is-never-opened",
                test_machine_policy(),
                test_machine_limits(),
            ),
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
                ProductionScalarGemmWorkerV3OpenObligationV1::RustTypeLayoutContract,
                ProductionScalarGemmWorkerV3OpenObligationV1::RustEffectContract,
            ]
        );
        assert_eq!(
            PRODUCTION_SCALAR_GEMM_WORKER_V3_CLOSED_OBLIGATIONS_V1,
            [ProductionScalarGemmWorkerV3ClosedObligationV1::ProofExecutableBinding]
        );
    }

    #[test]
    fn obligation_statuses_are_exhaustive_unique_and_fail_closed() {
        use std::collections::BTreeSet;

        let statuses = &PRODUCTION_SCALAR_GEMM_WORKER_V3_OBLIGATION_STATUS_V1;
        assert_eq!(
            statuses.len(),
            PRODUCTION_SCALAR_GEMM_WORKER_V3_OPEN_OBLIGATIONS_V1.len()
        );
        assert_eq!(
            statuses.map(ProductionScalarGemmWorkerV3ObligationStatusV1::obligation),
            PRODUCTION_SCALAR_GEMM_WORKER_V3_OPEN_OBLIGATIONS_V1
        );
        assert_eq!(
            statuses
                .iter()
                .map(|status| status.obligation())
                .collect::<BTreeSet<_>>()
                .len(),
            statuses.len()
        );
        assert_eq!(
            statuses
                .iter()
                .map(|status| status.missing_evidence())
                .collect::<BTreeSet<_>>()
                .len(),
            statuses.len()
        );
        for status in statuses {
            assert!(!status.authenticated_prerequisites().is_empty());
            assert!(!status.is_closed());
        }
    }

    #[test]
    fn closure_report_cannot_promote_authenticated_prerequisites_to_authority() {
        let closure = ProductionScalarGemmWorkerV3AuthorityClosureV1 { _private: () };
        assert_eq!(
            closure.obligation_statuses(),
            &PRODUCTION_SCALAR_GEMM_WORKER_V3_OBLIGATION_STATUS_V1
        );
        assert!(!closure.is_complete());
        assert!(!closure.can_enter_worker_v3_gate());
        assert!(!closure.grants_artifact_or_runtime_authority());
    }
}
