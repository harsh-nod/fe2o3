use core::ffi::{c_char, c_int, c_long, c_void};
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

use fe2o3_protected_service_profile::{
    PROTECTED_SERVICE_SECUREBITS_V1, ProtectedServiceCredentialProfileV1,
};

use crate::native_spawn::compiler_arguments::CompilerArguments;
use crate::native_spawn::compiler_child_channel::COMPILER_SERVICE_FD;
use crate::{
    PROTECTED_SERVICE_GATE_RELEASE_V1, PROTECTED_SERVICE_PROFILE_READY_V1,
    PROTECTED_SERVICE_STAGED_DESCRIPTOR_FLOOR_V1, ProtectedServiceDescriptorBindingV1,
};

const CLONE_PIDFD: u64 = 0x0000_1000;
const CLONE_NEWUSER: u64 = 0x1000_0000;
const CLONE_CLEAR_SIGHAND: u64 = 0x0000_0001_0000_0000;
const CLONE_INTO_CGROUP: u64 = 0x0000_0002_0000_0000;
const CLOSE_RANGE_CLOEXEC: u32 = 1 << 2;
const SIGCHLD: u64 = 17;
const SIGKILL: c_int = 9;
const SIGSTOP: c_int = 19;
const KERNEL_SIGNAL_COUNT: c_int = 64;
const KERNEL_SIGSET_BYTES: usize = 8;
const SIG_SETMASK: c_int = 2;
const PR_SET_PDEATHSIG: c_int = 1;
const PR_GET_PDEATHSIG: c_int = 2;
const PR_SET_DUMPABLE: c_int = 4;
const PR_GET_DUMPABLE: c_int = 3;
const PR_CAPBSET_READ: c_int = 23;
const PR_CAPBSET_DROP: c_int = 24;
const PR_GET_SECUREBITS: c_int = 27;
const PR_SET_SECUREBITS: c_int = 28;
const PR_SET_NO_NEW_PRIVS: c_int = 38;
const PR_GET_NO_NEW_PRIVS: c_int = 39;
const PR_CAP_AMBIENT: c_int = 47;
const PR_CAP_AMBIENT_IS_SET: c_int = 1;
const PR_CAP_AMBIENT_CLEAR_ALL: c_int = 4;
const RLIMIT_CORE: c_int = 4;
const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;
const FAILURE_BASE: u8 = 0xc0;

#[path = "native_compiler_channel_syscall.rs"]
mod compiler_channel;
pub(crate) use compiler_channel::SCRATCH as COMPILER_CHANNEL_SCRATCH;

#[path = "native_compiler_personality.rs"]
mod compiler_personality;
pub(crate) use compiler_personality::WORK as COMPILER_PERSONALITY_WORK;

#[path = "native_compiler_restrictions.rs"]
mod compiler_restrictions;
pub(crate) use compiler_restrictions::{
    INSTRUCTIONS as COMPILER_RESTRICTION_INSTRUCTIONS, SCRATCH as COMPILER_RESTRICTION_SCRATCH,
};

#[path = "native_compiler_trace_filter.rs"]
mod compiler_trace_filter;
pub(crate) use compiler_trace_filter::{
    INSTRUCTIONS as COMPILER_TRACE_INSTRUCTIONS, SCRATCH as COMPILER_TRACE_SCRATCH,
};

#[path = "native_compiler_filesystem.rs"]
mod compiler_filesystem;
pub(crate) use compiler_filesystem::{
    OUTPUT_DESTINATION as COMPILER_OUTPUT_DESTINATION, SCRATCH as COMPILER_FILESYSTEM_SCRATCH,
    WORK as COMPILER_FILESYSTEM_WORK,
};

#[path = "native_namespace_restrictions.rs"]
mod namespace_restrictions;
pub(crate) use namespace_restrictions::{
    INSTRUCTIONS as NAMESPACE_RESTRICTION_INSTRUCTIONS, SCRATCH as NAMESPACE_RESTRICTION_SCRATCH,
};

pub(crate) fn has_exact_root_identity() -> bool {
    let mut uids = [u32::MAX; 3];
    let mut gids = [u32::MAX; 3];
    // SAFETY: pointers name writable scalars; sentinel setfsid calls perform readback only.
    unsafe {
        libc::syscall(
            libc::SYS_getresuid,
            &raw mut uids[0],
            &raw mut uids[1],
            &raw mut uids[2],
        ) == 0
            && libc::syscall(
                libc::SYS_getresgid,
                &raw mut gids[0],
                &raw mut gids[1],
                &raw mut gids[2],
            ) == 0
            && uids == [0; 3]
            && gids == [0; 3]
            && libc::syscall(libc::SYS_setfsuid, u32::MAX) == 0
            && libc::syscall(libc::SYS_setfsgid, u32::MAX) == 0
    }
}

