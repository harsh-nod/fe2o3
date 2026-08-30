#![deny(missing_docs, unsafe_code)]
#![doc = include_str!("../README.md")]

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("fe2o3-protected-service-spawn requires Linux x86-64");

#[allow(unsafe_code)]
mod syscall;

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
use std::os::fd::{BorrowedFd, OwnedFd, RawFd};

use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileV1, ProtectedServiceProfileErrorV1, require_owned_sigchld_v1,
};

/// Maximum inherited descriptors installed by one protected-service spawn.
pub const MAX_PROTECTED_SERVICE_DESCRIPTOR_BINDINGS_V1: usize = 32;
/// First descriptor used for private post-clone staging.
pub const PROTECTED_SERVICE_STAGED_DESCRIPTOR_FLOOR_V1: RawFd = 400;
/// Exact child-to-parent profile-ready token.
pub const PROTECTED_SERVICE_PROFILE_READY_V1: u8 = 0xa5;
/// Exact parent-to-child gate-release token.
pub const PROTECTED_SERVICE_GATE_RELEASE_V1: u8 = 0x5a;

/// Requires exact real, effective, saved, and filesystem root IDs.
pub fn require_exact_root_identity_v1() -> Result<(), ProtectedServiceSpawnErrorV1> {
    if syscall::has_exact_root_identity() {
        Ok(())
    } else {
        Err(ProtectedServiceSpawnErrorV1::RootRequired)
    }
}

/// One borrowed source descriptor and its exact post-exec destination.
#[derive(Clone, Copy)]
pub struct ProtectedServiceDescriptorBindingV1<'a> {
    source: BorrowedFd<'a>,
    destination: RawFd,
}

impl<'a> ProtectedServiceDescriptorBindingV1<'a> {
    /// Binds one borrowed source to a fixed non-standard descriptor below the staging range.
    pub fn new(
        source: BorrowedFd<'a>,
        destination: RawFd,
    ) -> Result<Self, ProtectedServiceSpawnErrorV1> {
        if !(3..PROTECTED_SERVICE_STAGED_DESCRIPTOR_FLOOR_V1).contains(&destination) {
            return Err(ProtectedServiceSpawnErrorV1::InvalidDestination(
                destination,
            ));
        }
        Ok(Self {
            source,
            destination,
        })
    }

    /// Returns the exact post-exec destination descriptor.
    pub const fn destination(self) -> RawFd {
        self.destination
    }
}

/// Privately staged executable, descriptor table, and profile-gate channels.
///
/// The caller remains responsible for admitting and pinning the executable before staging. This
/// value is move-only and exposes no staged descriptor.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::StagedProtectedServiceExecV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<StagedProtectedServiceExecV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_protected_service_spawn::StagedProtectedServiceExecV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<StagedProtectedServiceExecV1>();
/// ```
pub struct StagedProtectedServiceExecV1 {
    inner: syscall::StagedProtectedServiceExecV1,
}

impl fmt::Debug for StagedProtectedServiceExecV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StagedProtectedServiceExecV1")
            .field("authority", &"root-protected-service-exec-only")
            .field("descriptor_count", &self.inner.descriptor_count())
            .finish_non_exhaustive()
    }
}

impl StagedProtectedServiceExecV1 {
    /// Duplicates every input above the fixed staging floor and freezes the descriptor table.
    pub fn new(
        executable: &File,
        bindings: &[ProtectedServiceDescriptorBindingV1<'_>],
        profile_ready_writer: BorrowedFd<'_>,
        gate_reader: BorrowedFd<'_>,
        exec_status_writer: BorrowedFd<'_>,
    ) -> Result<Self, ProtectedServiceSpawnErrorV1> {
        if bindings.is_empty() || bindings.len() > MAX_PROTECTED_SERVICE_DESCRIPTOR_BINDINGS_V1 {
            return Err(ProtectedServiceSpawnErrorV1::InvalidBindingCount(
                bindings.len(),
            ));
        }
        let mut destinations = [false; PROTECTED_SERVICE_STAGED_DESCRIPTOR_FLOOR_V1 as usize];
        for binding in bindings {
            let destination = usize::try_from(binding.destination).map_err(|_| {
                ProtectedServiceSpawnErrorV1::InvalidDestination(binding.destination)
            })?;
            if destinations[destination] {
                return Err(ProtectedServiceSpawnErrorV1::DuplicateDestination(
                    binding.destination,
                ));
            }
            destinations[destination] = true;
        }
        syscall::StagedProtectedServiceExecV1::new(
            executable,
            bindings,
            profile_ready_writer,
            gate_reader,
            exec_status_writer,
        )
        .map(|inner| Self { inner })
        .map_err(|source| io_error("stage protected-service descriptors", source))
    }

    /// Creates one direct protected-service child and atomically obtains its pidfd.
    ///
    /// The invoking process must have exact real, effective, saved, and filesystem root IDs.
    pub fn spawn(
        &self,
        credentials: ProtectedServiceCredentialProfileV1,
    ) -> Result<RootOwnedProtectedServiceChildV1, ProtectedServiceSpawnErrorV1> {
        require_exact_root_identity_v1()?;
        require_owned_sigchld_v1().map_err(ProtectedServiceSpawnErrorV1::ParentProfile)?;
        let cap_last_cap = read_cap_last_cap()?;
        syscall::spawn(
            &self.inner,
            credentials,
            cap_last_cap,
            rustix::process::getpid(),
        )
        .map(RootOwnedProtectedServiceChildV1::from_inner)
        .map_err(|source| io_error("clone protected-service child", source))
    }
}

/// Root-retained exact pidfd and reaping custody for one protected-service child.
///
/// Dropping live custody sends `SIGKILL` and synchronously reaps the direct child.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::RootOwnedProtectedServiceChildV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<RootOwnedProtectedServiceChildV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_protected_service_spawn::RootOwnedProtectedServiceChildV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<RootOwnedProtectedServiceChildV1>();
/// ```
pub struct RootOwnedProtectedServiceChildV1 {
    inner: syscall::RootOwnedProtectedServiceChildV1,
}

impl fmt::Debug for RootOwnedProtectedServiceChildV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RootOwnedProtectedServiceChildV1")
            .field("authority", &"root-lifecycle-custody-only")
            .field("pid", &self.pid())
            .finish_non_exhaustive()
    }
}

