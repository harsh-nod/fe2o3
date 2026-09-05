//! Contracted currentness observations for an admitted device token.

use std::fmt;
use std::os::fd::FromRawFd;

use fe2o3_kfd_uapi::{AMDKFD_IOC_SMI_EVENTS, KFD_SMI_EVENT_GPU_RESET_MASK, KfdIoctlSmiEventsArgs};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fd::OwnedFd;
use rustix::io::FdFlags;
use rustix::ioctl::{Opcode, Updater};

use crate::device::{CheckedGfx942XnackMinusDevice, DeviceBindingError, validate_apertures};

const SMI_EVENTS_OPCODE: Opcode = AMDKFD_IOC_SMI_EVENTS as Opcode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LatchInput {
    Clear,
    Event,
    ProtocolFailure,
}

fn classify_non_draining_reset_poll(ready: usize, events: PollFlags) -> LatchInput {
    if ready == 0 && events.is_empty() {
        return LatchInput::Clear;
    }
    let readable = PollFlags::IN | PollFlags::RDNORM;
    if ready == 1 && events.intersects(readable) && (events.bits() & !readable.bits()) == 0 {
        return LatchInput::Event;
    }
    LatchInput::ProtocolFailure
}

fn finish_non_draining_reset_poll(
    result: Result<usize, rustix::io::Errno>,
    events: PollFlags,
) -> Result<LatchInput, DeviceBindingError> {
    let ready = result.map_err(|source| DeviceBindingError::Syscall {
        operation: "poll KFD reset-event fence",
        source,
    })?;
    Ok(classify_non_draining_reset_poll(ready, events))
}

fn poll_reset_event_readiness(fd: &OwnedFd) -> Result<LatchInput, DeviceBindingError> {
    let timeout = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let mut descriptors = [PollFd::new(fd, PollFlags::IN | PollFlags::RDNORM)];
    let result = poll(&mut descriptors, Some(&timeout));
    finish_non_draining_reset_poll(result, descriptors[0].revents())
}

#[derive(Debug, Default)]
struct PoisonLatch {
    poisoned: bool,
}

impl PoisonLatch {
    fn observe(&mut self, input: LatchInput) -> Result<(), DeviceBindingError> {
        if self.poisoned {
            return Err(DeviceBindingError::CurrentnessFencePoisoned);
        }
        match input {
            LatchInput::Clear => Ok(()),
            LatchInput::Event => {
                self.poisoned = true;
                Err(DeviceBindingError::WholeGpuResetObserved)
            }
            LatchInput::ProtocolFailure => {
                self.poisoned = true;
                Err(DeviceBindingError::ResetEventFenceProtocol)
            }
        }
    }
}

/// Retained prospective KFD reset-event subscription.
///
/// The active KFD UAPI creates this descriptor without an atomic `CLOEXEC`
/// option. The adapter sets `CLOEXEC` immediately, but a concurrent fork/exec
/// can observe the descriptor in that interval. The descriptor is also created
/// with an empty event mask, so a reset can occur before userspace enables the
/// pre/post-reset bits. These are contracted platform limitations, not atomic
/// subscription or inheritance guarantees.
pub(super) struct ResetEventFence {
    fd: OwnedFd,
    latch: PoisonLatch,
}

impl fmt::Debug for ResetEventFence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResetEventFence")
            .field("poisoned", &self.latch.poisoned)
            .finish_non_exhaustive()
    }
}

