//! One-shot host admission for the completed Worker V3 capability-result carrier.

use std::fmt;

use fe2o3_artifact_transaction::RetainedDurableDirectoryV1;
use fe2o3_compiler_ffi::{
    InertProductionCapabilityResultIdentityV5, InertProductionCapabilityResultV5,
};
use fe2o3_runtime_protocol::{
    ConsumedWorkerV3CapabilityResultCarrierV1, RecoveredWorkerV3CapabilityResultCarrierV1,
    RecoveredWorkerV3LoadEnvelopeV2, WorkerV3CapabilityResultCarrierErrorV1,
    WorkerV3CapabilityResultCarrierIdentityV1, WorkerV3CapabilityResultCarrierWireV1,
    WorkerV3DynamicPreconditionRosterIdentityV1,
};
use sha2::{Digest, Sha256};

#[cfg(target_os = "linux")]
use crate::application_descriptor_handoff::RetainedWorkerV3ApplicationDescriptorsV1;
use crate::{
    AdmittedGeneratedHostContractV2, AuthenticatedWorkerV3RosterEntryV1,
    AuthenticatedWorkerV3RosterV1, CompilerGeneratedKernelExpectationRosterV1,
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedKernelExpectationV2,
    GeneratedHostContractErrorV2, ProductionGeneratedHostFactsV2,
    RecoveredWorkerV3AdmissionErrorV1, RecoveredWorkerV3PinnedRosterV1,
    WorkerV3ProtectedRosterVerificationEvidenceV1, WorkerV3ProtectedRosterVerifierAdapterV1,
    WorkerV3ProtectedRosterVerifierBackendV1, WorkerV3RosterEntryErrorV1,
    WorkerV3RosterVerificationAuthenticationErrorV1, WorkerV3RosterVerificationRequestV1,
    admit_recovered_worker_v3_roster_v1,
};

const SEALED_CAPABILITY_ROSTER_ADMISSION_DOMAIN_V1: &[u8] =
    b"fe2o3.host.worker-v3-capability-roster-admission.v1\0";
const PROTECTED_CAPABILITY_ROSTER_DECISION_DOMAIN_V1: &[u8] =
    b"fe2o3.host.worker-v3-capability-roster-decision.v1\0";

/// Exact completed-result-to-generated-roster mismatch.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3CapabilityResultRosterBindingErrorV1 {
    EntryCountMismatch {
        expected: usize,
        actual: usize,
    },
    KernelMismatch {
        ordinal: usize,
        expected: [u8; 32],
        actual: [u8; 32],
    },
    AssociationSubjectMismatch {
        ordinal: usize,
    },
}

impl fmt::Display for WorkerV3CapabilityResultRosterBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EntryCountMismatch { expected, actual } => write!(
                formatter,
                "completed capability roster has {actual} entries; generated roster has {expected}"
            ),
            Self::KernelMismatch { ordinal, .. } => write!(
                formatter,
                "completed capability subject {ordinal} names a different generated kernel"
            ),
            Self::AssociationSubjectMismatch { ordinal } => write!(
                formatter,
                "completed capability association {ordinal} names a different subject"
            ),
        }
    }
}

impl std::error::Error for WorkerV3CapabilityResultRosterBindingErrorV1 {}

/// Failure to join exact recovered V2 custody, the V5 carrier, and the generated roster.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3CapabilityResultRosterAdmissionFailureV1 {
    CapabilityBinding {
        error: WorkerV3CapabilityResultRosterBindingErrorV1,
        envelope: RecoveredWorkerV3LoadEnvelopeV2,
        carrier: RecoveredWorkerV3CapabilityResultCarrierV1,
    },
    HostAdmission {
        error: RecoveredWorkerV3AdmissionErrorV1,
        carrier: RecoveredWorkerV3CapabilityResultCarrierV1,
    },
}

/// Application-startup failure while joining inherited V2 custody to the exact V5 carrier.
#[derive(Debug)]
#[non_exhaustive]
pub enum RecoveredWorkerV3CapabilityApplicationAdmissionErrorV1 {
    CapabilityBinding(WorkerV3CapabilityResultRosterBindingErrorV1),
    HostAdmission(RecoveredWorkerV3AdmissionErrorV1),
}

