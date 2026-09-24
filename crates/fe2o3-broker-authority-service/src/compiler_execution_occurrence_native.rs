//! A live native observation joined to the exact, still-locked V4 publication.
use super::ExpectedCompilerExecutionPublicationV1 as Expected;
use crate::{
    ProtectedServiceAdmissionV2 as Service,
    compiler_execution_supervision::{NativeObservation, NativeObservationError},
};
use fe2o3_artifact_transaction::{
    CompilerExecutionSubjectErrorV2 as SubjectError,
    CompilerModuleHandoffConsumptionTokenV4 as Token,
    CompilerModuleHandoffCurrentnessLeaseV4 as Lease, CompilerModuleHandoffErrorV4 as HandoffError,
    InertCompilerExecutionSubjectStorageV2 as SubjectStorage,
    InertCompilerExecutionSubjectV2 as Subject,
    acquire_compiler_module_handoff_currentness_lease_v4 as acquire,
    recover_compiler_module_handoff_receipt_v4 as recover,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use sha2::{Digest, Sha256};
use std::mem::size_of;

#[derive(Debug)]
pub(crate) enum NativeOccurrenceError {
    Resource(Resource),
    Observation(NativeObservationError),
    Handoff(HandoffError),
    Subject(SubjectError),
    Expected(super::ProtectedCompilerExecutionOccurrenceErrorV1),
    Mismatch,
}
impl std::fmt::Display for NativeOccurrenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => write!(f, "{e}"),
            Self::Observation(e) => write!(f, "{e}"),
            Self::Handoff(e) => write!(f, "{e}"),
            Self::Subject(e) => write!(f, "{e}"),
            Self::Expected(e) => write!(f, "{e}"),
            Self::Mismatch => f.write_str("native compiler occurrence mismatch"),
        }
    }
}
type Result<T> = std::result::Result<T, NativeOccurrenceError>;
macro_rules! from_error {
    ($t:ty, $v:ident) => {
        impl From<$t> for NativeOccurrenceError {
            fn from(e: $t) -> Self {
                Self::$v(e)
            }
        }
    };
}
from_error!(Resource, Resource);
from_error!(NativeObservationError, Observation);
from_error!(HandoffError, Handoff);
from_error!(SubjectError, Subject);
from_error!(super::ProtectedCompilerExecutionOccurrenceErrorV1, Expected);

pub(crate) struct NativeOccurrence {
    observation: NativeObservation,
    publication: Lease,
    token: Token,
    subject: Subject,
    identity: [u8; 32],
    retained: usize,
}
impl NativeOccurrence {
    const FRAME: usize = 16 * fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3 + 8192;
    const WORK: usize = 4096 * fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3;

    pub(crate) fn observe(service: &Service, b: &mut Budget<'_>) -> Result<(Self, usize)> {
        b.with_prepaid_scope(
            service.retained_storage(),
            8,
            Self::WORK,
            Self::FRAME,
            |b| {
                let (observation, observation_storage) = NativeObservation::observe(service, b)?;
                b.reserve_storage(observation_storage)?;
                let expected = Expected::derive(observation.descriptor())?;
                let output = observation.output_dir();
                let receipt = recover(&output, &expected.producer, expected.attempt, b)?;
                let (publication, storage) = acquire(&output, &expected.producer, receipt, b)?;
                b.reserve_storage(storage.retained_storage())?;
                let (token, storage) = publication.acquire_current_token(b)?;
                b.reserve_storage(storage.retained_storage())?;
                let published = token.handoff().capsule().base().invocation();
                if published != observation.descriptor() {
                    return Err(NativeOccurrenceError::Mismatch);
                }
                let (subject, storage) = Subject::from_publication(receipt, token.handoff(), b)?;
                b.reserve_storage(storage.retained_storage())?;
                if subject.attempt() != expected.attempt
                    || subject.rustc_invocation_sha256() != &expected.invocation_digest
                    || subject.compiler_closure() != *observation.descriptor().compiler_closure()
                {
                    return Err(NativeOccurrenceError::Mismatch);
                }
                let mut digest = Sha256::new();
                digest.update(b"FE2O3/PROTECTED-COMPILER-EXECUTION-OCCURRENCE/V2\0");
                digest.update(observation.identity());
                digest.update(subject.canonical_bytes());
                let identity = digest.finalize().into();
                let retained = observation_storage
                    .checked_add(publication.storage().retained_storage())
                    .and_then(|n| n.checked_add(token.storage().retained_storage()))
                    .and_then(|n| n.checked_add(size_of::<(Subject, SubjectStorage)>()))
                    .and_then(|n| n.checked_add(size_of::<Self>()))
                    .ok_or(Resource::Arithmetic)?;
                let occurrence = Self {
                    observation,
                    publication,
                    token,
                    subject,
                    identity,
                    retained,
                };
                // All components are covered by the staged frame and their charges.
                occurrence.revalidate(service, b)?;
                Ok((occurrence, retained))
            },
        )
    }

    pub(crate) fn revalidate(&self, service: &Service, b: &mut Budget<'_>) -> Result<()> {
        let floor = self
            .retained_storage()
            .checked_add(service.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, 16, 128, |b| {
            // validate_current_token is a fixed Arc-identity comparison, not I/O.
            self.publication.validate_current_token(&self.token)?;
            self.observation.revalidate(service, b)?;
            self.token.revalidate_locked_currentness(b)?;
            Ok(())
        })
    }
    pub(crate) fn subject(&self) -> &Subject {
        &self.subject
    }
    pub(crate) fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }
}
