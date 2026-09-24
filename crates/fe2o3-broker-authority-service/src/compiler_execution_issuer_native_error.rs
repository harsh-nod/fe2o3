use super::*;

/// Failure of a consuming native issuer session. No successful response is
/// released after a custody, resource, currentness or durable-storage refusal.
#[derive(Debug)]
pub struct NativeIssuerServiceError(Failure);
#[derive(Debug)]
enum Failure {
    Resource(Resource),
    Admission(super::super::Error),
    Key(KeyError),
    Attestation(fe2o3_compiler_execution_protocol::CompilerExecutionAttestationErrorV2),
    Publication(fe2o3_compiler_execution_protocol::CompilerExecutionReceiptPublicationErrorV2),
    Protocol(fe2o3_compiler_execution_protocol::CompilerExecutionServiceProtocolErrorV2),
    Subject(fe2o3_artifact_transaction::CompilerExecutionSubjectErrorV2),
    Journal(fe2o3_compiler_execution_protocol::CompilerExecutionNativeJournalErrorV2),
    Worker(fe2o3_compiler_execution_protocol::CompilerExecutionWorkerAnchorJournalErrorV2),
    Directory(fe2o3_artifact_transaction::RetainedDurableDirectoryErrorV2),
    DirectoryAdmission(fe2o3_artifact_transaction::RetainedDurableDirectoryErrorV1),
    Occurrence(crate::compiler_execution_occurrence::NativeOccurrenceError),
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
from_error!(super::super::Error, Admission);
from_error!(KeyError, Key);
from_error!(
    fe2o3_compiler_execution_protocol::CompilerExecutionAttestationErrorV2,
    Attestation
);
from_error!(
    fe2o3_compiler_execution_protocol::CompilerExecutionReceiptPublicationErrorV2,
    Publication
);
from_error!(
    fe2o3_compiler_execution_protocol::CompilerExecutionServiceProtocolErrorV2,
    Protocol
);
from_error!(
    fe2o3_artifact_transaction::CompilerExecutionSubjectErrorV2,
    Subject
);
from_error!(
    fe2o3_compiler_execution_protocol::CompilerExecutionNativeJournalErrorV2,
    Journal
);
from_error!(
    fe2o3_artifact_transaction::RetainedDurableDirectoryErrorV2,
    Directory
);
from_error!(
    fe2o3_artifact_transaction::RetainedDurableDirectoryErrorV1,
    DirectoryAdmission
);
from_error!(
    crate::compiler_execution_occurrence::NativeOccurrenceError,
    Occurrence
);
from_error!(crate::CompilerExecutionServiceErrorV1, Transport);
from_error!(
    fe2o3_external_anchor_protocol::AnchorProtocolErrorV1,
    Anchor
);
from_error!(rustix::io::Errno, Io);
from_error!(std::io::Error, Lock);
from_error!(
    fe2o3_compiler_execution_protocol::CompilerExecutionWorkerAnchorJournalErrorV2,
    Worker
);
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
            Failure::Admission(e) => e.resource(),
            Failure::AnchorAdmission(e) => e.resource(),
            Failure::AnchorTransport(crate::ProtectedCompilerExecutionExternalAnchorErrorV1::Resource(e)) => Some(*e),
            Failure::Worker(fe2o3_compiler_execution_protocol::CompilerExecutionWorkerAnchorJournalErrorV2::Resource(e)) => Some(*e),
            Failure::Journal(fe2o3_compiler_execution_protocol::CompilerExecutionNativeJournalErrorV2::Resource(e)) => Some(*e),
            Failure::Key(KeyError::Resource(e)) => Some(*e),
            Failure::Directory(
                fe2o3_artifact_transaction::RetainedDurableDirectoryErrorV2::Resource(e),
            ) => Some(*e),
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