impl fmt::Display for RecoveredWorkerV3CapabilityApplicationAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapabilityBinding(error) => error.fmt(formatter),
            Self::HostAdmission(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RecoveredWorkerV3CapabilityApplicationAdmissionErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CapabilityBinding(error) => Some(error),
            Self::HostAdmission(error) => Some(error),
        }
    }
}

/// Move-only inherited application custody awaiting protected aggregate authentication.
#[derive(Debug)]
#[must_use = "the inherited capability application must enter sealed authentication"]
pub struct RecoveredWorkerV3CapabilityApplicationV1<R> {
    roster: RecoveredWorkerV3CapabilityRosterV1<R>,
    durable: RetainedDurableDirectoryV1,
}

impl<R: CompilerGeneratedKernelExpectationRosterV1> RecoveredWorkerV3CapabilityApplicationV1<R> {
    pub(crate) fn admit(
        envelope: RecoveredWorkerV3LoadEnvelopeV2,
        carrier: RecoveredWorkerV3CapabilityResultCarrierV1,
        durable: RetainedDurableDirectoryV1,
    ) -> Result<Self, RecoveredWorkerV3CapabilityApplicationAdmissionErrorV1> {
        let roster =
            RecoveredWorkerV3CapabilityRosterV1::admit(envelope, carrier).map_err(|failure| {
                match failure {
                    WorkerV3CapabilityResultRosterAdmissionFailureV1::CapabilityBinding {
                        error,
                        ..
                    } => RecoveredWorkerV3CapabilityApplicationAdmissionErrorV1::CapabilityBinding(
                        error,
                    ),
                    WorkerV3CapabilityResultRosterAdmissionFailureV1::HostAdmission {
                        error,
                        ..
                    } => {
                        RecoveredWorkerV3CapabilityApplicationAdmissionErrorV1::HostAdmission(error)
                    }
                }
            })?;
        Ok(Self { roster, durable })
    }

    pub fn production_result(&self) -> &InertProductionCapabilityResultV5 {
        self.roster.production_result()
    }

    pub fn revalidate_currentness(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.roster.roster.revalidate_currentness()
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn retain_application_descriptors(
        mut self,
        descriptors: RetainedWorkerV3ApplicationDescriptorsV1,
    ) -> Self {
        self.roster.roster = self
            .roster
            .roster
            .retain_application_descriptors(descriptors);
        self
    }

    /// Authenticates the complete roster and commits the one-shot carrier in the same directory.
    pub fn authenticate<B>(
        self,
        backend: &mut B,
    ) -> Result<
        AuthenticatedWorkerV3CapabilityApplicationV1<R>,
        WorkerV3CapabilityRosterAuthenticationFailureV1<R, B::Error>,
    >
    where
        B: WorkerV3ProtectedCapabilityRosterVerifierBackendV1<R>,
    {
        let Self { roster, durable } = self;
        let roster = roster.authenticate(&durable, backend)?;
        Ok(AuthenticatedWorkerV3CapabilityApplicationV1 { roster, durable })
    }
}

/// Authenticated roster plus the exact durable directory that consumed its V5 carrier.
#[derive(Debug)]
#[must_use = "authenticated application custody must remain live through dispatch completion"]
pub struct AuthenticatedWorkerV3CapabilityApplicationV1<R> {
    roster: AuthenticatedWorkerV3CapabilityRosterV1<R>,
    durable: RetainedDurableDirectoryV1,
}

/// Failure to derive one typed generated host contract from authenticated aggregate custody.
#[derive(Debug)]
#[non_exhaustive]
pub enum CapabilityGeneratedHostAdmissionErrorV1 {
    RosterEntry(WorkerV3RosterEntryErrorV1),
    ProductionResult(String),
    Contract(GeneratedHostContractErrorV2),
}

impl fmt::Display for CapabilityGeneratedHostAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RosterEntry(error) => {
                write!(formatter, "generated roster entry rejected: {error}")
            }
            Self::ProductionResult(error) => {
                write!(
                    formatter,
                    "completed V5 result could not be retained: {error}"
                )
            }
            Self::Contract(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CapabilityGeneratedHostAdmissionErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RosterEntry(error) => Some(error),
            Self::Contract(error) => Some(error),
            Self::ProductionResult(_) => None,
        }
    }
}

