//! Local VM and gate composition only: no KFD ioctl or process-global gate access.

use super::*;
use std::os::fd::AsFd;
use std::sync::{Arc, atomic::AtomicUsize};

#[path = "primary_fixture_tests.rs"]
mod tests;

#[derive(Default)]
struct Counts {
    reservations: AtomicUsize,
    payloads: AtomicUsize,
    events: AtomicUsize,
}

#[derive(Clone, Default)]
pub(crate) struct LocalResourcesV1(Arc<Counts>);

impl LocalResourcesV1 {
    pub(crate) fn live(&self) -> (usize, usize, usize) {
        (
            self.0.reservations.load(Ordering::SeqCst),
            self.0.payloads.load(Ordering::SeqCst),
            self.0.events.load(Ordering::SeqCst),
        )
    }

    pub(crate) fn event(&self) -> LocalEventV1 {
        let file = std::fs::File::open("/dev/null").unwrap();
        let binding = QueueExceptionBindingV1 {
            event_id: KfdSignalEventIdV1::new(11).unwrap(),
            opener_pid: std::process::id(),
            raw_fd: file.as_fd().as_raw_fd(),
        };
        self.0.events.fetch_add(1, Ordering::SeqCst);
        LocalEventV1(Arc::new(LocalEventInnerV1 {
            event: LinuxQueueExceptionEventV1 {
                binding,
                active: true,
                poisoned: false,
                observation_used: false,
            },
            file,
            counts: self.0.clone(),
        }))
    }
}

struct LocalEventInnerV1 {
    event: LinuxQueueExceptionEventV1,
    file: std::fs::File,
    counts: Arc<Counts>,
}

impl Drop for LocalEventInnerV1 {
    fn drop(&mut self) {
        self.counts.events.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Clone)]
pub(crate) struct LocalEventV1(Arc<LocalEventInnerV1>);

impl LocalEventV1 {
    pub(crate) fn id(&self) -> u32 {
        self.0.event.event_id_observation()
    }
}

struct PayloadCountV1 {
    counts: Arc<Counts>,
    active: bool,
}

impl PayloadCountV1 {
    fn released(&mut self) {
        if self.active {
            self.counts.payloads.fetch_sub(1, Ordering::SeqCst);
            self.active = false;
        }
    }
}

impl Drop for PayloadCountV1 {
    fn drop(&mut self) {
        self.released();
    }
}

struct LocalReservationV1 {
    base: NonNull<c_void>,
    bytes: usize,
    counts: Arc<Counts>,
}

impl LocalReservationV1 {
    fn new(counts: &Arc<Counts>) -> Result<Self, LinuxDoorbellErrorV1> {
        let bytes = crate::queue::submit::GFX942_CWSR_TOTAL_BYTES_V1;
        // Own the complete range before install replaces its control-stack pages.
        let mapped = unsafe {
            rustix::mm::mmap_anonymous(
                core::ptr::null_mut(),
                bytes,
                ProtFlags::empty(),
                MapFlags::PRIVATE | MapFlags::NORESERVE,
            )
        }
        .map_err(|source| LinuxDoorbellErrorV1::ShadowSyscall {
            operation: "reserve local primary CWSR fixture",
            source,
        })?;
        let Some(base) = NonNull::new(mapped) else {
            if unsafe { rustix::mm::munmap(mapped, bytes) }.is_err() {
                std::process::abort();
            }
            return Err(LinuxDoorbellErrorV1::Shadow("zero local reservation"));
        };
        counts.reservations.fetch_add(1, Ordering::SeqCst);
        Ok(Self {
            base,
            bytes,
            counts: counts.clone(),
        })
    }
}

impl Drop for LocalReservationV1 {
    fn drop(&mut self) {
        if unsafe { rustix::mm::munmap(self.base.as_ptr(), self.bytes) }.is_err() {
            std::process::abort();
        }
        self.counts.reservations.fetch_sub(1, Ordering::SeqCst);
    }
}

