use super::*;
use std::os::fd::AsFd;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Op {
    Event,
    Zero,
    Protect,
    Payload,
    Runtime,
    Doorbell,
}
const ORDER: [Op; 6] = [
    Op::Event,
    Op::Zero,
    Op::Protect,
    Op::Payload,
    Op::Runtime,
    Op::Doorbell,
];

#[derive(Default)]
struct Io {
    calls: Vec<Op>,
    event_input: Option<(i32, KfdIoctlDestroyEventArgsV1)>,
    runtime_input: Option<(i32, KfdIoctlRuntimeEnableArgsV1)>,
    fault: Option<(Op, bool)>,
    runtime_drift: bool,
    payload_unmapped: bool,
    doorbell_unmapped: bool,
}

impl Io {
    fn enter(&mut self, op: Op) -> Result<(), rustix::io::Errno> {
        self.calls.push(op);
        if let Some((wanted, panic)) = self.fault
            && wanted == op
        {
            if panic {
                std::panic::panic_any(("platform teardown", op));
            }
            return Err(rustix::io::Errno::IO);
        }
        Ok(())
    }
}

impl TeardownIoV1 for Io {
    fn destroy_event(
        &mut self,
        fd: BorrowedFd<'_>,
        args: KfdIoctlDestroyEventArgsV1,
    ) -> Result<(), rustix::io::Errno> {
        self.event_input = Some((fd.as_raw_fd(), args));
        self.enter(Op::Event)
    }
    fn zero_payload(&mut self, shadows: &LinuxCwsrShadowPagesV1) -> Result<(), rustix::io::Errno> {
        self.enter(Op::Zero)?;
        NativeIoV1.zero_payload(shadows)
    }
    fn protect_payload(
        &mut self,
        shadows: &LinuxCwsrShadowPagesV1,
    ) -> Result<(), rustix::io::Errno> {
        self.enter(Op::Protect)?;
        NativeIoV1.protect_payload(shadows)
    }
    fn unmap_payload(&mut self, shadows: &LinuxCwsrShadowPagesV1) -> Result<(), rustix::io::Errno> {
        self.enter(Op::Payload)?;
        NativeIoV1.unmap_payload(shadows)?;
        self.payload_unmapped = true;
        Ok(())
    }
    fn disable_runtime(
        &mut self,
        fd: BorrowedFd<'_>,
        args: &mut KfdIoctlRuntimeEnableArgsV1,
    ) -> Result<(), rustix::io::Errno> {
        self.runtime_input = Some((fd.as_raw_fd(), *args));
        if self.runtime_drift {
            *args = KfdIoctlRuntimeEnableArgsV1::from_untrusted_wire(37, 19, 23);
        }
        self.enter(Op::Runtime)
    }
    fn unmap_doorbell(&mut self, doorbell: &LinuxDoorbellSliceV1) -> Result<(), rustix::io::Errno> {
        self.enter(Op::Doorbell)?;
        NativeIoV1.unmap_doorbell(doorbell)?;
        self.doorbell_unmapped = true;
        Ok(())
    }
}

struct Fixture {
    custody: LinuxPrimaryTeardownCustodyV1,
    io: Io,
    gate: Mutex<ProcessGlobalKfdRuntimeGateV1>,
    file: std::fs::File,
    _pages: Vec<Box<[u8; 4096]>>,
}

impl Fixture {
    fn new(leases: usize) -> Self {
        let (shadows, pages, event, file) = super::super::tests::mapped_diagnostic_shadow_fixture();
        let pid = std::process::id();
        let runtime = LinuxKfdRuntimeEnabledV1 {
            binding: KfdRuntimeBindingV1 {
                opener_pid: pid,
                raw_fd: file.as_raw_fd(),
            },
            active: true,
            poisoned: false,
            phase: KfdRuntimeLifecyclePhaseV1::QueueLive,
        };
        let bytes = KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES as usize;
        // SAFETY: independent anonymous mapping, never MMIO or borrowed storage.
        let address = unsafe {
            rustix::mm::mmap_anonymous(
                core::ptr::null_mut(),
                bytes,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::PRIVATE,
            )
        }
        .unwrap();
        let doorbell = LinuxDoorbellSliceV1 {
            address: NonNull::new(address).unwrap(),
            plan: DoorbellMmapPlanV1 {
                encoded_slice_offset: 0,
                queue_byte_offset: 0,
                slice_bytes: bytes,
            },
            opener_pid: pid,
            active: true,
        };
        let mut gate = ProcessGlobalKfdRuntimeGateV1::new();
        gate.runtime = ProcessKfdRuntimeStateV1::Enabled {
            opener_pid: pid,
            leases,
        };
        Self {
            custody: LinuxPrimaryTeardownCustodyV1::new(runtime, event, shadows, doorbell),
            io: Io::default(),
            gate: Mutex::new(gate),
            file,
            _pages: pages,
        }
    }

