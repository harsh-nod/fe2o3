use super::*;

struct Fake {
    calls: Vec<&'static str>,
    fail_at: Option<usize>,
    poisoned: bool,
    raw: KfdIoctlGetClockCountersArgs,
}

impl Default for Fake {
    fn default() -> Self {
        Self {
            calls: vec![],
            fail_at: None,
            poisoned: false,
            raw: KfdIoctlGetClockCountersArgs {
                gpu_clock_counter: 11,
                cpu_clock_counter: 12,
                system_clock_counter: 13,
                system_clock_freq: 1_000_000_000,
                gpu_id: 7,
                pad: 0,
            },
        }
    }
}

impl Fake {
    fn step(&mut self, name: &'static str) -> Result<(), DeviceBindingError> {
        self.calls.push(name);
        if self.poisoned {
            Err(DeviceBindingError::CurrentnessFencePoisoned)
        } else if self.fail_at == Some(self.calls.len() - 1) {
            Err(DeviceBindingError::ObservableCurrentnessChanged(
                "injected clock failure",
            ))
        } else {
            Ok(())
        }
    }
}

impl ClockBackend for Fake {
    fn check(&mut self) -> Result<(), DeviceBindingError> {
        self.step("check")
    }
    fn gpu_id(&self) -> u32 {
        7
    }
    fn read(&mut self, gpu_id: u32) -> Result<KfdIoctlGetClockCountersArgs, DeviceBindingError> {
        assert_eq!(gpu_id, 7);
        self.step("ioctl")?;
        Ok(self.raw)
    }
    fn poison(&mut self) {
        self.poisoned = true;
    }
}

#[test]
fn clock_raw_values_preserved_under_two_full_checks() {
    let mut fake = Fake::default();
    let value = sample(&mut fake).unwrap();
    assert_eq!(fake.calls, ["check", "ioctl", "check"]);
    assert_eq!(value.gpu_id(), 7);
    assert_eq!(value.gpu_clock_counter(), 11);
    assert_eq!(value.cpu_clock_counter(), 12);
    assert_eq!(value.system_clock_counter(), 13);
    assert_eq!(value.system_clock_frequency_hz(), 1_000_000_000);
    assert!(!fake.poisoned);
}

#[test]
fn clock_entry_ioctl_and_exit_failure_latch_poison() {
    for at in 0..3 {
        let mut fake = Fake {
            fail_at: Some(at),
            ..Fake::default()
        };
        assert!(matches!(
            sample(&mut fake),
            Err(DeviceBindingError::ObservableCurrentnessChanged(
                "injected clock failure"
            ))
        ));
        assert_eq!(fake.calls, ["check", "ioctl", "check"][..=at]);
        assert!(fake.poisoned);
        fake.fail_at = None;
        assert!(matches!(
            sample(&mut fake),
            Err(DeviceBindingError::CurrentnessFencePoisoned)
        ));
        assert_eq!(fake.calls.last(), Some(&"check"));
        assert_eq!(fake.calls.len(), at + 2);
    }
}

#[test]
fn clock_wrong_gpu_padding_and_zero_frequency_poison() {
    for kind in 0..3 {
        let mut fake = Fake::default();
        match kind {
            0 => fake.raw.gpu_id = 8,
            1 => fake.raw.pad = 1,
            _ => fake.raw.system_clock_freq = 0,
        }
        assert!(matches!(
            sample(&mut fake),
            Err(DeviceBindingError::ObservableCurrentnessChanged(
                "KFD clock-counter correlation"
            ))
        ));
        assert_eq!(fake.calls, ["check", "ioctl"]);
        assert!(fake.poisoned);
    }
}

#[test]
fn clock_already_poisoned_refuses_before_ioctl() {
    let mut fake = Fake {
        poisoned: true,
        ..Fake::default()
    };
    assert!(matches!(
        sample(&mut fake),
        Err(DeviceBindingError::CurrentnessFencePoisoned)
    ));
    assert_eq!(fake.calls, ["check"]);
}

#[test]
fn clock_repeated_samples_are_fresh_without_counter_order_assumptions() {
    let mut fake = Fake::default();
    let first = sample(&mut fake).unwrap();
    fake.raw.gpu_clock_counter = 0;
    fake.raw.cpu_clock_counter = u64::MAX;
    fake.raw.system_clock_counter = 0;
    let second = sample(&mut fake).unwrap();
    assert_eq!(
        fake.calls,
        ["check", "ioctl", "check", "check", "ioctl", "check"]
    );
    assert_eq!(first.gpu_clock_counter(), 11);
    assert_eq!(second.gpu_clock_counter(), 0);
    assert_eq!(second.cpu_clock_counter(), u64::MAX);
    assert_eq!(second.system_clock_counter(), 0);
    assert!(!fake.poisoned);
}

#[test]
fn clock_public_sampler_signatures_remain_target_specific() {
    let _: fn(
        &mut crate::CheckedGfx950XnackMinusDevice,
    ) -> Result<KfdClockCorrelationObservationV1, DeviceBindingError> =
        crate::CheckedGfx950XnackMinusDevice::observe_clock_correlation;
    let _: fn(
        &mut crate::CheckedGfx942XnackMinusDevice,
    ) -> Result<KfdClockCorrelationObservationV1, DeviceBindingError> =
        crate::CheckedGfx942XnackMinusDevice::observe_clock_correlation;
}