pub(crate) struct LocalUnpublishedV1 {
    shadows: LinuxUnpublishedCwsrShadowPagesV1,
    payload_count: PayloadCountV1,
    reservation: LocalReservationV1,
    event: Arc<LocalEventInnerV1>,
}

pub(crate) struct LocalPublishedV1 {
    shadows: LinuxCwsrShadowPagesV1,
    _payload_count: PayloadCountV1,
    _reservation: LocalReservationV1,
    _event: Arc<LocalEventInnerV1>,
}

impl LocalUnpublishedV1 {
    pub(crate) fn install(event: &LocalEventV1) -> Result<Self, LinuxDoorbellErrorV1> {
        let reservation = LocalReservationV1::new(&event.0.counts)?;
        let plan = CwsrShadowPlanV1::from_owned_reservation(
            reservation.base.as_ptr() as usize as u64,
            reservation.bytes,
            4096,
        )?;
        let shadows = LinuxCwsrShadowPagesV1::install(plan, &event.0.event)?;
        event.0.counts.payloads.fetch_add(1, Ordering::SeqCst);
        Ok(Self {
            shadows,
            payload_count: PayloadCountV1 {
                counts: event.0.counts.clone(),
                active: true,
            },
            reservation,
            event: event.0.clone(),
        })
    }
    pub(crate) fn initialize(
        &self,
        bytes: &mut [u8],
    ) -> Result<(), crate::queue::submit::NativeAqlSubmissionErrorV1> {
        self.shadows
            .shadows()
            .initialize_and_validate_bo_headers(bytes)
    }
    pub(crate) fn validate(&self, event: &LocalEventV1) -> Result<(), LinuxDoorbellErrorV1> {
        event.0.event.validate_live_with_shadows(
            event.0.file.as_fd(),
            std::process::id(),
            self.shadows.shadows(),
        )
    }
    pub(crate) fn protect_read_only(&self) -> Result<(), LinuxDoorbellErrorV1> {
        // The fixture reservation is independent of its fake BO; this is not BO sealing.
        for page in self.shadows.shadows().pages {
            unsafe { rustix::mm::mprotect(page.as_ptr(), 4096, MprotectFlags::READ) }.map_err(
                |source| LinuxDoorbellErrorV1::ShadowSyscall {
                    operation: "protect local primary fixture",
                    source,
                },
            )?;
        }
        Ok(())
    }
    pub(crate) fn restore_write(&self) -> Result<(), LinuxDoorbellErrorV1> {
        self.shadows
            .shadows()
            .restore_kernel_write_access_after_bo_seal()?;
        // Read and write the same owned byte to exercise each restored page.
        for page in self.shadows.shadows().pages {
            let byte = page.as_ptr().cast::<u8>();
            unsafe {
                core::ptr::write_volatile(byte, core::ptr::read_volatile(byte));
            }
        }
        Ok(())
    }
    pub(crate) fn cleanup_terminal(&mut self) {
        self.shadows.cleanup_payload_for_terminal_retention();
        if !self.shadows.shadows.as_ref().unwrap().payload_page_active {
            self.payload_count.released();
        }
    }
    pub(crate) fn state(&self) -> (bool, bool, bool) {
        let shadows = self.shadows.shadows.as_ref().unwrap();
        (
            self.shadows.terminal_payload_released,
            shadows.active,
            shadows.payload_page_active,
        )
    }
    pub(crate) fn publish(self) -> LocalPublishedV1 {
        let Self {
            shadows,
            payload_count,
            reservation,
            event,
        } = self;
        LocalPublishedV1 {
            shadows: shadows.publish_for_native_queue_creation(),
            _payload_count: payload_count,
            _reservation: reservation,
            _event: event,
        }
    }
}

impl LocalPublishedV1 {
    pub(crate) fn state(&self) -> (bool, bool) {
        (self.shadows.active, self.shadows.payload_page_active)
    }
}

impl Drop for LocalPublishedV1 {
    fn drop(&mut self) {
        // Test-only disposal after simulated CREATE; no destroyed-event authority is minted.
        if self.shadows.payload_page_active && self.shadows.release_payload_page().is_err() {
            std::process::abort();
        }
        self.shadows.active = false;
    }
}

