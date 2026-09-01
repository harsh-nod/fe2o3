use std::{error::Error, fmt, marker::PhantomData};

use fe2o3_artifact_transaction::{
    DurableCurrentLinkPublicationTokenV1, InertCompilerExecutionSubjectV1,
};
use fe2o3_hsaco::{CodeObjectVersion, InspectedKernel, KernelDescriptorBinding};
use fe2o3_hsaco_finalize::{
    RevalidatedProtectedWorkerV3FinalizerDerivationV1, WorkerV3HsacoPublicationErrorV1,
    revalidate_protected_worker_v3_finalizer_derivation_v1,
};
use fe2o3_kernel_descriptor::{BlockSizeV1, DeviceDescriptorTableV1, KernelDescriptorV1, KernelId};
use fe2o3_runtime_protocol::{CompilerExecutionReceiptCarriageV1, WorkerV3LoadEnvelopeWireV1};
use fe2o3_verifier::{
    CompilerMultiRootProofValidationErrorV1, CompilerProofInputValidationErrorV4,
    CompilerTargetLineageValidationErrorV1, ValidatedCompilerMultiRootProofInputsV1,
    ValidatedCompilerMultiRootTargetLineageV1, ValidatedCompilerProofInputsV4,
    ValidatedCompilerTargetLineageV1, validate_compiler_multi_root_proof_inputs_v1,
    validate_compiler_multi_root_target_lineage_v1, validate_compiler_proof_inputs_v4,
    validate_compiler_target_lineage_v1,
};
use sha2::{Digest, Sha256};

#[cfg(target_os = "linux")]
use crate::compiler_execution_current_record_audit::WorkerV3CompilerCurrentRecordAuditV1;
use crate::recovered_worker_v3_admission::WorkerV3HostLineageEvidenceV1;
use crate::{
    CompilerGeneratedKernelExpectationRosterEntryV1, CompilerGeneratedKernelExpectationRosterV1,
    CompilerGeneratedKernelExpectationV1, RecoveredWorkerV3AdmissionErrorV1,
    RecoveredWorkerV3PinnedDescriptorV1, RecoveredWorkerV3PinnedRosterV1,
    WorkerV3HostLineageIdentityV1,
};

const WORKER_V3_VERIFICATION_CHALLENGE_DOMAIN_V1: &[u8] =
    b"fe2o3.host.worker-v3-verification-challenge.v1\0";
const WORKER_V3_ROSTER_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.host.worker-v3-verification-roster.v1\0";
const WORKER_V3_ROSTER_VERIFICATION_CHALLENGE_DOMAIN_V1: &[u8] =
    b"fe2o3.host.worker-v3-roster-verification-challenge.v1\0";

mod verifier_seal {
    pub trait Sealed<K> {}
}

/// Safety property established by a reviewed V3 verifier for one exact executable lineage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3SafetyPropertyV1 {
    Bounds,
    AddressOverflowFreedom,
    MemorySafety,
    Initialization,
    RaceFreedom,
    LaunchValidity,
    Synchronization,
    SemanticRefinement,
}

impl WorkerV3SafetyPropertyV1 {
    const fn bit(self) -> u8 {
        match self {
            Self::Bounds => 1 << 0,
            Self::AddressOverflowFreedom => 1 << 1,
            Self::MemorySafety => 1 << 2,
            Self::Initialization => 1 << 3,
            Self::RaceFreedom => 1 << 4,
            Self::LaunchValidity => 1 << 5,
            Self::Synchronization => 1 << 6,
            Self::SemanticRefinement => 1 << 7,
        }
    }
}

/// Canonical set of properties reported through the reviewed V3 verifier boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3SafetyPropertiesV1(u8);

impl WorkerV3SafetyPropertiesV1 {
    const KNOWN_BITS: u8 = u8::MAX;

    pub const fn new(bits: u8) -> Option<Self> {
        // V1 assigns every bit in its u8 wire representation. Keep the fallible constructor
        // signature stable so a later wire version can reject unknown bits without API churn.
        Some(Self(bits))
    }

    pub const fn required() -> Self {
        Self(Self::KNOWN_BITS)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains(self, property: WorkerV3SafetyPropertyV1) -> bool {
        self.0 & property.bit() != 0
    }
}

/// Exact marker-specific challenge over one independently admitted V3 lineage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV3VerificationChallengeIdentityV1([u8; 32]);

impl WorkerV3VerificationChallengeIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Borrowed request presented to a reviewed V3 compiler/Verus verifier.
pub struct WorkerV3VerificationRequestV1<'admission, K> {
    challenge: WorkerV3VerificationChallengeIdentityV1,
    lineage: WorkerV3HostLineageEvidenceV1,
    finalizer_derivation: &'admission RevalidatedProtectedWorkerV3FinalizerDerivationV1,
    finalizer_replay: &'admission WorkerV3LoadEnvelopeWireV1,
    compiler_execution_subject: &'admission InertCompilerExecutionSubjectV1,
    compiler_execution_receipt: &'admission CompilerExecutionReceiptCarriageV1,
    handoff: &'admission fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
    finalized_hsaco: &'admission [u8],
    descriptor: &'admission KernelDescriptorV1,
    target: fe2o3_amd_target::AmdTargetId,
    code_object_version: CodeObjectVersion,
    generated_host_contract: [u8; 32],
    _marker: PhantomData<fn() -> K>,
}

impl<K: CompilerGeneratedKernelExpectationV1> WorkerV3VerificationRequestV1<'_, K> {
    pub const fn challenge_identity(&self) -> WorkerV3VerificationChallengeIdentityV1 {
        self.challenge
    }

    pub const fn lineage_identity(&self) -> WorkerV3HostLineageIdentityV1 {
        self.lineage.identity()
    }

    /// Returns the finalizer derivation independently reconstructed by host admission.
    pub const fn finalizer_derivation(&self) -> &RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
        self.finalizer_derivation
    }

    /// Reconstructs a second move-only finalizer owner from the exact borrowed envelope bytes.
    ///
    /// Protected verifier backends use this operation instead of echoing host projections. The
    /// result remains authority-free and must still be compared at decision promotion.
    pub fn independently_revalidate_finalizer_derivation(
        &self,
    ) -> Result<RevalidatedProtectedWorkerV3FinalizerDerivationV1, WorkerV3HsacoPublicationErrorV1>
    {
        revalidate_protected_worker_v3_finalizer_derivation_v1(
            self.finalizer_replay.publication_intent_record().attempt(),
            self.finalizer_replay.outer_handoff(),
            self.finalizer_replay.external_provider_payloads(),
            self.finalizer_replay.transcript(),
            self.finalized_hsaco,
        )
    }

    /// Returns the finalizer identity bound into the complete host-lineage challenge.
    pub const fn finalizer_derivation_sha256(&self) -> [u8; 32] {
        self.lineage.finalizer_derivation_sha256()
    }

    /// Returns the exact canonical compiler occurrence reconstructed from durable V3 replay.
    pub const fn compiler_execution_subject(&self) -> &InertCompilerExecutionSubjectV1 {
        self.compiler_execution_subject
    }

    /// Returns the complete canonical compiler occurrence bytes.
    pub const fn compiler_execution_subject_bytes(&self) -> &[u8] {
        self.compiler_execution_subject.canonical_bytes()
    }

    /// Returns the complete receipt carriage retained by the V2 production envelope.
    ///
    /// The carriage is internally consistent but inert. A reviewed verifier must compare its
    /// policy with protected configuration, reacquire its exact Worker ledger record, and establish
    /// that its rollback position is current before reporting authenticated compiler execution.
    pub const fn compiler_execution_receipt_carriage(&self) -> &CompilerExecutionReceiptCarriageV1 {
        self.compiler_execution_receipt
    }

    /// Returns the exact canonical receipt carriage bytes, without a projected schema.
    pub const fn compiler_execution_receipt_bytes(&self) -> &[u8] {
        self.compiler_execution_receipt.canonical_bytes()
    }

    pub const fn compiler_execution_subject_sha256(&self) -> [u8; 32] {
        *self.compiler_execution_subject.identity().sha256()
    }

    pub const fn compiler_execution_carriage_sha256(&self) -> [u8; 32] {
        *self.compiler_execution_receipt.identity().as_bytes()
    }

    pub const fn compiler_execution_policy_sha256(&self) -> [u8; 32] {
        *self
            .compiler_execution_receipt
            .policy()
            .identity()
            .as_bytes()
    }

    pub const fn compiler_execution_issuer_journal_sha256(&self) -> [u8; 32] {
        self.compiler_execution_receipt
            .acknowledgment()
            .issuer_journal_identity()
    }

    pub const fn compiler_occurrence_sha256(&self) -> [u8; 32] {
        self.compiler_execution_receipt
            .acknowledgment()
            .compiler_occurrence_identity()
    }

    pub const fn compiler_execution_receipt_sha256(&self) -> [u8; 32] {
        *self
            .compiler_execution_receipt
            .acknowledgment()
            .receipt_identity()
            .as_bytes()
    }

    pub const fn compiler_execution_publication_sha256(&self) -> [u8; 32] {
        *self
            .compiler_execution_receipt
            .acknowledgment()
            .publication_identity()
            .as_bytes()
    }

    pub const fn compiler_execution_acknowledgment_sha256(&self) -> [u8; 32] {
        *self
            .compiler_execution_receipt
            .acknowledgment()
            .identity()
            .as_bytes()
    }

    pub const fn compiler_execution_worker_ledger_record_sha256(&self) -> [u8; 32] {
        self.compiler_execution_receipt
            .acknowledgment()
            .worker_ledger_record_identity()
    }

    pub const fn compiler_execution_sequence(&self) -> u64 {
        self.compiler_execution_receipt.acknowledgment().sequence()
    }

    pub const fn compiler_execution_prior_rollback_anchor(&self) -> [u8; 32] {
        self.compiler_execution_receipt
            .publication()
            .receipt()
            .prior_rollback_anchor()
    }

    pub const fn compiler_execution_current_rollback_anchor(&self) -> [u8; 32] {
        self.compiler_execution_receipt
            .acknowledgment()
            .current_rollback_anchor()
    }

    pub const fn descriptor(&self) -> &KernelDescriptorV1 {
        self.descriptor
    }

    /// Returns the exact canonical compiler handoff retained by host admission.
    ///
    /// The handoff is inert content, not compiler or proof authority. A reviewed verifier uses it
    /// to decode every stage receipt instead of trusting request-level digest projections.
    pub const fn semantic_compiler_handoff(
        &self,
    ) -> &fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3 {
        self.handoff
    }

    /// Returns the complete canonical semantic-capsule bytes presented to the verifier.
    pub fn semantic_capsule_bytes(&self) -> &[u8] {
        self.handoff.capsule().canonical_bytes()
    }

    /// Returns the exact canonical formal-memory receipt, not only its digest.
    pub fn formal_memory_receipt_bytes(&self) -> &[u8] {
        self.handoff
            .capsule()
            .receipts()
            .formal_memory()
            .canonical_preimage()
    }

    /// Returns the exact canonical proof-binding receipt, not only its digest.
    pub fn proof_binding_receipt_bytes(&self) -> &[u8] {
        self.handoff
            .capsule()
            .receipts()
            .proof_binding()
            .canonical_preimage()
    }

    /// Independently decodes and cross-checks every exact compiler proof input retained by the
    /// recovered capsule.
    ///
    /// The move-only result establishes canonical stage ownership, structural MIR-to-KIR
    /// association, and strict import of the exact signed aggregate MIR-to-live-PLIRON receipt
    /// under its embedded key. Protected compiler origin, LLVM/machine refinement, and runtime
    /// authority remain separate required joins.
    pub fn validate_compiler_proof_inputs_v4(
        &self,
    ) -> Result<ValidatedCompilerProofInputsV4, CompilerProofInputValidationErrorV4> {
        let receipts = self.handoff.capsule().receipts();
        validate_compiler_proof_inputs_v4(
            receipts.proof_binding(),
            receipts.semantic_mir(),
            receipts.middle_end(),
            receipts.kernel_ir(),
            receipts.mir_to_kir_correspondence(),
            receipts.formal_memory(),
        )
    }

    /// Independently decodes every singleton target-side association and replays KIR-to-LLVM.
    ///
    /// The caller must first obtain `proof_inputs` from [`Self::validate_compiler_proof_inputs_v4`].
    /// The returned move-only owner establishes exact association and deterministic replay only;
    /// semantic refinement, LLVM-to-machine refinement, and runtime authority remain separate.
    pub fn validate_compiler_target_lineage_v1(
        &self,
        proof_inputs: &ValidatedCompilerProofInputsV4,
    ) -> Result<ValidatedCompilerTargetLineageV1, CompilerTargetLineageValidationErrorV1> {
        validate_compiler_target_lineage_v1(self.handoff.capsule(), proof_inputs)
    }

    /// Returns the exact finalized HSACO bytes retained by the current-publication token.
    ///
    /// The host keeps that token alive for the complete verifier call and revalidates it before
    /// promoting the returned decision. A reviewed verifier must use these bytes, rather than a
    /// caller-supplied path or digest projection, for executable inspection and machine refinement.
    pub const fn finalized_hsaco_bytes(&self) -> &[u8] {
        self.finalized_hsaco
    }

    pub const fn capsule_sha256(&self) -> [u8; 32] {
        self.lineage.capsule_sha256()
    }

    pub const fn formal_memory_receipt_sha256(&self) -> [u8; 32] {
        self.lineage.formal_memory_sha256()
    }

    pub const fn proof_binding_receipt_sha256(&self) -> [u8; 32] {
        self.lineage.proof_binding_sha256()
    }

    pub const fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        self.lineage.finalized_sha256()
    }

    pub const fn finalized_hsaco_length(&self) -> u64 {
        self.lineage.finalized_length()
    }

    pub const fn target(&self) -> fe2o3_amd_target::AmdTargetId {
        self.target
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub const fn marker_logical_name(&self) -> &'static str {
        K::LOGICAL_NAME
    }

    pub const fn marker_export_name(&self) -> &'static str {
        K::EXPORT_NAME
    }

    pub const fn marker_binding_identity(&self) -> [u8; 32] {
        K::KERNEL_BINDING_ID_V1
    }

    pub const fn generated_host_contract_identity(&self) -> [u8; 32] {
        self.generated_host_contract
    }
}

