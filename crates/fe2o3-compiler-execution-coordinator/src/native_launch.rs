//! Native supervisor launch mechanics; authority stays in the closed family adapters.
use crate::native::CompilerExecutionPreparationErrorV2 as Preparation;
use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2 as Capability;
use fe2o3_compiler_execution_lifecycle::LifecycleLeaseErrorV2 as Lifecycle;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionServiceLaunchManifestErrorV3 as ManifestV3,
    CompilerExecutionServiceReadyErrorV3 as ServiceReadyV3,
    CompilerExecutionSupervisorReadyErrorV2 as ReadyV2,
    CompilerExecutionSupervisorReadyErrorV3 as ReadyV3,
};
use fe2o3_compiler_execution_supervisor::ProtectedIssuerServiceProvisioningErrorV2 as Inputs;
use fe2o3_external_anchor_coordinator::ExternalAnchorLaunchErrorV2 as Anchor;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_protected_service_profile::{ProtectedServiceProfileErrorV2 as Profile, observations};
use fe2o3_protected_service_spawn::{
    ProtectedServiceCleanupErrorV2 as Cleanup, RetainedResourceAccessErrorV2 as Retained,
    launch_io,
    native_spawn::{
        ProtectedServiceSpawnErrorV2 as Spawn, RootOwnedRetainedServiceChildV2 as Child,
    },
};
use fe2o3_protected_static_executable::ProtectedStaticExecutableErrorV2 as Image;
use rustix::{io::Errno, net, pipe};
use std::{error::Error, fmt, fs::File, mem::size_of, os::fd::OwnedFd};

pub(crate) type Result<T> = std::result::Result<T, CompilerExecutionLaunchErrorV2>;
pub(crate) const FILE_STORAGE: usize = size_of::<(File, usize)>();
pub(crate) const LOCAL_WORK: usize = 8 + 128 * 1088;

/// Fixed-shape native launch refusal, without owned diagnostic strings.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionLaunchErrorV2 {
    /// Original resource account or checked arithmetic refused.
    Resource(Resource),
    /// Retained preparation or its root-bound cleanup guard refused.
    Preparation(Preparation),
    /// Actual sealed native context refused.
    Capability(Capability),
    /// Lifecycle transfer refused.
    Lifecycle(Lifecycle),
    /// Final listener/root transfer refused.
    ServiceInputs(Inputs),
    /// Actual live anchor transfer or continuity refused.
    Anchor(Anchor),
    /// Measured executable transfer refused.
    Executable(Image),
    /// Atomic spawn or child operation refused.
    Spawn(Spawn),
    /// Persistent cleanup funding refused.
    Cleanup(Cleanup),
    /// Retained preparation access is poisoned.
    Retained(Retained),
    /// Namespace observation refused.
    Profile(Profile),
    /// Protected process profile observation refused.
    Observation(observations::Error),
    /// Native V2 readiness framing or context refused.
    ReadyV2(ReadyV2),
    /// Native V3 readiness framing or context refused.
    ReadyV3(ReadyV3),
    /// Private native V3 issuer launch manifest refused.
    ManifestV3(ManifestV3),
    /// Private native V3 issuer readiness framing refused.
    ServiceReadyV3(ServiceReadyV3),
    /// Finite shared readiness transport refused.
    Transport(launch_io::Failure),
    /// Fixed contextual or lifecycle mismatch.
    Invalid(&'static str),
    /// One non-retrying descriptor operation failed.
    Io {
        /// Fixed operation name.
        operation: &'static str,
        /// Kernel errno.
        source: Errno,
    },
}
use CompilerExecutionLaunchErrorV2 as Failure;
macro_rules! errors {
    ($($source:ty => $variant:ident),+ $(,)?) => {
        $(impl From<$source> for Failure {
            fn from(error: $source) -> Self { Self::$variant(error) }
        })+
        impl Error for Failure {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                match self {
                    $(Self::$variant(error) => Some(error),)+
                    Self::Io { source, .. } => Some(source),
                    Self::Transport(_) | Self::Invalid(_) => None,
                }
            }
        }
        impl fmt::Display for Failure {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $(Self::$variant(error) => error.fmt(f),)+
                    Self::Io { operation, source } => write!(f, "{operation}: {source}"),
                    Self::Transport(error) => write!(f, "native supervisor transport: {error:?}"),
                    Self::Invalid(message) => f.write_str(message),
                }
            }
        }
    };
}
errors!(Resource => Resource, Preparation => Preparation, Capability => Capability,
    Lifecycle => Lifecycle, Inputs => ServiceInputs, Anchor => Anchor, Image => Executable,
    Spawn => Spawn, Cleanup => Cleanup, Retained => Retained, Profile => Profile,
    observations::Error => Observation, ReadyV2 => ReadyV2, ReadyV3 => ReadyV3,
    ManifestV3 => ManifestV3, ServiceReadyV3 => ServiceReadyV3);

