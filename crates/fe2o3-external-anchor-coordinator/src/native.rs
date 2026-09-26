//! Bounded preparation mechanics. No process creation or readiness admission.
use crate::{STATE_ROOT_MODE_V1, StateRootSnapshotV1};
use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2 as CapabilityError;
use fe2o3_compiler_execution_lifecycle::LifecycleLeaseErrorV2 as LifecycleError;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableErrorV2 as ImageError,
    ProtectedStaticExecutableMeasurementV1 as ImageMeasurement,
};
use rustix::{
    fs::{FileType, OFlags},
    io::Errno,
};
use std::{error::Error, fmt, fs::File};

pub(crate) const ENTRY_WORK: usize = 8;
// At most 64 fixed credential, root-descriptor, PID and cleanup calls, plus controls.
pub(crate) const LOCAL_WORK: usize = ENTRY_WORK + 64 * 1024;
pub(crate) type Result<T> = std::result::Result<T, ExternalAnchorPreparationErrorV2>;

/// Additional unreserved growth above ALL consumed inputs, never borrowed context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalAnchorPreparationStorageV2(pub(crate) usize);
impl ExternalAnchorPreparationStorageV2 {
    /// Reserve before retaining the result; consumed input reservations remain live.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Complete logical work and additional peak above the full input floor.
/// Not an instruction, elapsed-time, generated-stack, allocator or RSS bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalAnchorPreparationQuotaV2 {
    pub(crate) work: usize,
    pub(crate) scratch: usize,
}
impl ExternalAnchorPreparationQuotaV2 {
    /// Includes nested native operations on the original ledger.
    pub const fn work(self) -> usize {
        self.work
    }
    /// Includes simultaneously live image growth and nested scratch.
    pub const fn scratch(self) -> usize {
        self.scratch
    }
}

/// Fixed preparation refusal, not a claim about provisioning provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalAnchorPreparationFailureV2 {
    /// All real/effective/saved/filesystem UID/GID values must be root.
    RootRequired,
    /// The preparing process changed.
    CoordinatorChanged,
    /// Actual supervisor and policy do not bind the anchor deployment.
    ContextMismatch,
    /// Provisioning names another actual deployment.
    ProvisioningMismatch,
    /// Root descriptor flags, type, credentials, permissions or link count failed.
    InvalidStateRoot,
    /// The retained root's pinned identity or metadata changed.
    StateRootChanged,
}

/// Bounded native and fixed mechanical failures; never owned diagnostic strings.
#[derive(Debug)]
pub enum ExternalAnchorPreparationErrorV2 {
    /// Original ledger or checked arithmetic refusal.
    Resource(Resource),
    /// Actual native capability or contextual record refused.
    Capability(CapabilityError),
    /// Native measured executable admission or revalidation refused.
    Executable(ImageError),
    /// Native lifecycle custody refused.
    Lifecycle(LifecycleError),
    /// Fixed coordinator contract failure.
    Invalid(ExternalAnchorPreparationFailureV2),
    /// One non-retrying descriptor operation failed.
    Io {
        /// Fixed operation label.
        operation: &'static str,
        /// Kernel errno from the single attempt.
        source: Errno,
    },
}
impl From<Resource> for ExternalAnchorPreparationErrorV2 {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<CapabilityError> for ExternalAnchorPreparationErrorV2 {
    fn from(e: CapabilityError) -> Self {
        Self::Capability(e)
    }
}
impl From<ImageError> for ExternalAnchorPreparationErrorV2 {
    fn from(e: ImageError) -> Self {
        Self::Executable(e)
    }
}
impl From<LifecycleError> for ExternalAnchorPreparationErrorV2 {
    fn from(e: LifecycleError) -> Self {
        Self::Lifecycle(e)
    }
}
impl fmt::Display for ExternalAnchorPreparationErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Capability(e) => e.fmt(f),
            Self::Executable(e) => e.fmt(f),
            Self::Lifecycle(e) => e.fmt(f),
            Self::Invalid(e) => write!(f, "native anchor preparation refused: {e:?}"),
            Self::Io { operation, source } => write!(f, "{operation}: {source}"),
        }
    }
}
impl Error for ExternalAnchorPreparationErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Capability(e) => Some(e),
            Self::Executable(e) => Some(e),
            Self::Lifecycle(e) => Some(e),
            Self::Io { source, .. } => Some(source),
            Self::Invalid(_) => None,
        }
    }
}

pub(crate) fn sum(values: &[usize]) -> Result<usize> {
    values.iter().try_fold(0usize, |sum, value| {
        sum.checked_add(*value).ok_or(Resource::Arithmetic.into())
    })
}
pub(crate) const fn maximum(values: &[usize]) -> usize {
    let mut result = 0;
    let mut i = 0;
    while i < values.len() {
        if values[i] > result {
            result = values[i];
        }
        i += 1;
    }
    result
}
pub(crate) fn measurement(m: Measurement) -> Result<ImageMeasurement> {
    ImageMeasurement::new(m.sha256(), m.byte_len(), 128 * 1024 * 1024)
        .map_err(|e| ImageError::from(e).into())
}
pub(crate) fn require_root<const ROOT: bool>() -> Result<()> {
    #[cfg(not(test))]
    const {
        assert!(ROOT);
    }
    if ROOT {
        fe2o3_protected_service_spawn::require_exact_root_identity_v1().map_err(|_| {
            ExternalAnchorPreparationErrorV2::Invalid(
                ExternalAnchorPreparationFailureV2::RootRequired,
            )
        })?;
    }
    Ok(())
}

pub(crate) enum RootError {
    Invalid,
    Io(&'static str, Errno),
}
impl From<RootError> for ExternalAnchorPreparationErrorV2 {
    fn from(e: RootError) -> Self {
        match e {
            RootError::Invalid => {
                Self::Invalid(ExternalAnchorPreparationFailureV2::InvalidStateRoot)
            }
            RootError::Io(operation, source) => Self::Io { operation, source },
        }
    }
}
// Shared with V1: same operations, ordering, flags, comparisons and error strings.
pub(crate) fn state_root(
    root: &File,
    service: Service,
) -> std::result::Result<StateRootSnapshotV1, RootError> {
    let descriptor_flags = rustix::io::fcntl_getfd(root)
        .map_err(|e| RootError::Io("inspect anchor state-root descriptor", e))?;
    let status = rustix::fs::fcntl_getfl(root)
        .map_err(|e| RootError::Io("inspect anchor state-root status", e))?;
    let stat =
        rustix::fs::fstat(root).map_err(|e| RootError::Io("inspect anchor state root", e))?;
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC)
        || status & OFlags::ACCMODE != OFlags::RDONLY
        || status.intersects(forbidden)
        || FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_mode & 0o7777 != STATE_ROOT_MODE_V1
        || stat.st_uid != service.uid()
        || stat.st_gid != service.gid()
        || stat.st_nlink == 0
    {
        return Err(RootError::Invalid);
    }
    Ok(StateRootSnapshotV1 {
        device: stat.st_dev,
        inode: stat.st_ino,
        mode: stat.st_mode,
        uid: stat.st_uid,
        gid: stat.st_gid,
        links: stat.st_nlink,
    })
}
