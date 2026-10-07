//! Bounded startup mechanics only; no descriptor adoption or admitted authority.
//!
//! Constructors use the original ledger and return an UNRESERVED full owner
//! charge. Reserve it immediately, retain it for every borrowed operation, and
//! release it only after dropping the owner. Work constants include entry work;
//! scratch constants are additional peaks above the caller's live reservations.
//! These are logical envelopes, not bounds on generated stack frames or libc RSS.
//! Transient snapshots are zeroizing; clearenv does not scrub the original OS
//! environment/argv backing storage. No process-wide erasure is claimed.
use crate::native_inherited::{
    CompilerExecutionRootDeploymentErrorV2 as Failure, CompilerExecutionRootStorageV2 as Storage,
    Result, invalid,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use rustix::{
    fs::{Mode, OFlags},
    io::Errno,
    net::{AddressFamily, SendFlags, SocketAddrUnix, SocketFlags, SocketType, sendto, socket_with},
};
use std::{ffi::OsStr, marker::PhantomData, mem::size_of, os::unix::ffi::OsStrExt, rc::Rc};
use zeroize::Zeroizing;

// Byte caps include terminating NULs. An abstract notify value additionally has
// its leading '@'; the address payload itself is capped at NOTIFY_PATH_MAX_BYTES.
pub(crate) const ARGV0_MAX_BYTES: usize = 4096;
pub(crate) const ENV_MAX_ENTRIES: usize = 256;
pub(crate) const ENV_ENTRY_MAX_BYTES: usize = 4096;
pub(crate) const ENV_TOTAL_MAX_BYTES: usize = 64 * 1024;
pub(crate) const NOTIFY_PATH_MAX_BYTES: usize = 107;
pub(crate) const DESCRIPTOR_ROLES: [&[u8]; 14] = [
    b"runtime-root",
    b"supervisor-root",
    b"anchor-root",
    b"supervisor",
    b"launcher",
    b"issuer",
    b"anchor-helper",
    b"anchor-daemon",
    b"supervisor-deployment",
    b"issuer-policy",
    b"anchor-deployment",
    b"anchor-provisioning",
    b"issuer-key-seed",
    b"anchor-key-seed",
];
const ACTIVATION_KEYS: [&[u8]; 4] = [
    b"LISTEN_PID",
    b"LISTEN_FDS",
    b"LISTEN_FDNAMES",
    b"NOTIFY_SOCKET",
];
const READY: &[u8; 7] = b"READY=1";
const ENTRY_WORK: usize = 8;

pub(crate) const ACTIVATION_STORAGE: usize = size_of::<(Activation, Storage)>();
// Covers initialization, bounded pointer/byte observations, parsing, staging,
// and zeroization on success, error, and unwind, plus fixed syscall envelopes.
pub(crate) const CAPTURE_WORK: usize = ENTRY_WORK
    + 16 * 1024
    + 32 * (ENV_TOTAL_MAX_BYTES + ARGV0_MAX_BYTES + ENV_MAX_ENTRIES * size_of::<usize>());
pub(crate) const CAPTURE_SCRATCH: usize = 3 * size_of::<EnvironmentSnapshot>()
    + 3 * (ARGV0_MAX_BYTES + 1)
    + 4 * ACTIVATION_STORAGE
    + 4096;
pub(crate) const PUBLISH_WORK: usize = ENTRY_WORK + 4096;
pub(crate) const PUBLISH_SCRATCH: usize = 4 * size_of::<SocketAddrUnix>() + 4096;
pub(crate) const SIGNALS_STORAGE: usize = size_of::<(TerminationSignals, Storage)>();
pub(crate) const INSTALL_WORK: usize = ENTRY_WORK + 4096;
pub(crate) const INSTALL_SCRATCH: usize = 4 * SIGNALS_STORAGE + 4096;
pub(crate) const WAIT_WORK: usize = ENTRY_WORK + 4096;
pub(crate) const WAIT_SCRATCH: usize = 2 * size_of::<libc::sigset_t>() + 4096;
pub(crate) const RESTORE_WORK: usize = ENTRY_WORK + 4096;
pub(crate) const RESTORE_SCRATCH: usize = 2 * size_of::<libc::sigset_t>() + 4096;
pub(crate) const WAIT_INTERVAL_SECONDS: i64 = 1;

/// Validated activation text, not ownership of the fourteen inherited slots.
/// Neither cloneable nor transferable to another thread; publishing is one-shot.
#[must_use]
pub(crate) struct Activation {
    main_pid: i32,
    target: [u8; NOTIFY_PATH_MAX_BYTES],
    target_length: usize,
    abstract_namespace: bool,
    publish_attempted: bool,
    thread_affine: PhantomData<Rc<()>>,
}

impl Activation {
    /// Snapshot, validate, then clear the environment exactly once. No FD is adopted.
    /// On success return the complete unreserved `ACTIVATION_STORAGE` charge.
    /// A failure from clearenv has no rollback guarantee; terminate startup.
    ///
    /// # Safety
    /// Call only once in the dedicated, single-threaded main-thread startup path,
    /// before spawning threads or children and before any activation FD adoption.
    /// The caller exclusively owns process environment/argv mutation and signal
    /// handler interaction throughout this call. No Rust or foreign environment
    /// access, borrowed environment pointers, or concurrent argv rewriting may
    /// exist. `environ` must be a valid C environment: a readable null-terminated
    /// pointer array with readable NUL-terminated strings. Do not retry capture,
    /// even after failure; this is a unique activation caller contract, not a lock.
    #[allow(unsafe_code)]
    pub(crate) unsafe fn capture(b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        capture_with(
            b,
            || {
                // SAFETY: the caller supplies the exclusive startup environment contract.
                unsafe { probe_process() }
            },
            || {
                // SAFETY: validation completed under the same uninterrupted contract.
                unsafe { clear_environment() }
            },
        )
    }

    /// One nonblocking datagram attempt from the captured main process/thread.
    /// A work/storage denial before entry leaves the attempt available. Once the
    /// attempt begins, any refusal or syscall error consumes it (no retry).
    pub(crate) fn publish(&mut self, b: &mut Budget<'_>) -> Result<()> {
        publish_with(self, b, |plan| {
            check_main_thread(plan.main_pid)?;
            let bytes = &plan.target[..plan.target_length];
            let address = if plan.abstract_namespace {
                SocketAddrUnix::new_abstract_name(bytes)
            } else {
                SocketAddrUnix::new(OsStr::from_bytes(bytes))
            }
            .map_err(|e| io("construct readiness address", e))?;
            let socket = socket_with(
                AddressFamily::UNIX,
                SocketType::DGRAM,
                SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
                None,
            )
            .map_err(|e| io("open readiness socket", e))?;
            let sent = sendto(
                &socket,
                READY,
                SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
                &address,
            )
            .map_err(|e| io("publish readiness", e))?;
            exact_ready_count(sent)
        })
    }
}

/// Provisioner-only environment removal under the same exclusive C-environment
/// ownership contract as capture. It neither parses activation nor adopts FDs.
#[allow(unsafe_code)]
pub(crate) unsafe fn clear_for_provisioning(b: &mut Budget<'_>) -> Result<()> {
    b.with_prepaid_scope(0, ENTRY_WORK, CAPTURE_WORK, CAPTURE_SCRATCH, |_| {
        check_main_thread(rustix::process::getpid().as_raw_pid())?;
        unsafe extern "C" {
            static mut environ: *mut *mut libc::c_char;
        }
        // SAFETY: the dedicated provisioner exclusively owns a valid C environment.
        let _snapshot = unsafe { snapshot_environment(environ.cast()) }?;
        // SAFETY: the snapshot completed under the same uninterrupted contract.
        unsafe { clear_environment() }
    })
}

// The caller exclusively owns the valid C environment through this one attempt.
#[allow(unsafe_code)]
unsafe fn clear_environment() -> Result<()> {
    // SAFETY: both startup callers retain the exclusive environment contract.
    if unsafe { libc::clearenv() } != 0 {
        return Err(last_error("clear startup environment"));
    }
    Ok(())
}

fn capture_with(
    b: &mut Budget<'_>,
    probe: impl FnOnce() -> Result<Activation>,
    clear: impl FnOnce() -> Result<()>,
) -> Result<(Activation, Storage)> {
    b.with_prepaid_scope(0, ENTRY_WORK, CAPTURE_WORK, CAPTURE_SCRATCH, |_| {
        let plan = probe()?;
        clear()?;
        Ok((plan, Storage(ACTIVATION_STORAGE)))
    })
}

fn publish_with(
    plan: &mut Activation,
    b: &mut Budget<'_>,
    send: impl FnOnce(&Activation) -> Result<()>,
) -> Result<()> {
    b.with_prepaid_scope(
        ACTIVATION_STORAGE,
        ENTRY_WORK,
        PUBLISH_WORK,
        PUBLISH_SCRATCH,
        |_| {
            if plan.publish_attempted {
                return Err(invalid("activation", "readiness already attempted"));
            }
            plan.publish_attempted = true;
            send(plan)
        },
    )
}

fn exact_ready_count(sent: usize) -> Result<()> {
    if sent != READY.len() {
        return Err(invalid("activation", "readiness datagram length mismatch"));
    }
    Ok(())
}

#[allow(unsafe_code)]
unsafe fn probe_process() -> Result<Activation> {
    let main_pid = rustix::process::getpid().as_raw_pid();
    check_main_thread(main_pid)?;
    let cmdline = rustix::fs::open(
        "/proc/self/cmdline",
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|e| io("open activation command line", e))?;
    read_cmdline(|bytes, offset| rustix::io::pread(&cmdline, bytes, offset))?;
    unsafe extern "C" {
        static mut environ: *mut *mut libc::c_char;
    }
    // SAFETY: inherited from capture; no other environment user exists.
    let snapshot = unsafe { snapshot_environment(environ.cast()) }?;
    parse_environment(main_pid, &snapshot)
}

fn read_cmdline(mut read: impl FnMut(&mut [u8], u64) -> rustix::io::Result<usize>) -> Result<()> {
    let mut bytes = Zeroizing::new([0u8; ARGV0_MAX_BYTES + 1]);
    let length = read(&mut bytes[..], 0).map_err(|e| io("read activation command line", e))?;
    if length > ARGV0_MAX_BYTES {
        return Err(invalid("activation", "command line exceeds bound"));
    }
    validate_cmdline(&bytes[..length])?;
    let mut eof = Zeroizing::new([0u8; 1]);
    if read(&mut eof[..], length as u64).map_err(|e| io("probe command line EOF", e))? != 0 {
        return Err(invalid("activation", "command line has trailing bytes"));
    }
    Ok(())
}

fn validate_cmdline(bytes: &[u8]) -> Result<()> {
    if bytes.len() < 2
        || bytes.len() > ARGV0_MAX_BYTES
        || bytes.last() != Some(&0)
        || bytes[..bytes.len() - 1].contains(&0)
    {
        return Err(invalid("activation", "expected one bounded nonempty argv0"));
    }
    Ok(())
}

struct EnvironmentSnapshot {
    bytes: Zeroizing<[u8; ENV_TOTAL_MAX_BYTES]>,
    ends: [usize; ENV_MAX_ENTRIES],
    count: usize,
    used: usize,
}
impl EnvironmentSnapshot {
    fn new() -> Self {
        Self {
            bytes: Zeroizing::new([0; ENV_TOTAL_MAX_BYTES]),
            ends: [0; ENV_MAX_ENTRIES],
            count: 0,
            used: 0,
        }
    }
    fn entry(&self, index: usize) -> &[u8] {
        let start = if index == 0 {
            0
        } else {
            self.ends[index - 1] + 1
        };
        &self.bytes[start..self.ends[index]]
    }
}

/// The pointer array must be readable through its first null pointer or index
/// ENV_MAX_ENTRIES (inclusive). Each nonnull string must be readable through its
/// first NUL or ENV_ENTRY_MAX_BYTES bytes, whichever comes first. All memory must
/// remain stable until return. No pointer/slice operation precedes its cap check.
#[allow(unsafe_code)]
unsafe fn snapshot_environment(entries: *const *const libc::c_char) -> Result<EnvironmentSnapshot> {
    let mut snapshot = EnvironmentSnapshot::new();
    if entries.is_null() {
        return Ok(snapshot);
    }
    for index in 0..=ENV_MAX_ENTRIES {
        // SAFETY: index is capped before the pointer read; validity is the caller's contract.
        let entry = unsafe { entries.add(index).read() };
        if entry.is_null() {
            return Ok(snapshot);
        }
        if index == ENV_MAX_ENTRIES {
            return Err(invalid("activation", "too many environment entries"));
        }
        let mut terminated = false;
        for offset in 0..ENV_ENTRY_MAX_BYTES {
            if snapshot.used == ENV_TOTAL_MAX_BYTES {
                return Err(invalid(
                    "activation",
                    "environment exceeds total byte bound",
                ));
            }
            // SAFETY: both byte caps were checked; stop immediately at the first NUL.
            let byte = unsafe { entry.add(offset).read() } as u8;
            snapshot.bytes[snapshot.used] = byte;
            snapshot.used += 1;
            if byte == 0 {
                snapshot.ends[index] = snapshot.used - 1;
                snapshot.count += 1;
                terminated = true;
                break;
            }
        }
        if !terminated {
            return Err(invalid(
                "activation",
                "environment entry exceeds byte bound",
            ));
        }
    }
    Err(invalid("activation", "environment terminator missing"))
}

fn parse_environment(main_pid: i32, snapshot: &EnvironmentSnapshot) -> Result<Activation> {
    let mut values: [Option<&[u8]>; 4] = [None; 4];
    for index in 0..snapshot.count {
        let entry = snapshot.entry(index);
        let split = entry
            .iter()
            .position(|&byte| byte == b'=')
            .filter(|&offset| offset > 0)
            .ok_or_else(|| invalid("activation", "malformed environment entry"))?;
        let key = &entry[..split];
        if let Some(slot) = ACTIVATION_KEYS.iter().position(|&name| name == key)
            && values[slot].replace(&entry[split + 1..]).is_some()
        {
            return Err(invalid("activation", "duplicate activation variable"));
        }
    }
    let [Some(pid), Some(fds), Some(names), Some(notify)] = values else {
        return Err(invalid("activation", "missing activation variable"));
    };
    parse_activation(main_pid, pid, fds, names, notify)
}

fn parse_activation(
    main_pid: i32,
    pid: &[u8],
    fds: &[u8],
    names: &[u8],
    notify: &[u8],
) -> Result<Activation> {
    if main_pid <= 0 || parse_pid(pid)? != main_pid || fds != b"14" {
        return Err(invalid("activation", "PID or descriptor count mismatch"));
    }
    validate_roles(names)?;
    if notify.is_empty() || notify.len() > NOTIFY_PATH_MAX_BYTES + 1 || notify.contains(&0) {
        return Err(invalid("activation", "invalid readiness address"));
    }
    let (abstract_namespace, bytes) = match notify[0] {
        b'@' => (true, &notify[1..]),
        b'/' => (false, notify),
        _ => {
            return Err(invalid(
                "activation",
                "readiness address must be absolute or abstract",
            ));
        }
    };
    if bytes.is_empty() || bytes.len() > NOTIFY_PATH_MAX_BYTES {
        return Err(invalid("activation", "readiness address exceeds bound"));
    }
    let mut target = [0; NOTIFY_PATH_MAX_BYTES];
    target[..bytes.len()].copy_from_slice(bytes);
    Ok(Activation {
        main_pid,
        target,
        target_length: bytes.len(),
        abstract_namespace,
        publish_attempted: false,
        thread_affine: PhantomData,
    })
}

fn parse_pid(bytes: &[u8]) -> Result<i32> {
    if bytes.is_empty() || bytes.len() > 10 || bytes[0] == b'0' {
        return Err(invalid(
            "activation",
            "PID is not canonical positive decimal",
        ));
    }
    bytes.iter().try_fold(0i32, |pid, byte| {
        if !byte.is_ascii_digit() {
            return Err(invalid(
                "activation",
                "PID is not canonical positive decimal",
            ));
        }
        pid.checked_mul(10)
            .and_then(|pid| pid.checked_add(i32::from(byte - b'0')))
            .ok_or_else(|| invalid("activation", "PID exceeds range"))
    })
}

fn validate_roles(bytes: &[u8]) -> Result<()> {
    if bytes.len() > ENV_ENTRY_MAX_BYTES {
        return Err(invalid("activation", "descriptor names exceed bound"));
    }
    let mut roles = bytes.split(|&byte| byte == b':');
    for expected in DESCRIPTOR_ROLES {
        if roles.next() != Some(expected) {
            return Err(invalid("activation", "descriptor roles mismatch"));
        }
    }
    if roles.next().is_some() {
        return Err(invalid("activation", "extra descriptor role"));
    }
    Ok(())
}

/// Owns a thread's blocked SIGTERM/SIGINT mask, not process-wide signal policy.
/// Explicit restoration is metered. Drop deliberately does NOT restore: a failed
/// or denied restore leaves the mask blocked; retain this owner or terminate the
/// dedicated process. Restoring can deliver pending signals immediately. Never
/// unblock before managed-service cleanup completes. This type is !Send/!Sync.
#[must_use]
pub(crate) struct TerminationSignals {
    set: libc::sigset_t,
    previous: libc::sigset_t,
    main_pid: i32,
    active: bool,
    thread_affine: PhantomData<Rc<()>>,
}

impl TerminationSignals {
    /// Blocks both signals in one pthread_sigmask attempt, saving the old mask.
    /// Returns the unreserved full `SIGNALS_STORAGE` charge. No fallible operation
    /// follows successful installation inside the prepaid callback.
    ///
    /// # Safety
    /// This must be the unique mask owner on the dedicated single-threaded main
    /// startup thread. No other code/handler may change this thread's mask until
    /// restore or process exit. Later threads must inherit the blocked mask and
    /// must not compete for these signals. The caller handles return-reservation
    /// denial by retaining the owner for metered restoration or terminating the
    /// process; dropping it is not restoration. Forked children must not use it.
    #[allow(unsafe_code)]
    pub(crate) unsafe fn install(b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        install_with(b, || {
            let main_pid = rustix::process::getpid().as_raw_pid();
            check_main_thread(main_pid)?;
            // SAFETY: zero initializes all sigset_t storage, including libc padding.
            let mut set: libc::sigset_t = unsafe { std::mem::zeroed() };
            let mut previous: libc::sigset_t = unsafe { std::mem::zeroed() };
            // SAFETY: initialized writable sigsets, fixed valid signal numbers.
            if unsafe { libc::sigemptyset(&mut set) } != 0 {
                return Err(last_error("initialize termination signal set"));
            }
            if unsafe { libc::sigaddset(&mut set, libc::SIGTERM) } != 0 {
                return Err(last_error("add termination signal"));
            }
            if unsafe { libc::sigaddset(&mut set, libc::SIGINT) } != 0 {
                return Err(last_error("add interrupt signal"));
            }
            // SAFETY: the caller exclusively owns the current thread's signal mask.
            mask_status(
                unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &set, &mut previous) },
                "block termination signals",
            )?;
            Ok(Self {
                set,
                previous,
                main_pid,
                active: true,
                thread_affine: PhantomData,
            })
        })
    }

    /// A single one-second sigtimedwait; EINTR/EAGAIN are empty ticks, never retried.
    pub(crate) fn wait_interval(&self, b: &mut Budget<'_>) -> Result<Option<i32>> {
        self.wait_once(false, b)
    }

    /// The same single funded signal observation without delaying a runnable
    /// compiler checkpoint. The caller still bounds turns and its deadline.
    pub(crate) fn poll(&self, b: &mut Budget<'_>) -> Result<Option<i32>> {
        self.wait_once(true, b)
    }

    #[allow(unsafe_code)]
    fn wait_once(&self, nonblocking: bool, b: &mut Budget<'_>) -> Result<Option<i32>> {
        wait_with(self, b, |signals| {
            check_main_thread(signals.main_pid)?;
            let timeout = signal_wait_timeout(nonblocking);
            // SAFETY: the install contract preserves this thread's initialized blocked set.
            let signal =
                unsafe { libc::sigtimedwait(&signals.set, std::ptr::null_mut(), &timeout) };
            let errno = if signal < 0 { current_errno() } else { 0 };
            wait_result(signal, errno)
        })
    }

    /// One attempt to restore the exact prior mask. On denial/failure, stays active
    /// and may be restored in a later, separately metered call. On success, further
    /// wait/restore calls are refused; its storage remains live until dropped.
    #[allow(unsafe_code)]
    pub(crate) fn restore(&mut self, b: &mut Budget<'_>) -> Result<()> {
        restore_with(self, b, |signals| {
            check_main_thread(signals.main_pid)?;
            // SAFETY: same main thread, saved initialized mask, unique mask ownership.
            mask_status(
                unsafe {
                    libc::pthread_sigmask(
                        libc::SIG_SETMASK,
                        &signals.previous,
                        std::ptr::null_mut(),
                    )
                },
                "restore termination signal mask",
            )
        })
    }
}

