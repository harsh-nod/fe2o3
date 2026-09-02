//! Contracted currentness observations for an admitted device token.

use std::fmt;
use std::os::fd::FromRawFd;

use fe2o3_kfd_uapi::{AMDKFD_IOC_SMI_EVENTS, KFD_SMI_EVENT_GPU_RESET_MASK, KfdIoctlSmiEventsArgs};
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

impl ObservableDeviceCurrentnessV1 {
    /// Returns the contracted destructive-reset observation.
    ///
    /// It can wrap and must not be interpreted as an all-reset generation.
    pub const fn vram_lost_counter(self) -> u32 {
        self.vram_lost_counter
    }
}

impl CheckedGfx942XnackMinusDevice {
    /// Rechecks the retained process, descriptors, UAPI mode, reset stream,
    /// and DRM reset observation used by an already-created queue.
    ///
    /// The full composite observation remains mandatory around device, VM,
    /// allocation, mapping, and queue lifecycle transitions. An active queue
    /// uses this bounded fence around ordinary mapped-memory and submission
    /// operations so their cost does not scale with the number of host
    /// topology sysfs files. Topology and aperture equality are therefore
    /// lifecycle observations; reset and descriptor loss remain hot-path
    /// observations.
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
        self.kfd
            .opened
            .ensure_process(std::process::id())
            .map_err(DeviceBindingError::Kfd)?;
        let process = crate::linux::observe_process_incarnation()?;
        if process != self.process {
            return Err(DeviceBindingError::ProcessIncarnationChanged);
        }

        // Check the prospective stream before and after the retained identity
        // observations so a reset concurrent with this scope is latched.
        self.reset_fence.check_clear()?;
        crate::linux::revalidate_descriptor(
            &self.kfd.opened.fd,
            self.kfd.opened.node_observation(),
            "KFD operational currentness fstat",
        )?;
        crate::linux::revalidate_render_descriptor(
            &self.render_fd,
            self.observation.render_descriptor(),
        )?;
        if crate::linux::observe_uapi(&self.kfd.opened.fd)? != self.kfd.uapi.reported_version() {
            return Err(DeviceBindingError::UapiChanged);
        }
        if crate::linux::query_xnack_mode(&self.kfd.opened.fd)? != 0 {
            return Err(DeviceBindingError::UnsupportedXnackMode);
        }
        if crate::linux::observe_drm_identity(&self.render_fd)? != self.observation.drm() {
            return Err(DeviceBindingError::ObservableCurrentnessChanged(
                "DRM identity or VRAM-loss counter",
            ));
        }
        self.reset_fence.check_clear()
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
}