/// Reviewed boundary that authenticates a real V3 compiler and Verus result.
///
/// # Safety
///
/// Implementations must authenticate immutable compiler and verifier executions under an
/// approved policy. They must compare the carried compiler-execution policy with independently
/// retained protected configuration, reacquire and match the exact protected Worker ledger record,
/// and establish the carried sequence and rollback anchor as current through an external protected
/// anti-rollback authority. They must also independently reconstruct the exact finalizer derivation
/// from the borrowed durable envelope bytes and retain that move-only owner in their result rather
/// than echoing the host admission identity. The resulting policy, ledger, rollback, and finalizer
/// verification identities must
/// bind the exact subject, carriage, compiler occurrence, and complete verification transcript;
/// they must not be copied or derived solely from request fields. Implementations must also
/// establish that the formal-memory and proof-binding receipts apply to this exact semantic
/// capsule, descriptor, final HSACO, and generated Rust marker, and that every reported safety
/// property covers all executable memory effects for every concrete invocation satisfying the
/// generated ABI, effect, alias, initialization, and launch contracts.
/// This is a universally quantified kernel theorem: the later safe composition boundary may
/// instantiate it only with compiler-generated capabilities and independently checked physical
/// runtime inputs. The inert V3 receipts do not establish these claims by themselves. A false
/// implementation can later authorize native code loading from safe generated code.
pub unsafe trait WorkerV3VerifierV1<K: CompilerGeneratedKernelExpectationV1>:
    verifier_seal::Sealed<K>
{
    type Error;

    /// Authenticates one exact request and returns independently checked identities.
    ///
    /// # Safety
    ///
    /// The implementation obligations are those of the unsafe trait. Returned identities must
    /// derive from authenticated executions and proof artifacts, never from untrusted request
    /// fields alone.
    unsafe fn verify(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<WorkerV3VerificationDecisionV1, Self::Error>;
}

/// Independently authenticated result returned by a protected verifier backend.
///
/// This value is consumed only by [`WorkerV3ProtectedVerifierAdapterV1`]. It carries the
/// move-only V4 proof inputs, including the imported signed aggregate Verus evidence, plus the
/// identities and universally quantified safety properties that cannot be derived by host
/// admission. The sealed adapter supplies every request-coordinate field directly from the exact
/// pinned host request and the existing promotion boundary compares the complete decision again.
pub struct WorkerV3ProtectedVerificationEvidenceV1 {
    finalizer_derivation: RevalidatedProtectedWorkerV3FinalizerDerivationV1,
    compiler_execution: WorkerV3CompilerExecutionVerificationV1,
    proof_inputs: ValidatedCompilerProofInputsV4,
    target_lineage: ValidatedCompilerTargetLineageV1,
    verifier_measurement_sha256: [u8; 32],
    verification_transcript_sha256: [u8; 32],
    proof_executable_binding_sha256: [u8; 32],
    rust_type_layout_contract_sha256: [u8; 32],
    rust_effect_contract_sha256: [u8; 32],
    safety_properties: WorkerV3SafetyPropertiesV1,
}

impl WorkerV3ProtectedVerificationEvidenceV1 {
    /// Constructs evidence produced by one reviewed protected backend execution.
    ///
    /// # Safety
    ///
    /// Every identity must be derived from independently authenticated protected state and bind
    /// the exact request passed to [`WorkerV3ProtectedVerifierBackendV1::verify_protected`]. The
    /// caller must satisfy the complete backend trait contract; nonzero or request-echoed values
    /// alone do not satisfy it.
    #[allow(clippy::too_many_arguments)]
    pub const unsafe fn new(
        finalizer_derivation: RevalidatedProtectedWorkerV3FinalizerDerivationV1,
        compiler_execution: WorkerV3CompilerExecutionVerificationV1,
        proof_inputs: ValidatedCompilerProofInputsV4,
        target_lineage: ValidatedCompilerTargetLineageV1,
        verifier_measurement_sha256: [u8; 32],
        verification_transcript_sha256: [u8; 32],
        proof_executable_binding_sha256: [u8; 32],
        rust_type_layout_contract_sha256: [u8; 32],
        rust_effect_contract_sha256: [u8; 32],
        safety_properties: WorkerV3SafetyPropertiesV1,
    ) -> Self {
        Self {
            finalizer_derivation,
            compiler_execution,
            proof_inputs,
            target_lineage,
            verifier_measurement_sha256,
            verification_transcript_sha256,
            proof_executable_binding_sha256,
            rust_type_layout_contract_sha256,
            rust_effect_contract_sha256,
            safety_properties,
        }
    }
}

/// External authority boundary used only through fe2o3's sealed production adapter.
///
/// # Safety
///
/// Implementations must run as a reviewed protected verifier and satisfy every obligation of
/// [`WorkerV3VerifierV1`]. In particular, they must compare independently retained compiler policy,
/// reacquire the exact protected Worker ledger record, enforce external rollback currentness, and
/// authenticate the proof-to-executable, Rust layout, Rust effect, and universal safety results.
/// Returned evidence must bind the exact borrowed request and may not be synthesized from request
/// fields. An invalid implementation can authorize native code loading from safe generated code.
pub unsafe trait WorkerV3ProtectedVerifierBackendV1<K: CompilerGeneratedKernelExpectationV1> {
    type Error;

    /// Authenticates one pinned host request using independent protected state.
    ///
    /// # Safety
    ///
    /// The implementation obligations are those of the unsafe trait.
    unsafe fn verify_protected(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<WorkerV3ProtectedVerificationEvidenceV1, Self::Error>;
}

/// Crate-owned sealed verifier that delegates only independent protected checks.
///
/// Construction is safe because it grants no authority by itself. Authentication remains gated by
/// the unsafe backend contract, exact request-coordinate construction below, decision validation,
/// and post-verifier current-publication revalidation.
pub struct WorkerV3ProtectedVerifierAdapterV1<B> {
    backend: B,
}

impl<B> WorkerV3ProtectedVerifierAdapterV1<B> {
    /// Wraps one reviewed protected backend without invoking it.
    pub const fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Returns the retained backend after the adapter is no longer needed.
    pub fn into_inner(self) -> B {
        self.backend
    }
}

impl<K, B> verifier_seal::Sealed<K> for WorkerV3ProtectedVerifierAdapterV1<B>
where
    K: CompilerGeneratedKernelExpectationV1,
    B: WorkerV3ProtectedVerifierBackendV1<K>,
{
}

// SAFETY: the adapter is crate-owned and sealed. Its only external authority boundary is the
// explicit unsafe protected-backend trait. It fills all request coordinates from the exact pinned
// host request and the caller validates the complete decision plus publication currentness.
unsafe impl<K, B> WorkerV3VerifierV1<K> for WorkerV3ProtectedVerifierAdapterV1<B>
where
    K: CompilerGeneratedKernelExpectationV1,
    B: WorkerV3ProtectedVerifierBackendV1<K>,
{
    type Error = B::Error;

    unsafe fn verify(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<WorkerV3VerificationDecisionV1, Self::Error> {
        // SAFETY: `B` owns the independent protected checks required by its unsafe trait.
        let evidence = unsafe { self.backend.verify_protected(request)? };
        Ok(WorkerV3VerificationDecisionV1::new(
            request.challenge_identity(),
            request.lineage_identity(),
            request.descriptor().kernel_id(),
            request.marker_binding_identity(),
            request.generated_host_contract_identity(),
            request.capsule_sha256(),
            request.formal_memory_receipt_sha256(),
            request.proof_binding_receipt_sha256(),
            request.finalized_hsaco_sha256(),
            request.finalized_hsaco_length(),
            request.target(),
            request.code_object_version(),
            evidence.finalizer_derivation,
            evidence.compiler_execution,
            evidence.proof_inputs,
            evidence.target_lineage,
            evidence.verifier_measurement_sha256,
            evidence.verification_transcript_sha256,
            evidence.proof_executable_binding_sha256,
            evidence.rust_type_layout_contract_sha256,
            evidence.rust_effect_contract_sha256,
            evidence.safety_properties,
        ))
    }
}

/// Explicit synthetic-verifier hook for the receipt-bearing integration harness.
///
/// This trait is absent from default and production builds. Enabling it permits downstream test
/// code to satisfy the otherwise sealed verifier boundary and must never be used to construct a
/// production application or authority claim.
#[cfg(feature = "worker-v3-verifier-test-support")]
#[doc(hidden)]
pub unsafe trait WorkerV3SyntheticVerifierV1<K: CompilerGeneratedKernelExpectationV1> {
    type Error;

    /// Returns synthetic descriptive evidence for hostile transition testing only.
    ///
    /// # Safety
    ///
    /// The implementation must remain confined to a test-only build and must not represent its
    /// result as protected compiler, proof, rollback, load, or launch authority.
    unsafe fn verify_synthetic(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<WorkerV3VerificationDecisionV1, Self::Error>;
}

/// Explicit test-only adapter for a synthetic verifier implementation.
#[cfg(feature = "worker-v3-verifier-test-support")]
#[doc(hidden)]
pub struct WorkerV3SyntheticVerifierAdapterV1<V> {
    verifier: V,
}

#[cfg(feature = "worker-v3-verifier-test-support")]
impl<V> WorkerV3SyntheticVerifierAdapterV1<V> {
    /// Wraps one synthetic verifier for test-only authentication.
    pub const fn new(verifier: V) -> Self {
        Self { verifier }
    }

    /// Returns the retained synthetic verifier.
    pub fn into_inner(self) -> V {
        self.verifier
    }
}

#[cfg(feature = "worker-v3-verifier-test-support")]
impl<K, V> verifier_seal::Sealed<K> for WorkerV3SyntheticVerifierAdapterV1<V>
where
    K: CompilerGeneratedKernelExpectationV1,
    V: WorkerV3SyntheticVerifierV1<K>,
{
}

#[cfg(feature = "worker-v3-verifier-test-support")]
// SAFETY: this explicit wrapper exists only under the test-support feature and is disjoint from
// the production adapter. The wrapped verifier must satisfy the unsafe synthetic trait contract.
unsafe impl<K, V> WorkerV3VerifierV1<K> for WorkerV3SyntheticVerifierAdapterV1<V>
where
    K: CompilerGeneratedKernelExpectationV1,
    V: WorkerV3SyntheticVerifierV1<K>,
{
    type Error = V::Error;

    unsafe fn verify(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<WorkerV3VerificationDecisionV1, Self::Error> {
        // SAFETY: the caller is inside the unsafe production verifier transition and the explicit
        // test-support implementation owns the synthetic invariants for this test-only build.
        unsafe { self.verifier.verify_synthetic(request) }
    }
}

/// Non-authoritative review of one exact V3 verification request.
///
/// Implementing this safe trait cannot grant load or launch authority. The host retains admission
/// custody, pins the current publication for the complete call, and returns only caller-defined
/// evidence after revalidating currentness.
pub trait WorkerV3AuditorV1<K: CompilerGeneratedKernelExpectationV1> {
    type Error;
    type Evidence;

    /// Audits exact request bytes without constructing a verification decision.
    fn audit(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<Self::Evidence, Self::Error>;
}

/// Move-only compiler-execution result returned by a reviewed protected verifier.
///
/// The exact coordinates must come from independently authenticated protected state, not merely
/// from copying an inert request. The final three identities bind the verifier's protected-policy
/// comparison, exact Worker-ledger reacquisition, and external rollback-currentness decision.
/// Production construction consumes the one-use signed current-record evidence after comparing
/// every coordinate with the exact subject and carriage. The retained evidence is still
/// authority-free until the final verifier joins protected deployment trust and all refinement
/// receipts.
#[derive(Debug)]
pub struct WorkerV3CompilerExecutionVerificationV1 {
    subject_sha256: [u8; 32],
    carriage_sha256: [u8; 32],
    policy_sha256: [u8; 32],
    issuer_journal_sha256: [u8; 32],
    compiler_occurrence_sha256: [u8; 32],
    receipt_sha256: [u8; 32],
    publication_sha256: [u8; 32],
    acknowledgment_sha256: [u8; 32],
    worker_ledger_record_sha256: [u8; 32],
    sequence: u64,
    prior_rollback_anchor: [u8; 32],
    current_rollback_anchor: [u8; 32],
    current_record_verification_sha256: [u8; 32],
    current_record_attestation_sha256: [u8; 32],
    protected_policy_verification_sha256: [u8; 32],
    protected_worker_ledger_verification_sha256: [u8; 32],
    external_rollback_verification_sha256: [u8; 32],
    _evidence: WorkerV3CompilerExecutionEvidenceV1,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum WorkerV3CompilerExecutionEvidenceV1 {
    #[cfg(target_os = "linux")]
    CurrentRecord(WorkerV3CompilerCurrentRecordAuditV1),
    #[cfg(feature = "worker-v3-verifier-test-support")]
    Synthetic,
}

impl WorkerV3CompilerExecutionVerificationV1 {
    #[cfg(target_os = "linux")]
    pub(crate) fn from_current_record_audit(
        subject: &InertCompilerExecutionSubjectV1,
        carriage: &CompilerExecutionReceiptCarriageV1,
        evidence: WorkerV3CompilerCurrentRecordAuditV1,
    ) -> Result<Self, WorkerV3CompilerExecutionEvidenceErrorV1> {
        if carriage.request().subject() != subject {
            return Err(WorkerV3CompilerExecutionEvidenceErrorV1::RequestMismatch);
        }
        let verification = evidence.verification();
        for (matches, field) in [
            (
                verification.subject_identity() == *subject.identity().sha256(),
                "compiler-execution subject",
            ),
            (
                verification.carriage_identity() == *carriage.identity().as_bytes(),
                "compiler-execution carriage",
            ),
            (
                verification.policy_identity() == *carriage.policy().identity().as_bytes(),
                "compiler-execution policy",
            ),
            (
                verification.issuer_journal_identity()
                    == carriage.acknowledgment().issuer_journal_identity(),
                "compiler-execution issuer journal",
            ),
            (
                verification.worker_ledger_record_identity()
                    == carriage.acknowledgment().worker_ledger_record_identity(),
                "compiler-execution Worker ledger record",
            ),
            (
                verification.sequence() == carriage.acknowledgment().sequence(),
                "compiler-execution rollback sequence",
            ),
            (
                verification.prior_rollback_anchor()
                    == carriage.publication().receipt().prior_rollback_anchor(),
                "compiler-execution prior rollback anchor",
            ),
            (
                verification.current_rollback_anchor()
                    == carriage.acknowledgment().current_rollback_anchor(),
                "compiler-execution current rollback anchor",
            ),
        ] {
            if !matches {
                return Err(WorkerV3CompilerExecutionEvidenceErrorV1::IdentityMismatch(
                    field,
                ));
            }
        }
        for (authenticated, field) in [
            (
                evidence.authenticates_pinned_signing_key(),
                "pinned compiler current-record signing key",
            ),
            (
                evidence.authenticates_expected_fresh_challenge(),
                "fresh compiler current-record challenge",
            ),
            (
                evidence.authenticates_external_anchor_commit(),
                "external rollback commit",
            ),
            (
                evidence.authenticates_external_rollback_currentness(),
                "external rollback currentness",
            ),
        ] {
            if !authenticated {
                return Err(
                    WorkerV3CompilerExecutionEvidenceErrorV1::MissingAuthenticatedEvidence(field),
                );
            }
        }
        for (identity, field) in [
            (
                verification.protected_policy_verification_identity(),
                "protected compiler policy verification",
            ),
            (
                verification.protected_worker_ledger_verification_identity(),
                "protected Worker ledger verification",
            ),
            (
                evidence.external_rollback_verification_identity(),
                "external rollback verification",
            ),
        ] {
            if identity == [0; 32] {
                return Err(
                    WorkerV3CompilerExecutionEvidenceErrorV1::MissingAuthenticatedEvidence(field),
                );
            }
        }

        Ok(Self {
            subject_sha256: *subject.identity().sha256(),
            carriage_sha256: *carriage.identity().as_bytes(),
            policy_sha256: *carriage.policy().identity().as_bytes(),
            issuer_journal_sha256: carriage.acknowledgment().issuer_journal_identity(),
            compiler_occurrence_sha256: carriage.acknowledgment().compiler_occurrence_identity(),
            receipt_sha256: *carriage.acknowledgment().receipt_identity().as_bytes(),
            publication_sha256: *carriage.acknowledgment().publication_identity().as_bytes(),
            acknowledgment_sha256: *carriage.acknowledgment().identity().as_bytes(),
            worker_ledger_record_sha256: carriage.acknowledgment().worker_ledger_record_identity(),
            sequence: carriage.acknowledgment().sequence(),
            prior_rollback_anchor: carriage.publication().receipt().prior_rollback_anchor(),
            current_rollback_anchor: carriage.acknowledgment().current_rollback_anchor(),
            current_record_verification_sha256: *verification.identity().as_bytes(),
            current_record_attestation_sha256: *evidence.attestation_identity().as_bytes(),
            protected_policy_verification_sha256: verification
                .protected_policy_verification_identity(),
            protected_worker_ledger_verification_sha256: verification
                .protected_worker_ledger_verification_identity(),
            external_rollback_verification_sha256: evidence
                .external_rollback_verification_identity(),
            _evidence: WorkerV3CompilerExecutionEvidenceV1::CurrentRecord(evidence),
        })
    }

    /// Constructs synthetic coordinates only for the explicit integration-test verifier seam.
    #[cfg(feature = "worker-v3-verifier-test-support")]
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub const fn synthetic_for_test_only(
        subject_sha256: [u8; 32],
        carriage_sha256: [u8; 32],
        policy_sha256: [u8; 32],
        issuer_journal_sha256: [u8; 32],
        compiler_occurrence_sha256: [u8; 32],
        receipt_sha256: [u8; 32],
        publication_sha256: [u8; 32],
        acknowledgment_sha256: [u8; 32],
        worker_ledger_record_sha256: [u8; 32],
        sequence: u64,
        prior_rollback_anchor: [u8; 32],
        current_rollback_anchor: [u8; 32],
        current_record_verification_sha256: [u8; 32],
        current_record_attestation_sha256: [u8; 32],
        protected_policy_verification_sha256: [u8; 32],
        protected_worker_ledger_verification_sha256: [u8; 32],
        external_rollback_verification_sha256: [u8; 32],
    ) -> Self {
        Self {
            subject_sha256,
            carriage_sha256,
            policy_sha256,
            issuer_journal_sha256,
            compiler_occurrence_sha256,
            receipt_sha256,
            publication_sha256,
            acknowledgment_sha256,
            worker_ledger_record_sha256,
            sequence,
            prior_rollback_anchor,
            current_rollback_anchor,
            current_record_verification_sha256,
            current_record_attestation_sha256,
            protected_policy_verification_sha256,
            protected_worker_ledger_verification_sha256,
            external_rollback_verification_sha256,
            _evidence: WorkerV3CompilerExecutionEvidenceV1::Synthetic,
        }
    }

    pub const fn subject_sha256(&self) -> [u8; 32] {
        self.subject_sha256
    }

    pub const fn carriage_sha256(&self) -> [u8; 32] {
        self.carriage_sha256
    }

    pub const fn policy_sha256(&self) -> [u8; 32] {
        self.policy_sha256
    }

    pub const fn issuer_journal_sha256(&self) -> [u8; 32] {
        self.issuer_journal_sha256
    }

    pub const fn compiler_occurrence_sha256(&self) -> [u8; 32] {
        self.compiler_occurrence_sha256
    }

    pub const fn receipt_sha256(&self) -> [u8; 32] {
        self.receipt_sha256
    }

    pub const fn publication_sha256(&self) -> [u8; 32] {
        self.publication_sha256
    }

    pub const fn acknowledgment_sha256(&self) -> [u8; 32] {
        self.acknowledgment_sha256
    }

    pub const fn worker_ledger_record_sha256(&self) -> [u8; 32] {
        self.worker_ledger_record_sha256
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn prior_rollback_anchor(&self) -> [u8; 32] {
        self.prior_rollback_anchor
    }

    pub const fn current_rollback_anchor(&self) -> [u8; 32] {
        self.current_rollback_anchor
    }

    pub const fn current_record_verification_sha256(&self) -> [u8; 32] {
        self.current_record_verification_sha256
    }

    pub const fn current_record_attestation_sha256(&self) -> [u8; 32] {
        self.current_record_attestation_sha256
    }

    pub const fn protected_policy_verification_sha256(&self) -> [u8; 32] {
        self.protected_policy_verification_sha256
    }

    pub const fn protected_worker_ledger_verification_sha256(&self) -> [u8; 32] {
        self.protected_worker_ledger_verification_sha256
    }

    pub const fn external_rollback_verification_sha256(&self) -> [u8; 32] {
        self.external_rollback_verification_sha256
    }

    /// Reports only whether this lane owns the signed fresh-currentness evidence.
    ///
    /// This does not establish protected service deployment, semantic refinement, or final verifier
    /// authority.
    pub const fn authenticates_signed_currentness_evidence(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            match &self._evidence {
                WorkerV3CompilerExecutionEvidenceV1::CurrentRecord(evidence) => {
                    evidence.authenticates_pinned_signing_key()
                        && evidence.authenticates_expected_fresh_challenge()
                        && evidence.authenticates_external_anchor_commit()
                        && evidence.authenticates_external_rollback_currentness()
                }
                #[cfg(feature = "worker-v3-verifier-test-support")]
                WorkerV3CompilerExecutionEvidenceV1::Synthetic => false,
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    pub const fn grants_verification_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3CompilerExecutionEvidenceErrorV1 {
    RequestMismatch,
    IdentityMismatch(&'static str),
    MissingAuthenticatedEvidence(&'static str),
}

/// Descriptive result returned by a reviewed V3 verifier.
///
/// Public construction grants no authority. Only the private promotion transition can compare
/// every field to an admitted request and retain it as authenticated state.
#[derive(Debug)]
pub struct WorkerV3VerificationDecisionV1 {
    challenge: WorkerV3VerificationChallengeIdentityV1,
    lineage: WorkerV3HostLineageIdentityV1,
    kernel_id: KernelId,
    marker_binding: [u8; 32],
    generated_host_contract: [u8; 32],
    capsule_sha256: [u8; 32],
    formal_memory_sha256: [u8; 32],
    proof_binding_sha256: [u8; 32],
    finalized_sha256: [u8; 32],
    finalized_length: u64,
    target: fe2o3_amd_target::AmdTargetId,
    code_object_version: CodeObjectVersion,
    finalizer_derivation: RevalidatedProtectedWorkerV3FinalizerDerivationV1,
    compiler_execution: WorkerV3CompilerExecutionVerificationV1,
    proof_inputs: WorkerV3ProofInputEvidenceV1,
    target_lineage: WorkerV3TargetLineageEvidenceV1,
    verifier_measurement_sha256: [u8; 32],
    verification_transcript_sha256: [u8; 32],
    proof_executable_binding_sha256: [u8; 32],
    rust_type_layout_contract_sha256: [u8; 32],
    rust_effect_contract_sha256: [u8; 32],
    safety_properties: WorkerV3SafetyPropertiesV1,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum WorkerV3ProofInputEvidenceV1 {
    Validated(ValidatedCompilerProofInputsV4),
    #[cfg(feature = "worker-v3-verifier-test-support")]
    Synthetic,
}

#[derive(Debug)]
enum WorkerV3TargetLineageEvidenceV1 {
    Validated(Box<ValidatedCompilerTargetLineageV1>),
    #[cfg(feature = "worker-v3-verifier-test-support")]
    Synthetic,
}

impl WorkerV3VerificationDecisionV1 {
    #[allow(
        dead_code,
        reason = "reserved for the crate-owned production verifier; synthetic builds enter through the explicitly gated constructor"
    )]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        challenge: WorkerV3VerificationChallengeIdentityV1,
        lineage: WorkerV3HostLineageIdentityV1,
        kernel_id: KernelId,
        marker_binding: [u8; 32],
        generated_host_contract: [u8; 32],
        capsule_sha256: [u8; 32],
        formal_memory_sha256: [u8; 32],
        proof_binding_sha256: [u8; 32],
        finalized_sha256: [u8; 32],
        finalized_length: u64,
        target: fe2o3_amd_target::AmdTargetId,
        code_object_version: CodeObjectVersion,
        finalizer_derivation: RevalidatedProtectedWorkerV3FinalizerDerivationV1,
        compiler_execution: WorkerV3CompilerExecutionVerificationV1,
        proof_inputs: ValidatedCompilerProofInputsV4,
        target_lineage: ValidatedCompilerTargetLineageV1,
        verifier_measurement_sha256: [u8; 32],
        verification_transcript_sha256: [u8; 32],
        proof_executable_binding_sha256: [u8; 32],
        rust_type_layout_contract_sha256: [u8; 32],
        rust_effect_contract_sha256: [u8; 32],
        safety_properties: WorkerV3SafetyPropertiesV1,
    ) -> Self {
        Self::new_with_evidence(
            challenge,
            lineage,
            kernel_id,
            marker_binding,
            generated_host_contract,
            capsule_sha256,
            formal_memory_sha256,
            proof_binding_sha256,
            finalized_sha256,
            finalized_length,
            target,
            code_object_version,
            finalizer_derivation,
            compiler_execution,
            WorkerV3ProofInputEvidenceV1::Validated(proof_inputs),
            WorkerV3TargetLineageEvidenceV1::Validated(Box::new(target_lineage)),
            verifier_measurement_sha256,
            verification_transcript_sha256,
            proof_executable_binding_sha256,
            rust_type_layout_contract_sha256,
            rust_effect_contract_sha256,
            safety_properties,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_evidence(
        challenge: WorkerV3VerificationChallengeIdentityV1,
        lineage: WorkerV3HostLineageIdentityV1,
        kernel_id: KernelId,
        marker_binding: [u8; 32],
        generated_host_contract: [u8; 32],
        capsule_sha256: [u8; 32],
        formal_memory_sha256: [u8; 32],
        proof_binding_sha256: [u8; 32],
        finalized_sha256: [u8; 32],
        finalized_length: u64,
        target: fe2o3_amd_target::AmdTargetId,
        code_object_version: CodeObjectVersion,
        finalizer_derivation: RevalidatedProtectedWorkerV3FinalizerDerivationV1,
        compiler_execution: WorkerV3CompilerExecutionVerificationV1,
        proof_inputs: WorkerV3ProofInputEvidenceV1,
        target_lineage: WorkerV3TargetLineageEvidenceV1,
        verifier_measurement_sha256: [u8; 32],
        verification_transcript_sha256: [u8; 32],
        proof_executable_binding_sha256: [u8; 32],
        rust_type_layout_contract_sha256: [u8; 32],
        rust_effect_contract_sha256: [u8; 32],
        safety_properties: WorkerV3SafetyPropertiesV1,
    ) -> Self {
        Self {
            challenge,
            lineage,
            kernel_id,
            marker_binding,
            generated_host_contract,
            capsule_sha256,
            formal_memory_sha256,
            proof_binding_sha256,
            finalized_sha256,
            finalized_length,
            target,
            code_object_version,
            finalizer_derivation,
            compiler_execution,
            proof_inputs,
            target_lineage,
            verifier_measurement_sha256,
            verification_transcript_sha256,
            proof_executable_binding_sha256,
            rust_type_layout_contract_sha256,
            rust_effect_contract_sha256,
            safety_properties,
        }
    }

    /// Constructs a descriptive decision only for the explicit integration-test verifier seam.
    #[cfg(feature = "worker-v3-verifier-test-support")]
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn synthetic_for_test_only(
        challenge: WorkerV3VerificationChallengeIdentityV1,
        lineage: WorkerV3HostLineageIdentityV1,
        kernel_id: KernelId,
        marker_binding: [u8; 32],
        generated_host_contract: [u8; 32],
        capsule_sha256: [u8; 32],
        formal_memory_sha256: [u8; 32],
        proof_binding_sha256: [u8; 32],
        finalized_sha256: [u8; 32],
        finalized_length: u64,
        target: fe2o3_amd_target::AmdTargetId,
        code_object_version: CodeObjectVersion,
        finalizer_derivation: RevalidatedProtectedWorkerV3FinalizerDerivationV1,
        compiler_execution: WorkerV3CompilerExecutionVerificationV1,
        verifier_measurement_sha256: [u8; 32],
        verification_transcript_sha256: [u8; 32],
        proof_executable_binding_sha256: [u8; 32],
        rust_type_layout_contract_sha256: [u8; 32],
        rust_effect_contract_sha256: [u8; 32],
        safety_properties: WorkerV3SafetyPropertiesV1,
    ) -> Self {
        Self::new_with_evidence(
            challenge,
            lineage,
            kernel_id,
            marker_binding,
            generated_host_contract,
            capsule_sha256,
            formal_memory_sha256,
            proof_binding_sha256,
            finalized_sha256,
            finalized_length,
            target,
            code_object_version,
            finalizer_derivation,
            compiler_execution,
            WorkerV3ProofInputEvidenceV1::Synthetic,
            WorkerV3TargetLineageEvidenceV1::Synthetic,
            verifier_measurement_sha256,
            verification_transcript_sha256,
            proof_executable_binding_sha256,
            rust_type_layout_contract_sha256,
            rust_effect_contract_sha256,
            safety_properties,
        )
    }

    pub const fn challenge_identity(&self) -> WorkerV3VerificationChallengeIdentityV1 {
        self.challenge
    }

    pub const fn lineage_identity(&self) -> WorkerV3HostLineageIdentityV1 {
        self.lineage
    }

    pub const fn safety_properties(&self) -> WorkerV3SafetyPropertiesV1 {
        self.safety_properties
    }

    pub const fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        self.finalized_sha256
    }

    pub const fn finalized_hsaco_length(&self) -> u64 {
        self.finalized_length
    }

    pub const fn compiler_execution(&self) -> &WorkerV3CompilerExecutionVerificationV1 {
        &self.compiler_execution
    }

    /// Returns the independently reconstructed finalizer owner retained by this decision.
    pub const fn finalizer_derivation(&self) -> &RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
        &self.finalizer_derivation
    }

    /// Returns exact decoded compiler proof inputs for a default-build production decision.
    ///
    /// The explicit synthetic test feature returns `None`; that lane never represents decoded
    /// proof-input authority.
    pub const fn validated_compiler_proof_inputs(&self) -> Option<&ValidatedCompilerProofInputsV4> {
        match &self.proof_inputs {
            WorkerV3ProofInputEvidenceV1::Validated(inputs) => Some(inputs),
            #[cfg(feature = "worker-v3-verifier-test-support")]
            WorkerV3ProofInputEvidenceV1::Synthetic => None,
        }
    }

    /// Returns independently decoded singleton target lineage for a production decision.
    ///
    /// The explicit synthetic test lane returns `None` and carries no target-lineage claim.
    pub const fn validated_compiler_target_lineage(
        &self,
    ) -> Option<&ValidatedCompilerTargetLineageV1> {
        match &self.target_lineage {
            WorkerV3TargetLineageEvidenceV1::Validated(lineage) => Some(lineage),
            #[cfg(feature = "worker-v3-verifier-test-support")]
            WorkerV3TargetLineageEvidenceV1::Synthetic => None,
        }
    }

    /// Reports custody of both current compiler-execution evidence and the independently imported
    /// signed aggregate MIR-to-live-PLIRON receipt. This does not establish LLVM or machine
    /// refinement and grants no runtime authority.
    pub const fn retains_current_compiler_and_signed_verus_evidence(&self) -> bool {
        match (&self.proof_inputs, &self.target_lineage) {
            (
                WorkerV3ProofInputEvidenceV1::Validated(inputs),
                WorkerV3TargetLineageEvidenceV1::Validated(target),
            ) => {
                inputs.authenticates_signed_verus_receipt_under_embedded_key()
                    && target.has_exact_receipt_association()
                    && target.has_exact_kir_to_llvm_replay()
                    && self
                        .compiler_execution
                        .authenticates_signed_currentness_evidence()
            }
            #[cfg(feature = "worker-v3-verifier-test-support")]
            _ => false,
        }
    }
}

/// Authenticated compiler/Verus state for one exact V3 executable.
///
/// This value is linear and still grants no load or launch authority. A later runtime-specific
/// transition must bind it to a checked live device. The exact current-publication token acquired
/// before verifier entry remains owned here until the complete runtime authority is consumed.
pub struct AuthenticatedWorkerV3ExecutableV1<K> {
    admission: RecoveredWorkerV3PinnedDescriptorV1,
    current: DurableCurrentLinkPublicationTokenV1,
    verification: WorkerV3VerificationDecisionV1,
    _marker: PhantomData<fn() -> K>,
}

impl<K> fmt::Debug for AuthenticatedWorkerV3ExecutableV1<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedWorkerV3ExecutableV1")
            .field("lineage", &self.verification.lineage)
            .field("kernel_id", &self.verification.kernel_id)
            .finish_non_exhaustive()
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> AuthenticatedWorkerV3ExecutableV1<K> {
    pub fn authenticate<V: WorkerV3VerifierV1<K>>(
        admission: RecoveredWorkerV3PinnedDescriptorV1,
        verifier: &mut V,
    ) -> Result<Self, WorkerV3VerificationAuthenticationErrorV1<V::Error>> {
        let current = admission
            .acquire_retained_currentness_token()
            .map_err(WorkerV3VerificationAuthenticationErrorV1::CurrentPublication)?;
        let request = prepare_request::<K>(&admission, &current).map_err(|error| match error {
            WorkerV3VerificationRequestPreparationErrorV1::Marker(field) => {
                WorkerV3VerificationAuthenticationErrorV1::Marker(field)
            }
            WorkerV3VerificationRequestPreparationErrorV1::UnsupportedGeneratedProfile => {
                WorkerV3VerificationAuthenticationErrorV1::UnsupportedGeneratedProfile
            }
        })?;
        // SAFETY: safe callers cannot implement the verifier trait. Every returned field is
        // independently compared to the exact admitted request below.
        let verification = unsafe { verifier.verify(&request) };
        admission
            .revalidate_retained_currentness_token(&current)
            .map_err(WorkerV3VerificationAuthenticationErrorV1::CurrentPublication)?;
        let verification =
            verification.map_err(WorkerV3VerificationAuthenticationErrorV1::Verifier)?;
        validate_decision::<K>(&request, &verification)
            .map_err(WorkerV3VerificationAuthenticationErrorV1::Decision)?;
        Ok(Self {
            admission,
            current,
            verification,
            _marker: PhantomData,
        })
    }

    pub const fn verification(&self) -> &WorkerV3VerificationDecisionV1 {
        &self.verification
    }

    pub fn descriptor(&self) -> &KernelDescriptorV1 {
        self.admission.descriptor()
    }

    pub fn target(&self) -> fe2o3_amd_target::AmdTargetId {
        self.admission.target()
    }

    pub fn revalidate_currentness(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.admission
            .revalidate_retained_currentness_token(&self.current)
    }

    #[cfg(feature = "qualification-legacy-hip-hsa")]
    pub fn authorize_hsa_load<A: crate::ReviewedHsaExecutableLifecycleAdapterV1>(
        self,
        observed: crate::ObservedContext,
        adapter: A,
    ) -> Result<
        crate::AuthorizedWorkerV3HsaLoadV1<K, A>,
        crate::WorkerV3HsaLoadAuthorizationErrorV1<A::Error>,
    > {
        crate::hsa_executable_lifecycle::authorize_worker_v3_hsa_load_v1(self, observed, adapter)
    }

    pub const fn authenticates_verification_authority(&self) -> bool {
        true
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub(crate) const fn admission(&self) -> &RecoveredWorkerV3PinnedDescriptorV1 {
        &self.admission
    }

    pub(crate) const fn current_publication_token(&self) -> &DurableCurrentLinkPublicationTokenV1 {
        &self.current
    }
}

/// Canonical identity of one complete generated marker roster.
///
/// The identity covers exact descriptor-table order, names, marker bindings, and generated host
/// contracts. Physical ELF kernel order is deliberately outside this identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV3VerificationRosterIdentityV1([u8; 32]);

impl WorkerV3VerificationRosterIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// One aggregate challenge over a complete admitted artifact and marker roster.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV3RosterVerificationChallengeIdentityV1([u8; 32]);

impl WorkerV3RosterVerificationChallengeIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Borrowed aggregate request presented once for one complete recovered artifact.
///
/// V4 compiler proof inputs are common capsule evidence. They do not independently establish a
/// proof-to-executable, Rust layout, or Rust effect theorem for each roster entry. The protected
/// aggregate backend must establish those per-entry joins separately. It must also independently
/// reconstruct the finalizer derivation from the exact replay; a host-projected digest is not
/// finalizer custody.
pub struct WorkerV3RosterVerificationRequestV1<'admission, R> {
    challenge: WorkerV3RosterVerificationChallengeIdentityV1,
    roster_identity: WorkerV3VerificationRosterIdentityV1,
    admission: &'admission RecoveredWorkerV3PinnedRosterV1<R>,
    current: &'admission DurableCurrentLinkPublicationTokenV1,
    _roster: PhantomData<fn() -> R>,
}

impl<R: CompilerGeneratedKernelExpectationRosterV1> WorkerV3RosterVerificationRequestV1<'_, R> {
    pub const fn challenge_identity(&self) -> WorkerV3RosterVerificationChallengeIdentityV1 {
        self.challenge
    }

    pub const fn roster_identity(&self) -> WorkerV3VerificationRosterIdentityV1 {
        self.roster_identity
    }

    pub const fn lineage_identity(&self) -> WorkerV3HostLineageIdentityV1 {
        self.admission.lineage_identity()
    }

    /// Returns the derivation independently reconstructed by host roster admission.
    pub const fn finalizer_derivation(&self) -> &RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
        self.admission.finalizer_derivation()
    }

    /// Reconstructs a second move-only finalizer owner from the exact borrowed envelope replay.
    ///
    /// Protected aggregate backends use this operation instead of echoing host projections. The
    /// returned owner remains authority-free and is compared again during decision promotion.
    pub fn independently_revalidate_finalizer_derivation(
        &self,
    ) -> Result<RevalidatedProtectedWorkerV3FinalizerDerivationV1, WorkerV3HsacoPublicationErrorV1>
    {
        let replay = self.admission.finalizer_replay();
        revalidate_protected_worker_v3_finalizer_derivation_v1(
            replay.publication_intent_record().attempt(),
            replay.outer_handoff(),
            replay.external_provider_payloads(),
            replay.transcript(),
            self.finalized_hsaco_bytes(),
        )
    }

    /// Returns the finalizer identity bound into every entry lineage and the roster lineage.
    pub const fn finalizer_derivation_sha256(&self) -> [u8; 32] {
        self.admission
            .lineage_evidence()
            .finalizer_derivation_sha256()
    }

    pub fn marker_entries(&self) -> &'static [CompilerGeneratedKernelExpectationRosterEntryV1] {
        R::ENTRIES
    }

    pub fn descriptor_table(&self) -> &DeviceDescriptorTableV1 {
        self.admission.descriptor_table()
    }

    pub fn descriptor(&self, ordinal: usize) -> Option<&KernelDescriptorV1> {
        self.admission.descriptor(ordinal)
    }

    pub fn physical_kernel(&self, ordinal: usize) -> Option<&InspectedKernel> {
        self.admission.physical_kernel(ordinal)
    }

    pub fn descriptor_binding(&self, ordinal: usize) -> Option<KernelDescriptorBinding> {
        self.admission.descriptor_binding(ordinal)
    }

    pub fn entry_lineage_identity(&self, ordinal: usize) -> Option<WorkerV3HostLineageIdentityV1> {
        self.admission
            .entrypoints()
            .get(ordinal)
            .map(|entrypoint| entrypoint.lineage_identity())
    }

    pub const fn compiler_execution_subject(&self) -> &InertCompilerExecutionSubjectV1 {
        self.admission.compiler_execution_subject()
    }

    pub const fn compiler_execution_subject_bytes(&self) -> &[u8] {
        self.compiler_execution_subject().canonical_bytes()
    }

    pub const fn compiler_execution_receipt_carriage(&self) -> &CompilerExecutionReceiptCarriageV1 {
        self.admission.compiler_execution_receipt()
    }

    pub const fn compiler_execution_receipt_bytes(&self) -> &[u8] {
        self.compiler_execution_receipt_carriage().canonical_bytes()
    }

    pub const fn compiler_execution_subject_sha256(&self) -> [u8; 32] {
        *self.compiler_execution_subject().identity().sha256()
    }

    pub const fn compiler_execution_carriage_sha256(&self) -> [u8; 32] {
        *self
            .compiler_execution_receipt_carriage()
            .identity()
            .as_bytes()
    }

    pub const fn compiler_execution_policy_sha256(&self) -> [u8; 32] {
        *self
            .compiler_execution_receipt_carriage()
            .policy()
            .identity()
            .as_bytes()
    }

    pub const fn compiler_execution_issuer_journal_sha256(&self) -> [u8; 32] {
        self.compiler_execution_receipt_carriage()
            .acknowledgment()
            .issuer_journal_identity()
    }

    pub const fn compiler_occurrence_sha256(&self) -> [u8; 32] {
        self.compiler_execution_receipt_carriage()
            .acknowledgment()
            .compiler_occurrence_identity()
    }

    pub const fn compiler_execution_receipt_sha256(&self) -> [u8; 32] {
        *self
            .compiler_execution_receipt_carriage()
            .acknowledgment()
            .receipt_identity()
            .as_bytes()
    }

    pub const fn compiler_execution_publication_sha256(&self) -> [u8; 32] {
        *self
            .compiler_execution_receipt_carriage()
            .acknowledgment()
            .publication_identity()
            .as_bytes()
    }

    pub const fn compiler_execution_acknowledgment_sha256(&self) -> [u8; 32] {
        *self
            .compiler_execution_receipt_carriage()
            .acknowledgment()
            .identity()
            .as_bytes()
    }

    pub const fn compiler_execution_worker_ledger_record_sha256(&self) -> [u8; 32] {
        self.compiler_execution_receipt_carriage()
            .acknowledgment()
            .worker_ledger_record_identity()
    }

    pub const fn compiler_execution_sequence(&self) -> u64 {
        self.compiler_execution_receipt_carriage()
            .acknowledgment()
            .sequence()
    }

    pub const fn compiler_execution_prior_rollback_anchor(&self) -> [u8; 32] {
        self.compiler_execution_receipt_carriage()
            .publication()
            .receipt()
            .prior_rollback_anchor()
    }

    pub const fn compiler_execution_current_rollback_anchor(&self) -> [u8; 32] {
        self.compiler_execution_receipt_carriage()
            .acknowledgment()
            .current_rollback_anchor()
    }

    pub const fn semantic_compiler_handoff(
        &self,
    ) -> &fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3 {
        self.admission.outer_handoff()
    }

    pub fn semantic_capsule_bytes(&self) -> &[u8] {
        self.semantic_compiler_handoff().capsule().canonical_bytes()
    }

    pub fn formal_memory_receipt_bytes(&self) -> &[u8] {
        self.semantic_compiler_handoff()
            .capsule()
            .receipts()
            .formal_memory()
            .canonical_preimage()
    }

    pub fn proof_binding_receipt_bytes(&self) -> &[u8] {
        self.semantic_compiler_handoff()
            .capsule()
            .receipts()
            .proof_binding()
            .canonical_preimage()
    }

    /// Decodes the one common multi-root compiler-proof owner for the complete capsule.
    ///
    /// This custody does not establish the per-entry proof-to-executable, layout, or effect joins
    /// returned separately by the protected aggregate backend.
    pub fn validate_compiler_multi_root_proof_inputs_v1(
        &self,
    ) -> Result<ValidatedCompilerMultiRootProofInputsV1, CompilerMultiRootProofValidationErrorV1>
    {
        let receipts = self.semantic_compiler_handoff().capsule().receipts();
        validate_compiler_multi_root_proof_inputs_v1(
            receipts.proof_binding(),
            receipts.semantic_mir(),
            receipts.middle_end(),
            receipts.kernel_ir(),
            receipts.mir_to_kir_correspondence(),
            receipts.formal_memory(),
        )
    }

    /// Independently validates exact multi-root target association and KIR-to-LLVM replay.
    ///
    /// The caller must retain `proof_inputs` from
    /// [`Self::validate_compiler_multi_root_proof_inputs_v1`]. The returned owner remains
    /// authority-free until host promotion cross-binds it to every descriptor, physical symbol,
    /// final compiler module, and finalizer derivation.
    pub fn validate_compiler_multi_root_target_lineage_v1(
        &self,
        proof_inputs: &ValidatedCompilerMultiRootProofInputsV1,
    ) -> Result<ValidatedCompilerMultiRootTargetLineageV1, CompilerTargetLineageValidationErrorV1>
    {
        validate_compiler_multi_root_target_lineage_v1(
            self.semantic_compiler_handoff().capsule(),
            proof_inputs,
        )
    }

    pub fn finalized_hsaco_bytes(&self) -> &[u8] {
        self.current.exact_artifact_bytes()
    }

    pub const fn capsule_sha256(&self) -> [u8; 32] {
        self.admission.lineage_evidence().capsule_sha256()
    }

    pub const fn formal_memory_receipt_sha256(&self) -> [u8; 32] {
        self.admission.lineage_evidence().formal_memory_sha256()
    }

    pub const fn proof_binding_receipt_sha256(&self) -> [u8; 32] {
        self.admission.lineage_evidence().proof_binding_sha256()
    }

    pub const fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        self.admission.lineage_evidence().finalized_sha256()
    }

    pub const fn finalized_hsaco_length(&self) -> u64 {
        self.admission.lineage_evidence().finalized_length()
    }

    pub fn target(&self) -> fe2o3_amd_target::AmdTargetId {
        self.admission.target()
    }

    pub fn code_object_version(&self) -> CodeObjectVersion {
        self.admission.code_object_version()
    }
}

/// Protected theorem evidence for one marker at one canonical descriptor ordinal.
///
/// The marker and generated-host coordinates are rechecked by host promotion. The remaining
/// identities must come from protected verification of this exact physical executable and all
/// invocations satisfying the marker's generated ABI, layout, effect, alias, initialization, and
/// launch contracts.
pub struct WorkerV3ProtectedRosterEntryEvidenceV1 {
    lineage: WorkerV3HostLineageIdentityV1,
    marker_binding: [u8; 32],
    generated_host_contract: [u8; 32],
    proof_executable_binding_sha256: [u8; 32],
    rust_type_layout_contract_sha256: [u8; 32],
    rust_effect_contract_sha256: [u8; 32],
    safety_properties: WorkerV3SafetyPropertiesV1,
}

impl WorkerV3ProtectedRosterEntryEvidenceV1 {
    /// Constructs one entry result from independently authenticated protected state.
    ///
    /// # Safety
    ///
    /// Every identity and property must cover the exact request entry named by `lineage` and
    /// `marker_binding`; request echoes or nonzero placeholders do not satisfy this contract.
    #[allow(clippy::too_many_arguments)]
    pub const unsafe fn new(
        lineage: WorkerV3HostLineageIdentityV1,
        marker_binding: [u8; 32],
        generated_host_contract: [u8; 32],
        proof_executable_binding_sha256: [u8; 32],
        rust_type_layout_contract_sha256: [u8; 32],
        rust_effect_contract_sha256: [u8; 32],
        safety_properties: WorkerV3SafetyPropertiesV1,
    ) -> Self {
        Self {
            lineage,
            marker_binding,
            generated_host_contract,
            proof_executable_binding_sha256,
            rust_type_layout_contract_sha256,
            rust_effect_contract_sha256,
            safety_properties,
        }
    }

    pub const fn lineage_identity(&self) -> WorkerV3HostLineageIdentityV1 {
        self.lineage
    }

    pub const fn marker_binding_identity(&self) -> [u8; 32] {
        self.marker_binding
    }

    pub const fn generated_host_contract_identity(&self) -> [u8; 32] {
        self.generated_host_contract
    }

    pub const fn proof_executable_binding_sha256(&self) -> [u8; 32] {
        self.proof_executable_binding_sha256
    }

    pub const fn rust_type_layout_contract_sha256(&self) -> [u8; 32] {
        self.rust_type_layout_contract_sha256
    }

    pub const fn rust_effect_contract_sha256(&self) -> [u8; 32] {
        self.rust_effect_contract_sha256
    }

    pub const fn safety_properties(&self) -> WorkerV3SafetyPropertiesV1 {
        self.safety_properties
    }
}

/// One protected result for a complete marker roster.
///
/// The finalizer derivation and multi-root source/target owners are common artifact custody.
/// `entries` separately supplies the protected proof-to-executable, layout, effect, and
/// universal-safety join for every canonical marker.
pub struct WorkerV3ProtectedRosterVerificationEvidenceV1 {
    finalizer_derivation: RevalidatedProtectedWorkerV3FinalizerDerivationV1,
    compiler_execution: WorkerV3CompilerExecutionVerificationV1,
    proof_inputs: WorkerV3RosterProofInputEvidenceV1,
    target_lineage: WorkerV3RosterTargetLineageEvidenceV1,
    verifier_measurement_sha256: [u8; 32],
    verification_transcript_sha256: [u8; 32],
    entries: Vec<WorkerV3ProtectedRosterEntryEvidenceV1>,
}

impl WorkerV3ProtectedRosterVerificationEvidenceV1 {
    /// Constructs aggregate evidence produced by one reviewed protected backend execution.
    ///
    /// # Safety
    ///
    /// `finalizer_derivation` must be independently reconstructed from the exact request replay,
    /// the common coordinates must bind the exact aggregate request, and `entries` must cover
    /// every request entry exactly once in canonical descriptor order.
    pub unsafe fn new(
        finalizer_derivation: RevalidatedProtectedWorkerV3FinalizerDerivationV1,
        compiler_execution: WorkerV3CompilerExecutionVerificationV1,
        proof_inputs: ValidatedCompilerMultiRootProofInputsV1,
        target_lineage: ValidatedCompilerMultiRootTargetLineageV1,
        verifier_measurement_sha256: [u8; 32],
        verification_transcript_sha256: [u8; 32],
        entries: Vec<WorkerV3ProtectedRosterEntryEvidenceV1>,
    ) -> Self {
        Self {
            finalizer_derivation,
            compiler_execution,
            proof_inputs: WorkerV3RosterProofInputEvidenceV1::Validated(proof_inputs),
            target_lineage: WorkerV3RosterTargetLineageEvidenceV1::Validated(Box::new(
                target_lineage,
            )),
            verifier_measurement_sha256,
            verification_transcript_sha256,
            entries,
        }
    }

    /// Constructs descriptive aggregate evidence for the explicit integration-test verifier seam.
    #[cfg(feature = "worker-v3-verifier-test-support")]
    #[doc(hidden)]
    pub unsafe fn synthetic_for_test_only(
        finalizer_derivation: RevalidatedProtectedWorkerV3FinalizerDerivationV1,
        compiler_execution: WorkerV3CompilerExecutionVerificationV1,
        verifier_measurement_sha256: [u8; 32],
        verification_transcript_sha256: [u8; 32],
        entries: Vec<WorkerV3ProtectedRosterEntryEvidenceV1>,
    ) -> Self {
        Self {
            finalizer_derivation,
            compiler_execution,
            proof_inputs: WorkerV3RosterProofInputEvidenceV1::Synthetic,
            target_lineage: WorkerV3RosterTargetLineageEvidenceV1::Synthetic,
            verifier_measurement_sha256,
            verification_transcript_sha256,
            entries,
        }
    }
}

#[allow(clippy::large_enum_variant)]
enum WorkerV3RosterProofInputEvidenceV1 {
    Validated(ValidatedCompilerMultiRootProofInputsV1),
    #[cfg(feature = "worker-v3-verifier-test-support")]
    Synthetic,
}

enum WorkerV3RosterTargetLineageEvidenceV1 {
    Validated(Box<ValidatedCompilerMultiRootTargetLineageV1>),
    #[cfg(feature = "worker-v3-verifier-test-support")]
    Synthetic,
}

/// External protected authority for one complete roster request.
///
/// # Safety
///
/// Implementations must satisfy the compiler-policy, protected Worker-ledger, and external
/// rollback obligations of [`WorkerV3ProtectedVerifierBackendV1`] once for the exact common
/// artifact. They must independently reconstruct and retain the move-only finalizer derivation from
/// the exact replay, rather than echoing a host digest. They must additionally inspect every
/// descriptor and independently selected physical executable, authenticate one ordered entry result
/// for every marker, and establish each result for all concrete invocations satisfying that
/// marker's generated contracts. The common multi-root source and target owners establish exact
/// compiler custody, but are not by themselves a proof-to-executable theorem.
pub unsafe trait WorkerV3ProtectedRosterVerifierBackendV1<
    R: CompilerGeneratedKernelExpectationRosterV1,
>
{
    type Error;

    /// Authenticates the complete pinned roster through one protected call.
    ///
    /// # Safety
    ///
    /// The implementation obligations are those of the unsafe trait.
    unsafe fn verify_protected_roster(
        &mut self,
        request: &WorkerV3RosterVerificationRequestV1<'_, R>,
    ) -> Result<WorkerV3ProtectedRosterVerificationEvidenceV1, Self::Error>;
}

/// Crate-owned aggregate adapter around one reviewed protected backend.
pub struct WorkerV3ProtectedRosterVerifierAdapterV1<B> {
    backend: B,
}

impl<B> WorkerV3ProtectedRosterVerifierAdapterV1<B> {
    pub const fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn into_inner(self) -> B {
        self.backend
    }

    unsafe fn verify<R>(
        &mut self,
        request: &WorkerV3RosterVerificationRequestV1<'_, R>,
    ) -> Result<WorkerV3RosterVerificationDecisionV1, B::Error>
    where
        R: CompilerGeneratedKernelExpectationRosterV1,
        B: WorkerV3ProtectedRosterVerifierBackendV1<R>,
    {
        // SAFETY: `B` owns the independent protected checks required by its unsafe trait.
        let evidence = unsafe { self.backend.verify_protected_roster(request)? };
        Ok(WorkerV3RosterVerificationDecisionV1 {
            challenge: request.challenge_identity(),
            lineage: request.lineage_identity(),
            roster_identity: request.roster_identity(),
            capsule_sha256: request.capsule_sha256(),
            formal_memory_sha256: request.formal_memory_receipt_sha256(),
            proof_binding_sha256: request.proof_binding_receipt_sha256(),
            finalized_sha256: request.finalized_hsaco_sha256(),
            finalized_length: request.finalized_hsaco_length(),
            target: request.target(),
            code_object_version: request.code_object_version(),
            finalizer_derivation: evidence.finalizer_derivation,
            compiler_execution: evidence.compiler_execution,
            proof_inputs: evidence.proof_inputs,
            target_lineage: evidence.target_lineage,
            verifier_measurement_sha256: evidence.verifier_measurement_sha256,
            verification_transcript_sha256: evidence.verification_transcript_sha256,
            entries: evidence.entries,
        })
    }
}

/// Authenticated aggregate result for one exact roster and artifact.
pub struct WorkerV3RosterVerificationDecisionV1 {
    challenge: WorkerV3RosterVerificationChallengeIdentityV1,
    lineage: WorkerV3HostLineageIdentityV1,
    roster_identity: WorkerV3VerificationRosterIdentityV1,
    capsule_sha256: [u8; 32],
    formal_memory_sha256: [u8; 32],
    proof_binding_sha256: [u8; 32],
    finalized_sha256: [u8; 32],
    finalized_length: u64,
    target: fe2o3_amd_target::AmdTargetId,
    code_object_version: CodeObjectVersion,
    finalizer_derivation: RevalidatedProtectedWorkerV3FinalizerDerivationV1,
    compiler_execution: WorkerV3CompilerExecutionVerificationV1,
    proof_inputs: WorkerV3RosterProofInputEvidenceV1,
    target_lineage: WorkerV3RosterTargetLineageEvidenceV1,
    verifier_measurement_sha256: [u8; 32],
    verification_transcript_sha256: [u8; 32],
    entries: Vec<WorkerV3ProtectedRosterEntryEvidenceV1>,
}

impl WorkerV3RosterVerificationDecisionV1 {
    pub const fn challenge_identity(&self) -> WorkerV3RosterVerificationChallengeIdentityV1 {
        self.challenge
    }

    pub const fn lineage_identity(&self) -> WorkerV3HostLineageIdentityV1 {
        self.lineage
    }

    pub const fn roster_identity(&self) -> WorkerV3VerificationRosterIdentityV1 {
        self.roster_identity
    }

    pub fn entries(&self) -> &[WorkerV3ProtectedRosterEntryEvidenceV1] {
        &self.entries
    }

    pub const fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        self.finalized_sha256
    }

    pub const fn finalized_hsaco_length(&self) -> u64 {
        self.finalized_length
    }

    /// Returns protected finalizer custody independently reconstructed from the exact roster replay.
    pub const fn finalizer_derivation(&self) -> &RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
        &self.finalizer_derivation
    }

    pub const fn compiler_execution(&self) -> &WorkerV3CompilerExecutionVerificationV1 {
        &self.compiler_execution
    }

    /// Returns the one common decoded multi-root compiler-proof owner for this artifact.
    pub const fn validated_compiler_proof_inputs(
        &self,
    ) -> Option<&ValidatedCompilerMultiRootProofInputsV1> {
        match &self.proof_inputs {
            WorkerV3RosterProofInputEvidenceV1::Validated(inputs) => Some(inputs),
            #[cfg(feature = "worker-v3-verifier-test-support")]
            WorkerV3RosterProofInputEvidenceV1::Synthetic => None,
        }
    }

    /// Returns exact multi-root target-lineage and KIR-to-LLVM replay custody.
    pub const fn validated_compiler_target_lineage(
        &self,
    ) -> Option<&ValidatedCompilerMultiRootTargetLineageV1> {
        match &self.target_lineage {
            WorkerV3RosterTargetLineageEvidenceV1::Validated(lineage) => Some(lineage),
            #[cfg(feature = "worker-v3-verifier-test-support")]
            WorkerV3RosterTargetLineageEvidenceV1::Synthetic => None,
        }
    }

    /// Reports common compiler and signed-Verus custody only.
    ///
    /// Per-entry proof-to-executable, layout, and effect authority is retained separately in
    /// [`Self::entries`].
    pub fn retains_current_compiler_and_signed_verus_evidence(&self) -> bool {
        match (&self.proof_inputs, &self.target_lineage) {
            (
                WorkerV3RosterProofInputEvidenceV1::Validated(inputs),
                WorkerV3RosterTargetLineageEvidenceV1::Validated(target),
            ) => {
                inputs.roots().iter().all(|root| {
                    root.verus_execution()
                        .authenticates_signed_receipt_under_embedded_key()
                }) && target.has_exact_receipt_association()
                    && target.has_exact_kir_to_llvm_replay()
                    && self
                        .compiler_execution
                        .authenticates_signed_currentness_evidence()
            }
            #[cfg(feature = "worker-v3-verifier-test-support")]
            _ => false,
        }
    }
}

/// Move-only authenticated custody for one complete recovered roster.
///
/// The owner retains the sole recovered artifact, one current-publication token, one aggregate
/// decision, and one common multi-root source/target custody pair. It grants no load or launch
/// authority.
pub struct AuthenticatedWorkerV3RosterV1<R> {
    admission: RecoveredWorkerV3PinnedRosterV1<R>,
    current: DurableCurrentLinkPublicationTokenV1,
    verification: WorkerV3RosterVerificationDecisionV1,
    _roster: PhantomData<fn() -> R>,
}

impl<R> fmt::Debug for AuthenticatedWorkerV3RosterV1<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedWorkerV3RosterV1")
            .field("lineage", &self.verification.lineage)
            .field("roster", &self.verification.roster_identity)
            .field("entry_count", &self.verification.entries.len())
            .finish_non_exhaustive()
    }
}

impl<R: CompilerGeneratedKernelExpectationRosterV1> AuthenticatedWorkerV3RosterV1<R> {
    pub fn authenticate<B>(
        admission: RecoveredWorkerV3PinnedRosterV1<R>,
        verifier: &mut WorkerV3ProtectedRosterVerifierAdapterV1<B>,
    ) -> Result<Self, WorkerV3RosterVerificationAuthenticationErrorV1<B::Error>>
    where
        B: WorkerV3ProtectedRosterVerifierBackendV1<R>,
    {
        let current = admission
            .acquire_retained_currentness_token()
            .map_err(WorkerV3RosterVerificationAuthenticationErrorV1::CurrentPublication)?;
        let request =
            prepare_roster_request::<R>(&admission, &current).map_err(|error| {
                match error {
            WorkerV3RosterVerificationRequestPreparationErrorV1::Marker { ordinal, field } => {
                WorkerV3RosterVerificationAuthenticationErrorV1::Marker { ordinal, field }
            }
            WorkerV3RosterVerificationRequestPreparationErrorV1::UnsupportedGeneratedProfile {
                ordinal,
            } => WorkerV3RosterVerificationAuthenticationErrorV1::UnsupportedGeneratedProfile {
                ordinal,
            },
        }
            })?;
        // SAFETY: callers cannot bypass the crate-owned adapter. The unsafe backend owns all
        // protected aggregate obligations and the result is fully revalidated below.
        let verification = unsafe { verifier.verify(&request) };
        admission
            .revalidate_retained_currentness_token(&current)
            .map_err(WorkerV3RosterVerificationAuthenticationErrorV1::CurrentPublication)?;
        let verification =
            verification.map_err(WorkerV3RosterVerificationAuthenticationErrorV1::Verifier)?;
        validate_roster_decision::<R>(&request, &verification)
            .map_err(WorkerV3RosterVerificationAuthenticationErrorV1::Decision)?;
        Ok(Self {
            admission,
            current,
            verification,
            _roster: PhantomData,
        })
    }

    pub const fn verification(&self) -> &WorkerV3RosterVerificationDecisionV1 {
        &self.verification
    }

    /// Returns the exact compiler module content identity retained by the protected finalizer.
    pub const fn compiler_module_identity(&self) -> fe2o3_hsaco_finalize::ContentIdentityV1 {
        self.verification
            .finalizer_derivation()
            .compiler_module_identity()
    }

    /// Returns the exact nested V2 compiler handoff identity retained by roster admission.
    pub const fn compiler_handoff_identity(
        &self,
    ) -> fe2o3_compiler_ffi::CompilerModuleHandoffIdentityV2 {
        self.admission.outer_handoff().module_handoff().identity()
    }

    /// Returns the exact compiler symbol-manifest identity retained by roster admission.
    pub const fn compiler_symbol_manifest_identity(
        &self,
    ) -> fe2o3_compiler_ffi::CompilerModuleSymbolManifestIdentityV1 {
        self.admission
            .outer_handoff()
            .module_handoff()
            .symbol_manifest()
            .identity()
    }

    pub fn entry_count(&self) -> usize {
        self.verification.entries.len()
    }

    pub fn entry<K: CompilerGeneratedKernelExpectationV1>(
        &self,
    ) -> Result<AuthenticatedWorkerV3RosterEntryV1<'_, R, K>, WorkerV3RosterEntryErrorV1> {
        let ordinal = R::ENTRIES
            .iter()
            .position(|entry| entry.kernel_binding_id() == K::KERNEL_BINDING_ID_V1)
            .ok_or(WorkerV3RosterEntryErrorV1::MarkerNotInRoster)?;
        let expected = &R::ENTRIES[ordinal];
        for (matches, field) in [
            (expected.logical_name() == K::LOGICAL_NAME, "logical name"),
            (expected.export_name() == K::EXPORT_NAME, "export name"),
            (
                expected.generated_host_contract_identity()
                    == K::PROFILE.generated_host_contract_identity(),
                "generated host contract",
            ),
        ] {
            if !matches {
                return Err(WorkerV3RosterEntryErrorV1::MarkerMismatch { ordinal, field });
            }
        }
        validate_marker::<K>(
            self.admission
                .descriptor(ordinal)
                .expect("authenticated roster retains every descriptor"),
        )
        .map_err(|field| WorkerV3RosterEntryErrorV1::MarkerMismatch { ordinal, field })?;
        Ok(AuthenticatedWorkerV3RosterEntryV1 {
            roster: self,
            ordinal,
            _marker: PhantomData,
        })
    }

    pub fn target(&self) -> fe2o3_amd_target::AmdTargetId {
        self.admission.target()
    }

    pub fn revalidate_currentness(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.admission
            .revalidate_retained_currentness_token(&self.current)
    }

    pub(crate) fn exact_current_hsaco_bytes(&self) -> &[u8] {
        self.current.exact_artifact_bytes()
    }

    pub(crate) const fn admitted_roster(&self) -> &RecoveredWorkerV3PinnedRosterV1<R> {
        &self.admission
    }

    pub const fn authenticates_verification_authority(&self) -> bool {
        true
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Non-Clone typed borrow of one authenticated roster entry.
///
/// This handle cannot outlive its aggregate owner and owns no artifact, proof, currentness, load,
/// or launch custody.
pub struct AuthenticatedWorkerV3RosterEntryV1<'roster, R, K> {
    roster: &'roster AuthenticatedWorkerV3RosterV1<R>,
    ordinal: usize,
    _marker: PhantomData<fn() -> K>,
}

impl<R, K> fmt::Debug for AuthenticatedWorkerV3RosterEntryV1<'_, R, K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedWorkerV3RosterEntryV1")
            .field("ordinal", &self.ordinal)
            .finish_non_exhaustive()
    }
}

impl<R, K> AuthenticatedWorkerV3RosterEntryV1<'_, R, K>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
    K: CompilerGeneratedKernelExpectationV1,
{
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub fn descriptor(&self) -> &KernelDescriptorV1 {
        self.roster
            .admission
            .descriptor(self.ordinal)
            .expect("authenticated roster retains every descriptor")
    }

    pub fn physical_kernel(&self) -> &InspectedKernel {
        self.roster
            .admission
            .physical_kernel(self.ordinal)
            .expect("authenticated roster retains every physical kernel")
    }

    pub fn descriptor_binding(&self) -> KernelDescriptorBinding {
        self.roster
            .admission
            .descriptor_binding(self.ordinal)
            .expect("authenticated roster retains every descriptor binding")
    }

    pub fn entry_verification(&self) -> &WorkerV3ProtectedRosterEntryEvidenceV1 {
        &self.roster.verification.entries[self.ordinal]
    }

    pub const fn aggregate_verification(&self) -> &WorkerV3RosterVerificationDecisionV1 {
        &self.roster.verification
    }

    pub const fn authenticates_verification_authority(&self) -> bool {
        true
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn prepare_roster_request<'admission, R: CompilerGeneratedKernelExpectationRosterV1>(
    admission: &'admission RecoveredWorkerV3PinnedRosterV1<R>,
    current: &'admission DurableCurrentLinkPublicationTokenV1,
) -> Result<
    WorkerV3RosterVerificationRequestV1<'admission, R>,
    WorkerV3RosterVerificationRequestPreparationErrorV1,
> {
    for (ordinal, expected) in R::ENTRIES.iter().enumerate() {
        let descriptor = admission.descriptor(ordinal).ok_or(
            WorkerV3RosterVerificationRequestPreparationErrorV1::Marker {
                ordinal,
                field: "descriptor",
            },
        )?;
        for (matches, field) in [
            (
                descriptor.logical_name().as_str() == expected.logical_name(),
                "logical name",
            ),
            (
                descriptor.entry_name().as_str() == expected.export_name(),
                "export name",
            ),
            (
                descriptor.kernel_id().as_bytes() == &expected.kernel_binding_id(),
                "binding identity",
            ),
        ] {
            if !matches {
                return Err(
                    WorkerV3RosterVerificationRequestPreparationErrorV1::Marker { ordinal, field },
                );
            }
        }
        if expected.generated_host_contract_identity() == [0; 32] {
            return Err(
                WorkerV3RosterVerificationRequestPreparationErrorV1::UnsupportedGeneratedProfile {
                    ordinal,
                },
            );
        }
    }
    let roster_identity = derive_roster_identity::<R>();
    let challenge = derive_roster_challenge(admission.lineage_identity(), roster_identity);
    Ok(WorkerV3RosterVerificationRequestV1 {
        challenge,
        roster_identity,
        admission,
        current,
        _roster: PhantomData,
    })
}

fn derive_roster_identity<R: CompilerGeneratedKernelExpectationRosterV1>()
-> WorkerV3VerificationRosterIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(WORKER_V3_ROSTER_IDENTITY_DOMAIN_V1);
    digest.update(
        u64::try_from(R::ENTRIES.len())
            .expect("generated roster length fits u64")
            .to_le_bytes(),
    );
    for entry in R::ENTRIES {
        for bytes in [
            entry.logical_name().as_bytes(),
            entry.export_name().as_bytes(),
        ] {
            digest.update(
                u64::try_from(bytes.len())
                    .expect("generated roster name length fits u64")
                    .to_le_bytes(),
            );
            digest.update(bytes);
        }
        digest.update(entry.kernel_binding_id());
        digest.update(entry.generated_host_contract_identity());
    }
    WorkerV3VerificationRosterIdentityV1(digest.finalize().into())
}

fn derive_roster_challenge(
    lineage: WorkerV3HostLineageIdentityV1,
    roster: WorkerV3VerificationRosterIdentityV1,
) -> WorkerV3RosterVerificationChallengeIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(WORKER_V3_ROSTER_VERIFICATION_CHALLENGE_DOMAIN_V1);
    digest.update(lineage.as_bytes());
    digest.update(roster.as_bytes());
    WorkerV3RosterVerificationChallengeIdentityV1(digest.finalize().into())
}

