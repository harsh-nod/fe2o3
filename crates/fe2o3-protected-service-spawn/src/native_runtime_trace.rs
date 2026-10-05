//! Original-trace takeover mechanics. No compiler execution admission is created.

use super::{Budget, ENTRY, Error, Pid, Resource, Result, RootTaskTraceV2, TraceState, io};
use rustix::process::{WaitId, WaitIdOptions, WaitIdStatus};
use std::{
    mem::{ManuallyDrop, MaybeUninit},
    time::Instant,
};

#[path = "native_runtime_trace_custody.rs"]
mod custody;

#[path = "native_runtime_task_observation.rs"]
mod observation;
pub use observation::RuntimeTaskObservationV1;

/// Fixed foreground custody bound. The final slot is reserved for an observed
/// over-limit birth so refusal never discards an already acquired child.
pub const MAX_RUNTIME_TASKS: usize = 32;
const CAPACITY: usize = MAX_RUNTIME_TASKS + 1;
const OPTIONS: usize = (libc::PTRACE_O_TRACEFORK
    | libc::PTRACE_O_TRACEVFORK
    | libc::PTRACE_O_TRACECLONE
    | libc::PTRACE_O_TRACEEXEC
    | libc::PTRACE_O_TRACEEXIT
    | libc::PTRACE_O_TRACESECCOMP
    | libc::PTRACE_O_TRACESYSGOOD
    | libc::PTRACE_O_EXITKILL) as usize;

#[derive(Clone, Copy)]
enum Request {
    SetOptions,
    EventMessage,
    Syscall,
    Continue,
    Listen,
    Interrupt,
    Registers,
    SyscallInfo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stop {
    Running,
    Interrupt,
    Exec,
    Seccomp,
    Syscall,
    Birth(i32),
    ExitBoundary,
    Signal(i32),
    Group(i32),
    Terminal { code: i32, value: i32 },
    Unknown,
}

impl Stop {
    fn parked(self) -> bool {
        !matches!(self, Self::Running | Self::Terminal { .. } | Self::Unknown)
    }
}

#[derive(Clone, Copy)]
struct Task {
    pid: Pid,
    stop: Stop,
    child: Option<Pid>,
    syscall: Option<u64>,
    kill_sent: bool,
}

/// Inert copy of an internally retained observation; never accepted as authority.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeTraceEventV1 {
    pid: Pid,
    stop: Stop,
}

impl RuntimeTraceEventV1 {
    /// Context only; no caller-supplied PID can select an operation.
    pub fn pid(self) -> Pid {
        self.pid
    }
    pub fn is_exec(self) -> bool {
        self.stop == Stop::Exec
    }
    pub fn is_checkpoint(self) -> bool {
        self.stop == Stop::Seccomp
    }
    pub fn is_syscall_stop(self) -> bool {
        self.stop == Stop::Syscall
    }
    pub fn is_birth(self) -> bool {
        matches!(self.stop, Stop::Birth(_))
    }
    pub fn is_exit_boundary(self) -> bool {
        self.stop == Stop::ExitBoundary
    }
    pub fn is_terminal(self) -> bool {
        matches!(self.stop, Stop::Terminal { .. })
    }
    pub fn exit_code(self) -> Option<i32> {
        match self.stop {
            Stop::Terminal {
                code: libc::CLD_EXITED,
                value,
            } => Some(value),
            _ => None,
        }
    }
    pub fn terminating_signal(self) -> Option<i32> {
        match self.stop {
            Stop::Terminal {
                code: libc::CLD_KILLED | libc::CLD_DUMPED,
                value,
            } => Some(value),
            _ => None,
        }
    }
}

/// Actual kernel seccomp-entry scalars, not a syscall permission or policy token.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeSyscallEntryV1 {
    pub number: u64,
    pub arguments: [u64; 6],
}

/// Consumes the SAME originating-thread trace and its original identity/slot.
/// There is no SEIZE, PID constructor, policy callback or enforcement guard.
///
/// The original dedicated process MUST retain an independent outside-domain
/// custodian. Until every acquired task has a consuming terminal wait, Drop
/// fail-stops that process without running destructors. TRACEEXIT obligations
/// must never enter the root-only background cleanup path.
pub struct RootRuntimeTraceV1<'work> {
    original: ManuallyDrop<RootTaskTraceV2<'work>>,
    tasks: [Option<Task>; CAPACITY],
    selected: Option<usize>,
    inflight: Option<usize>,
    root_completion: Option<RuntimeTraceEventV1>,
    generation: u64,
    census_generation: Option<u64>,
    retained: usize,
    deadline: Instant,
    armed: bool,
    cancelling: bool,
    retired: bool,
}