impl From<launch_io::Failure> for Failure {
    fn from(error: launch_io::Failure) -> Self {
        Self::Transport(error)
    }
}
impl From<launch_io::Error<Failure>> for Failure {
    fn from(error: launch_io::Error<Failure>) -> Self {
        match error {
            launch_io::Error::Failure(error) => error.into(),
            launch_io::Error::Observer(error) => error,
        }
    }
}

/// Additional unreserved growth above the consumed complete preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionLaunchStorageV2(pub(crate) usize);
impl CompilerExecutionLaunchStorageV2 {
    /// Reserve before retaining the result; keep the consumed input reservation.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Conservative complete original-request work and extra peak above preparation.
/// Not a time, allocator, generated-stack or RSS guarantee. Persistent cleanup
/// funding is separate and must include the complete retained preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionLaunchQuotaV2 {
    pub(crate) work: usize,
    pub(crate) scratch: usize,
}
impl CompilerExecutionLaunchQuotaV2 {
    /// Includes every finite readiness attempt and nested native operation.
    pub const fn work(self) -> usize {
        self.work
    }
    /// Includes full overlapping image charges, transport and returned growth.
    pub const fn scratch(self) -> usize {
        self.scratch
    }
}

pub(crate) fn sum(values: &[usize]) -> Result<usize> {
    values.iter().try_fold(0usize, |total, n| {
        total.checked_add(*n).ok_or(Resource::Arithmetic.into())
    })
}
pub(crate) fn io(operation: &'static str, source: Errno) -> Failure {
    Failure::Io { operation, source }
}
pub(crate) fn pid_u32(pid: rustix::process::Pid) -> Result<u32> {
    u32::try_from(pid.as_raw_pid()).map_err(|_| Failure::Invalid("invalid supervisor PID"))
}

pub(crate) struct Channels {
    pub root: OwnedFd,
    pub child: OwnedFd,
    pub exec_reader: OwnedFd,
    pub exec_writer: OwnedFd,
    pub profile_reader: OwnedFd,
    pub profile_writer: OwnedFd,
    pub gate_reader: OwnedFd,
    pub gate_writer: OwnedFd,
}
impl Channels {
    pub const STORAGE: usize = 8 * FILE_STORAGE;
    // The enclosing launch prepays the complete descriptor/frame and syscall charge.
    pub fn new() -> Result<Self> {
        let (root, child) = net::socketpair(
            net::AddressFamily::UNIX,
            net::SocketType::SEQPACKET,
            net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
            None,
        )
        .map_err(|e| io("create native supervisor bootstrap", e))?;
        net::sockopt::set_socket_passcred(&root, true)
            .map_err(|e| io("enable supervisor message credentials", e))?;
        let (exec_reader, exec_writer) = net::socketpair(
            net::AddressFamily::UNIX,
            net::SocketType::SEQPACKET,
            net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
            None,
        )
        .map_err(|e| io("create native supervisor exec status", e))?;
        let (profile_reader, profile_writer) =
            pipe::pipe_with(pipe::PipeFlags::CLOEXEC | pipe::PipeFlags::NONBLOCK)
                .map_err(|e| io("create native supervisor profile channel", e))?;
        let (gate_reader, gate_writer) = pipe::pipe_with(pipe::PipeFlags::CLOEXEC)
            .map_err(|e| io("create native supervisor release gate", e))?;
        Ok(Self {
            root,
            child,
            exec_reader,
            exec_writer,
            profile_reader,
            profile_writer,
            gate_reader,
            gate_writer,
        })
    }
}

pub(crate) struct Observer<'a, 'w, T: Send + 'static> {
    pub child: &'a Child<T>,
    pub budget: &'a mut Budget<'w>,
}
impl<T: Send + 'static> launch_io::Observer for Observer<'_, '_, T> {
    type Error = Failure;
    fn before_attempt(&mut self, boundary: launch_io::Boundary) -> Result<()> {
        Ok(self.budget.charge_work(boundary.work())?)
    }
    fn is_live(&mut self) -> Result<bool> {
        Ok(self.child.is_live(self.budget)?)
    }
}