#[repr(C)]
struct CloneArgsV1 {
    flags: u64,
    pidfd: u64,
    child_tid: u64,
    parent_tid: u64,
    exit_signal: u64,
    stack: u64,
    stack_size: u64,
    tls: u64,
    set_tid: u64,
    set_tid_size: u64,
    cgroup: u64,
}

#[repr(C)]
struct KernelSigactionV1 {
    handler: u64,
    flags: u64,
    restorer: u64,
    mask: u64,
}

#[repr(C)]
struct LinuxCapabilityHeaderV1 {
    version: u32,
    pid: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LinuxCapabilityDataV1 {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

#[repr(C)]
struct LinuxRlimit64V1 {
    current: u64,
    maximum: u64,
}

struct StagedBindingV1 {
    source: File,
    destination: RawFd,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum CompilerCheckpointMode {
    Basic,
    Traced,
}

pub(crate) struct StagedProtectedServiceExecV1 {
    executable: File,
    bindings: Vec<StagedBindingV1>,
    profile_ready_writer: OwnedFd,
    gate_reader: OwnedFd,
    exec_status_writer: OwnedFd,
    compiler: Option<(File, CompilerArguments)>,
    compiler_child_channel_transfer: Option<File>,
    compiler_checkpoints: CompilerCheckpointMode,
    compiler_output_confinement: bool,
}

impl StagedProtectedServiceExecV1 {
    pub(crate) fn requires_namespace_confinement(
        &self,
        mapping_gate: Option<(BorrowedFd<'_>, BorrowedFd<'_>)>,
    ) -> bool {
        // These are actual staging/mapping operations, not service-role claims.
        self.compiler.is_some() || mapping_gate.is_some()
    }

    pub(crate) fn new(
        executable: &File,
        bindings: &[ProtectedServiceDescriptorBindingV1<'_>],
        profile_ready_writer: BorrowedFd<'_>,
        gate_reader: BorrowedFd<'_>,
        exec_status_writer: BorrowedFd<'_>,
    ) -> io::Result<Self> {
        Self::new_with_bindings(
            executable,
            bindings.len(),
            bindings.iter().copied(),
            profile_ready_writer,
            gate_reader,
            exec_status_writer,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_compiler(
        executable: &File,
        standard_io: [Option<BorrowedFd<'_>>; 3],
        bindings: &[ProtectedServiceDescriptorBindingV1<'_>],
        profile_ready_writer: BorrowedFd<'_>,
        gate_reader: BorrowedFd<'_>,
        exec_status_writer: BorrowedFd<'_>,
        working_directory: BorrowedFd<'_>,
        arguments: CompilerArguments,
        child_channel_transfer: Option<BorrowedFd<'_>>,
    ) -> io::Result<Self> {
        let count = bindings.len() + standard_io.iter().flatten().count();
        let streams = standard_io
            .into_iter()
            .enumerate()
            .filter_map(|(destination, source)| {
                source.map(|source| ProtectedServiceDescriptorBindingV1 {
                    source,
                    destination: destination as RawFd,
                })
            });
        let mut staged = Self::new_with_bindings(
            executable,
            count,
            streams.chain(bindings.iter().copied()),
            profile_ready_writer,
            gate_reader,
            exec_status_writer,
        )?;
        let mut next = staged
            .exec_status_writer
            .as_raw_fd()
            .checked_add(1)
            .ok_or_else(|| io::Error::from_raw_os_error(libc::EOVERFLOW))?;
        let cwd = File::from(duplicate_above(working_directory, &mut next)?);
        staged.compiler = Some((cwd, arguments));
        if let Some(transfer) = child_channel_transfer {
            staged.compiler_child_channel_transfer =
                Some(File::from(duplicate_above(transfer, &mut next)?));
        }
        Ok(staged)
    }

    fn new_with_bindings<'a>(
        executable: &File,
        count: usize,
        bindings: impl Iterator<Item = ProtectedServiceDescriptorBindingV1<'a>>,
        profile_ready_writer: BorrowedFd<'_>,
        gate_reader: BorrowedFd<'_>,
        exec_status_writer: BorrowedFd<'_>,
    ) -> io::Result<Self> {
        let mut next = PROTECTED_SERVICE_STAGED_DESCRIPTOR_FLOOR_V1;
        let executable = File::from(duplicate_above(executable.as_fd(), &mut next)?);
        let mut staged_bindings = Vec::new();
        staged_bindings
            .try_reserve_exact(count)
            .map_err(|_| io::Error::from_raw_os_error(libc::ENOMEM))?;
        for binding in bindings {
            staged_bindings.push(StagedBindingV1 {
                source: File::from(duplicate_above(binding.source, &mut next)?),
                destination: binding.destination,
            });
        }
        Ok(Self {
            executable,
            bindings: staged_bindings,
            profile_ready_writer: duplicate_above(profile_ready_writer, &mut next)?,
            gate_reader: duplicate_above(gate_reader, &mut next)?,
            exec_status_writer: duplicate_above(exec_status_writer, &mut next)?,
            compiler: None,
            compiler_child_channel_transfer: None,
            compiler_checkpoints: CompilerCheckpointMode::Basic,
            compiler_output_confinement: false,
        })
    }

    pub(crate) fn compiler_arguments(&self) -> Option<&CompilerArguments> {
        self.compiler.as_ref().map(|(_, arguments)| arguments)
    }

    pub(crate) fn compiler_working_directory(&self) -> Option<&File> {
        self.compiler.as_ref().map(|(cwd, _)| cwd)
    }

    pub(crate) fn compiler_child_channel_transfer(&self) -> Option<&File> {
        self.compiler_child_channel_transfer.as_ref()
    }

    pub(crate) fn require_runtime_checkpoints(&mut self) -> bool {
        if self.compiler.is_none()
            || self.compiler_child_channel_transfer.is_none()
            || self.compiler_checkpoints != CompilerCheckpointMode::Basic
        {
            return false;
        }
        self.compiler_checkpoints = CompilerCheckpointMode::Traced;
        true
    }

    pub(crate) fn has_runtime_checkpoints(&self) -> bool {
        self.compiler_checkpoints == CompilerCheckpointMode::Traced
    }

    pub(crate) fn require_output_write_confinement(&mut self) -> bool {
        if !self.has_runtime_checkpoints()
            || self.compiler_output_confinement
            || self.binding(COMPILER_OUTPUT_DESTINATION).is_none()
        {
            return false;
        }
        self.compiler_output_confinement = true;
        true
    }

    pub(crate) const fn has_output_write_confinement(&self) -> bool {
        self.compiler_output_confinement
    }

    pub(crate) fn additional_child_work(&self) -> usize {
        let cwd = if self.compiler.is_some() {
            crate::native_work::COMPILER_CWD_WORK + crate::native_work::COMPILER_RESTRICTION_WORK
        } else {
            0
        };
        let channel = if self.compiler_child_channel_transfer.is_some() {
            crate::native_work::COMPILER_CHANNEL_WORK
        } else {
            0
        };
        let checkpoints = if self.has_runtime_checkpoints() {
            crate::native_work::COMPILER_TRACE_WORK
        } else {
            0
        };
        let filesystem = if self.has_output_write_confinement() {
            COMPILER_FILESYSTEM_WORK
        } else {
            0
        };
        cwd + channel + checkpoints + filesystem
    }

    pub(crate) fn descriptor_count(&self) -> usize {
        self.bindings.len() + usize::from(self.compiler_child_channel_transfer.is_some())
    }

    pub(crate) fn executable(&self) -> &File {
        &self.executable
    }

    pub(crate) fn binding(&self, destination: RawFd) -> Option<&File> {
        self.bindings
            .iter()
            .find(|b| b.destination == destination)
            .map(|b| &b.source)
    }

    pub(crate) const TABLE_STORAGE: usize = crate::MAX_PROTECTED_SERVICE_DESCRIPTOR_BINDINGS_V1
        * std::mem::size_of::<StagedBindingV1>();
}

fn duplicate_above(source: BorrowedFd<'_>, next: &mut RawFd) -> io::Result<OwnedFd> {
    let duplicate = rustix::io::fcntl_dupfd_cloexec(source, *next).map_err(io::Error::from)?;
    *next = duplicate
        .as_raw_fd()
        .checked_add(1)
        .ok_or_else(|| io::Error::from_raw_os_error(libc::EOVERFLOW))?;
    Ok(duplicate)
}

pub(crate) fn spawn(
    staged: &StagedProtectedServiceExecV1,
    credentials: ProtectedServiceCredentialProfileV1,
    cap_last_cap: u32,
    expected_parent: rustix::process::Pid,
) -> io::Result<RootOwnedProtectedServiceChildV1> {
    let (pid, pidfd, parent_mask) =
        clone_child(staged, credentials, cap_last_cap, expected_parent).map_err(io::Error::from)?;
    let child = RootOwnedProtectedServiceChildV1 { pid, pidfd };
    let pidfd = child.pidfd.as_ref();
    let Some(pidfd) = pidfd else {
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
        reap_pid(pid);
        parent_mask.restore().map_err(io::Error::from)?;
        return Err(io::Error::from_raw_os_error(libc::EBADFD));
    };
    parent_mask.restore().map_err(io::Error::from)?;
    let flags = rustix::io::fcntl_getfd(pidfd).map_err(io::Error::from)?;
    if !flags.contains(rustix::io::FdFlags::CLOEXEC) {
        return Err(io::Error::from_raw_os_error(libc::EPERM));
    }
    Ok(child)
}

// No fallible parent operations follow successful clone: the caller must adopt
// the complete result with its pre-clone cleanup slot and spawn lease immediately.
pub(crate) fn clone_child(
    staged: &StagedProtectedServiceExecV1,
    credentials: ProtectedServiceCredentialProfileV1,
    cap_last_cap: u32,
    expected_parent: rustix::process::Pid,
) -> rustix::io::Result<(
    rustix::process::Pid,
    Option<OwnedFd>,
    crate::clone_compat::ParentSignalMask,
)> {
    clone_child_with_cgroup(
        staged,
        credentials,
        cap_last_cap,
        expected_parent,
        None,
        None,
    )
}

// Placement alone grants no protected-service or proof-isolation admission.
// The caller retains this freshly created domain and adopts it with the child
// before any parent-side check can fail.
pub(crate) fn clone_child_with_cgroup(
    staged: &StagedProtectedServiceExecV1,
    credentials: ProtectedServiceCredentialProfileV1,
    cap_last_cap: u32,
    expected_parent: rustix::process::Pid,
    cgroup: Option<BorrowedFd<'_>>,
    mapping_gate: Option<(BorrowedFd<'_>, BorrowedFd<'_>)>,
) -> rustix::io::Result<(
    rustix::process::Pid,
    Option<OwnedFd>,
    crate::clone_compat::ParentSignalMask,
)> {
    if mapping_gate.is_some() && cgroup.is_none() {
        return Err(rustix::io::Errno::INVAL);
    }
    // Defense in depth before clone, including private constructor callers.
    if staged.compiler_child_channel_transfer.is_some()
        && (staged.compiler.is_none()
            || staged.descriptor_count() > crate::MAX_PROTECTED_SERVICE_DESCRIPTOR_BINDINGS_V1
            || staged
                .bindings
                .iter()
                .any(|b| b.destination == COMPILER_SERVICE_FD))
    {
        return Err(rustix::io::Errno::INVAL);
    }
    let mut pidfd_raw = -1_i32;
    let (result, parent_mask) = if cgroup.is_none() && mapping_gate.is_none() {
        // SAFETY: child_exec resets dispositions before unmasking and never returns.
        // The parent returns all custody, including its pending mask, to the adopter.
        unsafe { crate::clone_compat::clone_unmapped(&mut pidfd_raw) }?
    } else {
        let arguments = clone_arguments(&mut pidfd_raw, cgroup, mapping_gate.is_some());
        // SAFETY: placed/mapped launches retain the exact clone3-only ABI. There is
        // no legacy fallback that could lose atomic cgroup placement or mappings.
        let result = unsafe {
            libc::syscall(
                libc::SYS_clone3,
                &raw const arguments,
                std::mem::size_of::<CloneArgsV1>(),
            )
        };
        (result, crate::clone_compat::ParentSignalMask::unchanged())
    };
    if result < 0 {
        // SAFETY: failed direct syscall set this thread's errno.
        return Err(unsafe { rustix::io::Errno::from_raw_os_error(*libc::__errno_location()) });
    }
    if result == 0 {
        // SAFETY: this is the direct post-clone child; child_exec always execs or exits.
        unsafe {
            child_exec(
                staged,
                credentials,
                cap_last_cap,
                expected_parent.as_raw_pid(),
                mapping_gate,
            )
        }
    }
    let raw_pid = i32::try_from(result).unwrap_or_else(|_| std::process::abort());
    let pid = rustix::process::Pid::from_raw(raw_pid).unwrap_or_else(|| std::process::abort());
    let pidfd = if pidfd_raw < 0 {
        None
    } else {
        // SAFETY: successful CLONE_PIDFD installed one newly owned descriptor.
        Some(unsafe { OwnedFd::from_raw_fd(pidfd_raw) })
    };
    Ok((pid, pidfd, parent_mask))
}

fn clone_arguments(
    pidfd: &mut c_int,
    cgroup: Option<BorrowedFd<'_>>,
    fresh_user_namespace: bool,
) -> CloneArgsV1 {
    CloneArgsV1 {
        flags: CLONE_PIDFD
            | CLONE_CLEAR_SIGHAND
            | cgroup.map_or(0, |_| CLONE_INTO_CGROUP)
            | if fresh_user_namespace {
                CLONE_NEWUSER
            } else {
                0
            },
        pidfd: (pidfd as *mut c_int).addr() as u64,
        child_tid: 0,
        parent_tid: 0,
        exit_signal: SIGCHLD,
        stack: 0,
        stack_size: 0,
        tls: 0,
        set_tid: 0,
        set_tid_size: 0,
        cgroup: cgroup.map_or(0, |fd| fd.as_raw_fd() as u64),
    }
}

pub(crate) struct RootOwnedProtectedServiceChildV1 {
    pid: rustix::process::Pid,
    pidfd: Option<OwnedFd>,
}

impl RootOwnedProtectedServiceChildV1 {
    #[cfg(feature = "test-support")]
    pub(crate) fn admit_non_authoritative_test(
        pid: rustix::process::Pid,
        pidfd: OwnedFd,
    ) -> io::Result<Self> {
        if !rustix::io::fcntl_getfd(&pidfd)
            .map_err(io::Error::from)?
            .contains(rustix::io::FdFlags::CLOEXEC)
        {
            return Err(io::Error::from_raw_os_error(libc::EPERM));
        }
        Ok(Self {
            pid,
            pidfd: Some(pidfd),
        })
    }

    pub(crate) const fn pid(&self) -> rustix::process::Pid {
        self.pid
    }

    fn pidfd(&self) -> &OwnedFd {
        self.pidfd.as_ref().expect("live child retains pidfd")
    }

    pub(crate) fn is_live(&self) -> io::Result<bool> {
        match rustix::process::waitid(
            rustix::process::WaitId::PidFd(self.pidfd().as_fd()),
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOHANG
                | rustix::process::WaitIdOptions::NOWAIT,
        ) {
            Ok(None) | Err(rustix::io::Errno::INTR) => Ok(true),
            Ok(Some(_)) => Ok(false),
            Err(source) => Err(source.into()),
        }
    }

    pub(crate) fn try_clone_pidfd(&self) -> io::Result<OwnedFd> {
        rustix::io::fcntl_dupfd_cloexec(self.pidfd(), 0).map_err(io::Error::from)
    }

    pub(crate) fn exit_description(&self, fallback: &'static str) -> String {
        rustix::process::waitid(
            rustix::process::WaitId::PidFd(self.pidfd().as_fd()),
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOHANG
                | rustix::process::WaitIdOptions::NOWAIT,
        )
        .ok()
        .flatten()
        .map_or_else(|| fallback.to_owned(), |status| format!("{status:?}"))
    }

    pub(crate) fn cancel_and_reap(&mut self) -> Result<(), ReapErrorV1> {
        let Some(pidfd) = self.pidfd.as_ref() else {
            return Ok(());
        };
        let signal_error =
            match rustix::process::pidfd_send_signal(pidfd, rustix::process::Signal::KILL) {
                Ok(()) | Err(rustix::io::Errno::SRCH) => None,
                Err(pidfd_source) => {
                    match rustix::process::kill_process(self.pid, rustix::process::Signal::KILL) {
                        Ok(()) | Err(rustix::io::Errno::SRCH) => {
                            Some(io::Error::from(pidfd_source))
                        }
                        Err(_) => return Err(ReapErrorV1::Io(pidfd_source.into())),
                    }
                }
            };
        let wait_result = loop {
            match rustix::process::waitid(
                rustix::process::WaitId::PidFd(pidfd.as_fd()),
                rustix::process::WaitIdOptions::EXITED,
            ) {
                Ok(Some(_)) => break Ok(()),
                Ok(None) | Err(rustix::io::Errno::INTR) => {}
                Err(rustix::io::Errno::CHILD) => break Err(ReapErrorV1::OwnershipLost),
                Err(source) => break Err(ReapErrorV1::Io(source.into())),
            }
        };
        match wait_result {
            Ok(()) => {
                self.pidfd.take();
                signal_error.map_or(Ok(()), |source| Err(ReapErrorV1::Io(source)))
            }
            Err(ReapErrorV1::OwnershipLost) => {
                self.pidfd.take();
                Err(ReapErrorV1::OwnershipLost)
            }
            Err(error) => Err(error),
        }
    }
}

impl Drop for RootOwnedProtectedServiceChildV1 {
    fn drop(&mut self) {
        let _ = self.cancel_and_reap();
    }
}

pub(crate) enum ReapErrorV1 {
    OwnershipLost,
    Io(io::Error),
}

fn reap_pid(pid: rustix::process::Pid) {
    loop {
        match rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::empty()) {
            Ok(Some(_)) | Err(rustix::io::Errno::CHILD) => return,
            Ok(None) | Err(rustix::io::Errno::INTR) => {}
            Err(_) => return,
        }
    }
}

unsafe fn child_exec(
    staged: &StagedProtectedServiceExecV1,
    credentials: ProtectedServiceCredentialProfileV1,
    cap_last_cap: u32,
    expected_parent: i32,
    mapping_gate: Option<(BorrowedFd<'_>, BorrowedFd<'_>)>,
) -> ! {
    // SAFETY: every operation below is a direct scalar syscall over inherited storage.
    unsafe {
        if normalize_signal_state() != 0 {
            child_fail(staged.exec_status_writer.as_raw_fd(), 1);
        }
        let mut personality = None;
        let mut personality_acquisition_failed = false;
        if let Err(stage) = establish_guarded_profile(expected_parent, || {
            if let Some((reader, writer)) = mapping_gate {
                // The child must not keep its own release writer alive. This gate
                // precedes all ID changes and is already guarded by PDEATHSIG.
                if await_mapping_gate(reader.as_raw_fd(), writer.as_raw_fd()) != 0 {
                    return -1;
                }
            }
            if staged.compiler.is_some() {
                // The actual mapping gate has closed both child ends. Its
                // verified maps include root->root for UID/GID, so proc ownership
                // remains root-visible here. Profile setup closes no descriptors;
                // this private owner is consumed before READY/close_range/remaps.
                personality = compiler_personality::Observation::acquire();
                if personality.is_none() {
                    personality_acquisition_failed = true;
                    return -1;
                }
            }
            establish_profile(credentials, cap_last_cap)
        }) {
            if let Some(observation) = personality.take() {
                observation.abort();
            }
            child_fail(
                staged.exec_status_writer.as_raw_fd(),
                if personality_acquisition_failed {
                    15
                } else {
                    stage
                },
            );
        }
        // Socket SO_PEERCRED must capture the final child identity, not root.
        let compiler_client = if let Some(transfer) = &staged.compiler_child_channel_transfer {
            let client =
                compiler_channel::create_and_transfer(transfer.as_raw_fd(), expected_parent);
            if client < 0 {
                if let Some(observation) = personality.take() {
                    observation.abort();
                }
                child_fail(staged.exec_status_writer.as_raw_fd(), 11);
            }
            client
        } else {
            -1
        };
        // Only the closed compiler stage opts in. Install after child-channel
        // setup, before any READY or user instruction. Installation failure uses
        // the existing owned status/terminal cleanup path, never a weak fallback.
        if staged.compiler.is_some() {
            let installed = match personality.take() {
                Some(observation) => compiler_restrictions::install(observation),
                None => Err(compiler_restrictions::InstallFailure::PersonalityObservation),
            };
            if let Err(error) = installed {
                let stage = match error {
                    compiler_restrictions::InstallFailure::Filter => 13,
                    compiler_restrictions::InstallFailure::PersonalityObservation => 16,
                };
                child_fail(staged.exec_status_writer.as_raw_fd(), stage);
            }
        }
        // Compiler stages and actually mapped children must be confined after
        // mappings/profile/channel setup. Unmapped generic service stages keep
        // their legacy creator behavior, including direct clone3. This branch
        // does not authenticate a role or relax the caller's deployment duties.
        if staged.requires_namespace_confinement(mapping_gate) && !namespace_restrictions::install()
        {
            child_fail(staged.exec_status_writer.as_raw_fd(), 14);
        }
        if staged.has_output_write_confinement() {
            let output = match staged.binding(COMPILER_OUTPUT_DESTINATION) {
                Some(output) => output.as_raw_fd(),
                None => child_fail(staged.exec_status_writer.as_raw_fd(), 18),
            };
            if !compiler_filesystem::install(output) {
                child_fail(staged.exec_status_writer.as_raw_fd(), 18);
            }
        }
        let ready = PROTECTED_SERVICE_PROFILE_READY_V1;
        if libc::syscall(
            libc::SYS_write,
            staged.profile_ready_writer.as_raw_fd(),
            (&raw const ready).cast::<c_void>(),
            1_usize,
        ) != 1
        {
            child_fail(staged.exec_status_writer.as_raw_fd(), 4);
        }
        let release = match read_gate(staged.gate_reader.as_raw_fd()) {
            Ok(release) => release,
            Err(()) => child_fail(staged.exec_status_writer.as_raw_fd(), 5),
        };
        if release != PROTECTED_SERVICE_GATE_RELEASE_V1 {
            child_fail(staged.exec_status_writer.as_raw_fd(), 6);
        }
        // The original trace is armed before this exact gate is released. Keep
        // pre-READY filters unchanged; no userspace compiler instruction has run.
        if staged.has_runtime_checkpoints() && !compiler_trace_filter::install() {
            child_fail(staged.exec_status_writer.as_raw_fd(), 17);
        }
        if libc::syscall(libc::SYS_close_range, 3_u32, u32::MAX, CLOSE_RANGE_CLOEXEC) != 0 {
            child_fail(staged.exec_status_writer.as_raw_fd(), 7);
        }
        // Sources are already staged above the destination range. Close all
        // inherited streams first; compiler bindings reinstall only captured ones.
        libc::syscall(libc::SYS_close, 0);
        libc::syscall(libc::SYS_close, 1);
        libc::syscall(libc::SYS_close, 2);
        for binding in &staged.bindings {
            if libc::syscall(
                libc::SYS_dup3,
                binding.source.as_raw_fd(),
                binding.destination,
                0,
            ) != c_long::from(binding.destination)
            {
                child_fail(staged.exec_status_writer.as_raw_fd(), 8);
            }
        }
        if compiler_client >= 0
            && (libc::syscall(libc::SYS_dup3, compiler_client, COMPILER_SERVICE_FD, 0)
                != c_long::from(COMPILER_SERVICE_FD)
                || libc::syscall(libc::SYS_close, compiler_client) != 0)
        {
            child_fail(staged.exec_status_writer.as_raw_fd(), 12);
        }
        let name = c"fe2o3-protected-service";
        let arguments = [name.as_ptr().cast_mut(), std::ptr::null_mut()];
        let environment = [std::ptr::null_mut::<c_char>()];
        let (argv, envp) = if let Some((cwd, compiler)) = &staged.compiler {
            if libc::syscall(libc::SYS_fchdir, cwd.as_raw_fd()) != 0 {
                child_fail(staged.exec_status_writer.as_raw_fd(), 10);
            }
            (compiler.argv(), compiler.envp())
        } else {
            (
                arguments.as_ptr().cast::<usize>(),
                environment.as_ptr().cast::<usize>(),
            )
        };
        libc::syscall(
            libc::SYS_execveat,
            staged.executable.as_raw_fd(),
            c"".as_ptr(),
            argv,
            envp,
            libc::AT_EMPTY_PATH,
        );
        child_fail(staged.exec_status_writer.as_raw_fd(), 9)
    }
}

// No allocation, unwinding, or unbounded interrupted-syscall retry after clone.
unsafe fn read_gate(fd: RawFd) -> Result<u8, ()> {
    crate::pre_exec::read_child_gate(|release| {
        // SAFETY: one writable byte lives through the direct read syscall.
        unsafe {
            let count = libc::syscall(
                libc::SYS_read,
                fd,
                (release as *mut u8).cast::<c_void>(),
                1_usize,
            );
            if count < 0 {
                Err(rustix::io::Errno::from_raw_os_error(
                    *libc::__errno_location(),
                ))
            } else {
                Ok(count as usize)
            }
        }
    })
}

unsafe fn await_mapping_gate(reader: RawFd, writer: RawFd) -> c_int {
    // SAFETY: the private pipe ends are consumed before descriptor installation.
    // Use raw syscalls: libc cancellation points must not run after raw clone.
    unsafe {
        if libc::syscall(libc::SYS_close, writer) != 0
            || read_gate(reader) != Ok(PROTECTED_SERVICE_GATE_RELEASE_V1)
            || libc::syscall(libc::SYS_close, reader) != 0
        {
            -1
        } else {
            0
        }
    }
}

unsafe fn normalize_signal_state() -> c_int {
    let action = KernelSigactionV1 {
        handler: 0,
        flags: 0,
        restorer: 0,
        mask: 0,
    };
    for signal in 1..=KERNEL_SIGNAL_COUNT {
        if signal == SIGKILL || signal == SIGSTOP {
            continue;
        }
        // SAFETY: x86-64 rt_sigaction consumes this exact kernel layout and sigset width.
        if unsafe {
            libc::syscall(
                libc::SYS_rt_sigaction,
                signal,
                &raw const action,
                std::ptr::null_mut::<KernelSigactionV1>(),
                KERNEL_SIGSET_BYTES,
            )
        } != 0
        {
            return -1;
        }
    }
    let empty = 0_u64;
    // SAFETY: the x86-64 kernel sigset is one u64.
    if unsafe {
        libc::syscall(
            libc::SYS_rt_sigprocmask,
            SIG_SETMASK,
            &raw const empty,
            std::ptr::null_mut::<u64>(),
            KERNEL_SIGSET_BYTES,
        )
    } != 0
    {
        return -1;
    }
    0
}

unsafe fn arm_parent_death(expected_parent: i32) -> c_int {
    let mut observed = 0_i32;
    // SAFETY: scalar getppid/prctl operations run in the direct child.
    if unsafe { libc::syscall(libc::SYS_getppid) } != c_long::from(expected_parent)
        || unsafe { libc::prctl(PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0) } != 0
        || unsafe { libc::syscall(libc::SYS_getppid) } != c_long::from(expected_parent)
        || unsafe { libc::prctl(PR_GET_PDEATHSIG, &raw mut observed, 0, 0, 0) } != 0
        || observed != SIGKILL
    {
        return -1;
    }
    0
}

unsafe fn establish_guarded_profile(
    expected_parent: i32,
    establish: impl FnOnce() -> c_int,
) -> Result<(), u8> {
    // SAFETY: scalar post-clone checks; the private production callback uses direct syscalls.
    if unsafe { arm_parent_death(expected_parent) } != 0 {
        return Err(2);
    }
    if establish() != 0 {
        return Err(3);
    }
    // Changing effective/filesystem IDs clears PDEATHSIG. Re-arm and check the
    // exact parent again before readiness; the old pre-transition guard is not enough.
    if unsafe { arm_parent_death(expected_parent) } != 0 {
        return Err(2);
    }
    Ok(())
}

unsafe fn establish_profile(
    credentials: ProtectedServiceCredentialProfileV1,
    cap_last_cap: u32,
) -> c_int {
    let core = LinuxRlimit64V1 {
        current: 0,
        maximum: 0,
    };
    // SAFETY: fixed scalar arguments and local Linux ABI records are supplied throughout.
    if unsafe { libc::syscall(libc::SYS_umask, 0o077_u32) } < 0
        || unsafe {
            libc::syscall(
                libc::SYS_prlimit64,
                0,
                RLIMIT_CORE,
                &raw const core,
                std::ptr::null_mut::<LinuxRlimit64V1>(),
            )
        } != 0
        || unsafe { libc::prctl(PR_SET_SECUREBITS, PROTECTED_SERVICE_SECUREBITS_V1, 0, 0, 0) } != 0
        || unsafe { libc::prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_CLEAR_ALL, 0, 0, 0) } != 0
    {
        return -1;
    }
    for capability in 0..=cap_last_cap {
        if unsafe { libc::prctl(PR_CAPBSET_DROP, capability, 0, 0, 0) } != 0 {
            return -1;
        }
    }
    if unsafe { libc::syscall(libc::SYS_setgroups, 0_usize, std::ptr::null::<u32>()) } != 0
        || unsafe {
            libc::syscall(
                libc::SYS_setresgid,
                credentials.gid(),
                credentials.gid(),
                credentials.gid(),
            )
        } != 0
        || unsafe {
            libc::syscall(
                libc::SYS_setresuid,
                credentials.uid(),
                credentials.uid(),
                credentials.uid(),
            )
        } != 0
    {
        return -1;
    }
    let mut header = LinuxCapabilityHeaderV1 {
        version: LINUX_CAPABILITY_VERSION_3,
        pid: 0,
    };
    let empty = [LinuxCapabilityDataV1 {
        effective: 0,
        permitted: 0,
        inheritable: 0,
    }; 2];
    if unsafe { libc::syscall(libc::SYS_capset, &raw mut header, empty.as_ptr()) } != 0
        || unsafe { libc::prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        || unsafe { libc::prctl(PR_SET_DUMPABLE, 0, 0, 0, 0) } != 0
    {
        return -1;
    }
    // SAFETY: direct readback validates complete child-local profile state.
    unsafe { validate_profile(credentials, cap_last_cap) }
}

unsafe fn validate_profile(
    credentials: ProtectedServiceCredentialProfileV1,
    cap_last_cap: u32,
) -> c_int {
    let mut uids = [u32::MAX; 3];
    let mut gids = [u32::MAX; 3];
    let mut header = LinuxCapabilityHeaderV1 {
        version: LINUX_CAPABILITY_VERSION_3,
        pid: 0,
    };
    let mut data = [LinuxCapabilityDataV1 {
        effective: u32::MAX,
        permitted: u32::MAX,
        inheritable: u32::MAX,
    }; 2];
    let mut core = LinuxRlimit64V1 {
        current: u64::MAX,
        maximum: u64::MAX,
    };
    // SAFETY: pointers identify writable direct-child stack storage.
    if unsafe {
        libc::syscall(
            libc::SYS_getresuid,
            &raw mut uids[0],
            &raw mut uids[1],
            &raw mut uids[2],
        )
    } != 0
        || uids != [credentials.uid(); 3]
        || unsafe {
            libc::syscall(
                libc::SYS_getresgid,
                &raw mut gids[0],
                &raw mut gids[1],
                &raw mut gids[2],
            )
        } != 0
        || gids != [credentials.gid(); 3]
        || unsafe { libc::syscall(libc::SYS_setfsuid, u32::MAX) } != c_long::from(credentials.uid())
        || unsafe { libc::syscall(libc::SYS_setfsgid, u32::MAX) } != c_long::from(credentials.gid())
        || unsafe { libc::syscall(libc::SYS_getgroups, 0_usize, std::ptr::null_mut::<u32>()) } != 0
        || unsafe { libc::syscall(libc::SYS_capget, &raw mut header, data.as_mut_ptr()) } != 0
        || data
            .iter()
            .any(|value| value.effective != 0 || value.permitted != 0 || value.inheritable != 0)
    {
        return -1;
    }
    for capability in 0..=cap_last_cap {
        if unsafe { libc::prctl(PR_CAPBSET_READ, capability, 0, 0, 0) } != 0
            || unsafe { libc::prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_IS_SET, capability, 0, 0) } != 0
        {
            return -1;
        }
    }
    if unsafe { libc::prctl(PR_GET_SECUREBITS, 0, 0, 0, 0) }
        != PROTECTED_SERVICE_SECUREBITS_V1 as c_int
        || unsafe { libc::prctl(PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) } != 1
        || unsafe { libc::prctl(PR_GET_DUMPABLE, 0, 0, 0, 0) } != 0
        || unsafe {
            libc::syscall(
                libc::SYS_prlimit64,
                0,
                RLIMIT_CORE,
                std::ptr::null::<LinuxRlimit64V1>(),
                &raw mut core,
            )
        } != 0
        || core.current != 0
        || core.maximum != 0
        || unsafe { libc::syscall(libc::SYS_umask, 0o077_u32) } != 0o077
    {
        return -1;
    }
    0
}

unsafe fn child_fail(exec_status: RawFd, stage: u8) -> ! {
    let message = FAILURE_BASE.saturating_add(stage);
    // SAFETY: exec_status is the staged seqpacket and message names one live byte.
    unsafe {
        let _ = libc::syscall(
            libc::SYS_sendto,
            exec_status,
            (&raw const message).cast::<c_void>(),
            1_usize,
            libc::MSG_NOSIGNAL,
            std::ptr::null::<libc::sockaddr>(),
            0_usize,
        );
        libc::_exit(126)
    }
}

#[cfg(test)]
#[path = "profile_transition_tests.rs"]
mod profile_tests;