impl<'work> RootRuntimeTraceV1<'work> {
    pub const STORAGE_GROWTH: usize = std::mem::size_of::<Self>();
    pub const OPERATION_WORK: usize = ENTRY + CAPACITY * 16 * (1024 + 64);
    pub const OPERATION_SCRATCH: usize = 4 * std::mem::size_of::<Self>() + 4096;
    /// Additional fixed entry work above an original root observation.
    pub const ROOT_OBSERVATION_WORK: usize = ENTRY;

    /// The trace must hold its own pre-exec interrupt stop. Prepay STORAGE_GROWTH.
    ///
    /// # Safety
    /// Keep the original exec gate closed through takeover. Before ANY later
    /// release, authenticate the exact typed-checkpoint compiler stage and its
    /// complete runtime policy; a refusal may instead retire the unopened gate.
    /// No descendants or other tracers may exist. Preserve the original account,
    /// exclusive wait/mutation ownership, dedicated process and outside custodian
    /// for the entire lifetime. Supply the original absolute execution deadline.
    pub(super) unsafe fn begin(
        trace: RootTaskTraceV2<'work>,
        deadline: Instant,
        b: &mut Budget<'_>,
    ) -> Result<Self> {
        let retained = trace
            .retained_storage()
            .checked_add(Self::STORAGE_GROWTH)
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(
            retained,
            ENTRY,
            Self::OPERATION_WORK,
            Self::OPERATION_SCRATCH,
            |b| {
                trace.check_budget(b)?;
                trace.check_thread()?;
                trace.child.record()?.prepare_root_trace()?;
                if trace.held != TraceState::TrapStop || Instant::now() >= deadline {
                    return Err(Error::State(
                        "runtime takeover requires a held pre-exec interrupt",
                    ));
                }
                let mut tasks = [None; CAPACITY];
                tasks[0] = Some(Task {
                    pid: trace.pid(),
                    stop: Stop::Interrupt,
                    child: None,
                    syscall: None,
                    kill_sent: false,
                });
                let mut owner = Self {
                    original: ManuallyDrop::new(trace),
                    tasks,
                    selected: Some(0),
                    inflight: None,
                    root_completion: None,
                    generation: 0,
                    census_generation: None,
                    retained,
                    deadline,
                    armed: false,
                    cancelling: false,
                    retired: false,
                };
                // Mark the foreground obligation before the kernel may enable EXIT.
                owner.armed = true;
                request(Request::SetOptions, owner.root().pid(), 0, OPTIONS)?;
                owner.root_mut().retained = retained;
                Ok(owner)
            },
        )
    }

