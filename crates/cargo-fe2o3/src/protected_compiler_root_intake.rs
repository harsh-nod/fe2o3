//! Original-root TCB transport only. No accepted/compiler-ready wire status exists.
use super::ParentRustcInvocationCustody as Parent;
use crate::capability_broker::BrokeredInvocationAuthorityV1 as InvocationAuthority;
use fe2o3_compiler_closure_capability::{
    CompilerExecutionCapabilityErrorV2 as CapabilityError,
    CompilerExecutionClientProfileCapabilityV3 as Profile,
    RustcInvocationCapabilityV1 as Invocation,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ROOT_INTAKE_BYTES_V3 as N,
    COMPILER_EXECUTION_ROOT_INTAKE_STORAGE_V3 as RECORD_SCRATCH,
    COMPILER_EXECUTION_ROOT_INTAKE_WORK_V3 as RECORD_WORK,
    COMPILER_EXECUTION_SUPERVISOR_RUNTIME_DIRECTORY_MODE_V1 as DIRECTORY_MODE,
    COMPILER_EXECUTION_SUPERVISOR_SOCKET_MODE_V1 as SOCKET_MODE,
    COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1 as SOCKET_PATH,
    CompilerExecutionRootIntakeErrorV3 as RecordError, CompilerExecutionRootIntakeKindV3 as Kind,
    CompilerExecutionRootIntakeRecordV3 as Record, CompilerExecutionRootIntakeRoleV3 as Role,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_protected_service_spawn::launch_io::{self as io, MessageSender};
use fe2o3_rustc_invocation::{InvocationDigestV3, MAX_DESCRIPTOR_BYTES_V3};
use rustix::{fs, net};
use std::{
    fmt,
    fs::File,
    mem::size_of,
    os::fd::{AsFd, OwnedFd},
    time::{Duration, Instant},
};

const MAX_ATTEMPTS: usize = 120_001;
const LOCAL_WORK: usize = 8 + 128 * 1024;
const FRAME: usize = 4 * size_of::<Endpoint>() + 16 * N + 16 * size_of::<Error>() + 16384;
const TRANSFERS: usize = Invocation::NATIVE_FILE_STORAGE + 4 * size_of::<OwnedFd>();
pub(crate) const PARENT_MAX_STORAGE: usize =
    2 * Invocation::NATIVE_MAX_RETAINED_STORAGE + size_of::<Parent>();
// Existing V2 capture, V3 binding, descriptor clone, sealing and bounded
// validation passes. Paid BEFORE native capture, not retrospectively at contact.
pub(crate) const CAPTURE_WORK: usize = 8 * Invocation::NATIVE_ADMISSION_WORK;
pub(crate) const CAPTURE_SCRATCH: usize = 2 * Invocation::NATIVE_OPERATION_SCRATCH + 8192;

pub(crate) fn prepay_capture_once(
    used: &std::cell::Cell<bool>,
    command: &std::process::Command,
    argv0: &std::ffi::OsStr,
    cwd: &std::path::Path,
    environment: &[(std::ffi::OsString, std::ffi::OsString)],
    b: &mut Budget<'_>,
) -> Result<()> {
    if used.replace(true) {
        return Err(Error::Rejected("native capture reservation already used"));
    }
    b.charge_work(CAPTURE_WORK)?;
    check_capture_shape(command, argv0, cwd, environment)?;
    b.reserve_storage(PARENT_MAX_STORAGE)?;
    b.reserve_storage(CAPTURE_SCRATCH)?;
    b.reserve_storage(size_of::<InvocationAuthority>())?;
    Ok(())
}

/// Conservative allocation-free shape bound before capture copies any input.
/// Not a second validator or a universal bound on the old Command/environment
/// preparation pipeline. Canonical admission still uses the shared decoder.
pub(crate) fn check_capture_shape(
    command: &std::process::Command,
    argv0: &std::ffi::OsStr,
    cwd: &std::path::Path,
    environment: &[(std::ffi::OsString, std::ffi::OsString)],
) -> Result<()> {
    use fe2o3_rustc_invocation::{
        MAX_ARGUMENT_BYTES_V2, MAX_COMPILE_ENVIRONMENT_ENTRIES_V2, MAX_ENVIRONMENT_VALUE_BYTES_V2,
        MAX_NAME_BYTES_V2, MAX_PATH_BYTES_V2, MAX_RUSTC_ARGUMENTS_V2,
    };
    use std::os::unix::ffi::OsStrExt;
    if command.get_args().len() >= MAX_RUSTC_ARGUMENTS_V2
        || environment.len() > MAX_COMPILE_ENVIRONMENT_ENTRIES_V2
    {
        return Err(Error::Rejected("native capture count bound"));
    }
    // Overestimates fixed V3 header/closure/hash/count fields and field lengths.
    let mut total = 1024usize;
    let mut add = |bytes: usize, limit: usize| -> Result<()> {
        if bytes > limit {
            return Err(Error::Rejected("native capture field bound"));
        }
        total = total
            .checked_add(bytes)
            .and_then(|n| n.checked_add(8))
            .ok_or(Resource::Arithmetic)?;
        if total > MAX_DESCRIPTOR_BYTES_V3 {
            return Err(Error::Rejected("native capture aggregate bound"));
        }
        Ok(())
    };
    add(argv0.as_bytes().len(), MAX_ARGUMENT_BYTES_V2)?;
    add(cwd.as_os_str().as_bytes().len(), MAX_PATH_BYTES_V2)?;
    for argument in command.get_args() {
        add(argument.as_bytes().len(), MAX_ARGUMENT_BYTES_V2)?;
    }
    for (name, value) in environment {
        add(name.as_bytes().len(), MAX_NAME_BYTES_V2)?;
        add(value.as_bytes().len(), MAX_ENVIRONMENT_VALUE_BYTES_V2)?;
    }
    Ok(())
}
// These are COMPLETE additional transport quotas above parent/profile/authority
// owners. The V4 broker profile import has its own original-request quote.
pub(crate) const WORK: usize = LOCAL_WORK
    + MAX_ATTEMPTS * (LOCAL_WORK + io::packet_receive_work(N) + 4 * RECORD_WORK)
    + 2 * (PARENT_MAX_STORAGE + 6 * 1024 + Invocation::NATIVE_REVALIDATION_WORK + 8 + 3 * 1024)
    + 2 * Profile::IO_WORK
    + Invocation::NATIVE_TRANSFER_WORK
    + 4096 * MAX_DESCRIPTOR_BYTES_V3;
pub(crate) const SCRATCH: usize = FRAME
    + TRANSFERS
    + Invocation::NATIVE_TRANSFER_SCRATCH
    + Invocation::NATIVE_OPERATION_SCRATCH
    + Profile::IO_STORAGE
    + RECORD_SCRATCH
    + io::packet_receive_scratch(N)
    + 8192;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Refusal {
    RuntimeEnforcementUnavailable,
}

#[derive(Debug)]
pub(crate) enum Error {
    Resource(Resource),
    Capability(CapabilityError),
    Record(RecordError),
    Io {
        operation: &'static str,
        source: rustix::io::Errno,
    },
    Rejected(&'static str),
}
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<CapabilityError> for Error {
    fn from(e: CapabilityError) -> Self {
        Self::Capability(e)
    }
}
impl From<RecordError> for Error {
    fn from(e: RecordError) -> Self {
        Self::Record(e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Capability(e) => e.fmt(f),
            Self::Record(e) => e.fmt(f),
            Self::Io { operation, source } => write!(f, "{operation}: {source}"),
            Self::Rejected(reason) => f.write_str(reason),
        }
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;
fn transport(error: io::Failure) -> Error {
    match error {
        io::Failure::Io { operation, source } => Error::Io { operation, source },
        _ => Error::Rejected("authenticated root intake transport refused"),
    }
}
fn syscall<T>(operation: &'static str, result: rustix::io::Result<T>) -> Result<T> {
    result.map_err(|source| Error::Io { operation, source })
}

impl Parent {
    /// Keeps the actual invocation stream borrowed until the exact authenticated
    /// terminal ACK. There is deliberately no successful FD195 transition here:
    /// failure/EOF never releases authority to a child, and the only reply refuses
    /// runtime enforcement. The caller must keep the original profile admission
    /// account and these full input owners prepaid, not wrap them in a fresh meter.
    pub(crate) fn contact_original_root(
        &self,
        profile: &Profile,
        _authority: &InvocationAuthority,
        b: &mut Budget<'_>,
    ) -> Result<Refusal> {
        let floor = self
            .native_retained_storage()?
            .checked_add(profile.retained_storage())
            .and_then(|n| n.checked_add(size_of::<InvocationAuthority>()))
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, LOCAL_WORK, FRAME, |b| {
            self.revalidate_native(b)?;
            profile.revalidate(b)?;
            let stdio = self
                .stdio
                .as_ref()
                .ok_or(Error::Rejected("missing original wrapper stdio capture"))?;
            // Stage Some clears CLOEXEC at its final destination. Select only
            // streams which the original exec would keep; retain the complete
            // original capture in Parent, including closed-on-exec observations.
            let streams = [stdio.stdin(), stdio.stdout(), stdio.stderr()].map(|slot| {
                slot.filter(|fd| !fd.descriptor_flags().contains(rustix::io::FdFlags::CLOEXEC))
            });
            let mask = streams
                .iter()
                .enumerate()
                .fold(0, |mask, (i, fd)| mask | (u8::from(fd.is_some()) << i));
            b.reserve_storage(TRANSFERS)?;
            let (invocation, _) = self.capability.try_clone_for_transfer_native(b)?;
            let length = syscall("root intake invocation length", fs::fstat(&invocation))?.st_size;
            let identity = b.with_prepaid_scope(
                0,
                8,
                4096 * MAX_DESCRIPTOR_BYTES_V3,
                Invocation::NATIVE_OPERATION_SCRATCH,
                |_| {
                    InvocationDigestV3::calculate(self.invocation.descriptor())
                        .map_err(|_| Error::Rejected("cannot bind exact invocation descriptor"))
                },
            )?;
            let mut nonce = [0; 32];
            if syscall(
                "wrapper intake nonce",
                rustix::rand::getrandom(&mut nonce, rustix::rand::GetRandomFlags::NONBLOCK),
            )? != nonce.len()
            {
                return Err(Error::Rejected("short wrapper intake nonce"));
            }
            let (hello, _) = Record::hello(
                profile.profile().policy(),
                identity.into_bytes(),
                nonce,
                mask,
                u64::try_from(length).map_err(|_| Error::Rejected("negative invocation length"))?,
                b,
            )?;
            let endpoint = Endpoint::capture(profile.profile().supervisor_gid())?;
            let socket = syscall(
                "create root intake socket",
                net::socket_with(
                    net::AddressFamily::UNIX,
                    net::SocketType::SEQPACKET,
                    net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
                    None,
                ),
            )?;
            syscall(
                "enable root packet credentials",
                net::sockopt::set_socket_passcred(&socket, true),
            )?;
            // Linux may autobind once PASSCRED is enabled; no client path identity
            // is inferred. Only exact fixed server binding and kernel credentials.
            let address = syscall(
                "fixed root socket address",
                net::SocketAddrUnix::new(SOCKET_PATH),
            )?;
            let deadline = Instant::now()
                .checked_add(Duration::from_secs(120))
                .ok_or(Error::Rejected("intake deadline overflow"))?;
            let mut phase = Phase::Connect;
            let mut sender = None;
            let mut challenge = None;
            let mut last = None;
            for _ in 0..MAX_ATTEMPTS {
                b.charge_work(LOCAL_WORK)?;
                if Instant::now() >= deadline {
                    return Err(Error::Rejected("root intake deadline exceeded"));
                }
                let finished = b.with_prepaid_scope(
                    0,
                    8,
                    io::packet_receive_work(N),
                    io::packet_receive_scratch(N),
                    |b| {
                        match phase {
                            Phase::Connect => {
                                match net::connect(&socket, &address) {
                                    Ok(()) => {}
                                    Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                                        return Ok(false);
                                    }
                                    Err(source) => {
                                        return Err(Error::Io {
                                            operation: "connect original root",
                                            source,
                                        });
                                    }
                                }
                                endpoint.revalidate()?;
                                let peer = syscall(
                                    "authenticate original root peer",
                                    net::sockopt::socket_peercred(&socket),
                                )?;
                                sender = Some(root_sender(peer)?);
                                phase = Phase::Hello;
                            }
                            Phase::Hello => {
                                if io::send_packet(socket.as_fd(), hello.canonical_bytes())
                                    .map_err(transport)?
                                    .is_some()
                                {
                                    phase = Phase::Challenge;
                                }
                            }
                            Phase::Challenge => {
                                let Some(bytes) = io::receive_authenticated_packet::<N>(
                                    socket.as_fd(),
                                    sender.unwrap(),
                                )
                                .map_err(transport)?
                                else {
                                    return Ok(false);
                                };
                                let (record, _) = Record::decode(&bytes, b)?;
                                if record.kind() != Kind::Challenge
                                    || !record.matches_predecessor(&hello, b)?
                                {
                                    return Err(Error::Rejected(
                                        "root challenge differs from exact hello",
                                    ));
                                }
                                challenge = Some(record);
                                phase = Phase::Input(0);
                            }
                            Phase::Input(index) => {
                                let challenge = challenge.as_ref().unwrap();
                                let role = challenge
                                    .roles()
                                    .nth(index)
                                    .ok_or(Error::Rejected("input count exceeds selected roles"))?;
                                let (record, _) = Record::input(challenge, role, b)?;
                                let source = match role {
                                    Role::Invocation => invocation.as_fd(),
                                    Role::WorkingDirectory => self
                                        .working_directory
                                        .native_source()
                                        .map_err(|_| Error::Rejected("original cwd changed"))?,
                                    Role::Stdin => streams[0].unwrap().source(),
                                    Role::Stdout => streams[1].unwrap().source(),
                                    Role::Stderr => streams[2].unwrap().source(),
                                };
                                stdio
                                    .revalidate()
                                    .map_err(|_| Error::Rejected("original stdio changed"))?;
                                if io::send_packet_with_descriptor(
                                    socket.as_fd(),
                                    record.canonical_bytes(),
                                    source,
                                )
                                .map_err(transport)?
                                .is_some()
                                {
                                    last = Some(record);
                                    phase = if index + 1 == challenge.roles().count() {
                                        Phase::Ack
                                    } else {
                                        Phase::Input(index + 1)
                                    };
                                }
                            }
                            Phase::Ack => {
                                let Some(bytes) = io::receive_authenticated_packet::<N>(
                                    socket.as_fd(),
                                    sender.unwrap(),
                                )
                                .map_err(transport)?
                                else {
                                    return Ok(false);
                                };
                                let (ack, _) = Record::decode(&bytes, b)?;
                                if ack.kind() != Kind::Ack
                                    || !ack.matches_predecessor(last.as_ref().unwrap(), b)?
                                {
                                    return Err(Error::Rejected(
                                        "root ACK differs from complete intake",
                                    ));
                                }
                                endpoint.revalidate()?;
                                profile.revalidate(b)?;
                                self.revalidate_native(b)?;
                                return Ok(true);
                            }
                        }
                        Ok(false)
                    },
                )?;
                if Instant::now() >= deadline {
                    return Err(Error::Rejected("root intake deadline exceeded"));
                }
                if finished {
                    return Ok(Refusal::RuntimeEnforcementUnavailable);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(Error::Rejected("root intake attempt bound exhausted"))
        })
    }
}

#[derive(Clone, Copy)]
enum Phase {
    Connect,
    Hello,
    Challenge,
    Input(usize),
    Ack,
}

fn root_sender(peer: net::UCred) -> Result<MessageSender> {
    // This is the documented privileged root transport TCB, not the nonroot
    // supervisor identity from ClientProfileV3 and not measured process authority.
    if peer.uid.as_raw() != 0 || peer.gid.as_raw() != 0 {
        return Err(Error::Rejected(
            "intake peer is not the original root transport",
        ));
    }
    Ok(MessageSender::new(peer.pid.as_raw_pid(), 0, 0))
}

const DIRECTORY_FLAGS: fs::OFlags = fs::OFlags::RDONLY
    .union(fs::OFlags::DIRECTORY)
    .union(fs::OFlags::NOFOLLOW)
    .union(fs::OFlags::CLOEXEC);
#[derive(Clone, Copy, Eq, PartialEq)]
struct Identity {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
}
impl Identity {
    fn stat(stat: fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            uid: stat.st_uid,
            gid: stat.st_gid,
        }
    }
    fn directory(self, exact: bool) -> Result<Self> {
        if self.uid != 0
            || self.gid != 0
            || fs::FileType::from_raw_mode(self.mode) != fs::FileType::Directory
            || self.mode & 0o022 != 0
            || (exact && self.mode & 0o7777 != DIRECTORY_MODE)
        {
            return Err(Error::Rejected(
                "root intake directory is not fixed root custody",
            ));
        }
        Ok(self)
    }
}
struct Endpoint {
    directories: [File; 3],
    identities: [Identity; 3],
    socket: Identity,
    group: u32,
}
impl Endpoint {
    fn capture(group: u32) -> Result<Self> {
        let root = File::from(syscall(
            "pin intake root",
            fs::open("/", DIRECTORY_FLAGS, fs::Mode::empty()),
        )?);
        let run = File::from(syscall(
            "pin intake run",
            fs::openat(&root, "run", DIRECTORY_FLAGS, fs::Mode::empty()),
        )?);
        let directory = File::from(syscall(
            "pin intake directory",
            fs::openat(&run, "fe2o3", DIRECTORY_FLAGS, fs::Mode::empty()),
        )?);
        let directories = [root, run, directory];
        let mut identities = [Identity {
            device: 0,
            inode: 0,
            mode: 0,
            uid: 0,
            gid: 0,
        }; 3];
        for (i, file) in directories.iter().enumerate() {
            identities[i] = Identity::stat(syscall("check intake directory", fs::fstat(file))?)
                .directory(i == 2)?;
        }
        let socket = Self::socket(&directories[2], group)?;
        Ok(Self {
            directories,
            identities,
            socket,
            group,
        })
    }
    fn socket(directory: &File, group: u32) -> Result<Identity> {
        let identity = Identity::stat(syscall(
            "check intake socket binding",
            fs::statat(
                directory,
                "compiler-execution-supervisor.sock",
                fs::AtFlags::SYMLINK_NOFOLLOW,
            ),
        )?);
        if identity.uid != 0
            || identity.gid != group
            || identity.mode != libc::S_IFSOCK | SOCKET_MODE
        {
            return Err(Error::Rejected(
                "root intake socket ownership or mode changed",
            ));
        }
        Ok(identity)
    }
    fn revalidate(&self) -> Result<()> {
        for (i, file) in self.directories.iter().enumerate() {
            if Identity::stat(syscall(
                "recheck retained intake directory",
                fs::fstat(file),
            )?)
            .directory(i == 2)?
                != self.identities[i]
            {
                return Err(Error::Rejected("retained intake directory changed"));
            }
        }
        let current = Self::capture(self.group)?;
        if current.identities != self.identities
            || current.socket != self.socket
            || Self::socket(&self.directories[2], self.group)? != self.socket
        {
            return Err(Error::Rejected(
                "fixed intake pathname no longer names original objects",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "protected_compiler_root_intake_tests.rs"]
mod tests;
