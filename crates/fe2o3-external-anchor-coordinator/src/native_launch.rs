//! Shared native launch accounting and root-owned transport, not a provider API.
use crate::native::{self, ExternalAnchorPreparationErrorV2 as Preparation};
use fe2o3_broker_authority_service::ProtectedExternalAnchorServiceErrorV2 as Admission;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileErrorV1 as Credentials,
    ProtectedServiceProfileErrorV2 as Profile, observations,
};
use fe2o3_protected_service_spawn::native_spawn::{
    ProtectedServiceSpawnErrorV2 as Spawn, RootOwnedProtectedServiceChildV2 as Child,
};
use rustix::{io::Errno, net, pipe};
use std::{error::Error as StdError, fmt, fs::File, mem::size_of, os::fd::OwnedFd};

pub(crate) type Result<T> = std::result::Result<T, ExternalAnchorLaunchErrorV2>;
pub(crate) const FILE_STORAGE: usize = size_of::<(File, usize)>();
pub(crate) const LOCAL_WORK: usize = 8 + 128 * (1024 + 64);

/// Native root launch refusal; no owned diagnostics or successful admission implied.
#[derive(Debug)]
pub enum ExternalAnchorLaunchErrorV2 {
    /// Original resource ledger or arithmetic refused.
    Resource(Resource),
    /// Actual native inputs or context refused.
    Preparation(Preparation),
    /// Shared native process custody refused.
    Spawn(Spawn),
    /// Native endpoint admission refused.
    Admission(Admission),
    /// Namespace observation refused.
    Profile(Profile),
    /// Invalid protected credentials.
    Credentials(Credentials),
    /// Bounded process-profile observation refused.
    Observation(observations::Error),
    /// Fixed launch protocol or ownership failure.
    Invalid(&'static str),
    /// The helper reported a pre-exec failure stage.
    ChildStage(u8),
    /// The direct child exited at this boundary.
    ChildExited(&'static str),
    /// Deadline or finite attempt limit was exhausted.
    Timeout(&'static str),
    /// A single non-retrying root descriptor operation failed.
    Io {
        /// Fixed operation name.
        operation: &'static str,
        /// Kernel errno.
        source: Errno,
    },
}
use ExternalAnchorLaunchErrorV2 as Error;
macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for Error {
            fn from(e: $source) -> Self {
                Self::$variant(e)
            }
        }
    };
}
from_error!(Resource, Resource);
from_error!(Preparation, Preparation);
from_error!(Spawn, Spawn);
from_error!(Admission, Admission);
from_error!(Profile, Profile);
from_error!(Credentials, Credentials);
from_error!(observations::Error, Observation);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Preparation(e) => e.fmt(f),
            Self::Spawn(e) => e.fmt(f),
            Self::Admission(e) => e.fmt(f),
            Self::Profile(e) => e.fmt(f),
            Self::Credentials(e) => e.fmt(f),
            Self::Observation(e) => e.fmt(f),
            Self::Invalid(s) => f.write_str(s),
            Self::ChildStage(stage) => write!(f, "native anchor child stage {stage}"),
            Self::ChildExited(s) => write!(f, "native anchor exited: {s}"),
            Self::Timeout(s) => write!(f, "native anchor launch timed out: {s}"),
            Self::Io { operation, source } => write!(f, "{operation}: {source}"),
        }
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Preparation(e) => Some(e),
            Self::Spawn(e) => Some(e),
            Self::Admission(e) => Some(e),
            Self::Profile(e) => Some(e),
            Self::Credentials(e) => Some(e),
            Self::Observation(e) => Some(e),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Additional unreserved managed-owner growth above the consumed prepared owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalAnchorLaunchStorageV2(pub(crate) usize);
impl ExternalAnchorLaunchStorageV2 {
    /// Reserve before retention; keep the consumed prepared owner's reservation.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Conservative complete launch work and additional peak above the input floor.
/// Logical units only, not execution time, generated stack or RSS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalAnchorLaunchQuotaV2 {
    pub(crate) work: usize,
    pub(crate) scratch: usize,
}
impl ExternalAnchorLaunchQuotaV2 {
    /// Includes all bounded readiness attempts and nested native operations.
    pub const fn work(self) -> usize {
        self.work
    }
    /// Includes all overlapping transfer images, transport and managed-owner growth.
    pub const fn scratch(self) -> usize {
        self.scratch
    }
}

pub(crate) fn io(operation: &'static str, source: Errno) -> Error {
    Error::Io { operation, source }
}
pub(crate) fn sum(values: &[usize]) -> Result<usize> {
    Ok(native::sum(values)?)
}

pub(crate) struct Channels {
    pub root: OwnedFd,
    pub child: OwnedFd,
    pub profile_reader: OwnedFd,
    pub profile_writer: OwnedFd,
    pub gate_reader: OwnedFd,
    pub gate_writer: OwnedFd,
}
impl Channels {
    pub const STORAGE: usize = 6 * FILE_STORAGE;
    // Called only inside the launch's prepaid fixed syscall/frame scope.
    pub fn new() -> Result<Self> {
        let (root, child) = net::socketpair(
            net::AddressFamily::UNIX,
            net::SocketType::SEQPACKET,
            net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
            None,
        )
        .map_err(|e| io("create native anchor bootstrap", e))?;
        let (profile_reader, profile_writer) =
            pipe::pipe_with(pipe::PipeFlags::CLOEXEC | pipe::PipeFlags::NONBLOCK)
                .map_err(|e| io("create native anchor profile pipe", e))?;
        let (gate_reader, gate_writer) = pipe::pipe_with(pipe::PipeFlags::CLOEXEC)
            .map_err(|e| io("create native anchor gate", e))?;
        Ok(Self {
            root,
            child,
            profile_reader,
            profile_writer,
            gate_reader,
            gate_writer,
        })
    }
}

pub(crate) struct Observer<'a, 'w> {
    pub child: &'a Child,
    pub budget: &'a mut Budget<'w>,
}
impl crate::launch_io::Observer for Observer<'_, '_> {
    type Error = Error;
    fn before_attempt(&mut self, boundary: crate::launch_io::Boundary) -> Result<()> {
        Ok(self.budget.charge_work(boundary.work())?)
    }
    fn is_live(&mut self) -> Result<bool> {
        Ok(self.child.is_live(self.budget)?)
    }
}

pub(crate) fn protocol(error: crate::launch_io::Error<Error>) -> Error {
    use crate::launch_io::{Error as E, Failure as F};
    match error {
        E::Observer(e) => e,
        E::Failure(f) => match f {
            F::Io { operation, source } => io(operation, source),
            F::InvalidTimeout => Error::Invalid("invalid native anchor launch timeout"),
            F::ChildStage(s) => Error::ChildStage(s),
            F::ChildExited(s) => Error::ChildExited(s),
            F::Timeout(s) => Error::Timeout(s),
            F::NoncanonicalProfileReady => {
                Error::Invalid("noncanonical native profile-ready record")
            }
            F::NoncanonicalGateRelease => Error::Invalid("noncanonical native release write"),
            F::MalformedReadyTransfer => Error::Invalid("malformed native ready transfer"),
            F::MalformedExecStatus => Error::Invalid("malformed native exec status"),
        },
    }
}
