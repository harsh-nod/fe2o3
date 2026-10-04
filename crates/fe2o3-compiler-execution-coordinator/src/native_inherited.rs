//! Fixed root descriptor intake and shared accounting; native families bind authority.
use crate::{
    native, native_root_source as source,
    runtime_listener::{RuntimeListener, RuntimeListenerError, RuntimeRoot},
};
use fe2o3_compiler_execution_protocol::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{
    error::Error,
    fmt,
    fs::File,
    mem::size_of,
    os::fd::{BorrowedFd, FromRawFd, OwnedFd},
    path::Path,
};

pub(crate) type Result<T> = std::result::Result<T, CompilerExecutionRootDeploymentErrorV2>;
pub(crate) const FILE_STORAGE: usize = size_of::<(File, usize)>();
pub(crate) const DESCRIPTORS: [i32; 14] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
pub(crate) const LOCAL_WORK: usize = 8 + 128 * 1024;
pub(crate) const TURN_WORK: usize = 8 + 4096;
pub(crate) const INTAKE_SCRATCH: usize = 4 * size_of::<[File; 14]>() + 8192;
pub(crate) const LISTENER_GROWTH: usize =
    size_of::<(RuntimeListener, usize)>() + 4 * 108 + FILE_STORAGE;
pub(crate) const LISTENER_SCRATCH: usize = 4 * LISTENER_GROWTH + 8192;

/// Native root admission/launch refusal with no owned diagnostic strings.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionRootDeploymentErrorV2 {
    /// Original accounting refused.
    Resource(Resource),
    /// Untrusted root source provenance or stable byte observation refused.
    Source(source::RootSourceErrorV2),
    /// Native sealed capability construction refused.
    Capability(fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2),
    /// Native V2 policy decoding refused.
    PolicyV2(CompilerExecutionAttestationErrorV2),
    /// Native V3 policy decoding refused.
    PolicyV3(CompilerExecutionAttestationErrorV3),
    /// Native V2 supervisor context refused.
    SupervisorV2(CompilerExecutionSupervisorDeploymentErrorV2),
    /// Native V3 supervisor context refused.
    SupervisorV3(CompilerExecutionSupervisorDeploymentErrorV3),
    /// Native V2 anchor context refused.
    AnchorV2(CompilerExecutionExternalAnchorDeploymentErrorV2),
    /// Native V3 anchor context refused.
    AnchorV3(CompilerExecutionExternalAnchorDeploymentErrorV3),
    /// Native V2 provisioning context refused.
    ProvisioningV2(CompilerExecutionExternalAnchorProvisioningErrorV2),
    /// Native V3 provisioning context refused.
    ProvisioningV3(CompilerExecutionExternalAnchorProvisioningErrorV3),
    /// Root-bound lifecycle validation refused.
    Lifecycle(fe2o3_compiler_execution_lifecycle::LifecycleLeaseErrorV2),
    /// Exact service identity refused.
    Credentials(fe2o3_compiler_execution_supervisor::IssuerServiceCredentialProfileErrorV1),
    /// Listener/root validation refused.
    Inputs(fe2o3_compiler_execution_supervisor::ProtectedIssuerServiceProvisioningErrorV2),
    /// Native trust binding refused.
    Trust(crate::CompilerExecutionSupervisorTrustErrorV2),
    /// Native compiler preparation refused.
    Preparation(crate::CompilerExecutionPreparationErrorV2),
    /// Native anchor preparation refused.
    AnchorPreparation(fe2o3_external_anchor_coordinator::ExternalAnchorPreparationErrorV2),
    /// Native anchor launch refused.
    AnchorLaunch(fe2o3_external_anchor_coordinator::ExternalAnchorLaunchErrorV2),
    /// Native compiler launch refused.
    CompilerLaunch(crate::CompilerExecutionLaunchErrorV2),
    /// Persistent cleanup funding or state refused.
    Cleanup(fe2o3_protected_service_spawn::ProtectedServiceCleanupErrorV2),
    /// A fixed role's input shape or process state is invalid.
    Invalid {
        /// Fixed descriptor or process role.
        role: &'static str,
        /// Fixed refusal reason, never input-derived text.
        reason: &'static str,
    },
    /// One bounded descriptor or filesystem operation failed.
    Io {
        /// Fixed operation name.
        operation: &'static str,
        /// Kernel error number.
        source: rustix::io::Errno,
    },
}
use CompilerExecutionRootDeploymentErrorV2 as Failure;
macro_rules! errors {
    ($($source:ty => $variant:ident),+ $(,)?) => {
        $(impl From<$source> for Failure {
            fn from(error: $source) -> Self { Self::$variant(error) }
        })+
        impl Error for Failure {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                match self {
                    $(Self::$variant(e) => Some(e),)+
                    Self::Io { source, .. } => Some(source),
                    Self::Invalid { .. } => None,
                }
            }
        }
        impl fmt::Display for Failure {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(Self::$variant(e) => e.fmt(f),)+
                    Self::Io { operation, source } => write!(f, "{operation}: {source}"),
                    Self::Invalid { role, reason } => write!(f, "native root {role}: {reason}"),
                }
            }
        }
    };
}
errors!(Resource => Resource, source::RootSourceErrorV2 => Source,
    fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2 => Capability,
    CompilerExecutionAttestationErrorV2 => PolicyV2, CompilerExecutionAttestationErrorV3 => PolicyV3,
    CompilerExecutionSupervisorDeploymentErrorV2 => SupervisorV2, CompilerExecutionSupervisorDeploymentErrorV3 => SupervisorV3,
    CompilerExecutionExternalAnchorDeploymentErrorV2 => AnchorV2, CompilerExecutionExternalAnchorDeploymentErrorV3 => AnchorV3,
    CompilerExecutionExternalAnchorProvisioningErrorV2 => ProvisioningV2, CompilerExecutionExternalAnchorProvisioningErrorV3 => ProvisioningV3,
    fe2o3_compiler_execution_lifecycle::LifecycleLeaseErrorV2 => Lifecycle,
    fe2o3_compiler_execution_supervisor::IssuerServiceCredentialProfileErrorV1 => Credentials,
    fe2o3_compiler_execution_supervisor::ProtectedIssuerServiceProvisioningErrorV2 => Inputs,
    crate::CompilerExecutionSupervisorTrustErrorV2 => Trust,
    crate::CompilerExecutionPreparationErrorV2 => Preparation,
    fe2o3_external_anchor_coordinator::ExternalAnchorPreparationErrorV2 => AnchorPreparation,
    fe2o3_external_anchor_coordinator::ExternalAnchorLaunchErrorV2 => AnchorLaunch,
    crate::CompilerExecutionLaunchErrorV2 => CompilerLaunch,
    fe2o3_protected_service_spawn::ProtectedServiceCleanupErrorV2 => Cleanup);
