//! Retained platform owners for primary-queue teardown. No implicit native cleanup.

use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CallV1 {
    attempted: bool,
    result: Option<Result<(), rustix::io::Errno>>,
}

impl CallV1 {
    fn run(
        &mut self,
        call: impl FnOnce() -> Result<(), rustix::io::Errno>,
    ) -> Result<(), rustix::io::Errno> {
        self.attempted = true;
        let result = call();
        self.result = Some(result);
        result
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PhaseV1 {
    Live,
    QueueDestroyed,
    EventDestroyed,
    PayloadZeroed,
    PayloadProtected,
    PayloadUnmapped,
    RuntimeDisabled,
    DoorbellReleased,
    Complete,
}

pub(crate) struct LinuxPrimaryTeardownCustodyV1 {
    runtime: LinuxKfdRuntimeEnabledV1,
    event: LinuxQueueExceptionEventV1,
    shadows: LinuxCwsrShadowPagesV1,
    doorbell: LinuxDoorbellSliceV1,
    disabled: Option<LinuxKfdRuntimeDisabledV1>,
    phase: PhaseV1,
    failed: bool,
    event_request: Option<KfdIoctlDestroyEventArgsV1>,
    runtime_request: Option<KfdIoctlRuntimeEnableArgsV1>,
    shared_lease_released: bool,
    event_destroy: CallV1,
    payload_zero: CallV1,
    payload_protect: CallV1,
    payload_unmap: CallV1,
    runtime_disable: CallV1,
    doorbell_unmap: CallV1,
}

// Only this module invokes these operations, after checking the retained owners.
// Test implementations use the same transition functions and real owned VMAs.
trait TeardownIoV1 {
    fn destroy_event(
        &mut self,
        fd: BorrowedFd<'_>,
        args: KfdIoctlDestroyEventArgsV1,
    ) -> Result<(), rustix::io::Errno>;
    fn zero_payload(&mut self, shadows: &LinuxCwsrShadowPagesV1) -> Result<(), rustix::io::Errno>;
    fn protect_payload(
        &mut self,
        shadows: &LinuxCwsrShadowPagesV1,
    ) -> Result<(), rustix::io::Errno>;
    fn unmap_payload(&mut self, shadows: &LinuxCwsrShadowPagesV1) -> Result<(), rustix::io::Errno>;
    fn disable_runtime(
        &mut self,
        fd: BorrowedFd<'_>,
        args: &mut KfdIoctlRuntimeEnableArgsV1,
    ) -> Result<(), rustix::io::Errno>;
    fn unmap_doorbell(&mut self, doorbell: &LinuxDoorbellSliceV1) -> Result<(), rustix::io::Errno>;
}

struct NativeIoV1;

#[cfg(test)]
pub(super) fn local_payload_step_v1(
    shadows: &LinuxCwsrShadowPagesV1,
    step: u8,
) -> Result<(), rustix::io::Errno> {
    // The local fixture retains the original mapping and guards each phase.
    match step {
        0 => NativeIoV1.zero_payload(shadows),
        1 => NativeIoV1.protect_payload(shadows),
        2 => NativeIoV1.unmap_payload(shadows),
        _ => unreachable!("local payload phase"),
    }
}

impl TeardownIoV1 for NativeIoV1 {
    fn destroy_event(
        &mut self,
        fd: BorrowedFd<'_>,
        args: KfdIoctlDestroyEventArgsV1,
    ) -> Result<(), rustix::io::Errno> {
        // SAFETY: Setter owns the exact pointer-free 8-byte record, not a reference.
        let request = unsafe { Setter::<DESTROY_EVENT_OPCODE, _>::new(args) };
        // SAFETY: the caller validated this retained event's process/fd binding.
        unsafe { rustix::ioctl::ioctl(fd, request) }
    }

    fn zero_payload(&mut self, shadows: &LinuxCwsrShadowPagesV1) -> Result<(), rustix::io::Errno> {
        // SAFETY: the checked live owner retains this writable, aligned payload.
        // The one-shot phase guard prevents any write after protection/unmapping.
        unsafe { core::ptr::write_volatile(shadows.payload.as_ptr(), 0_u64) };
        Ok(())
    }

    fn protect_payload(
        &mut self,
        shadows: &LinuxCwsrShadowPagesV1,
    ) -> Result<(), rustix::io::Errno> {
        // SAFETY: the exact owned page remains mapped; no reference spans protection.
        unsafe {
            rustix::mm::mprotect(
                shadows.payload_page.as_ptr(),
                shadows.page_bytes,
                MprotectFlags::empty(),
            )
        }
    }

    fn unmap_payload(&mut self, shadows: &LinuxCwsrShadowPagesV1) -> Result<(), rustix::io::Errno> {
        // SAFETY: the exact protected page is still owned and has no escaping alias.
        unsafe { rustix::mm::munmap(shadows.payload_page.as_ptr(), shadows.page_bytes) }
    }

    fn disable_runtime(
        &mut self,
        fd: BorrowedFd<'_>,
        args: &mut KfdIoctlRuntimeEnableArgsV1,
    ) -> Result<(), rustix::io::Errno> {
        // SAFETY: the exact pointer-free 16-byte record is exclusively borrowed.
        let request = unsafe { Updater::<RUNTIME_ENABLE_OPCODE, _>::new(args) };
        // SAFETY: the caller validated the runtime's process/fd binding under its gate.
        unsafe { rustix::ioctl::ioctl(fd, request) }
    }

    fn unmap_doorbell(&mut self, doorbell: &LinuxDoorbellSliceV1) -> Result<(), rustix::io::Errno> {
        // SAFETY: this active owner retains the whole slice; no MMIO reference escapes.
        unsafe { rustix::mm::munmap(doorbell.address.as_ptr(), doorbell.plan.slice_bytes) }
    }
}

impl LinuxPrimaryTeardownCustodyV1 {
    pub(crate) fn new(
        runtime: LinuxKfdRuntimeEnabledV1,
        event: LinuxQueueExceptionEventV1,
        shadows: LinuxCwsrShadowPagesV1,
        doorbell: LinuxDoorbellSliceV1,
    ) -> Self {
        Self {
            runtime,
            event,
            shadows,
            doorbell,
            disabled: None,
            phase: PhaseV1::Live,
            failed: false,
            event_request: None,
            runtime_request: None,
            shared_lease_released: false,
            event_destroy: CallV1::default(),
            payload_zero: CallV1::default(),
            payload_protect: CallV1::default(),
            payload_unmap: CallV1::default(),
            runtime_disable: CallV1::default(),
            doorbell_unmap: CallV1::default(),
        }
    }

    pub(crate) fn validate_live(
        &self,
        fd: BorrowedFd<'_>,
        pid: u32,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        if self.failed || self.phase != PhaseV1::Live {
            return Err(LinuxDoorbellErrorV1::Runtime("terminal platform teardown"));
        }
        Self::validate_owners(
            &self.runtime,
            &self.event,
            &self.shadows,
            &self.doorbell,
            fd,
            pid,
        )
    }

    pub(crate) fn validate_owners(
        runtime: &LinuxKfdRuntimeEnabledV1,
        event: &LinuxQueueExceptionEventV1,
        shadows: &LinuxCwsrShadowPagesV1,
        doorbell: &LinuxDoorbellSliceV1,
        fd: BorrowedFd<'_>,
        pid: u32,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        runtime.check_binding(fd, pid)?;
        runtime.validate_queue_live_process(pid)?;
        event.check_binding(fd, pid)?;
        if !shadows.matches(event.binding)
            || !shadows.active
            || !shadows.payload_page_active
            || doorbell.opener_pid != pid
            || !doorbell.active
        {
            return Err(LinuxDoorbellErrorV1::Shadow(
                "primary teardown owner binding",
            ));
        }
        Ok(())
    }

    fn run(
        &mut self,
        phase: PhaseV1,
        gate: &Mutex<ProcessGlobalKfdRuntimeGateV1>,
        body: impl FnOnce(&mut Self) -> Result<(), LinuxDoorbellErrorV1>,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        if self.failed || self.phase != phase {
            return Err(LinuxDoorbellErrorV1::Runtime("terminal platform teardown"));
        }
        let result = catch_unwind(AssertUnwindSafe(|| body(self)));
        match result {
            Ok(Ok(())) => Ok(()),
            result => {
                self.failed = true;
                self.runtime.poisoned = true;
                // The body has returned/unwound: its runtime gate guard is gone.
                lock_runtime_gate_v1(gate).poison();
                match result {
                    Ok(Err(error)) => Err(error),
                    Err(payload) => resume_unwind(payload),
                    Ok(Ok(())) => unreachable!(),
                }
            }
        }
    }

    pub(crate) fn after_queue_destroyed(
        &mut self,
        fd: BorrowedFd<'_>,
        pid: u32,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        self.after_queue_destroyed_with(fd, pid, &KFD_RUNTIME_GATE, &mut NativeIoV1)
    }

    fn after_queue_destroyed_with(
        &mut self,
        fd: BorrowedFd<'_>,
        pid: u32,
        gate: &Mutex<ProcessGlobalKfdRuntimeGateV1>,
        io: &mut impl TeardownIoV1,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        self.run(PhaseV1::Live, gate, |this| {
            this.validate_live(fd, pid)?;
            this.runtime.mark_queue_destroyed()?;
            this.phase = PhaseV1::QueueDestroyed;
            let args = KfdIoctlDestroyEventArgsV1::new(this.event.binding.event_id);
            this.event_request = Some(args);
            this.event_destroy
                .run(|| io.destroy_event(fd, args))
                .map_err(|source| LinuxDoorbellErrorV1::EventSyscall {
                    operation: "AMDKFD_IOC_DESTROY_EVENT",
                    source,
                })?;
            this.event.active = false;
            this.shadows.active = false;
            this.phase = PhaseV1::EventDestroyed;
            this.payload_zero
                .run(|| io.zero_payload(&this.shadows))
                .map_err(|source| LinuxDoorbellErrorV1::ShadowSyscall {
                    operation: "zero CWSR payload",
                    source,
                })?;
            this.phase = PhaseV1::PayloadZeroed;
            this.payload_protect
                .run(|| io.protect_payload(&this.shadows))
                .map_err(|source| LinuxDoorbellErrorV1::ShadowSyscall {
                    operation: "mprotect CWSR payload inaccessible",
                    source,
                })?;
            this.phase = PhaseV1::PayloadProtected;
            this.payload_unmap
                .run(|| io.unmap_payload(&this.shadows))
                .map_err(|source| LinuxDoorbellErrorV1::ShadowSyscall {
                    operation: "munmap CWSR payload page",
                    source,
                })?;
            this.shadows.payload_page_active = false;
            this.phase = PhaseV1::PayloadUnmapped;
            this.runtime.mark_event_destroyed()?;
            this.disable_runtime(fd, pid, gate, io)?;
            this.phase = PhaseV1::RuntimeDisabled;
            this.validate_shadows()
        })
    }

    fn disable_runtime(
        &mut self,
        fd: BorrowedFd<'_>,
        pid: u32,
        gate: &Mutex<ProcessGlobalKfdRuntimeGateV1>,
        io: &mut impl TeardownIoV1,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        self.runtime.check_binding(fd, pid)?;
        if self.runtime.phase != KfdRuntimeLifecyclePhaseV1::EventDestroyed {
            return Err(LinuxDoorbellErrorV1::Runtime("runtime disable phase"));
        }
        let mut guard = lock_runtime_gate_v1(gate);
        if guard.runtime.release_plan(pid)? {
            let expected = KfdIoctlRuntimeEnableArgsV1::new_queue_exception_disable();
            let args = self.runtime_request.insert(expected);
            self.runtime_disable
                .run(|| io.disable_runtime(fd, args))
                .map_err(|source| LinuxDoorbellErrorV1::RuntimeSyscall {
                    operation: "AMDKFD_IOC_RUNTIME_ENABLE(disable)",
                    source,
                })?;
            if *args != expected || !args.is_exact_queue_exception_disable() {
                return Err(LinuxDoorbellErrorV1::Runtime(
                    "RUNTIME_ENABLE disable output drift",
                ));
            }
            guard.runtime.commit_last_disabled();
        } else {
            self.shared_lease_released = true;
        }
        // No fallible work or owner drop between the lease commit and these receipts.
        self.runtime.active = false;
        self.runtime.phase = KfdRuntimeLifecyclePhaseV1::Disabled;
        self.disabled = Some(LinuxKfdRuntimeDisabledV1 {
            binding: self.runtime.binding,
            completion_pending: true,
        });
        Ok(())
    }

    pub(crate) fn release_doorbell(&mut self) -> Result<(), LinuxDoorbellErrorV1> {
        self.release_doorbell_with(&KFD_RUNTIME_GATE, &mut NativeIoV1)
    }

    fn release_doorbell_with(
        &mut self,
        gate: &Mutex<ProcessGlobalKfdRuntimeGateV1>,
        io: &mut impl TeardownIoV1,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        self.run(PhaseV1::RuntimeDisabled, gate, |this| {
            this.validate_shadows()?;
            if this.doorbell.opener_pid != std::process::id() || !this.doorbell.active {
                return Err(LinuxDoorbellErrorV1::ProcessChanged);
            }
            this.doorbell_unmap
                .run(|| io.unmap_doorbell(&this.doorbell))
                .map_err(|source| LinuxDoorbellErrorV1::Syscall {
                    operation: "munmap complete KFD doorbell slice",
                    source,
                })?;
            this.doorbell.active = false;
            this.phase = PhaseV1::DoorbellReleased;
            Ok(())
        })
    }

    fn validate_shadows(&self) -> Result<(), LinuxDoorbellErrorV1> {
        let Some(runtime) = &self.disabled else {
            return Err(LinuxDoorbellErrorV1::Shadow("missing disabled runtime"));
        };
        if self.failed
            || self.shadows.active
            || self.shadows.payload_page_active
            || self.event.active
            || !runtime.completion_pending
            || self.shadows.binding.opener_pid != std::process::id()
            || runtime.binding.opener_pid != self.shadows.binding.opener_pid
            || runtime.binding.raw_fd != self.shadows.binding.raw_fd
        {
            return Err(LinuxDoorbellErrorV1::Shadow(
                "retained shadow release state",
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_for_release(&self) -> Result<(), LinuxDoorbellErrorV1> {
        if self.phase != PhaseV1::DoorbellReleased || self.doorbell.active {
            return Err(LinuxDoorbellErrorV1::Shadow(
                "retained doorbell release state",
            ));
        }
        self.validate_shadows()
    }

    pub(crate) fn complete_shadows(&mut self) -> Result<(), LinuxDoorbellErrorV1> {
        self.complete_shadows_with(&KFD_RUNTIME_GATE)
    }

    fn complete_shadows_with(
        &mut self,
        gate: &Mutex<ProcessGlobalKfdRuntimeGateV1>,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        self.run(PhaseV1::DoorbellReleased, gate, |this| {
            this.validate_for_release()?;
            this.disabled
                .as_mut()
                .expect("validated disabled runtime")
                .completion_pending = false;
            this.phase = PhaseV1::Complete;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests;