impl<R: CompilerGeneratedKernelExpectationRosterV1>
    AuthenticatedWorkerV3CapabilityApplicationV1<R>
{
    pub const fn roster(&self) -> &AuthenticatedWorkerV3CapabilityRosterV1<R> {
        &self.roster
    }

    pub fn revalidate_currentness(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.roster.revalidate_currentness()
    }

    pub const fn durable_directory(&self) -> &RetainedDurableDirectoryV1 {
        &self.durable
    }

    /// Derives one ordinal-specific generated contract from the authenticated V5 roster and the
    /// independently inspected descriptor table.
    #[doc(hidden)]
    pub(crate) fn admit_generated_host_contract_v2<K>(
        &self,
    ) -> Result<AdmittedGeneratedHostContractV2<K>, CapabilityGeneratedHostAdmissionErrorV1>
    where
        K: CompilerGeneratedKernelExpectationV2,
    {
        let entry = self
            .roster
            .entry::<K>()
            .map_err(CapabilityGeneratedHostAdmissionErrorV1::RosterEntry)?;
        let ordinal = entry.ordinal();
        let descriptor = entry.descriptor();
        let table = self.roster.descriptor_table();
        let result = InertProductionCapabilityResultV5::decode(
            self.roster.production_result().canonical_bytes(),
        )
        .map_err(|error| {
            CapabilityGeneratedHostAdmissionErrorV1::ProductionResult(error.to_string())
        })?;
        let facts = ProductionGeneratedHostFactsV2::from_production_capability_result_v5(
            K::KERNEL_BINDING_ID_V1,
            K::PROFILE.generated_host_contract_identity(),
            table.device_target().to_string(),
            table.canonical_code_object_digest(),
            fe2o3_kernel_descriptor::DeviceDescriptorTableDigest::calculate(table).map_err(
                |_| {
                    CapabilityGeneratedHostAdmissionErrorV1::Contract(
                        GeneratedHostContractErrorV2::DescriptorIdentity,
                    )
                },
            )?,
            fe2o3_kernel_descriptor::KernelDescriptorDigest::calculate(descriptor),
            descriptor.entry_name().as_str(),
            ordinal,
            result,
        )
        .map_err(CapabilityGeneratedHostAdmissionErrorV1::Contract)?;
        crate::admit_generated_host_contract_v2(table, descriptor, facts)
            .map_err(CapabilityGeneratedHostAdmissionErrorV1::Contract)
    }
}

/// Inert joined custody awaiting one protected aggregate admission.
#[derive(Debug)]
#[must_use = "the completed capability result must be consumed by sealed host admission"]
pub struct RecoveredWorkerV3CapabilityRosterV1<R> {
    roster: RecoveredWorkerV3PinnedRosterV1<R>,
    carrier: RecoveredWorkerV3CapabilityResultCarrierV1,
}

impl<R: CompilerGeneratedKernelExpectationRosterV1> RecoveredWorkerV3CapabilityRosterV1<R> {
    /// Joins the exact completed V5 roster to the generated roster before verification.
    pub fn admit(
        envelope: RecoveredWorkerV3LoadEnvelopeV2,
        carrier: RecoveredWorkerV3CapabilityResultCarrierV1,
    ) -> Result<Self, WorkerV3CapabilityResultRosterAdmissionFailureV1> {
        if let Err(error) = validate_result_roster::<R>(carrier.production_result()) {
            return Err(
                WorkerV3CapabilityResultRosterAdmissionFailureV1::CapabilityBinding {
                    error,
                    envelope,
                    carrier,
                },
            );
        }
        let roster = match admit_recovered_worker_v3_roster_v1(envelope) {
            Ok(roster) => roster,
            Err(error) => {
                return Err(
                    WorkerV3CapabilityResultRosterAdmissionFailureV1::HostAdmission {
                        error,
                        carrier,
                    },
                );
            }
        };
        Ok(Self { roster, carrier })
    }

