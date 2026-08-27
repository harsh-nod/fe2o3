//! Live protected-rustc and current-publication occurrence join.

use std::error::Error;
use std::fmt;

use fe2o3_artifact_transaction::{
    AttemptCodecError, BUILD_ATTEMPT_ENV_V1, BuildAttempt, CompilerExecutionSubjectErrorV1,
    CompilerModuleHandoffConsumptionTokenV3, CompilerModuleHandoffCurrentnessLeaseV3,
    CompilerModuleHandoffErrorV3, CompilerModuleHandoffSlotV3, EmitError,
    InertCompilerExecutionSubjectV1, ProducerIdentity, ProducerIdentityFromRustcErrorV1,
    acquire_compiler_module_handoff_currentness_lease_v3,
    recover_compiler_module_handoff_receipt_in_slot_v3,
};
use fe2o3_rustc_invocation::{
    DigestError, InvocationDigestV3, RustcArgsErrorV2, RustcInvocationDescriptorV3,
};
use sha2::{Digest, Sha256};

use crate::{
    CompilerExecutionSupervisionErrorV1, ProtectedServiceAdmissionV1,
    ValidatedRemoteRustcProcessObservationV1,
};

const SHA256_BYTES: usize = 32;
const OCCURRENCE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/PROTECTED-COMPILER-EXECUTION-OCCURRENCE/V1\0";

enum CompilerExecutionOccurrenceCustodyV1 {
    Current {
        publication: CompilerModuleHandoffCurrentnessLeaseV3,
        observation: Box<ValidatedRemoteRustcProcessObservationV1>,
    },
    #[cfg(test)]
    Synthetic,
}

enum CompilerExecutionOccurrenceGuardCustodyV1 {
    Current(Box<CompilerModuleHandoffConsumptionTokenV3>),
    #[cfg(test)]
    Synthetic,
}

/// Opaque, move-only proof that one live protected rustc process produced the exact current V3
/// publication represented by a canonical compiler-execution subject.
///
/// Construction and every issuer use independently revalidate both the retained process and the
/// current publication. The type and its constructor remain private to the issuer service.
pub(crate) struct ProtectedCompilerExecutionOccurrenceV1 {
    subject: InertCompilerExecutionSubjectV1,
    identity: [u8; SHA256_BYTES],
    custody: CompilerExecutionOccurrenceCustodyV1,
}

