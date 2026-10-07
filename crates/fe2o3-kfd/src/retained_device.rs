//! Immutable device observation without extracting or replacing native custody.

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use crate::{CheckedGfx942XnackMinusDevice, DeviceBindingError};

pub(crate) trait RetainedDeviceScopeOwnerV1 {
    type Subject: ?Sized;
    type Error;

    fn check_scope(&mut self) -> Result<(), Self::Error>;
    fn subject(&self) -> &Self::Subject;
    /// Must not panic or discard native custody.
    fn poison_scope(&mut self);
}

fn check<O: RetainedDeviceScopeOwnerV1>(owner: &mut O) -> Result<(), O::Error> {
    match catch_unwind(AssertUnwindSafe(|| owner.check_scope())) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            owner.poison_scope();
            Err(error)
        }
        Err(payload) => {
            owner.poison_scope();
            resume_unwind(payload)
        }
    }
}

pub(crate) fn with_retained_device_scope_v1<O: RetainedDeviceScopeOwnerV1, R>(
    owner: &mut O,
    observe: impl FnOnce(&O::Subject) -> R,
) -> Result<R, O::Error> {
    check(owner)?;
    let result = match catch_unwind(AssertUnwindSafe(|| observe(owner.subject()))) {
        Ok(result) => result,
        Err(payload) => {
            owner.poison_scope();
            resume_unwind(payload)
        }
    };
    // Keep the candidate outside the check's unwind boundary. Poisoning must
    // precede its disposal if the closing observation fails or panics.
    check(owner)?;
    Ok(result)
}

impl RetainedDeviceScopeOwnerV1 for CheckedGfx942XnackMinusDevice {
    type Subject = Self;
    type Error = DeviceBindingError;

    fn check_scope(&mut self) -> Result<(), Self::Error> {
        self.check_observable_currentness().map(|_| ())
    }

    fn subject(&self) -> &Self {
        self
    }

    fn poison_scope(&mut self) {
        self.currentness_poisoned = true;
    }
}

impl CheckedGfx942XnackMinusDevice {
    /// Observes the retained device inside a full currentness envelope.
    ///
    /// The callback cannot replace or consume the device. Its result must not
    /// borrow the device, and conveys no execution or future-currentness authority.
    /// A callback error value still receives the closing check. Currentness
    /// failure or unwind permanently poisons this owner; unwind performs no
    /// additional fallible observation. No VM, allocation or queue is created.
    pub fn with_retained_device_v1<R>(
        &mut self,
        observe: impl FnOnce(&Self) -> R,
    ) -> Result<R, DeviceBindingError> {
        with_retained_device_scope_v1(self, observe)
    }
}

#[cfg(test)]
mod tests;