impl RootOwnedProtectedServiceChildV1 {
    fn from_inner(inner: syscall::RootOwnedProtectedServiceChildV1) -> Self {
        Self { inner }
    }

    /// Admits caller-created direct-child custody for non-authoritative lifecycle tests.
    #[cfg(feature = "test-support")]
    pub fn admit_non_authoritative_test(
        pid: rustix::process::Pid,
        pidfd: OwnedFd,
    ) -> Result<Self, ProtectedServiceSpawnErrorV1> {
        syscall::RootOwnedProtectedServiceChildV1::admit_non_authoritative_test(pid, pidfd)
            .map(Self::from_inner)
            .map_err(|source| io_error("admit test protected-service pidfd", source))
    }

    /// Returns the exact direct-child PID.
    pub const fn pid(&self) -> rustix::process::Pid {
        self.inner.pid()
    }

    /// Returns whether the exact pidfd child is currently live without reaping it.
    pub fn is_live(&self) -> Result<bool, ProtectedServiceSpawnErrorV1> {
        self.inner
            .is_live()
            .map_err(|source| io_error("observe exact protected-service pidfd", source))
    }

    /// Duplicates the retained pidfd for one already validated authority transfer.
    pub fn try_clone_pidfd(&self) -> Result<OwnedFd, ProtectedServiceSpawnErrorV1> {
        self.inner
            .try_clone_pidfd()
            .map_err(|source| io_error("clone exact protected-service pidfd", source))
    }

    /// Returns an inert immediate exit description when the child has exited.
    pub fn exit_description(&self, fallback: &'static str) -> String {
        self.inner.exit_description(fallback)
    }

    /// Sends `SIGKILL` through the pidfd and synchronously reaps the exact child once.
    pub fn cancel_and_reap(&mut self) -> Result<(), ProtectedServiceSpawnErrorV1> {
        self.inner.cancel_and_reap().map_err(map_reap_error)
    }
}

fn read_cap_last_cap() -> Result<u32, ProtectedServiceSpawnErrorV1> {
    let text = std::fs::read_to_string("/proc/sys/kernel/cap_last_cap")
        .map_err(|source| io_error("read kernel capability ceiling", source))?;
    let value = text
        .trim()
        .parse::<u32>()
        .map_err(|_| ProtectedServiceSpawnErrorV1::InvalidKernelProfile)?;
    if value > 63 {
        return Err(ProtectedServiceSpawnErrorV1::InvalidKernelProfile);
    }
    Ok(value)
}

fn map_reap_error(error: syscall::ReapErrorV1) -> ProtectedServiceSpawnErrorV1 {
    match error {
        syscall::ReapErrorV1::OwnershipLost => ProtectedServiceSpawnErrorV1::ReapingOwnershipLost,
        syscall::ReapErrorV1::Io(source) => io_error("reap exact protected-service pidfd", source),
    }
}

fn io_error(operation: &'static str, source: io::Error) -> ProtectedServiceSpawnErrorV1 {
    ProtectedServiceSpawnErrorV1::Io { operation, source }
}

/// Stable preparation, spawn, or lifecycle failures.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedServiceSpawnErrorV1 {
    /// The operation requires exact real root credentials.
    RootRequired,
    /// The descriptor table is empty or exceeds the fixed bound.
    InvalidBindingCount(usize),
    /// A destination overlaps standard I/O or the private staging range.
    InvalidDestination(RawFd),
    /// Two source descriptors target the same inherited descriptor.
    DuplicateDestination(RawFd),
    /// The host capability ceiling is outside the supported kernel profile.
    InvalidKernelProfile,
    /// The root parent cannot retain exclusive direct-child reaping custody.
    ParentProfile(ProtectedServiceProfileErrorV1),
    /// Another owner reaped the direct child.
    ReapingOwnershipLost,
    /// A bounded kernel or filesystem operation failed.
    Io {
        /// Exact operation that failed.
        operation: &'static str,
        /// Kernel or filesystem failure.
        source: io::Error,
    },
}