impl ResetEventFence {
    pub(super) fn subscribe(kfd_fd: &OwnedFd, gpu_id: u32) -> Result<Self, DeviceBindingError> {
        let mut args = KfdIoctlSmiEventsArgs::new(gpu_id);
        // SAFETY: the opcode and 8-byte in/out layout are pinned by
        // fe2o3-kfd-uapi. `args` remains initialized, exclusively borrowed,
        // and live for the call.
        let request = unsafe { Updater::<SMI_EVENTS_OPCODE, _>::new(&mut args) };
        // SAFETY: the request contract is established above. The returned
        // descriptor is untrusted and range-checked before ownership transfer.
        unsafe { rustix::ioctl::ioctl(kfd_fd, request) }.map_err(|source| {
            DeviceBindingError::Syscall {
                operation: "KFD SMI_EVENTS",
                source,
            }
        })?;

        let raw = i32::try_from(args.anon_fd)
            .map_err(|_| DeviceBindingError::InvalidResetEventDescriptor(args.anon_fd))?;
        // SAFETY: a successful SMI_EVENTS request transfers one new descriptor
        // to userspace. The checked nonnegative value has no other Rust owner.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };

        // The UAPI cannot request this atomically; close the inheritance window
        // at the first possible userspace instruction after taking ownership.
        rustix::io::fcntl_setfd(&fd, FdFlags::CLOEXEC).map_err(|source| {
            DeviceBindingError::Syscall {
                operation: "set reset-event descriptor CLOEXEC",
                source,
            }
        })?;
        let fd_flags =
            rustix::io::fcntl_getfd(&fd).map_err(|source| DeviceBindingError::Syscall {
                operation: "verify reset-event descriptor CLOEXEC",
                source,
            })?;
        if !fd_flags.contains(FdFlags::CLOEXEC) {
            return Err(DeviceBindingError::ResetEventFenceProtocol);
        }

        let mask = KFD_SMI_EVENT_GPU_RESET_MASK.to_ne_bytes();
        let written =
            rustix::io::write(&fd, &mask).map_err(|source| DeviceBindingError::Syscall {
                operation: "enable KFD reset events",
                source,
            })?;
        if written != mask.len() {
            return Err(DeviceBindingError::ResetEventFenceProtocol);
        }

        let mut fence = Self {
            fd,
            latch: PoisonLatch::default(),
        };
        fence.check_clear()?;
        Ok(fence)
    }

    /// Checks without draining the complete successful event record. Once one
    /// byte is observed, the latch remains poisoned, so FIFO overflow cannot
    /// erase the first reset indication from a live fe2o3 token.
    pub(super) fn check_clear(&mut self) -> Result<(), DeviceBindingError> {
        if self.latch.poisoned {
            return Err(DeviceBindingError::CurrentnessFencePoisoned);
        }
        let mut byte = [0_u8; 1];
        let input = match rustix::io::read(&self.fd, &mut byte) {
            Err(error) if error == rustix::io::Errno::AGAIN => LatchInput::Clear,
            Err(source) => {
                self.latch.poisoned = true;
                return Err(DeviceBindingError::Syscall {
                    operation: "read KFD reset-event fence",
                    source,
                });
            }
            Ok(read) if read > 0 => LatchInput::Event,
            Ok(_) => LatchInput::ProtocolFailure,
        };
        self.latch.observe(input)
    }

    /// Checks reset readiness without consuming bytes from the retained FIFO.
    ///
    /// The mapping from readable readiness to a nonempty reset FIFO is a
    /// contract of the pinned KFD source, not proof of the loaded kernel.
    pub(super) fn check_clear_non_draining(&mut self) -> Result<(), DeviceBindingError> {
        if self.latch.poisoned {
            return Err(DeviceBindingError::CurrentnessFencePoisoned);
        }
        let input = match poll_reset_event_readiness(&self.fd) {
            Ok(input) => input,
            Err(error) => {
                self.latch.poisoned = true;
                return Err(error);
            }
        };
        self.latch.observe(input)
    }
}

trait OperationalCurrentnessObservation {
    fn ensure_opener_process(&mut self) -> Result<(), DeviceBindingError>;
    fn check_reset_readiness(&mut self) -> Result<(), DeviceBindingError>;
    fn observe_vram_lost_counter(&mut self) -> Result<u32, DeviceBindingError>;
}

fn check_operational_observations(
    observation: &mut impl OperationalCurrentnessObservation,
    admitted_vram_lost_counter: u32,
) -> Result<(), DeviceBindingError> {
    // The opener check must precede any access to the inherited FIFO.
    observation.ensure_opener_process()?;
    observation.check_reset_readiness()?;
    if observation.observe_vram_lost_counter()? != admitted_vram_lost_counter {
        return Err(DeviceBindingError::ObservableCurrentnessChanged(
            "DRM VRAM-loss counter",
        ));
    }
    observation.check_reset_readiness()
}

