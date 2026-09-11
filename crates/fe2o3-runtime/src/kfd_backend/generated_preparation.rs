//! Nonexecuting access to the actual retained device, including lazy bootstrap.

use super::*;

impl KfdRuntimeBackendV1 {
    pub(crate) fn with_retained_preparation_device_v1<R>(
        &mut self,
        backend_device: u64,
        prepare: impl FnOnce(&CheckedGfx942XnackMinusDevice) -> R,
    ) -> Result<R, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if self.queue_retired {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Terminal,
                "retained-device preparation requires a live backend",
            ));
        }
        if backend_device != self.description.backend_device || !self.native_available {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "retained-device preparation requires the exact native device",
            ));
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let prepare = |device: &CheckedGfx942XnackMinusDevice| {
                if device.observation().unique_id() != backend_device {
                    Err("retained device does not match the backend description")
                } else {
                    Ok(prepare(device))
                }
            };
            match (&mut self.admitted_device, &mut self.queue) {
                (Some(device), None) => device
                    .with_retained_device_v1(prepare)
                    .map_err(|error| error.to_string())
                    .and_then(|result| result.map_err(str::to_owned)),
                (None, Some(queue)) => queue
                    .with_retained_device_v1(prepare)
                    .map_err(|error| error.to_string())
                    .and_then(|result| result.map_err(str::to_owned)),
                _ => Err("missing or duplicated retained-device owner".to_owned()),
            }
        }));
        match result {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(self.terminal_error(format!("KFD preparation scope: {error}"))),
            Err(payload) => {
                // Native scopes already poison their exact owner before unwinding.
                self.terminal = true;
                if let Some(queue) = self.queue.as_mut() {
                    queue.poison_after_runtime_owner_failure_v1();
                }
                std::panic::resume_unwind(payload)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparation_backend_rejects_terminal_retired_synthetic_and_foreign_before_callback() {
        for mode in 0..4 {
            let mut backend = KfdRuntimeBackendV1::mock();
            if mode == 0 {
                backend.terminal = true;
            }
            if mode == 1 {
                backend.queue_retired = true;
            }
            let device = if mode == 3 { 8 } else { 7 };
            let result = backend
                .with_retained_preparation_device_v1(device, |_| panic!("rejected callback"));
            if mode == 0 {
                assert!(matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))));
            } else {
                assert!(matches!(result, Err(RuntimeBackendFailureV1::Rejected(_))));
            }
            assert!(backend.queue.is_none());
            assert!(backend.admitted_device.is_none());
            assert_eq!(backend.next_handle, 1);
            if mode == 0 {
                // Terminal backends must be retained even in a no-device fixture.
                core::mem::forget(backend);
            }
        }
    }
}
