//! Root-task mechanics only. Descendant discovery and proof enforcement remain separate.

use super::{Budget, Child, ENTRY, Error, Pid, Poll, Result, RootOwnedProtectedServiceChildV2, io};
use crate::native_spawn::ProtectedServiceSpawnStorageV2 as Storage;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use rustix::{io::Errno, process::WaitIdStatus};
use std::{fmt, marker::PhantomData, mem::size_of, rc::Rc, thread::ThreadId};

#[path = "native_root_observation.rs"]
mod observation;
pub use observation::RootTaskObservationV2;

// One word of private payload plus Rc's strong/weak counters. This is logical
// allocation storage, not allocator metadata or RSS. No child backing is shared.
struct IdentityAllocation {
    _private: usize,
}
pub(super) const IDENTITY_ALLOCATION_STORAGE: usize =
    size_of::<IdentityAllocation>() + 2 * size_of::<usize>();

/// Move-only, inert identity of one original trace allocation, not its address,
/// PID, liveness, wait custody or authority. Only a live scoped root observation
/// can retain a handle. There is no public constructor or unmetered clone.
///
/// A handle may outlive its trace solely to prevent allocation-identity reuse;
/// it retains no child, descriptor, artifact lease or cleanup slot. Its FULL
/// handle-plus-allocation charge stays reserved on the original account until
/// Drop, even when another handle or the trace also pays for that allocation.
/// Keep that Work borrow and Budget alive at the admitting address throughout
/// retention. Comparing handles does not check this accounting or live custody.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootTaskIdentityV2;
/// let identity = RootTaskIdentityV2 { allocation: std::rc::Rc::new(()) };
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootTaskIdentityV2;
/// let identity = RootTaskIdentityV2::default();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootTaskIdentityV2;
/// fn clone<T: Clone>() {} clone::<RootTaskIdentityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootTaskIdentityV2;
/// fn copy<T: Copy>() {} copy::<RootTaskIdentityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootTaskIdentityV2;
/// fn send<T: Send>() {} send::<RootTaskIdentityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootTaskIdentityV2;
/// fn sync<T: Sync>() {} sync::<RootTaskIdentityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootTaskIdentityV2;
/// fn fd<T: std::os::fd::AsFd>() {} fd::<RootTaskIdentityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootTaskIdentityV2;
/// fn raw<T: std::os::fd::FromRawFd>() {} raw::<RootTaskIdentityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootTaskIdentityV2;
/// fn hash<T: std::hash::Hash>() {} hash::<RootTaskIdentityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootTaskIdentityV2;
/// let identity: RootTaskIdentityV2 = 1_usize.into();
/// ```
pub struct RootTaskIdentityV2 {
    allocation: Rc<IdentityAllocation>,
}

impl RootTaskIdentityV2 {
    const STORAGE: usize = size_of::<(Self, Storage)>() + IDENTITY_ALLOCATION_STORAGE;

    /// Inert allocation equality only, with no liveness or custody implication.
    /// Neither reference count is changed and no PID/address is exported.
    pub fn matches(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.allocation, &other.allocation)
    }

    /// FULL retained handle, output-charge metadata and shared allocation charge.
    pub const fn retained_storage(&self) -> usize {
        Self::STORAGE
    }
}

// No TRACEEXIT: cancellation must never need an originating-thread ptrace
// resume after the record has moved to the shared terminal cleanup pool.
const OPTIONS: usize = (libc::PTRACE_O_TRACEEXEC | libc::PTRACE_O_EXITKILL) as usize;

/// Closed observation from this controller; it is never accepted as wait authority.
/// There is no public constructor or conversion from a PID, status or scalar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootTaskTraceEventV2 {
    state: TraceState,
}

impl RootTaskTraceEventV2 {
    /// One nonblocking wait found no new status and no held stop.
    pub fn is_pending(self) -> bool {
        self.state == TraceState::Pending
    }

    /// The root reached an exec stop, without admitting its image or descendants.
    pub fn is_exec(self) -> bool {
        self.state == TraceState::Exec
    }

    /// An observed signal-delivery stop; resume re-delivers this signal.
    pub fn signal_stop(self) -> Option<i32> {
        if let TraceState::SignalStop(signal) = self.state {
            Some(signal)
        } else {
            None
        }
    }