    fn root(&self) -> &RootTaskTraceV2<'work> {
        &self.original
    }
    fn root_mut(&mut self) -> &mut RootTaskTraceV2<'work> {
        &mut self.original
    }
    pub fn retained_storage(&self) -> usize {
        self.retained
    }

    fn invalidate_census(&mut self) -> Result<()> {
        self.census_generation = None;
        self.generation = self.generation.checked_add(1).ok_or(Resource::Arithmetic)?;
        Ok(())
    }

    pub(in crate::native_spawn) fn check_budget(&self, b: &Budget<'_>) -> Result<()> {
        // Retained resource/publication custody survives terminal retirement.
        // This checks no task liveness and grants no process-operation authority.
        self.root().check_budget(b)?;
        self.root().check_thread()
    }

    /// Read-only original-root observation. The callback gets no consuming wait
    /// or resume authority; each exported descriptor still requires admission.
    pub fn with_task_observation<'budget, R, E>(
        &self,
        b: &mut Budget<'budget>,
        operation: impl FnOnce(
            &super::RootTaskObservationV2<'_, 'work>,
            &mut Budget<'budget>,
        ) -> std::result::Result<R, E>,
    ) -> std::result::Result<R, E>
    where
        E: From<Resource> + From<Error>,
    {
        b.with_prepaid_scope(self.retained, ENTRY, Self::ROOT_OBSERVATION_WORK, 0, |b| {
            self.check(b, true)?;
            self.root().with_task_observation(b, operation)
        })
    }

    fn check(&self, b: &Budget<'_>, executing: bool) -> Result<()> {
        self.check_budget(b)?;
        if self.retired || (executing && (self.cancelling || Instant::now() >= self.deadline)) {
            return Err(Error::State(
                "runtime trace is retired, cancelling or expired",
            ));
        }
        Ok(())
    }

    fn scope<R>(
        &mut self,
        b: &mut Budget<'_>,
        executing: bool,
        f: impl FnOnce(&mut Self) -> Result<R>,
    ) -> Result<R> {
        b.with_prepaid_scope(
            self.retained,
            ENTRY,
            Self::OPERATION_WORK,
            Self::OPERATION_SCRATCH,
            |b| {
                self.check(b, executing)?;
                f(self)
            },
        )
    }

    fn selected(&self) -> Result<(usize, Task)> {
        let index = self
            .selected
            .ok_or(Error::State("runtime trace has no selected observation"))?;
        Ok((
            index,
            self.tasks[index].ok_or(Error::State("runtime task custody is missing"))?,
        ))
    }

    /// One bounded scan, retaining every acquired status before any decoding or
    /// birth query can fail. Repeats the selected held observation until resumed.
    pub fn poll(&mut self, b: &mut Budget<'_>) -> Result<Option<RuntimeTraceEventV1>> {
        self.scope(b, true, |this| {
            if let Some(index) = this.selected {
                let task = this.tasks[index].ok_or(Error::State("selected task is absent"))?;
                return Ok(Some(RuntimeTraceEventV1 {
                    pid: task.pid,
                    stop: task.stop,
                }));
            }
            if let Some(index) = this.inflight {
                this.observe(index)?;
                let task = this.tasks[index].ok_or(Error::State("requester disappeared"))?;
                if task.stop == Stop::Running {
                    return Ok(None);
                }
                this.selected = Some(index);
                return Ok(Some(RuntimeTraceEventV1 {
                    pid: task.pid,
                    stop: task.stop,
                }));
            }
            for index in 0..CAPACITY {
                if this.tasks[index].is_some_and(|t| t.stop == Stop::Running) {
                    this.observe(index)?;
                }
                if let Some(task) = this.tasks[index] {
                    if task.stop != Stop::Running {
                        this.selected = Some(index);
                        return Ok(Some(RuntimeTraceEventV1 {
                            pid: task.pid,
                            stop: task.stop,
                        }));
                    }
                }
            }
            Ok(None)
        })
    }

    fn observe(&mut self, index: usize) -> Result<()> {
        let task = self.tasks[index].ok_or(Error::State("unknown runtime task"))?;
        let status = if index == 0 {
            self.root_mut().record_mut()?.wait_root_trace()?
        } else {
            rustix::process::waitid(
                WaitId::Pid(task.pid),
                WaitIdOptions::EXITED
                    | WaitIdOptions::STOPPED
                    | WaitIdOptions::NOHANG
                    | WaitIdOptions::from_bits_retain(libc::__WALL as u32),
            )
            .map_err(|e| io("consume retained descendant status", e))?
        };
        let Some(status) = status else {
            return Ok(());
        };
        // A consumed unexpected status remains an unresolved owned obligation.
        self.tasks[index].as_mut().expect("retained task").stop = Stop::Unknown;
        self.invalidate_census()?;
        let stop = decode(status)?;
        self.tasks[index].as_mut().expect("retained task").stop = stop;
        if index == 0 {
            self.root_mut().held = match stop {
                Stop::Exec => TraceState::Exec,
                Stop::Terminal {
                    code: libc::CLD_EXITED,
                    value,
                } => TraceState::Exited(value),
                Stop::Terminal { code, value } => TraceState::Signaled {
                    signal: value,
                    core_dumped: code == libc::CLD_DUMPED,
                },
                _ => TraceState::TrapStop,
            };
        }
        if matches!(stop, Stop::Birth(_)) {
            self.acquire_birth(index)?;
        }
        Ok(())
    }

    fn acquire_birth(&mut self, parent: usize) -> Result<()> {
        let task = self.tasks[parent].ok_or(Error::State("birth parent missing"))?;
        if task.child.is_some() {
            return Ok(());
        }
        let mut value = 0_usize;
        request(
            Request::EventMessage,
            task.pid,
            0,
            (&raw mut value) as usize,
        )?;
        let raw = i32::try_from(value).map_err(|_| Error::State("noncanonical birth identity"))?;
        let pid = Pid::from_raw(raw).ok_or(Error::State("invalid birth identity"))?;
        if self.tasks.iter().flatten().any(|t| t.pid == pid) {
            return Err(Error::State("birth reused retained task identity"));
        }
        let index = self
            .tasks
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(index, task)| task.is_none().then_some(index))
            .ok_or(Error::State("runtime emergency custody slot exhausted"))?;
        self.tasks[index] = Some(Task {
            pid,
            stop: Stop::Running,
            child: None,
            syscall: None,
            kill_sent: false,
        });
        self.tasks[parent].as_mut().expect("retained parent").child = Some(pid);
        if index >= MAX_RUNTIME_TASKS {
            self.cancelling = true;
            return Err(Error::State("runtime task bound exceeded; child retained"));
        }
        Ok(())
    }

    /// Reads the exact selected SECCOMP stop. Caller policy must validate these
    /// scalars while all sharers are parked before stepping the operation.
    pub fn syscall_entry(&mut self, b: &mut Budget<'_>) -> Result<RuntimeSyscallEntryV1> {
        self.scope(b, true, |this| {
            let (_, task) = this.selected()?;
            if task.stop != Stop::Seccomp {
                return Err(Error::State("not a syscall checkpoint"));
            }
            let (bytes, info) = syscall_info(task.pid)?;
            if bytes < 84 || info[0] != 3 || word(&info, 4) != 0xc000_003e {
                return Err(Error::State(
                    "kernel did not authenticate native seccomp entry",
                ));
            }
            let number = double(&info, 24);
            if number >= 0x4000_0000 {
                return Err(Error::State("unsupported syscall ABI"));
            }
            let mut arguments = [0; 6];
            for (index, argument) in arguments.iter_mut().enumerate() {
                *argument = double(&info, 32 + 8 * index);
            }
            Ok(RuntimeSyscallEntryV1 { number, arguments })
        })
    }

    /// Steps only the selected entry with every other owned task parked. No
    /// sibling is resumed. The original syscall number is retained internally.
    pub fn step_syscall(&mut self, b: &mut Budget<'_>) -> Result<()> {
        let entry = self.syscall_entry(b)?;
        self.scope(b, true, |this| {
            if this.census_generation != Some(this.generation) {
                return Err(Error::State("syscall entry lacks a fresh stopped census"));
            }
            let (index, task) = this.selected()?;
            if this.tasks.iter().enumerate().any(|(other, t)| {
                other != index
                    && t.is_some_and(|t| {
                        !t.stop.parked() && !matches!(t.stop, Stop::Terminal { .. })
                    })
            }) {
                return Err(Error::State("syscall step requires all siblings parked"));
            }
            this.tasks[index]
                .as_mut()
                .expect("retained requester")
                .syscall = Some(entry.number);
            this.invalidate_census()?;
            request(Request::Syscall, task.pid, 0, 0)?;
            this.tasks[index].as_mut().expect("retained requester").stop = Stop::Running;
            this.selected = None;
            this.inflight = Some(index);
            Ok(())
        })
    }

    /// Resume only the internally selected observation. Entry checkpoints must
    /// use step_syscall. A syscall-exit observation must first pass syscall_result;
    /// this method does not replace the caller's mapping/descriptor policy.
    pub fn resume_selected(&mut self, b: &mut Budget<'_>) -> Result<()> {
        let (_, selected) = self.selected()?;
        if selected.stop == Stop::Syscall {
            let _ = self.syscall_result(b)?;
        }
        self.scope(b, true, |this| {
            let (index, task) = this.selected()?;
            if matches!(task.stop, Stop::Exec | Stop::Syscall)
                && this.census_generation != Some(this.generation)
            {
                return Err(Error::State("runtime resume lacks a fresh stopped census"));
            }
            let continued = if task.syscall.is_some() {
                Request::Syscall
            } else {
                Request::Continue
            };
            let (operation, signal) = match task.stop {
                Stop::Interrupt => (continued, 0),
                Stop::Exec | Stop::Syscall | Stop::ExitBoundary => (Request::Continue, 0),
                Stop::Signal(signal) => (continued, signal as usize),
                Stop::Group(_) => (Request::Listen, 0),
                Stop::Birth(_) if task.child.is_some() && task.syscall.is_some() => {
                    let child = task.child.expect("checked birth");
                    if !this
                        .tasks
                        .iter()
                        .flatten()
                        .any(|t| t.pid == child && t.stop.parked())
                    {
                        return Err(Error::State("birth child is not held at its own stop"));
                    }
                    (Request::Syscall, 0)
                }
                _ => {
                    return Err(Error::State(
                        "selected runtime observation is not resumable",
                    ));
                }
            };
            this.invalidate_census()?;
            request(operation, task.pid, 0, signal)?;
            let retained = this.tasks[index].as_mut().expect("retained task");
            retained.stop = Stop::Running;
            retained.child = None;
            if task.stop == Stop::Syscall {
                retained.syscall = None;
                this.inflight = None;
            }
            if index == 0 {
                this.root_mut().held = TraceState::Pending;
            }
            this.selected = None;
            Ok(())
        })
    }

    /// Observe only the child already acquired from the selected birth event.
    /// This never resumes the parent or child, and accepts no supplied task ID.
    pub fn hold_born_child(&mut self, b: &mut Budget<'_>) -> Result<bool> {
        self.scope(b, true, |this| {
            let (_, task) = this.selected()?;
            if !matches!(task.stop, Stop::Birth(_)) {
                return Err(Error::State("selected observation is not a birth"));
            }
            let child = task
                .child
                .ok_or(Error::State("birth child is not acquired"))?;
            let index = this
                .tasks
                .iter()
                .position(|t| t.is_some_and(|t| t.pid == child))
                .ok_or(Error::State("acquired child is absent"))?;
            if this.tasks[index].is_some_and(|t| t.stop == Stop::Running) {
                this.observe(index)?;
            }
            Ok(this.tasks[index].is_some_and(|t| t.stop.parked()))
        })
    }

    /// Retire only the selected status already obtained by this controller's
    /// consuming wait. Keep root completion distinct; descendant slot reuse is
    /// permitted only after this transition, never after a signal or EXIT stop.
    pub fn acknowledge_terminal(&mut self, b: &mut Budget<'_>) -> Result<RuntimeTraceEventV1> {
        self.scope(b, false, |this| {
            let (index, task) = this.selected()?;
            if !matches!(task.stop, Stop::Terminal { .. }) {
                return Err(Error::State("runtime terminal wait has not been consumed"));
            }
            let event = RuntimeTraceEventV1 {
                pid: task.pid,
                stop: task.stop,
            };
            this.invalidate_census()?;
            if index == 0 {
                if this.root_completion.is_some() {
                    return Err(Error::State("runtime root completion already retired"));
                }
                this.root_completion = Some(event);
            }
            this.tasks[index] = None;
            this.selected = None;
            if this.inflight == Some(index) {
                this.inflight = None;
            }
            this.retired = this.root_completion.is_some() && this.tasks.iter().all(Option::is_none);
            if this.retired {
                let _ = this.root_mut().cancel();
            }
            Ok(event)
        })
    }

    /// Actual root terminal observation, if separately acknowledged. This is
    /// not success of descendants, cleanup, compiler validation or publication.
    pub fn root_completion(&self) -> Option<RuntimeTraceEventV1> {
        self.root_completion
    }

    /// Budgetless refusal path: retain every obligation and forbid execution.
    /// No signal, wait, resume, background transfer or cleanup claim occurs.
    /// The outer owner must still drive funded cancel_step or preserve fail-stop.
    pub fn mark_cancellation(&mut self) {
        self.cancelling = true;
        self.census_generation = None;
    }

    /// Inert trace-retirement state only, not domain or publication completion.
    pub fn is_trace_retired(&self) -> bool {
        self.retired
    }

    /// Report the original cleanup mechanism only AFTER every trace obligation
    /// has a consuming terminal wait. Pending/domain quarantine remain distinct.
    pub fn cleanup_after_retirement(&mut self) -> Result<super::Poll> {
        if !self.retired {
            return Err(Error::State("runtime trace is not terminally retired"));
        }
        self.root().check_thread()?;
        Ok(self.root_mut().cancel())
    }

    /// Use the SAME stable-stop scheduler as the proof controller. Every poll
    /// or interrupt is charged to the original account; the original absolute
    /// deadline is checked even if the complete census was already stopped.
    pub fn park_all(&mut self, b: &mut Budget<'_>) -> Result<()> {
        self.check(b, true)?;
        let deadline = self.deadline;
        b.with_prepaid_scope(self.retained, ENTRY, ENTRY, Self::OPERATION_SCRATCH, |b| {
            let mut census = Census {
                owner: self,
                budget: b,
            };
            crate::trace_runtime::stable::park_all(&mut census, deadline, &mut || Ok(()))?;
            census.owner.check(census.budget, true)?;
            census.owner.census_generation = Some(census.owner.generation);
            Ok(())
        })
    }

    /// Resume only scheduler-owned interrupt stops, leaving checkpoints, signals
    /// and the selected syscall requester parked. No new user-selected PID is used.
    pub fn release_interrupts(&mut self, b: &mut Budget<'_>) -> Result<()> {
        self.scope(b, true, |this| {
            if this.inflight.is_some() {
                return Err(Error::State("syscall still in flight"));
            }
            this.invalidate_census()?;
            for index in 0..CAPACITY {
                if this.selected == Some(index) {
                    continue;
                }
                if let Some(task) = this.tasks[index] {
                    if task.stop == Stop::Interrupt {
                        request(Request::Continue, task.pid, 0, 0)?;
                        this.tasks[index].as_mut().expect("retained task").stop = Stop::Running;
                        if index == 0 {
                            this.root_mut().held = TraceState::Pending;
                        }
                    }
                }
            }
            Ok(())
        })
    }

    /// Returns the kernel-authenticated result while the requester and siblings
    /// are still stopped. No returned descriptor is accepted or resumed here.
    pub fn syscall_result(&mut self, b: &mut Budget<'_>) -> Result<i64> {
        self.scope(b, true, |this| {
            let (_, task) = this.selected()?;
            let number = task.syscall.ok_or(Error::State("no owned syscall entry"))?;
            if task.stop != Stop::Syscall {
                return Err(Error::State("no exact syscall exit stop"));
            }
            let (bytes, info) = syscall_info(task.pid)?;
            if bytes < 33 {
                return Err(Error::State("short kernel syscall-exit record"));
            }
            let mut registers: libc::user_regs_struct = unsafe { std::mem::zeroed() };
            request(
                Request::Registers,
                task.pid,
                0,
                (&raw mut registers) as usize,
            )?;
            crate::trace_runtime::validate_syscall_exit(
                &info,
                number,
                registers.orig_rax,
                registers.rax,
            )
            .map_err(|e| Error::State(e.message()))
        })
    }

    /// Bounded foreground cancellation step. First acquire/quiesce the complete
    /// known tree, then kill every member, then resume only killed stops. False
    /// retains all custody; errors do not transfer it to background cleanup.
    pub fn cancel_step(&mut self, b: &mut Budget<'_>) -> Result<bool> {
        self.scope(b, false, |this| {
            this.cancelling = true;
            this.invalidate_census()?;
            for index in 0..CAPACITY {
                let Some(task) = this.tasks[index] else {
                    continue;
                };
                if task.stop == Stop::Running {
                    this.observe(index)?;
                }
                if this.tasks[index].is_some_and(|t| matches!(t.stop, Stop::Birth(_))) {
                    this.acquire_birth(index)?;
                }
                let task = this.tasks[index].expect("retained task");
                if task.stop == Stop::Running && !task.kill_sent {
                    request(Request::Interrupt, task.pid, 0, 0)?;
                }
            }
            if this.tasks.iter().flatten().any(|t| t.stop == Stop::Unknown) {
                return Err(Error::State("runtime cancellation retains unknown status"));
            }
            if this
                .tasks
                .iter()
                .flatten()
                .any(|t| t.stop == Stop::Running && !t.kill_sent)
            {
                return Ok(false);
            }
            for task in this.tasks.iter_mut().flatten() {
                if !matches!(task.stop, Stop::Terminal { .. }) && !task.kill_sent {
                    rustix::process::kill_process(task.pid, rustix::process::Signal::KILL)
                        .map_err(|e| io("kill retained runtime task", e))?;
                    task.kill_sent = true;
                }
            }
            for task in this.tasks.iter_mut().flatten() {
                if task.kill_sent && task.stop.parked() {
                    match request(Request::Continue, task.pid, 0, libc::SIGKILL as usize) {
                        Ok(_)
                        | Err(Error::Io {
                            source: rustix::io::Errno::SRCH,
                            ..
                        }) => {}
                        Err(error) => return Err(error),
                    }
                    task.stop = Stop::Running;
                }
            }
            this.retired = this
                .tasks
                .iter()
                .flatten()
                .all(|t| matches!(t.stop, Stop::Terminal { .. }));
            // Trace retirement does not imply domain/slot cleanup. Once every
            // trace task is terminal, the original pool may finish that work.
            if this.retired {
                let _ = this.root_mut().cancel();
            }
            Ok(this.retired)
        })
    }
}