#[derive(Clone)]
pub(crate) struct LocalGateV1(Arc<Mutex<ProcessGlobalKfdRuntimeGateV1>>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LocalRuntimeObservationV1 {
    Disabled,
    Enabled { opener_pid: u32, leases: usize },
    Poisoned,
}

impl LocalGateV1 {
    pub(crate) fn new() -> Self {
        Self(Arc::new(Mutex::new(ProcessGlobalKfdRuntimeGateV1::new())))
    }
    pub(crate) fn register_runtime(
        &self,
        opener_pid: u32,
    ) -> Result<LocalRuntimeRegistrationV1, LinuxDoorbellErrorV1> {
        if opener_pid != std::process::id() {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        {
            let mut gate = lock_runtime_gate_v1(&self.0);
            if gate.admit_runtime(opener_pid)? {
                gate.runtime.commit_first_enabled(opener_pid);
            }
        }
        Ok(LocalRuntimeRegistrationV1 {
            gate: self.clone(),
            opener_pid,
            phase: KfdRuntimeLifecyclePhaseV1::EnabledBeforeQueue,
        })
    }
    pub(crate) fn arm(&self) -> Result<LocalCreationArmV1, LinuxDoorbellErrorV1> {
        lock_runtime_gate_v1(&self.0).arm_creation()?;
        Ok(LocalCreationArmV1 {
            gate: self.clone(),
            finished: false,
        })
    }
    pub(crate) fn poison(&self) {
        lock_runtime_gate_v1(&self.0).poison();
    }
    pub(crate) fn observation(&self) -> (bool, bool) {
        let gate = lock_runtime_gate_v1(&self.0);
        (gate.creation_in_flight, gate.permanently_poisoned)
    }
    pub(crate) fn runtime_observation(&self) -> LocalRuntimeObservationV1 {
        match lock_runtime_gate_v1(&self.0).runtime {
            ProcessKfdRuntimeStateV1::Disabled => LocalRuntimeObservationV1::Disabled,
            ProcessKfdRuntimeStateV1::Enabled { opener_pid, leases } => {
                LocalRuntimeObservationV1::Enabled { opener_pid, leases }
            }
            ProcessKfdRuntimeStateV1::Poisoned => LocalRuntimeObservationV1::Poisoned,
        }
    }
}

pub(crate) struct LocalRuntimeRegistrationV1 {
    gate: LocalGateV1,
    opener_pid: u32,
    phase: KfdRuntimeLifecyclePhaseV1,
}

impl LocalRuntimeRegistrationV1 {
    pub(crate) fn validate_binding(
        &self,
        gate: &LocalGateV1,
        opener_pid: u32,
    ) -> Result<(), LinuxDoorbellErrorV1> {
        if self.opener_pid != opener_pid || opener_pid != std::process::id() {
            return Err(LinuxDoorbellErrorV1::ProcessChanged);
        }
        if !Arc::ptr_eq(&self.gate.0, &gate.0) {
            return Err(LinuxDoorbellErrorV1::Runtime("local runtime gate binding"));
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
    pub(crate) fn is_queue_live(&self) -> bool {
        self.phase == KfdRuntimeLifecyclePhaseV1::QueueLive
    }
}

impl Drop for LocalRuntimeRegistrationV1 {
    fn drop(&mut self) {
        // Fixture disposal is not a confirmed native runtime-disable operation.
        self.gate.poison();
    }
}

pub(crate) struct LocalCreationArmV1 {
    gate: LocalGateV1,
    finished: bool,
}

impl LocalCreationArmV1 {
    pub(crate) fn finish(&mut self, pid: u32) -> Result<(), LinuxDoorbellErrorV1> {
        finish_runtime_gate_creation_checked_v1(&self.gate.0, &mut self.finished, pid)
    }
}

impl Drop for LocalCreationArmV1 {
    fn drop(&mut self) {
        if !self.finished {
            finish_runtime_gate_creation_arm(&self.gate.0, false);
        }
    }
}