    /// A job-control stop; resume uses LISTEN to preserve that stop.
    pub fn group_stop(self) -> Option<i32> {
        if let TraceState::GroupStop(signal) = self.state {
            Some(signal)
        } else {
            None
        }
    }

    /// A seized task stopped after interruption or group-stop wakeup.
    pub fn is_trap_stop(self) -> bool {
        self.state == TraceState::TrapStop
    }

    /// The root's consumed ordinary exit code, after cleanup notification.
    pub fn exit_code(self) -> Option<i32> {
        if let TraceState::Exited(code) = self.state {
            Some(code)
        } else {
            None
        }
    }

    /// The root's consumed fatal signal, after cleanup notification.
    pub fn terminating_signal(self) -> Option<i32> {
        if let TraceState::Signaled { signal, .. } = self.state {
            Some(signal)
        } else {
            None
        }
    }

    /// Whether the consumed terminal status reported a core dump.
    pub fn core_dumped(self) -> bool {
        matches!(
            self.state,
            TraceState::Signaled {
                core_dumped: true,
                ..
            }
        )
    }

    /// A terminal root status was consumed; domain/descendant cleanup is separate.
    pub fn is_terminal(self) -> bool {
        self.state.is_terminal()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TraceState {
    Pending,
    Refused,
    Exec,
    SignalStop(i32),
    GroupStop(i32),
    TrapStop,
    Exited(i32),
    Signaled { signal: i32, core_dumped: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TraceRequest {
    Seize,
    Continue,
    Listen,
}

impl TraceState {
    fn is_terminal(self) -> bool {
        matches!(self, Self::Exited(_) | Self::Signaled { .. })
    }

    fn restart(self) -> Option<(TraceRequest, usize)> {
        match self {
            Self::Exec | Self::TrapStop => Some((TraceRequest::Continue, 0)),
            Self::SignalStop(signal) => Some((TraceRequest::Continue, signal as usize)),
            Self::GroupStop(_) => Some((TraceRequest::Listen, 0)),
            _ => None,
        }
    }
}

/// Move-only root trace owning the original child, artifact lease and cleanup slot.
/// Each operation receives the original ledger and performs bounded work without
/// sleeps or retries. This is !Send/!Sync: foreground ptrace and waits stay on the
/// originating thread. The terminal cleanup pool may wait after pidfd SIGKILL.
///
/// Only root exec and signal/group stops are traced. No child birth, syscall,
/// mapping, interpreter, descendant or proof admission is established. In
/// particular, exec observation does not release the inherited artifact lease.
/// The originating Work borrow stays live for this owner's lifetime; a different
/// Work ledger or Budget address refuses even with identical storage/work quota.
/// Keep the original Budget live at its admitting address until this owner drops.
/// Address equality is an in-borrow accounting check, not persistent identity.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::native_spawn::RootOwnedProtectedServiceChildV2 as Child;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn send<T: Send>(_: T) {}
/// fn move_trace(child: Child, b: &mut Budget<'_>) { send(child.into_root_trace(b).unwrap()); }
/// ```
pub struct RootTaskTraceV2<'work> {
    child: RootOwnedProtectedServiceChildV2,
    identity: Rc<IdentityAllocation>,
    retained: usize,
    origin: (Pid, Pid, ThreadId),
    ledger: Ledger,
    budget_address: usize,
    held: TraceState,
    local: PhantomData<(&'work Budget<'work>, Rc<()>)>,
}

impl<'work> RootTaskTraceV2<'work> {
    /// Fixed allowance for one poll/resume or pre-seize observation plus SEIZE.
    pub const OPERATION_WORK: usize = ENTRY + 8 * (1024 + 64);
    /// Fixed wait/ptrace/controller staging, excluding the complete retained owner.
    pub const OPERATION_SCRATCH: usize = RootOwnedProtectedServiceChildV2::OPERATION_SCRATCH
        + 4 * std::mem::size_of::<Self>()
        + 4 * std::mem::size_of::<WaitIdStatus>();
    /// Full confirmation work, including the nested native-child lease release.
    pub const CONFIRM_EXEC_WORK: usize =
        Self::OPERATION_WORK + RootOwnedProtectedServiceChildV2::OPERATION_WORK;
    /// Full overlapping confirmation scratch, above the retained trace charge.
    pub const CONFIRM_EXEC_SCRATCH: usize =
        Self::OPERATION_SCRATCH + RootOwnedProtectedServiceChildV2::OPERATION_SCRATCH;

    pub(in crate::native_spawn) fn begin(
        child: RootOwnedProtectedServiceChildV2,
        retained: usize,
        b: &mut Budget<'work>,
    ) -> Result<Self> {
        b.with_prepaid_scope(
            retained,
            ENTRY,
            Self::OPERATION_WORK,
            Self::OPERATION_SCRATCH,
            |b| {
                child.check_pidfd()?;
                child.record()?.prepare_root_trace()?;
                let origin = origin();
                // ROOT_TRACE_GROWTH prepaid this allocation on the original Budget.
                let identity = Rc::new(IdentityAllocation { _private: 0 });
                ptrace(TraceRequest::Seize, child.pid(), OPTIONS)?;
                Ok(Self {
                    child,
                    identity,
                    retained,
                    origin,
                    ledger: b.work_ledger_identity_v1(),
                    budget_address: b as *const Budget<'_> as usize,
                    held: TraceState::Pending,
                    local: PhantomData,
                })
            },
        )
    }

    /// Full original owner/backing plus prepaid trace growth.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// Scalar identity of the owned root, not separate wait or signal authority.
    pub fn pid(&self) -> Pid {
        self.child.pid()
    }

    fn check_thread(&self) -> Result<()> {
        if self.origin != origin() {
            Err(Error::State("root trace left its originating thread"))
        } else {
            Ok(())
        }
    }

    pub(in crate::native_spawn) fn check_budget(
        &self,
        b: &Budget<'_>,
    ) -> std::result::Result<(), Resource> {
        if self.ledger != b.work_ledger_identity_v1()
            || self.budget_address != b as *const Budget<'_> as usize
        {
            Err(Resource::Accounting)
        } else {
            Ok(())
        }
    }

    fn record_mut(&mut self) -> Result<&mut Child> {
        self.child
            .custody
            .0
            .as_mut()
            .map(|(child, _)| child)
            .ok_or(Error::State("root trace custody was retired or deferred"))
    }

    /// Consumes at most one exact-pidfd status, never a sibling/helper status.
    /// Terminal cleanup is notified before this returns. A held stop or terminal
    /// observation is repeatable; only resume clears a held nonterminal stop.
    /// ECHILD records ownership loss; EINTR refuses without retrying.
    pub fn poll(&mut self, b: &mut Budget<'_>) -> Result<RootTaskTraceEventV2> {
        b.with_prepaid_scope(
            self.retained,
            ENTRY,
            Self::OPERATION_WORK,
            Self::OPERATION_SCRATCH,
            |b| {
                self.check_budget(b)?;
                self.check_thread()?;
                self.child.record()?;
                if self.held == TraceState::Refused {
                    return Err(Error::State("root trace retained an unsupported status"));
                }
                if self.held.is_terminal() {
                    return Ok(RootTaskTraceEventV2 { state: self.held });
                }
                if let Some(status) = self.record_mut()?.wait_root_trace()? {
                    self.held = TraceState::Refused;
                    self.held = decode(status)?;
                }
                Ok(RootTaskTraceEventV2 { state: self.held })
            },
        )
    }

    /// Resumes only an internally retained observed stop. No caller signal,
    /// status, PID, ptrace options or callback can authorize this transition.
    /// Group stops use LISTEN; signal stops re-deliver their observed signal.
    pub fn resume(&mut self, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(
            self.retained,
            ENTRY,
            Self::OPERATION_WORK,
            Self::OPERATION_SCRATCH,
            |b| {
                self.check_budget(b)?;
                self.check_thread()?;
                self.child.record()?.prepare_root_trace()?;
                let (request, data) = self
                    .held
                    .restart()
                    .ok_or(Error::State("root trace has no resumable observed stop"))?;
                ptrace(request, self.pid(), data)?;
                self.held = TraceState::Pending;
                Ok(())
            },
        )
    }

    /// Releases only the spawn lease while this controller still holds an exec
    /// stop observed by its own consuming wait. Resuming, retiring, deferring or
    /// losing custody refuses confirmation. The stop, slot and backing remain.
    /// CONFIRM_EXEC_WORK/SCRATCH cover both this check and the child operation.
    ///
    /// # Safety
    /// Authenticate the exact child's successful exec under the native launch
    /// protocol and establish closure of ALL inherited artifact-lock aliases,
    /// including any held by untraced descendants. A root exec event alone does
    /// not authenticate the executable or exclude those aliases. Preserve
    /// exclusive consuming-wait ownership. Refusal leaves the lease retained.
    pub unsafe fn confirm_exec(&mut self, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(
            self.retained,
            ENTRY,
            Self::OPERATION_WORK,
            Self::OPERATION_SCRATCH,
            |b| {
                self.check_budget(b)?;
                self.check_thread()?;
                if self.held != TraceState::Exec {
                    return Err(Error::State("root trace has no held exec observation"));
                }
                self.child.record()?.prepare_root_trace()?;
                // SAFETY: the caller authenticates exec/alias closure; the owned
                // observed exec stop, original account and custody are checked above.
                unsafe { self.child.confirm_exec(b) }
            },
        )
    }

    /// Uses the original single prepaid cancellation step. SIGKILL bypasses all
    /// enabled stops; a pending exact terminal wait moves to the existing slot.
    pub fn cancel(&mut self) -> Poll {
        self.child.cancel()
    }
}

impl fmt::Debug for RootTaskTraceV2<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootTaskTraceV2")
            .field("pid", &self.pid())
            .field("held", &self.held)
            .field("authority", &"root-task-trace-only")
            .finish_non_exhaustive()
    }
}

fn origin() -> (Pid, Pid, ThreadId) {
    (
        rustix::process::getpid(),
        rustix::thread::gettid(),
        std::thread::current().id(),
    )
}

fn ptrace(request: TraceRequest, pid: Pid, data: usize) -> Result<()> {
    // libc's request type differs between glibc and musl; infer its native ABI.
    let request = match request {
        TraceRequest::Seize => libc::PTRACE_SEIZE,
        TraceRequest::Continue => libc::PTRACE_CONT,
        TraceRequest::Listen => libc::PTRACE_LISTEN,
    };
    // SAFETY: the closed caller retains the unreaped clone owner; SEIZE installs
    // only OPTIONS, and restarts target only its same-thread observed root stop.
    let result = unsafe {
        libc::ptrace(
            request,
            pid.as_raw_nonzero().get(),
            std::ptr::null_mut::<libc::c_void>(),
            data as *mut libc::c_void,
        )
    };
    if result == -1 {
        let errno = std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or(libc::EIO);
        Err(io(
            "operate owned root trace",
            Errno::from_raw_os_error(errno),
        ))
    } else {
        Ok(())
    }
}

fn decode(status: WaitIdStatus) -> Result<TraceState> {
    let detail = status
        .exit_status()
        .or_else(|| status.terminating_signal())
        .or_else(|| status.trapping_signal());
    classify(status.raw_code(), detail).ok_or(Error::State(
        "root trace observed an unsupported kernel status",
    ))
}

// Scalar classification is inert: only wait_root_trace can notify terminal custody.
fn classify(code: i32, detail: Option<i32>) -> Option<TraceState> {
    use TraceState as Event;
    let value = detail?;
    match code {
        libc::CLD_EXITED if (0..=255).contains(&value) => Some(Event::Exited(value)),
        libc::CLD_KILLED | libc::CLD_DUMPED if (1..=64).contains(&value) => Some(Event::Signaled {
            signal: value,
            core_dumped: code == libc::CLD_DUMPED,
        }),
        libc::CLD_TRAPPED => {
            let signal = value & 0xff;
            match value >> 8 {
                0 if (1..=64).contains(&signal) && signal != libc::SIGKILL => {
                    Some(Event::SignalStop(signal))
                }
                libc::PTRACE_EVENT_EXEC if signal == libc::SIGTRAP => Some(Event::Exec),
                libc::PTRACE_EVENT_STOP if signal == libc::SIGTRAP => Some(Event::TrapStop),
                libc::PTRACE_EVENT_STOP
                    if matches!(
                        signal,
                        libc::SIGSTOP | libc::SIGTSTP | libc::SIGTTIN | libc::SIGTTOU
                    ) =>
                {
                    Some(Event::GroupStop(signal))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "native_root_trace_tests.rs"]
mod tests;