    pub const fn production_result(&self) -> &InertProductionCapabilityResultV5 {
        self.carrier.production_result()
    }

    pub const fn carrier_identity(&self) -> WorkerV3CapabilityResultCarrierIdentityV1 {
        self.carrier.identity()
    }

    pub const fn grants_verification_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Runs the existing sealed aggregate admission and then durably consumes the carrier once.
    pub fn authenticate<B>(
        self,
        directory: &RetainedDurableDirectoryV1,
        backend: &mut B,
    ) -> Result<
        AuthenticatedWorkerV3CapabilityRosterV1<R>,
        WorkerV3CapabilityRosterAuthenticationFailureV1<R, B::Error>,
    >
    where
        B: WorkerV3ProtectedCapabilityRosterVerifierBackendV1<R>,
    {
        let Self { roster, carrier } = self;
        let authentication = {
            let mut verifier =
                WorkerV3ProtectedRosterVerifierAdapterV1::new(CapabilityRosterBackendBridgeV1 {
                    backend,
                    carrier: carrier.wire(),
                });
            AuthenticatedWorkerV3RosterV1::authenticate(roster, &mut verifier)
        };
        let authenticated = match authentication {
            Ok(authenticated) => authenticated,
            Err(failure) => {
                let (error, roster) = failure.into_parts();
                return Err(
                    WorkerV3CapabilityRosterAuthenticationFailureV1::Verification {
                        error,
                        roster,
                        carrier,
                    },
                );
            }
        };
        let sealed_identity = sealed_admission_identity(&authenticated, &carrier);
        match carrier.commit_sealed_host_consumption_v1(directory, sealed_identity) {
            Ok(carrier) => Ok(AuthenticatedWorkerV3CapabilityRosterV1 {
                roster: authenticated,
                carrier,
            }),
            Err(failure) => {
                let (error, carrier) = failure.into_parts();
                Err(
                    WorkerV3CapabilityRosterAuthenticationFailureV1::Consumption {
                        error,
                        roster: authenticated,
                        carrier,
                    },
                )
            }
        }
    }
}

/// Non-clone borrowed capability carrier presented to the protected aggregate backend.
pub struct WorkerV3CapabilityResultEvidenceViewV1<'carrier> {
    carrier: &'carrier WorkerV3CapabilityResultCarrierWireV1,
}

impl WorkerV3CapabilityResultEvidenceViewV1<'_> {
    pub const fn carrier_identity(&self) -> WorkerV3CapabilityResultCarrierIdentityV1 {
        self.carrier.identity()
    }

    pub const fn production_result_identity(&self) -> InertProductionCapabilityResultIdentityV5 {
        self.carrier.production_result_identity()
    }

    pub const fn production_result(&self) -> &InertProductionCapabilityResultV5 {
        self.carrier.production_result()
    }

    pub fn exact_carrier_bytes(&self) -> &[u8] {
        self.carrier.canonical_bytes()
    }

    pub const fn dynamic_precondition_roster_identity(
        &self,
    ) -> WorkerV3DynamicPreconditionRosterIdentityV1 {
        self.carrier.dynamic_precondition_roster_identity()
    }

    pub const fn grants_verification_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Protected roster evidence plus the exact V5 result identity independently authenticated.
pub struct WorkerV3ProtectedCapabilityRosterEvidenceV1 {
    roster: WorkerV3ProtectedRosterVerificationEvidenceV1,
    production_result: InertProductionCapabilityResultIdentityV5,
}

impl WorkerV3ProtectedCapabilityRosterEvidenceV1 {
    /// Constructs evidence returned by an implementation of the unsafe protected backend.
    ///
    /// # Safety
    ///
    /// `production_result` must identify the exact canonical result independently authenticated
    /// while producing `roster`; echoing the borrowed carrier identity is insufficient.
    pub const unsafe fn new(
        roster: WorkerV3ProtectedRosterVerificationEvidenceV1,
        production_result: InertProductionCapabilityResultIdentityV5,
    ) -> Self {
        Self {
            roster,
            production_result,
        }
    }
}

