//! Private Linux ioctl boundary for the native queue adapter foundation.
//!
//! There is deliberately no production backend yet. The memory owner does not
//! expose the typed mapped ring/control/EOP/CWSR authorities required to make
//! these calls sound. These functions keep the eventual unsafe boundary small
//! without making an fd or numeric address public.

use core::ffi::c_void;
use core::ptr::NonNull;
use core::sync::atomic::{Ordering, fence};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::sync::{Mutex, MutexGuard};

use fe2o3_kfd_uapi::{
    AMDKFD_IOC_CREATE_EVENT, AMDKFD_IOC_CREATE_QUEUE, AMDKFD_IOC_DESTROY_EVENT,
    AMDKFD_IOC_DESTROY_QUEUE, AMDKFD_IOC_RUNTIME_ENABLE, AMDKFD_IOC_UPDATE_QUEUE,
    AMDKFD_IOC_WAIT_EVENTS, KFD_GFX942_DOORBELL_BYTES, KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES,
    KfdEventDataArrayAddressV1, KfdEventDataV1, KfdGfx942CreateQueueOutputs,
    KfdIoctlCreateEventArgsV1, KfdIoctlCreateQueueArgs, KfdIoctlDestroyEventArgsV1,
    KfdIoctlDestroyQueueArgs, KfdIoctlRuntimeEnableArgsV1, KfdIoctlUpdateQueueArgs,
    KfdIoctlWaitEventsArgsV1, KfdQueueExceptionPayloadAddressV1, KfdQueueExceptionReasonV1,
    KfdSignalEventIdV1, KfdWaitResultV1,
};
use rustix::ioctl::{Opcode, Setter, Updater};
use rustix::mm::{Advice, MapFlags, MprotectFlags, ProtFlags};

const CREATE_QUEUE_OPCODE: Opcode = AMDKFD_IOC_CREATE_QUEUE as Opcode;
const DESTROY_QUEUE_OPCODE: Opcode = AMDKFD_IOC_DESTROY_QUEUE as Opcode;
const CREATE_EVENT_OPCODE: Opcode = AMDKFD_IOC_CREATE_EVENT as Opcode;
const DESTROY_EVENT_OPCODE: Opcode = AMDKFD_IOC_DESTROY_EVENT as Opcode;
const WAIT_EVENTS_OPCODE: Opcode = AMDKFD_IOC_WAIT_EVENTS as Opcode;
const RUNTIME_ENABLE_OPCODE: Opcode = AMDKFD_IOC_RUNTIME_ENABLE as Opcode;
const MAX_QUEUE_EXCEPTION_WAIT_MS: u32 = 1_000;
static KFD_RUNTIME_GATE: Mutex<ProcessGlobalKfdRuntimeGateV1> =
    Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
#[allow(dead_code)]
const UPDATE_QUEUE_OPCODE: Opcode = AMDKFD_IOC_UPDATE_QUEUE as Opcode;

struct RuntimeGateTerminalTeardownArmV1<'a> {
    gate: &'a Mutex<ProcessGlobalKfdRuntimeGateV1>,
    counted: bool,
    confirmed: bool,
}

impl RuntimeGateTerminalTeardownArmV1<'_> {
    fn confirm_destroyed(mut self) {
        finish_runtime_gate_teardown_arm(self.gate, self.counted, true);
        self.confirmed = true;
    }
}

impl Drop for RuntimeGateTerminalTeardownArmV1<'_> {
    fn drop(&mut self) {
        if !self.confirmed {
            finish_runtime_gate_teardown_arm(self.gate, self.counted, false);
        }
    }
}

fn arm_runtime_gate_for_terminal_teardown<'a>(
    gate: &'a Mutex<ProcessGlobalKfdRuntimeGateV1>,
) -> RuntimeGateTerminalTeardownArmV1<'a> {
    let counted = lock_runtime_gate_v1(gate).arm_teardown();
    RuntimeGateTerminalTeardownArmV1 {
        gate,
        counted,
        confirmed: false,
    }
}

fn finish_runtime_gate_teardown_arm(
    gate: &Mutex<ProcessGlobalKfdRuntimeGateV1>,
    counted: bool,
    confirmed: bool,
) {
    lock_runtime_gate_v1(gate).finish_teardown_arm(counted, confirmed);
}