struct LinuxOperationalCurrentnessObservation<'a> {
    kfd: &'a crate::OpenedKfd,
    render_fd: &'a OwnedFd,
    reset_fence: &'a mut ResetEventFence,
}

impl OperationalCurrentnessObservation for LinuxOperationalCurrentnessObservation<'_> {
    fn ensure_opener_process(&mut self) -> Result<(), DeviceBindingError> {
        self.kfd
            .ensure_process(std::process::id())
            .map_err(DeviceBindingError::Kfd)
    }

    fn check_reset_readiness(&mut self) -> Result<(), DeviceBindingError> {
        self.reset_fence.check_clear_non_draining()
    }

    fn observe_vram_lost_counter(&mut self) -> Result<u32, DeviceBindingError> {
        crate::linux::observe_vram_lost_counter(self.render_fd)
    }
}

/// A successful composite currentness observation for the retained R1 device.
///
/// This value is **Contracted**, not proof authority and not an all-reset
/// generation. The counter is a wrapping `u32` incremented by the admitted
/// driver only when selected reset recovery paths report VRAM loss. Whole-GPU
/// reset detection comes separately from the retained prospective KFD event
/// descriptor, and topology generation is only compared for equality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservableDeviceCurrentnessV1 {
    vram_lost_counter: u32,
}

/// One KFD-correlated GPU/CPU/system clock observation.
///
/// The counters are sampled by one `GET_CLOCK_COUNTERS` ioctl under selected
/// device currentness checks. This supports clock-domain calibration only; it
/// does not identify dispatch publication, start, or completion boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdClockCorrelationObservationV1 {
    gpu_clock_counter: u64,
    cpu_clock_counter: u64,
    system_clock_counter: u64,
    system_clock_frequency_hz: u64,
    gpu_id: u32,
}

impl KfdClockCorrelationObservationV1 {
    pub const fn gpu_clock_counter(self) -> u64 {
        self.gpu_clock_counter
    }

    pub const fn cpu_clock_counter(self) -> u64 {
        self.cpu_clock_counter
    }

    pub const fn system_clock_counter(self) -> u64 {
        self.system_clock_counter
    }

    pub const fn system_clock_frequency_hz(self) -> u64 {
        self.system_clock_frequency_hz
    }

    pub const fn gpu_id(self) -> u32 {
        self.gpu_id
    }
}

fn admit_clock_correlation(
    raw: fe2o3_kfd_uapi::KfdIoctlGetClockCountersArgs,
    selected_gpu: u32,
) -> Option<KfdClockCorrelationObservationV1> {
    if raw.gpu_id != selected_gpu || raw.pad != 0 || raw.system_clock_freq == 0 {
        return None;
    }
    Some(KfdClockCorrelationObservationV1 {
        gpu_clock_counter: raw.gpu_clock_counter,
        cpu_clock_counter: raw.cpu_clock_counter,
        system_clock_counter: raw.system_clock_counter,
        system_clock_frequency_hz: raw.system_clock_freq,
        gpu_id: raw.gpu_id,
    })
}

impl ObservableDeviceCurrentnessV1 {
    /// Returns the contracted destructive-reset observation.
    ///
    /// It can wrap and must not be interpreted as an all-reset generation.
    pub const fn vram_lost_counter(self) -> u32 {
        self.vram_lost_counter
    }
}

impl CheckedGfx942XnackMinusDevice {
    /// Samples the three KFD clock domains for this exact selected GPU.
    pub fn observe_clock_correlation(
        &mut self,
    ) -> Result<KfdClockCorrelationObservationV1, DeviceBindingError> {
        self.check_operational_currentness()?;
        let selected_gpu = self.observation.kfd_gpu_id();
        let raw = crate::linux::observe_clock_counters(&self.kfd.opened.fd, selected_gpu)?;
        let Some(observation) = admit_clock_correlation(raw, selected_gpu) else {
            self.currentness_poisoned = true;
            return Err(DeviceBindingError::ObservableCurrentnessChanged(
                "KFD clock-counter correlation",
            ));
        };
        self.check_operational_currentness()?;
        Ok(observation)
    }