/// Protected authority for the complete generated roster and completed V5 result.
///
/// # Safety
///
/// Implementations must satisfy [`WorkerV3ProtectedRosterVerifierBackendV1`] and independently
/// authenticate the exact result bytes. They must bind every ordered kernel/root association,
/// source and machine receipt, compiler occurrence and currentness record, target model, launch
/// contract, and complete dynamic-precondition input roster to the exact physical request. Dynamic
/// requirements are authenticated here but remain obligations for generated per-dispatch host
/// admission; this transition does not claim that an unknown future launch satisfies them.
pub unsafe trait WorkerV3ProtectedCapabilityRosterVerifierBackendV1<
    R: CompilerGeneratedKernelExpectationRosterV1,
>
{
    type Error;

    /// Authenticates one exact joined roster without taking or duplicating carrier custody.
    ///
    /// # Safety
    ///
    /// The implementation obligations are those of the unsafe trait.
    unsafe fn verify_protected_capability_roster(
        &mut self,
        request: &WorkerV3RosterVerificationRequestV1<'_, R>,
        capability: WorkerV3CapabilityResultEvidenceViewV1<'_>,
    ) -> Result<WorkerV3ProtectedCapabilityRosterEvidenceV1, Self::Error>;
}

/// Failure at the protected-backend boundary before existing roster decision promotion.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3ProtectedCapabilityRosterVerifierErrorV1<E> {
    Backend(E),
    ProductionResultIdentityMismatch,
}

impl<E: fmt::Display> fmt::Display for WorkerV3ProtectedCapabilityRosterVerifierErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(error) => {
                write!(formatter, "protected capability verifier failed: {error}")
            }
            Self::ProductionResultIdentityMismatch => formatter.write_str(
                "protected capability verifier authenticated a different completed V5 result",
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for WorkerV3ProtectedCapabilityRosterVerifierErrorV1<E>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Backend(error) => Some(error),
            Self::ProductionResultIdentityMismatch => None,
        }
    }
}

struct CapabilityRosterBackendBridgeV1<'carrier, B> {
    backend: &'carrier mut B,
    carrier: &'carrier WorkerV3CapabilityResultCarrierWireV1,
}

// SAFETY: this crate-owned bridge adds an exact-result identity check and otherwise delegates the
// complete unsafe contract to `B`; it does not manufacture protected evidence.
unsafe impl<R, B> WorkerV3ProtectedRosterVerifierBackendV1<R>
    for CapabilityRosterBackendBridgeV1<'_, B>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
    B: WorkerV3ProtectedCapabilityRosterVerifierBackendV1<R>,
{
    type Error = WorkerV3ProtectedCapabilityRosterVerifierErrorV1<B::Error>;

    unsafe fn verify_protected_roster(
        &mut self,
        request: &WorkerV3RosterVerificationRequestV1<'_, R>,
    ) -> Result<WorkerV3ProtectedRosterVerificationEvidenceV1, Self::Error> {
        let expected = self.carrier.production_result_identity();
        // SAFETY: `B` owns the independent checks required by its unsafe trait.
        let evidence = unsafe {
            self.backend.verify_protected_capability_roster(
                request,
                WorkerV3CapabilityResultEvidenceViewV1 {
                    carrier: self.carrier,
                },
            )
        }
        .map_err(WorkerV3ProtectedCapabilityRosterVerifierErrorV1::Backend)?;
        if evidence.production_result != expected {
            return Err(
                WorkerV3ProtectedCapabilityRosterVerifierErrorV1::ProductionResultIdentityMismatch,
            );
        }
        Ok(evidence.roster)
    }
}