fn validate_roster_decision<R: CompilerGeneratedKernelExpectationRosterV1>(
    request: &WorkerV3RosterVerificationRequestV1<'_, R>,
    decision: &WorkerV3RosterVerificationDecisionV1,
) -> Result<(), WorkerV3RosterVerificationDecisionErrorV1> {
    for (matches, field) in [
        (
            decision.challenge == request.challenge_identity(),
            "verification challenge",
        ),
        (
            decision.lineage == request.lineage_identity(),
            "host roster lineage",
        ),
        (
            decision.roster_identity == request.roster_identity(),
            "generated marker roster",
        ),
        (
            decision.capsule_sha256 == request.capsule_sha256(),
            "semantic capsule",
        ),
        (
            decision.formal_memory_sha256 == request.formal_memory_receipt_sha256(),
            "formal memory receipt",
        ),
        (
            decision.proof_binding_sha256 == request.proof_binding_receipt_sha256(),
            "proof binding receipt",
        ),
        (
            decision.finalized_sha256 == request.finalized_hsaco_sha256(),
            "finalized HSACO",
        ),
        (
            decision.finalized_length == request.finalized_hsaco_length(),
            "finalized HSACO length",
        ),
        (
            decision.finalizer_derivation.identity() == request.finalizer_derivation().identity(),
            "finalizer derivation",
        ),
        (decision.target == request.target(), "target"),
        (
            decision.code_object_version == request.code_object_version(),
            "code-object version",
        ),
        (
            decision.compiler_execution.subject_sha256
                == request.compiler_execution_subject_sha256(),
            "compiler-execution subject",
        ),
        (
            decision.compiler_execution.carriage_sha256
                == request.compiler_execution_carriage_sha256(),
            "compiler-execution carriage",
        ),
        (
            decision.compiler_execution.policy_sha256 == request.compiler_execution_policy_sha256(),
            "compiler-execution policy",
        ),
        (
            decision.compiler_execution.issuer_journal_sha256
                == request.compiler_execution_issuer_journal_sha256(),
            "compiler-execution issuer journal",
        ),
        (
            decision.compiler_execution.compiler_occurrence_sha256
                == request.compiler_occurrence_sha256(),
            "compiler occurrence",
        ),
        (
            decision.compiler_execution.receipt_sha256
                == request.compiler_execution_receipt_sha256(),
            "compiler-execution receipt",
        ),
        (
            decision.compiler_execution.publication_sha256
                == request.compiler_execution_publication_sha256(),
            "compiler-execution receipt publication",
        ),
        (
            decision.compiler_execution.acknowledgment_sha256
                == request.compiler_execution_acknowledgment_sha256(),
            "compiler-execution publication acknowledgment",
        ),
        (
            decision.compiler_execution.worker_ledger_record_sha256
                == request.compiler_execution_worker_ledger_record_sha256(),
            "compiler-execution Worker ledger record",
        ),
        (
            decision.compiler_execution.sequence == request.compiler_execution_sequence(),
            "compiler-execution rollback sequence",
        ),
        (
            decision.compiler_execution.prior_rollback_anchor
                == request.compiler_execution_prior_rollback_anchor(),
            "compiler-execution prior rollback anchor",
        ),
        (
            decision.compiler_execution.current_rollback_anchor
                == request.compiler_execution_current_rollback_anchor(),
            "compiler-execution current rollback anchor",
        ),
    ] {
        if !matches {
            return Err(WorkerV3RosterVerificationDecisionErrorV1::IdentityMismatch(
                field,
            ));
        }
    }
    for (identity, field) in [
        (decision.verifier_measurement_sha256, "verifier measurement"),
        (
            decision.verification_transcript_sha256,
            "verification transcript",
        ),
        (
            decision
                .compiler_execution
                .current_record_verification_sha256,
            "compiler current-record verification",
        ),
        (
            decision
                .compiler_execution
                .current_record_attestation_sha256,
            "compiler current-record attestation",
        ),
        (
            decision
                .compiler_execution
                .protected_policy_verification_sha256,
            "protected compiler policy verification",
        ),
        (
            decision
                .compiler_execution
                .protected_worker_ledger_verification_sha256,
            "protected Worker ledger verification",
        ),
        (
            decision
                .compiler_execution
                .external_rollback_verification_sha256,
            "external rollback verification",
        ),
    ] {
        if identity == [0; 32] {
            return Err(
                WorkerV3RosterVerificationDecisionErrorV1::ZeroAuthenticatedIdentity(field),
            );
        }
    }
    if decision.entries.len() != R::ENTRIES.len() {
        return Err(
            WorkerV3RosterVerificationDecisionErrorV1::EntryCountMismatch {
                expected: R::ENTRIES.len(),
                actual: decision.entries.len(),
            },
        );
    }
    for (ordinal, (expected, actual)) in R::ENTRIES.iter().zip(&decision.entries).enumerate() {
        let expected_lineage = request
            .entry_lineage_identity(ordinal)
            .expect("admitted roster retains every entry lineage");
        for (matches, field) in [
            (actual.lineage == expected_lineage, "entry lineage"),
            (
                actual.marker_binding == expected.kernel_binding_id(),
                "marker binding",
            ),
            (
                actual.generated_host_contract == expected.generated_host_contract_identity(),
                "generated host contract",
            ),
        ] {
            if !matches {
                return Err(
                    WorkerV3RosterVerificationDecisionErrorV1::EntryIdentityMismatch {
                        ordinal,
                        field,
                    },
                );
            }
        }
        for (identity, field) in [
            (
                actual.proof_executable_binding_sha256,
                "proof/executable binding",
            ),
            (
                actual.rust_type_layout_contract_sha256,
                "Rust type/layout contract",
            ),
            (actual.rust_effect_contract_sha256, "Rust effect contract"),
        ] {
            if identity == [0; 32] {
                return Err(
                    WorkerV3RosterVerificationDecisionErrorV1::ZeroEntryAuthenticatedIdentity {
                        ordinal,
                        field,
                    },
                );
            }
        }
        for property in [
            WorkerV3SafetyPropertyV1::Bounds,
            WorkerV3SafetyPropertyV1::AddressOverflowFreedom,
            WorkerV3SafetyPropertyV1::MemorySafety,
            WorkerV3SafetyPropertyV1::Initialization,
            WorkerV3SafetyPropertyV1::RaceFreedom,
            WorkerV3SafetyPropertyV1::LaunchValidity,
            WorkerV3SafetyPropertyV1::Synchronization,
            WorkerV3SafetyPropertyV1::SemanticRefinement,
        ] {
            if !actual.safety_properties.contains(property) {
                return Err(
                    WorkerV3RosterVerificationDecisionErrorV1::MissingEntrySafetyProperty {
                        ordinal,
                        property,
                    },
                );
            }
        }
    }
    validate_roster_decision_proof_inputs(request, decision)?;
    validate_roster_decision_target_lineage(request, decision)
}

