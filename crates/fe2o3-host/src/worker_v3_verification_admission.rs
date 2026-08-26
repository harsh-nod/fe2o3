use std::{error::Error, fmt, marker::PhantomData};

use fe2o3_artifact_transaction::DurableCurrentLinkPublicationTokenV1;
use fe2o3_hsaco::CodeObjectVersion;
use fe2o3_kernel_descriptor::{KernelDescriptorV1, KernelId};
use sha2::{Digest, Sha256};

use crate::recovered_worker_v3_admission::WorkerV3HostLineageEvidenceV1;
use crate::{
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedKernelProfileV1, DeviceIdentity,
    RecoveredWorkerV3AdmissionErrorV1, RecoveredWorkerV3PinnedDescriptorV1,
    WorkerV3HostLineageIdentityV1,
};

const WORKER_V3_VERIFICATION_CHALLENGE_DOMAIN_V1: &[u8] =
    b"fe2o3.host.worker-v3-verification-challenge.v1\0";

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
        }
    }
}

/// Canonical set of properties reported through the reviewed V3 verifier boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3SafetyPropertiesV1(u8);

impl WorkerV3SafetyPropertiesV1 {
    const KNOWN_BITS: u8 = (1 << 6) - 1;

    pub const fn new(bits: u8) -> Option<Self> {
        if bits & !Self::KNOWN_BITS == 0 {
            Some(Self(bits))
        } else {
            None
        }
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
    handoff: &'admission fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
    finalized_hsaco: &'admission [u8],
    descriptor: &'admission KernelDescriptorV1,
    target: fe2o3_amd_target::AmdTargetId,
    code_object_version: CodeObjectVersion,
    device: &'admission DeviceIdentity,
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

    pub const fn device(&self) -> &DeviceIdentity {
        self.device
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
/// approved policy. They must establish that the formal-memory and proof-binding receipts apply
/// to this exact semantic capsule, descriptor, final HSACO, and generated Rust marker, and that
/// every reported safety property covers all executable memory effects. The inert V3 receipts do
/// not establish these claims by themselves. A false implementation can later authorize native
/// code loading from safe generated code.
pub unsafe trait WorkerV3VerifierV1<K: CompilerGeneratedKernelExpectationV1> {
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

/// Descriptive result returned by a reviewed V3 verifier.
///
/// Public construction grants no authority. Only the private promotion transition can compare
/// every field to an admitted request and retain it as authenticated state.
#[derive(Clone, Debug, Eq, PartialEq)]
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
    verifier_measurement_sha256: [u8; 32],
    verification_transcript_sha256: [u8; 32],
    proof_executable_binding_sha256: [u8; 32],
    rust_type_layout_contract_sha256: [u8; 32],
    rust_effect_contract_sha256: [u8; 32],
    safety_properties: WorkerV3SafetyPropertiesV1,
}

impl WorkerV3VerificationDecisionV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
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
            verifier_measurement_sha256,
            verification_transcript_sha256,
            proof_executable_binding_sha256,
            rust_type_layout_contract_sha256,
            rust_effect_contract_sha256,
            safety_properties,
        }
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
}

/// Authenticated compiler/Verus state for one exact V3 executable.
///
/// This value is linear and still grants no HSA load or launch authority. A later transition must
/// bind it to a reviewed HSA runtime and a retained current-publication token.
pub struct AuthenticatedWorkerV3ExecutableV1<K> {
    admission: RecoveredWorkerV3PinnedDescriptorV1,
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
        drop(current);
        Ok(Self {
            admission,
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

    pub const fn device(&self) -> &DeviceIdentity {
        self.admission.device()
    }

    pub fn revalidate_currentness(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.admission.revalidate_currentness()
    }

    pub fn authorize_hsa_load<A: crate::ReviewedHsaExecutableLifecycleAdapterV1>(
        self,
        adapter: A,
    ) -> Result<
        crate::AuthorizedWorkerV3HsaLoadV1<K, A>,
        crate::WorkerV3HsaLoadAuthorizationErrorV1<A::Error>,
    > {
        crate::hsa_executable_lifecycle::authorize_worker_v3_hsa_load_v1(self, adapter)
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
}

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
        handoff: admission.outer_handoff(),
        finalized_hsaco: current.exact_artifact_bytes(),
        descriptor: admission.descriptor(),
        target: admission.target(),
        code_object_version: admission.code_object_version(),
        device: admission.device(),
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
    match K::PROFILE {
        CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
            generated_host_contract_identity,
        } => generated_host_contract_identity,
        CompilerGeneratedKernelProfileV1::TypedVecAddF32V1
        | CompilerGeneratedKernelProfileV1::TypedVecAddF32RustcLayoutV2 => [0; 32],
    }
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
    ] {
        if !decision.safety_properties.contains(property) {
            return Err(WorkerV3VerificationDecisionErrorV1::MissingSafetyProperty(
                property,
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