/// Authentication failure retaining whichever exact custody state reached the failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3CapabilityRosterAuthenticationFailureV1<R, E> {
    Verification {
        error: WorkerV3RosterVerificationAuthenticationErrorV1<
            WorkerV3ProtectedCapabilityRosterVerifierErrorV1<E>,
        >,
        roster: RecoveredWorkerV3PinnedRosterV1<R>,
        carrier: RecoveredWorkerV3CapabilityResultCarrierV1,
    },
    Consumption {
        error: WorkerV3CapabilityResultCarrierErrorV1,
        roster: AuthenticatedWorkerV3RosterV1<R>,
        carrier: RecoveredWorkerV3CapabilityResultCarrierV1,
    },
}

impl<R, E: fmt::Display> fmt::Display for WorkerV3CapabilityRosterAuthenticationFailureV1<R, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verification { error, .. } => error.fmt(formatter),
            Self::Consumption { error, .. } => {
                write!(
                    formatter,
                    "one-shot capability carrier consumption failed: {error}"
                )
            }
        }
    }
}

impl<R, E> std::error::Error for WorkerV3CapabilityRosterAuthenticationFailureV1<R, E>
where
    E: std::error::Error + 'static,
    R: fmt::Debug + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Verification { error, .. } => Some(error),
            Self::Consumption { error, .. } => Some(error),
        }
    }
}

/// Move-only authenticated roster retaining the durably consumed exact V5 carrier.
#[derive(Debug)]
#[must_use = "authenticated capability custody must remain with later host admission"]
pub struct AuthenticatedWorkerV3CapabilityRosterV1<R> {
    roster: AuthenticatedWorkerV3RosterV1<R>,
    carrier: ConsumedWorkerV3CapabilityResultCarrierV1,
}

impl<R: CompilerGeneratedKernelExpectationRosterV1> AuthenticatedWorkerV3CapabilityRosterV1<R> {
    pub const fn carrier_identity(&self) -> WorkerV3CapabilityResultCarrierIdentityV1 {
        self.carrier.wire().identity()
    }

    pub const fn production_result_identity(&self) -> InertProductionCapabilityResultIdentityV5 {
        self.carrier.wire().production_result_identity()
    }

    pub const fn production_result(&self) -> &InertProductionCapabilityResultV5 {
        self.carrier.production_result()
    }

    pub const fn dynamic_precondition_roster_identity(
        &self,
    ) -> WorkerV3DynamicPreconditionRosterIdentityV1 {
        self.carrier.wire().dynamic_precondition_roster_identity()
    }

    pub const fn sealed_host_admission_identity(&self) -> [u8; 32] {
        self.carrier.sealed_host_admission_identity()
    }

    pub const fn verification(&self) -> &crate::WorkerV3RosterVerificationDecisionV1 {
        self.roster.verification()
    }

    pub fn entry<K: CompilerGeneratedKernelExpectationV1>(
        &self,
    ) -> Result<AuthenticatedWorkerV3RosterEntryV1<'_, R, K>, WorkerV3RosterEntryErrorV1> {
        self.roster.entry::<K>()
    }

    pub fn revalidate_currentness(&self) -> Result<(), RecoveredWorkerV3AdmissionErrorV1> {
        self.roster.revalidate_currentness()
    }

    pub fn entry_count(&self) -> usize {
        self.roster.entry_count()
    }

    pub fn target(&self) -> fe2o3_amd_target::AmdTargetId {
        self.roster.target()
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

    pub(crate) fn exact_current_hsaco_bytes(&self) -> &[u8] {
        self.roster.exact_current_hsaco_bytes()
    }

    pub(crate) fn descriptor_table(&self) -> &fe2o3_kernel_descriptor::DeviceDescriptorTableV1 {
        self.roster.admitted_roster().descriptor_table()
    }
}

