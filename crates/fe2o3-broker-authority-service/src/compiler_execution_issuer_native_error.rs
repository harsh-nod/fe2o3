use super::*;
use crate::compiler_execution_supervision::NativeObservationError;
use fe2o3_protected_service_spawn::{
    ProtectedServiceCleanupErrorV2 as CleanupError,
    native_spawn::ProtectedServiceSpawnErrorV2 as SpawnError,
};

/// Failure of a consuming native issuer session. No successful response is
/// released after a custody, resource, currentness or durable-storage refusal.
#[derive(Debug)]
pub struct NativeIssuerServiceError(Failure);
#[derive(Debug)]
enum Failure {
    Resource(Resource),
    Admission(super::super::Error),
    Key(KeyError),
    Attestation(AttestationError),
    Publication(PublicationError),
    Protocol(ProtocolError),
    Manifest(ManifestError),
    Readiness(ReadyError),
    Subject(SubjectError),
    Journal(JournalError),
    Worker(WorkerJournalError),
    Directory(fe2o3_artifact_transaction::RetainedDurableDirectoryErrorV2),
    DirectoryAdmission(fe2o3_artifact_transaction::RetainedDurableDirectoryErrorV1),
    Occurrence(OccurrenceError),
    Transport(crate::CompilerExecutionServiceErrorV1),
    Anchor(fe2o3_external_anchor_protocol::AnchorProtocolErrorV1),
    AnchorAdmission(crate::ProtectedExternalAnchorServiceErrorV2),
    AnchorTransport(crate::ProtectedCompilerExecutionExternalAnchorErrorV1),
    Io(rustix::io::Errno),
    Lock(std::io::Error),
    Rejected(&'static str),
}
macro_rules! from_error {
    ($ty:ty, $variant:ident) => {
        impl From<$ty> for NativeIssuerServiceError {
            fn from(e: $ty) -> Self {
                Self(Failure::$variant(e))
            }
        }
    };
}
from_error!(Resource, Resource);
from_error!(ManifestError, Manifest);
from_error!(ReadyError, Readiness);
from_error!(super::super::Error, Admission);
from_error!(KeyError, Key);
from_error!(AttestationError, Attestation);
from_error!(PublicationError, Publication);
from_error!(ProtocolError, Protocol);
from_error!(SubjectError, Subject);
from_error!(JournalError, Journal);
from_error!(
    fe2o3_artifact_transaction::RetainedDurableDirectoryErrorV2,
    Directory
);
from_error!(
    fe2o3_artifact_transaction::RetainedDurableDirectoryErrorV1,
    DirectoryAdmission
);
from_error!(OccurrenceError, Occurrence);
from_error!(crate::CompilerExecutionServiceErrorV1, Transport);
from_error!(
    fe2o3_external_anchor_protocol::AnchorProtocolErrorV1,
    Anchor
);
from_error!(rustix::io::Errno, Io);
from_error!(std::io::Error, Lock);
from_error!(WorkerJournalError, Worker);
from_error!(
    crate::ProtectedExternalAnchorServiceErrorV2,
    AnchorAdmission
);
from_error!(
    crate::ProtectedCompilerExecutionExternalAnchorErrorV1,
    AnchorTransport
);
impl NativeIssuerServiceError {
    pub(super) fn rejected(reason: &'static str) -> Self {
        Self(Failure::Rejected(reason))
    }
    pub fn resource(&self) -> Option<Resource> {
        match &self.0 {
            Failure::Resource(e) => Some(*e),
            Failure::Manifest(ManifestError::Resource(e)) => Some(*e),
            Failure::Readiness(ReadyError::Resource(e)) => Some(*e),
            Failure::Admission(e) => e.resource(),
            Failure::AnchorAdmission(e) => e.resource(),
            Failure::AnchorTransport(
                crate::ProtectedCompilerExecutionExternalAnchorErrorV1::Resource(e),
            ) => Some(*e),
            Failure::Worker(WorkerJournalError::Resource(e)) => Some(*e),
            Failure::Journal(JournalError::Resource(e)) => Some(*e),
            Failure::Key(KeyError::Resource(e)) => Some(*e),
            Failure::Directory(
                fe2o3_artifact_transaction::RetainedDurableDirectoryErrorV2::Resource(e),
            ) => Some(*e),
            Failure::Occurrence(e) => match e {
                OccurrenceError::Resource(e)
                | OccurrenceError::Subject(SubjectError::Resource(e)) => Some(*e),
                OccurrenceError::Observation(e) => match e {
                    NativeObservationError::Resource(e)
                    | NativeObservationError::Capability(KeyError::Resource(e)) => Some(*e),
                    NativeObservationError::Service(e) => e.resource(),
                    NativeObservationError::Spawn(e) => match e {
                        SpawnError::Resource(e)
                        | SpawnError::Cleanup(CleanupError::Resource(e)) => Some(*e),
                        SpawnError::Profile(_)
                        | SpawnError::Cleanup(_)
                        | SpawnError::Retained(_)
                        | SpawnError::SpawnLease(_)
                        | SpawnError::State(_)
                        | SpawnError::Io { .. } => None,
                    },
                    _ => None,
                },
                // Both nominal handoff errors expose their typed Resource as the
                // immediate source. Do not reinterpret I/O or framing diagnostics.
                OccurrenceError::Handoff(e) => std::error::Error::source(e)
                    .and_then(|source| source.downcast_ref::<Resource>())
                    .copied(),
                _ => None,
            },
            _ => None,
        }
    }
}
impl std::fmt::Display for NativeIssuerServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("native issuer service refused: ")?;
        match &self.0 {
            Failure::Resource(e) => write!(f, "{e}"),
            Failure::Admission(e) => write!(f, "{e}"),
            Failure::Key(e) => write!(f, "{e}"),
            Failure::Attestation(e) => write!(f, "{e}"),
            Failure::Publication(e) => write!(f, "{e}"),
            Failure::Protocol(e) => write!(f, "{e}"),
            Failure::Manifest(e) => write!(f, "{e}"),
            Failure::Readiness(e) => write!(f, "{e}"),
            Failure::Subject(e) => write!(f, "{e}"),
            Failure::Journal(e) => write!(f, "{e}"),
            Failure::Worker(e) => write!(f, "{e}"),
            Failure::Directory(e) => write!(f, "{e}"),
            Failure::DirectoryAdmission(e) => write!(f, "{e}"),
            Failure::Occurrence(e) => write!(f, "{e}"),
            Failure::Transport(e) => write!(f, "{e}"),
            Failure::Anchor(e) => write!(f, "{e}"),
            Failure::AnchorAdmission(e) => write!(f, "{e}"),
            Failure::AnchorTransport(e) => write!(f, "{e}"),
            Failure::Io(e) => write!(f, "{e}"),
            Failure::Lock(e) => write!(f, "{e}"),
            Failure::Rejected(reason) => f.write_str(reason),
        }
    }
}
impl std::error::Error for NativeIssuerServiceError {}

#[cfg(test)]
#[path = "compiler_execution_issuer_native_error_tests.rs"]
mod tests;