impl fmt::Debug for ProtectedCompilerExecutionOccurrenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedCompilerExecutionOccurrenceV1")
            .field("subject", &self.subject.identity())
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl ProtectedCompilerExecutionOccurrenceV1 {
    pub(crate) fn observe_current(
        admission: &ProtectedServiceAdmissionV1,
    ) -> Result<Self, ProtectedCompilerExecutionOccurrenceErrorV1> {
        let observation = ValidatedRemoteRustcProcessObservationV1::observe(admission)
            .map_err(ProtectedCompilerExecutionOccurrenceErrorV1::Observe)?;
        let expected = ExpectedCompilerExecutionPublicationV1::derive(observation.descriptor())?;
        let output_dir = observation.artifact_directory_procfd_path();
        let receipt = recover_compiler_module_handoff_receipt_in_slot_v3(
            &output_dir,
            &expected.producer,
            expected.attempt,
            CompilerModuleHandoffSlotV3::Production,
        )
        .map_err(ProtectedCompilerExecutionOccurrenceErrorV1::RecoverPublication)?;
        let publication = acquire_compiler_module_handoff_currentness_lease_v3(
            &output_dir,
            &expected.producer,
            receipt,
        )
        .map_err(ProtectedCompilerExecutionOccurrenceErrorV1::AcquirePublicationLease)?;

        let token = publication
            .acquire_current_token()
            .map_err(ProtectedCompilerExecutionOccurrenceErrorV1::AcquireCurrentToken)?;
        let (subject, identity) =
            reconstruct_locked_occurrence(&publication, &observation, &token)?;
        drop(token);

        Ok(Self {
            subject,
            identity,
            custody: CompilerExecutionOccurrenceCustodyV1::Current {
                publication,
                observation: Box::new(observation),
            },
        })
    }

    pub(crate) const fn subject(&self) -> &InertCompilerExecutionSubjectV1 {
        &self.subject
    }

    #[cfg(test)]
    pub(crate) const fn identity(&self) -> &[u8; SHA256_BYTES] {
        &self.identity
    }

    #[cfg(test)]
    pub(crate) fn revalidate(&self) -> Result<(), ProtectedCompilerExecutionOccurrenceErrorV1> {
        let guard = self.acquire_for_issuer()?;
        drop(guard);
        Ok(())
    }

    pub(crate) fn acquire_for_issuer(
        &self,
    ) -> Result<
        ProtectedCompilerExecutionOccurrenceGuardV1<'_>,
        ProtectedCompilerExecutionOccurrenceErrorV1,
    > {
        match &self.custody {
            CompilerExecutionOccurrenceCustodyV1::Current {
                publication,
                observation,
            } => {
                let token = publication
                    .acquire_current_token()
                    .map_err(ProtectedCompilerExecutionOccurrenceErrorV1::AcquireCurrentToken)?;
                validate_locked_occurrence(self, publication, observation, &token)?;
                Ok(ProtectedCompilerExecutionOccurrenceGuardV1 {
                    occurrence: self,
                    custody: CompilerExecutionOccurrenceGuardCustodyV1::Current(Box::new(token)),
                })
            }
            #[cfg(test)]
            CompilerExecutionOccurrenceCustodyV1::Synthetic => {
                validate_synthetic(self)?;
                Ok(ProtectedCompilerExecutionOccurrenceGuardV1 {
                    occurrence: self,
                    custody: CompilerExecutionOccurrenceGuardCustodyV1::Synthetic,
                })
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn from_supervised_subject_for_test(
        subject: InertCompilerExecutionSubjectV1,
        identity: [u8; SHA256_BYTES],
    ) -> Result<Self, ProtectedCompilerExecutionOccurrenceErrorV1> {
        let occurrence = Self {
            subject,
            identity,
            custody: CompilerExecutionOccurrenceCustodyV1::Synthetic,
        };
        validate_synthetic(&occurrence)?;
        Ok(occurrence)
    }

    #[cfg(test)]
    pub(crate) const fn grants_publication_authority(&self) -> bool {
        false
    }

    #[cfg(test)]
    pub(crate) const fn grants_load_authority(&self) -> bool {
        false
    }

    #[cfg(test)]
    pub(crate) const fn grants_launch_authority(&self) -> bool {
        false
    }
}

pub(crate) struct ProtectedCompilerExecutionOccurrenceGuardV1<'a> {
    occurrence: &'a ProtectedCompilerExecutionOccurrenceV1,
    custody: CompilerExecutionOccurrenceGuardCustodyV1,
}

impl ProtectedCompilerExecutionOccurrenceGuardV1<'_> {
    pub(crate) const fn subject(&self) -> &InertCompilerExecutionSubjectV1 {
        self.occurrence.subject()
    }

    pub(crate) const fn identity(&self) -> &[u8; SHA256_BYTES] {
        &self.occurrence.identity
    }

    pub(crate) fn revalidate_immediately_before_signing(
        &self,
    ) -> Result<(), ProtectedCompilerExecutionOccurrenceErrorV1> {
        match (&self.occurrence.custody, &self.custody) {
            (
                CompilerExecutionOccurrenceCustodyV1::Current {
                    publication,
                    observation,
                },
                CompilerExecutionOccurrenceGuardCustodyV1::Current(token),
            ) => validate_locked_occurrence(self.occurrence, publication, observation, token),
            #[cfg(test)]
            (
                CompilerExecutionOccurrenceCustodyV1::Synthetic,
                CompilerExecutionOccurrenceGuardCustodyV1::Synthetic,
            ) => validate_synthetic(self.occurrence),
            #[cfg(test)]
            _ => Err(ProtectedCompilerExecutionOccurrenceErrorV1::InvalidSyntheticOccurrence),
        }
    }
}

struct ExpectedCompilerExecutionPublicationV1 {
    producer: ProducerIdentity,
    attempt: BuildAttempt,
    invocation_digest: [u8; SHA256_BYTES],
}

impl ExpectedCompilerExecutionPublicationV1 {
    fn derive(
        descriptor: &RustcInvocationDescriptorV3,
    ) -> Result<Self, ProtectedCompilerExecutionOccurrenceErrorV1> {
        let producer = ProducerIdentity::from_rustc_invocation_descriptor_v3(descriptor)
            .map_err(map_producer_identity_error)?;
        let attempt_value = descriptor
            .compile_environment()
            .entries()
            .iter()
            .find(|entry| entry.key() == BUILD_ATTEMPT_ENV_V1)
            .ok_or(ProtectedCompilerExecutionOccurrenceErrorV1::MissingBuildAttempt)?
            .value();
        let attempt = BuildAttempt::from_env_value(attempt_value)?;
        let invocation_digest = InvocationDigestV3::calculate(descriptor)?.into_bytes();
        Ok(Self {
            producer,
            attempt,
            invocation_digest,
        })
    }
}

fn map_producer_identity_error(
    error: ProducerIdentityFromRustcErrorV1,
) -> ProtectedCompilerExecutionOccurrenceErrorV1 {
    match error {
        ProducerIdentityFromRustcErrorV1::RustcArguments(error) => {
            ProtectedCompilerExecutionOccurrenceErrorV1::RustcArguments(error)
        }
        ProducerIdentityFromRustcErrorV1::NotCompileInvocation => {
            ProtectedCompilerExecutionOccurrenceErrorV1::NotCompileInvocation
        }
        ProducerIdentityFromRustcErrorV1::Producer(error) => {
            ProtectedCompilerExecutionOccurrenceErrorV1::Producer(error)
        }
    }
}

fn reconstruct_locked_occurrence(
    publication: &CompilerModuleHandoffCurrentnessLeaseV3,
    observation: &ValidatedRemoteRustcProcessObservationV1,
    token: &CompilerModuleHandoffConsumptionTokenV3,
) -> Result<
    (InertCompilerExecutionSubjectV1, [u8; SHA256_BYTES]),
    ProtectedCompilerExecutionOccurrenceErrorV1,
> {
    observation
        .revalidate()
        .map_err(ProtectedCompilerExecutionOccurrenceErrorV1::RevalidateObservation)?;
    let expected = ExpectedCompilerExecutionPublicationV1::derive(observation.descriptor())?;
    let subject =
        InertCompilerExecutionSubjectV1::from_publication(publication.receipt(), token.handoff())
            .map_err(ProtectedCompilerExecutionOccurrenceErrorV1::Subject)?;
    validate_subject(
        &subject,
        observation.descriptor(),
        token.handoff().capsule().invocation(),
        &expected,
    )?;
    observation
        .revalidate()
        .map_err(ProtectedCompilerExecutionOccurrenceErrorV1::RevalidateObservation)?;
    token
        .revalidate_locked_currentness()
        .map_err(ProtectedCompilerExecutionOccurrenceErrorV1::RevalidateCurrentPublication)?;
    let identity = derive_occurrence_identity(observation.identity(), &subject)?;
    Ok((subject, identity))
}

fn validate_locked_occurrence(
    occurrence: &ProtectedCompilerExecutionOccurrenceV1,
    publication: &CompilerModuleHandoffCurrentnessLeaseV3,
    observation: &ValidatedRemoteRustcProcessObservationV1,
    token: &CompilerModuleHandoffConsumptionTokenV3,
) -> Result<(), ProtectedCompilerExecutionOccurrenceErrorV1> {
    let (subject, identity) = reconstruct_locked_occurrence(publication, observation, token)?;
    if subject != occurrence.subject {
        return Err(ProtectedCompilerExecutionOccurrenceErrorV1::PublicationSubjectChanged);
    }
    if identity != occurrence.identity {
        return Err(ProtectedCompilerExecutionOccurrenceErrorV1::IdentityChanged);
    }
    Ok(())
}

fn validate_subject(
    subject: &InertCompilerExecutionSubjectV1,
    observed_descriptor: &RustcInvocationDescriptorV3,
    published_descriptor: &RustcInvocationDescriptorV3,
    expected: &ExpectedCompilerExecutionPublicationV1,
) -> Result<(), ProtectedCompilerExecutionOccurrenceErrorV1> {
    if !subject
        .identity()
        .matches_canonical_bytes(subject.canonical_bytes())
    {
        return Err(ProtectedCompilerExecutionOccurrenceErrorV1::NonCanonicalSubject);
    }
    if subject.attempt() != expected.attempt {
        return Err(ProtectedCompilerExecutionOccurrenceErrorV1::AttemptMismatch);
    }
    if subject.slot() != CompilerModuleHandoffSlotV3::Production {
        return Err(ProtectedCompilerExecutionOccurrenceErrorV1::SlotMismatch);
    }
    if published_descriptor.compiler_closure() != observed_descriptor.compiler_closure() {
        return Err(ProtectedCompilerExecutionOccurrenceErrorV1::CompilerClosureMismatch);
    }
    if published_descriptor != observed_descriptor {
        return Err(ProtectedCompilerExecutionOccurrenceErrorV1::ExactInvocationMismatch);
    }
    if subject.rustc_invocation_sha256() != &expected.invocation_digest {
        return Err(ProtectedCompilerExecutionOccurrenceErrorV1::InvocationDigestMismatch);
    }
    if subject.compiler_closure() != *observed_descriptor.compiler_closure() {
        return Err(ProtectedCompilerExecutionOccurrenceErrorV1::CompilerClosureMismatch);
    }
    Ok(())
}

fn derive_occurrence_identity(
    observation_identity: &[u8; SHA256_BYTES],
    subject: &InertCompilerExecutionSubjectV1,
) -> Result<[u8; SHA256_BYTES], ProtectedCompilerExecutionOccurrenceErrorV1> {
    let mut digest = Sha256::new();
    digest.update(OCCURRENCE_IDENTITY_DOMAIN_V1);
    digest.update((observation_identity.len() as u64).to_le_bytes());
    digest.update(observation_identity);
    digest.update((subject.canonical_bytes().len() as u64).to_le_bytes());
    digest.update(subject.canonical_bytes());
    let identity = digest.finalize().into();
    if identity == [0; SHA256_BYTES] {
        return Err(ProtectedCompilerExecutionOccurrenceErrorV1::IdentityChanged);
    }
    Ok(identity)
}

#[cfg(test)]
fn validate_synthetic(
    occurrence: &ProtectedCompilerExecutionOccurrenceV1,
) -> Result<(), ProtectedCompilerExecutionOccurrenceErrorV1> {
    if occurrence.identity == [0; SHA256_BYTES]
        || !occurrence
            .subject
            .identity()
            .matches_canonical_bytes(occurrence.subject.canonical_bytes())
    {
        return Err(ProtectedCompilerExecutionOccurrenceErrorV1::InvalidSyntheticOccurrence);
    }
    Ok(())
}

/// Failure to join one live protected rustc process to its exact current V3 publication.
#[derive(Debug)]
pub enum ProtectedCompilerExecutionOccurrenceErrorV1 {
    /// The admitted remote rustc process could not be observed.
    Observe(CompilerExecutionSupervisionErrorV1),
    /// The retained remote rustc process changed during revalidation.
    RevalidateObservation(CompilerExecutionSupervisionErrorV1),
    /// The exact rustc argument stream is not a valid bounded invocation.
    RustcArguments(RustcArgsErrorV2),
    /// The observed process is not a rustc compile invocation.
    NotCompileInvocation,
    /// The complete environment has no managed build attempt.
    MissingBuildAttempt,
    /// The managed build-attempt encoding is not canonical.
    BuildAttempt(AttemptCodecError),
    /// The crate/source producer identity is invalid.
    Producer(EmitError),
    /// The canonical invocation digest could not be derived.
    InvocationDigest(DigestError),
    /// The exact V3 publication could not be recovered.
    RecoverPublication(CompilerModuleHandoffErrorV3),
    /// Currentness custody could not be acquired for the recovered publication.
    AcquirePublicationLease(CompilerModuleHandoffErrorV3),
    /// The publication lock and currentness token could not be acquired.
    AcquireCurrentToken(CompilerModuleHandoffErrorV3),
    /// The locked publication changed or ceased to be current.
    RevalidateCurrentPublication(CompilerModuleHandoffErrorV3),
    /// The current publication does not form a canonical compiler-execution subject.
    Subject(CompilerExecutionSubjectErrorV1),
    /// The reconstructed subject is not canonically self-identifying.
    NonCanonicalSubject,
    /// The subject names a different build attempt.
    AttemptMismatch,
    /// The subject does not name the sole production slot.
    SlotMismatch,
    /// The handoff carries a different exact protected rustc descriptor.
    ExactInvocationMismatch,
    /// The subject names a different protected rustc invocation digest.
    InvocationDigestMismatch,
    /// The subject names a different complete compiler closure.
    CompilerClosureMismatch,
    /// Revalidation reconstructed a different publication subject.
    PublicationSubjectChanged,
    /// Revalidation derived a different occurrence identity.
    IdentityChanged,
    #[cfg(test)]
    InvalidSyntheticOccurrence,
}

impl fmt::Display for ProtectedCompilerExecutionOccurrenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observe(error) => write!(formatter, "compiler observation failed: {error}"),
            Self::RevalidateObservation(error) => {
                write!(
                    formatter,
                    "compiler observation revalidation failed: {error}"
                )
            }
            Self::RustcArguments(error) => write!(formatter, "invalid rustc arguments: {error}"),
            Self::NotCompileInvocation => {
                formatter.write_str("protected rustc occurrence is not a compile invocation")
            }
            Self::MissingBuildAttempt => {
                formatter.write_str("protected rustc environment has no managed build attempt")
            }
            Self::BuildAttempt(error) => {
                write!(formatter, "invalid managed build attempt: {error}")
            }
            Self::Producer(error) => write!(formatter, "invalid compiler producer: {error}"),
            Self::InvocationDigest(error) => {
                write!(
                    formatter,
                    "cannot derive protected invocation digest: {error}"
                )
            }
            Self::RecoverPublication(error) => {
                write!(formatter, "compiler publication recovery failed: {error}")
            }
            Self::AcquirePublicationLease(error) => {
                write!(
                    formatter,
                    "compiler publication lease acquisition failed: {error}"
                )
            }
            Self::AcquireCurrentToken(error) => {
                write!(
                    formatter,
                    "compiler publication lock acquisition failed: {error}"
                )
            }
            Self::RevalidateCurrentPublication(error) => {
                write!(
                    formatter,
                    "locked compiler publication revalidation failed: {error}"
                )
            }
            Self::Subject(error) => {
                write!(formatter, "compiler-execution subject rejected: {error}")
            }
            Self::NonCanonicalSubject => {
                formatter.write_str("compiler-execution subject is not canonical")
            }
            Self::AttemptMismatch => {
                formatter.write_str("compiler-execution subject build attempt mismatch")
            }
            Self::SlotMismatch => {
                formatter.write_str("compiler-execution subject publication-slot mismatch")
            }
            Self::ExactInvocationMismatch => {
                formatter.write_str("published and observed rustc descriptors differ")
            }
            Self::InvocationDigestMismatch => {
                formatter.write_str("compiler-execution subject invocation-digest mismatch")
            }
            Self::CompilerClosureMismatch => {
                formatter.write_str("compiler-execution subject compiler-closure mismatch")
            }
            Self::PublicationSubjectChanged => {
                formatter.write_str("current compiler publication subject changed")
            }
            Self::IdentityChanged => {
                formatter.write_str("protected compiler occurrence identity changed")
            }
            #[cfg(test)]
            Self::InvalidSyntheticOccurrence => {
                formatter.write_str("invalid synthetic compiler occurrence")
            }
        }
    }
}