/// Excludes a new queue session until teardown is confirmed end to end.
pub(crate) struct ProcessGlobalKfdRuntimeTeardownArmV1(RuntimeGateTerminalTeardownArmV1<'static>);

impl ProcessGlobalKfdRuntimeTeardownArmV1 {
    pub(crate) fn confirm_destroyed(self) {
        self.0.confirm_destroyed();
    }
}

pub(crate) fn arm_process_global_kfd_runtime_gate_for_teardown_v1()
-> ProcessGlobalKfdRuntimeTeardownArmV1 {
    ProcessGlobalKfdRuntimeTeardownArmV1(arm_runtime_gate_for_terminal_teardown(&KFD_RUNTIME_GATE))
}

pub(crate) fn permanently_poison_process_global_kfd_runtime_gate_v1() {
    lock_runtime_gate_v1(&KFD_RUNTIME_GATE).poison();
}

fn lock_runtime_gate_v1(
    gate: &Mutex<ProcessGlobalKfdRuntimeGateV1>,
) -> MutexGuard<'_, ProcessGlobalKfdRuntimeGateV1> {
    match gate.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            let mut guard = poisoned.into_inner();
            guard.poison();
            guard
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProcessKfdRuntimeStateV1 {
    Disabled,
    Enabled { opener_pid: u32, leases: usize },
    Poisoned,
}

#[derive(Debug)]
struct ProcessGlobalKfdRuntimeGateV1 {
    runtime: ProcessKfdRuntimeStateV1,
    teardown_arms: usize,
    permanently_poisoned: bool,
}

impl ProcessGlobalKfdRuntimeGateV1 {
    const fn new() -> Self {
        Self {
            runtime: ProcessKfdRuntimeStateV1::Disabled,
            teardown_arms: 0,
            permanently_poisoned: false,
        }
    }

    fn admit_runtime(&mut self, opener_pid: u32) -> Result<bool, LinuxDoorbellErrorV1> {
        if self.is_blocked() {
            return Err(LinuxDoorbellErrorV1::Runtime(
                "process-global gate poisoned",
            ));
        }
        self.runtime.join_enabled(opener_pid)
    }

    const fn is_blocked(&self) -> bool {
        self.teardown_arms != 0 || self.permanently_poisoned
    }

    fn arm_teardown(&mut self) -> bool {
        match self.teardown_arms.checked_add(1) {
            Some(arms) => {
                self.teardown_arms = arms;
                true
            }
            None => {
                self.poison();
                false
            }
        }
    }

    fn finish_teardown_arm(&mut self, counted: bool, confirmed: bool) {
        if counted {
            match self.teardown_arms.checked_sub(1) {
                Some(arms) => self.teardown_arms = arms,
                None => self.poison(),
            }
        } else {
            self.poison();
        }
        if !confirmed {
            self.poison();
        }
    }

    fn poison(&mut self) {
        self.permanently_poisoned = true;
        self.runtime.poison();
    }
}

impl ProcessKfdRuntimeStateV1 {
    fn join_enabled(&mut self, opener_pid: u32) -> Result<bool, LinuxDoorbellErrorV1> {
        match *self {
            Self::Disabled => Ok(true),
            Self::Enabled {
                opener_pid: owner_pid,
                leases,
            } if owner_pid == opener_pid => {
                let leases = leases
                    .checked_add(1)
                    .ok_or(LinuxDoorbellErrorV1::Runtime("runtime lease capacity"))?;
                *self = Self::Enabled { opener_pid, leases };
                Ok(false)
            }
            Self::Enabled { .. } => Err(LinuxDoorbellErrorV1::ProcessChanged),
            Self::Poisoned => Err(LinuxDoorbellErrorV1::Runtime(
                "process runtime context poisoned",
            )),
        }
    }

    fn commit_first_enabled(&mut self, opener_pid: u32) {
        debug_assert_eq!(*self, Self::Disabled);
        *self = Self::Enabled {
            opener_pid,
            leases: 1,
        };
    }

    fn release_plan(&mut self, opener_pid: u32) -> Result<bool, LinuxDoorbellErrorV1> {
        match *self {
            Self::Enabled {
                opener_pid: owner_pid,
                leases,
            } if owner_pid == opener_pid && leases > 1 => {
                *self = Self::Enabled {
                    opener_pid,
                    leases: leases - 1,
                };
                Ok(false)
            }
            Self::Enabled {
                opener_pid: owner_pid,
                leases: 1,
            } if owner_pid == opener_pid => Ok(true),
            Self::Enabled { .. } => Err(LinuxDoorbellErrorV1::ProcessChanged),
            Self::Disabled => Err(LinuxDoorbellErrorV1::Runtime("runtime lease underflow")),
            Self::Poisoned => Err(LinuxDoorbellErrorV1::Runtime(
                "process runtime context poisoned",
            )),
        }
    }

    fn commit_last_disabled(&mut self) {
        debug_assert!(matches!(*self, Self::Enabled { leases: 1, .. }));
        *self = Self::Disabled;
    }

    fn poison(&mut self) {
        *self = Self::Poisoned;
    }
}

#[derive(Debug)]
pub(super) enum LinuxDoorbellErrorV1 {
    ProcessChanged,
    UnsupportedPageSize(usize),
    InvalidObservation(&'static str),
    Syscall {
        operation: &'static str,
        source: rustix::io::Errno,
    },
    #[cfg(feature = "live-validation")]
    IsolationRequired,
    #[cfg(feature = "live-validation")]
    ChildProbe(&'static str),
    #[cfg(feature = "live-validation")]
    MappingInherited,
    Event(&'static str),
    EventSyscall {
        operation: &'static str,
        source: rustix::io::Errno,
    },
    Runtime(&'static str),
    RuntimeSyscall {
        operation: &'static str,
        source: rustix::io::Errno,
    },
    Shadow(&'static str),
    ShadowSyscall {
        operation: &'static str,
        source: rustix::io::Errno,
    },
    #[cfg(feature = "live-validation")]
    ShadowMappingInherited,
}

impl core::fmt::Display for LinuxDoorbellErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ProcessChanged => formatter.write_str("doorbell mapping process changed"),
            Self::UnsupportedPageSize(size) => {
                write!(formatter, "unsupported doorbell host page size {size}")
            }
            Self::InvalidObservation(field) => {
                write!(formatter, "invalid doorbell mapping observation: {field}")
            }
            Self::Syscall { operation, source } => {
                write!(formatter, "{operation} failed: {source}")
            }
            #[cfg(feature = "live-validation")]
            Self::IsolationRequired => formatter.write_str(
                "doorbell DONTFORK child probe requires an isolated single-threaded process",
            ),
            #[cfg(feature = "live-validation")]
            Self::ChildProbe(detail) => write!(formatter, "doorbell child probe failed: {detail}"),
            #[cfg(feature = "live-validation")]
            Self::MappingInherited => {
                formatter.write_str("doorbell VMA was inherited despite MADV_DONTFORK")
            }
            Self::Event(detail) => write!(formatter, "queue exception event invalid: {detail}"),
            Self::EventSyscall { operation, source } => {
                write!(formatter, "{operation} failed: {source}")
            }
            Self::Runtime(detail) => write!(formatter, "KFD runtime state invalid: {detail}"),
            Self::RuntimeSyscall { operation, source } => {
                write!(formatter, "{operation} failed: {source}")
            }
            Self::Shadow(detail) => write!(formatter, "CWSR shadow invalid: {detail}"),
            Self::ShadowSyscall { operation, source } => {
                write!(formatter, "{operation} failed: {source}")
            }
            #[cfg(feature = "live-validation")]
            Self::ShadowMappingInherited => {
                formatter.write_str("CWSR shadow VMA was inherited despite MADV_DONTFORK")
            }
        }
    }
}

impl std::error::Error for LinuxDoorbellErrorV1 {}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CwsrShadowPlanV1 {
    base: u64,
    bytes: usize,
    page_bytes: usize,
}

const CWSR_CONTROL_STACK_PAGES_PER_XCC_V1: usize =
    crate::GFX942_CONTROL_STACK_BYTES_PER_XCC_V1 as usize / 4096;
pub(crate) const GFX942_CWSR_SHADOW_PAGES_V1: usize =
    crate::queue::submit::GFX942_CWSR_XCC_COUNT_V1 * CWSR_CONTROL_STACK_PAGES_PER_XCC_V1;

impl CwsrShadowPlanV1 {
    pub(crate) fn from_owned_reservation(
        base: u64,
        bytes: usize,
        page_bytes: usize,
    ) -> Result<Self, LinuxDoorbellErrorV1> {
        use crate::queue::submit::{
            GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1, GFX942_CWSR_TOTAL_BYTES_V1,
            GFX942_CWSR_XCC_COUNT_V1,
        };
        if bytes != GFX942_CWSR_TOTAL_BYTES_V1 || page_bytes != 4096 {
            return Err(LinuxDoorbellErrorV1::Shadow("reservation geometry"));
        }
        if !base.is_multiple_of(page_bytes as u64) {
            return Err(LinuxDoorbellErrorV1::Shadow("reservation alignment"));
        }
        let end = base
            .checked_add(bytes as u64)
            .ok_or(LinuxDoorbellErrorV1::Shadow("reservation overflow"))?;
        for xcc in 0..GFX942_CWSR_XCC_COUNT_V1 {
            let offset = xcc
                .checked_mul(GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1)
                .ok_or(LinuxDoorbellErrorV1::Shadow("XCC offset"))?;
            let control_stack_end = base
                .checked_add(offset as u64)
                .and_then(|address| {
                    address.checked_add(u64::from(crate::GFX942_CONTROL_STACK_BYTES_PER_XCC_V1))
                })
                .ok_or(LinuxDoorbellErrorV1::Shadow("XCC control-stack overflow"))?;
            if control_stack_end > end {
                return Err(LinuxDoorbellErrorV1::Shadow("XCC control-stack range"));
            }
        }
        Ok(Self {
            base,
            bytes,
            page_bytes,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QueueExceptionWaitObservationV1 {
    NoExceptionAtObservation,
    Exception(KfdQueueExceptionReasonV1),
}

fn admit_queue_exception_wait(
    wait: KfdWaitResultV1,
    reason: KfdQueueExceptionReasonV1,
) -> Result<QueueExceptionWaitObservationV1, LinuxDoorbellErrorV1> {
    match (wait, reason.is_empty()) {
        (KfdWaitResultV1::Timeout, true) => {
            Ok(QueueExceptionWaitObservationV1::NoExceptionAtObservation)
        }
        (KfdWaitResultV1::Complete, false) => {
            Ok(QueueExceptionWaitObservationV1::Exception(reason))
        }
        _ => Err(LinuxDoorbellErrorV1::Event("wait/payload disagreement")),
    }
}

fn begin_one_shot_observation(used: &mut bool) -> Result<(), LinuxDoorbellErrorV1> {
    if *used {
        Err(LinuxDoorbellErrorV1::Event(
            "queue exception observation is one-shot",
        ))
    } else {
        *used = true;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KfdRuntimeBindingV1 {
    opener_pid: u32,
    raw_fd: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KfdRuntimeLifecyclePhaseV1 {
    EnabledBeforeQueue,
    QueueLive,
    QueueDestroyed,
    EventDestroyed,
    Disabled,
}

fn admit_runtime_transition(
    current: KfdRuntimeLifecyclePhaseV1,
    required: KfdRuntimeLifecyclePhaseV1,
    next: KfdRuntimeLifecyclePhaseV1,
) -> Result<KfdRuntimeLifecyclePhaseV1, LinuxDoorbellErrorV1> {
    if current == required {
        Ok(next)
    } else {
        Err(LinuxDoorbellErrorV1::Runtime(
            "runtime/queue/event ordering",
        ))
    }
}

/// One linear queue lease on the process-global KFD runtime context.
///
/// The first lease enables the kernel runtime and the last fully torn-down
/// lease disables it. Intermediate leases own independent queue/event phases,
/// so queues on admitted devices can coexist without repeating the process
/// transition. A foreign KFD client in the same process remains outside this
/// authority boundary.
pub(crate) struct LinuxKfdRuntimeEnabledV1 {
    binding: KfdRuntimeBindingV1,
    active: bool,
    poisoned: bool,
    phase: KfdRuntimeLifecyclePhaseV1,
}

pub(crate) struct LinuxKfdRuntimeDisabledV1 {
    binding: KfdRuntimeBindingV1,
    completion_pending: bool,
}

impl LinuxKfdRuntimeEnabledV1 {
    pub(crate) fn enable(
        kfd: BorrowedFd<'_>,
        opener_pid: u32,
    ) -> Result<Self, LinuxDoorbellErrorV1> {
        if opener_pid != std::process::id() || kfd.as_raw_fd() < 0 {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        let mut gate = lock_runtime_gate_v1(&KFD_RUNTIME_GATE);
        let requires_kernel_enable = gate.admit_runtime(opener_pid)?;
        if !requires_kernel_enable {
            return Ok(Self {
                binding: KfdRuntimeBindingV1 {
                    opener_pid,
                    raw_fd: kfd.as_raw_fd(),
                },
                active: true,
                poisoned: false,
                phase: KfdRuntimeLifecyclePhaseV1::EnabledBeforeQueue,
            });
        }

        let expected = KfdIoctlRuntimeEnableArgsV1::new_queue_exception_enable();
        let mut args = expected;
        // SAFETY: the exact 16-byte in/out record remains exclusively borrowed
        // for the complete ioctl. No pointer is embedded in this profile.
        let request = unsafe { Updater::<RUNTIME_ENABLE_OPCODE, _>::new(&mut args) };
        // SAFETY: the retained process-bound fd and exact record satisfy the
        // reviewed request. Every error is treated as an ambiguous transition.
        if let Err(source) = unsafe { rustix::ioctl::ioctl(kfd, request) } {
            gate.poison();
            return Err(LinuxDoorbellErrorV1::RuntimeSyscall {
                operation: "AMDKFD_IOC_RUNTIME_ENABLE(enable)",
                source,
            });
        }
        if args != expected || !args.is_exact_queue_exception_enable() {
            gate.poison();
            return Err(LinuxDoorbellErrorV1::Runtime(
                "RUNTIME_ENABLE enable output drift",
            ));
        }
        gate.runtime.commit_first_enabled(opener_pid);
        Ok(Self {
            binding: KfdRuntimeBindingV1 {
                opener_pid,
                raw_fd: kfd.as_raw_fd(),
            },
            active: true,
            poisoned: false,
            phase: KfdRuntimeLifecyclePhaseV1::EnabledBeforeQueue,
        })
    }

    fn check_binding(
        &self,
        kfd: BorrowedFd<'_>,
        opener_pid: u32,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        if opener_pid != std::process::id()
            || self.binding.opener_pid != opener_pid
            || self.binding.raw_fd != kfd.as_raw_fd()
        {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        if !self.active || self.poisoned {
            return Err(LinuxDoorbellErrorV1::Runtime("linear runtime state"));
        }
        Ok(())
    }

    pub(crate) fn validate_active(
        &self,
        kfd: BorrowedFd<'_>,
        opener_pid: u32,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        self.check_binding(kfd, opener_pid)
    }

    pub(crate) fn validate_queue_live_process(
        &self,
        opener_pid: u32,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        if opener_pid != std::process::id() || self.binding.opener_pid != opener_pid {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        if !self.active || self.poisoned {
            return Err(LinuxDoorbellErrorV1::Runtime("linear runtime state"));
        }
        if self.phase != KfdRuntimeLifecyclePhaseV1::QueueLive {
            return Err(LinuxDoorbellErrorV1::Runtime(
                "runtime does not own one live queue",
            ));
        }
        Ok(())
    }

    pub(crate) fn mark_queue_created(&mut self) -> Result<(), LinuxDoorbellErrorV1> {
        self.phase = admit_runtime_transition(
            self.phase,
            KfdRuntimeLifecyclePhaseV1::EnabledBeforeQueue,
            KfdRuntimeLifecyclePhaseV1::QueueLive,
        )?;
        Ok(())
    }

    pub(crate) fn mark_queue_destroyed(&mut self) -> Result<(), LinuxDoorbellErrorV1> {
        self.phase = admit_runtime_transition(
            self.phase,
            KfdRuntimeLifecyclePhaseV1::QueueLive,
            KfdRuntimeLifecyclePhaseV1::QueueDestroyed,
        )?;
        Ok(())
    }

    pub(crate) fn mark_event_destroyed(&mut self) -> Result<(), LinuxDoorbellErrorV1> {
        self.phase = admit_runtime_transition(
            self.phase,
            KfdRuntimeLifecyclePhaseV1::QueueDestroyed,
            KfdRuntimeLifecyclePhaseV1::EventDestroyed,
        )?;
        Ok(())
    }

    pub(crate) fn disable(
        self,
        kfd: BorrowedFd<'_>,
        opener_pid: u32,
    ) -> Result<LinuxKfdRuntimeDisabledV1, LinuxDoorbellErrorV1> {
        self.disable_at_phase(kfd, opener_pid, KfdRuntimeLifecyclePhaseV1::EventDestroyed)
    }

    pub(crate) fn disable_debug_target(
        self,
        kfd: BorrowedFd<'_>,
        opener_pid: u32,
    ) -> Result<LinuxKfdRuntimeDisabledV1, LinuxDoorbellErrorV1> {
        self.disable_at_phase(
            kfd,
            opener_pid,
            KfdRuntimeLifecyclePhaseV1::EnabledBeforeQueue,
        )
    }

    fn disable_at_phase(
        mut self,
        kfd: BorrowedFd<'_>,
        opener_pid: u32,
        required_phase: KfdRuntimeLifecyclePhaseV1,
    ) -> Result<LinuxKfdRuntimeDisabledV1, LinuxDoorbellErrorV1> {
        if let Err(error) = self.check_binding(kfd, opener_pid) {
            self.poisoned = true;
            return Err(error);
        }
        if self.phase != required_phase {
            self.poisoned = true;
            return Err(LinuxDoorbellErrorV1::Runtime(
                "disable before queue and event destruction",
            ));
        }
        let mut gate = lock_runtime_gate_v1(&KFD_RUNTIME_GATE);
        let requires_kernel_disable = match gate.runtime.release_plan(opener_pid) {
            Ok(plan) => plan,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        if !requires_kernel_disable {
            self.active = false;
            self.phase = admit_runtime_transition(
                self.phase,
                required_phase,
                KfdRuntimeLifecyclePhaseV1::Disabled,
            )?;
            return Ok(LinuxKfdRuntimeDisabledV1 {
                binding: self.binding,
                completion_pending: true,
            });
        }
        let expected = KfdIoctlRuntimeEnableArgsV1::new_queue_exception_disable();
        let mut args = expected;
        // SAFETY: exact pointer-free 16-byte transition record.
        let request = unsafe { Updater::<RUNTIME_ENABLE_OPCODE, _>::new(&mut args) };
        // SAFETY: matching process-bound fd and exclusive record. Any error is
        // ambiguous, poisons the global owner, and permits no later cleanup.
        if let Err(source) = unsafe { rustix::ioctl::ioctl(kfd, request) } {
            self.poisoned = true;
            gate.poison();
            return Err(LinuxDoorbellErrorV1::RuntimeSyscall {
                operation: "AMDKFD_IOC_RUNTIME_ENABLE(disable)",
                source,
            });
        }
        if args != expected || !args.is_exact_queue_exception_disable() {
            self.poisoned = true;
            gate.poison();
            return Err(LinuxDoorbellErrorV1::Runtime(
                "RUNTIME_ENABLE disable output drift",
            ));
        }
        gate.runtime.commit_last_disabled();
        self.active = false;
        self.phase = admit_runtime_transition(
            self.phase,
            required_phase,
            KfdRuntimeLifecyclePhaseV1::Disabled,
        )?;
        Ok(LinuxKfdRuntimeDisabledV1 {
            binding: self.binding,
            completion_pending: true,
        })
    }
}

impl Drop for LinuxKfdRuntimeEnabledV1 {
    fn drop(&mut self) {
        if self.active || self.poisoned {
            permanently_poison_process_global_kfd_runtime_gate_v1();
        }
        // Deliberately no implicit ioctl.
    }
}

impl LinuxKfdRuntimeDisabledV1 {
    pub(crate) fn complete(mut self) {
        self.completion_pending = false;
    }
}

impl Drop for LinuxKfdRuntimeDisabledV1 {
    fn drop(&mut self) {
        if self.completion_pending {
            permanently_poison_process_global_kfd_runtime_gate_v1();
        }
        // Deliberately no implicit ioctl.
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct QueueExceptionBindingV1 {
    event_id: KfdSignalEventIdV1,
    opener_pid: u32,
    raw_fd: i32,
}

pub(crate) struct LinuxQueueExceptionEventV1 {
    binding: QueueExceptionBindingV1,
    active: bool,
    poisoned: bool,
    observation_used: bool,
}

pub(crate) struct LinuxDestroyedQueueExceptionEventV1 {
    binding: QueueExceptionBindingV1,
}

impl LinuxQueueExceptionEventV1 {
    pub(crate) fn create(
        kfd: BorrowedFd<'_>,
        opener_pid: u32,
    ) -> Result<Self, LinuxDoorbellErrorV1> {
        if opener_pid != std::process::id() || kfd.as_raw_fd() < 0 {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        let mut args = KfdIoctlCreateEventArgsV1::new_queue_exception_signal(None);
        // SAFETY: event layout/opcode are frozen by the independent KFD 1.18
        // oracle and the initialized exclusive record spans the complete call.
        let request = unsafe { Updater::<CREATE_EVENT_OPCODE, _>::new(&mut args) };
        // SAFETY: the retained fd and exclusive in/out record satisfy the
        // request contract. Every output remains untrusted until admission.
        unsafe { rustix::ioctl::ioctl(kfd, request) }.map_err(|source| {
            LinuxDoorbellErrorV1::EventSyscall {
                operation: "AMDKFD_IOC_CREATE_EVENT",
                source,
            }
        })?;
        let observation = args
            .admit_first_internal_queue_exception_signal_output()
            .map_err(|_| LinuxDoorbellErrorV1::Event("CREATE_EVENT output"))?;
        Ok(Self {
            binding: QueueExceptionBindingV1 {
                event_id: observation.id(),
                opener_pid,
                raw_fd: kfd.as_raw_fd(),
            },
            active: true,
            poisoned: false,
            observation_used: false,
        })
    }

    pub(crate) fn event_id_observation(&self) -> u32 {
        self.binding.event_id.get()
    }

    fn check_binding(
        &self,
        kfd: BorrowedFd<'_>,
        opener_pid: u32,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        if opener_pid != std::process::id()
            || self.binding.opener_pid != opener_pid
            || self.binding.raw_fd != kfd.as_raw_fd()
        {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        if !self.active || self.poisoned {
            return Err(LinuxDoorbellErrorV1::Event("linear event state"));
        }
        Ok(())
    }

    pub(crate) fn validate_live_with_shadows(
        &self,
        kfd: BorrowedFd<'_>,
        opener_pid: u32,
        shadows: &LinuxCwsrShadowPagesV1,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        self.check_binding(kfd, opener_pid)?;
        if !shadows.matches(self.binding) {
            return Err(LinuxDoorbellErrorV1::Event("event/shadow substitution"));
        }
        shadows.validate_readback()
    }

    pub(crate) fn validate_live_with_shadows_for_diagnostic(
        &self,
        kfd: BorrowedFd<'_>,
        opener_pid: u32,
        shadows: &LinuxCwsrShadowPagesV1,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        self.check_binding(kfd, opener_pid)?;
        if !shadows.matches(self.binding) {
            return Err(LinuxDoorbellErrorV1::Event("event/shadow substitution"));
        }
        shadows.validate_structural_readback()
    }

    #[allow(dead_code)]
    pub(crate) fn wait_and_observe(
        &mut self,
        kfd: BorrowedFd<'_>,
        opener_pid: u32,
        shadows: &LinuxCwsrShadowPagesV1,
        timeout_ms: u32,
    ) -> Result<QueueExceptionWaitObservationV1, LinuxDoorbellErrorV1> {
        if timeout_ms > MAX_QUEUE_EXCEPTION_WAIT_MS {
            self.poisoned = true;
            return Err(LinuxDoorbellErrorV1::Event("wait timeout bound"));
        }
        if let Err(error) = begin_one_shot_observation(&mut self.observation_used) {
            self.poisoned = true;
            return Err(error);
        }
        if let Err(error) = self.check_binding(kfd, opener_pid) {
            self.poisoned = true;
            return Err(error);
        }
        if !shadows.matches(self.binding) {
            self.poisoned = true;
            return Err(LinuxDoorbellErrorV1::Event("event/shadow substitution"));
        }
        let mut event_data = KfdEventDataV1::new_signal(self.binding.event_id, 0);
        let address = match KfdEventDataArrayAddressV1::new(
            (&mut event_data as *mut KfdEventDataV1) as usize as u64,
            1,
        ) {
            Some(address) => address,
            None => {
                self.poisoned = true;
                return Err(LinuxDoorbellErrorV1::Event("event-data address"));
            }
        };
        let mut args = KfdIoctlWaitEventsArgsV1::new_one_signal(address, timeout_ms);
        // SAFETY: the exact wait record and nested single event_data value stay
        // live and exclusively located for the complete ioctl.
        let request = unsafe { Updater::<WAIT_EVENTS_OPCODE, _>::new(&mut args) };
        // SAFETY: the nested pointer, count, opcode, and exclusive output
        // borrow are established above. Error is treated as ambiguous.
        if let Err(source) = unsafe { rustix::ioctl::ioctl(kfd, request) } {
            self.poisoned = true;
            return Err(LinuxDoorbellErrorV1::EventSyscall {
                operation: "AMDKFD_IOC_WAIT_EVENTS",
                source,
            });
        }
        let wait = match args.admit_successful_result(address, timeout_ms) {
            Ok(wait) => wait,
            Err(_) => {
                self.poisoned = true;
                return Err(LinuxDoorbellErrorV1::Event("WAIT_EVENTS output"));
            }
        };
        let reason = match shadows.observe_reason() {
            Ok(reason) => reason,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        match admit_queue_exception_wait(wait, reason) {
            Ok(observation) => {
                // A bounded wait and payload read are not an atomic absence
                // proof. The one-shot observation is terminal either way.
                self.poisoned = true;
                Ok(observation)
            }
            Err(error) => {
                self.poisoned = true;
                Err(error)
            }
        }
    }

    pub(crate) fn destroy(
        mut self,
        kfd: BorrowedFd<'_>,
        opener_pid: u32,
    ) -> Result<LinuxDestroyedQueueExceptionEventV1, LinuxDoorbellErrorV1> {
        self.check_binding(kfd, opener_pid)?;
        let args = KfdIoctlDestroyEventArgsV1::new(self.binding.event_id);
        // SAFETY: Setter must own the exact 8-byte record. Passing `&args`
        // would make its input type a reference and send reference bytes.
        let request = unsafe { Setter::<DESTROY_EVENT_OPCODE, _>::new(args) };
        // SAFETY: the retained matching fd and input record satisfy the
        // reviewed request. Any error is ambiguous and no cleanup follows.
        unsafe { rustix::ioctl::ioctl(kfd, request) }.map_err(|source| {
            LinuxDoorbellErrorV1::EventSyscall {
                operation: "AMDKFD_IOC_DESTROY_EVENT",
                source,
            }
        })?;
        self.active = false;
        Ok(LinuxDestroyedQueueExceptionEventV1 {
            binding: self.binding,
        })
    }
}

impl Drop for LinuxQueueExceptionEventV1 {
    fn drop(&mut self) {
        // Deliberately no implicit event ioctl.
    }
}

pub(crate) struct LinuxCwsrShadowPagesV1 {
    pages: [NonNull<c_void>; GFX942_CWSR_SHADOW_PAGES_V1],
    payload_page: NonNull<c_void>,
    payload: NonNull<u64>,
    binding: QueueExceptionBindingV1,
    page_bytes: usize,
    payload_page_active: bool,
    active: bool,
}

pub(crate) struct LinuxUnpublishedCwsrShadowPagesV1 {
    shadows: Option<LinuxCwsrShadowPagesV1>,
}

pub(crate) struct LinuxCwsrShadowsAfterEventDestroyedV1 {
    shadows: LinuxCwsrShadowPagesV1,
}

pub(crate) struct LinuxCwsrShadowsReadyForReleaseV1 {
    shadows: LinuxCwsrShadowPagesV1,
    runtime: LinuxKfdRuntimeDisabledV1,
}

impl LinuxCwsrShadowPagesV1 {
    pub(crate) fn install(
        plan: CwsrShadowPlanV1,
        event: &LinuxQueueExceptionEventV1,
    ) -> Result<LinuxUnpublishedCwsrShadowPagesV1, LinuxDoorbellErrorV1> {
        use crate::queue::submit::{
            CWSR_HEADER_BYTES, GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1, GFX942_CWSR_XCC_COUNT_V1,
            gfx942_cwsr_header_bytes,
        };
        if !event.active || event.poisoned || event.binding.opener_pid != std::process::id() {
            return Err(LinuxDoorbellErrorV1::Event("shadow event state"));
        }
        if plan.page_bytes != 4096
            || crate::GFX942_CONTROL_STACK_BYTES_PER_XCC_V1 as usize
                != CWSR_CONTROL_STACK_PAGES_PER_XCC_V1 * plan.page_bytes
        {
            return Err(LinuxDoorbellErrorV1::Shadow("control-stack page geometry"));
        }
        let mut pages = Vec::with_capacity(GFX942_CWSR_SHADOW_PAGES_V1);
        for xcc in 0..GFX942_CWSR_XCC_COUNT_V1 {
            let xcc_offset = xcc
                .checked_mul(GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1)
                .ok_or(LinuxDoorbellErrorV1::Shadow("XCC offset"))?;
            for control_page in 0..CWSR_CONTROL_STACK_PAGES_PER_XCC_V1 {
                let offset = control_page
                    .checked_mul(plan.page_bytes)
                    .and_then(|offset| xcc_offset.checked_add(offset))
                    .ok_or(LinuxDoorbellErrorV1::Shadow("control-stack page offset"))?;
                let requested = plan
                    .base
                    .checked_add(offset as u64)
                    .ok_or(LinuxDoorbellErrorV1::Shadow("control-stack page address"))?;
                let pointer = usize::try_from(requested)
                    .map_err(|_| LinuxDoorbellErrorV1::Shadow("control-stack address width"))?
                    as *mut c_void;
                // SAFETY: the plan was minted only for this exact owned PROT_NONE
                // reservation. MAP_FIXED replaces exactly one page and starts it
                // PROT_NONE, without exposing a setup window.
                let mapped = unsafe {
                    rustix::mm::mmap_anonymous(
                        pointer,
                        plan.page_bytes,
                        ProtFlags::empty(),
                        MapFlags::PRIVATE | MapFlags::FIXED | MapFlags::NORESERVE,
                    )
                }
                .map_err(|source| LinuxDoorbellErrorV1::ShadowSyscall {
                    operation: "mmap fixed CWSR control-stack shadow page",
                    source,
                })?;
                if mapped != pointer {
                    std::process::abort();
                }
                let page = NonNull::new(mapped).ok_or(LinuxDoorbellErrorV1::Shadow("zero page"))?;
                // SAFETY: exact new page remains PROT_NONE and exclusively owned.
                unsafe {
                    rustix::mm::madvise(page.as_ptr(), plan.page_bytes, Advice::LinuxDontFork)
                }
                .map_err(|source| LinuxDoorbellErrorV1::ShadowSyscall {
                    operation: "madvise CWSR shadow MADV_DONTFORK",
                    source,
                })?;
                // SAFETY: DONTFORK is installed before the page gains access.
                unsafe {
                    rustix::mm::mprotect(
                        page.as_ptr(),
                        plan.page_bytes,
                        MprotectFlags::READ | MprotectFlags::WRITE,
                    )
                }
                .map_err(|source| LinuxDoorbellErrorV1::ShadowSyscall {
                    operation: "mprotect CWSR shadow read/write",
                    source,
                })?;
                pages.push(page);
            }
        }
        let pages: [NonNull<c_void>; GFX942_CWSR_SHADOW_PAGES_V1] = pages
            .try_into()
            .map_err(|_| LinuxDoorbellErrorV1::Shadow("shadow page count"))?;
        let payload_page = map_cwsr_payload_page(plan.page_bytes)?;
        let setup = (|| {
            let payload_pointer = payload_page.as_ptr().cast::<u64>();
            let payload = NonNull::new(payload_pointer)
                .ok_or(LinuxDoorbellErrorV1::Shadow("payload address"))?;
            let payload_observation =
                KfdQueueExceptionPayloadAddressV1::new(payload.as_ptr() as usize as u64)
                    .ok_or(LinuxDoorbellErrorV1::Shadow("payload admission"))?;
            // SAFETY: the exact aligned payload word is exclusively owned and
            // was not previously initialized as a Rust object.
            unsafe { core::ptr::write_volatile(payload.as_ptr(), 0_u64) };
            for xcc in 0..GFX942_CWSR_XCC_COUNT_V1 {
                let page = pages[xcc * CWSR_CONTROL_STACK_PAGES_PER_XCC_V1];
                let header =
                    gfx942_cwsr_header_bytes(xcc, payload_observation, event.binding.event_id)
                        .map_err(|_| LinuxDoorbellErrorV1::Shadow("typed header"))?;
                debug_assert_eq!(header.len(), CWSR_HEADER_BYTES);
                // SAFETY: each exact page has at least 40 writable bytes and
                // no reference or pointer escapes this boundary.
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        header.as_ptr(),
                        page.as_ptr().cast::<u8>(),
                        CWSR_HEADER_BYTES,
                    )
                };
            }
            Ok(payload)
        })();
        let payload = match setup {
            Ok(payload) => payload,
            Err(error) => {
                discard_unpublished_cwsr_payload_page_or_abort(payload_page, plan.page_bytes);
                return Err(error);
            }
        };
        let owner = Self {
            pages,
            payload_page,
            payload,
            binding: event.binding,
            page_bytes: plan.page_bytes,
            payload_page_active: true,
            active: true,
        };
        admit_installed_cwsr_shadows(owner).map(|shadows| LinuxUnpublishedCwsrShadowPagesV1 {
            shadows: Some(shadows),
        })
    }

    /// Restores write access required by KFD's documented suspend operation.
    ///
    /// The executable BO seal covers the whole CPU range, including the
    /// anonymous control-stack shadows. KFD writes the context header and the
    /// used control-stack bytes through this target VMA after suspension.
    pub(crate) fn restore_kernel_write_access_after_bo_seal(
        &self,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        if !self.active
            || !self.payload_page_active
            || self.binding.opener_pid != std::process::id()
        {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        for page in self.pages {
            // SAFETY: each pointer names one exact retained anonymous shadow
            // page. No Rust reference spans the protection transition.
            unsafe {
                rustix::mm::mprotect(
                    page.as_ptr(),
                    self.page_bytes,
                    MprotectFlags::READ | MprotectFlags::WRITE,
                )
            }
            .map_err(|source| LinuxDoorbellErrorV1::ShadowSyscall {
                operation: "restore CWSR shadow read/write after BO seal",
                source,
            })?;
        }
        self.validate_readback()
    }

    pub(crate) fn initialize_and_validate_bo_headers(
        &self,
        bytes: &mut [u8],
    ) -> Result<(), crate::queue::submit::NativeAqlSubmissionErrorV1> {
        let payload = KfdQueueExceptionPayloadAddressV1::new(self.payload.as_ptr() as usize as u64)
            .ok_or(
                crate::queue::submit::NativeAqlSubmissionErrorV1::InvalidCwsr("payload readback"),
            )?;
        crate::queue::submit::initialize_gfx942_cwsr_headers(
            bytes,
            payload,
            self.binding.event_id,
        )?;
        for xcc in 0..crate::queue::submit::GFX942_CWSR_XCC_COUNT_V1 {
            let offset = xcc * crate::queue::submit::GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1;
            let expected = crate::queue::submit::gfx942_cwsr_header_bytes(
                xcc,
                payload,
                self.binding.event_id,
            )?;
            if bytes.get(offset..offset + expected.len()) != Some(expected.as_slice()) {
                return Err(
                    crate::queue::submit::NativeAqlSubmissionErrorV1::InvalidCwsr(
                        "BO header readback",
                    ),
                );
            }
        }
        self.validate_readback().map_err(|_| {
            crate::queue::submit::NativeAqlSubmissionErrorV1::InvalidCwsr("shadow readback")
        })
    }

    fn matches(&self, binding: QueueExceptionBindingV1) -> bool {
        self.active && self.binding == binding && self.binding.opener_pid == std::process::id()
    }

    fn validate_structural_readback(&self) -> Result<(), LinuxDoorbellErrorV1> {
        if !self.payload_page_active
            || self.payload.as_ptr().cast::<c_void>() != self.payload_page.as_ptr()
        {
            return Err(LinuxDoorbellErrorV1::Shadow("payload page binding"));
        }
        let payload_address = self.payload.as_ptr() as usize;
        for page in self.pages {
            let start = page.as_ptr() as usize;
            let end = start
                .checked_add(self.page_bytes)
                .ok_or(LinuxDoorbellErrorV1::Shadow("shadow page range"))?;
            if (start..end).contains(&payload_address) {
                return Err(LinuxDoorbellErrorV1::Shadow(
                    "payload aliases control stack",
                ));
            }
        }
        let payload = KfdQueueExceptionPayloadAddressV1::new(self.payload.as_ptr() as usize as u64)
            .ok_or(LinuxDoorbellErrorV1::Shadow("payload readback"))?;
        for xcc in 0..crate::queue::submit::GFX942_CWSR_XCC_COUNT_V1 {
            let page = self.pages[xcc * CWSR_CONTROL_STACK_PAGES_PER_XCC_V1];
            let expected =
                crate::queue::submit::gfx942_cwsr_header_bytes(xcc, payload, self.binding.event_id)
                    .map_err(|_| LinuxDoorbellErrorV1::Shadow("typed header readback"))?;
            for (index, expected_byte) in expected.iter().enumerate() {
                // SAFETY: exact owned live page and bounded header byte.
                let observed =
                    unsafe { core::ptr::read_volatile(page.as_ptr().cast::<u8>().add(index)) };
                if observed != *expected_byte {
                    return Err(LinuxDoorbellErrorV1::Shadow("header readback"));
                }
            }
        }
        Ok(())
    }

    fn validate_readback(&self) -> Result<(), LinuxDoorbellErrorV1> {
        self.validate_structural_readback()?;
        if self.observe_reason()?.get() != 0 {
            return Err(LinuxDoorbellErrorV1::Shadow("initial payload"));
        }
        Ok(())
    }

    pub(crate) fn observe_reason(&self) -> Result<KfdQueueExceptionReasonV1, LinuxDoorbellErrorV1> {
        if !self.active
            || !self.payload_page_active
            || self.binding.opener_pid != std::process::id()
        {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        // SAFETY: the aligned payload remains in the separately owned live page.
        let observed = u64::from_le(unsafe { core::ptr::read_volatile(self.payload.as_ptr()) });
        KfdQueueExceptionReasonV1::from_untrusted_wire(observed)
            .ok_or(LinuxDoorbellErrorV1::Shadow("queue exception reason"))
    }

    pub(crate) fn after_event_destroy(
        mut self,
        destroyed: LinuxDestroyedQueueExceptionEventV1,
    ) -> Result<LinuxCwsrShadowsAfterEventDestroyedV1, LinuxDoorbellErrorV1> {
        if !self.matches(destroyed.binding) {
            return Err(LinuxDoorbellErrorV1::Shadow("destroyed event substitution"));
        }
        self.active = false;
        if self.release_payload_page().is_err() {
            // The event is already destroyed and `self` is consumed. Returning
            // would discard the only owner of a possibly mapped payload page.
            std::process::abort();
        }
        Ok(LinuxCwsrShadowsAfterEventDestroyedV1 { shadows: self })
    }

    fn release_payload_page(&mut self) -> Result<(), LinuxDoorbellErrorV1> {
        if !self.payload_page_active || self.binding.opener_pid != std::process::id() {
            return Err(LinuxDoorbellErrorV1::Shadow("payload release state"));
        }
        zero_protect_unmap_cwsr_payload_page(self.payload_page, self.page_bytes)?;
        self.payload_page_active = false;
        Ok(())
    }

    #[cfg(feature = "live-validation")]
    pub(crate) fn verify_dontfork_child_negative(&self) -> Result<(), LinuxDoorbellErrorV1> {
        if !self.active
            || !self.payload_page_active
            || self.binding.opener_pid != std::process::id()
        {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        let tasks = std::fs::read_dir("/proc/self/task")
            .map_err(|_| LinuxDoorbellErrorV1::ChildProbe("read /proc/self/task"))?
            .take(2)
            .count();
        if tasks != 1 {
            return Err(LinuxDoorbellErrorV1::IsolationRequired);
        }
        // SAFETY: isolated child probes VMA existence only, then exits.
        match unsafe { rustix::runtime::kernel_fork() }.map_err(|source| {
            LinuxDoorbellErrorV1::ShadowSyscall {
                operation: "fork CWSR shadow DONTFORK probe",
                source,
            }
        })? {
            rustix::runtime::Fork::Child(_) => {
                let mut residency = [0_u8; 1];
                for page in self
                    .pages
                    .iter()
                    .copied()
                    .chain(core::iter::once(self.payload_page))
                {
                    // SAFETY: mincore does not dereference an absent child VMA.
                    let result =
                        unsafe { mincore(page.as_ptr(), self.page_bytes, residency.as_mut_ptr()) };
                    if result != -1 || std::io::Error::last_os_error().raw_os_error() != Some(12) {
                        rustix::runtime::exit_group(if result == 0 { 1 } else { 2 });
                    }
                }
                rustix::runtime::exit_group(0);
            }
            rustix::runtime::Fork::ParentOf(child) => {
                let (_, status) =
                    rustix::process::waitpid(Some(child), rustix::process::WaitOptions::empty())
                        .map_err(|source| LinuxDoorbellErrorV1::ShadowSyscall {
                            operation: "wait CWSR shadow DONTFORK probe",
                            source,
                        })?
                        .ok_or(LinuxDoorbellErrorV1::ChildProbe("child was not waitable"))?;
                match status.exit_status() {
                    Some(0) => Ok(()),
                    Some(1) => Err(LinuxDoorbellErrorV1::ShadowMappingInherited),
                    _ => Err(LinuxDoorbellErrorV1::ChildProbe("child mincore protocol")),
                }
            }
        }
    }
}

impl LinuxCwsrShadowsAfterEventDestroyedV1 {
    pub(crate) fn after_runtime_destroy(
        self,
        runtime: LinuxKfdRuntimeDisabledV1,
    ) -> Result<LinuxCwsrShadowsReadyForReleaseV1, LinuxDoorbellErrorV1> {
        if self.shadows.active
            || self.shadows.payload_page_active
            || self.shadows.binding.opener_pid != std::process::id()
            || runtime.binding.opener_pid != self.shadows.binding.opener_pid
        {
            return Err(LinuxDoorbellErrorV1::Shadow("runtime/shadow release state"));
        }
        Ok(LinuxCwsrShadowsReadyForReleaseV1 {
            shadows: self.shadows,
            runtime,
        })
    }
}

impl LinuxUnpublishedCwsrShadowPagesV1 {
    pub(crate) fn shadows(&self) -> &LinuxCwsrShadowPagesV1 {
        self.shadows
            .as_ref()
            .expect("unpublished CWSR shadow custody is armed")
    }

    pub(crate) fn publish_for_native_queue_creation(mut self) -> LinuxCwsrShadowPagesV1 {
        self.shadows
            .take()
            .expect("unpublished CWSR shadow custody is armed")
    }
}

impl Drop for LinuxUnpublishedCwsrShadowPagesV1 {
    fn drop(&mut self) {
        let Some(mut shadows) = self.shadows.take() else {
            return;
        };
        if shadows.release_payload_page().is_err() {
            std::process::abort();
        }
    }
}

impl LinuxCwsrShadowsReadyForReleaseV1 {
    pub(crate) fn validate_for_release(&self) -> Result<(), LinuxDoorbellErrorV1> {
        let cleanup_is_pending = self.runtime.completion_pending;
        if self.shadows.active
            || !cleanup_is_pending
            || self.shadows.binding.opener_pid != std::process::id()
            || self.shadows.payload_page_active
            || self.runtime.binding.opener_pid != self.shadows.binding.opener_pid
        {
            Err(LinuxDoorbellErrorV1::Shadow("release state"))
        } else {
            Ok(())
        }
    }

    pub(crate) fn complete(self) -> Result<(), LinuxDoorbellErrorV1> {
        self.validate_for_release()?;
        self.runtime.complete();
        Ok(())
    }
}

impl Drop for LinuxCwsrShadowPagesV1 {
    fn drop(&mut self) {
        // No implicit unmap. Published payload release occurs explicitly just
        // after event destruction; the full reservation has its own owner.
    }
}

fn admit_installed_cwsr_shadows(
    mut owner: LinuxCwsrShadowPagesV1,
) -> Result<LinuxCwsrShadowPagesV1, LinuxDoorbellErrorV1> {
    if let Err(error) = owner.validate_readback() {
        if owner.release_payload_page().is_err() {
            std::process::abort();
        }
        return Err(error);
    }
    Ok(owner)
}

fn zero_protect_unmap_cwsr_payload_page(
    payload_page: NonNull<c_void>,
    page_bytes: usize,
) -> Result<(), LinuxDoorbellErrorV1> {
    // SAFETY: the payload occupies the first aligned u64 of the exact retained
    // private page, which is still writable at this transition.
    unsafe { core::ptr::write_volatile(payload_page.as_ptr().cast::<u64>(), 0_u64) };
    // SAFETY: no reference spans the exact page protection transition.
    unsafe { rustix::mm::mprotect(payload_page.as_ptr(), page_bytes, MprotectFlags::empty()) }
        .map_err(|source| LinuxDoorbellErrorV1::ShadowSyscall {
            operation: "mprotect CWSR payload inaccessible",
            source,
        })?;
    // SAFETY: the exact mapping is linearly owned and no pointer escapes.
    unsafe { rustix::mm::munmap(payload_page.as_ptr(), page_bytes) }.map_err(|source| {
        LinuxDoorbellErrorV1::ShadowSyscall {
            operation: "munmap CWSR payload page",
            source,
        }
    })
}

fn discard_unpublished_cwsr_payload_page_or_abort(
    payload_page: NonNull<c_void>,
    page_bytes: usize,
) {
    if zero_protect_unmap_cwsr_payload_page(payload_page, page_bytes).is_err() {
        std::process::abort();
    }
}

fn map_cwsr_payload_page(page_bytes: usize) -> Result<NonNull<c_void>, LinuxDoorbellErrorV1> {
    if page_bytes != 4096 {
        return Err(LinuxDoorbellErrorV1::Shadow("payload page geometry"));
    }
    // SAFETY: this creates a fresh private inaccessible page with no alias.
    let mapped = unsafe {
        rustix::mm::mmap_anonymous(
            core::ptr::null_mut(),
            page_bytes,
            ProtFlags::empty(),
            MapFlags::PRIVATE | MapFlags::NORESERVE,
        )
    }
    .map_err(|source| LinuxDoorbellErrorV1::ShadowSyscall {
        operation: "mmap CWSR payload page",
        source,
    })?;
    let payload_page =
        NonNull::new(mapped).ok_or(LinuxDoorbellErrorV1::Shadow("zero payload page"))?;
    let cleanup_or_abort = || {
        // SAFETY: setup has not published the fresh mapping.
        if unsafe { rustix::mm::munmap(payload_page.as_ptr(), page_bytes) }.is_err() {
            std::process::abort();
        }
    };
    // SAFETY: the mapping is fresh, inaccessible, and not published.
    if let Err(source) = unsafe { rustix::mm::madvise(mapped, page_bytes, Advice::LinuxDontFork) } {
        cleanup_or_abort();
        return Err(LinuxDoorbellErrorV1::ShadowSyscall {
            operation: "madvise CWSR payload MADV_DONTFORK",
            source,
        });
    }
    // SAFETY: DONTFORK is installed before access is granted.
    if let Err(source) = unsafe {
        rustix::mm::mprotect(
            payload_page.as_ptr(),
            page_bytes,
            MprotectFlags::READ | MprotectFlags::WRITE,
        )
    } {
        cleanup_or_abort();
        return Err(LinuxDoorbellErrorV1::ShadowSyscall {
            operation: "mprotect CWSR payload read/write",
            source,
        });
    }
    Ok(payload_page)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DoorbellMmapPlanV1 {
    encoded_slice_offset: u64,
    queue_byte_offset: u64,
    slice_bytes: usize,
}

fn doorbell_mmap_plan(
    outputs: KfdGfx942CreateQueueOutputs,
) -> Result<DoorbellMmapPlanV1, LinuxDoorbellErrorV1> {
    let page_size = rustix::param::page_size();
    let slice_bytes = usize::try_from(KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES)
        .map_err(|_| LinuxDoorbellErrorV1::InvalidObservation("slice length"))?;
    if page_size != 4096 || !slice_bytes.is_multiple_of(page_size) {
        return Err(LinuxDoorbellErrorV1::UnsupportedPageSize(page_size));
    }
    let observation = outputs.doorbell_offset();
    let encoded_slice_offset = observation.encoded_process_slice_offset();
    let queue_byte_offset = observation.in_process_byte_offset();
    if !encoded_slice_offset.is_multiple_of(KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES)
        || queue_byte_offset
            .checked_add(KFD_GFX942_DOORBELL_BYTES)
            .is_none_or(|end| end > KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES)
        || !queue_byte_offset.is_multiple_of(KFD_GFX942_DOORBELL_BYTES)
        || observation.raw() != (encoded_slice_offset | queue_byte_offset)
    {
        return Err(LinuxDoorbellErrorV1::InvalidObservation(
            "encoded whole-slice offset",
        ));
    }
    Ok(DoorbellMmapPlanV1 {
        encoded_slice_offset,
        queue_byte_offset,
        slice_bytes,
    })
}

/// Non-Clone, non-address-exposing ownership of one complete process slice.
pub(super) struct LinuxDoorbellSliceV1 {
    address: NonNull<c_void>,
    plan: DoorbellMmapPlanV1,
    opener_pid: u32,
    active: bool,
}

impl LinuxDoorbellSliceV1 {
    pub(super) fn map(
        kfd: BorrowedFd<'_>,
        outputs: KfdGfx942CreateQueueOutputs,
        opener_pid: u32,
    ) -> Result<Self, LinuxDoorbellErrorV1> {
        if opener_pid != std::process::id() {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        let plan = doorbell_mmap_plan(outputs)?;
        // SAFETY: the admitted KFD output fixes the mmap type/GPU hash and
        // complete 8192-byte slice offset. PROT_NONE prevents MMIO access
        // before the mandatory DONTFORK ordering point.
        let mapped = unsafe {
            rustix::mm::mmap(
                core::ptr::null_mut(),
                plan.slice_bytes,
                ProtFlags::empty(),
                MapFlags::SHARED,
                kfd,
                plan.encoded_slice_offset,
            )
        }
        .map_err(|source| LinuxDoorbellErrorV1::Syscall {
            operation: "mmap complete KFD doorbell slice",
            source,
        })?;
        let Some(address) = NonNull::new(mapped) else {
            // SAFETY: this is the exact successful mapping. Returning address
            // zero is outside the admitted process profile.
            if unsafe { rustix::mm::munmap(mapped, plan.slice_bytes) }.is_err() {
                std::process::abort();
            }
            return Err(LinuxDoorbellErrorV1::InvalidObservation(
                "doorbell VMA address",
            ));
        };
        let cleanup_or_abort = || {
            // SAFETY: no MMIO access was enabled and the exact VMA is owned.
            if unsafe { rustix::mm::munmap(address.as_ptr(), plan.slice_bytes) }.is_err() {
                std::process::abort();
            }
        };
        // SAFETY: the exact mapping is exclusively owned and still PROT_NONE.
        if let Err(source) = unsafe {
            rustix::mm::madvise(address.as_ptr(), plan.slice_bytes, Advice::LinuxDontFork)
        } {
            cleanup_or_abort();
            return Err(LinuxDoorbellErrorV1::Syscall {
                operation: "madvise doorbell MADV_DONTFORK",
                source,
            });
        }
        // SAFETY: DONTFORK is installed, no reference exists, and the mapping
        // covers exactly the reviewed process slice. No safe MMIO accessor is
        // exposed by this capability.
        if let Err(source) = unsafe {
            rustix::mm::mprotect(
                address.as_ptr(),
                plan.slice_bytes,
                MprotectFlags::READ | MprotectFlags::WRITE,
            )
        } {
            cleanup_or_abort();
            return Err(LinuxDoorbellErrorV1::Syscall {
                operation: "mprotect doorbell read/write",
                source,
            });
        }
        Ok(Self {
            address,
            plan,
            opener_pid,
            active: true,
        })
    }

    pub(super) const fn slice_bytes(&self) -> usize {
        self.plan.slice_bytes
    }

    pub(super) const fn queue_byte_offset(&self) -> u64 {
        self.plan.queue_byte_offset
    }

    pub(super) fn store_packet_id_release(
        &mut self,
        packet_id: u64,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        if self.opener_pid != std::process::id() {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        if !self.active {
            return Err(LinuxDoorbellErrorV1::InvalidObservation(
                "doorbell store state",
            ));
        }
        let offset = usize::try_from(self.plan.queue_byte_offset)
            .map_err(|_| LinuxDoorbellErrorV1::InvalidObservation("doorbell store offset"))?;
        let end = offset.checked_add(core::mem::size_of::<u64>()).ok_or(
            LinuxDoorbellErrorV1::InvalidObservation("doorbell store range"),
        )?;
        if end > self.plan.slice_bytes || !offset.is_multiple_of(core::mem::align_of::<u64>()) {
            return Err(LinuxDoorbellErrorV1::InvalidObservation(
                "doorbell store range",
            ));
        }
        // This is one deliberately narrow CPU-memory/MMIO ordering boundary:
        // release-published packet bytes precede the WC MMIO notification.
        fence(Ordering::Release);
        #[cfg(target_arch = "x86_64")]
        // SAFETY: SFENCE has no memory operand and is universally available on
        // the admitted x86_64 platform.
        unsafe {
            core::arch::x86_64::_mm_sfence();
        }
        // SAFETY: the retained capability uniquely owns the live complete
        // process slice, the checked offset selects its exact aligned 8-byte
        // queue doorbell, and no pointer or reference escapes this call.
        unsafe {
            core::ptr::write_volatile(
                self.address.as_ptr().cast::<u8>().add(offset).cast::<u64>(),
                packet_id.to_le(),
            );
        }
        Ok(())
    }

    pub(super) fn release(mut self) -> Result<(), LinuxDoorbellErrorV1> {
        if self.opener_pid != std::process::id() {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        if !self.active {
            return Err(LinuxDoorbellErrorV1::InvalidObservation(
                "doorbell release state",
            ));
        }
        // SAFETY: the exact mapping remains linearly owned and no MMIO pointer
        // or reference can escape the capability.
        unsafe { rustix::mm::munmap(self.address.as_ptr(), self.plan.slice_bytes) }.map_err(
            |source| LinuxDoorbellErrorV1::Syscall {
                operation: "munmap complete KFD doorbell slice",
                source,
            },
        )?;
        self.active = false;
        Ok(())
    }

    #[cfg(feature = "live-validation")]
    pub(super) fn verify_dontfork_child_negative(&self) -> Result<(), LinuxDoorbellErrorV1> {
        if self.opener_pid != std::process::id() || !self.active {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        let tasks = std::fs::read_dir("/proc/self/task")
            .map_err(|_| LinuxDoorbellErrorV1::ChildProbe("read /proc/self/task"))?
            .take(2)
            .count();
        if tasks != 1 {
            return Err(LinuxDoorbellErrorV1::IsolationRequired);
        }
        // SAFETY: the isolated child only probes VMA existence and exits. It
        // never reads or writes MMIO and the parent waits synchronously.
        match unsafe { rustix::runtime::kernel_fork() }.map_err(|source| {
            LinuxDoorbellErrorV1::Syscall {
                operation: "fork doorbell DONTFORK probe",
                source,
            }
        })? {
            rustix::runtime::Fork::Child(_) => {
                let mut residency = [0_u8; 2];
                // SAFETY: mincore does not dereference the absent child VMA.
                let result = unsafe {
                    mincore(
                        self.address.as_ptr(),
                        self.plan.slice_bytes,
                        residency.as_mut_ptr(),
                    )
                };
                let code =
                    if result == -1 && std::io::Error::last_os_error().raw_os_error() == Some(12) {
                        0
                    } else if result == 0 {
                        1
                    } else {
                        2
                    };
                rustix::runtime::exit_group(code);
            }
            rustix::runtime::Fork::ParentOf(child) => {
                let (_, status) =
                    rustix::process::waitpid(Some(child), rustix::process::WaitOptions::empty())
                        .map_err(|source| LinuxDoorbellErrorV1::Syscall {
                            operation: "wait doorbell DONTFORK probe",
                            source,
                        })?
                        .ok_or(LinuxDoorbellErrorV1::ChildProbe("child was not waitable"))?;
                match status.exit_status() {
                    Some(0) => Ok(()),
                    Some(1) => Err(LinuxDoorbellErrorV1::MappingInherited),
                    _ => Err(LinuxDoorbellErrorV1::ChildProbe("child mincore protocol")),
                }
            }
        }
    }
}

impl Drop for LinuxDoorbellSliceV1 {
    fn drop(&mut self) {
        // No implicit munmap, MMIO store, or queue operation. Ambiguous or
        // still-live mappings remain inaccessible until process teardown.
    }
}

#[cfg(feature = "live-validation")]
unsafe extern "C" {
    fn mincore(address: *mut c_void, length: usize, residency: *mut u8) -> i32;
}

/// Issues one CREATE_QUEUE call through an exclusively borrowed C-layout
/// record. The returned record and errno remain untrusted.
pub fn create_queue(
    kfd: BorrowedFd<'_>,
    args: &mut KfdIoctlCreateQueueArgs,
) -> Result<(), rustix::io::Errno> {
    // SAFETY: the reviewed x86_64 KFD 1.18 opcode and record layout are fixed
    // by fe2o3-kfd-uapi. The initialized record remains exclusively borrowed
    // for the complete ioctl. Kernel-written fields remain untrusted.
    let request = unsafe { Updater::<CREATE_QUEUE_OPCODE, _>::new(args) };
    // SAFETY: request construction establishes the lifetime and exclusive
    // output borrow. The caller still has to validate every returned field.
    unsafe { rustix::ioctl::ioctl(kfd, request) }.map(|_| ())
}

/// Issues one UPDATE_QUEUE call over a fully initialized input record.
#[allow(dead_code)]
pub fn update_queue(
    kfd: BorrowedFd<'_>,
    args: &KfdIoctlUpdateQueueArgs,
) -> Result<(), rustix::io::Errno> {
    // SAFETY: the exact write-only opcode and initialized input lifetime are
    // fixed here. Numeric queue/address fields are not authority by themselves.
    let request = unsafe { Setter::<UPDATE_QUEUE_OPCODE, _>::new(*args) };
    // SAFETY: the setter owns the exact record for the complete call. Passing
    // the reference itself would send reference bytes rather than UAPI bytes.
    unsafe { rustix::ioctl::ioctl(kfd, request) }.map(|_| ())
}

/// Issues one DESTROY_QUEUE call. The in/out record remains untrusted even
/// though the admitted driver profile does not define a useful output field.
pub fn destroy_queue(
    kfd: BorrowedFd<'_>,
    args: &mut KfdIoctlDestroyQueueArgs,
) -> Result<(), rustix::io::Errno> {
    // SAFETY: the reviewed x86_64 opcode and record layout are fixed, and the
    // initialized record remains exclusively borrowed for the complete call.
    let request = unsafe { Updater::<DESTROY_QUEUE_OPCODE, _>::new(args) };
    // SAFETY: request construction establishes the exclusive in/out borrow.
    unsafe { rustix::ioctl::ioctl(kfd, request) }.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::submit::{
        GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1, GFX942_CWSR_TOTAL_BYTES_V1, GFX942_CWSR_XCC_COUNT_V1,
    };
    use std::os::fd::AsFd;

    const MAPPING_ABSENCE_CHILD_ENV: &str = "FE2O3_TEST_CWSR_MAPPING_ABSENCE_CHILD";

    fn run_mapping_absence_test_in_isolated_process(exact_test_name: &str, test: impl FnOnce()) {
        if std::env::var_os(MAPPING_ABSENCE_CHILD_ENV).is_some() {
            test();
            return;
        }
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg(exact_test_name)
            .arg("--test-threads=1")
            .env(MAPPING_ABSENCE_CHILD_ENV, "1")
            .status()
            .unwrap();
        assert!(status.success(), "isolated mapping-absence test failed");
    }

    type DiagnosticShadowFixture = (
        LinuxCwsrShadowPagesV1,
        Vec<Box<[u8; 4096]>>,
        Box<[u8; 4096]>,
        LinuxQueueExceptionEventV1,
        std::fs::File,
    );

    fn diagnostic_shadow_fixture() -> DiagnosticShadowFixture {
        let file = std::fs::File::open("/dev/null").unwrap();
        let binding = QueueExceptionBindingV1 {
            event_id: KfdSignalEventIdV1::new(7).unwrap(),
            opener_pid: std::process::id(),
            raw_fd: file.as_fd().as_raw_fd(),
        };
        let mut storage: Vec<Box<[u8; 4096]>> = (0..GFX942_CWSR_SHADOW_PAGES_V1)
            .map(|_| Box::new([0_u8; 4096]))
            .collect();
        let mut payload_storage = Box::new([0_u8; 4096]);
        let payload_page = NonNull::new(payload_storage.as_mut_ptr().cast::<c_void>()).unwrap();
        let payload = NonNull::new(payload_storage.as_mut_ptr().cast::<u64>()).unwrap();
        let payload_address =
            KfdQueueExceptionPayloadAddressV1::new(payload.as_ptr() as usize as u64).unwrap();
        for xcc in 0..GFX942_CWSR_XCC_COUNT_V1 {
            let page = &mut storage[xcc * CWSR_CONTROL_STACK_PAGES_PER_XCC_V1];
            let header = crate::queue::submit::gfx942_cwsr_header_bytes(
                xcc,
                payload_address,
                binding.event_id,
            )
            .unwrap();
            page[..header.len()].copy_from_slice(&header);
        }
        let pages: [NonNull<c_void>; GFX942_CWSR_SHADOW_PAGES_V1] = storage
            .iter_mut()
            .map(|page| NonNull::new(page.as_mut_ptr().cast::<c_void>()).unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let shadows = LinuxCwsrShadowPagesV1 {
            pages,
            payload_page,
            payload,
            binding,
            page_bytes: 4096,
            payload_page_active: true,
            active: true,
        };
        let event = LinuxQueueExceptionEventV1 {
            binding,
            active: true,
            poisoned: false,
            observation_used: false,
        };
        (shadows, storage, payload_storage, event, file)
    }

    type MappedDiagnosticShadowFixture = (
        LinuxCwsrShadowPagesV1,
        Vec<Box<[u8; 4096]>>,
        LinuxQueueExceptionEventV1,
        std::fs::File,
    );

    fn mapped_diagnostic_shadow_fixture() -> MappedDiagnosticShadowFixture {
        let file = std::fs::File::open("/dev/null").unwrap();
        let binding = QueueExceptionBindingV1 {
            event_id: KfdSignalEventIdV1::new(7).unwrap(),
            opener_pid: std::process::id(),
            raw_fd: file.as_fd().as_raw_fd(),
        };
        let mut storage: Vec<Box<[u8; 4096]>> = (0..GFX942_CWSR_SHADOW_PAGES_V1)
            .map(|_| Box::new([0_u8; 4096]))
            .collect();
        let payload_page = map_cwsr_payload_page(4096).unwrap();
        let payload = NonNull::new(payload_page.as_ptr().cast::<u64>()).unwrap();
        let payload_address =
            KfdQueueExceptionPayloadAddressV1::new(payload.as_ptr() as usize as u64).unwrap();
        for xcc in 0..GFX942_CWSR_XCC_COUNT_V1 {
            let page = &mut storage[xcc * CWSR_CONTROL_STACK_PAGES_PER_XCC_V1];
            let header = crate::queue::submit::gfx942_cwsr_header_bytes(
                xcc,
                payload_address,
                binding.event_id,
            )
            .unwrap();
            page[..header.len()].copy_from_slice(&header);
        }
        let pages: [NonNull<c_void>; GFX942_CWSR_SHADOW_PAGES_V1] = storage
            .iter_mut()
            .map(|page| NonNull::new(page.as_mut_ptr().cast::<c_void>()).unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let shadows = LinuxCwsrShadowPagesV1 {
            pages,
            payload_page,
            payload,
            binding,
            page_bytes: 4096,
            payload_page_active: true,
            active: true,
        };
        let event = LinuxQueueExceptionEventV1 {
            binding,
            active: true,
            poisoned: false,
            observation_used: false,
        };
        (shadows, storage, event, file)
    }

    fn assert_mapping_absent(address: NonNull<c_void>) {
        let mut residency = [0_u8; 1];
        // SAFETY: mincore only queries whether the saved page address is still
        // mapped and does not dereference it.
        let result = unsafe { libc::mincore(address.as_ptr(), 4096, residency.as_mut_ptr()) };
        assert_eq!(result, -1);
        assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(12));
    }

    #[test]
    fn shadow_plan_is_exact_and_hostile_geometry_fails_closed() {
        let plan = CwsrShadowPlanV1::from_owned_reservation(
            0x1_0000_0000,
            GFX942_CWSR_TOTAL_BYTES_V1,
            4096,
        )
        .unwrap();
        assert_eq!(plan.bytes, 0xb16_7000);
        assert_eq!(plan.page_bytes, 4096);
        let mut addresses = [0_u64; GFX942_CWSR_SHADOW_PAGES_V1];
        for xcc in 0..GFX942_CWSR_XCC_COUNT_V1 {
            for page in 0..CWSR_CONTROL_STACK_PAGES_PER_XCC_V1 {
                let ordinal = xcc * CWSR_CONTROL_STACK_PAGES_PER_XCC_V1 + page;
                addresses[ordinal] =
                    plan.base + (xcc * GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1 + page * 4096) as u64;
                assert!(addresses[ordinal].is_multiple_of(4096));
            }
        }
        assert!(addresses.windows(2).all(|pair| pair[0] < pair[1]));

        for result in [
            CwsrShadowPlanV1::from_owned_reservation(
                plan.base + 1,
                GFX942_CWSR_TOTAL_BYTES_V1,
                4096,
            ),
            CwsrShadowPlanV1::from_owned_reservation(
                plan.base,
                GFX942_CWSR_TOTAL_BYTES_V1 - 4096,
                4096,
            ),
            CwsrShadowPlanV1::from_owned_reservation(plan.base, GFX942_CWSR_TOTAL_BYTES_V1, 8192),
            CwsrShadowPlanV1::from_owned_reservation(
                u64::MAX - 4095,
                GFX942_CWSR_TOTAL_BYTES_V1,
                4096,
            ),
        ] {
            assert!(result.is_err());
        }
    }

    #[test]
    fn unpublished_payload_is_unmapped_when_final_admission_fails() {
        run_mapping_absence_test_in_isolated_process(
            "queue_linux::tests::unpublished_payload_is_unmapped_when_final_admission_fails",
            || {
                let (shadows, mut storage, _event, _file) = mapped_diagnostic_shadow_fixture();
                let payload_page = shadows.payload_page;
                storage[0][0] ^= 1;
                assert!(admit_installed_cwsr_shadows(shadows).is_err());
                assert_mapping_absent(payload_page);
            },
        );
    }

    #[test]
    fn payload_is_unmapped_at_event_destroy_boundary_before_later_cleanup() {
        run_mapping_absence_test_in_isolated_process(
            "queue_linux::tests::payload_is_unmapped_at_event_destroy_boundary_before_later_cleanup",
            || {
                let (shadows, _storage, _event, file) = mapped_diagnostic_shadow_fixture();
                let payload_page = shadows.payload_page;
                let binding = shadows.binding;
                let after_event = shadows
                    .after_event_destroy(LinuxDestroyedQueueExceptionEventV1 { binding })
                    .unwrap();
                assert_mapping_absent(payload_page);

                let ready = after_event
                    .after_runtime_destroy(LinuxKfdRuntimeDisabledV1 {
                        binding: KfdRuntimeBindingV1 {
                            opener_pid: std::process::id(),
                            raw_fd: file.as_fd().as_raw_fd(),
                        },
                        completion_pending: true,
                    })
                    .unwrap();
                ready.complete().unwrap();
            },
        );
    }

    #[test]
    fn unpublished_custody_unmaps_payload_on_early_return() {
        run_mapping_absence_test_in_isolated_process(
            "queue_linux::tests::unpublished_custody_unmaps_payload_on_early_return",
            || {
                let (shadows, _storage, _event, _file) = mapped_diagnostic_shadow_fixture();
                let payload_page = shadows.payload_page;
                drop(LinuxUnpublishedCwsrShadowPagesV1 {
                    shadows: Some(shadows),
                });
                assert_mapping_absent(payload_page);
            },
        );
    }

    #[test]
    fn unpublished_custody_cleanup_failure_is_process_terminal() {
        const CHILD_ENV: &str = "FE2O3_TEST_UNPUBLISHED_CWSR_PAYLOAD_RELEASE_ABORT";
        if std::env::var_os(CHILD_ENV).is_some() {
            let (mut shadows, _storage, _event, _file) = mapped_diagnostic_shadow_fixture();
            shadows.page_bytes = 0;
            drop(LinuxUnpublishedCwsrShadowPagesV1 {
                shadows: Some(shadows),
            });
            panic!("unpublished payload cleanup failure returned instead of terminating");
        }

        use std::os::unix::process::ExitStatusExt;
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("queue_linux::tests::unpublished_custody_cleanup_failure_is_process_terminal")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(libc::SIGABRT));
    }

    #[test]
    fn payload_release_failure_after_event_destroy_is_process_terminal() {
        const CHILD_ENV: &str = "FE2O3_TEST_CWSR_PAYLOAD_RELEASE_ABORT";
        if std::env::var_os(CHILD_ENV).is_some() {
            let (mut shadows, _storage, _event, _file) = mapped_diagnostic_shadow_fixture();
            let binding = shadows.binding;
            // A zero-length mprotect/munmap request cannot complete release of
            // the retained mapping. The production transition must abort
            // rather than return after consuming its only owner.
            shadows.page_bytes = 0;
            let _ = shadows.after_event_destroy(LinuxDestroyedQueueExceptionEventV1 { binding });
            panic!("payload cleanup failure returned instead of terminating");
        }

        use std::os::unix::process::ExitStatusExt;
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("queue_linux::tests::payload_release_failure_after_event_destroy_is_process_terminal")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .status()
            .unwrap();
        assert_eq!(status.signal(), Some(libc::SIGABRT));
    }

    #[test]
    fn wait_and_payload_must_agree_and_unknown_reasons_are_rejected() {
        let empty = KfdQueueExceptionReasonV1::from_untrusted_wire(0).unwrap();
        let fault = KfdQueueExceptionReasonV1::from_untrusted_wire(1).unwrap();
        assert_eq!(
            admit_queue_exception_wait(KfdWaitResultV1::Timeout, empty).unwrap(),
            QueueExceptionWaitObservationV1::NoExceptionAtObservation
        );
        assert_eq!(
            admit_queue_exception_wait(KfdWaitResultV1::Complete, fault).unwrap(),
            QueueExceptionWaitObservationV1::Exception(fault)
        );
        assert!(admit_queue_exception_wait(KfdWaitResultV1::Complete, empty).is_err());
        assert!(admit_queue_exception_wait(KfdWaitResultV1::Timeout, fault).is_err());
        assert!(KfdQueueExceptionReasonV1::from_untrusted_wire(1 << 63).is_none());
    }

    #[test]
    fn timeout_diagnostic_admits_reason_but_rejects_malformed_shadow_state() {
        let (shadows, mut storage, _payload_storage, event, file) = diagnostic_shadow_fixture();
        // SAFETY: the fixture retains the aligned writable payload word.
        unsafe { core::ptr::write_volatile(shadows.payload.as_ptr(), 1_u64.to_le()) };
        assert!(
            event
                .validate_live_with_shadows_for_diagnostic(
                    file.as_fd(),
                    std::process::id(),
                    &shadows,
                )
                .is_ok()
        );
        assert!(
            event
                .validate_live_with_shadows(file.as_fd(), std::process::id(), &shadows)
                .is_err()
        );
        assert_eq!(shadows.observe_reason().unwrap().get(), 1);

        // SAFETY: the fixture retains the aligned writable payload word.
        unsafe { core::ptr::write_volatile(shadows.payload.as_ptr(), (1_u64 << 63).to_le()) };
        assert!(shadows.observe_reason().is_err());

        storage[CWSR_CONTROL_STACK_PAGES_PER_XCC_V1][0] ^= 1;
        assert!(
            event
                .validate_live_with_shadows_for_diagnostic(
                    file.as_fd(),
                    std::process::id(),
                    &shadows,
                )
                .is_err()
        );
    }

    #[test]
    fn runtime_queue_event_order_is_linear_and_hostile_reordering_fails() {
        use KfdRuntimeLifecyclePhaseV1 as P;
        let mut phase = P::EnabledBeforeQueue;
        phase = admit_runtime_transition(phase, P::EnabledBeforeQueue, P::QueueLive).unwrap();
        phase = admit_runtime_transition(phase, P::QueueLive, P::QueueDestroyed).unwrap();
        phase = admit_runtime_transition(phase, P::QueueDestroyed, P::EventDestroyed).unwrap();
        phase = admit_runtime_transition(phase, P::EventDestroyed, P::Disabled).unwrap();
        assert_eq!(phase, P::Disabled);

        assert!(
            admit_runtime_transition(P::EnabledBeforeQueue, P::QueueLive, P::QueueDestroyed)
                .is_err()
        );
        assert!(admit_runtime_transition(P::QueueLive, P::EventDestroyed, P::Disabled).is_err());
        assert!(
            admit_runtime_transition(P::QueueDestroyed, P::EventDestroyed, P::Disabled).is_err()
        );
        assert!(
            admit_runtime_transition(P::Disabled, P::EnabledBeforeQueue, P::QueueLive).is_err()
        );
    }

    #[test]
    fn queue_exception_observation_cannot_be_reused() {
        let mut used = false;
        assert!(begin_one_shot_observation(&mut used).is_ok());
        assert!(used);
        assert!(begin_one_shot_observation(&mut used).is_err());
    }

    #[test]
    fn terminal_teardown_arm_clears_only_after_confirmed_success() {
        let gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
        let first = arm_runtime_gate_for_terminal_teardown(&gate);
        let second = arm_runtime_gate_for_terminal_teardown(&gate);
        assert_eq!(lock_runtime_gate_v1(&gate).teardown_arms, 2);
        first.confirm_destroyed();
        assert_eq!(lock_runtime_gate_v1(&gate).teardown_arms, 1);
        assert!(!lock_runtime_gate_v1(&gate).permanently_poisoned);
        second.confirm_destroyed();
        assert_eq!(lock_runtime_gate_v1(&gate).teardown_arms, 0);
        assert!(!lock_runtime_gate_v1(&gate).permanently_poisoned);

        let arm = arm_runtime_gate_for_terminal_teardown(&gate);
        assert_eq!(lock_runtime_gate_v1(&gate).teardown_arms, 1);
        drop(arm);
        assert_eq!(lock_runtime_gate_v1(&gate).teardown_arms, 0);
        assert!(lock_runtime_gate_v1(&gate).permanently_poisoned);

        let panic_gate = Mutex::new(ProcessGlobalKfdRuntimeGateV1::new());
        let result = std::panic::catch_unwind(|| {
            let _arm = arm_runtime_gate_for_terminal_teardown(&panic_gate);
            panic!("simulated teardown panic");
        });
        assert!(result.is_err());
        assert_eq!(lock_runtime_gate_v1(&panic_gate).teardown_arms, 0);
        assert!(lock_runtime_gate_v1(&panic_gate).permanently_poisoned);
    }

    #[test]
    fn teardown_arm_attempt_after_admission_check_linearizes_after_lease() {
        use std::sync::{Arc, Barrier, mpsc};

        let pid = 41;
        let gate = Arc::new(Mutex::new(ProcessGlobalKfdRuntimeGateV1::new()));
        let start_arm = Arc::new(Barrier::new(2));
        let (attempted_tx, attempted_rx) = mpsc::sync_channel(0);
        let (armed_tx, armed_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);

        let mut admission = lock_runtime_gate_v1(&gate);
        assert!(!admission.is_blocked());
        let worker_gate = Arc::clone(&gate);
        let worker_barrier = Arc::clone(&start_arm);
        let worker = std::thread::spawn(move || {
            worker_barrier.wait();
            attempted_tx.send(()).unwrap();
            let arm = arm_runtime_gate_for_terminal_teardown(&worker_gate);
            armed_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            arm.confirm_destroyed();
        });

        start_arm.wait();
        attempted_rx.recv().unwrap();
        // This models the former gap between the final arm check and the lease
        // join. The arm thread has started, but the shared gate lock keeps its
        // transition ordered after this admission.
        assert_eq!(admission.teardown_arms, 0);
        assert!(admission.runtime.join_enabled(pid).unwrap());
        admission.runtime.commit_first_enabled(pid);
        drop(admission);

        armed_rx.recv().unwrap();
        let mut blocked_admission = lock_runtime_gate_v1(&gate);
        assert!(blocked_admission.is_blocked());
        assert!(matches!(
            blocked_admission.admit_runtime(pid),
            Err(LinuxDoorbellErrorV1::Runtime(
                "process-global gate poisoned"
            ))
        ));
        drop(blocked_admission);
        release_tx.send(()).unwrap();
        worker.join().unwrap();

        let gate = lock_runtime_gate_v1(&gate);
        assert_eq!(gate.teardown_arms, 0);
        assert!(!gate.permanently_poisoned);
        assert_eq!(
            gate.runtime,
            ProcessKfdRuntimeStateV1::Enabled {
                opener_pid: pid,
                leases: 1,
            }
        );
    }

    #[test]
    fn process_runtime_context_multiplexes_independent_queue_leases() {
        let pid = 41;
        let mut state = ProcessKfdRuntimeStateV1::Disabled;
        assert!(state.join_enabled(pid).unwrap());
        state.commit_first_enabled(pid);
        assert!(!state.join_enabled(pid).unwrap());
        assert!(!state.join_enabled(pid).unwrap());
        assert_eq!(
            state,
            ProcessKfdRuntimeStateV1::Enabled {
                opener_pid: pid,
                leases: 3,
            }
        );

        assert!(!state.release_plan(pid).unwrap());
        assert!(!state.release_plan(pid).unwrap());
        assert!(state.release_plan(pid).unwrap());
        state.commit_last_disabled();
        assert_eq!(state, ProcessKfdRuntimeStateV1::Disabled);
    }

    #[test]
    fn process_runtime_context_rejects_cross_process_and_poisoned_joins() {
        let mut state = ProcessKfdRuntimeStateV1::Enabled {
            opener_pid: 17,
            leases: 1,
        };
        assert!(matches!(
            state.join_enabled(18),
            Err(LinuxDoorbellErrorV1::ProcessChanged)
        ));
        assert!(matches!(
            state.release_plan(18),
            Err(LinuxDoorbellErrorV1::ProcessChanged)
        ));
        state.poison();
        assert!(matches!(
            state.join_enabled(17),
            Err(LinuxDoorbellErrorV1::Runtime(
                "process runtime context poisoned"
            ))
        ));
    }

    #[test]
    fn destroy_event_setter_owns_the_wire_record_not_reference_bytes() {
        let source = include_str!("queue_linux.rs");
        let production = source.split("\n#[cfg(test)]\nmod tests").next().unwrap();
        assert!(production.contains("Setter::<DESTROY_EVENT_OPCODE, _>::new(args)"));
        assert!(!production.contains("Setter::<DESTROY_EVENT_OPCODE, _>::new(&args)"));
        assert!(production.contains("Setter::<UPDATE_QUEUE_OPCODE, _>::new(*args)"));
        assert!(!production.contains("Setter::<UPDATE_QUEUE_OPCODE, _>::new(args)"));
    }
}