    fn run(&mut self) -> Result<(), LinuxDoorbellErrorV1> {
        self.custody.after_queue_destroyed_with(
            self.file.as_fd(),
            std::process::id(),
            &self.gate,
            &mut self.io,
        )?;
        self.custody
            .release_doorbell_with(&self.gate, &mut self.io)?;
        self.custody.complete_shadows_with(&self.gate)
    }

    fn progress(&self) -> [CallV1; 6] {
        let c = &self.custody;
        [
            c.event_destroy,
            c.payload_zero,
            c.payload_protect,
            c.payload_unmap,
            c.runtime_disable,
            c.doorbell_unmap,
        ]
    }

    fn reject_retry(&mut self) {
        let progress = self.progress();
        let calls = self.io.calls.clone();
        let phase = self.custody.phase;
        self.io.fault = None;
        assert!(self.run().is_err());
        assert!(
            self.custody
                .release_doorbell_with(&self.gate, &mut self.io)
                .is_err()
        );
        assert!(self.custody.complete_shadows_with(&self.gate).is_err());
        assert_eq!(self.progress(), progress);
        assert_eq!(self.io.calls, calls);
        assert_eq!(self.custody.phase, phase);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // The fixture, not the terminal production owner, releases test-only VMAs.
        if !self.io.payload_unmapped {
            NativeIoV1.unmap_payload(&self.custody.shadows).unwrap();
        }
        if !self.io.doorbell_unmapped {
            NativeIoV1.unmap_doorbell(&self.custody.doorbell).unwrap();
        }
        // These leases belong to the local fake gate, never the process-global gate.
        self.custody.runtime.active = false;
        self.custody.runtime.poisoned = false;
        if let Some(disabled) = &mut self.custody.disabled {
            disabled.completion_pending = false;
        }
    }
}

#[test]
fn platform_teardown_success_retains_receipts_and_outer_gate_until_confirmation() {
    let mut f = Fixture::new(1);
    // The arm is held separately from platform custody, as it will be in the parent.
    let gate = &f.gate;
    let arm = arm_runtime_gate_for_terminal_teardown(gate);
    f.custody
        .after_queue_destroyed_with(f.file.as_fd(), std::process::id(), gate, &mut f.io)
        .unwrap();
    assert!(!f.custody.event.active && !f.custody.shadows.payload_page_active);
    assert_eq!(
        f.custody.runtime_request,
        Some(KfdIoctlRuntimeEnableArgsV1::new_queue_exception_disable())
    );
    assert!(f.custody.disabled.as_ref().unwrap().completion_pending);
    assert!(f.custody.validate_for_release().is_err());
    f.custody.release_doorbell_with(gate, &mut f.io).unwrap();
    f.custody.validate_for_release().unwrap();
    f.custody.complete_shadows_with(gate).unwrap();
    assert_eq!(f.io.calls, ORDER);
    assert_eq!(
        f.progress(),
        [CallV1 {
            attempted: true,
            result: Some(Ok(()))
        }; 6]
    );
    assert_eq!(f.custody.phase, PhaseV1::Complete);
    assert!(!f.custody.disabled.as_ref().unwrap().completion_pending);
    assert!(
        lock_runtime_gate_v1(gate)
            .admit_runtime(std::process::id())
            .is_err()
    );
    arm.confirm_destroyed();
    assert!(!lock_runtime_gate_v1(gate).is_blocked());
    assert_eq!(
        lock_runtime_gate_v1(gate).runtime,
        ProcessKfdRuntimeStateV1::Disabled
    );
    f.reject_retry();
}

#[test]
fn platform_teardown_error_and_panic_matrix_keeps_exact_native_prefix() {
    for (index, op) in ORDER.into_iter().enumerate() {
        for panic in [false, true] {
            let mut f = Fixture::new(1);
            let addresses = (f.custody.shadows.payload_page, f.custody.doorbell.address);
            f.io.fault = Some((op, panic));
            let result = catch_unwind(AssertUnwindSafe(|| f.run()));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, Op)>(),
                    Some(&("platform teardown", op))
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(f.io.calls, ORDER[..=index]);
            let progress = f.progress();
            for (i, call) in progress.iter().enumerate() {
                assert_eq!(
                    *call,
                    if i < index {
                        CallV1 {
                            attempted: true,
                            result: Some(Ok(())),
                        }
                    } else if i == index {
                        CallV1 {
                            attempted: true,
                            result: (!panic).then_some(Err(rustix::io::Errno::IO)),
                        }
                    } else {
                        CallV1::default()
                    }
                );
            }
            assert_eq!(
                f.custody.phase,
                [
                    PhaseV1::QueueDestroyed,
                    PhaseV1::EventDestroyed,
                    PhaseV1::PayloadZeroed,
                    PhaseV1::PayloadProtected,
                    PhaseV1::PayloadUnmapped,
                    PhaseV1::RuntimeDisabled
                ][index]
            );
            assert_eq!(f.custody.event.active, index == 0);
            assert_eq!(f.custody.shadows.payload_page_active, index <= 3);
            assert_eq!(f.custody.runtime.active, index <= 4);
            assert!(f.custody.doorbell.active && f.custody.failed);
            assert_eq!(
                (f.custody.shadows.payload_page, f.custody.doorbell.address),
                addresses
            );
            assert_eq!(
                f.custody.event_request,
                Some(KfdIoctlDestroyEventArgsV1::new(
                    f.custody.event.binding.event_id
                ))
            );
            assert_eq!(
                f.io.event_input,
                Some((f.file.as_raw_fd(), f.custody.event_request.unwrap()))
            );
            assert_eq!(
                f.io.runtime_input,
                (index >= 4).then_some((
                    f.file.as_raw_fd(),
                    KfdIoctlRuntimeEnableArgsV1::new_queue_exception_disable()
                ))
            );
            assert!(lock_runtime_gate_v1(&f.gate).permanently_poisoned);
            f.reject_retry();
        }
    }
}