impl Error for ProtectedCompilerExecutionOccurrenceErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Observe(error) | Self::RevalidateObservation(error) => Some(error),
            Self::RustcArguments(error) => Some(error),
            Self::BuildAttempt(error) => Some(error),
            Self::Producer(error) => Some(error),
            Self::InvocationDigest(error) => Some(error),
            Self::RecoverPublication(error)
            | Self::AcquirePublicationLease(error)
            | Self::AcquireCurrentToken(error)
            | Self::RevalidateCurrentPublication(error) => Some(error),
            Self::Subject(error) => Some(error),
            _ => None,
        }
    }
}

impl From<RustcArgsErrorV2> for ProtectedCompilerExecutionOccurrenceErrorV1 {
    fn from(error: RustcArgsErrorV2) -> Self {
        Self::RustcArguments(error)
    }
}

impl From<AttemptCodecError> for ProtectedCompilerExecutionOccurrenceErrorV1 {
    fn from(error: AttemptCodecError) -> Self {
        Self::BuildAttempt(error)
    }
}

impl From<EmitError> for ProtectedCompilerExecutionOccurrenceErrorV1 {
    fn from(error: EmitError) -> Self {
        Self::Producer(error)
    }
}

impl From<DigestError> for ProtectedCompilerExecutionOccurrenceErrorV1 {
    fn from(error: DigestError) -> Self {
        Self::InvocationDigest(error)
    }
}