struct Census<'owner, 'budget, 'work> {
    owner: &'owner mut RootRuntimeTraceV1<'work>,
    budget: &'owner mut Budget<'budget>,
}

impl Census<'_, '_, '_> {
    fn debit(&mut self) -> Result<()> {
        self.budget.charge_work(ENTRY + 16 * (1024 + 64))?;
        self.owner.check(self.budget, true)
    }
}

impl crate::trace_runtime::stable::StoppedCensus for Census<'_, '_, '_> {
    type Task = usize;
    type Snapshot = std::iter::Flatten<std::array::IntoIter<Option<usize>, CAPACITY>>;
    type Error = Error;

    fn tasks(&self) -> Self::Snapshot {
        std::array::from_fn(|index| {
            self.owner.tasks[index]
                .filter(|t| !matches!(t.stop, Stop::Terminal { .. }))
                .map(|_| index)
        })
        .into_iter()
        .flatten()
    }
    fn parked(&self, index: usize) -> bool {
        self.owner.tasks[index].is_some_and(|t| t.stop.parked())
    }
    fn observe_and_remember(&mut self, index: usize) -> Result<bool> {
        self.debit()?;
        self.owner.observe(index)?;
        Ok(self.owner.tasks[index].is_some_and(|t| t.stop != Stop::Running))
    }
    fn interrupt(&mut self, index: usize) -> Result<()> {
        self.debit()?;
        let task = self.owner.tasks[index].ok_or(Error::State("census task is absent"))?;
        request(Request::Interrupt, task.pid, 0, 0)?;
        Ok(())
    }
    fn escaped_lifecycle(&self) -> bool {
        self.owner.tasks.iter().flatten().any(|t| {
            matches!(
                t.stop,
                Stop::Birth(_) | Stop::ExitBoundary | Stop::Terminal { .. } | Stop::Unknown
            )
        })
    }
    fn lifecycle_error(&self) -> Error {
        Error::State("unmediated lifecycle transition in runtime census")
    }
    fn timeout_error(&self) -> Error {
        Error::State("original runtime deadline elapsed during stopped census")
    }
    fn validate_census(&mut self) -> Result<()> {
        for index in 0..CAPACITY {
            let Some(task) = self.owner.tasks[index] else {
                continue;
            };
            if !task.stop.parked() {
                return Err(Error::State("runtime census is not stopped"));
            }
            self.debit()?;
            let path = format!("/proc/{}/task", task.pid.as_raw_nonzero().get());
            let directory = rustix::fs::open(
                path.as_str(),
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NOFOLLOW,
                rustix::fs::Mode::empty(),
            )
            .map_err(|e| io("open owned stopped task census", e))?;
            let mut buffer = [MaybeUninit::uninit(); 4096];
            let mut directory = rustix::fs::RawDir::new(directory, &mut buffer);
            let mut count = 0;
            let mut found = false;
            while let Some(entry) = directory.next() {
                self.debit()?;
                let entry = entry.map_err(|e| io("read owned stopped task census", e))?;
                let name = entry.file_name().to_bytes();
                if matches!(name, b"." | b"..") {
                    continue;
                }
                count += 1;
                if count > MAX_RUNTIME_TASKS
                    || name.first() == Some(&b'0')
                    || !name.iter().all(u8::is_ascii_digit)
                {
                    return Err(Error::State("runtime census exceeds canonical task bound"));
                }
                let tid = std::str::from_utf8(name)
                    .ok()
                    .and_then(|s| s.parse::<i32>().ok())
                    .and_then(Pid::from_raw)
                    .ok_or(Error::State("invalid runtime census identity"))?;
                if !self
                    .owner
                    .tasks
                    .iter()
                    .flatten()
                    .any(|t| t.pid == tid && t.stop.parked())
                {
                    return Err(Error::State("runtime census contains unaccounted sharer"));
                }
                found |= tid == task.pid;
            }
            if !found {
                return Err(Error::State("runtime census lost its retained task"));
            }
        }
        Ok(())
    }
}