#[test]
fn platform_teardown_shared_lease_is_released_once_without_disable_ioctl() {
    let mut f = Fixture::new(2);
    f.run().unwrap();
    assert_eq!(
        f.io.calls,
        [Op::Event, Op::Zero, Op::Protect, Op::Payload, Op::Doorbell]
    );
    assert!(f.custody.shared_lease_released);
    assert_eq!(f.custody.runtime_request, None);
    assert_eq!(f.custody.runtime_disable, CallV1::default());
    f.reject_retry();
    assert_eq!(
        lock_runtime_gate_v1(&f.gate).runtime,
        ProcessKfdRuntimeStateV1::Enabled {
            opener_pid: std::process::id(),
            leases: 1
        }
    );
}

#[test]
fn platform_teardown_retains_mutated_disable_wire_on_errno_panic_and_output_drift() {
    for fault in [None, Some((Op::Runtime, false)), Some((Op::Runtime, true))] {
        let mut f = Fixture::new(1);
        f.io.runtime_drift = true;
        f.io.fault = fault;
        let result = catch_unwind(AssertUnwindSafe(|| f.run()));
        if fault == Some((Op::Runtime, true)) {
            assert_eq!(
                result.unwrap_err().downcast_ref::<(&str, Op)>(),
                Some(&("platform teardown", Op::Runtime))
            );
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(
            f.custody.runtime_request,
            Some(KfdIoctlRuntimeEnableArgsV1::from_untrusted_wire(37, 19, 23))
        );
        assert_eq!(
            f.io.runtime_input,
            Some((
                f.file.as_raw_fd(),
                KfdIoctlRuntimeEnableArgsV1::new_queue_exception_disable()
            ))
        );
        assert!(f.custody.runtime.active && f.custody.disabled.is_none());
        assert_eq!(
            f.custody.runtime_disable.result,
            match fault {
                None => Some(Ok(())),
                Some((_, false)) => Some(Err(rustix::io::Errno::IO)),
                _ => None,
            }
        );
        assert_eq!(f.io.calls, ORDER[..5]);
        assert!(lock_runtime_gate_v1(&f.gate).permanently_poisoned);
        f.reject_retry();
    }
}

#[test]
fn platform_teardown_binding_substitution_rejected_before_native_effects() {
    for which in 0..6 {
        let mut f = Fixture::new(1);
        match which {
            0 => f.custody.runtime.binding.raw_fd = -1,
            1 => f.custody.event.binding.raw_fd = -1,
            2 => f.custody.shadows.binding.event_id = KfdSignalEventIdV1::new(8).unwrap(),
            3 => f.custody.doorbell.opener_pid = 0,
            4 => f.custody.runtime.binding.opener_pid = 0,
            _ => f.custody.shadows.payload_page_active = false,
        }
        assert!(f.run().is_err());
        assert!(f.io.calls.is_empty());
        assert_eq!(f.progress(), [CallV1::default(); 6]);
        assert!(f.custody.failed);
        f.reject_retry();
    }
}

#[test]
fn platform_teardown_post_doorbell_validation_failure_retains_inactive_receipt() {
    let mut f = Fixture::new(1);
    f.custody
        .after_queue_destroyed_with(f.file.as_fd(), std::process::id(), &f.gate, &mut f.io)
        .unwrap();
    f.custody.release_doorbell_with(&f.gate, &mut f.io).unwrap();
    f.custody.disabled.as_mut().unwrap().binding.raw_fd = -1;
    assert!(f.custody.complete_shadows_with(&f.gate).is_err());
    assert!(!f.custody.doorbell.active);
    assert!(f.custody.disabled.as_ref().unwrap().completion_pending);
    assert_eq!(f.custody.doorbell_unmap.result, Some(Ok(())));
    assert_eq!(f.io.calls, ORDER);
    f.reject_retry();
}