impl From<RuntimeListenerError> for Failure {
    fn from(error: RuntimeListenerError) -> Self {
        match error {
            RuntimeListenerError::Invalid { role, reason } => Self::Invalid { role, reason },
            RuntimeListenerError::Io { operation, source } => Self::Io { operation, source },
        }
    }
}

/// Unreserved full admission charge or consuming-launch growth, as documented by the operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionRootStorageV2(pub(crate) usize);
impl CompilerExecutionRootStorageV2 {
    /// Reserve before retaining the returned owner on the original account.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Conservative logical admission envelope, including all raw inputs, retained
/// native owners and nested scratch. Not a time, generated-stack or RSS bound.
/// Consuming launch and the independent cleanup account are separate operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionRootAdmissionQuotaV2 {
    pub(crate) work: usize,
    pub(crate) scratch: usize,
}
impl CompilerExecutionRootAdmissionQuotaV2 {
    /// Complete admission work on the original account, including entry work.
    pub const fn work(self) -> usize {
        self.work
    }
    /// Additional peak above entry storage; no inherited input prepayment is needed.
    pub const fn scratch(self) -> usize {
        self.scratch
    }
}

/// Inert funding plan for one activation/admission/launch and a finite service
/// lifetime. The request and cleanup limits belong to two independent original
/// accounts, never renewed per phase or tick. This grants no execution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionStartupQuotaV2 {
    pub(crate) request_work: usize,
    pub(crate) request_storage: usize,
    pub(crate) cleanup_work: usize,
    pub(crate) cleanup_storage: usize,
}
impl CompilerExecutionStartupQuotaV2 {
    /// Complete original-request work including finite monitoring and cleanup control.
    pub const fn request_work(self) -> usize {
        self.request_work
    }
    /// Peak logical request storage from an empty account, not process RSS.
    pub const fn request_storage(self) -> usize {
        self.request_storage
    }
    /// Original persistent-account work, including guard, payload, scans and shutdown.
    pub const fn cleanup_work(self) -> usize {
        self.cleanup_work
    }
    /// Full cleanup pool and retained preparation payload, independent of request storage.
    pub const fn cleanup_storage(self) -> usize {
        self.cleanup_storage
    }
}