    pub(crate) fn check_gfx942_xgmi_publication_currentness(
        &mut self,
    ) -> Result<(), DeviceBindingError> {
        if self.currentness_poisoned {
            return Err(DeviceBindingError::CurrentnessFencePoisoned);
        }
        let result = self.check_gfx942_xgmi_publication_currentness_inner();
        if result.is_err() {
            self.currentness_poisoned = true;
        }
        result
    }

    fn check_gfx942_xgmi_publication_currentness_inner(
        &mut self,
    ) -> Result<(), DeviceBindingError> {
        self.kfd
            .opened
            .ensure_process(std::process::id())
            .map_err(DeviceBindingError::Kfd)?;
        let process = crate::linux::observe_process_incarnation()?;
        if process != self.process {
            return Err(DeviceBindingError::ProcessIncarnationChanged);
        }
        self.reset_fence.check_clear()
    }

    pub(crate) fn check_gfx942_xgmi_route_currentness(
        &mut self,
        route: crate::topology::Gfx942XgmiRouteV1,
    ) -> Result<(), DeviceBindingError> {
        if self.currentness_poisoned {
            return Err(DeviceBindingError::CurrentnessFencePoisoned);
        }
        let result = self.check_gfx942_xgmi_route_currentness_inner(route);
        if result.is_err() {
            self.currentness_poisoned = true;
        }
        result
    }

    fn check_gfx942_xgmi_route_currentness_inner(
        &mut self,
        route: crate::topology::Gfx942XgmiRouteV1,
    ) -> Result<(), DeviceBindingError> {
        self.kfd
            .opened
            .ensure_process(std::process::id())
            .map_err(DeviceBindingError::Kfd)?;
        let process_before = crate::linux::observe_process_incarnation()?;
        if process_before != self.process {
            return Err(DeviceBindingError::ProcessIncarnationChanged);
        }
        self.reset_fence.check_clear()?;
        let selected_gpu = self.observation.kfd_gpu_id();
        if !route.canonical_mapping_gpu_ids().contains(&selected_gpu) {
            return Err(DeviceBindingError::ObservableCurrentnessChanged(
                "XGMI selected-device binding",
            ));
        }
        let retained = self
            .topology
            .topology()
            .admit_gfx942_xgmi_route(route.source_gpu_id(), route.destination_gpu_id())
            .map_err(|_| {
                DeviceBindingError::ObservableCurrentnessChanged("retained XGMI topology route")
            })?;
        let observed = crate::topology::discover_default_topology()?;
        let observed_route = observed
            .topology()
            .admit_gfx942_xgmi_route(route.source_gpu_id(), route.destination_gpu_id())
            .map_err(|_| {
                DeviceBindingError::ObservableCurrentnessChanged("observed XGMI topology route")
            })?;
        if retained != route || observed_route != route {
            return Err(DeviceBindingError::ObservableCurrentnessChanged(
                "directional XGMI topology route",
            ));
        }
        let process_after = crate::linux::observe_process_incarnation()?;
        if process_after != process_before || process_after != self.process {
            return Err(DeviceBindingError::ProcessIncarnationChanged);
        }
        self.reset_fence.check_clear()
    }

    /// Reobserves the additive SDMA topology sidecar without changing frozen
    /// base-device equality or admission semantics.
    pub(crate) fn check_gfx942_sdma_topology_capability_currentness(
        &mut self,
    ) -> Result<(), DeviceBindingError> {
        if self.currentness_poisoned {
            return Err(DeviceBindingError::CurrentnessFencePoisoned);
        }
        let result = self.check_gfx942_sdma_topology_capability_currentness_inner();
        if result.is_err() {
            self.currentness_poisoned = true;
        }
        result
    }