fn validate_roster_decision_proof_inputs<R: CompilerGeneratedKernelExpectationRosterV1>(
    request: &WorkerV3RosterVerificationRequestV1<'_, R>,
    decision: &WorkerV3RosterVerificationDecisionV1,
) -> Result<(), WorkerV3RosterVerificationDecisionErrorV1> {
    #[cfg(not(feature = "worker-v3-verifier-test-support"))]
    let WorkerV3RosterProofInputEvidenceV1::Validated(inputs) = &decision.proof_inputs;
    #[cfg(feature = "worker-v3-verifier-test-support")]
    let inputs = match &decision.proof_inputs {
        WorkerV3RosterProofInputEvidenceV1::Validated(inputs) => inputs,
        WorkerV3RosterProofInputEvidenceV1::Synthetic => return Ok(()),
    };
    let receipts = request.semantic_compiler_handoff().capsule().receipts();
    for (matches, field) in [
        (
            inputs.association().canonical_bytes() == receipts.proof_binding().canonical_preimage(),
            "proof-binding association",
        ),
        (
            inputs.verus_roster().canonical_bytes()
                == inputs.association().verus_execution_evidence(),
            "multi-root Verus roster",
        ),
        (
            inputs.semantic_mir().canonical_encoding()
                == receipts.semantic_mir().canonical_preimage(),
            "semantic MIR",
        ),
        (
            inputs.middle_end_roster().canonical_bytes()
                == receipts.middle_end().canonical_preimage(),
            "middle-end roster",
        ),
        (
            inputs.kernel_ir().canonical_bytes() == receipts.kernel_ir().canonical_preimage(),
            "Kernel IR",
        ),
        (
            inputs.correspondence_roster().canonical_bytes()
                == receipts.mir_to_kir_correspondence().canonical_preimage(),
            "MIR-to-KIR correspondence roster",
        ),
        (
            inputs.formal_memory_roster().canonical_bytes()
                == receipts.formal_memory().canonical_preimage(),
            "formal-memory roster",
        ),
    ] {
        if !matches {
            return Err(WorkerV3RosterVerificationDecisionErrorV1::ProofInputMismatch(field));
        }
    }
    if inputs.receipt_identity() != receipts.proof_binding().identity() {
        return Err(
            WorkerV3RosterVerificationDecisionErrorV1::ProofInputMismatch(
                "proof-binding receipt identity",
            ),
        );
    }
    if inputs.roots().len() != R::ENTRIES.len() {
        return Err(
            WorkerV3RosterVerificationDecisionErrorV1::EntryCountMismatch {
                expected: R::ENTRIES.len(),
                actual: inputs.roots().len(),
            },
        );
    }
    let mut matched_roots = vec![false; inputs.roots().len()];
    for (ordinal, expected) in R::ENTRIES.iter().enumerate() {
        let descriptor = request
            .descriptor(ordinal)
            .expect("admitted roster retains every descriptor");
        let physical = request
            .physical_kernel(ordinal)
            .expect("admitted roster retains every physical kernel");
        let root_index = inputs
            .roots()
            .iter()
            .position(|root| root.kernel_binding() == &expected.kernel_binding_id())
            .ok_or(
                WorkerV3RosterVerificationDecisionErrorV1::EntryIdentityMismatch {
                    ordinal,
                    field: "multi-root proof binding",
                },
            )?;
        if matched_roots[root_index] {
            return Err(
                WorkerV3RosterVerificationDecisionErrorV1::EntryIdentityMismatch {
                    ordinal,
                    field: "unique multi-root proof binding",
                },
            );
        }
        matched_roots[root_index] = true;
        let root = &inputs.roots()[root_index];
        for (matches, field) in [
            (
                root.logical_name() == expected.logical_name(),
                "proof logical name",
            ),
            (
                root.export_symbol() == expected.export_name(),
                "proof export name",
            ),
            (
                root.kernel_id() == expected.export_name(),
                "proof kernel ID",
            ),
            (
                descriptor.logical_name().as_str() == root.logical_name(),
                "descriptor logical name",
            ),
            (
                descriptor.entry_name().as_str() == root.export_symbol(),
                "descriptor export name",
            ),
            (
                descriptor.kernel_id().as_bytes() == root.kernel_binding(),
                "descriptor kernel binding",
            ),
            (
                physical.name() == root.export_symbol(),
                "physical export name",
            ),
            (
                physical.symbol() == descriptor.descriptor_symbol().as_str(),
                "physical descriptor symbol",
            ),
        ] {
            if !matches {
                return Err(
                    WorkerV3RosterVerificationDecisionErrorV1::EntryIdentityMismatch {
                        ordinal,
                        field,
                    },
                );
            }
        }
    }
    if matched_roots.iter().any(|matched| !matched) {
        return Err(
            WorkerV3RosterVerificationDecisionErrorV1::ProofInputMismatch(
                "complete multi-root proof roster",
            ),
        );
    }
    Ok(())
}