pub(crate) fn repeated(count: usize, work: usize) -> Result<usize> {
    count.checked_mul(work).ok_or(Resource::Arithmetic.into())
}

// Sum scratch conservatively rather than relying on the order of nested scopes.
// Capability I/O scratch includes its complete retained owner; two copies cover
// the inner decode/seal peak and the retained result in the outer admission.
pub(crate) fn record_quota<const N: usize>(
    decode_work: usize,
    decode_scratch: usize,
    capability_work: usize,
    capability_scratch: usize,
) -> Result<CompilerExecutionRootAdmissionQuotaV2> {
    Ok(CompilerExecutionRootAdmissionQuotaV2 {
        work: sum(&[8, source::record_work::<N>()?, decode_work, capability_work])?,
        scratch: sum(&[
            N,
            N,
            4096,
            N,
            source::record_scratch::<N>()?,
            decode_scratch,
            capability_scratch,
            capability_scratch,
        ])?,
    })
}
pub(crate) fn sum(values: &[usize]) -> Result<usize> {
    values.iter().try_fold(0usize, |total, n| {
        total.checked_add(*n).ok_or(Resource::Arithmetic.into())
    })
}
pub(crate) fn length(n: u64) -> Result<usize> {
    usize::try_from(n).map_err(|_| Resource::Arithmetic.into())
}
pub(crate) fn invalid(role: &'static str, reason: &'static str) -> Failure {
    Failure::Invalid { role, reason }
}
fn io(operation: &'static str, source: rustix::io::Errno) -> Failure {
    Failure::Io { operation, source }
}

// The caller prepays ALL backing bytes before adoption, not merely descriptor numbers.
// SAFETY: only the root entrypoint may transfer these uniquely owned, live slots.
#[allow(unsafe_code)]
pub(crate) unsafe fn take(limits: &[usize; 11], b: &mut Budget<'_>) -> Result<[File; 14]> {
    b.with_prepaid_scope(14 * FILE_STORAGE, 8, LOCAL_WORK, INTAKE_SCRATCH, |_| {
        native::require_root()?;
        require_single_threaded()?;
        for (index, fd) in DESCRIPTORS.into_iter().enumerate() {
            // SAFETY: F_GETFD observes the scalar descriptor without taking ownership.
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
            if flags < 0 {
                return Err(io(
                    "inspect inherited input",
                    rustix::io::Errno::from_raw_os_error(
                        std::io::Error::last_os_error()
                            .raw_os_error()
                            .unwrap_or(libc::EIO),
                    ),
                ));
            }
            if flags != 0 {
                return Err(invalid("descriptor", "input is not exactly inheritable"));
            }
            // SAFETY: F_GETFD just established validity. The exclusive single-thread
            // transfer contract keeps the source live; this borrow takes no ownership.
            let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
            let stat =
                rustix::fs::fstat(borrowed).map_err(|e| io("inspect inherited source size", e))?;
            check_source_shape(index, limits, &stat)?;
        }
        // SAFETY: the caller transfers the entire table; preflight proved every
        // slot live and no thread can close/reuse one during this one-shot intake.
        let files = DESCRIPTORS.map(|fd| unsafe { File::from_raw_fd(fd) });
        for file in &files {
            rustix::io::fcntl_setfd(file, rustix::io::FdFlags::CLOEXEC)
                .map_err(|e| io("protect inherited input", e))?;
        }
        Ok(files)
    })
}