fn signal_wait_timeout(nonblocking: bool) -> libc::timespec {
    libc::timespec {
        tv_sec: if nonblocking {
            0
        } else {
            WAIT_INTERVAL_SECONDS
        },
        tv_nsec: 0,
    }
}

fn install_with(
    b: &mut Budget<'_>,
    install: impl FnOnce() -> Result<TerminationSignals>,
) -> Result<(TerminationSignals, Storage)> {
    b.with_prepaid_scope(0, ENTRY_WORK, INSTALL_WORK, INSTALL_SCRATCH, |_| {
        Ok((install()?, Storage(SIGNALS_STORAGE)))
    })
}

fn wait_with(
    signals: &TerminationSignals,
    b: &mut Budget<'_>,
    wait: impl FnOnce(&TerminationSignals) -> Result<Option<i32>>,
) -> Result<Option<i32>> {
    b.with_prepaid_scope(SIGNALS_STORAGE, ENTRY_WORK, WAIT_WORK, WAIT_SCRATCH, |_| {
        require_active(signals)?;
        wait(signals)
    })
}

fn restore_with(
    signals: &mut TerminationSignals,
    b: &mut Budget<'_>,
    restore: impl FnOnce(&TerminationSignals) -> Result<()>,
) -> Result<()> {
    b.with_prepaid_scope(
        SIGNALS_STORAGE,
        ENTRY_WORK,
        RESTORE_WORK,
        RESTORE_SCRATCH,
        |_| {
            require_active(signals)?;
            restore(signals)?;
            signals.active = false;
            Ok(())
        },
    )
}