fn validate_roster_decision_target_lineage<R: CompilerGeneratedKernelExpectationRosterV1>(
    request: &WorkerV3RosterVerificationRequestV1<'_, R>,
    decision: &WorkerV3RosterVerificationDecisionV1,
) -> Result<(), WorkerV3RosterVerificationDecisionErrorV1> {
    #[cfg(not(feature = "worker-v3-verifier-test-support"))]
    let WorkerV3RosterTargetLineageEvidenceV1::Validated(lineage) = &decision.target_lineage;
    #[cfg(feature = "worker-v3-verifier-test-support")]
    let lineage = match &decision.target_lineage {
        WorkerV3RosterTargetLineageEvidenceV1::Validated(lineage) => lineage,
        WorkerV3RosterTargetLineageEvidenceV1::Synthetic => return Ok(()),
    };
    #[cfg(not(feature = "worker-v3-verifier-test-support"))]
    let WorkerV3RosterProofInputEvidenceV1::Validated(proof_inputs) = &decision.proof_inputs;
    #[cfg(feature = "worker-v3-verifier-test-support")]
    let proof_inputs = match &decision.proof_inputs {
        WorkerV3RosterProofInputEvidenceV1::Validated(inputs) => inputs,
        WorkerV3RosterProofInputEvidenceV1::Synthetic => {
            return Err(
                WorkerV3RosterVerificationDecisionErrorV1::TargetLineageMismatch(
                    "source/target custody shape",
                ),
            );
        }
    };
    let capsule = request.semantic_compiler_handoff().capsule();
    let receipts = capsule.receipts();
    let module = request
        .semantic_compiler_handoff()
        .module_handoff()
        .module_identity();
    let finalizer_module = request.finalizer_derivation().compiler_module_identity();
    let final_llvm = lineage.final_llvm_identity();
    let target_binding = lineage.target_binding_receipt_identity();
    let data_layout = lineage.data_layout_receipt_identity();
    let semantic_to_llvm = lineage.semantic_to_llvm_receipt_identity();
    let final_commitment = lineage.final_compiler_module_commitment_identity();
    for (matches, field) in [
        (
            lineage.target_binding().canonical_bytes()
                == receipts.target_binding().canonical_preimage(),
            "target-binding transcript",
        ),
        (
            lineage.data_layout().canonical_bytes() == receipts.data_layout().canonical_preimage(),
            "data-layout transcript",
        ),
        (
            lineage.semantic_to_llvm().canonical_bytes()
                == receipts.semantic_to_llvm().canonical_preimage(),
            "semantic-to-LLVM transcript",
        ),
        (
            target_binding.sha256() == *receipts.target_binding().identity().sha256()
                && target_binding.byte_len() == receipts.target_binding().identity().byte_len(),
            "target-binding receipt",
        ),
        (
            data_layout.sha256() == *receipts.data_layout().identity().sha256()
                && data_layout.byte_len() == receipts.data_layout().identity().byte_len(),
            "data-layout receipt",
        ),
        (
            semantic_to_llvm.sha256() == *receipts.semantic_to_llvm().identity().sha256()
                && semantic_to_llvm.byte_len() == receipts.semantic_to_llvm().identity().byte_len(),
            "semantic-to-LLVM receipt",
        ),
        (
            lineage.replay().kernel_ir_receipt_identity() == receipts.kernel_ir().identity(),
            "replayed Kernel IR receipt",
        ),
        (
            lineage.replay().amdgpu_lowering_receipt_identity()
                == receipts.amdgpu_lowering().identity(),
            "replayed AMDGPU-lowering receipt",
        ),
        (
            final_llvm.sha256() == *module.sha256() && final_llvm.byte_len() == module.byte_len(),
            "final LLVM module",
        ),
        (
            final_llvm.sha256() == *finalizer_module.sha256()
                && final_llvm.byte_len() == finalizer_module.byte_len(),
            "finalizer compiler module",
        ),
        (
            final_commitment.sha256()
                == *receipts
                    .final_compiler_module_commitment()
                    .identity()
                    .sha256()
                && final_commitment.byte_len()
                    == receipts
                        .final_compiler_module_commitment()
                        .identity()
                        .byte_len(),
            "final compiler-module commitment",
        ),
        (
            lineage.target_binding().code_object_version()
                == u16::from(request.code_object_version().number()),
            "code-object version",
        ),
        (
            lineage.target_binding().configured_target() == request.target().to_string(),
            "configured target",
        ),
        (
            lineage.target_binding().root_count() == R::ENTRIES.len(),
            "target root count",
        ),
    ] {
        if !matches {
            return Err(WorkerV3RosterVerificationDecisionErrorV1::TargetLineageMismatch(field));
        }
    }

    for (ordinal, expected) in R::ENTRIES.iter().enumerate() {
        let descriptor = request
            .descriptor(ordinal)
            .expect("admitted roster retains every descriptor");
        let root_index = proof_inputs
            .roots()
            .iter()
            .position(|root| root.kernel_binding() == &expected.kernel_binding_id())
            .ok_or(
                WorkerV3RosterVerificationDecisionErrorV1::EntryIdentityMismatch {
                    ordinal,
                    field: "target-lineage proof binding",
                },
            )?;
        let root = &proof_inputs.roots()[root_index];
        let target_root = lineage.target_binding().workgroup(root_index).ok_or(
            WorkerV3RosterVerificationDecisionErrorV1::TargetLineageMismatch(
                "per-root target workgroup",
            ),
        )?;
        let descriptor_workgroup = match descriptor.launch().block_size() {
            BlockSizeV1::Exact(dimensions) => [dimensions.x(), dimensions.y(), dimensions.z()],
            BlockSizeV1::Any | BlockSizeV1::AtMost(_) => {
                return Err(
                    WorkerV3RosterVerificationDecisionErrorV1::TargetLineageMismatch(
                        "exact descriptor workgroup",
                    ),
                );
            }
        };
        if target_root.kernel() != root.kernel_id()
            || target_root.workgroup() != root.workgroup()
            || descriptor_workgroup != root.workgroup()
        {
            return Err(
                WorkerV3RosterVerificationDecisionErrorV1::EntryIdentityMismatch {
                    ordinal,
                    field: "target workgroup",
                },
            );
        }
    }
    Ok(())
}

