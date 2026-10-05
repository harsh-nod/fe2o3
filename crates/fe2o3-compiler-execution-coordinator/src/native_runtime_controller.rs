//! Original stopped-task policy orchestration, not production admission.
//!
//! No gate release, publisher token or provider-supplied enforcement guard is
//! constructed here. Open pre-effects remain a concrete missing confinement
//! obligation and refuse. The caller must keep the original trace, immutable
//! backing and outside-custodian contract through foreground retirement.
use crate::{
    compiler_invocation_backing::CompilerInvocationBacking as Backing,
    native_runtime_guard::{self as guard, Error, NativeKernelImage, descriptor},
    native_runtime_inventory::NativeCompilerExecutableInventory as Inventory,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_spawn::native_spawn::{
    MAX_RUNTIME_TASKS, RootRuntimeTraceV1 as Runtime, RuntimeTaskObservationV1 as View,
    RuntimeTraceEventV1 as Event,
};
use std::mem::size_of;

#[path = "native_runtime_controller_state.rs"]
mod state;
use state::{EntryKind, TaskImages, TaskKey};
type Result<T> = std::result::Result<T, Error>;
const _: () = assert!(state::CAPACITY == MAX_RUNTIME_TASKS);

const PARK_OPERATIONS: usize = 8192;
const LOCAL_WORK: usize = 8 + 128 * MAX_RUNTIME_TASKS * MAX_RUNTIME_TASKS;
const FRAME: usize = 4 * size_of::<NativeRuntimeController>() + 4096;

pub(crate) struct NativeRuntimeController {
    root: i32,
    ledger: Ledger,
    address: usize,
    tasks: TaskImages<NativeKernelImage>,
    pending: Option<Pending>,
    refused: bool,
}

struct Pending {
    task: TaskKey,
    generation: u64,
    kind: EntryKind,
    descriptor: Option<descriptor::PendingDescriptorCheck>,
    child: Option<TaskKey>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Progress {
    Pending,
    Advanced,
    RootTerminal {
        exit_code: Option<i32>,
        signal: Option<i32>,
    },
    TraceRetired,
}

impl NativeRuntimeController {
    pub(crate) const STORAGE: usize = size_of::<Self>();
    pub(crate) const INITIAL_WORK: usize = LOCAL_WORK + Runtime::OPERATION_WORK;
    pub(crate) const INITIAL_SCRATCH: usize = FRAME + Runtime::OPERATION_SCRATCH;
    pub(crate) const STEP_SCRATCH: usize = FRAME
        + Runtime::OPERATION_SCRATCH
        + View::FRAME
        + NativeKernelImage::STORAGE
        + NativeKernelImage::CAPTURE_SCRATCH
        + guard::IMAGE_SCRATCH
        + guard::MEMORY_SCRATCH
        + descriptor::SCRATCH;

    /// Complete fixed step quote, including bounded parking and every callback.
    /// Existing absolute deadlines and independently funded cleanup are separate.
    pub(crate) fn step_work() -> Result<usize> {
        let parts = [
            LOCAL_WORK,
            16 * Runtime::OPERATION_WORK,
            Runtime::bounded_census_work(PARK_OPERATIONS)?,
            View::CENSUS_WORK,
            2 * View::VIEW_WORK,
            MAX_RUNTIME_TASKS * guard::IMAGE_WORK,
            NativeKernelImage::CAPTURE_WORK,
            guard::MEMORY_WORK,
            descriptor::ENTRY_WORK,
            descriptor::EXIT_WORK,
        ];
        parts.into_iter().try_fold(0_usize, |sum, part| {
            sum.checked_add(part)
                .ok_or_else(|| Resource::Arithmetic.into())
        })
    }

    /// Bind only an actual held first root exec. Returned inline storage is FULL
    /// and unreserved. This does not resume, release the gate or admit a compiler.
    pub(crate) fn from_root_exec(runtime: &mut Runtime<'_>, b: &mut Budget<'_>) -> Result<Self> {
        b.with_prepaid_scope(0, 8, LOCAL_WORK, FRAME, |b| {
            let event = runtime
                .poll(b)?
                .ok_or(Error::Invalid("native controller needs held root exec"))?;
            if !event.is_exec() || event.pid() != runtime.pid() {
                return Err(Error::Invalid(
                    "native controller must begin at its original root exec",
                ));
            }
            let root = event.pid().as_raw_pid();
            Ok(Self {
                root,
                ledger: b.work_ledger_identity_v1(),
                address: b as *const Budget<'_> as usize,
                tasks: TaskImages::new(TaskKey {
                    pid: root,
                    birth: event.generation(),
                }),
                pending: None,
                refused: false,
            })
        })
    }

    /// A single owned event transition. Any refusal marks cancellation; the
    /// outer owner must drive funded retirement or retain its fail-stop contract.
    /// Source/output confinement is NOT established by a successful transition.
    pub(crate) fn step(
        &mut self,
        runtime: &mut Runtime<'_>,
        backing: &Backing,
        inventory: &Inventory,
        b: &mut Budget<'_>,
    ) -> Result<Progress> {
        let result = b.with_prepaid_scope(Self::STORAGE, 8, LOCAL_WORK, FRAME, |b| {
            if self.refused
                || runtime.pid().as_raw_pid() != self.root
                || b.work_ledger_identity_v1() != self.ledger
                || b as *const Budget<'_> as usize != self.address
            {
                return Err(Error::Invalid(
                    "native controller lost original owner/account association",
                ));
            }
            if runtime.is_trace_retired() {
                if self.tasks.count() != 0 || self.pending.is_some() {
                    return Err(Error::Invalid(
                        "native trace retirement lacks matching policy retirement",
                    ));
                }
                return Ok(Progress::TraceRetired);
            }
            let Some(event) = runtime.poll(b)? else {
                return Ok(Progress::Pending);
            };
            let key = self
                .tasks
                .key(event.pid().as_raw_pid())
                .map_err(Error::Invalid)?;
            if event.generation() < key.birth {
                return Err(Error::Invalid(
                    "native observation predates its acquired task generation",
                ));
            }
            if event.is_checkpoint() {
                return self.entry(event, key, runtime, backing, inventory, b);
            }
            if event.is_birth() {
                return self.birth(event, key, runtime, b);
            }
            if event.is_syscall_stop() {
                return self.exit(event, key, runtime, backing, inventory, b);
            }
            if event.is_terminal() {
                return self.terminal(event, key, runtime, backing, inventory, b);
            }
            if event.is_exit_boundary() {
                let pending = self
                    .pending
                    .as_ref()
                    .ok_or(Error::Invalid("unrequested runtime exit boundary"))?;
                self.bind_pending(event, key)?;
                if pending.kind != EntryKind::Exit {
                    return Err(Error::Invalid(
                        "runtime exited during a nonterminal syscall",
                    ));
                }
                // PTRACE_EVENT_EXIT resumes only kernel teardown, not user code.
                runtime.resume_selected(b)?;
                return Ok(Progress::Advanced);
            }
            if self.pending.is_some() {
                return Err(Error::Invalid(
                    "unexpected stop before original syscall completion",
                ));
            }
            if event.is_exec() {
                runtime.park_all_bounded(PARK_OPERATIONS, b)?;
                let image = runtime.with_selected_task_observation(b, |view, b| {
                    NativeKernelImage::capture(view, b)
                })?;
                b.reserve_storage(NativeKernelImage::STORAGE)?;
                let replaced = self.tasks.replace_exec(key, image).map_err(Error::Invalid);
                b.release_storage(NativeKernelImage::STORAGE)?;
                replaced?;
                self.validate_all(runtime, backing, inventory, b)?;
                runtime.resume_selected(b)?;
                runtime.release_interrupts(b)?;
                return Ok(Progress::Advanced);
            }
            if event.is_interrupt()
                || event.delivery_signal().is_some()
                || event.group_signal().is_some()
            {
                runtime.park_all_bounded(PARK_OPERATIONS, b)?;
                self.validate_all(runtime, backing, inventory, b)?;
                runtime.resume_selected(b)?;
                runtime.release_interrupts(b)?;
                return Ok(Progress::Advanced);
            }
            Err(Error::Invalid(
                "native controller received an unsupported stop kind",
            ))
        });
        if result.is_err() {
            self.refused = true;
            runtime.mark_cancellation();
        }
        result
    }

    fn entry(
        &mut self,
        event: Event,
        key: TaskKey,
        runtime: &mut Runtime<'_>,
        backing: &Backing,
        inventory: &Inventory,
        b: &mut Budget<'_>,
    ) -> Result<Progress> {
        if self.pending.is_some() {
            return Err(Error::Invalid(
                "native syscall entry overlaps its predecessor",
            ));
        }
        runtime.park_all_bounded(PARK_OPERATIONS, b)?;
        let entry = runtime.syscall_entry(b)?;
        let kind = state::entry_kind(entry.number, entry.arguments, self.tasks.count())
            .map_err(Error::Invalid)?;
        self.validate_all(runtime, backing, inventory, b)?;
        let descriptor = match kind {
            EntryKind::Memory => {
                runtime.with_selected_task_observation(b, |view, b| {
                    guard::validate_memory_entry(entry, view, backing, inventory, b)
                })?;
                None
            }
            EntryKind::Descriptor => {
                Some(runtime.with_selected_task_observation(b, |view, b| {
                    descriptor::prepare_descriptor_entry(entry, view, b)
                })?)
            }
            EntryKind::Birth | EntryKind::Exit | EntryKind::ThreadName => None,
        };
        // Inline pending storage is included in Self::STORAGE. Install it before
        // stepping, so an error never loses which operation owns completion.
        self.pending = Some(Pending {
            task: key,
            generation: event.generation(),
            kind,
            descriptor,
            child: None,
        });
        runtime.step_syscall(b)?;
        Ok(Progress::Advanced)
    }

    fn bind_pending(&self, event: Event, key: TaskKey) -> Result<()> {
        let pending = self.pending.as_ref().ok_or(Error::Invalid(
            "native syscall exit lacks original pending entry",
        ))?;
        state::require_exit_binding(pending.task, key, pending.generation, event.generation())
            .map_err(Error::Invalid)
    }

    fn birth(
        &mut self,
        event: Event,
        key: TaskKey,
        runtime: &mut Runtime<'_>,
        b: &mut Budget<'_>,
    ) -> Result<Progress> {
        self.bind_pending(event, key)?;
        let pending = self.pending.as_mut().expect("bound original entry");
        if pending.kind != EntryKind::Birth || pending.child.is_some() {
            return Err(Error::Invalid(
                "runtime birth differs from its one original syscall",
            ));
        }
        if !runtime.hold_born_child(b)? {
            return Ok(Progress::Pending);
        }
        let child = TaskKey {
            pid: runtime.selected_birth_child(b)?.as_raw_pid(),
            birth: event.generation(),
        };
        self.tasks.inherit(key, child).map_err(Error::Invalid)?;
        pending.child = Some(child);
        // Parent advances only to syscall exit; the acquired child stays parked.
        // Every image, including the inherited child image, is checked there.
        runtime.resume_selected(b)?;
        Ok(Progress::Advanced)
    }

    fn exit(
        &mut self,
        event: Event,
        key: TaskKey,
        runtime: &mut Runtime<'_>,
        backing: &Backing,
        inventory: &Inventory,
        b: &mut Budget<'_>,
    ) -> Result<Progress> {
        self.bind_pending(event, key)?;
        // The original owner also compares the saved syscall number with the
        // actual kernel exit ABI and stopped register result before returning.
        let result = runtime.syscall_result(b)?;
        runtime.park_all_bounded(PARK_OPERATIONS, b)?;
        let pending = self.pending.take().expect("bound original entry");
        if pending.kind == EntryKind::Exit {
            return Err(Error::Invalid(
                "terminal syscall unexpectedly returned to user execution",
            ));
        }
        if pending.kind == EntryKind::Birth {
            match (result, pending.child) {
                (-4095..=-1, None) => {}
                (result, Some(child)) if result == i64::from(child.pid) => {}
                _ => {
                    return Err(Error::Invalid(
                        "native birth result lacks exact acquired child",
                    ));
                }
            }
        }
        if let Some(descriptor) = pending.descriptor {
            runtime.with_selected_task_observation(b, |view, b| {
                descriptor::validate_descriptor_exit(descriptor, result, view, b)
            })?;
        }
        self.validate_all(runtime, backing, inventory, b)?;
        runtime.resume_selected(b)?;
        runtime.release_interrupts(b)?;
        Ok(Progress::Advanced)
    }

    fn terminal(
        &mut self,
        event: Event,
        key: TaskKey,
        runtime: &mut Runtime<'_>,
        backing: &Backing,
        inventory: &Inventory,
        b: &mut Budget<'_>,
    ) -> Result<Progress> {
        self.bind_pending(event, key)?;
        if self.pending.as_ref().expect("bound original entry").kind != EntryKind::Exit {
            return Err(Error::Invalid(
                "unexpected terminal wait before checked syscall completion",
            ));
        }
        let consumed = runtime.acknowledge_terminal(b)?;
        if consumed.pid() != event.pid()
            || consumed.generation() != event.generation()
            || consumed.exit_code() != event.exit_code()
            || consumed.terminating_signal() != event.terminating_signal()
        {
            return Err(Error::Invalid(
                "native terminal acknowledgement changed original observation",
            ));
        }
        self.tasks.remove_terminal(key).map_err(Error::Invalid)?;
        self.pending = None;
        if !runtime.is_trace_retired() {
            runtime.park_all_bounded(PARK_OPERATIONS, b)?;
            self.validate_all(runtime, backing, inventory, b)?;
            runtime.release_interrupts(b)?;
        }
        if key.pid == self.root {
            Ok(Progress::RootTerminal {
                exit_code: consumed.exit_code(),
                signal: consumed.terminating_signal(),
            })
        } else {
            Ok(Progress::Advanced)
        }
    }

    fn validate_all(
        &self,
        runtime: &Runtime<'_>,
        backing: &Backing,
        inventory: &Inventory,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        let mut seen = 0;
        runtime.for_each_task_observation(b, |view, b| -> Result<()> {
            let key = self
                .tasks
                .key(view.pid().as_raw_pid())
                .map_err(Error::Invalid)?;
            let image = self.tasks.image(key).map_err(Error::Invalid)?;
            guard::validate_stopped_image(view, backing, inventory, image, b)?;
            seen += 1;
            Ok(())
        })?;
        if seen != self.tasks.count() {
            return Err(Error::Invalid(
                "native image census differs from actual stopped task census",
            ));
        }
        Ok(())
    }
}