fn validate_result_roster<R: CompilerGeneratedKernelExpectationRosterV1>(
    result: &InertProductionCapabilityResultV5,
) -> Result<(), WorkerV3CapabilityResultRosterBindingErrorV1> {
    let expected = R::ENTRIES;
    let subjects = result.handoff().subjects();
    let associations = result.capability_associations().entries();
    if subjects.len() != expected.len() || associations.len() != expected.len() {
        return Err(
            WorkerV3CapabilityResultRosterBindingErrorV1::EntryCountMismatch {
                expected: expected.len(),
                actual: subjects.len().max(associations.len()),
            },
        );
    }
    for (ordinal, ((expected, subject), association)) in
        expected.iter().zip(subjects).zip(associations).enumerate()
    {
        let actual = *subject.kernel().digest().as_bytes();
        if actual != expected.kernel_binding_id() {
            return Err(
                WorkerV3CapabilityResultRosterBindingErrorV1::KernelMismatch {
                    ordinal,
                    expected: expected.kernel_binding_id(),
                    actual,
                },
            );
        }
        if association.subject() != *subject {
            return Err(
                WorkerV3CapabilityResultRosterBindingErrorV1::AssociationSubjectMismatch {
                    ordinal,
                },
            );
        }
    }
    Ok(())
}

fn sealed_admission_identity<R: CompilerGeneratedKernelExpectationRosterV1>(
    roster: &AuthenticatedWorkerV3RosterV1<R>,
    carrier: &RecoveredWorkerV3CapabilityResultCarrierV1,
) -> [u8; 32] {
    let decision = roster.verification();
    derive_sealed_admission_identity(
        carrier.identity(),
        carrier.production_result().identity(),
        carrier.dynamic_precondition_roster_identity(),
        decision.challenge_identity().as_bytes(),
        decision.lineage_identity().as_bytes(),
        decision.roster_identity().as_bytes(),
        decision.finalized_hsaco_sha256(),
        decision.finalized_hsaco_length(),
        protected_decision_identity(decision),
    )
}

fn protected_decision_identity(decision: &crate::WorkerV3RosterVerificationDecisionV1) -> [u8; 32] {
    let compiler = decision.compiler_execution();
    let mut digest = Sha256::new();
    digest.update(PROTECTED_CAPABILITY_ROSTER_DECISION_DOMAIN_V1);
    for identity in [
        decision.verifier_measurement_sha256(),
        decision.verification_transcript_sha256(),
        compiler.subject_sha256(),
        compiler.carriage_sha256(),
        compiler.policy_sha256(),
        compiler.issuer_journal_sha256(),
        compiler.compiler_occurrence_sha256(),
        compiler.receipt_sha256(),
        compiler.publication_sha256(),
        compiler.acknowledgment_sha256(),
        compiler.worker_ledger_record_sha256(),
        compiler.prior_rollback_anchor(),
        compiler.current_rollback_anchor(),
        compiler.current_record_verification_sha256(),
        compiler.current_record_attestation_sha256(),
        compiler.protected_policy_verification_sha256(),
        compiler.protected_worker_ledger_verification_sha256(),
        compiler.external_rollback_verification_sha256(),
    ] {
        digest.update(identity);
    }
    digest.update(compiler.sequence().to_le_bytes());
    digest.update((decision.entries().len() as u64).to_le_bytes());
    for entry in decision.entries() {
        digest.update(entry.lineage_identity().as_bytes());
        digest.update(entry.marker_binding_identity());
        digest.update(entry.generated_host_contract_identity());
        digest.update(entry.proof_executable_binding_sha256());
        digest.update(entry.rust_type_layout_contract_sha256());
        digest.update(entry.rust_effect_contract_sha256());
        digest.update([entry.safety_properties().bits()]);
    }
    digest.finalize().into()
}

#[allow(clippy::too_many_arguments)]
fn derive_sealed_admission_identity(
    carrier: WorkerV3CapabilityResultCarrierIdentityV1,
    result: InertProductionCapabilityResultIdentityV5,
    dynamic: WorkerV3DynamicPreconditionRosterIdentityV1,
    challenge: &[u8; 32],
    lineage: &[u8; 32],
    roster: &[u8; 32],
    artifact: [u8; 32],
    artifact_len: u64,
    protected_decision: [u8; 32],
) -> [u8; 32] {
    derive_sealed_admission_identity_parts(
        carrier.sha256(),
        carrier.byte_len(),
        result.sha256(),
        result.byte_len(),
        dynamic.as_bytes(),
        challenge,
        lineage,
        roster,
        artifact,
        artifact_len,
        protected_decision,
    )
}