impl Drop for RootRuntimeTraceV1<'_> {
    fn drop(&mut self) {
        if self.armed && !self.retired {
            // SAFETY: takeover requires a dedicated process with an independent
            // domain custodian. Do not run root-only cleanup or release retained
            // artifacts while descendant/TRACEEXIT custody remains unresolved.
            unsafe { libc::_exit(125) }
        }
        // SAFETY: original is initialized once and never moved out. Before arm,
        // or after all actual terminal waits, its normal cleanup is appropriate.
        unsafe { ManuallyDrop::drop(&mut self.original) };
    }
}

fn request(request: Request, pid: Pid, address: usize, data: usize) -> Result<usize> {
    // Infer libc's request ABI for both glibc and musl, as the original tracer does.
    let request = match request {
        Request::SetOptions => libc::PTRACE_SETOPTIONS,
        Request::EventMessage => libc::PTRACE_GETEVENTMSG,
        Request::Syscall => libc::PTRACE_SYSCALL,
        Request::Continue => libc::PTRACE_CONT,
        Request::Listen => libc::PTRACE_LISTEN,
        Request::Interrupt => libc::PTRACE_INTERRUPT,
        Request::Registers => libc::PTRACE_GETREGS,
        Request::SyscallInfo => libc::PTRACE_GET_SYSCALL_INFO,
    };
    // SAFETY: callers select only a PID retained from original clone custody or
    // an actual owned birth event, and keep every pointed-to output buffer live.
    let result = unsafe {
        libc::ptrace(
            request,
            pid.as_raw_nonzero().get(),
            address as *mut libc::c_void,
            data as *mut libc::c_void,
        )
    };
    if result == -1 {
        let errno = std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or(libc::EIO);
        return Err(io(
            "operate retained runtime trace",
            rustix::io::Errno::from_raw_os_error(errno),
        ));
    }
    Ok(result as usize)
}

