//! A consuming observation of the original no-VM device, never a queue fault.

#![forbid(unsafe_code)]

use std::any::Any;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_runtime_model::ModelDeviceAdmissionV1;

use crate::{
    CheckedGfx942XnackMinusDevice, DeviceBindingError, DeviceBindingObservation,
    ProcessIncarnationObservation,
};

/// Result of checking one original device before its first VM activation.
///
/// Only `Ready` returns an activatable token. `ResetObserved` retains a
/// permanently unavailable original device; it is not packet cancellation,
/// native resource retirement, or proof that any other device remains healthy.
/// Every caller must preserve the existing process-global runtime gate and
/// independently validate other owners. No queue-terminal error is converted.
#[derive(Debug)]
pub enum Gfx942ColdDeviceCheckV1 {
    Ready(CheckedGfx942XnackMinusDevice),
    ResetObserved(Gfx942ColdDeviceResetV1),
    Refused(Gfx942ColdDeviceRefusalV1),
}

/// The original no-VM token after a newly observed subscribed whole-GPU reset.
///
/// This is move-only and has no conversion back to a checked device. It retains
/// the original descriptors and model generation, not a reconstructed identity.
/// Dropping it uses the existing no-VM device/model drop behavior, not native
/// cleanup or a process-global poison reset. Reset-event coverage remains the
/// contracted, prospective coverage of the original device admission.
///
/// ```compile_fail
/// fn duplicate(owner: &fe2o3_kfd::Gfx942ColdDeviceResetV1)
///     -> fe2o3_kfd::Gfx942ColdDeviceResetV1 {
///     owner.clone()
/// }
/// ```
///
/// ```compile_fail
/// fn reactivate(owner: fe2o3_kfd::Gfx942ColdDeviceResetV1)
///     -> fe2o3_kfd::CheckedGfx942XnackMinusDevice {
///     owner.into_device()
/// }
/// ```
#[derive(Debug)]
pub struct Gfx942ColdDeviceResetV1 {
    device: CheckedGfx942XnackMinusDevice,
}

impl Gfx942ColdDeviceResetV1 {
    /// Rechecks only the original opener/process occurrence. Success grants no
    /// device reactivation, native settlement, or process-global gate reset.
    /// Refusal leaves this permanently unavailable original owner retained.
    pub fn check_process(&self) -> Result<(), DeviceBindingError> {
        self.device.check_process()
    }

    /// Cached inert identity; not a replacement device or execution capability.
    pub const fn observation(&self) -> &DeviceBindingObservation {
        self.device.observation()
    }

    /// The original retained model admission, not a new generation.
    pub const fn model_admission(&self) -> ModelDeviceAdmissionV1 {
        self.device.model_admission()
    }

    pub const fn process_incarnation(&self) -> ProcessIncarnationObservation {
        self.device.process_incarnation()
    }

    pub fn retained_descriptor_count(&self) -> usize {
        self.device.descriptor_count()
    }
}

/// A refusal cannot be used as evidence of a device-local reset or settlement.
#[derive(Debug)]
pub enum Gfx942ColdDeviceRefusalReasonV1 {
    /// The private no-VM condition was not present. No observation was made.
    NotBeforeFirstActivation,
    /// This owner had already failed an earlier currentness observation.
    AlreadyPoisoned,
    /// Includes a failed closing process check after observing a reset.
    Observation(DeviceBindingError),
    /// The exact panic payload remains owned privately until this owner drops.
    ObservationPanicked,
}

/// Original device custody after an unclassified cold-observation refusal.
///
/// There is no retry, extraction, or conversion to a local-reset owner. This
/// does not weaken any existing process-wide terminal/quarantine requirement.
pub struct Gfx942ColdDeviceRefusalV1 {
    device: CheckedGfx942XnackMinusDevice,
    reason: Gfx942ColdDeviceRefusalReasonV1,
    _panic: Option<Box<dyn Any + Send>>,
}

impl fmt::Debug for Gfx942ColdDeviceRefusalV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ColdDeviceRefusalV1")
            .field("device", &self.device)
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

impl Gfx942ColdDeviceRefusalV1 {
    pub const fn reason(&self) -> &Gfx942ColdDeviceRefusalReasonV1 {
        &self.reason
    }

