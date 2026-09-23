use super::*;
use crate::{
    ProtectedIssuerHandoffErrorV2 as HandoffError,
    ProtectedIssuerSupervisorErrorV2 as SupervisorError,
};
use fe2o3_broker_authority_service::LiveClientPidfdErrorV2 as ParentError;
use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2 as CapabilityError;
use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestErrorV2 as ManifestError;
use fe2o3_static_preexec_manifest::StaticPreexecManifestErrorV1 as StaticError;
use std::error::Error as StdError;

/// Fixed native preparation refusal; no failure retries through legacy authority.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedIssuerLaunchPreparationErrorV2 {
    /// The caller's work or storage ledger refused the operation.
    Resource(Resource),
    /// Native supervisor custody no longer agrees with the trusted inputs.
    Supervisor(SupervisorError),
    /// The admitted client handoff or its transferred descriptors changed.
    Handoff(HandoffError),
    /// A native sealed capability failed creation, identity or revalidation.
    Capability(CapabilityError),
    /// Native canonical launch framing failed.
    Manifest(ManifestError),
    /// The inert static-launch record failed canonical validation.
    StaticManifest(StaticError),
    /// Bounded current-process identity observation failed.
    ParentIdentity(ParentError),
    /// Current PID is outside the static launch ABI.
    InvalidParentIdentity,
    /// Current PID or start time differs from the prepared parent identity.
    ParentChanged,
    /// The sealed launch record disagrees with the authenticated handoff.
    LaunchManifestMismatch,
    /// A fixed descriptor invariant was not satisfied.
    InvalidDescriptor {
        /// Descriptor role.
        role: &'static str,
        /// Failed invariant.
        reason: &'static str,
    },
    /// A retained object or canonical byte record changed.
    DescriptorChanged(&'static str),
    /// Roles requiring distinct objects alias each other.
    DescriptorAlias(&'static str),
    /// A single-attempt operating-system call failed.
    Io {
        /// Fixed operation label.
        operation: &'static str,
        /// Kernel errno; no allocated diagnostic is retained.
        errno: rustix::io::Errno,
    },
}
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<SupervisorError> for Error {
    fn from(e: SupervisorError) -> Self {
        Self::Supervisor(e)
    }
}
impl From<HandoffError> for Error {
    fn from(e: HandoffError) -> Self {
        Self::Handoff(e)
    }
}
impl From<CapabilityError> for Error {
    fn from(e: CapabilityError) -> Self {
        Self::Capability(e)
    }
}
impl From<ManifestError> for Error {
    fn from(e: ManifestError) -> Self {
        Self::Manifest(e)
    }
}
impl From<StaticError> for Error {
    fn from(e: StaticError) -> Self {
        Self::StaticManifest(e)
    }
}
impl From<ParentError> for Error {
    fn from(e: ParentError) -> Self {
        Self::ParentIdentity(e)
    }
}
impl From<checks::Failure> for Error {
    fn from(e: checks::Failure) -> Self {
        match e {
            checks::Failure::InvalidDescriptor { role, reason } => {
                Self::InvalidDescriptor { role, reason }
            }
            checks::Failure::DescriptorChanged(role) => Self::DescriptorChanged(role),
            checks::Failure::DescriptorAlias(reason) => Self::DescriptorAlias(reason),
            checks::Failure::Io { operation, source } => Self::Io {
                operation,
                errno: source,
            },
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Supervisor(e) => write!(f, "protected supervisor changed: {e}"),
            Self::Handoff(e) => write!(f, "authenticated handoff changed: {e}"),
            Self::Capability(e) => write!(f, "native launch capability failed: {e}"),
            Self::Manifest(e) => e.fmt(f),
            Self::StaticManifest(e) => e.fmt(f),
            Self::ParentIdentity(e) => write!(f, "cannot observe supervisor identity: {e}"),
            Self::InvalidParentIdentity => f.write_str("supervisor PID is outside the launch ABI"),
            Self::ParentChanged => f.write_str("supervisor PID or start time changed"),
            Self::LaunchManifestMismatch => {
                f.write_str("service launch manifest disagrees with authenticated handoff")
            }
            Self::InvalidDescriptor { role, reason } => {
                write!(f, "invalid {role} descriptor: {reason}")
            }
            Self::DescriptorChanged(role) => write!(f, "retained {role} changed"),
            Self::DescriptorAlias(reason) => f.write_str(reason),
            Self::Io { operation, errno } => write!(f, "{operation}: {errno}"),
        }
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Supervisor(e) => Some(e),
            Self::Handoff(e) => Some(e),
            Self::Capability(e) => Some(e),
            Self::Manifest(e) => Some(e),
            Self::StaticManifest(e) => Some(e),
            Self::ParentIdentity(e) => Some(e),
            Self::Io { errno, .. } => Some(errno),
            _ => None,
        }
    }
}
