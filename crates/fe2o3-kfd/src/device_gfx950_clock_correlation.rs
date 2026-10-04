//! Raw KFD counter observations on the retained gfx950 device descriptor.

use super::*;
use crate::currentness::{KfdClockCorrelationObservationV1, admit_clock_correlation};
use fe2o3_kfd_uapi::KfdIoctlGetClockCountersArgs;

trait ClockBackend {
    fn check(&mut self) -> Result<(), DeviceBindingError>;
    fn gpu_id(&self) -> u32;
    fn read(&mut self, gpu_id: u32) -> Result<KfdIoctlGetClockCountersArgs, DeviceBindingError>;
    fn poison(&mut self);
}

fn sample(
    backend: &mut impl ClockBackend,
) -> Result<KfdClockCorrelationObservationV1, DeviceBindingError> {
    let result = (|| {
        backend.check()?;
        let gpu_id = backend.gpu_id();
        let raw = backend.read(gpu_id)?;
        let observation = admit_clock_correlation(raw, gpu_id).ok_or(
            DeviceBindingError::ObservableCurrentnessChanged("KFD clock-counter correlation"),
        )?;
        backend.check()?;
        Ok(observation)
    })();
    if result.is_err() {
        backend.poison();
    }
    result
}

struct NativeClock<'a>(&'a mut CheckedGfx950XnackMinusDevice);

impl ClockBackend for NativeClock<'_> {
    fn check(&mut self) -> Result<(), DeviceBindingError> {
        self.0.check_observable_currentness()
    }

    fn gpu_id(&self) -> u32 {
        self.0.observation.kfd_gpu_id()
    }

    fn read(&mut self, gpu_id: u32) -> Result<KfdIoctlGetClockCountersArgs, DeviceBindingError> {
        crate::linux::observe_clock_counters(&self.0.kfd.opened.fd, gpu_id)
    }

    fn poison(&mut self) {
        self.0.currentness_poisoned = true;
    }
}

impl CheckedGfx950XnackMinusDevice {
    /// Samples raw KFD GPU, CPU and system counters for this retained device.
    /// Full observable-currentness checks bracket the owned-descriptor ioctl.
    /// Any check, syscall or response failure permanently poisons this token.
    ///
    /// The reported frequency belongs to the system counter, not the GPU
    /// counter. This observation supplies no dispatch timing, clock-domain
    /// equivalence, calibration, wrap/reset handling or cross-device alignment.
    pub fn observe_clock_correlation(
        &mut self,
    ) -> Result<KfdClockCorrelationObservationV1, DeviceBindingError> {
        sample(&mut NativeClock(self))
    }
}

#[cfg(test)]
#[path = "device_gfx950_clock_correlation_tests.rs"]
mod tests;