enum WorkerV3RosterVerificationRequestPreparationErrorV1 {
    Marker { ordinal: usize, field: &'static str },
    UnsupportedGeneratedProfile { ordinal: usize },
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3RosterVerificationAuthenticationErrorV1<E> {
    Marker { ordinal: usize, field: &'static str },
    UnsupportedGeneratedProfile { ordinal: usize },
    CurrentPublication(RecoveredWorkerV3AdmissionErrorV1),
    Verifier(E),
    Decision(WorkerV3RosterVerificationDecisionErrorV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3RosterVerificationDecisionErrorV1 {
    IdentityMismatch(&'static str),
    ZeroAuthenticatedIdentity(&'static str),
    EntryCountMismatch {
        expected: usize,
        actual: usize,
    },
    EntryIdentityMismatch {
        ordinal: usize,
        field: &'static str,
    },
    ZeroEntryAuthenticatedIdentity {
        ordinal: usize,
        field: &'static str,
    },
    MissingEntrySafetyProperty {
        ordinal: usize,
        property: WorkerV3SafetyPropertyV1,
    },
    ProofInputMismatch(&'static str),
    TargetLineageMismatch(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3RosterEntryErrorV1 {
    MarkerNotInRoster,
    MarkerMismatch { ordinal: usize, field: &'static str },
}

impl<E: fmt::Display> fmt::Display for WorkerV3RosterVerificationAuthenticationErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Marker { ordinal, field } => {
                write!(
                    formatter,
                    "generated roster marker {ordinal} {field} mismatch"
                )
            }
            Self::UnsupportedGeneratedProfile { ordinal } => write!(
                formatter,
                "Worker V3 roster verification requires generated host-contract identity at ordinal {ordinal}",
            ),
            Self::CurrentPublication(error) => {
                write!(
                    formatter,
                    "Worker V3 roster publication revalidation failed: {error}"
                )
            }
            Self::Verifier(error) => {
                write!(formatter, "reviewed V3 roster verifier failed: {error}")
            }
            Self::Decision(error) => {
                write!(formatter, "invalid V3 roster verifier decision: {error}")
            }
        }
    }
}

impl fmt::Display for WorkerV3RosterVerificationDecisionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdentityMismatch(field) => write!(formatter, "{field} identity mismatch"),
            Self::ZeroAuthenticatedIdentity(field) => {
                write!(formatter, "{field} identity is zero")
            }
            Self::EntryCountMismatch { expected, actual } => write!(
                formatter,
                "protected roster entry count {actual} differs from expected {expected}",
            ),
            Self::EntryIdentityMismatch { ordinal, field } => {
                write!(
                    formatter,
                    "protected roster entry {ordinal} {field} mismatch"
                )
            }
            Self::ZeroEntryAuthenticatedIdentity { ordinal, field } => write!(
                formatter,
                "protected roster entry {ordinal} {field} identity is zero",
            ),
            Self::MissingEntrySafetyProperty { ordinal, property } => write!(
                formatter,
                "protected roster entry {ordinal} is missing safety property {property:?}",
            ),
            Self::ProofInputMismatch(field) => write!(
                formatter,
                "validated compiler {field} differs from the exact roster request",
            ),
            Self::TargetLineageMismatch(field) => write!(
                formatter,
                "validated compiler target-lineage {field} differs from the exact roster request",
            ),
        }
    }
}

impl fmt::Display for WorkerV3RosterEntryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MarkerNotInRoster => {
                formatter.write_str("generated marker is not in the authenticated roster")
            }
            Self::MarkerMismatch { ordinal, field } => {
                write!(
                    formatter,
                    "authenticated roster marker {ordinal} {field} mismatch"
                )
            }
        }
    }
}

impl<E> Error for WorkerV3RosterVerificationAuthenticationErrorV1<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurrentPublication(error) => Some(error),
            Self::Verifier(error) => Some(error),
            Self::Decision(error) => Some(error),
            Self::Marker { .. } | Self::UnsupportedGeneratedProfile { .. } => None,
        }
    }
}