    fn check_gfx942_sdma_topology_capability_currentness_inner(
        &mut self,
    ) -> Result<(), DeviceBindingError> {
        self.kfd
            .opened
            .ensure_process(std::process::id())
            .map_err(DeviceBindingError::Kfd)?;
        let process_before = crate::linux::observe_process_incarnation()?;
        if process_before != self.process {
            return Err(DeviceBindingError::ProcessIncarnationChanged);
        }
        self.reset_fence.check_clear()?;

        let observed = crate::topology::discover_default_topology()?;
        if observed != self.topology {
            return Err(DeviceBindingError::TopologySnapshotChanged);
        }
        let selected_unique_id = self.observation.unique_id();
        let retained_inventory = self
            .topology
            .topology()
            .gpu_nodes()
            .iter()
            .find(|gpu| gpu.unique_id() == selected_unique_id)
            .map_or((None, None), |gpu| gpu.sdma_engine_inventory());
        let observed_inventory = observed
            .topology()
            .gpu_nodes()
            .iter()
            .find(|gpu| gpu.unique_id() == selected_unique_id)
            .map_or((None, None), |gpu| gpu.sdma_engine_inventory());
        let expected = (
            Some(fe2o3_kfd_uapi::KFD_GFX942_SDMA_ENGINE_COUNT_V1),
            Some(fe2o3_kfd_uapi::KFD_GFX942_SDMA_QUEUES_PER_ENGINE_V1),
        );
        if retained_inventory != expected || observed_inventory != retained_inventory {
            return Err(DeviceBindingError::ObservableCurrentnessChanged(
                "targeted SDMA topology capability",
            ));
        }

        let process_after = crate::linux::observe_process_incarnation()?;
        if process_after != process_before || process_after != self.process {
            return Err(DeviceBindingError::ProcessIncarnationChanged);
        }
        self.reset_fence.check_clear()
    }

    /// Rechecks the opener process, reset readiness, and the admitted DRM
    /// VRAM-loss counter used by an already-created queue.
    ///
    /// The full composite observation remains mandatory around device, VM,
    /// allocation, mapping, and queue lifecycle transitions. An active queue
    /// uses this bounded fence around ordinary mapped-memory and submission
    /// operations so their cost does not scale with the number of host
    /// topology sysfs files. Topology and aperture equality are therefore
    /// lifecycle observations. The prospective reset stream and wrapping
    /// VRAM-loss counter are the retained-queue hot-path observations.
    pub(crate) fn check_operational_currentness(&mut self) -> Result<(), DeviceBindingError> {
        if self.currentness_poisoned {
            return Err(DeviceBindingError::CurrentnessFencePoisoned);
        }
        let result = self.check_operational_currentness_inner();
        if result.is_err() {
            self.currentness_poisoned = true;
        }
        result
    }

    fn check_operational_currentness_inner(&mut self) -> Result<(), DeviceBindingError> {
        let admitted_vram_lost_counter = self.observation.drm().vram_lost_counter();
        let mut observation = LinuxOperationalCurrentnessObservation {
            kfd: &self.kfd.opened,
            render_fd: &self.render_fd,
            reset_fence: &mut self.reset_fence,
        };
        check_operational_observations(&mut observation, admitted_vram_lost_counter)
    }

    /// Rechecks every retained R1 identity observation under the prospective
    /// whole-GPU reset subscription.
    ///
    /// Any failure permanently poisons this operation for the token. Success
    /// detects all changes observable through the admitted KFD/DRM/sysfs
    /// queries, but cannot exclude unreported engine/per-queue resets, a reset
    /// outside the subscribed KFD paths (including the create-to-mask-enable
    /// interval), counter wrap, or ABA in observations without a monotonic
    /// generation. Kernel support for a retained-device, nonwrapping generation
    /// incremented by every reset class, or an atomic generation-snapshot/event
    /// handshake, is required for an all-reset currentness proof.
    pub fn check_observable_currentness(
        &mut self,
    ) -> Result<ObservableDeviceCurrentnessV1, DeviceBindingError> {
        if self.currentness_poisoned {
            return Err(DeviceBindingError::CurrentnessFencePoisoned);
        }
        let result = self.check_observable_currentness_inner();
        if result.is_err() {
            self.currentness_poisoned = true;
        }
        result
    }