fn syscall_info(pid: Pid) -> Result<(usize, [u8; 88])> {
    let mut info = [0; 88];
    let bytes = request(
        Request::SyscallInfo,
        pid,
        info.len(),
        info.as_mut_ptr() as usize,
    )?;
    Ok((bytes, info))
}

fn word(bytes: &[u8], start: usize) -> u32 {
    u32::from_ne_bytes(
        bytes[start..start + 4]
            .try_into()
            .expect("fixed kernel record"),
    )
}
fn double(bytes: &[u8], start: usize) -> u64 {
    u64::from_ne_bytes(
        bytes[start..start + 8]
            .try_into()
            .expect("fixed kernel record"),
    )
}

fn decode(status: WaitIdStatus) -> Result<Stop> {
    let detail = status
        .exit_status()
        .or_else(|| status.terminating_signal())
        .or_else(|| status.trapping_signal())
        .ok_or(Error::State("missing runtime wait detail"))?;
    classify(status.raw_code(), detail)
}

fn classify(code: i32, detail: i32) -> Result<Stop> {
    match code {
        libc::CLD_EXITED if (0..=255).contains(&detail) => Ok(Stop::Terminal {
            code,
            value: detail,
        }),
        libc::CLD_KILLED | libc::CLD_DUMPED if (1..=64).contains(&detail) => Ok(Stop::Terminal {
            code,
            value: detail,
        }),
        libc::CLD_TRAPPED => {
            let signal = detail & 0xff;
            let event = detail >> 8;
            match (event, signal) {
                (0, value) if value == (libc::SIGTRAP | 0x80) => Ok(Stop::Syscall),
                (libc::PTRACE_EVENT_EXEC, libc::SIGTRAP) => Ok(Stop::Exec),
                (libc::PTRACE_EVENT_SECCOMP, libc::SIGTRAP) => Ok(Stop::Seccomp),
                (libc::PTRACE_EVENT_EXIT, libc::SIGTRAP) => Ok(Stop::ExitBoundary),
                (libc::PTRACE_EVENT_STOP, libc::SIGTRAP) => Ok(Stop::Interrupt),
                (event, libc::SIGTRAP)
                    if matches!(
                        event,
                        libc::PTRACE_EVENT_FORK
                            | libc::PTRACE_EVENT_VFORK
                            | libc::PTRACE_EVENT_CLONE
                    ) =>
                {
                    Ok(Stop::Birth(event))
                }
                (libc::PTRACE_EVENT_STOP, signal)
                    if matches!(
                        signal,
                        libc::SIGSTOP | libc::SIGTSTP | libc::SIGTTIN | libc::SIGTTOU
                    ) =>
                {
                    Ok(Stop::Group(signal))
                }
                (0, signal) if (1..=64).contains(&signal) && signal != libc::SIGKILL => {
                    Ok(Stop::Signal(signal))
                }
                _ => Err(Error::State("unsupported runtime trace status")),
            }
        }
        _ => Err(Error::State("unsupported runtime wait code")),
    }
}

#[cfg(test)]
#[path = "native_runtime_trace_tests.rs"]
mod tests;