impl Error for WorkerV3RosterVerificationDecisionErrorV1 {}

impl Error for WorkerV3RosterEntryErrorV1 {}

/// Borrows one admitted V3 artifact for non-authoritative compiler/proof auditing.
///
/// Unlike [`AuthenticatedWorkerV3ExecutableV1::authenticate`], this operation does not consume
/// admission custody and cannot produce a load-authorizing state. The exact current artifact is
/// pinned and revalidated around the complete audit call.
pub fn audit_recovered_worker_v3_verification_v1<K, A>(
    admission: &RecoveredWorkerV3PinnedDescriptorV1,
    auditor: &mut A,
) -> Result<A::Evidence, WorkerV3VerificationAuditErrorV1<A::Error>>
where
    K: CompilerGeneratedKernelExpectationV1,
    A: WorkerV3AuditorV1<K>,
{
    let current = admission
        .acquire_retained_currentness_token()
        .map_err(WorkerV3VerificationAuditErrorV1::CurrentPublication)?;
    let request = prepare_request::<K>(admission, &current).map_err(|error| match error {
        WorkerV3VerificationRequestPreparationErrorV1::Marker(field) => {
            WorkerV3VerificationAuditErrorV1::Marker(field)
        }
        WorkerV3VerificationRequestPreparationErrorV1::UnsupportedGeneratedProfile => {
            WorkerV3VerificationAuditErrorV1::UnsupportedGeneratedProfile
        }
    })?;
    let evidence = auditor.audit(&request);
    admission
        .revalidate_retained_currentness_token(&current)
        .map_err(WorkerV3VerificationAuditErrorV1::CurrentPublication)?;
    evidence.map_err(WorkerV3VerificationAuditErrorV1::Auditor)
}

fn prepare_request<'admission, K: CompilerGeneratedKernelExpectationV1>(
    admission: &'admission RecoveredWorkerV3PinnedDescriptorV1,
    current: &'admission DurableCurrentLinkPublicationTokenV1,
) -> Result<
    WorkerV3VerificationRequestV1<'admission, K>,
    WorkerV3VerificationRequestPreparationErrorV1,
> {
    validate_marker::<K>(admission.descriptor())
        .map_err(WorkerV3VerificationRequestPreparationErrorV1::Marker)?;
    let lineage = admission.lineage_evidence();
    let generated_host_contract = generated_host_contract::<K>();
    if generated_host_contract == [0; 32] {
        return Err(WorkerV3VerificationRequestPreparationErrorV1::UnsupportedGeneratedProfile);
    }
    let challenge = derive_challenge::<K>(lineage.identity(), generated_host_contract);
    Ok(WorkerV3VerificationRequestV1 {
        challenge,
        lineage,
        finalizer_derivation: admission.finalizer_derivation(),
        finalizer_replay: admission.finalizer_replay(),
        compiler_execution_subject: admission.compiler_execution_subject(),
        compiler_execution_receipt: admission.compiler_execution_receipt(),
        handoff: admission.outer_handoff(),
        finalized_hsaco: current.exact_artifact_bytes(),
        descriptor: admission.descriptor(),
        target: admission.target(),
        code_object_version: admission.code_object_version(),
        generated_host_contract,
        _marker: PhantomData,
    })
}

enum WorkerV3VerificationRequestPreparationErrorV1 {
    Marker(&'static str),
    UnsupportedGeneratedProfile,
}

fn validate_marker<K: CompilerGeneratedKernelExpectationV1>(
    descriptor: &KernelDescriptorV1,
) -> Result<(), &'static str> {
    if descriptor.logical_name().as_str() != K::LOGICAL_NAME {
        return Err("logical name");
    }
    if descriptor.entry_name().as_str() != K::EXPORT_NAME {
        return Err("export name");
    }
    if descriptor.kernel_id() != KernelId::from_bytes(K::KERNEL_BINDING_ID_V1) {
        return Err("binding identity");
    }
    Ok(())
}

fn generated_host_contract<K: CompilerGeneratedKernelExpectationV1>() -> [u8; 32] {
    K::PROFILE.generated_host_contract_identity()
}

fn derive_challenge<K: CompilerGeneratedKernelExpectationV1>(
    lineage: WorkerV3HostLineageIdentityV1,
    generated_host_contract: [u8; 32],
) -> WorkerV3VerificationChallengeIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(WORKER_V3_VERIFICATION_CHALLENGE_DOMAIN_V1);
    digest.update(lineage.as_bytes());
    digest.update(K::KERNEL_BINDING_ID_V1);
    digest.update(
        u64::try_from(K::LOGICAL_NAME.len())
            .expect("generated marker name length fits u64")
            .to_le_bytes(),
    );
    digest.update(K::LOGICAL_NAME.as_bytes());
    digest.update(
        u64::try_from(K::EXPORT_NAME.len())
            .expect("generated export name length fits u64")
            .to_le_bytes(),
    );
    digest.update(K::EXPORT_NAME.as_bytes());
    digest.update(generated_host_contract);
    WorkerV3VerificationChallengeIdentityV1(digest.finalize().into())
}