// The enclosing admission/installer prepays this fixed check before any I/O.
pub(crate) fn require_single_threaded() -> Result<()> {
    let tasks = rustix::fs::open(
        "/proc/self/task",
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map_err(|e| io("open process thread set", e))?;
    single_thread_entries(tasks, rustix::process::getpid().as_raw_pid() as u32)
}

// RawDir uses this fixed buffer, not libc's allocation-sized directory stream.
// At most '.', '..', the sole main PID, and EOF are allowed; never retry EINTR.
fn single_thread_entries(tasks: OwnedFd, expected_pid: u32) -> Result<()> {
    let mut buffer = [std::mem::MaybeUninit::uninit(); 1024];
    let mut entries = rustix::fs::RawDir::new(tasks, &mut buffer);
    let mut seen = false;
    for _ in 0..4 {
        let Some(entry) = entries.next() else {
            return if seen {
                Ok(())
            } else {
                Err(invalid("process", "missing main thread"))
            };
        };
        let entry = entry.map_err(|e| io("read process thread set", e))?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        if seen
            || name.is_empty()
            || name.len() > 10
            || !name.iter().all(u8::is_ascii_digit)
            || entry.file_type() != rustix::fs::FileType::Directory
            || std::str::from_utf8(name)
                .ok()
                .and_then(|s| s.parse::<u32>().ok())
                != Some(expected_pid)
        {
            return Err(invalid(
                "process",
                "descriptor intake requires only the main thread",
            ));
        }
        seen = true;
    }
    Err(invalid(
        "process",
        "thread enumeration exceeded its fixed bound",
    ))
}

fn check_source_shape(index: usize, limits: &[usize; 11], stat: &rustix::fs::Stat) -> Result<()> {
    use rustix::fs::FileType;
    let kind = FileType::from_raw_mode(stat.st_mode);
    let valid = if index < 3 {
        kind == FileType::Directory
    } else {
        let n = usize::try_from(stat.st_size)
            .map_err(|_| invalid("source", "negative or oversized length"))?;
        let limit = *limits
            .get(index - 3)
            .ok_or_else(|| invalid("source", "unknown descriptor role"))?;
        kind == FileType::RegularFile && n > 0 && n <= limit && (index < 8 || n == limit)
    };
    if !valid {
        return Err(invalid(
            "source",
            "inherited shape exceeds the prepaid role bound",
        ));
    }
    Ok(())
}

pub(crate) fn listener(
    runtime_root: OwnedFd,
    group: u32,
    b: &mut Budget<'_>,
) -> Result<(RuntimeListener, usize)> {
    b.with_prepaid_scope(FILE_STORAGE, 8, LOCAL_WORK, LISTENER_SCRATCH, |b| {
        b.reserve_storage(LISTENER_GROWTH)?;
        let root = RuntimeRoot::admit(
            runtime_root,
            Path::new(COMPILER_EXECUTION_SUPERVISOR_RUNTIME_DIRECTORY_V1),
            0,
            0,
            COMPILER_EXECUTION_SUPERVISOR_RUNTIME_DIRECTORY_MODE_V1,
        )?;
        let listener = RuntimeListener::construct(
            root,
            Path::new(COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1),
            "compiler-execution-supervisor.sock",
            0,
            group,
            COMPILER_EXECUTION_SUPERVISOR_SOCKET_MODE_V1,
        )?;
        Ok((listener, LISTENER_GROWTH))
    })
}

// This helper handles bytes/accounting only; each closed family supplies actual
// native decoders and seals the resulting nominal record against its real context.
pub(crate) fn record<const N: usize, T>(
    file: File,
    b: &mut Budget<'_>,
    decode: impl FnOnce(&[u8; N], &mut Budget<'_>) -> Result<(T, usize)>,
) -> Result<(T, usize)> {
    b.with_prepaid_scope(
        source::record_input_storage::<N>()?,
        8,
        8,
        2 * N + 4096,
        |b| {
            let bytes = source::read_record::<N>(&file, b)?;
            b.reserve_storage(N)?;
            decode(&bytes, b)
        },
    )
}

#[cfg(test)]
#[path = "native_inherited_tests.rs"]
mod tests;