    fn check_observable_currentness_inner(
        &mut self,
    ) -> Result<ObservableDeviceCurrentnessV1, DeviceBindingError> {
        self.kfd
            .opened
            .ensure_process(std::process::id())
            .map_err(DeviceBindingError::Kfd)?;
        let process_before = crate::linux::observe_process_incarnation()?;
        if process_before != self.process {
            return Err(DeviceBindingError::ProcessIncarnationChanged);
        }
        // Validate the opener process before touching the shared kernel FIFO.
        // An inherited child must not be able to consume a reset-event byte.
        self.reset_fence.check_clear()?;

        crate::linux::validate_kfd_descriptor_and_sysfs(
            &self.kfd.opened.fd,
            self.kfd.opened.node_observation(),
        )?;
        crate::linux::revalidate_render_descriptor(
            &self.render_fd,
            self.observation.render_descriptor(),
        )?;

        let uapi = crate::linux::observe_uapi(&self.kfd.opened.fd)?;
        if uapi != self.kfd.uapi.reported_version() {
            return Err(DeviceBindingError::UapiChanged);
        }
        let drm = crate::linux::observe_drm_identity(&self.render_fd)?;
        if drm != self.observation.drm() {
            return Err(DeviceBindingError::ObservableCurrentnessChanged(
                "DRM identity or VRAM-loss counter",
            ));
        }
        if crate::linux::query_xnack_mode(&self.kfd.opened.fd)? != 0 {
            return Err(DeviceBindingError::UnsupportedXnackMode);
        }
        let apertures = validate_apertures(
            crate::linux::observe_process_apertures(&self.kfd.opened.fd)?,
            &self.topology,
        )?;
        if apertures != self.apertures {
            return Err(DeviceBindingError::AperturesChanged);
        }
        let topology = crate::topology::discover_default_topology()?;
        if topology != self.topology {
            return Err(DeviceBindingError::TopologySnapshotChanged);
        }

        crate::linux::revalidate_descriptor(
            &self.kfd.opened.fd,
            self.kfd.opened.node_observation(),
            "KFD currentness fstat",
        )?;
        crate::linux::revalidate_render_descriptor(
            &self.render_fd,
            self.observation.render_descriptor(),
        )?;
        let process_after = crate::linux::observe_process_incarnation()?;
        if process_after != process_before || process_after != self.process {
            return Err(DeviceBindingError::ProcessIncarnationChanged);
        }
        if crate::linux::query_xnack_mode(&self.kfd.opened.fd)? != 0 {
            return Err(DeviceBindingError::UnsupportedXnackMode);
        }
        let drm_after = crate::linux::observe_drm_identity(&self.render_fd)?;
        if drm_after != drm {
            return Err(DeviceBindingError::ObservableCurrentnessChanged(
                "DRM identity or VRAM-loss counter during currentness check",
            ));
        }
        self.reset_fence.check_clear()?;

        Ok(ObservableDeviceCurrentnessV1 {
            vram_lost_counter: drm_after.vram_lost_counter(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct ScriptedOperationalObservation {
        steps: Vec<&'static str>,
        fail_at: Option<&'static str>,
        reset_checks: usize,
        vram_lost_counter: u32,
    }

    impl OperationalCurrentnessObservation for ScriptedOperationalObservation {
        fn ensure_opener_process(&mut self) -> Result<(), DeviceBindingError> {
            self.steps.push("pid");
            if self.fail_at == Some("pid") {
                Err(DeviceBindingError::ProcessIncarnationChanged)
            } else {
                Ok(())
            }
        }

        fn check_reset_readiness(&mut self) -> Result<(), DeviceBindingError> {
            self.reset_checks += 1;
            let step = if self.reset_checks == 1 {
                "reset-before"
            } else {
                "reset-after"
            };
            self.steps.push(step);
            if self.fail_at == Some(step) {
                Err(DeviceBindingError::ResetEventFenceProtocol)
            } else {
                Ok(())
            }
        }

        fn observe_vram_lost_counter(&mut self) -> Result<u32, DeviceBindingError> {
            self.steps.push("vram");
            if self.fail_at == Some("vram") {
                Err(DeviceBindingError::ObservableCurrentnessChanged(
                    "injected VRAM query failure",
                ))
            } else {
                Ok(self.vram_lost_counter)
            }
        }
    }

    #[test]
    fn clear_observations_do_not_poison() {
        let mut latch = PoisonLatch::default();
        for _ in 0..32 {
            assert!(latch.observe(LatchInput::Clear).is_ok());
        }
        assert!(!latch.poisoned);
    }

    #[test]
    fn first_event_is_permanently_poisoning() {
        let mut latch = PoisonLatch::default();
        assert!(matches!(
            latch.observe(LatchInput::Event),
            Err(DeviceBindingError::WholeGpuResetObserved)
        ));
        for input in [
            LatchInput::Clear,
            LatchInput::Event,
            LatchInput::ProtocolFailure,
        ] {
            assert!(matches!(
                latch.observe(input),
                Err(DeviceBindingError::CurrentnessFencePoisoned)
            ));
        }
    }

    #[test]
    fn protocol_failure_is_permanently_poisoning() {
        let mut latch = PoisonLatch::default();
        assert!(matches!(
            latch.observe(LatchInput::ProtocolFailure),
            Err(DeviceBindingError::ResetEventFenceProtocol)
        ));
        assert!(matches!(
            latch.observe(LatchInput::Clear),
            Err(DeviceBindingError::CurrentnessFencePoisoned)
        ));
    }

    #[test]
    fn non_draining_poll_classifier_accepts_only_clear_or_readable() {
        assert_eq!(
            classify_non_draining_reset_poll(0, PollFlags::empty()),
            LatchInput::Clear
        );
        for readable in [
            PollFlags::IN,
            PollFlags::RDNORM,
            PollFlags::IN | PollFlags::RDNORM,
        ] {
            assert_eq!(
                classify_non_draining_reset_poll(1, readable),
                LatchInput::Event
            );
        }
    }

    #[test]
    fn non_draining_poll_classifier_rejects_errors_and_protocol_anomalies() {
        for (ready, events) in [
            (0, PollFlags::IN),
            (1, PollFlags::empty()),
            (2, PollFlags::IN),
            (1, PollFlags::ERR),
            (1, PollFlags::HUP),
            (1, PollFlags::NVAL),
            (1, PollFlags::OUT),
            (1, PollFlags::PRI),
            (1, PollFlags::RDHUP),
            (1, PollFlags::IN | PollFlags::ERR),
            (1, PollFlags::RDNORM | PollFlags::HUP),
        ] {
            assert_eq!(
                classify_non_draining_reset_poll(ready, events),
                LatchInput::ProtocolFailure,
                "ready={ready}, events={events:?}"
            );
        }
    }

    #[test]
    fn non_draining_poll_syscall_errors_fail_closed_without_retry() {
        for source in [
            rustix::io::Errno::INTR,
            rustix::io::Errno::AGAIN,
            rustix::io::Errno::BADF,
        ] {
            assert!(matches!(
                finish_non_draining_reset_poll(Err(source), PollFlags::empty()),
                Err(DeviceBindingError::Syscall {
                    operation: "poll KFD reset-event fence",
                    source: observed,
                }) if observed == source
            ));
        }
    }

    #[test]
    fn linux_readiness_probe_does_not_drain_a_readable_descriptor() {
        use rustix::pipe::{PipeFlags, pipe_with};

        let (reader, writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
        assert_eq!(
            poll_reset_event_readiness(&reader).unwrap(),
            LatchInput::Clear
        );
        assert_eq!(rustix::io::write(&writer, b"r").unwrap(), 1);
        assert_eq!(
            poll_reset_event_readiness(&reader).unwrap(),
            LatchInput::Event
        );
        assert_eq!(
            poll_reset_event_readiness(&reader).unwrap(),
            LatchInput::Event
        );

        let mut byte = [0_u8; 1];
        assert_eq!(rustix::io::read(&reader, &mut byte).unwrap(), 1);
        assert_eq!(byte, *b"r");
        assert_eq!(
            poll_reset_event_readiness(&reader).unwrap(),
            LatchInput::Clear
        );
    }

    #[test]
    fn linux_readiness_probe_and_latch_fail_closed_on_hangup_and_stay_poisoned() {
        use rustix::pipe::{PipeFlags, pipe_with};

        let (reader, writer) = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).unwrap();
        drop(writer);
        let mut fence = ResetEventFence {
            fd: reader,
            latch: PoisonLatch::default(),
        };
        assert!(matches!(
            fence.check_clear_non_draining(),
            Err(DeviceBindingError::ResetEventFenceProtocol)
        ));
        assert!(matches!(
            fence.check_clear_non_draining(),
            Err(DeviceBindingError::CurrentnessFencePoisoned)
        ));
    }

    #[test]
    fn operational_observations_preserve_pid_reset_counter_reset_order() {
        let mut observation = ScriptedOperationalObservation {
            vram_lost_counter: 17,
            ..ScriptedOperationalObservation::default()
        };
        check_operational_observations(&mut observation, 17).unwrap();
        assert_eq!(
            observation.steps,
            ["pid", "reset-before", "vram", "reset-after"]
        );
    }

    #[test]
    fn operational_observations_stop_before_fifo_after_process_change() {
        let mut observation = ScriptedOperationalObservation {
            fail_at: Some("pid"),
            ..ScriptedOperationalObservation::default()
        };
        assert!(matches!(
            check_operational_observations(&mut observation, 0),
            Err(DeviceBindingError::ProcessIncarnationChanged)
        ));
        assert_eq!(observation.steps, ["pid"]);
    }

    #[test]
    fn operational_observations_fail_closed_at_each_live_boundary() {
        for (failure, expected_steps) in [
            ("reset-before", &[][..]),
            ("vram", &["vram"][..]),
            ("reset-after", &["vram", "reset-after"][..]),
        ] {
            let mut observation = ScriptedOperationalObservation {
                fail_at: Some(failure),
                ..ScriptedOperationalObservation::default()
            };
            assert!(check_operational_observations(&mut observation, 0).is_err());
            assert_eq!(&observation.steps[2..], expected_steps, "failure={failure}");
        }
    }

    #[test]
    fn operational_observations_reject_counter_change_before_closing_probe() {
        let mut observation = ScriptedOperationalObservation {
            vram_lost_counter: 18,
            ..ScriptedOperationalObservation::default()
        };
        assert!(matches!(
            check_operational_observations(&mut observation, 17),
            Err(DeviceBindingError::ObservableCurrentnessChanged(
                "DRM VRAM-loss counter"
            ))
        ));
        assert_eq!(observation.steps, ["pid", "reset-before", "vram"]);
    }

    #[test]
    fn clock_correlation_requires_exact_gpu_zero_pad_and_frequency() {
        let raw = fe2o3_kfd_uapi::KfdIoctlGetClockCountersArgs {
            gpu_clock_counter: 11,
            cpu_clock_counter: 12,
            system_clock_counter: 13,
            system_clock_freq: 1_000_000_000,
            gpu_id: 7,
            pad: 0,
        };
        let admitted = admit_clock_correlation(raw, 7).unwrap();
        assert_eq!(admitted.gpu_clock_counter(), 11);
        assert_eq!(admitted.cpu_clock_counter(), 12);
        assert_eq!(admitted.system_clock_counter(), 13);
        assert_eq!(admitted.system_clock_frequency_hz(), 1_000_000_000);
        assert_eq!(admitted.gpu_id(), 7);
        assert!(
            admit_clock_correlation(
                fe2o3_kfd_uapi::KfdIoctlGetClockCountersArgs { gpu_id: 8, ..raw },
                7
            )
            .is_none()
        );
        assert!(
            admit_clock_correlation(
                fe2o3_kfd_uapi::KfdIoctlGetClockCountersArgs { pad: 1, ..raw },
                7
            )
            .is_none()
        );
        assert!(
            admit_clock_correlation(
                fe2o3_kfd_uapi::KfdIoctlGetClockCountersArgs {
                    system_clock_freq: 0,
                    ..raw
                },
                7
            )
            .is_none()
        );

        type QueueClockObserver =
            fn(
                &mut crate::ComputeAqlQueueSessionV1,
            )
                -> Result<KfdClockCorrelationObservationV1, crate::ComputeAqlQueueSessionErrorV1>;
        let _: QueueClockObserver = crate::ComputeAqlQueueSessionV1::observe_clock_correlation;
    }
}