fn validate_decision<K: CompilerGeneratedKernelExpectationV1>(
    request: &WorkerV3VerificationRequestV1<'_, K>,
    decision: &WorkerV3VerificationDecisionV1,
) -> Result<(), WorkerV3VerificationDecisionErrorV1> {
    for (matches, field) in [
        (
            decision.challenge == request.challenge,
            "verification challenge",
        ),
        (
            decision.lineage == request.lineage.identity(),
            "host lineage",
        ),
        (
            decision.finalizer_derivation.identity() == request.finalizer_derivation.identity(),
            "finalizer derivation",
        ),
        (
            decision.kernel_id == request.descriptor.kernel_id(),
            "kernel identity",
        ),
        (
            decision.marker_binding == K::KERNEL_BINDING_ID_V1,
            "generated marker binding",
        ),
        (
            decision.generated_host_contract == request.generated_host_contract,
            "generated host contract",
        ),
        (
            decision.capsule_sha256 == request.lineage.capsule_sha256(),
            "semantic capsule",
        ),
        (
            decision.formal_memory_sha256 == request.lineage.formal_memory_sha256(),
            "formal memory receipt",
        ),
        (
            decision.proof_binding_sha256 == request.lineage.proof_binding_sha256(),
            "proof binding receipt",
        ),
        (
            decision.finalized_sha256 == request.lineage.finalized_sha256(),
            "finalized HSACO",
        ),
        (
            decision.finalized_length == request.lineage.finalized_length(),
            "finalized HSACO length",
        ),
        (decision.target == request.target, "target"),
        (
            decision.code_object_version == request.code_object_version,
            "code-object version",
        ),
        (
            decision.compiler_execution.subject_sha256
                == request.compiler_execution_subject_sha256(),
            "compiler-execution subject",
        ),
        (
            decision.compiler_execution.carriage_sha256
                == request.compiler_execution_carriage_sha256(),
            "compiler-execution carriage",
        ),
        (
            decision.compiler_execution.policy_sha256 == request.compiler_execution_policy_sha256(),
            "compiler-execution policy",
        ),
        (
            decision.compiler_execution.issuer_journal_sha256
                == request.compiler_execution_issuer_journal_sha256(),
            "compiler-execution issuer journal",
        ),
        (
            decision.compiler_execution.compiler_occurrence_sha256
                == request.compiler_occurrence_sha256(),
            "compiler occurrence",
        ),
        (
            decision.compiler_execution.receipt_sha256
                == request.compiler_execution_receipt_sha256(),
            "compiler-execution receipt",
        ),
        (
            decision.compiler_execution.publication_sha256
                == request.compiler_execution_publication_sha256(),
            "compiler-execution receipt publication",
        ),
        (
            decision.compiler_execution.acknowledgment_sha256
                == request.compiler_execution_acknowledgment_sha256(),
            "compiler-execution publication acknowledgment",
        ),
        (
            decision.compiler_execution.worker_ledger_record_sha256
                == request.compiler_execution_worker_ledger_record_sha256(),
            "compiler-execution Worker ledger record",
        ),
        (
            decision.compiler_execution.sequence == request.compiler_execution_sequence(),
            "compiler-execution rollback sequence",
        ),
        (
            decision.compiler_execution.prior_rollback_anchor
                == request.compiler_execution_prior_rollback_anchor(),
            "compiler-execution prior rollback anchor",
        ),
        (
            decision.compiler_execution.current_rollback_anchor
                == request.compiler_execution_current_rollback_anchor(),
            "compiler-execution current rollback anchor",
        ),
    ] {
        if !matches {
            return Err(WorkerV3VerificationDecisionErrorV1::IdentityMismatch(field));
        }
    }
    for (identity, field) in [
        (decision.verifier_measurement_sha256, "verifier measurement"),
        (
            decision.verification_transcript_sha256,
            "verification transcript",
        ),
        (
            decision.proof_executable_binding_sha256,
            "proof/executable binding",
        ),
        (
            decision
                .compiler_execution
                .current_record_verification_sha256,
            "compiler current-record verification",
        ),
        (
            decision
                .compiler_execution
                .current_record_attestation_sha256,
            "compiler current-record attestation",
        ),
        (
            decision
                .compiler_execution
                .protected_policy_verification_sha256,
            "protected compiler policy verification",
        ),
        (
            decision
                .compiler_execution
                .protected_worker_ledger_verification_sha256,
            "protected Worker ledger verification",
        ),
        (
            decision
                .compiler_execution
                .external_rollback_verification_sha256,
            "external rollback verification",
        ),
        (
            decision.rust_type_layout_contract_sha256,
            "Rust type/layout contract",
        ),
        (decision.rust_effect_contract_sha256, "Rust effect contract"),
    ] {
        if identity == [0; 32] {
            return Err(WorkerV3VerificationDecisionErrorV1::ZeroAuthenticatedIdentity(field));
        }
    }
    for property in [
        WorkerV3SafetyPropertyV1::Bounds,
        WorkerV3SafetyPropertyV1::AddressOverflowFreedom,
        WorkerV3SafetyPropertyV1::MemorySafety,
        WorkerV3SafetyPropertyV1::Initialization,
        WorkerV3SafetyPropertyV1::RaceFreedom,
        WorkerV3SafetyPropertyV1::LaunchValidity,
        WorkerV3SafetyPropertyV1::Synchronization,
        WorkerV3SafetyPropertyV1::SemanticRefinement,
    ] {
        if !decision.safety_properties.contains(property) {
            return Err(WorkerV3VerificationDecisionErrorV1::MissingSafetyProperty(
                property,
            ));
        }
    }
    validate_decision_proof_inputs(request, decision)?;
    validate_decision_target_lineage(request, decision)?;
    Ok(())
}

fn validate_decision_proof_inputs<K: CompilerGeneratedKernelExpectationV1>(
    request: &WorkerV3VerificationRequestV1<'_, K>,
    decision: &WorkerV3VerificationDecisionV1,
) -> Result<(), WorkerV3VerificationDecisionErrorV1> {
    #[cfg(not(feature = "worker-v3-verifier-test-support"))]
    let WorkerV3ProofInputEvidenceV1::Validated(inputs) = &decision.proof_inputs;
    #[cfg(feature = "worker-v3-verifier-test-support")]
    let inputs = match &decision.proof_inputs {
        WorkerV3ProofInputEvidenceV1::Validated(inputs) => inputs,
        WorkerV3ProofInputEvidenceV1::Synthetic => return Ok(()),
    };
    let receipts = request.handoff.capsule().receipts();
    for (matches, field) in [
        (
            inputs.association().canonical_bytes() == receipts.proof_binding().canonical_preimage(),
            "proof-binding association",
        ),
        (
            inputs.verus_execution().canonical_bytes()
                == inputs.association().verus_execution_evidence(),
            "aggregate Verus execution",
        ),
        (
            inputs.semantic_mir().canonical_encoding()
                == receipts.semantic_mir().canonical_preimage(),
            "semantic MIR",
        ),
        (
            inputs.middle_end().canonical_bytes() == receipts.middle_end().canonical_preimage(),
            "middle-end evidence",
        ),
        (
            inputs.kernel_ir().canonical_bytes() == receipts.kernel_ir().canonical_preimage(),
            "Kernel IR",
        ),
        (
            inputs.correspondence().canonical_bytes()
                == receipts.mir_to_kir_correspondence().canonical_preimage(),
            "MIR-to-KIR correspondence",
        ),
        (
            inputs.formal_memory().canonical_bytes()
                == receipts.formal_memory().canonical_preimage(),
            "formal-memory admission",
        ),
    ] {
        if !matches {
            return Err(WorkerV3VerificationDecisionErrorV1::ProofInputMismatch(
                field,
            ));
        }
    }
    if inputs.receipt_identity() != receipts.proof_binding().identity() {
        return Err(WorkerV3VerificationDecisionErrorV1::ProofInputMismatch(
            "proof-binding receipt identity",
        ));
    }
    Ok(())
}

fn validate_decision_target_lineage<K: CompilerGeneratedKernelExpectationV1>(
    request: &WorkerV3VerificationRequestV1<'_, K>,
    decision: &WorkerV3VerificationDecisionV1,
) -> Result<(), WorkerV3VerificationDecisionErrorV1> {
    #[cfg(not(feature = "worker-v3-verifier-test-support"))]
    let WorkerV3TargetLineageEvidenceV1::Validated(lineage) = &decision.target_lineage;
    #[cfg(feature = "worker-v3-verifier-test-support")]
    let lineage = match &decision.target_lineage {
        WorkerV3TargetLineageEvidenceV1::Validated(lineage) => lineage,
        WorkerV3TargetLineageEvidenceV1::Synthetic => return Ok(()),
    };
    let capsule = request.handoff.capsule();
    let receipts = capsule.receipts();
    let module = request.handoff.module_handoff().module_identity();
    let finalizer_module = request.finalizer_derivation.compiler_module_identity();
    let final_llvm = lineage.final_llvm_identity();
    let target_binding = lineage.target_binding_receipt_identity();
    let data_layout = lineage.data_layout_receipt_identity();
    let semantic_to_llvm = lineage.semantic_to_llvm_receipt_identity();
    let final_commitment = lineage.final_compiler_module_commitment_identity();
    let target_inputs = lineage.target_binding().inputs().map_err(|_| {
        WorkerV3VerificationDecisionErrorV1::TargetLineageMismatch(
            "target-binding transcript inputs",
        )
    })?;
    let descriptor_workgroup = match request.descriptor().launch().block_size() {
        BlockSizeV1::Exact(dimensions) => [dimensions.x(), dimensions.y(), dimensions.z()],
        BlockSizeV1::Any | BlockSizeV1::AtMost(_) => {
            return Err(WorkerV3VerificationDecisionErrorV1::TargetLineageMismatch(
                "exact descriptor workgroup",
            ));
        }
    };
    for (matches, field) in [
        (
            lineage.target_binding().canonical_bytes()
                == receipts.target_binding().canonical_preimage(),
            "target-binding transcript",
        ),
        (
            lineage.data_layout().canonical_bytes() == receipts.data_layout().canonical_preimage(),
            "data-layout transcript",
        ),
        (
            lineage.semantic_to_llvm().canonical_bytes()
                == lineage.semantic_to_llvm_association_bytes(),
            "semantic-to-LLVM transcript",
        ),
        (
            target_binding.sha256() == *receipts.target_binding().identity().sha256()
                && target_binding.byte_len() == receipts.target_binding().identity().byte_len(),
            "target-binding receipt",
        ),
        (
            data_layout.sha256() == *receipts.data_layout().identity().sha256()
                && data_layout.byte_len() == receipts.data_layout().identity().byte_len(),
            "data-layout receipt",
        ),
        (
            semantic_to_llvm.sha256() == *receipts.semantic_to_llvm().identity().sha256()
                && semantic_to_llvm.byte_len() == receipts.semantic_to_llvm().identity().byte_len(),
            "semantic-to-LLVM receipt",
        ),
        (
            lineage.replay().kernel_ir_receipt_identity() == receipts.kernel_ir().identity(),
            "replayed Kernel IR receipt",
        ),
        (
            lineage.replay().amdgpu_lowering_receipt_identity()
                == receipts.amdgpu_lowering().identity(),
            "replayed AMDGPU-lowering receipt",
        ),
        (
            final_llvm.sha256() == *module.sha256() && final_llvm.byte_len() == module.byte_len(),
            "final LLVM module",
        ),
        (
            final_llvm.sha256() == *finalizer_module.sha256()
                && final_llvm.byte_len() == finalizer_module.byte_len(),
            "finalizer compiler module",
        ),
        (
            final_commitment.sha256()
                == *receipts
                    .final_compiler_module_commitment()
                    .identity()
                    .sha256()
                && final_commitment.byte_len()
                    == receipts
                        .final_compiler_module_commitment()
                        .identity()
                        .byte_len(),
            "final compiler-module commitment",
        ),
        (
            target_inputs.code_object_version == u16::from(request.code_object_version().number()),
            "code-object version",
        ),
        (
            target_inputs.default_workgroup == descriptor_workgroup,
            "default workgroup",
        ),
    ] {
        if !matches {
            return Err(WorkerV3VerificationDecisionErrorV1::TargetLineageMismatch(
                field,
            ));
        }
    }
    Ok(())
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3VerificationAuthenticationErrorV1<E> {
    Marker(&'static str),
    UnsupportedGeneratedProfile,
    CurrentPublication(RecoveredWorkerV3AdmissionErrorV1),
    Verifier(E),
    Decision(WorkerV3VerificationDecisionErrorV1),
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3VerificationAuditErrorV1<E> {
    Marker(&'static str),
    UnsupportedGeneratedProfile,
    CurrentPublication(RecoveredWorkerV3AdmissionErrorV1),
    Auditor(E),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3VerificationDecisionErrorV1 {
    IdentityMismatch(&'static str),
    ZeroAuthenticatedIdentity(&'static str),
    MissingSafetyProperty(WorkerV3SafetyPropertyV1),
    ProofInputMismatch(&'static str),
    TargetLineageMismatch(&'static str),
}

impl<E: fmt::Display> fmt::Display for WorkerV3VerificationAuthenticationErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Marker(field) => write!(formatter, "generated marker {field} mismatch"),
            Self::UnsupportedGeneratedProfile => formatter
                .write_str("Worker V3 verification requires a generated host-contract identity"),
            Self::CurrentPublication(error) => {
                write!(
                    formatter,
                    "Worker V3 publication revalidation failed: {error}"
                )
            }
            Self::Verifier(error) => write!(formatter, "reviewed V3 verifier failed: {error}"),
            Self::Decision(error) => write!(formatter, "invalid V3 verifier decision: {error}"),
        }
    }
}

impl fmt::Display for WorkerV3VerificationDecisionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdentityMismatch(field) => write!(formatter, "{field} identity mismatch"),
            Self::ZeroAuthenticatedIdentity(field) => {
                write!(formatter, "{field} identity is zero")
            }
            Self::MissingSafetyProperty(property) => {
                write!(formatter, "missing safety property {property:?}")
            }
            Self::ProofInputMismatch(field) => {
                write!(
                    formatter,
                    "validated compiler {field} differs from the exact request"
                )
            }
            Self::TargetLineageMismatch(field) => {
                write!(
                    formatter,
                    "validated compiler {field} differs from the exact target lineage"
                )
            }
        }
    }
}

impl fmt::Display for WorkerV3CompilerExecutionEvidenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestMismatch => formatter.write_str(
                "compiler current-record evidence subject differs from its receipt carriage",
            ),
            Self::IdentityMismatch(field) => {
                write!(
                    formatter,
                    "compiler current-record {field} identity mismatch"
                )
            }
            Self::MissingAuthenticatedEvidence(field) => {
                write!(
                    formatter,
                    "compiler current-record {field} evidence is missing"
                )
            }
        }
    }
}

impl<E: fmt::Display> fmt::Display for WorkerV3VerificationAuditErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Marker(field) => write!(formatter, "generated marker {field} mismatch"),
            Self::UnsupportedGeneratedProfile => {
                formatter.write_str("Worker V3 audit requires a generated host-contract identity")
            }
            Self::CurrentPublication(error) => {
                write!(
                    formatter,
                    "Worker V3 publication revalidation failed: {error}"
                )
            }
            Self::Auditor(error) => write!(formatter, "reviewed V3 audit failed: {error}"),
        }
    }
}

impl<E> Error for WorkerV3VerificationAuthenticationErrorV1<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurrentPublication(error) => Some(error),
            Self::Verifier(error) => Some(error),
            Self::Decision(error) => Some(error),
            Self::Marker(_) | Self::UnsupportedGeneratedProfile => None,
        }
    }
}

impl Error for WorkerV3VerificationDecisionErrorV1 {}

impl Error for WorkerV3CompilerExecutionEvidenceErrorV1 {}

impl<E> Error for WorkerV3VerificationAuditErrorV1<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CurrentPublication(error) => Some(error),
            Self::Auditor(error) => Some(error),
            Self::Marker(_) | Self::UnsupportedGeneratedProfile => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_worker_v3_properties_are_explicit_and_complete() {
        let required = WorkerV3SafetyPropertiesV1::required();
        assert_eq!(required.bits(), u8::MAX);
        assert_eq!(WorkerV3SafetyPropertiesV1::new(u8::MAX), Some(required));
        for property in [
            WorkerV3SafetyPropertyV1::Bounds,
            WorkerV3SafetyPropertyV1::AddressOverflowFreedom,
            WorkerV3SafetyPropertyV1::MemorySafety,
            WorkerV3SafetyPropertyV1::Initialization,
            WorkerV3SafetyPropertyV1::RaceFreedom,
            WorkerV3SafetyPropertyV1::LaunchValidity,
            WorkerV3SafetyPropertyV1::Synchronization,
            WorkerV3SafetyPropertyV1::SemanticRefinement,
        ] {
            assert!(required.contains(property));
        }
    }
}