fn require_active(signals: &TerminationSignals) -> Result<()> {
    if !signals.active {
        return Err(invalid("activation", "termination mask already restored"));
    }
    Ok(())
}

fn wait_result(signal: i32, errno: i32) -> Result<Option<i32>> {
    if signal == libc::SIGTERM || signal == libc::SIGINT {
        return Ok(Some(signal));
    }
    if signal < 0 {
        if errno == libc::EAGAIN || errno == libc::EINTR {
            return Ok(None);
        }
        return Err(io(
            "wait for termination signal",
            Errno::from_raw_os_error(errno),
        ));
    }
    Err(invalid("activation", "unexpected termination wait result"))
}

fn mask_status(status: i32, operation: &'static str) -> Result<()> {
    if status != 0 {
        // pthread_sigmask returns an error number, not -1/errno.
        return Err(io(operation, Errno::from_raw_os_error(status)));
    }
    Ok(())
}

fn check_main_thread(main_pid: i32) -> Result<()> {
    check_main_identity(
        main_pid,
        rustix::process::getpid().as_raw_pid(),
        rustix::thread::gettid().as_raw_pid(),
    )
}

fn check_main_identity(main_pid: i32, current_pid: i32, current_tid: i32) -> Result<()> {
    if main_pid <= 0 || current_pid != main_pid || current_tid != main_pid {
        return Err(invalid(
            "activation",
            "not the captured main process and thread",
        ));
    }
    Ok(())
}
fn io(operation: &'static str, source: Errno) -> Failure {
    Failure::Io { operation, source }
}
fn current_errno() -> i32 {
    std::io::Error::last_os_error()
        .raw_os_error()
        .unwrap_or(libc::EIO)
}
fn last_error(operation: &'static str) -> Failure {
    io(operation, Errno::from_raw_os_error(current_errno()))
}

#[cfg(test)]
#[path = "native_activation_tests.rs"]
mod tests;