#[allow(clippy::too_many_arguments)]
fn derive_sealed_admission_identity_parts(
    carrier_sha256: [u8; 32],
    carrier_len: u64,
    result_sha256: [u8; 32],
    result_len: u64,
    dynamic: &[u8; 32],
    challenge: &[u8; 32],
    lineage: &[u8; 32],
    roster: &[u8; 32],
    artifact: [u8; 32],
    artifact_len: u64,
    protected_decision: [u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(SEALED_CAPABILITY_ROSTER_ADMISSION_DOMAIN_V1);
    digest.update(carrier_sha256);
    digest.update(carrier_len.to_le_bytes());
    digest.update(result_sha256);
    digest.update(result_len.to_le_bytes());
    digest.update(dynamic);
    digest.update(challenge);
    digest.update(lineage);
    digest.update(roster);
    digest.update(artifact);
    digest.update(artifact_len.to_le_bytes());
    digest.update(protected_decision);
    digest.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate_kernel_roster(
        expected: &[[u8; 32]],
        actual: &[[u8; 32]],
    ) -> Result<(), WorkerV3CapabilityResultRosterBindingErrorV1> {
        if expected.len() != actual.len() {
            return Err(
                WorkerV3CapabilityResultRosterBindingErrorV1::EntryCountMismatch {
                    expected: expected.len(),
                    actual: actual.len(),
                },
            );
        }
        for (ordinal, (expected, actual)) in expected.iter().zip(actual).enumerate() {
            if expected != actual {
                return Err(
                    WorkerV3CapabilityResultRosterBindingErrorV1::KernelMismatch {
                        ordinal,
                        expected: *expected,
                        actual: *actual,
                    },
                );
            }
        }
        Ok(())
    }

    #[test]
    fn result_roster_rejects_omission_reorder_and_cross_kernel_splicing() {
        let expected = [[1; 32], [2; 32], [3; 32]];
        assert!(validate_kernel_roster(&expected, &expected).is_ok());
        assert!(matches!(
            validate_kernel_roster(&expected, &expected[..2]),
            Err(WorkerV3CapabilityResultRosterBindingErrorV1::EntryCountMismatch { .. })
        ));
        assert!(matches!(
            validate_kernel_roster(&expected, &[[2; 32], [1; 32], [3; 32]]),
            Err(WorkerV3CapabilityResultRosterBindingErrorV1::KernelMismatch { ordinal: 0, .. })
        ));
        assert!(matches!(
            validate_kernel_roster(&expected, &[[1; 32], [9; 32], [3; 32]]),
            Err(WorkerV3CapabilityResultRosterBindingErrorV1::KernelMismatch { ordinal: 1, .. })
        ));
    }

    #[test]
    fn sealed_identity_changes_for_every_bound_axis() {
        let baseline = derive_sealed_admission_identity_parts(
            [1; 32], 2, [3; 32], 4, &[5; 32], &[6; 32], &[7; 32], &[8; 32], [9; 32], 10, [11; 32],
        );
        for replacement in 11_u8..=21 {
            let actual = derive_sealed_admission_identity_parts(
                if replacement == 11 { [11; 32] } else { [1; 32] },
                if replacement == 12 { 12 } else { 2 },
                if replacement == 13 { [13; 32] } else { [3; 32] },
                if replacement == 14 { 14 } else { 4 },
                if replacement == 15 {
                    &[15; 32]
                } else {
                    &[5; 32]
                },
                if replacement == 16 {
                    &[16; 32]
                } else {
                    &[6; 32]
                },
                if replacement == 17 {
                    &[17; 32]
                } else {
                    &[7; 32]
                },
                if replacement == 18 {
                    &[18; 32]
                } else {
                    &[8; 32]
                },
                if replacement == 19 { [19; 32] } else { [9; 32] },
                if replacement == 20 { 20 } else { 10 },
                if replacement == 21 {
                    [21; 32]
                } else {
                    [11; 32]
                },
            );
            assert_ne!(actual, baseline, "axis {replacement}");
        }
    }
}