impl fmt::Display for ProtectedServiceSpawnErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootRequired => formatter.write_str("protected-service spawn requires root"),
            Self::InvalidBindingCount(count) => {
                write!(
                    formatter,
                    "invalid protected-service descriptor count {count}"
                )
            }
            Self::InvalidDestination(destination) => write!(
                formatter,
                "invalid protected-service destination descriptor {destination}"
            ),
            Self::DuplicateDestination(destination) => write!(
                formatter,
                "duplicate protected-service destination descriptor {destination}"
            ),
            Self::InvalidKernelProfile => {
                formatter.write_str("unsupported protected-service kernel capability profile")
            }
            Self::ParentProfile(error) => {
                write!(
                    formatter,
                    "invalid protected-service parent profile: {error}"
                )
            }
            Self::ReapingOwnershipLost => {
                formatter.write_str("protected-service child reaping ownership was lost")
            }
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for ProtectedServiceSpawnErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ParentProfile(source) => Some(source),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsFd;

    use rustix::net::{AddressFamily, SocketFlags, SocketType, socketpair};

    use super::*;

    #[test]
    fn descriptor_bindings_are_bounded_unique_and_below_staging() {
        let (first, second) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        assert!(ProtectedServiceDescriptorBindingV1::new(first.as_fd(), 3).is_ok());
        assert!(matches!(
            ProtectedServiceDescriptorBindingV1::new(first.as_fd(), 2),
            Err(ProtectedServiceSpawnErrorV1::InvalidDestination(2))
        ));
        assert!(matches!(
            ProtectedServiceDescriptorBindingV1::new(
                second.as_fd(),
                PROTECTED_SERVICE_STAGED_DESCRIPTOR_FLOOR_V1
            ),
            Err(ProtectedServiceSpawnErrorV1::InvalidDestination(_))
        ));
    }

    #[test]
    fn staged_table_rejects_empty_duplicate_and_excess_bindings() {
        let executable = File::open("/proc/self/exe").unwrap();
        let (source, _) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
        let (_, profile_writer) =
            rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
        let (gate_reader, _) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
        let (_, status_writer) = rustix::net::socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();

        assert!(matches!(
            StagedProtectedServiceExecV1::new(
                &executable,
                &[],
                profile_writer.as_fd(),
                gate_reader.as_fd(),
                status_writer.as_fd(),
            ),
            Err(ProtectedServiceSpawnErrorV1::InvalidBindingCount(0))
        ));
        let duplicate = [
            ProtectedServiceDescriptorBindingV1::new(source.as_fd(), 3).unwrap(),
            ProtectedServiceDescriptorBindingV1::new(source.as_fd(), 3).unwrap(),
        ];
        assert!(matches!(
            StagedProtectedServiceExecV1::new(
                &executable,
                &duplicate,
                profile_writer.as_fd(),
                gate_reader.as_fd(),
                status_writer.as_fd(),
            ),
            Err(ProtectedServiceSpawnErrorV1::DuplicateDestination(3))
        ));
        let excess = (0..=MAX_PROTECTED_SERVICE_DESCRIPTOR_BINDINGS_V1)
            .map(|index| {
                ProtectedServiceDescriptorBindingV1::new(
                    source.as_fd(),
                    3 + i32::try_from(index).unwrap(),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            StagedProtectedServiceExecV1::new(
                &executable,
                &excess,
                profile_writer.as_fd(),
                gate_reader.as_fd(),
                status_writer.as_fd(),
            ),
            Err(ProtectedServiceSpawnErrorV1::InvalidBindingCount(count))
                if count == MAX_PROTECTED_SERVICE_DESCRIPTOR_BINDINGS_V1 + 1
        ));
    }

    #[test]
    fn non_root_spawn_fails_before_clone() {
        if rustix::process::geteuid().is_root() || rustix::process::getegid().is_root() {
            return;
        }
        let executable = File::open("/proc/self/exe").unwrap();
        let (source, _) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
        let (profile_reader, profile_writer) =
            rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
        let (gate_reader, gate_writer) =
            rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
        let (status_reader, status_writer) = rustix::net::socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap();
        let binding = ProtectedServiceDescriptorBindingV1::new(source.as_fd(), 3).unwrap();
        let staged = StagedProtectedServiceExecV1::new(
            &executable,
            &[binding],
            profile_writer.as_fd(),
            gate_reader.as_fd(),
            status_writer.as_fd(),
        )
        .unwrap();
        let credentials = ProtectedServiceCredentialProfileV1::new(
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .unwrap();
        assert!(matches!(
            staged.spawn(credentials),
            Err(ProtectedServiceSpawnErrorV1::RootRequired)
        ));
        drop((profile_reader, gate_writer, status_reader));
    }
}