    pub const fn observation(&self) -> &DeviceBindingObservation {
        self.device.observation()
    }

    pub const fn model_admission(&self) -> ModelDeviceAdmissionV1 {
        self.device.model_admission()
    }

    pub fn retained_descriptor_count(&self) -> usize {
        self.device.descriptor_count()
    }
}

// Only the actual retained no-VM token implements this outside CPU tests.
trait ColdOwner {
    fn before_first_activation(&self) -> bool;
    fn poisoned(&self) -> bool;
    fn poison(&mut self);
    fn observe(&mut self) -> Result<(), DeviceBindingError>;
    fn closing_process(&self) -> Result<(), DeviceBindingError>;
}

enum Checked<O> {
    Ready(O),
    Reset(O),
    Refused {
        owner: O,
        reason: Gfx942ColdDeviceRefusalReasonV1,
        panic: Option<Box<dyn Any + Send>>,
    },
}

fn check<O: ColdOwner>(mut owner: O) -> Checked<O> {
    let refusal = if !owner.before_first_activation() {
        Some(Gfx942ColdDeviceRefusalReasonV1::NotBeforeFirstActivation)
    } else if owner.poisoned() {
        Some(Gfx942ColdDeviceRefusalReasonV1::AlreadyPoisoned)
    } else {
        None
    };
    if let Some(reason) = refusal {
        owner.poison();
        return Checked::Refused {
            owner,
            reason,
            panic: None,
        };
    }
    let result = catch_unwind(AssertUnwindSafe(|| match owner.observe() {
        Ok(()) => Ok(false),
        Err(DeviceBindingError::WholeGpuResetObserved) => {
            // A reset must not hide a changed/forked opener. The exact original
            // subscription is inside observe; no caller-supplied error enters.
            owner.closing_process()?;
            Ok(true)
        }
        Err(error) => Err(error),
    }));
    if matches!(result, Ok(Ok(false))) {
        return Checked::Ready(owner);
    }
    // Latch before returning failure or moving any panic payload. Production
    // poison is an infallible field assignment, with no cleanup side effects.
    owner.poison();
    match result {
        Ok(Ok(true)) => Checked::Reset(owner),
        Ok(Err(error)) => Checked::Refused {
            owner,
            reason: Gfx942ColdDeviceRefusalReasonV1::Observation(error),
            panic: None,
        },
        Err(payload) => Checked::Refused {
            owner,
            reason: Gfx942ColdDeviceRefusalReasonV1::ObservationPanicked,
            panic: Some(payload),
        },
        Ok(Ok(false)) => Checked::Ready(owner),
    }
}

impl ColdOwner for CheckedGfx942XnackMinusDevice {
    fn before_first_activation(&self) -> bool {
        self.retire_model_on_drop
    }

    fn poisoned(&self) -> bool {
        self.currentness_poisoned
    }

    fn poison(&mut self) {
        self.currentness_poisoned = true;
    }

    fn observe(&mut self) -> Result<(), DeviceBindingError> {
        self.check_observable_currentness().map(|_| ())
    }

    fn closing_process(&self) -> Result<(), DeviceBindingError> {
        self.check_process()
    }
}

impl CheckedGfx942XnackMinusDevice {
    /// Consumes the original token through its full existing currentness check.
    ///
    /// This is a no-VM, no-allocation, no-queue boundary. Only a newly observed
    /// event on this token's original reset subscription, followed by another
    /// successful original-process check, produces `ResetObserved`. Other
    /// failures and panics retain a non-reusable refusal owner. It neither
    /// checks nor resets the process-global runtime gate and does not certify
    /// healthy-child continuation. All existing activation checks still apply.
    pub fn check_before_first_native_activation_v1(self) -> Gfx942ColdDeviceCheckV1 {
        match check(self) {
            Checked::Ready(device) => Gfx942ColdDeviceCheckV1::Ready(device),
            Checked::Reset(device) => {
                Gfx942ColdDeviceCheckV1::ResetObserved(Gfx942ColdDeviceResetV1 { device })
            }
            Checked::Refused {
                owner: device,
                reason,
                panic,
            } => Gfx942ColdDeviceCheckV1::Refused(Gfx942ColdDeviceRefusalV1 {
                device,
                reason,
                _panic: panic,
            }),
        }
    }
}

#[cfg(test)]
mod tests;
